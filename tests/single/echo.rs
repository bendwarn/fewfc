use fewfc::application::{
    AutomaticReason, EventSource, GameRecord, advance_automatic, apply_event, handle_command,
};
use fewfc::domain::{
    AttackOutcome, CONFLUENCE_GENERATION_MODULE_ID, CardInstanceId, CardMoveDelta, CardZone,
    ChoiceAnswer, Command, CounterEffect, DARK_GLIMMER_MODULE_ID, DeckPlacement, ECHO_MODULE_ID,
    Element, EnvironmentTransferDelta, FIVE_DIRECTIONS_LEGEND_MODULE_ID, FormationAreaState,
    FormationInArea, FormationNoEffectReason, FormationSuppression, GameError, GameEvent,
    GameSetup, GameState, HERO_SCHOOLS_MODULE_ID, JIANGHU_MODULE_ID, JianghuState,
    JianghuStateKind, MelodyExecutionOrigin, PERSONAL_DECK_MODULE_ID, PassiveFlipOutcome,
    PassiveNoEffectGround, PassiveTriggerTiming, PendingChoiceKind, PendingResolution, Phase,
    PlayerId, PlayerSpirit, PreparedProfessionAbility, RandomnessDeck, RandomnessOperation,
    RingingMetalSelection, RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID, ScheduledEcho,
    ScheduledPlantEarth, SpiritKind, StarKind, StatusDuration, StatusEffect, StatusOwner, TeamId,
    TeamStar, TimedEffectReduction, TrustedRandomnessAnswer, ValidationError,
};
use fewfc::public_view::{
    PublicCardRefs, PublicGameEvent, PublicPendingChoice, PublicPendingChoicePresentation, Viewer,
    events_for, state_for,
};
use fewfc::rules::{OfficialRules, PlayableAction};

fn dependencies() -> Vec<RuleModuleId> {
    [
        STAR_MODULE_ID,
        FIVE_DIRECTIONS_LEGEND_MODULE_ID,
        HERO_SCHOOLS_MODULE_ID,
    ]
    .into_iter()
    .map(RuleModuleId::new)
    .collect()
}

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

fn configured_setup() -> GameSetup {
    configured_setup_with_modules(&[])
}

fn configured_setup_with_modules(extra_modules: &[&str]) -> GameSetup {
    let rules = OfficialRules::new();
    let mut modules = dependencies();
    modules.push(RuleModuleId::new(ECHO_MODULE_ID));
    modules.extend(
        extra_modules
            .iter()
            .map(|module| RuleModuleId::new(*module)),
    );
    rules
        .configure_game(
            GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            modules,
        )
        .unwrap()
}

fn configured_state() -> GameState {
    let mut state = GameState::from_setup(&configured_setup());
    state.phase = Phase::ActiveEffects;
    state
}

fn formation_ids(actions: Vec<PlayableAction>) -> Vec<String> {
    actions
        .into_iter()
        .filter_map(|action| match action {
            PlayableAction::PerformFormation(candidate) => Some(candidate.formation_id),
            _ => None,
        })
        .collect()
}

fn apply_all(state: &mut GameState, events: &[GameEvent]) {
    for event in events {
        apply_event(state, event);
    }
}

fn answer_choice(
    state: &GameState,
    player: PlayerId,
    answer: ChoiceAnswer,
) -> Result<Vec<GameEvent>, GameError> {
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

fn answer_record_choice(
    record: &mut GameRecord,
    player: PlayerId,
    answer: ChoiceAnswer,
) -> Result<Vec<GameEvent>, GameError> {
    let choice_id = record
        .state()
        .pending_choice
        .as_ref()
        .expect("pending choice")
        .choice_id;
    record.handle(Command::AnswerChoice {
        player,
        choice_id,
        answer,
    })
}

#[test]
fn echo_is_default_on_and_requires_all_advanced_modules() {
    let rules = OfficialRules::new();
    assert!(
        rules
            .default_rule_modules()
            .iter()
            .any(|module| module.as_str() == ECHO_MODULE_ID)
    );

    let players =
        fewfc::domain::GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).players;
    assert!(matches!(
        rules.configure_game(
            players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            vec![RuleModuleId::new(ECHO_MODULE_ID)],
        ),
        Err(GameError::Validation(
            ValidationError::MissingRuleModuleDependencies { .. }
        ))
    ));
}

#[test]
fn melody_and_base_formation_offer_separate_declarations_for_the_same_cards() {
    let setup = configured_setup();
    let rules = OfficialRules::new();
    let deck = rules.official_deck_order(&setup).unwrap();
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();

    let player = PlayerId::new("p1");
    let selected = [card(1), card(2)];
    let actions = record.playable_actions(&player, &selected).unwrap();
    for formation_id in ["weapon", "echo:ringing-metal"] {
        assert!(
            actions.iter().any(|action| matches!(
                action, PlayableAction::PerformFormation(candidate)
                    if candidate.formation_id == formation_id && candidate.cards == selected
            )),
            "{formation_id} missing from {actions:#?}"
        );
    }
}

#[test]
fn star_substitution_does_not_make_a_melody_legal() {
    let mut state = configured_state();
    state.hands[0].cards = vec![card(1), card(73)];
    state.team_stars.push(TeamStar {
        team: state.players[0].team.clone(),
        star: StarKind::Metal,
    });

    let ids = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(1), card(73)])
            .unwrap(),
    );
    assert!(ids.contains(&"weapon".to_string()));
    assert!(!ids.contains(&"echo:ringing-metal".to_string()));
}

#[test]
fn card_interpretation_must_explicitly_include_the_melody_scope() {
    let mut state = configured_state();
    state.hands[0].cards = vec![card(1), card(73)];
    apply_event(
        &mut state,
        &GameEvent::ProfessionAbilityActivated {
            player: PlayerId::new("p1"),
            ability_id: "test:interpretation".to_string(),
            prepared: Some(PreparedProfessionAbility {
                player: PlayerId::new("p1"),
                ability_id: "test:interpretation".to_string(),
                card: card(73),
                element: Element::Metal,
                level: fewfc::domain::EffectiveCardLevel::new(1),
                allowed_formation_scope: vec!["base".to_string()],
                prepared_on_turn: 1,
                interpretation_revision: 1,
            }),
        },
    );
    let base_only = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(1), card(73)])
            .unwrap(),
    );
    assert!(!base_only.contains(&"echo:ringing-metal".to_string()));

    state.prepared_profession_abilities[0].allowed_formation_scope =
        vec!["echo:ringing-metal".to_string()];
    let explicitly_scoped = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(1), card(73)])
            .unwrap(),
    );
    assert!(explicitly_scoped.contains(&"echo:ringing-metal".to_string()));
}

#[test]
fn lethal_war_fire_uses_direct_hp_loss_and_offers_no_echo_cost() {
    let mut state = configured_state();
    state.hp[1].hp = 10;
    state.hands[0].cards = vec![card(55), card(56), card(57)];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:war-fire".to_string(),
            cards: vec![card(55), card(56)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.old_hp() == 10 && change.new_hp() == 0
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
}

#[test]
fn formation_use_war_fire_immediately_triggers_shared_fate() {
    let mut state = configured_state();
    state.hands[0].cards = vec![card(55), card(56), card(57)];
    state.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Death,
        power: 2,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:war-fire".to_string(),
            cards: vec![card(55), card(56)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    let changes = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::HpChanged { change } => Some(change),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].team(), &TeamId::new("team:p2"));
    assert_eq!(changes[0].delta(), -15);
    assert_eq!(changes[1].team(), &TeamId::new("team:p1"));
    assert_eq!(changes[1].delta(), -10);
}

#[test]
fn echo_war_fire_does_not_trigger_shared_fate() {
    let mut state = configured_state();
    state.phase = Phase::TurnStart;
    state.scheduled_echoes.push(ScheduledEcho {
        player: PlayerId::new("p1"),
        melody_id: "echo:war-fire".to_string(),
        due_turn_number: state.turn_number,
    });
    state.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Death,
        power: 2,
    });

    let events = advance_automatic(&state).unwrap();
    let changes = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::HpChanged { change } => Some(change),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(changes.len(), 1);
    assert_eq!(changes[0].team(), &TeamId::new("team:p2"));
    assert_eq!(changes[0].delta(), -15);
}

