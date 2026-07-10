use crate::domain::{
    CardInstanceId, CardMoveDelta, CardOrigin, CardZone, Command, Element, GameError, GameEvent,
    GamePreparationStage, GameResult, GameState, GameStatus, POUCH_MODULE_ID, PlayerCardPile,
    PlayerId, PouchLevelBonus, ProfessionId, RandomnessDeck, SecretStrategy, SpiritKind,
    StarBreakReason, StarKind, StatusDuration, StatusEffect, StatusOwner, TemporaryStarEffect,
    ValidationError,
};
use crate::rules::{
    BaseFormationSpec, EffectDef, EffectPlan, FormationCategory, FormationDef, FormationPattern,
    PointFormula, SpellPlanDef,
};

pub(crate) const CHAIN_ID: &str = "pouch:chain";
pub(crate) const GOLDEN_CICADA_STATUS: &str = "PouchGoldenCicada";
pub(crate) const WATCH_FIRE_STATUS: &str = "PouchWatchFire";
pub(crate) const LURE_PLAYER_STATUS: &str = "PouchLurePlayer";
pub(crate) const LURE_SPIRIT_STATUS: &str = "PouchLureSpirit";

pub(crate) fn formation_specs() -> Vec<BaseFormationSpec> {
    vec![BaseFormationSpec {
        formation: FormationDef {
            id: CHAIN_ID.to_string(),
            name: "連環".to_string(),
            rule_text: "三張不同行、不同級牌；從自身牌組選擇一或兩張牌".to_string(),
            category: FormationCategory::Spell,
            pattern: FormationPattern::Custom(CHAIN_ID.to_string()),
            effect_id: CHAIN_ID.to_string(),
            point_formula: PointFormula::Fixed(0),
        },
        effect: EffectDef {
            id: CHAIN_ID.to_string(),
            plan: EffectPlan::ActiveSpell(SpellPlanDef {
                resolver_id: CHAIN_ID.to_string(),
            }),
        },
    }]
}

pub(crate) fn matches_chain(cards: &[crate::rules::SubmittedCardFacts]) -> bool {
    cards.len() == 3
        && cards.iter().enumerate().all(|(index, card)| {
            cards[..index]
                .iter()
                .all(|other| other.element != card.element && other.level != card.level)
        })
}

pub(crate) fn initial_events(
    setup: &crate::domain::GameSetup,
) -> GameResult<Option<Vec<GameEvent>>> {
    if !setup.has_rule_module(POUCH_MODULE_ID) {
        return Ok(None);
    }
    let player_decks = setup
        .turn_order
        .iter()
        .map(|player| PlayerCardPile {
            player: player.clone(),
            cards: setup
                .card_instances
                .iter()
                .filter(|card| matches!(&card.origin, CardOrigin::Player(owner) if owner == player))
                .map(|card| card.instance)
                .collect(),
        })
        .collect();
    Ok(Some(vec![GameEvent::GamePreparationStarted {
        player_decks,
    }]))
}

pub(crate) fn decide_command(
    state: &GameState,
    command: &Command,
) -> GameResult<Option<Vec<GameEvent>>> {
    match command {
        Command::ChooseInitialPouch { player, card } => {
            choose_initial_pouch(state, player, *card).map(Some)
        }
        Command::TriggerSecretStrategy {
            player,
            strategy,
            target_player,
            star,
            break_star,
            discard_card,
            deck_cards,
            discard_cards,
        } => resolve_owned_pouch(
            state,
            player,
            *strategy,
            target_player.as_ref(),
            *star,
            *break_star,
            *discard_card,
            deck_cards,
            discard_cards,
        )
        .map(Some),
        _ => Ok(None),
    }
}

fn choose_initial_pouch(
    state: &GameState,
    player: &PlayerId,
    card: CardInstanceId,
) -> GameResult<Vec<GameEvent>> {
    if !state.has_rule_module(POUCH_MODULE_ID) {
        return Err(GameError::Validation(ValidationError::PouchRuleDisabled));
    }
    let expected = match &state.status {
        GameStatus::Preparing {
            stage: GamePreparationStage::InitialPouchSelection { player },
        } => player,
        _ => {
            return Err(GameError::Validation(
                ValidationError::InitialPouchSelectionUnavailable,
            ));
        }
    };
    if expected != player {
        return Err(GameError::Validation(ValidationError::WrongPlayer {
            expected: expected.clone(),
            actual: player.clone(),
        }));
    }
    if !state
        .deck_for(player)
        .is_some_and(|deck| deck.contains(&card))
    {
        return Err(GameError::Validation(ValidationError::InvalidInitialPouch(
            card,
        )));
    }
    let index = state
        .turn_order
        .iter()
        .position(|candidate| candidate == player)
        .expect("preparation player must be in Turn Order");
    let next_player = state.turn_order.get(index + 1).cloned();
    let mut events = vec![
        GameEvent::PouchPlaced {
            source: player.clone(),
            owner: player.clone(),
            card,
            known_by: vec![player.clone()],
            previous: None,
        },
        GameEvent::InitialPouchChosen {
            player: player.clone(),
            card,
            next_player: next_player.clone(),
        },
    ];
    if next_player.is_none() {
        let first = state
            .turn_order
            .first()
            .expect("validated non-empty Turn Order");
        let remaining = state
            .deck_for(first)
            .expect("Personal Deck exists")
            .iter()
            .copied()
            .filter(|candidate| *candidate != card || first != player)
            .collect();
        events.push(GameEvent::RandomnessRequested {
            request: crate::domain::PendingRandomness {
                request_id: format!("pouch:initial-shuffle:{}", first.as_str()),
                deck: RandomnessDeck::Player(first.clone()),
                continuation_id: "pouch:initial-shuffle".to_string(),
                current_order: remaining,
            },
        });
    }
    Ok(events)
}

