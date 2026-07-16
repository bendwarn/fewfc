use crate::domain::{
    CardInstanceId, CardMoveDelta, CardOrigin, CardZone, EarthRendingPlayerAnswer,
    EarthRendingResolution, EffectChoiceAnswer, EffectChoiceOptions, Element, GameError, GameEvent,
    GameResult, GameState, HpChangeDelta, PendingChoiceKind, PendingRandomness, PlayerId,
    RandomnessContinuation, RandomnessDeck, RustedForestResolution, StatusDuration, StatusEffect,
    StatusOwner, TeamId, TribulationRandomnessContinuation, ValidationError,
    targeting::TurnOrderTargets,
};
use crate::rules::{
    AttackCategory, AttackPlanDef, BaseFormationSpec, DamageTarget, EffectDef, EffectPlan,
    FormationCategory, FormationDef, FormationPattern, PointFormula, SpellPlanDef,
    SubmittedCardFacts,
};

pub(crate) const THUNDER_FIRE: &str = "tribulation:thunder-fire";
pub(crate) const GALE_RAIN: &str = "tribulation:gale-rain";
pub(crate) const MUDSLIDE_TORRENT: &str = "tribulation:mudslide-torrent";
pub(crate) const EARTH_RENDING: &str = "tribulation:earth-rending";
pub(crate) const RUSTED_FOREST: &str = "tribulation:rusted-forest";
pub(crate) const DIVINE_CALCULATION: &str = "tribulation:divine-calculation";

pub(crate) fn matches_elements(
    submitted: &[SubmittedCardFacts],
    first: Element,
    second: Element,
) -> bool {
    submitted.len() >= 4
        && submitted
            .iter()
            .all(|card| card.element == first || card.element == second)
        && [first, second].into_iter().all(|element| {
            let cards = submitted
                .iter()
                .filter(|card| card.element == element)
                .collect::<Vec<_>>();
            !cards.is_empty() && cards.into_iter().map(|card| card.level).sum::<u32>() >= 7
        })
}

pub(crate) fn formation_specs() -> Vec<BaseFormationSpec> {
    vec![
        tribulation(
            THUNDER_FIRE,
            "天雷劫火",
            "金火各至少一張且各自等級合計至少７；特殊攻擊６０，雙方生命各扣１５",
        ),
        tribulation(
            GALE_RAIN,
            "烈風暴雨",
            "火水各至少一張且各自等級合計至少７；特殊攻擊６０，所有玩家獲得兩回合烈風暴雨狀態",
        ),
        tribulation(
            MUDSLIDE_TORRENT,
            "泥石轟流",
            "水土各至少一張且各自等級合計至少７；所有防護罩扣２０，若有實際扣除則特殊攻擊８０，否則６０",
        ),
        tribulation(
            EARTH_RENDING,
            "裂地崩山",
            "土木各至少一張且各自等級合計至少７；特殊攻擊６０並轉移環境，各玩家捨棄環行牌或展示手牌",
        ),
        tribulation(
            RUSTED_FOREST,
            "鏽鐵枯林",
            "木金各至少一張且各自等級合計至少７；特殊攻擊６０並處理牌組頂八張牌",
        ),
        BaseFormationSpec {
            formation: FormationDef {
                id: DIVINE_CALCULATION.to_string(),
                name: "神算".to_string(),
                rule_text: "一張有效等級４以上的牌；取得神算狀態".to_string(),
                category: FormationCategory::Spell,
                pattern: FormationPattern::Custom(DIVINE_CALCULATION.to_string()),
                effect_id: DIVINE_CALCULATION.to_string(),
                point_formula: PointFormula::Fixed(0),
            },
            effect: EffectDef {
                id: DIVINE_CALCULATION.to_string(),
                plan: EffectPlan::ActiveSpell(SpellPlanDef {
                    resolver_id: DIVINE_CALCULATION.to_string(),
                }),
            },
        },
    ]
}