#[test]
fn echo_war_fire_runs_automatic_bloom_before_game_conclusion() {
    let mut state = configured_state();
    state
        .enabled_rule_modules
        .push(RuleModuleId::new(SPIRIT_MODULE_ID));
    state.phase = Phase::TurnStart;
    state.hp[1].hp = 15;
    state.scheduled_echoes.push(ScheduledEcho {
        player: PlayerId::new("p1"),
        melody_id: "echo:war-fire".to_string(),
        due_turn_number: state.turn_number,
    });
    state.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Wood,
        power: 6,
    });

    let events = advance_automatic(&state).unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::EchoResolutionStarted { .. },
            GameEvent::HpChanged { change },
            GameEvent::EchoResolutionCompleted { .. },
            GameEvent::AutomaticBloomsResolved { resolutions },
            GameEvent::TurnStarted { .. },
        ] if change.team() == &TeamId::new("team:p2")
            && change.new_hp() == 0
            && resolutions.len() == 1
            && resolutions[0].team == TeamId::new("team:p2")
            && resolutions[0].hp_change.old_hp() == 0
            && resolutions[0].hp_change.new_hp() == 40
    ));
}

#[test]
fn echo_falling_wood_recovers_even_when_its_team_is_poisoned() {
    let mut state = configured_state();
    state.phase = Phase::TurnStart;
    state.hp[0].hp = 100;
    state.jianghu_states.push(JianghuState {
        owner: PlayerId::new("p1"),
        kind: JianghuStateKind::Poison,
        remaining_turns: 1,
        expires_on_turn: None,
        last_resolved_turn: None,
    });
    state.scheduled_echoes.push(ScheduledEcho {
        player: PlayerId::new("p1"),
        melody_id: "echo:falling-wood".to_string(),
        due_turn_number: state.turn_number,
    });

    let events = advance_automatic(&state).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team() == &TeamId::new("team:p1")
                && change.old_hp() == 100
                && change.delta() == 15
                && change.new_hp() == 115
                && change.effective_delta() == 15
    )));
}

#[test]
fn only_echo_split_earth_suppresses_a_sacred_beast() {
    let player = PlayerId::new("p1");
    let target = PlayerId::new("p2");
    let beast = "west-white-tiger";
    for origin in [
        MelodyExecutionOrigin::FormationUse,
        MelodyExecutionOrigin::PlantedEarth,
        MelodyExecutionOrigin::Echo,
    ] {
        let opening = match origin {
            MelodyExecutionOrigin::FormationUse | MelodyExecutionOrigin::Echo => [73, 74],
            MelodyExecutionOrigin::PlantedEarth => [27, 85],
        };
        // 需要等到第 4 回合的案例，以第 1 張牌發動金擊再從抽牌留住第 6 張，
        // 讓白虎出手時仍然備齊五張金牌。
        let mut prefix = opening.to_vec();
        prefix.extend([75, 19, 1, 2, 3, 4, 5, 7, 8, 9, 10, 6]);
        let mut record = start_ordered_echo_record(configured_setup(), &prefix);
        match origin {
            MelodyExecutionOrigin::FormationUse => {
                perform_record_formation(&mut record, &player, "echo:split-earth", &opening);
                select_record_formation(&mut record, &player, beast);
                answer_record_choice(&mut record, player.clone(), ChoiceAnswer::Decline).unwrap();
            }
            MelodyExecutionOrigin::Echo => {
                perform_record_formation(&mut record, &player, "echo:split-earth", &opening);
                select_record_formation(&mut record, &player, "weapon");
                answer_record_choice(
                    &mut record,
                    player.clone(),
                    ChoiceAnswer::Cards {
                        cards: vec![card(75)],
                    },
                )
                .unwrap();
            }
            MelodyExecutionOrigin::PlantedEarth => {
                perform_record_formation(&mut record, &player, "echo:plant-earth", &opening);
            }
        }
        finish_record_turn(&mut record);

        let (beast_cards, target_turn) = if origin == MelodyExecutionOrigin::FormationUse {
            ([1, 2, 3, 4, 5], 2)
        } else {
            perform_record_formation(&mut record, &target, "metal-strike", &[1]);
            finish_record_turn(&mut record);
            if origin == MelodyExecutionOrigin::PlantedEarth {
                assert_eq!(
                    record.state().pending_resolution,
                    Some(PendingResolution::MelodyPlantEarthMelody {
                        origin: MelodyExecutionOrigin::PlantedEarth,
                    })
                );
                select_record_formation(&mut record, &player, "echo:split-earth");
            }
            assert_eq!(
                record.state().pending_resolution,
                Some(PendingResolution::MelodySplitEarthFormation { origin })
            );
            select_record_formation(&mut record, &player, beast);
            record.advance_automatic().unwrap();
            perform_record_formation(&mut record, &player, "wood-strike", &[19]);
            finish_record_turn(&mut record);
            ([2, 3, 4, 5, 6], 4)
        };

        let state = record.state();
        assert_eq!(state.turn_number, target_turn);
        assert_eq!(
            state.formation_suppressions,
            [FormationSuppression {
                source: player.clone(),
                target: target.clone(),
                formation_id: beast.to_string(),
                expires_on_turn_number: target_turn,
                origin,
            }]
        );
        let old_hp = state.hp.clone();
        let old_environment = state.environment;
        let events = perform_record_formation(&mut record, &target, beast, &beast_cards);
        let state = record.state();
        let expected_outcome = if origin == MelodyExecutionOrigin::Echo {
            AttackOutcome::DamagePrevented
        } else {
            AttackOutcome::Resolved
        };
        // 白虎仍會轉移環境，包含傷害受回響壓制的情況。
        let transfer = EnvironmentTransferDelta {
            player: target.clone(),
            formation_id: beast.to_string(),
            from: old_environment,
            to: Element::Metal,
        };
        assert!(
            events.iter().any(|event| matches!(
                event,
                GameEvent::AttackResolved {
                    attacker,
                    formation_id,
                    hp_changes,
                    elemental_context_update: Some(effects),
                    ..
                } if attacker == &target && formation_id == beast
                    && effects.outcome == expected_outcome
                    && effects.environment_transfers.as_slice() == std::slice::from_ref(&transfer)
                    && hp_changes.iter().any(|entry| entry.change.effective_delta() < 0)
                        == (expected_outcome == AttackOutcome::Resolved)
            )),
            "{origin:?}: {events:#?}"
        );
        assert_eq!(state.environment, Some(Element::Metal), "{origin:?}");
        assert!(events.contains(&GameEvent::FormationCardsDiscarded {
            player: target.clone(),
            formation_id: beast.to_string(),
            cards: beast_cards.map(card).to_vec(),
        }));
        assert!(state.discard.ends_with(&beast_cards.map(card)));
        if origin == MelodyExecutionOrigin::Echo {
            assert!(events.contains(&GameEvent::FormationEffectIgnored {
                player: target.clone(),
                formation_id: beast.to_string(),
                reason: FormationNoEffectReason::SuppressedBySplitEarth {
                    source: player.clone()
                },
            }));
            assert_eq!(state.hp, old_hp);
        } else {
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, GameEvent::FormationEffectIgnored { .. })),
                "{origin:?}"
            );
            assert!(state.hp[0].hp < old_hp[0].hp, "{origin:?}");
        }
        assert_eq!(state.phase, Phase::TurnDraw);
        assert_eq!(record.replay().unwrap(), *state);
        assert_eq!(record.verify_replay().unwrap(), *state);
    }
}

#[test]
fn planted_earth_war_fire_immediately_triggers_shared_fate() {
    let mut state = configured_state();
    state.phase = Phase::TurnStart;
    state
        .scheduled_plant_earth
        .push(fewfc::domain::ScheduledPlantEarth {
            player: PlayerId::new("p1"),
            due_turn_number: state.turn_number,
        });
    state.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Death,
        power: 2,
    });

    let started = advance_automatic(&state).unwrap();
    apply_all(&mut state, &started);
    let selected = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Formation {
            formation_id: "echo:war-fire".to_string(),
        },
    )
    .unwrap();
    let changes = selected
        .iter()
        .filter_map(|event| match event {
            GameEvent::HpChanged { change } => Some(change),
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].team(), &TeamId::new("team:p2"));
    assert_eq!(changes[0].delta(), -15);
    assert_eq!(changes[1].team(), &TeamId::new("team:p1"));
    assert_eq!(changes[1].delta(), -10);
}

#[test]
fn planted_earth_war_fire_choice_reuses_its_hp_plan_for_bloom() {
    let mut state = configured_state();
    state
        .enabled_rule_modules
        .push(RuleModuleId::new(SPIRIT_MODULE_ID));
    state.phase = Phase::TurnStart;
    state
        .scheduled_plant_earth
        .push(fewfc::domain::ScheduledPlantEarth {
            player: PlayerId::new("p1"),
            due_turn_number: state.turn_number,
        });
    state.hp[1].hp = 15;
    state.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Wood,
        power: 6,
    });

    let started = advance_automatic(&state).unwrap();
    apply_all(&mut state, &started);
    let selected = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Formation {
            formation_id: "echo:war-fire".to_string(),
        },
    )
    .unwrap();

    assert!(matches!(
        selected.as_slice(),
        [
            GameEvent::ChoiceMade { .. },
            GameEvent::HpChanged { change },
            GameEvent::PlantEarthResolutionCompleted { .. },
            GameEvent::AutomaticBloomsResolved { resolutions },
        ] if change.team() == &TeamId::new("team:p2")
            && change.old_hp() == 15
            && change.new_hp() == 0
            && resolutions.len() == 1
            && resolutions[0].team == TeamId::new("team:p2")
            && resolutions[0].hp_change.old_hp() == 0
            && resolutions[0].hp_change.new_hp() == 40
    ));
}