pub(crate) fn after_randomness_events(
    state: &GameState,
    continuation_id: &str,
    resolved_deck: &RandomnessDeck,
) -> GameResult<Vec<GameEvent>> {
    if continuation_id != "pouch:initial-shuffle" {
        return Ok(Vec::new());
    }
    let next_index = match resolved_deck {
        RandomnessDeck::Player(player) => state
            .turn_order
            .iter()
            .position(|candidate| candidate == player)
            .map(|index| index + 1)
            .unwrap_or(state.turn_order.len()),
        RandomnessDeck::Shared => state.turn_order.len(),
    };
    if let Some(player) = state.turn_order.get(next_index) {
        let order = state
            .deck_for(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
            .to_vec();
        return Ok(vec![GameEvent::RandomnessRequested {
            request: crate::domain::PendingRandomness {
                request_id: format!("pouch:initial-shuffle:{}", player.as_str()),
                deck: RandomnessDeck::Player(player.clone()),
                continuation_id: "pouch:initial-shuffle".to_string(),
                current_order: order,
            },
        }]);
    }
    let mut events = Vec::new();
    for (index, player) in state.turn_order.iter().enumerate() {
        let count = if index == 0 { 4 } else { 5 };
        let cards = state
            .deck_for(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
            .iter()
            .take(count)
            .copied()
            .collect();
        events.push(GameEvent::CardsDealt {
            player: player.clone(),
            cards,
        });
    }
    events.push(GameEvent::GamePreparationCompleted);
    Ok(events)
}

#[allow(clippy::too_many_arguments)]
fn resolve_owned_pouch(
    state: &GameState,
    player: &PlayerId,
    strategy: SecretStrategy,
    target_player: Option<&PlayerId>,
    star: Option<StarKind>,
    break_star: bool,
    discard_card: Option<CardInstanceId>,
    deck_cards: &[CardInstanceId],
    discard_cards: &[CardInstanceId],
) -> GameResult<Vec<GameEvent>> {
    if !state.has_rule_module(POUCH_MODULE_ID) {
        return Err(GameError::Validation(ValidationError::PouchRuleDisabled));
    }
    if !matches!(state.status, GameStatus::InProgress)
        || state.current_player() != Some(player)
        || state.phase != crate::domain::Phase::Main
    {
        return Err(GameError::Validation(
            ValidationError::GamePreparationInProgress,
        ));
    }
    let pouch = state.pouch_for(player).ok_or_else(|| {
        GameError::Validation(ValidationError::NoPouch {
            player: player.clone(),
        })
    })?;
    let definition = state.card_def(pouch.card).ok_or(GameError::Validation(
        ValidationError::MissingCardInstanceDefinition(pouch.card),
    ))?;
    if !strategy_matches(strategy, definition.element, definition.level) {
        return Err(GameError::Validation(
            ValidationError::SecretStrategyConditionMismatch,
        ));
    }
    let mut events = vec![GameEvent::PouchRevealed {
        player: player.clone(),
        owner: Some(player.clone()),
        card: pouch.card,
        strategy,
    }];
    events.extend(strategy_events(
        state,
        player,
        pouch.card,
        strategy,
        target_player,
        star,
        break_star,
        discard_card,
        deck_cards,
        discard_cards,
    )?);
    events.push(GameEvent::PouchConsumed {
        owner: Some(player.clone()),
        card: pouch.card,
    });
    Ok(events)
}

pub(crate) fn strategy_matches(strategy: SecretStrategy, element: Element, level: u32) -> bool {
    match strategy {
        SecretStrategy::GoldenCicada => element == Element::Metal,
        SecretStrategy::StealTheBeam => element == Element::Wood,
        SecretStrategy::MuddyWaters => element == Element::Water,
        SecretStrategy::WatchTheFire => element == Element::Fire,
        SecretStrategy::LureTheTigerAway => element == Element::Earth,
        SecretStrategy::ReturnSoul => level == 1,
        SecretStrategy::SheepStealing => level == 2,
        SecretStrategy::DarkCrossing => level == 3,
        SecretStrategy::DeceiveHeaven => level == 4,
        SecretStrategy::Retreat => level == 5,
    }
}

pub(crate) fn chain_events(
    state: &GameState,
    player: &PlayerId,
    targets: &[crate::domain::TargetDecl],
) -> GameResult<Vec<GameEvent>> {
    let owner = targets
        .iter()
        .find_map(|target| match target {
            crate::domain::TargetDecl::Player(player) => Some(player.clone()),
            _ => None,
        })
        .unwrap_or_else(|| player.clone());
    let player_team = state
        .players
        .iter()
        .find(|candidate| &candidate.id == player)
        .map(|candidate| &candidate.team);
    let owner_team = state
        .players
        .iter()
        .find(|candidate| candidate.id == owner)
        .map(|candidate| &candidate.team);
    if player_team.is_none() || player_team != owner_team {
        return Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ));
    }
    let pouch_card = targets
        .iter()
        .find_map(|target| match target {
            crate::domain::TargetDecl::FormationRole { role, card } if role == "pouch" => {
                Some(*card)
            }
            _ => None,
        })
        .ok_or(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ))?;
    let trigger_card = targets.iter().find_map(|target| match target {
        crate::domain::TargetDecl::FormationRole { role, card } if role == "trigger" => Some(*card),
        _ => None,
    });
    let deck = state
        .deck_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if !deck.contains(&pouch_card)
        || trigger_card.is_some_and(|card| !deck.contains(&card) || card == pouch_card)
    {
        return Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ));
    }
    if let Some(trigger) = trigger_card {
        let pouch_def = state.card_def(pouch_card).ok_or(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ))?;
        let trigger_def = state.card_def(trigger).ok_or(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ))?;
        if pouch_def.element == trigger_def.element || pouch_def.level == trigger_def.level {
            return Err(GameError::Validation(
                ValidationError::SecretStrategyInputInvalid,
            ));
        }
    }
    let previous = state.pouch_for(&owner).map(|pouch| pouch.card);
    let mut known_by = vec![player.clone()];
    if owner != *player {
        known_by.push(owner.clone());
    }
    let mut events = vec![GameEvent::PouchPlaced {
        source: player.clone(),
        owner: owner.clone(),
        card: pouch_card,
        known_by,
        previous,
    }];
    if let Some(source_card) = trigger_card {
        let strategy = targets
            .iter()
            .find_map(|target| match target {
                crate::domain::TargetDecl::SecretStrategy(strategy) => Some(*strategy),
                _ => None,
            })
            .ok_or(GameError::Validation(
                ValidationError::SecretStrategyInputInvalid,
            ))?;
        let definition = state.card_def(source_card).ok_or(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ))?;
        if !strategy_matches(strategy, definition.element, definition.level) {
            return Err(GameError::Validation(
                ValidationError::SecretStrategyConditionMismatch,
            ));
        }
        let mut projected = state.clone();
        crate::rules::projection::apply_event(&mut projected, &events[0]);
        let (
            strategy_target,
            strategy_star,
            strategy_break_star,
            strategy_discard_card,
            strategy_deck_cards,
            strategy_discard_cards,
        ) = targets
            .iter()
            .find_map(|target| match target {
                crate::domain::TargetDecl::SecretStrategyOptions {
                    target_player,
                    star,
                    break_star,
                    discard_card,
                    deck_cards,
                    discard_cards,
                } => Some((
                    target_player.as_ref(),
                    *star,
                    *break_star,
                    *discard_card,
                    deck_cards.as_slice(),
                    discard_cards.as_slice(),
                )),
                _ => None,
            })
            .unwrap_or((None, None, false, None, &[], &[]));
        events.push(GameEvent::PouchRevealed {
            player: player.clone(),
            owner: None,
            card: source_card,
            strategy,
        });
        events.extend(strategy_events(
            &projected,
            player,
            source_card,
            strategy,
            strategy_target.or(Some(&owner)),
            strategy_star,
            strategy_break_star,
            strategy_discard_card,
            strategy_deck_cards,
            strategy_discard_cards,
        )?);
        events.push(GameEvent::PouchConsumed {
            owner: None,
            card: source_card,
        });
    }
    Ok(events)
}