fn tribulation(id: &str, name: &str, rule_text: &str) -> BaseFormationSpec {
    BaseFormationSpec {
        formation: FormationDef {
            id: id.to_string(),
            name: name.to_string(),
            rule_text: rule_text.to_string(),
            category: FormationCategory::Attack,
            pattern: FormationPattern::Custom(id.to_string()),
            effect_id: id.to_string(),
            point_formula: PointFormula::Fixed(60),
        },
        effect: EffectDef {
            id: id.to_string(),
            plan: EffectPlan::Attack(AttackPlanDef {
                category: AttackCategory::Special,
                point_formula: PointFormula::Fixed(60),
                damage_target: DamageTarget::PreviousPlayer,
            }),
        },
    }
}

pub(crate) fn active_spell_events(
    state: &GameState,
    player: &PlayerId,
    resolver_id: &str,
) -> Option<Vec<GameEvent>> {
    (resolver_id == DIVINE_CALCULATION).then(|| {
        let mut events = state
            .statuses
            .iter()
            .filter(|status| status.kind == "DivineCalculation")
            .map(|status| GameEvent::StatusRemoved {
                status_id: status.id.clone(),
                owner: status.owner.clone(),
            })
            .collect::<Vec<_>>();
        events.push(GameEvent::StatusAdded {
            status: StatusEffect {
                id: format!("tribulation:divine-calculation:{}", state.turn_number),
                owner: StatusOwner::Player(player.clone()),
                kind: "DivineCalculation".to_string(),
                value: None,
                duration: StatusDuration::Permanent,
            },
        });
        events
    })
}

pub(crate) fn is_tribulation(id: &str) -> bool {
    matches!(
        id,
        THUNDER_FIRE | GALE_RAIN | MUDSLIDE_TORRENT | EARTH_RENDING | RUSTED_FOREST
    )
}

pub(crate) fn divine_calculation_owner(state: &GameState) -> Option<PlayerId> {
    state.statuses.iter().find_map(|status| {
        (status.kind == "DivineCalculation").then(|| match &status.owner {
            StatusOwner::Player(player) => Some(player.clone()),
            StatusOwner::Team(_) => None,
        })?
    })
}

pub(crate) fn reduces_attack_damage(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) -> bool {
    is_tribulation(formation_id) && divine_calculation_owner(state).as_ref() == Some(player)
}

pub(crate) fn suppress_formation_recovery(
    state: &GameState,
    performer: &PlayerId,
    events: &mut [GameEvent],
) {
    let blocked = state.statuses.iter().any(|status| {
        status.kind == "GaleRain" && status.owner == StatusOwner::Player(performer.clone())
    });
    if !blocked {
        return;
    }
    for event in events {
        let change = match event {
            GameEvent::HpChanged { change } => Some(change),
            GameEvent::AttackResolved { hp_change, .. } => Some(hp_change),
            _ => None,
        };
        if let Some(change) = change
            && change.effective_delta > 0
        {
            change.new_hp = change.old_hp;
            change.effective_delta = 0;
        }
    }
}

pub(crate) fn pre_attack_events(
    state: &GameState,
    attacker: &PlayerId,
    formation_id: &str,
) -> GameResult<Vec<GameEvent>> {
    let protected = divine_calculation_owner(state);
    match formation_id {
        THUNDER_FIRE => state
            .hp
            .iter()
            .filter(|entry| {
                protected
                    .as_ref()
                    .and_then(|player| team_of(state, player).ok())
                    .is_none_or(|team| team != entry.team)
            })
            .map(|entry| {
                let new_hp = (entry.hp - 15).max(0);
                Ok(GameEvent::HpChanged {
                    change: HpChangeDelta {
                        team: entry.team.clone(),
                        old_hp: entry.hp,
                        delta: -15,
                        new_hp,
                        effective_delta: new_hp - entry.hp,
                    },
                })
            })
            .collect(),
        MUDSLIDE_TORRENT => Ok(state
            .shields
            .iter()
            .filter(|shield| shield.value > 0 && protected.as_ref() != Some(&shield.player))
            .map(|shield| {
                let new_value = (shield.value - 20).max(0);
                GameEvent::ShieldChanged {
                    player: shield.player.clone(),
                    old_value: shield.value,
                    delta: new_value - shield.value,
                    new_value,
                }
            })
            .collect()),
        GALE_RAIN | EARTH_RENDING | RUSTED_FOREST => {
            let _ = attacker;
            Ok(Vec::new())
        }
        _ => Ok(Vec::new()),
    }
}