#[test]
fn flow_layers_stack_but_only_one_can_trigger_during_a_turn_draw() {
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    let mut record = start_ordered_echo_record(
        configured_setup(),
        &[37, 38, 1, 39, 55, 56, 57, 58, 59, 3, 40, 2, 19],
    );
    perform_record_formation(&mut record, &player, "echo:flowing-water", &[37, 38]);
    answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(1)],
        },
    )
    .unwrap();
    finish_record_turn(&mut record);
    assert!(!record.state().flow_layers_by_player.contains_key(&player));

    perform_record_formation(&mut record, &opponent, "fire-strike", &[55]);
    let echoed = finish_record_turn(&mut record);
    assert!(echoed.contains(&GameEvent::FlowStateChanged {
        player: player.clone(),
        old_layers: 0,
        new_layers: 1,
    }));
    assert!(echoed.contains(&GameEvent::EchoResolutionCompleted {
        player: player.clone(),
        melody_id: "echo:flowing-water".to_string(),
        due_turn_number: 3,
    }));

    let stacked = perform_record_formation(&mut record, &player, "echo:flowing-water", &[39, 40]);
    assert!(stacked.contains(&GameEvent::FlowStateChanged {
        player: player.clone(),
        old_layers: 1,
        new_layers: 2,
    }));
    answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(2)],
        },
    )
    .unwrap();
    // 只留一張手牌，容量足以容納兩層的抽牌加成，因此不會由手牌上限代替觸發限制。
    assert_eq!(record.state().hand(&player).unwrap(), [card(19)]);
    assert!(record.state().base_draw + 2 <= record.state().hand_limit - 1);

    let drawn = record.advance_automatic().unwrap();
    let triggers = drawn
        .iter()
        .filter(|event| matches!(event, GameEvent::FlowStateTriggered { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        triggers,
        [&GameEvent::FlowStateTriggered {
            player: player.clone(),
            old_layers: 2,
            new_layers: 1,
            old_draw_bonus: 0,
            new_draw_bonus: 1,
        }]
    );
    let state = record.state();
    assert_eq!(state.turn_draw_pool.len(), state.base_draw + 2);
    assert_eq!(state.flow_layers_by_player.get(&player), Some(&1));
    assert_eq!(state.turn_draw_bonus_by_player.get(&player), Some(&1));
    assert!(!state.turn_draw_bonus_by_player.contains_key(&opponent));
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn split_earth_selects_a_catalog_formation_and_only_its_next_use_is_ineffective() {
    let player = PlayerId::new("p1");
    let target = PlayerId::new("p2");
    let setup = configured_setup();
    let mut catalog = OfficialRules::new()
        .formation_names(&setup)
        .unwrap()
        .into_keys()
        .collect::<Vec<_>>();
    catalog.sort();
    let mut record = start_ordered_echo_record(setup, &[73, 74, 75, 19, 1, 2, 3, 4, 5]);
    let performed = perform_record_formation(&mut record, &player, "echo:split-earth", &[73, 74]);
    assert!(performed.contains(&GameEvent::HandInspected {
        viewer: player.clone(),
        target: target.clone(),
        cards: [1, 2, 3, 4, 5].map(card).to_vec(),
    }));
    assert!(performed.iter().any(|event| matches!(
        event,
        GameEvent::ChoiceRequested {
            choice,
            resolution: PendingResolution::MelodySplitEarthFormation { origin: MelodyExecutionOrigin::FormationUse },
        } if choice.player == player && choice.kind == (PendingChoiceKind::Formation {
            formations: catalog.clone(),
            can_decline: false,
        })
    )));

    let selected = select_record_formation(&mut record, &player, "weapon");
    let suppression = FormationSuppression {
        source: player.clone(),
        target: target.clone(),
        formation_id: "weapon".to_string(),
        expires_on_turn_number: 2,
        origin: MelodyExecutionOrigin::FormationUse,
    };
    assert!(selected.contains(&GameEvent::FormationSuppressionSet {
        suppression: suppression.clone()
    }));
    assert_eq!(record.state().formation_suppressions, [suppression]);
    assert_eq!(
        record.state().pending_resolution,
        Some(PendingResolution::MelodyCost {
            melody_id: "echo:split-earth".to_string(),
            origin: MelodyExecutionOrigin::FormationUse,
        })
    );
    answer_record_choice(&mut record, player.clone(), ChoiceAnswer::Decline).unwrap();
    finish_record_turn(&mut record);

    let old_hp = record.state().hp.clone();
    let suppressed = perform_record_formation(&mut record, &target, "weapon", &[1, 2]);
    assert!(suppressed.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, .. }
            if player == &target && formation_id == "weapon" && cards == &[card(1), card(2)]
    )));
    assert!(suppressed.contains(&GameEvent::FormationEffectIgnored {
        player: target.clone(),
        formation_id: "weapon".to_string(),
        reason: FormationNoEffectReason::SuppressedBySplitEarth {
            source: player.clone()
        },
    }));
    assert!(suppressed.contains(&GameEvent::FormationCardsDiscarded {
        player: target.clone(),
        formation_id: "weapon".to_string(),
        cards: [1, 2].map(card).to_vec(),
    }));
    let state = record.state();
    assert_eq!(state.hp, old_hp);
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(state.hand(&target).unwrap(), [3, 4, 5].map(card));
    assert!(state.discard.ends_with(&[card(1), card(2)]));
    assert!(state.formation_area(&target).unwrap().formation.is_none());

    let expired = finish_record_turn(&mut record);
    assert!(expired.contains(&GameEvent::FormationSuppressionExpired {
        target: target.clone(),
        formation_id: "weapon".to_string(),
        expired_on_turn_number: 2,
    }));
    assert!(record.state().formation_suppressions.is_empty());
    perform_record_formation(&mut record, &player, "wood-strike", &[19]);
    finish_record_turn(&mut record);
    assert_eq!(record.state().turn_number, 4);

    let old_hp = record.state().hp[0].hp;
    let later = perform_record_formation(&mut record, &target, "weapon", &[3, 4]);
    assert!(later.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { attacker, formation_id, hp_changes, .. }
            if attacker == &target && formation_id == "weapon" && hp_changes.iter().any(|change| change.change.effective_delta() < 0)
    )));
    assert!(record.state().hp[0].hp < old_hp);
    assert_eq!(record.replay().unwrap(), *record.state());
    assert_eq!(record.verify_replay().unwrap(), *record.state());
}

#[test]
fn an_ineffective_melody_neither_runs_its_main_effect_nor_offers_echo() {
    let player = PlayerId::new("p1");
    let target = PlayerId::new("p2");
    for (selected_formation, suppressed) in [("weapon", false), ("echo:falling-wood", true)] {
        let mut record =
            start_ordered_echo_record(configured_setup(), &[1, 73, 74, 75, 19, 20, 21, 2, 5]);
        // 先以一般攻擊扣除生命值，讓後續枯木的回復有實際效果。
        perform_record_formation(&mut record, &player, "metal-strike", &[1]);
        finish_record_turn(&mut record);
        perform_record_formation(&mut record, &target, "metal-strike", &[2]);
        finish_record_turn(&mut record);

        perform_record_formation(&mut record, &player, "echo:split-earth", &[73, 74]);
        select_record_formation(&mut record, &player, selected_formation);
        answer_record_choice(&mut record, player.clone(), ChoiceAnswer::Decline).unwrap();
        finish_record_turn(&mut record);

        let old_hp = record.state().hp.clone();
        let events = perform_record_formation(&mut record, &target, "echo:falling-wood", &[19, 20]);
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::FormationCommitted { player, formation_id, cards, .. }
                if player == &target && formation_id == "echo:falling-wood" && cards == &[card(19), card(20)]
        )));
        if suppressed {
            assert!(events.contains(&GameEvent::FormationEffectIgnored {
                player: target.clone(),
                formation_id: "echo:falling-wood".to_string(),
                reason: FormationNoEffectReason::SuppressedBySplitEarth {
                    source: player.clone()
                },
            }));
            assert!(!events.iter().any(|event| matches!(
                event,
                GameEvent::HpChanged { .. } | GameEvent::ChoiceRequested { .. }
            )));
            assert_eq!(record.state().hp, old_hp);
            assert!(record.state().pending_choice.is_none());
        } else {
            assert!(events.iter().any(|event| matches!(
                event,
                GameEvent::HpChanged { change } if change.effective_delta() > 0
            )));
            assert!(record.state().hp[1].hp > old_hp[1].hp);
            assert_eq!(
                record.state().pending_resolution,
                Some(PendingResolution::MelodyCost {
                    melody_id: "echo:falling-wood".to_string(),
                    origin: MelodyExecutionOrigin::FormationUse,
                })
            );
            answer_record_choice(&mut record, target.clone(), ChoiceAnswer::Decline).unwrap();
        }

        let state = record.state();
        assert!(state.hand(&target).unwrap().contains(&card(21)));
        assert!(state.discard.ends_with(&[card(19), card(20)]));
        assert!(state.formation_area(&target).unwrap().formation.is_none());
        assert_eq!(state.phase, Phase::TurnDraw);
        assert_eq!(record.replay().unwrap(), *state);
        assert_eq!(record.verify_replay().unwrap(), *state);
    }
}

