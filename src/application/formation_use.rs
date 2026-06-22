use crate::domain::{
    ActionModification, AttackPointBreakdown, CardInstanceId, CardMoveDelta, CardZone,
    DamageTransform, Element, ElementInteraction, EngineInvariantError, GameError, GameEvent,
    GameResult, GameState, HpChangeDelta, LastElementalAttack, LastElementalAttackUpdate,
    PassiveFlipOutcome, PassiveNoEffectReason, PlayerId, ShieldChangeDelta, TargetDecl, TeamId,
    ValidationError,
    targeting::{RulePlayerTarget, RuleTeamTarget, TurnOrderTargets},
};
use crate::rules::{
    AttackCategory, AttackPlanDef, DamageTarget, EffectPlan, PointFormula, SubmittedCardFacts,
    base_formation_matcher, base_formation_registry,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FormationUseRequest {
    pub(super) player: PlayerId,
    pub(super) formation_id: String,
    pub(super) cards: Vec<CardInstanceId>,
    pub(super) declared_targets: Vec<TargetDecl>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FormationUsePlan {
    player: PlayerId,
    formation_id: String,
    cards: Vec<CardInstanceId>,
    declared_targets: Vec<TargetDecl>,
    effect_plan: EffectPlan,
}

pub(super) fn resolve(
    state: &GameState,
    request: FormationUseRequest,
) -> GameResult<Vec<GameEvent>> {
    let plan = BaseFormationPlanner::new().plan_use(state, request)?;
    BaseEffectResolver::new().resolve(state, plan)
}

pub(super) fn answer_effect_choice(
    state: &GameState,
    player: &PlayerId,
    effect_id: &str,
    continuation_id: &str,
    selected_cards: &[CardInstanceId],
) -> GameResult<Vec<GameEvent>> {
    let intents =
        resume_effect_choice_intents(state, player, effect_id, continuation_id, selected_cards)?;
    effect_intent_events(state, intents)
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
        let registry = base_formation_registry();
        let formation = registry.formation(&request.formation_id).ok_or_else(|| {
            GameError::Validation(ValidationError::UnknownFormation(
                request.formation_id.clone(),
            ))
        })?;

        let hand = state.hand(&request.player).ok_or_else(|| {
            GameError::Validation(ValidationError::UnknownPlayer(request.player.clone()))
        })?;
        let mut seen = HashSet::new();
        let mut submitted_cards = Vec::new();

        for card in &request.cards {
            if !seen.insert(*card) {
                return Err(GameError::Validation(
                    ValidationError::DuplicateSubmittedCard(*card),
                ));
            }

            if !hand.contains(card) {
                return Err(GameError::Validation(ValidationError::CardNotInHand(*card)));
            }

            let card_def = state.card_def(*card).ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))?;
            submitted_cards.push(SubmittedCardFacts {
                element: card_def.element,
                level: card_def.level,
            });
        }

        if !base_formation_matcher().matches(&formation.pattern, &submitted_cards) {
            return Err(GameError::Validation(
                ValidationError::FormationPatternMismatch {
                    formation_id: request.formation_id,
                },
            ));
        }

        let effect = registry
            .effect_for(formation)
            .expect("base formation registry must link every formation to an effect");

        Ok(FormationUsePlan {
            player: request.player,
            formation_id: request.formation_id,
            cards: request.cards,
            declared_targets: request.declared_targets,
            effect_plan: effect.plan.clone(),
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
                if !plan.declared_targets.is_empty() {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }
                let passive_resolutions =
                    passive_resolutions(state, &plan.player, IncomingActionKind::Attack);
                let action_modifications = action_modifications(&passive_resolutions);
                let mut events = passive_events(passive_resolutions);
                let target = attack_target(state, &plan.player, attack_plan)?;
                let target_team = player_team(state, &target)?;
                let points =
                    compute_attack_points(state, &attack_plan.point_formula, &plan.cards, &target)?;
                let has_target_shield = state.shield(&target).is_some_and(|value| value > 0);
                let point_breakdown = attack_point_breakdown(
                    state,
                    &attack_plan.category,
                    &target,
                    points,
                    has_target_shield,
                );
                let final_amount = point_breakdown.final_amount;
                let damage_prevented =
                    action_modifications.contains(&ActionModification::PreventDamage);
                let shield_change = if damage_prevented {
                    None
                } else {
                    shield_absorption(state, &target, final_amount)
                };
                let has_shield_change = shield_change.is_some();
                let split_attack_damage = action_modifications
                    .contains(&ActionModification::SplitAttackDamage)
                    && !matches!(
                        point_breakdown.damage_transform,
                        DamageTransform::HealTarget
                    );
                let hp_change = if damage_prevented || has_shield_change {
                    no_hp_change(state, &target_team)?
                } else if split_attack_damage {
                    apply_attack_amount(
                        state,
                        &target_team,
                        (final_amount + 1) / 2,
                        DamageTransform::NormalDamage,
                    )?
                } else {
                    apply_attack_amount(
                        state,
                        &target_team,
                        final_amount,
                        point_breakdown.damage_transform,
                    )?
                };
                let card_moves = plan
                    .cards
                    .iter()
                    .copied()
                    .map(|card| CardMoveDelta {
                        card,
                        from: CardZone::Hand(plan.player.clone()),
                        to: CardZone::Discard,
                    })
                    .collect::<Vec<_>>();
                let elemental_context_update =
                    elemental_context_update(&attack_plan.category, state.turn_number).map(
                        |attack| LastElementalAttackUpdate {
                            player: plan.player.clone(),
                            attack,
                        },
                    );

                events.push(GameEvent::AttackResolved {
                    attacker: plan.player.clone(),
                    target,
                    formation_id: plan.formation_id.clone(),
                    used_cards: plan.cards.clone(),
                    point_breakdown,
                    hp_change,
                    shield_change,
                    card_moves,
                    elemental_context_update,
                });

                if split_attack_damage && !damage_prevented && !has_shield_change {
                    let attacker_team = player_team(state, &plan.player)?;
                    let attacker_damage = final_amount / 2;
                    if attacker_damage > 0 {
                        events.push(GameEvent::HpChanged {
                            change: apply_attack_amount(
                                state,
                                &attacker_team,
                                attacker_damage,
                                DamageTransform::NormalDamage,
                            )?,
                        });
                    }
                }

                if plan.formation_id == "five-streams-unite" {
                    let old_value = state
                        .turn_draw_bonus_by_player
                        .get(&plan.player)
                        .copied()
                        .unwrap_or(0);
                    events.push(GameEvent::TurnDrawBonusChanged {
                        player: plan.player,
                        old_value,
                        delta: 1,
                        new_value: old_value + 1,
                    });
                }

                Ok(events)
            }
            EffectPlan::PassiveSpell(_) => {
                if !plan.declared_targets.is_empty() {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }

                if state
                    .covered_passives
                    .iter()
                    .any(|passive| passive.owner == plan.player)
                {
                    return Err(GameError::Validation(
                        ValidationError::PendingPassiveAlreadyCovered {
                            player: plan.player,
                        },
                    ));
                }

                let passive_resolutions =
                    passive_resolutions(state, &plan.player, IncomingActionKind::PassiveSpell);
                let action_modifications = action_modifications(&passive_resolutions);
                let mut events = passive_events(passive_resolutions);
                events.push(GameEvent::PassiveCovered {
                    player: plan.player,
                    formation_id: plan.formation_id,
                    cards: plan.cards,
                    sealed: action_modifications.contains(&ActionModification::SealCoveredPassive),
                });
                Ok(events)
            }
            EffectPlan::ActiveSpell(spell) => {
                if !plan.declared_targets.is_empty() {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }

                let passive_resolutions =
                    passive_resolutions(state, &plan.player, IncomingActionKind::ActiveSpell);
                let action_modifications = action_modifications(&passive_resolutions);
                let mut events = passive_events(passive_resolutions);
                events.push(GameEvent::FormationPerformed {
                    player: plan.player.clone(),
                    formation_id: plan.formation_id,
                    used_cards: plan.cards.clone(),
                    declared_targets: plan.declared_targets,
                });
                if !action_modifications.contains(&ActionModification::CancelSpell) {
                    let intents =
                        active_spell_intents(state, &plan.player, &spell.resolver_id, &plan.cards)?;
                    events.extend(effect_intent_events(state, intents)?);
                }
                Ok(events)
            }
        }
    }
}