pub(crate) fn attack_points(formation_id: &str, pre_events: &[GameEvent]) -> Option<u32> {
    is_tribulation(formation_id).then(|| {
        if formation_id == MUDSLIDE_TORRENT
            && pre_events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::ShieldChanged { delta, .. } if *delta < 0
                )
            })
        {
            80
        } else {
            60
        }
    })
}

pub(crate) fn post_attack_events(
    state: &GameState,
    attacker: &PlayerId,
    formation_id: &str,
) -> GameResult<Vec<GameEvent>> {
    if !is_tribulation(formation_id) {
        return Ok(Vec::new());
    }
    let protected = divine_calculation_owner(state);
    let mut events = Vec::new();
    if formation_id == GALE_RAIN {
        for player in state
            .players
            .iter()
            .map(|entry| &entry.id)
            .filter(|player| protected.as_ref() != Some(*player))
        {
            let turns = usize::from(player != attacker) + 1;
            let expires_on =
                TurnOrderTargets::new(state).nth_future_turn_for_player(player, turns)?;
            events.push(GameEvent::StatusAdded {
                status: StatusEffect {
                    id: format!(
                        "tribulation:gale-rain:{}:{}",
                        player.as_str(),
                        state.turn_number
                    ),
                    owner: StatusOwner::Player(player.clone()),
                    kind: "GaleRain".to_string(),
                    value: None,
                    duration: StatusDuration::UntilTurnEndNumber {
                        player: player.clone(),
                        turn_number: expires_on,
                    },
                },
            });
        }
    }
    events.extend(divine_calculation_consumption_events(state, formation_id));
    Ok(events)
}

pub(crate) fn divine_calculation_consumption_events(
    state: &GameState,
    formation_id: &str,
) -> Vec<GameEvent> {
    if !is_tribulation(formation_id) {
        return Vec::new();
    }
    state
        .statuses
        .iter()
        .find(|status| status.kind == "DivineCalculation")
        .map(|status| {
            vec![GameEvent::StatusRemoved {
                status_id: status.id.clone(),
                owner: status.owner.clone(),
            }]
        })
        .unwrap_or_default()
}

pub(crate) fn earth_rending_start_events(
    state: &GameState,
    attacker: &PlayerId,
    used_cards: &[CardInstanceId],
    damage_prevented: bool,
    split_attack_damage: bool,
) -> Vec<GameEvent> {
    let attacker_index = state
        .turn_order
        .iter()
        .position(|player| player == attacker)
        .expect("validated attacker must be in Turn Order");
    let remaining_players = (1..=state.turn_order.len())
        .map(|offset| state.turn_order[(attacker_index + offset) % state.turn_order.len()].clone())
        .collect();
    vec![
        GameEvent::EarthRendingStarted {
            resolution: EarthRendingResolution {
                attacker: attacker.clone(),
                used_cards: used_cards.to_vec(),
                environment: None,
                remaining_players,
                answers: Vec::new(),
                damage_prevented,
                split_attack_damage,
            },
        },
        GameEvent::EffectChoiceRequested {
            player: attacker.clone(),
            kind: PendingChoiceKind::TypedEffect {
                effect_id: EARTH_RENDING.to_string(),
                continuation_id: "tribulation:earth-rending:environment".to_string(),
                options: EffectChoiceOptions {
                    environments: vec![
                        Element::Metal,
                        Element::Wood,
                        Element::Water,
                        Element::Fire,
                        Element::Earth,
                    ],
                    ..Default::default()
                },
            },
        },
    ]
}

pub(crate) fn answer_choice(
    state: &GameState,
    continuation_id: &str,
    answer: &EffectChoiceAnswer,
) -> GameResult<Option<Vec<GameEvent>>> {
    let Some(active) = state.active_earth_rending_resolution.as_ref() else {
        return Ok(None);
    };
    let mut events = match continuation_id {
        "tribulation:earth-rending:environment" => {
            let EffectChoiceAnswer::Environment { environment } = answer else {
                return Err(GameError::Validation(
                    ValidationError::InvalidEffectChoiceAnswer,
                ));
            };
            vec![GameEvent::EarthRendingEnvironmentChosen {
                environment: *environment,
            }]
        }
        "tribulation:earth-rending:card" => {
            let EffectChoiceAnswer::Cards { cards } = answer else {
                return Err(GameError::Validation(
                    ValidationError::InvalidEffectChoiceAnswer,
                ));
            };
            vec![GameEvent::EarthRendingPlayerAnswered {
                answer: EarthRendingPlayerAnswer {
                    player: active.remaining_players.first().cloned().ok_or_else(|| {
                        GameError::RuleImplementation(
                            crate::domain::RuleImplementationError::EffectNotImplemented(
                                "earth-rending:missing-player".to_string(),
                            ),
                        )
                    })?,
                    card: cards.first().copied(),
                    revealed_hand: Vec::new(),
                    protected: false,
                },
            }]
        }
        _ => return Ok(None),
    };
    continue_earth_rending(state, &mut events)?;
    Ok(Some(events))
}