#[test]
fn ringing_metal_reveals_then_shuffles_the_remainder_before_placing_it_on_top() {
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    let opening_deck = [1, 2, 19, 18, 10, 11, 12, 13, 14, 3, 4, 5];
    let mut record =
        GameRecord::start(configured_setup(), opening_deck.map(card).to_vec()).unwrap();
    record.advance_automatic().unwrap();
    let opponent_hand = record.state().hand(&opponent).unwrap().to_vec();

    let performed = perform_record_formation(&mut record, &player, "echo:ringing-metal", &[1, 2]);
    assert!(performed.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player: performer, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if performer == &player && formation_id == "echo:ringing-metal" && cards == &[card(1), card(2)]
    )));
    let search = record.state().pending_choice.as_ref().unwrap();
    assert_eq!(
        record.state().pending_resolution,
        Some(PendingResolution::MelodyRingingMetalDeckCard {
            origin: MelodyExecutionOrigin::FormationUse,
        })
    );
    assert_eq!(
        search.kind,
        PendingChoiceKind::Card {
            cards: vec![card(3), card(4), card(5)],
            minimum: 1,
            maximum: 1,
            can_decline: false,
        }
    );
    for viewer in [Viewer::Observer, Viewer::Player(opponent.clone())] {
        assert_eq!(
            state_for(record.state(), viewer).pending_choice,
            Some(PublicPendingChoice::Hidden {
                player: player.clone(),
                reason: PublicPendingChoicePresentation::EchoRingingMetalDeckCard,
            })
        );
    }
    assert_eq!(
        state_for(record.state(), Viewer::Player(player.clone())).pending_choice,
        Some(PublicPendingChoice::Visible {
            choice_id: search.choice_id,
            player: player.clone(),
            reason: PublicPendingChoicePresentation::EchoRingingMetalDeckCard,
            choice: search.kind.clone(),
        })
    );

    let selected = answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(4)],
        },
    )
    .unwrap();
    let reveal = RingingMetalSelection {
        player: player.clone(),
        card: card(4),
        deck: RandomnessDeck::Shared,
    };
    let revealed = GameEvent::RingingMetalCardRevealed {
        selection: reveal.clone(),
    };
    assert!(selected.contains(&revealed));
    assert!(events_for(&selected, Viewer::Observer).contains(&PublicGameEvent::Public(revealed)));
    let shuffle = record.state().pending_randomness.as_ref().unwrap();
    assert_eq!(
        shuffle.operation,
        RandomnessOperation::DeckShuffle {
            deck: RandomnessDeck::Shared
        }
    );
    assert_eq!(shuffle.current_order, [card(3), card(5)]);
    assert_eq!(record.state().deck, [card(3), card(5)]);

    let shuffled = resolve_record_shuffle(&mut record, vec![card(5), card(3)]);
    assert!(shuffled.contains(&GameEvent::RingingMetalCompleted { selection: reveal }));
    assert_eq!(record.state().deck, [card(4), card(5), card(3)]);
    assert_eq!(
        record.state().pending_resolution,
        Some(PendingResolution::MelodyCost {
            melody_id: "echo:ringing-metal".to_string(),
            origin: MelodyExecutionOrigin::FormationUse,
        })
    );
    assert_eq!(
        record.state().pending_choice.as_ref().unwrap().kind,
        PendingChoiceKind::Card {
            cards: vec![card(18)],
            minimum: 1,
            maximum: 1,
            can_decline: true,
        }
    );

    let declined =
        answer_record_choice(&mut record, player.clone(), ChoiceAnswer::Decline).unwrap();
    assert!(declined.contains(&GameEvent::EchoDeclined {
        player: player.clone(),
        melody_id: "echo:ringing-metal".to_string(),
    }));
    let state = record.state();
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(state.hand(&player).unwrap(), &[card(19), card(18)]);
    assert_eq!(state.hand(&opponent).unwrap(), opponent_hand);
    assert_eq!(state.discard, [card(1), card(2)]);
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn ringing_metal_empty_deck_recycles_before_an_independent_post_search_shuffle() {
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    // 開局發九張牌，兩個回合的正常抽牌共取走剩下五張。
    let opening_deck = [1, 2, 3, 4, 10, 11, 12, 13, 14, 5, 15, 16, 6, 17];
    let mut record =
        GameRecord::start(configured_setup(), opening_deck.map(card).to_vec()).unwrap();
    record.advance_automatic().unwrap();
    perform_record_formation(&mut record, &player, "weapon", &[3, 4]);
    choose_first_turn_draw_discard(&mut record);
    record.advance_automatic().unwrap();
    perform_record_formation(&mut record, &opponent, "metal-strike", &[10]);
    choose_first_turn_draw_discard(&mut record);
    record.advance_automatic().unwrap();
    assert!(record.state().deck.is_empty());
    let original_discard = [3, 4, 5, 10, 6].map(card);
    assert_eq!(record.state().discard, original_discard);

    perform_record_formation(&mut record, &player, "echo:ringing-metal", &[1, 2]);
    let recycle = record.state().pending_randomness.as_ref().unwrap();
    let recycle_id = recycle.request_id.clone();
    assert_eq!(
        recycle.operation,
        RandomnessOperation::DiscardShuffle {
            pile: RandomnessDeck::Shared,
            placement: DeckPlacement::Bottom,
        }
    );
    assert_eq!(recycle.current_order, original_discard);
    assert_eq!(
        record.state().pending_resolution,
        Some(PendingResolution::MelodyRingingMetalRecycleDiscard {
            origin: MelodyExecutionOrigin::FormationUse,
        })
    );
    assert!(record.state().pending_choice.is_none());
    assert_eq!(
        record
            .state()
            .formation_area(&player)
            .unwrap()
            .formation
            .as_ref()
            .unwrap()
            .cards,
        [card(1), card(2)]
    );

    let recycled_order = [4, 3, 5, 6, 10].map(card).to_vec();
    resolve_record_shuffle(&mut record, recycled_order.clone());
    assert!(record.state().discard.is_empty());
    assert_eq!(record.state().deck, recycled_order);
    assert_eq!(
        record.state().pending_choice.as_ref().unwrap().kind,
        PendingChoiceKind::Card {
            cards: recycled_order,
            minimum: 1,
            maximum: 1,
            can_decline: false,
        }
    );

    answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(4)],
        },
    )
    .unwrap();
    let post_search = record.state().pending_randomness.as_ref().unwrap();
    assert_ne!(post_search.request_id, recycle_id);
    assert_eq!(
        post_search.operation,
        RandomnessOperation::DeckShuffle {
            deck: RandomnessDeck::Shared
        }
    );
    assert_eq!(post_search.current_order, [3, 5, 6, 10].map(card));
    assert_eq!(
        record.state().pending_resolution,
        Some(PendingResolution::MelodyRingingMetalPostSearch {
            origin: MelodyExecutionOrigin::FormationUse,
        })
    );

    resolve_record_shuffle(&mut record, [10, 6, 5, 3].map(card).to_vec());
    answer_record_choice(&mut record, player, ChoiceAnswer::Decline).unwrap();
    let state = record.state();
    assert_eq!(state.deck, [4, 10, 6, 5, 3].map(card));
    assert_eq!(state.discard, [card(1), card(2)]);
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn ringing_metal_with_empty_piles_requests_an_empty_shuffle_before_any_card_choice() {
    // 九張牌在開局全數發完，搜尋時牌堆與棄牌堆都尚未有可取用的牌。
    let mut record = GameRecord::start(configured_setup(), (1..=9).map(card).collect()).unwrap();
    record.advance_automatic().unwrap();
    assert!(record.state().deck.is_empty() && record.state().discard.is_empty());

    let player = PlayerId::new("p1");
    let cards = vec![card(1), card(2)];
    let events = record
        .handle(Command::PerformFormation {
            player: player.clone(),
            formation_id: "echo:ringing-metal".to_string(),
            cards: cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            events.as_slice(),
            [
                GameEvent::FormationCommitted { player: performer, formation_id, cards: committed, state: FormationAreaState::FaceUpResolving, .. },
                GameEvent::RandomnessRequested {
                    request,
                    resolution: PendingResolution::MelodyRingingMetalRecycleDiscard { origin: fewfc::domain::MelodyExecutionOrigin::FormationUse },
                },
            ] if performer == &player && formation_id == "echo:ringing-metal" && committed == &cards
                && request.operation == (fewfc::domain::RandomnessOperation::DiscardShuffle {
                    pile: fewfc::domain::RandomnessDeck::Shared,
                    placement: fewfc::domain::DeckPlacement::Bottom,
                })
                && request.current_order.is_empty()
        ),
        "{events:#?}"
    );
    let state = record.state();
    assert_eq!(state.phase, Phase::Action);
    assert!(state.pending_choice.is_none());
    assert!(state.deck.is_empty() && state.discard.is_empty());
    assert_eq!(
        state
            .formation_area(&player)
            .unwrap()
            .formation
            .as_ref()
            .unwrap()
            .cards,
        cards
    );
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn ringing_metal_uses_the_performing_players_personal_deck() {
    let setup = configured_setup_with_modules(&[PERSONAL_DECK_MODULE_ID]);
    let deck = OfficialRules::new().official_deck_order(&setup).unwrap();
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    let starting_deck = record.state().deck_for(&player).unwrap().to_vec();
    let opponent_deck = record.state().deck_for(&opponent).unwrap().to_vec();
    let opponent_hand = record.state().hand(&opponent).unwrap().to_vec();
    assert!(!opponent_deck.is_empty());
    assert!(record.state().deck.is_empty());

    perform_record_formation(&mut record, &player, "echo:ringing-metal", &[1, 2]);
    assert_eq!(
        record.state().pending_choice.as_ref().unwrap().kind,
        PendingChoiceKind::Card {
            cards: starting_deck.clone(),
            minimum: 1,
            maximum: 1,
            can_decline: false,
        }
    );

    let (&searched_card, remainder) = starting_deck.split_first().unwrap();
    let selected = answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Cards {
            cards: vec![searched_card],
        },
    )
    .unwrap();
    let selection = RingingMetalSelection {
        player: player.clone(),
        card: searched_card,
        deck: RandomnessDeck::Player(player.clone()),
    };
    assert!(selected.contains(&GameEvent::RingingMetalCardRevealed {
        selection: selection.clone()
    }));
    let shuffle = record.state().pending_randomness.as_ref().unwrap();
    assert_eq!(
        shuffle.operation,
        RandomnessOperation::DeckShuffle {
            deck: RandomnessDeck::Player(player.clone()),
        }
    );
    assert_eq!(shuffle.current_order, remainder);

    let shuffled_remainder = remainder.iter().rev().copied().collect::<Vec<_>>();
    let shuffled = resolve_record_shuffle(&mut record, shuffled_remainder.clone());
    assert!(shuffled.contains(&GameEvent::RingingMetalCompleted { selection }));
    answer_record_choice(&mut record, player.clone(), ChoiceAnswer::Decline).unwrap();

    let state = record.state();
    let final_deck = state.deck_for(&player).unwrap();
    assert_eq!(final_deck[0], searched_card);
    assert_eq!(&final_deck[1..], shuffled_remainder);
    assert_eq!(state.deck_for(&opponent).unwrap(), opponent_deck);
    assert_eq!(state.hand(&opponent).unwrap(), opponent_hand);
    assert!(state.deck.is_empty());
    assert!(state.discard.is_empty());
    assert_eq!(state.player_discards[0].cards, [card(1), card(2)]);
    assert!(state.player_discards[1].cards.is_empty());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn split_earth_echo_resolves_a_fresh_choice_without_scheduling_again() {
    let player = PlayerId::new("p1");
    let target = PlayerId::new("p2");
    let mut record =
        start_ordered_echo_record(configured_setup(), &[73, 74, 75, 19, 1, 2, 3, 4, 5]);
    let original_hand = record.state().hand(&target).unwrap().to_vec();
    perform_record_formation(&mut record, &player, "echo:split-earth", &[73, 74]);
    select_record_formation(&mut record, &player, "fire-strike");
    let paid = answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(75)],
        },
    )
    .unwrap();
    let schedule = ScheduledEcho {
        player: player.clone(),
        melody_id: "echo:split-earth".to_string(),
        due_turn_number: 3,
    };
    assert!(paid.contains(&GameEvent::EchoCostPaid {
        player: player.clone(),
        melody_id: "echo:split-earth".to_string(),
        card_move: CardMoveDelta {
            card: card(75),
            from: CardZone::Hand(player.clone()),
            to: CardZone::Discard
        },
    }));
    assert!(paid.contains(&GameEvent::EchoScheduled {
        schedule: schedule.clone()
    }));
    finish_record_turn(&mut record);

    perform_record_formation(&mut record, &target, "weapon", &[1, 2]);
    let started = finish_record_turn(&mut record);
    let fresh_hand = record.state().hand(&target).unwrap().to_vec();
    assert_ne!(fresh_hand, original_hand);
    assert!(started.contains(&GameEvent::EchoResolutionStarted { schedule }));
    assert!(started.contains(&GameEvent::HandInspected {
        viewer: player.clone(),
        target: target.clone(),
        cards: fresh_hand,
    }));
    assert!(started.iter().any(|event| matches!(
        event,
        GameEvent::ChoiceRequested {
            choice,
            resolution: PendingResolution::MelodySplitEarthFormation { origin: MelodyExecutionOrigin::Echo },
        } if choice.player == player && matches!(&choice.kind,
            PendingChoiceKind::Formation { formations, can_decline: false } if formations.iter().any(|id| id == "weapon")
        )
    )));
    assert!(
        !started
            .iter()
            .any(|event| matches!(event, GameEvent::TurnStarted { .. }))
    );
    assert_eq!(record.state().phase, Phase::TurnStart);

    let answered = select_record_formation(&mut record, &player, "weapon");
    assert!(answered.contains(&GameEvent::FormationSuppressionSet {
        suppression: FormationSuppression {
            source: player.clone(),
            target,
            formation_id: "weapon".to_string(),
            expires_on_turn_number: 4,
            origin: MelodyExecutionOrigin::Echo,
        },
    }));
    assert!(answered.contains(&GameEvent::EchoResolutionCompleted {
        player: player.clone(),
        melody_id: "echo:split-earth".to_string(),
        due_turn_number: 3,
    }));
    assert!(!answered.iter().any(|event| matches!(
        event,
        GameEvent::EchoScheduled { .. } | GameEvent::ChoiceRequested { .. }
    )));
    assert!(record.state().scheduled_echoes.is_empty());
    assert!(record.state().active_echo_resolution.is_none());
    assert!(record.state().pending_choice.is_none());
    assert!(record.state().pending_resolution.is_none());

    let resumed = record.advance_automatic().unwrap();
    assert!(resumed.contains(&GameEvent::TurnStarted {
        player,
        turn_number: 3
    }));
    assert_eq!(record.state().phase, Phase::ActiveEffects);
    assert_eq!(record.replay().unwrap(), *record.state());
    assert_eq!(record.verify_replay().unwrap(), *record.state());
}

