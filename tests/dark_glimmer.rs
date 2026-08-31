use fewfc::application::{apply_event, handle_command, replay, resolve_trusted_randomness};
use fewfc::domain::{
    CardInstanceId, CardMoveDelta, CardZone, Command, DARK_GLIMMER_MODULE_ID, Element,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError, GameEvent, GameSetup, GameState,
    HERO_SCHOOLS_MODULE_ID, Phase, Player, PlayerId, PlayerProfession, PlayerSpirit, ProfessionId,
    RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID, SpiritKind, SpiritPowerChangeReason,
    SpiritSkill, TeamId, TrustedRandomnessAnswer, ValidationError,
};
use fewfc::public_view::{PublicGameEvent, Viewer, event_for};
use fewfc::rules::{OfficialRules, PlayableAction};

#[test]
fn dark_glimmer_is_default_on_and_requires_spirit_transitively() {
    let rules = OfficialRules::new();
    assert!(
        rules
            .default_rule_modules()
            .iter()
            .any(|module| module.as_str() == DARK_GLIMMER_MODULE_ID)
    );
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
    assert!(
        rules
            .configure_game(
                players,
                vec![PlayerId::new("p1"), PlayerId::new("p2")],
                vec![RuleModuleId::new(DARK_GLIMMER_MODULE_ID)],
            )
            .is_err()
    );
}

fn game_setup(player_count: usize) -> GameSetup {
    let players = (1..=player_count)
        .map(|index| Player {
            id: PlayerId::new(format!("p{index}")),
            team: TeamId::new(if index % 2 == 1 { "team:a" } else { "team:b" }),
        })
        .collect::<Vec<_>>();
    let turn_order = players.iter().map(|player| player.id.clone()).collect();
    OfficialRules::new()
        .configure_game(
            players,
            turn_order,
            vec![
                RuleModuleId::new(STAR_MODULE_ID),
                RuleModuleId::new(FIVE_DIRECTIONS_LEGEND_MODULE_ID),
                RuleModuleId::new(HERO_SCHOOLS_MODULE_ID),
                RuleModuleId::new(SPIRIT_MODULE_ID),
                RuleModuleId::new(DARK_GLIMMER_MODULE_ID),
            ],
        )
        .unwrap()
}

fn state(player_count: usize) -> GameState {
    let setup = game_setup(player_count);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    state
}

