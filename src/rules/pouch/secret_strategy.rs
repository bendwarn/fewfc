//! 錦囊秘計的封閉決策模組。
//!
//! 外層 `pouch` 只負責取得來源、揭露、放置、消耗與連環的洗牌生命週期；本模組
//! 則以同一個完整 Decision 建構 offer、驗證輸入並規劃秘計後果。

use super::{
    GOLDEN_CICADA_STATUS, LURE_PLAYER_STATUS, LURE_SPIRIT_STATUS, WATCH_FIRE_STATUS, has_status,
    player_is_protected, profession_for_element, sheep_choice_events, spirit_for_element,
};
use crate::domain::{
    CardInstanceId, CardZone, DeckPlacement, Element, GameError, GameEvent, GameResult, GameState,
    PlayerId, PouchLevelBonus, RandomnessDeck, RuleImplementationError, SecretStrategy,
    SecretStrategyDecision, SecretStrategyEnvironmentOperation, SecretStrategyStarOperation,
    StarBreakReason, StarKind, StatusDuration, StatusEffect, StatusOwner, TemporaryStarEffect,
    ValidationError,
};
use crate::rules::PlayerFacingActionDetail;
use serde::Serialize;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
pub(crate) struct SecretStrategyCardOption {
    pub strategy: SecretStrategy,
}

/// 對玩家可見的答案形 offer。每個變體僅攜帶其相應 Decision 需要的候選資料。
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SecretStrategyOption {
    NoInput {
        source_card: CardInstanceId,
        strategy: SecretStrategy,
        detail: PlayerFacingActionDetail,
    },
    TargetPlayer {
        source_card: CardInstanceId,
        target_players: Vec<PlayerId>,
        detail: PlayerFacingActionDetail,
    },
    Star {
        source_card: CardInstanceId,
        gain_stars: Vec<StarKind>,
        break_stars: Vec<StarKind>,
        detail: PlayerFacingActionDetail,
    },
    Environment {
        source_card: CardInstanceId,
        hand_cards: Vec<CardInstanceId>,
        detail: PlayerFacingActionDetail,
    },
    SheepStealing {
        source_card: CardInstanceId,
        detail: PlayerFacingActionDetail,
    },
}

impl SecretStrategyOption {
    pub fn source_card(&self) -> CardInstanceId {
        match self {
            Self::NoInput { source_card, .. }
            | Self::TargetPlayer { source_card, .. }
            | Self::Star { source_card, .. }
            | Self::Environment { source_card, .. }
            | Self::SheepStealing { source_card, .. } => *source_card,
        }
    }

    pub fn strategy(&self) -> SecretStrategy {
        match self {
            Self::NoInput { strategy, .. } => *strategy,
            Self::TargetPlayer { .. } => SecretStrategy::LureTheTigerAway,
            Self::Star { .. } => SecretStrategy::DeceiveHeaven,
            Self::Environment { .. } => SecretStrategy::Retreat,
            Self::SheepStealing { .. } => SecretStrategy::SheepStealing,
        }
    }

    pub fn detail(&self) -> &PlayerFacingActionDetail {
        match self {
            Self::NoInput { detail, .. }
            | Self::TargetPlayer { detail, .. }
            | Self::Star { detail, .. }
            | Self::Environment { detail, .. }
            | Self::SheepStealing { detail, .. } => detail,
        }
    }

    pub(crate) fn representative_decision(&self) -> Option<SecretStrategyDecision> {
        match self {
            Self::NoInput {
                source_card,
                strategy,
                ..
            } => Some(SecretStrategyDecision::NoInput {
                source_card: *source_card,
                strategy: *strategy,
            }),
            Self::TargetPlayer {
                source_card,
                target_players,
                ..
            } => target_players.first().cloned().map(|target_player| {
                SecretStrategyDecision::TargetPlayer {
                    source_card: *source_card,
                    target_player,
                }
            }),
            Self::Star {
                source_card,
                gain_stars,
                ..
            } => gain_stars
                .first()
                .copied()
                .map(|star| SecretStrategyDecision::Star {
                    source_card: *source_card,
                    operation: SecretStrategyStarOperation::Gain { star },
                }),
            Self::Environment { source_card, .. } => Some(SecretStrategyDecision::Environment {
                source_card: *source_card,
                operation: SecretStrategyEnvironmentOperation::Clear,
            }),
            Self::SheepStealing { source_card, .. } => {
                Some(SecretStrategyDecision::SheepStealing {
                    source_card: *source_card,
                })
            }
        }
    }
}

