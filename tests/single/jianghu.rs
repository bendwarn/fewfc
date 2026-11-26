use fewfc::application::{advance_automatic, apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent, GameOutcome,
    GameState, GameStatus, HERO_SCHOOLS_MODULE_ID, JIANGHU_MODULE_ID, JianghuState,
    JianghuStateKind, Phase, Player, PlayerId, PlayerProfession, PlayerSpirit, ProfessionId,
    RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID, SpiritKind, StatusDuration, StatusEffect,
    StatusExpiryTiming, StatusOwner, TeamId,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};
mod support;
use support::record as play;

fn state() -> GameState {
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
    let setup = OfficialRules::new()
        .configure_game(
            players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            vec![
                RuleModuleId::new(STAR_MODULE_ID),
                RuleModuleId::new(FIVE_DIRECTIONS_LEGEND_MODULE_ID),
                RuleModuleId::new(HERO_SCHOOLS_MODULE_ID),
                RuleModuleId::new(JIANGHU_MODULE_ID),
            ],
        )
        .unwrap();
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
            vec![
                PlayerId::new("p1"),
                PlayerId::new("p2"),
                PlayerId::new("p3"),
                PlayerId::new("p4"),
            ],
            vec![
                RuleModuleId::new(STAR_MODULE_ID),
                RuleModuleId::new(FIVE_DIRECTIONS_LEGEND_MODULE_ID),
                RuleModuleId::new(HERO_SCHOOLS_MODULE_ID),
                RuleModuleId::new(SPIRIT_MODULE_ID),
                RuleModuleId::new(JIANGHU_MODULE_ID),
            ],
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::TurnEnd;
    state
}

#[test]
fn jianghu_is_default_on_and_contributes_nine_professions() {
    let defaults = OfficialRules::new().default_rule_modules();
    assert!(
        defaults
            .iter()
            .any(|module| module.as_str() == JIANGHU_MODULE_ID)
    );

    let state = state();
    let public = state_for(&state, Viewer::Observer);
    assert!(public.jianghu_states.is_empty());
    assert_eq!(
        public
            .enabled_rule_modules
            .iter()
            .filter(|module| module.as_str() == JIANGHU_MODULE_ID)
            .count(),
        1
    );
}

#[test]
fn jianghu_profession_changes_use_the_published_patterns() {
    let mut game = state();
    let lone = cards(&game, &[(Element::Earth, 3)]);
    set_hand(&mut game, "p1", lone.clone());
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p1"), &lone)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ChangeProfession(candidate)
            if candidate.profession_id == ProfessionId::new("jianghu:lone-wanderer")
    )));

    set_profession(&mut game, "p1", "first-wanderer");
    let swordsman = cards(&game, &[(Element::Metal, 3), (Element::Water, 3)]);
    set_hand(&mut game, "p1", swordsman.clone());
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p1"), &swordsman)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ChangeProfession(candidate)
            if candidate.profession_id == ProfessionId::new("jianghu:swordsman")
    )));
}

#[test]
fn heavenly_yang_aura_lasts_one_round_and_expires_at_its_owners_next_turn_start() {
    let mut game = state();
    set_profession(&mut game, "p1", "jianghu:qi-cultivator");
    let revealed = cards(&game, &[(Element::Wood, 1)]);
    set_hand(&mut game, "p1", revealed.clone());

    let events = handle_command(
        &game,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p1"),
            ability_id: "jianghu:heavenly-yang-aura".to_string(),
            cards: revealed,
            target_card: None,
            declared_element: None,
            declared_level: None,
        },
    )
    .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::StatusAdded { status }
            if status.kind == "JianghuYangAura"
                && status.duration == StatusDuration::UntilTurnStart {
                    player: PlayerId::new("p1"),
                }
    )));
    for event in &events {
        apply_event(&mut game, event);
    }

    game.current_turn_index = 1;
    game.turn_number = 2;
    game.phase = Phase::TurnStart;
    let next_player_events = advance_automatic(&game).unwrap();
    assert!(
        !next_player_events
            .iter()
            .any(|event| matches!(event, GameEvent::StatusExpired { .. }))
    );

    game.current_turn_index = 0;
    game.turn_number = 3;
    let owner_events = advance_automatic(&game).unwrap();
    assert!(matches!(
        owner_events.first(),
        Some(GameEvent::StatusExpired {
            status_id,
            owner: StatusOwner::Player(owner),
            expired_at: StatusExpiryTiming::TurnStart { player },
        }) if status_id.starts_with("jianghu-heavenly-yang-aura-")
            && owner == &PlayerId::new("p1")
            && player == &PlayerId::new("p1")
    ));
}

