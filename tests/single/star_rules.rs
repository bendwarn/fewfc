use fewfc::application::{GameRecord, apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, Command, Element, FiveStarAlignment, GameError, GameEvent, GameOutcome,
    GameSetup, GameState, GameStatus, Phase, Player, PlayerId, RuleModuleId, STAR_MODULE_ID,
    StarBreakReason, StarElementSubstitution, StarKind, TargetDecl, TeamId, TeamStar,
    ValidationError,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};
mod support;
use support::record as play;

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
    state.phase = Phase::ActiveEffects;
    state
}

fn card(state: &GameState, element: Element, level: u32) -> CardInstanceId {
    state
        .card_instances
        .iter()
        .find(|instance| {
            state.card_def(instance.instance).is_some_and(|definition| {
                definition.element == element && definition.level.value() == level
            })
        })
        .unwrap()
        .instance
}

fn set_hand(state: &mut GameState, player: &str, cards: Vec<CardInstanceId>) {
    *state.hand_mut(&PlayerId::new(player)).unwrap() = cards;
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
    let p1 = PlayerId::new("p1");
    for enabled in [false, true] {
        let modules = if enabled { &[STAR_MODULE_ID][..] } else { &[] };
        let mut record = play::start(play::setup(2, modules), &[9, 13, 16, 1]);
        let events = play::perform(&mut record, &p1, "triple-metal", &[9, 13, 16]);
        assert_eq!(
            events
                .iter()
                .any(|event| matches!(event, GameEvent::StarSummoned { .. })),
            enabled
        );
        assert_eq!(base_attack_points(&events), 36);
        let public = state_for(record.state(), Viewer::Observer);
        assert_eq!(public.team_stars.is_empty(), !enabled);
        assert_eq!(public.star_histories.is_empty(), !enabled);
        assert!(public.five_star_alignment.is_none());
        play::assert_replay(&record);
    }
}

#[test]
fn each_qualifying_base_attack_summons_its_star() {
    let p1 = PlayerId::new("p1");
    let team = TeamId::new("team:a");
    for (formation, star, offset) in [
        ("triple-metal", StarKind::Metal, 0),
        ("triple-wood", StarKind::Wood, 18),
        ("triple-water", StarKind::Water, 36),
        ("triple-fire", StarKind::Fire, 54),
        ("triple-earth", StarKind::Earth, 72),
    ] {
        let cards = [9 + offset, 13 + offset, 16 + offset];
        let mut record = play::start(play::setup(2, &[STAR_MODULE_ID]), &cards);
        let events = play::perform(&mut record, &p1, formation, &cards);
        assert!(events.contains(&GameEvent::StarSummoned {
            player: p1.clone(),
            team: team.clone(),
            star
        }));
        assert_eq!(base_attack_points(&events), 36);
        assert_eq!(record.state().star_for_team(&team), Some(star));
        assert_eq!(
            record.state().summoned_stars_for(&p1),
            Some([star].as_slice())
        );
        assert_eq!(record.state().discard, play::ids(&cards));
        play::assert_replay(&record);
    }
}

#[test]
fn base_attack_below_thirty_points_does_not_summon() {
    let p1 = PlayerId::new("p1");
    for (cards, points, summons) in [([1, 5, 9], 18, false), ([1, 13, 16], 30, true)] {
        let mut record = play::start(play::setup(2, &[STAR_MODULE_ID]), &cards);
        let events = play::perform(&mut record, &p1, "triple-metal", &cards);
        assert_eq!(base_attack_points(&events), points);
        assert_eq!(
            events
                .iter()
                .any(|event| matches!(event, GameEvent::StarSummoned { .. })),
            summons
        );
        assert_eq!(record.state().discard, play::ids(&cards));
        play::assert_replay(&record);
    }
}

