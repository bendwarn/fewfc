use fewfc::application::{apply_event, handle_command};
use fewfc::domain::{
    CONFLUENCE_GENERATION_MODULE_ID, CardInstanceId, Command, DARK_GLIMMER_MODULE_ID,
    DeckPlacement, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent, GameState, GameStatus,
    HERO_SCHOOLS_MODULE_ID, JIANGHU_MODULE_ID, JianghuState, JianghuStateKind, LastTurnDiscard,
    LimitedUse, Phase, Player, PlayerId, PlayerProfession, PlayerSpirit, ProfessionId,
    RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID, SpiritKind, SpiritSkill, TargetDecl, TeamId,
};
use fewfc::public_view::{Viewer, state_for};
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
    state.phase = Phase::Main;
    state
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
    state.phase = Phase::Main;
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
                            definition.element == *element && definition.level == *level
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
            .any(|event| matches!(event, GameEvent::EffectChoiceRequested { .. }))
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

    let events = handle_command(
        &game,
        Command::AnswerEffectChoice {
            player: PlayerId::new("p2"),
            selected_cards: inspected,
        },
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
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team == TeamId::new("team:a") && change.new_hp == 0
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::EffectChoiceRequested { .. }))
    );

    for event in &events {
        apply_event(&mut game, event);
    }
    assert!(matches!(game.status, GameStatus::InProgress));
    assert!(game.pending_choice.is_some());

    let events = handle_command(
        &game,
        Command::AnswerEffectChoice {
            player: PlayerId::new("p2"),
            selected_cards: inspected,
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut game, event);
    }
    assert!(matches!(game.status, GameStatus::Finished { .. }));
}

#[test]
fn tailwind_recovers_for_all_shared_deck_owners_but_only_personal_pile_owner() {
    let mut shared = state(false);
    shared.limited_uses = vec![limited("p1", 0), limited("p2", 0)];
    let recycled = cards(&shared, &[(Element::Earth, 1)])[0];
    shared.discard.push(recycled);
    apply_event(
        &mut shared,
        &GameEvent::DiscardRecycledIntoDeck {
            shuffled_order: vec![recycled],
            placement: DeckPlacement::Bottom,
        },
    );
    assert!(
        shared
            .limited_uses
            .iter()
            .all(|use_count| use_count.remaining == 1)
    );

    let mut personal = state(true);
    personal.limited_uses = vec![limited("p1", 0), limited("p2", 0)];
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
        &GameEvent::PlayerDiscardRecycledIntoDeck {
            player: PlayerId::new("p1"),
            shuffled_order: vec![recycled],
            placement: DeckPlacement::Bottom,
        },
    );
    assert_eq!(personal.limited_uses[0].remaining, 1);
    assert_eq!(personal.limited_uses[1].remaining, 0);
}

fn limited(player: &str, remaining: u32) -> LimitedUse {
    LimitedUse {
        owner: PlayerId::new(player),
        key: "confluence:tailwind".to_string(),
        remaining,
        maximum: 1,
    }
}