pub(crate) fn strategy_options_for_card(
    element: Element,
    level: u32,
) -> Vec<SecretStrategyCardOption> {
    let elemental = match element {
        Element::Metal => SecretStrategy::GoldenCicada,
        Element::Wood => SecretStrategy::StealTheBeam,
        Element::Water => SecretStrategy::MuddyWaters,
        Element::Fire => SecretStrategy::WatchTheFire,
        Element::Earth => SecretStrategy::LureTheTigerAway,
    };
    let leveled = match level {
        1 => Some(SecretStrategy::ReturnSoul),
        2 => Some(SecretStrategy::SheepStealing),
        3 => Some(SecretStrategy::DarkCrossing),
        4 => Some(SecretStrategy::DeceiveHeaven),
        5 => Some(SecretStrategy::Retreat),
        _ => None,
    };
    std::iter::once(elemental)
        .chain(leveled)
        .map(|strategy| SecretStrategyCardOption { strategy })
        .collect()
}

pub(crate) fn strategy_matches(strategy: SecretStrategy, element: Element, level: u32) -> bool {
    strategy_options_for_card(element, level)
        .iter()
        .any(|option| option.strategy == strategy)
}

pub(crate) fn strategy_action_options(
    state: &GameState,
    player: &PlayerId,
    source_card: CardInstanceId,
) -> Vec<SecretStrategyOption> {
    let Some(definition) = state.card_def(source_card) else {
        return Vec::new();
    };
    let detail = |strategy| crate::rules::action_detail::secret_strategy_detail(strategy);
    let elemental = match definition.element {
        Element::Metal => SecretStrategyOption::NoInput {
            source_card,
            strategy: SecretStrategy::GoldenCicada,
            detail: detail(SecretStrategy::GoldenCicada),
        },
        Element::Wood => SecretStrategyOption::NoInput {
            source_card,
            strategy: SecretStrategy::StealTheBeam,
            detail: detail(SecretStrategy::StealTheBeam),
        },
        Element::Water => SecretStrategyOption::NoInput {
            source_card,
            strategy: SecretStrategy::MuddyWaters,
            detail: detail(SecretStrategy::MuddyWaters),
        },
        Element::Fire => SecretStrategyOption::NoInput {
            source_card,
            strategy: SecretStrategy::WatchTheFire,
            detail: detail(SecretStrategy::WatchTheFire),
        },
        Element::Earth => SecretStrategyOption::TargetPlayer {
            source_card,
            target_players: state.turn_order.clone(),
            detail: detail(SecretStrategy::LureTheTigerAway),
        },
    };
    let leveled = match definition.level.value() {
        1 => Some(SecretStrategyOption::NoInput {
            source_card,
            strategy: SecretStrategy::ReturnSoul,
            detail: detail(SecretStrategy::ReturnSoul),
        }),
        2 => Some(SecretStrategyOption::SheepStealing {
            source_card,
            detail: detail(SecretStrategy::SheepStealing),
        }),
        3 => Some(SecretStrategyOption::NoInput {
            source_card,
            strategy: SecretStrategy::DarkCrossing,
            detail: detail(SecretStrategy::DarkCrossing),
        }),
        4 => Some(SecretStrategyOption::Star {
            source_card,
            gain_stars: vec![
                StarKind::Metal,
                StarKind::Wood,
                StarKind::Water,
                StarKind::Fire,
                StarKind::Earth,
            ],
            break_stars: state.team_stars.iter().map(|owned| owned.star).collect(),
            detail: detail(SecretStrategy::DeceiveHeaven),
        }),
        5 => Some(SecretStrategyOption::Environment {
            source_card,
            hand_cards: state.hand(player).unwrap_or_default().to_vec(),
            detail: detail(SecretStrategy::Retreat),
        }),
        _ => None,
    };
    std::iter::once(elemental)
        .chain(leveled)
        .filter(|option| {
            option.representative_decision().is_some_and(|decision| {
                validate_decision(state, player, source_card, &decision).is_ok()
            })
        })
        .collect()
}

