use fewfc::application::{GameRecord, apply_event, handle_command, resolve_trusted_randomness};
use fewfc::domain::{
    AttackOutcome, AttackResolutionEffects, AttackStatusRemoval, CardInstanceId, CardMoveDelta,
    CardOrigin, CardZone, ChoiceAnswer, Command, DARK_GLIMMER_MODULE_ID, ECHO_MODULE_ID,
    EarthRendingPlayerAnswer, Element, EnvironmentTransferDelta, FIVE_DIRECTIONS_LEGEND_MODULE_ID,
    FormationNoEffectReason, GameEvent, GameSetup, GameState, HERO_SCHOOLS_MODULE_ID, HpChangeRole,
    LimitedUse, PERSONAL_DECK_MODULE_ID, PendingChoiceKind, PendingResolution, Phase, PlayerId,
    PlayerProfession, PlayerSpirit, ProfessionId, RandomnessDeck, RandomnessOperation,
    RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID, ShieldChangeDelta, SpiritKind, StatusDuration,
    StatusEffect, StatusOwner, TRIBULATION_MODULE_ID, TeamId, TrustedRandomnessAnswer,
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

const TRIBULATION_MODULES: [&str; 4] = [
    STAR_MODULE_ID,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID,
    HERO_SCHOOLS_MODULE_ID,
    TRIBULATION_MODULE_ID,
];

fn setup() -> GameSetup {
    setup_with_modules(&[])
}

fn setup_with_modules(extra: &[&str]) -> GameSetup {
    let mut modules = TRIBULATION_MODULES.to_vec();
    modules.extend_from_slice(extra);
    ScenarioPlan::two_player(&modules).setup().unwrap()
}

fn state() -> GameState {
    let mut state = GameState::from_setup(&setup());
    state.phase = Phase::ActiveEffects;
    state
}

fn personal_deck_state() -> GameState {
    let mut state = GameState::from_setup(&setup_with_modules(&[PERSONAL_DECK_MODULE_ID]));
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
    for before in &state.hp {
        assert!(attack_hp_changes.iter().any(|resolved| {
            resolved.role == HpChangeRole::FormationEffect
                && resolved.change.team() == &before.team
                && resolved.change.old_hp() == before.hp
                && resolved.change.effective_delta() == -15
        }));
    }
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown,
            hp_changes,
            ..
        } if point_breakdown.base_points == 60
            && hp_changes.iter().any(|resolved|
                matches!(&resolved.role, HpChangeRole::AttackDamage { target } if target == &PlayerId::new("p2"))
                    && resolved.change.old_hp() == 185 && resolved.change.effective_delta() == -60)
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
        tribulation
            .iter()
            .filter(|event| matches!(event, GameEvent::AttackResolved { .. }))
            .count(),
        1
    );
    assert!(tribulation.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_changes, .. }
            if hp_changes.len() == 2 && hp_changes.iter().all(|resolved|
                resolved.change.team() == &TeamId::new("team:p2"))
    )));
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
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let thunder_fire = "tribulation:thunder-fire";
    let formation = [13, 16, 67, 70].map(card);

    // 以水環境作為對照，確認裂土會在同一個解析邊界阻止天雷劫火的傷害與附加效果。
    for (previous_formation, divine_calculation) in [
        ("water-strike", false),
        ("water-strike", true),
        ("north-black-tortoise", true),
        ("echo:split-earth", false),
        ("echo:split-earth", true),
    ] {
        let suppressed = previous_formation == "echo:split-earth";
        let opening = [18, 13, 16, 67, 37, 38, 39, 74, 75, 10, 70, 1];
        let mut opening = opening.map(card);
        if previous_formation == "north-black-tortoise" {
            opening[7..9].copy_from_slice(&[40, 41].map(card));
        }
        let mut record = start_tribulation_record(setup_with_modules(&[ECHO_MODULE_ID]), &opening);

        let first_formation = if divine_calculation {
            "tribulation:divine-calculation"
        } else {
            "metal-strike"
        };
        perform_record_formation(&mut record, &p1, first_formation, &[card(18)]);
        let protection = record
            .state()
            .statuses
            .iter()
            .find(|status| status.kind == "DivineCalculation")
            .map(|status| AttackStatusRemoval {
                status_id: status.id.clone(),
                owner: status.owner.clone(),
            });
        assert_eq!(
            protection.as_ref().map(|status| &status.owner),
            divine_calculation.then_some(&StatusOwner::Player(p1.clone()))
        );
        finish_record_turn(&mut record, &p1);

        let previous_cards = match previous_formation {
            "water-strike" => vec![card(37)],
            "north-black-tortoise" => [37, 38, 39, 40, 41].map(card).to_vec(),
            "echo:split-earth" => [74, 75].map(card).to_vec(),
            _ => unreachable!(),
        };
        perform_record_formation(&mut record, &p2, previous_formation, &previous_cards);
        if suppressed {
            answer_record_choice(
                &mut record,
                &p2,
                ChoiceAnswer::Formation {
                    formation_id: thunder_fire.to_string(),
                },
            );
            answer_record_choice(&mut record, &p2, ChoiceAnswer::Decline);
        }
        finish_record_turn(&mut record, &p2);

        let hp_before = record.state().hp.clone();
        let environment = record.state().environment;
        assert_eq!(
            environment,
            (previous_formation == "north-black-tortoise").then_some(Element::Water)
        );
        let events = perform_record_formation(&mut record, &p1, thunder_fire, &formation);
        let effects = attack_effects(&events);
        assert_eq!(
            effects.statuses_removed,
            protection.into_iter().collect::<Vec<_>>()
        );
        if suppressed {
            assert!(events.contains(&GameEvent::FormationEffectIgnored {
                player: p1.clone(),
                formation_id: thunder_fire.to_string(),
                reason: FormationNoEffectReason::SuppressedBySplitEarth { source: p2.clone() },
            }));
            assert_eq!(effects.outcome, AttackOutcome::DamagePrevented);
            assert!(events.iter().any(|event| matches!(
                event,
                GameEvent::AttackResolved { hp_changes, shield_change: None, .. } if hp_changes.is_empty()
            )));
            assert_eq!(record.state().hp, hp_before);
        } else {
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, GameEvent::FormationEffectIgnored { .. }))
            );
            assert_eq!(effects.outcome, AttackOutcome::Resolved);
            assert_eq!(
                record.state().hp[0].hp,
                hp_before[0].hp - if divine_calculation { 0 } else { 15 }
            );
            assert_eq!(record.state().hp[1].hp, hp_before[1].hp - 75);
        }
        assert!(events.iter().any(|event| matches!(event,
            GameEvent::FormationCommitted { player, formation_id, cards, .. }
                if player == &p1 && formation_id == thunder_fire && cards == &formation
        )));
        assert!(events.contains(&GameEvent::FormationCardsDiscarded {
            player: p1.clone(),
            formation_id: thunder_fire.to_string(),
            cards: formation.to_vec(),
        }));
        let state = record.state();
        assert!(state.discard.ends_with(&formation));
        assert!(
            state
                .statuses
                .iter()
                .all(|status| status.kind != "DivineCalculation")
        );
        assert_eq!(state.environment, environment);
        assert_eq!(state.phase, Phase::TurnDraw);
        assert_eq!(record.replay().unwrap(), *state);
        assert_eq!(record.verify_replay().unwrap(), *state);
    }
}

