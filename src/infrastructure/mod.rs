//! 基礎設施轉接器與確定性的設定輔助工具。

use crate::application::{GameRecord, RecordedDecision, ReplayVerificationError, replay};
use crate::domain::{
    CardInstanceId, GameConclusion, GameEndCause, GameError, GameEvent, GameOutcome, GameSetup,
    GameState, RulesetId, ValidationError,
};
use crate::ports::{DeckPreparation, EventLogStorage, GameRecordRepository, SnapshotStorage};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::convert::Infallible;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PersistenceMetadata {
    pub ruleset_id: String,
    pub engine_version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PersistedGameRecord {
    pub metadata: PersistenceMetadata,
    pub setup: GameSetup,
    pub recorded_decisions: Vec<RecordedDecision>,
}

impl PersistedGameRecord {
    pub fn from_record(metadata: PersistenceMetadata, record: &GameRecord) -> Self {
        let mut metadata = metadata;
        metadata.ruleset_id = record.setup().ruleset.as_str().to_string();

        Self {
            metadata,
            setup: record.setup().clone(),
            recorded_decisions: record.recorded_decisions(),
        }
    }

    pub fn replay(&self) -> Result<GameState, GameError> {
        let metadata_ruleset = RulesetId::new(self.metadata.ruleset_id.clone());
        if self.setup.ruleset != metadata_ruleset {
            return Err(GameError::Validation(ValidationError::RulesetMismatch {
                setup: self.setup.ruleset.clone(),
                metadata: metadata_ruleset,
            }));
        }

        let events = self
            .recorded_decisions
            .iter()
            .flat_map(|decision| decision.events.iter().cloned())
            .collect::<Vec<_>>();
        replay(&self.setup, &events)
    }

    pub fn to_record(&self) -> Result<GameRecord, PersistedGameRecordLoadError> {
        self.replay()?;
        GameRecord::from_recorded_decisions(self.setup.clone(), self.recorded_decisions.clone())
            .map_err(PersistedGameRecordLoadError::Replay)
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut value = serde_json::from_str(json)?;
        migrate_legacy_wire_format(&mut value);
        let mut persisted = serde_json::from_value::<Self>(value)?;
        persisted.migrate_legacy_terminal_events();
        Ok(persisted)
    }

    fn migrate_legacy_terminal_events(&mut self) {
        let mut state = GameState::from_setup(&self.setup);
        let mut finished = false;
        for decision in &mut self.recorded_decisions {
            for event in &decision.events {
                crate::rules::projection::apply_event(&mut state, event);
                finished |= matches!(event, GameEvent::GameEnded { .. });
            }
            if finished {
                continue;
            }
            let direct_victory = decision.events.iter().rev().find_map(|event| match event {
                GameEvent::FiveStarAlignmentAchieved { team, .. } => Some(GameConclusion::new(
                    GameOutcome::Winner(team.clone()),
                    vec![GameEndCause::DirectVictory {
                        rule: "five-star-alignment".to_string(),
                        team: team.clone(),
                    }],
                )),
                GameEvent::KingYamaDecreeVictoryAchieved { team, .. } => Some(GameConclusion::new(
                    GameOutcome::Winner(team.clone()),
                    vec![GameEndCause::DirectVictory {
                        rule: "king-yama-decree".to_string(),
                        team: team.clone(),
                    }],
                )),
                _ => None,
            });
            let conclusion = direct_victory
                .or_else(|| crate::rules::projection::game_conclusion_if_needed(&state));
            if let Some(conclusion) = conclusion {
                let event = GameEvent::GameEnded { conclusion };
                crate::rules::projection::apply_event(&mut state, &event);
                decision.events.push(event);
                finished = true;
            }
        }
    }
}

#[derive(Debug)]
pub enum PersistedGameRecordLoadError {
    Game(GameError),
    Replay(ReplayVerificationError),
}

impl From<GameError> for PersistedGameRecordLoadError {
    fn from(error: GameError) -> Self {
        Self::Game(error)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PersistedSnapshot {
    pub after_sequence: u64,
    pub state: GameState,
}

impl PersistedSnapshot {
    pub fn from_state(after_sequence: u64, state: GameState) -> Self {
        Self {
            after_sequence,
            state,
        }
    }

    pub fn to_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }

    pub fn from_json(json: &str) -> Result<Self, serde_json::Error> {
        let mut value = serde_json::from_str(json)?;
        migrate_legacy_wire_format(&mut value);
        if let Some(state) = value.get_mut("state") {
            migrate_legacy_snapshot_state(state);
        }
        serde_json::from_value(value)
    }
}

/// 持久化邊界上的明確線上格式相容處理。既有的命令/事件日誌仍會透過舊版
/// 事件處理器（特別是 `FormationPerformed` 與 `TurnDiscardChosen`）回放；
/// 新的寫入端不會產生它們。快照是快取，因此會在這裡反序列化前升級舊的
/// 僅含狀態格式，而不是交由公開投影猜測。
fn migrate_legacy_wire_format(value: &mut Value) {
    match value {
        Value::Array(values) => {
            for value in values {
                migrate_legacy_wire_format(value);
            }
        }
        Value::Object(values) => {
            if let Some(Value::String(phase)) = values.get_mut("phase") {
                match phase.as_str() {
                    "Main" => *phase = "ActiveEffects".to_string(),
                    "TurnDrawDiscardChoice" => *phase = "TurnDraw".to_string(),
                    _ => {}
                }
            }
            migrate_legacy_attack_resolution(values);
            for value in values.values_mut() {
                migrate_legacy_wire_format(value);
            }
        }
        _ => {}
    }
}

/// `AttackResolved.elemental_context_update` 過去只包含元素脈絡物件。新的
/// 欄位包含完整的原子解析負載，因此包裝舊物件即可，不改變其效果。
fn migrate_legacy_attack_resolution(values: &mut Map<String, Value>) {
    let Some(Value::Object(attack)) = values.get_mut("AttackResolved") else {
        return;
    };
    let Some(update) = attack.get_mut("elemental_context_update") else {
        return;
    };
    if update.is_null()
        || update
            .as_object()
            .is_some_and(|payload| payload.contains_key("outcome"))
    {
        return;
    }

    let old_context = std::mem::replace(update, Value::Null);
    let mut payload = Map::new();
    payload.insert("outcome".to_string(), Value::String("Resolved".to_string()));
    payload.insert("elemental_context_update".to_string(), old_context);
    *update = Value::Object(payload);
}

fn migrate_legacy_snapshot_state(state: &mut Value) {
    let Some(values) = state.as_object_mut() else {
        return;
    };
    values
        .entry("turn_draw_pool".to_string())
        .or_insert_with(|| Value::Array(Vec::new()));
    migrate_legacy_finished_status(values);
    if values.contains_key("formation_areas") {
        return;
    }

    let players = values
        .get("players")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let covered = values
        .get("covered_passives")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let areas = players
        .into_iter()
        .filter_map(|player| {
            let player_id = player.get("id")?.clone();
            let passive = covered.iter().find(|passive| {
                passive
                    .get("owner")
                    .is_some_and(|owner| owner == &player_id)
            });
            let formation = passive.map(|passive| {
                let mut waiting = Map::new();
                waiting.insert(
                    "sealed".to_string(),
                    passive.get("sealed").cloned().unwrap_or(Value::Bool(false)),
                );
                waiting.insert("revealed".to_string(), Value::Bool(false));
                waiting.insert("neutralized".to_string(), Value::Bool(false));
                waiting.insert(
                    "trigger_timing".to_string(),
                    Value::String("NextPlayerActionStart".to_string()),
                );
                let mut state = Map::new();
                state.insert("FaceDownWaiting".to_string(), Value::Object(waiting));
                let mut formation = Map::new();
                formation.insert(
                    "formation_id".to_string(),
                    passive
                        .get("formation_id")
                        .cloned()
                        .unwrap_or(Value::String("legacy-covered-passive".to_string())),
                );
                formation.insert(
                    "cards".to_string(),
                    passive
                        .get("cards")
                        .cloned()
                        .unwrap_or(Value::Array(Vec::new())),
                );
                formation.insert(
                    "star_substitution".to_string(),
                    passive
                        .get("star_substitution")
                        .cloned()
                        .unwrap_or(Value::Null),
                );
                formation.insert("state".to_string(), Value::Object(state));
                Value::Object(formation)
            });
            let mut area = Map::new();
            area.insert("player".to_string(), player_id);
            area.insert("formation".to_string(), formation.unwrap_or(Value::Null));
            Some(Value::Object(area))
        })
        .collect();
    values.insert("formation_areas".to_string(), Value::Array(areas));
}

fn migrate_legacy_finished_status(state: &mut Map<String, Value>) {
    let defeated = state
        .get("hp")
        .and_then(Value::as_array)
        .map(|teams| {
            teams
                .iter()
                .filter(|team| team.get("hp") == Some(&Value::from(0)))
                .filter_map(|team| team.get("team").cloned())
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let Some(Value::Object(status)) = state.get_mut("status") else {
        return;
    };
    let Some(Value::Object(finished)) = status.get_mut("Finished") else {
        return;
    };
    if finished.contains_key("conclusion") {
        return;
    }
    let mut outcome = finished
        .remove("outcome")
        .unwrap_or(Value::String("Draw".to_string()));
    if let Value::Object(outcome_object) = &mut outcome
        && let Some(team) = outcome_object.remove("Team")
    {
        outcome_object.insert("Winner".to_string(), team);
    }
    let mut cause = Map::new();
    cause.insert(
        "TeamHpDepleted".to_string(),
        Value::Object(Map::from_iter([(
            String::from("teams"),
            Value::Array(defeated),
        )])),
    );
    let mut conclusion = Map::new();
    conclusion.insert("outcome".to_string(), outcome);
    conclusion.insert(
        "causes".to_string(),
        Value::Array(vec![Value::Object(cause)]),
    );
    finished.insert("conclusion".to_string(), Value::Object(conclusion));
}

#[derive(Debug)]
pub enum FileSystemPersistenceError {
    Io(io::Error),
    Json(serde_json::Error),
    Load(PersistedGameRecordLoadError),
}

impl From<io::Error> for FileSystemPersistenceError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}

impl From<serde_json::Error> for FileSystemPersistenceError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json(error)
    }
}

impl From<PersistedGameRecordLoadError> for FileSystemPersistenceError {
    fn from(error: PersistedGameRecordLoadError) -> Self {
        Self::Load(error)
    }
}

#[derive(Clone, Debug)]
pub struct FileSystemPersistence {
    root: PathBuf,
}

impl FileSystemPersistence {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn event_log_path(&self, game_id: &str) -> PathBuf {
        self.root.join("event_logs").join(format!("{game_id}.json"))
    }

    fn snapshot_path(&self, game_id: &str) -> PathBuf {
        self.root.join("snapshots").join(format!("{game_id}.json"))
    }

    fn write_json(path: &Path, json: &str) -> Result<(), FileSystemPersistenceError> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, json)?;
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FixedDeckPreparation {
    deck_order: Vec<CardInstanceId>,
}

impl FixedDeckPreparation {
    pub fn new(deck_order: Vec<CardInstanceId>) -> Self {
        Self { deck_order }
    }
}

impl DeckPreparation for FixedDeckPreparation {
    type Error = Infallible;

    fn prepare_deck(&mut self, _setup: &GameSetup) -> Result<Vec<CardInstanceId>, Self::Error> {
        Ok(self.deck_order.clone())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SeededDeckPreparation {
    seed: u64,
}

impl SeededDeckPreparation {
    pub fn new(seed: u64) -> Self {
        Self { seed }
    }
}

impl DeckPreparation for SeededDeckPreparation {
    type Error = Infallible;

    fn prepare_deck(&mut self, setup: &GameSetup) -> Result<Vec<CardInstanceId>, Self::Error> {
        let mut deck_order = setup
            .card_instances
            .iter()
            .map(|card| card.instance)
            .collect::<Vec<_>>();
        deck_order.sort();

        let mut state = self.seed;
        for index in (1..deck_order.len()).rev() {
            state = next_shuffle_state(state);
            let swap_index = (state as usize) % (index + 1);
            deck_order.swap(index, swap_index);
        }

        Ok(deck_order)
    }
}

fn next_shuffle_state(state: u64) -> u64 {
    state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407)
}

#[derive(Default)]
pub struct InMemoryPersistence {
    event_logs: HashMap<String, PersistedGameRecord>,
    snapshots: HashMap<String, PersistedSnapshot>,
}

impl EventLogStorage for InMemoryPersistence {
    type EventLog = PersistedGameRecord;
    type Error = Infallible;

    fn save_event_log(
        &mut self,
        game_id: &str,
        event_log: &Self::EventLog,
    ) -> Result<(), Self::Error> {
        self.event_logs
            .insert(game_id.to_string(), event_log.clone());
        Ok(())
    }

    fn load_event_log(&self, game_id: &str) -> Result<Option<Self::EventLog>, Self::Error> {
        Ok(self.event_logs.get(game_id).cloned())
    }
}

impl SnapshotStorage for InMemoryPersistence {
    type Snapshot = PersistedSnapshot;
    type Error = Infallible;

    fn save_snapshot(
        &mut self,
        game_id: &str,
        snapshot: &Self::Snapshot,
    ) -> Result<(), Self::Error> {
        self.snapshots.insert(game_id.to_string(), snapshot.clone());
        Ok(())
    }

    fn load_snapshot(&self, game_id: &str) -> Result<Option<Self::Snapshot>, Self::Error> {
        Ok(self.snapshots.get(game_id).cloned())
    }
}

impl EventLogStorage for FileSystemPersistence {
    type EventLog = PersistedGameRecord;
    type Error = FileSystemPersistenceError;

    fn save_event_log(
        &mut self,
        game_id: &str,
        event_log: &Self::EventLog,
    ) -> Result<(), Self::Error> {
        let json = event_log.to_json()?;
        Self::write_json(&self.event_log_path(game_id), &json)
    }

    fn load_event_log(&self, game_id: &str) -> Result<Option<Self::EventLog>, Self::Error> {
        let path = self.event_log_path(game_id);
        match fs::read_to_string(path) {
            Ok(json) => Ok(Some(PersistedGameRecord::from_json(&json)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl SnapshotStorage for FileSystemPersistence {
    type Snapshot = PersistedSnapshot;
    type Error = FileSystemPersistenceError;

    fn save_snapshot(
        &mut self,
        game_id: &str,
        snapshot: &Self::Snapshot,
    ) -> Result<(), Self::Error> {
        let json = snapshot.to_json()?;
        Self::write_json(&self.snapshot_path(game_id), &json)
    }

    fn load_snapshot(&self, game_id: &str) -> Result<Option<Self::Snapshot>, Self::Error> {
        let path = self.snapshot_path(game_id);
        match fs::read_to_string(path) {
            Ok(json) => Ok(Some(PersistedSnapshot::from_json(&json)?)),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(None),
            Err(error) => Err(error.into()),
        }
    }
}

impl GameRecordRepository for FileSystemPersistence {
    type Error = FileSystemPersistenceError;

    fn save_record(&mut self, game_id: &str, record: &GameRecord) -> Result<(), Self::Error> {
        let persisted = PersistedGameRecord::from_record(repository_metadata(), record);
        self.save_event_log(game_id, &persisted)
    }

    fn load_record(&self, game_id: &str) -> Result<Option<GameRecord>, Self::Error> {
        let Some(persisted) = self.load_event_log(game_id)? else {
            return Ok(None);
        };
        Ok(Some(persisted.to_record()?))
    }
}

impl GameRecordRepository for InMemoryPersistence {
    type Error = PersistedGameRecordLoadError;

    fn save_record(&mut self, game_id: &str, record: &GameRecord) -> Result<(), Self::Error> {
        let persisted = PersistedGameRecord::from_record(repository_metadata(), record);
        self.event_logs.insert(game_id.to_string(), persisted);
        Ok(())
    }

    fn load_record(&self, game_id: &str) -> Result<Option<GameRecord>, Self::Error> {
        let Some(persisted) = self.event_logs.get(game_id).cloned() else {
            return Ok(None);
        };
        Ok(Some(persisted.to_record()?))
    }
}

fn repository_metadata() -> PersistenceMetadata {
    PersistenceMetadata {
        ruleset_id: RulesetId::base().as_str().to_string(),
        engine_version: env!("CARGO_PKG_VERSION").to_string(),
    }
}
