use crate::domain::{
    CardInstanceId, CardMoveDelta, ChoiceRequest, EngineInvariantError, GameError, GameEvent,
    GameResult, PlayerId, TeamId, ValidationError,
    hp::{HpChangePlan, HpChangeRequest},
};
use crate::rules::{AttackCategory, PointFormula};

use super::attack_resolution::{self, AttackRequest, AttackResolutionMode};
use crate::rules::formation_effect_sequence::{FormationEffectSequence, ResolvedFormationEffect};

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
        request: ChoiceRequest,
    },
}

/// 陣法 outer resolution 可傳入既有 ledger；Echo/一般 continuation 則不帶 performer。
pub(in crate::rules::base) fn effect_intent_events_with_plan(
    intents: Vec<EffectIntent>,
    hp: &mut HpChangePlan,
    formation_performer: Option<&PlayerId>,
    sequence: &mut FormationEffectSequence,
) -> GameResult<()> {
    let mut requested_choice_player = None;

    let mut intents = intents.into_iter().peekable();
    while let Some(intent) = intents.next() {
        match intent {
            EffectIntent::SetShield { player, value } => {
                let old_value = sequence.state().shield(&player).unwrap_or(0);
                if old_value == value {
                    continue;
                }
                sequence.extend([GameEvent::ShieldChanged {
                    player,
                    old_value,
                    delta: value - old_value,
                    new_value: value,
                }]);
            }
            EffectIntent::ChangeHp { team, delta } => {
                let mut changes = vec![(team, delta)];
                while matches!(intents.peek(), Some(EffectIntent::ChangeHp { .. })) {
                    let Some(EffectIntent::ChangeHp { team, delta }) = intents.next() else {
                        unreachable!("peeked ChangeHp must remain available");
                    };
                    changes.push((team, delta));
                }
                if formation_performer.is_some() {
                    let mut effect = hp.begin_formation_effect();
                    let events = changes
                        .into_iter()
                        .map(|(team, delta)| {
                            let request = crate::rules::base::formation_use::formation_hp_request(
                                sequence.state(),
                                formation_performer.expect("formation performer was checked"),
                                &team,
                                delta,
                            );
                            Ok(GameEvent::HpChanged {
                                change: effect.plan(&team, request)?,
                            })
                        })
                        .collect::<GameResult<Vec<_>>>()?;
                    let outcome = effect.finish();
                    sequence.append(hp, ResolvedFormationEffect::new(events, outcome))?;
                } else {
                    for (team, delta) in changes {
                        let request = if delta > 0
                            && crate::rules::jianghu::team_has_poison(sequence.state(), &team)
                        {
                            HpChangeRequest::Prevented(delta)
                        } else {
                            HpChangeRequest::By(delta)
                        };
                        sequence.extend([GameEvent::HpChanged {
                            change: hp.plan(&team, request)?,
                        }]);
                    }
                }
                continue;
            }
            EffectIntent::MoveCards { card_moves } => {
                sequence.extend([GameEvent::CardsMoved { card_moves }]);
            }
            EffectIntent::AddStatus { status } => sequence.extend([GameEvent::StatusAdded {
                status: crate::rules::jianghu::shorten_enemy_status(sequence.state(), status),
            }]),
            EffectIntent::EstablishCounterEffect { owner, effect_id } => {
                sequence.extend([GameEvent::CounterEffectEstablished { owner, effect_id }])
            }
            EffectIntent::InspectHand {
                viewer,
                target,
                cards,
            } => sequence.extend([GameEvent::HandInspected {
                viewer,
                target,
                cards,
            }]),
            EffectIntent::ResolveCopiedAttack {
                formation_id,
                category,
                point_formula,
                used_cards,
            } => {
                sequence.extend(attack_resolution::resolve_with_plan(
                    sequence.state(),
                    AttackRequest {
                        attacker: sequence
                            .state()
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
                        pre_resolution_effects:
                            super::attack_resolution::AttackPreResolutionEffects::default(),
                        trailing_hp_role: crate::domain::HpChangeRole::TriggeredEffect,
                    },
                    hp,
                )?);
                continue;
            }
            EffectIntent::RequestChoice { request } => {
                if let Some(existing_player) = requested_choice_player {
                    return Err(GameError::EngineInvariant(
                        EngineInvariantError::DuplicatePendingChoice {
                            player: existing_player,
                        },
                    ));
                }

                requested_choice_player = Some(request.player.clone());
                sequence.extend([crate::rules::pending_choice::request_event(
                    sequence.state(),
                    request,
                )?]);
            }
        };
    }

    Ok(())
}

#[cfg(test)]
fn effect_intent_events(
    state: &crate::domain::GameState,
    intents: Vec<EffectIntent>,
) -> GameResult<Vec<GameEvent>> {
    let mut hp = HpChangePlan::new(state)?;
    let mut sequence = FormationEffectSequence::new(state);
    effect_intent_events_with_plan(intents, &mut hp, None, &mut sequence)?;
    Ok(sequence.into_events())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ChoiceRequest, GameSetup, GameState, PendingChoiceKind, PendingResolution,
    };

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
                    request: ChoiceRequest {
                        player: PlayerId::new("p1"),
                        kind: PendingChoiceKind::Card {
                            cards: Vec::new(),
                            minimum: 0,
                            maximum: 0,
                            can_decline: false,
                        },
                        resolution: PendingResolution::HolyWindTakeHighest,
                    },
                },
                EffectIntent::RequestChoice {
                    request: ChoiceRequest {
                        player: PlayerId::new("p2"),
                        kind: PendingChoiceKind::Card {
                            cards: Vec::new(),
                            minimum: 0,
                            maximum: 0,
                            can_decline: false,
                        },
                        resolution: PendingResolution::ChaosReturnTwo,
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
