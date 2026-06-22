//! Application services: command handling, automatic advancement, and replay.

mod formation_use;

use crate::domain::{
    AutomaticReason, CardInstanceId, CardMoveDelta, CardZone, Command, CommandContext, CommandId,
    CommandKind, DeckPlacement, EngineInvariantError, EventMetadata, EventSource, GameError,
    GameEvent, GameOutcome, GameResult, GameSetup, GameState, GameStatus, LastFormationUse,
    PassActionReason, Phase, PublicGameEvent, PublicGameState, RecordedEvent, RulesetId,
    ShieldChangeDelta, TurnDrawSkipReason, ValidationError, Viewer, validate_setup,
};
use crate::ports::DeckPreparation as DeckPreparationPort;
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
        let mut state = GameState::from_setup(&self.setup);
        for event in &self.events {
            apply_event(&mut state, event);
        }
        Ok(state)
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

        apply_event(&mut projected, &event);
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

fn apply_card_move(state: &mut GameState, card_move: &CardMoveDelta) {
    let removed = match &card_move.from {
        CardZone::Hand(player) => {
            let hand = state
                .hand_mut(player)
                .expect("canonical card move must move cards from a known hand");
            let position = hand
                .iter()
                .position(|card| card == &card_move.card)
                .expect("canonical card move must move an existing card");
            hand.remove(position)
        }
        CardZone::DeckTop => {
            assert!(
                !state.deck.is_empty(),
                "canonical card move must move from a non-empty deck"
            );
            let position = state
                .deck
                .iter()
                .position(|deck_card| deck_card == &card_move.card)
                .expect("canonical card move must move an existing deck card");
            state.deck.remove(position)
        }
        CardZone::Discard => {
            let position = state
                .discard
                .iter()
                .position(|card| card == &card_move.card)
                .expect("canonical card move must move an existing discarded card");
            state.discard.remove(position)
        }
    };

    match &card_move.to {
        CardZone::Hand(player) => {
            let hand = state
                .hand_mut(player)
                .expect("canonical card move must move cards to a known hand");
            hand.push(removed);
        }
        CardZone::DeckTop => state.deck.insert(0, removed),
        CardZone::Discard => state.discard.push(removed),
    }
}

fn apply_shield_change(state: &mut GameState, change: &ShieldChangeDelta) {
    let shield = state
        .shields
        .iter_mut()
        .find(|shield| shield.player == change.player)
        .expect("canonical shield event must target an existing player");
    shield.value = change.new_value;
}