#[test]
fn thousand_blades_applies_immediately_and_replays_as_typed_state() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record =
        play::start_with_reserves(jianghu_setup(), &[&[1, 9, 45], &[]], &[&[5, 23], &[]]);
    for (profession, cards) in [
        ("first-wanderer", &[1][..]),
        ("jianghu:swordsman", &[9, 45][..]),
    ] {
        play::change(&mut record, &p1, profession, cards);
        advance_jianghu_round(&mut record, &p1, &p2);
    }
    let events = play::perform(
        &mut record,
        &p1,
        "jianghu:thousand-blades-sword-art",
        &[5, 23],
    );
    assert!(events.iter().any(|event| matches!(event, GameEvent::AttackResolved { point_breakdown, .. } if point_breakdown.base_points == 18)));
    assert!(events.iter().any(|event| matches!(event, GameEvent::JianghuStateApplied { state } if state.owner == p1 && state.kind == JianghuStateKind::ThousandBlades)));
    assert!(
        record
            .state()
            .jianghu_states
            .iter()
            .any(|state| state.owner == p1 && state.kind == JianghuStateKind::ThousandBlades)
    );
    assert!(record.state().discard.ends_with(&play::ids(&[5, 23])));
    play::assert_replay(&record);
}

#[test]
fn poison_stacks_ticks_at_affected_players_turn_end_and_uses_poison_mastery() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    for mastery in [false, true] {
        let mut record = play::start_with_reserves(
            jianghu_setup(),
            &[&[1, 19, 37], &[]],
            &[&[2, 55, 73, 9, 27, 63, 3], &[]],
        );
        play::change(&mut record, &p1, "first-wanderer", &[1]);
        advance_jianghu_round(&mut record, &p1, &p2);
        play::change(&mut record, &p1, "jianghu:poisoner", &[19, 37]);
        advance_jianghu_round(&mut record, &p1, &p2);
        if mastery {
            play::change(&mut record, &p1, "jianghu:poison-saint", &[2, 55, 73]);
            advance_jianghu_round(&mut record, &p1, &p2);
        }
        let applied = play::perform(
            &mut record,
            &p1,
            "jianghu:thousand-poison-hand",
            &[9, 27, 63],
        );
        assert!(applied.iter().any(|event| matches!(event, GameEvent::JianghuStateApplied { state }
            if state.owner == p2 && state.kind == JianghuStateKind::Poison && state.remaining_turns == 2)));
        let unrelated_turn = play::finish(&mut record, &p1);
        assert!(
            !unrelated_turn
                .iter()
                .any(|event| matches!(event, GameEvent::JianghuPoisonTicked { .. }))
        );
        play::strike_with_spare(&mut record, &p2, &[]);
        let expected = if mastery { 15 } else { 10 };
        let before = record.state().hp.clone();
        let events = play::finish(&mut record, &p2);
        assert!(
            matches!(events.iter().filter(|event| matches!(event, GameEvent::JianghuPoisonTicked { .. })).collect::<Vec<_>>().as_slice(),
            [GameEvent::JianghuPoisonTicked { owner, damage, remaining_turns: 1, hp_change }]
                if owner == &p2 && *damage == expected && hp_change.old_hp() == before[1].hp
                    && hp_change.new_hp() == before[1].hp - expected)
        );
        assert_eq!(record.state().hp[0], before[0]);
        assert!(
            record
                .state()
                .jianghu_states
                .iter()
                .any(|active| active.owner == p2
                    && active.kind == JianghuStateKind::Poison
                    && active.remaining_turns == 1)
        );
        let reapplied = play::perform(&mut record, &p1, "jianghu:poison-dart", &[3]);
        assert!(reapplied.iter().any(|event| matches!(event, GameEvent::JianghuStateApplied { state }
            if state.owner == p2 && state.kind == JianghuStateKind::Poison && state.remaining_turns == 2)));
        play::assert_replay(&record);
    }
}

