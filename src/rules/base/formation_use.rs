use crate::domain::{
    CannotPerformFormationReason, CardInstanceId, CardMoveDelta, CardZone, GameError, GameEvent,
    GameResult, GameState, PendingResolution, PlayerId, TargetDecl, TeamId, ValidationError,
    hp::{FormationHpEffectOutcome, HpChangePlan, HpChangeRequest},
    targeting::{RulePlayerTarget, RuleTeamTarget, TurnOrderTargets},
};
use crate::rules::{
    EffectPlan, PointFormula, base_formation_registry, environment_makes_formation_ineffective,
    formation_resolved_on_previous_turn, sacred_beast_element,
};

use super::attack_resolution::{self, AttackRequest, AttackResolutionMode};
use super::covered_passive::{self, IncomingActionKind, TriggerRequest};
use super::effect_intent::EffectIntent;
use super::formation_selection::FormationSelection;
use crate::rules::formation_effect_sequence::{FormationEffectSequence, ResolvedFormationEffect};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FormationUseRequest {
    pub(super) player: PlayerId,
    pub(super) formation_id: String,
    pub(super) cards: Vec<CardInstanceId>,
    pub(super) declared_targets: Vec<TargetDecl>,
    pub(super) trusted_random_cards: Option<Vec<CardInstanceId>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FormationUsePlan {
    player: PlayerId,
    formation_id: String,
    cards: Vec<CardInstanceId>,
    composition: crate::domain::FormationComposition,
    declared_targets: Vec<TargetDecl>,
    star_substitution: Option<crate::domain::StarElementSubstitution>,
    effect_plan: EffectPlan,
    trusted_random_cards: Option<Vec<CardInstanceId>>,
}

pub(super) fn resolve(
    state: &GameState,
    request: FormationUseRequest,
    hp: &mut HpChangePlan,
) -> GameResult<Vec<GameEvent>> {
    if super::player_has_status(state, &request.player, "CannotAct") {
        return Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::CannotActByStatus {
                    player: request.player,
                },
            },
        ));
    }
    let player = request.player.clone();
    let formation_id = request.formation_id.clone();
    let plan = BaseFormationPlanner::new().plan_use(state, request)?;
    let composition = plan.composition.clone();
    let star_substitution = plan.star_substitution.clone();
    let committed_state = match plan.effect_plan {
        EffectPlan::PassiveSpell(_) => crate::domain::FormationAreaState::FaceDownResolving,
        EffectPlan::Attack(_) | EffectPlan::ActiveSpell(_) => {
            crate::domain::FormationAreaState::FaceUpResolving
        }
    };
    // 一次陣法施展只建立一份 HP ledger；每個陣法效果完成後立即結算同命。
    let mut events = BaseEffectResolver::new().resolve(state, plan, hp)?;
    // 複合陣形效果過去會將自己的陣形卡牌作為手牌到棄牌堆的差異攜帶。在陣形
    // 區架構下，那些移動由流程負責，因此只保留真正額外的移動。
    for event in &mut events {
        match event {
            GameEvent::VoidReversionResolved { card_moves, .. }
            | GameEvent::VoidSpiritShatteringResolved { card_moves, .. } => {
                card_moves.retain(|movement| !composition.physical_cards.contains(&movement.card));
            }
            _ => {}
        }
    }
    // 驗證已經成功。因此陣形提交是第一個解析事實，不會被防止、封印或無效
    // 結果回滾。
    events.retain(|event| !matches!(event, GameEvent::FormationPerformed { .. }));
    // 待處理隨機性請求仍是作用中行動的一部分。它的來源牌堆不能假裝已提交的
    // 陣形卡牌已經抵達棄牌堆。
    for event in &mut events {
        if let GameEvent::RandomnessRequested { request, .. } = event
            && request.operation.is_discard_shuffle()
        {
            request
                .current_order
                .retain(|card| !composition.physical_cards.contains(card));
        }
    }
    events.insert(
        0,
        GameEvent::FormationCommitted {
            player: player.clone(),
            formation_id: formation_id.clone(),
            cards: composition.physical_cards.clone(),
            star_substitution,
            state: committed_state,
        },
    );
    if state.formation_requirements.iter().any(|requirement| {
        requirement.player == player && requirement.applied_on_turn == state.turn_number
    }) {
        events.push(GameEvent::FormationRequirementFulfilled {
            player: player.clone(),
            formation_id: formation_id.clone(),
            composition: crate::domain::FormationComposition {
                physical_cards: composition.physical_cards.clone(),
                virtual_card: composition.virtual_card.clone(),
            },
        });
    }
    if let Some(event) = crate::rules::confluence::obligation_completion_event(
        state,
        &player,
        &composition.physical_cards,
    ) {
        events.push(event);
    }
    crate::rules::dark::append_mischief_events(state, hp, &mut events)?;
    attack_resolution::absorb_simultaneous_events(
        &mut events,
        crate::domain::HpChangeRole::TriggeredEffect,
    );
    complete(state, events)
}

/// 陣法效果進入 HP seam 前決定防止；完成 session 後絕不可再改寫 delta。
pub(crate) fn formation_hp_request(
    state: &GameState,
    performer: &PlayerId,
    team: &TeamId,
    delta: i32,
) -> HpChangeRequest {
    formation_hp_request_with_poison(state, performer, team, delta, true)
}

/// 少數規則（如 Forest Resonance）只檢查施術者本人中毒，不能把 teammate poison
/// 擴張成 team-wide recovery prevention。
pub(crate) fn formation_hp_request_without_team_poison(
    state: &GameState,
    performer: &PlayerId,
    team: &TeamId,
    delta: i32,
) -> HpChangeRequest {
    formation_hp_request_with_poison(state, performer, team, delta, false)
}

fn formation_hp_request_with_poison(
    state: &GameState,
    performer: &PlayerId,
    team: &TeamId,
    delta: i32,
    prevent_team_poison_recovery: bool,
) -> HpChangeRequest {
    let watch_fire =
        crate::rules::pouch::has_status(state, performer, crate::rules::pouch::WATCH_FIRE_STATUS)
            && !crate::rules::pouch::player_is_protected(state, performer);
    let gale_rain = state.statuses.iter().any(|status| {
        status.kind == "GaleRain"
            && status.owner == crate::domain::StatusOwner::Player(performer.clone())
    });
    if watch_fire
        || (delta > 0
            && (gale_rain
                || (prevent_team_poison_recovery
                    && crate::rules::jianghu::team_has_poison(state, team))))
    {
        HpChangeRequest::Prevented(delta)
    } else {
        HpChangeRequest::By(delta)
    }
}