#[test]
fn mudslide_uses_eighty_points_only_after_effective_global_shield_loss() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mudslide = "tribulation:mudslide-torrent";
    let formation = [49, 52, 85, 88].map(card);

    for (target_barrier, old_target_shield, own_shield, protected) in [
        (None, 0, false, false),
        (Some([20, 21, 1, 55]), 16, true, false),
        (Some([24, 27, 9, 63]), 44, true, false),
        (Some([24, 27, 9, 63]), 44, false, true),
    ] {
        let own_barrier = [19, 23, 5, 60];
        let mut opening = if own_shield {
            own_barrier.to_vec()
        } else {
            vec![19, 49, 85, 1]
        };
        opening.extend(target_barrier.unwrap_or([20, 21, 6, 55]));
        opening.push(if protected { 16 } else { 37 });
        opening.extend(
            [2, 52, 88, 49, 85]
                .into_iter()
                .take(if own_shield { 5 } else { 3 }),
        );

        let mut configured = setup();
        // 固定抽牌數，讓打出四張氣壁後仍可在下一次行動持有泥石轟流的四張牌。
        configured.base_draw = 4;
        let mut record = start_tribulation_record(
            configured,
            &opening.into_iter().map(card).collect::<Vec<_>>(),
        );
        if own_shield {
            perform_record_formation(&mut record, &p1, "barrier", &own_barrier.map(card));
        } else {
            perform_record_formation(&mut record, &p1, "wood-strike", &[card(19)]);
        }
        assert_eq!(
            record.state().shield(&p1),
            Some(if own_shield { 28 } else { 0 })
        );
        finish_record_turn(&mut record, &p1);

        if let Some(barrier) = target_barrier {
            perform_record_formation(&mut record, &p2, "barrier", &barrier.map(card));
        } else {
            record
                .handle(Command::ChangeProfession {
                    player: p2.clone(),
                    profession: ProfessionId::new("first-wanderer"),
                    cards: vec![card(37)],
                })
                .unwrap();
        }
        assert_eq!(record.state().shield(&p2), Some(old_target_shield));
        finish_record_turn(&mut record, &p2);

        let protection = if protected {
            // 先建立氣壁，再施放神算。兩個回合都保留 P2 的護盾。
            record
                .handle(Command::ChangeProfession {
                    player: p1.clone(),
                    profession: ProfessionId::new("first-wanderer"),
                    cards: vec![card(1)],
                })
                .unwrap();
            finish_record_turn(&mut record, &p1);
            perform_record_formation(
                &mut record,
                &p2,
                "tribulation:divine-calculation",
                &[card(16)],
            );
            let status = record
                .state()
                .statuses
                .iter()
                .find(|status| status.kind == "DivineCalculation")
                .unwrap();
            let removal = AttackStatusRemoval {
                status_id: status.id.clone(),
                owner: StatusOwner::Player(p2.clone()),
            };
            finish_record_turn(&mut record, &p2);
            Some(removal)
        } else {
            None
        };

        let hp_before = record.state().hp.clone();
        assert_eq!(record.state().shield(&p2), Some(old_target_shield));
        let base_points = if own_shield { 80 } else { 60 };
        let shield_after_effect = if protected {
            old_target_shield
        } else {
            (old_target_shield - 20_i32).max(0)
        };
        let events = perform_record_formation(&mut record, &p1, mudslide, &formation);
        let effects = attack_effects(&events);
        let expected_shield_changes = if own_shield {
            vec![
                ShieldChangeDelta {
                    player: p1.clone(),
                    old_value: 28,
                    delta: -20,
                    new_value: 8,
                },
                ShieldChangeDelta {
                    player: p2.clone(),
                    old_value: old_target_shield,
                    delta: shield_after_effect - old_target_shield,
                    new_value: shield_after_effect,
                },
            ]
        } else {
            Vec::new()
        };
        assert_eq!(effects.shield_changes, expected_shield_changes);
        assert_eq!(
            effects.statuses_removed,
            protection.into_iter().collect::<Vec<_>>()
        );
        let attack_shield_change = (shield_after_effect != 0).then(|| ShieldChangeDelta {
            player: p2.clone(),
            old_value: shield_after_effect,
            delta: -base_points,
            new_value: 0,
        });
        assert!(
            matches!(
                events.as_slice(),
                [
                    GameEvent::FormationCommitted { player, formation_id, cards, .. },
                    GameEvent::AttackResolved { attacker, target, formation_id: resolved, used_cards, point_breakdown, shield_change, hp_changes, .. },
                    GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
                ] if player == &p1 && attacker == &p1 && target == &p2 && discarded_by == &p1
                    && formation_id == mudslide && resolved == mudslide && discarded == mudslide
                    && cards == &formation && used_cards == &formation && discarded_cards == &formation
                    && point_breakdown.base_points == base_points && shield_change == &attack_shield_change
                    && hp_changes.len() == usize::from(attack_shield_change.is_none())
            ),
            "{events:#?}"
        );
        let state = record.state();
        assert_eq!(state.shield(&p1), Some(if own_shield { 8 } else { 0 }));
        assert_eq!(state.shield(&p2), Some(0));
        assert_eq!(state.hp[0], hp_before[0]);
        assert_eq!(
            state.hp[1].hp,
            hp_before[1].hp
                - if attack_shield_change.is_none() {
                    base_points
                } else {
                    0
                }
        );
        assert!(state.discard.ends_with(&formation));
        assert_eq!(state.phase, Phase::TurnDraw);
        assert_eq!(record.replay().unwrap(), *state);
        assert_eq!(record.verify_replay().unwrap(), *state);
    }
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
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let earth_rending = "tribulation:earth-rending";
    let formation = [31, 34, 85, 88].map(card);
    let p2_hand = [67, 1, 2, 3, 4].map(card);
    let opening = [formation.as_slice(), p2_hand.as_slice()].concat();
    let mut record = start_tribulation_record(setup(), &opening);
    let hp_before = record.state().hp.clone();

    let started = perform_record_formation(&mut record, &p1, earth_rending, &formation);
    assert!(
        matches!(
            started.as_slice(),
            [
                GameEvent::FormationCommitted { player, formation_id, cards, .. },
                GameEvent::EarthRendingStarted { resolution },
                GameEvent::ChoiceRequested { choice, resolution: PendingResolution::TribulationEarthRendingEnvironment },
            ] if player == &p1 && formation_id == earth_rending && cards == &formation
                && resolution.attacker == p1 && resolution.remaining_players == [p2.clone(), p1.clone()]
                && choice.player == p1 && matches!(&choice.kind, PendingChoiceKind::Environment { can_decline: false, .. })
        ),
        "{started:#?}"
    );

    let environment = answer_record_choice(
        &mut record,
        &p1,
        ChoiceAnswer::Environment {
            environment: Element::Fire,
        },
    );
    assert!(
        environment.contains(&GameEvent::EarthRendingEnvironmentChosen {
            environment: Element::Fire
        })
    );
    assert!(
        !environment
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
    );
    let state = record.state();
    assert_eq!(state.phase, Phase::Action);
    assert_eq!(state.environment, None);
    assert_eq!(state.hp, hp_before);
    assert_eq!(state.hand(&p2), Some(p2_hand.as_slice()));
    assert!(state.discard.is_empty());
    let pending = state.pending_choice.as_ref().unwrap();
    let kind = PendingChoiceKind::Card {
        cards: vec![card(67)],
        minimum: 1,
        maximum: 1,
        can_decline: false,
    };
    assert_eq!(
        state.pending_resolution,
        Some(PendingResolution::TribulationEarthRendingCard)
    );
    assert!(matches!(
        state_for(state, Viewer::Player(p1.clone())).pending_choice,
        Some(PublicPendingChoice::Hidden { player, .. }) if player == p2
    ));
    assert!(matches!(
        state_for(state, Viewer::Player(p2.clone())).pending_choice,
        Some(PublicPendingChoice::Visible { player, choice_id, choice, .. })
            if player == p2 && choice_id == pending.choice_id && choice == kind
    ));
    assert_eq!(record.verify_replay().unwrap(), *state);

    let recovered: GameState =
        serde_json::from_str(&serde_json::to_string(state).unwrap()).unwrap();
    let answer = ChoiceAnswer::Cards {
        cards: vec![card(67)],
    };
    let expected = answer_choice(&recovered, p2.clone(), answer.clone()).unwrap();
    let events = answer_record_choice(&mut record, &p2, answer);
    assert_eq!(events, expected);
    assert!(events.contains(&GameEvent::HandRevealed {
        player: p1.clone(),
        cards: Vec::new()
    }));
    let effects = attack_effects(&events);
    assert_eq!(
        effects.environment_transfers,
        [EnvironmentTransferDelta {
            player: p1.clone(),
            formation_id: earth_rending.to_string(),
            from: None,
            to: Element::Fire,
        }]
    );
    assert_eq!(
        effects.card_moves,
        [CardMoveDelta {
            card: card(67),
            from: CardZone::Hand(p2.clone()),
            to: CardZone::Discard,
        }]
    );
    assert!(events.contains(&GameEvent::EarthRendingCompleted { player: p1.clone() }));
    assert!(events.contains(&GameEvent::FormationCardsDiscarded {
        player: p1.clone(),
        formation_id: earth_rending.to_string(),
        cards: formation.to_vec(),
    }));
    let state = record.state();
    assert_eq!(state.environment, Some(Element::Fire));
    assert_eq!(state.hp[0], hp_before[0]);
    assert_eq!(state.hp[1].hp, hp_before[1].hp - 60);
    assert_eq!(state.discard, [67, 31, 34, 85, 88].map(card));
    assert_eq!(state.hand(&p2), Some(&p2_hand[1..]));
    assert!(state.active_earth_rending_resolution.is_none());
    assert!(state.pending_resolution.is_none());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn earth_rending_collects_four_player_answers_in_turn_order_and_performer_last() {
    let [p1, p2, p3, p4] = ["p1", "p2", "p3", "p4"].map(PlayerId::new);
    let shape = GameSetup::team_mode(
        TeamId::new("a"),
        vec![p1.clone(), p3.clone()],
        TeamId::new("b"),
        vec![p2.clone(), p4.clone()],
        30,
    );
    let configured = OfficialRules::new()
        .configure_game(
            shape.players,
            shape.turn_order,
            setup().enabled_rule_modules,
        )
        .unwrap();
    let formation = [31, 34, 85, 88].map(card);
    let p3_hand = [5, 6, 7, 8, 9].map(card);
    let opening = [
        formation.as_slice(),
        &[67, 1, 2, 3, 4].map(card),
        p3_hand.as_slice(),
        &[70, 10, 11, 12, 13].map(card),
    ]
    .concat();
    let mut record = start_tribulation_record(configured, &opening);
    let hp_before = record.state().hp.clone();

    perform_record_formation(&mut record, &p1, "tribulation:earth-rending", &formation);
    answer_record_choice(
        &mut record,
        &p1,
        ChoiceAnswer::Environment {
            environment: Element::Fire,
        },
    );
    assert_eq!(
        record
            .state()
            .pending_choice
            .as_ref()
            .map(|choice| &choice.player),
        Some(&p2)
    );

    let second = answer_record_choice(
        &mut record,
        &p2,
        ChoiceAnswer::Cards {
            cards: vec![card(67)],
        },
    );
    assert!(second.contains(&GameEvent::HandRevealed {
        player: p3.clone(),
        cards: p3_hand.to_vec()
    }));
    let state = record.state();
    assert_eq!(
        state.pending_choice.as_ref().map(|choice| &choice.player),
        Some(&p4)
    );
    let progress = state.active_earth_rending_resolution.as_ref().unwrap();
    assert_eq!(
        progress
            .answers
            .iter()
            .map(|answer| &answer.player)
            .collect::<Vec<_>>(),
        [&p2, &p3]
    );
    assert_eq!(progress.remaining_players, [p4.clone(), p1.clone()]);
    assert_eq!(state.environment, None);
    assert_eq!(state.hp, hp_before);
    assert!(state.hand(&p2).unwrap().contains(&card(67)));
    assert!(state.discard.is_empty());

    let fourth = answer_record_choice(
        &mut record,
        &p4,
        ChoiceAnswer::Cards {
            cards: vec![card(70)],
        },
    );
    let final_answers = fourth
        .iter()
        .filter_map(|event| match event {
            GameEvent::EarthRendingPlayerAnswered { answer } => Some(answer.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        final_answers,
        [
            EarthRendingPlayerAnswer {
                player: p4.clone(),
                card: Some(card(70)),
                revealed_hand: Vec::new(),
                protected: false
            },
            EarthRendingPlayerAnswer {
                player: p1.clone(),
                card: None,
                revealed_hand: Vec::new(),
                protected: false
            },
        ]
    );
    assert!(
        matches!(
            fourth.iter().filter(|event| matches!(event,
                GameEvent::HandRevealed { .. } | GameEvent::AttackResolved { .. } | GameEvent::EarthRendingCompleted { .. }
            )).collect::<Vec<_>>().as_slice(),
            [
                GameEvent::HandRevealed { player, cards },
                GameEvent::AttackResolved { target, .. },
                GameEvent::EarthRendingCompleted { player: completed },
            ] if player == &p1 && cards.is_empty() && target == &p4 && completed == &p1
        ),
        "{fourth:#?}"
    );
    assert_eq!(
        attack_effects(&fourth).card_moves,
        [
            CardMoveDelta {
                card: card(67),
                from: CardZone::Hand(p2),
                to: CardZone::Discard
            },
            CardMoveDelta {
                card: card(70),
                from: CardZone::Hand(p4),
                to: CardZone::Discard
            },
        ]
    );
    let state = record.state();
    assert_eq!(state.environment, Some(Element::Fire));
    assert_eq!(state.discard, [67, 70, 31, 34, 85, 88].map(card));
    assert_eq!(state.hand(&p3), Some(p3_hand.as_slice()));
    assert_eq!(state.hp[0], hp_before[0]);
    assert_eq!(state.hp[1].hp, hp_before[1].hp - 60);
    assert!(state.active_earth_rending_resolution.is_none());
    assert!(state.pending_choice.is_none());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn rusted_forest_reveals_discards_and_waits_for_the_trusted_shuffle() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let forest = "tribulation:rusted-forest";
    let revealed = [9, 19, 27, 37, 45, 56, 63, 73].map(card);
    let opening = [
        // P1 保留鏽鐵枯林的陣形牌。P2 依序召靈、轉魔靈師、召死精靈。
        13, 16, 31, 1, 55, 59, 67, 32, 20, 2, 34, 3, 4, 85, 14, 5, 6, 7, 8, 10, 11, 12, 15, 17, 18,
    ]
    .map(card);
    let mut record = start_tribulation_record(
        setup_with_modules(&[SPIRIT_MODULE_ID, DARK_GLIMMER_MODULE_ID]),
        &[opening.as_slice(), revealed.as_slice()].concat(),
    );
    perform_record_formation(&mut record, &p1, "metal-strike", &[card(1)]);
    finish_record_turn(&mut record, &p1);
    perform_record_formation(
        &mut record,
        &p2,
        "fire-spirit-summoning",
        &[card(55), card(59)],
    );
    finish_record_turn(&mut record, &p2);
    perform_record_formation(&mut record, &p1, "metal-strike", &[card(3)]);
    finish_record_turn(&mut record, &p1);
    record
        .handle(Command::ChangeProfession {
            player: p2.clone(),
            profession: ProfessionId::new("dark:demon-spirit-master"),
            cards: vec![card(67), card(32)],
        })
        .unwrap();
    finish_record_turn(&mut record, &p2);
    perform_record_formation(&mut record, &p1, "metal-strike", &[card(6)]);
    finish_record_turn(&mut record, &p1);
    let summoned = perform_record_formation(
        &mut record,
        &p2,
        "dark:death-spirit-summoning",
        &[card(85), card(14)],
    );
    assert!(summoned.contains(&GameEvent::SpiritPowerChanged {
        player: p2.clone(),
        spirit: SpiritKind::Death,
        old_power: 2,
        delta: 2,
        new_power: 4,
        reason: fewfc::domain::SpiritPowerChangeReason::SkillEffect,
    }));
    finish_record_turn(&mut record, &p2);

    let state = record.state();
    let hp_before = state.hp.clone();
    let spirit = state.spirit_for(&p2).unwrap().clone();
    let formation = [13, 16, 31, 34].map(card);
    let discarded = [9, 27, 45, 63].map(card);
    assert_eq!(&state.deck[..8], &revealed);
    let remainder = state
        .deck
        .iter()
        .copied()
        .filter(|card| !discarded.contains(card))
        .collect::<Vec<_>>();
    let started = perform_record_formation(&mut record, &p1, forest, &formation);
    assert!(started.iter().any(
        |event| matches!(event, GameEvent::FormationCommitted { player, formation_id, cards, .. }
        if player == &p1 && formation_id == forest && cards == &formation)
    ));
    assert!(started.contains(&GameEvent::RustedForestCardsRevealed {
        deck: RandomnessDeck::Shared,
        cards: revealed.to_vec()
    }));
    assert!(
        started.contains(&GameEvent::CardsMoved {
            card_moves: discarded
                .iter()
                .copied()
                .map(|card| CardMoveDelta {
                    card,
                    from: CardZone::DeckTop,
                    to: CardZone::Discard
                })
                .collect(),
        })
    );
    assert!(
        !started
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
    );
    let state = record.state();
    assert_eq!(state.hp, hp_before);
    assert_eq!(state.phase, Phase::Action);
    assert_eq!(
        state.pending_resolution,
        Some(PendingResolution::TribulationRustedForestShuffle)
    );
    assert_eq!(
        state.pending_randomness.as_ref().unwrap().operation,
        RandomnessOperation::DeckShuffle {
            deck: RandomnessDeck::Shared
        }
    );
    assert_eq!(
        state.pending_randomness.as_ref().unwrap().current_order,
        remainder
    );
    assert_eq!(state.deck, remainder);
    assert!(state.discard.ends_with(&discarded));
    assert_eq!(record.verify_replay().unwrap(), *state);

    let shuffle_order = remainder.into_iter().rev().collect::<Vec<_>>();
    let resolved = resolve_record_shuffle(&mut record, shuffle_order.clone());
    assert!(resolved.iter().any(|event| matches!(event,
        GameEvent::AttackResolved { attacker, target, formation_id, used_cards, hp_changes, .. }
            if attacker == &p1 && target == &p2 && formation_id == forest && used_cards == &formation
                && matches!(hp_changes.as_slice(), [entry] if entry.role == (HpChangeRole::AttackDamage { target: p2.clone() })
                    && entry.change.team() == &hp_before[1].team && entry.change.effective_delta() == -60)
    )), "{resolved:#?}");
    assert!(
        !resolved
            .iter()
            .any(|event| matches!(event, GameEvent::HpChanged { .. }))
    );
    assert!(resolved.contains(&GameEvent::RustedForestCompleted { player: p1.clone() }));
    assert!(resolved.contains(&GameEvent::FormationCardsDiscarded {
        player: p1.clone(),
        formation_id: forest.to_string(),
        cards: formation.to_vec(),
    }));
    let state = record.state();
    assert_eq!(state.hp[0], hp_before[0]);
    assert_eq!(state.hp[1].hp, hp_before[1].hp - 60);
    assert_eq!(state.spirit_for(&p2), Some(&spirit));
    assert_eq!(state.deck, shuffle_order);
    assert!(
        state
            .discard
            .ends_with(&[discarded.as_slice(), formation.as_slice()].concat())
    );
    assert!(state.active_rusted_forest_resolution.is_none());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(record.replay().unwrap(), *state);
    assert_eq!(record.verify_replay().unwrap(), *state);
}

#[test]
fn rusted_forest_processes_each_personal_deck_and_skips_only_the_protected_owner() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let configured = setup_with_modules(&[PERSONAL_DECK_MODULE_ID]);
    let initial = GameState::from_setup(&configured);
    let metal_three = personal_card(&initial, &p1, Element::Metal, 3);
    let metal_four = personal_card(&initial, &p1, Element::Metal, 4);
    let wood_three = personal_card(&initial, &p1, Element::Wood, 3);
    let wood_four = personal_card(&initial, &p1, Element::Wood, 4);
    let first_strike = personal_card(&initial, &p1, Element::Metal, 1);
    let calculation_card = personal_card(&initial, &p2, Element::Metal, 4);
    let formation = [metal_three, metal_four, wood_three, wood_four];
    let mut opening = vec![
        metal_three,
        metal_four,
        wood_three,
        first_strike,
        personal_card(&initial, &p1, Element::Earth, 1),
        wood_four,
        personal_card(&initial, &p1, Element::Water, 1),
        personal_card(&initial, &p1, Element::Earth, 5),
        calculation_card,
    ];
    for (element, level) in [
        (Element::Wood, 1),
        (Element::Water, 1),
        (Element::Fire, 1),
        (Element::Earth, 1),
        (Element::Wood, 2),
        (Element::Fire, 2),
        (Element::Fire, 5),
    ] {
        opening.push(personal_card(&initial, &p2, element, level));
    }

    for protected in [false, true] {
        let mut record = start_tribulation_record(configured.clone(), &opening);
        perform_record_formation(&mut record, &p1, "metal-strike", &[first_strike]);
        finish_record_turn(&mut record, &p1);
        let second_formation = if protected {
            "tribulation:divine-calculation"
        } else {
            "metal-strike"
        };
        perform_record_formation(&mut record, &p2, second_formation, &[calculation_card]);
        let protection = record
            .state()
            .statuses
            .iter()
            .find(|status| status.kind == "DivineCalculation")
            .map(|status| AttackStatusRemoval {
                status_id: status.id.clone(),
                owner: status.owner.clone(),
            });
        assert_eq!(
            protection.as_ref().map(|status| &status.owner),
            protected.then_some(&StatusOwner::Player(p2.clone()))
        );
        finish_record_turn(&mut record, &p2);

        let hp_before = record.state().hp.clone();
        let mut expected_decks =
            [&p1, &p2].map(|player| record.state().deck_for(player).unwrap().to_vec());
        let mut expected_discards =
            [&p1, &p2].map(|player| record.state().discard_for(player).unwrap().to_vec());
        let mut events =
            perform_record_formation(&mut record, &p1, "tribulation:rusted-forest", &formation);

        // 用同一副開局牌比較兩副牌堆皆處理，以及神算只保護自己的牌堆。
        for (index, player) in [&p1, &p2]
            .into_iter()
            .take(if protected { 1 } else { 2 })
            .enumerate()
        {
            let deck = RandomnessDeck::Player(player.clone());
            let revealed = &expected_decks[index][..8];
            assert!(events.contains(&GameEvent::RustedForestCardsRevealed {
                deck: deck.clone(),
                cards: revealed.to_vec()
            }));
            let discarded = revealed
                .iter()
                .copied()
                .filter(|card| record.state().card_def(*card).unwrap().level.value() >= 3)
                .collect::<Vec<_>>();
            assert!(!discarded.is_empty());
            assert!(
                events.contains(&GameEvent::CardsMoved {
                    card_moves: discarded
                        .iter()
                        .copied()
                        .map(|card| CardMoveDelta {
                            card,
                            from: CardZone::PlayerDeckTop(player.clone()),
                            to: CardZone::PlayerDiscard(player.clone()),
                        })
                        .collect(),
                })
            );
            expected_discards[index].extend(&discarded);
            expected_decks[index].retain(|card| !discarded.contains(card));
            let state = record.state();
            let request = state.pending_randomness.as_ref().unwrap();
            assert_eq!(
                state.pending_resolution,
                Some(PendingResolution::TribulationRustedForestShuffle)
            );
            assert_eq!(
                request.operation,
                RandomnessOperation::DeckShuffle { deck: deck.clone() }
            );
            assert_eq!(request.current_order, expected_decks[index]);
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
            );
            assert_eq!(state.hp, hp_before);
            expected_decks[index].reverse();
            events.extend(resolve_record_shuffle(
                &mut record,
                expected_decks[index].clone(),
            ));
            assert!(events.contains(&GameEvent::RustedForestDeckProcessed { deck }));
        }

        let processed = events
            .iter()
            .filter_map(|event| match event {
                GameEvent::RustedForestCardsRevealed {
                    deck: RandomnessDeck::Player(owner),
                    ..
                } => Some(owner),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            processed,
            if protected { vec![&p1] } else { vec![&p1, &p2] }
        );
        let removed = events
            .iter()
            .filter_map(|event| match event {
                GameEvent::StatusRemoved { status_id, owner } => Some(AttackStatusRemoval {
                    status_id: status_id.clone(),
                    owner: owner.clone(),
                }),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(removed, protection.into_iter().collect::<Vec<_>>());
        assert!(events.contains(&GameEvent::RustedForestCompleted { player: p1.clone() }));
        expected_discards[0].extend(formation);
        for (index, player) in [&p1, &p2].into_iter().enumerate() {
            assert_eq!(
                record.state().deck_for(player),
                Some(expected_decks[index].as_slice())
            );
            assert_eq!(
                record.state().discard_for(player),
                Some(expected_discards[index].as_slice())
            );
        }
        let state = record.state();
        assert_eq!(state.hp[0], hp_before[0]);
        assert_eq!(
            state.hp[1].hp,
            hp_before[1].hp - if protected { 40 } else { 60 }
        );
        assert!(state.deck.is_empty() && state.discard.is_empty());
        assert!(state.active_rusted_forest_resolution.is_none());
        assert!(state.pending_randomness.is_none());
        assert!(
            state
                .statuses
                .iter()
                .all(|status| status.kind != "DivineCalculation")
        );
        assert_eq!(state.phase, Phase::TurnDraw);
        assert_eq!(record.replay().unwrap(), *state);
        assert_eq!(record.verify_replay().unwrap(), *state);
    }
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


fn start_tribulation_record(setup: GameSetup, opening: &[CardInstanceId]) -> GameRecord {
    let remaining = OfficialRules::new().official_deck_order(&setup).unwrap();
    let deck = opening
        .iter()
        .copied()
        .chain(remaining.into_iter().filter(|card| !opening.contains(card)))
        .collect();
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();
    record
}

fn perform_record_formation(
    record: &mut GameRecord,
    player: &PlayerId,
    formation_id: &str,
    cards: &[CardInstanceId],
) -> Vec<GameEvent> {
    record
        .handle(Command::PerformFormation {
            player: player.clone(),
            formation_id: formation_id.to_string(),
            cards: cards.to_vec(),
            declared_targets: Vec::new(),
        })
        .unwrap()
}

fn answer_record_choice(
    record: &mut GameRecord,
    player: &PlayerId,
    answer: ChoiceAnswer,
) -> Vec<GameEvent> {
    let choice_id = record.state().pending_choice.as_ref().unwrap().choice_id;
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id,
            answer,
        })
        .unwrap()
}

fn finish_record_turn(record: &mut GameRecord, player: &PlayerId) {
    record.advance_automatic().unwrap();
    let choice = record.state().pending_choice.as_ref().unwrap();
    assert_eq!(&choice.player, player);
    let PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("回合抽牌應要求選擇棄牌");
    };
    answer_record_choice(
        record,
        player,
        ChoiceAnswer::Cards {
            cards: vec![cards[0]],
        },
    );
    record.advance_automatic().unwrap();
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