#[test]
fn echo_schedule_round_trips_through_recorded_decision_verification() {
    let setup = configured_setup();
    let leading = [19, 20, 21, 1, 2, 3, 4, 5, 6];
    let mut deck = leading.into_iter().map(card).collect::<Vec<_>>();
    deck.extend((1..=90).filter(|id| !leading.contains(id)).map(card));
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:falling-wood".to_string(),
            cards: vec![card(19), card(20)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    answer_record_choice(
        &mut record,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: vec![card(21)],
        },
    )
    .unwrap();

    assert_eq!(record.state().scheduled_echoes.len(), 1);
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn pure_fire_composite_reduction_round_trips_through_recorded_decisions() {
    let setup = configured_setup();
    let leading = [45, 67, 1, 2, 3, 4, 5, 6, 7];
    let mut deck = leading.into_iter().map(card).collect::<Vec<_>>();
    deck.extend((1..=90).filter(|id| !leading.contains(id)).map(card));
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:pure-fire".to_string(),
            cards: vec![card(45), card(67)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    answer_record_choice(
        &mut record,
        PlayerId::new("p1"),
        ChoiceAnswer::Player {
            player: PlayerId::new("p2"),
        },
    )
    .unwrap();

    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn split_earth_uses_the_next_player_in_four_player_turn_order() {
    let player = PlayerId::new("p1");
    let next = PlayerId::new("p2");
    let previous = PlayerId::new("p4");
    let mut modules = dependencies();
    modules.push(RuleModuleId::new(ECHO_MODULE_ID));
    let shape = GameSetup::team_mode(
        TeamId::new("a"),
        vec![player.clone(), PlayerId::new("p3")],
        TeamId::new("b"),
        vec![next.clone(), previous.clone()],
        30,
    );
    let setup = OfficialRules::new()
        .configure_game(shape.players, shape.turn_order, modules)
        .unwrap();
    let mut record = start_ordered_echo_record(setup, &[73, 74]);
    let inspected = record.state().hand(&next).unwrap().to_vec();
    assert_ne!(record.state().hand(&previous).unwrap(), inspected);

    let events = perform_record_formation(&mut record, &player, "echo:split-earth", &[73, 74]);
    let inspections = events
        .iter()
        .filter(|event| matches!(event, GameEvent::HandInspected { .. }))
        .collect::<Vec<_>>();
    assert_eq!(
        inspections,
        [&GameEvent::HandInspected {
            viewer: player.clone(),
            target: next.clone(),
            cards: inspected.clone(),
        }]
    );
    let owner_view = events_for(&events, Viewer::Player(player.clone()));
    let previous_view = events_for(&events, Viewer::Player(previous));
    assert!(owner_view.contains(&PublicGameEvent::HandInspected {
        viewer: player.clone(),
        target: next.clone(),
        cards: PublicCardRefs::Known(inspected.clone()),
    }));
    assert!(previous_view.contains(&PublicGameEvent::HandInspected {
        viewer: player.clone(),
        target: next.clone(),
        cards: PublicCardRefs::Hidden {
            count: inspected.len()
        },
    }));

    let chosen = select_record_formation(&mut record, &player, "weapon");
    assert!(chosen.contains(&GameEvent::FormationSuppressionSet {
        suppression: FormationSuppression {
            source: player.clone(),
            target: next.clone(),
            formation_id: "weapon".to_string(),
            expires_on_turn_number: 2,
            origin: MelodyExecutionOrigin::FormationUse,
        },
    }));
    answer_record_choice(&mut record, player, ChoiceAnswer::Decline).unwrap();
    finish_record_turn(&mut record);
    assert_eq!(record.state().current_player(), Some(&next));
    assert_eq!(record.replay().unwrap(), *record.state());
    assert_eq!(record.verify_replay().unwrap(), *record.state());
}

#[test]
fn pure_fire_and_plant_earth_require_the_published_mixed_level_seven_patterns() {
    let mut state = configured_state();
    state.hands[0].cards = vec![
        card(45),
        card(67),
        card(37),
        card(63),
        card(73),
        card(27),
        card(85),
    ];

    let pure_fire = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(45), card(67)])
            .unwrap(),
    );
    assert!(pure_fire.contains(&"echo:pure-fire".to_string()));
    let low_pure_fire = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(37), card(63)])
            .unwrap(),
    );
    assert!(!low_pure_fire.contains(&"echo:pure-fire".to_string()));

    state.team_stars.push(TeamStar {
        team: state.players[0].team.clone(),
        star: StarKind::Water,
    });
    let star_substitution = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(45), card(73)])
            .unwrap(),
    );
    assert!(!star_substitution.contains(&"echo:pure-fire".to_string()));

    let current_turn = state.turn_number;
    apply_event(
        &mut state,
        &GameEvent::ProfessionAbilityActivated {
            player: PlayerId::new("p1"),
            ability_id: "test:level-interpretation".to_string(),
            prepared: Some(PreparedProfessionAbility {
                player: PlayerId::new("p1"),
                ability_id: "test:level-interpretation".to_string(),
                card: card(37),
                element: Element::Water,
                level: fewfc::domain::EffectiveCardLevel::new(5),
                allowed_formation_scope: vec!["echo:pure-fire".to_string()],
                prepared_on_turn: current_turn,
                interpretation_revision: 1,
            }),
        },
    );
    let interpreted_level = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(37), card(63)])
            .unwrap(),
    );
    assert!(interpreted_level.contains(&"echo:pure-fire".to_string()));

    let plant_earth = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(27), card(85)])
            .unwrap(),
    );
    assert!(plant_earth.contains(&"echo:plant-earth".to_string()));
}

