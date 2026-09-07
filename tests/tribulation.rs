use fewfc::application::{apply_event, handle_command, resolve_trusted_randomness};
use fewfc::domain::{
    AttackResolutionEffects, CardInstanceId, CardOrigin, ChoiceAnswer, Command, Element,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent, GameSetup, GameState, HERO_SCHOOLS_MODULE_ID,
    HpChangeRole, LimitedUse, PERSONAL_DECK_MODULE_ID, Phase, PlayerId, PlayerProfession,
    PlayerSpirit, ProfessionId, RandomnessDeck, RuleModuleId, STAR_MODULE_ID, SpiritKind,
    StatusDuration, StatusEffect, StatusOwner, TRIBULATION_MODULE_ID, TeamId,
    TrustedRandomnessAnswer,
};
use fewfc::public_view::{PublicPendingChoice, Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};
mod support;
use support::ScenarioPlan;

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
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

fn setup() -> GameSetup {
    ScenarioPlan::two_player(&[
        STAR_MODULE_ID,
        FIVE_DIRECTIONS_LEGEND_MODULE_ID,
        HERO_SCHOOLS_MODULE_ID,
        TRIBULATION_MODULE_ID,
    ])
    .setup()
    .unwrap()
}

fn state() -> GameState {
    let mut state = GameState::from_setup(&setup());
    state.phase = Phase::ActiveEffects;
    state
}

fn personal_deck_state() -> GameState {
    let shape = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
    let mut configured = setup().enabled_rule_modules.to_vec();
    configured.push(RuleModuleId::new(PERSONAL_DECK_MODULE_ID));
    let setup = OfficialRules::new()
        .configure_game_with_decks(shape.players, shape.turn_order, configured, Vec::new())
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    state
}

fn personal_card(
    state: &GameState,
    player: &PlayerId,
    element: Element,
    level: u32,
) -> CardInstanceId {
    state
        .card_instances
        .iter()
        .find(|instance| {
            instance.origin == CardOrigin::Player(player.clone())
                && state.card_def(instance.instance).is_some_and(|definition| {
                    definition.element == element && definition.level.value() == level
                })
        })
        .expect("preconstructed personal deck has the requested card")
        .instance
}

fn apply_all(state: &mut GameState, events: &[GameEvent]) {
    for event in events {
        apply_event(state, event);
    }
}

fn attack_effects(events: &[GameEvent]) -> &AttackResolutionEffects {
    events
        .iter()
        .find_map(|event| match event {
            GameEvent::AttackResolved {
                elemental_context_update: Some(effects),
                ..
            } => Some(effects),
            _ => None,
        })
        .expect("tribulation attack must carry its simultaneous effects atomically")
}

#[test]
fn tribulation_is_default_on_and_requires_all_advanced_modules() {
    assert!(
        OfficialRules::new()
            .default_rule_modules()
            .iter()
            .any(|module| module.as_str() == TRIBULATION_MODULE_ID)
    );
    let error = OfficialRules::new()
        .configure_game(
            GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            vec![RuleModuleId::new(TRIBULATION_MODULE_ID)],
        )
        .unwrap_err();
    assert!(matches!(
        error,
        fewfc::domain::GameError::Validation(
            fewfc::domain::ValidationError::MissingRuleModuleDependencies { .. }
        )
    ));
}

#[test]
fn catalog_offers_thunder_fire_only_for_the_complete_variable_pattern() {
    let mut state = state();
    state.hands[0].cards = vec![card(13), card(16), card(67), card(70)];
    let actions = OfficialRules::new()
        .playable_actions(
            &state,
            &PlayerId::new("p1"),
            &[card(13), card(16), card(67), card(70)],
        )
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "tribulation:thunder-fire"
    )));
}

