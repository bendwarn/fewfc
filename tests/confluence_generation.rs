use fewfc::application::{apply_event, handle_command, resolve_trusted_randomness};
use fewfc::domain::{
    CONFLUENCE_GENERATION_MODULE_ID, CardInstanceId, ChoiceAnswer, Command, DARK_GLIMMER_MODULE_ID,
    DeckPlacement, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent, GameState, GameStatus,
    HERO_SCHOOLS_MODULE_ID, JIANGHU_MODULE_ID, JianghuState, JianghuStateKind, LastTurnDiscard,
    LimitedUse, PendingRandomness, Phase, Player, PlayerId, PlayerProfession, PlayerSpirit,
    ProfessionId, RandomnessContinuation, RandomnessDeck, RandomnessOperation, RuleModuleId,
    SPIRIT_MODULE_ID, STAR_MODULE_ID, SpiritKind, SpiritSkill, TargetDecl, TeamId,
    TrustedRandomnessAnswer,
};
use fewfc::public_view::{PublicGameEvent, Viewer, event_for, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};

fn state(personal_deck: bool) -> GameState {
    let players = vec![
        Player {
            id: PlayerId::new("p1"),
            team: TeamId::new("team:a"),
        },
        Player {
            id: PlayerId::new("p2"),
            team: TeamId::new("team:b"),
        },
    ];
    let mut modules = vec![
        RuleModuleId::new(STAR_MODULE_ID),
        RuleModuleId::new(FIVE_DIRECTIONS_LEGEND_MODULE_ID),
        RuleModuleId::new(HERO_SCHOOLS_MODULE_ID),
        RuleModuleId::new(CONFLUENCE_GENERATION_MODULE_ID),
    ];
    if personal_deck {
        modules.push(RuleModuleId::new("personal-deck"));
    }
    let setup = OfficialRules::new()
        .configure_game(
            players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            modules,
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.phase = Phase::ActiveEffects;
    state
}

fn answer_choice(
    state: &GameState,
    player: PlayerId,
    answer: ChoiceAnswer,
) -> fewfc::domain::GameResult<Vec<GameEvent>> {
    handle_command(
        state,
        Command::AnswerChoice {
            player,
            choice_id: state
                .pending_choice
                .as_ref()
                .expect("pending choice")
                .choice_id,
            answer,
        },
    )
}

fn team_state(extra_modules: Vec<RuleModuleId>) -> GameState {
    let players = vec![
        Player {
            id: PlayerId::new("p1"),
            team: TeamId::new("team:a"),
        },
        Player {
            id: PlayerId::new("p2"),
            team: TeamId::new("team:b"),
        },
        Player {
            id: PlayerId::new("p3"),
            team: TeamId::new("team:a"),
        },
        Player {
            id: PlayerId::new("p4"),
            team: TeamId::new("team:b"),
        },
    ];
    let mut modules = vec![
        RuleModuleId::new(STAR_MODULE_ID),
        RuleModuleId::new(FIVE_DIRECTIONS_LEGEND_MODULE_ID),
        RuleModuleId::new(HERO_SCHOOLS_MODULE_ID),
        RuleModuleId::new(CONFLUENCE_GENERATION_MODULE_ID),
    ];
    modules.extend(extra_modules);
    let setup = OfficialRules::new()
        .configure_game(
            players,
            vec![
                PlayerId::new("p1"),
                PlayerId::new("p2"),
                PlayerId::new("p3"),
                PlayerId::new("p4"),
            ],
            modules,
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.phase = Phase::ActiveEffects;
    state
}

fn cards(state: &GameState, requested: &[(Element, u32)]) -> Vec<CardInstanceId> {
    cards_excluding(state, requested, &[])
}

fn cards_excluding(
    state: &GameState,
    requested: &[(Element, u32)],
    excluded: &[CardInstanceId],
) -> Vec<CardInstanceId> {
    let mut used = Vec::new();
    requested
        .iter()
        .map(|(element, level)| {
            let card = state
                .card_instances
                .iter()
                .map(|instance| instance.instance)
                .find(|instance| {
                    !used.contains(instance)
                        && !excluded.contains(instance)
                        && state.card_def(*instance).is_some_and(|definition| {
                            definition.element == *element && definition.level.value() == *level
                        })
                })
                .unwrap();
            used.push(card);
            card
        })
        .collect()
}

fn set_hand(state: &mut GameState, cards: Vec<CardInstanceId>) {
    set_player_hand(state, "p2", cards);
}

fn set_player_hand(state: &mut GameState, player: &str, cards: Vec<CardInstanceId>) {
    state
        .hands
        .iter_mut()
        .find(|hand| hand.player == PlayerId::new(player))
        .unwrap()
        .cards = cards;
}

fn cards_for_player(
    state: &GameState,
    player: &PlayerId,
    requested: &[(Element, u32)],
) -> Vec<CardInstanceId> {
    let mut used = Vec::new();
    requested
        .iter()
        .map(|(element, level)| {
            let card = state
                .card_instances
                .iter()
                .filter(|instance| {
                    !state.uses_personal_decks()
                        || matches!(
                            &instance.origin,
                            fewfc::domain::CardOrigin::Player(owner) if owner == player
                        )
                })
                .map(|instance| instance.instance)
                .find(|instance| {
                    !used.contains(instance)
                        && state.card_def(*instance).is_some_and(|definition| {
                            definition.element == *element && definition.level.value() == *level
                        })
                })
                .unwrap();
            used.push(card);
            card
        })
        .collect()
}

fn clear_wind_ten_thousand_miles_game(
    personal_deck: bool,
) -> (
    GameState,
    PlayerId,
    Vec<CardInstanceId>,
    CardInstanceId,
    Vec<CardInstanceId>,
) {
    let mut game = state(personal_deck);
    let player = PlayerId::new("p2");
    game.professions.push(PlayerProfession {
        player: player.clone(),
        profession: ProfessionId::new("confluence:clear-wind-envoy"),
    });
    let hand = cards_for_player(
        &game,
        &player,
        &[
            (Element::Metal, 1),
            (Element::Wood, 2),
            (Element::Water, 3),
            (Element::Fire, 4),
            (Element::Earth, 5),
        ],
    );
    let used_cards = hand[..4].to_vec();
    let remaining_card = hand[4];
    let drawn_cards = game
        .card_instances
        .iter()
        .filter(|instance| {
            (!personal_deck
                || matches!(
                    &instance.origin,
                    fewfc::domain::CardOrigin::Player(owner) if owner == &player
                ))
                && !hand.contains(&instance.instance)
        })
        .map(|instance| instance.instance)
        .take(10)
        .collect::<Vec<_>>();
    assert_eq!(drawn_cards.len(), 10);
    set_hand(&mut game, hand);
    let deck = game.deck_for_mut(&player).unwrap();
    deck.clear();
    deck.extend(drawn_cards.iter().copied());

    (game, player, used_cards, remaining_card, drawn_cards)
}

fn request_clear_wind_ten_thousand_miles(
    game: &mut GameState,
    player: &PlayerId,
    used_cards: Vec<CardInstanceId>,
) -> Vec<GameEvent> {
    let events = handle_command(
        game,
        Command::PerformFormation {
            player: player.clone(),
            formation_id: "confluence:clear-wind-ten-thousand-miles".to_string(),
            cards: used_cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(game, event);
    }
    events
}

fn set_residual(state: &mut GameState, card: CardInstanceId) {
    state
        .discard_for_mut(&PlayerId::new("p1"))
        .unwrap()
        .push(card);
    state.last_turn_discard_by_player.insert(
        PlayerId::new("p1"),
        LastTurnDiscard {
            card,
            turn_number: 1,
        },
    );
}

#[test]
fn clear_wind_reveals_once_then_waits_for_a_destination_choice() {
    let mut game = state(false);
    let player = PlayerId::new("p2");
    game.professions.push(PlayerProfession {
        player: player.clone(),
        profession: ProfessionId::new("confluence:clear-wind-adept"),
    });
    let top = cards(&game, &[(Element::Metal, 1)])[0];
    let deck = game.deck_for_mut(&player).unwrap();
    deck.clear();
    deck.push(top);

    let candidates = OfficialRules::new()
        .playable_actions(&game, &player, &[])
        .unwrap()
        .into_iter()
        .filter_map(|action| match action {
            PlayableAction::ActivateProfessionAbility(candidate)
                if candidate.ability_id == "confluence:clear-wind" =>
            {
                Some(candidate)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].target_card, None);
    assert_eq!(candidates[0].declared_level, None);

    let events = handle_command(
        &game,
        Command::ActivateProfessionAbility {
            player: player.clone(),
            ability_id: "confluence:clear-wind".to_string(),
            cards: Vec::new(),
            target_card: None,
            declared_element: None,
            declared_level: None,
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::DeckTopRevealed { player: owner, card }
            if owner == &player && card == &top
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::ChoiceRequested { choice }
            if choice.player == player
                && matches!(&choice.kind, fewfc::domain::PendingChoiceKind::Card {
                    cards,
                    minimum: 0,
                    maximum: 1,
                    ..
                } if cards == &vec![top])
                && matches!(choice.continuation,
                    fewfc::domain::ChoiceContinuation::Confluence(
                        fewfc::domain::ConfluenceChoiceContinuation::ClearWindDiscardTop))
    )));

    for event in &events {
        apply_event(&mut game, event);
    }
    let return_events = answer_choice(
        &game,
        player.clone(),
        ChoiceAnswer::Cards { cards: Vec::new() },
    )
    .unwrap();
    assert!(
        !return_events
            .iter()
            .any(|event| matches!(event, GameEvent::CardsMoved { .. }))
    );

    let discard_events =
        answer_choice(&game, player, ChoiceAnswer::Cards { cards: vec![top] }).unwrap();
    assert!(discard_events.iter().any(|event| matches!(
        event,
        GameEvent::CardsMoved { card_moves }
            if card_moves.len() == 1
                && card_moves[0].card == top
                && matches!(card_moves[0].to, fewfc::domain::CardZone::Discard)
    )));
}

#[test]
fn clear_wind_ten_thousand_miles_limits_kept_cards_and_discards_unselected_shared_cards() {
    let (mut game, player, used_cards, remaining_card, drawn_cards) =
        clear_wind_ten_thousand_miles_game(false);
    request_clear_wind_ten_thousand_miles(&mut game, &player, used_cards.clone());

    assert!(matches!(
        &game.pending_choice,
        Some(fewfc::domain::PendingChoice {
            player: choice_player,
            kind: fewfc::domain::PendingChoiceKind::Card {
                cards: allowed_cards,
                minimum: 0,
                maximum: 4,
                ..
            },
            continuation: fewfc::domain::ChoiceContinuation::Confluence(
                fewfc::domain::ConfluenceChoiceContinuation::ClearWindKeepCards),
            ..
        }) if choice_player == &player
            && allowed_cards == &drawn_cards
    ));

    let kept_cards = drawn_cards[..4].to_vec();
    let events = answer_choice(
        &game,
        player.clone(),
        ChoiceAnswer::Cards {
            cards: kept_cards.clone(),
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceMade { .. }))
    );
    for event in &events {
        apply_event(&mut game, event);
    }

    let hand = game.hand(&player).unwrap();
    assert_eq!(hand.len(), 5);
    assert!(hand.contains(&remaining_card));
    assert!(kept_cards.iter().all(|card| hand.contains(card)));
    assert!(
        drawn_cards[4..]
            .iter()
            .all(|card| game.discard.contains(card))
    );
    assert!(kept_cards.iter().all(|card| !game.discard.contains(card)));
    assert!(used_cards.iter().all(|card| game.discard.contains(card)));
}

#[test]
fn clear_wind_ten_thousand_miles_allows_keeping_no_cards() {
    let (mut game, player, used_cards, remaining_card, drawn_cards) =
        clear_wind_ten_thousand_miles_game(false);
    request_clear_wind_ten_thousand_miles(&mut game, &player, used_cards);

    let events = answer_choice(
        &game,
        player.clone(),
        ChoiceAnswer::Cards { cards: Vec::new() },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut game, event);
    }

    assert_eq!(game.hand(&player).unwrap(), &[remaining_card]);
    assert!(drawn_cards.iter().all(|card| game.discard.contains(card)));
}

#[test]
fn clear_wind_ten_thousand_miles_kept_cards_are_hidden_from_other_players() {
    let (mut game, player, used_cards, _, drawn_cards) = clear_wind_ten_thousand_miles_game(false);
    request_clear_wind_ten_thousand_miles(&mut game, &player, used_cards);
    let kept_cards = drawn_cards[..4].to_vec();

    let events = answer_choice(&game, player, ChoiceAnswer::Cards { cards: kept_cards }).unwrap();

    let answer = events
        .iter()
        .find(|event| matches!(event, GameEvent::ChoiceMade { .. }))
        .unwrap();
    assert_eq!(
        event_for(answer, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::ChoiceMade {
            player: PlayerId::new("p2")
        }
    );
}

#[test]
fn clear_wind_ten_thousand_miles_rejects_answers_above_the_pending_maximum() {
    let (mut game, player, used_cards, _, drawn_cards) = clear_wind_ten_thousand_miles_game(false);
    request_clear_wind_ten_thousand_miles(&mut game, &player, used_cards);
    let state_before_answer = game.clone();

    let result = answer_choice(
        &game,
        player,
        ChoiceAnswer::Cards {
            cards: drawn_cards[..5].to_vec(),
        },
    );

    assert!(result.is_err());
    assert_eq!(game, state_before_answer);
}

#[test]
fn clear_wind_ten_thousand_miles_discards_personal_deck_cards_to_their_origin_piles() {
    let (mut game, player, used_cards, _, mut drawn_cards) =
        clear_wind_ten_thousand_miles_game(true);
    let foreign_owner = PlayerId::new("p1");
    let foreign_card = game
        .card_instances
        .iter()
        .find(|instance| {
            matches!(
                &instance.origin,
                fewfc::domain::CardOrigin::Player(owner) if owner == &foreign_owner
            )
        })
        .unwrap()
        .instance;
    game.deck_for_mut(&foreign_owner)
        .unwrap()
        .retain(|card| card != &foreign_card);
    let replaced_card = drawn_cards[9];
    let deck = game.deck_for_mut(&player).unwrap();
    let replaced_position = deck.iter().position(|card| card == &replaced_card).unwrap();
    deck[replaced_position] = foreign_card;
    drawn_cards[9] = foreign_card;
    game.exposed_foreign_cards.push(foreign_card);
    request_clear_wind_ten_thousand_miles(&mut game, &player, used_cards);

    let events = answer_choice(
        &game,
        player.clone(),
        ChoiceAnswer::Cards { cards: Vec::new() },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::CardsMoved { card_moves }
            if card_moves.len() == drawn_cards.len()
                && card_moves.iter().all(|move_delta| {
                    matches!(
                        move_delta.to,
                        fewfc::domain::CardZone::PlayerDiscard(ref owner)
                            if owner == if move_delta.card == foreign_card {
                                &foreign_owner
                            } else {
                                &player
                            }
                    )
                })
    )));
    for event in &events {
        apply_event(&mut game, event);
    }

    let discard = game.discard_for(&player).unwrap();
    assert!(
        drawn_cards
            .iter()
            .filter(|card| **card != foreign_card)
            .all(|card| discard.contains(card))
    );
    assert!(
        game.discard_for(&foreign_owner)
            .unwrap()
            .contains(&foreign_card)
    );
    assert!(drawn_cards.iter().all(|card| !game.discard.contains(card)));
}