#[allow(clippy::too_many_arguments)]
fn strategy_events(
    state: &GameState,
    player: &PlayerId,
    source_card: CardInstanceId,
    strategy: SecretStrategy,
    target_player: Option<&PlayerId>,
    star: Option<StarKind>,
    break_star: bool,
    discard_card: Option<CardInstanceId>,
    deck_cards: &[CardInstanceId],
    discard_cards: &[CardInstanceId],
) -> GameResult<Vec<GameEvent>> {
    let status = |kind: &str, owner: PlayerId, duration: StatusDuration| GameEvent::StatusAdded {
        status: StatusEffect {
            id: format!("{kind}:{}:{}", owner.as_str(), state.turn_number),
            owner: StatusOwner::Player(owner),
            kind: kind.to_string(),
            value: None,
            duration,
        },
    };
    Ok(match strategy {
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
        SecretStrategy::LureTheTigerAway => {
            let target = target_player.ok_or(GameError::Validation(
                ValidationError::SecretStrategyInputInvalid,
            ))?;
            let mut events = Vec::new();
            if !player_is_protected(state, target) {
                events.push(status(
                    LURE_PLAYER_STATUS,
                    target.clone(),
                    StatusDuration::UntilTurnEnd {
                        player: target.clone(),
                    },
                ));
            }
            events.push(status(
                LURE_SPIRIT_STATUS,
                target.clone(),
                StatusDuration::UntilTurnEnd {
                    player: target.clone(),
                },
            ));
            events
        }
        SecretStrategy::ReturnSoul => {
            let element = state
                .card_element(source_card)
                .ok_or(GameError::Validation(
                    ValidationError::SecretStrategyInputInvalid,
                ))?;
            let spirit = spirit_for_element(element);
            let previous = state.spirit_for(player);
            let lure_blocks_gain = has_status(state, player, LURE_SPIRIT_STATUS);
            let power = if lure_blocks_gain {
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
        SecretStrategy::SheepStealing => {
            sheep_stealing_events(state, player, source_card, deck_cards, discard_cards)?
        }
        SecretStrategy::DarkCrossing => {
            if has_status(state, player, "CannotChangeProfession") {
                Vec::new()
            } else {
                let element = state
                    .card_element(source_card)
                    .ok_or(GameError::Validation(
                        ValidationError::SecretStrategyInputInvalid,
                    ))?;
                let profession = profession_for_element(element);
                vec![GameEvent::ProfessionTransformed {
                    player: player.clone(),
                    previous: state.profession_for(player).cloned(),
                    profession,
                    reason: "pouch:dark-crossing".to_string(),
                }]
            }
        }
        SecretStrategy::DeceiveHeaven => {
            let selected = star.ok_or(GameError::Validation(
                ValidationError::SecretStrategyInputInvalid,
            ))?;
            if break_star {
                state
                    .team_stars
                    .iter()
                    .find(|owned| owned.star == selected)
                    .map(|owned| {
                        vec![GameEvent::StarBroken {
                            team: owned.team.clone(),
                            star: selected,
                            reason: StarBreakReason::SecretStrategy,
                            hp_change: None,
                        }]
                    })
                    .unwrap_or_default()
            } else {
                vec![GameEvent::TemporaryStarEffectGranted {
                    effect: TemporaryStarEffect {
                        player: player.clone(),
                        star: selected,
                        applied_on_turn: state.turn_number,
                    },
                }]
            }
        }
        SecretStrategy::Retreat => {
            if let Some(card) = discard_card {
                if !state.hand(player).is_some_and(|hand| hand.contains(&card)) {
                    return Err(GameError::Validation(
                        ValidationError::SecretStrategyInputInvalid,
                    ));
                }
                let element = state.card_element(card).ok_or(GameError::Validation(
                    ValidationError::SecretStrategyInputInvalid,
                ))?;
                vec![
                    GameEvent::CardsMoved {
                        card_moves: vec![CardMoveDelta {
                            card,
                            from: CardZone::Hand(player.clone()),
                            to: discard_zone(state, card),
                        }],
                    },
                    GameEvent::EnvironmentTransferred {
                        player: player.clone(),
                        formation_id: "pouch:retreat".to_string(),
                        from: state.environment,
                        to: element,
                    },
                ]
            } else {
                state
                    .environment
                    .map(|environment| {
                        vec![GameEvent::EnvironmentCleared {
                            player: player.clone(),
                            formation_id: "pouch:retreat".to_string(),
                            environment,
                            hp_changes: Vec::new(),
                        }]
                    })
                    .unwrap_or_default()
            }
        }
    })
}

fn sheep_stealing_events(
    state: &GameState,
    player: &PlayerId,
    source_card: CardInstanceId,
    deck_cards: &[CardInstanceId],
    discard_cards: &[CardInstanceId],
) -> GameResult<Vec<GameEvent>> {
    if deck_cards.len() != 2
        || discard_cards.len() != 2
        || deck_cards[0] == deck_cards[1]
        || discard_cards[0] == discard_cards[1]
        || deck_cards.contains(&source_card)
        || !deck_cards.iter().all(|card| {
            state
                .deck_for(player)
                .is_some_and(|deck| deck.contains(card))
        })
        || !discard_cards.iter().all(|card| {
            state
                .discard_for(player)
                .is_some_and(|discard| discard.contains(card))
        })
    {
        return Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ));
    }
    let mut moves = deck_cards
        .iter()
        .map(|card| CardMoveDelta {
            card: *card,
            from: CardZone::PlayerDeckTop(player.clone()),
            to: discard_zone(state, *card),
        })
        .collect::<Vec<_>>();
    moves.extend(discard_cards.iter().map(|card| CardMoveDelta {
        card: *card,
        from: discard_zone(state, *card),
        to: CardZone::PlayerDeckTop(player.clone()),
    }));
    let moved = GameEvent::CardsMoved { card_moves: moves };
    let mut projected = state.clone();
    crate::rules::projection::apply_event(&mut projected, &moved);
    let order = projected
        .deck_for(player)
        .expect("validated Player Deck")
        .to_vec();
    Ok(vec![
        moved,
        GameEvent::RandomnessRequested {
            request: crate::domain::PendingRandomness {
                request_id: format!(
                    "pouch:sheep-stealing:{}:{}",
                    player.as_str(),
                    state.turn_number
                ),
                deck: RandomnessDeck::Player(player.clone()),
                continuation_id: "pouch:sheep-stealing".to_string(),
                current_order: order,
            },
        },
    ])
}