#[test]
fn thunder_fire_deducts_each_team_then_resolves_its_special_attack() {
    let mut state = state();
    state.hands[0].cards = vec![card(13), card(16), card(67), card(70)];
    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:thunder-fire".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    let attack_hp_changes = events
        .iter()
        .find_map(|event| match event {
            GameEvent::AttackResolved { hp_changes, .. } => Some(hp_changes),
            _ => None,
        })
        .expect("attack event");
    assert_eq!(attack_hp_changes.len(), 3);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown,
            hp_changes,
            ..
        } if point_breakdown.base_points == 60
            && hp_changes.iter().any(|resolved| resolved.change.old_hp() == 185)
    )));
}

#[test]
fn thunder_fire_triggers_shared_fate_only_for_death_spirits_on_actually_losing_teams() {
    let shape = GameSetup::team_mode(
        TeamId::new("a"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("b"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        30,
    );
    let setup = OfficialRules::new()
        .configure_game(
            shape.players,
            shape.turn_order,
            setup().enabled_rule_modules,
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    state.hands[0].cards = vec![card(13), card(16), card(67), card(70)];
    state.spirits.extend([
        PlayerSpirit {
            player: PlayerId::new("p3"),
            spirit: SpiritKind::Death,
            power: 1,
        },
        PlayerSpirit {
            player: PlayerId::new("p4"),
            spirit: SpiritKind::Death,
            power: 1,
        },
    ]);
    state.statuses.push(StatusEffect {
        id: "divine:p3".to_string(),
        owner: StatusOwner::Player(PlayerId::new("p3")),
        kind: "DivineCalculation".to_string(),
        value: None,
        duration: StatusDuration::Permanent,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:thunder-fire".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    let hp_changes = events
        .iter()
        .find_map(|event| match event {
            GameEvent::AttackResolved { hp_changes, .. } => Some(hp_changes),
            _ => None,
        })
        .expect("attack event")
        .iter()
        .collect::<Vec<_>>();
    assert_eq!(hp_changes.len(), 3);
    assert!(matches!(
        hp_changes.as_slice(),
        [
            fewfc::domain::ResolvedHpChange {
                role: HpChangeRole::FormationEffect,
                change: first,
            },
            fewfc::domain::ResolvedHpChange {
                role: HpChangeRole::TriggeredEffect,
                change: second,
            },
            fewfc::domain::ResolvedHpChange {
                role: HpChangeRole::AttackDamage { target },
                change: third,
            },
        ] if first.team() == &TeamId::new("b")
            && first.old_hp() == 250
            && first.effective_delta() == -15
            && first.new_hp() == 235
            && second.team() == &TeamId::new("a")
            && second.old_hp() == 250
            && second.effective_delta() == -10
            && second.new_hp() == 240
            && target == &PlayerId::new("p4")
            && third.team() == &TeamId::new("b")
            && third.old_hp() == 235
            && third.effective_delta() == -60
            && third.new_hp() == 175
    ));
}

#[test]
fn divine_calculation_replaces_its_owner_and_protects_the_next_tribulation() {
    let mut state = state();
    state.hands[0].cards = vec![card(16)];
    let divine = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:divine-calculation".to_string(),
            cards: vec![card(16)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &divine);
    assert!(state.statuses.iter().any(|status| {
        status.kind == "DivineCalculation"
            && status.owner == fewfc::domain::StatusOwner::Player(PlayerId::new("p1"))
    }));

    state.phase = Phase::ActiveEffects;
    state.hands[0].cards = vec![card(13), card(16), card(67), card(70)];
    let tribulation = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:thunder-fire".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(
        tribulation.iter().find_map(|event| match event {
            GameEvent::AttackResolved { hp_changes, .. } => Some(hp_changes.len()),
            _ => None,
        }),
        Some(2)
    );
    assert!(
        attack_effects(&tribulation)
            .statuses_removed
            .iter()
            .any(|status| status
                .status_id
                .starts_with("tribulation:divine-calculation:"))
    );
}

#[test]
fn ineffective_tribulation_still_consumes_divine_calculation() {
    let mut state = state();
    state.environment = Some(Element::Water);
    state.hands[0].cards = vec![card(16)];
    let divine = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:divine-calculation".to_string(),
            cards: vec![card(16)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &divine);

    state.phase = Phase::ActiveEffects;
    state.hands[0].cards = vec![card(13), card(16), card(67), card(70)];
    let tribulation = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:thunder-fire".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(
        attack_effects(&tribulation)
            .statuses_removed
            .iter()
            .any(|status| status
                .status_id
                .starts_with("tribulation:divine-calculation:"))
    );
}

#[test]
fn mudslide_uses_eighty_points_only_after_effective_global_shield_loss() {
    let mut state = state();
    state.hands[0].cards = vec![card(49), card(52), card(85), card(88)];
    state.shields[0].value = 30;
    state.shields[1].value = 10;
    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:mudslide-torrent".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert_eq!(attack_effects(&events).shield_changes.len(), 2);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown,
            shield_change: None,
            ..
        } if point_breakdown.base_points == 80
    )));
}

