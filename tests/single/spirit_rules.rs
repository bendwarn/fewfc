use fewfc::application::{GameRecord, apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError, GameEvent,
    GameOutcome, GameSetup, GameState, GameStatus, HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID,
    PendingChoiceKind, Phase, Player, PlayerId, PlayerSpirit, RuleModuleId, SPIRIT_MODULE_ID,
    STAR_MODULE_ID, SpiritKind, SpiritSkill, StarKind, StatusDuration, StatusEffect, StatusOwner,
    TargetDecl, TeamId, ValidationError,
};
use fewfc::public_view::{PublicGameEvent, Viewer, event_for, state_for};
use fewfc::rules::OfficialRules;
mod support;
use support::record as play;

fn players() -> (Vec<Player>, Vec<PlayerId>) {
    (
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
        vec![PlayerId::new("p1"), PlayerId::new("p2")],
    )
}

fn spirit_modules() -> Vec<RuleModuleId> {
    [
        STAR_MODULE_ID,
        FIVE_DIRECTIONS_LEGEND_MODULE_ID,
        HERO_SCHOOLS_MODULE_ID,
        SPIRIT_MODULE_ID,
    ]
    .into_iter()
    .map(RuleModuleId::new)
    .collect()
}

fn spirit_state() -> GameState {
    let (players, turn_order) = players();
    let setup = OfficialRules::new()
        .configure_game(players, turn_order, spirit_modules())
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    state
}

fn team_spirit_state() -> GameState {
    let setup = OfficialRules::new()
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
            spirit_modules(),
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

fn apply_all(state: &mut GameState, events: &[GameEvent]) {
    for event in events {
        apply_event(state, event);
    }
}

fn use_skill(
    state: &GameState,
    skill: SpiritSkill,
    selected_card: Option<CardInstanceId>,
    declared_level: Option<u32>,
) -> Result<Vec<GameEvent>, GameError> {
    handle_command(
        state,
        Command::UseSpiritSkill {
            player: PlayerId::new("p1"),
            skill,
            selected_card,
            declared_level,
        },
    )
}

#[test]
fn spirit_requires_every_advanced_rule_module_and_is_default_on() {
    let rules = OfficialRules::new();
    let defaults = rules.default_rule_modules();
    assert!(
        defaults
            .iter()
            .any(|module| module.as_str() == SPIRIT_MODULE_ID)
    );

    let (players, turn_order) = players();
    let error = rules
        .configure_game(
            players,
            turn_order,
            vec![
                RuleModuleId::new(STAR_MODULE_ID),
                RuleModuleId::new(SPIRIT_MODULE_ID),
            ],
        )
        .unwrap_err();
    assert_eq!(
        error,
        GameError::Validation(ValidationError::MissingRuleModuleDependencies {
            module: RuleModuleId::new(SPIRIT_MODULE_ID),
            required: vec![
                RuleModuleId::new(FIVE_DIRECTIONS_LEGEND_MODULE_ID),
                RuleModuleId::new(HERO_SCHOOLS_MODULE_ID),
            ],
        })
    );
}

#[test]
fn summoning_formations_replace_the_players_spirit_at_two_power() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = play::start(
        shared_spirit_setup(2),
        &[1, 5, 19, 23, 37, 38, 39, 40, 41, 2, 55, 73],
    );
    let metal = play::perform(&mut record, &p1, "metal-spirit-summoning", &[1, 5]);
    assert!(metal.contains(&GameEvent::SpiritSummoned {
        player: p1.clone(),
        previous: None,
        spirit: SpiritKind::Metal
    }));
    assert_eq!(record.state().spirit_for(&p1).unwrap().power, 2);
    play::finish_discarding(&mut record, &p1, Some(CardInstanceId::new(2)));
    assert_eq!(record.state().spirit_for(&p1).unwrap().power, 3);
    play::perform(&mut record, &p2, "water-strike", &[37]);
    play::finish(&mut record, &p2);

    let wood = play::perform(&mut record, &p1, "wood-spirit-summoning", &[19, 23]);
    assert!(wood.contains(&GameEvent::SpiritSummoned {
        player: p1.clone(),
        previous: Some(SpiritKind::Metal),
        spirit: SpiritKind::Wood
    }));
    assert_eq!(
        record.state().spirit_for(&p1),
        Some(&PlayerSpirit {
            player: p1.clone(),
            power: 2,
            spirit: SpiritKind::Wood
        })
    );
    assert!(record.state().discard.ends_with(&play::ids(&[19, 23])));
    play::assert_replay(&record);
}

