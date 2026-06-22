//! Application services: command handling, automatic advancement, and replay.

mod formation_use;
mod projection;

use crate::domain::{
    AutomaticReason, CardInstanceId, Command, CommandContext, CommandId, CommandKind,
    DeckPlacement, EngineInvariantError, EventMetadata, EventSource, GameError, GameEvent,
    GameResult, GameSetup, GameState, GameStatus, PassActionReason, Phase, RecordedEvent,
    RulesetId, TurnDrawSkipReason, ValidationError, validate_setup,
};
use crate::ports::DeckPreparation as DeckPreparationPort;
use crate::public_view::{PublicGameEvent, PublicGameState, Viewer};
use std::collections::HashSet;

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

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BaseRuleset;

impl BaseRuleset {
    pub fn new() -> Self {
        Self
    }

    pub fn id(&self) -> RulesetId {
        RulesetId::base()
    }

    pub fn start_game(
        &self,
        setup: &GameSetup,
        deck_order: Vec<CardInstanceId>,
    ) -> GameResult<Vec<GameEvent>> {
        validate_setup(setup)?;
        validate_card_instances(setup, &deck_order)?;
        initial_events(setup, deck_order)
    }

    pub fn decide_command(
        &self,
        state: &GameState,
        command: Command,
    ) -> GameResult<Vec<GameEvent>> {
        decide_command_with_base_ruleset(state, command)
    }

