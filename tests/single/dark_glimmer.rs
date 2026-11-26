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
mod support;
use support::record as play;

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
    let p1 = PlayerId::new("p1");
    let (record, events) = dark_envoy_record(&[], &[]);
    assert!(matches!(events.iter().filter(|event| matches!(event,
        GameEvent::ProfessionTransformed { .. } | GameEvent::AttackResolved { .. }
    )).collect::<Vec<_>>().as_slice(),
        [GameEvent::ProfessionTransformed { player, previous, profession, .. }, GameEvent::AttackResolved { formation_id, .. }]
            if player == &p1 && previous.as_ref().is_some_and(|id| id.as_str() == "dark:dark-walker")
                && profession.as_str() == "dark:dark-spirit-envoy" && formation_id == "dark:dark-shock-burst"));
    assert_eq!(
        record.state().profession_for(&p1),
        Some(&ProfessionId::new("dark:dark-spirit-envoy"))
    );
    play::assert_replay(&record);
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
    let p1 = PlayerId::new("p1");
    let (mut record, _) = dark_envoy_record(&[88], &[]);
    let card = play::ids(&[88])[0];
    record
        .handle(Command::ActivateProfessionAbility {
            player: p1.clone(),
            ability_id: "dark:dark-spirit".to_string(),
            cards: vec![card],
            target_card: Some(card),
            declared_element: Some(Element::Earth),
            declared_level: Some(1),
        })
        .unwrap();
    assert_eq!(
        record.state().card_level_for(&p1, card),
        Some(fewfc::domain::EffectiveCardLevel::new(1))
    );
    let prepared = record.state().clone();
    assert!(matches!(
        record.handle(Command::PassAction {
            player: p1.clone(),
            reason: fewfc::domain::PassActionReason::NoCardsInHand
        }),
        Err(GameError::Validation(_))
    ));
    assert_eq!(*record.state(), prepared);
    let events = play::perform(&mut record, &p1, "earth-strike", &[]);
    assert!(events.iter().any(
        |event| matches!(event, GameEvent::AttackResolved { used_cards, point_breakdown, .. }
        if used_cards == &[card] && point_breakdown.base_points == 5)
    ));
    assert!(record.state().discard.ends_with(&[card]));
    play::assert_replay(&record);
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
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = play::start(
        game_setup(2),
        &[
            1, 5, 6, 23, 37, 38, 39, 40, 41, 13, 73, 55, 19, 20, 14, 56, 57, 21, 22, 15, 58, 24, 25,
        ],
    );
    play::perform(&mut record, &p1, "metal-spirit-summoning", &[1, 5]);
    for (power, charge) in [(3, 13), (4, 14), (5, 15)] {
        if power > 3 {
            play::strike_with_spare(&mut record, &p1, &[6, 23]);
        }
        play::finish_discarding(&mut record, &p1, Some(CardInstanceId::new(charge)));
        assert_eq!(record.state().spirit_for(&p1).unwrap().power, power);
        play::strike_with_spare(&mut record, &p2, &[]);
        play::finish(&mut record, &p2);
    }
    let events = play::change(&mut record, &p1, "dark:demon-spirit-master", &[6, 23]);
    assert!(events.iter().any(|event| matches!(event, GameEvent::SpiritTransformed { player, spirit: SpiritKind::Evil, power: 5, .. } if player == &p1)));
    assert_eq!(
        record.state().spirit_for(&p1),
        Some(&PlayerSpirit {
            player: p1.clone(),
            spirit: SpiritKind::Evil,
            power: 5
        })
    );
    assert_eq!(
        record.state().profession_for(&p1).unwrap().as_str(),
        "dark:demon-spirit-master"
    );
    play::assert_replay(&record);
}