#[test]
fn residual_element_and_level_survive_after_the_source_discard_moves() {
    let mut game = state(false);
    let residual = cards(&game, &[(Element::Metal, 2)])[0];
    set_residual(&mut game, residual);
    let transition = cards(&game, &[(Element::Metal, 1), (Element::Metal, 2)]);
    set_hand(&mut game, transition.clone());

    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &transition)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ChangeProfession(candidate)
            if candidate.profession_id == ProfessionId::new("confluence:tuner")
    )));

    game.discard_for_mut(&PlayerId::new("p1")).unwrap().clear();
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &transition)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ChangeProfession(candidate)
            if candidate.profession_id == ProfessionId::new("confluence:tuner")
    )));
}

#[test]
fn five_resonance_requires_distinct_cards_for_element_and_residual_level_slots() {
    let mut game = state(false);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:tuner"),
    });
    let residual = cards(&game, &[(Element::Metal, 3)])[0];
    set_residual(&mut game, residual);

    let overlapping = cards(&game, &[(Element::Metal, 3), (Element::Fire, 5)]);
    set_hand(&mut game, overlapping.clone());
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &overlapping)
        .unwrap();
    assert!(!actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "confluence:mirror-resonance"
    )));

    let distinct = cards(&game, &[(Element::Metal, 4), (Element::Fire, 3)]);
    set_hand(&mut game, distinct.clone());
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &distinct)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "confluence:mirror-resonance"
    )));
}

