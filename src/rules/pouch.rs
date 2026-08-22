use crate::domain::{
    CardInstanceId, CardMoveDelta, CardOrigin, CardZone, Command, Element, GameError, GameEvent,
    GamePreparationStage, GameResult, GameState, GameStatus, POUCH_MODULE_ID, PlayerCardPile,
    PlayerId, PouchLevelBonus, PouchRandomnessContinuation, ProfessionId, RandomnessContinuation,
    RandomnessDeck, SecretStrategy, SpiritKind, StarBreakReason, StarKind, StatusDuration,
    StatusEffect, StatusOwner, TemporaryStarEffect, ValidationError,
};
use crate::rules::{
    BaseFormationSpec, ConsequenceCertainty, EffectDef, EffectPlan, FollowUpChoice,
    FormationCategory, FormationDef, FormationEffect, FormationPattern, PlayerFacingActionDetail,
    PointFormula, RuleConsequence, SpellPlanDef,
};
use serde::Serialize;

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
                player_facing_effect: FormationEffect::BeginChainChoice,
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
    if !matches!(
        state.status,
        GameStatus::Preparing {
            stage: GamePreparationStage::InitialPouchSelection,
        }
    ) {
        return Err(GameError::Validation(
            ValidationError::InitialPouchSelectionUnavailable,
        ));
    }
    if !state.turn_order.contains(player) {
        return Err(GameError::Validation(ValidationError::UnknownPlayer(
            player.clone(),
        )));
    }
    if state.pouch_for(player).is_some() {
        return Err(GameError::Validation(
            ValidationError::InitialPouchAlreadyChosen {
                player: player.clone(),
            },
        ));
    }
    if !state
        .deck_for(player)
        .is_some_and(|deck| deck.contains(&card))
    {
        return Err(GameError::Validation(ValidationError::InvalidInitialPouch(
            card,
        )));
    }
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
        },
    ];
    if state
        .turn_order
        .iter()
        .all(|candidate| candidate == player || state.pouch_for(candidate).is_some())
    {
        events.push(GameEvent::InitialPouchSelectionCompleted);
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
                operation: crate::domain::RandomnessOperation::DeckShuffle {
                    deck: RandomnessDeck::Player(first.clone()),
                },
                continuation: RandomnessContinuation::Pouch(
                    PouchRandomnessContinuation::InitialShuffle,
                ),
                current_order: remaining,
            },
        });
    }
    Ok(events)
}