#[test]
fn poison_tick_aggregates_teammate_death_spirits_as_following_hp_changed_event() {
    let mut game = team_state();
    for team_hp in &mut game.hp {
        team_hp.hp = 30;
    }
    for team_hp in &mut game.initial_hp {
        team_hp.hp = 30;
    }
    game.spirits = vec![
        PlayerSpirit {
            player: PlayerId::new("p1"),
            spirit: SpiritKind::Death,
            power: 2,
        },
        PlayerSpirit {
            player: PlayerId::new("p3"),
            spirit: SpiritKind::Death,
            power: 2,
        },
    ];
    game.jianghu_states.push(JianghuState {
        owner: PlayerId::new("p1"),
        kind: JianghuStateKind::Poison,
        remaining_turns: 1,
        expires_on_turn: None,
        last_resolved_turn: None,
    });

    let events = advance_automatic(&game).unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::JianghuPoisonTicked {
                owner,
                damage: 10,
                remaining_turns: 0,
                hp_change,
                ..
            },
            GameEvent::HpChanged { change },
            GameEvent::TurnEnded { player },
            GameEvent::TurnStarted {
                player: next_player,
                turn_number: 2,
            },
        ] if owner == &PlayerId::new("p1")
            && hp_change.team() == &TeamId::new("team:a")
            && hp_change.old_hp() == 30
            && hp_change.delta() == -10
            && hp_change.new_hp() == 20
            && change.team() == &TeamId::new("team:b")
            && change.old_hp() == 30
            && change.delta() == -20
            && change.new_hp() == 10
            && player == &PlayerId::new("p1")
            && next_player == &PlayerId::new("p2")
    ));

    for event in &events {
        apply_event(&mut game, event);
    }
    assert_eq!(game.hp[0].hp, 20);
    assert_eq!(game.hp[1].hp, 10);
}

#[test]
fn delayed_fan_damage_emits_aggregated_shared_fate_as_its_next_event() {
    let mut game = team_state();
    for team_hp in &mut game.hp {
        team_hp.hp = 30;
    }
    for team_hp in &mut game.initial_hp {
        team_hp.hp = 30;
    }
    game.spirits = vec![
        PlayerSpirit {
            player: PlayerId::new("p1"),
            spirit: SpiritKind::Death,
            power: 2,
        },
        PlayerSpirit {
            player: PlayerId::new("p3"),
            spirit: SpiritKind::Death,
            power: 2,
        },
    ];
    game.statuses.push(StatusEffect {
        id: "jianghu-fan-beyond-heaven-1".to_string(),
        owner: StatusOwner::Player(PlayerId::new("p1")),
        kind: "JianghuFanBeyondHeaven".to_string(),
        value: Some(20),
        duration: StatusDuration::UntilTurnEndNumber {
            player: PlayerId::new("p1"),
            turn_number: game.turn_number,
        },
    });

    let events = advance_automatic(&game).unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::JianghuDelayedDamageResolved { hp_change, .. },
            GameEvent::HpChanged { change },
            GameEvent::TurnEnded { .. },
            GameEvent::TurnStarted { .. },
        ] if hp_change.team() == &TeamId::new("team:a")
            && hp_change.old_hp() == 30
            && hp_change.delta() == -20
            && hp_change.new_hp() == 10
            && change.team() == &TeamId::new("team:b")
            && change.old_hp() == 30
            && change.delta() == -20
            && change.new_hp() == 10
    ));
}

#[test]
fn lethal_poison_tick_blooms_before_turn_end_game_conclusion() {
    let mut game = team_state();
    for team_hp in &mut game.hp {
        if team_hp.team == TeamId::new("team:a") {
            team_hp.hp = 10;
        }
    }
    for team_hp in &mut game.initial_hp {
        if team_hp.team == TeamId::new("team:a") {
            team_hp.hp = 50;
        }
    }
    game.spirits = vec![PlayerSpirit {
        player: PlayerId::new("p1"),
        spirit: SpiritKind::Wood,
        power: 6,
    }];
    game.jianghu_states.push(JianghuState {
        owner: PlayerId::new("p1"),
        kind: JianghuStateKind::Poison,
        remaining_turns: 1,
        expires_on_turn: None,
        last_resolved_turn: None,
    });

    let events = advance_automatic(&game).unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::JianghuPoisonTicked { hp_change, .. },
            GameEvent::AutomaticBloomsResolved { resolutions },
            GameEvent::TurnEnded { .. },
            GameEvent::TurnStarted { .. },
        ] if hp_change.old_hp() == 10
            && hp_change.new_hp() == 0
            && resolutions.len() == 1
            && resolutions[0].team == TeamId::new("team:a")
            && resolutions[0].hp_change.old_hp() == 0
            && resolutions[0].hp_change.new_hp() == 40
    ));
}

