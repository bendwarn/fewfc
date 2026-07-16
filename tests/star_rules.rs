use fewfc::application::{GameRecord, apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, Command, CoveredPassive, Element, FiveStarAlignment, GameError, GameEvent,
    GameOutcome, GameSetup, GameState, GameStatus, PassiveTriggerTiming, PendingChoiceKind, Phase,
    Player, PlayerId, PlayerStarHistory, RuleModuleId, STAR_MODULE_ID, StarBreakReason,
    StarElementSubstitution, StarKind, TargetDecl, TeamId, TeamStar, ValidationError,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};

fn two_player_state(stars_enabled: bool) -> GameState {
    let players = vec![
        Player {
            id: PlayerId::new("p1"),
            team: TeamId::new("team:p1"),
        },
        Player {
            id: PlayerId::new("p2"),
            team: TeamId::new("team:p2"),
        },
    ];
    let modules = if stars_enabled {
        vec![RuleModuleId::new(STAR_MODULE_ID)]
    } else {
        Vec::new()
    };
    let setup = OfficialRules::new()
        .configure_game(
            players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            modules,
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state
}

fn team_state() -> GameState {
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
    let setup = OfficialRules::new()
        .configure_game(
            players,
            ["p1", "p2", "p3", "p4"]
                .into_iter()
                .map(PlayerId::new)
                .collect(),
            vec![RuleModuleId::new(STAR_MODULE_ID)],
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state
}

fn card(state: &GameState, element: Element, level: u32) -> CardInstanceId {
    state
        .card_instances
        .iter()
        .find(|instance| {
            state.card_def(instance.instance).is_some_and(|definition| {
                definition.element == element && definition.level == level
            })
        })
        .unwrap()
        .instance
}

fn set_hand(state: &mut GameState, player: &str, cards: Vec<CardInstanceId>) {
    *state.hand_mut(&PlayerId::new(player)).unwrap() = cards;
}

fn perform(
    state: &GameState,
    player: &str,
    formation_id: &str,
    cards: Vec<CardInstanceId>,
) -> Vec<GameEvent> {
    handle_command(
        state,
        Command::PerformFormation {
            player: PlayerId::new(player),
            formation_id: formation_id.to_string(),
            cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap()
}

fn apply_all(state: &mut GameState, events: &[GameEvent]) {
    for event in events {
        apply_event(state, event);
    }
}

fn deck_starting_with(setup: &GameSetup, first: &[CardInstanceId]) -> Vec<CardInstanceId> {
    let mut deck = first.to_vec();
    deck.extend(
        setup
            .card_instances
            .iter()
            .map(|instance| instance.instance)
            .filter(|card| !first.contains(card)),
    );
    deck
}

#[test]
fn base_only_games_do_not_summon_or_expose_stars() {
    let mut state = two_player_state(false);
    let cards = [3, 4, 5]
        .map(|level| card(&state, Element::Metal, level))
        .to_vec();
    set_hand(&mut state, "p1", cards.clone());

    let events = perform(&state, "p1", "triple-metal", cards);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::StarSummoned { .. }))
    );
    let public = state_for(&state, Viewer::Observer);
    assert!(public.team_stars.is_empty());
    assert!(public.star_histories.is_empty());
    assert!(public.five_star_alignment.is_none());
}

#[test]
fn each_qualifying_base_attack_summons_its_star() {
    let cases = [
        (Element::Metal, "triple-metal", StarKind::Metal),
        (Element::Wood, "triple-wood", StarKind::Wood),
        (Element::Water, "triple-water", StarKind::Water),
        (Element::Fire, "triple-fire", StarKind::Fire),
        (Element::Earth, "triple-earth", StarKind::Earth),
    ];

    for (element, formation_id, expected_star) in cases {
        let mut state = two_player_state(true);
        let cards = [3, 4, 5].map(|level| card(&state, element, level)).to_vec();
        set_hand(&mut state, "p1", cards.clone());
        let events = perform(&state, "p1", formation_id, cards);

        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::StarSummoned { player, star, .. }
                if player == &PlayerId::new("p1") && *star == expected_star
        )));
        apply_all(&mut state, &events);
        assert_eq!(
            state.star_for_team(&TeamId::new("team:p1")),
            Some(expected_star)
        );
        assert_eq!(
            state.summoned_stars_for(&PlayerId::new("p1")).unwrap(),
            &[expected_star]
        );
    }
}