pub(crate) fn after_initial_shuffle_randomness_events(
    state: &GameState,
    resolved_deck: &RandomnessDeck,
) -> GameResult<Vec<GameEvent>> {
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
                operation: crate::domain::RandomnessOperation::DeckShuffle {
                    deck: RandomnessDeck::Player(player.clone()),
                },
                continuation: RandomnessContinuation::Pouch(
                    PouchRandomnessContinuation::InitialShuffle,
                ),
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
        || state.phase != crate::domain::Phase::ActiveEffects
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
    if !strategy_matches(strategy, definition.element, definition.level.value()) {
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
    if strategy != SecretStrategy::SheepStealing {
        events.push(GameEvent::PouchConsumed {
            owner: Some(player.clone()),
            card: pouch.card,
        });
    }
    Ok(events)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SecretStrategyInputRequirement {
    None,
    TargetPlayer,
    DeckDiscardSwap,
    Star,
    Retreat,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SecretStrategyCardOption {
    pub strategy: SecretStrategy,
    pub input: SecretStrategyInputRequirement,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SecretStrategyOption {
    pub source_card: CardInstanceId,
    pub strategy: SecretStrategy,
    pub input: SecretStrategyInputRequirement,
    pub target_players: Vec<PlayerId>,
    pub stars: Vec<StarKind>,
    pub break_stars: Vec<StarKind>,
    pub deck_cards: Vec<CardInstanceId>,
    pub discard_cards: Vec<CardInstanceId>,
    pub hand_cards: Vec<CardInstanceId>,
    pub required_card_count: usize,
    pub detail: PlayerFacingActionDetail,
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
        .map(|strategy| SecretStrategyCardOption {
            input: match strategy {
                SecretStrategy::LureTheTigerAway => SecretStrategyInputRequirement::TargetPlayer,
                SecretStrategy::SheepStealing => SecretStrategyInputRequirement::DeckDiscardSwap,
                SecretStrategy::DeceiveHeaven => SecretStrategyInputRequirement::Star,
                SecretStrategy::Retreat => SecretStrategyInputRequirement::Retreat,
                _ => SecretStrategyInputRequirement::None,
            },
            strategy,
        })
        .collect()
}

pub(crate) fn strategy_matches(strategy: SecretStrategy, element: Element, level: u32) -> bool {
    strategy_options_for_card(element, level)
        .iter()
        .any(|option| option.strategy == strategy)
}

fn sheep_deck_card_options(
    state: &GameState,
    player: &PlayerId,
    source_card: CardInstanceId,
) -> Vec<CardInstanceId> {
    state
        .deck_for(player)
        .unwrap_or_default()
        .iter()
        .copied()
        .filter(|card| *card != source_card)
        .collect()
}

fn sheep_return_card_options(
    state: &GameState,
    player: &PlayerId,
    source_card: CardInstanceId,
) -> Vec<CardInstanceId> {
    let mut cards = state.discard_for(player).unwrap_or_default().to_vec();
    cards.extend(
        sheep_deck_card_options(state, player, source_card)
            .into_iter()
            .filter(|card| {
                matches!(state.card_origin(*card), Some(CardOrigin::Player(owner)) if owner == player)
            }),
    );
    cards
}

pub(crate) fn strategy_action_options(
    state: &GameState,
    player: &PlayerId,
    source_card: CardInstanceId,
) -> Vec<SecretStrategyOption> {
    let Some(definition) = state.card_def(source_card) else {
        return Vec::new();
    };
    strategy_options_for_card(definition.element, definition.level.value())
        .into_iter()
        .map(|option| {
            let input = option.input;
            SecretStrategyOption {
                source_card,
                strategy: option.strategy,
                input,
                target_players: matches!(
                    option.input,
                    SecretStrategyInputRequirement::TargetPlayer
                )
                .then(|| state.turn_order.clone())
                .unwrap_or_default(),
                stars: matches!(option.input, SecretStrategyInputRequirement::Star)
                    .then(|| {
                        vec![
                            StarKind::Metal,
                            StarKind::Wood,
                            StarKind::Water,
                            StarKind::Fire,
                            StarKind::Earth,
                        ]
                    })
                    .unwrap_or_default(),
                break_stars: matches!(option.input, SecretStrategyInputRequirement::Star)
                    .then(|| state.team_stars.iter().map(|owned| owned.star).collect())
                    .unwrap_or_default(),
                deck_cards: matches!(
                    option.input,
                    SecretStrategyInputRequirement::DeckDiscardSwap
                )
                .then(|| sheep_deck_card_options(state, player, source_card))
                .unwrap_or_default(),
                discard_cards: matches!(
                    option.input,
                    SecretStrategyInputRequirement::DeckDiscardSwap
                )
                .then(|| sheep_return_card_options(state, player, source_card))
                .unwrap_or_default(),
                hand_cards: matches!(option.input, SecretStrategyInputRequirement::Retreat)
                    .then(|| state.hand(player).unwrap_or_default().to_vec())
                    .unwrap_or_default(),
                required_card_count: matches!(
                    option.input,
                    SecretStrategyInputRequirement::DeckDiscardSwap
                )
                .then_some(2)
                .unwrap_or_default(),
                detail: crate::rules::action_detail::secret_strategy_detail(option.strategy),
            }
        })
        .collect()
}

pub(crate) fn playable_owned_strategy_actions(
    state: &GameState,
    player: &PlayerId,
) -> Vec<SecretStrategyOption> {
    let Some(source_card) = state.pouch_for(player).map(|pouch| pouch.card) else {
        return Vec::new();
    };

    strategy_action_options(state, player, source_card)
        .into_iter()
        .filter(|option| {
            let target_player = option.target_players.first();
            let star = option.stars.first().copied();
            resolve_owned_pouch(
                state,
                player,
                option.strategy,
                target_player,
                star,
                false,
                None,
                &[],
                &[],
            )
            .is_ok()
        })
        .collect()
}

/// Chain 擁有自己的待選擇生命週期。此處說明開始前已知的承諾，但不攜帶
/// Choice ID 或延續。
pub(crate) fn formation_action_detail_consequences(id: &str) -> Option<Vec<RuleConsequence>> {
    (id == CHAIN_ID).then(|| {
        vec![RuleConsequence::FollowUpChoice {
            certainty: ConsequenceCertainty::FollowUp,
            choice: FollowUpChoice::SelectPouchOwnerAndOptionalStrategy,
        }]
    })
}

pub(crate) fn chain_events(
    state: &GameState,
    player: &PlayerId,
    targets: &[crate::domain::TargetDecl],
) -> GameResult<Vec<GameEvent>> {
    if targets.is_empty() {
        let deck = state
            .deck_for(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
        if deck.len() < 2 {
            let discard = state.discard_for(player).unwrap_or_default();
            if !discard.is_empty() {
                return Ok(vec![GameEvent::RandomnessRequested {
                    request: crate::domain::PendingRandomness {
                        request_id: format!(
                            "pouch:chain:recycle:{}:{}",
                            player.as_str(),
                            state.turn_number
                        ),
                        operation: crate::domain::RandomnessOperation::DiscardShuffle {
                            pile: RandomnessDeck::Player(player.clone()),
                            placement: crate::domain::DeckPlacement::Bottom,
                        },
                        continuation: RandomnessContinuation::Pouch(
                            PouchRandomnessContinuation::ChainRecycle,
                        ),
                        current_order: discard.to_vec(),
                    },
                }]);
            }
        }
        if deck.is_empty() {
            return Err(GameError::Validation(
                ValidationError::SecretStrategyInputInvalid,
            ));
        }
        let team = state
            .players
            .iter()
            .find(|candidate| &candidate.id == player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
            .team
            .clone();
        return Ok(vec![crate::rules::pending_choice::request_event(
            state,
            crate::domain::ChoiceRequest {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::Chain {
                    pouch_owners: state
                        .players
                        .iter()
                        .filter(|candidate| candidate.team == team)
                        .map(|candidate| candidate.id.clone())
                        .collect(),
                    deck_cards: deck.to_vec(),
                },
                continuation: crate::domain::ChoiceContinuation::Pouch(
                    crate::domain::PouchChoiceContinuation::Chain,
                ),
            },
        )?]);
    }
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
    let (strategy_target, strategy_star, strategy_break_star, strategy_discard_card) = targets
        .iter()
        .find_map(|target| match target {
            crate::domain::TargetDecl::SecretStrategyOptions {
                target_player,
                star,
                break_star,
                discard_card,
                ..
            } => Some((target_player.clone(), *star, *break_star, *discard_card)),
            _ => None,
        })
        .unwrap_or((None, None, false, None));
    let strategy = if let Some(source_card) = trigger_card {
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
        if !strategy_matches(strategy, definition.element, definition.level.value()) {
            return Err(GameError::Validation(
                ValidationError::SecretStrategyConditionMismatch,
            ));
        }
        Some(strategy)
    } else {
        None
    };
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
        let strategy = strategy.ok_or(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ))?;
        let revealed = GameEvent::PouchRevealed {
            player: player.clone(),
            owner: None,
            card: source_card,
            strategy,
        };
        events.push(revealed);
    }
    let mut projected = state.clone();
    for event in &events {
        crate::rules::projection::apply_event(&mut projected, event);
    }

    if let (Some(source_card), Some(strategy)) = (trigger_card, strategy) {
        let strategy_events = strategy_events(
            &projected,
            player,
            source_card,
            strategy,
            strategy_target.as_ref().or(Some(&owner)),
            strategy_star,
            strategy_break_star,
            strategy_discard_card,
            &[],
            &[],
        )?;
        let has_pending = strategy_events.iter().any(|event| {
            matches!(
                event,
                GameEvent::ChoiceRequested { .. } | GameEvent::RandomnessRequested { .. }
            )
        });
        events.extend(strategy_events);

        // 牽羊會自行建立交換後的 Deck Shuffle，不能再追加一次連環洗牌。
        if strategy == SecretStrategy::SheepStealing || has_pending {
            return Ok(events);
        }
        events.push(GameEvent::PouchConsumed {
            owner: None,
            card: source_card,
        });
    }

    projected = state.clone();
    for event in &events {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    let remainder = projected
        .deck_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
        .to_vec();
    let continuation = PouchRandomnessContinuation::ChainPostSearch {
        player: player.clone(),
    };
    if !remainder.is_empty() {
        events.push(GameEvent::RandomnessRequested {
            request: crate::domain::PendingRandomness {
                request_id: format!(
                    "pouch:chain:post-search:{}:{}",
                    player.as_str(),
                    state.turn_number
                ),
                operation: crate::domain::RandomnessOperation::DeckShuffle {
                    deck: RandomnessDeck::Player(player.clone()),
                },
                continuation: RandomnessContinuation::Pouch(continuation),
                current_order: remainder,
            },
        });
    }
    Ok(events)
}

pub(crate) fn answer_chain_choice(
    state: &GameState,
    player: &PlayerId,
    answer: &crate::domain::ChoiceAnswer,
) -> GameResult<Option<Vec<GameEvent>>> {
    let crate::domain::ChoiceAnswer::Chain {
        pouch_owner,
        pouch_card,
        trigger_card,
        strategy,
        target_player,
        star,
        break_star,
        discard_card,
    } = answer
    else {
        return Ok(None);
    };
    let mut targets = vec![
        crate::domain::TargetDecl::Player(pouch_owner.clone()),
        crate::domain::TargetDecl::FormationRole {
            role: "pouch".to_string(),
            card: *pouch_card,
        },
    ];
    if let (Some(trigger), Some(strategy)) = (trigger_card, strategy) {
        let target_is_player = target_player.as_ref().is_some_and(|target| {
            state
                .players
                .iter()
                .any(|candidate| candidate.id == *target)
        });
        let input_is_valid = match strategy {
            SecretStrategy::LureTheTigerAway => {
                target_is_player && star.is_none() && !*break_star && discard_card.is_none()
            }
            SecretStrategy::DeceiveHeaven => {
                target_player.is_none() && star.is_some() && discard_card.is_none()
            }
            SecretStrategy::Retreat => target_player.is_none() && star.is_none() && !*break_star,
            SecretStrategy::SheepStealing
            | SecretStrategy::GoldenCicada
            | SecretStrategy::StealTheBeam
            | SecretStrategy::MuddyWaters
            | SecretStrategy::WatchTheFire
            | SecretStrategy::ReturnSoul
            | SecretStrategy::DarkCrossing => {
                target_player.is_none() && star.is_none() && !*break_star && discard_card.is_none()
            }
        };
        if !input_is_valid {
            return Err(GameError::Validation(
                ValidationError::SecretStrategyInputInvalid,
            ));
        }
        targets.push(crate::domain::TargetDecl::FormationRole {
            role: "trigger".to_string(),
            card: *trigger,
        });
        targets.push(crate::domain::TargetDecl::SecretStrategy(*strategy));
        targets.push(crate::domain::TargetDecl::SecretStrategyOptions {
            target_player: target_player.clone(),
            star: *star,
            break_star: *break_star,
            discard_card: *discard_card,
            deck_cards: Vec::new(),
            discard_cards: Vec::new(),
        });
    } else if trigger_card.is_some()
        || strategy.is_some()
        || target_player.is_some()
        || star.is_some()
        || *break_star
        || discard_card.is_some()
    {
        return Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ));
    }
    Ok(Some(chain_events(state, player, &targets)?))
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
    _deck_cards: &[CardInstanceId],
    _discard_cards: &[CardInstanceId],
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
        SecretStrategy::SheepStealing => sheep_choice_events(state, player, source_card)?,
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
    let deck = state
        .deck_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if deck.len() < 2 {
        // 具型別的交換選擇只有在 `sheep_choice_events` 確認存在兩張牌堆卡牌後
        // 才會產生。接受該選擇時不要建立第二個回收請求：過期答案必須被拒絕。
        return Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ));
    }
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
    let mut projected = state.clone();
    crate::rules::projection::apply_event(
        &mut projected,
        &GameEvent::CardsMoved {
            card_moves: moves.clone(),
        },
    );
    if !discard_cards.iter().all(|card| {
        projected
            .discard_for(player)
            .is_some_and(|discard| discard.contains(card))
    }) {
        return Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid,
        ));
    }
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
                operation: crate::domain::RandomnessOperation::DeckShuffle {
                    deck: RandomnessDeck::Player(player.clone()),
                },
                continuation: RandomnessContinuation::Pouch(
                    PouchRandomnessContinuation::SheepStealing {
                        source_card,
                        owner: state
                            .pouch_for(player)
                            .filter(|pouch| pouch.card == source_card)
                            .map(|pouch| pouch.owner.clone()),
                    },
                ),
                current_order: order,
            },
        },
    ])
}