fn cards(state: &GameState, requested: &[(Element, u32)]) -> Vec<CardInstanceId> {
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

fn set_hand(state: &mut GameState, player: &str, cards: Vec<CardInstanceId>) {
    state
        .hands
        .iter_mut()
        .find(|hand| hand.player == PlayerId::new(player))
        .unwrap()
        .cards = cards;
}

fn set_profession(state: &mut GameState, player: &str, profession: &str) {
    state
        .professions
        .retain(|owned| owned.player != PlayerId::new(player));
    state.professions.push(PlayerProfession {
        player: PlayerId::new(player),
        profession: ProfessionId::new(profession),
    });
}

#[test]
fn dark_walker_transforms_before_its_dark_formation_resolves() {
    let mut game = state(2);
    set_profession(&mut game, "p1", "dark:dark-walker");
    let used = cards(
        &game,
        &[
            (Element::Fire, 1),
            (Element::Fire, 2),
            (Element::Water, 1),
            (Element::Earth, 1),
        ],
    );
    set_hand(&mut game, "p1", used.clone());

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "dark:dark-shock-burst".to_string(),
            cards: used,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(matches!(
        events.iter().find(|event| matches!(event, GameEvent::ProfessionTransformed { .. })),
        Some(GameEvent::ProfessionTransformed { profession, .. })
            if profession == &ProfessionId::new("dark:dark-spirit-envoy")
    ));
    for event in &events {
        apply_event(&mut game, event);
    }
    assert_eq!(
        game.profession_for(&PlayerId::new("p1")),
        Some(&ProfessionId::new("dark:dark-spirit-envoy"))
    );
}

#[test]
fn dark_walking_professions_offer_no_ordinary_profession_changes() {
    let mut game = state(2);
    let selected = cards(&game, &[(Element::Metal, 1)]);
    set_hand(&mut game, "p1", selected.clone());

    for profession in ["dark:dark-walker", "dark:dark-spirit-envoy"] {
        set_profession(&mut game, "p1", profession);
        let actions = OfficialRules::new()
            .playable_actions(&game, &PlayerId::new("p1"), &selected)
            .unwrap();
        assert!(
            !actions
                .iter()
                .any(|action| matches!(action, PlayableAction::ChangeProfession(_))),
            "{profession} must not offer a normal Profession Change",
        );
    }
}

#[test]
fn dark_walking_professions_reject_direct_normal_profession_change_commands() {
    let mut game = state(2);
    set_profession(&mut game, "p1", "dark:dark-walker");
    let selected = cards(&game, &[(Element::Metal, 1)]);
    set_hand(&mut game, "p1", selected.clone());

    assert!(matches!(
        handle_command(
            &game,
            Command::ChangeProfession {
                player: PlayerId::new("p1"),
                profession: ProfessionId::new("first-wanderer"),
                cards: selected,
            },
        ),
        Err(GameError::Validation(
            ValidationError::ProfessionChangePatternMismatch { .. }
        ))
    ));
}

#[test]
fn other_professions_keep_their_legal_cross_system_profession_changes() {
    let mut game = state(2);
    set_profession(&mut game, "p1", "windwalker");
    let selected = cards(&game, &[(Element::Metal, 1), (Element::Metal, 5)]);
    set_hand(&mut game, "p1", selected.clone());

    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p1"), &selected)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ChangeProfession(candidate)
            if candidate.profession_id == ProfessionId::new("dark:shadow-warrior")
    )));
}

#[test]
fn dark_spirit_lowers_one_physical_card_and_automatically_requires_it() {
    let mut game = state(2);
    set_profession(&mut game, "p1", "dark:dark-spirit-envoy");
    let card = cards(&game, &[(Element::Earth, 5)])[0];
    set_hand(&mut game, "p1", vec![card]);
    let events = handle_command(
        &game,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p1"),
            ability_id: "dark:dark-spirit".to_string(),
            cards: vec![card],
            target_card: Some(card),
            declared_element: Some(Element::Earth),
            declared_level: Some(1),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut game, event);
    }
    assert_eq!(
        game.card_level_for(&PlayerId::new("p1"), card),
        Some(fewfc::domain::EffectiveCardLevel::new(1))
    );
    assert!(matches!(
        handle_command(
            &game,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: fewfc::domain::PassActionReason::NoCardsInHand,
            }
        ),
        Err(GameError::Validation(_))
    ));
    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "earth-strike".to_string(),
            cards: Vec::new(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(event,
        GameEvent::AttackResolved { used_cards, point_breakdown, .. }
            if used_cards == &vec![card] && point_breakdown.base_points == 5
    )));
}

#[test]
fn dark_spirit_rejects_an_element_that_does_not_match_the_offer() {
    let mut game = state(2);
    set_profession(&mut game, "p1", "dark:dark-spirit-envoy");
    let card = cards(&game, &[(Element::Earth, 5)])[0];
    set_hand(&mut game, "p1", vec![card]);

    let result = handle_command(
        &game,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p1"),
            ability_id: "dark:dark-spirit".to_string(),
            cards: vec![card],
            target_card: Some(card),
            declared_element: Some(Element::Fire),
            declared_level: Some(1),
        },
    );

    assert!(matches!(
        result,
        Err(GameError::Validation(
            ValidationError::ProfessionAbilityCannotResolve(ref ability)
        )) if ability == "dark:dark-spirit"
    ));
}

