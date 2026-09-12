use crate::domain::{
    CardInstanceId, CardMoveDelta, CardOrigin, CardZone, ChainPouchDecision, Command, Element,
    GameError, GameEvent, GamePreparationStage, GameResult, GameState, GameStatus, POUCH_MODULE_ID,
    PendingResolution, PlayerCardPile, PlayerId, ProfessionId, RandomnessDeck, SecretStrategy,
    SecretStrategyDecision, SpiritKind, StatusOwner, ValidationError,
};
use crate::rules::{
    BaseFormationSpec, ConsequenceCertainty, EffectDef, EffectPlan, FollowUpChoice,
    FormationCategory, FormationDef, FormationEffect, FormationPattern, PointFormula,
    RuleConsequence, SpellPlanDef,
};

mod secret_strategy;

pub use secret_strategy::SecretStrategyOption;
pub(crate) use secret_strategy::{
    SecretStrategyCardOption, strategy_action_options, strategy_options_for_card, validate_decision,
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
    };
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
        Command::TriggerSecretStrategy { player, decision } => {
            resolve_owned_pouch(state, player, decision).map(Some)
        }
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
    // 先檢查玩家是否已完成選擇，再檢查整體準備階段。最後一位玩家的
    // 選擇會同時推進階段；同一玩家重試時仍應得到穩定的重複選擇錯誤。
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
                current_order: remaining,
            },
            resolution: PendingResolution::PouchInitialShuffle,
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
                current_order: order,
            },
            resolution: PendingResolution::PouchInitialShuffle,
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
        crate::rules::deck_supply::plan(
            state,
            &RandomnessDeck::Player(player.clone()),
            count,
            crate::domain::DeckPlacement::Bottom,
        )?;
        events.push(GameEvent::CardsDealt {
            player: player.clone(),
            cards,
        });
    }
    events.push(GameEvent::GamePreparationCompleted);
    Ok(events)
}

fn resolve_owned_pouch(
    state: &GameState,
    player: &PlayerId,
    decision: &SecretStrategyDecision,
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
    validate_decision(state, player, pouch.card, decision)?;
    let strategy = decision.strategy();
    let mut events = vec![GameEvent::PouchRevealed {
        player: player.clone(),
        owner: Some(player.clone()),
        card: pouch.card,
        strategy,
    }];
    events.extend(secret_strategy::strategy_events(state, player, decision)?);
    if strategy != SecretStrategy::SheepStealing {
        events.push(GameEvent::PouchConsumed {
            owner: Some(player.clone()),
            card: pouch.card,
        });
    }
    Ok(events)
}

pub(crate) fn playable_owned_strategy_actions(
    state: &GameState,
    player: &PlayerId,
) -> Vec<SecretStrategyOption> {
    let Some(source_card) = state.pouch_for(player).map(|pouch| pouch.card) else {
        return Vec::new();
    };

    strategy_action_options(state, player, source_card)
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
    decision: Option<&ChainPouchDecision>,
) -> GameResult<Vec<GameEvent>> {
    let Some(decision) = decision else {
        let pile = RandomnessDeck::Player(player.clone());
        if let Some(event) = crate::rules::deck_supply::request_if_needed(
            state,
            &pile,
            2,
            crate::domain::DeckPlacement::Bottom,
            format!(
                "pouch:chain:recycle:{}:{}",
                player.as_str(),
                state.turn_number
            ),
            PendingResolution::PouchChainRecycle,
        )? {
            return Ok(vec![event]);
        }
        let deck = state
            .deck_for(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
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
                resolution: PendingResolution::PouchChain,
            },
        )?]);
    };
    let plan = chain_decision_plan(state, player, decision)?;
    let trigger_decision = plan.trigger_decision;
    let trigger_card = trigger_decision.map(SecretStrategyDecision::source_card);
    let strategy = plan.strategy;
    let mut events = vec![plan.placed];
    if let Some(revealed) = plan.revealed {
        events.push(revealed);
    }
    if let (Some(source_card), Some(strategy), Some(decision)) =
        (trigger_card, strategy, trigger_decision)
    {
        let strategy_events = secret_strategy::strategy_events(&plan.projected, player, decision)?;
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

    let mut projected = state.clone();
    for event in &events {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    let remainder = projected
        .deck_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
        .to_vec();
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
                current_order: remainder,
            },
            resolution: PendingResolution::PouchChainPostSearch {
                player: player.clone(),
            },
        });
    }
    Ok(events)
}

/// 在建立 `ChoiceMade` 前檢查連環 envelope 與巢狀秘計 Decision。這個入口同時
/// 由 Pending Choice 邊界與實際事件規劃使用，保證無效答案不會先留下選擇事件。
pub(crate) fn validate_chain_decision(
    state: &GameState,
    player: &PlayerId,
    decision: &ChainPouchDecision,
) -> GameResult<()> {
    chain_decision_plan(state, player, decision).map(|_| ())
}

struct ChainDecisionPlan<'a> {
    trigger_decision: Option<&'a SecretStrategyDecision>,
    strategy: Option<SecretStrategy>,
    placed: GameEvent,
    revealed: Option<GameEvent>,
    projected: GameState,
}