fn sheep_choice_events(
    state: &GameState,
    player: &PlayerId,
    source_card: CardInstanceId,
) -> GameResult<Vec<GameEvent>> {
    let deck = state
        .deck_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if deck.len() < 2 {
        let discard = state.discard_for(player).unwrap_or_default();
        if deck.len() + discard.len() < 2 || discard.is_empty() {
            return Err(GameError::Validation(
                ValidationError::SecretStrategyInputInvalid,
            ));
        }
        return Ok(vec![GameEvent::RandomnessRequested {
            request: crate::domain::PendingRandomness {
                request_id: format!(
                    "pouch:sheep-stealing:recycle:{}:{}",
                    player.as_str(),
                    state.turn_number
                ),
                operation: crate::domain::RandomnessOperation::DiscardShuffle {
                    pile: RandomnessDeck::Player(player.clone()),
                    placement: crate::domain::DeckPlacement::Bottom,
                },
                continuation: RandomnessContinuation::Pouch(
                    PouchRandomnessContinuation::SheepStealingRecycle { source_card },
                ),
                current_order: discard.to_vec(),
            },
        }]);
    }
    Ok(vec![crate::rules::pending_choice::request_event(
        state,
        crate::domain::ChoiceRequest {
            player: player.clone(),
            kind: crate::domain::PendingChoiceKind::SheepStealing {
                source_card,
                owner: state
                    .pouch_for(player)
                    .filter(|pouch| pouch.card == source_card)
                    .map(|pouch| pouch.owner.clone()),
                deck_cards: state.deck_for(player).unwrap_or_default().to_vec(),
                discard_cards: state.discard_for(player).unwrap_or_default().to_vec(),
            },
            continuation: crate::domain::ChoiceContinuation::Pouch(
                crate::domain::PouchChoiceContinuation::SheepStealing,
            ),
        },
    )?])
}