#[test]
fn king_yama_decree_wins_directly_through_watch_fire_protection() {
    use fewfc::domain::{
        PERSONAL_DECK_MODULE_ID, POUCH_MODULE_ID, SecretStrategy, SecretStrategyDecision,
    };
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = play::setup(
        2,
        &[
            STAR_MODULE_ID,
            FIVE_DIRECTIONS_LEGEND_MODULE_ID,
            HERO_SCHOOLS_MODULE_ID,
            JIANGHU_MODULE_ID,
            PERSONAL_DECK_MODULE_ID,
            SPIRIT_MODULE_ID,
            POUCH_MODULE_ID,
        ],
    );
    setup.hp[1].hp = 40;
    setup.base_draw = 5;
    setup.hand_limit = 10;
    let prefix = play::personal_prefix(
        &setup,
        &[
            &[
                (Element::Fire, 1),
                (Element::Metal, 1),
                (Element::Wood, 1),
                (Element::Water, 1),
                (Element::Earth, 1),
                (Element::Fire, 2),
                (Element::Metal, 1),
                (Element::Fire, 1),
                (Element::Earth, 1),
                (Element::Water, 1),
                (Element::Wood, 1),
            ],
            &[(Element::Fire, 1)],
        ],
    );
    let mut record = play::start_pouch(setup, &[prefix[0], prefix[11]], &prefix[1..11]);
    for (profession, indices) in [
        ("first-wanderer", &[1][..]),
        ("jianghu:poisoner", &[2, 3][..]),
        ("jianghu:poison-saint", &[6, 7, 8][..]),
    ] {
        let cards = indices
            .iter()
            .map(|index| prefix[*index].as_u64())
            .collect::<Vec<_>>();
        play::change(&mut record, &p1, profession, &cards);
        play::finish(&mut record, &p1);
        if profession != "jianghu:poison-saint" {
            play::strike_with_spare(&mut record, &p2, &[]);
            play::finish(&mut record, &p2);
        }
    }
    for watch in [false, true] {
        let mut record = record.clone();
        if watch {
            let events = record
                .handle(Command::TriggerSecretStrategy {
                    player: p2.clone(),
                    decision: SecretStrategyDecision::NoInput {
                        source_card: prefix[11],
                        strategy: SecretStrategy::WatchTheFire,
                    },
                })
                .unwrap();
            assert!(events.iter().any(|event| matches!(event, GameEvent::StatusAdded { status }
                if status.owner == StatusOwner::Player(p1.clone()) && status.kind == "PouchWatchFire")));
        }
        play::strike_with_spare(&mut record, &p2, &[]);
        play::finish(&mut record, &p2);
        assert_eq!(record.state().hp[1].hp, 40);
        assert_eq!(
            record
                .state()
                .statuses
                .iter()
                .any(|status| status.owner == StatusOwner::Player(p1.clone())
                    && status.kind == "PouchWatchFire"),
            watch
        );
        let events = play::perform(
            &mut record,
            &p1,
            "jianghu:king-yama-decree",
            &[prefix[4].as_u64()],
        );
        assert!(events.contains(&GameEvent::KingYamaDecreeVictoryAchieved {
            player: p1.clone(),
            team: TeamId::new("team:a")
        }));
        assert!(
            matches!(&record.state().status, GameStatus::Finished { conclusion } if conclusion.outcome == GameOutcome::Winner(TeamId::new("team:a")))
        );
        assert_eq!(record.state().hp[1].hp, 40);
        play::assert_replay(&record);
    }
}

