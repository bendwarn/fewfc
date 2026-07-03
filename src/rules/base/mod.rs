mod attack_resolution;
mod covered_passive;
mod effect_intent;
mod formation_selection;
mod formation_use;

use crate::domain::{
    CannotPerformFormationReason, CardDef, CardDefId, CardInstanceDef, CardInstanceId,
    CardMoveDelta, CardOrigin, CardZone, Command, DISCARD_RETRIEVAL_MODULE_ID, DeckPlacement,
    Element, EngineInvariantError, GameError, GameEvent, GameResult, GameSetup, GameState,
    GameStatus, HpChangeDelta, PERSONAL_DECK_MODULE_ID, PassActionReason, Phase, Player,
    PlayerDeckList, PlayerId, RulesetId, TeamHp, TurnDrawSkipReason, ValidationError,
    validate_setup,
};
use crate::rules::PlayableAction;
use crate::rules::projection;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct BaseRuleset;

impl BaseRuleset {
    pub(crate) fn new() -> Self {
        Self
    }

    pub(crate) fn id(&self) -> RulesetId {
        RulesetId::base()
    }

    pub(crate) fn start_game(
        &self,
        setup: &GameSetup,
        deck_order: Vec<CardInstanceId>,
    ) -> GameResult<Vec<GameEvent>> {
        validate_setup(setup)?;
        validate_card_instances(setup, &deck_order)?;
        initial_events(setup, deck_order)
    }

    pub(crate) fn decide_command(
        &self,
        state: &GameState,
        command: Command,
    ) -> GameResult<Vec<GameEvent>> {
        let mut events = decide_command_with_base_ruleset(state, command)?;
        crate::rules::spirit::append_automatic_blooms(state, &mut events)?;
        Ok(events)
    }

    pub(crate) fn advance_automatic(&self, state: &GameState) -> GameResult<Vec<GameEvent>> {
        advance_automatic(state)
    }

    pub(crate) fn playable_actions(
        &self,
        state: &GameState,
        player: &crate::domain::PlayerId,
        selected_cards: &[CardInstanceId],
    ) -> GameResult<Vec<PlayableAction>> {
        ensure_can_query_playable_actions(state, player)?;

        let mut actions = Vec::new();
        if !player_has_status(state, player, "CannotAct") {
            actions.extend(
                formation_selection::FormationSelection::new(
                    state,
                    player,
                    selected_cards.to_vec(),
                )?
                .candidates()
                .into_iter()
                .map(PlayableAction::PerformFormation),
            );
            actions.extend(
                crate::rules::profession::playable_profession_changes(
                    state,
                    player,
                    selected_cards,
                )?
                .into_iter()
                .map(PlayableAction::ChangeProfession),
            );
            actions.extend(
                crate::rules::profession::playable_profession_abilities(
                    state,
                    player,
                    selected_cards,
                )?
                .into_iter()
                .map(PlayableAction::ActivateProfessionAbility),
            );
        }
        actions.extend(
            crate::rules::spirit::playable_skills(state, player, selected_cards)
                .into_iter()
                .map(PlayableAction::UseSpiritSkill),
        );
        Ok(actions)
    }

    pub(crate) fn official_game_setup(
        &self,
        players: Vec<Player>,
        turn_order: Vec<PlayerId>,
    ) -> GameSetup {
        let mut teams = HashSet::new();
        let hp = players
            .iter()
            .filter_map(|player| {
                teams.insert(player.team.clone()).then_some(TeamHp {
                    team: player.team.clone(),
                    hp: 20,
                })
            })
            .collect();

        GameSetup {
            ruleset: self.id(),
            enabled_rule_modules: Vec::new(),
            players,
            turn_order,
            hp,
            card_defs: official_card_defs(),
            card_instances: official_card_instances(),
            deck_lists: Vec::new(),
            hand_limit: 5,
            base_draw: 2,
        }
    }

    pub(crate) fn configure_personal_decks(
        &self,
        setup: &mut GameSetup,
        requested_decks: Vec<PlayerDeckList>,
    ) {
        let deck_lists = setup
            .players
            .iter()
            .map(|player| {
                requested_decks
                    .iter()
                    .find(|deck| deck.player == player.id)
                    .filter(|deck| valid_personal_deck(&setup.card_defs, deck))
                    .cloned()
                    .unwrap_or_else(|| preconstructed_deck(player.id.clone()))
            })
            .collect::<Vec<_>>();

        let mut next_instance = 1;
        let mut card_instances = Vec::new();
        for deck in &deck_lists {
            for definition in &deck.cards {
                card_instances.push(CardInstanceDef {
                    instance: CardInstanceId::new(next_instance),
                    definition: definition.clone(),
                    origin: CardOrigin::Player(deck.player.clone()),
                });
                next_instance += 1;
            }
        }

        setup.deck_lists = deck_lists;
        setup.card_instances = card_instances;
    }