#[test]
fn dark_chaos_accepts_only_a_trusted_selection_from_the_target_hand() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let (mut record, _) = dark_envoy_record(&[73, 77, 19, 1], &[56, 41, 9]);
    let cards = play::ids(&[73, 77, 19, 1]);
    let before = record.state().clone();
    assert!(matches!(
        record.handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "dark:dark-chaos".to_string(),
            cards: cards.clone(),
            declared_targets: Vec::new(),
        }),
        Err(GameError::Validation(
            ValidationError::TrustedRandomSelectionRequired { .. }
        ))
    ));
    assert_eq!(*record.state(), before);

    let selected = play::ids(&[9, 56]);
    let events = record
        .handle(Command::PerformFormationWithTrustedRandomness {
            player: p1.clone(),
            formation_id: "dark:dark-chaos".to_string(),
            cards: cards.clone(),
            declared_targets: Vec::new(),
            random_cards: selected.clone(),
        })
        .unwrap();
    let expected = selected
        .into_iter()
        .rev()
        .map(|card| fewfc::domain::CardMoveDelta {
            card,
            from: fewfc::domain::CardZone::Hand(p2.clone()),
            to: fewfc::domain::CardZone::DeckTop,
        })
        .collect::<Vec<_>>();
    assert!(events.contains(&GameEvent::CardsMoved {
        card_moves: expected
    }));
    assert_eq!(&record.state().deck[..2], &play::ids(&[9, 56]));
    assert!(record.state().discard.ends_with(&cards));
    play::assert_replay(&record);
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
        GameEvent::HpChanged { change } if change.delta() == -10
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
            GameEvent::AttackResolved { hp_changes, .. } if hp_changes.iter().any(|resolved|
                matches!(
                    resolved.role,
                    fewfc::domain::HpChangeRole::TriggeredEffect
                )
            )
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
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Wood,
        power: 6,
    });
    game.hp[1].hp = 16;
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
    let hp_events = completed
        .iter()
        .filter(|event| {
            matches!(
                event,
                GameEvent::HpChanged { .. } | GameEvent::AutomaticBloomsResolved { .. }
            )
        })
        .collect::<Vec<_>>();
    assert!(matches!(
        hp_events.as_slice(),
        [
            GameEvent::HpChanged { change },
            GameEvent::AutomaticBloomsResolved { resolutions },
        ] if change.team() == &TeamId::new("team:b")
            && change.old_hp() == 16
            && change.delta() == -16
            && change.new_hp() == 0
            && resolutions.len() == 1
            && resolutions[0].team == TeamId::new("team:b")
            && resolutions[0].hp_change.old_hp() == 0
            && resolutions[0].hp_change.new_hp() == 40
    ));

    for event in &completed {
        apply_event(&mut game, event);
    }
    assert_eq!(game.spirit_for(&PlayerId::new("p1")).unwrap().power, 1);
    assert!(game.spirit_for(&PlayerId::new("p2")).is_none());
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
        GameEvent::HpChanged { change } if change.delta() == -16
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
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = play::start(
        game_setup(2),
        &[
            9, 27, 45, 1, 55, 59, 67, 31, 37, 2, 3, 4, 38, 39, 40, 5, 6, 41, 42, 43,
        ],
    );
    play::strike_with_spare(&mut record, &p1, &[9, 27, 45]);
    play::finish(&mut record, &p1);
    play::perform(&mut record, &p2, "fire-spirit-summoning", &[55, 59]);
    play::finish(&mut record, &p2);
    play::strike_with_spare(&mut record, &p1, &[9, 27, 45]);
    play::finish(&mut record, &p1);
    play::change(&mut record, &p2, "dark:demon-spirit-master", &[67, 31]);
    record.advance_automatic().unwrap();
    let choice = record.state().pending_choice.as_ref().unwrap();
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        unreachable!()
    };
    let discard = *cards
        .iter()
        .find(|card| record.state().card_def(**card).unwrap().level.value() != 4)
        .unwrap();
    play::finish_discarding(&mut record, &p2, Some(discard));
    assert_eq!(
        record.state().spirit_for(&p2),
        Some(&PlayerSpirit {
            player: p2.clone(),
            spirit: SpiritKind::Death,
            power: 2
        })
    );
    let events = play::perform(&mut record, &p1, "void-spirit-shattering", &[9, 27, 45]);
    assert!(events.iter().any(
        |event| matches!(event, GameEvent::VoidSpiritShatteringResolved {
        broken_professions, revived_spirits, shared_fate_hp_changes, ..
    } if broken_professions.len() == 1 && revived_spirits == &[PlayerSpirit {
        player: p2.clone(), spirit: SpiritKind::Death, power: 2,
    }] && shared_fate_hp_changes.is_empty())
    ));
    assert!(record.state().profession_for(&p2).is_none());
    assert_eq!(record.state().spirit_for(&p2).unwrap().power, 2);
    play::assert_replay(&record);
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
    let team_a_baseline = game
        .hp
        .iter()
        .find(|owned| owned.team == TeamId::new("team:a"))
        .unwrap()
        .hp;
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::VoidSpiritShatteringResolved {
            shared_fate_hp_changes,
            ..
        } if matches!(shared_fate_hp_changes.as_slice(), [change]
            if change.team() == &TeamId::new("team:a")
                && change.old_hp() == team_a_baseline
                && change.delta() == -10
                && change.new_hp() == team_a_baseline - 10
                && change.effective_delta() == -10)
    )));
}