fn chain_decision_plan<'a>(
    state: &GameState,
    player: &PlayerId,
    decision: &'a ChainPouchDecision,
) -> GameResult<ChainDecisionPlan<'a>> {
    let owner = decision.pouch_owner().clone();
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
    let pouch_card = decision.pouch_card();
    let trigger_decision = decision.trigger_decision();
    let trigger_card = trigger_decision.map(SecretStrategyDecision::source_card);
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
    let strategy = trigger_decision.map(SecretStrategyDecision::strategy);
    let previous = state.pouch_for(&owner).map(|pouch| pouch.card);
    let mut known_by = vec![player.clone()];
    if owner != *player {
        known_by.push(owner.clone());
    }
    let placed = GameEvent::PouchPlaced {
        source: player.clone(),
        owner: owner.clone(),
        card: pouch_card,
        known_by,
        previous,
    };
    let mut projected = state.clone();
    crate::rules::projection::apply_event(&mut projected, &placed);
    let revealed =
        trigger_card
            .zip(strategy)
            .map(|(source_card, strategy)| GameEvent::PouchRevealed {
                player: player.clone(),
                owner: None,
                card: source_card,
                strategy,
            });
    if let Some(revealed) = &revealed {
        crate::rules::projection::apply_event(&mut projected, revealed);
    }

    // 所有可立即決定的輸入先在尚未提交的投影上驗證；任何失敗都不會輸出
    // PouchPlaced 或 PouchRevealed。
    if let Some(decision) = trigger_decision {
        validate_decision(&projected, player, decision.source_card(), decision)?;
    }

    Ok(ChainDecisionPlan {
        trigger_decision,
        strategy,
        placed,
        revealed,
        projected,
    })
}

pub(crate) fn answer_chain_choice(
    state: &GameState,
    player: &PlayerId,
    answer: &crate::domain::ChoiceAnswer,
) -> GameResult<Option<Vec<GameEvent>>> {
    let crate::domain::ChoiceAnswer::Chain { decision } = answer else {
        return Ok(None);
    };
    Ok(Some(chain_events(state, player, Some(decision))?))
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
        .map(|card| {
            crate::domain::discard::move_from(state, *card, CardZone::PlayerDeckTop(player.clone()))
        })
        .collect::<GameResult<Vec<_>>>()?;
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
        from: CardZone::PlayerDiscard(player.clone()),
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
                current_order: order,
            },
            resolution: PendingResolution::PouchSheepStealing {
                source_card,
                owner: state
                    .pouch_for(player)
                    .filter(|pouch| pouch.card == source_card)
                    .map(|pouch| pouch.owner.clone()),
            },
        },
    ])
}

fn sheep_choice_events(
    state: &GameState,
    player: &PlayerId,
    source_card: CardInstanceId,
) -> GameResult<Vec<GameEvent>> {
    let pile = RandomnessDeck::Player(player.clone());
    if let Some(event) = crate::rules::deck_supply::request_if_needed(
        state,
        &pile,
        2,
        crate::domain::DeckPlacement::Bottom,
        format!(
            "pouch:sheep-stealing:recycle:{}:{}",
            player.as_str(),
            state.turn_number
        ),
        PendingResolution::PouchSheepStealingRecycle { source_card },
    )? {
        return Ok(vec![event]);
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
            resolution: PendingResolution::PouchSheepStealingChoice,
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
    chain_events(state, player, None)
}

pub(crate) fn after_chain_post_search_randomness_events(
    _state: &GameState,
    _player: &PlayerId,
) -> GameResult<Vec<GameEvent>> {
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
                        |event| matches!(event, GameEvent::RandomnessRequested { request, .. }
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
                        GameEvent::RandomnessRequested { request, .. } =>
                            match &request.operation {
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

    #[test]
    fn final_initial_pouch_choice_remains_duplicate_error_after_completion_advances_stage() {
        let setup = setup();
        let mut record = crate::application::GameRecord::start(setup, Vec::new()).unwrap();
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let alice_card = record.state().deck_for(&alice).unwrap()[0];
        let bob_card = record.state().deck_for(&bob).unwrap()[0];

        record
            .handle(Command::ChooseInitialPouch {
                player: alice,
                card: alice_card,
            })
            .unwrap();
        record
            .handle(Command::ChooseInitialPouch {
                player: bob.clone(),
                card: bob_card,
            })
            .unwrap();
        let event_count = record.events().len();

        assert_eq!(
            record.handle(Command::ChooseInitialPouch {
                player: bob.clone(),
                card: bob_card,
            }),
            Err(GameError::Validation(
                ValidationError::InitialPouchAlreadyChosen { player: bob },
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
    fn pouch_command_serializes_closed_decision_as_camel_case() {
        let command = Command::TriggerSecretStrategy {
            player: PlayerId::new("alice"),
            decision: SecretStrategyDecision::Environment {
                source_card: CardInstanceId::new(7),
                operation: crate::domain::SecretStrategyEnvironmentOperation::TransferByDiscard {
                    card: CardInstanceId::new(8),
                },
            },
        };
        let json = serde_json::to_value(command).unwrap();
        let decision = &json["TriggerSecretStrategy"]["decision"];
        assert_eq!(decision["type"], "environment");
        assert_eq!(decision["sourceCard"], 7);
        assert_eq!(decision["operation"]["type"], "transferByDiscard");
        assert_eq!(decision["operation"]["card"], 8);
        assert!(decision.get("source_card").is_none());
    }

    #[test]
    fn initial_pouch_chosen_event_has_no_next_player_payload() {
        let event = GameEvent::InitialPouchChosen {
            player: PlayerId::new("alice"),
            card: CardInstanceId::new(7),
        };
        let json = serde_json::to_value(event).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "InitialPouchChosen": { "player": "alice", "card": 7 }
            }),
        );
        assert!(json["InitialPouchChosen"].get("next_player").is_none());
        assert!(json["InitialPouchChosen"].get("nextPlayer").is_none());
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