#[derive(Clone, Copy)]
enum IncomingActionKind {
    Attack,
    ActiveSpell,
    PassiveSpell,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PassiveResolution {
    event: GameEvent,
    modifications: Vec<ActionModification>,
}

fn passive_resolutions(
    state: &GameState,
    incoming_player: &PlayerId,
    incoming_kind: IncomingActionKind,
) -> Vec<PassiveResolution> {
    let Some(previous_player) = previous_player(state, incoming_player).ok() else {
        return Vec::new();
    };

    state
        .covered_passives
        .iter()
        .filter(|passive| passive.owner == previous_player)
        .map(|passive| {
            let modifications =
                passive_spell_intents(&passive.formation_id, incoming_kind, passive.sealed)
                    .into_iter()
                    .filter_map(|intent| match intent {
                        EffectIntent::ModifyAction { modification } => Some(modification),
                        _ => None,
                    })
                    .collect::<Vec<_>>();

            PassiveResolution {
                event: GameEvent::PassiveFlipped {
                    owner: passive.owner.clone(),
                    incoming_player: incoming_player.clone(),
                    passive_id: passive.formation_id.clone(),
                    cards: passive.cards.clone(),
                    outcome: passive_outcome(
                        &passive.formation_id,
                        incoming_kind,
                        passive.sealed,
                        &modifications,
                    ),
                },
                modifications,
            }
        })
        .collect()
}

fn passive_events(resolutions: Vec<PassiveResolution>) -> Vec<GameEvent> {
    resolutions
        .into_iter()
        .map(|resolution| resolution.event)
        .collect()
}

fn action_modifications(resolutions: &[PassiveResolution]) -> Vec<ActionModification> {
    resolutions
        .iter()
        .flat_map(|resolution| resolution.modifications.iter().cloned())
        .collect()
}

fn passive_outcome(
    passive_id: &str,
    incoming_kind: IncomingActionKind,
    sealed: bool,
    modifications: &[ActionModification],
) -> PassiveFlipOutcome {
    if sealed {
        return PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::Sealed,
        };
    }

