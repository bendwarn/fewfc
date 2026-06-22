//! Application services: command handling, automatic advancement, and replay.

mod recorded_event_log;

use crate::domain::{
    CardInstanceId, Command, CommandId, GameError, GameEvent, GameResult, GameSetup, GameState,
    validate_setup,
};
use crate::ports::DeckPreparation as DeckPreparationPort;
use crate::public_view::{PublicGameEvent, PublicGameState, Viewer};
use recorded_event_log::RecordedEventLog;

pub use crate::rules::base::BaseRuleset;
pub use recorded_event_log::{
    AutomaticReason, CommandContext, CommandKind, EventMetadata, EventSource, RecordedEvent,
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
    latest_snapshot: Option<GameState>,
}

impl GameRecord {
    pub fn start(setup: GameSetup, deck_order: Vec<CardInstanceId>) -> GameResult<Self> {
        let ruleset = BaseRuleset::new();
        let events = ruleset.start_game(&setup, deck_order)?;

        let record = Self {
            setup,
            event_log: RecordedEventLog::from_setup_events(events),
            latest_snapshot: None,
        };

        record.replay()?;
        Ok(record)
    }

    pub fn start_game(input: StartGame) -> GameResult<Self> {
        Self::start(input.setup, input.deck_order)
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

    pub fn recorded_event_count(&self) -> usize {
        self.event_log.len()
    }

    pub fn public_events_for(&self, viewer: Viewer) -> Vec<PublicGameEvent> {
        crate::public_view::events_for(self.events(), viewer)
    }

    pub fn public_view(&self, viewer: Viewer) -> GameResult<PublicGameState> {
        Ok(crate::public_view::state_for(&self.state()?, viewer))
    }

    pub fn state(&self) -> GameResult<GameState> {
        self.replay()
    }

    pub fn replay(&self) -> GameResult<GameState> {
        replay(&self.setup, self.event_log.events())
    }

    pub fn latest_snapshot(&self) -> Option<&GameState> {
        self.latest_snapshot.as_ref()
    }

    pub fn apply(&mut self, command: Command) -> GameResult<EventBatch> {
        let state = self.state()?;
        let source = EventSource::Command {
            command_id: self.next_command_id(),
            context: command_context(&command),
        };
        let events = BaseRuleset::new().decide_command(&state, command)?;
        self.event_log.append(events.clone(), |event| {
            source_for_command_event(event, &source)
        });
        Ok(EventBatch::new(events))
    }

    pub fn handle(&mut self, command: Command) -> GameResult<Vec<GameEvent>> {
        self.apply(command).map(EventBatch::into_events)
    }

    pub fn advance_until_decision(&mut self) -> GameResult<EventBatch> {
        let state = self.state()?;
        let events = BaseRuleset::new().advance_automatic(&state)?;
        self.event_log
            .append(events.clone(), source_for_automatic_event);
        Ok(EventBatch::new(events))
    }

    pub fn advance_automatic(&mut self) -> GameResult<Vec<GameEvent>> {
        self.advance_until_decision().map(EventBatch::into_events)
    }

    pub fn verify_replay(&self) -> Result<GameState, ReplayVerificationError> {
        verify_recorded_events(&self.setup, &self.recorded_events())
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

fn source_for_command_event(_event: &GameEvent, source: &EventSource) -> EventSource {
    source.clone()
}

fn source_for_automatic_event(event: &GameEvent) -> EventSource {
    EventSource::Automatic {
        reason: automatic_reason(event)
            .expect("automatic advancement must only emit automatic events"),
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
        | GameEvent::ActionPassed { .. }
        | GameEvent::AttackResolved { .. }
        | GameEvent::CardsMoved { .. }
        | GameEvent::EffectChoiceAnswered { .. }
        | GameEvent::EffectChoiceRequested { .. }
        | GameEvent::FormationPerformed { .. }
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

pub fn advance_automatic(state: &GameState) -> GameResult<Vec<GameEvent>> {
    BaseRuleset::new().advance_automatic(state)
}

pub fn handle_command(state: &GameState, command: Command) -> GameResult<Vec<GameEvent>> {
    BaseRuleset::new().decide_command(state, command)
}

pub fn apply_event(state: &mut GameState, event: &GameEvent) {
    crate::rules::base::apply_event(state, event);
}

pub fn replay(setup: &GameSetup, events: &[GameEvent]) -> Result<GameState, GameError> {
    crate::rules::base::project(setup, events)
}

pub fn verify_recorded_events(
    setup: &GameSetup,
    recorded_events: &[RecordedEvent],
) -> Result<GameState, ReplayVerificationError> {
    validate_setup(setup).map_err(|error| ReplayVerificationError::DecisionFailed {
        sequence: 0,
        source: EventSource::Setup,
        error: Box::new(error),
    })?;

    let mut state = GameState::from_setup(setup);
    let mut index = 0;

    while index < recorded_events.len() {
        let sequence = recorded_events[index].metadata.sequence;
        let source = recorded_events[index].metadata.source.clone();
        let end = verification_group_end(recorded_events, index);
        let actual = recorded_events[index..end]
            .iter()
            .map(|recorded| recorded.event.clone())
            .collect::<Vec<_>>();

        let expected = match &source {
            EventSource::Setup => {
                let deck_order = actual
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::DeckPrepared { deck_order } => Some(deck_order.clone()),
                        _ => None,
                    })
                    .ok_or(ReplayVerificationError::MissingSetupDeck { sequence })?;
                BaseRuleset::new()
                    .start_game(setup, deck_order)
                    .map_err(|error| ReplayVerificationError::DecisionFailed {
                        sequence,
                        source: source.clone(),
                        error: Box::new(error),
                    })?
            }
            EventSource::Automatic { .. } => advance_automatic(&state).map_err(|error| {
                ReplayVerificationError::DecisionFailed {
                    sequence,
                    source: source.clone(),
                    error: Box::new(error),
                }
            })?,
            EventSource::Command { context, .. } => {
                let command = command_from_recorded_events(context, &actual).ok_or_else(|| {
                    ReplayVerificationError::CommandContextUnavailable {
                        sequence,
                        source: source.clone(),
                    }
                })?;
                handle_command(&state, command).map_err(|error| {
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

        for event in &actual {
            apply_event(&mut state, event);
        }
        index = end;
    }

    Ok(state)
}

fn verification_group_end(recorded_events: &[RecordedEvent], start: usize) -> usize {
    let source = &recorded_events[start].metadata.source;
    let mut end = start + 1;

    while end < recorded_events.len()
        && verification_sources_share_group(source, &recorded_events[end].metadata.source)
    {
        end += 1;
    }

    end
}

fn verification_sources_share_group(first: &EventSource, next: &EventSource) -> bool {
    match (first, next) {
        (EventSource::Setup, EventSource::Setup) => true,
        (EventSource::Automatic { .. }, EventSource::Automatic { .. }) => true,
        (
            EventSource::Command {
                command_id: first_id,
                ..
            },
            EventSource::Command {
                command_id: next_id,
                ..
            },
        ) => first_id == next_id,
        _ => false,
    }
}

fn command_from_recorded_events(context: &CommandContext, events: &[GameEvent]) -> Option<Command> {
    match &context.kind {
        CommandKind::PassAction => events.iter().find_map(|event| match event {
            GameEvent::ActionPassed { reason, .. } => Some(Command::PassAction {
                player: context.player.clone(),
                reason: *reason,
            }),
            _ => None,
        }),
        CommandKind::PerformFormation { formation_id } => {
            events.iter().find_map(|event| match event {
                GameEvent::FormationPerformed {
                    used_cards,
                    declared_targets,
                    ..
                } => Some(Command::PerformFormation {
                    player: context.player.clone(),
                    formation_id: formation_id.clone(),
                    cards: used_cards.clone(),
                    declared_targets: declared_targets.clone(),
                }),
                GameEvent::AttackResolved { used_cards, .. } => Some(Command::PerformFormation {
                    player: context.player.clone(),
                    formation_id: formation_id.clone(),
                    cards: used_cards.clone(),
                    declared_targets: Vec::new(),
                }),
                _ => None,
            })
        }
        CommandKind::ChooseTurnDiscard => events.iter().find_map(|event| match event {
            GameEvent::TurnDiscardChosen { discard, .. } => Some(Command::ChooseTurnDiscard {
                player: context.player.clone(),
                discard: *discard,
            }),
            _ => None,
        }),
        CommandKind::AnswerEffectChoice => events.iter().find_map(|event| match event {
            GameEvent::EffectChoiceAnswered { selected_cards, .. } => {
                Some(Command::AnswerEffectChoice {
                    player: context.player.clone(),
                    selected_cards: selected_cards.clone(),
                })
            }
            _ => None,
        }),
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