#[test]
fn acquiring_dao_mage_persists_a_public_limited_use() {
    let mut game = state(false);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("seeker"),
    });
    let transition = cards(&game, &[(Element::Fire, 1), (Element::Fire, 5)]);
    set_hand(&mut game, transition.clone());
    let events = handle_command(
        &game,
        Command::ChangeProfession {
            player: PlayerId::new("p2"),
            profession: ProfessionId::new("confluence:dao-mage"),
            cards: transition,
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::LimitedUseChanged {
            key,
            new_remaining: 1,
            maximum: 1,
            ..
        } if key == "confluence:imprisoning-array"
    )));
    for event in &events {
        apply_event(&mut game, event);
    }
    assert_eq!(state_for(&game, Viewer::Observer).limited_uses.len(), 1);
}

#[test]
fn tuning_is_offered_only_when_the_retrieved_card_can_complete_a_profession_change() {
    let mut game = state(false);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:tuner"),
    });
    let residual = cards(&game, &[(Element::Metal, 2)])[0];
    set_residual(&mut game, residual);
    let hand = cards(&game, &[(Element::Fire, 3), (Element::Metal, 4)]);
    set_hand(&mut game, hand.clone());
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &[hand[0]])
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ActivateProfessionAbility(candidate)
            if candidate.ability_id == "confluence:tuning"
    )));
}