#[test]
fn pure_fire_is_a_successful_no_change_effect_and_schedules_free_echo() {
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    let mut record = start_ordered_echo_record(configured_setup(), &[45, 67, 1, 2, 3, 4, 5, 6, 7]);
    let hp = record.state().hp.clone();

    perform_record_formation(&mut record, &player, "echo:pure-fire", &[45, 67]);
    let answered = answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Player {
            player: opponent.clone(),
        },
    )
    .unwrap();
    let expected_echo = ScheduledEcho {
        player: player.clone(),
        melody_id: "echo:pure-fire".to_string(),
        due_turn_number: 3,
    };
    assert!(
        matches!(
            answered.as_slice(),
            [
                GameEvent::ChoiceMade { .. },
                GameEvent::TimedEffectsReduced { source, target, reductions },
                GameEvent::EchoScheduled { schedule },
                GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id, cards },
            ] if source == &player && target == &opponent && reductions.is_empty()
                && schedule == &expected_echo && discarded_by == &player
                && formation_id == "echo:pure-fire" && cards == &[card(45), card(67)]
        ),
        "{answered:#?}"
    );
    let state = record.state();
    assert_eq!(state.hp, hp);
    assert_eq!(state.scheduled_echoes, [expected_echo]);
    assert_eq!(state.hand(&player).unwrap(), [card(1), card(2)]);
    assert_eq!(state.discard, [card(45), card(67)]);
    assert!(state.pending_choice.is_none());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn pure_fire_atomically_reduces_eligible_effects_and_preserves_hidden_passive_until_trigger() {
    let mut state = configured_state();
    state
        .enabled_rule_modules
        .push(RuleModuleId::new(JIANGHU_MODULE_ID));
    state
        .enabled_rule_modules
        .push(RuleModuleId::new(CONFLUENCE_GENERATION_MODULE_ID));
    state
        .enabled_rule_modules
        .push(RuleModuleId::new(SPIRIT_MODULE_ID));
    state
        .enabled_rule_modules
        .push(RuleModuleId::new(DARK_GLIMMER_MODULE_ID));
    state.hands[0].cards = vec![card(45), card(67)];
    let performed = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:pure-fire".to_string(),
            cards: vec![card(45), card(67)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &performed);

    let target = PlayerId::new("p2");
    state.statuses.extend([
        StatusEffect {
            id: "radiance-cannot-act-p2-turn-1".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "CannotAct".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 5,
            },
        },
        StatusEffect {
            id: "radiance-cannot-draw-p2-turn-1".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "CannotDraw".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 5,
            },
        },
        StatusEffect {
            id: "spirit-stone-shield-p2-turn-3".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "SpiritStoneShield".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 3,
            },
        },
        StatusEffect {
            id: "magic-reflection-cannot-draw-p2-turn-1".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "CannotDraw".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 3,
            },
        },
        StatusEffect {
            id: "confluence-imprisoning-array-1-CannotAct".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "CannotAct".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 3,
            },
        },
        StatusEffect {
            id: "dark-dark-radiance-1-CannotDraw".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "CannotDraw".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 5,
            },
        },
        StatusEffect {
            id: "jianghu-fan-beyond-heaven-1".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "JianghuFanBeyondHeaven".to_string(),
            value: Some(20),
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 3,
            },
        },
        StatusEffect {
            id: "jianghu-lingering-frost-cannotact-1".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "CannotAct".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 3,
            },
        },
        StatusEffect {
            id: "jianghu-lingering-frost-cannotdraw-1".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "CannotDraw".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 3,
            },
        },
        StatusEffect {
            id: "battle-soul-cannot-act-p2-turn-1".to_string(),
            owner: StatusOwner::Player(target.clone()),
            kind: "CannotAct".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEndNumber {
                player: target.clone(),
                turn_number: 3,
            },
        },
    ]);
    state.formation_area_mut(&target).unwrap().formation = Some(FormationInArea {
        formation_id: "defense".to_string(),
        cards: vec![card(89), card(90)],
        star_substitution: None,
        state: FormationAreaState::FaceDownWaiting {
            ineffective_environment: None,
            sealed: false,
            revealed: false,
            neutralized: false,
            trigger_timing: PassiveTriggerTiming::NextPlayerActionStart,
        },
    });
    state.counter_effects.push(CounterEffect {
        owner: target.clone(),
        effect_id: "magic-seal".to_string(),
        established_on_turn: 0,
    });
    state.flow_layers_by_player.insert(target.clone(), 2);
    state.formation_suppressions.push(FormationSuppression {
        source: PlayerId::new("p1"),
        target: target.clone(),
        formation_id: "weapon".to_string(),
        expires_on_turn_number: 3,
        origin: fewfc::domain::MelodyExecutionOrigin::FormationUse,
    });
    state.jianghu_states.push(JianghuState {
        owner: target.clone(),
        kind: JianghuStateKind::Poison,
        remaining_turns: 2,
        expires_on_turn: None,
        last_resolved_turn: None,
    });

    let answered = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Player {
            player: target.clone(),
        },
    )
    .unwrap();
    let reductions = answered
        .iter()
        .find_map(|event| match event {
            GameEvent::TimedEffectsReduced { reductions, .. } => Some(reductions),
            _ => None,
        })
        .expect("Pure Fire must record one composite reduction");
    assert_eq!(reductions.len(), 13);
    assert_eq!(
        reductions
            .iter()
            .filter(|reduction| matches!(reduction, TimedEffectReduction::Status { .. }))
            .count(),
        8
    );
    apply_all(&mut state, &answered);

    assert_eq!(state.flow_layers_by_player.get(&target), Some(&1));
    assert!(state.formation_suppressions.is_empty());
    assert!(state.counter_effects.is_empty());
    assert_eq!(state.jianghu_states[0].remaining_turns, 1);
    assert!(matches!(
        state.covered_passive(&target).unwrap().state,
        FormationAreaState::FaceDownWaiting {
            neutralized: true,
            ..
        }
    ));
    assert!(state.statuses.iter().any(|status| {
        status.id == "spirit-stone-shield-p2-turn-3"
            && status.duration
                == StatusDuration::UntilTurnEndNumber {
                    player: target.clone(),
                    turn_number: 3,
                }
    }));
    assert!(
        state
            .statuses
            .iter()
            .any(|status| status.id == "battle-soul-cannot-act-p2-turn-1")
    );
    assert!(
        state
            .statuses
            .iter()
            .filter(|status| status.id.starts_with("radiance-"))
            .all(|status| {
                status.duration
                    == StatusDuration::UntilTurnEndNumber {
                        player: target.clone(),
                        turn_number: 3,
                    }
            })
    );

    state.pending_choice = None;
    state.phase = Phase::ActiveEffects;
    state.hands[0].cards = vec![card(1), card(2)];
    let triggered = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(2)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(triggered.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            outcome: PassiveFlipOutcome::NoEffect {
                grounds,
            },
            ..
        } if grounds == &vec![PassiveNoEffectGround::Neutralized]
    )));
}

