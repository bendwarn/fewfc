use crate::domain::{
    CardInstanceId, CardMoveDelta, CardOrigin, CardZone, Element, GameError, GameEvent, GameResult,
    GameState, HpChangeDelta, PlayerId, PlayerSpirit, SpiritBreakReason, SpiritKind,
    SpiritPowerDelta, SpiritSkill, StatusDuration, StatusEffect, StatusOwner, TeamBloomResolution,
    ValidationError,
};

use super::{
    BaseFormationSpec, EffectDef, EffectPlan, FormationCategory, FormationDef, FormationEffect,
    FormationPattern, PointFormula, SpellPlanDef, SpiritSkillCandidate, SpiritSkillEffect,
};

pub(super) fn specs() -> Vec<BaseFormationSpec> {
    let mut specs = all_spirits()
        .into_iter()
        .map(summoning_spec)
        .collect::<Vec<_>>();
    specs.push(void_spirit_shattering_spec());
    specs
}

pub(crate) fn all_spirits() -> [SpiritKind; 5] {
    [
        SpiritKind::Metal,
        SpiritKind::Wood,
        SpiritKind::Water,
        SpiritKind::Fire,
        SpiritKind::Earth,
    ]
}

pub(crate) fn element(spirit: SpiritKind) -> Element {
    match spirit {
        SpiritKind::Metal => Element::Metal,
        SpiritKind::Wood => Element::Wood,
        SpiritKind::Water => Element::Water,
        SpiritKind::Fire => Element::Fire,
        SpiritKind::Earth => Element::Earth,
        SpiritKind::Evil | SpiritKind::Death => {
            unreachable!("Dark Spirits have no elemental identity")
        }
    }
}

pub(crate) fn turn_discard_charges(
    state: &GameState,
    spirit: SpiritKind,
    card: CardInstanceId,
) -> bool {
    match spirit {
        SpiritKind::Evil => state.card_def(card).is_some_and(|card| card.level == 2),
        SpiritKind::Death => state.card_def(card).is_some_and(|card| card.level == 4),
        _ => state.card_element(card) == Some(element(spirit)),
    }
}

pub(crate) fn summoning_formation_spirit(formation_id: &str) -> Option<SpiritKind> {
    all_spirits()
        .into_iter()
        .find(|spirit| formation_id == summoning_id(*spirit))
}

pub(crate) fn stone_shield_prevents_attack(state: &GameState, attacker: &PlayerId) -> bool {
    state.statuses.iter().any(|status| {
        status.kind == "SpiritStoneShield"
            && matches!(&status.owner, StatusOwner::Player(player) if player == attacker)
    })
}

pub(crate) fn append_automatic_blooms(
    state: &GameState,
    events: &mut Vec<GameEvent>,
) -> GameResult<()> {
    if !state.has_rule_module(crate::domain::SPIRIT_MODULE_ID) {
        return Ok(());
    }

    let mut projected = state.clone();
    for event in events.iter() {
        super::projection::apply_event(&mut projected, event);
    }
    if matches!(projected.status, crate::domain::GameStatus::Finished { .. }) {
        return Ok(());
    }

    let mut resolutions = Vec::new();
    for team_hp in projected.hp.iter().filter(|team_hp| team_hp.hp == 0) {
        let spirit_changes = projected
            .spirits
            .iter()
            .filter(|owned| {
                owned.spirit == SpiritKind::Wood
                    && owned.power == 6
                    && team_for_player(&projected, &owned.player)
                        .is_ok_and(|team| team == team_hp.team)
            })
            .map(|owned| SpiritPowerDelta {
                player: owned.player.clone(),
                spirit: owned.spirit,
                old_power: owned.power,
                delta: -6,
                new_power: 0,
            })
            .collect::<Vec<_>>();
        if spirit_changes.is_empty() {
            continue;
        }

        let recovery = i32::try_from(spirit_changes.len())
            .unwrap_or(i32::MAX)
            .saturating_mul(40);
        let initial_hp = projected.initial_hp(&team_hp.team).ok_or_else(|| {
            GameError::Validation(ValidationError::MissingTeamHp(team_hp.team.clone()))
        })?;
        let new_hp = recovery.min(initial_hp);
        resolutions.push(TeamBloomResolution {
            team: team_hp.team.clone(),
            spirit_changes,
            hp_change: HpChangeDelta {
                team: team_hp.team.clone(),
                old_hp: 0,
                delta: recovery,
                new_hp,
                effective_delta: new_hp,
            },
        });
    }

    if !resolutions.is_empty() {
        events.push(GameEvent::AutomaticBloomsResolved { resolutions });
    }
    Ok(())
}

