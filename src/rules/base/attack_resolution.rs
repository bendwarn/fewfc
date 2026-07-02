use crate::domain::{
    AttackPointBreakdown, CardInstanceId, CardMoveDelta, CardZone, DamageTransform, Element,
    ElementInteraction, EnvironmentAttackEffect, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError,
    GameEvent, GameResult, GameState, HpChangeDelta, LastElementalAttack,
    LastElementalAttackUpdate, PlayerId, STAR_MODULE_ID, ShieldChangeDelta, StarBreakReason,
    StarKind, TeamId, ValidationError,
    targeting::{RulePlayerTarget, TurnOrderTargets},
};
use crate::rules::{
    AttackCategory, AttackPlanDef, DamageTarget, EffectPlan, PointFormula,
    official_formation_registry, sacred_beast_element, star,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AttackResolutionMode {
    FormationUse,
    CopiedEffect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct AttackRequest {
    pub(super) attacker: PlayerId,
    pub(super) formation_id: String,
    pub(super) category: AttackCategory,
    pub(super) point_formula: PointFormula,
    pub(super) used_cards: Vec<CardInstanceId>,
    pub(super) damage_prevented: bool,
    pub(super) split_attack_damage: bool,
    pub(super) mode: AttackResolutionMode,
}

pub(super) fn resolve(state: &GameState, request: AttackRequest) -> GameResult<Vec<GameEvent>> {
    let plan = AttackPlanDef {
        category: request.category.clone(),
        point_formula: request.point_formula.clone(),
        damage_target: DamageTarget::PreviousPlayer,
    };
    let target = attack_target(state, &request.attacker, &plan)?;
    let target_team = player_team(state, &target)?;
    let raw_points = compute_attack_points(
        state,
        &request.attacker,
        &request.point_formula,
        &request.used_cards,
        &target,
    )?;
    let points = if request.mode == AttackResolutionMode::FormationUse {
        crate::rules::hero::modify_attack_points(
            state,
            &request.attacker,
            &request.formation_id,
            &request.used_cards,
            raw_points,
        )
    } else {
        raw_points
    };
    let has_target_shield = state.shield(&target).is_some_and(|value| value > 0);
    let mut point_breakdown =
        attack_point_breakdown(state, &request.category, &target, points, has_target_shield);
    if !has_target_shield {
        match crate::rules::hero::incoming_damage_modifier(
            state,
            &target,
            &request.category,
            point_breakdown.damage_transform != DamageTransform::HealTarget,
        ) {
            crate::rules::hero::IncomingDamageModifier::None => {}
            crate::rules::hero::IncomingDamageModifier::HalfRoundUp => {
                point_breakdown.final_amount = (point_breakdown.final_amount + 1) / 2;
            }
            crate::rules::hero::IncomingDamageModifier::Prevent => {
                point_breakdown.final_amount = 0;
            }
        }
    }
    let final_amount = point_breakdown.final_amount;
    let damage_transform = point_breakdown.damage_transform;
    let split_attack_damage = request.split_attack_damage;
    let defender_amount = if split_attack_damage {
        (final_amount + 1) / 2
    } else {
        final_amount
    };
    let shield_change = if request.damage_prevented {
        None
    } else {
        let shield_damage = match request.category {
            AttackCategory::Physical => defender_amount * 2,
            AttackCategory::Elemental(_) | AttackCategory::Special => defender_amount,
        };
        shield_absorption(state, &target, shield_damage)
    };
    let has_shield_change = shield_change.is_some();
    let hp_change = if request.damage_prevented || has_shield_change {
        no_hp_change(state, &target_team)?
    } else {
        apply_attack_amount(
            state,
            &target_team,
            defender_amount,
            point_breakdown.damage_transform,
        )?
    };
    let card_moves = match request.mode {
        AttackResolutionMode::FormationUse => request
            .used_cards
            .iter()
            .copied()
            .map(|card| CardMoveDelta {
                card,
                from: CardZone::Hand(request.attacker.clone()),
                to: if state.uses_personal_decks() {
                    match state.card_origin(card) {
                        Some(crate::domain::CardOrigin::Player(owner)) => {
                            CardZone::PlayerDiscard(owner.clone())
                        }
                        _ => CardZone::Discard,
                    }
                } else {
                    CardZone::Discard
                },
            })
            .collect(),
        AttackResolutionMode::CopiedEffect => Vec::new(),
    };
    let elemental_context_update = elemental_context_update(&request.category, state.turn_number)
        .map(|attack| LastElementalAttackUpdate {
            player: request.attacker.clone(),
            attack,
        });

    let mut events = vec![GameEvent::AttackResolved {
        attacker: request.attacker.clone(),
        target,
        formation_id: request.formation_id.clone(),
        used_cards: request.used_cards.clone(),
        point_breakdown,
        hp_change,
        shield_change,
        card_moves,
        elemental_context_update,
    }];

    if request.mode == AttackResolutionMode::FormationUse
        && split_attack_damage
        && !request.damage_prevented
    {
        let attacker_team = player_team(state, &request.attacker)?;
        let attacker_amount = (final_amount + 1) / 2;
        if attacker_amount > 0 {
            events.push(GameEvent::HpChanged {
                change: apply_attack_amount(
                    state,
                    &attacker_team,
                    attacker_amount,
                    damage_transform,
                )?,
            });
        }
    }

    if request.formation_id == "five-streams-unite" {
        let old_value = state
            .turn_draw_bonus_by_player
            .get(&request.attacker)
            .copied()
            .unwrap_or(0);
        events.push(GameEvent::TurnDrawBonusChanged {
            player: request.attacker.clone(),
            old_value,
            delta: 1,
            new_value: old_value + 1,
        });
    }

    if let Some(environment) = sacred_beast_element(&request.formation_id) {
        events.push(GameEvent::EnvironmentTransferred {
            player: request.attacker.clone(),
            formation_id: request.formation_id.clone(),
            from: state.environment,
            to: environment,
        });
    }

    if request.mode == AttackResolutionMode::FormationUse && state.has_rule_module(STAR_MODULE_ID) {
        if let Some(star) = star::summoning_formation_star(&request.formation_id)
            && points >= 30
            && crate::rules::hero::star_summoning_allowed(
                state,
                &request.attacker,
                &request.formation_id,
                &request.used_cards,
            )
        {
            events.extend(star_summoning_events(state, &request.attacker, star)?);
        }

        if let Some(star) = star::three_card_formation_star(&request.formation_id) {
            let old_value = state
                .turn_draw_bonus_by_player
                .get(&request.attacker)
                .copied()
                .unwrap_or(0);
            events.push(GameEvent::TurnDrawBonusChanged {
                player: request.attacker.clone(),
                old_value,
                delta: 1,
                new_value: old_value + 1,
            });
            let team = player_team(state, &request.attacker)?;
            events.push(GameEvent::StarBroken {
                team,
                star,
                reason: StarBreakReason::StarFormationUsed {
                    formation_id: request.formation_id.clone(),
                },
                hp_change: None,
            });
        }
    }

    if request.mode == AttackResolutionMode::FormationUse {
        for intent in crate::rules::hero::post_formation_intents(
            state,
            &request.attacker,
            &request.formation_id,
        )? {
            match intent {
                crate::rules::hero::PostFormationIntent::AddTurnDraw { player, amount } => {
                    let old_value = state
                        .turn_draw_bonus_by_player
                        .get(&player)
                        .copied()
                        .unwrap_or(0);
                    events.push(GameEvent::TurnDrawBonusChanged {
                        player,
                        old_value,
                        delta: amount as i32,
                        new_value: old_value + amount,
                    });
                }
                crate::rules::hero::PostFormationIntent::AddStatus { status } => {
                    events.push(GameEvent::StatusAdded { status });
                }
                crate::rules::hero::PostFormationIntent::EstablishCounterEffect {
                    owner,
                    effect_id,
                } => {
                    events.push(GameEvent::CounterEffectEstablished { owner, effect_id });
                }
            }
        }
    }

    Ok(events)
}

pub(super) fn preview_attack_points(
    state: &GameState,
    attacker: &PlayerId,
    formation_id: &str,
    plan: &AttackPlanDef,
    cards: &[CardInstanceId],
) -> GameResult<i32> {
    let target = attack_target(state, attacker, plan)?;
    let points = compute_attack_points(state, attacker, &plan.point_formula, cards, &target)?;
    Ok(crate::rules::hero::modify_attack_points(
        state,
        attacker,
        formation_id,
        cards,
        points,
    ))
}

fn star_summoning_events(
    state: &GameState,
    player: &PlayerId,
    summoned_star: StarKind,
) -> GameResult<Vec<GameEvent>> {
    if state
        .team_stars
        .iter()
        .any(|owned| owned.star == summoned_star)
    {
        return Ok(Vec::new());
    }

    let team = player_team(state, player)?;
    let mut events = Vec::new();
    let mut broken = Vec::new();

    if let Some(current_star) = state.star_for_team(&team) {
        broken.push((team.clone(), current_star));
        events.push(GameEvent::StarBroken {
            team: team.clone(),
            star: current_star,
            reason: StarBreakReason::Replaced,
            hp_change: None,
        });
    }

    let opposed = star::opposing_star(summoned_star);
    for owned in state
        .team_stars
        .iter()
        .filter(|owned| owned.star == opposed)
    {
        if broken
            .iter()
            .any(|(team, star)| team == &owned.team && star == &owned.star)
        {
            continue;
        }
        events.push(GameEvent::StarBroken {
            team: owned.team.clone(),
            star: owned.star,
            reason: StarBreakReason::OpposedBy(summoned_star),
            hp_change: None,
        });
    }

    events.push(GameEvent::StarSummoned {
        player: player.clone(),
        team: team.clone(),
        star: summoned_star,
    });

    let history = state.summoned_stars_for(player).unwrap_or_default();
    if history.len() == 4 && !history.contains(&summoned_star) {
        events.push(GameEvent::FiveStarAlignmentAchieved {
            player: player.clone(),
            team,
        });
    }

    Ok(events)
}

fn attack_target(
    state: &GameState,
    attacker: &PlayerId,
    plan: &AttackPlanDef,
) -> GameResult<PlayerId> {
    match plan.damage_target {
        DamageTarget::PreviousPlayer => {
            TurnOrderTargets::new(state).player_target(attacker, RulePlayerTarget::PreviousPlayer)
        }
    }
}

fn player_team(state: &GameState, player: &PlayerId) -> GameResult<TeamId> {
    TurnOrderTargets::new(state).team_of(player)
}

fn compute_attack_points(
    state: &GameState,
    attacker: &PlayerId,
    formula: &PointFormula,
    cards: &[CardInstanceId],
    target: &PlayerId,
) -> GameResult<i32> {
    let level_sum = || -> GameResult<i32> {
        cards.iter().try_fold(0, |sum, card| {
            let level = state
                .card_level_for(attacker, *card)
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(*card),
                ))? as i32;
            Ok(sum + level)
        })
    };

    match formula {
        PointFormula::Fixed(points) => Ok(*points as i32),
        PointFormula::LevelPlus(bonus) => Ok(level_sum()? + *bonus as i32),
        PointFormula::LevelSumTimes(multiplier) => Ok(level_sum()? * *multiplier as i32),
        PointFormula::TargetHandCountTimes(multiplier) => {
            let target_hand = state.hand(target).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
            })?;
            Ok(target_hand.len() as i32 * *multiplier as i32)
        }
        PointFormula::ElementProductTimes {
            element,
            multiplier,
        } => {
            let levels = cards
                .iter()
                .filter(|card| {
                    state
                        .card_def(**card)
                        .is_some_and(|definition| definition.element == *element)
                })
                .filter_map(|card| state.card_level_for(attacker, *card))
                .map(|level| level as i32)
                .collect::<Vec<_>>();
            Ok(levels.into_iter().product::<i32>() * *multiplier as i32)
        }
    }
}