    pub fn advance_automatic(&self, state: &GameState) -> GameResult<Vec<GameEvent>> {
        advance_automatic(state)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    setup: GameSetup,
    events: Vec<GameEvent>,
    event_metadata: Vec<EventMetadata>,
    latest_snapshot: Option<GameState>,
}

impl GameRecord {
    pub fn start(setup: GameSetup, deck_order: Vec<CardInstanceId>) -> GameResult<Self> {
        let ruleset = BaseRuleset::new();
        let events = ruleset.start_game(&setup, deck_order)?;
        let event_metadata = metadata_for_events(events.len(), EventSource::Setup);

        let record = Self {
            setup,
            events,
            event_metadata,
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
        &self.events
    }

    pub fn setup(&self) -> &GameSetup {
        &self.setup
    }

    pub fn recorded_events(&self) -> Vec<RecordedEvent> {
        self.events
            .iter()
            .zip(self.event_metadata.iter())
            .map(|(event, metadata)| RecordedEvent {
                metadata: metadata.clone(),
                event: event.clone(),
            })
            .collect()
    }

    pub fn public_events_for(&self, viewer: Viewer) -> Vec<PublicGameEvent> {
        crate::public_view::events_for(&self.events, viewer)
    }

    pub fn public_view(&self, viewer: Viewer) -> GameResult<PublicGameState> {
        Ok(crate::public_view::state_for(&self.state()?, viewer))
    }

    pub fn state(&self) -> GameResult<GameState> {
        self.replay()
    }

    pub fn replay(&self) -> GameResult<GameState> {
        projection::project(&self.setup, &self.events)
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
        self.extend_events(events.clone(), |event| {
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
        self.extend_events(events.clone(), source_for_automatic_event);
        Ok(EventBatch::new(events))
    }

    pub fn advance_automatic(&mut self) -> GameResult<Vec<GameEvent>> {
        self.advance_until_decision().map(EventBatch::into_events)
    }

    pub fn verify_replay(&self) -> Result<GameState, ReplayVerificationError> {
        verify_recorded_events(&self.setup, &self.recorded_events())
    }

    fn extend_events(
        &mut self,
        events: Vec<GameEvent>,
        source_for_event: impl Fn(&GameEvent) -> EventSource,
    ) {
        let first_sequence = self.events.len() as u64 + 1;
        self.event_metadata.extend(
            events
                .iter()
                .enumerate()
                .map(|(index, event)| EventMetadata {
                    sequence: first_sequence + index as u64,
                    source: source_for_event(event),
                }),
        );
        self.events.extend(events.clone());
    }

    fn next_command_id(&self) -> CommandId {
        let next_id = self
            .event_metadata
            .iter()
            .filter_map(|metadata| match &metadata.source {
                EventSource::Command { command_id, .. } => Some(command_id.as_u64()),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1;
        CommandId::new(next_id)
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

fn metadata_for_events(count: usize, source: EventSource) -> Vec<EventMetadata> {
    (0..count)
        .map(|index| EventMetadata {
            sequence: index as u64 + 1,
            source: source.clone(),
        })
        .collect()
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

fn validate_card_instances(setup: &GameSetup, deck_order: &[CardInstanceId]) -> GameResult<()> {
    let mut seen = HashSet::new();
    let known_instances = setup
        .card_instances
        .iter()
        .map(|card| card.instance)
        .collect::<HashSet<_>>();

    for card in deck_order {
        if !seen.insert(*card) {
            return Err(GameError::Validation(ValidationError::DuplicateCard(*card)));
        }

        if !known_instances.contains(card) {
            return Err(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ));
        }
    }

    Ok(())
}

fn initial_events(
    setup: &GameSetup,
    deck_order: Vec<CardInstanceId>,
) -> GameResult<Vec<GameEvent>> {
    let needed = initial_deal_count(setup);
    if deck_order.len() < needed {
        return Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed,
                available: deck_order.len(),
            },
        ));
    }

    let mut events = vec![GameEvent::DeckPrepared {
        deck_order: deck_order.clone(),
    }];
    let mut next_card_index = 0;

    for (turn_index, player) in setup.turn_order.iter().enumerate() {
        let card_count = if turn_index == 0 { 4 } else { 5 };
        let cards = deck_order[next_card_index..next_card_index + card_count].to_vec();
        next_card_index += card_count;
        events.push(GameEvent::CardsDealt {
            player: player.clone(),
            cards,
        });
    }

    Ok(events)
}

fn initial_deal_count(setup: &GameSetup) -> usize {
    setup
        .turn_order
        .iter()
        .enumerate()
        .map(|(index, _)| if index == 0 { 4 } else { 5 })
        .sum()
}

pub fn advance_automatic(state: &GameState) -> GameResult<Vec<GameEvent>> {
    ensure_engine_invariants(state)?;

    if matches!(state.status, GameStatus::Finished { .. }) {
        return Ok(Vec::new());
    }
    if state.pending_choice.is_some() {
        return Ok(Vec::new());
    }

    let mut projected = state.clone();
    let mut events = Vec::new();

    loop {
        let next_event = match projected.phase {
            Phase::TurnStart => status_expiry_event(
                &projected,
                crate::domain::StatusExpiryTiming::TurnStart {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                        .clone(),
                },
            )
            .or_else(|| {
                Some(GameEvent::TurnStarted {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))
                        .ok()?
                        .clone(),
                    turn_number: projected.turn_number,
                })
            }),
            Phase::TurnDraw => next_turn_draw_event(&projected)?,
            Phase::TurnEnd => status_expiry_event(
                &projected,
                crate::domain::StatusExpiryTiming::TurnEnd {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                        .clone(),
                },
            )
            .or_else(|| {
                Some(GameEvent::TurnEnded {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))
                        .ok()?
                        .clone(),
                })
            }),
            Phase::Main | Phase::TurnDrawDiscardChoice => None,
        };

        let Some(event) = next_event else {
            break;
        };

        projection::apply_event(&mut projected, &event);
        events.push(event);
    }

