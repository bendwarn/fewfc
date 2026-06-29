use crate::domain::{Command, CommandId, GameEvent, PlayerId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct RecordedEventLog {
    events: Vec<GameEvent>,
    recorded_decisions: Vec<RecordedDecision>,
    recorded_events: Vec<RecordedEvent>,
}

impl RecordedEventLog {
    pub(super) fn from_setup_events(events: Vec<GameEvent>) -> Self {
        let mut log = Self {
            events: Vec::new(),
            recorded_decisions: Vec::new(),
            recorded_events: Vec::new(),
        };
        log.append_setup(events);
        log
    }

    pub(super) fn from_decisions(decisions: Vec<RecordedDecision>) -> Self {
        let mut log = Self {
            events: Vec::new(),
            recorded_decisions: Vec::new(),
            recorded_events: Vec::new(),
        };

        for decision in decisions {
            log.append_decision(decision.source, decision.events);
        }

        log
    }

    pub(super) fn events(&self) -> &[GameEvent] {
        &self.events
    }

    pub(super) fn recorded_events(&self) -> &[RecordedEvent] {
        &self.recorded_events
    }

    pub(super) fn recorded_decisions(&self) -> &[RecordedDecision] {
        &self.recorded_decisions
    }

    pub(super) fn len(&self) -> usize {
        self.recorded_events.len()
    }

    pub(super) fn append_command(
        &mut self,
        command_id: CommandId,
        command: Command,
        events: Vec<GameEvent>,
    ) {
        self.append_decision(
            RecordedDecisionSource::Command {
                command_id,
                command,
            },
            events,
        );
    }

    pub(super) fn append_automatic(&mut self, events: Vec<GameEvent>) {
        self.append_decision(RecordedDecisionSource::Automatic, events);
    }

    fn append_setup(&mut self, events: Vec<GameEvent>) {
        self.append_decision(RecordedDecisionSource::Setup, events);
    }

    fn append_decision(&mut self, source: RecordedDecisionSource, events: Vec<GameEvent>) {
        let first_sequence = self.recorded_events.len() as u64 + 1;
        self.recorded_events
            .extend(
                events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| RecordedEvent {
                        metadata: EventMetadata {
                            sequence: first_sequence + index as u64,
                            source: event_source(&source, event),
                        },
                        event: event.clone(),
                    }),
            );
        self.recorded_decisions.push(RecordedDecision {
            source,
            events: events.clone(),
        });
        self.events.extend(events);
    }

    pub(super) fn next_command_id(&self) -> CommandId {
        let next_id = self
            .recorded_decisions
            .iter()
            .filter_map(|recorded| match &recorded.source {
                RecordedDecisionSource::Command { command_id, .. } => Some(command_id.as_u64()),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1;
        CommandId::new(next_id)
    }
}

fn event_source(source: &RecordedDecisionSource, event: &GameEvent) -> EventSource {
    match source {
        RecordedDecisionSource::Setup => EventSource::Setup,
        RecordedDecisionSource::Automatic => EventSource::Automatic {
            reason: automatic_reason(event)
                .expect("automatic advancement must only emit automatic events"),
        },
        RecordedDecisionSource::Command {
            command_id,
            command,
        } => EventSource::Command {
            command_id: *command_id,
            context: command_context(command),
        },
    }
}

fn automatic_reason(event: &GameEvent) -> Option<AutomaticReason> {
    match event {
        GameEvent::TurnStarted { .. } => Some(AutomaticReason::TurnStart),
        GameEvent::CardsDrawnForTurnDiscardChoice { .. } => Some(AutomaticReason::TurnDraw),
        GameEvent::TurnDrawSkipped { .. } => Some(AutomaticReason::TurnDrawSkipped),
        GameEvent::DiscardRecycledIntoDeck { .. } => Some(AutomaticReason::DiscardRecycle),
        GameEvent::StatusExpired { .. } => Some(AutomaticReason::StatusExpired),
        GameEvent::TurnEnded { .. } => Some(AutomaticReason::TurnEnd),
        GameEvent::DeckPrepared { .. }
        | GameEvent::CardsDealt { .. }
        | GameEvent::CounterEffectEstablished { .. }
        | GameEvent::CounterEffectResolved { .. }
        | GameEvent::ActionPassed { .. }
        | GameEvent::AttackResolved { .. }
        | GameEvent::CardsMoved { .. }
        | GameEvent::EffectChoiceAnswered { .. }
        | GameEvent::EffectChoiceRequested { .. }
        | GameEvent::FormationEffectCopied { .. }
        | GameEvent::FormationPerformed { .. }
        | GameEvent::HandInspected { .. }
        | GameEvent::HpChanged { .. }
        | GameEvent::PassiveCovered { .. }
        | GameEvent::PassiveFlipped { .. }
        | GameEvent::ShieldChanged { .. }
        | GameEvent::StatusAdded { .. }
        | GameEvent::StatusRemoved { .. }
        | GameEvent::TurnDrawBonusChanged { .. }
        | GameEvent::TurnDiscardChosen { .. } => None,
    }
}

fn command_context(command: &Command) -> CommandContext {
    match command {
        Command::PassAction { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::PassAction,
        },
        Command::PerformFormation {
            player,
            formation_id,
            ..
        } => CommandContext {
            player: player.clone(),
            kind: CommandKind::PerformFormation {
                formation_id: formation_id.clone(),
            },
        },
        Command::ChooseTurnDiscard { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::ChooseTurnDiscard,
        },
        Command::AnswerEffectChoice { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::AnswerEffectChoice,
        },
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RecordedDecision {
    pub source: RecordedDecisionSource,
    pub events: Vec<GameEvent>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RecordedDecisionSource {
    Setup,
    Automatic,
    Command {
        command_id: CommandId,
        command: Command,
    },
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