#[test]
fn base_attack_below_thirty_points_does_not_summon() {
    let mut state = two_player_state(true);
    let cards = [1, 2, 3]
        .map(|level| card(&state, Element::Metal, level))
        .to_vec();
    set_hand(&mut state, "p1", cards.clone());

    let events = perform(&state, "p1", "triple-metal", cards);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::StarSummoned { .. }))
    );
}

#[test]
fn summoning_replaces_own_star_breaks_opposing_star_and_never_duplicates() {
    let mut state = two_player_state(true);
    state.team_stars = vec![
        TeamStar {
            team: TeamId::new("team:p1"),
            star: StarKind::Earth,
        },
        TeamStar {
            team: TeamId::new("team:p2"),
            star: StarKind::Wood,
        },
    ];
    let cards = [3, 4, 5]
        .map(|level| card(&state, Element::Metal, level))
        .to_vec();
    set_hand(&mut state, "p1", cards.clone());
    let events = perform(&state, "p1", "triple-metal", cards);

    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GameEvent::StarBroken { .. }))
            .count(),
        2
    );
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::StarBroken {
            star: StarKind::Earth,
            reason: StarBreakReason::Replaced,
            ..
        }
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::StarBroken {
            star: StarKind::Wood,
            reason: StarBreakReason::OpposedBy(StarKind::Metal),
            ..
        }
    )));
    apply_all(&mut state, &events);
    assert_eq!(state.team_stars.len(), 1);
    assert_eq!(state.team_stars[0].star, StarKind::Metal);

    state.phase = Phase::Main;
    let cards = [1, 2, 3]
        .map(|level| card(&state, Element::Metal, level))
        .to_vec();
    set_hand(&mut state, "p1", cards.clone());
    let repeated = perform(&state, "p1", "triple-metal", cards);
    assert!(!repeated.iter().any(|event| matches!(
        event,
        GameEvent::StarSummoned { .. } | GameEvent::StarBroken { .. }
    )));
}

#[test]
fn owned_star_enables_both_star_formations_and_one_base_substitution() {
    let mut state = two_player_state(true);
    state.team_stars.push(TeamStar {
        team: TeamId::new("team:p1"),
        star: StarKind::Metal,
    });

    let metal = card(&state, Element::Metal, 2);
    set_hand(&mut state, "p1", vec![metal]);
    let strike_actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &[metal])
        .unwrap();
    assert!(strike_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(formation)
            if formation.formation_id == "taibai-star-strike"
    )));

    let star_cards = vec![
        card(&state, Element::Metal, 2),
        card(&state, Element::Metal, 3),
        card(&state, Element::Earth, 4),
    ];
    set_hand(&mut state, "p1", star_cards.clone());
    let formation_actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &star_cards)
        .unwrap();
    assert!(formation_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(formation)
            if formation.formation_id == "taibai-heaven-forging"
    )));

    let substituted = vec![
        card(&state, Element::Metal, 1),
        card(&state, Element::Earth, 1),
    ];
    set_hand(&mut state, "p1", substituted.clone());
    let base_actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &substituted)
        .unwrap();
    assert!(base_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(formation) if formation.formation_id == "weapon"
    )));

    let earth = card(&state, Element::Earth, 1);
    set_hand(&mut state, "p1", vec![earth]);
    let no_star_substitution = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &[earth])
        .unwrap();
    assert!(!no_star_substitution.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(formation)
            if formation.formation_id == "taibai-star-strike"
    )));
}

#[test]
fn substituted_passive_records_and_redacts_the_declared_card() {
    let mut state = two_player_state(true);
    state.team_stars.push(TeamStar {
        team: TeamId::new("team:p1"),
        star: StarKind::Wood,
    });
    let wood = card(&state, Element::Wood, 1);
    let water = card(&state, Element::Water, 2);
    let cards = vec![wood, water];
    let substitution = StarElementSubstitution {
        card: water,
        printed_element: Element::Water,
        interpreted_element: Element::Wood,
    };
    set_hand(&mut state, "p1", cards.clone());

    let actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &cards)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "defense"
                && candidate.star_substitution.as_ref() == Some(&substitution)
    )));
    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards,
            declared_targets: vec![TargetDecl::Card(water)],
        },
    )
    .unwrap();
    assert!(matches!(
        events.as_slice(),
        [GameEvent::PassiveCovered {
            star_substitution: Some(actual),
            ..
        }] if actual == &substitution
    ));

    apply_all(&mut state, &events);
    assert_eq!(
        state.covered_passives[0].star_substitution,
        Some(substitution.clone())
    );

    let owner = state_for(&state, Viewer::Player(PlayerId::new("p1")));
    assert_eq!(
        owner.covered_passives[0].star_substitution,
        Some(substitution)
    );
    let opponent = state_for(&state, Viewer::Player(PlayerId::new("p2")));
    assert_eq!(opponent.covered_passives[0].star_substitution, None);
    let observer = state_for(&state, Viewer::Observer);
    assert_eq!(observer.covered_passives[0].star_substitution, None);
}