#[test]
fn demon_spirit_possession_transforms_without_resetting_power() {
    let mut game = state(2);
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p1"),
        spirit: SpiritKind::Metal,
        power: 5,
    });
    let used = cards(&game, &[(Element::Metal, 2), (Element::Wood, 2)]);
    set_hand(&mut game, "p1", used.clone());
    let events = handle_command(
        &game,
        Command::ChangeProfession {
            player: PlayerId::new("p1"),
            profession: ProfessionId::new("dark:demon-spirit-master"),
            cards: used,
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::SpiritTransformed {
            spirit: SpiritKind::Evil,
            power: 5,
            ..
        }
    )));
}

#[test]
fn dark_chaos_accepts_only_a_trusted_selection_from_the_target_hand() {
    let mut game = state(2);
    set_profession(&mut game, "p1", "dark:dark-spirit-envoy");
    let used = cards(
        &game,
        &[
            (Element::Earth, 1),
            (Element::Earth, 2),
            (Element::Wood, 1),
            (Element::Metal, 1),
        ],
    );
    let target_hand = cards(
        &game,
        &[(Element::Fire, 1), (Element::Water, 2), (Element::Metal, 3)],
    );
    set_hand(&mut game, "p1", used.clone());
    set_hand(&mut game, "p2", target_hand.clone());

    assert!(matches!(
        handle_command(
            &game,
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "dark:dark-chaos".to_string(),
                cards: used.clone(),
                declared_targets: Vec::new(),
            },
        ),
        Err(GameError::Validation(
            ValidationError::TrustedRandomSelectionRequired { .. }
        ))
    ));

    let selected = vec![target_hand[2], target_hand[0]];
    let events = handle_command(
        &game,
        Command::PerformFormationWithTrustedRandomness {
            player: PlayerId::new("p1"),
            formation_id: "dark:dark-chaos".to_string(),
            cards: used,
            declared_targets: Vec::new(),
            random_cards: selected.clone(),
        },
    )
    .unwrap();
    let expected_return_order = selected.into_iter().rev().collect::<Vec<_>>();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::CardsMoved { card_moves }
            if card_moves.iter().map(|card_move| card_move.card).collect::<Vec<_>>()
                == expected_return_order
    )));
}

#[test]
fn shared_fate_triggers_when_a_formation_effect_deducts_the_death_owners_team_hp() {
    let mut game = state(4);
    game.current_turn_index = 3;
    set_profession(&mut game, "p4", "dark:dark-spirit-envoy");
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p3"),
        spirit: SpiritKind::Death,
        power: 4,
    });
    let used = cards(
        &game,
        &[
            (Element::Metal, 1),
            (Element::Metal, 2),
            (Element::Fire, 1),
            (Element::Water, 1),
        ],
    );
    set_hand(&mut game, "p4", used.clone());
    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p4"),
            formation_id: "dark:dark-radiance".to_string(),
            cards: used,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GameEvent::HpChanged { .. }))
            .count(),
        2
    );
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.delta == -10
    )));
}

#[test]
fn shared_fate_does_not_trigger_from_attack_damage() {
    let mut game = state(2);
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Death,
        power: 4,
    });
    let used = cards(&game, &[(Element::Metal, 1)]);
    set_hand(&mut game, "p1", used.clone());

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: used,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(
        events.iter().all(|event| !matches!(
            event,
            GameEvent::AttackResolved {
                elemental_context_update: Some(effects),
                ..
            } if !effects.hp_changes.is_empty()
        )),
        "ordinary attack damage must not trigger Shared Fate",
    );
}