#[test]
fn matching_turn_discard_charges_only_the_owners_spirit_to_six() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let record = charged_spirit_record(Element::Metal, 6, &[], &[]);
    let events = record.recorded_events();
    let charges = events
        .iter()
        .filter_map(|recorded| match &recorded.event {
            GameEvent::SpiritPowerChanged {
                player,
                old_power,
                new_power,
                reason: fewfc::domain::SpiritPowerChangeReason::TurnDrawDiscard { .. },
                ..
            } if player == &p1 => Some((*old_power, *new_power)),
            _ => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(charges, [(2, 3), (3, 4), (4, 5), (5, 6)]);
    let p2_before = record.state().spirit_for(&p2).cloned();
    assert!(p2_before.is_none());

    for matching in [false, true] {
        let mut checked = record.clone();
        play::strike_with_spare(&mut checked, &p1, &[]);
        checked.advance_automatic().unwrap();
        let kind = &checked.state().pending_choice.as_ref().unwrap().kind;
        let PendingChoiceKind::Card { cards, .. } = kind else {
            unreachable!()
        };
        if let Some(discard) = cards
            .iter()
            .copied()
            .find(|card| (checked.state().card_element(*card) == Some(Element::Metal)) == matching)
        {
            let events = play::finish_discarding(&mut checked, &p1, Some(discard));
            assert!(events.iter().any(
                |event| matches!(event, GameEvent::TurnDrawResolved { player, .. } if player == &p1)
            ));
            assert!(
                !events
                    .iter()
                    .any(|event| matches!(event, GameEvent::SpiritPowerChanged { .. }))
            );
            assert_eq!(checked.state().spirit_for(&p1).unwrap().power, 6);
            assert_eq!(checked.state().spirit_for(&p2), p2_before.as_ref());
            play::assert_replay(&checked);
        } else {
            panic!("固定牌序應包含兩種棄牌選擇");
        }
    }
    play::assert_replay(&record);
}

#[test]
fn public_views_expose_spirit_kind_and_power_when_enabled() {
    let mut state = spirit_state();
    apply_event(
        &mut state,
        &GameEvent::SpiritSummoned {
            player: PlayerId::new("p1"),
            previous: None,
            spirit: SpiritKind::Fire,
        },
    );

    let public = state_for(&state, Viewer::Observer);
    assert_eq!(public.spirits, state.spirits);
}

#[test]
fn spirit_summoning_replays_and_supports_same_kind_team_ownership() {
    let [p1, p2, p3] = ["p1", "p2", "p3"].map(PlayerId::new);
    let mut record = play::start_with_reserves(
        shared_spirit_setup(4),
        &[&[55, 59], &[], &[56, 60], &[]],
        &[&[], &[], &[], &[]],
    );
    play::perform(&mut record, &p1, "fire-spirit-summoning", &[55, 59]);
    play::finish(&mut record, &p1);
    play::strike_with_spare(&mut record, &p2, &[]);
    play::finish(&mut record, &p2);
    let second = play::perform(&mut record, &p3, "fire-spirit-summoning", &[56, 60]);
    assert!(second.contains(&GameEvent::SpiritSummoned {
        player: p3.clone(),
        previous: None,
        spirit: SpiritKind::Fire
    }));
    assert_eq!(
        record
            .state()
            .spirits
            .iter()
            .map(|owned| (&owned.player, owned.spirit))
            .collect::<Vec<_>>(),
        [(&p1, SpiritKind::Fire), (&p3, SpiritKind::Fire)]
    );
    play::assert_replay(&record);
}

#[test]
fn metal_and_wood_skills_change_hp_consume_power_and_allow_ineffective_recovery() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut blade = charged_spirit_record(Element::Metal, 6, &[], &[]);
    let hp = blade.state().hp.clone();
    let events = blade
        .handle(skill_command(&p1, SpiritSkill::FlyingBlade, None, None))
        .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::HpChanged { change }
        if change.team() == &hp[1].team && change.delta() == -10))
    );
    assert_eq!(blade.state().spirit_for(&p1).unwrap().power, 4);
    assert_eq!(blade.state().hp[1].hp, hp[1].hp - 10);
    let after = blade.state().clone();
    assert!(matches!(
        blade.handle(skill_command(&p1, SpiritSkill::FlyingBlade, None, None)),
        Err(GameError::Validation(
            ValidationError::SpiritSkillAlreadyUsed { .. }
        ))
    ));
    assert_eq!(*blade.state(), after);
    play::assert_replay(&blade);

    let mut rain = charged_spirit_record(Element::Metal, 6, &[], &[]);
    let events = rain
        .handle(skill_command(&p1, SpiritSkill::SwordRain, None, None))
        .unwrap();
    assert!(matches!(
        events.last(),
        Some(GameEvent::SpiritBroken {
            spirit: SpiritKind::Metal,
            ..
        })
    ));
    assert!(rain.state().spirit_for(&p1).is_none());
    play::assert_replay(&rain);

    let mut wood =
        play::start_with_reserves(shared_spirit_setup(2), &[&[19, 23], &[1]], &[&[], &[]]);
    play::perform(&mut wood, &p1, "wood-spirit-summoning", &[19, 23]);
    play::finish(&mut wood, &p1);
    play::change(&mut wood, &p2, "first-wanderer", &[1]);
    play::finish(&mut wood, &p2);
    assert_eq!(wood.state().spirit_for(&p1).unwrap().power, 2);
    let events = wood
        .handle(skill_command(&p1, SpiritSkill::Fragrance, None, None))
        .unwrap();
    assert!(events.iter().any(
        |event| matches!(event, GameEvent::HpChanged { change } if change.effective_delta() == 0)
    ));
    assert!(wood.state().spirit_for(&p1).is_none());
    play::assert_replay(&wood);
}

