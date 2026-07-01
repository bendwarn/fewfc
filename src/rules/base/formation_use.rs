use crate::domain::{
    CardInstanceId, CardMoveDelta, CardZone, GameError, GameEvent, GameResult, GameState, PlayerId,
    TargetDecl, TeamId, ValidationError,
    targeting::{RulePlayerTarget, RuleTeamTarget, TurnOrderTargets},
};
use crate::rules::{
    EffectPlan, base_formation_registry, environment_makes_formation_ineffective,
    sacred_beast_element,
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
        let selected = FormationSelection::new(state, &request.player, request.cards)?
            .require(&request.formation_id)?;

        Ok(FormationUsePlan {
            player: request.player,
            formation_id: selected.formation_id,
            cards: selected.cards,
            declared_targets: request.declared_targets,
            effect_plan: selected.effect_plan,
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
                let passive_trigger = covered_passive::trigger(
                    state,
                    TriggerRequest {
                        incoming_player: plan.player.clone(),
                        incoming_kind: IncomingActionKind::Attack,
                        ignores_formation_effects: sacred_beast_element(&plan.formation_id)
                            .is_some(),
                    },
                );
                let environment_ineffective =
                    environment_makes_formation_ineffective(state, &plan.formation_id);
                let damage_prevented = passive_trigger.prevents_damage() || environment_ineffective;
                let split_attack_damage = passive_trigger.splits_attack_damage();
                let mut events = passive_trigger.events();
                if environment_ineffective {
                    events.push(formation_effect_ignored_event(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    ));
                }
                events.extend(attack_resolution::resolve(
                    state,
                    AttackRequest {
                        attacker: plan.player,
                        formation_id: plan.formation_id,
                        category: attack_plan.category.clone(),
                        point_formula: attack_plan.point_formula.clone(),
                        used_cards: plan.cards,
                        damage_prevented,
                        split_attack_damage,
                        mode: AttackResolutionMode::FormationUse,
                    },
                )?);

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

                let passive_trigger = covered_passive::trigger(
                    state,
                    TriggerRequest {
                        incoming_player: plan.player.clone(),
                        incoming_kind: IncomingActionKind::PassiveSpell,
                        ignores_formation_effects: false,
                    },
                );
                let sealed = passive_trigger.seals_covered_passive();
                let mut events = passive_trigger.events();
                events.push(GameEvent::PassiveCovered {
                    player: plan.player,
                    formation_id: plan.formation_id,
                    cards: plan.cards,
                    sealed,
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

                let passive_trigger = covered_passive::trigger(
                    state,
                    TriggerRequest {
                        incoming_player: plan.player.clone(),
                        incoming_kind: IncomingActionKind::ActiveSpell,
                        ignores_formation_effects: false,
                    },
                );
                let spell_cancelled = passive_trigger.cancels_spell();
                let spell_ineffective =
                    environment_makes_formation_ineffective(state, &plan.formation_id);
                let mut events = passive_trigger.events();
                events.push(GameEvent::FormationPerformed {
                    player: plan.player.clone(),
                    formation_id: plan.formation_id.clone(),
                    used_cards: plan.cards.clone(),
                    declared_targets: plan.declared_targets,
                });
                if !spell_cancelled && spell_ineffective {
                    events.push(formation_effect_ignored_event(
                        state,
                        &plan.player,
                        &plan.formation_id,
                    ));
                }
                if !spell_cancelled && !spell_ineffective {
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
        "metamorphosis" => Ok(metamorphosis_intents(state, player, used_cards)?.1),
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
) -> GameResult<(Option<String>, Vec<EffectIntent>)> {
    let previous_player =
        resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)?;
    let Some(last_formation) = state.last_formation_by_player.get(&previous_player) else {
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
            active_spell_intents(state, player, &spell.resolver_id, used_cards)?,
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
                        to: if state.uses_personal_decks() {
                            CardZone::PlayerDeckTop(player.clone())
                        } else {
                            CardZone::DeckTop
                        },
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