#[test]
fn multiple_star_substitution_options_require_the_selected_card() {
    let mut state = two_player_state(true);
    state.team_stars.push(TeamStar {
        team: TeamId::new("team:p1"),
        star: StarKind::Wood,
    });
    let first_water = card(&state, Element::Water, 1);
    let second_water = card(&state, Element::Water, 2);
    let fire = card(&state, Element::Fire, 3);
    let cards = vec![first_water, second_water, fire];
    set_hand(&mut state, "p1", cards.clone());

    let substitutions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &cards)
        .unwrap()
        .into_iter()
        .filter_map(|action| match action {
            PlayableAction::PerformFormation(candidate)
                if candidate.formation_id == "generating-formation" =>
            {
                candidate
                    .star_substitution
                    .map(|substitution| substitution.card)
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(substitutions, vec![first_water, second_water]);

    assert_eq!(
        handle_command(
            &state,
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "generating-formation".to_string(),
                cards: cards.clone(),
                declared_targets: Vec::new(),
            },
        ),
        Err(GameError::Validation(
            ValidationError::FormationMatchOptionRequired {
                formation_id: "generating-formation".to_string(),
            }
        ))
    );
    assert!(
        handle_command(
            &state,
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "generating-formation".to_string(),
                cards,
                declared_targets: vec![TargetDecl::Card(second_water)],
            },
        )
        .is_ok()
    );
}

#[test]
fn substituted_passive_replays_and_verifies_the_exact_declared_card() {
    let setup = OfficialRules::new()
        .configure_game(
            vec![
                Player {
                    id: PlayerId::new("p1"),
                    team: TeamId::new("team:p1"),
                },
                Player {
                    id: PlayerId::new("p2"),
                    team: TeamId::new("team:p2"),
                },
            ],
            ["p1", "p2"].into_iter().map(PlayerId::new).collect(),
            vec![RuleModuleId::new(STAR_MODULE_ID)],
        )
        .unwrap();
    let initial = GameState::from_setup(&setup);
    let wood_1 = card(&initial, Element::Wood, 1);
    let wood_3 = card(&initial, Element::Wood, 3);
    let wood_4 = card(&initial, Element::Wood, 4);
    let wood_5 = card(&initial, Element::Wood, 5);
    let water_1 = card(&initial, Element::Water, 1);
    let water_2 = card(&initial, Element::Water, 2);
    let metal_1 = card(&initial, Element::Metal, 1);
    let metal_2 = card(&initial, Element::Metal, 2);
    let fire_1 = card(&initial, Element::Fire, 1);
    let earth_1 = card(&initial, Element::Earth, 1);
    let earth_2 = card(&initial, Element::Earth, 2);
    let earth_3 = card(&initial, Element::Earth, 3);
    let first_cards = vec![
        wood_3, wood_4, wood_5, wood_1, metal_1, metal_2, water_1, fire_1, earth_3, water_2,
        earth_1, earth_2,
    ];
    let deck = deck_starting_with(&setup, &first_cards);
    let mut record = GameRecord::start(setup, deck).unwrap();

    let advance_to_choice_or_main = |record: &mut GameRecord| loop {
        record.advance_until_decision().unwrap();
        if record.state().pending_choice.is_some() || record.state().phase == Phase::Main {
            break;
        }
    };
    advance_to_choice_or_main(&mut record);
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "triple-wood".to_string(),
            cards: vec![wood_3, wood_4, wood_5],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_to_choice_or_main(&mut record);
    let first_discard = match &record.state().pending_choice.as_ref().unwrap().kind {
        PendingChoiceKind::TurnDrawDiscard {
            allowed_discards, ..
        } => *allowed_discards
            .iter()
            .find(|card| **card != water_2)
            .unwrap(),
        other => panic!("unexpected first draw choice: {other:?}"),
    };
    record
        .handle(Command::ChooseTurnDiscard {
            player: PlayerId::new("p1"),
            discard: first_discard,
        })
        .unwrap();
    advance_to_choice_or_main(&mut record);
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            cards: vec![metal_1],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_to_choice_or_main(&mut record);
    let second_discard = match &record.state().pending_choice.as_ref().unwrap().kind {
        PendingChoiceKind::TurnDrawDiscard {
            allowed_discards, ..
        } => allowed_discards[0],
        other => panic!("unexpected second draw choice: {other:?}"),
    };
    record
        .handle(Command::ChooseTurnDiscard {
            player: PlayerId::new("p2"),
            discard: second_discard,
        })
        .unwrap();
    advance_to_choice_or_main(&mut record);

    let events = record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![wood_1, water_2],
            declared_targets: vec![TargetDecl::Card(water_2)],
        })
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [GameEvent::PassiveCovered {
            star_substitution: Some(StarElementSubstitution { card, .. }),
            ..
        }] if *card == water_2
    ));
    assert_eq!(record.replay().unwrap(), *record.state());
    assert!(record.verify_replay().is_ok());

}