fn attack_point_breakdown(
    state: &GameState,
    category: &AttackCategory,
    target: &PlayerId,
    base_points: i32,
    skip_interaction: bool,
) -> AttackPointBreakdown {
    let Some(current_element) = elemental_attack_element(category) else {
        return AttackPointBreakdown {
            base_points,
            environment_effect: EnvironmentAttackEffect::None,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        };
    };

    let mut amount = base_points;
    let mut environment_effect = EnvironmentAttackEffect::None;
    let mut environment_converts_to_healing = false;
    if state.has_rule_module(FIVE_DIRECTIONS_LEGEND_MODULE_ID)
        && let Some(environment) = state.environment
    {
        if current_element == environment {
            amount *= 2;
            environment_effect =
                EnvironmentAttackEffect::MatchingElementDamageDoubled { environment };
        } else if !skip_interaction && generates(current_element, environment) {
            environment_converts_to_healing = true;
            environment_effect =
                EnvironmentAttackEffect::GeneratingElementDamageConvertedToHealing { environment };
        }
    }

    if skip_interaction {
        return AttackPointBreakdown {
            base_points,
            environment_effect,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: amount,
        };
    }

    let interaction = match previous_formation_element(state, target) {
        Some(previous_element) if current_element == previous_element => ElementInteraction::Same,
        Some(previous_element) if generates(current_element, previous_element) => {
            ElementInteraction::Generating
        }
        Some(previous_element) if overcomes(current_element, previous_element) => {
            ElementInteraction::Overcoming
        }
        Some(_) | None => ElementInteraction::None,
    };

    let (damage_transform, final_amount) = match interaction {
        ElementInteraction::Same => (
            if environment_converts_to_healing {
                DamageTransform::HealTarget
            } else {
                DamageTransform::HalfDamageRoundUp
            },
            (amount + 1) / 2,
        ),
        ElementInteraction::Generating => (DamageTransform::HealTarget, amount),
        ElementInteraction::Overcoming => (
            if environment_converts_to_healing {
                DamageTransform::HealTarget
            } else {
                DamageTransform::DoubleDamage
            },
            amount * 2,
        ),
        ElementInteraction::None if environment_converts_to_healing => {
            (DamageTransform::HealTarget, amount)
        }
        ElementInteraction::None => (DamageTransform::NormalDamage, amount),
    };

    AttackPointBreakdown {
        base_points,
        environment_effect,
        interaction,
        damage_transform,
        final_amount,
    }
}