    if !modifications.is_empty() {
        return PassiveFlipOutcome::Applied {
            effect_id: passive_id.to_string(),
            modifications: modifications.to_vec(),
        };
    }

    match (passive_id, incoming_kind) {
        ("defense", IncomingActionKind::ActiveSpell | IncomingActionKind::PassiveSpell) => {
            PassiveFlipOutcome::NoEffect {
                reason: PassiveNoEffectReason::NotAnAttack,
            }
        }
        ("countershock", IncomingActionKind::ActiveSpell | IncomingActionKind::PassiveSpell) => {
            PassiveFlipOutcome::NoEffect {
                reason: PassiveNoEffectReason::NotAnAttack,
            }
        }
        ("seal", IncomingActionKind::Attack) => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotASpell,
        },
        _ => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotASpell,
        },
    }
}

fn passive_spell_intents(
    passive_id: &str,
    incoming_kind: IncomingActionKind,
    sealed: bool,
) -> Vec<EffectIntent> {
    if sealed {
        return Vec::new();
    }

    let modification = match (passive_id, incoming_kind) {
        ("defense", IncomingActionKind::Attack) => ActionModification::PreventDamage,
        ("countershock", IncomingActionKind::Attack) => ActionModification::SplitAttackDamage,
        ("seal", IncomingActionKind::ActiveSpell) => ActionModification::CancelSpell,
        ("seal", IncomingActionKind::PassiveSpell) => ActionModification::SealCoveredPassive,
        _ => return Vec::new(),
    };

    vec![EffectIntent::ModifyAction { modification }]
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EffectIntent {
    SetShield {
        player: PlayerId,
        value: i32,
    },
    ChangeHp {
        team: TeamId,
        delta: i32,
    },
    MoveCards {
        card_moves: Vec<CardMoveDelta>,
    },
    AddStatus {
        status: crate::domain::StatusEffect,
    },
    ResolveCopiedAttack {
        formation_id: String,
        category: AttackCategory,
        point_formula: PointFormula,
        used_cards: Vec<CardInstanceId>,
    },
    ModifyAction {
        modification: ActionModification,
    },
    RequestChoice {
        player: PlayerId,
        kind: crate::domain::PendingChoiceKind,
    },
}

fn active_spell_intents(
    state: &GameState,
    player: &PlayerId,
    resolver_id: &str,
    used_cards: &[CardInstanceId],
) -> GameResult<Vec<EffectIntent>> {
    match resolver_id {
        "barrier" => Ok(vec![EffectIntent::SetShield {
            player: player.clone(),
            value: level_sum(state, used_cards)? * 4,
        }]),
        "metamorphosis" => metamorphosis_intents(state, player, used_cards),
        "generating-formation" => {
            let team = resolve_rule_team_target(state, player, RuleTeamTarget::OwnSide)?;
            Ok(vec![EffectIntent::ChangeHp {
                team,
                delta: level_sum(state, used_cards)? * 3,
            }])
        }
        "overcoming-formation" => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let old_value = state.shield(&target).unwrap_or(0);
            let new_value = (old_value - level_sum(state, used_cards)? * 3).max(0);
            Ok(vec![EffectIntent::SetShield {
                player: target,
                value: new_value,
            }])
        }
        "radiance" => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let expires_at = nth_future_turn_for_player(state, &target, 2)?;
            Ok(vec![
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
            Ok(vec![EffectIntent::RequestChoice {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::EffectGenerated {
                    effect_id: resolver_id.to_string(),
                    continuation_id: "chaos:return-two".to_string(),
                    allowed_cards,
                },
            }])
        }
        "return-to-origin" => {
            let team = player_team(state, player)?;
            Ok(vec![EffectIntent::ChangeHp {
                team,
                delta: level_sum(state, used_cards)? * 4,
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
) -> GameResult<Vec<EffectIntent>> {
    let previous_player =
        resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)?;
    let Some(last_formation) = state.last_formation_by_player.get(&previous_player) else {
        return Ok(Vec::new());
    };

    let registry = base_formation_registry();
    let Some(formation) = registry.formation(&last_formation.formation_id) else {
        return Ok(Vec::new());
    };
    let effect = registry
        .effect_for(formation)
        .expect("base formation registry must link every formation to an effect");

    match &effect.plan {
        EffectPlan::Attack(plan) => Ok(vec![EffectIntent::ResolveCopiedAttack {
            formation_id: formation.id.clone(),
            category: plan.category.clone(),
            point_formula: plan.point_formula.clone(),
            used_cards: used_cards.to_vec(),
        }]),
        EffectPlan::ActiveSpell(spell) if spell.resolver_id != "metamorphosis" => {
            active_spell_intents(state, player, &spell.resolver_id, used_cards)
        }
        EffectPlan::ActiveSpell(_) | EffectPlan::PassiveSpell(_) => Ok(Vec::new()),
    }
}

fn level_sum(state: &GameState, cards: &[CardInstanceId]) -> GameResult<i32> {
    cards.iter().try_fold(0, |sum, card| {
        let level = state
            .card_def(*card)
            .ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))?
            .level as i32;
        Ok(sum + level)
    })
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

fn effect_intent_events(
    state: &GameState,
    intents: Vec<EffectIntent>,
) -> GameResult<Vec<GameEvent>> {
    let mut requested_choice_player = None;
    let mut events = Vec::new();

    for intent in intents {
        let event = match intent {
            EffectIntent::SetShield { player, value } => {
                let old_value = state.shield(&player).unwrap_or(0);
                if old_value == value {
                    continue;
                }
                GameEvent::ShieldChanged {
                    player,
                    old_value,
                    delta: value - old_value,
                    new_value: value,
                }
            }
            EffectIntent::ChangeHp { team, delta } => {
                if delta == 0 {
                    continue;
                }
                let old_hp = state
                    .hp
                    .iter()
                    .find(|team_hp| team_hp.team == team)
                    .ok_or_else(|| {
                        GameError::Validation(ValidationError::MissingTeamHp(team.clone()))
                    })?
                    .hp;
                GameEvent::HpChanged {
                    change: HpChangeDelta {
                        team,
                        old_hp,
                        delta,
                        new_hp: old_hp + delta,
                        effective_delta: delta,
                    },
                }
            }
            EffectIntent::MoveCards { card_moves } => GameEvent::CardsMoved { card_moves },
            EffectIntent::AddStatus { status } => GameEvent::StatusAdded { status },
            EffectIntent::ResolveCopiedAttack {
                formation_id,
                category,
                point_formula,
                used_cards,
            } => {
                let target = attack_target(
                    state,
                    &state
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                        .clone(),
                    &AttackPlanDef {
                        category: category.clone(),
                        point_formula: point_formula.clone(),
                        damage_target: DamageTarget::PreviousPlayer,
                    },
                )?;
                let target_team = player_team(state, &target)?;
                let player = state
                    .current_player()
                    .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                    .clone();
                let points = compute_attack_points(state, &point_formula, &used_cards, &target)?;
                let has_target_shield = state.shield(&target).is_some_and(|value| value > 0);
                let point_breakdown =
                    attack_point_breakdown(state, &category, &target, points, has_target_shield);
                let shield_change = shield_absorption(state, &target, point_breakdown.final_amount);
                let hp_change = if shield_change.is_some() {
                    no_hp_change(state, &target_team)?
                } else {
                    apply_attack_amount(
                        state,
                        &target_team,
                        point_breakdown.final_amount,
                        point_breakdown.damage_transform,
                    )?
                };
                GameEvent::AttackResolved {
                    attacker: player.clone(),
                    target,
                    formation_id,
                    used_cards,
                    point_breakdown,
                    hp_change,
                    shield_change,
                    card_moves: Vec::new(),
                    elemental_context_update: elemental_context_update(
                        &category,
                        state.turn_number,
                    )
                    .map(|attack| LastElementalAttackUpdate { player, attack }),
                }
            }
            EffectIntent::ModifyAction { modification } => match modification {
                ActionModification::PreventDamage
                | ActionModification::SplitAttackDamage
                | ActionModification::CancelSpell
                | ActionModification::SealCoveredPassive => continue,
            },
            EffectIntent::RequestChoice { player, kind } => {
                if let Some(existing_player) = requested_choice_player {
                    return Err(GameError::EngineInvariant(
                        EngineInvariantError::DuplicatePendingChoice {
                            player: existing_player,
                        },
                    ));
                }

                requested_choice_player = Some(player.clone());
                GameEvent::EffectChoiceRequested { player, kind }
            }
        };

        events.push(event);
    }

    Ok(events)
}

fn resume_effect_choice_intents(
    state: &GameState,
    player: &PlayerId,
    effect_id: &str,
    continuation_id: &str,
    selected_cards: &[CardInstanceId],
) -> GameResult<Vec<EffectIntent>> {
    match (effect_id, continuation_id) {
        ("metamorphosis", "metamorphosis:choose-card") => {
            let selected_card = selected_cards
                .first()
                .ok_or(GameError::Validation(ValidationError::MissingPendingChoice))?;
            let value = state
                .card_def(*selected_card)
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(*selected_card),
                ))?
                .level as i32;

            Ok(vec![EffectIntent::SetShield {
                player: player.clone(),
                value,
            }])
        }
        ("chaos", "chaos:return-two") => {
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
                        to: CardZone::DeckTop,
                    })
                    .collect(),
            }])
        }
        _ => Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(
                continuation_id.to_string(),
            ),
        )),
    }
}