    pub(crate) fn preconstructed_deck(&self, player: PlayerId) -> PlayerDeckList {
        preconstructed_deck(player)
    }

    pub(crate) fn official_deck_order(&self, setup: &GameSetup) -> Vec<CardInstanceId> {
        let mut deck_order = setup
            .card_instances
            .iter()
            .map(|card| card.instance)
            .collect::<Vec<_>>();
        deck_order.sort();
        deck_order
    }

    pub(crate) fn card_labels(&self, setup: &GameSetup) -> HashMap<CardInstanceId, String> {
        setup
            .card_instances
            .iter()
            .filter_map(|instance| {
                let card_def = setup
                    .card_defs
                    .iter()
                    .find(|card_def| card_def.id == instance.definition)?;
                Some((
                    instance.instance,
                    format!("{} {}", card_def.name, card_def.level),
                ))
            })
            .collect()
    }
}

fn official_card_defs() -> Vec<CardDef> {
    elements()
        .into_iter()
        .flat_map(|element| {
            (1..=5).map(move |level| {
                card_def(
                    &format!("{}-{}", element.id, level),
                    element.name,
                    element.element,
                    level,
                )
            })
        })
        .collect()
}

fn official_card_instances() -> Vec<CardInstanceDef> {
    let mut next_instance = 1;
    let mut instances = Vec::new();

    for element in elements() {
        for level in 1..=5 {
            let copies = official_copy_count(level);
            for _ in 0..copies {
                instances.push(CardInstanceDef {
                    instance: CardInstanceId::new(next_instance),
                    definition: CardDefId::new(format!("{}-{}", element.id, level)),
                    origin: CardOrigin::Shared,
                });
                next_instance += 1;
            }
        }
    }

    instances
}

fn official_copy_count(level: u32) -> u64 {
    match level {
        1..=3 => 4,
        4..=5 => 3,
        _ => 0,
    }
}

fn preconstructed_deck(player: PlayerId) -> PlayerDeckList {
    let copies_by_level = [3, 2, 3, 2, 2];
    let cards = elements()
        .into_iter()
        .flat_map(|element| {
            copies_by_level
                .into_iter()
                .enumerate()
                .flat_map(move |(index, copies)| {
                    std::iter::repeat_n(
                        CardDefId::new(format!("{}-{}", element.id, index + 1)),
                        copies,
                    )
                })
        })
        .collect();

    PlayerDeckList {
        player,
        name: "五行均衡預組".to_string(),
        cards,
    }
}

fn valid_personal_deck(card_defs: &[CardDef], deck: &PlayerDeckList) -> bool {
    if deck.cards.len() != 60 {
        return false;
    }

    let definitions = card_defs
        .iter()
        .map(|definition| (&definition.id, definition))
        .collect::<HashMap<_, _>>();
    let mut level_total = 0;
    let mut counts = HashMap::<&CardDefId, usize>::new();

    for card in &deck.cards {
        let Some(definition) = definitions.get(card) else {
            return false;
        };
        level_total += definition.level;
        *counts.entry(card).or_default() += 1;
    }

    level_total <= 170
        && counts.into_iter().all(|(card, actual)| {
            let level = definitions
                .get(card)
                .expect("counted definitions must exist")
                .level;
            actual
                <= match level {
                    1..=3 => 4,
                    4..=5 => 3,
                    _ => 0,
                }
        })
}

fn elements() -> [ElementSpec; 5] {
    [
        ElementSpec {
            id: "metal",
            name: "金",
            element: Element::Metal,
        },
        ElementSpec {
            id: "wood",
            name: "木",
            element: Element::Wood,
        },
        ElementSpec {
            id: "water",
            name: "水",
            element: Element::Water,
        },
        ElementSpec {
            id: "fire",
            name: "火",
            element: Element::Fire,
        },
        ElementSpec {
            id: "earth",
            name: "土",
            element: Element::Earth,
        },
    ]
}

#[derive(Clone, Copy)]
struct ElementSpec {
    id: &'static str,
    name: &'static str,
    element: Element,
}

fn card_def(id: &str, name: &str, element: Element, level: u32) -> CardDef {
    CardDef {
        id: CardDefId::new(id),
        name: name.to_string(),
        element,
        level,
    }
}

fn validate_card_instances(setup: &GameSetup, deck_order: &[CardInstanceId]) -> GameResult<()> {
    let mut seen = HashSet::new();
    let known_instances = setup
        .card_instances
        .iter()
        .map(|card| card.instance)
        .collect::<HashSet<_>>();

    for card in deck_order {
        if !seen.insert(*card) {
            return Err(GameError::Validation(ValidationError::DuplicateCard(*card)));
        }

        if !known_instances.contains(card) {
            return Err(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ));
        }
    }

    Ok(())
}