pub(crate) fn validate_decision(
    state: &GameState,
    player: &PlayerId,
    expected_source: CardInstanceId,
    decision: &SecretStrategyDecision,
) -> GameResult<()> {
    if decision.source_card() != expected_source {
        return Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ));
    }
    let definition = state
        .card_def(expected_source)
        .ok_or(GameError::Validation(
            ValidationError::MissingCardInstanceDefinition(expected_source),
        ))?;
    if !strategy_matches(
        decision.strategy(),
        definition.element,
        definition.level.value(),
    ) {
        return Err(GameError::Validation(
            ValidationError::SecretStrategyConditionMismatch,
        ));
    }

    match decision {
        SecretStrategyDecision::NoInput { strategy, .. } => {
            if matches!(
                strategy,
                SecretStrategy::LureTheTigerAway
                    | SecretStrategy::SheepStealing
                    | SecretStrategy::DeceiveHeaven
                    | SecretStrategy::Retreat
            ) {
                return Err(GameError::Validation(
                    ValidationError::SecretStrategyInputInvalid,
                ));
            }
        }
        SecretStrategyDecision::TargetPlayer { target_player, .. } => {
            if !state.turn_order.contains(target_player) {
                return Err(GameError::Validation(
                    ValidationError::SecretStrategyInputInvalid,
                ));
            }
        }
        SecretStrategyDecision::Star { operation, .. } => {
            if let SecretStrategyStarOperation::Break { star } = operation
                && !state.team_stars.iter().any(|owned| owned.star == *star)
            {
                return Err(GameError::Validation(
                    ValidationError::SecretStrategyInputInvalid,
                ));
            }
        }
        SecretStrategyDecision::Environment { operation, .. } => {
            if let SecretStrategyEnvironmentOperation::TransferByDiscard { card } = operation {
                if !state.hand(player).is_some_and(|hand| hand.contains(card))
                    || state.card_element(*card).is_none()
                {
                    return Err(GameError::Validation(
                        ValidationError::SecretStrategyInputInvalid,
                    ));
                }
            }
        }
        SecretStrategyDecision::SheepStealing { .. } => {
            crate::rules::deck_supply::plan(
                state,
                &RandomnessDeck::Player(player.clone()),
                2,
                DeckPlacement::Bottom,
            )?;
        }
    }
    Ok(())
}