pub(crate) fn answer_sheep_choice(
    state: &GameState,
    choice: &crate::domain::PendingChoice,
    player: &PlayerId,
    answer: &crate::domain::ChoiceAnswer,
) -> GameResult<Option<Vec<GameEvent>>> {
    let crate::domain::ChoiceAnswer::SheepStealing {
        deck_cards,
        discard_cards,
    } = answer
    else {
        return Ok(None);
    };
    let crate::domain::PendingChoice {
        kind: crate::domain::PendingChoiceKind::SheepStealing { source_card, .. },
        ..
    } = choice
    else {
        return Ok(None);
    };
    Ok(Some(sheep_stealing_events(
        state,
        player,
        *source_card,
        deck_cards,
        discard_cards,
    )?))
}

pub(crate) fn after_sheep_recycle_randomness_events(
    state: &GameState,
    source_card: CardInstanceId,
) -> GameResult<Vec<GameEvent>> {
    let player = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;
    sheep_choice_events(state, player, source_card)
}

pub(crate) fn after_chain_recycle_randomness_events(
    state: &GameState,
) -> GameResult<Vec<GameEvent>> {
    let player = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;
    chain_events(state, player, &[])
}

pub(crate) fn after_chain_post_search_randomness_events(
    _state: &GameState,
    continuation: &PouchRandomnessContinuation,
) -> GameResult<Vec<GameEvent>> {
    let PouchRandomnessContinuation::ChainPostSearch { player: _ } = continuation else {
        return Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(
                "pouch:chain:missing-post-search-continuation".to_string(),
            ),
        ));
    };

    Ok(Vec::new())
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
    if !has_status(state, player, WATCH_FIRE_STATUS) || player_is_protected(state, player) {
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
        FIVE_DIRECTIONS_LEGEND_MODULE_ID, HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID,
        RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID, TrustedRandomnessAnswer,
    };
    use crate::rules::OfficialRules;

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

    #[test]
    fn initial_pouch_choices_are_independent_and_converge_in_two_and_four_player_games() {
        for setup in [setup(), four_player_setup()] {
            let turn_order = setup.turn_order.clone();
            let forward = complete_initial_pouch_selection(&setup, turn_order.clone());
            let reverse =
                complete_initial_pouch_selection(&setup, turn_order.into_iter().rev().collect());

            assert_eq!(forward.state(), reverse.state());
            assert_eq!(forward.verify_replay().unwrap(), forward.state().clone());
            assert_eq!(reverse.verify_replay().unwrap(), reverse.state().clone());
            assert_eq!(
                forward
                    .events()
                    .iter()
                    .filter(|event| matches!(event, GameEvent::InitialPouchSelectionCompleted))
                    .count(),
                1,
            );
            assert_eq!(
                forward
                    .events()
                    .iter()
                    .filter(
                        |event| matches!(event, GameEvent::RandomnessRequested { request }
                        if request.request_id == "pouch:initial-shuffle:alice")
                    )
                    .count(),
                1,
            );
            assert_eq!(
                forward
                    .events()
                    .iter()
                    .filter_map(|event| match event {
                        GameEvent::RandomnessRequested { request } => match &request.operation {
                            crate::domain::RandomnessOperation::DeckShuffle {
                                deck: RandomnessDeck::Player(player),
                            } if request.request_id.starts_with("pouch:initial-shuffle:") => {
                                Some(player.clone())
                            }
                            _ => None,
                        },
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                setup.turn_order,
            );
            assert_eq!(
                forward
                    .events()
                    .iter()
                    .filter_map(|event| match event {
                        GameEvent::CardsDealt { player, .. } => Some(player.clone()),
                        _ => None,
                    })
                    .collect::<Vec<_>>(),
                setup.turn_order,
            );
        }
    }

    #[test]
    fn initial_pouch_choice_is_immutable_after_a_successful_independent_submission() {
        let setup = setup();
        let mut record = crate::application::GameRecord::start(setup, Vec::new()).unwrap();
        let alice = PlayerId::new("alice");
        let card = record.state().deck_for(&alice).unwrap()[0];
        record
            .handle(Command::ChooseInitialPouch {
                player: alice.clone(),
                card,
            })
            .unwrap();
        let event_count = record.events().len();

        assert_eq!(
            record.handle(Command::ChooseInitialPouch {
                player: alice.clone(),
                card,
            }),
            Err(GameError::Validation(
                ValidationError::InitialPouchAlreadyChosen { player: alice }
            )),
        );
        assert_eq!(record.events().len(), event_count);
    }

    fn four_player_setup() -> crate::domain::GameSetup {
        let rules = OfficialRules::new();
        let players = crate::domain::GameSetup::team_mode(
            crate::domain::TeamId::new("a"),
            vec![PlayerId::new("alice"), PlayerId::new("cara")],
            crate::domain::TeamId::new("b"),
            vec![PlayerId::new("bob"), PlayerId::new("drew")],
            200,
        )
        .players;
        let order = vec![
            PlayerId::new("alice"),
            PlayerId::new("bob"),
            PlayerId::new("cara"),
            PlayerId::new("drew"),
        ];
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

    fn complete_initial_pouch_selection(
        setup: &crate::domain::GameSetup,
        selection_order: Vec<PlayerId>,
    ) -> crate::application::GameRecord {
        let mut record = crate::application::GameRecord::start(setup.clone(), Vec::new()).unwrap();
        for player in selection_order {
            let card = record.state().deck_for(&player).unwrap()[0];
            record
                .handle(Command::ChooseInitialPouch { player, card })
                .unwrap();
        }
        while let Some(request) = record.state().pending_randomness.clone() {
            record
                .resolve_randomness(TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order: request.current_order,
                })
                .unwrap();
        }
        record
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
}