#[test]
fn water_skills_move_cards_and_add_turn_draw_bonus() {
    let p1 = PlayerId::new("p1");
    let selected = CardInstanceId::new(55);
    let mut flow = charged_spirit_record(Element::Water, 3, &[55], &[]);
    let events = flow
        .handle(skill_command(&p1, SpiritSkill::Flow, Some(selected), None))
        .unwrap();
    assert!(events.contains(&GameEvent::CardsMoved {
        card_moves: vec![fewfc::domain::CardMoveDelta {
            card: selected,
            from: fewfc::domain::CardZone::Hand(p1.clone()),
            to: fewfc::domain::CardZone::Discard,
        }]
    }));
    assert!(events.contains(&GameEvent::TurnDrawBonusChanged {
        player: p1.clone(),
        old_value: 0,
        delta: 1,
        new_value: 1
    }));
    assert!(!flow.state().hand(&p1).unwrap().contains(&selected));
    assert_eq!(flow.state().discard.last(), Some(&selected));
    assert_eq!(flow.state().spirit_for(&p1).unwrap().power, 2);
    play::assert_replay(&flow);

    let mut vastness = charged_spirit_record(Element::Water, 3, &[], &[]);
    let events = vastness
        .handle(skill_command(&p1, SpiritSkill::Vastness, None, None))
        .unwrap();
    assert!(events.iter().any(|event| matches!(event, GameEvent::SpiritBroken { player, spirit: SpiritKind::Water, .. } if player == &p1)));
    assert_eq!(
        vastness.state().turn_draw_bonus_by_player.get(&p1),
        Some(&1)
    );
    assert!(vastness.state().spirit_for(&p1).is_none());
    play::assert_replay(&vastness);
}

#[test]
fn flow_moves_personal_cards_to_their_origin_discard() {
    use fewfc::domain::{CardMoveDelta, CardZone};
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let configured = play::setup(
        2,
        &[
            STAR_MODULE_ID,
            FIVE_DIRECTIONS_LEGEND_MODULE_ID,
            HERO_SCHOOLS_MODULE_ID,
            SPIRIT_MODULE_ID,
            PERSONAL_DECK_MODULE_ID,
        ],
    );
    let prefix = play::personal_prefix(
        &configured,
        &[
            &[(Element::Water, 1), (Element::Water, 2), (Element::Fire, 1)],
            &[],
        ],
    );
    let mut record = play::start_with_cards(configured, &prefix);
    play::perform(
        &mut record,
        &p1,
        "water-spirit-summoning",
        &[prefix[0].as_u64(), prefix[1].as_u64()],
    );
    play::finish(&mut record, &p1);
    play::strike_with_spare(&mut record, &p2, &[]);
    play::finish(&mut record, &p2);
    let selected = prefix[2];
    let other_discard = record.state().discard_for(&p2).unwrap().to_vec();
    let events = record
        .handle(skill_command(&p1, SpiritSkill::Flow, Some(selected), None))
        .unwrap();
    assert!(events.contains(&GameEvent::CardsMoved {
        card_moves: vec![CardMoveDelta {
            card: selected,
            from: CardZone::Hand(p1.clone()),
            to: CardZone::PlayerDiscard(p1.clone()),
        }]
    }));
    assert_eq!(
        record.state().discard_for(&p1).unwrap().last(),
        Some(&selected)
    );
    assert_eq!(
        record.state().discard_for(&p2),
        Some(other_discard.as_slice())
    );
    assert!(record.state().discard.is_empty());
    play::assert_replay(&record);
}

#[test]
fn fire_interpretation_changes_points_star_qualification_and_redacts_the_card() {
    let p1 = PlayerId::new("p1");
    let selected = CardInstanceId::new(1);
    let cards = [1, 9, 16];
    let original = charged_spirit_record(Element::Fire, 3, &cards, &[]);
    for splendor in [false, true] {
        let mut record = original.clone();
        if splendor {
            let events = record
                .handle(skill_command(
                    &p1,
                    SpiritSkill::Splendor,
                    Some(selected),
                    Some(5),
                ))
                .unwrap();
            let canonical = events
                .iter()
                .find(|event| matches!(event, GameEvent::SpiritSkillUsed { .. }))
                .unwrap();
            assert!(matches!(
                event_for(canonical, Viewer::Observer),
                PublicGameEvent::SpiritSkillUsed {
                    selected_card: None,
                    declared_level: Some(5),
                    ..
                }
            ));
            assert!(matches!(event_for(canonical, Viewer::Player(p1.clone())),
                PublicGameEvent::SpiritSkillUsed { selected_card: Some(card), declared_level: Some(5), .. } if card == selected));
        }
        let attack = play::perform(&mut record, &p1, "triple-metal", &cards);
        assert!(attack.iter().any(
            |event| matches!(event, GameEvent::AttackResolved { point_breakdown, .. }
            if point_breakdown.base_points == if splendor { 39 } else { 27 })
        ));
        assert_eq!(
            attack.iter().any(|event| matches!(
                event,
                GameEvent::StarSummoned {
                    star: StarKind::Metal,
                    ..
                }
            )),
            splendor
        );
        assert!(record.state().discard.ends_with(&play::ids(&cards)));
        play::assert_replay(&record);
    }
}

