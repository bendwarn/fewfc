use crate::domain::{
    CardMoveDelta, CardOrigin, CardZone, DeckPlacement, GameEvent, GameOutcome, GameResult,
    GameSetup, GameState, GameStatus, LastFormationUse, ShieldChangeDelta, validate_setup,
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
        GameEvent::GamePreparationStarted { player_decks } => {
            state.player_decks = player_decks.clone();
        }
        GameEvent::InitialPouchChosen { next_player, .. } => {
            state.status = if let Some(player) = next_player {
                GameStatus::Preparing {
                    stage: crate::domain::GamePreparationStage::InitialPouchSelection {
                        player: player.clone(),
                    },
                }
            } else {
                GameStatus::Preparing {
                    stage: crate::domain::GamePreparationStage::PendingDeckShuffle,
                }
            };
        }
        GameEvent::GamePreparationCompleted => {
            state.status = GameStatus::InProgress;
        }
        GameEvent::PouchPlaced {
            source,
            owner,
            card,
            known_by,
            previous,
        } => {
            if let Some(previous) = previous {
                let position = state
                    .pouches
                    .iter()
                    .position(|pouch| &pouch.owner == owner && pouch.card == *previous)
                    .expect("canonical Pouch replacement must target an owned Pouch");
                state.pouches.remove(position);
                push_to_origin_discard(state, *previous);
            }
            let deck = state
                .deck_for_mut(source)
                .expect("canonical Pouch placement must target an owned Deck");
            let position = deck
                .iter()
                .position(|candidate| candidate == card)
                .expect("canonical Pouch placement must select a Deck Card");
            deck.remove(position);
            state.pouches.push(crate::domain::PlayerPouch {
                owner: owner.clone(),
                card: *card,
                known_by: known_by.clone(),
            });
        }
        GameEvent::PouchRevealed {
            owner: None, card, ..
        } => {
            for pile in &mut state.player_decks {
                if let Some(position) = pile.cards.iter().position(|candidate| candidate == card) {
                    pile.cards.remove(position);
                    break;
                }
            }
        }
        GameEvent::PouchRevealed { .. } => {}
        GameEvent::PouchConsumed { owner, card } => {
            if let Some(owner) = owner {
                let position = state
                    .pouches
                    .iter()
                    .position(|pouch| &pouch.owner == owner && pouch.card == *card)
                    .expect("canonical Pouch consumption must target an owned Pouch");
                state.pouches.remove(position);
            } else {
                for pile in &mut state.player_decks {
                    if let Some(position) =
                        pile.cards.iter().position(|candidate| candidate == card)
                    {
                        pile.cards.remove(position);
                        break;
                    }
                }
            }
            push_to_origin_discard(state, *card);
        }
        GameEvent::PouchLevelBonusGranted { bonus } => {
            state.pouch_level_bonuses.push(bonus.clone());
        }
        GameEvent::TemporaryStarEffectGranted { effect } => {
            state.temporary_star_effects.push(effect.clone());
        }
        GameEvent::SpiritRevived {
            player,
            spirit,
            power,
            ..
        } => {
            state.spirits.retain(|owned| &owned.player != player);
            state.spirits.push(crate::domain::PlayerSpirit {
                player: player.clone(),
                spirit: *spirit,
                power: *power,
            });
            state.spirit_skill_use_turns.remove(player);
        }
        GameEvent::DeckPrepared { deck_order } => {
            state.deck = deck_order.clone();
        }
        GameEvent::PlayerDeckPrepared { player, deck_order } => {
            let pile = state
                .player_decks
                .iter_mut()
                .find(|pile| &pile.player == player)
                .expect("canonical player deck event must target a known player");
            pile.cards = deck_order.clone();
        }
        GameEvent::CardsDealt { player, cards } => {
            let hand = state
                .hand_mut(player)
                .expect("canonical deal event must target a known player");
            hand.extend(cards.iter().copied());

            let deck = state
                .deck_for_mut(player)
                .expect("canonical deal event must target a known player deck");
            for card in cards {
                let position = deck
                    .iter()
                    .position(|deck_card| deck_card == card)
                    .expect("canonical deal event must contain cards from deck");
                deck.remove(position);
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
            clear_prepared_ability(state, player);
        }
        GameEvent::ProfessionChanged {
            player,
            profession,
            card_moves,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::Main);
            for card_move in card_moves {
                apply_card_move(state, card_move);
            }
            clear_confluence_obligations_for_cards(
                state,
                player,
                &card_moves
                    .iter()
                    .map(|movement| movement.card)
                    .collect::<Vec<_>>(),
            );
            if let Some(owned) = state
                .professions
                .iter_mut()
                .find(|owned| &owned.player == player)
            {
                owned.profession = profession.clone();
            } else {
                state.professions.push(crate::domain::PlayerProfession {
                    player: player.clone(),
                    profession: profession.clone(),
                });
            }
            state.phase = crate::domain::Phase::TurnDraw;
            clear_prepared_ability(state, player);
        }
        GameEvent::ProfessionTransformed {
            player, profession, ..
        } => {
            if let Some(owned) = state
                .professions
                .iter_mut()
                .find(|owned| &owned.player == player)
            {
                owned.profession = profession.clone();
            } else {
                state.professions.push(crate::domain::PlayerProfession {
                    player: player.clone(),
                    profession: profession.clone(),
                });
            }
        }
        GameEvent::ProfessionBroken { player, profession } => {
            let position = state
                .professions
                .iter()
                .position(|owned| &owned.player == player && &owned.profession == profession)
                .expect("canonical Profession breaking must target an owned Profession");
            state.professions.remove(position);
        }
        GameEvent::ProfessionAbilityActivated {
            player, prepared, ..
        } => {
            state
                .activated_profession_ability_turns
                .insert(player.clone(), state.turn_number);
            clear_prepared_ability(state, player);
            if let Some(prepared) = prepared {
                state.card_interpretation_revision = state
                    .card_interpretation_revision
                    .max(prepared.interpretation_revision);
                state.prepared_profession_abilities.push(prepared.clone());
            }
        }
        GameEvent::FormationRequirementSet { requirement } => {
            state
                .formation_requirements
                .retain(|existing| &existing.player != &requirement.player);
            state.formation_requirements.push(requirement.clone());
        }
        GameEvent::FormationRequirementFulfilled { player, .. } => {
            state
                .formation_requirements
                .retain(|requirement| &requirement.player != player);
        }
        GameEvent::SpiritSummoned { player, spirit, .. } => {
            state.spirit_skill_use_turns.remove(player);
            if let Some(owned) = state
                .spirits
                .iter_mut()
                .find(|owned| &owned.player == player)
            {
                owned.spirit = *spirit;
                owned.power = 2;
            } else {
                state.spirits.push(crate::domain::PlayerSpirit {
                    player: player.clone(),
                    spirit: *spirit,
                    power: 2,
                });
            }
        }
        GameEvent::SpiritTransformed {
            player,
            previous,
            spirit,
            power,
        } => {
            let owned = state
                .spirits
                .iter_mut()
                .find(|owned| &owned.player == player && owned.spirit == *previous)
                .expect("canonical Spirit transformation must target the owned Spirit");
            owned.spirit = *spirit;
            owned.power = *power;
            state.spirit_skill_use_turns.remove(player);
        }
        GameEvent::SpiritPowerChanged {
            player,
            spirit,
            old_power,
            new_power,
            ..
        } => {
            let owned = state
                .spirits
                .iter_mut()
                .find(|owned| &owned.player == player && owned.spirit == *spirit)
                .expect("canonical Spirit Power change must target the owned Spirit");
            debug_assert_eq!(owned.power, *old_power);
            owned.power = *new_power;
        }
        GameEvent::SpiritSkillUsed {
            player,
            spirit,
            old_power,
            new_power,
            ..
        } => {
            let owned = state
                .spirits
                .iter_mut()
                .find(|owned| &owned.player == player && owned.spirit == *spirit)
                .expect("canonical Spirit Skill must target the owned Spirit");
            debug_assert_eq!(owned.power, *old_power);
            owned.power = *new_power;
            state
                .spirit_skill_use_turns
                .insert(player.clone(), state.turn_number);
        }
        GameEvent::SpiritLevelInterpreted {
            player,
            skill,
            card,
            level,
            applied_on_turn,
            interpretation_revision,
        } => {
            state.card_interpretation_revision = state
                .card_interpretation_revision
                .max(*interpretation_revision);
            state
                .spirit_level_interpretations
                .push(crate::domain::SpiritLevelInterpretation {
                    player: player.clone(),
                    skill: *skill,
                    card: *card,
                    level: *level,
                    applied_on_turn: *applied_on_turn,
                    interpretation_revision: *interpretation_revision,
                });
        }
        GameEvent::SpiritBroken { player, spirit, .. } => {
            let position = state
                .spirits
                .iter()
                .position(|owned| &owned.player == player && owned.spirit == *spirit)
                .expect("canonical Spirit breaking must target the owned Spirit");
            state.spirits.remove(position);
            state.spirit_skill_use_turns.remove(player);
        }
        GameEvent::AutomaticBloomsResolved { resolutions } => {
            for resolution in resolutions {
                for change in &resolution.spirit_changes {
                    let owned = state
                        .spirits
                        .iter_mut()
                        .find(|owned| {
                            owned.player == change.player && owned.spirit == change.spirit
                        })
                        .expect("canonical Bloom must target an owned Spirit");
                    debug_assert_eq!(owned.power, change.old_power);
                    owned.power = change.new_power;
                    if change.new_power == 0 {
                        state.spirit_skill_use_turns.remove(&change.player);
                    }
                }
                let team_hp = state
                    .hp
                    .iter_mut()
                    .find(|team_hp| team_hp.team == resolution.team)
                    .expect("canonical Bloom must target an existing Team");
                debug_assert_eq!(team_hp.hp, resolution.hp_change.old_hp);
                team_hp.hp = resolution.hp_change.new_hp;
            }
            state.spirits.retain(|owned| owned.power > 0);
            finish_game_if_needed(state);
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
                push_to_origin_discard(state, removed);
            }
            clear_confluence_obligations_for_cards(state, player, used_cards);

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
            clear_prepared_ability(state, player);
            state
                .formation_requirements
                .retain(|requirement| &requirement.player != player);
        }
        GameEvent::FormationMatchOptionDeclared { .. } => {}
        GameEvent::FormationEffectCopied { player, effect_id } => {
            let last_formation = state
                .last_formation_by_player
                .get_mut(player)
                .expect("copied effect must follow a formation use");
            last_formation.resolved_effect_id = effect_id.clone();
        }
        GameEvent::FormationEffectIgnored { .. } => {}
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
        GameEvent::DeckTopRevealed { .. } => {}
        GameEvent::PassiveCovered {
            player,
            formation_id,
            cards,
            star_substitution,
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
                star_substitution: star_substitution.clone(),
                sealed: *sealed,
                covered_on_turn: state.turn_number,
                reveal_timing: crate::domain::PassiveTriggerTiming::NextPlayerActionStart,
            });
            clear_confluence_obligations_for_cards(state, player, cards);
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
            clear_prepared_ability(state, player);
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
            state
                .neutralized_covered_passive_owners
                .retain(|player| player != owner);
            state
                .revealed_covered_passive_owners
                .retain(|player| player != owner);
            for card in cards {
                push_to_origin_discard(state, *card);
            }
        }
        GameEvent::PassiveCoverRevealed { owner } => {
            if !state.revealed_covered_passive_owners.contains(owner) {
                state.revealed_covered_passive_owners.push(owner.clone());
            }
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
            clear_confluence_obligations_for_cards(state, attacker, used_cards);

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
            clear_prepared_ability(state, attacker);
            state
                .formation_requirements
                .retain(|requirement| &requirement.player != attacker);
        }
        GameEvent::EnvironmentTransferred { to, .. } => {
            state.environment = Some(*to);
        }
        GameEvent::EnvironmentCleared { hp_changes, .. } => {
            state.environment = None;
            for change in hp_changes {
                let team_hp = state
                    .hp
                    .iter_mut()
                    .find(|team_hp| team_hp.team == change.team)
                    .expect("canonical environment clearing must target an existing team");
                team_hp.hp = change.new_hp;
            }
            finish_game_if_needed(state);
        }
        GameEvent::StarBroken {
            team,
            star,
            hp_change,
            ..
        } => {
            let position = state
                .team_stars
                .iter()
                .position(|owned| &owned.team == team && owned.star == *star)
                .expect("canonical Star breaking must target an owned Star");
            state.team_stars.remove(position);

            if let Some(change) = hp_change {
                let team_hp = state
                    .hp
                    .iter_mut()
                    .find(|team_hp| team_hp.team == change.team)
                    .expect("canonical Star breaking must target an existing Team");
                team_hp.hp = change.new_hp;
            }
        }
        GameEvent::StarSummoned { player, team, star } => {
            debug_assert!(state.star_for_team(team).is_none());
            debug_assert!(!state.team_stars.iter().any(|owned| owned.star == *star));
            state.team_stars.push(crate::domain::TeamStar {
                team: team.clone(),
                star: *star,
            });
            let history = state
                .star_histories
                .iter_mut()
                .find(|history| &history.player == player)
                .expect("canonical Star summoning must target a known Player");
            if !history.stars.contains(star) {
                history.stars.push(*star);
            }
        }
        GameEvent::VoidStarBreakingCompleted { .. } => {
            finish_game_if_needed(state);
        }
        GameEvent::VoidReversionResolved {
            player,
            hp_change,
            card_moves,
            broken_professions,
            ..
        } => {
            for card_move in card_moves {
                apply_card_move(state, card_move);
            }
            let team_hp = state
                .hp
                .iter_mut()
                .find(|team_hp| team_hp.team == hp_change.team)
                .expect("canonical Void Reversion must target an existing Team");
            team_hp.hp = hp_change.new_hp;
            state
                .professions
                .retain(|owned| !broken_professions.iter().any(|broken| broken == owned));
            state.last_formation_by_player.insert(
                player.clone(),
                LastFormationUse {
                    formation_id: "void-reversion".to_string(),
                    resolved_effect_id: "void-reversion".to_string(),
                    used_cards: card_moves.iter().map(|movement| movement.card).collect(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = crate::domain::Phase::TurnDraw;
            clear_prepared_ability(state, player);
            state
                .formation_requirements
                .retain(|requirement| &requirement.player != player);
            finish_game_if_needed(state);
        }
        GameEvent::VoidSpiritShatteringResolved {
            player,
            card_moves,
            spirit_changes,
            broken_spirits,
            hp_changes,
            broken_professions,
            revived_spirits,
            shared_fate_hp_changes,
        } => {
            for card_move in card_moves {
                apply_card_move(state, card_move);
            }
            for change in spirit_changes {
                let owned = state
                    .spirits
                    .iter_mut()
                    .find(|owned| owned.player == change.player && owned.spirit == change.spirit)
                    .expect("canonical Void Spirit-Shattering must target an owned Spirit");
                debug_assert_eq!(owned.power, change.old_power);
                owned.power = change.new_power;
            }
            for broken in broken_spirits {
                state.spirit_skill_use_turns.remove(&broken.player);
            }
            state.spirits.retain(|owned| owned.power > 0);
            state
                .professions
                .retain(|owned| !broken_professions.contains(owned));
            for revived in revived_spirits {
                state.spirits.retain(|owned| owned.player != revived.player);
                state.spirits.push(revived.clone());
                state.spirit_skill_use_turns.remove(&revived.player);
            }
            for change in hp_changes {
                let team_hp = state
                    .hp
                    .iter_mut()
                    .find(|team_hp| team_hp.team == change.team)
                    .expect("canonical Void Spirit-Shattering must target an existing Team");
                debug_assert_eq!(team_hp.hp, change.old_hp);
                team_hp.hp = change.new_hp;
            }
            for change in shared_fate_hp_changes {
                let team_hp = state
                    .hp
                    .iter_mut()
                    .find(|team_hp| team_hp.team == change.team)
                    .expect("canonical Shared Fate must target an existing Team");
                debug_assert_eq!(team_hp.hp, change.old_hp);
                team_hp.hp = change.new_hp;
            }
            state.last_formation_by_player.insert(
                player.clone(),
                LastFormationUse {
                    formation_id: "void-spirit-shattering".to_string(),
                    resolved_effect_id: "void-spirit-shattering".to_string(),
                    used_cards: card_moves.iter().map(|movement| movement.card).collect(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = crate::domain::Phase::TurnDraw;
            clear_prepared_ability(state, player);
            finish_game_if_needed(state);
        }
        GameEvent::FiveStarAlignmentAchieved { player, team } => {
            state.five_star_alignment = Some(crate::domain::FiveStarAlignment {
                player: player.clone(),
                team: team.clone(),
            });
            state.status = GameStatus::Finished {
                outcome: GameOutcome::Team(team.clone()),
            };
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
        GameEvent::JianghuStateApplied {
            state: applied_state,
        } => {
            if let Some(active) = state.jianghu_states.iter_mut().find(|active| {
                active.owner == applied_state.owner && active.kind == applied_state.kind
            }) {
                *active = applied_state.clone();
            } else {
                state.jianghu_states.push(applied_state.clone());
            }
        }
        GameEvent::JianghuStateExpired { owner, kind } => {
            let position = state
                .jianghu_states
                .iter()
                .position(|active| &active.owner == owner && active.kind == *kind)
                .expect("canonical Jianghu State expiry must target an active State");
            state.jianghu_states.remove(position);
        }
        GameEvent::JianghuPoisonTicked {
            owner,
            remaining_turns,
            hp_change,
            shared_fate_hp_change,
            ..
        } => {
            let team_hp = state
                .hp
                .iter_mut()
                .find(|team_hp| team_hp.team == hp_change.team)
                .expect("canonical Poison tick must target an existing Team");
            debug_assert_eq!(team_hp.hp, hp_change.old_hp);
            team_hp.hp = hp_change.new_hp;
            let position = state
                .jianghu_states
                .iter()
                .position(|active| {
                    &active.owner == owner && active.kind == crate::domain::JianghuStateKind::Poison
                })
                .expect("canonical Poison tick must target active Poison");
            if *remaining_turns == 0 {
                state.jianghu_states.remove(position);
            } else {
                state.jianghu_states[position].remaining_turns = *remaining_turns;
                state.jianghu_states[position].last_resolved_turn = Some(state.turn_number);
            }
            if let Some(change) = shared_fate_hp_change {
                let target_hp = state
                    .hp
                    .iter_mut()
                    .find(|team_hp| team_hp.team == change.team)
                    .expect("canonical Poison Shared Fate targets an existing Team");
                debug_assert_eq!(target_hp.hp, change.old_hp);
                target_hp.hp = change.new_hp;
            }
            finish_game_if_needed(state);
        }
        GameEvent::JianghuDelayedDamageResolved {
            owner,
            status_id,
            hp_change,
            shared_fate_hp_change,
        } => {
            let position = state
                .statuses
                .iter()
                .position(|status| {
                    status.id == *status_id
                        && status.owner == crate::domain::StatusOwner::Player(owner.clone())
                })
                .expect("canonical delayed Jianghu damage must target an active Status");
            state.statuses.remove(position);
            let team_hp = state
                .hp
                .iter_mut()
                .find(|team_hp| team_hp.team == hp_change.team)
                .expect("canonical delayed Jianghu damage must target an existing Team");
            team_hp.hp = hp_change.new_hp;
            if let Some(change) = shared_fate_hp_change {
                let target_hp = state
                    .hp
                    .iter_mut()
                    .find(|team_hp| team_hp.team == change.team)
                    .expect("canonical delayed Shared Fate targets an existing Team");
                debug_assert_eq!(target_hp.hp, change.old_hp);
                target_hp.hp = change.new_hp;
            }
            finish_game_if_needed(state);
        }
        GameEvent::LimitedUseChanged {
            owner,
            key,
            new_remaining,
            maximum,
            ..
        } => {
            if let Some(use_count) = state
                .limited_uses
                .iter_mut()
                .find(|use_count| &use_count.owner == owner && use_count.key == *key)
            {
                use_count.remaining = *new_remaining;
                use_count.maximum = *maximum;
            } else {
                state.limited_uses.push(crate::domain::LimitedUse {
                    owner: owner.clone(),
                    key: key.clone(),
                    remaining: *new_remaining,
                    maximum: *maximum,
                });
            }
        }
        GameEvent::ConfluenceCardObligationSet { obligation } => {
            state
                .confluence_card_obligations
                .retain(|active| active.owner != obligation.owner);
            state.confluence_card_obligations.push(obligation.clone());
        }
        GameEvent::ConfluenceCardObligationCleared { owner, card } => {
            state
                .confluence_card_obligations
                .retain(|active| &active.owner != owner || active.card != *card);
        }
        GameEvent::ChoiceRequested { choice } => {
            if matches!(state.status, GameStatus::Finished { .. }) {
                state.status = GameStatus::InProgress;
            }
            assert!(
                state.pending_choice.is_none(),
                "canonical choice request cannot replace a pending choice"
            );
            assert_eq!(choice.choice_id, state.next_choice_id);
            state.next_choice_id = choice.choice_id.next();
            state.pending_choice = Some(choice.clone());
        }
        GameEvent::ChoiceMade {
            player,
            choice_id,
            answer,
        } => {
            let choice = state
                .pending_choice
                .as_ref()
                .expect("canonical choice answer must have a pending choice");
            crate::rules::pending_choice::validate_answer(choice, player, *choice_id, answer)
                .expect("canonical choice answer must match the pending choice");
            state.pending_choice = None;
            finish_game_if_needed(state);
        }
        GameEvent::RandomnessRequested { request } => {
            assert!(
                state.pending_randomness.is_none(),
                "canonical randomness request cannot replace a pending request"
            );
            state.pending_randomness = Some(request.clone());
        }
        GameEvent::RandomnessResolved {
            request_id,
            operation,
            shuffled_order,
        } => {
            let pending = state
                .pending_randomness
                .as_ref()
                .expect("canonical randomness result must have a pending request");
            assert_eq!(
                (&pending.request_id, &pending.operation),
                (request_id, operation),
                "canonical randomness result must match the pending request"
            );
            let recycled_cards = pending.current_order.clone();
            match operation {
                crate::domain::RandomnessOperation::DeckShuffle { deck } => match deck {
                    crate::domain::RandomnessDeck::Shared => state.deck = shuffled_order.clone(),
                    crate::domain::RandomnessDeck::Player(player) => {
                        state
                            .deck_for_mut(player)
                            .expect("canonical randomness result must target a known player deck")
                            .clone_from(shuffled_order);
                    }
                },
                crate::domain::RandomnessOperation::DiscardShuffle { pile, placement } => {
                    match (pile, placement) {
                        (crate::domain::RandomnessDeck::Shared, DeckPlacement::Bottom) => {
                            state.deck.extend(shuffled_order.iter().copied());
                            for card in &recycled_cards {
                                let position = state
                                    .discard
                                    .iter()
                                    .position(|discarded| discarded == card)
                                    .expect("Discard Shuffle source Card must remain in the Discard Pile");
                                state.discard.remove(position);
                            }
                        }
                        (crate::domain::RandomnessDeck::Player(player), DeckPlacement::Bottom) => {
                            state
                                .deck_for_mut(player)
                                .expect("Discard Shuffle must target a known player deck")
                                .extend(shuffled_order.iter().copied());
                            let discard = state
                                .discard_for_mut(player)
                                .expect("Discard Shuffle must target a known player discard");
                            for card in &recycled_cards {
                                let position = discard
                                    .iter()
                                    .position(|discarded| discarded == card)
                                    .expect("Discard Shuffle source Card must remain in the Discard Pile");
                                discard.remove(position);
                            }
                        }
                    }
                }
            }
            state.pending_randomness = None;
        }
        GameEvent::EchoCostPaid { card_move, .. } => {
            apply_card_move(state, card_move);
        }
        GameEvent::EchoDeclined { .. } => {}
        GameEvent::EchoScheduled { schedule } => {
            state.scheduled_echoes.push(schedule.clone());
        }
        GameEvent::EchoResolutionStarted { schedule } => {
            let position = state
                .scheduled_echoes
                .iter()
                .position(|pending| pending == schedule)
                .expect("canonical Echo start must target a scheduled Echo");
            state.scheduled_echoes.remove(position);
            state.active_echo_resolution = Some(schedule.clone());
        }
        GameEvent::EchoResolutionCompleted { .. } => {
            state.active_echo_resolution = None;
        }
        GameEvent::TimedEffectsReduced { reductions, .. } => {
            for reduction in reductions {
                match reduction {
                    crate::domain::TimedEffectReduction::Status {
                        status_id,
                        owner,
                        new_duration,
                        ..
                    } => {
                        let position = state
                            .statuses
                            .iter()
                            .position(|status| &status.id == status_id && &status.owner == owner)
                            .expect("timed reduction must target an active Status Effect");
                        if let Some(duration) = new_duration {
                            state.statuses[position].duration = duration.clone();
                        } else {
                            state.statuses.remove(position);
                        }
                    }
                    crate::domain::TimedEffectReduction::CoveredPassive { owner } => {
                        assert!(
                            state
                                .covered_passives
                                .iter()
                                .any(|passive| &passive.owner == owner),
                            "timed reduction must target a Covered Passive"
                        );
                        if !state.neutralized_covered_passive_owners.contains(owner) {
                            state.neutralized_covered_passive_owners.push(owner.clone());
                        }
                    }
                    crate::domain::TimedEffectReduction::CounterEffect { owner, effect_id } => {
                        let position = state
                            .counter_effects
                            .iter()
                            .position(|counter| {
                                &counter.owner == owner && &counter.effect_id == effect_id
                            })
                            .expect("timed reduction must target a Counter Effect");
                        state.counter_effects.remove(position);
                    }
                    crate::domain::TimedEffectReduction::JianghuState {
                        owner,
                        kind,
                        new_remaining_turns,
                        new_expires_on_turn,
                        ..
                    } => {
                        let position = state
                            .jianghu_states
                            .iter()
                            .position(|active| &active.owner == owner && &active.kind == kind)
                            .expect("timed reduction must target a Jianghu State");
                        if *new_remaining_turns == 0 && new_expires_on_turn.is_none() {
                            state.jianghu_states.remove(position);
                        } else {
                            state.jianghu_states[position].remaining_turns = *new_remaining_turns;
                            state.jianghu_states[position].expires_on_turn = *new_expires_on_turn;
                        }
                    }
                    crate::domain::TimedEffectReduction::FlowState {
                        player, new_layers, ..
                    } => {
                        if *new_layers == 0 {
                            state.flow_layers_by_player.remove(player);
                        } else {
                            state
                                .flow_layers_by_player
                                .insert(player.clone(), *new_layers);
                        }
                    }
                    crate::domain::TimedEffectReduction::FormationSuppression {
                        target,
                        formation_id,
                    } => {
                        state.formation_suppressions.retain(|active| {
                            &active.target != target || &active.formation_id != formation_id
                        });
                    }
                }
            }
        }
        GameEvent::FlowStateChanged {
            player, new_layers, ..
        } => {
            if *new_layers == 0 {
                state.flow_layers_by_player.remove(player);
            } else {
                state
                    .flow_layers_by_player
                    .insert(player.clone(), *new_layers);
            }
        }
        GameEvent::FlowStateTriggered {
            player,
            new_layers,
            new_draw_bonus,
            ..
        } => {
            if *new_layers == 0 {
                state.flow_layers_by_player.remove(player);
            } else {
                state
                    .flow_layers_by_player
                    .insert(player.clone(), *new_layers);
            }
            state
                .turn_draw_bonus_by_player
                .insert(player.clone(), *new_draw_bonus);
            state
                .flow_triggered_turn_by_player
                .insert(player.clone(), state.turn_number);
        }
        GameEvent::FormationSuppressionSet { suppression } => {
            state
                .formation_suppressions
                .retain(|active| active.target != suppression.target);
            state.formation_suppressions.push(suppression.clone());
        }
        GameEvent::FormationSuppressionExpired {
            target,
            formation_id,
            ..
        } => {
            state
                .formation_suppressions
                .retain(|active| &active.target != target || &active.formation_id != formation_id);
        }
        GameEvent::RingingMetalCardRevealed { selection } => {
            let deck = state
                .deck_for_mut(&selection.player)
                .expect("canonical Ringing Metal reveal must target a known deck");
            let position = deck
                .iter()
                .position(|card| card == &selection.card)
                .expect("canonical Ringing Metal reveal must select a card in the deck");
            deck.remove(position);
            state.ringing_metal_selection = Some(selection.clone());
        }
        GameEvent::RingingMetalCompleted { selection } => {
            state
                .deck_for_mut(&selection.player)
                .expect("canonical Ringing Metal completion must target a known deck")
                .insert(0, selection.card);
            state.ringing_metal_selection = None;
        }
        GameEvent::PlantEarthScheduled { schedule } => {
            state.scheduled_plant_earth.push(schedule.clone());
        }
        GameEvent::PlantEarthResolutionStarted { schedule } => {
            let position = state
                .scheduled_plant_earth
                .iter()
                .position(|pending| pending == schedule)
                .expect("canonical Plant Earth start must target a schedule");
            state.scheduled_plant_earth.remove(position);
            state.active_plant_earth_resolution = Some(schedule.clone());
        }
        GameEvent::PlantEarthResolutionCompleted { .. } => {
            state.active_plant_earth_resolution = None;
        }
        GameEvent::EarthRendingStarted { resolution } => {
            state.active_earth_rending_resolution = Some(resolution.clone());
        }
        GameEvent::EarthRendingEnvironmentChosen { environment } => {
            state
                .active_earth_rending_resolution
                .as_mut()
                .expect("Earth Rending environment choice requires an active resolution")
                .environment = Some(*environment);
        }
        GameEvent::EarthRendingPlayerAnswered { answer } => {
            let resolution = state
                .active_earth_rending_resolution
                .as_mut()
                .expect("Earth Rending answer requires an active resolution");
            assert_eq!(resolution.remaining_players.first(), Some(&answer.player));
            resolution.remaining_players.remove(0);
            resolution.answers.push(answer.clone());
        }
        GameEvent::HandRevealed { .. } => {}
        GameEvent::EarthRendingCompleted { .. } => {
            state.active_earth_rending_resolution = None;
        }
        GameEvent::RustedForestStarted { resolution } => {
            state.active_rusted_forest_resolution = Some(resolution.clone());
        }
        GameEvent::RustedForestCardsRevealed { .. } => {}
        GameEvent::RustedForestDeckProcessed { deck } => {
            let resolution = state
                .active_rusted_forest_resolution
                .as_mut()
                .expect("Rusted Forest deck completion requires active state");
            assert_eq!(resolution.remaining_decks.first(), Some(deck));
            resolution.remaining_decks.remove(0);
        }
        GameEvent::RustedForestCompleted { .. } => {
            state.active_rusted_forest_resolution = None;
        }
        GameEvent::CardsDrawnForProfessionChoice { player, cards, .. } => {
            let hand = state
                .hand_mut(player)
                .expect("canonical Profession draw must target a known Player");
            hand.extend(cards.iter().copied());
            let deck = state
                .deck_for_mut(player)
                .expect("canonical Profession draw must target a known Deck");
            for card in cards {
                let position = deck
                    .iter()
                    .position(|candidate| candidate == card)
                    .expect("canonical Profession draw must remove cards from the Deck");
                deck.remove(position);
            }
        }
        GameEvent::CardsDrawnForTurnDiscardChoice {
            player,
            drawn_cards,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnDraw);

            let hand = state
                .hand_mut(player)
                .expect("canonical draw event must target a known player");
            hand.extend(drawn_cards.iter().copied());
            state
                .deck_for_mut(player)
                .expect("canonical draw event must target a known player deck")
                .drain(0..drawn_cards.len());
            state.phase = crate::domain::Phase::TurnDrawDiscardChoice;
        }
        GameEvent::TurnDiscardChosen { player, discard } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnDrawDiscardChoice);

            let hand = state
                .hand_mut(player)
                .expect("canonical discard event must target a known player");
            let discard_position = hand
                .iter()
                .position(|card| card == discard)
                .expect("canonical discard event must remove a card from hand");
            let discarded = hand.remove(discard_position);

            push_to_origin_discard(state, discarded);
            state.last_turn_discard_by_player.insert(
                player.clone(),
                crate::domain::LastTurnDiscard {
                    card: discarded,
                    turn_number: state.turn_number,
                },
            );
            state.phase = crate::domain::Phase::TurnEnd;
        }
        GameEvent::TurnDrawSkipped { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnDraw);
            state.phase = crate::domain::Phase::TurnEnd;
        }
        GameEvent::DiscardRetrieved {
            hp_change,
            card_move,
            ..
        } => {
            apply_card_move(state, card_move);
            let team_hp = state
                .hp
                .iter_mut()
                .find(|team_hp| team_hp.team == hp_change.team)
                .expect("canonical discard retrieval must target an existing team");
            team_hp.hp = hp_change.new_hp;
            finish_game_if_needed(state);
        }
        GameEvent::TurnEnded { player } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, crate::domain::Phase::TurnEnd);

            state.turn_draw_bonus_by_player.remove(player);
            state.flow_triggered_turn_by_player.remove(player);
            state
                .spirit_level_interpretations
                .retain(|interpretation| &interpretation.player != player);
            state
                .pouch_level_bonuses
                .retain(|bonus| &bonus.player != player);
            state
                .temporary_star_effects
                .retain(|effect| &effect.player != player);
            state.current_turn_index = (state.current_turn_index + 1) % state.turn_order.len();
            state.turn_number += 1;
            state.phase = crate::domain::Phase::TurnStart;
            clear_prepared_ability(state, player);
        }
    }
}