fn initial_events(
    setup: &GameSetup,
    deck_order: Vec<CardInstanceId>,
) -> GameResult<Vec<GameEvent>> {
    if setup.has_rule_module(PERSONAL_DECK_MODULE_ID) {
        let mut events = Vec::new();
        for (turn_index, player) in setup.turn_order.iter().enumerate() {
            let player_deck = deck_order
                .iter()
                .copied()
                .filter(|card| {
                    setup
                        .card_instances
                        .iter()
                        .find(|instance| instance.instance == *card)
                        .is_some_and(
                            |instance| matches!(&instance.origin, CardOrigin::Player(owner) if owner == player),
                        )
                })
                .collect::<Vec<_>>();
            let card_count = if turn_index == 0 { 4 } else { 5 };
            if player_deck.len() < card_count {
                return Err(GameError::EngineInvariant(
                    EngineInvariantError::NotEnoughCards {
                        needed: card_count,
                        available: player_deck.len(),
                    },
                ));
            }

            events.push(GameEvent::PlayerDeckPrepared {
                player: player.clone(),
                deck_order: player_deck.clone(),
            });
            events.push(GameEvent::CardsDealt {
                player: player.clone(),
                cards: player_deck[..card_count].to_vec(),
            });
        }
        return Ok(events);
    }

    let needed = initial_deal_count(setup);
    if deck_order.len() < needed {
        return Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed,
                available: deck_order.len(),
            },
        ));
    }

    let mut events = vec![GameEvent::DeckPrepared {
        deck_order: deck_order.clone(),
    }];
    let mut next_card_index = 0;

    for (turn_index, player) in setup.turn_order.iter().enumerate() {
        let card_count = if turn_index == 0 { 4 } else { 5 };
        let cards = deck_order[next_card_index..next_card_index + card_count].to_vec();
        next_card_index += card_count;
        events.push(GameEvent::CardsDealt {
            player: player.clone(),
            cards,
        });
    }

    Ok(events)
}

fn initial_deal_count(setup: &GameSetup) -> usize {
    setup
        .turn_order
        .iter()
        .enumerate()
        .map(|(index, _)| if index == 0 { 4 } else { 5 })
        .sum()
}

fn advance_automatic(state: &GameState) -> GameResult<Vec<GameEvent>> {
    ensure_engine_invariants(state)?;

    if matches!(state.status, GameStatus::Finished { .. }) {
        return Ok(Vec::new());
    }
    if state.pending_choice.is_some() || state.pending_randomness.is_some() {
        return Ok(Vec::new());
    }

    let mut projected = state.clone();
    let mut events = Vec::new();

    loop {
        let next_event = match projected.phase {
            Phase::TurnStart => status_expiry_event(
                &projected,
                crate::domain::StatusExpiryTiming::TurnStart {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                        .clone(),
                },
            )
            .or_else(|| {
                Some(GameEvent::TurnStarted {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))
                        .ok()?
                        .clone(),
                    turn_number: projected.turn_number,
                })
            }),
            Phase::TurnDraw => next_turn_draw_event(&projected)?,
            Phase::TurnEnd => crate::rules::jianghu::turn_end_event(&projected)?
                .or_else(|| {
                    status_expiry_event(
                        &projected,
                        crate::domain::StatusExpiryTiming::TurnEnd {
                            player: projected
                                .current_player()
                                .expect("validated non-empty turn order")
                                .clone(),
                        },
                    )
                })
                .or_else(|| {
                    Some(GameEvent::TurnEnded {
                        player: projected
                            .current_player()
                            .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))
                            .ok()?
                            .clone(),
                    })
                }),
            Phase::Main | Phase::TurnDrawDiscardChoice => None,
        };

        let Some(event) = next_event else {
            break;
        };

        projection::apply_event(&mut projected, &event);
        events.push(event);
    }

    Ok(events)
}

fn status_expiry_event(
    state: &GameState,
    timing: crate::domain::StatusExpiryTiming,
) -> Option<GameEvent> {
    let status = state
        .statuses
        .iter()
        .find(|status| status_expires_at(&status.duration, &timing, state.turn_number))?;

    Some(GameEvent::StatusExpired {
        status_id: status.id.clone(),
        owner: status.owner.clone(),
        expired_at: timing,
    })
}

fn status_expires_at(
    duration: &crate::domain::StatusDuration,
    timing: &crate::domain::StatusExpiryTiming,
    current_turn_number: u64,
) -> bool {
    match (duration, timing) {
        (
            crate::domain::StatusDuration::UntilTurnStart {
                player: duration_player,
            },
            crate::domain::StatusExpiryTiming::TurnStart {
                player: timing_player,
            },
        )
        | (
            crate::domain::StatusDuration::UntilTurnEnd {
                player: duration_player,
            },
            crate::domain::StatusExpiryTiming::TurnEnd {
                player: timing_player,
            },
        ) => duration_player == timing_player,
        (
            crate::domain::StatusDuration::UntilTurnEndNumber {
                player: duration_player,
                turn_number,
            },
            crate::domain::StatusExpiryTiming::TurnEnd {
                player: timing_player,
            },
        ) => duration_player == timing_player && *turn_number == current_turn_number,
        (crate::domain::StatusDuration::Permanent, _) => false,
        _ => false,
    }
}