#[test]
fn sacred_art_composes_with_fire_level_but_rejects_star_substitution() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = play::start_with_reserves(
        shared_spirit_setup(2),
        &[&[55, 59], &[]],
        &[&[1, 5, 2, 16, 13, 31, 49, 45, 50, 52, 19, 6, 63, 70], &[]],
    );
    play::perform(&mut record, &p1, "fire-spirit-summoning", &[55, 59]);
    play::finish_discarding(&mut record, &p1, Some(CardInstanceId::new(70)));
    play::strike_with_spare(&mut record, &p2, &[]);
    play::finish(&mut record, &p2);
    for (id, cards) in [
        ("warrior", &[1, 5][..]),
        ("war-god", &[2, 16][..]),
        ("saint", &[13, 31, 49][..]),
    ] {
        play::change(&mut record, &p1, id, cards);
        play::finish(&mut record, &p1);
        play::strike_with_spare(&mut record, &p2, &[]);
        play::finish(&mut record, &p2);
    }
    play::perform(&mut record, &p1, "triple-water", &[45, 50, 52]);
    play::finish(&mut record, &p1);
    play::strike_with_spare(&mut record, &p2, &[]);
    play::finish(&mut record, &p2);

    let wood = CardInstanceId::new(19);
    let cards = play::ids(&[19, 6, 63]);
    let multiplicity = TargetDecl::CardMultiplicity {
        card: wood,
        slots: 2,
    };
    let candidates = |record: &GameRecord| {
        record.playable_actions(&p1, &cards).unwrap().iter().any(|action| matches!(action,
        fewfc::rules::PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "barrier" && candidate.declared_targets == [multiplicity.clone()]))
    };
    assert!(!candidates(&record));
    record
        .handle(skill_command(
            &p1,
            SpiritSkill::Splendor,
            Some(wood),
            Some(4),
        ))
        .unwrap();
    assert!(candidates(&record));
    let before = record.state().clone();
    assert!(matches!(
        record.handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: cards.clone(),
            declared_targets: vec![
                multiplicity.clone(),
                TargetDecl::Card(CardInstanceId::new(6))
            ],
        }),
        Err(GameError::Validation(
            ValidationError::FormationPatternMismatch { .. }
        ))
    ));
    assert_eq!(*record.state(), before);
    record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: cards.clone(),
            declared_targets: vec![multiplicity],
        })
        .unwrap();
    assert!(record.state().shield(&p1).unwrap() > 0);
    assert!(record.state().discard.ends_with(&cards));
    play::assert_replay(&record);
}

#[test]
fn spirit_skills_remain_legal_under_cannot_act_and_replacement_resets_the_allowance() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = play::start_with_reserves(
        shared_spirit_setup(2),
        &[&[1, 5], &[2, 6, 55, 37]],
        &[&[9], &[]],
    );
    play::perform(&mut record, &p1, "metal-spirit-summoning", &[1, 5]);
    play::finish_discarding(&mut record, &p1, Some(CardInstanceId::new(9)));
    play::perform(&mut record, &p2, "radiance", &[2, 6, 55, 37]);
    play::finish(&mut record, &p2);
    assert!(record.state().statuses.iter().any(|status| status.owner
        == StatusOwner::Player(p1.clone())
        && status.kind == "CannotAct"));
    let actions = record.playable_actions(&p1, &[]).unwrap();
    assert!(actions.iter().any(|action| matches!(action, fewfc::rules::PlayableAction::UseSpiritSkill(candidate) if candidate.skill == SpiritSkill::FlyingBlade)));
    let use_blade = skill_command(&p1, SpiritSkill::FlyingBlade, None, None);
    record.handle(use_blade.clone()).unwrap();
    assert_eq!(
        record.state().spirit_skill_use_turns.get(&p1),
        Some(&record.state().turn_number)
    );
    play::assert_replay(&record);

    // 投影契約直接重播替換事件，以隔離同一回合的技能次數。
    let mut projected = record.state().clone();
    apply_event(
        &mut projected,
        &GameEvent::SpiritSummoned {
            player: p1.clone(),
            previous: Some(SpiritKind::Metal),
            spirit: SpiritKind::Metal,
        },
    );
    assert!(!projected.spirit_skill_use_turns.contains_key(&p1));
    assert!(handle_command(&projected, use_blade).is_ok());
}

