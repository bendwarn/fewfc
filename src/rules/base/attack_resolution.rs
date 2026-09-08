use crate::domain::{
    AttackCounterEffect, AttackOutcome, AttackPointBreakdown, AttackResolutionEffects,
    AttackStatusRemoval, CardInstanceId, DamageTransform, Element, ElementInteraction,
    EnvironmentAttackEffect, EnvironmentTransferDelta, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError,
    GameEvent, GameResult, GameState, HpChangeDelta, HpChangeRole, LastElementalAttack,
    LastElementalAttackUpdate, PlayerId, ResolvedHpChange, STAR_MODULE_ID, ShieldChangeDelta,
    StarBreakReason, StarKind, TeamId, TurnDrawBonusDelta, ValidationError,
    hp::{HpChangePlan, HpChangeRequest},
    targeting::{RulePlayerTarget, TurnOrderTargets},
};
use crate::rules::{
    AttackCategory, AttackPlanDef, DamageTarget, EffectPlan, PointFormula,
    formation_resolved_on_previous_turn, official_formation_registry, sacred_beast_element, star,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum AttackResolutionMode {
    FormationUse,
    CopiedEffect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AttackRequest {
    pub(crate) attacker: PlayerId,
    pub(crate) formation_id: String,
    pub(crate) category: AttackCategory,
    pub(crate) point_formula: PointFormula,
    pub(crate) used_cards: Vec<CardInstanceId>,
    pub(crate) damage_prevented: bool,
    pub(crate) split_attack_damage: bool,
    pub(crate) mode: AttackResolutionMode,
    pub(crate) pre_resolution_effects: AttackPreResolutionEffects,
    /// 呼叫端擁有攻擊後附帶 HP 差異的語意角色；解析器不依 formation id 猜測。
    pub(crate) trailing_hp_role: HpChangeRole,
}

/// 攻擊前已決定、但必須和攻擊共用 HP ledger 的差異。
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub(crate) struct AttackPreResolutionEffects {
    pub(crate) hp_changes: Vec<HpChangeDelta>,
    pub(crate) effects: AttackResolutionEffects,
}

/// 將沒有其他時序的後果轉換為外層攻擊的負載。呼叫端用它支援舊版模組解析器，
/// 同時標準記錄仍維持單一 `AttackResolved` 事件。
pub(crate) fn effects_from_events(events: &[GameEvent]) -> GameResult<AttackPreResolutionEffects> {
    let mut pre_resolution = AttackPreResolutionEffects::default();
    for event in events.iter().cloned() {
        if let GameEvent::HpChanged { change } = event {
            pre_resolution.hp_changes.push(change);
            continue;
        }
        append_simultaneous_effect(&mut pre_resolution.effects, event).map_err(|event| {
            GameError::RuleImplementation(
                crate::domain::RuleImplementationError::EffectNotImplemented(format!(
                    "attack-side-effect:{}",
                    event_name(&event)
                )),
            )
        })?;
    }
    Ok(pre_resolution)
}

/// 將模組產生、與攻擊同時序的效果收納進攻擊的原子負載。宣告其他規則時序的
/// 事件仍會是分開的標準事件。
pub(crate) fn absorb_simultaneous_events(events: &mut Vec<GameEvent>, hp_role: HpChangeRole) {
    let Some(attack_index) = events
        .iter()
        .position(|event| matches!(event, GameEvent::AttackResolved { .. }))
    else {
        return;
    };
    let tail = events.split_off(attack_index + 1);
    let mut effects = AttackResolutionEffects::default();
    let mut hp_effects = Vec::new();
    let mut retained = Vec::new();
    for event in tail {
        if let GameEvent::HpChanged { change } = event {
            hp_effects.push(change);
            continue;
        }
        match append_simultaneous_effect(&mut effects, event) {
            Ok(()) => {}
            Err(event) => retained.push(event),
        }
    }
    if (!effects.is_empty() || !hp_effects.is_empty())
        && let GameEvent::AttackResolved {
            elemental_context_update,
            hp_changes,
            ..
        } = &mut events[attack_index]
    {
        hp_changes.extend(hp_effects.into_iter().map(|change| ResolvedHpChange {
            role: hp_role.clone(),
            change,
        }));
        let target = elemental_context_update.get_or_insert_with(AttackResolutionEffects::default);
        target.totem_changes.extend(effects.totem_changes);
        target.shield_changes.extend(effects.shield_changes);
        target.card_moves.extend(effects.card_moves);
        target.statuses_added.extend(effects.statuses_added);
        target.statuses_removed.extend(effects.statuses_removed);
        target
            .counter_effects_established
            .extend(effects.counter_effects_established);
        target
            .turn_draw_bonus_changes
            .extend(effects.turn_draw_bonus_changes);
        target
            .environment_transfers
            .extend(effects.environment_transfers);
    }
    events.extend(retained);
}

// 攻擊解析使用 GameEvent 作為區域控制流程訊號，讓呼叫端可以保留無法表示在
// 原子攻擊負載中的事件。裝箱不會改善標準線上格式，反而會模糊這個特意採用
// 事件形狀的邊界。
#[allow(clippy::result_large_err)]
fn append_simultaneous_effect(
    effects: &mut AttackResolutionEffects,
    event: GameEvent,
) -> Result<(), GameEvent> {
    match event {
        GameEvent::TotemChanged {
            player,
            previous,
            totem,
            reason,
        } => effects.totem_changes.push(crate::domain::TotemChange {
            player,
            previous,
            totem,
            reason,
        }),
        GameEvent::HpChanged { .. } => return Err(event),
        GameEvent::ShieldChanged {
            player,
            old_value,
            delta,
            new_value,
        } => effects.shield_changes.push(ShieldChangeDelta {
            player,
            old_value,
            delta,
            new_value,
        }),
        GameEvent::CardsMoved { card_moves } => effects.card_moves.extend(card_moves),
        GameEvent::StatusAdded { status } => effects.statuses_added.push(status),
        GameEvent::StatusRemoved { status_id, owner } => effects
            .statuses_removed
            .push(AttackStatusRemoval { status_id, owner }),
        GameEvent::CounterEffectEstablished { owner, effect_id } => effects
            .counter_effects_established
            .push(AttackCounterEffect { owner, effect_id }),
        GameEvent::TurnDrawBonusChanged {
            player,
            old_value,
            delta,
            new_value,
        } => effects.turn_draw_bonus_changes.push(TurnDrawBonusDelta {
            player,
            old_value,
            delta,
            new_value,
        }),
        GameEvent::EnvironmentTransferred {
            player,
            formation_id,
            from,
            to,
        } => effects
            .environment_transfers
            .push(EnvironmentTransferDelta {
                player,
                formation_id,
                from,
                to,
            }),
        event => return Err(event),
    }
    Ok(())
}

fn event_name(event: &GameEvent) -> &'static str {
    match event {
        GameEvent::HpChanged { .. } => "hp-changed",
        GameEvent::ShieldChanged { .. } => "shield-changed",
        GameEvent::CardsMoved { .. } => "cards-moved",
        GameEvent::StatusAdded { .. } => "status-added",
        GameEvent::StatusRemoved { .. } => "status-removed",
        GameEvent::CounterEffectEstablished { .. } => "counter-effect-established",
        GameEvent::TurnDrawBonusChanged { .. } => "turn-draw-bonus-changed",
        GameEvent::EnvironmentTransferred { .. } => "environment-transferred",
        _ => "unsupported",
    }
}

/// 同一外層解析若已經有 HP ledger，攻擊必須借用它，不能重新從 state 建立。
pub(crate) fn resolve_with_plan(
    state: &GameState,
    request: AttackRequest,
    hp: &mut HpChangePlan,
) -> GameResult<Vec<GameEvent>> {
    let pre_resolution_effects = request.pre_resolution_effects.effects.clone();
    // 攻擊前陣法效果已由外層同一份 ledger 的 scoped session 規劃完成。
    let pre_resolution_hp_changes = request.pre_resolution_effects.hp_changes.clone();
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
    let points = crate::rules::jianghu::modify_attack_points(
        state,
        &request.attacker,
        &request.formation_id,
        &request.point_formula,
        &request.used_cards,
        raw_points,
    );
    let points = crate::rules::dark::modify_attack_points(
        state,
        &request.attacker,
        &request.formation_id,
        &request.used_cards,
        points,
    );
    let points = if request.mode == AttackResolutionMode::FormationUse {
        crate::rules::hero::modify_attack_points(
            state,
            &request.attacker,
            &request.formation_id,
            &request.used_cards,
            points,
        )
    } else {
        points
    };
    let extreme_yang = crate::rules::jianghu::extreme_yang_applies(
        state,
        &request.attacker,
        &request.formation_id,
    );
    let has_target_shield = !extreme_yang && state.shield(&target).is_some_and(|value| value > 0);
    let attack_element = elemental_attack_element(&request.category);
    let ignores_totem = sacred_beast_element(&request.formation_id).is_some();
    let target_exempt = !ignores_totem
        && !has_target_shield
        && crate::rules::totem::owned(state, &target)
            .is_some_and(|totem| attack_element == Some(crate::rules::totem::element(totem)));
    let attacker_exempt = !ignores_totem
        && crate::rules::totem::owned(state, &request.attacker)
            .is_some_and(|totem| attack_element == Some(crate::rules::totem::element(totem)));
    let environment_doubled = state.has_rule_module(FIVE_DIRECTIONS_LEGEND_MODULE_ID)
        && attack_element.is_some()
        && attack_element == state.environment;
    let consumed_totem = crate::rules::totem::owned(state, &request.attacker).filter(|totem| {
        !ignores_totem
            && !has_target_shield
            && !request.damage_prevented
            && attack_element == Some(crate::rules::totem::element(*totem))
            && state.environment.is_some_and(|environment| {
                attack_element.is_some_and(|element| generates(element, environment))
            })
    });
    let totem_split = request.split_attack_damage && (target_exempt || attacker_exempt);
    // 圖騰例外先於其他修正及其取整；圖騰反震先分配，再依各受擊玩家套用環境倍數。
    let mut point_breakdown = attack_point_breakdown(
        state,
        &request.category,
        &target,
        points,
        has_target_shield,
        consumed_totem.is_some(),
        target_exempt || totem_split,
    );
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
        if crate::rules::jianghu::halves_incoming_damage(state, &target, &request.category)
            && point_breakdown.damage_transform != DamageTransform::HealTarget
        {
            point_breakdown.final_amount = (point_breakdown.final_amount + 1) / 2;
        }
    }
    let final_amount = point_breakdown.final_amount;
    let damage_transform = point_breakdown.damage_transform;
    let split_attack_damage = request.split_attack_damage;
    let defender_amount = if totem_split && environment_doubled {
        ((final_amount + 1) / 2) * if target_exempt { 1 } else { 2 }
    } else if split_attack_damage {
        (final_amount + 1) / 2
    } else {
        final_amount
    };
    let defender_amount = if !has_target_shield
        && crate::rules::tribulation::reduces_attack_damage(state, &target, &request.formation_id)
    {
        (defender_amount - 20).max(0)
    } else {
        defender_amount
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
    // 防護罩與被動在進入 HP seam 前即已阻止攻擊，因此不能捏造 HP no-op fact。
    let hp_change = if request.damage_prevented || has_shield_change {
        None
    } else {
        Some(apply_attack_amount(
            state,
            hp,
            &target_team,
            defender_amount,
            point_breakdown.damage_transform,
        )?)
    };
    // 陣形卡牌的區域移動由 FormationCommitted 與 FormationCardsDiscarded 擁有。
    // AttackResolved 只包含額外的卡牌差異，絕不重複手牌到棄牌堆的基準移動。
    let card_moves = Vec::new();
    let mut resolution_effects = pre_resolution_effects;
    if let Some(totem) = consumed_totem.filter(|_| final_amount > 0) {
        resolution_effects
            .totem_changes
            .push(crate::domain::TotemChange {
                player: request.attacker.clone(),
                previous: Some(totem),
                totem: None,
                reason: crate::domain::TotemChangeReason::EnvironmentRecoveryPrevented,
            });
    }
    let mut countershock_changes = Vec::new();
    resolution_effects.elemental_context_update =
        elemental_context_update(&request.category, state.turn_number).map(|attack| {
            LastElementalAttackUpdate {
                player: request.attacker.clone(),
                attack,
            }
        });

    if extreme_yang {
        let old_value = state.shield(&target).unwrap_or(0);
        if old_value > 0 {
            resolution_effects.shield_changes.push(ShieldChangeDelta {
                player: target.clone(),
                old_value,
                delta: -old_value,
                new_value: 0,
            });
        }
    }
    if request.mode == AttackResolutionMode::FormationUse
        && split_attack_damage
        && !request.damage_prevented
    {
        let attacker_team = player_team(state, &request.attacker)?;
        let mut attacker_amount = if totem_split && environment_doubled {
            ((final_amount + 1) / 2) * if attacker_exempt { 1 } else { 2 }
        } else {
            (final_amount + 1) / 2
        };
        if crate::rules::tribulation::reduces_attack_damage(
            state,
            &request.attacker,
            &request.formation_id,
        ) {
            attacker_amount = (attacker_amount - 20).max(0);
        }
        if attacker_amount > 0 {
            countershock_changes.push(apply_attack_amount(
                state,
                hp,
                &attacker_team,
                attacker_amount,
                damage_transform,
            )?);
        }
    }

    if request.formation_id == "five-streams-unite" {
        let old_value = state
            .turn_draw_bonus_by_player
            .get(&request.attacker)
            .copied()
            .unwrap_or(0);
        resolution_effects
            .turn_draw_bonus_changes
            .push(TurnDrawBonusDelta {
                player: request.attacker.clone(),
                old_value,
                delta: 1,
                new_value: old_value + 1,
            });
    }

    if let Some(environment) = sacred_beast_element(&request.formation_id) {
        resolution_effects
            .environment_transfers
            .push(EnvironmentTransferDelta {
                player: request.attacker.clone(),
                formation_id: request.formation_id.clone(),
                from: state.environment,
                to: environment,
            });
    }

    resolution_effects.outcome = if request.damage_prevented {
        AttackOutcome::DamagePrevented
    } else if shield_change.is_some() {
        AttackOutcome::AbsorbedByShield
    } else {
        AttackOutcome::Resolved
    };
    let mut hp_changes = Vec::new();
    hp_changes.extend(
        pre_resolution_hp_changes
            .iter()
            .cloned()
            .map(|change| ResolvedHpChange {
                role: HpChangeRole::FormationEffect,
                change,
            }),
    );
    if let Some(change) = hp_change {
        hp_changes.push(ResolvedHpChange {
            role: HpChangeRole::AttackDamage {
                target: target.clone(),
            },
            change,
        });
    }
    hp_changes.extend(
        countershock_changes
            .iter()
            .cloned()
            .map(|change| ResolvedHpChange {
                role: HpChangeRole::AttackDamage {
                    target: request.attacker.clone(),
                },
                change,
            }),
    );
    if totem_split && environment_doubled && !target_exempt {
        point_breakdown.final_amount *= 2;
        point_breakdown.environment_effect =
            EnvironmentAttackEffect::MatchingElementDamageDoubled {
                environment: state.environment.expect("matching environment"),
            };
    }
    let mut events = vec![GameEvent::AttackResolved {
        attacker: request.attacker.clone(),
        target: target.clone(),
        formation_id: request.formation_id.clone(),
        used_cards: request.used_cards.clone(),
        point_breakdown,
        hp_changes,
        shield_change,
        card_moves,
        elemental_context_update: Some(resolution_effects),
    }];

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
            if state.star_for_team(&team) == Some(star) {
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
    }

    let game_continues = {
        let mut projected = state.clone();
        for event in &events {
            crate::rules::projection::apply_event(&mut projected, event);
        }
        crate::rules::projection::game_conclusion_if_needed(&projected).is_none()
    };

    if request.mode == AttackResolutionMode::FormationUse && game_continues {
        super::formation_use::append_post_formation_events(
            state,
            &request.attacker,
            &request.formation_id,
            &mut events,
        )?;
        let mut projected = state.clone();
        for event in &events {
            crate::rules::projection::apply_event(&mut projected, event);
        }
        events.extend(crate::rules::jianghu::post_attack_events(
            &projected,
            &request.attacker,
            &request.formation_id,
            &request.used_cards,
            hp,
        )?);
    }
    if request.mode == AttackResolutionMode::CopiedEffect
        && game_continues
        && crate::rules::jianghu::extreme_yang_applies(
            state,
            &request.attacker,
            &request.formation_id,
        )
    {
        let mut projected = state.clone();
        for event in &events {
            crate::rules::projection::apply_event(&mut projected, event);
        }
        events.extend(crate::rules::jianghu::post_attack_events(
            &projected,
            &request.attacker,
            &request.formation_id,
            &request.used_cards,
            hp,
        )?);
    }

    absorb_simultaneous_events(&mut events, request.trailing_hp_role.clone());
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
    let points = crate::rules::jianghu::modify_attack_points(
        state,
        attacker,
        formation_id,
        &plan.point_formula,
        cards,
        points,
    );
    let points =
        crate::rules::dark::modify_attack_points(state, attacker, formation_id, cards, points);
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
        cards
            .iter()
            .try_fold(0, |sum, card| {
                let level = state
                    .card_level_for(attacker, *card)
                    .ok_or(GameError::Validation(
                        ValidationError::MissingCardInstanceDefinition(*card),
                    ))?
                    .value() as i32;
                Ok(sum + level)
            })
            .map(|physical| {
                physical
                    + state
                        .formation_requirements
                        .iter()
                        .find(|requirement| {
                            &requirement.player == attacker
                                && requirement.applied_on_turn == state.turn_number
                        })
                        .and_then(|requirement| requirement.virtual_card.as_ref())
                        .map_or(0, |card| card.level.value() as i32)
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
            let mut levels = cards
                .iter()
                .filter_map(|card| state.effective_card_facts(attacker, *card))
                .filter(|facts| facts.element == *element)
                .map(|facts| facts.level.value() as i32)
                .collect::<Vec<_>>();
            if state.formation_requirements.iter().any(|requirement| {
                &requirement.player == attacker
                    && requirement.applied_on_turn == state.turn_number
                    && requirement
                        .virtual_card
                        .as_ref()
                        .is_some_and(|card| card.element == *element)
            }) {
                levels.push(
                    state
                        .formation_requirements
                        .iter()
                        .find_map(|requirement| {
                            (&requirement.player == attacker
                                && requirement.applied_on_turn == state.turn_number)
                                .then_some(requirement.virtual_card.as_ref())
                                .flatten()
                                .filter(|card| card.element == *element)
                                .map(|card| card.level.value() as i32)
                        })
                        .expect("matching virtual card must exist"),
                );
            }
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
    prevent_environment_recovery: bool,
    prevent_environment_doubling: bool,
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
        if current_element == environment && !prevent_environment_doubling {
            amount *= 2;
            environment_effect =
                EnvironmentAttackEffect::MatchingElementDamageDoubled { environment };
        } else if !skip_interaction
            && !prevent_environment_recovery
            && generates(current_element, environment)
        {
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
    let formation_id = formation_resolved_on_previous_turn(state, player)?.effective_effect_id();
    if formation_id == crate::rules::totem::SOUTH {
        return state
            .last_elemental_attack_by_player
            .get(player)
            .filter(|attack| attack.resolved_turn + 1 == state.turn_number)
            .map(|attack| attack.element);
    }
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
    hp: &mut HpChangePlan,
    team: &TeamId,
    amount: i32,
    transform: DamageTransform,
) -> GameResult<HpChangeDelta> {
    let request = match transform {
        DamageTransform::HealTarget if crate::rules::jianghu::team_has_poison(state, team) => {
            HpChangeRequest::Prevented(amount)
        }
        DamageTransform::HealTarget => HpChangeRequest::By(amount),
        DamageTransform::NormalDamage
        | DamageTransform::DoubleDamage
        | DamageTransform::HalfDamageRoundUp => HpChangeRequest::By(-amount),
    };
    hp.plan(team, request)
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
                    level: crate::domain::PrintedCardLevel::new(3),
                },
                CardDef {
                    id: CardDefId::new("wood"),
                    name: "wood".to_string(),
                    element: crate::rules::Element::Wood,
                    level: crate::domain::PrintedCardLevel::new(2),
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
