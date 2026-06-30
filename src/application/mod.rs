//! Application services: command handling, automatic advancement, and replay.

mod recorded_event_log;

use crate::domain::{
    CardInstanceId, Command, CommandId, GameError, GameEvent, GameResult, GameSetup, GameState,
};
use crate::ports::DeckPreparation as DeckPreparationPort;
use crate::public_view::{PublicGameEvent, PublicGameState, Viewer};
use crate::rules::{FormationCandidate, OfficialRules};
use recorded_event_log::RecordedEventLog;

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
        let events = OfficialRules::new().advance_automatic(self.state())?;
        self.event_log.append_automatic(events.clone());
        for event in &events {
            apply_event(&mut self.current_state, event);
        }
        Ok(EventBatch::new(events))
    }

    pub fn advance_automatic(&mut self) -> GameResult<Vec<GameEvent>> {
        self.advance_until_decision().map(EventBatch::into_events)
    }

    pub fn verify_replay(&self) -> Result<GameState, ReplayVerificationError> {
        verify_recorded_decisions(&self.setup, &self.recorded_decisions())
    }

    pub fn playable_formations(
        &self,
        player: &crate::domain::PlayerId,
        selected_cards: &[CardInstanceId],
    ) -> GameResult<Vec<FormationCandidate>> {
        OfficialRules::new().playable_formations(self.state(), player, selected_cards)
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

fn automatic_reason(event: &GameEvent) -> Option<AutomaticReason> {
    match event {
        GameEvent::TurnStarted { .. } => Some(AutomaticReason::TurnStart),
        GameEvent::CardsDrawnForTurnDiscardChoice { .. } => Some(AutomaticReason::TurnDraw),
        GameEvent::TurnDrawSkipped { .. } => Some(AutomaticReason::TurnDrawSkipped),
        GameEvent::DiscardRecycledIntoDeck { .. }
        | GameEvent::PlayerDiscardRecycledIntoDeck { .. } => Some(AutomaticReason::DiscardRecycle),
        GameEvent::StatusExpired { .. } => Some(AutomaticReason::StatusExpired),
        GameEvent::TurnEnded { .. } => Some(AutomaticReason::TurnEnd),
        GameEvent::DeckPrepared { .. }
        | GameEvent::PlayerDeckPrepared { .. }
        | GameEvent::CardsDealt { .. }
        | GameEvent::CounterEffectEstablished { .. }
        | GameEvent::CounterEffectResolved { .. }
        | GameEvent::ActionPassed { .. }
        | GameEvent::AttackResolved { .. }
        | GameEvent::EnvironmentTransferred { .. }
        | GameEvent::EnvironmentCleared { .. }
        | GameEvent::CardsMoved { .. }
        | GameEvent::EffectChoiceAnswered { .. }
        | GameEvent::EffectChoiceRequested { .. }
        | GameEvent::FormationEffectCopied { .. }
        | GameEvent::FormationEffectIgnored { .. }
        | GameEvent::FormationPerformed { .. }
        | GameEvent::HandInspected { .. }
        | GameEvent::HpChanged { .. }
        | GameEvent::PassiveCovered { .. }
        | GameEvent::PassiveFlipped { .. }
        | GameEvent::ShieldChanged { .. }
        | GameEvent::StatusAdded { .. }
        | GameEvent::StatusRemoved { .. }
        | GameEvent::TurnDrawBonusChanged { .. }
        | GameEvent::TurnDiscardChosen { .. }
        | GameEvent::DiscardRetrieved { .. } => None,
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

pub fn apply_event(state: &mut GameState, event: &GameEvent) {
    crate::rules::projection::apply_event(state, event);
}

pub fn replay(setup: &GameSetup, events: &[GameEvent]) -> Result<GameState, GameError> {
    OfficialRules::new().validate_setup(setup)?;
    crate::rules::projection::project(setup, events)
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
                if deck_order.is_empty() {
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
    use crate::domain::{PlayerId, TeamId};

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