#[test]
fn earth_skills_replace_shields_and_stone_shield_prevents_sacred_beast_damage_only() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let barrier = [31, 35, 13, 59];
    let mut wall = charged_spirit_record(Element::Earth, 6, &barrier, &[1]);
    play::perform(&mut wall, &p1, "barrier", &barrier);
    play::finish(&mut wall, &p1);
    play::change(&mut wall, &p2, "first-wanderer", &[1]);
    play::finish(&mut wall, &p2);
    assert_eq!(wall.state().shield(&p1), Some(60));
    let events = wall
        .handle(skill_command(&p1, SpiritSkill::RockWall, None, None))
        .unwrap();
    assert!(events.contains(&GameEvent::ShieldChanged {
        player: p1.clone(),
        old_value: 60,
        delta: -20,
        new_value: 40
    }));
    assert_eq!(wall.state().shield(&p1), Some(40));
    assert!(wall.state().spirit_for(&p1).is_none());
    play::assert_replay(&wall);

    let beast = [2, 3, 4, 5, 6];
    let mut stone = play::start_with_reserves(
        shared_spirit_setup(2),
        &[&[73, 77], &[2, 3, 4, 5, 55]],
        &[&[], &[6]],
    );
    play::perform(&mut stone, &p1, "earth-spirit-summoning", &[73, 77]);
    play::finish(&mut stone, &p1);
    play::perform(&mut stone, &p2, "fire-strike", &[55]);
    play::finish(&mut stone, &p2);
    for protected in [false, true] {
        let mut record = stone.clone();
        if protected {
            record
                .handle(skill_command(&p1, SpiritSkill::StoneShield, None, None))
                .unwrap();
        }
        play::strike_with_spare(&mut record, &p1, &[]);
        play::finish(&mut record, &p1);
        let old_hp = record.state().hp[0].hp;
        let attack = play::perform(&mut record, &p2, "west-white-tiger", &beast);
        assert!(
            attack
                .iter()
                .any(|event| matches!(event, GameEvent::AttackResolved {
            hp_changes, elemental_context_update: Some(effects), ..
        } if hp_changes.is_empty() == protected
            && effects.environment_transfers.iter().any(|transfer| transfer.to == Element::Metal)))
        );
        assert_eq!(record.state().hp[0].hp == old_hp, protected);
        assert_eq!(record.state().environment, Some(Element::Metal));
        play::assert_replay(&record);
    }
}

#[test]
fn all_eligible_teammate_wood_spirits_bloom_atomically_and_cap_recovery() {
    let [p1, p2, p3, p4] = ["p1", "p2", "p3", "p4"].map(PlayerId::new);
    let elements = [Element::Metal, Element::Wood, Element::Fire, Element::Wood];
    let team = TeamId::new("team:b");
    let shock = [55, 59, 37, 81];
    let mut record = four_summoned_spirits(elements, [0, 4, 0, 4], &[], [250, 50], &shock);
    let before = record.state().hp.clone();
    assert!((1..=10).contains(&before[1].hp), "{before:?}");
    let cap = record.state().initial_hp(&team).unwrap();
    assert_eq!(cap, 50);
    let events = record
        .handle(skill_command(&p1, SpiritSkill::FlyingBlade, None, None))
        .unwrap();
    assert!(
        matches!(events.last(), Some(GameEvent::AutomaticBloomsResolved { resolutions })
        if resolutions.len() == 1 && resolutions[0].team == team && resolutions[0].spirit_changes.len() == 2
            && resolutions[0].hp_change.old_hp() == 0 && resolutions[0].hp_change.delta() == 80
            && resolutions[0].hp_change.new_hp() == cap)
    );
    assert_eq!(record.state().hp[1].hp, cap);
    assert_eq!(record.state().hp[0], before[0]);
    assert!(matches!(record.state().status, GameStatus::InProgress));
    assert!(record.state().spirit_for(&p1).is_none());
    assert!(record.state().spirit_for(&p2).is_none());
    assert!(record.state().spirit_for(&p4).is_none());
    assert_eq!(record.state().spirit_for(&p3).unwrap().power, 2);
    play::assert_replay(&record);
}

#[test]
fn lethal_attack_threads_its_hp_ledger_into_wood_bloom_and_replays_the_final_state() {
    let mut state = team_spirit_state();
    state.spirits = vec![PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Wood,
        power: 6,
    }];
    state
        .hp
        .iter_mut()
        .find(|owned| owned.team == TeamId::new("team:b"))
        .unwrap()
        .hp = 1;
    state
        .initial_hp
        .iter_mut()
        .find(|owned| owned.team == TeamId::new("team:b"))
        .unwrap()
        .hp = 50;
    let strike = vec![card(&state, Element::Metal, 1)];
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = strike.clone();

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: strike,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_changes, .. }
            if hp_changes.iter().any(|resolved|
                resolved.change.old_hp() == 1 && resolved.change.new_hp() == 0)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AutomaticBloomsResolved { resolutions }
            if resolutions.len() == 1
                && resolutions[0].hp_change.old_hp() == 0
                && resolutions[0].hp_change.new_hp() == 40
    )));

    let mut replayed = state.clone();
    apply_all(&mut replayed, &events);
    assert_eq!(
        replayed
            .hp
            .iter()
            .find(|owned| owned.team == TeamId::new("team:b"))
            .unwrap()
            .hp,
        40
    );
}