#[test]
fn tuning_is_offered_when_easy_string_can_complete_an_inherited_profession_formation() {
    let mut game = state(false);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:string-changer"),
    });
    let residual = cards(&game, &[(Element::Metal, 2)])[0];
    set_residual(&mut game, residual);
    let hand = cards(&game, &[(Element::Fire, 5), (Element::Fire, 2)]);
    set_hand(&mut game, hand.clone());

    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &[hand[0]])
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ActivateProfessionAbility(candidate)
            if candidate.ability_id == "confluence:tuning"
    )));

    let events = handle_command(
        &game,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p2"),
            ability_id: "confluence:tuning".to_string(),
            cards: vec![hand[0]],
            target_card: None,
            declared_element: None,
            declared_level: None,
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut game, event);
    }

    let mirror_cards = vec![residual, hand[1]];
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &mirror_cards)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "confluence:mirror-resonance"
    )));

    assert!(
        handle_command(
            &game,
            Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "metal-strike".to_string(),
                cards: vec![residual],
                declared_targets: vec![],
            },
        )
        .is_err()
    );

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "confluence:mirror-resonance".to_string(),
            cards: mirror_cards,
            declared_targets: vec![],
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::ConfluenceCardObligationCleared { owner, card }
            if owner == &PlayerId::new("p2") && *card == residual
    )));
}