pub(crate) fn void_spirit_shattering_event(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<GameEvent> {
    let spirit_changes = state
        .spirits
        .iter()
        .map(|owned| {
            let new_power = owned.power.saturating_sub(2);
            SpiritPowerDelta {
                player: owned.player.clone(),
                spirit: owned.spirit,
                old_power: owned.power,
                delta: -2,
                new_power,
            }
        })
        .collect::<Vec<_>>();
    let broken_spirits = state
        .spirits
        .iter()
        .filter(|owned| owned.power <= 2)
        .cloned()
        .collect::<Vec<PlayerSpirit>>();

    let mut owner_counts = std::collections::HashMap::new();
    for owned in &state.spirits {
        let team = team_for_player(state, &owned.player)?;
        *owner_counts.entry(team).or_insert(0_i32) += 1;
    }
    let hp_changes: Vec<HpChangeDelta> = state
        .hp
        .iter()
        .filter_map(|team_hp| {
            let owner_count = owner_counts.get(&team_hp.team).copied().unwrap_or(0);
            if owner_count == 0 {
                return None;
            }
            let delta = owner_count.saturating_mul(-20);
            let new_hp = (team_hp.hp + delta).max(0);
            Some(HpChangeDelta {
                team: team_hp.team.clone(),
                old_hp: team_hp.hp,
                delta,
                new_hp,
                effective_delta: new_hp - team_hp.hp,
            })
        })
        .collect();
    let card_moves = cards
        .iter()
        .copied()
        .map(|card| CardMoveDelta {
            card,
            from: CardZone::Hand(player.clone()),
            to: discard_zone_for_card(state, card),
        })
        .collect();

    let broken_professions = broken_spirits
        .iter()
        .filter(|broken| matches!(broken.spirit, SpiritKind::Evil | SpiritKind::Death))
        .filter_map(|broken| {
            state
                .professions
                .iter()
                .find(|owned| {
                    owned.player == broken.player
                        && owned.profession.as_str() == crate::rules::dark::DEMON_SPIRIT_MASTER_ID
                })
                .cloned()
        })
        .collect::<Vec<_>>();
    let revived_spirits = broken_spirits
        .iter()
        .filter(|broken| {
            matches!(broken.spirit, SpiritKind::Evil | SpiritKind::Death)
                && broken_professions
                    .iter()
                    .any(|profession| profession.player == broken.player)
        })
        .map(|broken| PlayerSpirit {
            player: broken.player.clone(),
            spirit: broken.spirit,
            power: 2,
        })
        .collect::<Vec<_>>();

    let mut hp_after_primary = state
        .hp
        .iter()
        .map(|entry| (entry.team.clone(), entry.hp))
        .collect::<std::collections::HashMap<_, _>>();
    for change in &hp_changes {
        hp_after_primary.insert(change.team.clone(), change.new_hp);
    }
    let mut shared_fate_counts = std::collections::HashMap::new();
    for owned in state
        .spirits
        .iter()
        .filter(|owned| owned.spirit == SpiritKind::Death && owned.power > 2)
    {
        let owner_team = team_for_player(state, &owned.player)?;
        if !hp_changes
            .iter()
            .any(|change| change.team == owner_team && change.effective_delta < 0)
        {
            continue;
        }
        let target = next_player(state, &owned.player)?;
        let team = team_for_player(state, &target)?;
        *shared_fate_counts.entry(team).or_insert(0_i32) += 1;
    }
    let shared_fate_hp_changes = shared_fate_counts
        .into_iter()
        .map(|(team, count)| {
            let old_hp = hp_after_primary.get(&team).copied().unwrap_or(0);
            let delta = count.saturating_mul(-10);
            let new_hp = (old_hp + delta).max(0);
            HpChangeDelta {
                team,
                old_hp,
                delta,
                new_hp,
                effective_delta: new_hp - old_hp,
            }
        })
        .collect();

    Ok(GameEvent::VoidSpiritShatteringResolved {
        player: player.clone(),
        card_moves,
        spirit_changes,
        broken_spirits,
        hp_changes,
        broken_professions,
        revived_spirits,
        shared_fate_hp_changes,
    })
}

pub(crate) fn playable_skills(
    state: &GameState,
    player: &PlayerId,
    selected_cards: &[CardInstanceId],
) -> Vec<SpiritSkillCandidate> {
    let Some(owned) = state.spirit_for(player) else {
        return Vec::new();
    };
    if state.spirit_skill_use_turns.get(player) == Some(&state.turn_number) {
        return Vec::new();
    }
    let selected_card = selected_cards.first().copied().filter(|card| {
        selected_cards.len() == 1 && state.hand(player).is_some_and(|hand| hand.contains(card))
    });

    skills_for(owned.spirit)
        .into_iter()
        .flat_map(|skill| {
            let definition = skill_definition(skill);
            if owned.power < definition.cost {
                return Vec::new();
            }
            match skill {
                SpiritSkill::Flow => selected_card
                    .map(|card| vec![candidate(skill, definition, Some(card), None)])
                    .unwrap_or_default(),
                SpiritSkill::Glimmer => selected_card
                    .map(|card| vec![candidate(skill, definition, Some(card), Some(3))])
                    .unwrap_or_default(),
                SpiritSkill::Splendor => selected_card
                    .map(|card| {
                        (1..=5)
                            .map(|level| candidate(skill, definition, Some(card), Some(level)))
                            .collect()
                    })
                    .unwrap_or_default(),
                _ => vec![candidate(skill, definition, None, None)],
            }
        })
        .collect()
}

pub(crate) fn use_skill(
    state: &GameState,
    player: &PlayerId,
    skill: SpiritSkill,
    selected_card: Option<CardInstanceId>,
    declared_level: Option<u32>,
    trusted_random_cards: Option<&[CardInstanceId]>,
) -> GameResult<Vec<GameEvent>> {
    if !state.has_rule_module(crate::domain::SPIRIT_MODULE_ID) {
        return Err(GameError::Validation(ValidationError::SpiritRuleDisabled));
    }
    let owned = state.spirit_for(player).ok_or_else(|| {
        GameError::Validation(ValidationError::NoSpirit {
            player: player.clone(),
        })
    })?;
    let definition = skill_definition(skill);
    if definition.spirit != owned.spirit {
        return Err(GameError::Validation(
            ValidationError::SpiritSkillUnavailable {
                spirit: owned.spirit,
                skill,
            },
        ));
    }
    if state.spirit_skill_use_turns.get(player) == Some(&state.turn_number) {
        return Err(GameError::Validation(
            ValidationError::SpiritSkillAlreadyUsed {
                player: player.clone(),
                turn_number: state.turn_number,
            },
        ));
    }
    if owned.power < definition.cost {
        return Err(GameError::Validation(
            ValidationError::InsufficientSpiritPower {
                required: definition.cost,
                actual: owned.power,
            },
        ));
    }
    validate_input(state, player, skill, selected_card, declared_level)?;

    let new_power = owned.power - definition.cost;
    let mut events = vec![GameEvent::SpiritSkillUsed {
        player: player.clone(),
        spirit: owned.spirit,
        skill,
        old_power: owned.power,
        new_power,
        selected_card,
        declared_level,
    }];
    if crate::rules::pouch::spirit_is_suppressed(state, player) {
        return Ok(events);
    }
    events.extend(skill_effect_events(
        state,
        player,
        skill,
        selected_card,
        declared_level,
        trusted_random_cards,
    )?);
    crate::rules::dark::append_mischief_events(state, &mut events)?;
    let mut projected = state.clone();
    for event in &events {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    if projected
        .spirit_for(player)
        .is_some_and(|spirit| spirit.power == 0)
    {
        events.push(GameEvent::SpiritBroken {
            player: player.clone(),
            spirit: owned.spirit,
            reason: SpiritBreakReason::PowerDepleted,
        });
    }
    Ok(events)
}

#[derive(Clone, Copy)]
struct SkillDefinition {
    spirit: SpiritKind,
    cost: u32,
    name: &'static str,
}

fn skill_definition(skill: SpiritSkill) -> SkillDefinition {
    match skill {
        SpiritSkill::FlyingBlade => SkillDefinition {
            spirit: SpiritKind::Metal,
            cost: 2,
            name: "飛刃",
        },
        SpiritSkill::SwordRain => SkillDefinition {
            spirit: SpiritKind::Metal,
            cost: 6,
            name: "劍雨",
        },
        SpiritSkill::Fragrance => SkillDefinition {
            spirit: SpiritKind::Wood,
            cost: 2,
            name: "芬芳",
        },
        SpiritSkill::Bloom => SkillDefinition {
            spirit: SpiritKind::Wood,
            cost: 6,
            name: "綻放",
        },
        SpiritSkill::Flow => SkillDefinition {
            spirit: SpiritKind::Water,
            cost: 1,
            name: "川流",
        },
        SpiritSkill::Vastness => SkillDefinition {
            spirit: SpiritKind::Water,
            cost: 3,
            name: "浩瀚",
        },
        SpiritSkill::Glimmer => SkillDefinition {
            spirit: SpiritKind::Fire,
            cost: 1,
            name: "螢光",
        },
        SpiritSkill::Splendor => SkillDefinition {
            spirit: SpiritKind::Fire,
            cost: 3,
            name: "絢爛",
        },
        SpiritSkill::StoneShield => SkillDefinition {
            spirit: SpiritKind::Earth,
            cost: 2,
            name: "石盾",
        },
        SpiritSkill::RockWall => SkillDefinition {
            spirit: SpiritKind::Earth,
            cost: 6,
            name: "岩壁",
        },
        SpiritSkill::EvilGaze => SkillDefinition {
            spirit: SpiritKind::Evil,
            cost: 2,
            name: "惡視",
        },
        SpiritSkill::DeathOmen => SkillDefinition {
            spirit: SpiritKind::Death,
            cost: 4,
            name: "死兆",
        },
    }
}

pub(crate) fn skill_cost(skill: SpiritSkill) -> u32 {
    skill_definition(skill).cost
}

/// Kept beside `skill_effect_events`: changing a skill's resolver requires an
/// explicit player-facing semantic fact as well.
pub(crate) fn player_facing_effect(skill: SpiritSkill) -> SpiritSkillEffect {
    match skill {
        SpiritSkill::FlyingBlade => SpiritSkillEffect::DamagePreviousTeam { amount: 10 },
        SpiritSkill::SwordRain => SpiritSkillEffect::DamagePreviousTeam { amount: 40 },
        SpiritSkill::Fragrance => SpiritSkillEffect::RecoverOwnTeam { amount: 10 },
        SpiritSkill::Bloom => SpiritSkillEffect::RecoverOwnTeam { amount: 40 },
        SpiritSkill::Flow => {
            SpiritSkillEffect::DiscardSelectedCardAndIncreaseTurnDraw { amount: 1 }
        }
        SpiritSkill::Vastness => SpiritSkillEffect::IncreaseTurnDraw { amount: 1 },
        SpiritSkill::Glimmer | SpiritSkill::Splendor => {
            SpiritSkillEffect::InterpretSelectedCardLevel
        }
        SpiritSkill::StoneShield => SpiritSkillEffect::ProtectNextPlayerFromAttack,
        SpiritSkill::RockWall => SpiritSkillEffect::SetOwnShield { amount: 40 },
        SpiritSkill::EvilGaze => SpiritSkillEffect::InspectRandomNextPlayerHandCards { count: 2 },
        SpiritSkill::DeathOmen => SpiritSkillEffect::DiscardNextPlayerDeckAndDamageByHighestLevel {
            count: 4,
            multiplier: 4,
        },
    }
}

fn skills_for(spirit: SpiritKind) -> Vec<SpiritSkill> {
    match spirit {
        SpiritKind::Metal => vec![SpiritSkill::FlyingBlade, SpiritSkill::SwordRain],
        SpiritKind::Wood => vec![SpiritSkill::Fragrance, SpiritSkill::Bloom],
        SpiritKind::Water => vec![SpiritSkill::Flow, SpiritSkill::Vastness],
        SpiritKind::Fire => vec![SpiritSkill::Glimmer, SpiritSkill::Splendor],
        SpiritKind::Earth => vec![SpiritSkill::StoneShield, SpiritSkill::RockWall],
        SpiritKind::Evil => vec![SpiritSkill::EvilGaze],
        SpiritKind::Death => vec![SpiritSkill::DeathOmen],
    }
}

fn candidate(
    skill: SpiritSkill,
    definition: SkillDefinition,
    selected_card: Option<CardInstanceId>,
    declared_level: Option<u32>,
) -> SpiritSkillCandidate {
    SpiritSkillCandidate {
        skill,
        skill_name: definition.name.to_string(),
        selected_card,
        declared_level,
        detail: crate::rules::PlayerFacingActionDetail::pending_composition(),
    }
}

fn validate_input(
    state: &GameState,
    player: &PlayerId,
    skill: SpiritSkill,
    selected_card: Option<CardInstanceId>,
    declared_level: Option<u32>,
) -> GameResult<()> {
    let selected_in_hand = selected_card
        .is_some_and(|card| state.hand(player).is_some_and(|hand| hand.contains(&card)));
    let valid = match skill {
        SpiritSkill::Flow => selected_in_hand && declared_level.is_none(),
        SpiritSkill::Glimmer => selected_in_hand && declared_level == Some(3),
        SpiritSkill::Splendor => {
            selected_in_hand && declared_level.is_some_and(|level| (1..=5).contains(&level))
        }
        _ => selected_card.is_none() && declared_level.is_none(),
    };
    if valid {
        Ok(())
    } else {
        Err(GameError::Validation(
            ValidationError::SpiritSkillInputInvalid { skill },
        ))
    }
}

fn skill_effect_events(
    state: &GameState,
    player: &PlayerId,
    skill: SpiritSkill,
    selected_card: Option<CardInstanceId>,
    declared_level: Option<u32>,
    trusted_random_cards: Option<&[CardInstanceId]>,
) -> GameResult<Vec<GameEvent>> {
    match skill {
        SpiritSkill::FlyingBlade => Ok(vec![hp_event(
            state,
            &team_for_player(state, &previous_player(state, player)?)?,
            -10,
        )?]),
        SpiritSkill::SwordRain => Ok(vec![hp_event(
            state,
            &team_for_player(state, &previous_player(state, player)?)?,
            -40,
        )?]),
        SpiritSkill::Fragrance => Ok(vec![hp_event(state, &team_for_player(state, player)?, 10)?]),
        SpiritSkill::Bloom => Ok(vec![hp_event(state, &team_for_player(state, player)?, 40)?]),
        SpiritSkill::Flow => {
            let card = selected_card.expect("validated Flow must select one Card");
            let old_value = state
                .turn_draw_bonus_by_player
                .get(player)
                .copied()
                .unwrap_or(0);
            Ok(vec![
                GameEvent::CardsMoved {
                    card_moves: vec![CardMoveDelta {
                        card,
                        from: CardZone::Hand(player.clone()),
                        to: discard_zone_for_card(state, card),
                    }],
                },
                GameEvent::TurnDrawBonusChanged {
                    player: player.clone(),
                    old_value,
                    delta: 1,
                    new_value: old_value + 1,
                },
            ])
        }
        SpiritSkill::Vastness => {
            let old_value = state
                .turn_draw_bonus_by_player
                .get(player)
                .copied()
                .unwrap_or(0);
            Ok(vec![GameEvent::TurnDrawBonusChanged {
                player: player.clone(),
                old_value,
                delta: 1,
                new_value: old_value + 1,
            }])
        }
        SpiritSkill::Glimmer | SpiritSkill::Splendor => {
            Ok(vec![GameEvent::SpiritLevelInterpreted {
                player: player.clone(),
                skill: Some(skill),
                card: selected_card.expect("validated Fire Skill must select one Card"),
                level: declared_level.expect("validated Fire Skill must declare a level"),
                applied_on_turn: state.turn_number,
                interpretation_revision: state.card_interpretation_revision + 1,
            }])
        }
        SpiritSkill::StoneShield => {
            let target = next_player(state, player)?;
            Ok(vec![GameEvent::StatusAdded {
                status: StatusEffect {
                    id: format!(
                        "spirit-stone-shield-{}-turn-{}",
                        target.as_str(),
                        state.turn_number + 1
                    ),
                    owner: StatusOwner::Player(target.clone()),
                    kind: "SpiritStoneShield".to_string(),
                    value: None,
                    duration: StatusDuration::UntilTurnEndNumber {
                        player: target,
                        turn_number: state.turn_number + 1,
                    },
                },
            }])
        }
        SpiritSkill::RockWall => {
            let old_value = state.shield(player).unwrap_or(0);
            Ok(vec![GameEvent::ShieldChanged {
                player: player.clone(),
                old_value,
                delta: 40 - old_value,
                new_value: 40,
            }])
        }
        SpiritSkill::EvilGaze => {
            let target = next_player(state, player)?;
            let inspected = crate::rules::dark::validate_trusted_random_hand_cards(
                state,
                &target,
                trusted_random_cards,
                "evil-gaze",
            )?;
            Ok(vec![GameEvent::HandInspected {
                viewer: player.clone(),
                target,
                cards: inspected,
            }])
        }
        SpiritSkill::DeathOmen => {
            let target = next_player(state, player)?;
            let cards = state
                .deck_for(&target)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
                })?
                .iter()
                .take(4)
                .copied()
                .collect::<Vec<_>>();
            let highest = cards
                .iter()
                .filter_map(|card| state.card_def(*card).map(|definition| definition.level))
                .max()
                .unwrap_or(0);
            let mut events = vec![GameEvent::CardsMoved {
                card_moves: cards
                    .iter()
                    .map(|card| CardMoveDelta {
                        card: *card,
                        from: if state.uses_personal_decks() {
                            CardZone::PlayerDeckTop(target.clone())
                        } else {
                            CardZone::DeckTop
                        },
                        to: discard_zone_for_card(state, *card),
                    })
                    .collect(),
            }];
            events.push(hp_event(
                state,
                &team_for_player(state, &target)?,
                -(highest as i32 * 4),
            )?);
            if cards.iter().any(|card| {
                state
                    .card_def(*card)
                    .is_some_and(|definition| definition.level == 4)
            }) {
                let power_after_cost = state
                    .spirit_for(player)
                    .expect("validated Death Spirit")
                    .power
                    .saturating_sub(4);
                events.push(GameEvent::SpiritPowerChanged {
                    player: player.clone(),
                    spirit: SpiritKind::Death,
                    old_power: power_after_cost,
                    delta: 1,
                    new_power: (power_after_cost + 1).min(6),
                    reason: crate::domain::SpiritPowerChangeReason::SkillEffect,
                });
            }
            Ok(events)
        }
    }
}