pub fn apply_event(state: &mut GameState, event: &GameEvent) {
    match event {
        GameEvent::DeckPrepared { deck_order } => {
            state.deck = deck_order.clone();
        }
        GameEvent::CardsDealt { player, cards } => {
            let hand = state
                .hand_mut(player)
                .expect("canonical deal event must target a known player");
            hand.extend(cards.iter().copied());

            for card in cards {
                let position = state
                    .deck
                    .iter()
                    .position(|deck_card| deck_card == card)
                    .expect("canonical deal event must contain cards from deck");
                state.deck.remove(position);
            }
        }
        GameEvent::TurnStarted { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnStart);
            state.phase = Phase::Main;
        }
        GameEvent::ActionPassed { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::Main);
            state.phase = Phase::TurnDraw;
        }
        GameEvent::FormationPerformed {
            player,
            formation_id,
            used_cards,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::Main);

            for used_card in used_cards {
                let hand = state
                    .hand_mut(player)
                    .expect("canonical formation event must target a known player");
                let position = hand
                    .iter()
                    .position(|card| card == used_card)
                    .expect("canonical formation event must remove cards from hand");
                let removed = hand.remove(position);
                state.discard.push(removed);
            }

            state.last_formation_by_player.insert(
                player.clone(),
                LastFormationUse {
                    formation_id: formation_id.clone(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = Phase::TurnDraw;
        }
        GameEvent::PassiveCovered {
            player,
            formation_id,
            cards,
            sealed,
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::Main);

            let hand = state
                .hand_mut(player)
                .expect("canonical passive cover event must target a known player");
            for card in cards {
                let position = hand
                    .iter()
                    .position(|hand_card| hand_card == card)
                    .expect("canonical passive cover event must remove cards from hand");
                hand.remove(position);
            }

            state.covered_passives.push(crate::domain::CoveredPassive {
                owner: player.clone(),
                formation_id: formation_id.clone(),
                cards: cards.clone(),
                sealed: *sealed,
                covered_on_turn: state.turn_number,
                reveal_timing: crate::domain::PassiveTriggerTiming::NextPlayerActionStart,
            });
            state.last_formation_by_player.insert(
                player.clone(),
                LastFormationUse {
                    formation_id: formation_id.clone(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = Phase::TurnDraw;
        }
        GameEvent::PassiveFlipped {
            owner,
            passive_id: _,
            cards,
            ..
        } => {
            let passive_position = state
                .covered_passives
                .iter()
                .position(|passive| &passive.owner == owner)
                .expect("canonical passive flip event must target a covered passive");
            state.covered_passives.remove(passive_position);
            state.discard.extend(cards.iter().copied());
        }
        GameEvent::AttackResolved {
            attacker,
            formation_id,
            hp_change,
            shield_change,
            card_moves,
            elemental_context_update,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(attacker));
            debug_assert!(matches!(state.phase, Phase::Main | Phase::TurnDraw));

            let team_hp = state
                .hp
                .iter_mut()
                .find(|team_hp| team_hp.team == hp_change.team)
                .expect("canonical attack event must target an existing team");
            team_hp.hp = hp_change.new_hp;
            finish_game_if_needed(state);

            if let Some(shield_change) = shield_change {
                apply_shield_change(state, shield_change);
            }

            for card_move in card_moves {
                apply_card_move(state, card_move);
            }

            if let Some(update) = elemental_context_update {
                state
                    .last_elemental_attack_by_player
                    .insert(update.player.clone(), update.attack.clone());
            }

            state.last_formation_by_player.insert(
                attacker.clone(),
                LastFormationUse {
                    formation_id: formation_id.clone(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = Phase::TurnDraw;
        }
        GameEvent::TurnDrawBonusChanged {
            player, new_value, ..
        } => {
            state
                .turn_draw_bonus_by_player
                .insert(player.clone(), *new_value);
        }
        GameEvent::HpChanged { change } => {
            let team_hp = state
                .hp
                .iter_mut()
                .find(|team_hp| team_hp.team == change.team)
                .expect("canonical hp event must target an existing team");
            team_hp.hp = change.new_hp;
            finish_game_if_needed(state);
        }
        GameEvent::CardsMoved { card_moves } => {
            for card_move in card_moves {
                apply_card_move(state, card_move);
            }
        }
        GameEvent::ShieldChanged {
            player,
            old_value: _,
            delta: _,
            new_value,
        } => {
            apply_shield_change(
                state,
                &ShieldChangeDelta {
                    player: player.clone(),
                    old_value: state.shield(player).unwrap_or(0),
                    delta: new_value - state.shield(player).unwrap_or(0),
                    new_value: *new_value,
                },
            );
        }
        GameEvent::StatusAdded { status } => {
            state.statuses.push(status.clone());
        }
        GameEvent::StatusExpired {
            status_id, owner, ..
        }
        | GameEvent::StatusRemoved { status_id, owner } => {
            let position = state
                .statuses
                .iter()
                .position(|status| &status.id == status_id && &status.owner == owner)
                .expect("canonical status removal event must target an active status");
            state.statuses.remove(position);
        }
        GameEvent::EffectChoiceRequested { player, kind } => {
            state.pending_choice = Some(crate::domain::PendingChoice {
                player: player.clone(),
                kind: kind.clone(),
            });
        }
        GameEvent::EffectChoiceAnswered {
            player,
            selected_cards,
            ..
        } => {
            match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind: crate::domain::PendingChoiceKind::EffectGenerated { allowed_cards, .. },
                }) if choice_player == player => {
                    for selected_card in selected_cards {
                        if !allowed_cards.contains(selected_card) {
                            panic!("canonical effect choice answer must select allowed cards");
                        }
                    }
                }
                _ => panic!("canonical effect choice answer must have a matching pending choice"),
            }

            state.pending_choice = None;
        }
        GameEvent::CardsDrawnForTurnDiscardChoice {
            player,
            drawn_cards,
            allowed_discards,
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnDraw);

            let hand = state
                .hand_mut(player)
                .expect("canonical draw event must target a known player");
            hand.extend(drawn_cards.iter().copied());
            state.deck.drain(0..drawn_cards.len());
            state.pending_choice = Some(crate::domain::PendingChoice {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::TurnDrawDiscard {
                    drawn_cards: drawn_cards.clone(),
                    allowed_discards: allowed_discards.clone(),
                },
            });
            state.phase = Phase::TurnDrawDiscardChoice;
        }
        GameEvent::TurnDiscardChosen { player, discard } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnDrawDiscardChoice);

            let allowed_discards = match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::TurnDrawDiscard {
                            allowed_discards, ..
                        },
                }) if choice_player == player => allowed_discards,
                _ => panic!("canonical discard event must have a matching pending choice"),
            };

            if !allowed_discards.contains(discard) {
                panic!("canonical discard event must choose an allowed card");
            }

            let hand = state
                .hand_mut(player)
                .expect("canonical discard event must target a known player");
            let discard_position = hand
                .iter()
                .position(|card| card == discard)
                .expect("canonical discard event must remove a card from hand");
            let discarded = hand.remove(discard_position);

            state.discard.push(discarded);
            state.pending_choice = None;
            state.phase = Phase::TurnEnd;
        }
        GameEvent::TurnDrawSkipped { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnDraw);
            state.phase = Phase::TurnEnd;
        }
        GameEvent::DiscardRecycledIntoDeck {
            shuffled_order,
            placement,
        } => {
            debug_assert_eq!(state.phase, Phase::TurnDraw);

            match placement {
                DeckPlacement::Bottom => state.deck.extend(shuffled_order.iter().copied()),
            }

            for card in shuffled_order {
                let position = state
                    .discard
                    .iter()
                    .position(|discarded| discarded == card)
                    .expect("canonical recycle event must contain cards from discard");
                state.discard.remove(position);
            }
        }
        GameEvent::TurnEnded { player } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnEnd);

            state.turn_draw_bonus_by_player.remove(player);
            state.current_turn_index = (state.current_turn_index + 1) % state.turn_order.len();
            state.turn_number += 1;
            state.phase = Phase::TurnStart;
        }
    }
}

fn finish_game_if_needed(state: &mut GameState) {
    let alive_teams = state
        .hp
        .iter()
        .filter(|team_hp| team_hp.hp > 0)
        .map(|team_hp| team_hp.team.clone())
        .collect::<Vec<_>>();
    let defeated_count = state.hp.iter().filter(|team_hp| team_hp.hp == 0).count();

    if defeated_count == 0 {
        return;
    }

    state.status = if alive_teams.len() == 1 {
        GameStatus::Finished {
            outcome: GameOutcome::Team(alive_teams[0].clone()),
        }
    } else {
        GameStatus::Finished {
            outcome: GameOutcome::Draw,
        }
    };
}

pub fn replay(setup: &GameSetup, events: &[GameEvent]) -> Result<GameState, GameError> {
    validate_setup(setup)?;
    let mut state = GameState::from_setup(setup);
    for event in events {
        apply_event(&mut state, event);
    }
    Ok(state)
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