pub(crate) fn strategy_events(
    state: &GameState,
    player: &PlayerId,
    decision: &SecretStrategyDecision,
) -> GameResult<Vec<GameEvent>> {
    let source_card = decision.source_card();
    let status = |kind: &str, owner: PlayerId, duration: StatusDuration| GameEvent::StatusAdded {
        status: StatusEffect {
            id: format!("{kind}:{}:{}", owner.as_str(), state.turn_number),
            owner: StatusOwner::Player(owner),
            kind: kind.to_string(),
            value: None,
            duration,
        },
    };
    let unexpected = || {
        GameError::RuleImplementation(RuleImplementationError::EffectNotImplemented(
            "pouch:secret-strategy:validated-decision-shape".to_string(),
        ))
    };

    match decision {
        SecretStrategyDecision::NoInput { strategy, .. } => Ok(match strategy {
            SecretStrategy::GoldenCicada => vec![status(
                GOLDEN_CICADA_STATUS,
                player.clone(),
                StatusDuration::UntilTurnEnd {
                    player: player.clone(),
                },
            )],
            SecretStrategy::StealTheBeam => vec![GameEvent::PouchLevelBonusGranted {
                bonus: PouchLevelBonus {
                    player: player.clone(),
                    cards: state.hand(player).unwrap_or_default().to_vec(),
                    applied_on_turn: state.turn_number,
                },
            }],
            SecretStrategy::MuddyWaters => {
                let old = state
                    .turn_draw_bonus_by_player
                    .get(player)
                    .copied()
                    .unwrap_or(0);
                vec![GameEvent::TurnDrawBonusChanged {
                    player: player.clone(),
                    old_value: old,
                    delta: 1,
                    new_value: old + 1,
                }]
            }
            SecretStrategy::WatchTheFire => {
                let index = state
                    .turn_order
                    .iter()
                    .position(|candidate| candidate == player)
                    .ok_or_else(|| {
                        GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
                    })?;
                let next = state.turn_order[(index + 1) % state.turn_order.len()].clone();
                vec![status(
                    WATCH_FIRE_STATUS,
                    next.clone(),
                    StatusDuration::UntilTurnEnd { player: next },
                )]
            }
            SecretStrategy::ReturnSoul => {
                let element = state.card_element(source_card).ok_or_else(unexpected)?;
                let spirit = spirit_for_element(element);
                let previous = state.spirit_for(player);
                let power = if has_status(state, player, LURE_SPIRIT_STATUS) {
                    1
                } else {
                    1 + previous.map_or(0, |owned| owned.power).min(5)
                };
                vec![GameEvent::SpiritRevived {
                    player: player.clone(),
                    previous: previous.map(|owned| owned.spirit),
                    spirit,
                    power,
                }]
            }
            SecretStrategy::DarkCrossing => {
                if has_status(state, player, "CannotChangeProfession") {
                    Vec::new()
                } else {
                    let element = state.card_element(source_card).ok_or_else(unexpected)?;
                    vec![GameEvent::ProfessionTransformed {
                        player: player.clone(),
                        previous: state.profession_for(player).cloned(),
                        profession: profession_for_element(element),
                        reason: "pouch:dark-crossing".to_string(),
                    }]
                }
            }
            SecretStrategy::LureTheTigerAway
            | SecretStrategy::SheepStealing
            | SecretStrategy::DeceiveHeaven
            | SecretStrategy::Retreat => return Err(unexpected()),
        }),
        SecretStrategyDecision::TargetPlayer { target_player, .. } => {
            let mut events = Vec::new();
            if !player_is_protected(state, target_player) {
                events.push(status(
                    LURE_PLAYER_STATUS,
                    target_player.clone(),
                    StatusDuration::UntilTurnEnd {
                        player: target_player.clone(),
                    },
                ));
            }
            events.push(status(
                LURE_SPIRIT_STATUS,
                target_player.clone(),
                StatusDuration::UntilTurnEnd {
                    player: target_player.clone(),
                },
            ));
            Ok(events)
        }
        SecretStrategyDecision::Star { operation, .. } => match operation {
            SecretStrategyStarOperation::Gain { star } => {
                Ok(vec![GameEvent::TemporaryStarEffectGranted {
                    effect: TemporaryStarEffect {
                        player: player.clone(),
                        star: *star,
                        applied_on_turn: state.turn_number,
                    },
                }])
            }
            SecretStrategyStarOperation::Break { star } => {
                let owned = state
                    .team_stars
                    .iter()
                    .find(|owned| owned.star == *star)
                    .ok_or_else(unexpected)?;
                Ok(vec![GameEvent::StarBroken {
                    team: owned.team.clone(),
                    star: *star,
                    reason: StarBreakReason::SecretStrategy,
                    hp_change: None,
                }])
            }
        },
        SecretStrategyDecision::Environment { operation, .. } => match operation {
            SecretStrategyEnvironmentOperation::Clear => Ok(state
                .environment
                .map(|environment| {
                    vec![GameEvent::EnvironmentCleared {
                        player: player.clone(),
                        formation_id: "pouch:retreat".to_string(),
                        environment,
                        hp_changes: Vec::new(),
                    }]
                })
                .unwrap_or_default()),
            SecretStrategyEnvironmentOperation::TransferByDiscard { card } => {
                let element = state.card_element(*card).ok_or_else(unexpected)?;
                Ok(vec![
                    GameEvent::CardsMoved {
                        card_moves: vec![crate::domain::discard::move_from(
                            state,
                            *card,
                            CardZone::Hand(player.clone()),
                        )?],
                    },
                    GameEvent::EnvironmentTransferred {
                        player: player.clone(),
                        formation_id: "pouch:retreat".to_string(),
                        from: state.environment,
                        to: element,
                    },
                ])
            }
        },
        SecretStrategyDecision::SheepStealing { .. } => {
            sheep_choice_events(state, player, source_card)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail() -> PlayerFacingActionDetail {
        PlayerFacingActionDetail {
            consequences: Vec::new(),
        }
    }

    #[test]
    fn strategy_options_serialize_each_answer_family_with_exact_camel_case_fields() {
        let source_card = CardInstanceId::new(7);
        let target = PlayerId::new("target");
        let options = vec![
            SecretStrategyOption::NoInput {
                source_card,
                strategy: SecretStrategy::GoldenCicada,
                detail: detail(),
            },
            SecretStrategyOption::TargetPlayer {
                source_card,
                target_players: vec![target],
                detail: detail(),
            },
            SecretStrategyOption::Star {
                source_card,
                gain_stars: vec![StarKind::Fire],
                break_stars: vec![StarKind::Water],
                detail: detail(),
            },
            SecretStrategyOption::Environment {
                source_card,
                hand_cards: vec![CardInstanceId::new(8)],
                detail: detail(),
            },
            SecretStrategyOption::SheepStealing {
                source_card,
                detail: detail(),
            },
        ];

        assert_eq!(
            serde_json::to_value(options).unwrap(),
            serde_json::json!([
                {
                    "type": "noInput",
                    "sourceCard": 7,
                    "strategy": "GoldenCicada",
                    "detail": { "consequences": [] }
                },
                {
                    "type": "targetPlayer",
                    "sourceCard": 7,
                    "targetPlayers": ["target"],
                    "detail": { "consequences": [] }
                },
                {
                    "type": "star",
                    "sourceCard": 7,
                    "gainStars": ["Fire"],
                    "breakStars": ["Water"],
                    "detail": { "consequences": [] }
                },
                {
                    "type": "environment",
                    "sourceCard": 7,
                    "handCards": [8],
                    "detail": { "consequences": [] }
                },
                {
                    "type": "sheepStealing",
                    "sourceCard": 7,
                    "detail": { "consequences": [] }
                }
            ])
        );
    }
}