#[test]
fn summoning_replaces_own_star_breaks_opposing_star_and_never_duplicates() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = play::start_with_reserves(
        play::setup(2, &[STAR_MODULE_ID]),
        &[&[1], &[27, 31, 34]],
        &[&[81, 85, 88, 9, 13, 16, 10, 14, 17], &[]],
    );
    play::perform(&mut record, &p1, "metal-strike", &[1]);
    play::finish(&mut record, &p1);
    play::perform(&mut record, &p2, "triple-wood", &[27, 31, 34]);
    play::finish(&mut record, &p2);
    play::perform(&mut record, &p1, "triple-earth", &[81, 85, 88]);
    play::finish(&mut record, &p1);
    play::strike_with_spare(&mut record, &p2, &[]);
    play::finish(&mut record, &p2);

    let events = play::perform(&mut record, &p1, "triple-metal", &[9, 13, 16]);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GameEvent::StarBroken { .. }))
            .count(),
        2
    );
    assert!(events.contains(&GameEvent::StarBroken {
        team: TeamId::new("team:a"),
        star: StarKind::Earth,
        reason: StarBreakReason::Replaced,
        hp_change: None,
    }));
    assert!(events.contains(&GameEvent::StarBroken {
        team: TeamId::new("team:b"),
        star: StarKind::Wood,
        reason: StarBreakReason::OpposedBy(StarKind::Metal),
        hp_change: None,
    }));
    let ownership = vec![TeamStar {
        team: TeamId::new("team:a"),
        star: StarKind::Metal,
    }];
    assert_eq!(record.state().team_stars, ownership);
    play::finish(&mut record, &p1);
    play::strike_with_spare(&mut record, &p2, &[]);
    play::finish(&mut record, &p2);

    let repeated = play::perform(&mut record, &p1, "triple-metal", &[10, 14, 17]);
    assert_eq!(base_attack_points(&repeated), 36);
    assert!(!repeated.iter().any(|event| matches!(
        event,
        GameEvent::StarSummoned { .. } | GameEvent::StarBroken { .. }
    )));
    assert_eq!(record.state().team_stars, ownership);
    assert_eq!(
        record.state().summoned_stars_for(&p1),
        Some([StarKind::Earth, StarKind::Metal].as_slice())
    );
    play::assert_replay(&record);
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
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveCovered {
            star_substitution: Some(actual),
            ..
        } if actual == &substitution
    )));

    apply_all(&mut state, &events);
    assert_eq!(
        state
            .covered_passive(&PlayerId::new("p1"))
            .unwrap()
            .star_substitution,
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
fn void_star_breaking_is_atomic_and_seal_cancels_every_consequence() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    for sealed in [false, true] {
        let mut setup = play::setup(2, &[STAR_MODULE_ID]);
        // 兩次 36 點攻擊後，雙方各留下破星術可同時扣完的 20 點。
        setup.hp.iter_mut().for_each(|entry| entry.hp = 56);
        let mut record = play::start_with_reserves(
            setup,
            &[&[9, 13, 16], &[45, 49, 52, 37, 38]],
            &[&[10, 27, 63], &[]],
        );
        play::perform(&mut record, &p1, "triple-metal", &[9, 13, 16]);
        play::finish(&mut record, &p1);
        if sealed {
            play::perform(&mut record, &p2, "seal", &[37, 38]);
        } else {
            play::perform(&mut record, &p2, "triple-water", &[45, 49, 52]);
        }
        play::finish(&mut record, &p2);
        let hp = record.state().hp.clone();
        let stars = record.state().team_stars.clone();
        let events = play::perform(&mut record, &p1, "void-star-breaking", &[10, 27, 63]);
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(event, GameEvent::StarBroken { .. }))
                .count(),
            if sealed { 0 } else { 2 }
        );
        assert_eq!(
            events
                .iter()
                .any(|event| matches!(event, GameEvent::VoidStarBreakingCompleted { .. })),
            !sealed
        );
        if sealed {
            assert_eq!(record.state().team_stars, stars);
            assert_eq!(record.state().hp, hp);
            assert!(record.state().discard.ends_with(&play::ids(&[10, 27, 63])));
        } else {
            assert_eq!(
                hp.iter().map(|entry| entry.hp).collect::<Vec<_>>(),
                [20, 20]
            );
            assert!(record.state().team_stars.is_empty());
            assert!(
                matches!(&record.state().status, GameStatus::Finished { conclusion } if conclusion.outcome == GameOutcome::Draw)
            );
        }
        play::assert_replay(&record);
    }
}