pub(crate) fn has_status(state: &GameState, player: &PlayerId, kind: &str) -> bool {
    state.statuses.iter().any(|status| {
        status.kind == kind
            && matches!(&status.owner, StatusOwner::Player(owner) if owner == player)
    })
}

pub(crate) fn player_is_protected(state: &GameState, player: &PlayerId) -> bool {
    has_status(state, player, GOLDEN_CICADA_STATUS)
}

pub(crate) fn spirit_is_suppressed(state: &GameState, player: &PlayerId) -> bool {
    has_status(state, player, LURE_SPIRIT_STATUS)
}

pub(crate) fn profession_is_suppressed(state: &GameState, player: &PlayerId) -> bool {
    has_status(state, player, LURE_PLAYER_STATUS)
}

pub(crate) fn suppress_watch_fire_formation_hp_changes(
    state: &GameState,
    player: &PlayerId,
    events: &mut [GameEvent],
) {
    if !has_status(state, player, WATCH_FIRE_STATUS) {
        return;
    }
    for event in events {
        match event {
            GameEvent::AttackResolved {
                hp_change,
                shield_change,
                ..
            } => {
                prevent_hp_change(hp_change);
                *shield_change = None;
            }
            GameEvent::EnvironmentCleared { hp_changes, .. }
            | GameEvent::VoidSpiritShatteringResolved { hp_changes, .. } => {
                hp_changes.clear();
            }
            GameEvent::StarBroken { hp_change, .. } => {
                *hp_change = None;
            }
            GameEvent::VoidReversionResolved { hp_change, .. }
            | GameEvent::HpChanged { change: hp_change }
            | GameEvent::DiscardRetrieved { hp_change, .. } => {
                prevent_hp_change(hp_change);
            }
            _ => {}
        }
    }
}

