//! Application services: command handling, automatic advancement, and replay.

mod recorded_event_log;

use crate::domain::{
    CardInstanceId, Command, CommandId, GameError, GameEvent, GameResult, GameSetup, GameState,
    TrustedRandomnessAnswer,
};
use crate::ports::DeckPreparation as DeckPreparationPort;
use crate::public_view::{PublicGameEvent, PublicGameState, Viewer};
use crate::rules::{OfficialRules, PlayableAction};
use recorded_event_log::{RecordedEventLog, automatic_reason};

pub use recorded_event_log::{
    AutomaticReason, CommandContext, CommandKind, EventMetadata, EventSource, RecordedDecision,
    RecordedDecisionSource, RecordedEvent,
};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventBatch {
    events: Vec<GameEvent>,
}

impl EventBatch {
    pub fn new(events: Vec<GameEvent>) -> Self {
        Self { events }
    }

    pub fn events(&self) -> &[GameEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<GameEvent> {
        self.events
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartGame {
    pub setup: GameSetup,
    pub deck_order: Vec<CardInstanceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartGameWithDeckPreparationError<E> {
    DeckPreparation(E),
    Game(GameError),
}

impl<E> From<GameError> for StartGameWithDeckPreparationError<E> {
    fn from(error: GameError) -> Self {
        Self::Game(error)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    setup: GameSetup,
    event_log: RecordedEventLog,
    current_state: GameState,
}

/// A prefix of a canonical record suitable for a read-only replay viewer.
///
/// `step` is deliberately expressed in player commands, not canonical events:
/// setup and every decision before the first command are step zero; each later
/// step contains one command and all following automatic/randomness decisions
/// until the next command.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReplayFrame {
    pub step: usize,
    pub total_steps: usize,
    pub state: GameState,
    pub events: Vec<GameEvent>,
}

impl GameRecord {
    pub fn start(setup: GameSetup, deck_order: Vec<CardInstanceId>) -> GameResult<Self> {
        let ruleset = OfficialRules::new();
        let events = ruleset.start_game(&setup, deck_order)?;
        let current_state = replay(&setup, &events)?;

        let record = Self {
            setup,
            event_log: RecordedEventLog::from_setup_events(events),
            current_state,
        };

        Ok(record)
    }

    pub fn start_game(input: StartGame) -> GameResult<Self> {
        Self::start(input.setup, input.deck_order)
    }

    pub fn from_recorded_decisions(
        setup: GameSetup,
        recorded_decisions: Vec<RecordedDecision>,
    ) -> Result<Self, ReplayVerificationError> {
        let current_state = verify_recorded_decisions(&setup, &recorded_decisions)?;
        let record = Self {
            setup,
            event_log: RecordedEventLog::from_decisions(recorded_decisions),
            current_state,
        };
        Ok(record)
    }

    pub fn start_with_deck_preparation<P>(
        setup: GameSetup,
        deck_preparation: &mut P,
    ) -> Result<Self, StartGameWithDeckPreparationError<P::Error>>
    where
        P: DeckPreparationPort,
    {
        let deck_order = deck_preparation
            .prepare_deck(&setup)
            .map_err(StartGameWithDeckPreparationError::DeckPreparation)?;
        Self::start(setup, deck_order).map_err(StartGameWithDeckPreparationError::Game)
    }

    pub fn events(&self) -> &[GameEvent] {
        self.event_log.events()
    }

    pub fn setup(&self) -> &GameSetup {
        &self.setup
    }

    pub fn recorded_events(&self) -> Vec<RecordedEvent> {
        self.event_log.recorded_events().to_vec()
    }

    pub fn recorded_decisions(&self) -> Vec<RecordedDecision> {
        self.event_log.recorded_decisions().to_vec()
    }

    pub fn recorded_event_count(&self) -> usize {
        self.event_log.len()
    }

    pub fn public_events_for(&self, viewer: Viewer) -> Vec<PublicGameEvent> {
        crate::public_view::events_for(self.events(), viewer)
    }

    pub fn public_view(&self, viewer: Viewer) -> GameResult<PublicGameState> {
        Ok(crate::public_view::state_for(self.state(), viewer))
    }

    pub fn state(&self) -> &GameState {
        &self.current_state
    }

    #[cfg(test)]
    pub(crate) fn fixture_state_mut(&mut self) -> &mut GameState {
        &mut self.current_state
    }

    pub fn replay(&self) -> GameResult<GameState> {
        replay(&self.setup, self.event_log.events())
    }

    pub fn apply(&mut self, command: Command) -> GameResult<EventBatch> {
        let command_id = self.next_command_id();
        let events = OfficialRules::new().decide_command(self.state(), command.clone())?;
        self.event_log
            .append_command(command_id, command, events.clone());
        for event in &events {
            apply_event(&mut self.current_state, event);
        }
        Ok(EventBatch::new(events))
    }

    pub fn handle(&mut self, command: Command) -> GameResult<Vec<GameEvent>> {
        self.apply(command).map(EventBatch::into_events)
    }

    pub fn advance_until_decision(&mut self) -> GameResult<EventBatch> {
        let mut advanced = Vec::new();

        loop {
            let events = OfficialRules::new().advance_automatic(self.state())?;
            self.event_log.append_automatic(events.clone());
            for event in &events {
                apply_event(&mut self.current_state, event);
            }
            advanced.extend(events);

            let Some(command) = self.sole_forced_pass_command()? else {
                return Ok(EventBatch::new(advanced));
            };
            advanced.extend(self.apply(command)?.into_events());
        }
    }

    pub fn advance_automatic(&mut self) -> GameResult<Vec<GameEvent>> {
        self.advance_until_decision().map(EventBatch::into_events)
    }

    pub fn resolve_randomness(
        &mut self,
        answer: TrustedRandomnessAnswer,
    ) -> GameResult<EventBatch> {
        let events = resolve_trusted_randomness(self.state(), &answer)?;
        self.event_log.append_randomness(answer, events.clone());
        for event in &events {
            apply_event(&mut self.current_state, event);
        }
        Ok(EventBatch::new(events))
    }

    pub fn verify_replay(&self) -> Result<GameState, ReplayVerificationError> {
        verify_recorded_decisions(&self.setup, &self.recorded_decisions())
    }

    pub fn playable_actions(
        &self,
        player: &crate::domain::PlayerId,
        selected_cards: &[CardInstanceId],
    ) -> GameResult<Vec<PlayableAction>> {
        OfficialRules::new().playable_actions(self.state(), player, selected_cards)
    }

    fn sole_forced_pass_command(&self) -> GameResult<Option<Command>> {
        if !matches!(self.state().status, crate::domain::GameStatus::InProgress)
            || self.state().phase != crate::domain::Phase::ActiveEffects
            || self.state().pending_choice.is_some()
            || self.state().pending_randomness.is_some()
        {
            return Ok(None);
        }
        let Some(player) = self.state().current_player().cloned() else {
            return Ok(None);
        };
        let actions = self.playable_actions(&player, &[])?;

        Ok(match actions.as_slice() {
            [PlayableAction::Pass { reason }] => Some(Command::PassAction {
                player,
                reason: *reason,
            }),
            _ => None,
        })
    }

    fn next_command_id(&self) -> CommandId {
        self.event_log.next_command_id()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayVerificationError {
    DecisionFailed {
        sequence: u64,
        source: EventSource,
        error: Box<GameError>,
    },
    EventMismatch {
        sequence: u64,
        expected: Vec<GameEvent>,
        actual: Vec<GameEvent>,
    },
    MissingSetupDeck {
        sequence: u64,
    },
    CommandContextUnavailable {
        sequence: u64,
        source: EventSource,
    },
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

pub fn advance_automatic(state: &GameState) -> GameResult<Vec<GameEvent>> {
    OfficialRules::new().advance_automatic(state)
}

pub fn handle_command(state: &GameState, command: Command) -> GameResult<Vec<GameEvent>> {
    OfficialRules::new().decide_command(state, command)
}

pub fn resolve_trusted_randomness(
    state: &GameState,
    answer: &TrustedRandomnessAnswer,
) -> GameResult<Vec<GameEvent>> {
    crate::rules::randomness::resolve_trusted_randomness(state, answer)
}

pub fn apply_event(state: &mut GameState, event: &GameEvent) {
    crate::rules::projection::apply_event(state, event);
}

pub fn replay(setup: &GameSetup, events: &[GameEvent]) -> Result<GameState, GameError> {
    OfficialRules::new().validate_setup(setup)?;
    crate::rules::projection::project(setup, events)
}

/// Projects a canonical decision record without re-running commands, automatic
/// advancement, or trusted randomness.  Verification is intentionally a
/// separate operation (`verify_recorded_decisions`).
pub fn replay_frame(
    setup: &GameSetup,
    recorded_decisions: &[RecordedDecision],
    step: usize,
) -> Result<ReplayFrame, GameError> {
    OfficialRules::new().validate_setup(setup)?;

    let groups = replay_decision_groups(recorded_decisions);
    let total_steps = groups.len().saturating_sub(1);
    if step > total_steps {
        return Err(GameError::Validation(
            crate::domain::ValidationError::ReplayStepOutOfRange { step, total_steps },
        ));
    }

    let mut state = GameState::from_setup(setup);
    let mut events = Vec::new();
    for group in groups.into_iter().take(step + 1) {
        for decision in group {
            for event in &decision.events {
                apply_event(&mut state, event);
                events.push(event.clone());
            }
        }
    }

    Ok(ReplayFrame {
        step,
        total_steps,
        state,
        events,
    })
}

/// Keeps decision grouping in the application layer so the archive adapter
/// never has to understand canonical sources or trusted randomness payloads.
pub fn replay_decision_groups(
    recorded_decisions: &[RecordedDecision],
) -> Vec<Vec<&RecordedDecision>> {
    let mut groups = vec![Vec::new()];
    for decision in recorded_decisions {
        if matches!(decision.source, RecordedDecisionSource::Command { .. }) {
            groups.push(Vec::new());
        }
        groups
            .last_mut()
            .expect("replay always has setup group")
            .push(decision);
    }
    groups
}

pub fn verify_recorded_decisions(
    setup: &GameSetup,
    recorded_decisions: &[RecordedDecision],
) -> Result<GameState, ReplayVerificationError> {
    OfficialRules::new()
        .validate_setup(setup)
        .map_err(|error| ReplayVerificationError::DecisionFailed {
            sequence: 0,
            source: EventSource::Setup,
            error: Box::new(error),
        })?;

    let mut state = GameState::from_setup(setup);
    let mut sequence = 1;

    for decision in recorded_decisions {
        let source = source_for_recorded_decision(decision);
        let actual = decision.events.clone();
        let expected = match &decision.source {
            RecordedDecisionSource::Setup => {
                let deck_order = actual
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::DeckPrepared { deck_order } => Some(deck_order.clone()),
                        _ => None,
                    })
                    .unwrap_or_else(|| {
                        actual
                            .iter()
                            .filter_map(|event| match event {
                                GameEvent::PlayerDeckPrepared { deck_order, .. } => {
                                    Some(deck_order.iter().copied())
                                }
                                _ => None,
                            })
                            .flatten()
                            .collect()
                    });
                if deck_order.is_empty() && !setup.has_rule_module(crate::domain::POUCH_MODULE_ID) {
                    return Err(ReplayVerificationError::MissingSetupDeck { sequence });
                }
                OfficialRules::new()
                    .start_game(setup, deck_order)
                    .map_err(|error| ReplayVerificationError::DecisionFailed {
                        sequence,
                        source: source.clone(),
                        error: Box::new(error),
                    })?
            }
            RecordedDecisionSource::Automatic => advance_automatic(&state).map_err(|error| {
                ReplayVerificationError::DecisionFailed {
                    sequence,
                    source: source.clone(),
                    error: Box::new(error),
                }
            })?,
            RecordedDecisionSource::Randomness { answer } => {
                resolve_trusted_randomness(&state, answer).map_err(|error| {
                    ReplayVerificationError::DecisionFailed {
                        sequence,
                        source: source.clone(),
                        error: Box::new(error),
                    }
                })?
            }
            RecordedDecisionSource::Command { command, .. } => {
                handle_command(&state, command.clone()).map_err(|error| {
                    ReplayVerificationError::DecisionFailed {
                        sequence,
                        source: source.clone(),
                        error: Box::new(error),
                    }
                })?
            }
        };

        if expected != actual {
            return Err(ReplayVerificationError::EventMismatch {
                sequence,
                expected,
                actual,
            });
        }

        for event in &decision.events {
            apply_event(&mut state, event);
        }
        sequence += decision.events.len() as u64;
    }

    Ok(state)
}

fn source_for_recorded_decision(decision: &RecordedDecision) -> EventSource {
    match &decision.source {
        RecordedDecisionSource::Setup => EventSource::Setup,
        RecordedDecisionSource::Automatic => EventSource::Automatic {
            reason: decision
                .events
                .first()
                .and_then(automatic_reason)
                .unwrap_or(AutomaticReason::TurnStart),
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Command, CommandId, GameEvent, PassActionReason, Phase, PlayerId, TeamId};

    #[test]
    fn sole_pass_is_recorded_as_an_explicit_command_before_advancing() {
        let rules = OfficialRules::new();
        let base = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
        let setup = rules
            .configure_game(base.players, base.turn_order, Vec::new())
            .unwrap();
        let deck = rules.official_deck_order(&setup).unwrap();
        let mut record = GameRecord::start(setup, deck).unwrap();
        record.advance_until_decision().unwrap();
        assert_eq!(record.state().phase, Phase::ActiveEffects);

        let player = record.state().current_player().cloned().unwrap();
        let discarded = std::mem::take(record.fixture_state_mut().hand_mut(&player).unwrap());
        record.fixture_state_mut().discard.extend(discarded);

        let events = record.advance_until_decision().unwrap().into_events();

        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::ActionPassed {
                player: passed,
                reason: PassActionReason::NoCardsInHand,
            } if passed == &player
        )));
        assert!(record.recorded_decisions().iter().any(|decision| matches!(
            &decision.source,
            RecordedDecisionSource::Command {
                command:
                    Command::PassAction {
                        player: passed,
                        reason: PassActionReason::NoCardsInHand,
                    },
                ..
            } if passed == &player
        )));
        assert_ne!(
            (record.state().current_player(), record.state().phase),
            (Some(&player), Phase::ActiveEffects)
        );
    }

    #[test]
    fn replay_frames_group_trailing_automatic_decisions_without_redeciding_commands() {
        let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
        // This command cannot be legal in the untouched setup. Pure replay must
        // still project the recorded (empty) event batch rather than attempting
        // verification and rejecting it.
        let decisions = vec![
            RecordedDecision {
                source: RecordedDecisionSource::Setup,
                events: vec![],
            },
            RecordedDecision {
                source: RecordedDecisionSource::Command {
                    command_id: CommandId::new(1),
                    command: Command::PassAction {
                        player: PlayerId::new("p1"),
                        reason: PassActionReason::NoCardsInHand,
                    },
                },
                events: vec![],
            },
            RecordedDecision {
                source: RecordedDecisionSource::Automatic,
                events: vec![],
            },
        ];

        assert_eq!(
            replay_decision_groups(&decisions)
                .iter()
                .map(Vec::len)
                .collect::<Vec<_>>(),
            vec![1, 2]
        );
        assert_eq!(replay_frame(&setup, &decisions, 0).unwrap().total_steps, 1);
        assert_eq!(replay_frame(&setup, &decisions, 1).unwrap().step, 1);
        assert!(replay_frame(&setup, &decisions, 2).is_err());
    }

    fn two_player_state() -> GameState {
        GameState::from_setup(&GameSetup::two_player(
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            30,
        ))
    }

    fn team_mode_state() -> GameState {
        GameState::from_setup(&GameSetup::team_mode(
            TeamId::new("A"),
            vec![PlayerId::new("p1"), PlayerId::new("p3")],
            TeamId::new("B"),
            vec![PlayerId::new("p2"), PlayerId::new("p4")],
            30,
        ))
    }

    #[test]
    fn rule_player_targets_resolve_in_two_player_games() {
        let state = two_player_state();
        let targets = crate::domain::targeting::TurnOrderTargets::new(&state);

        assert_eq!(
            targets.player_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RulePlayerTarget::SelfPlayer
            ),
            Ok(PlayerId::new("p1"))
        );
        assert_eq!(
            targets.player_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RulePlayerTarget::PreviousPlayer
            ),
            Ok(PlayerId::new("p2"))
        );
        assert_eq!(
            targets.player_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RulePlayerTarget::NextPlayer
            ),
            Ok(PlayerId::new("p2"))
        );
    }

