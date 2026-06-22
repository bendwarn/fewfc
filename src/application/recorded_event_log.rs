use crate::domain::{CommandId, GameEvent, PlayerId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RecordedEventLog {
    events: Vec<GameEvent>,
    recorded_events: Vec<RecordedEvent>,
}

impl RecordedEventLog {
    pub(super) fn from_setup_events(events: Vec<GameEvent>) -> Self {
        let mut log = Self {
            events: Vec::new(),
            recorded_events: Vec::new(),
        };
        log.append(events, |_| EventSource::Setup);
        log
    }

    pub(super) fn events(&self) -> &[GameEvent] {
        &self.events
    }

    pub(super) fn recorded_events(&self) -> &[RecordedEvent] {
        &self.recorded_events
    }

    pub(super) fn len(&self) -> usize {
        self.recorded_events.len()
    }

    pub(super) fn append(
        &mut self,
        events: Vec<GameEvent>,
        source_for_event: impl Fn(&GameEvent) -> EventSource,
    ) {
        let first_sequence = self.recorded_events.len() as u64 + 1;
        self.recorded_events
            .extend(
                events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| RecordedEvent {
                        metadata: EventMetadata {
                            sequence: first_sequence + index as u64,
                            source: source_for_event(event),
                        },
                        event: event.clone(),
                    }),
            );
        self.events.extend(events);
    }

    pub(super) fn next_command_id(&self) -> CommandId {
        let next_id = self
            .recorded_events
            .iter()
            .filter_map(|recorded| match &recorded.metadata.source {
                EventSource::Command { command_id, .. } => Some(command_id.as_u64()),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1;
        CommandId::new(next_id)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RecordedEvent {
    pub metadata: EventMetadata,
    pub event: GameEvent,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EventMetadata {
    pub sequence: u64,
    pub source: EventSource,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum EventSource {
    Setup,
    Automatic {
        reason: AutomaticReason,
    },
    Command {
        command_id: CommandId,
        context: CommandContext,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutomaticReason {
    TurnStart,
    TurnDraw,
    TurnDrawSkipped,
    DiscardRecycle,
    StatusExpired,
    TurnEnd,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CommandContext {
    pub player: PlayerId,
    pub kind: CommandKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CommandKind {
    PassAction,
    PerformFormation { formation_id: String },
    ChooseTurnDiscard,
    AnswerEffectChoice,
}