#[test]
fn gale_rain_tracks_each_player_and_blocks_only_its_owners_formation_recovery() {
    let mut state = state();
    state.hands[0].cards = vec![card(49), card(52), card(67), card(70)];
    let gale = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:gale-rain".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &gale);
    assert_eq!(
        state
            .statuses
            .iter()
            .filter(|status| status.kind == "GaleRain")
            .count(),
        2
    );

    state.phase = Phase::ActiveEffects;
    state.hp[0].hp = 100;
    state.hands[0].cards = vec![card(37), card(38), card(73), card(19)];
    let recovery = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "return-to-origin".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(recovery.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.old_hp() == 100 && change.new_hp() == 100 && change.effective_delta() == 0
    )));
}

#[test]
fn earth_rending_waits_for_environment_and_player_answers_before_resolving() {
    let mut state = state();
    state.hands[0].cards = vec![card(31), card(34), card(85), card(88)];
    state.hands[1].cards = vec![card(67), card(1)];
    let started = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:earth-rending".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(
        started
            .iter()
            .any(|event| matches!(event, GameEvent::EarthRendingStarted { .. }))
    );
    assert!(
        !started
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
    );
    apply_all(&mut state, &started);

    let environment = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Environment {
            environment: Element::Fire,
        },
    )
    .unwrap();
    apply_all(&mut state, &environment);
    assert_eq!(state.environment, None);
    assert_eq!(state.hp[1].hp, 200);
    assert!(state.hands[1].cards.contains(&card(67)));
    assert!(matches!(
        state_for(&state, Viewer::Player(PlayerId::new("p1")))
            .pending_choice
            .unwrap(),
        PublicPendingChoice::Hidden { .. }
    ));
    assert!(matches!(
        state_for(&state, Viewer::Player(PlayerId::new("p2")))
            .pending_choice
            .unwrap(),
        PublicPendingChoice::Visible { .. }
    ));

    let recovered: GameState =
        serde_json::from_str(&serde_json::to_string(&state).unwrap()).unwrap();
    let answered = answer_choice(
        &state,
        PlayerId::new("p2"),
        ChoiceAnswer::Cards {
            cards: vec![card(67)],
        },
    )
    .unwrap();
    assert_eq!(
        answered,
        answer_choice(
            &recovered,
            PlayerId::new("p2"),
            ChoiceAnswer::Cards {
                cards: vec![card(67)],
            },
        )
        .unwrap()
    );
    assert!(answered.iter().any(|event| matches!(
        event,
        GameEvent::HandRevealed { player, .. } if player == &PlayerId::new("p1")
    )));
    assert!(answered.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(effects),
            ..
        } if effects.environment_transfers.iter().any(|transfer| transfer.to == Element::Fire)
    )));
    assert!(
        answered
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
    );
    apply_all(&mut state, &answered);
    assert_eq!(state.environment, Some(Element::Fire));
    assert!(state.discard.contains(&card(67)));
    assert!(state.active_earth_rending_resolution.is_none());
}