/// 完成一個可能暫停過的 Formation Use。
///
/// 呼叫端只需交付目前累積的 canonical events；此 deep interface 取得 sequence
/// 的 ownership，並在內部決定 post-Formation hooks、Game Conclusion 與三種合法
/// 結尾的順序。
pub(crate) fn complete(
    state: &GameState,
    mut events: Vec<GameEvent>,
) -> GameResult<Vec<GameEvent>> {
    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }

    if let Some((player, formation_id, _)) = face_up_formation_ready_to_complete(&projected) {
        let registry = base_formation_registry();
        let is_active_spell = registry
            .formation(&formation_id)
            .and_then(|definition| registry.effect_for(definition))
            .is_some_and(|effect| matches!(effect.plan, EffectPlan::ActiveSpell(_)));
        if is_active_spell {
            append_post_formation_events(state, &player, &formation_id, &mut events)?;
        }
    }

    super::append_terminal_game_end(state, &mut events);

    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    if let Some((player, formation_id, cards)) = face_up_formation_ready_to_complete(&projected) {
        events.push(GameEvent::FormationCardsDiscarded {
            player,
            formation_id,
            cards,
        });
    }
    Ok(events)
}

fn face_up_formation_ready_to_complete(
    state: &GameState,
) -> Option<(PlayerId, String, Vec<CardInstanceId>)> {
    if state.pending_choice.is_some()
        || state.pending_randomness.is_some()
        || !matches!(state.status, crate::domain::GameStatus::InProgress)
        || state.phase != crate::domain::Phase::Action
    {
        return None;
    }
    let player = state.current_player()?.clone();
    let formation = state.formation_area(&player)?.formation.as_ref()?;
    matches!(
        formation.state,
        crate::domain::FormationAreaState::FaceUpResolving
    )
    .then(|| {
        (
            player,
            formation.formation_id.clone(),
            formation.cards.clone(),
        )
    })
}

pub(super) fn append_post_formation_events(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
    events: &mut Vec<GameEvent>,
) -> GameResult<()> {
    if events
        .iter()
        .any(|event| matches!(event, GameEvent::GameEnded { .. }))
    {
        return Ok(());
    }

    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    if projected.pending_choice.is_some() || projected.pending_randomness.is_some() {
        return Ok(());
    }

    // 在不改變實際事件流的情況下測試是否終止：終止事實必須保持在最後，因而
    // 陣形後果永遠不能出現在它之後。
    let mut terminal_probe = events.clone();
    super::append_terminal_game_end(state, &mut terminal_probe);
    if terminal_probe
        .iter()
        .any(|event| matches!(event, GameEvent::GameEnded { .. }))
    {
        return Ok(());
    }

    for intent in crate::rules::hero::post_formation_intents(&projected, player, formation_id)? {
        match intent {
            crate::rules::hero::PostFormationIntent::AddTurnDraw { player, amount } => {
                let old_value = projected
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
                events.push(GameEvent::StatusAdded {
                    status: crate::rules::jianghu::shorten_enemy_status(&projected, status),
                });
            }
            crate::rules::hero::PostFormationIntent::EstablishCounterEffect {
                owner,
                effect_id,
            } => {
                events.push(GameEvent::CounterEffectEstablished { owner, effect_id });
            }
        }
    }
    Ok(())
}

