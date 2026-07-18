use crate::domain::{Command, CommandId, GameEvent, PlayerId, TrustedRandomnessAnswer};
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

    pub(super) fn append_randomness(
        &mut self,
        answer: TrustedRandomnessAnswer,
        events: Vec<GameEvent>,
    ) {
        self.append_decision(RecordedDecisionSource::Randomness { answer }, events);
    }

    fn append_setup(&mut self, events: Vec<GameEvent>) {
        self.append_decision(RecordedDecisionSource::Setup, events);
    }

    fn append_decision(&mut self, source: RecordedDecisionSource, events: Vec<GameEvent>) {
        let first_sequence = self.recorded_events.len() as u64 + 1;
        let mut inherited_automatic_reason = None;
        self.recorded_events
            .extend(
                events
                    .iter()
                    .enumerate()
                    .map(|(index, event)| RecordedEvent {
                        metadata: EventMetadata {
                            sequence: first_sequence + index as u64,
                            source: event_source(&source, event, &mut inherited_automatic_reason),
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

fn event_source(
    source: &RecordedDecisionSource,
    event: &GameEvent,
    inherited_automatic_reason: &mut Option<AutomaticReason>,
) -> EventSource {
    match source {
        RecordedDecisionSource::Setup => EventSource::Setup,
        RecordedDecisionSource::Automatic => EventSource::Automatic {
            reason: automatic_reason(event)
                .or(*inherited_automatic_reason)
                .inspect(|reason| *inherited_automatic_reason = Some(*reason))
                .expect("automatic advancement must begin with an automatic event"),
        },
        RecordedDecisionSource::Randomness { answer } => EventSource::Randomness {
            request_id: answer.request_id.clone(),
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

pub(super) fn automatic_reason(event: &GameEvent) -> Option<AutomaticReason> {
    match event {
        GameEvent::TurnStarted { .. } => Some(AutomaticReason::TurnStart),
        GameEvent::CardsDrawnForTurnDiscardChoice { .. } => Some(AutomaticReason::TurnDraw),
        GameEvent::TurnDrawSkipped { .. } => Some(AutomaticReason::TurnDrawSkipped),
        GameEvent::StatusExpired { .. } => Some(AutomaticReason::StatusExpired),
        GameEvent::JianghuStateExpired { .. } => Some(AutomaticReason::StatusExpired),
        GameEvent::JianghuPoisonTicked { .. } => Some(AutomaticReason::TurnEnd),
        GameEvent::JianghuDelayedDamageResolved { .. } => Some(AutomaticReason::TurnEnd),
        GameEvent::TurnEnded { .. } | GameEvent::FormationSuppressionExpired { .. } => {
            Some(AutomaticReason::TurnEnd)
        }
        GameEvent::EchoResolutionStarted { .. }
        | GameEvent::EchoResolutionCompleted { .. }
        | GameEvent::FlowStateChanged { .. }
        | GameEvent::HpChanged { .. } => Some(AutomaticReason::EchoResolution),
        GameEvent::PlantEarthResolutionStarted { .. }
        | GameEvent::PlantEarthResolutionCompleted { .. } => Some(AutomaticReason::EchoResolution),
        GameEvent::FlowStateTriggered { .. } => Some(AutomaticReason::TurnDraw),
        GameEvent::RandomnessRequested { request }
            if matches!(
                &request.continuation,
                crate::domain::RandomnessContinuation::Base(
                    crate::domain::BaseRandomnessContinuation::TurnDraw
                )
            ) =>
        {
            Some(AutomaticReason::TurnDraw)
        }
        GameEvent::RandomnessRequested { .. } => None,
        GameEvent::DeckPrepared { .. }
        | GameEvent::PlayerDeckPrepared { .. }
        | GameEvent::GamePreparationStarted { .. }
        | GameEvent::InitialPouchChosen { .. }
        | GameEvent::GamePreparationCompleted
        | GameEvent::PouchPlaced { .. }
        | GameEvent::PouchRevealed { .. }
        | GameEvent::PouchConsumed { .. }
        | GameEvent::PouchLevelBonusGranted { .. }
        | GameEvent::TemporaryStarEffectGranted { .. }
        | GameEvent::SpiritRevived { .. }
        | GameEvent::CardsDealt { .. }
        | GameEvent::CounterEffectEstablished { .. }
        | GameEvent::CounterEffectResolved { .. }
        | GameEvent::ActionPassed { .. }
        | GameEvent::ProfessionChanged { .. }
        | GameEvent::ProfessionTransformed { .. }
        | GameEvent::ProfessionBroken { .. }
        | GameEvent::ProfessionAbilityActivated { .. }
        | GameEvent::FormationRequirementSet { .. }
        | GameEvent::FormationRequirementFulfilled { .. }
        | GameEvent::SpiritSummoned { .. }
        | GameEvent::SpiritTransformed { .. }
        | GameEvent::SpiritPowerChanged { .. }
        | GameEvent::SpiritSkillUsed { .. }
        | GameEvent::SpiritLevelInterpreted { .. }
        | GameEvent::SpiritBroken { .. }
        | GameEvent::AutomaticBloomsResolved { .. }
        | GameEvent::CardsDrawnForProfessionChoice { .. }
        | GameEvent::AttackResolved { .. }
        | GameEvent::EnvironmentTransferred { .. }
        | GameEvent::EnvironmentCleared { .. }
        | GameEvent::StarBroken { .. }
        | GameEvent::StarSummoned { .. }
        | GameEvent::VoidStarBreakingCompleted { .. }
        | GameEvent::VoidReversionResolved { .. }
        | GameEvent::VoidSpiritShatteringResolved { .. }
        | GameEvent::FiveStarAlignmentAchieved { .. }
        | GameEvent::CardsMoved { .. }
        | GameEvent::ChoiceMade { .. }
        | GameEvent::ChoiceRequested { .. }
        | GameEvent::RandomnessResolved { .. }
        | GameEvent::EchoCostPaid { .. }
        | GameEvent::EchoDeclined { .. }
        | GameEvent::EchoScheduled { .. }
        | GameEvent::TimedEffectsReduced { .. }
        | GameEvent::FormationSuppressionSet { .. }
        | GameEvent::RingingMetalCardRevealed { .. }
        | GameEvent::RingingMetalCompleted { .. }
        | GameEvent::PlantEarthScheduled { .. }
        | GameEvent::EarthRendingStarted { .. }
        | GameEvent::EarthRendingEnvironmentChosen { .. }
        | GameEvent::EarthRendingPlayerAnswered { .. }
        | GameEvent::HandRevealed { .. }
        | GameEvent::EarthRendingCompleted { .. }
        | GameEvent::RustedForestStarted { .. }
        | GameEvent::RustedForestCardsRevealed { .. }
        | GameEvent::RustedForestDeckProcessed { .. }
        | GameEvent::RustedForestCompleted { .. }
        | GameEvent::FormationEffectCopied { .. }
        | GameEvent::FormationEffectIgnored { .. }
        | GameEvent::FormationPerformed { .. }
        | GameEvent::FormationMatchOptionDeclared { .. }
        | GameEvent::HandInspected { .. }
        | GameEvent::DeckTopRevealed { .. }
        | GameEvent::PassiveCovered { .. }
        | GameEvent::PassiveCoverRevealed { .. }
        | GameEvent::PassiveFlipped { .. }
        | GameEvent::ShieldChanged { .. }
        | GameEvent::StatusAdded { .. }
        | GameEvent::StatusRemoved { .. }
        | GameEvent::JianghuStateApplied { .. }
        | GameEvent::LimitedUseChanged { .. }
        | GameEvent::ConfluenceCardObligationSet { .. }
        | GameEvent::ConfluenceCardObligationCleared { .. }
        | GameEvent::TurnDrawBonusChanged { .. }
        | GameEvent::TurnDiscardChosen { .. }
        | GameEvent::DiscardRetrieved { .. } => None,
    }
}

fn command_context(command: &Command) -> CommandContext {
    match command {
        Command::ChooseInitialPouch { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::ChooseInitialPouch,
        },
        Command::TriggerSecretStrategy {
            player, strategy, ..
        } => CommandContext {
            player: player.clone(),
            kind: CommandKind::TriggerSecretStrategy {
                strategy: *strategy,
            },
        },
        Command::PassAction { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::PassAction,
        },
        Command::PerformFormation {
            player,
            formation_id,
            ..
        }
        | Command::PerformFormationWithTrustedRandomness {
            player,
            formation_id,
            ..
        } => CommandContext {
            player: player.clone(),
            kind: CommandKind::PerformFormation {
                formation_id: formation_id.clone(),
            },
        },
        Command::ChangeProfession {
            player, profession, ..
        } => CommandContext {
            player: player.clone(),
            kind: CommandKind::ChangeProfession {
                profession: profession.clone(),
            },
        },
        Command::ActivateProfessionAbility {
            player, ability_id, ..
        } => CommandContext {
            player: player.clone(),
            kind: CommandKind::ActivateProfessionAbility {
                ability_id: ability_id.clone(),
            },
        },
        Command::UseSpiritSkill { player, skill, .. }
        | Command::UseSpiritSkillWithTrustedRandomness { player, skill, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::UseSpiritSkill { skill: *skill },
        },
        Command::AnswerChoice { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::AnswerChoice,
        },
        Command::RetrievePreviousTurnDiscard { player } => CommandContext {
            player: player.clone(),
            kind: CommandKind::RetrievePreviousTurnDiscard,
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
    Randomness {
        answer: TrustedRandomnessAnswer,
    },
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
    Randomness {
        request_id: String,
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
    EchoResolution,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CommandContext {
    pub player: PlayerId,
    pub kind: CommandKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CommandKind {
    ChooseInitialPouch,
    TriggerSecretStrategy {
        strategy: crate::domain::SecretStrategy,
    },
    PassAction,
    PerformFormation {
        formation_id: String,
    },
    ChangeProfession {
        profession: crate::domain::ProfessionId,
    },
    ActivateProfessionAbility {
        ability_id: String,
    },
    UseSpiritSkill {
        skill: crate::domain::SpiritSkill,
    },
    AnswerChoice,
    RetrievePreviousTurnDiscard,
}