#[test]
fn temporary_ability_loss_prevents_automatic_bloom() {
    let mut state = team_spirit_state();
    state.spirits = vec![
        PlayerSpirit {
            player: PlayerId::new("p1"),
            spirit: SpiritKind::Metal,
            power: 2,
        },
        PlayerSpirit {
            player: PlayerId::new("p2"),
            spirit: SpiritKind::Wood,
            power: 6,
        },
    ];
    state
        .hp
        .iter_mut()
        .find(|owned| owned.team == TeamId::new("team:b"))
        .unwrap()
        .hp = 10;
    state.statuses.push(StatusEffect {
        id: "lure-spirit-p2".to_string(),
        owner: StatusOwner::Player(PlayerId::new("p2")),
        kind: "PouchLureSpirit".to_string(),
        value: None,
        duration: StatusDuration::UntilTurnEnd {
            player: PlayerId::new("p2"),
        },
    });

    let events = use_skill(&state, SpiritSkill::FlyingBlade, None, None).unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::AutomaticBloomsResolved { .. }))
    );
}

#[test]
fn five_star_direct_victory_takes_priority_over_automatic_bloom() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = shared_spirit_setup(2);
    setup.hp[0].hp = 5;
    setup.hp[1].hp = 170;
    setup.base_draw = 4;
    setup.hand_limit = 9;
    let mut record = play::start(
        setup,
        &[
            19, 23, 1, 2, 3, 55, 56, 59, 60, 20, 27, 31, 34, 4, 37, 38, 41, 42, 44, 21, 45, 49, 52,
            5, 57, 58, 71, 22, 63, 67, 70, 6, 72, 73, 74, 75, 24, 81, 85, 88, 7, 76, 77, 78, 8, 9,
            13, 16, 79, 80, 82,
        ],
    );
    let steps = [
        ("wood-spirit-summoning", &[19, 23][..], 20),
        ("triple-wood", &[27, 31, 34][..], 21),
        ("triple-water", &[45, 49, 52][..], 22),
        ("triple-fire", &[63, 67, 70][..], 24),
        ("triple-earth", &[81, 85, 88][..], 8),
    ];
    for (index, (formation, cards, discard)) in steps.into_iter().enumerate() {
        play::perform(&mut record, &p1, formation, cards);
        play::finish_discarding(&mut record, &p1, Some(CardInstanceId::new(discard)));
        match index {
            0 => {
                play::change(&mut record, &p2, "first-wanderer", &[3]);
            }
            1 => {
                play::change(&mut record, &p2, "mesmer", &[38]);
            }
            2 => {
                play::change(&mut record, &p2, "spirit-mesmer", &[41, 42, 44]);
            }
            3 => {
                play::perform(&mut record, &p2, "fire-spirit-summoning", &[59, 60]);
            }
            _ => {
                play::perform(&mut record, &p2, "countershock", &[55, 56]);
            }
        }
        play::finish(&mut record, &p2);
    }
    assert_eq!(record.state().summoned_stars_for(&p1).unwrap().len(), 4);
    assert_eq!(record.state().spirit_for(&p1).unwrap().power, 6);
    let events = play::perform(&mut record, &p1, "triple-metal", &[9, 13, 16]);
    assert!(events.iter().any(|event| matches!(event, GameEvent::FiveStarAlignmentAchieved { player, .. } if player == &p1)));
    assert!(events.iter().any(|event| matches!(event, GameEvent::AttackResolved { hp_changes, .. }
        if hp_changes.iter().any(|entry| entry.change.team() == &TeamId::new("team:a") && entry.change.new_hp() == 0))));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::AutomaticBloomsResolved { .. }))
    );
    assert!(
        matches!(&record.state().status, GameStatus::Finished { conclusion } if conclusion.outcome == GameOutcome::Winner(TeamId::new("team:a")))
    );
    assert_eq!(record.state().spirit_for(&p1).unwrap().power, 6);
    play::assert_replay(&record);
}