fn continue_earth_rending(state: &GameState, events: &mut Vec<GameEvent>) -> GameResult<()> {
    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    loop {
        let active = projected
            .active_earth_rending_resolution
            .clone()
            .expect("Earth Rending continuation requires active state");
        let Some(player) = active.remaining_players.first().cloned() else {
            finish_earth_rending(&projected, events)?;
            return Ok(());
        };
        let environment = active.environment.ok_or_else(|| {
            GameError::RuleImplementation(
                crate::domain::RuleImplementationError::EffectNotImplemented(
                    "earth-rending:missing-environment".to_string(),
                ),
            )
        })?;
        let protected = divine_calculation_owner(&projected).as_ref() == Some(&player);
        let hand = projected
            .hand(&player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
        let hand = hand.to_vec();
        let allowed_cards = hand
            .iter()
            .copied()
            .filter(|card| {
                projected
                    .card_def(*card)
                    .is_some_and(|definition| definition.element == environment)
                    && !(player == active.attacker && active.used_cards.contains(card))
            })
            .collect::<Vec<_>>();
        if !protected && !allowed_cards.is_empty() {
            events.push(GameEvent::EffectChoiceRequested {
                player,
                kind: PendingChoiceKind::TypedEffect {
                    effect_id: EARTH_RENDING.to_string(),
                    continuation_id: "tribulation:earth-rending:card".to_string(),
                    options: EffectChoiceOptions {
                        cards: Some(crate::domain::CardChoiceOptions {
                            allowed_cards,
                            minimum: 1,
                            maximum: 1,
                        }),
                        ..Default::default()
                    },
                },
            });
            return Ok(());
        }
        let event = GameEvent::EarthRendingPlayerAnswered {
            answer: EarthRendingPlayerAnswer {
                player: player.clone(),
                card: None,
                revealed_hand: if protected { Vec::new() } else { hand.clone() },
                protected,
            },
        };
        crate::rules::projection::apply_event(&mut projected, &event);
        events.push(event);
        if !protected {
            events.push(GameEvent::HandRevealed {
                player,
                cards: hand,
            });
        }
    }
}

fn finish_earth_rending(state: &GameState, events: &mut Vec<GameEvent>) -> GameResult<()> {
    let active = state
        .active_earth_rending_resolution
        .clone()
        .expect("Earth Rending completion requires active state");
    let environment = active.environment.expect("environment was chosen");
    events.push(GameEvent::EnvironmentTransferred {
        player: active.attacker.clone(),
        formation_id: EARTH_RENDING.to_string(),
        from: state.environment,
        to: environment,
    });
    let card_moves = active
        .answers
        .iter()
        .filter_map(|answer| answer.card.map(|card| (answer.player.clone(), card)))
        .map(|(player, card)| CardMoveDelta {
            card,
            from: CardZone::Hand(player),
            to: discard_zone(state, card),
        })
        .collect::<Vec<_>>();
    if !card_moves.is_empty() {
        events.push(GameEvent::CardsMoved { card_moves });
    }
    let mut projected = state.clone();
    for event in events.iter().skip_while(|event| {
        !matches!(event, GameEvent::EnvironmentTransferred { formation_id, .. } if formation_id == EARTH_RENDING)
    }) {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    let mut attack_events = crate::rules::base::attack_resolution::resolve(
        &projected,
        crate::rules::base::attack_resolution::AttackRequest {
            attacker: active.attacker.clone(),
            formation_id: EARTH_RENDING.to_string(),
            category: AttackCategory::Special,
            point_formula: PointFormula::Fixed(60),
            used_cards: active.used_cards,
            damage_prevented: active.damage_prevented,
            split_attack_damage: active.split_attack_damage,
            mode: crate::rules::base::attack_resolution::AttackResolutionMode::FormationUse,
        },
    )?;
    attack_events.extend(post_attack_events(
        &projected,
        &active.attacker,
        EARTH_RENDING,
    )?);
    crate::rules::dark::append_shared_fate_events(
        &projected,
        &active.attacker,
        EARTH_RENDING,
        &mut attack_events,
    )?;
    events.extend(attack_events);
    events.push(GameEvent::EarthRendingCompleted {
        player: active.attacker,
    });
    Ok(())
}

fn discard_zone(state: &GameState, card: CardInstanceId) -> CardZone {
    if state.uses_personal_decks() {
        match state.card_origin(card) {
            Some(CardOrigin::Player(owner)) => CardZone::PlayerDiscard(owner.clone()),
            _ => CardZone::Discard,
        }
    } else {
        CardZone::Discard
    }
}

pub(crate) fn rusted_forest_start_events(
    state: &GameState,
    attacker: &PlayerId,
    used_cards: &[CardInstanceId],
    damage_prevented: bool,
    split_attack_damage: bool,
) -> GameResult<Vec<GameEvent>> {
    let protected = divine_calculation_owner(state);
    let remaining_decks = if state.uses_personal_decks() {
        state
            .turn_order
            .iter()
            .filter(|player| protected.as_ref() != Some(*player))
            .cloned()
            .map(RandomnessDeck::Player)
            .collect()
    } else {
        vec![RandomnessDeck::Shared]
    };
    let mut events = vec![GameEvent::RustedForestStarted {
        resolution: RustedForestResolution {
            attacker: attacker.clone(),
            used_cards: used_cards.to_vec(),
            remaining_decks,
            damage_prevented,
            split_attack_damage,
        },
    }];
    continue_rusted_forest(state, &mut events)?;
    Ok(events)
}

pub(crate) fn after_rusted_forest_randomness_events(
    state: &GameState,
) -> GameResult<Vec<GameEvent>> {
    let deck = state
        .active_rusted_forest_resolution
        .as_ref()
        .and_then(|active| active.remaining_decks.first())
        .cloned()
        .ok_or_else(|| {
            GameError::RuleImplementation(
                crate::domain::RuleImplementationError::EffectNotImplemented(
                    "rusted-forest:missing-deck".to_string(),
                ),
            )
        })?;
    let mut events = vec![GameEvent::RustedForestDeckProcessed { deck }];
    continue_rusted_forest(state, &mut events)?;
    Ok(events)
}

fn continue_rusted_forest(state: &GameState, events: &mut Vec<GameEvent>) -> GameResult<()> {
    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    loop {
        let active = projected
            .active_rusted_forest_resolution
            .clone()
            .expect("Rusted Forest continuation requires active state");
        let Some(deck_kind) = active.remaining_decks.first().cloned() else {
            finish_rusted_forest(&projected, events)?;
            return Ok(());
        };
        let deck_owner = match &deck_kind {
            RandomnessDeck::Shared => &active.attacker,
            RandomnessDeck::Player(player) => player,
        };
        let deck = projected
            .deck_for(deck_owner)
            .ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(deck_owner.clone()))
            })?
            .to_vec();
        let revealed = deck.iter().take(8).copied().collect::<Vec<_>>();
        let discarded = revealed
            .iter()
            .copied()
            .filter(|card| {
                projected
                    .card_def(*card)
                    .is_some_and(|definition| definition.level >= 3)
            })
            .collect::<Vec<_>>();
        let reveal_event = GameEvent::RustedForestCardsRevealed {
            deck: deck_kind.clone(),
            cards: revealed,
        };
        crate::rules::projection::apply_event(&mut projected, &reveal_event);
        events.push(reveal_event);
        if !discarded.is_empty() {
            let card_moves = discarded
                .into_iter()
                .map(|card| CardMoveDelta {
                    card,
                    from: match &deck_kind {
                        RandomnessDeck::Shared => CardZone::DeckTop,
                        RandomnessDeck::Player(player) => CardZone::PlayerDeckTop(player.clone()),
                    },
                    to: discard_zone(&projected, card),
                })
                .collect();
            let move_event = GameEvent::CardsMoved { card_moves };
            crate::rules::projection::apply_event(&mut projected, &move_event);
            events.push(move_event);
        }
        let current_order = projected
            .deck_for(deck_owner)
            .expect("validated deck")
            .to_vec();
        if current_order.len() > 1 {
            events.push(GameEvent::RandomnessRequested {
                request: PendingRandomness {
                    request_id: format!(
                        "tribulation:rusted-forest:{}:{}",
                        projected.turn_number,
                        match &deck_kind {
                            RandomnessDeck::Shared => "shared",
                            RandomnessDeck::Player(player) => player.as_str(),
                        }
                    ),
                    operation: crate::domain::RandomnessOperation::DeckShuffle {
                        deck: deck_kind,
                    },
                    continuation: RandomnessContinuation::Tribulation(
                        TribulationRandomnessContinuation::RustedForestShuffle,
                    ),
                    current_order,
                },
            });
            return Ok(());
        }
        let processed = GameEvent::RustedForestDeckProcessed { deck: deck_kind };
        crate::rules::projection::apply_event(&mut projected, &processed);
        events.push(processed);
    }
}