#[test]
fn water_dotting_fan_only_reads_the_immediately_previous_turns_formation() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    for fresh in [false, true] {
        let mut configured = jianghu_setup();
        configured.hp[0].hp = 800;
        let mut record = play::start_with_reserves(
            configured,
            &[&[1, 9, 10, 2], &[11, 27, 45, 63, 81]],
            &[&[], &[12, 28, 46, 64, 82, 3]],
        );
        play::change(&mut record, &p1, "first-wanderer", &[1]);
        play::finish(&mut record, &p1);
        play::perform(
            &mut record,
            &p2,
            "five-streams-unite",
            &[11, 27, 45, 63, 81],
        );
        play::finish(&mut record, &p2);
        play::change(&mut record, &p1, "jianghu:ink-seeker", &[9, 10]);
        play::finish(&mut record, &p1);
        if fresh {
            play::perform(
                &mut record,
                &p2,
                "five-streams-unite",
                &[12, 28, 46, 64, 82],
            );
        } else {
            play::change(&mut record, &p2, "first-wanderer", &[3]);
        }
        play::finish(&mut record, &p2);
        let last = &record.state().last_formation_by_player[&p2];
        assert_eq!(last.resolved_turn, if fresh { 4 } else { 2 });
        assert_eq!(last.used_cards.len(), 5);
        let events = play::perform(&mut record, &p1, "jianghu:water-dotting-fan", &[2]);
        let effects = events
            .iter()
            .find_map(|event| match event {
                GameEvent::AttackResolved {
                    target,
                    elemental_context_update: Some(effects),
                    ..
                } if target == &p2 => Some(effects),
                _ => None,
            })
            .unwrap();
        let statuses = effects
            .statuses_added
            .iter()
            .map(|status| (&status.owner, status.kind.as_str()))
            .collect::<Vec<_>>();
        let owner = StatusOwner::Player(p2.clone());
        assert_eq!(
            statuses,
            if fresh {
                vec![(&owner, "CannotAct"), (&owner, "CannotDraw")]
            } else {
                vec![]
            }
        );
        assert!(record.state().discard.ends_with(&play::ids(&[2])));
        play::assert_replay(&record);
    }
}

#[test]
fn dancing_yang_uses_the_attack_attribute_and_formation_card_count() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    for (activated, eligible_attack) in [(false, true), (true, true), (true, false)] {
        let mut record = play::start_with_reserves(
            jianghu_setup(),
            &[&[1, 27, 63], &[]],
            &[&[34, 70, 28, 31, 35, 23, 29, 37, 20], &[]],
        );
        for (profession, cards) in [
            ("first-wanderer", &[1][..]),
            ("jianghu:qi-cultivator", &[27, 63][..]),
            ("jianghu:qi-grandmaster", &[34, 70][..]),
        ] {
            play::change(&mut record, &p1, profession, cards);
            advance_jianghu_round(&mut record, &p1, &p2);
        }
        play::perform(&mut record, &p1, "triple-wood", &[28, 31, 35]);
        advance_jianghu_round(&mut record, &p1, &p2);
        if activated {
            record
                .handle(Command::ActivateProfessionAbility {
                    player: p1.clone(),
                    ability_id: "jianghu:dancing-yang-art".to_string(),
                    cards: vec![],
                    target_card: None,
                    declared_element: None,
                    declared_level: None,
                })
                .unwrap();
        }
        let (formation, cards) = if eligible_attack {
            ("suixing-heaven-pillar", &[23, 29, 37][..])
        } else {
            ("wood-strike", &[20][..])
        };
        let events = play::perform(&mut record, &p1, formation, cards);
        let changes = events
            .iter()
            .filter_map(|event| match event {
                GameEvent::AttackResolved {
                    elemental_context_update: Some(effects),
                    ..
                } => Some(&effects.turn_draw_bonus_changes),
                _ => None,
            })
            .flatten()
            .map(|change| (change.old_value, change.new_value))
            .collect::<Vec<_>>();
        assert_eq!(
            changes,
            match (eligible_attack, activated) {
                (true, true) => vec![(0, 1), (1, 2)],
                (true, false) => vec![(0, 1)],
                (false, _) => vec![],
            }
        );
        assert!(record.state().discard.ends_with(&play::ids(cards)));
        play::assert_replay(&record);
    }
}

fn jianghu_setup() -> fewfc::domain::GameSetup {
    play::setup(
        2,
        &[
            STAR_MODULE_ID,
            FIVE_DIRECTIONS_LEGEND_MODULE_ID,
            HERO_SCHOOLS_MODULE_ID,
            JIANGHU_MODULE_ID,
        ],
    )
}

fn advance_jianghu_round(
    record: &mut fewfc::application::GameRecord,
    p1: &PlayerId,
    p2: &PlayerId,
) {
    play::finish(record, p1);
    play::strike_with_spare(record, p2, &[]);
    play::finish(record, p2);
}