#[test]
fn void_spirit_shattering_resolves_power_breaking_hp_and_cards_as_one_event() {
    let [p1, p2, p3, p4] = ["p1", "p2", "p3", "p4"].map(PlayerId::new);
    let formation = [9, 27, 45];
    let elements = [Element::Wood, Element::Metal, Element::Fire, Element::Earth];
    let charges = [4, 0, 2, 0];
    let baseline = four_summoned_spirits(elements, charges, &formation, [250, 250], &[]);
    let hp = &baseline.state().hp;
    let mut record = four_summoned_spirits(
        elements,
        charges,
        &formation,
        [290 - hp[0].hp, 290 - hp[1].hp],
        &[],
    );
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .map(|entry| entry.hp)
            .collect::<Vec<_>>(),
        [40, 40]
    );
    let events = play::perform(&mut record, &p1, "void-spirit-shattering", &formation);
    let payloads = events
        .iter()
        .filter(|event| matches!(event, GameEvent::VoidSpiritShatteringResolved { .. }))
        .collect::<Vec<_>>();
    assert!(
        matches!(payloads.as_slice(), [GameEvent::VoidSpiritShatteringResolved {
        spirit_changes, broken_spirits, hp_changes, card_moves, ..
    }] if spirit_changes.len() == 4 && broken_spirits.len() == 2 && hp_changes.len() == 2 && card_moves.is_empty())
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::AutomaticBloomsResolved { .. }))
    );
    assert!(
        matches!(&record.state().status, GameStatus::Finished { conclusion } if conclusion.outcome == GameOutcome::Draw)
    );
    assert_eq!(record.state().spirit_for(&p1).unwrap().power, 4);
    assert_eq!(record.state().spirit_for(&p3).unwrap().power, 2);
    assert!(record.state().spirit_for(&p2).is_none());
    assert!(record.state().spirit_for(&p4).is_none());
    assert_eq!(
        record
            .state()
            .formation_area(&p1)
            .unwrap()
            .formation
            .as_ref()
            .unwrap()
            .cards,
        play::ids(&formation)
    );
    play::assert_replay(&record);
}

#[test]
fn seal_cancels_void_spirit_shattering_but_still_discards_the_formation() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let used = [9, 27, 45];
    for sealed in [false, true] {
        let mut record = play::start_with_reserves(
            shared_spirit_setup(2),
            &[&[19, 23], &[37, 41]],
            &[&used, &[]],
        );
        play::perform(&mut record, &p1, "wood-spirit-summoning", &[19, 23]);
        play::finish(&mut record, &p1);
        if sealed {
            play::perform(&mut record, &p2, "seal", &[37, 41]);
        } else {
            play::strike_with_spare(&mut record, &p2, &[]);
        }
        play::finish(&mut record, &p2);
        let spirit = record.state().spirit_for(&p1).cloned().unwrap();
        let hp = record.state().hp.clone();
        let events = play::perform(&mut record, &p1, "void-spirit-shattering", &used);
        assert!(events.iter().any(|event| matches!(event, GameEvent::FormationCommitted { player, formation_id, cards, .. }
            if player == &p1 && formation_id == "void-spirit-shattering" && cards == &play::ids(&used))));
        assert_eq!(
            events
                .iter()
                .any(|event| matches!(event, GameEvent::VoidSpiritShatteringResolved { .. })),
            !sealed
        );
        if sealed {
            assert!(events.iter().any(|event| matches!(event, GameEvent::PassiveFlipped { outcome: fewfc::domain::PassiveFlipOutcome::Applied { modifications, .. }, .. }
                if modifications.contains(&fewfc::domain::ActionModification::CancelSpell))));
            assert_eq!(record.state().spirit_for(&p1), Some(&spirit));
            assert_eq!(record.state().hp, hp);
        } else {
            assert_eq!(record.state().hp[0].hp, hp[0].hp - 20);
        }
        assert!(record.state().discard.ends_with(&play::ids(&used)));
        play::assert_replay(&record);
    }
}

fn shared_spirit_setup(players: usize) -> GameSetup {
    play::setup(
        players,
        &[
            STAR_MODULE_ID,
            FIVE_DIRECTIONS_LEGEND_MODULE_ID,
            HERO_SCHOOLS_MODULE_ID,
            SPIRIT_MODULE_ID,
        ],
    )
}

fn skill_command(
    player: &PlayerId,
    skill: SpiritSkill,
    selected_card: Option<CardInstanceId>,
    declared_level: Option<u32>,
) -> Command {
    Command::UseSpiritSkill {
        player: player.clone(),
        skill,
        selected_card,
        declared_level,
    }
}

