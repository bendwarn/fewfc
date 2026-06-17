//! Infrastructure adapters and deterministic setup helpers.

use crate::application::{GameRecord, replay};
use crate::domain::{GameError, GameSetup, GameState, RecordedEvent, RulesetId, ValidationError};
use crate::ports::{EventLogStorage, SnapshotStorage};
use std::collections::HashMap;
use std::convert::Infallible;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistenceMetadata {
    pub ruleset_id: String,
    pub engine_version: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PersistedGameRecord {
    pub metadata: PersistenceMetadata,
    pub setup: GameSetup,
    pub recorded_events: Vec<RecordedEvent>,
    pub latest_snapshot: Option<PersistedSnapshot>,
}

impl PersistedGameRecord {
    pub fn from_record(metadata: PersistenceMetadata, record: &GameRecord) -> Self {
        let mut metadata = metadata;
        metadata.ruleset_id = record.setup().ruleset.as_str().to_string();

        Self {
            metadata,
            setup: record.setup().clone(),
            recorded_events: record.recorded_events(),
            latest_snapshot: record
                .latest_snapshot()
                .cloned()
                .map(|state| PersistedSnapshot {
                    after_sequence: record.recorded_events().len() as u64,
                    state,
                }),
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
            .recorded_events
            .iter()
            .map(|recorded| recorded.event.clone())
            .collect::<Vec<_>>();
        replay(&self.setup, &events)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