    #[test]
    fn rule_team_targets_resolve_in_two_player_games() {
        let state = two_player_state();
        let targets = crate::domain::targeting::TurnOrderTargets::new(&state);

        assert_eq!(
            targets.team_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RuleTeamTarget::OwnSide
            ),
            Ok(TeamId::new("team:p1"))
        );
        assert_eq!(
            targets.team_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RuleTeamTarget::OpposingSide
            ),
            Ok(TeamId::new("team:p2"))
        );
    }

    #[test]
    fn rule_player_targets_resolve_in_team_mode() {
        let state = team_mode_state();
        let targets = crate::domain::targeting::TurnOrderTargets::new(&state);

        assert_eq!(
            targets.player_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RulePlayerTarget::SelfPlayer
            ),
            Ok(PlayerId::new("p1"))
        );
        assert_eq!(
            targets.player_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RulePlayerTarget::PreviousPlayer
            ),
            Ok(PlayerId::new("p4"))
        );
        assert_eq!(
            targets.player_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RulePlayerTarget::NextPlayer
            ),
            Ok(PlayerId::new("p2"))
        );
    }

    #[test]
    fn rule_team_targets_resolve_in_team_mode() {
        let state = team_mode_state();
        let targets = crate::domain::targeting::TurnOrderTargets::new(&state);

        assert_eq!(
            targets.team_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RuleTeamTarget::OwnSide
            ),
            Ok(TeamId::new("A"))
        );
        assert_eq!(
            targets.team_target(
                &PlayerId::new("p1"),
                crate::domain::targeting::RuleTeamTarget::OpposingSide
            ),
            Ok(TeamId::new("B"))
        );
    }
}