#[test]
fn voluntary_non_action_effect_cannot_consume_the_only_tuning_completion_path() {
    let mut game = state(false);
    game.enabled_rule_modules
        .push(RuleModuleId::new(SPIRIT_MODULE_ID));
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:string-changer"),
    });
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Water,
        power: 1,
    });
    let residual = cards(&game, &[(Element::Metal, 2)])[0];
    set_residual(&mut game, residual);
    let hand = cards(&game, &[(Element::Fire, 5), (Element::Fire, 2)]);
    set_hand(&mut game, hand.clone());

    let events = handle_command(
        &game,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p2"),
            ability_id: "confluence:tuning".to_string(),
            cards: vec![hand[0]],
            target_card: None,
            declared_element: None,
            declared_level: None,
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut game, event);
    }
    assert!(game.confluence_card_obligations.iter().any(|obligation| {
        obligation.owner == PlayerId::new("p2")
            && obligation.card == residual
            && obligation.applied_on_turn == game.turn_number
    }));

    assert!(
        handle_command(
            &game,
            Command::UseSpiritSkill {
                player: PlayerId::new("p2"),
                skill: SpiritSkill::Flow,
                selected_card: Some(hand[1]),
                declared_level: None,
            },
        )
        .is_err()
    );

    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &[hand[1]])
        .unwrap();
    assert!(!actions.iter().any(|action| matches!(
        action,
        PlayableAction::UseSpiritSkill(candidate) if candidate.skill == SpiritSkill::Flow
    )));
}