fn attack_target(
    state: &GameState,
    attacker: &PlayerId,
    plan: &AttackPlanDef,
) -> GameResult<PlayerId> {
    match plan.damage_target {
        DamageTarget::PreviousPlayer => {
            resolve_rule_player_target(state, attacker, RulePlayerTarget::PreviousPlayer)
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

fn previous_player(state: &GameState, player: &PlayerId) -> GameResult<PlayerId> {
    resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)
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
        PointFormula::LevelPlus(bonus) => Ok(state
            .card_def(cards[0])
            .ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(cards[0]),
            ))?
            .level as i32
            + *bonus as i32),
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
    let Some(previous_attack) = state.last_elemental_attack_by_player.get(target) else {
        return AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        };
    };

    if current_element == previous_attack.element {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::Same,
            damage_transform: DamageTransform::HalfDamageRoundUp,
            final_amount: (base_points + 1) / 2,
        }
    } else if generates(current_element, previous_attack.element) {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::Generating,
            damage_transform: DamageTransform::HealTarget,
            final_amount: base_points,
        }
    } else if overcomes(current_element, previous_attack.element) {
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
    let new_hp = (old_hp + delta).max(0);

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
    use crate::domain::{CardDef, CardDefId, CardInstanceDef};

    #[test]
    fn formation_points_sum_submitted_card_levels() {
        let setup =
            crate::domain::GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30)
                .with_cards(
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