fn next_turn_draw_event(state: &GameState) -> GameResult<Option<GameEvent>> {
    let player = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
        .clone();
    let hand = state
        .hand(&player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if player_has_status(state, &player, "CannotDraw") {
        return Ok(Some(GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::CannotDrawByStatus,
        }));
    }
    let available_space = state.hand_limit.saturating_sub(hand.len());

    if available_space == 0 {
        return Ok(Some(GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::HandLimitReached,
        }));
    }

    let draw_bonus = state
        .turn_draw_bonus_by_player
        .get(&player)
        .copied()
        .unwrap_or(0);
    let draw_count = (state.base_draw + draw_bonus).min(available_space) + 1;
    let deck = state
        .deck_for(&player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    let discard = state
        .discard_for(&player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if deck.len() < draw_count {
        if deck.len() + discard.len() >= draw_count && !discard.is_empty() {
            return Ok(Some(if state.uses_personal_decks() {
                GameEvent::PlayerDiscardRecycledIntoDeck {
                    player,
                    shuffled_order: discard.to_vec(),
                    placement: DeckPlacement::Bottom,
                }
            } else {
                GameEvent::DiscardRecycledIntoDeck {
                    shuffled_order: discard.to_vec(),
                    placement: DeckPlacement::Bottom,
                }
            }));
        }

        return Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed: draw_count,
                available: deck.len(),
            },
        ));
    }

    let drawn_cards = deck.iter().take(draw_count).copied().collect::<Vec<_>>();

    Ok(Some(GameEvent::CardsDrawnForTurnDiscardChoice {
        player,
        allowed_discards: drawn_cards.clone(),
        drawn_cards,
    }))
}