fn previous_formation_element(state: &GameState, player: &PlayerId) -> Option<Element> {
    let formation_id = state
        .last_formation_by_player
        .get(player)?
        .effective_effect_id();
    let registry = official_formation_registry(&state.enabled_rule_modules);
    let formation = registry.formation(formation_id)?;
    let effect = registry.effect_for(formation)?;

    match &effect.plan {
        EffectPlan::Attack(plan) => elemental_attack_element(&plan.category),
        EffectPlan::ActiveSpell(_) | EffectPlan::PassiveSpell(_) => None,
    }
}

fn elemental_attack_element(category: &AttackCategory) -> Option<Element> {
    match category {
        AttackCategory::Elemental(element) => Some(*element),
        AttackCategory::Physical | AttackCategory::Special => None,
    }
}

fn generates(current: Element, previous: Element) -> bool {
    matches!(
        (current, previous),
        (Element::Metal, Element::Water)
            | (Element::Water, Element::Wood)
            | (Element::Wood, Element::Fire)
            | (Element::Fire, Element::Earth)
            | (Element::Earth, Element::Metal)
    )
}

fn overcomes(current: Element, previous: Element) -> bool {
    matches!(
        (current, previous),
        (Element::Metal, Element::Wood)
            | (Element::Wood, Element::Earth)
            | (Element::Earth, Element::Water)
            | (Element::Water, Element::Fire)
            | (Element::Fire, Element::Metal)
    )
}