fn hp_event(state: &GameState, team: &crate::domain::TeamId, delta: i32) -> GameResult<GameEvent> {
    let old_hp = state
        .hp
        .iter()
        .find(|owned| &owned.team == team)
        .map(|owned| owned.hp)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;
    let initial_hp = state
        .initial_hp(team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;
    let new_hp = (old_hp + delta).clamp(0, initial_hp);
    Ok(GameEvent::HpChanged {
        change: HpChangeDelta {
            team: team.clone(),
            old_hp,
            delta,
            new_hp,
            effective_delta: new_hp - old_hp,
        },
    })
}

fn team_for_player(state: &GameState, player: &PlayerId) -> GameResult<crate::domain::TeamId> {
    state
        .players
        .iter()
        .find(|candidate| &candidate.id == player)
        .map(|candidate| candidate.team.clone())
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))
}

fn previous_player(state: &GameState, player: &PlayerId) -> GameResult<PlayerId> {
    adjacent_player(state, player, false)
}

fn next_player(state: &GameState, player: &PlayerId) -> GameResult<PlayerId> {
    adjacent_player(state, player, true)
}

fn adjacent_player(state: &GameState, player: &PlayerId, next: bool) -> GameResult<PlayerId> {
    let index = state
        .turn_order
        .iter()
        .position(|candidate| candidate == player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    let adjacent = if next {
        (index + 1) % state.turn_order.len()
    } else if index == 0 {
        state.turn_order.len() - 1
    } else {
        index - 1
    };
    state
        .turn_order
        .get(adjacent)
        .cloned()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))
}