#[test]
fn blaze_resonance_affects_previous_player_not_their_death_spirit_teammate() {
    let mut game = team_state(vec![
        RuleModuleId::new(SPIRIT_MODULE_ID),
        RuleModuleId::new(DARK_GLIMMER_MODULE_ID),
    ]);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:tuner"),
    });
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p3"),
        spirit: SpiritKind::Death,
        power: 4,
    });
    let residual = cards(&game, &[(Element::Fire, 2)])[0];
    set_residual(&mut game, residual);
    let used = cards(&game, &[(Element::Fire, 3), (Element::Metal, 2)]);
    set_hand(&mut game, used.clone());

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "confluence:blaze-resonance".to_string(),
            cards: used,
            declared_targets: vec![],
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team == TeamId::new("team:a") && change.effective_delta == -20
    )));
    assert!(!events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team == TeamId::new("team:b") && change.delta == -10
    )));
}

#[test]
fn forest_resonance_recovery_restriction_checks_performer_not_teammate() {
    let mut game = team_state(vec![RuleModuleId::new(JIANGHU_MODULE_ID)]);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:tuner"),
    });
    game.hp
        .iter_mut()
        .find(|hp| hp.team == TeamId::new("team:b"))
        .unwrap()
        .hp = 80;
    game.jianghu_states.push(JianghuState {
        owner: PlayerId::new("p4"),
        kind: JianghuStateKind::Poison,
        remaining_turns: 1,
        expires_on_turn: None,
        last_resolved_turn: None,
    });
    let residual = cards(&game, &[(Element::Wood, 2)])[0];
    set_residual(&mut game, residual);
    let used = cards(&game, &[(Element::Wood, 3), (Element::Metal, 2)]);
    set_hand(&mut game, used.clone());

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "confluence:forest-resonance".to_string(),
            cards: used,
            declared_targets: vec![],
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team == TeamId::new("team:b") && change.effective_delta == 20
    )));
}

