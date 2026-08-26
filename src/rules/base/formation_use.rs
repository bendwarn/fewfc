use crate::domain::{
    CannotPerformFormationReason, CardInstanceId, CardMoveDelta, CardZone, GameError, GameEvent,
    GameResult, GameState, PendingResolution, PlayerId, TargetDecl, TeamId, ValidationError,
    targeting::{RulePlayerTarget, RuleTeamTarget, TurnOrderTargets},
};
use crate::rules::{
    EffectPlan, PointFormula, base_formation_registry, environment_makes_formation_ineffective,
    formation_resolved_on_previous_turn, sacred_beast_element,
};

use super::attack_resolution::{self, AttackRequest, AttackResolutionMode};
use super::covered_passive::{self, IncomingActionKind, TriggerRequest};
use super::effect_intent::{EffectIntent, effect_intent_events};
use super::formation_selection::FormationSelection;

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
    let is_passive = matches!(plan.effect_plan, EffectPlan::PassiveSpell(_));
    let is_active_spell = matches!(&plan.effect_plan, EffectPlan::ActiveSpell(_));
    let mut events = BaseEffectResolver::new().resolve(state, plan)?;
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
    crate::rules::tribulation::suppress_formation_recovery(state, &player, &mut events);
    crate::rules::pouch::suppress_watch_fire_formation_hp_changes(state, &player, &mut events);
    crate::rules::dark::append_shared_fate_events(state, &player, &formation_id, &mut events)?;
    crate::rules::dark::append_mischief_events(state, &mut events)?;
    if is_active_spell {
        append_post_formation_events(state, &player, &formation_id, &mut events)?;
    }
    attack_resolution::absorb_simultaneous_events(&mut events);
    super::append_terminal_game_end(state, &mut events);
    let waits_for_choice = events.iter().any(|event| {
        matches!(
            event,
            GameEvent::ChoiceRequested { .. } | GameEvent::RandomnessRequested { .. }
        )
    });
    let has_terminal_event = events
        .iter()
        .any(|event| matches!(event, GameEvent::GameEnded { .. }));
    if !is_passive && !waits_for_choice && !has_terminal_event {
        events.push(GameEvent::FormationCardsDiscarded {
            player: player.clone(),
            formation_id: formation_id.clone(),
            cards: composition.physical_cards,
        });
    }
    Ok(events)
}