fn discard_zone_for_card(state: &GameState, card: CardInstanceId) -> CardZone {
    if state.uses_personal_decks() {
        match state.card_origin(card) {
            Some(CardOrigin::Player(owner)) => CardZone::PlayerDiscard(owner.clone()),
            _ => CardZone::Discard,
        }
    } else {
        CardZone::Discard
    }
}

fn summoning_id(spirit: SpiritKind) -> &'static str {
    match spirit {
        SpiritKind::Metal => "metal-spirit-summoning",
        SpiritKind::Wood => "wood-spirit-summoning",
        SpiritKind::Water => "water-spirit-summoning",
        SpiritKind::Fire => "fire-spirit-summoning",
        SpiritKind::Earth => "earth-spirit-summoning",
        SpiritKind::Evil | SpiritKind::Death => {
            unreachable!("Dark Spirits use Profession Formations")
        }
    }
}

fn summoning_name(spirit: SpiritKind) -> &'static str {
    match spirit {
        SpiritKind::Metal => "金靈喚術",
        SpiritKind::Wood => "木靈喚術",
        SpiritKind::Water => "水靈喚術",
        SpiritKind::Fire => "火靈喚術",
        SpiritKind::Earth => "土靈喚術",
        SpiritKind::Evil | SpiritKind::Death => {
            unreachable!("Dark Spirits use Profession Formations")
        }
    }
}