pub(crate) fn answer_choice(
    state: &GameState,
    choice: &crate::domain::PendingChoice,
    player: &PlayerId,
    resolution: &PendingResolution,
    selected_cards: &[CardInstanceId],
    hp: &mut HpChangePlan,
) -> GameResult<Vec<GameEvent>> {
    let intents = resume_choice_intents(state, choice, player, resolution, selected_cards)?;
    let mut sequence = FormationEffectSequence::new(state);
    super::effect_intent::effect_intent_events_with_plan(intents, hp, Some(player), &mut sequence)?;
    if let Some((events, outcome)) =
        crate::rules::confluence::after_choice_events(sequence.state(), player, resolution, hp)?
    {
        sequence.append(hp, ResolvedFormationEffect::new(events, outcome))?;
    }
    Ok(sequence.into_events())
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BaseFormationPlanner;

impl BaseFormationPlanner {
    fn new() -> Self {
        Self
    }

    fn plan_use(
        &self,
        state: &GameState,
        request: FormationUseRequest,
    ) -> GameResult<FormationUsePlan> {
        let selected = FormationSelection::new(state, &request.player, request.cards)?
            .require(&request.formation_id, request.declared_targets)?;

        Ok(FormationUsePlan {
            player: request.player,
            formation_id: selected.formation_id,
            cards: selected.cards,
            composition: selected.composition,
            declared_targets: selected.declared_targets,
            star_substitution: selected.star_substitution,
            effect_plan: selected.effect_plan,
            trusted_random_cards: request.trusted_random_cards,
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BaseEffectResolver;

impl BaseEffectResolver {
    fn new() -> Self {
        Self
    }

    fn resolve(
        &self,
        state: &GameState,
        plan: FormationUsePlan,
        hp: &mut HpChangePlan,
    ) -> GameResult<Vec<GameEvent>> {
        match &plan.effect_plan {
            EffectPlan::Attack(attack_plan) => {
                if has_effect_targets(&plan.declared_targets) {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }
                let attack_points = attack_resolution::preview_attack_points(
                    state,
                    &plan.player,
                    &plan.formation_id,
                    attack_plan,
                    &plan.cards,
                )?;
                let ignores_formation_effects = sacred_beast_element(&plan.formation_id).is_some()
                    || crate::rules::jianghu::ignores_other_formation_effects(state, &plan.player)
                    || plan.formation_id == crate::rules::jianghu::SNOW_TREADING_SWORD_ART;
                let passive_trigger = covered_passive::trigger(
                    state,
                    TriggerRequest {
                        incoming_player: plan.player.clone(),
                        incoming_kind: IncomingActionKind::Attack,
                        ignores_formation_effects,
                        ignores_counter_effects_by_profession_ability:
                            crate::rules::hero::windwalking_applies(
                                state,
                                &plan.player,
                                attack_points,
                            ),
                        ignores_counter_effects_by_golden_cicada:
                            crate::rules::pouch::player_is_protected(state, &plan.player),
                        attack_points: Some(attack_points),
                    },
                );
                let environment_ineffective = !ignores_formation_effects
                    && !crate::rules::dark::ignores_environment(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    )
                    && environment_makes_formation_ineffective(state, &plan.formation_id);
                let formation_suppressed = crate::rules::echo::formation_use_is_suppressed(
                    state,
                    &plan.player,
                    &plan.formation_id,
                );
                let damage_prevented = passive_trigger.prevents_damage()
                    || environment_ineffective
                    || formation_suppressed
                    || crate::rules::spirit::stone_shield_prevents_attack(state, &plan.player)
                    || (crate::rules::pouch::has_status(
                        state,
                        &plan.player,
                        crate::rules::pouch::WATCH_FIRE_STATUS,
                    ) && !crate::rules::pouch::player_is_protected(state, &plan.player));
                let split_attack_damage = passive_trigger.splits_attack_damage();
                let mut prefix = crate::rules::dark::pre_formation_events(
                    state,
                    &plan.player,
                    &plan.formation_id,
                );
                prefix.extend(match_option_events(&plan));
                prefix.extend(passive_trigger.events());
                prefix.extend(crate::rules::jianghu::poison_smoke_flip_events(
                    state, &prefix,
                ));
                let mut sequence = FormationEffectSequence::new(state);
                sequence.extend(prefix);
                if environment_ineffective || formation_suppressed {
                    sequence.extend([formation_effect_ignored_event(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    )]);
                }
                if plan.formation_id == crate::rules::tribulation::EARTH_RENDING
                    && !environment_ineffective
                    && !formation_suppressed
                {
                    sequence.extend(crate::rules::tribulation::earth_rending_start_events(
                        state,
                        &plan.player,
                        &plan.cards,
                        damage_prevented,
                        split_attack_damage,
                    )?);
                    return Ok(sequence.into_events());
                }
                if plan.formation_id == crate::rules::tribulation::RUSTED_FOREST
                    && !environment_ineffective
                    && !formation_suppressed
                {
                    sequence.extend(crate::rules::tribulation::rusted_forest_start_events(
                        state,
                        &plan.player,
                        &plan.cards,
                        damage_prevented,
                        split_attack_damage,
                        hp,
                    )?);
                    return Ok(sequence.into_events());
                }
                let (tribulation_pre_events, tribulation_effect) =
                    if environment_ineffective || formation_suppressed {
                        (Vec::new(), None)
                    } else {
                        crate::rules::tribulation::pre_attack_events(
                            sequence.state(),
                            &plan.player,
                            &plan.formation_id,
                            hp,
                        )?
                    };
                let pre_primary_hp_count = tribulation_pre_events
                    .iter()
                    .filter(|event| matches!(event, GameEvent::HpChanged { .. }))
                    .count();
                let point_formula = crate::rules::tribulation::attack_points(
                    &plan.formation_id,
                    &tribulation_pre_events,
                )
                .map(PointFormula::Fixed)
                .unwrap_or_else(|| attack_plan.point_formula.clone());
                let (attack_state, pre_resolution_effects) =
                    if let Some(outcome) = tribulation_effect {
                        sequence.append(
                            hp,
                            ResolvedFormationEffect::new(tribulation_pre_events.clone(), outcome),
                        )?;
                        (
                            sequence.state().clone(),
                            attack_resolution::effects_from_events(&[])?,
                        )
                    } else {
                        // 非 HP 的攻擊前效果（泥石轟流）仍屬於 AttackResolved 的原子
                        // payload。先投影它們取得正確攻擊狀態，但不輸出獨立事件。
                        let mut projected = sequence.state().clone();
                        for event in &tribulation_pre_events {
                            crate::rules::projection::apply_event(&mut projected, event);
                        }
                        (
                            projected,
                            attack_resolution::effects_from_events(&tribulation_pre_events)?,
                        )
                    };
                // HP 前效果及同命已投影；非 HP 前效果則由 AttackResolved 擁有。
                sequence.extend(attack_resolution::resolve_with_plan(
                    &attack_state,
                    AttackRequest {
                        attacker: plan.player.clone(),
                        formation_id: plan.formation_id.clone(),
                        category: attack_plan.category.clone(),
                        point_formula,
                        used_cards: plan.cards.clone(),
                        damage_prevented,
                        split_attack_damage,
                        mode: AttackResolutionMode::FormationUse,
                        pre_resolution_effects,
                        trailing_hp_role: crate::domain::HpChangeRole::FormationEffect,
                    },
                    hp,
                )?);
                if !environment_ineffective && !formation_suppressed {
                    sequence.extend(crate::rules::tribulation::post_attack_events(
                        sequence.state(),
                        &plan.player,
                        &plan.formation_id,
                    )?);
                } else {
                    sequence.extend(
                        crate::rules::tribulation::divine_calculation_consumption_events(
                            state,
                            &plan.formation_id,
                        ),
                    );
                }

                let (dark_events, outcome) = crate::rules::dark::post_attack_events(
                    sequence.state(),
                    &plan.player,
                    &plan.formation_id,
                    &plan.cards,
                    hp,
                )?;
                let post_primary_hp_count = dark_events
                    .iter()
                    .filter(|event| matches!(event, GameEvent::HpChanged { .. }))
                    .count();
                if let Some(outcome) = outcome {
                    sequence.append(hp, ResolvedFormationEffect::new(dark_events, outcome))?;
                } else {
                    sequence.extend(dark_events);
                }
                let mut events = sequence.into_events();
                absorb_formation_effect_hp_events(
                    &mut events,
                    pre_primary_hp_count,
                    post_primary_hp_count,
                );
                attack_resolution::absorb_simultaneous_events(
                    &mut events,
                    crate::domain::HpChangeRole::TriggeredEffect,
                );
                Ok(events)
            }
            EffectPlan::PassiveSpell(_) => {
                if has_effect_targets(&plan.declared_targets) {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }

                if state
                    .formation_area(&plan.player)
                    .is_some_and(|area| area.formation.is_some())
                {
                    return Err(GameError::Validation(
                        ValidationError::PendingPassiveAlreadyCovered {
                            player: plan.player,
                        },
                    ));
                }

                let passive_trigger = covered_passive::trigger(
                    state,
                    TriggerRequest {
                        incoming_player: plan.player.clone(),
                        incoming_kind: IncomingActionKind::PassiveSpell,
                        ignores_formation_effects: false,
                        ignores_counter_effects_by_profession_ability:
                            crate::rules::hero::spell_counter_immunity(state, &plan.player),
                        ignores_counter_effects_by_golden_cicada:
                            crate::rules::pouch::player_is_protected(state, &plan.player),
                        attack_points: None,
                    },
                );
                let sealed = passive_trigger.seals_covered_passive();
                let revealed = passive_trigger.reveals_covered_passive();
                let mut events = crate::rules::dark::pre_formation_events(
                    state,
                    &plan.player,
                    &plan.formation_id,
                );
                events.extend(match_option_events(&plan));
                events.extend(passive_trigger.events());
                events.extend(crate::rules::jianghu::poison_smoke_flip_events(
                    state, &events,
                ));
                if crate::rules::echo::formation_use_is_suppressed(
                    state,
                    &plan.player,
                    &plan.formation_id,
                ) {
                    events.push(GameEvent::FormationPerformed {
                        player: plan.player.clone(),
                        formation_id: plan.formation_id.clone(),
                        used_cards: plan.cards.clone(),
                        declared_targets: plan.declared_targets.clone(),
                    });
                    events.push(formation_effect_ignored_event(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    ));
                    return Ok(events);
                }
                events.push(GameEvent::PassiveCovered {
                    player: plan.player.clone(),
                    formation_id: plan.formation_id,
                    cards: plan.cards,
                    star_substitution: plan.star_substitution,
                    sealed,
                });
                if revealed {
                    events.push(GameEvent::PassiveCoverRevealed { owner: plan.player });
                }
                Ok(events)
            }
            EffectPlan::ActiveSpell(spell) => {
                if spell.resolver_id != crate::rules::pouch::CHAIN_ID
                    && has_effect_targets(&plan.declared_targets)
                {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }

                let passive_trigger = covered_passive::trigger(
                    state,
                    TriggerRequest {
                        incoming_player: plan.player.clone(),
                        incoming_kind: IncomingActionKind::ActiveSpell,
                        ignores_formation_effects: false,
                        ignores_counter_effects_by_profession_ability:
                            crate::rules::hero::spell_counter_immunity(state, &plan.player),
                        ignores_counter_effects_by_golden_cicada:
                            crate::rules::pouch::player_is_protected(state, &plan.player),
                        attack_points: None,
                    },
                );
                let spell_cancelled = passive_trigger.cancels_spell();
                let spell_ineffective =
                    !crate::rules::dark::ignores_environment(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    ) && environment_makes_formation_ineffective(state, &plan.formation_id)
                        || crate::rules::echo::formation_use_is_suppressed(
                            state,
                            &plan.player,
                            &plan.formation_id,
                        );
                let mut events = crate::rules::dark::pre_formation_events(
                    state,
                    &plan.player,
                    &plan.formation_id,
                );
                events.extend(match_option_events(&plan));
                events.extend(passive_trigger.events());
                events.extend(crate::rules::jianghu::poison_smoke_flip_events(
                    state, &events,
                ));
                let composite_spell_succeeds = matches!(
                    spell.resolver_id.as_str(),
                    "void-reversion" | "void-spirit-shattering"
                ) && !spell_cancelled
                    && !spell_ineffective;
                if !composite_spell_succeeds {
                    events.push(GameEvent::FormationPerformed {
                        player: plan.player.clone(),
                        formation_id: plan.formation_id.clone(),
                        used_cards: plan.cards.clone(),
                        declared_targets: plan.declared_targets.clone(),
                    });
                }
                if !spell_cancelled && spell_ineffective {
                    events.push(formation_effect_ignored_event(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    ));
                }
                if !spell_cancelled && !spell_ineffective {
                    if spell.resolver_id == crate::rules::pouch::CHAIN_ID {
                        if !plan.declared_targets.is_empty() {
                            return Err(GameError::Validation(
                                crate::domain::ValidationError::SecretStrategyInputInvalid,
                            ));
                        }
                        events.extend(crate::rules::pouch::chain_events(
                            state,
                            &plan.player,
                            None,
                        )?);
                        return Ok(events);
                    }
                    if spell.resolver_id == "void-reversion" {
                        let mut sequence = FormationEffectSequence::new(state);
                        sequence.extend(events.clone());
                        if let Some((events, outcome)) =
                            crate::rules::confluence::void_transcendence_events(
                                sequence.state(),
                                &plan.player,
                                &plan.cards,
                                hp,
                            )?
                        {
                            sequence.append(hp, ResolvedFormationEffect::new(events, outcome))?;
                        }
                        let (event, outcome) =
                            void_reversion_event(sequence.state(), &plan.player, &plan.cards, hp)?;
                        sequence.append(hp, ResolvedFormationEffect::new(vec![event], outcome))?;
                        sequence.extend(crate::rules::confluence::void_realm_consumption_events(
                            sequence.state(),
                            &plan.player,
                            &plan.cards,
                        ));
                        return Ok(sequence.into_events());
                    }
                    if spell.resolver_id == "void-spirit-shattering" {
                        let event = crate::rules::spirit::void_spirit_shattering_event(
                            state,
                            &plan.player,
                            &plan.cards,
                            hp,
                        )?;
                        // 此 canonical 複合事件內含同命結算；不可再交給外層重複處理。
                        events.push(event);
                        return Ok(events);
                    }
                    if spell.resolver_id == "void-meridian-severing" {
                        let mut sequence = FormationEffectSequence::new(state);
                        sequence.extend(events);
                        if let Some((event, outcome)) =
                            environment_clearing_event(sequence.state(), &plan.player, hp)?
                        {
                            sequence
                                .append(hp, ResolvedFormationEffect::new(vec![event], outcome))?;
                        }
                        return Ok(sequence.into_events());
                    }
                    if spell.resolver_id == "void-star-breaking" {
                        let mut sequence = FormationEffectSequence::new(state);
                        sequence.extend(events);
                        let (star_events, outcome) =
                            void_star_breaking_events(sequence.state(), &plan.player, hp)?;
                        if let Some(outcome) = outcome {
                            sequence
                                .append(hp, ResolvedFormationEffect::new(star_events, outcome))?;
                        } else {
                            sequence.extend(star_events);
                        }
                        return Ok(sequence.into_events());
                    }
                    if let Some(mut echo_events) = {
                        let mut projected = state.clone();
                        for event in &events {
                            crate::rules::projection::apply_event(&mut projected, event);
                        }
                        crate::rules::echo::formation_main_effect_events(
                            &projected,
                            &plan.player,
                            &spell.resolver_id,
                            hp,
                        )?
                    } {
                        events.append(&mut echo_events);
                        return Ok(events);
                    }
                    if let Some(mut tribulation_events) =
                        crate::rules::tribulation::active_spell_events(
                            state,
                            &plan.player,
                            &spell.resolver_id,
                        )
                    {
                        events.append(&mut tribulation_events);
                        return Ok(events);
                    }
                    if let Some(jianghu_events) = {
                        let mut sequence = FormationEffectSequence::new(state);
                        sequence.extend(events.clone());
                        let result = crate::rules::jianghu::active_spell_events(
                            sequence.state(),
                            &plan.player,
                            &spell.resolver_id,
                            hp,
                        )?;
                        if let Some((jianghu_events, outcome)) = result {
                            if let Some(outcome) = outcome {
                                sequence.append(
                                    hp,
                                    ResolvedFormationEffect::new(jianghu_events, outcome),
                                )?;
                            } else {
                                sequence.extend(jianghu_events);
                            }
                            Some(sequence.into_events())
                        } else {
                            None
                        }
                    } {
                        return Ok(jianghu_events);
                    }
                    if let Some(confluence_events) = {
                        let mut sequence = FormationEffectSequence::new(state);
                        sequence.extend(events.clone());
                        if spell.resolver_id == crate::rules::confluence::VOID_RETURN_TO_NOTHING {
                            if let Some((events, outcome)) =
                                crate::rules::confluence::void_transcendence_events(
                                    sequence.state(),
                                    &plan.player,
                                    &plan.cards,
                                    hp,
                                )?
                            {
                                sequence
                                    .append(hp, ResolvedFormationEffect::new(events, outcome))?;
                            }
                        }
                        let result = crate::rules::confluence::active_spell_events(
                            sequence.state(),
                            &plan.player,
                            &spell.resolver_id,
                            &plan.declared_targets,
                            hp,
                        )?;
                        if let Some((events, outcome)) = result {
                            if let Some(outcome) = outcome {
                                sequence
                                    .append(hp, ResolvedFormationEffect::new(events, outcome))?;
                            } else {
                                sequence.extend(events);
                            }
                            Some(sequence.into_events())
                        } else {
                            None
                        }
                    } {
                        return Ok(confluence_events);
                    }
                    if let Some(dark_events) = {
                        let mut sequence = FormationEffectSequence::new(state);
                        sequence.extend(events.clone());
                        let result = crate::rules::dark::active_spell_events(
                            sequence.state(),
                            &plan.player,
                            &spell.resolver_id,
                            &plan.cards,
                            plan.trusted_random_cards.as_deref(),
                            hp,
                        )?;
                        if let Some((dark_events, outcome)) = result {
                            if let Some(outcome) = outcome {
                                sequence.append(
                                    hp,
                                    ResolvedFormationEffect::new(dark_events, outcome),
                                )?;
                            } else {
                                sequence.extend(dark_events);
                            }
                            Some(sequence.into_events())
                        } else {
                            None
                        }
                    } {
                        return Ok(dark_events);
                    }
                    if let Some(spirit) =
                        crate::rules::spirit::summoning_formation_spirit(&spell.resolver_id)
                    {
                        let mut sequence = FormationEffectSequence::new(state);
                        sequence.extend(events);
                        let previous = sequence
                            .state()
                            .spirit_for(&plan.player)
                            .map(|owned| owned.spirit);
                        sequence.extend([GameEvent::SpiritSummoned {
                            player: plan.player.clone(),
                            previous,
                            spirit,
                        }]);
                        return Ok(sequence.into_events());
                    }
                    let (copied_effect_id, intents) = if spell.resolver_id == "metamorphosis" {
                        metamorphosis_intents(state, &plan.player, &plan.cards)?
                    } else {
                        (
                            None,
                            active_spell_intents(
                                state,
                                &plan.player,
                                &spell.resolver_id,
                                &plan.cards,
                                &plan.declared_targets,
                            )?,
                        )
                    };
                    if let Some(effect_id) = copied_effect_id {
                        events.push(GameEvent::FormationEffectCopied {
                            player: plan.player.clone(),
                            effect_id,
                        });
                    }
                    let mut sequence = FormationEffectSequence::new(state);
                    sequence.extend(events);
                    super::effect_intent::effect_intent_events_with_plan(
                        intents,
                        hp,
                        Some(&plan.player),
                        &mut sequence,
                    )?;
                    return Ok(sequence.into_events());
                }
                Ok(events)
            }
        }
    }
}

/// 將已依序規劃的攻擊前後陣法效果重收納為 AttackResolved 的明確角色。
/// 規劃與投影先在 sequence 中完成，這裡只調整等價的 canonical event 形狀。
pub(crate) fn absorb_formation_effect_hp_events(
    events: &mut Vec<GameEvent>,
    pre_primary_hp_count: usize,
    post_primary_hp_count: usize,
) {
    let Some(attack_index) = events
        .iter()
        .position(|event| matches!(event, GameEvent::AttackResolved { .. }))
    else {
        return;
    };

    let mut before = Vec::new();
    let mut retained_before = Vec::new();
    for event in events.drain(..attack_index) {
        match event {
            GameEvent::HpChanged { change } => before.push(change),
            event => retained_before.push(event),
        }
    }
    let attack = events.remove(0);
    let mut after = Vec::new();
    let mut retained_after = Vec::new();
    for event in events.drain(..) {
        match event {
            GameEvent::HpChanged { change } => after.push(change),
            event => retained_after.push(event),
        }
    }
    let mut attack = attack;
    if let GameEvent::AttackResolved { hp_changes, .. } = &mut attack {
        let before_len = before.len();
        let mut merged = Vec::with_capacity(before_len + hp_changes.len() + after.len());
        merged.extend(before.into_iter().enumerate().map(|(index, change)| {
            crate::domain::ResolvedHpChange {
                role: if index < pre_primary_hp_count {
                    crate::domain::HpChangeRole::FormationEffect
                } else {
                    crate::domain::HpChangeRole::TriggeredEffect
                },
                change,
            }
        }));
        merged.append(hp_changes);
        merged.extend(after.into_iter().enumerate().map(|(index, change)| {
            crate::domain::ResolvedHpChange {
                role: if index < post_primary_hp_count {
                    crate::domain::HpChangeRole::FormationEffect
                } else {
                    crate::domain::HpChangeRole::TriggeredEffect
                },
                change,
            }
        }));
        *hp_changes = merged;
    }
    retained_before.push(attack);
    retained_before.extend(retained_after);
    *events = retained_before;
}

fn has_effect_targets(targets: &[TargetDecl]) -> bool {
    targets.iter().any(|target| {
        matches!(
            target,
            TargetDecl::Player(_) | TargetDecl::Team(_) | TargetDecl::Card(_)
        )
    })
}

fn match_option_events(plan: &FormationUsePlan) -> Vec<GameEvent> {
    let targets = plan
        .declared_targets
        .iter()
        .filter(|target| {
            matches!(
                target,
                TargetDecl::FormationRole { .. } | TargetDecl::CardMultiplicity { .. }
            )
        })
        .cloned()
        .collect::<Vec<_>>();
    if targets.is_empty() {
        Vec::new()
    } else {
        vec![GameEvent::FormationMatchOptionDeclared {
            player: plan.player.clone(),
            formation_id: plan.formation_id.clone(),
            targets,
        }]
    }
}

fn void_star_breaking_events(
    state: &GameState,
    player: &PlayerId,
    hp: &mut HpChangePlan,
) -> GameResult<(Vec<GameEvent>, Option<FormationHpEffectOutcome>)> {
    let mut effect = hp.begin_formation_effect();
    let mut events = state
        .team_stars
        .iter()
        .map(|owned| -> GameResult<_> {
            Ok(GameEvent::StarBroken {
                team: owned.team.clone(),
                star: owned.star,
                reason: crate::domain::StarBreakReason::VoidStarBreaking,
                hp_change: Some(effect.plan(
                    &owned.team,
                    formation_hp_request(state, player, &owned.team, -20),
                )?),
            })
        })
        .collect::<GameResult<Vec<_>>>()?;

    if !events.is_empty() {
        events.push(GameEvent::VoidStarBreakingCompleted {
            player: player.clone(),
        });
    }
    let outcome = (!events.is_empty()).then(|| effect.finish());
    Ok((events, outcome))
}

fn formation_effect_ignored_event(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) -> GameEvent {
    if let Some(suppression) = state.formation_suppressions.iter().find(|suppression| {
        &suppression.target == player
            && suppression.formation_id == formation_id
            && suppression.expires_on_turn_number == state.turn_number
    }) {
        return GameEvent::FormationEffectIgnored {
            player: player.clone(),
            formation_id: formation_id.to_string(),
            reason: crate::domain::FormationNoEffectReason::SuppressedBySplitEarth {
                source: suppression.source.clone(),
            },
        };
    }
    let environment = state
        .environment
        .expect("an Environment can only make a Formation ineffective while it exists");
    GameEvent::FormationEffectIgnored {
        player: player.clone(),
        formation_id: formation_id.to_string(),
        reason: crate::domain::FormationNoEffectReason::IneffectiveInEnvironment { environment },
    }
}

fn environment_clearing_event(
    state: &GameState,
    player: &PlayerId,
    hp: &mut HpChangePlan,
) -> GameResult<Option<(GameEvent, FormationHpEffectOutcome)>> {
    let Some(environment) = state.environment else {
        return Ok(None);
    };
    let mut effect = hp.begin_formation_effect();
    let hp_changes = state
        .hp
        .iter()
        .map(|team_hp| {
            effect.plan(
                &team_hp.team,
                formation_hp_request(state, player, &team_hp.team, -20),
            )
        })
        .collect::<GameResult<Vec<_>>>()?;

    Ok(Some((
        GameEvent::EnvironmentCleared {
            player: player.clone(),
            formation_id: "void-meridian-severing".to_string(),
            environment,
            hp_changes,
        },
        effect.finish(),
    )))
}

fn void_reversion_event(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
    hp: &mut HpChangePlan,
) -> GameResult<(GameEvent, FormationHpEffectOutcome)> {
    let team = player_team(state, player)?;
    let high_level = cards.iter().try_fold(true, |all_high_level, card| {
        state
            .card_level_for(player, *card)
            .map(|level| all_high_level && level >= 3)
            .ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))
    })?;
    let (broken_professions, retained_legendary_professions) = state
        .professions
        .iter()
        .filter(|owned| {
            !(high_level && crate::rules::confluence::void_realm_protects(state, &owned.player))
        })
        .cloned()
        .partition(|owned| {
            high_level || !crate::rules::profession::is_legendary(&owned.profession)
        });
    let card_moves = cards
        .iter()
        .copied()
        .map(|card| crate::domain::discard::move_from(state, card, CardZone::Hand(player.clone())))
        .collect::<GameResult<Vec<_>>>()?;

    let mut effect = hp.begin_formation_effect();
    let event = GameEvent::VoidReversionResolved {
        player: player.clone(),
        hp_change: effect.plan(&team, formation_hp_request(state, player, &team, -20))?,
        card_moves,
        broken_professions,
        retained_legendary_professions,
    };
    Ok((event, effect.finish()))
}

fn active_spell_intents(
    state: &GameState,
    player: &PlayerId,
    resolver_id: &str,
    used_cards: &[CardInstanceId],
    declared_targets: &[TargetDecl],
) -> GameResult<Vec<EffectIntent>> {
    match resolver_id {
        "reincarnation" => {
            let standalone = declared_targets.iter().find_map(|target| match target {
                TargetDecl::FormationRole { role, card } if role == "standalone-wood" => {
                    Some(*card)
                }
                _ => None,
            });
            let standalone = standalone.ok_or(GameError::Validation(
                ValidationError::FormationMatchOptionRequired {
                    formation_id: resolver_id.to_string(),
                },
            ))?;
            let level = state
                .card_level_for(player, standalone)
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(standalone),
                ))?
                .value() as i32;
            Ok(vec![EffectIntent::ChangeHp {
                team: player_team(state, player)?,
                delta: level * 25,
            }])
        }
        "purple-light-shield" => Ok(vec![EffectIntent::SetShield {
            player: player.clone(),
            value: level_sum(state, player, used_cards)? * 5,
        }]),
        "shadow-assault" => {
            let target =
                resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)?;
            Ok(vec![EffectIntent::ChangeHp {
                team: player_team(state, &target)?,
                delta: -level_sum(state, player, used_cards)? * 3,
            }])
        }
        "instant-shadow-death" => {
            let target =
                resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)?;
            let team = player_team(state, &target)?;
            let old_hp = team_hp(state, &team)?;
            Ok(vec![EffectIntent::ChangeHp {
                team,
                delta: old_hp / 2 - old_hp,
            }])
        }
        "holy-wind" => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let team = player_team(state, &target)?;
            let hand = state.hand(&target).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
            })?;
            let highest = hand
                .iter()
                .filter_map(|card| state.effective_card_facts(&target, *card))
                .map(|facts| facts.level)
                .max();
            let allowed_cards = highest.map_or_else(Vec::new, |highest| {
                hand.iter()
                    .copied()
                    .filter(|card| {
                        state
                            .effective_card_facts(&target, *card)
                            .is_some_and(|facts| facts.level == highest)
                    })
                    .collect()
            });
            let mut intents = vec![
                EffectIntent::ChangeHp { team, delta: -20 },
                EffectIntent::InspectHand {
                    viewer: player.clone(),
                    target: target.clone(),
                    cards: hand.to_vec(),
                },
            ];
            if !allowed_cards.is_empty() {
                intents.push(EffectIntent::RequestChoice {
                    request: crate::domain::ChoiceRequest {
                        player: player.clone(),
                        kind: crate::domain::PendingChoiceKind::Card {
                            cards: allowed_cards,
                            minimum: 1,
                            maximum: 1,
                            can_decline: false,
                        },
                        resolution: PendingResolution::HolyWindTakeHighest,
                    },
                });
            }
            Ok(intents)
        }
        "barrier" => Ok(vec![EffectIntent::SetShield {
            player: player.clone(),
            value: level_sum(state, player, used_cards)? * 4,
        }]),
        "metamorphosis" => Ok(metamorphosis_intents(state, player, used_cards)?.1),
        "generating-formation" => {
            let team = resolve_rule_team_target(state, player, RuleTeamTarget::OwnSide)?;
            Ok(vec![EffectIntent::ChangeHp {
                team,
                delta: level_sum(state, player, used_cards)? * 3,
            }])
        }
        "overcoming-formation" => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let old_value = state.shield(&target).unwrap_or(0);
            let new_value = (old_value - level_sum(state, player, used_cards)? * 3).max(0);
            Ok(vec![EffectIntent::SetShield {
                player: target,
                value: new_value,
            }])
        }
        "radiance" => {
            if crate::rules::hero::target_ignores_disruptive_spell(state, player, resolver_id)? {
                return Ok(Vec::new());
            }
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let inspected_cards = state
                .hand(&target)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
                })?
                .to_vec();
            let expires_at = nth_future_turn_for_player(state, &target, 2)?;
            Ok(vec![
                EffectIntent::InspectHand {
                    viewer: player.clone(),
                    target: target.clone(),
                    cards: inspected_cards,
                },
                EffectIntent::AddStatus {
                    status: crate::domain::StatusEffect {
                        id: format!(
                            "radiance-cannot-act-{}-turn-{}",
                            target.as_str(),
                            state.turn_number
                        ),
                        owner: crate::domain::StatusOwner::Player(target.clone()),
                        kind: "CannotAct".to_string(),
                        value: None,
                        duration: crate::domain::StatusDuration::UntilTurnEndNumber {
                            player: target.clone(),
                            turn_number: expires_at,
                        },
                    },
                },
                EffectIntent::AddStatus {
                    status: crate::domain::StatusEffect {
                        id: format!(
                            "radiance-cannot-draw-{}-turn-{}",
                            target.as_str(),
                            state.turn_number
                        ),
                        owner: crate::domain::StatusOwner::Player(target.clone()),
                        kind: "CannotDraw".to_string(),
                        value: None,
                        duration: crate::domain::StatusDuration::UntilTurnEndNumber {
                            player: target,
                            turn_number: expires_at,
                        },
                    },
                },
            ])
        }
        "chaos" => {
            if crate::rules::hero::target_ignores_disruptive_spell(state, player, resolver_id)? {
                return Ok(Vec::new());
            }
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let allowed_cards = state
                .hand(&target)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
                })?
                .to_vec();
            if allowed_cards.is_empty() {
                return Ok(Vec::new());
            }
            Ok(vec![
                EffectIntent::InspectHand {
                    viewer: player.clone(),
                    target: target.clone(),
                    cards: allowed_cards.clone(),
                },
                EffectIntent::RequestChoice {
                    request: crate::domain::ChoiceRequest {
                        player: player.clone(),
                        kind: crate::domain::PendingChoiceKind::Card {
                            maximum: allowed_cards.len().min(2),
                            minimum: allowed_cards.len().min(2),
                            cards: allowed_cards,
                            can_decline: false,
                        },
                        resolution: PendingResolution::ChaosReturnTwo,
                    },
                },
            ])
        }
        "return-to-origin" => {
            let team = player_team(state, player)?;
            Ok(vec![EffectIntent::ChangeHp {
                team,
                delta: level_sum(state, player, used_cards)? * 4,
            }])
        }
        "five-elements-cycle" => {
            let own_team = player_team(state, player)?;
            let opposing_team =
                resolve_rule_team_target(state, player, RuleTeamTarget::OpposingSide)?;
            let own_hp = team_hp(state, &own_team)?;
            let opposing_hp = team_hp(state, &opposing_team)?;
            Ok(vec![
                EffectIntent::ChangeHp {
                    team: own_team,
                    delta: opposing_hp - own_hp,
                },
                EffectIntent::ChangeHp {
                    team: opposing_team,
                    delta: own_hp - opposing_hp,
                },
            ])
        }
        _ => Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(resolver_id.to_string()),
        )),
    }
}