#[test]
fn five_star_alignment_overrides_the_same_formations_hp_result() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = play::setup(2, &[STAR_MODULE_ID, fewfc::domain::HERO_SCHOOLS_MODULE_ID]);
    setup.hp[0].hp = 1;
    setup.hp[1].hp = 145;
    let mut record = play::start_with_reserves(
        setup,
        &[&[27, 31, 34], &[1, 55, 56]],
        &[
            &[45, 49, 52, 63, 67, 70, 81, 85, 88, 9, 13, 16],
            &[19, 20, 35],
        ],
    );
    for (id, cards, profession) in [
        (
            "triple-wood",
            [27, 31, 34],
            Some(("first-wanderer", &[1][..])),
        ),
        ("triple-water", [45, 49, 52], Some(("seeker", &[19][..]))),
        (
            "triple-fire",
            [63, 67, 70],
            Some(("expounder", &[20, 35][..])),
        ),
        ("triple-earth", [81, 85, 88], None),
    ] {
        play::perform(&mut record, &p1, id, &cards);
        play::finish(&mut record, &p1);
        if let Some((profession, cards)) = profession {
            play::change(&mut record, &p2, profession, cards);
        } else {
            play::perform(&mut record, &p2, "countershock", &[55, 56]);
        }
        play::finish(&mut record, &p2);
    }
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .map(|entry| entry.hp)
            .collect::<Vec<_>>(),
        [1, 1]
    );
    assert_eq!(record.state().summoned_stars_for(&p1).unwrap().len(), 4);

    let events = play::perform(&mut record, &p1, "triple-metal", &[9, 13, 16]);
    let team = TeamId::new("team:a");
    assert!(events.contains(&GameEvent::FiveStarAlignmentAchieved {
        player: p1.clone(),
        team: team.clone()
    }));
    assert!(events.iter().any(|event| matches!(event,
        GameEvent::AttackResolved { hp_changes, .. }
            if hp_changes.len() == 2 && hp_changes.iter().all(|entry| entry.change.new_hp() == 0)
    )));
    assert_eq!(
        record.state().five_star_alignment,
        Some(FiveStarAlignment {
            player: p1,
            team: team.clone()
        })
    );
    assert!(
        matches!(&record.state().status, GameStatus::Finished { conclusion } if conclusion.outcome == GameOutcome::Winner(team))
    );
    play::assert_replay(&record);
}

#[test]
fn teammates_share_owned_star_but_not_alignment_history() {
    let [p1, p2, p3, p4] = ["p1", "p2", "p3", "p4"].map(PlayerId::new);
    let mut record = play::start_with_reserves(
        play::setup(4, &[STAR_MODULE_ID]),
        &[&[9, 13, 16], &[], &[5, 81, 85, 88], &[]],
        &[&[45, 49, 52, 63, 67, 70], &[], &[27, 31, 34], &[]],
    );
    play::perform(&mut record, &p1, "triple-metal", &[9, 13, 16]);
    play::finish(&mut record, &p1);
    play::strike_with_spare(&mut record, &p2, &[]);
    play::finish(&mut record, &p2);
    let actions = record.playable_actions(&p3, &play::ids(&[5])).unwrap();
    assert!(actions.iter().any(|action| matches!(action,
        PlayableAction::PerformFormation(formation) if formation.formation_id == "taibai-star-strike"
    )));
    play::perform(&mut record, &p3, "triple-earth", &[81, 85, 88]);
    play::finish(&mut record, &p3);
    play::strike_with_spare(&mut record, &p4, &[]);
    play::finish(&mut record, &p4);
    play::perform(&mut record, &p1, "triple-water", &[45, 49, 52]);
    play::finish(&mut record, &p1);
    play::strike_with_spare(&mut record, &p2, &[]);
    play::finish(&mut record, &p2);
    play::perform(&mut record, &p3, "triple-wood", &[27, 31, 34]);
    play::finish(&mut record, &p3);
    play::strike_with_spare(&mut record, &p4, &[]);
    play::finish(&mut record, &p4);
    let events = play::perform(&mut record, &p1, "triple-fire", &[63, 67, 70]);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::FiveStarAlignmentAchieved { .. }))
    );
    assert_eq!(
        record.state().summoned_stars_for(&p1),
        Some([StarKind::Metal, StarKind::Water, StarKind::Fire].as_slice())
    );
    assert_eq!(
        record.state().summoned_stars_for(&p3),
        Some([StarKind::Earth, StarKind::Wood].as_slice())
    );
    assert!(record.state().five_star_alignment.is_none());
    play::assert_replay(&record);
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

fn base_attack_points(events: &[GameEvent]) -> i32 {
    events
        .iter()
        .find_map(|event| match event {
            GameEvent::AttackResolved {
                point_breakdown, ..
            } => Some(point_breakdown.base_points),
            _ => None,
        })
        .expect("陣形應解析為攻擊")
}