#[test]
fn death_omen_shuffles_discard_before_consuming_the_skill_then_discards_four_cards() {
    let mut game = state(2);
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p1"),
        spirit: SpiritKind::Death,
        power: 4,
    });
    let top = cards(&game, &[(Element::Metal, 1)])[0];
    let discard = cards(
        &game,
        &[(Element::Wood, 2), (Element::Fire, 4), (Element::Water, 3)],
    );
    game.deck = vec![top];
    game.discard = discard.clone();

    let requested = handle_command(
        &game,
        Command::UseSpiritSkill {
            player: PlayerId::new("p1"),
            skill: SpiritSkill::DeathOmen,
            selected_card: None,
            declared_level: None,
        },
    )
    .unwrap();

    assert!(matches!(
        requested.as_slice(),
        [GameEvent::RandomnessRequested { request, .. }]
            if request.operation.is_discard_shuffle() && request.current_order == discard
    ));
    assert!(matches!(
        event_for(requested.first().unwrap(), Viewer::Observer),
        PublicGameEvent::RandomnessRequested {
            card_count: 3,
            operation: fewfc::public_view::PublicRandomnessOperation::DiscardShuffle,
            ..
        }
    ));
    assert!(
        !requested.iter().any(|event| matches!(
            event,
            GameEvent::SpiritSkillUsed { .. } | GameEvent::SpiritPowerChanged { .. }
        )),
        "the pending shuffle must not consume Death Omen or Spirit Power",
    );

    for event in &requested {
        apply_event(&mut game, event);
    }
    assert_eq!(game.spirit_for(&PlayerId::new("p1")).unwrap().power, 4);
    assert!(game.spirit_skill_use_turns.is_empty());
    assert_eq!(game.deck, vec![top]);
    assert_eq!(game.discard, discard);

    let request = game.pending_randomness.clone().unwrap();
    let completed = resolve_trusted_randomness(
        &game,
        &TrustedRandomnessAnswer {
            request_id: request.request_id,
            shuffled_order: request.current_order.into_iter().rev().collect(),
        },
    )
    .unwrap();
    assert!(matches!(
        completed.first(),
        Some(GameEvent::RandomnessResolved { .. })
    ));
    assert!(completed.iter().any(|event| matches!(
        event,
        GameEvent::CardsMoved { card_moves } if card_moves.len() == 4
    )));
    assert!(completed.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.delta == -16
    )));

    for event in &completed {
        apply_event(&mut game, event);
    }
    assert_eq!(game.spirit_for(&PlayerId::new("p1")).unwrap().power, 1);
    assert!(game.deck.is_empty());
    assert_eq!(game.discard.len(), 4);
}

#[test]
fn death_omen_discard_shuffle_replays_from_the_canonical_events() {
    let setup = game_setup(2);
    let blank = GameState::from_setup(&setup);
    let top = cards(&blank, &[(Element::Metal, 1)])[0];
    let discard = cards(
        &blank,
        &[(Element::Wood, 2), (Element::Fire, 4), (Element::Water, 3)],
    );
    let mut events = vec![
        GameEvent::DeckPrepared {
            deck_order: std::iter::once(top)
                .chain(discard.iter().copied())
                .collect(),
        },
        GameEvent::CardsMoved {
            card_moves: discard
                .iter()
                .map(|card| CardMoveDelta {
                    card: *card,
                    from: CardZone::DeckTop,
                    to: CardZone::Discard,
                })
                .collect(),
        },
        GameEvent::SpiritSummoned {
            player: PlayerId::new("p1"),
            previous: None,
            spirit: SpiritKind::Death,
        },
        GameEvent::SpiritPowerChanged {
            player: PlayerId::new("p1"),
            spirit: SpiritKind::Death,
            old_power: 2,
            delta: 2,
            new_power: 4,
            reason: SpiritPowerChangeReason::SkillEffect,
        },
        GameEvent::TurnStarted {
            player: PlayerId::new("p1"),
            turn_number: 1,
        },
    ];
    let mut game = replay(&setup, &events).unwrap();

    let requested = handle_command(
        &game,
        Command::UseSpiritSkill {
            player: PlayerId::new("p1"),
            skill: SpiritSkill::DeathOmen,
            selected_card: None,
            declared_level: None,
        },
    )
    .unwrap();
    for event in &requested {
        apply_event(&mut game, event);
    }
    events.extend(requested);

    let request = game.pending_randomness.clone().unwrap();
    let completed = resolve_trusted_randomness(
        &game,
        &TrustedRandomnessAnswer {
            request_id: request.request_id,
            shuffled_order: request.current_order.into_iter().rev().collect(),
        },
    )
    .unwrap();
    for event in &completed {
        apply_event(&mut game, event);
    }
    events.extend(completed);

    assert_eq!(replay(&setup, &events).unwrap(), game);
}