fn apply_attack_amount(
    state: &GameState,
    team: &TeamId,
    amount: i32,
    transform: DamageTransform,
) -> GameResult<HpChangeDelta> {
    let old_hp = state
        .hp
        .iter()
        .find(|team_hp| &team_hp.team == team)
        .map(|team_hp| team_hp.hp)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;
    let delta = match transform {
        DamageTransform::HealTarget => amount,
        DamageTransform::NormalDamage
        | DamageTransform::DoubleDamage
        | DamageTransform::HalfDamageRoundUp => -amount,
    };
    let initial_hp = state
        .initial_hp(team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;
    let new_hp = (old_hp + delta).clamp(0, initial_hp);

    Ok(HpChangeDelta {
        team: team.clone(),
        old_hp,
        delta,
        new_hp,
        effective_delta: new_hp - old_hp,
    })
}

fn no_hp_change(state: &GameState, team: &TeamId) -> GameResult<HpChangeDelta> {
    let old_hp = state
        .hp
        .iter()
        .find(|team_hp| &team_hp.team == team)
        .map(|team_hp| team_hp.hp)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;

    Ok(HpChangeDelta {
        team: team.clone(),
        old_hp,
        delta: 0,
        new_hp: old_hp,
        effective_delta: 0,
    })
}

fn shield_absorption(
    state: &GameState,
    player: &PlayerId,
    incoming_damage: i32,
) -> Option<ShieldChangeDelta> {
    let old_value = state.shield(player)?;
    if old_value <= 0 {
        return None;
    }

    Some(ShieldChangeDelta {
        player: player.clone(),
        old_value,
        delta: -incoming_damage,
        new_value: (old_value - incoming_damage).max(0),
    })
}

fn elemental_context_update(
    category: &AttackCategory,
    resolved_turn: u64,
) -> Option<LastElementalAttack> {
    match category {
        AttackCategory::Elemental(element) => Some(LastElementalAttack {
            element: *element,
            resolved_turn,
        }),
        AttackCategory::Physical | AttackCategory::Special => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CardDef, CardDefId, CardInstanceDef, GameSetup};

    #[test]
    fn level_sum_formula_sums_submitted_card_levels() {
        let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).with_cards(
            vec![
                CardDef {
                    id: CardDefId::new("metal"),
                    name: "metal".to_string(),
                    element: crate::rules::Element::Metal,
                    level: 3,
                },
                CardDef {
                    id: CardDefId::new("wood"),
                    name: "wood".to_string(),
                    element: crate::rules::Element::Wood,
                    level: 2,
                },
            ],
            vec![
                CardInstanceDef {
                    instance: CardInstanceId::new(1),
                    definition: CardDefId::new("metal"),
                    origin: Default::default(),
                },
                CardInstanceDef {
                    instance: CardInstanceId::new(2),
                    definition: CardDefId::new("wood"),
                    origin: Default::default(),
                },
            ],
        );
        let state = GameState::from_setup(&setup);

        assert_eq!(
            compute_attack_points(
                &state,
                &PlayerId::new("p1"),
                &PointFormula::LevelSumTimes(1),
                &[CardInstanceId::new(1), CardInstanceId::new(2)],
                &PlayerId::new("p2"),
            ),
            Ok(5)
        );
    }
}