    Ok(events)
}

fn status_expiry_event(
    state: &GameState,
    timing: crate::domain::StatusExpiryTiming,
) -> Option<GameEvent> {
    let status = state
        .statuses
        .iter()
        .find(|status| status_expires_at(&status.duration, &timing, state.turn_number))?;

    Some(GameEvent::StatusExpired {
        status_id: status.id.clone(),
        owner: status.owner.clone(),
        expired_at: timing,
    })
}

fn status_expires_at(
    duration: &crate::domain::StatusDuration,
    timing: &crate::domain::StatusExpiryTiming,
    current_turn_number: u64,
) -> bool {
    match (duration, timing) {
        (
            crate::domain::StatusDuration::UntilTurnStart {
                player: duration_player,
            },
            crate::domain::StatusExpiryTiming::TurnStart {
                player: timing_player,
            },
        )
        | (
            crate::domain::StatusDuration::UntilTurnEnd {
                player: duration_player,
            },
            crate::domain::StatusExpiryTiming::TurnEnd {
                player: timing_player,
            },
        ) => duration_player == timing_player,
        (
            crate::domain::StatusDuration::UntilTurnEndNumber {
                player: duration_player,
                turn_number,
            },
            crate::domain::StatusExpiryTiming::TurnEnd {
                player: timing_player,
            },
        ) => duration_player == timing_player && *turn_number == current_turn_number,
        (crate::domain::StatusDuration::Permanent, _) => false,
        _ => false,
    }
}

fn next_turn_draw_event(state: &GameState) -> GameResult<Option<GameEvent>> {
    let player = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
        .clone();
    let hand = state
        .hand(&player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if player_has_status(state, &player, "CannotDraw") {
        return Ok(Some(GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::CannotDrawByStatus,
        }));
    }
    let available_space = state.hand_limit.saturating_sub(hand.len());

    if available_space == 0 {
        return Ok(Some(GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::HandLimitReached,
        }));
    }

    let draw_bonus = state
        .turn_draw_bonus_by_player
        .get(&player)
        .copied()
        .unwrap_or(0);
    let draw_count = (state.base_draw + draw_bonus).min(available_space) + 1;
    if state.deck.len() < draw_count {
        if state.deck.len() + state.discard.len() >= draw_count && !state.discard.is_empty() {
            return Ok(Some(GameEvent::DiscardRecycledIntoDeck {
                shuffled_order: state.discard.clone(),
                placement: DeckPlacement::Bottom,
            }));
        }

        return Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed: draw_count,
                available: state.deck.len(),
            },
        ));
    }

    let drawn_cards = state
        .deck
        .iter()
        .take(draw_count)
        .copied()
        .collect::<Vec<_>>();

    Ok(Some(GameEvent::CardsDrawnForTurnDiscardChoice {
        player,
        allowed_discards: drawn_cards.clone(),
        drawn_cards,
    }))
}

pub fn handle_command(state: &GameState, command: Command) -> GameResult<Vec<GameEvent>> {
    BaseRuleset::new().decide_command(state, command)
}