#[test]
fn pure_fire_echo_chooses_a_fresh_target_and_does_not_schedule_again() {
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    let mut record =
        start_ordered_echo_record(configured_setup(), &[45, 67, 1, 2, 37, 38, 3, 4, 5]);
    perform_record_formation(&mut record, &player, "echo:pure-fire", &[45, 67]);
    answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Player {
            player: player.clone(),
        },
    )
    .unwrap();
    finish_record_turn(&mut record);

    perform_record_formation(&mut record, &opponent, "echo:flowing-water", &[37, 38]);
    answer_record_choice(&mut record, opponent.clone(), ChoiceAnswer::Decline).unwrap();
    // 剩餘手牌讓本次抽牌沒有加抽的空間，流水保留到淨火回響。
    let started = finish_record_turn(&mut record);
    assert_eq!(
        record.state().flow_layers_by_player.get(&opponent),
        Some(&1)
    );
    assert!(started.contains(&GameEvent::EchoResolutionStarted {
        schedule: ScheduledEcho {
            player: player.clone(),
            melody_id: "echo:pure-fire".to_string(),
            due_turn_number: 3
        },
    }));
    assert!(
        !started
            .iter()
            .any(|event| matches!(event, GameEvent::TurnStarted { .. }))
    );
    assert_eq!(
        record.state().pending_resolution,
        Some(PendingResolution::MelodyPureFireTarget {
            origin: MelodyExecutionOrigin::Echo,
        })
    );
    let hp = record.state().hp.clone();
    let hands = record.state().hands.clone();
    let discard = record.state().discard.clone();

    let answered = answer_record_choice(
        &mut record,
        player.clone(),
        ChoiceAnswer::Player {
            player: opponent.clone(),
        },
    )
    .unwrap();
    assert!(
        matches!(
            answered.as_slice(),
            [
                GameEvent::ChoiceMade { .. },
                GameEvent::TimedEffectsReduced { source, target, reductions },
                GameEvent::EchoResolutionCompleted { player: performer, melody_id, due_turn_number: 3 },
            ] if source == &player && target == &opponent && performer == &player && melody_id == "echo:pure-fire"
                && reductions == &[TimedEffectReduction::FlowState { player: opponent.clone(), old_layers: 1, new_layers: 0 }]
        ),
        "{answered:#?}"
    );
    let state = record.state();
    assert!(!state.flow_layers_by_player.contains_key(&opponent));
    assert!(state.pending_choice.is_none());
    assert!(state.active_echo_resolution.is_none());
    assert!(state.scheduled_echoes.is_empty());
    assert_eq!(state.hands, hands);
    assert_eq!(state.discard, discard);
    assert_eq!(state.hp, hp);
    assert!(
        record
            .advance_automatic()
            .unwrap()
            .contains(&GameEvent::TurnStarted {
                player,
                turn_number: 3
            })
    );
    assert_eq!(record.replay().unwrap(), *record.state());
    assert_eq!(record.verify_replay().unwrap(), *record.state());
}

#[test]
fn lethal_turn_start_melody_completes_its_resolution_without_entering_main() {
    let mut state = configured_state();
    state.scheduled_echoes.push(ScheduledEcho {
        player: PlayerId::new("p1"),
        melody_id: "echo:war-fire".to_string(),
        due_turn_number: 3,
    });
    state
        .hp
        .iter_mut()
        .find(|entry| entry.team == TeamId::new("team:p2"))
        .unwrap()
        .hp = 10;
    state.turn_number = 3;
    state.current_turn_index = 0;
    state.phase = Phase::TurnStart;

    let events = advance_automatic(&state).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.new_hp() == 0
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::EchoResolutionCompleted { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::TurnStarted { .. }))
    );
}

#[test]
fn plant_earth_accepts_all_five_basic_melodies_and_finishes_nested_choices() {
    let melodies = [
        "echo:ringing-metal",
        "echo:falling-wood",
        "echo:flowing-water",
        "echo:war-fire",
        "echo:split-earth",
    ];
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    for melody_id in melodies {
        let mut record =
            start_ordered_echo_record(configured_setup(), &[27, 85, 19, 20, 1, 2, 3, 4, 5]);
        perform_record_formation(&mut record, &player, "echo:plant-earth", &[27, 85]);
        finish_record_turn(&mut record);
        perform_record_formation(&mut record, &opponent, "metal-strike", &[1]);
        finish_record_turn(&mut record);

        assert_eq!(
            record.state().pending_choice.as_ref().unwrap().kind,
            PendingChoiceKind::Formation {
                formations: melodies.map(str::to_string).to_vec(),
                can_decline: false,
            }
        );
        let old_hp = record.state().hp.clone();
        let old_discard = record.state().discard.clone();
        let mut events = select_record_formation(&mut record, &player, melody_id);

        match melody_id {
            "echo:ringing-metal" => {
                assert_eq!(
                    record.state().pending_resolution,
                    Some(PendingResolution::MelodyRingingMetalDeckCard {
                        origin: MelodyExecutionOrigin::PlantedEarth,
                    })
                );
                let top = record.state().deck[0];
                events.extend(
                    answer_record_choice(
                        &mut record,
                        player.clone(),
                        ChoiceAnswer::Cards { cards: vec![top] },
                    )
                    .unwrap(),
                );
                assert_eq!(
                    record.state().pending_resolution,
                    Some(PendingResolution::MelodyRingingMetalPostSearch {
                        origin: MelodyExecutionOrigin::PlantedEarth,
                    })
                );
                let remainder = record
                    .state()
                    .deck
                    .iter()
                    .rev()
                    .copied()
                    .collect::<Vec<_>>();
                events.extend(resolve_record_shuffle(&mut record, remainder.clone()));
                assert_eq!(record.state().deck[0], top);
                assert_eq!(&record.state().deck[1..], remainder);
                assert!(events.contains(&GameEvent::RingingMetalCompleted {
                    selection: RingingMetalSelection {
                        player: player.clone(),
                        card: top,
                        deck: RandomnessDeck::Shared
                    },
                }));
            }
            "echo:falling-wood" => assert!(record.state().hp[0].hp > old_hp[0].hp),
            "echo:flowing-water" => assert!(events.contains(&GameEvent::FlowStateChanged {
                player: player.clone(),
                old_layers: 0,
                new_layers: 1,
            })),
            "echo:war-fire" => assert_eq!(record.state().hp[1].hp, old_hp[1].hp - 15),
            "echo:split-earth" => {
                assert_eq!(
                    record.state().pending_resolution,
                    Some(PendingResolution::MelodySplitEarthFormation {
                        origin: MelodyExecutionOrigin::PlantedEarth,
                    })
                );
                events.extend(select_record_formation(&mut record, &player, "weapon"));
                assert!(events.contains(&GameEvent::FormationSuppressionSet {
                    suppression: FormationSuppression {
                        source: player.clone(),
                        target: opponent.clone(),
                        formation_id: "weapon".to_string(),
                        expires_on_turn_number: 4,
                        origin: MelodyExecutionOrigin::PlantedEarth
                    },
                }));
            }
            _ => unreachable!(),
        }

        assert!(events.contains(&GameEvent::PlantEarthResolutionCompleted {
            player: player.clone(),
            due_turn_number: 3,
            melody_id: melody_id.to_string(),
        }));
        assert!(!events.iter().any(|event| matches!(
            event,
            GameEvent::EchoScheduled { .. }
                | GameEvent::EchoCostPaid { .. }
                | GameEvent::FormationCommitted { .. }
        )));
        let state = record.state();
        assert!(state.active_plant_earth_resolution.is_none());
        assert!(state.scheduled_plant_earth.is_empty());
        assert!(state.scheduled_echoes.is_empty());
        assert!(state.pending_resolution.is_none());
        assert_eq!(state.discard, old_discard);
        assert_eq!(record.replay().unwrap(), *state);
        assert_eq!(record.verify_replay().unwrap(), *state);
    }
}