fn prevent_hp_change(change: &mut crate::domain::HpChangeDelta) {
    change.delta = 0;
    change.new_hp = change.old_hp;
    change.effective_delta = 0;
}

fn spirit_for_element(element: Element) -> SpiritKind {
    match element {
        Element::Metal => SpiritKind::Metal,
        Element::Wood => SpiritKind::Wood,
        Element::Water => SpiritKind::Water,
        Element::Fire => SpiritKind::Fire,
        Element::Earth => SpiritKind::Earth,
    }
}

fn profession_for_element(element: Element) -> ProfessionId {
    ProfessionId::new(match element {
        Element::Metal => crate::rules::hero::WARRIOR_ID,
        Element::Wood => crate::rules::hero::SEEKER_ID,
        Element::Water => crate::rules::hero::MESMER_ID,
        Element::Fire => crate::rules::hero::MAGE_ID,
        Element::Earth => crate::rules::hero::WINDWALKER_ID,
    })
}

fn discard_zone(state: &GameState, card: CardInstanceId) -> CardZone {
    match state.card_origin(card) {
        Some(CardOrigin::Player(player)) => CardZone::PlayerDiscard(player.clone()),
        _ => CardZone::Discard,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        FIVE_DIRECTIONS_LEGEND_MODULE_ID, HERO_SCHOOLS_MODULE_ID, HpChangeDelta,
        PERSONAL_DECK_MODULE_ID, PlayerProfession, RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID,
        TeamStar, TrustedRandomnessAnswer,
    };
    use crate::rules::{AttackCategory, OfficialRules, PointFormula};

    fn setup() -> crate::domain::GameSetup {
        let rules = OfficialRules::new();
        let players =
            crate::domain::GameSetup::two_player(PlayerId::new("alice"), PlayerId::new("bob"), 200)
                .players;
        let order = vec![PlayerId::new("alice"), PlayerId::new("bob")];
        let decks = order
            .iter()
            .cloned()
            .map(|player| rules.preconstructed_deck(player))
            .collect();
        rules
            .configure_game_with_decks(
                players,
                order,
                [
                    PERSONAL_DECK_MODULE_ID,
                    STAR_MODULE_ID,
                    FIVE_DIRECTIONS_LEGEND_MODULE_ID,
                    HERO_SCHOOLS_MODULE_ID,
                    SPIRIT_MODULE_ID,
                    POUCH_MODULE_ID,
                ]
                .into_iter()
                .map(RuleModuleId::new)
                .collect(),
                decks,
            )
            .unwrap()
    }

    fn apply(state: &mut GameState, events: &[GameEvent]) {
        for event in events {
            crate::rules::projection::apply_event(state, event);
        }
    }

    #[test]
    fn initial_pouches_are_chosen_before_each_personal_deck_is_shuffled_and_dealt() {
        let setup = setup();
        let rules = OfficialRules::new();
        let opening = rules.start_game(&setup, Vec::new()).unwrap();
        let mut state = GameState::from_setup(&setup);
        apply(&mut state, &opening);

        let alice_card = state.deck_for(&PlayerId::new("alice")).unwrap()[0];
        let alice_events = rules
            .decide_command(
                &state,
                Command::ChooseInitialPouch {
                    player: PlayerId::new("alice"),
                    card: alice_card,
                },
            )
            .unwrap();
        apply(&mut state, &alice_events);
        assert!(matches!(
            state.status,
            GameStatus::Preparing {
                stage: GamePreparationStage::InitialPouchSelection { ref player }
            } if player == &PlayerId::new("bob")
        ));

        let bob_card = state.deck_for(&PlayerId::new("bob")).unwrap()[0];
        let bob_events = rules
            .decide_command(
                &state,
                Command::ChooseInitialPouch {
                    player: PlayerId::new("bob"),
                    card: bob_card,
                },
            )
            .unwrap();
        apply(&mut state, &bob_events);
        assert_eq!(
            state.pouch_for(&PlayerId::new("alice")).unwrap().card,
            alice_card
        );
        assert_eq!(
            state.pouch_for(&PlayerId::new("bob")).unwrap().card,
            bob_card
        );

        for player in [PlayerId::new("alice"), PlayerId::new("bob")] {
            let request = state.pending_randomness.clone().unwrap();
            assert_eq!(request.deck, RandomnessDeck::Player(player));
            let mut order = request.current_order.clone();
            order.reverse();
            let events = crate::application::resolve_trusted_randomness(
                &state,
                &TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order: order,
                },
            )
            .unwrap();
            apply(&mut state, &events);
        }

        assert_eq!(state.status, GameStatus::InProgress);
        assert_eq!(state.hand(&PlayerId::new("alice")).unwrap().len(), 4);
        assert_eq!(state.hand(&PlayerId::new("bob")).unwrap().len(), 5);
        assert_eq!(state.deck_for(&PlayerId::new("alice")).unwrap().len(), 55);
        assert_eq!(state.deck_for(&PlayerId::new("bob")).unwrap().len(), 54);
    }

    #[test]
    fn steal_the_beam_snapshots_only_cards_already_in_hand() {
        let mut state = GameState::from_setup(&setup());
        state.status = GameStatus::InProgress;
        state.phase = crate::domain::Phase::Main;
        let player = PlayerId::new("alice");
        let cards = state
            .card_instances
            .iter()
            .filter(|card| matches!(&card.origin, CardOrigin::Player(owner) if owner == &player))
            .take(3)
            .map(|card| card.instance)
            .collect::<Vec<_>>();
        state
            .hand_mut(&player)
            .unwrap()
            .extend(cards[..2].iter().copied());
        state.pouches.push(crate::domain::PlayerPouch {
            owner: player.clone(),
            card: cards[2],
            known_by: vec![player.clone()],
        });
        let definition = state.card_def(cards[2]).unwrap().clone();
        let strategy = match definition.element {
            Element::Wood => SecretStrategy::StealTheBeam,
            _ => {
                let wood = state
                    .card_instances
                    .iter()
                    .find(|card| {
                        matches!(&card.origin, CardOrigin::Player(owner) if owner == &player)
                            && state.card_def(card.instance).unwrap().element == Element::Wood
                            && !cards[..2].contains(&card.instance)
                    })
                    .unwrap()
                    .instance;
                state.pouches[0].card = wood;
                SecretStrategy::StealTheBeam
            }
        };
        let events =
            resolve_owned_pouch(&state, &player, strategy, None, None, false, None, &[], &[])
                .unwrap();
        apply(&mut state, &events);
        assert_eq!(state.pouch_level_bonuses[0].cards, cards[..2]);
    }

    #[test]
    fn pouch_command_serializes_multiword_fields_as_camel_case() {
        let command = Command::TriggerSecretStrategy {
            player: PlayerId::new("alice"),
            strategy: SecretStrategy::DeceiveHeaven,
            target_player: Some(PlayerId::new("bob")),
            star: Some(StarKind::Fire),
            break_star: true,
            discard_card: Some(CardInstanceId::new(7)),
            deck_cards: vec![CardInstanceId::new(8)],
            discard_cards: vec![CardInstanceId::new(9)],
        };
        let json = serde_json::to_value(command).unwrap();
        assert!(json.get("TriggerSecretStrategy").is_some());
    }

    #[test]
    fn pouch_requires_personal_deck_and_spirit() {
        let rules = OfficialRules::new();
        let base =
            crate::domain::GameSetup::two_player(PlayerId::new("alice"), PlayerId::new("bob"), 200);
        let error = rules
            .configure_game(
                base.players,
                base.turn_order,
                vec![RuleModuleId::new(POUCH_MODULE_ID)],
            )
            .unwrap_err();
        assert!(matches!(
            error,
            GameError::Validation(ValidationError::MissingRuleModuleDependencies {
                module,
                required,
            }) if module.as_str() == POUCH_MODULE_ID
                && required.iter().any(|module| module.as_str() == PERSONAL_DECK_MODULE_ID)
                && required.iter().any(|module| module.as_str() == SPIRIT_MODULE_ID)
        ));
    }

    #[test]
    fn strategy_catalog_resolves_typed_cross_module_effects() {
        let mut state = GameState::from_setup(&setup());
        state.status = GameStatus::InProgress;
        state.phase = crate::domain::Phase::Main;
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let source = state
            .card_instances
            .iter()
            .find(|card| matches!(&card.origin, CardOrigin::Player(owner) if owner == &alice))
            .unwrap()
            .instance;

        let lure = strategy_events(
            &state,
            &alice,
            source,
            SecretStrategy::LureTheTigerAway,
            Some(&bob),
            None,
            false,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert_eq!(
            lure.iter()
                .filter(|event| matches!(event, GameEvent::StatusAdded { .. }))
                .count(),
            2
        );

        state.spirits.push(crate::domain::PlayerSpirit {
            player: alice.clone(),
            spirit: SpiritKind::Wood,
            power: 4,
        });
        state.statuses.push(StatusEffect {
            id: "lure".to_string(),
            owner: StatusOwner::Player(alice.clone()),
            kind: LURE_SPIRIT_STATUS.to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEnd {
                player: alice.clone(),
            },
        });
        let revived = strategy_events(
            &state,
            &alice,
            source,
            SecretStrategy::ReturnSoul,
            None,
            None,
            false,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert!(matches!(
            revived.as_slice(),
            [GameEvent::SpiritRevived { power: 1, .. }]
        ));

        let temporary = strategy_events(
            &state,
            &alice,
            source,
            SecretStrategy::DeceiveHeaven,
            None,
            Some(StarKind::Fire),
            false,
            None,
            &[],
            &[],
        )
        .unwrap();
        assert!(matches!(
            temporary.as_slice(),
            [GameEvent::TemporaryStarEffectGranted {
                effect: TemporaryStarEffect {
                    star: StarKind::Fire,
                    ..
                }
            }]
        ));
    }

    #[test]
    fn golden_cicada_blocks_only_the_player_half_of_lure() {
        let mut state = GameState::from_setup(&setup());
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        state.statuses.push(StatusEffect {
            id: "gold".to_string(),
            owner: StatusOwner::Player(bob.clone()),
            kind: GOLDEN_CICADA_STATUS.to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEnd {
                player: bob.clone(),
            },
        });
        let source = state.card_instances[0].instance;

        let events = strategy_events(
            &state,
            &alice,
            source,
            SecretStrategy::LureTheTigerAway,
            Some(&bob),
            None,
            false,
            None,
            &[],
            &[],
        )
        .unwrap();

        assert_eq!(events.len(), 1);
        assert!(matches!(
            &events[0],
            GameEvent::StatusAdded { status }
                if status.kind == LURE_SPIRIT_STATUS
                    && status.owner == StatusOwner::Player(bob)
        ));
    }

    #[test]
    fn watch_fire_removes_formation_hp_changes_without_removing_other_effects() {
        let mut state = GameState::from_setup(&setup());
        let alice = PlayerId::new("alice");
        let team = state.players[0].team.clone();
        state.statuses.push(StatusEffect {
            id: "watch".to_string(),
            owner: StatusOwner::Player(alice.clone()),
            kind: WATCH_FIRE_STATUS.to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEnd {
                player: alice.clone(),
            },
        });
        let change = HpChangeDelta {
            team,
            old_hp: 200,
            delta: -20,
            new_hp: 180,
            effective_delta: -20,
        };
        let mut events = vec![
            GameEvent::HpChanged {
                change: change.clone(),
            },
            GameEvent::EnvironmentCleared {
                player: alice.clone(),
                formation_id: "test".to_string(),
                environment: Element::Fire,
                hp_changes: vec![change],
            },
        ];

        suppress_watch_fire_formation_hp_changes(&state, &alice, &mut events);

        assert!(matches!(
            &events[0],
            GameEvent::HpChanged { change }
                if change.delta == 0 && change.new_hp == change.old_hp
        ));
        assert!(matches!(
            &events[1],
            GameEvent::EnvironmentCleared { hp_changes, .. } if hp_changes.is_empty()
        ));
    }

    #[test]
    fn lure_lets_an_activated_profession_ability_pay_its_cost_but_suppresses_effects() {
        let rules = OfficialRules::new();
        let mut state = GameState::from_setup(&setup());
        let alice = PlayerId::new("alice");
        state.status = GameStatus::InProgress;
        state.phase = crate::domain::Phase::Main;
        state.professions.push(PlayerProfession {
            player: alice.clone(),
            profession: ProfessionId::new(crate::rules::hero::IMMORTAL_ID),
        });
        state.statuses.push(StatusEffect {
            id: "lure".to_string(),
            owner: StatusOwner::Player(alice.clone()),
            kind: LURE_PLAYER_STATUS.to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEnd {
                player: alice.clone(),
            },
        });
        let card = state
            .card_instances
            .iter()
            .find(|card| {
                matches!(&card.origin, CardOrigin::Player(owner) if owner == &alice)
                    && state
                        .card_def(card.instance)
                        .is_some_and(|card| card.level >= 4)
            })
            .unwrap()
            .instance;
        state.hand_mut(&alice).unwrap().push(card);

        let events = rules
            .decide_command(
                &state,
                Command::ActivateProfessionAbility {
                    player: alice,
                    ability_id: "meditation".to_string(),
                    cards: vec![card],
                    target_card: None,
                    declared_element: None,
                    declared_level: None,
                },
            )
            .unwrap();

        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0],
            GameEvent::ProfessionAbilityActivated { .. }
        ));
        assert!(matches!(events[1], GameEvent::CardsMoved { .. }));
    }

    #[test]
    fn temporary_star_formation_never_breaks_a_different_owned_star() {
        let mut state = GameState::from_setup(&setup());
        let alice = PlayerId::new("alice");
        state.status = GameStatus::InProgress;
        state.phase = crate::domain::Phase::Main;
        state.team_stars.push(TeamStar {
            team: state.players[0].team.clone(),
            star: StarKind::Wood,
        });
        state.temporary_star_effects.push(TemporaryStarEffect {
            player: alice.clone(),
            star: StarKind::Fire,
            applied_on_turn: state.turn_number,
        });

        let events = crate::rules::base::attack_resolution::resolve(
            &state,
            crate::rules::base::attack_resolution::AttackRequest {
                attacker: alice,
                formation_id: "yinghuo-heaven-blazing".to_string(),
                category: AttackCategory::Elemental(Element::Fire),
                point_formula: PointFormula::Fixed(10),
                used_cards: Vec::new(),
                damage_prevented: true,
                split_attack_damage: false,
                mode: crate::rules::base::attack_resolution::AttackResolutionMode::FormationUse,
            },
        )
        .unwrap();

        assert!(
            events
                .iter()
                .any(|event| matches!(event, GameEvent::TurnDrawBonusChanged { .. }))
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, GameEvent::StarBroken { .. }))
        );
    }

    #[test]
    fn chain_forwards_typed_strategy_options_after_placing_the_new_pouch() {
        let mut state = GameState::from_setup(&setup());
        let alice = PlayerId::new("alice");
        state.status = GameStatus::InProgress;
        state.phase = crate::domain::Phase::Main;
        let alice_cards = state
            .card_instances
            .iter()
            .filter(|card| matches!(&card.origin, CardOrigin::Player(owner) if owner == &alice))
            .map(|card| card.instance)
            .collect::<Vec<_>>();
        state
            .deck_for_mut(&alice)
            .unwrap()
            .extend(alice_cards.iter().copied());
        let trigger = alice_cards
            .iter()
            .copied()
            .find(|card| state.card_def(*card).is_some_and(|card| card.level == 4))
            .unwrap();
        let trigger_definition = state.card_def(trigger).unwrap().clone();
        let pouch = alice_cards
            .iter()
            .copied()
            .find(|card| {
                state.card_def(*card).is_some_and(|card| {
                    card.level != trigger_definition.level
                        && card.element != trigger_definition.element
                })
            })
            .unwrap();

        let events = chain_events(
            &state,
            &alice,
            &[
                crate::domain::TargetDecl::Player(alice.clone()),
                crate::domain::TargetDecl::FormationRole {
                    role: "pouch".to_string(),
                    card: pouch,
                },
                crate::domain::TargetDecl::FormationRole {
                    role: "trigger".to_string(),
                    card: trigger,
                },
                crate::domain::TargetDecl::SecretStrategy(SecretStrategy::DeceiveHeaven),
                crate::domain::TargetDecl::SecretStrategyOptions {
                    target_player: None,
                    star: Some(StarKind::Fire),
                    break_star: false,
                    discard_card: None,
                    deck_cards: Vec::new(),
                    discard_cards: Vec::new(),
                },
            ],
        )
        .unwrap();

        assert!(matches!(events[0], GameEvent::PouchPlaced { .. }));
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::TemporaryStarEffectGranted {
                effect: TemporaryStarEffect {
                    star: StarKind::Fire,
                    ..
                }
            }
        )));
        assert!(matches!(
            events.last(),
            Some(GameEvent::PouchConsumed { owner: None, .. })
        ));
    }

    #[test]
    fn pouch_state_and_placement_events_are_private_to_the_recorded_viewers() {
        let mut state = GameState::from_setup(&setup());
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let card = state.card_instances[0].instance;
        state.pouches.push(crate::domain::PlayerPouch {
            owner: alice.clone(),
            card,
            known_by: vec![alice.clone()],
        });

        let alice_view = crate::public_view::state_for(
            &state,
            crate::public_view::Viewer::Player(alice.clone()),
        );
        let bob_view =
            crate::public_view::state_for(&state, crate::public_view::Viewer::Player(bob.clone()));
        assert_eq!(alice_view.pouches[0].card, Some(card));
        assert_eq!(bob_view.pouches[0].card, None);

        let event = GameEvent::PouchPlaced {
            source: alice.clone(),
            owner: alice.clone(),
            card,
            known_by: vec![alice.clone()],
            previous: None,
        };
        assert!(matches!(
            crate::public_view::event_for(
                &event,
                crate::public_view::Viewer::Player(alice)
            ),
            crate::public_view::PublicGameEvent::PouchPlaced {
                card: Some(visible),
                ..
            } if visible == card
        ));
        assert!(matches!(
            crate::public_view::event_for(&event, crate::public_view::Viewer::Player(bob)),
            crate::public_view::PublicGameEvent::PouchPlaced { card: None, .. }
        ));
    }
}