fn metamorphosis_intents(
    state: &GameState,
    player: &PlayerId,
    used_cards: &[CardInstanceId],
) -> GameResult<(Option<String>, Vec<EffectIntent>)> {
    let previous_player =
        resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)?;
    let Some(last_formation) = formation_resolved_on_previous_turn(state, &previous_player) else {
        return Ok((None, Vec::new()));
    };

    let registry = base_formation_registry();
    let Some(formation) = registry.formation(last_formation.effective_effect_id()) else {
        return Ok((None, Vec::new()));
    };
    let effect = registry
        .effect_for(formation)
        .expect("base formation registry must link every formation to an effect");

    match &effect.plan {
        EffectPlan::Attack(plan) => Ok((
            Some(formation.id.clone()),
            vec![EffectIntent::ResolveCopiedAttack {
                formation_id: formation.id.clone(),
                category: plan.category.clone(),
                point_formula: plan.point_formula.clone(),
                used_cards: used_cards.to_vec(),
            }],
        )),
        EffectPlan::ActiveSpell(spell) if spell.resolver_id != "metamorphosis" => Ok((
            Some(formation.id.clone()),
            active_spell_intents(state, player, &spell.resolver_id, used_cards, &[])?,
        )),
        EffectPlan::PassiveSpell(_) if formation.id == "empty-city" => {
            Ok((Some(formation.id.clone()), Vec::new()))
        }
        EffectPlan::PassiveSpell(_) => Ok((
            Some(formation.id.clone()),
            vec![EffectIntent::EstablishCounterEffect {
                owner: player.clone(),
                effect_id: formation.id.clone(),
            }],
        )),
        EffectPlan::ActiveSpell(_) => Ok((None, Vec::new())),
    }
}

