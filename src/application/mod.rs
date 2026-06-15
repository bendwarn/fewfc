//! Application services: command handling, automatic advancement, and replay.

use crate::domain::{
    CardInstanceId, Command, DeckPlacement, EventMetadata, EventSource, GameError, GameEvent,
    GameResult, GameSetup, GameState, Phase, RecordedEvent, TurnDrawSkipReason, validate_setup,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    setup: GameSetup,
    events: Vec<GameEvent>,
    latest_snapshot: Option<GameState>,
}

impl GameRecord {
    pub fn start(setup: GameSetup, deck_order: Vec<CardInstanceId>) -> GameResult<Self> {
        validate_setup(&setup)?;
        validate_card_instances(&setup, &deck_order)?;

        let record = Self {
            setup,
            events: vec![GameEvent::DeckPrepared { deck_order }],
            latest_snapshot: None,
        };

        record.replay()?;
        Ok(record)
    }

    pub fn events(&self) -> &[GameEvent] {
        &self.events
    }

    pub fn recorded_events(&self) -> Vec<RecordedEvent> {
        self.events
            .iter()
            .enumerate()
            .map(|(index, event)| RecordedEvent {
                metadata: EventMetadata {
                    sequence: index as u64 + 1,
                    source: event_source(event),
                },
                event: event.clone(),
            })
            .collect()
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

    pub fn handle(&mut self, command: Command) -> GameResult<Vec<GameEvent>> {
        let state = self.state()?;
        let events = handle_command(&state, command)?;
        self.events.extend(events.clone());
        Ok(events)
    }

    pub fn advance_automatic(&mut self) -> GameResult<Vec<GameEvent>> {
        let state = self.state()?;
        let events = advance_automatic(&state)?;
        self.events.extend(events.clone());
        Ok(events)
    }
}

fn event_source(event: &GameEvent) -> EventSource {
    match event {
        GameEvent::DeckPrepared { .. }
        | GameEvent::TurnStarted { .. }
        | GameEvent::CardsDrawnForTurnDiscardChoice { .. }
        | GameEvent::TurnDrawSkipped { .. }
        | GameEvent::DiscardRecycledIntoDeck { .. }
        | GameEvent::TurnEnded { .. } => EventSource::System,
        GameEvent::ActiveWindowEnded { .. }
        | GameEvent::ActionSkipped { .. }
        | GameEvent::TurnDiscardChosen { .. } => EventSource::Command,
    }
}

fn validate_card_instances(setup: &GameSetup, deck_order: &[CardInstanceId]) -> GameResult<()> {
    let mut seen = HashSet::new();

    for hand in &setup.starting_hands {
        for card in &hand.cards {
            if !seen.insert(*card) {
                return Err(GameError::DuplicateCard(*card));
            }
        }
    }

    for card in deck_order {
        if !seen.insert(*card) {
            return Err(GameError::DuplicateCard(*card));
        }
    }

    Ok(())
}

pub fn advance_automatic(state: &GameState) -> GameResult<Vec<GameEvent>> {
    let mut projected = state.clone();
    let mut events = Vec::new();

    loop {
        let next_event = match projected.phase {
            Phase::TurnStart => Some(GameEvent::TurnStarted {
                player: projected
                    .current_player()
                    .ok_or(GameError::EmptyTurnOrder)?
                    .clone(),
                turn_number: projected.turn_number,
            }),
            Phase::TurnDraw => next_turn_draw_event(&projected)?,
            Phase::TurnEnd => Some(GameEvent::TurnEnded {
                player: projected
                    .current_player()
                    .ok_or(GameError::EmptyTurnOrder)?
                    .clone(),
            }),
            Phase::ActiveWindow | Phase::Action | Phase::TurnDrawDiscardChoice => None,
        };

        let Some(event) = next_event else {
            break;
        };

        apply_event(&mut projected, &event);
        events.push(event);
    }

    Ok(events)
}

fn next_turn_draw_event(state: &GameState) -> GameResult<Option<GameEvent>> {
    let player = state
        .current_player()
        .ok_or(GameError::EmptyTurnOrder)?
        .clone();
    let hand = state
        .hand(&player)
        .ok_or_else(|| GameError::UnknownPlayer(player.clone()))?;
    let available_space = state.hand_limit.saturating_sub(hand.len());

    if available_space == 0 {
        return Ok(Some(GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::HandLimitReached,
        }));
    }

    let draw_count = state.base_draw.min(available_space) + 1;
    if state.deck.len() < draw_count {
        if state.deck.len() + state.discard.len() >= draw_count && !state.discard.is_empty() {
            return Ok(Some(GameEvent::DiscardRecycledIntoDeck {
                shuffled_order: state.discard.clone(),
                placement: DeckPlacement::Bottom,
            }));
        }

        return Err(GameError::NotEnoughCards {
            needed: draw_count,
            available: state.deck.len(),
        });
    }

    let drawn_cards = state
        .deck
        .iter()
        .take(draw_count)
        .copied()
        .collect::<Vec<_>>();
    let mut allowed_discards = hand.to_vec();
    allowed_discards.extend(drawn_cards.iter().copied());

    Ok(Some(GameEvent::CardsDrawnForTurnDiscardChoice {
        player,
        drawn_cards,
        allowed_discards,
    }))
}

pub fn handle_command(state: &GameState, command: Command) -> GameResult<Vec<GameEvent>> {
    match command {
        Command::EndActiveWindow { player } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::ActiveWindow)?;
            Ok(vec![GameEvent::ActiveWindowEnded { player }])
        }
        Command::SkipAction { player } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Action)?;
            Ok(vec![GameEvent::ActionSkipped { player }])
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
                _ => return Err(GameError::MissingPendingChoice),
            };

            if !allowed_discards.contains(&discard) {
                return Err(GameError::IllegalDiscard(discard));
            }

            Ok(vec![GameEvent::TurnDiscardChosen { player, discard }])
        }
    }
}

fn ensure_current_player(state: &GameState, actual: &crate::domain::PlayerId) -> GameResult<()> {
    let expected = state.current_player().ok_or(GameError::EmptyTurnOrder)?;

    if expected == actual {
        Ok(())
    } else {
        Err(GameError::WrongPlayer {
            expected: expected.clone(),
            actual: actual.clone(),
        })
    }
}

fn ensure_phase(state: &GameState, expected: Phase) -> GameResult<()> {
    if state.phase == expected {
        Ok(())
    } else {
        Err(GameError::WrongPhase {
            expected,
            actual: state.phase,
        })
    }
}

pub fn apply_event(state: &mut GameState, event: &GameEvent) {
    match event {
        GameEvent::DeckPrepared { deck_order } => {
            state.deck = deck_order.clone();
        }
        GameEvent::TurnStarted { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnStart);
            state.phase = Phase::ActiveWindow;
        }
        GameEvent::ActiveWindowEnded { player } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::ActiveWindow);
            state.phase = Phase::Action;
        }
        GameEvent::ActionSkipped { player } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::Action);
            state.phase = Phase::TurnDraw;
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

            state.current_turn_index = (state.current_turn_index + 1) % state.turn_order.len();
            state.turn_number += 1;
            state.phase = Phase::TurnStart;
        }
    }
}

pub fn replay(setup: &GameSetup, events: &[GameEvent]) -> Result<GameState, GameError> {
    validate_setup(setup)?;
    let mut state = GameState::from_setup(setup);
    for event in events {
        apply_event(&mut state, event);
    }
    Ok(state)
}
