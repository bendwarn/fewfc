//! Ports for external persistence or integration boundaries.

use crate::application::GameRecord;
use crate::domain::{CardInstanceId, GameSetup};

pub trait DeckPreparation {
    type Error;

    fn prepare_deck(&mut self, setup: &GameSetup) -> Result<Vec<CardInstanceId>, Self::Error>;
}

pub trait EventLogStorage {
    type EventLog: Clone;
    type Error;

    fn save_event_log(
        &mut self,
        game_id: &str,
        event_log: &Self::EventLog,
    ) -> Result<(), Self::Error>;
    fn load_event_log(&self, game_id: &str) -> Result<Option<Self::EventLog>, Self::Error>;
}

pub trait SnapshotStorage {
    type Snapshot: Clone;
    type Error;

    fn save_snapshot(
        &mut self,
        game_id: &str,
        snapshot: &Self::Snapshot,
    ) -> Result<(), Self::Error>;
    fn load_snapshot(&self, game_id: &str) -> Result<Option<Self::Snapshot>, Self::Error>;
}

pub trait GameRecordRepository {
    type Error;

    fn save_record(&mut self, game_id: &str, record: &GameRecord) -> Result<(), Self::Error>;
    fn load_record(&self, game_id: &str) -> Result<Option<GameRecord>, Self::Error>;
}