#[test]
fn earth_rending_collects_four_player_answers_in_turn_order_and_performer_last() {
    let shape = GameSetup::team_mode(
        TeamId::new("a"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("b"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        30,
    );
    let setup = OfficialRules::new()
        .configure_game(
            shape.players,
            shape.turn_order,
            setup().enabled_rule_modules,
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    state.hands[0].cards = vec![card(31), card(34), card(85), card(88)];
    state.hands[1].cards = vec![card(67)];
    state.hands[2].cards = vec![card(1)];
    state.hands[3].cards = vec![card(70)];

    let started = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:earth-rending".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &started);
    let environment = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Environment {
            environment: Element::Fire,
        },
    )
    .unwrap();
    apply_all(&mut state, &environment);
    assert_eq!(
        state.pending_choice.as_ref().map(|choice| &choice.player),
        Some(&PlayerId::new("p2"))
    );

    let p2 = answer_choice(
        &state,
        PlayerId::new("p2"),
        ChoiceAnswer::Cards {
            cards: vec![card(67)],
        },
    )
    .unwrap();
    assert!(p2.iter().any(|event| matches!(
        event,
        GameEvent::HandRevealed { player, .. } if player == &PlayerId::new("p3")
    )));
    apply_all(&mut state, &p2);
    assert_eq!(
        state.pending_choice.as_ref().map(|choice| &choice.player),
        Some(&PlayerId::new("p4"))
    );

    let p4 = answer_choice(
        &state,
        PlayerId::new("p4"),
        ChoiceAnswer::Cards {
            cards: vec![card(70)],
        },
    )
    .unwrap();
    assert!(p4.iter().any(|event| matches!(
        event,
        GameEvent::HandRevealed { player, .. } if player == &PlayerId::new("p1")
    )));
    assert!(
        p4.iter()
            .any(|event| matches!(event, GameEvent::EarthRendingCompleted { .. }))
    );
}

#[test]
fn rusted_forest_reveals_discards_and_waits_for_the_trusted_shuffle() {
    let mut state = state();
    state.hands[0].cards = vec![card(13), card(16), card(31), card(34)];
    state.deck = vec![
        card(1),
        card(9),
        card(19),
        card(27),
        card(37),
        card(45),
        card(55),
        card(63),
        card(73),
    ];
    state.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Death,
        power: 1,
    });
    let started = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "tribulation:rusted-forest".to_string(),
            cards: state.hands[0].cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(started.iter().any(|event| matches!(
        event,
        GameEvent::RustedForestCardsRevealed { cards, .. } if cards.len() == 8
    )));
    assert!(
        !started
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
    );
    apply_all(&mut state, &started);
    for discarded in [9, 27, 45, 63] {
        assert!(state.discard.contains(&card(discarded)));
    }
    let request = state
        .pending_randomness
        .clone()
        .expect("shuffle is pending");
    let resolved = resolve_trusted_randomness(
        &state,
        &TrustedRandomnessAnswer {
            request_id: request.request_id,
            shuffled_order: request.current_order.into_iter().rev().collect(),
        },
    )
    .unwrap();
    assert!(
        resolved
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
    );
    assert!(
        !resolved
            .iter()
            .any(|event| matches!(event, GameEvent::HpChanged { .. })),
        "Rusted Forest's ordinary Attack damage must not trigger Shared Fate",
    );
    assert!(
        resolved
            .iter()
            .any(|event| matches!(event, GameEvent::RustedForestCompleted { .. }))
    );
}