#[test]
fn three_card_star_formation_breaks_star_and_grants_draw_even_when_damage_is_prevented() {
    let mut state = two_player_state(true);
    state.team_stars.push(TeamStar {
        team: TeamId::new("team:p1"),
        star: StarKind::Metal,
    });
    let passive_cards = vec![
        card(&state, Element::Wood, 1),
        card(&state, Element::Wood, 2),
    ];
    state.covered_passives.push(CoveredPassive {
        owner: PlayerId::new("p2"),
        formation_id: "defense".to_string(),
        cards: passive_cards,
        sealed: false,
        covered_on_turn: 0,
        reveal_timing: PassiveTriggerTiming::NextPlayerActionStart,
        star_substitution: None,
    });
    let cards = vec![
        card(&state, Element::Metal, 2),
        card(&state, Element::Metal, 3),
        card(&state, Element::Earth, 4),
    ];
    set_hand(&mut state, "p1", cards.clone());

    let events = perform(&state, "p1", "taibai-heaven-forging", cards);
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::TurnDrawBonusChanged { delta: 1, .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::StarBroken {
            star: StarKind::Metal,
            reason: StarBreakReason::StarFormationUsed { .. },
            ..
        }
    )));
    apply_all(&mut state, &events);
    assert_eq!(
        state.turn_draw_bonus_by_player.get(&PlayerId::new("p1")),
        Some(&1)
    );
    assert!(state.team_stars.is_empty());
}

#[test]
fn void_star_breaking_is_atomic_and_seal_cancels_every_consequence() {
    let mut state = two_player_state(true);
    state.team_stars = vec![
        TeamStar {
            team: TeamId::new("team:p1"),
            star: StarKind::Metal,
        },
        TeamStar {
            team: TeamId::new("team:p2"),
            star: StarKind::Water,
        },
    ];
    for hp in &mut state.hp {
        hp.hp = 20;
    }
    let cards = [Element::Metal, Element::Wood, Element::Fire]
        .map(|element| card(&state, element, 3))
        .to_vec();
    set_hand(&mut state, "p1", cards.clone());
    let events = perform(&state, "p1", "void-star-breaking", cards);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GameEvent::StarBroken { .. }))
            .count(),
        2
    );
    assert!(matches!(
        events.last(),
        Some(GameEvent::VoidStarBreakingCompleted { .. })
    ));
    apply_all(&mut state, &events);
    assert!(state.team_stars.is_empty());
    assert_eq!(
        state.status,
        GameStatus::Finished {
            outcome: GameOutcome::Draw
        }
    );

    let mut sealed = two_player_state(true);
    sealed.team_stars.push(TeamStar {
        team: TeamId::new("team:p1"),
        star: StarKind::Metal,
    });
    sealed.covered_passives.push(CoveredPassive {
        owner: PlayerId::new("p2"),
        formation_id: "seal".to_string(),
        cards: vec![
            card(&sealed, Element::Water, 1),
            card(&sealed, Element::Water, 2),
        ],
        sealed: false,
        covered_on_turn: 0,
        reveal_timing: PassiveTriggerTiming::NextPlayerActionStart,
        star_substitution: None,
    });
    let cards = [Element::Metal, Element::Wood, Element::Fire]
        .map(|element| card(&sealed, element, 3))
        .to_vec();
    set_hand(&mut sealed, "p1", cards.clone());
    let cancelled = perform(&sealed, "p1", "void-star-breaking", cards);
    assert!(!cancelled.iter().any(|event| matches!(
        event,
        GameEvent::StarBroken { .. } | GameEvent::VoidStarBreakingCompleted { .. }
    )));
    apply_all(&mut sealed, &cancelled);
    assert_eq!(sealed.team_stars.len(), 1);
}