fn level_sum(state: &GameState, player: &PlayerId, cards: &[CardInstanceId]) -> GameResult<i32> {
    let physical = cards.iter().try_fold(0, |sum, card| {
        let level = state
            .card_level_for(player, *card)
            .ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))?
            .value() as i32;
        Ok(sum + level)
    })?;
    Ok(physical
        + state
            .formation_requirements
            .iter()
            .find(|requirement| {
                &requirement.player == player && requirement.applied_on_turn == state.turn_number
            })
            .and_then(|requirement| requirement.virtual_card.as_ref())
            .map_or(0, |card| card.level.value() as i32))
}

fn team_hp(state: &GameState, team: &TeamId) -> GameResult<i32> {
    state
        .hp
        .iter()
        .find(|team_hp| &team_hp.team == team)
        .map(|team_hp| team_hp.hp)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))
}

fn nth_future_turn_for_player(
    state: &GameState,
    player: &PlayerId,
    occurrence: usize,
) -> GameResult<u64> {
    TurnOrderTargets::new(state).nth_future_turn_for_player(player, occurrence)
}

fn resume_choice_intents(
    state: &GameState,
    choice: &crate::domain::PendingChoice,
    player: &PlayerId,
    resolution: &PendingResolution,
    selected_cards: &[CardInstanceId],
) -> GameResult<Vec<EffectIntent>> {
    match resolution {
        PendingResolution::ConfluenceClearWindDiscardTop => {
            let Some(card) = selected_cards.first().copied() else {
                return Ok(Vec::new());
            };
            Ok(vec![EffectIntent::MoveCards {
                card_moves: vec![crate::domain::discard::move_from(
                    state,
                    card,
                    if state.uses_personal_decks() {
                        CardZone::PlayerDeckTop(player.clone())
                    } else {
                        CardZone::DeckTop
                    },
                )?],
            }])
        }
        PendingResolution::ChaosReturnTwo => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let target_hand = state.hand(&target).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
            })?;
            let required_count = target_hand.len().min(2);
            if selected_cards.len() != required_count {
                return Err(GameError::Validation(ValidationError::MissingPendingChoice));
            }

            Ok(vec![EffectIntent::MoveCards {
                card_moves: selected_cards
                    .iter()
                    .rev()
                    .copied()
                    .map(|card| CardMoveDelta {
                        card,
                        from: CardZone::Hand(target.clone()),
                        to: if state.uses_personal_decks() {
                            CardZone::PlayerDeckTop(player.clone())
                        } else {
                            CardZone::DeckTop
                        },
                    })
                    .collect(),
            }])
        }
        PendingResolution::HolyWindTakeHighest => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let card = *selected_cards
                .first()
                .ok_or(GameError::Validation(ValidationError::MissingPendingChoice))?;
            Ok(vec![EffectIntent::MoveCards {
                card_moves: vec![CardMoveDelta {
                    card,
                    from: CardZone::Hand(target),
                    to: CardZone::Hand(player.clone()),
                }],
            }])
        }
        PendingResolution::HeroRevelationKeepOne => {
            let allowed_cards = match &choice.kind {
                crate::domain::PendingChoiceKind::Card { cards, .. } => cards,
                _ => return Err(GameError::Validation(ValidationError::MissingPendingChoice)),
            };
            Ok(vec![EffectIntent::MoveCards {
                card_moves: allowed_cards
                    .iter()
                    .filter(|card| !selected_cards.contains(card))
                    .copied()
                    .map(|card| {
                        crate::domain::discard::move_from(
                            state,
                            card,
                            CardZone::Hand(player.clone()),
                        )
                    })
                    .collect::<GameResult<Vec<_>>>()?,
            }])
        }
        PendingResolution::JianghuAzureCloudStepReturnOne => {
            let allowed_cards = match &choice.kind {
                crate::domain::PendingChoiceKind::Card { cards, .. } => cards,
                _ => {
                    return Err(GameError::Validation(ValidationError::MissingPendingChoice));
                }
            };
            if selected_cards.len() != 1 || !allowed_cards.contains(&selected_cards[0]) {
                return Err(GameError::Validation(ValidationError::MissingPendingChoice));
            }
            let returned = selected_cards[0];
            Ok(vec![EffectIntent::MoveCards {
                card_moves: allowed_cards
                    .iter()
                    .copied()
                    .map(|card| {
                        if card == returned {
                            Ok(CardMoveDelta {
                                card,
                                from: CardZone::Hand(player.clone()),
                                to: if state.uses_personal_decks() {
                                    CardZone::PlayerDeckTop(player.clone())
                                } else {
                                    CardZone::DeckTop
                                },
                            })
                        } else {
                            crate::domain::discard::move_from(
                                state,
                                card,
                                CardZone::Hand(player.clone()),
                            )
                        }
                    })
                    .collect::<GameResult<Vec<_>>>()?,
            }])
        }
        PendingResolution::ConfluenceDiscardInspectedCard { .. } => {
            let target =
                resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)?;
            let card = *selected_cards
                .first()
                .ok_or(GameError::Validation(ValidationError::MissingPendingChoice))?;
            Ok(vec![EffectIntent::MoveCards {
                card_moves: vec![crate::domain::discard::move_from(
                    state,
                    card,
                    CardZone::Hand(target),
                )?],
            }])
        }
        PendingResolution::ConfluenceClearWindKeepCards => {
            let allowed_cards = match &choice.kind {
                crate::domain::PendingChoiceKind::Card { cards, .. } => cards,
                _ => {
                    return Err(GameError::Validation(ValidationError::MissingPendingChoice));
                }
            };
            Ok(vec![EffectIntent::MoveCards {
                card_moves: allowed_cards
                    .iter()
                    .filter(|card| !selected_cards.contains(card))
                    .copied()
                    .map(|card| {
                        crate::domain::discard::move_from(
                            state,
                            card,
                            CardZone::Hand(player.clone()),
                        )
                    })
                    .collect::<GameResult<Vec<_>>>()?,
            }])
        }
        _ => Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(
                "不支援的待處理規則流程".to_string(),
            ),
        )),
    }
}

fn resolve_rule_player_target(
    state: &GameState,
    player: &PlayerId,
    target: RulePlayerTarget,
) -> GameResult<PlayerId> {
    TurnOrderTargets::new(state).player_target(player, target)
}

fn resolve_rule_team_target(
    state: &GameState,
    player: &PlayerId,
    target: RuleTeamTarget,
) -> GameResult<TeamId> {
    TurnOrderTargets::new(state).team_target(player, target)
}

fn player_team(state: &GameState, player: &PlayerId) -> GameResult<TeamId> {
    TurnOrderTargets::new(state).team_of(player)
}