#[test]
fn afterimage_slash_keeps_attack_effect_and_shared_fate_hp_changes_in_order() {
    let mut game = state(2);
    set_profession(&mut game, "p1", "dark:shadow-warrior");
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Death,
        power: 4,
    });
    let used = cards(&game, &[(Element::Earth, 1), (Element::Metal, 1)]);
    set_hand(&mut game, "p1", used.clone());
    let team_a_baseline = game
        .hp
        .iter()
        .find(|owned| owned.team == TeamId::new("team:a"))
        .unwrap()
        .hp;
    let team_b_baseline = game
        .hp
        .iter()
        .find(|owned| owned.team == TeamId::new("team:b"))
        .unwrap()
        .hp;

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "dark:afterimage-slash".to_string(),
            cards: used.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_changes, .. }
            if matches!(hp_changes.as_slice(), [
                fewfc::domain::ResolvedHpChange {
                    role: fewfc::domain::HpChangeRole::AttackDamage { target },
                    change: attack_damage,
                },
                fewfc::domain::ResolvedHpChange {
                    role: fewfc::domain::HpChangeRole::FormationEffect,
                    change: afterimage,
                },
                fewfc::domain::ResolvedHpChange {
                    role: fewfc::domain::HpChangeRole::TriggeredEffect,
                    change: shared_fate,
                },
            ] if target == &PlayerId::new("p2")
                && attack_damage.team() == &TeamId::new("team:b")
                && attack_damage.old_hp() == team_b_baseline
                && attack_damage.new_hp() == team_b_baseline - 8
                && afterimage.team() == &TeamId::new("team:b")
                && afterimage.old_hp() == attack_damage.new_hp()
                && afterimage.delta() == -8
                && afterimage.new_hp() == team_b_baseline - 16
                && shared_fate.team() == &TeamId::new("team:a")
                && shared_fate.old_hp() == team_a_baseline
                && shared_fate.delta() == -10
                && shared_fate.new_hp() == team_a_baseline - 10)
    )));

    let mut deck_order = used.clone();
    deck_order.extend(
        game.card_instances
            .iter()
            .map(|instance| instance.instance)
            .filter(|card| !used.contains(card)),
    );
    let mut canonical_events = vec![
        GameEvent::DeckPrepared { deck_order },
        GameEvent::TurnStarted {
            player: PlayerId::new("p1"),
            turn_number: 1,
        },
        GameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: used,
        },
        GameEvent::SpiritSummoned {
            player: PlayerId::new("p2"),
            previous: None,
            spirit: SpiritKind::Death,
        },
        GameEvent::SpiritPowerChanged {
            player: PlayerId::new("p2"),
            spirit: SpiritKind::Death,
            old_power: 2,
            delta: 2,
            new_power: 4,
            reason: SpiritPowerChangeReason::SkillEffect,
        },
    ];
    canonical_events.extend(events);
    let replayed = replay(&game_setup(2), &canonical_events).unwrap();
    assert_eq!(
        replayed
            .hp
            .iter()
            .find(|owned| owned.team == TeamId::new("team:b"))
            .unwrap()
            .hp,
        team_b_baseline - 16
    );
    assert_eq!(
        replayed
            .hp
            .iter()
            .find(|owned| owned.team == TeamId::new("team:a"))
            .unwrap()
            .hp,
        team_a_baseline - 10
    );
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
        GameEvent::HpChanged { change } if change.delta() == -10
    )));
}

fn dark_envoy_record(
    used: &[u64],
    target_cards: &[u64],
) -> (fewfc::application::GameRecord, Vec<GameEvent>) {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let shock = [55, 59, 39, 75];
    let mut reserve = shock.to_vec();
    reserve.extend_from_slice(used);
    let mut record =
        play::start_with_reserves(game_setup(2), &[&[3], target_cards], &[&reserve, &[]]);
    play::change(&mut record, &p1, "dark:dark-walker", &[3]);
    play::finish(&mut record, &p1);
    play::strike_with_spare(&mut record, &p2, target_cards);
    play::finish(&mut record, &p2);
    let events = play::perform(&mut record, &p1, "dark:dark-shock-burst", &shock);
    if !used.is_empty() {
        play::finish(&mut record, &p1);
        play::strike_with_spare(&mut record, &p2, target_cards);
        play::finish(&mut record, &p2);
    }
    (record, events)
}