fn element_label(spirit: SpiritKind) -> &'static str {
    match spirit {
        SpiritKind::Metal => "金",
        SpiritKind::Wood => "木",
        SpiritKind::Water => "水",
        SpiritKind::Fire => "火",
        SpiritKind::Earth => "土",
        SpiritKind::Evil | SpiritKind::Death => {
            unreachable!("Dark Spirits have no elemental summoning label")
        }
    }
}

fn summoning_spec(spirit: SpiritKind) -> BaseFormationSpec {
    let id = summoning_id(spirit);
    let label = element_label(spirit);
    BaseFormationSpec {
        formation: FormationDef {
            id: id.to_string(),
            name: summoning_name(spirit).to_string(),
            rule_text: format!("{label}{label}／主動術式，召喚{label}精靈"),
            category: FormationCategory::Spell,
            pattern: FormationPattern::ExactElements(vec![element(spirit); 2]),
            effect_id: id.to_string(),
            point_formula: PointFormula::Fixed(0),
        },
        effect: EffectDef {
            id: id.to_string(),
            plan: EffectPlan::ActiveSpell(SpellPlanDef {
                resolver_id: id.to_string(),
                player_facing_effect: FormationEffect::SummonSpirit,
            }),
        },
    }
}

fn void_spirit_shattering_spec() -> BaseFormationSpec {
    let id = "void-spirit-shattering";
    BaseFormationSpec {
        formation: FormationDef {
            id: id.to_string(),
            name: "虛空碎靈術".to_string(),
            rule_text: "三張同等級／主動術式，所有精靈靈力－２；每名精靈持有者使其隊伍生命－２０"
                .to_string(),
            category: FormationCategory::Spell,
            pattern: FormationPattern::Custom("three-same-level".to_string()),
            effect_id: id.to_string(),
            point_formula: PointFormula::Fixed(0),
        },
        effect: EffectDef {
            id: id.to_string(),
            plan: EffectPlan::ActiveSpell(SpellPlanDef {
                resolver_id: id.to_string(),
                player_facing_effect: FormationEffect::ShatterSpirits,
            }),
        },
    }
}