pub(super) fn append_completed_active_spell_post_formation_events(
    state: &GameState,
    events: &mut Vec<GameEvent>,
) -> GameResult<()> {
    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }

    if projected.pending_choice.is_some()
        || projected.pending_randomness.is_some()
        || !matches!(projected.status, crate::domain::GameStatus::InProgress)
        || projected.phase != crate::domain::Phase::Action
    {
        return Ok(());
    }
    let Some(player) = projected.current_player().cloned() else {
        return Ok(());
    };
    let Some(formation) = projected
        .formation_area(&player)
        .and_then(|area| area.formation.as_ref())
    else {
        return Ok(());
    };
    if !matches!(
        formation.state,
        crate::domain::FormationAreaState::FaceUpResolving
    ) {
        return Ok(());
    }

    let registry = base_formation_registry();
    let is_active_spell = registry
        .formation(&formation.formation_id)
        .and_then(|definition| registry.effect_for(definition))
        .is_some_and(|effect| matches!(effect.plan, EffectPlan::ActiveSpell(_)));
    if is_active_spell {
        append_post_formation_events(state, &player, &formation.formation_id, events)?;
    }
    Ok(())
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
) -> GameResult<Vec<GameEvent>> {
    let intents = resume_choice_intents(state, choice, player, resolution, selected_cards)?;
    let mut events = effect_intent_events(state, intents)?;
    events.extend(crate::rules::confluence::after_choice_events(
        state, player, resolution,
    )?);
    Ok(events)
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

    fn resolve(&self, state: &GameState, plan: FormationUsePlan) -> GameResult<Vec<GameEvent>> {
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
                if environment_ineffective || formation_suppressed {
                    events.push(formation_effect_ignored_event(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    ));
                }
                if plan.formation_id == crate::rules::tribulation::EARTH_RENDING
                    && !environment_ineffective
                    && !formation_suppressed
                {
                    events.extend(crate::rules::tribulation::earth_rending_start_events(
                        state,
                        &plan.player,
                        &plan.cards,
                        damage_prevented,
                        split_attack_damage,
                    )?);
                    return Ok(events);
                }
                if plan.formation_id == crate::rules::tribulation::RUSTED_FOREST
                    && !environment_ineffective
                    && !formation_suppressed
                {
                    events.extend(crate::rules::tribulation::rusted_forest_start_events(
                        state,
                        &plan.player,
                        &plan.cards,
                        damage_prevented,
                        split_attack_damage,
                    )?);
                    return Ok(events);
                }
                let tribulation_pre_events = if environment_ineffective || formation_suppressed {
                    Vec::new()
                } else {
                    crate::rules::tribulation::pre_attack_events(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    )
                };
                let point_formula = crate::rules::tribulation::attack_points(
                    &plan.formation_id,
                    &tribulation_pre_events,
                )
                .map(PointFormula::Fixed)
                .unwrap_or_else(|| attack_plan.point_formula.clone());
                let mut projected = state.clone();
                for event in &events {
                    crate::rules::projection::apply_event(&mut projected, event);
                }
                // Tribulation 的攻擊前差異會參與同一個標準 AttackResolved 事件，
                // 但攻擊計算仍必須看到它們解析後的狀態（特別是 Mudslide 護盾）。
                let mut attack_state = projected.clone();
                for event in &tribulation_pre_events {
                    crate::rules::projection::apply_event(&mut attack_state, event);
                }
                events.extend(attack_resolution::resolve(
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
                        pre_resolution_effects: attack_resolution::effects_from_events(
                            &tribulation_pre_events,
                        )?,
                    },
                )?);
                if !environment_ineffective && !formation_suppressed {
                    events.extend(crate::rules::tribulation::post_attack_events(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    )?);
                } else {
                    events.extend(
                        crate::rules::tribulation::divine_calculation_consumption_events(
                            state,
                            &plan.formation_id,
                        ),
                    );
                }

                attack_resolution::absorb_simultaneous_events(&mut events);

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
                        events.extend(crate::rules::confluence::void_transcendence_events(
                            state,
                            &plan.player,
                            &plan.cards,
                        )?);
                        let mut projected = state.clone();
                        for event in &events {
                            crate::rules::projection::apply_event(&mut projected, event);
                        }
                        events.push(void_reversion_event(&projected, &plan.player, &plan.cards)?);
                        events.extend(crate::rules::confluence::void_realm_consumption_events(
                            &projected,
                            &plan.player,
                            &plan.cards,
                        ));
                        return Ok(events);
                    }
                    if spell.resolver_id == "void-spirit-shattering" {
                        events.push(crate::rules::spirit::void_spirit_shattering_event(
                            state,
                            &plan.player,
                            &plan.cards,
                        )?);
                        return Ok(events);
                    }
                    if spell.resolver_id == "void-meridian-severing" {
                        if let Some(event) = environment_clearing_event(state, &plan.player)? {
                            events.push(event);
                        }
                        return Ok(events);
                    }
                    if spell.resolver_id == "void-star-breaking" {
                        events.extend(void_star_breaking_events(state, &plan.player));
                        return Ok(events);
                    }
                    if let Some(mut echo_events) = {
                        let mut projected = state.clone();
                        for event in &events {
                            crate::rules::projection::apply_event(&mut projected, event);
                        }
                        crate::rules::echo::formation_main_effect_events(
                            state,
                            &projected,
                            &plan.player,
                            &spell.resolver_id,
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
                    if let Some(mut jianghu_events) = crate::rules::jianghu::active_spell_events(
                        state,
                        &plan.player,
                        &spell.resolver_id,
                    )? {
                        events.append(&mut jianghu_events);
                        return Ok(events);
                    }
                    if let Some(mut confluence_events) = {
                        if spell.resolver_id == crate::rules::confluence::VOID_RETURN_TO_NOTHING {
                            events.extend(crate::rules::confluence::void_transcendence_events(
                                state,
                                &plan.player,
                                &plan.cards,
                            )?);
                        }
                        let mut projected = state.clone();
                        for event in &events {
                            crate::rules::projection::apply_event(&mut projected, event);
                        }
                        crate::rules::confluence::active_spell_events(
                            &projected,
                            &plan.player,
                            &spell.resolver_id,
                            &plan.declared_targets,
                        )?
                    } {
                        events.append(&mut confluence_events);
                        return Ok(events);
                    }
                    if let Some(mut dark_events) = crate::rules::dark::active_spell_events(
                        state,
                        &plan.player,
                        &spell.resolver_id,
                        &plan.cards,
                        plan.trusted_random_cards.as_deref(),
                    )? {
                        events.append(&mut dark_events);
                        return Ok(events);
                    }
                    if let Some(spirit) =
                        crate::rules::spirit::summoning_formation_spirit(&spell.resolver_id)
                    {
                        events.push(GameEvent::SpiritSummoned {
                            player: plan.player.clone(),
                            previous: state.spirit_for(&plan.player).map(|owned| owned.spirit),
                            spirit,
                        });
                        return Ok(events);
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
                    events.extend(effect_intent_events(state, intents)?);
                }
                Ok(events)
            }
        }
    }
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