fn decide_command_with_base_ruleset(
    state: &GameState,
    command: Command,
) -> GameResult<Vec<GameEvent>> {
    ensure_engine_invariants(state)?;

    if matches!(state.status, GameStatus::Finished { .. }) {
        return Err(GameError::Validation(ValidationError::GameFinished));
    }
    if let Some(request) = &state.pending_randomness {
        return Err(GameError::Validation(
            ValidationError::PendingRandomnessInProgress {
                request_id: request.request_id.clone(),
            },
        ));
    }
    if let Some(choice) = &state.pending_choice {
        let is_choice_answer = matches!(
            (&choice.kind, &command),
            (
                crate::domain::PendingChoiceKind::TurnDrawDiscard { .. },
                Command::ChooseTurnDiscard { .. }
            ) | (
                crate::domain::PendingChoiceKind::EffectGenerated { .. },
                Command::AnswerEffectChoice { .. } | Command::AnswerEffectChoiceTyped { .. }
            ) | (
                crate::domain::PendingChoiceKind::CardSetChoice { .. },
                Command::AnswerEffectChoice { .. } | Command::AnswerEffectChoiceTyped { .. }
            ) | (
                crate::domain::PendingChoiceKind::TypedEffect { .. },
                Command::AnswerEffectChoiceTyped { .. }
            )
        );

        if !is_choice_answer {
            return Err(GameError::Validation(
                ValidationError::PendingChoiceInProgress {
                    player: choice.player.clone(),
                },
            ));
        }
    }

    match command {
        Command::PassAction { player, reason } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;
            if state.confluence_card_obligations.iter().any(|obligation| {
                obligation.owner == player && obligation.applied_on_turn == state.turn_number
            }) {
                return Err(GameError::Validation(
                    ValidationError::ProfessionAbilityCannotResolve(
                        "confluence:tuning-obligation".to_string(),
                    ),
                ));
            }

            match reason {
                PassActionReason::NoCardsInHand => {
                    let hand = state.hand(&player).ok_or_else(|| {
                        GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
                    })?;
                    if !hand.is_empty() {
                        return Err(GameError::Validation(ValidationError::CannotPassAction {
                            reason,
                        }));
                    }
                }
                PassActionReason::CannotActByStatus => {
                    if !player_has_status(state, &player, "CannotAct") {
                        return Err(GameError::Validation(ValidationError::CannotPassAction {
                            reason,
                        }));
                    }
                }
            }

            let passive_trigger = covered_passive::trigger(
                state,
                covered_passive::TriggerRequest {
                    incoming_player: player.clone(),
                    incoming_kind: covered_passive::IncomingActionKind::Pass,
                    ignores_formation_effects: false,
                    ignores_counter_effects: false,
                    attack_points: None,
                },
            );
            let mut events = passive_trigger.events();
            events.push(GameEvent::ActionPassed { player, reason });
            Ok(events)
        }
        Command::PerformFormation {
            player,
            formation_id,
            cards,
            declared_targets,
        } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;

            formation_use::resolve(
                state,
                formation_use::FormationUseRequest {
                    player,
                    formation_id,
                    cards,
                    declared_targets,
                    trusted_random_cards: None,
                },
            )
        }
        Command::PerformFormationWithTrustedRandomness {
            player,
            formation_id,
            cards,
            declared_targets,
            random_cards,
        } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;

            formation_use::resolve(
                state,
                formation_use::FormationUseRequest {
                    player,
                    formation_id,
                    cards,
                    declared_targets,
                    trusted_random_cards: Some(random_cards),
                },
            )
        }
        Command::ChangeProfession {
            player,
            profession,
            cards,
        } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;
            if !crate::rules::confluence::profession_change_satisfies_obligation(
                state, &player, &cards,
            ) {
                return Err(GameError::Validation(
                    ValidationError::ProfessionAbilityCannotResolve(
                        "confluence:tuning-obligation".to_string(),
                    ),
                ));
            }
            if player_has_status(state, &player, "CannotAct") {
                return Err(GameError::Validation(
                    ValidationError::CannotChangeProfession {
                        reason: CannotPerformFormationReason::CannotActByStatus {
                            player: player.clone(),
                        },
                    },
                ));
            }
            crate::rules::profession::validate_profession_change(
                state,
                &player,
                &profession,
                &cards,
            )?;
            let passive_trigger = covered_passive::trigger(
                state,
                covered_passive::TriggerRequest {
                    incoming_player: player.clone(),
                    incoming_kind: covered_passive::IncomingActionKind::ProfessionChange,
                    ignores_formation_effects: false,
                    ignores_counter_effects: false,
                    attack_points: None,
                },
            );
            let submitted_cards = cards.clone();
            let card_moves = cards
                .into_iter()
                .map(|card| CardMoveDelta {
                    card,
                    from: CardZone::Hand(player.clone()),
                    to: discard_zone_for_card(state, card),
                })
                .collect();
            let mut events = passive_trigger.events();
            let previous_profession = state.profession_for(&player).cloned();
            events.push(GameEvent::ProfessionChanged {
                player: player.clone(),
                previous: previous_profession.clone(),
                profession: profession.clone(),
                card_moves,
            });
            events.extend(crate::rules::confluence::profession_acquired_events(
                state,
                &player,
                previous_profession.as_ref(),
                &profession,
            ));
            events.extend(crate::rules::dark::profession_acquired_events(
                state,
                &player,
                &profession,
                &submitted_cards,
            ));
            Ok(events)
        }
        Command::ActivateProfessionAbility {
            player,
            ability_id,
            cards,
            target_card,
            declared_element,
            declared_level,
        } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;
            if player_has_status(state, &player, "CannotAct") {
                return Err(GameError::Validation(
                    ValidationError::ProfessionAbilityCannotResolve(ability_id),
                ));
            }
            crate::rules::profession::activate_profession_ability(
                state,
                &player,
                &ability_id,
                &cards,
                target_card,
                declared_element,
                declared_level,
            )
        }
        Command::UseSpiritSkill {
            player,
            skill,
            selected_card,
            declared_level,
        } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;
            crate::rules::spirit::use_skill(
                state,
                &player,
                skill,
                selected_card,
                declared_level,
                None,
            )
        }
        Command::UseSpiritSkillWithTrustedRandomness {
            player,
            skill,
            selected_card,
            declared_level,
            random_cards,
        } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;
            crate::rules::spirit::use_skill(
                state,
                &player,
                skill,
                selected_card,
                declared_level,
                Some(&random_cards),
            )
        }
        Command::ChooseTurnDiscard { player, discard } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::TurnDrawDiscardChoice)?;

            let allowed_discards = match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::TurnDrawDiscard {
                            allowed_discards, ..
                        },
                }) if choice_player == &player => allowed_discards,
                _ => return Err(GameError::Validation(ValidationError::MissingPendingChoice)),
            };

            if !allowed_discards.contains(&discard) {
                return Err(GameError::Validation(ValidationError::IllegalDiscard(
                    discard,
                )));
            }

            let mut events = vec![GameEvent::TurnDiscardChosen {
                player: player.clone(),
                discard,
            }];
            if let Some(owned) = state.spirit_for(&player)
                && owned.power < 6
                && crate::rules::spirit::turn_discard_charges(state, owned.spirit, discard)
            {
                events.push(GameEvent::SpiritPowerChanged {
                    player,
                    spirit: owned.spirit,
                    old_power: owned.power,
                    delta: 1,
                    new_power: owned.power + 1,
                    reason: crate::domain::SpiritPowerChangeReason::TurnDrawDiscard {
                        card: discard,
                    },
                });
            }
            Ok(events)
        }
        Command::AnswerEffectChoice {
            player,
            selected_cards,
        } => {
            let (effect_id, continuation_id, allowed_cards, minimum, maximum) =
                match &state.pending_choice {
                    Some(crate::domain::PendingChoice {
                        player: choice_player,
                        kind:
                            crate::domain::PendingChoiceKind::EffectGenerated {
                                effect_id,
                                continuation_id,
                                allowed_cards,
                            },
                    }) if choice_player == &player => {
                        let required = state
                            .pending_choice
                            .as_ref()
                            .expect("matched pending choice")
                            .kind
                            .required_count();
                        (
                            effect_id.clone(),
                            continuation_id.clone(),
                            allowed_cards,
                            required,
                            required,
                        )
                    }
                    Some(crate::domain::PendingChoice {
                        player: choice_player,
                        kind:
                            crate::domain::PendingChoiceKind::CardSetChoice {
                                effect_id,
                                continuation_id,
                                allowed_cards,
                                minimum,
                                maximum,
                            },
                    }) if choice_player == &player => (
                        effect_id.clone(),
                        continuation_id.clone(),
                        allowed_cards,
                        *minimum,
                        *maximum,
                    ),
                    _ => return Err(GameError::Validation(ValidationError::MissingPendingChoice)),
                };

            if selected_cards.len() < minimum || selected_cards.len() > maximum {
                return Err(GameError::Validation(ValidationError::MissingPendingChoice));
            }

            let mut seen = HashSet::new();
            for selected_card in &selected_cards {
                if !seen.insert(*selected_card) {
                    return Err(GameError::Validation(ValidationError::DuplicateChoiceCard(
                        *selected_card,
                    )));
                }

                if !allowed_cards.contains(selected_card) {
                    return Err(GameError::Validation(ValidationError::IllegalChoiceCard(
                        *selected_card,
                    )));
                }
            }

            let mut events = vec![GameEvent::EffectChoiceAnswered {
                player: player.clone(),
                effect_id: effect_id.clone(),
                continuation_id: continuation_id.clone(),
                selected_cards: selected_cards.clone(),
            }];
            let resumed_events = formation_use::answer_effect_choice(
                state,
                &player,
                &effect_id,
                &continuation_id,
                &selected_cards,
            )?;
            events.extend(resumed_events);
            Ok(events)
        }
        Command::AnswerEffectChoiceTyped { player, answer } => {
            let (effect_id, continuation_id) = match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::EffectGenerated {
                            effect_id,
                            continuation_id,
                            allowed_cards,
                        },
                }) if choice_player == &player
                    && matches!(
                        &answer,
                        crate::domain::EffectChoiceAnswer::Cards { cards }
                            if cards.len()
                                == state
                                    .pending_choice
                                    .as_ref()
                                    .expect("matched pending choice")
                                    .kind
                                    .required_count()
                                && effect_choice_cards_are_valid(allowed_cards, cards)
                    ) =>
                {
                    (effect_id.clone(), continuation_id.clone())
                }
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::CardSetChoice {
                            effect_id,
                            continuation_id,
                            allowed_cards,
                            minimum,
                            maximum,
                        },
                }) if choice_player == &player
                    && matches!(
                        &answer,
                        crate::domain::EffectChoiceAnswer::Cards { cards }
                            if cards.len() >= *minimum
                                && cards.len() <= *maximum
                                && effect_choice_cards_are_valid(allowed_cards, cards)
                    ) =>
                {
                    (effect_id.clone(), continuation_id.clone())
                }
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::TypedEffect {
                            effect_id,
                            continuation_id,
                            options,
                        },
                }) if choice_player == &player
                    && effect_choice_answer_is_valid(options, &answer) =>
                {
                    (effect_id.clone(), continuation_id.clone())
                }
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    ..
                }) if choice_player != &player => {
                    return Err(GameError::Validation(ValidationError::MissingPendingChoice));
                }
                Some(_) => {
                    return Err(GameError::Validation(
                        ValidationError::InvalidEffectChoiceAnswer,
                    ));
                }
                None => {
                    return Err(GameError::Validation(ValidationError::MissingPendingChoice));
                }
            };

            let mut events = vec![GameEvent::TypedEffectChoiceAnswered {
                player: player.clone(),
                effect_id: effect_id.clone(),
                continuation_id: continuation_id.clone(),
                answer: answer.clone(),
            }];
            if let crate::domain::EffectChoiceAnswer::Cards { cards } = &answer {
                events.extend(formation_use::answer_effect_choice(
                    state,
                    &player,
                    &effect_id,
                    &continuation_id,
                    cards,
                )?);
            }
            Ok(events)
        }
        Command::RetrievePreviousTurnDiscard { player } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;
            if !state.has_rule_module(DISCARD_RETRIEVAL_MODULE_ID) {
                return Err(GameError::Validation(
                    ValidationError::DiscardRetrievalDisabled,
                ));
            }

            let previous_player = previous_player(state, &player)?;
            let card = state
                .last_turn_discard_by_player
                .get(&previous_player)
                .filter(|discard| discard.turn_number + 1 == state.turn_number)
                .map(|discard| discard.card)
                .filter(|card| {
                    state
                        .discard_for(&previous_player)
                        .is_some_and(|discard| discard.contains(card))
                })
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::NoRetrievableDiscard {
                        previous_player: previous_player.clone(),
                    })
                })?;
            let level = state
                .card_def(card)
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(card),
                ))?
                .level as i32;
            let team = state
                .players
                .iter()
                .find(|candidate| candidate.id == player)
                .map(|candidate| candidate.team.clone())
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
                })?;
            let old_hp = state
                .hp
                .iter()
                .find(|team_hp| team_hp.team == team)
                .map(|team_hp| team_hp.hp)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::MissingTeamHp(team.clone()))
                })?;
            let hp_cost = crate::rules::hero::discard_retrieval_cost(state, &player, level * 2);
            let new_hp = (old_hp - hp_cost).max(0);
            let card_move = if state.uses_personal_decks() {
                CardMoveDelta {
                    card,
                    from: CardZone::PlayerDiscard(previous_player.clone()),
                    to: CardZone::PlayerDeckTop(player.clone()),
                }
            } else {
                CardMoveDelta {
                    card,
                    from: CardZone::Discard,
                    to: CardZone::DeckTop,
                }
            };

            Ok(vec![GameEvent::DiscardRetrieved {
                player,
                previous_player,
                card,
                hp_change: HpChangeDelta {
                    team,
                    old_hp,
                    delta: -hp_cost,
                    new_hp,
                    effective_delta: new_hp - old_hp,
                },
                card_move,
            }])
        }
    }
}

