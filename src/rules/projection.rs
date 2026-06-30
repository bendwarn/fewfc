use crate::domain::{
    CardMoveDelta, CardZone, DeckPlacement, GameEvent, GameOutcome, GameResult, GameSetup,
    GameState, GameStatus, LastFormationUse, ShieldChangeDelta, validate_setup,
};

pub(crate) fn project(setup: &GameSetup, events: &[GameEvent]) -> GameResult<GameState> {
    validate_setup(setup)?;
    let mut state = GameState::from_setup(setup);
    for event in events {
        apply_event(&mut state, event);
    }
    Ok(state)
}

pub(crate) fn apply_event(state: &mut GameState, event: &GameEvent) {
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
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnStart);
            state.phase = crate::domain::Phase::Main;
        }
        GameEvent::ActionPassed { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::Main);
            state.phase = crate::domain::Phase::TurnDraw;
        }
        GameEvent::FormationPerformed {
            player,
            formation_id,
            used_cards,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::Main);

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
                    resolved_effect_id: formation_id.clone(),
                    used_cards: used_cards.clone(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = crate::domain::Phase::TurnDraw;
        }
        GameEvent::FormationEffectCopied { player, effect_id } => {
            let last_formation = state
                .last_formation_by_player
                .get_mut(player)
                .expect("copied effect must follow a formation use");
            last_formation.resolved_effect_id = effect_id.clone();
        }
        GameEvent::CounterEffectEstablished { owner, effect_id } => {
            state.counter_effects.push(crate::domain::CounterEffect {
                owner: owner.clone(),
                effect_id: effect_id.clone(),
                established_on_turn: state.turn_number,
            });
        }
        GameEvent::CounterEffectResolved {
            owner, effect_id, ..
        } => {
            let position = state
                .counter_effects
                .iter()
                .position(|counter| counter.owner == *owner && counter.effect_id == *effect_id)
                .expect("resolved counter effect must exist");
            state.counter_effects.remove(position);
        }
        GameEvent::HandInspected { .. } => {}
        GameEvent::PassiveCovered {
            player,
            formation_id,
            cards,
            sealed,
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::Main);

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
                    resolved_effect_id: formation_id.clone(),
                    used_cards: cards.clone(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = crate::domain::Phase::TurnDraw;
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
            used_cards,
            hp_change,
            shield_change,
            card_moves,
            elemental_context_update,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(attacker));
            debug_assert!(matches!(
                state.phase,
                crate::domain::Phase::Main | crate::domain::Phase::TurnDraw
            ));

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

            if let Some(last_formation) = state.last_formation_by_player.get_mut(attacker)
                && last_formation.resolved_turn == state.turn_number
                && last_formation.formation_id == "metamorphosis"
            {
                last_formation.resolved_effect_id = formation_id.clone();
            } else {
                state.last_formation_by_player.insert(
                    attacker.clone(),
                    LastFormationUse {
                        formation_id: formation_id.clone(),
                        resolved_effect_id: formation_id.clone(),
                        used_cards: used_cards.clone(),
                        resolved_turn: state.turn_number,
                    },
                );
            }
            state.phase = crate::domain::Phase::TurnDraw;
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
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnDraw);

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
            state.phase = crate::domain::Phase::TurnDrawDiscardChoice;
        }
        GameEvent::TurnDiscardChosen { player, discard } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnDrawDiscardChoice);

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
            state.phase = crate::domain::Phase::TurnEnd;
        }
        GameEvent::TurnDrawSkipped { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnDraw);
            state.phase = crate::domain::Phase::TurnEnd;
        }
        GameEvent::DiscardRecycledIntoDeck {
            shuffled_order,
            placement,
        } => {
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnDraw);

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
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnEnd);

            state.turn_draw_bonus_by_player.remove(player);
            state.current_turn_index = (state.current_turn_index + 1) % state.turn_order.len();
            state.turn_number += 1;
            state.phase = crate::domain::Phase::TurnStart;
        }
    }
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