fn charged_spirit_record(
    element: Element,
    power: u32,
    reserved: &[u64],
    opponent_reserved: &[u64],
) -> GameRecord {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let (offset, formation) = match element {
        Element::Metal => (0, "metal-spirit-summoning"),
        Element::Wood => (18, "wood-spirit-summoning"),
        Element::Water => (36, "water-spirit-summoning"),
        Element::Fire => (54, "fire-spirit-summoning"),
        Element::Earth => (72, "earth-spirit-summoning"),
    };
    let first = (offset + 1..=offset + 18)
        .filter(|card| !reserved.contains(card) && !opponent_reserved.contains(card))
        .take(6)
        .collect::<Vec<_>>();
    let summon = &first[..2];
    let charge_turns = (power - 2).max(1) as usize;
    let fixed = reserved
        .iter()
        .chain(opponent_reserved)
        .chain(&first)
        .copied()
        .collect::<Vec<_>>();
    let mut spare =
        (1..=90).filter(|card| !fixed.contains(card) && !(offset + 1..=offset + 18).contains(card));
    let mut wanted = reserved.iter().copied();
    let mut prefix = summon.to_vec();
    for _ in 0..2 {
        prefix.push(wanted.next().unwrap_or_else(|| spare.next().unwrap()));
    }
    let mut held = reserved.len().min(2);
    prefix.extend_from_slice(opponent_reserved);
    prefix.extend(spare.by_ref().take(5 - opponent_reserved.len()));
    for turn in 0..charge_turns {
        let keep = if turn < 2 { 2 } else { 1 };
        prefix.push(if power == 2 {
            spare.next().unwrap()
        } else {
            first[turn + 2]
        });
        let hand_size = if turn == 0 { 4 } else { 5 };
        let capacity = hand_size - usize::from(turn + 1 < charge_turns);
        for _ in 0..keep {
            let card = if held < capacity {
                wanted.next().inspect(|_| held += 1)
            } else {
                None
            };
            prefix.push(card.unwrap_or_else(|| spare.next().unwrap()));
        }
        prefix.extend(spare.by_ref().take(2));
    }
    assert!(wanted.next().is_none(), "保留牌超過充能流程可取得的手牌數");
    if power == 6 && element == Element::Metal {
        // 充到上限後，同一次正常抽牌提供一張同元素牌與一張不同元素牌。
        prefix.push(
            (offset + 1..=offset + 18)
                .find(|card| !prefix.contains(card) && !fixed.contains(card))
                .unwrap(),
        );
        prefix.push(spare.next().unwrap());
    }

    let mut record = play::start(shared_spirit_setup(2), &prefix);
    play::perform(&mut record, &p1, formation, summon);
    for turn in 0..charge_turns {
        if turn > 0 {
            play::strike_with_spare(&mut record, &p1, reserved);
        }
        play::finish(&mut record, &p1);
        assert_eq!(
            record.state().spirit_for(&p1).unwrap().power,
            if power == 2 { 2 } else { 3 + turn as u32 }
        );
        play::strike_with_spare(&mut record, &p2, opponent_reserved);
        play::finish(&mut record, &p2);
    }
    record
}

fn four_summoned_spirits(
    elements: [Element; 4],
    charges: [usize; 4],
    reserved: &[u64],
    hp: [i32; 2],
    teammate_shock: &[u64],
) -> GameRecord {
    let players = ["p1", "p2", "p3", "p4"].map(PlayerId::new);
    let mut configured = shared_spirit_setup(4);
    for (entry, hp) in configured.hp.iter_mut().zip(hp) {
        entry.hp = hp;
    }
    let mut available = (1..=90)
        .filter(|card| !reserved.contains(card) && !teammate_shock.contains(card))
        .collect::<Vec<_>>();
    let elemental_cards = elements.map(|element| match element {
        Element::Metal => (1..=18, "metal-spirit-summoning"),
        Element::Wood => (19..=36, "wood-spirit-summoning"),
        Element::Water => (37..=54, "water-spirit-summoning"),
        Element::Fire => (55..=72, "fire-spirit-summoning"),
        Element::Earth => (73..=90, "earth-spirit-summoning"),
    });
    let sources = std::array::from_fn::<_, 4, _>(|seat| {
        let range = &elemental_cards[seat].0;
        (0..2 + charges[seat])
            .map(|_| {
                let index = available
                    .iter()
                    .position(|card| range.contains(card))
                    .unwrap();
                available.remove(index)
            })
            .collect::<Vec<_>>()
    });

    let mut prefix = Vec::new();
    let mut reserves = [reserved, &[], teammate_shock, &[]].map(|cards| cards.iter().copied());
    for (seat, cards) in sources.iter().enumerate() {
        prefix.extend(&cards[..2]);
        for _ in 0..if seat == 0 { 2 } else { 3 } {
            prefix.push(reserves[seat].next().unwrap_or_else(|| available.remove(0)));
        }
    }
    for round in 0..4 {
        for seat in 0..4 {
            let discard = if round < charges[seat] {
                sources[seat][round + 2]
            } else {
                let index = available
                    .iter()
                    .position(|card| !elemental_cards[seat].0.contains(card))
                    .unwrap();
                available.remove(index)
            };
            prefix.push(discard);
            let keep = if round == 0
                || (round == 1 && seat == 0)
                || (round == 3 && seat == 2 && !teammate_shock.is_empty())
            {
                2
            } else {
                1
            };
            for _ in 0..keep {
                prefix.push(reserves[seat].next().unwrap_or_else(|| available.remove(0)));
            }
        }
    }
    assert!(reserves.iter_mut().all(|cards| cards.next().is_none()));
    let mut record = play::start(configured, &prefix);
    for round in 0..4 {
        for (seat, player) in players.iter().enumerate() {
            if round == 0 {
                play::perform(
                    &mut record,
                    player,
                    elemental_cards[seat].1,
                    &sources[seat][..2],
                );
            } else if round == 3 && seat == 2 && !teammate_shock.is_empty() {
                play::perform(&mut record, player, "shock-burst", teammate_shock);
            } else {
                let protected = match seat {
                    0 => reserved,
                    2 => teammate_shock,
                    _ => &[],
                };
                play::strike_with_spare(&mut record, player, protected);
            }
            play::finish(&mut record, player);
        }
    }
    for (seat, player) in players.iter().enumerate() {
        assert_eq!(
            record.state().spirit_for(player).unwrap().power,
            charges[seat] as u32 + 2
        );
    }
    record
}