#[test]
fn thousand_resonance_resolves_residual_mirror_choice_before_selected_resonance() {
    let mut game = state(false);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:string-changer"),
    });
    game.hp
        .iter_mut()
        .find(|hp| hp.team == TeamId::new("team:a"))
        .unwrap()
        .hp = 10;
    let residual = cards(&game, &[(Element::Metal, 2)])[0];
    set_residual(&mut game, residual);
    let used = cards_excluding(
        &game,
        &[(Element::Metal, 2), (Element::Metal, 3), (Element::Fire, 2)],
        &[residual],
    );
    let inspected = cards_excluding(&game, &[(Element::Wood, 5)], &[residual]);
    let role_card = used[0];
    set_hand(&mut game, used.clone());
    set_player_hand(&mut game, "p1", inspected.clone());

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "confluence:thousand-resonance".to_string(),
            cards: used,
            declared_targets: vec![TargetDecl::FormationRole {
                role: "confluence:thousand-resonance:Fire".to_string(),
                card: role_card,
            }],
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
    assert!(!events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team == TeamId::new("team:a") && change.effective_delta < 0
    )));

    for event in &events {
        apply_event(&mut game, event);
    }
    assert!(matches!(game.status, GameStatus::InProgress));
    assert!(game.pending_choice.is_some());

    let events = answer_choice(
        &game,
        PlayerId::new("p2"),
        ChoiceAnswer::Cards { cards: inspected },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team == TeamId::new("team:a") && change.new_hp == 0
    )));
}

#[test]
fn myriad_resonance_defers_game_outcome_until_mirror_choice_finishes() {
    let mut game = state(false);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:heavenly-resonator"),
    });
    game.hp
        .iter_mut()
        .find(|hp| hp.team == TeamId::new("team:a"))
        .unwrap()
        .hp = 10;
    let residual = cards(&game, &[(Element::Fire, 2)])[0];
    set_residual(&mut game, residual);
    let used = cards_excluding(
        &game,
        &[
            (Element::Metal, 1),
            (Element::Wood, 1),
            (Element::Water, 1),
            (Element::Fire, 2),
            (Element::Earth, 1),
        ],
        &[residual],
    );
    let inspected = cards_excluding(&game, &[(Element::Metal, 5)], &[residual]);
    set_hand(&mut game, used.clone());
    set_player_hand(&mut game, "p1", inspected.clone());

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "confluence:myriad-resonance".to_string(),
            cards: used,
            declared_targets: vec![],
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );

    for event in &events {
        apply_event(&mut game, event);
    }
    assert!(matches!(game.status, GameStatus::InProgress));
    assert!(game.pending_choice.is_some());

    let events = answer_choice(
        &game,
        PlayerId::new("p2"),
        ChoiceAnswer::Cards { cards: inspected },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team == TeamId::new("team:a") && change.new_hp == 0
    )));
    assert!(matches!(events.last(), Some(GameEvent::GameEnded { .. })));
    for event in &events {
        apply_event(&mut game, event);
    }
    assert!(matches!(game.status, GameStatus::Finished { .. }));
}