fn finish_rusted_forest(state: &GameState, events: &mut Vec<GameEvent>) -> GameResult<()> {
    let active = state
        .active_rusted_forest_resolution
        .clone()
        .expect("Rusted Forest completion requires active state");
    let mut attack_events = crate::rules::base::attack_resolution::resolve(
        state,
        crate::rules::base::attack_resolution::AttackRequest {
            attacker: active.attacker.clone(),
            formation_id: RUSTED_FOREST.to_string(),
            category: AttackCategory::Special,
            point_formula: PointFormula::Fixed(60),
            used_cards: active.used_cards,
            damage_prevented: active.damage_prevented,
            split_attack_damage: active.split_attack_damage,
            mode: crate::rules::base::attack_resolution::AttackResolutionMode::FormationUse,
        },
    )?;
    attack_events.extend(post_attack_events(state, &active.attacker, RUSTED_FOREST)?);
    crate::rules::dark::append_shared_fate_events(
        state,
        &active.attacker,
        RUSTED_FOREST,
        &mut attack_events,
    )?;
    events.extend(attack_events);
    events.push(GameEvent::RustedForestCompleted {
        player: active.attacker,
    });
    Ok(())
}

fn team_of(state: &GameState, player: &PlayerId) -> GameResult<TeamId> {
    state
        .players
        .iter()
        .find(|entry| &entry.id == player)
        .map(|entry| entry.team.clone())
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::{SubmittedCardFacts, base_formation_matcher};

    #[test]
    fn catalog_contains_all_published_formations() {
        assert_eq!(
            formation_specs()
                .into_iter()
                .map(|spec| spec.formation.name)
                .collect::<Vec<_>>(),
            [
                "天雷劫火",
                "烈風暴雨",
                "泥石轟流",
                "裂地崩山",
                "鏽鐵枯林",
                "神算"
            ]
        );
    }

    #[test]
    fn tribulations_require_only_the_named_elements_and_seven_levels_each() {
        let pattern = formation_specs()[0].formation.pattern.clone();
        let matcher = base_formation_matcher();
        assert!(matcher.matches(
            &pattern,
            &[
                SubmittedCardFacts {
                    element: crate::domain::Element::Metal,
                    level: 4,
                },
                SubmittedCardFacts {
                    element: crate::domain::Element::Metal,
                    level: 3,
                },
                SubmittedCardFacts {
                    element: crate::domain::Element::Fire,
                    level: 4,
                },
                SubmittedCardFacts {
                    element: crate::domain::Element::Fire,
                    level: 3,
                },
            ],
        ));
        assert!(!matcher.matches(
            &pattern,
            &[
                SubmittedCardFacts {
                    element: crate::domain::Element::Metal,
                    level: 5,
                },
                SubmittedCardFacts {
                    element: crate::domain::Element::Fire,
                    level: 5,
                },
                SubmittedCardFacts {
                    element: crate::domain::Element::Wood,
                    level: 5,
                },
                SubmittedCardFacts {
                    element: crate::domain::Element::Fire,
                    level: 2,
                },
            ],
        ));
    }
}