#[test]
fn rusted_forest_processes_each_personal_deck_and_skips_only_the_protected_owner() {
    let mut state = personal_deck_state();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let formation = vec![
        personal_card(&state, &p1, Element::Wood, 4),
        personal_card(&state, &p1, Element::Wood, 3),
        personal_card(&state, &p1, Element::Metal, 4),
        personal_card(&state, &p1, Element::Metal, 3),
    ];
    let p1_deck = state
        .card_instances
        .iter()
        .filter(|instance| {
            instance.origin == CardOrigin::Player(p1.clone())
                && !formation.contains(&instance.instance)
        })
        .map(|instance| instance.instance)
        .take(8)
        .collect();
    *state.deck_for_mut(&p1).unwrap() = p1_deck;
    *state.hand_mut(&p1).unwrap() = formation.clone();
    state.statuses.push(StatusEffect {
        id: "divine:p2".to_string(),
        owner: StatusOwner::Player(p2.clone()),
        kind: "DivineCalculation".to_string(),
        value: None,
        duration: StatusDuration::Permanent,
    });

    let mut events = handle_command(
        &state,
        Command::PerformFormation {
            player: p1.clone(),
            formation_id: "tribulation:rusted-forest".to_string(),
            cards: formation,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    let mut processed_decks = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::RustedForestCardsRevealed { deck, .. } => Some(deck.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    apply_all(&mut state, &events);
    while let Some(request) = state.pending_randomness.clone() {
        events = resolve_trusted_randomness(
            &state,
            &TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order: request.current_order.into_iter().rev().collect(),
            },
        )
        .unwrap();
        processed_decks.extend(events.iter().filter_map(|event| match event {
            GameEvent::RustedForestCardsRevealed { deck, .. } => Some(deck.clone()),
            _ => None,
        }));
        apply_all(&mut state, &events);
    }

    assert_eq!(processed_decks, vec![RandomnessDeck::Player(p1.clone())]);
    assert!(!state.discard_for(&p1).unwrap().is_empty());
    assert!(state.discard_for(&p2).unwrap().is_empty());
    assert!(state.active_rusted_forest_resolution.is_none());
    assert!(!state.statuses.iter().any(|status| status.id == "divine:p2"));
}

#[test]
fn rusted_forest_deck_shuffle_does_not_recover_exhausted_tailwind() {
    let mut state = personal_deck_state();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let formation = vec![
        personal_card(&state, &p1, Element::Wood, 4),
        personal_card(&state, &p1, Element::Wood, 3),
        personal_card(&state, &p1, Element::Metal, 4),
        personal_card(&state, &p1, Element::Metal, 3),
    ];
    let p1_deck = state
        .card_instances
        .iter()
        .filter(|instance| {
            instance.origin == CardOrigin::Player(p1.clone())
                && !formation.contains(&instance.instance)
        })
        .map(|instance| instance.instance)
        .take(8)
        .collect();
    *state.deck_for_mut(&p1).unwrap() = p1_deck;
    *state.hand_mut(&p1).unwrap() = formation.clone();
    state.professions.push(PlayerProfession {
        player: p2.clone(),
        profession: ProfessionId::new("confluence:clear-wind-envoy"),
    });
    state.limited_uses.push(LimitedUse {
        owner: p2.clone(),
        key: "confluence:tailwind".to_string(),
        remaining: 0,
        maximum: 1,
    });
    let p2_deck = state
        .card_instances
        .iter()
        .filter(|instance| instance.origin == CardOrigin::Player(p2.clone()))
        .map(|instance| instance.instance)
        .take(10)
        .collect();
    *state.deck_for_mut(&p2).unwrap() = p2_deck;
    assert!(state.deck_for(&p2).unwrap().len() > 8);

    let mut events = handle_command(
        &state,
        Command::PerformFormation {
            player: p1,
            formation_id: "tribulation:rusted-forest".to_string(),
            cards: formation,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &events);
    let mut p2_deck_shuffle_seen = false;
    while let Some(request) = state.pending_randomness.clone() {
        if matches!(request.operation, fewfc::domain::RandomnessOperation::DeckShuffle {
            deck: RandomnessDeck::Player(ref owner)
        } if owner == &p2)
        {
            p2_deck_shuffle_seen = true;
        }
        events = resolve_trusted_randomness(
            &state,
            &TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order: request.current_order.into_iter().rev().collect(),
            },
        )
        .unwrap();
        assert!(!events.iter().any(|event| matches!(
            event,
            GameEvent::LimitedUseChanged { owner, key, .. }
                if owner == &p2 && key == "confluence:tailwind"
        )));
        apply_all(&mut state, &events);
    }

    assert!(p2_deck_shuffle_seen);
    assert_eq!(
        state
            .limited_uses
            .iter()
            .find(|use_count| use_count.owner == p2 && use_count.key == "confluence:tailwind")
            .unwrap()
            .remaining,
        0
    );
}