fn clear_prepared_ability(state: &mut GameState, player: &crate::domain::PlayerId) {
    state
        .prepared_profession_abilities
        .retain(|prepared| &prepared.player != player);
}

fn clear_confluence_obligations_for_cards(
    state: &mut GameState,
    player: &crate::domain::PlayerId,
    cards: &[crate::domain::CardInstanceId],
) {
    state
        .confluence_card_obligations
        .retain(|obligation| &obligation.owner != player || !cards.contains(&obligation.card));
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
        CardZone::PlayerDeckTop(player) => {
            let deck = state
                .deck_for_mut(player)
                .expect("canonical card move must target a known player deck");
            let position = deck
                .iter()
                .position(|card| card == &card_move.card)
                .expect("canonical card move must move an existing player deck card");
            deck.remove(position)
        }
        CardZone::PlayerDiscard(player) => {
            let discard = state
                .discard_for_mut(player)
                .expect("canonical card move must target a known player discard");
            let position = discard
                .iter()
                .position(|card| card == &card_move.card)
                .expect("canonical card move must move an existing player discard card");
            discard.remove(position)
        }
        CardZone::Pouch(player) => {
            let position = state
                .pouches
                .iter()
                .position(|pouch| &pouch.owner == player && pouch.card == card_move.card)
                .expect("canonical card move must move an owned Pouch");
            state.pouches.remove(position).card
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
        CardZone::PlayerDeckTop(player) => {
            let is_foreign = matches!(
                state.card_origin(removed),
                Some(CardOrigin::Player(origin)) if origin != player
            );
            state
                .deck_for_mut(player)
                .expect("canonical card move must target a known player deck")
                .insert(0, removed);
            if is_foreign && !state.exposed_foreign_cards.contains(&removed) {
                state.exposed_foreign_cards.push(removed);
            }
        }
        CardZone::PlayerDiscard(player) => {
            state
                .discard_for_mut(player)
                .expect("canonical card move must target a known player discard")
                .push(removed);
            state
                .exposed_foreign_cards
                .retain(|exposed| *exposed != removed);
        }
        CardZone::Pouch(player) => {
            state.pouches.retain(|pouch| &pouch.owner != player);
            state.pouches.push(crate::domain::PlayerPouch {
                owner: player.clone(),
                card: removed,
                known_by: vec![player.clone()],
            });
        }
    }
}

fn push_to_origin_discard(state: &mut GameState, card: crate::domain::CardInstanceId) {
    if state.uses_personal_decks()
        && let Some(CardOrigin::Player(player)) = state.card_origin(card).cloned()
    {
        state
            .discard_for_mut(&player)
            .expect("card origin must identify a known player discard")
            .push(card);
        state
            .exposed_foreign_cards
            .retain(|exposed| *exposed != card);
    } else {
        state.discard.push(card);
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
    if state.has_rule_module(crate::domain::SPIRIT_MODULE_ID)
        && state
            .hp
            .iter()
            .filter(|team_hp| team_hp.hp == 0)
            .any(|team_hp| {
                state.spirits.iter().any(|owned| {
                    owned.spirit == crate::domain::SpiritKind::Wood
                        && owned.power == 6
                        && state
                            .players
                            .iter()
                            .find(|player| player.id == owned.player)
                            .is_some_and(|player| player.team == team_hp.team)
                })
            })
    {
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