fn discard_zone_for_card(state: &GameState, card: CardInstanceId) -> CardZone {
    if state.uses_personal_decks() {
        match state.card_origin(card) {
            Some(CardOrigin::Player(owner)) => CardZone::PlayerDiscard(owner.clone()),
            _ => CardZone::Discard,
        }
    } else {
        CardZone::Discard
    }
}

fn previous_player(state: &GameState, player: &PlayerId) -> GameResult<PlayerId> {
    let index = state
        .turn_order
        .iter()
        .position(|candidate| candidate == player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    let previous_index = if index == 0 {
        state.turn_order.len().saturating_sub(1)
    } else {
        index - 1
    };
    state
        .turn_order
        .get(previous_index)
        .cloned()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))
}

fn ensure_engine_invariants(state: &GameState) -> GameResult<()> {
    let mut covered_passive_owners = HashSet::new();
    for passive in &state.covered_passives {
        if !covered_passive_owners.insert(passive.owner.clone()) {
            return Err(GameError::EngineInvariant(
                EngineInvariantError::DuplicateCoveredPassive {
                    player: passive.owner.clone(),
                },
            ));
        }
    }

    let mut profession_owners = HashSet::new();
    for profession in &state.professions {
        if !profession_owners.insert(profession.player.clone()) {
            return Err(GameError::EngineInvariant(
                EngineInvariantError::DuplicateProfession {
                    player: profession.player.clone(),
                },
            ));
        }
    }

    Ok(())
}

