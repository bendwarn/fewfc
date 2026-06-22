use crate::domain::{
    ActionModification, CardInstanceId, CardMoveDelta, EngineInvariantError, GameError, GameEvent,
    GameResult, GameState, LastElementalAttackUpdate, PlayerId, TeamId, ValidationError,
};
use crate::rules::{AttackCategory, AttackPlanDef, DamageTarget, PointFormula};

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::rules::base) enum EffectIntent {
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

pub(in crate::rules::base) fn effect_intent_events(
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
                    change: crate::domain::HpChangeDelta {
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
                let target = super::formation_use::attack_target(
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
                let target_team = super::formation_use::player_team(state, &target)?;
                let player = state
                    .current_player()
                    .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                    .clone();
                let points = super::formation_use::compute_attack_points(
                    state,
                    &point_formula,
                    &used_cards,
                    &target,
                )?;
                let has_target_shield = state.shield(&target).is_some_and(|value| value > 0);
                let point_breakdown = super::formation_use::attack_point_breakdown(
                    state,
                    &category,
                    &target,
                    points,
                    has_target_shield,
                );
                let shield_change = super::formation_use::shield_absorption(
                    state,
                    &target,
                    point_breakdown.final_amount,
                );
                let hp_change = if shield_change.is_some() {
                    super::formation_use::no_hp_change(state, &target_team)?
                } else {
                    super::formation_use::apply_attack_amount(
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
                    elemental_context_update: super::formation_use::elemental_context_update(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GameSetup, PendingChoiceKind};

    fn state() -> GameState {
        GameState::from_setup(&GameSetup::two_player(
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            30,
        ))
    }

    #[test]
    fn set_shield_intent_skips_noop_changes() {
        assert_eq!(
            effect_intent_events(
                &state(),
                vec![EffectIntent::SetShield {
                    player: PlayerId::new("p1"),
                    value: 0,
                }],
            ),
            Ok(Vec::new())
        );
    }

    #[test]
    fn duplicate_choice_intents_are_engine_invariants() {
        let result = effect_intent_events(
            &state(),
            vec![
                EffectIntent::RequestChoice {
                    player: PlayerId::new("p1"),
                    kind: PendingChoiceKind::EffectGenerated {
                        effect_id: "first".to_string(),
                        continuation_id: "first".to_string(),
                        allowed_cards: Vec::new(),
                    },
                },
                EffectIntent::RequestChoice {
                    player: PlayerId::new("p2"),
                    kind: PendingChoiceKind::EffectGenerated {
                        effect_id: "second".to_string(),
                        continuation_id: "second".to_string(),
                        allowed_cards: Vec::new(),
                    },
                },
            ],
        );

        assert_eq!(
            result,
            Err(GameError::EngineInvariant(
                EngineInvariantError::DuplicatePendingChoice {
                    player: PlayerId::new("p1"),
                },
            ))
        );
    }
}
