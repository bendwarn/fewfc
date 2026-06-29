use crate::domain::{
    CardInstanceId, CardMoveDelta, EngineInvariantError, GameError, GameEvent, GameResult,
    GameState, PlayerId, TeamId, ValidationError,
};
use crate::rules::{AttackCategory, PointFormula};

use super::attack_resolution::{self, AttackRequest, AttackResolutionMode};

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
    EstablishCounterEffect {
        owner: PlayerId,
        effect_id: String,
    },
    InspectHand {
        viewer: PlayerId,
        target: PlayerId,
        cards: Vec<CardInstanceId>,
    },
    ResolveCopiedAttack {
        formation_id: String,
        category: AttackCategory,
        point_formula: PointFormula,
        used_cards: Vec<CardInstanceId>,
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
                let initial_hp = state.initial_hp(&team).ok_or_else(|| {
                    GameError::Validation(ValidationError::MissingTeamHp(team.clone()))
                })?;
                let new_hp = (old_hp + delta).clamp(0, initial_hp);
                GameEvent::HpChanged {
                    change: crate::domain::HpChangeDelta {
                        team,
                        old_hp,
                        delta,
                        new_hp,
                        effective_delta: new_hp - old_hp,
                    },
                }
            }
            EffectIntent::MoveCards { card_moves } => GameEvent::CardsMoved { card_moves },
            EffectIntent::AddStatus { status } => GameEvent::StatusAdded { status },
            EffectIntent::EstablishCounterEffect { owner, effect_id } => {
                GameEvent::CounterEffectEstablished { owner, effect_id }
            }
            EffectIntent::InspectHand {
                viewer,
                target,
                cards,
            } => GameEvent::HandInspected {
                viewer,
                target,
                cards,
            },
            EffectIntent::ResolveCopiedAttack {
                formation_id,
                category,
                point_formula,
                used_cards,
            } => {
                events.extend(attack_resolution::resolve(
                    state,
                    AttackRequest {
                        attacker: state
                            .current_player()
                            .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                            .clone(),
                        formation_id,
                        category,
                        point_formula,
                        used_cards,
                        damage_prevented: false,
                        split_attack_damage: false,
                        mode: AttackResolutionMode::CopiedEffect,
                    },
                )?);
                continue;
            }
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
