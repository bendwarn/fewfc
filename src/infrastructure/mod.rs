//! Infrastructure adapters and deterministic setup helpers.

use crate::application::{GameRecord, RecordedDecision, ReplayVerificationError, replay};
use crate::domain::{CardInstanceId, GameError, GameSetup, GameState, RulesetId, ValidationError};
use crate::ports::{DeckPreparation, EventLogStorage, GameRecordRepository, SnapshotStorage};
use serde::{Deserialize, Serialize};
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
        serde_json::from_str(json)
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
        serde_json::from_str(json)
    }
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