#[test]
fn death_omen_with_four_deck_cards_keeps_the_existing_immediate_resolution() {
    let mut game = state(2);
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p1"),
        spirit: SpiritKind::Death,
        power: 4,
    });
    let deck = cards(
        &game,
        &[
            (Element::Metal, 1),
            (Element::Wood, 2),
            (Element::Fire, 4),
            (Element::Water, 3),
        ],
    );
    game.deck = deck.clone();

    let events = handle_command(
        &game,
        Command::UseSpiritSkill {
            player: PlayerId::new("p1"),
            skill: SpiritSkill::DeathOmen,
            selected_card: None,
            declared_level: None,
        },
    )
    .unwrap();

    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::RandomnessRequested { .. }))
    );
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::CardsMoved { card_moves }
            if card_moves.iter().map(|move_| move_.card).collect::<Vec<_>>() == deck
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.delta == -16
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::SpiritPowerChanged {
            old_power: 0,
            new_power: 1,
            ..
        }
    )));
}

#[test]
fn void_shattering_revives_broken_death_spirit_without_retroactive_shared_fate() {
    let mut game = state(2);
    set_profession(&mut game, "p2", "dark:demon-spirit-master");
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Death,
        power: 2,
    });
    let used = cards(
        &game,
        &[(Element::Metal, 3), (Element::Wood, 3), (Element::Water, 3)],
    );
    set_hand(&mut game, "p1", used.clone());
    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "void-spirit-shattering".to_string(),
            cards: used,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::VoidSpiritShatteringResolved {
            broken_professions,
            revived_spirits,
            shared_fate_hp_changes,
            ..
        } if broken_professions.len() == 1
            && revived_spirits == &vec![PlayerSpirit {
                player: PlayerId::new("p2"),
                spirit: SpiritKind::Death,
                power: 2,
            }]
            && shared_fate_hp_changes.is_empty()
    )));
}

#[test]
fn surviving_death_spirit_triggers_shared_fate_during_void_shattering() {
    let mut game = state(2);
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Death,
        power: 3,
    });
    let used = cards(
        &game,
        &[(Element::Metal, 3), (Element::Wood, 3), (Element::Water, 3)],
    );
    set_hand(&mut game, "p1", used.clone());
    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "void-spirit-shattering".to_string(),
            cards: used,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::VoidSpiritShatteringResolved {
            shared_fate_hp_changes,
            ..
        } if shared_fate_hp_changes.len() == 1
            && shared_fate_hp_changes[0].delta == -10
    )));
}

#[test]
fn mischief_uses_only_the_cards_evil_gaze_actually_inspected() {
    let mut game = state(2);
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p1"),
        spirit: SpiritKind::Evil,
        power: 2,
    });
    let target_hand = cards(
        &game,
        &[(Element::Metal, 1), (Element::Wood, 2), (Element::Fire, 5)],
    );
    set_hand(&mut game, "p2", target_hand.clone());
    assert!(matches!(
        handle_command(
            &game,
            Command::UseSpiritSkill {
                player: PlayerId::new("p1"),
                skill: SpiritSkill::EvilGaze,
                selected_card: None,
                declared_level: None,
            },
        ),
        Err(GameError::Validation(
            ValidationError::TrustedRandomSelectionRequired { .. }
        ))
    ));
    let events = handle_command(
        &game,
        Command::UseSpiritSkillWithTrustedRandomness {
            player: PlayerId::new("p1"),
            skill: SpiritSkill::EvilGaze,
            selected_card: None,
            declared_level: None,
            random_cards: vec![target_hand[2], target_hand[0]],
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HandInspected { cards, .. } if cards == &vec![target_hand[2], target_hand[0]]
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.delta == -10
    )));
}