#[test]
fn plant_earth_schedules_a_fresh_turn_start_melody_without_echo() {
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    let mut record =
        start_ordered_echo_record(configured_setup(), &[27, 85, 19, 20, 1, 2, 3, 4, 5]);
    let schedule = ScheduledPlantEarth {
        player: player.clone(),
        due_turn_number: 3,
    };

    let performed = perform_record_formation(&mut record, &player, "echo:plant-earth", &[27, 85]);
    assert!(performed.contains(&GameEvent::PlantEarthScheduled {
        schedule: schedule.clone()
    }));
    assert!(
        !performed
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
    assert_eq!(record.state().scheduled_plant_earth, [schedule.clone()]);
    assert_eq!(record.state().discard, [card(27), card(85)]);
    assert_eq!(record.state().phase, Phase::TurnDraw);
    finish_record_turn(&mut record);

    perform_record_formation(&mut record, &opponent, "metal-strike", &[1]);
    let started = finish_record_turn(&mut record);
    assert!(started.contains(&GameEvent::PlantEarthResolutionStarted { schedule }));
    assert!(
        !started
            .iter()
            .any(|event| matches!(event, GameEvent::TurnStarted { .. }))
    );
    assert_eq!(record.state().phase, Phase::TurnStart);
    assert_eq!(
        record.state().pending_resolution,
        Some(PendingResolution::MelodyPlantEarthMelody {
            origin: MelodyExecutionOrigin::PlantedEarth,
        })
    );
    let hp = record.state().hp.clone();

    let selected = select_record_formation(&mut record, &player, "echo:falling-wood");
    assert!(
        matches!(
            selected.as_slice(),
            [
                GameEvent::ChoiceMade { .. },
                GameEvent::HpChanged { change },
                GameEvent::PlantEarthResolutionCompleted { player: performer, due_turn_number: 3, melody_id },
            ] if performer == &player && melody_id == "echo:falling-wood"
                && change.team() == &hp[0].team && change.old_hp() == hp[0].hp
                && change.delta() == 15 && change.effective_delta() > 0
        ),
        "{selected:#?}"
    );
    assert_eq!(record.state().hp[1], hp[1]);
    assert!(record.state().active_plant_earth_resolution.is_none());
    assert!(record.state().pending_choice.is_none());
    assert!(record.state().scheduled_echoes.is_empty());
    assert!(
        record
            .advance_automatic()
            .unwrap()
            .contains(&GameEvent::TurnStarted {
                player,
                turn_number: 3
            })
    );
    assert_eq!(record.replay().unwrap(), *record.state());
    assert_eq!(record.verify_replay().unwrap(), *record.state());
}

#[test]
fn plant_earth_turn_start_choice_records_automatic_metadata_without_panicking() {
    let setup = configured_setup();
    let leading = [1, 2, 8, 9, 27, 85, 3, 4, 5];
    let mut deck = leading.into_iter().map(card).collect::<Vec<_>>();
    deck.extend((1..=90).filter(|id| !leading.contains(id)).map(card));
    let mut record = GameRecord::start(setup, deck).unwrap();

    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    choose_first_turn_draw_discard(&mut record);
    record.advance_automatic().unwrap();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "echo:plant-earth".to_string(),
            cards: vec![card(27), card(85)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    choose_first_turn_draw_discard(&mut record);
    record.advance_automatic().unwrap();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    choose_first_turn_draw_discard(&mut record);

    let started = record.advance_automatic().unwrap();
    assert!(
        started
            .iter()
            .any(|event| matches!(event, GameEvent::PlantEarthResolutionStarted { .. }))
    );
    assert!(
        started
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
    assert!(record.state().pending_choice.is_some());
    let public_choice = record
        .public_view(Viewer::Player(PlayerId::new("p1")))
        .unwrap()
        .pending_choice
        .expect("Plant Earth choice must be visible as a pending decision");
    assert!(matches!(
        public_choice,
        PublicPendingChoice::Hidden { player, .. } if player == PlayerId::new("p2")
    ));
    assert!(
        record
            .recorded_events()
            .iter()
            .rev()
            .take(2)
            .all(|recorded| {
                recorded.metadata.source
                    == EventSource::Automatic {
                        reason: AutomaticReason::EchoResolution,
                    }
            })
    );
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

fn choose_first_turn_draw_discard(record: &mut GameRecord) {
    let events = record.advance_automatic().unwrap();
    let (player, choice_id, discard) = events
        .iter()
        .find_map(|event| match event {
            GameEvent::ChoiceRequested { choice, .. } => Some((
                choice.player.clone(),
                choice.choice_id,
                match &choice.kind {
                    fewfc::domain::PendingChoiceKind::Card { cards, .. } => cards[0],
                    _ => panic!("turn draw must use a card choice"),
                },
            )),
            _ => None,
        })
        .expect("turn draw must request a discard");
    record
        .handle(Command::AnswerChoice {
            player,
            choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        })
        .unwrap();
}

fn perform_record_formation(
    record: &mut GameRecord,
    player: &PlayerId,
    formation_id: &str,
    cards: &[u64],
) -> Vec<GameEvent> {
    record
        .handle(Command::PerformFormation {
            player: player.clone(),
            formation_id: formation_id.to_string(),
            cards: cards.iter().copied().map(card).collect(),
            declared_targets: Vec::new(),
        })
        .unwrap()
}

fn resolve_record_shuffle(
    record: &mut GameRecord,
    shuffled_order: Vec<CardInstanceId>,
) -> Vec<GameEvent> {
    let request_id = record
        .state()
        .pending_randomness
        .as_ref()
        .unwrap()
        .request_id
        .clone();
    record
        .resolve_randomness(TrustedRandomnessAnswer {
            request_id,
            shuffled_order,
        })
        .unwrap()
        .into_events()
}

fn start_ordered_echo_record(setup: GameSetup, first_cards: &[u64]) -> GameRecord {
    let mut deck = first_cards.iter().copied().map(card).collect::<Vec<_>>();
    let remaining = OfficialRules::new()
        .official_deck_order(&setup)
        .unwrap()
        .into_iter()
        .filter(|card| !deck.contains(card))
        .collect::<Vec<_>>();
    deck.extend(remaining);
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();
    record
}

fn select_record_formation(
    record: &mut GameRecord,
    player: &PlayerId,
    formation_id: &str,
) -> Vec<GameEvent> {
    answer_record_choice(
        record,
        player.clone(),
        ChoiceAnswer::Formation {
            formation_id: formation_id.to_string(),
        },
    )
    .unwrap()
}

fn finish_record_turn(record: &mut GameRecord) -> Vec<GameEvent> {
    choose_first_turn_draw_discard(record);
    record.advance_automatic().unwrap()
}