fn ensure_can_query_playable_actions(
    state: &GameState,
    player: &crate::domain::PlayerId,
) -> GameResult<()> {
    ensure_engine_invariants(state)?;

    if matches!(state.status, GameStatus::Finished { .. }) {
        return Err(GameError::Validation(ValidationError::GameFinished));
    }

    if let Some(choice) = &state.pending_choice {
        return Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::PendingChoiceInProgress {
                    player: choice.player.clone(),
                },
            },
        ));
    }
    if let Some(request) = &state.pending_randomness {
        return Err(GameError::Validation(
            ValidationError::PendingRandomnessInProgress {
                request_id: request.request_id.clone(),
            },
        ));
    }

    let expected = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;
    if expected != player {
        return Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::WrongPlayer {
                    expected: expected.clone(),
                    actual: player.clone(),
                },
            },
        ));
    }

    if state.phase != Phase::Main {
        return Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::WrongPhase {
                    expected: Phase::Main,
                    actual: state.phase,
                },
            },
        ));
    }

    Ok(())
}

pub(crate) fn effect_choice_answer_is_valid(
    options: &crate::domain::EffectChoiceOptions,
    answer: &crate::domain::EffectChoiceAnswer,
) -> bool {
    match answer {
        crate::domain::EffectChoiceAnswer::Cards { cards } => {
            let Some(card_options) = &options.cards else {
                return false;
            };
            if cards.len() < card_options.minimum || cards.len() > card_options.maximum {
                return false;
            }
            let mut seen = HashSet::new();
            cards
                .iter()
                .all(|card| seen.insert(*card) && card_options.allowed_cards.contains(card))
        }
        crate::domain::EffectChoiceAnswer::Player { player } => options.players.contains(player),
        crate::domain::EffectChoiceAnswer::Formation { formation_id } => {
            options.formations.contains(formation_id)
        }
        crate::domain::EffectChoiceAnswer::Decline => options.can_decline,
    }
}

