use crate::domain::{
    AttackPointBreakdown, CardInstanceId, CardMoveDelta, CardZone, DamageTransform, Element,
    ElementInteraction, GameError, GameEvent, GameResult, GameState, HpChangeDelta,
    LastElementalAttack, LastElementalAttackUpdate, PlayerId, ShieldChangeDelta, TeamId,
    ValidationError,
    targeting::{RulePlayerTarget, TurnOrderTargets},
};
use crate::rules::{
    AttackCategory, AttackPlanDef, DamageTarget, EffectPlan, PointFormula, base_formation_registry,
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
    let points =
        compute_attack_points(state, &request.point_formula, &request.used_cards, &target)?;
    let has_target_shield = state.shield(&target).is_some_and(|value| value > 0);
    let point_breakdown =
        attack_point_breakdown(state, &request.category, &target, points, has_target_shield);
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
                to: CardZone::Discard,
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
        used_cards: request.used_cards,
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
            player: request.attacker,
            old_value,
            delta: 1,
            new_value: old_value + 1,
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
        DamageTarget::DeclaredPlayer | DamageTarget::TeamOfDeclaredPlayer => {
            Err(GameError::RuleImplementation(
                crate::domain::RuleImplementationError::EffectNotImplemented(
                    "declared-attack-target".to_string(),
                ),
            ))
        }
    }
}

fn player_team(state: &GameState, player: &PlayerId) -> GameResult<TeamId> {
    TurnOrderTargets::new(state).team_of(player)
}

fn compute_attack_points(
    state: &GameState,
    formula: &PointFormula,
    cards: &[CardInstanceId],
    target: &PlayerId,
) -> GameResult<i32> {
    let level_sum = || -> GameResult<i32> {
        cards.iter().try_fold(0, |sum, card| {
            let level = state
                .card_def(*card)
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(*card),
                ))?
                .level as i32;
            Ok(sum + level)
        })
    };

    match formula {
        PointFormula::Fixed(points) => Ok(*points as i32),
        PointFormula::CardCount => Ok(cards.len() as i32),
        PointFormula::FormationPoints => level_sum(),
        PointFormula::LevelPlus(bonus) => Ok(level_sum()? + *bonus as i32),
        PointFormula::LevelSumTimes(multiplier) => Ok(level_sum()? * *multiplier as i32),
        PointFormula::TargetHandCountTimes(multiplier) => {
            let target_hand = state.hand(target).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
            })?;
            Ok(target_hand.len() as i32 * *multiplier as i32)
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
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        };
    };
    if skip_interaction {
        return AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        };
    }
    let Some(previous_element) = previous_formation_element(state, target) else {
        return AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        };
    };

    if current_element == previous_element {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::Same,
            damage_transform: DamageTransform::HalfDamageRoundUp,
            final_amount: (base_points + 1) / 2,
        }
    } else if generates(current_element, previous_element) {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::Generating,
            damage_transform: DamageTransform::HealTarget,
            final_amount: base_points,
        }
    } else if overcomes(current_element, previous_element) {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::Overcoming,
            damage_transform: DamageTransform::DoubleDamage,
            final_amount: base_points * 2,
        }
    } else {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        }
    }
}

fn previous_formation_element(state: &GameState, player: &PlayerId) -> Option<Element> {
    let formation_id = state
        .last_formation_by_player
        .get(player)?
        .effective_effect_id();
    let registry = base_formation_registry();
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
    fn formation_points_sum_submitted_card_levels() {
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
                },
                CardInstanceDef {
                    instance: CardInstanceId::new(2),
                    definition: CardDefId::new("wood"),
                },
            ],
        );
        let state = GameState::from_setup(&setup);

        assert_eq!(
            compute_attack_points(
                &state,
                &PointFormula::FormationPoints,
                &[CardInstanceId::new(1), CardInstanceId::new(2)],
                &PlayerId::new("p2"),
            ),
            Ok(5)
        );
    }
}