#[test]
fn five_star_alignment_overrides_the_same_formations_hp_result() {
    let mut state = two_player_state(true);
    state.star_histories = vec![
        PlayerStarHistory {
            player: PlayerId::new("p1"),
            stars: vec![
                StarKind::Wood,
                StarKind::Water,
                StarKind::Fire,
                StarKind::Earth,
            ],
        },
        PlayerStarHistory {
            player: PlayerId::new("p2"),
            stars: Vec::new(),
        },
    ];
    for hp in &mut state.hp {
        hp.hp = 1;
    }
    state.covered_passives.push(CoveredPassive {
        owner: PlayerId::new("p2"),
        formation_id: "countershock".to_string(),
        cards: vec![
            card(&state, Element::Fire, 1),
            card(&state, Element::Fire, 2),
        ],
        sealed: false,
        covered_on_turn: 0,
        reveal_timing: PassiveTriggerTiming::NextPlayerActionStart,
        star_substitution: None,
    });
    let cards = [3, 4, 5]
        .map(|level| card(&state, Element::Metal, level))
        .to_vec();
    set_hand(&mut state, "p1", cards.clone());

    let events = perform(&state, "p1", "triple-metal", cards);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FiveStarAlignmentAchieved { player, .. }
            if player == &PlayerId::new("p1")
    )));
    apply_all(&mut state, &events);
    assert_eq!(
        state.five_star_alignment,
        Some(FiveStarAlignment {
            player: PlayerId::new("p1"),
            team: TeamId::new("team:p1"),
        })
    );
    assert_eq!(
        state.status,
        GameStatus::Finished {
            outcome: GameOutcome::Team(TeamId::new("team:p1"))
        }
    );
}

#[test]
fn teammates_share_owned_star_but_not_alignment_history() {
    let mut state = team_state();
    state.team_stars.push(TeamStar {
        team: TeamId::new("team:a"),
        star: StarKind::Metal,
    });
    state.current_turn_index = 2;
    let metal = card(&state, Element::Metal, 2);
    set_hand(&mut state, "p3", vec![metal]);
    let teammate_actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p3"), &[metal])
        .unwrap();
    assert!(teammate_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(formation)
            if formation.formation_id == "taibai-star-strike"
    )));

    state.star_histories = vec![
        PlayerStarHistory {
            player: PlayerId::new("p1"),
            stars: vec![StarKind::Wood, StarKind::Water, StarKind::Fire],
        },
        PlayerStarHistory {
            player: PlayerId::new("p2"),
            stars: Vec::new(),
        },
        PlayerStarHistory {
            player: PlayerId::new("p3"),
            stars: vec![StarKind::Metal, StarKind::Earth],
        },
        PlayerStarHistory {
            player: PlayerId::new("p4"),
            stars: Vec::new(),
        },
    ];
    state.team_stars.clear();
    state.current_turn_index = 0;
    let cards = [3, 4, 5]
        .map(|level| card(&state, Element::Earth, level))
        .to_vec();
    set_hand(&mut state, "p1", cards.clone());
    let events = perform(&state, "p1", "triple-earth", cards);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::FiveStarAlignmentAchieved { .. }))
    );
}

#[test]
fn star_events_replay_identically_in_two_player_and_team_games() {
    let setup_cases = vec![
        OfficialRules::new()
            .configure_game(
                vec![
                    Player {
                        id: PlayerId::new("p1"),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: PlayerId::new("p2"),
                        team: TeamId::new("team:p2"),
                    },
                ],
                ["p1", "p2"].into_iter().map(PlayerId::new).collect(),
                vec![RuleModuleId::new(STAR_MODULE_ID)],
            )
            .unwrap(),
        OfficialRules::new()
            .configure_game(
                vec![
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
                ],
                ["p1", "p2", "p3", "p4"]
                    .into_iter()
                    .map(PlayerId::new)
                    .collect(),
                vec![RuleModuleId::new(STAR_MODULE_ID)],
            )
            .unwrap(),
    ];

    for setup in setup_cases {
        let initial = GameState::from_setup(&setup);
        let cards = [3, 4, 5]
            .map(|level| card(&initial, Element::Metal, level))
            .to_vec();
        let mut first_cards = cards.clone();
        first_cards.push(card(&initial, Element::Wood, 1));
        let deck = deck_starting_with(&setup, &first_cards);
        let mut record = GameRecord::start(setup, deck).unwrap();
        record.advance_until_decision().unwrap();
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "triple-metal".to_string(),
                cards,
                declared_targets: Vec::new(),
            })
            .unwrap();

        assert_eq!(
            record
                .state()
                .star_for_team(&record.state().players[0].team),
            Some(StarKind::Metal)
        );
        assert_eq!(record.replay().unwrap(), *record.state());
        assert!(record.verify_replay().is_ok());
    }
}