pub(crate) fn effect_choice_cards_are_valid(
    allowed_cards: &[crate::domain::CardInstanceId],
    cards: &[crate::domain::CardInstanceId],
) -> bool {
    let mut seen = HashSet::new();
    cards
        .iter()
        .all(|card| seen.insert(*card) && allowed_cards.contains(card))
}

fn ensure_current_player(state: &GameState, actual: &crate::domain::PlayerId) -> GameResult<()> {
    let expected = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;

    if expected == actual {
        Ok(())
    } else {
        Err(GameError::Validation(ValidationError::WrongPlayer {
            expected: expected.clone(),
            actual: actual.clone(),
        }))
    }
}

fn ensure_phase(state: &GameState, expected: Phase) -> GameResult<()> {
    if state.phase == expected {
        Ok(())
    } else {
        Err(GameError::Validation(ValidationError::WrongPhase {
            expected,
            actual: state.phase,
        }))
    }
}

fn player_has_status(state: &GameState, player: &crate::domain::PlayerId, kind: &str) -> bool {
    state.statuses.iter().any(|status| {
        matches!(&status.owner, crate::domain::StatusOwner::Player(owner) if owner == player)
            && status.kind == kind
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CardDef, CardDefId, CardInstanceDef, Element, PlayerId};

    fn card(id: u64) -> CardInstanceId {
        CardInstanceId::new(id)
    }

    fn setup() -> GameSetup {
        GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).with_cards(
            vec![CardDef {
                id: CardDefId::new("metal"),
                name: "metal".to_string(),
                element: Element::Metal,
                level: 3,
            }],
            (1..=10)
                .map(|id| CardInstanceDef {
                    instance: card(id),
                    definition: CardDefId::new("metal"),
                    origin: Default::default(),
                })
                .collect(),
        )
    }

    #[test]
    fn official_game_setup_matches_rulebook_card_composition() {
        let ruleset = BaseRuleset::new();
        let setup = ruleset.official_game_setup(
            GameSetup::two_player(PlayerId::new("alice"), PlayerId::new("bob"), 20).players,
            vec![PlayerId::new("alice"), PlayerId::new("bob")],
        );

        assert_eq!(ruleset.official_deck_order(&setup).len(), 90);
        assert_eq!(setup.card_defs.len(), 25);
        assert_eq!(setup.card_instances.len(), 90);

        for element in [
            Element::Metal,
            Element::Wood,
            Element::Water,
            Element::Fire,
            Element::Earth,
        ] {
            for level in 1..=5 {
                let matching_instances = setup
                    .card_instances
                    .iter()
                    .filter(|instance| {
                        setup
                            .card_defs
                            .iter()
                            .find(|card_def| card_def.id == instance.definition)
                            .is_some_and(|card_def| {
                                card_def.element == element && card_def.level == level
                            })
                    })
                    .count();

                assert_eq!(
                    matching_instances,
                    official_copy_count(level) as usize,
                    "unexpected copies for {element:?} level {level}"
                );
            }
        }
    }

    #[test]
    fn start_game_emits_initial_setup_events() {
        let events = BaseRuleset::new()
            .start_game(&setup(), (1..=10).map(card).collect())
            .unwrap();

        assert!(matches!(
            events.first(),
            Some(GameEvent::DeckPrepared { .. })
        ));
        assert_eq!(events.len(), 3);
    }

    #[test]
    fn advance_automatic_stops_at_main_decision() {
        let events = BaseRuleset::new()
            .start_game(&setup(), (1..=10).map(card).collect())
            .unwrap();
        let state = projection::project(&setup(), &events).unwrap();

        assert_eq!(
            BaseRuleset::new().advance_automatic(&state).unwrap(),
            vec![GameEvent::TurnStarted {
                player: PlayerId::new("p1"),
                turn_number: 1,
            }]
        );
    }

    #[test]
    fn decide_command_accepts_action_pass_for_empty_hand() {
        let mut state = GameState::from_setup(&setup());
        state.phase = Phase::Main;
        state.hands[0].cards.clear();

        assert_eq!(
            BaseRuleset::new()
                .decide_command(
                    &state,
                    Command::PassAction {
                        player: PlayerId::new("p1"),
                        reason: PassActionReason::NoCardsInHand,
                    },
                )
                .unwrap(),
            vec![GameEvent::ActionPassed {
                player: PlayerId::new("p1"),
                reason: PassActionReason::NoCardsInHand,
            }]
        );
    }
}