fn void_star_breaking_events(state: &GameState, player: &PlayerId) -> Vec<GameEvent> {
    let mut events = state
        .team_stars
        .iter()
        .map(|owned| {
            let old_hp = state
                .hp
                .iter()
                .find(|team_hp| team_hp.team == owned.team)
                .expect("an owned Star must belong to a Team with HP")
                .hp;
            let new_hp = (old_hp - 20).max(0);
            GameEvent::StarBroken {
                team: owned.team.clone(),
                star: owned.star,
                reason: crate::domain::StarBreakReason::VoidStarBreaking,
                hp_change: Some(crate::domain::HpChangeDelta {
                    team: owned.team.clone(),
                    old_hp,
                    delta: -20,
                    new_hp,
                    effective_delta: new_hp - old_hp,
                }),
            }
        })
        .collect::<Vec<_>>();

    if !events.is_empty() {
        events.push(GameEvent::VoidStarBreakingCompleted {
            player: player.clone(),
        });
    }
    events
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
) -> GameResult<Option<GameEvent>> {
    let Some(environment) = state.environment else {
        return Ok(None);
    };
    let hp_changes = state
        .hp
        .iter()
        .map(|team_hp| {
            let new_hp = (team_hp.hp - 20).max(0);
            crate::domain::HpChangeDelta {
                team: team_hp.team.clone(),
                old_hp: team_hp.hp,
                delta: -20,
                new_hp,
                effective_delta: new_hp - team_hp.hp,
            }
        })
        .collect();

    Ok(Some(GameEvent::EnvironmentCleared {
        player: player.clone(),
        formation_id: "void-meridian-severing".to_string(),
        environment,
        hp_changes,
    }))
}

fn void_reversion_event(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<GameEvent> {
    let team = player_team(state, player)?;
    let old_hp = team_hp(state, &team)?;
    let new_hp = (old_hp - 20).max(0);
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
        .map(|card| CardMoveDelta {
            card,
            from: CardZone::Hand(player.clone()),
            to: super::discard_zone_for_card(state, card),
        })
        .collect();

    Ok(GameEvent::VoidReversionResolved {
        player: player.clone(),
        hp_change: crate::domain::HpChangeDelta {
            team,
            old_hp,
            delta: -20,
            new_hp,
            effective_delta: new_hp - old_hp,
        },
        card_moves,
        broken_professions,
        retained_legendary_professions,
    })
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
                card_moves: vec![CardMoveDelta {
                    card,
                    from: if state.uses_personal_decks() {
                        CardZone::PlayerDeckTop(player.clone())
                    } else {
                        CardZone::DeckTop
                    },
                    to: super::discard_zone_for_card(state, card),
                }],
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
                    .map(|card| CardMoveDelta {
                        card,
                        from: CardZone::Hand(player.clone()),
                        to: super::discard_zone_for_card(state, card),
                    })
                    .collect(),
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
                    .map(|card| CardMoveDelta {
                        card,
                        from: CardZone::Hand(player.clone()),
                        to: if card == returned {
                            if state.uses_personal_decks() {
                                CardZone::PlayerDeckTop(player.clone())
                            } else {
                                CardZone::DeckTop
                            }
                        } else {
                            super::discard_zone_for_card(state, card)
                        },
                    })
                    .collect(),
            }])
        }
        PendingResolution::ConfluenceDiscardInspectedCard { .. } => {
            let target =
                resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)?;
            let card = *selected_cards
                .first()
                .ok_or(GameError::Validation(ValidationError::MissingPendingChoice))?;
            Ok(vec![EffectIntent::MoveCards {
                card_moves: vec![CardMoveDelta {
                    card,
                    from: CardZone::Hand(target),
                    to: super::discard_zone_for_card(state, card),
                }],
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
                    .map(|card| CardMoveDelta {
                        card,
                        from: CardZone::Hand(player.clone()),
                        to: super::discard_zone_for_card(state, card),
                    })
                    .collect(),
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