#[test]
fn tailwind_recovers_for_all_shared_deck_owners_but_only_personal_pile_owner() {
    let mut shared = state(false);
    shared.limited_uses = vec![limited("p1", 0), limited("p2", 0)];
    shared.professions = vec![
        PlayerProfession {
            player: PlayerId::new("p1"),
            profession: ProfessionId::new("confluence:clear-wind-envoy"),
        },
        PlayerProfession {
            player: PlayerId::new("p2"),
            profession: ProfessionId::new("confluence:clear-wind-envoy"),
        },
    ];
    let recycled = cards(&shared, &[(Element::Earth, 1)])[0];
    shared.discard.push(recycled);
    apply_event(
        &mut shared,
        &GameEvent::RandomnessRequested {
            request: PendingRandomness {
                request_id: "shared-discard".to_string(),
                operation: RandomnessOperation::DiscardShuffle {
                    pile: RandomnessDeck::Shared,
                    placement: DeckPlacement::Bottom,
                },
                continuation: RandomnessContinuation::Base(
                    fewfc::domain::BaseRandomnessContinuation::TurnDraw,
                ),
                current_order: vec![recycled],
            },
        },
    );
    let events = resolve_trusted_randomness(
        &shared,
        &TrustedRandomnessAnswer {
            request_id: "shared-discard".to_string(),
            shuffled_order: vec![recycled],
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .filter(|event| matches!(event, GameEvent::LimitedUseChanged { .. }))
            .count()
            == 2
    );
    for event in &events {
        apply_event(&mut shared, event);
    }
    assert!(
        shared
            .limited_uses
            .iter()
            .all(|use_count| use_count.remaining == 1)
    );

    let mut personal = state(true);
    personal.limited_uses = vec![limited("p1", 0), limited("p2", 0)];
    personal.professions = vec![
        PlayerProfession {
            player: PlayerId::new("p1"),
            profession: ProfessionId::new("confluence:clear-wind-envoy"),
        },
        PlayerProfession {
            player: PlayerId::new("p2"),
            profession: ProfessionId::new("confluence:clear-wind-envoy"),
        },
    ];
    let recycled = personal.player_discards[0]
        .cards
        .first()
        .copied()
        .unwrap_or_else(|| {
            let card = cards(&personal, &[(Element::Earth, 1)])[0];
            personal.player_discards[0].cards.push(card);
            card
        });
    apply_event(
        &mut personal,
        &GameEvent::RandomnessRequested {
            request: PendingRandomness {
                request_id: "personal-discard".to_string(),
                operation: RandomnessOperation::DiscardShuffle {
                    pile: RandomnessDeck::Player(PlayerId::new("p1")),
                    placement: DeckPlacement::Bottom,
                },
                continuation: RandomnessContinuation::Base(
                    fewfc::domain::BaseRandomnessContinuation::TurnDraw,
                ),
                current_order: vec![recycled],
            },
        },
    );
    let events = resolve_trusted_randomness(
        &personal,
        &TrustedRandomnessAnswer {
            request_id: "personal-discard".to_string(),
            shuffled_order: vec![recycled],
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut personal, event);
    }
    assert_eq!(personal.limited_uses[0].remaining, 1);
    assert_eq!(personal.limited_uses[1].remaining, 0);
}

#[test]
fn deck_shuffles_and_ineligible_tailwind_owners_do_not_emit_recovery_events() {
    let mut state = state(false);
    state.limited_uses = vec![limited("p1", 0), limited("p2", 0)];
    state.professions = vec![
        PlayerProfession {
            player: PlayerId::new("p1"),
            profession: ProfessionId::new("confluence:clear-wind-envoy"),
        },
        PlayerProfession {
            player: PlayerId::new("p2"),
            profession: ProfessionId::new("confluence:clear-wind-envoy"),
        },
    ];
    let shuffled = cards(&state, &[(Element::Earth, 1)])[0];
    state.deck.push(shuffled);
    apply_event(
        &mut state,
        &GameEvent::RandomnessRequested {
            request: PendingRandomness {
                request_id: "forest-deck-shuffle".to_string(),
                operation: RandomnessOperation::DeckShuffle {
                    deck: RandomnessDeck::Shared,
                },
                continuation: RandomnessContinuation::Base(
                    fewfc::domain::BaseRandomnessContinuation::TurnDraw,
                ),
                current_order: vec![shuffled],
            },
        },
    );
    let events = resolve_trusted_randomness(
        &state,
        &TrustedRandomnessAnswer {
            request_id: "forest-deck-shuffle".to_string(),
            shuffled_order: vec![shuffled],
        },
    )
    .unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::LimitedUseChanged { .. }))
    );
}

fn limited(player: &str, remaining: u32) -> LimitedUse {
    LimitedUse {
        owner: PlayerId::new(player),
        key: "confluence:tailwind".to_string(),
        remaining,
        maximum: 1,
    }
}