fn decide_command_with_base_ruleset(
    state: &GameState,
    command: Command,
) -> GameResult<Vec<GameEvent>> {
    ensure_engine_invariants(state)?;

    if matches!(state.status, GameStatus::Finished { .. }) {
        return Err(GameError::Validation(ValidationError::GameFinished));
    }
    if let Some(choice) = &state.pending_choice {
        let is_choice_answer = matches!(
            (&choice.kind, &command),
            (
                crate::domain::PendingChoiceKind::TurnDrawDiscard { .. },
                Command::ChooseTurnDiscard { .. }
            ) | (
                crate::domain::PendingChoiceKind::EffectGenerated { .. },
                Command::AnswerEffectChoice { .. }
            )
        );

        if !is_choice_answer {
            return Err(GameError::Validation(
                ValidationError::PendingChoiceInProgress {
                    player: choice.player.clone(),
                },
            ));
        }
    }

    match command {
        Command::PassAction { player, reason } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;

            match reason {
                PassActionReason::NoCardsInHand => {
                    let hand = state.hand(&player).ok_or_else(|| {
                        GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
                    })?;
                    if !hand.is_empty() {
                        return Err(GameError::Validation(ValidationError::CannotPassAction {
                            reason,
                        }));
                    }
                }
                PassActionReason::CannotActByStatus => {
                    if !player_has_status(state, &player, "CannotAct") {
                        return Err(GameError::Validation(ValidationError::CannotPassAction {
                            reason,
                        }));
                    }
                }
            }

            Ok(vec![GameEvent::ActionPassed { player, reason }])
        }
        Command::PerformFormation {
            player,
            formation_id,
            cards,
            declared_targets,
        } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;

            formation_use::resolve(
                state,
                formation_use::FormationUseRequest {
                    player,
                    formation_id,
                    cards,
                    declared_targets,
                },
            )
        }
        Command::ChooseTurnDiscard { player, discard } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::TurnDrawDiscardChoice)?;

            let allowed_discards = match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::TurnDrawDiscard {
                            allowed_discards, ..
                        },
                }) if choice_player == &player => allowed_discards,
                _ => return Err(GameError::Validation(ValidationError::MissingPendingChoice)),
            };

            if !allowed_discards.contains(&discard) {
                return Err(GameError::Validation(ValidationError::IllegalDiscard(
                    discard,
                )));
            }

            Ok(vec![GameEvent::TurnDiscardChosen { player, discard }])
        }
        Command::AnswerEffectChoice {
            player,
            selected_cards,
        } => {
            let (effect_id, continuation_id, allowed_cards) = match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::EffectGenerated {
                            effect_id,
                            continuation_id,
                            allowed_cards,
                        },
                }) if choice_player == &player => {
                    (effect_id.clone(), continuation_id.clone(), allowed_cards)
                }
                _ => return Err(GameError::Validation(ValidationError::MissingPendingChoice)),
            };

            let mut seen = HashSet::new();
            for selected_card in &selected_cards {
                if !seen.insert(*selected_card) {
                    return Err(GameError::Validation(ValidationError::DuplicateChoiceCard(
                        *selected_card,
                    )));
                }

                if !allowed_cards.contains(selected_card) {
                    return Err(GameError::Validation(ValidationError::IllegalChoiceCard(
                        *selected_card,
                    )));
                }
            }

            let mut events = vec![GameEvent::EffectChoiceAnswered {
                player: player.clone(),
                effect_id: effect_id.clone(),
                continuation_id: continuation_id.clone(),
                selected_cards: selected_cards.clone(),
            }];
            let resumed_events = formation_use::answer_effect_choice(
                state,
                &player,
                &effect_id,
                &continuation_id,
                &selected_cards,
            )?;
            events.extend(resumed_events);
            Ok(events)
        }
    }
}

fn ensure_engine_invariants(state: &GameState) -> GameResult<()> {
    let mut covered_passive_owners = HashSet::new();
    for passive in &state.covered_passives {
        if !covered_passive_owners.insert(passive.owner.clone()) {
            return Err(GameError::EngineInvariant(
                EngineInvariantError::DuplicateCoveredPassive {
                    player: passive.owner.clone(),
                },
            ));
        }
    }

    Ok(())
}

fn ensure_current_player(state: &GameState, actual: &crate::domain::PlayerId) -> GameResult<()> {
    let expected = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;

    if expected == actual {
        Ok(())
    } else {
        Err(GameError::Validation(ValidationError::WrongPlayer {
            expected: expected.clone(),
            actual: actual.clone(),
        }))
    }
}

fn ensure_phase(state: &GameState, expected: Phase) -> GameResult<()> {
    if state.phase == expected {
        Ok(())
    } else {
        Err(GameError::Validation(ValidationError::WrongPhase {
            expected,
            actual: state.phase,
        }))
    }
}

fn player_has_status(state: &GameState, player: &crate::domain::PlayerId, kind: &str) -> bool {
    state.statuses.iter().any(|status| {
        matches!(&status.owner, crate::domain::StatusOwner::Player(owner) if owner == player)
            && status.kind == kind
    })
}

pub fn apply_event(state: &mut GameState, event: &GameEvent) {
    projection::apply_event(state, event);
}

pub fn replay(setup: &GameSetup, events: &[GameEvent]) -> Result<GameState, GameError> {
    projection::project(setup, events)
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
                initial_events(setup, deck_order).map_err(|error| {
                    ReplayVerificationError::DecisionFailed {
                        sequence,
                        source: source.clone(),
                        error: Box::new(error),
                    }
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
            projection::apply_event(&mut state, event);
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
