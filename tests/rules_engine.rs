use fewfc::application::{
    AutomaticReason, CommandContext, CommandKind, EventSource, GameRecord, StartGame,
    advance_automatic as advance_state_automatic, apply_event, handle_command,
};
use fewfc::domain::{
    ActionModification, AttackPointBreakdown, CardDef, CardDefId, CardInstanceDef, CardInstanceId,
    CardMoveDelta, CardZone, Command, CommandId, DamageTransform, DeckPlacement,
    EffectChoiceAnswer, ElementInteraction, EngineInvariantError, EnvironmentAttackEffect,
    GameError, GameEvent, GameOutcome, GameSetup, GameState, GameStatus, HpChangeDelta,
    LastElementalAttack, LastElementalAttackUpdate, LastFormationUse, PassActionReason,
    PassiveFlipOutcome, PassiveNoEffectReason, PendingChoice, PendingChoiceKind, Phase, Player,
    PlayerHand, PlayerId, PlayerShield, RuleImplementationError, RuleModuleId, RulesetId,
    ShieldChangeDelta, StatusDuration, StatusEffect, StatusExpiryTiming, StatusOwner, TeamHp,
    TeamId, TurnDrawSkipReason, ValidationError,
};
use fewfc::public_view::{
    self, PublicCardRefs, PublicCoveredPassive, PublicGameEvent, PublicPendingChoice,
    PublicPendingChoiceKind, PublicPendingChoicePresentation, PublicPlayerHand,
    PublicPreviousTurnFormation, Viewer,
};
use fewfc::rules::Element;

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

fn card_def(id: &str, element: Element) -> CardDef {
    CardDef {
        id: CardDefId::new(id),
        name: id.to_string(),
        element,
        level: match id {
            "metal" => 3,
            "wood" => 2,
            "water" => 1,
            "fire" => 4,
            "earth" => 5,
            _ => 1,
        },
    }
}

fn card_instance(instance: u64, def_id: &str) -> CardInstanceDef {
    CardInstanceDef {
        instance: card(instance),
        definition: CardDefId::new(def_id),
        origin: Default::default(),
    }
}

fn two_player_setup() -> GameSetup {
    two_player_setup_with_hp(30)
}

fn two_player_setup_with_hp(starting_hp: i32) -> GameSetup {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    GameSetup::two_player(p1, p2, starting_hp).with_cards(
        vec![
            card_def("metal", Element::Metal),
            card_def("wood", Element::Wood),
            card_def("water", Element::Water),
            card_def("fire", Element::Fire),
            card_def("earth", Element::Earth),
        ],
        (1..=20)
            .map(|id| {
                let def_id = match id % 5 {
                    1 => "metal",
                    2 => "wood",
                    3 => "water",
                    4 => "fire",
                    _ => "earth",
                };
                card_instance(id, def_id)
            })
            .collect(),
    )
}

fn bare_team_setup(players_by_team: &[(&str, &str)]) -> GameSetup {
    GameSetup {
        ruleset: RulesetId::base(),
        enabled_rule_modules: Vec::new(),
        players: players_by_team
            .iter()
            .map(|(player, team)| Player {
                id: PlayerId::new(*player),
                team: TeamId::new(*team),
            })
            .collect(),
        turn_order: players_by_team
            .iter()
            .map(|(player, _)| PlayerId::new(*player))
            .collect(),
        hp: players_by_team
            .iter()
            .map(|(_, team)| TeamId::new(*team))
            .fold(Vec::<TeamId>::new(), |mut teams, team| {
                if !teams.contains(&team) {
                    teams.push(team);
                }
                teams
            })
            .into_iter()
            .map(|team| TeamHp { team, hp: 30 })
            .collect(),
        card_defs: Vec::new(),
        card_instances: Vec::new(),
        deck_lists: Vec::new(),
        hand_limit: 5,
        base_draw: 2,
    }
}

fn official_deck() -> Vec<CardInstanceId> {
    (1..=20).map(card).collect()
}

fn deck_starting_with(first_cards: &[u64]) -> Vec<CardInstanceId> {
    let mut deck = first_cards.iter().copied().map(card).collect::<Vec<_>>();

    for id in 1..=20 {
        let candidate = card(id);
        if !deck.contains(&candidate) {
            deck.push(candidate);
        }
    }

    deck
}

fn cannot_act_status(player: PlayerId) -> StatusEffect {
    StatusEffect {
        id: format!("cannot-act-{}", player.as_str()),
        owner: StatusOwner::Player(player),
        kind: "CannotAct".to_string(),
        value: None,
        duration: StatusDuration::Permanent,
    }
}

fn add_status(state: &mut GameState, status: StatusEffect) {
    apply_event(state, &GameEvent::StatusAdded { status });
}

fn state_after_cannot_act_pass(record: &GameRecord, player: PlayerId) -> GameState {
    let mut state = record.state().clone();
    add_status(&mut state, cannot_act_status(player.clone()));

    let events = handle_command(
        &state,
        Command::PassAction {
            player,
            reason: PassActionReason::CannotActByStatus,
        },
    )
    .unwrap();

    for event in events {
        apply_event(&mut state, &event);
    }

    state
}

fn advance_record_to_next_main_after_turn_draw(record: &mut GameRecord, discard: CardInstanceId) {
    record.advance_automatic().unwrap();
    record
        .handle(Command::ChooseTurnDiscard {
            player: PlayerId::new("p1"),
            discard,
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn record_after_p1_metal_attack_on_turn_1() -> GameRecord {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(10));
    record
}

#[test]
fn sacred_beast_attacks_then_transfers_the_environment() {
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.push(card_instance(21, "metal"));
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.hands = vec![
        PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(6), card(11), card(16), card(21)],
        ),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "west-white-tiger".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 81,
                    final_amount: 81,
                    ..
                },
                ..
            },
            GameEvent::EnvironmentTransferred {
                player,
                formation_id,
                from: None,
                to: Element::Metal,
            }
        ] if player == &PlayerId::new("p1") && formation_id == "west-white-tiger"
    ));

    for event in &events {
        apply_event(&mut state, event);
    }
    assert_eq!(state.environment, Some(Element::Metal));
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p2"))
            .map(|team_hp| team_hp.hp),
        Some(119)
    );
}

#[test]
fn matching_environment_damage_stacks_with_overcoming_interaction() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state.last_formation_by_player.insert(
        PlayerId::new("p2"),
        LastFormationUse {
            formation_id: "wood-strike".to_string(),
            resolved_effect_id: "wood-strike".to_string(),
            used_cards: vec![card(2)],
            resolved_turn: 0,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown {
                base_points: 7,
                environment_effect: EnvironmentAttackEffect::MatchingElementDamageDoubled {
                    environment: Element::Metal,
                },
                interaction: ElementInteraction::Overcoming,
                damage_transform: DamageTransform::DoubleDamage,
                final_amount: 28,
            },
            hp_change: HpChangeDelta { new_hp: 72, .. },
            ..
        }]
    ));
}

#[test]
fn environment_healing_stacks_with_overcoming_interaction() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(5)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state
        .hp
        .iter_mut()
        .find(|team_hp| team_hp.team == TeamId::new("team:p2"))
        .unwrap()
        .hp = 40;
    state.last_formation_by_player.insert(
        PlayerId::new("p2"),
        LastFormationUse {
            formation_id: "water-strike".to_string(),
            resolved_effect_id: "water-strike".to_string(),
            used_cards: vec![card(3)],
            resolved_turn: 0,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown {
                base_points: 9,
                environment_effect:
                    EnvironmentAttackEffect::GeneratingElementDamageConvertedToHealing {
                        environment: Element::Metal,
                    },
                interaction: ElementInteraction::Overcoming,
                damage_transform: DamageTransform::HealTarget,
                final_amount: 18,
            },
            hp_change: HpChangeDelta { new_hp: 58, .. },
            ..
        }]
    ));
}

#[test]
fn overlapping_environment_and_generating_recovery_applies_once() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(5)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state
        .hp
        .iter_mut()
        .find(|team_hp| team_hp.team == TeamId::new("team:p2"))
        .unwrap()
        .hp = 40;
    state.last_formation_by_player.insert(
        PlayerId::new("p2"),
        LastFormationUse {
            formation_id: "metal-strike".to_string(),
            resolved_effect_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            resolved_turn: 0,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown {
                environment_effect:
                    EnvironmentAttackEffect::GeneratingElementDamageConvertedToHealing {
                        environment: Element::Metal,
                    },
                interaction: ElementInteraction::Generating,
                damage_transform: DamageTransform::HealTarget,
                final_amount: 9,
                ..
            },
            hp_change: HpChangeDelta { new_hp: 49, .. },
            ..
        }]
    ));
}

#[test]
fn environment_recovery_is_halved_rounding_up_by_same_element_neutralization() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(5)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state
        .hp
        .iter_mut()
        .find(|team_hp| team_hp.team == TeamId::new("team:p2"))
        .unwrap()
        .hp = 40;
    state.last_formation_by_player.insert(
        PlayerId::new("p2"),
        LastFormationUse {
            formation_id: "earth-strike".to_string(),
            resolved_effect_id: "earth-strike".to_string(),
            used_cards: vec![card(5)],
            resolved_turn: 0,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown {
                interaction: ElementInteraction::Same,
                damage_transform: DamageTransform::HealTarget,
                final_amount: 5,
                ..
            },
            hp_change: HpChangeDelta { new_hp: 45, .. },
            ..
        }]
    ));
}

#[test]
fn shield_receives_damage_before_environment_can_convert_it_to_healing() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(5)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state
        .hp
        .iter_mut()
        .find(|team_hp| team_hp.team == TeamId::new("team:p2"))
        .unwrap()
        .hp = 40;
    state
        .shields
        .iter_mut()
        .find(|shield| shield.player == PlayerId::new("p2"))
        .unwrap()
        .value = 20;

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown {
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 9,
                ..
            },
            hp_change: HpChangeDelta { new_hp: 40, .. },
            shield_change: Some(ShieldChangeDelta { new_value: 11, .. }),
            ..
        }]
    ));
}

#[test]
fn sacred_beast_consumes_defense_without_preventing_damage() {
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.push(card_instance(21, "metal"));
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.hands = vec![
        PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(6), card(11), card(16), card(21)],
        ),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p2"),
        formation_id: "defense".to_string(),
        cards: vec![card(2), card(7)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 0,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "west-white-tiger".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::PassiveFlipped {
                outcome: PassiveFlipOutcome::NoEffect {
                    reason: PassiveNoEffectReason::IgnoredBySacredBeast,
                },
                ..
            },
            GameEvent::AttackResolved {
                hp_change: HpChangeDelta { new_hp: 119, .. },
                ..
            },
            GameEvent::EnvironmentTransferred { .. },
        ]
    ));
}

#[test]
fn ineffective_barrier_is_performed_without_replacing_an_existing_shield() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(2), card(7), card(1), card(4)],
        ),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state.shields = vec![
        PlayerShield {
            player: PlayerId::new("p1"),
            value: 5,
        },
        PlayerShield {
            player: PlayerId::new("p2"),
            value: 0,
        },
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "barrier".to_string(),
            cards: vec![card(2), card(7), card(1), card(4)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::FormationPerformed { formation_id, .. },
            GameEvent::FormationEffectIgnored {
                reason:
                    fewfc::domain::FormationNoEffectReason::IneffectiveInEnvironment {
                        environment: Element::Metal,
                    },
                ..
            },
        ] if formation_id == "barrier"
    ));
    for event in &events {
        apply_event(&mut state, event);
    }
    assert_eq!(state.shield(&PlayerId::new("p1")), Some(5));
    assert!(state.hand(&PlayerId::new("p1")).unwrap().is_empty());
}

#[test]
fn ineffective_weapon_is_performed_without_dealing_damage() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Fire);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(6)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::FormationEffectIgnored {
                reason: fewfc::domain::FormationNoEffectReason::IneffectiveInEnvironment {
                    environment: Element::Fire,
                },
                ..
            },
            GameEvent::AttackResolved {
                hp_change: HpChangeDelta {
                    old_hp: 100,
                    new_hp: 100,
                    effective_delta: 0,
                    ..
                },
                shield_change: None,
                ..
            },
        ]
    ));
}

#[test]
fn ineffective_defense_flips_and_is_consumed_without_preventing_damage() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(1)]),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "defense".to_string(),
        cards: vec![card(2), card(7)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 0,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::PassiveFlipped {
                outcome: PassiveFlipOutcome::NoEffect {
                    reason: PassiveNoEffectReason::IneffectiveInEnvironment {
                        environment: Element::Metal,
                    },
                },
                ..
            },
            GameEvent::AttackResolved {
                hp_change: HpChangeDelta { new_hp: 86, .. },
                ..
            },
        ]
    ));
}

#[test]
fn public_state_view_exposes_the_shared_environment_to_every_viewer() {
    let mut state = GameState::from_setup(
        &two_player_setup_with_hp(100)
            .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]),
    );
    state.environment = Some(Element::Water);

    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p1"))).environment,
        Some(Element::Water)
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Observer).environment,
        Some(Element::Water)
    );
}

#[test]
fn void_meridian_severing_clears_environment_and_changes_team_hp_atomically() {
    let setup = two_player_setup_with_hp(15)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Fire);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(6), card(11)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(1), card(6), card(11)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::FormationPerformed { .. },
            GameEvent::EnvironmentCleared {
                player,
                formation_id,
                environment: Element::Fire,
                hp_changes,
            },
        ] if player == &PlayerId::new("p1")
            && formation_id == "void-meridian-severing"
            && hp_changes.len() == 2
            && hp_changes.iter().all(|change| change.new_hp == 0)
    ));

    for event in &events {
        apply_event(&mut state, event);
    }
    assert_eq!(state.environment, None);
    assert_eq!(
        state.status,
        GameStatus::Finished {
            outcome: GameOutcome::Draw,
        }
    );
}

#[test]
fn void_meridian_severing_without_environment_does_not_change_hp() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(6), card(11)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(1), card(6), card(11)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [GameEvent::FormationPerformed { .. }]
    ));
    for event in &events {
        apply_event(&mut state, event);
    }
    assert!(state.hp.iter().all(|team_hp| team_hp.hp == 100));
}

#[test]
fn sealed_void_meridian_severing_does_not_clear_environment_or_change_hp() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Fire);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(6), card(11)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p2"),
        formation_id: "seal".to_string(),
        cards: vec![card(3), card(8)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 0,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(1), card(6), card(11)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(!events.iter().any(|event| matches!(
        event,
        GameEvent::EnvironmentCleared { .. } | GameEvent::HpChanged { .. }
    )));
    for event in &events {
        apply_event(&mut state, event);
    }
    assert_eq!(state.environment, Some(Element::Fire));
    assert!(state.hp.iter().all(|team_hp| team_hp.hp == 100));
}

#[test]
fn void_meridian_severing_changes_each_team_hp_once_in_team_mode() {
    let card_setup = two_player_setup();
    let setup = GameSetup::team_mode(
        TeamId::new("A"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("B"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        250,
    )
    .with_cards(card_setup.card_defs, card_setup.card_instances)
    .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Earth);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(6), card(11)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
        PlayerHand::new(PlayerId::new("p3"), Vec::new()),
        PlayerHand::new(PlayerId::new("p4"), Vec::new()),
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(1), card(6), card(11)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::FormationPerformed { .. },
            GameEvent::EnvironmentCleared { hp_changes, .. },
        ] if hp_changes.len() == 2
            && hp_changes.iter().all(|change| change.old_hp == 250 && change.new_hp == 230)
    ));
}

#[test]
fn same_element_sacred_beast_still_records_environment_transfer() {
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.push(card_instance(21, "metal"));
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(6), card(11), card(16), card(21)],
        ),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "west-white-tiger".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(matches!(
        events.last(),
        Some(GameEvent::EnvironmentTransferred {
            from: Some(Element::Metal),
            to: Element::Metal,
            ..
        })
    ));
}

fn defense_setup_deck() -> Vec<CardInstanceId> {
    deck_starting_with(&[2, 7, 1, 3])
}

fn seal_setup_deck() -> Vec<CardInstanceId> {
    deck_starting_with(&[3, 8, 1, 2])
}

fn record_after_p1_covers_defense() -> GameRecord {
    let mut record = GameRecord::start(two_player_setup(), defense_setup_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(10));
    record
}

fn record_after_p1_covers_seal() -> GameRecord {
    let mut record = GameRecord::start(two_player_setup(), seal_setup_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(10));
    record
}

fn record_after_p1_covers_countershock() -> GameRecord {
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[4, 9, 1, 2, 5, 10, 3, 6, 7]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(8));
    record
}

#[test]
fn new_game_deals_initial_hands_from_prepared_deck_order() {
    let deck = official_deck();
    let record = GameRecord::start(two_player_setup(), deck.clone()).unwrap();

    assert_eq!(
        record.events(),
        &[
            GameEvent::DeckPrepared {
                deck_order: deck.clone(),
            },
            GameEvent::CardsDealt {
                player: PlayerId::new("p1"),
                cards: vec![card(1), card(2), card(3), card(4)],
            },
            GameEvent::CardsDealt {
                player: PlayerId::new("p2"),
                cards: vec![card(5), card(6), card(7), card(8), card(9)],
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(1), card(2), card(3), card(4)].as_slice())
    );
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(5), card(6), card(7), card(8), card(9)].as_slice())
    );
    assert_eq!(state.deck, (10..=20).map(card).collect::<Vec<_>>());
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn new_game_preserves_card_instance_definitions_for_lookup() {
    let record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let state = record.state().clone();

    assert_eq!(
        state.card_def(card(1)),
        Some(&CardDef {
            id: CardDefId::new("metal"),
            name: "metal".to_string(),
            element: Element::Metal,
            level: 3,
        })
    );
    assert_eq!(state.card_def(card(99)), None);
}

#[test]
fn game_state_resolves_card_instance_elements_for_formation_matching() {
    let record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let state = record.state().clone();

    assert_eq!(state.card_element(card(1)), Some(Element::Metal));
    assert_eq!(state.card_element(card(2)), Some(Element::Wood));
    assert_eq!(state.card_element(card(99)), None);
}

#[test]
fn new_game_rejects_deck_that_cannot_satisfy_initial_deal() {
    assert_eq!(
        GameRecord::start(two_player_setup(), vec![card(1), card(2), card(3)]),
        Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed: 9,
                available: 3,
            }
        ))
    );
}

#[test]
fn new_game_persists_deck_order_and_replay_matches_current_state() {
    let deck = official_deck();
    let record = GameRecord::start(two_player_setup(), deck.clone()).unwrap();

    assert_eq!(
        record.events().first(),
        Some(&GameEvent::DeckPrepared {
            deck_order: deck.clone()
        })
    );

    let state = record.state().clone();
    assert_eq!(state.phase, Phase::TurnStart);
    assert_eq!(record.setup().ruleset, RulesetId::base());
    assert_eq!(state.current_player(), Some(&PlayerId::new("p1")));
    assert_eq!(state.deck, (10..=20).map(card).collect::<Vec<_>>());
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn game_record_facade_applies_commands_and_verifies_replay() {
    let mut record = GameRecord::start_game(StartGame {
        setup: two_player_setup(),
        deck_order: official_deck(),
    })
    .unwrap();

    let automatic = record.advance_until_decision().unwrap();
    assert_eq!(
        automatic.events(),
        &[GameEvent::TurnStarted {
            player: PlayerId::new("p1"),
            turn_number: 1,
        }]
    );

    let command_events = record
        .apply(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        command_events.events(),
        [GameEvent::AttackResolved { .. }]
    ));

    let view = record.public_view(Viewer::Observer).unwrap();
    assert_eq!(view.phase, Phase::TurnDraw);
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn game_record_exposes_recorded_events_with_sequence_metadata() {
    let deck = official_deck();
    let mut record = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let recorded_events = record.recorded_events();

    assert_eq!(
        recorded_events
            .iter()
            .map(|recorded| recorded.metadata.sequence)
            .collect::<Vec<_>>(),
        (1..=recorded_events.len() as u64).collect::<Vec<_>>()
    );
    assert_eq!(recorded_events[0].metadata.source, EventSource::Setup);
    assert_eq!(
        recorded_events[0].event,
        GameEvent::DeckPrepared { deck_order: deck }
    );
    assert_eq!(
        recorded_events[3].metadata.source,
        EventSource::Automatic {
            reason: AutomaticReason::TurnStart,
        }
    );
    assert_eq!(
        recorded_events.last().unwrap().metadata.source,
        EventSource::Command {
            command_id: CommandId::new(1),
            context: CommandContext {
                player: PlayerId::new("p1"),
                kind: CommandKind::PerformFormation {
                    formation_id: "metal-strike".to_string(),
                },
            },
        }
    );
}

#[test]
fn apply_event_projects_canonical_events_without_returning_validation_errors() {
    let setup = two_player_setup();
    let mut state = fewfc::domain::GameState::from_setup(&setup);

    let projected: () = apply_event(
        &mut state,
        &GameEvent::DeckPrepared {
            deck_order: vec![card(10), card(11)],
        },
    );

    assert_eq!(projected, ());
    assert_eq!(state.deck, vec![card(10), card(11)]);
}

#[test]
fn setup_validation_rejects_duplicate_card_instances_across_hands_and_deck() {
    let mut deck = official_deck();
    deck[1] = card(1);

    assert_eq!(
        GameRecord::start(two_player_setup(), deck),
        Err(GameError::Validation(ValidationError::DuplicateCard(card(
            1
        ))))
    );
}

#[test]
fn setup_validation_rejects_deck_card_without_instance_definition() {
    let mut setup = two_player_setup();
    setup
        .card_instances
        .retain(|instance_def| instance_def.instance != card(20));

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::MissingCardInstanceDefinition(card(20))
        ))
    );
}

#[test]
fn setup_validation_rejects_card_instance_with_unknown_definition() {
    let mut setup = two_player_setup();
    setup.card_instances[0].definition = CardDefId::new("missing");

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::MissingCardDefinition(CardDefId::new("missing"))
        ))
    );
}

#[test]
fn setup_validation_rejects_duplicate_card_instance_definitions() {
    let mut setup = two_player_setup();
    setup.card_instances.push(card_instance(1, "metal"));

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(ValidationError::DuplicateCard(card(
            1
        ))))
    );
}

#[test]
fn setup_validation_requires_hp_for_every_team() {
    let setup = GameSetup {
        ruleset: RulesetId::base(),
        enabled_rule_modules: Vec::new(),
        players: vec![Player {
            id: PlayerId::new("p1"),
            team: TeamId::new("A"),
        }],
        turn_order: vec![PlayerId::new("p1")],
        hp: Vec::new(),
        card_defs: Vec::new(),
        card_instances: Vec::new(),
        deck_lists: Vec::new(),
        hand_limit: 5,
        base_draw: 2,
    };

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(ValidationError::MissingTeamHp(
            TeamId::new("A")
        )))
    );
}

#[test]
fn setup_validation_rejects_team_mode_turn_order_that_is_not_alternating() {
    let setup = GameSetup {
        ruleset: RulesetId::base(),
        enabled_rule_modules: Vec::new(),
        players: vec![
            Player {
                id: PlayerId::new("p1"),
                team: TeamId::new("A"),
            },
            Player {
                id: PlayerId::new("p2"),
                team: TeamId::new("A"),
            },
            Player {
                id: PlayerId::new("p3"),
                team: TeamId::new("B"),
            },
            Player {
                id: PlayerId::new("p4"),
                team: TeamId::new("B"),
            },
        ],
        turn_order: vec![
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            PlayerId::new("p3"),
            PlayerId::new("p4"),
        ],
        hp: vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 30,
            },
        ],
        card_defs: Vec::new(),
        card_instances: Vec::new(),
        deck_lists: Vec::new(),
        hand_limit: 5,
        base_draw: 2,
    };

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::TeamSeatingNotAlternating {
                previous_player: PlayerId::new("p1"),
                player: PlayerId::new("p2"),
                team: TeamId::new("A"),
            }
        ))
    );
}

#[test]
fn setup_validation_requires_turn_order_to_contain_every_player_once() {
    let mut setup = two_player_setup();
    setup.turn_order = vec![PlayerId::new("p1"), PlayerId::new("p1")];

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::DuplicateTurnOrderPlayer(PlayerId::new("p1"))
        ))
    );

    let mut setup = two_player_setup();
    setup.turn_order = vec![PlayerId::new("p1")];

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::MissingTurnOrderPlayer(PlayerId::new("p2"))
        ))
    );
}

#[test]
fn setup_validation_rejects_invalid_team_mode_shapes() {
    assert_eq!(
        GameRecord::start(
            bare_team_setup(&[("p1", "A"), ("p2", "A"), ("p3", "B")]),
            official_deck()
        ),
        Err(GameError::Validation(
            ValidationError::TeamModeRequiresAtLeastFourPlayers { player_count: 3 }
        ))
    );

    assert_eq!(
        GameRecord::start(
            bare_team_setup(&[
                ("p1", "A"),
                ("p2", "B"),
                ("p3", "C"),
                ("p4", "A"),
                ("p5", "B"),
                ("p6", "C"),
            ]),
            deck_starting_with(&(1..=30).collect::<Vec<_>>()),
        ),
        Err(GameError::Validation(
            ValidationError::TeamModeRequiresExactlyTwoTeams { team_count: 3 }
        ))
    );

    assert_eq!(
        GameRecord::start(
            bare_team_setup(&[
                ("p1", "A"),
                ("p2", "B"),
                ("p3", "A"),
                ("p4", "B"),
                ("p5", "A"),
            ]),
            deck_starting_with(&(1..=30).collect::<Vec<_>>()),
        ),
        Err(GameError::Validation(
            ValidationError::TeamModeRequiresEqualTeamSizes {
                first_team: TeamId::new("A"),
                first_count: 3,
                second_team: TeamId::new("B"),
                second_count: 2,
            }
        ))
    );
}

#[test]
fn team_mode_builder_produces_valid_alternating_setup() {
    let setup = GameSetup::team_mode(
        TeamId::new("A"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("B"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        30,
    );

    assert_eq!(
        setup.turn_order,
        vec![
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            PlayerId::new("p3"),
            PlayerId::new("p4"),
        ]
    );
    assert_eq!(setup.ruleset, RulesetId::base());
    let card_setup = two_player_setup();
    GameRecord::start(
        setup.with_cards(card_setup.card_defs, card_setup.card_instances),
        official_deck(),
    )
    .unwrap();
}

#[test]
fn team_mode_attack_resolves_previous_player_and_opposing_team_without_declared_targets() {
    let card_setup = two_player_setup();
    let setup = GameSetup::team_mode(
        TeamId::new("A"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("B"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        30,
    )
    .with_cards(card_setup.card_defs, card_setup.card_instances);
    let mut record = GameRecord::start(setup, official_deck()).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p1"),
            target: PlayerId::new("p4"),
            formation_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            point_breakdown: AttackPointBreakdown {
                base_points: 7,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 7,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("B"),
                old_hp: 30,
                delta: -7,
                new_hp: 23,
                effective_delta: -7,
            },
            shield_change: None,
            card_moves: vec![CardMoveDelta {
                card: card(1),
                from: CardZone::Hand(PlayerId::new("p1")),
                to: CardZone::Discard,
            }],
            elemental_context_update: Some(LastElementalAttackUpdate {
                player: PlayerId::new("p1"),
                attack: LastElementalAttack {
                    element: Element::Metal,
                    resolved_turn: 1,
                },
            }),
        }]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 23,
            },
        ]
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn team_mode_attack_uses_only_the_previous_players_personal_shield() {
    let card_setup = two_player_setup();
    let setup = GameSetup::team_mode(
        TeamId::new("A"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("B"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        30,
    )
    .with_cards(card_setup.card_defs, card_setup.card_instances);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
        PlayerHand::new(PlayerId::new("p3"), Vec::new()),
        PlayerHand::new(PlayerId::new("p4"), Vec::new()),
    ];
    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p2"),
            old_value: 0,
            delta: 20,
            new_value: 20,
        },
    );
    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p4"),
            old_value: 0,
            delta: 5,
            new_value: 5,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(state.shield(&PlayerId::new("p2")), Some(20));
    assert_eq!(state.shield(&PlayerId::new("p4")), Some(0));
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("B"))
            .map(|team_hp| team_hp.hp),
        Some(30)
    );
}

#[test]
fn rule_derived_attack_targets_reject_declared_targets_without_events() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(
        record.handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: vec![fewfc::domain::TargetDecl::Player(PlayerId::new("p2"))],
        }),
        Err(GameError::Validation(
            ValidationError::UnexpectedDeclaredTargets {
                formation_id: "metal-strike".to_string(),
            }
        ))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn rule_derived_active_spell_targets_reject_declared_targets_without_events() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[2, 7, 1, 4])).unwrap();
    record.advance_automatic().unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(
        record.handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "barrier".to_string(),
            cards: vec![card(2), card(7), card(1), card(4)],
            declared_targets: vec![fewfc::domain::TargetDecl::Player(PlayerId::new("p1"))],
        }),
        Err(GameError::Validation(
            ValidationError::UnexpectedDeclaredTargets {
                formation_id: "barrier".to_string(),
            }
        ))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn new_game_state_exposes_core_status_shields_and_passive_zones() {
    let record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let state = record.state().clone();

    assert_eq!(state.status, GameStatus::InProgress);
    assert_eq!(state.shield(&PlayerId::new("p1")), Some(0));
    assert_eq!(state.shield(&PlayerId::new("p2")), Some(0));
    assert!(state.covered_passives.is_empty());
    assert!(state.statuses.is_empty());
}

#[test]
fn pending_choices_can_store_effect_generated_continuations() {
    let choice = PendingChoice {
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::EffectGenerated {
            effect_id: "choose-card-to-seal".to_string(),
            continuation_id: "seal-resolution".to_string(),
            allowed_cards: vec![card(1), card(2)],
        },
    };

    assert_eq!(choice.clone(), choice);
}

#[test]
fn unknown_effect_choice_continuation_is_rule_implementation_error_without_events_or_state_changes()
{
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.pending_choice = Some(PendingChoice {
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::EffectGenerated {
            effect_id: "unknown-effect".to_string(),
            continuation_id: "unknown-continuation".to_string(),
            allowed_cards: Vec::new(),
        },
    });
    let state_before = state.clone();

    assert_eq!(
        handle_command(
            &state,
            Command::AnswerEffectChoice {
                player: PlayerId::new("p1"),
                selected_cards: Vec::new(),
            },
        ),
        Err(GameError::RuleImplementation(
            RuleImplementationError::EffectNotImplemented("unknown-continuation".to_string())
        ))
    );
    assert_eq!(state, state_before);
}

#[test]
fn automatic_draw_invariant_error_emits_no_events_and_leaves_state_unchanged() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::TurnDraw;
    let state_before = state.clone();

    assert_eq!(
        advance_state_automatic(&state),
        Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed: 3,
                available: 0,
            }
        ))
    );
    assert_eq!(state, state_before);
}

#[test]
fn duplicate_covered_passive_is_engine_invariant_before_command_validation() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "defense".to_string(),
        cards: vec![card(2), card(7)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "seal".to_string(),
        cards: vec![card(3), card(8)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });
    let state_before = state.clone();

    assert_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::NoCardsInHand,
            },
        ),
        Err(GameError::EngineInvariant(
            EngineInvariantError::DuplicateCoveredPassive {
                player: PlayerId::new("p1"),
            }
        ))
    );
    assert_eq!(state, state_before);
}

#[test]
fn turn_start_status_expiration_is_event_logged_before_turn_starts() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    apply_event(
        &mut state,
        &GameEvent::StatusAdded {
            status: StatusEffect {
                id: "cannot-act-p1".to_string(),
                owner: StatusOwner::Player(PlayerId::new("p1")),
                kind: "CannotAct".to_string(),
                value: None,
                duration: StatusDuration::UntilTurnStart {
                    player: PlayerId::new("p1"),
                },
            },
        },
    );

    let events = advance_state_automatic(&state).unwrap();
    assert_eq!(
        events,
        vec![
            GameEvent::StatusExpired {
                status_id: "cannot-act-p1".to_string(),
                owner: StatusOwner::Player(PlayerId::new("p1")),
                expired_at: StatusExpiryTiming::TurnStart {
                    player: PlayerId::new("p1"),
                },
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p1"),
                turn_number: 1,
            },
        ]
    );

    let mut replayed = GameState::from_setup(&setup);
    add_status(
        &mut replayed,
        StatusEffect {
            id: "cannot-act-p1".to_string(),
            owner: StatusOwner::Player(PlayerId::new("p1")),
            kind: "CannotAct".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnStart {
                player: PlayerId::new("p1"),
            },
        },
    );

    for event in &events {
        apply_event(&mut state, event);
        apply_event(&mut replayed, event);
    }
    assert!(state.statuses.is_empty());
    assert_eq!(state.phase, Phase::Main);
    assert_eq!(replayed, state);
}

#[test]
fn turn_end_status_expiration_is_event_logged_before_turn_ends() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::TurnEnd;
    apply_event(
        &mut state,
        &GameEvent::StatusAdded {
            status: StatusEffect {
                id: "cannot-act-until-end".to_string(),
                owner: StatusOwner::Player(PlayerId::new("p1")),
                kind: "CannotAct".to_string(),
                value: None,
                duration: StatusDuration::UntilTurnEnd {
                    player: PlayerId::new("p1"),
                },
            },
        },
    );

    let events = advance_state_automatic(&state).unwrap();
    assert_eq!(
        events,
        vec![
            GameEvent::StatusExpired {
                status_id: "cannot-act-until-end".to_string(),
                owner: StatusOwner::Player(PlayerId::new("p1")),
                expired_at: StatusExpiryTiming::TurnEnd {
                    player: PlayerId::new("p1"),
                },
            },
            GameEvent::TurnEnded {
                player: PlayerId::new("p1"),
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p2"),
                turn_number: 2,
            },
        ]
    );

    let mut replayed = GameState::from_setup(&setup);
    replayed.phase = Phase::TurnEnd;
    add_status(
        &mut replayed,
        StatusEffect {
            id: "cannot-act-until-end".to_string(),
            owner: StatusOwner::Player(PlayerId::new("p1")),
            kind: "CannotAct".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEnd {
                player: PlayerId::new("p1"),
            },
        },
    );

    for event in &events {
        apply_event(&mut state, event);
        apply_event(&mut replayed, event);
    }
    assert!(state.statuses.is_empty());
    assert_eq!(state.phase, Phase::Main);
    assert_eq!(state.current_player(), Some(&PlayerId::new("p2")));
    assert_eq!(replayed, state);
}

#[test]
fn permanent_statuses_do_not_expire_and_remain_visible_to_command_validation() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = record.state().clone();
    add_status(&mut state, cannot_act_status(PlayerId::new("p1")));

    assert_eq!(advance_state_automatic(&state).unwrap(), Vec::new());
    assert_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::CannotActByStatus,
            },
        )
        .unwrap(),
        vec![GameEvent::ActionPassed {
            player: PlayerId::new("p1"),
            reason: PassActionReason::CannotActByStatus,
        }]
    );
    assert_eq!(state.statuses, vec![cannot_act_status(PlayerId::new("p1"))]);
}

#[test]
fn invalid_command_returns_error_without_appending_events_or_changing_state() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    let result = record.handle(Command::ChooseTurnDiscard {
        player: PlayerId::new("p1"),
        discard: card(1),
    });

    assert_eq!(
        result,
        Err(GameError::Validation(ValidationError::WrongPhase {
            expected: Phase::TurnDrawDiscardChoice,
            actual: Phase::TurnStart,
        }))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn automatic_advance_stops_at_main_after_turn_start() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();

    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![GameEvent::TurnStarted {
            player: PlayerId::new("p1"),
            turn_number: 1,
        }]
    );

    let state = record.state().clone();
    assert_eq!(state.phase, Phase::Main);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn pass_action_with_no_cards_consumes_the_turn_action_and_enters_turn_draw() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    apply_event(
        &mut state,
        &GameEvent::TurnStarted {
            player: PlayerId::new("p1"),
            turn_number: 1,
        },
    );

    assert_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::NoCardsInHand,
            },
        )
        .unwrap(),
        vec![GameEvent::ActionPassed {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        }]
    );
}

#[test]
fn pass_action_with_cards_is_rejected_without_changing_state() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    let result = record.handle(Command::PassAction {
        player: PlayerId::new("p1"),
        reason: PassActionReason::NoCardsInHand,
    });

    assert_eq!(
        result,
        Err(GameError::Validation(ValidationError::CannotPassAction {
            reason: PassActionReason::NoCardsInHand,
        }))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn pass_action_is_allowed_when_player_cannot_act_by_status() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = record.state().clone();
    add_status(&mut state, cannot_act_status(PlayerId::new("p1")));

    assert_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::CannotActByStatus,
            },
        )
        .unwrap(),
        vec![GameEvent::ActionPassed {
            player: PlayerId::new("p1"),
            reason: PassActionReason::CannotActByStatus,
        }]
    );
}

#[test]
fn pass_action_still_flips_and_discards_the_previous_players_covered_passive() {
    let mut state = record_after_p1_covers_defense().state().clone();
    add_status(&mut state, cannot_act_status(PlayerId::new("p2")));

    let events = handle_command(
        &state,
        Command::PassAction {
            player: PlayerId::new("p2"),
            reason: PassActionReason::CannotActByStatus,
        },
    )
    .unwrap();

    for event in &events {
        apply_event(&mut state, event);
    }

    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            passive_id,
            outcome: PassiveFlipOutcome::NoEffect {
                reason: PassiveNoEffectReason::NotAnAttack,
            },
            ..
        } if owner == &PlayerId::new("p1") && passive_id == "defense"
    )));
    assert!(state.covered_passives.is_empty());
    assert!(state.discard.contains(&card(2)));
    assert!(state.discard.contains(&card(7)));
    assert_eq!(state.phase, Phase::TurnDraw);
}

#[test]
fn turn_draw_creates_pending_discard_choice_after_drawing_available_space_plus_one() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = state_after_cannot_act_pass(&record, PlayerId::new("p1"));

    assert_eq!(
        advance_state_automatic(&state).unwrap(),
        vec![GameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("p1"),
            drawn_cards: vec![card(10), card(11)],
            allowed_discards: vec![card(10), card(11)],
        }]
    );

    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.phase, Phase::TurnDrawDiscardChoice);
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(1), card(2), card(3), card(4), card(10), card(11)].as_slice())
    );
    assert_eq!(state.deck, (12..=20).map(card).collect::<Vec<_>>());
    assert_eq!(
        state.pending_choice,
        Some(PendingChoice {
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::TurnDrawDiscard {
                drawn_cards: vec![card(10), card(11)],
                allowed_discards: vec![card(10), card(11)],
            },
        })
    );
}

#[test]
fn choosing_turn_discard_finishes_draw_choice_and_moves_card_to_discard() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = state_after_cannot_act_pass(&record, PlayerId::new("p1"));
    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    assert_eq!(
        handle_command(
            &state,
            Command::ChooseTurnDiscard {
                player: PlayerId::new("p1"),
                discard: card(10),
            },
        )
        .unwrap(),
        vec![GameEvent::TurnDiscardChosen {
            player: PlayerId::new("p1"),
            discard: card(10),
        }]
    );

    for event in handle_command(
        &state,
        Command::ChooseTurnDiscard {
            player: PlayerId::new("p1"),
            discard: card(10),
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.phase, Phase::TurnEnd);
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(1), card(2), card(3), card(4), card(11)].as_slice())
    );
    assert_eq!(state.discard, vec![card(10)]);
    assert!(state.pending_choice.is_none());
}

#[test]
fn choosing_turn_discard_rejects_cards_not_drawn_this_turn() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = state_after_cannot_act_pass(&record, PlayerId::new("p1"));
    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }
    let state_before = state.clone();

    assert_eq!(
        handle_command(
            &state,
            Command::ChooseTurnDiscard {
                player: PlayerId::new("p1"),
                discard: card(2),
            },
        ),
        Err(GameError::Validation(ValidationError::IllegalDiscard(
            card(2)
        )))
    );
    assert_eq!(state, state_before);
}

#[test]
fn turn_end_automatic_advance_starts_next_players_turn() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = state_after_cannot_act_pass(&record, PlayerId::new("p1"));
    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }
    for event in handle_command(
        &state,
        Command::ChooseTurnDiscard {
            player: PlayerId::new("p1"),
            discard: card(10),
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }

    assert_eq!(
        advance_state_automatic(&state).unwrap(),
        vec![
            GameEvent::TurnEnded {
                player: PlayerId::new("p1"),
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p2"),
                turn_number: 2,
            },
        ]
    );

    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.phase, Phase::Main);
    assert_eq!(state.current_player(), Some(&PlayerId::new("p2")));
    assert_eq!(state.turn_number, 2);
}

#[test]
fn turn_draw_is_skipped_when_hand_is_already_at_limit() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = state_after_cannot_act_pass(&record, PlayerId::new("p1"));
    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }
    for event in handle_command(
        &state,
        Command::ChooseTurnDiscard {
            player: PlayerId::new("p1"),
            discard: card(10),
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }
    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    add_status(&mut state, cannot_act_status(PlayerId::new("p2")));
    for event in handle_command(
        &state,
        Command::PassAction {
            player: PlayerId::new("p2"),
            reason: PassActionReason::CannotActByStatus,
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }

    assert_eq!(
        advance_state_automatic(&state).unwrap(),
        vec![
            GameEvent::TurnDrawSkipped {
                player: PlayerId::new("p2"),
                reason: TurnDrawSkipReason::HandLimitReached,
            },
            GameEvent::TurnEnded {
                player: PlayerId::new("p2"),
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p1"),
                turn_number: 3,
            },
        ]
    );

    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.phase, Phase::Main);
    assert_eq!(state.current_player(), Some(&PlayerId::new("p1")));
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(5), card(6), card(7), card(8), card(9)].as_slice())
    );
    assert_eq!(state.deck, (12..=20).map(card).collect::<Vec<_>>());
    assert!(state.pending_choice.is_none());
}

#[test]
fn turn_draw_recycles_discard_to_deck_bottom_when_deck_is_insufficient() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::TurnDraw;
    state.current_turn_index = 1;
    state.deck = vec![card(12)];
    state.discard = vec![card(2)];
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(
            PlayerId::new("p2"),
            vec![card(5), card(6), card(7), card(8)],
        ),
    ];

    assert_eq!(
        advance_state_automatic(&state).unwrap(),
        vec![
            GameEvent::DiscardRecycledIntoDeck {
                shuffled_order: vec![card(2)],
                placement: DeckPlacement::Bottom,
            },
            GameEvent::CardsDrawnForTurnDiscardChoice {
                player: PlayerId::new("p2"),
                drawn_cards: vec![card(12), card(2)],
                allowed_discards: vec![card(12), card(2)],
            },
        ]
    );

    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.phase, Phase::TurnDrawDiscardChoice);
    assert_eq!(state.deck, Vec::<CardInstanceId>::new());
    assert_eq!(state.discard, Vec::<CardInstanceId>::new());
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(5), card(6), card(7), card(8), card(12), card(2)].as_slice())
    );
}

#[test]
fn perform_attack_formation_damages_previous_players_team_and_moves_cards_to_discard() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p1"),
            target: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            point_breakdown: AttackPointBreakdown {
                base_points: 7,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 7,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 30,
                delta: -7,
                new_hp: 23,
                effective_delta: -7,
            },
            shield_change: None,
            card_moves: vec![CardMoveDelta {
                card: card(1),
                from: CardZone::Hand(PlayerId::new("p1")),
                to: CardZone::Discard,
            }],
            elemental_context_update: Some(LastElementalAttackUpdate {
                player: PlayerId::new("p1"),
                attack: LastElementalAttack {
                    element: Element::Metal,
                    resolved_turn: 1,
                },
            }),
        }]
    );

    let state = record.state().clone();
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(2), card(3), card(4)].as_slice())
    );
    assert_eq!(state.discard, vec![card(1)]);
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 23,
            },
        ]
    );
    assert_eq!(
        state
            .last_elemental_attack_by_player
            .get(&PlayerId::new("p1")),
        Some(&LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        })
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn triple_fire_uses_level_sum_times_three_and_replays() {
    let mut record = GameRecord::start(
        two_player_setup_with_hp(100),
        deck_starting_with(&[4, 9, 14, 1]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "triple-fire".to_string(),
            cards: vec![card(4), card(9), card(14)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p2"))
            .map(|team_hp| team_hp.hp),
        Some(64)
    );
    assert_eq!(state.discard, vec![card(4), card(9), card(14)]);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn shock_burst_uses_level_sum_times_four_without_elemental_context() {
    let mut record = GameRecord::start(
        two_player_setup_with_hp(100),
        deck_starting_with(&[4, 9, 3, 5]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "shock-burst".to_string(),
            cards: vec![card(4), card(9), card(3), card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p2"))
            .map(|team_hp| team_hp.hp),
        Some(44)
    );
    assert!(
        !state
            .last_elemental_attack_by_player
            .contains_key(&PlayerId::new("p1"))
    );
    assert_eq!(state.discard, vec![card(4), card(9), card(3), card(5)]);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn five_streams_unite_uses_target_hand_count_and_increases_the_same_turn_draw() {
    let mut setup = two_player_setup_with_hp(100);
    setup.card_instances.push(card_instance(21, "metal"));
    let mut record =
        GameRecord::start(setup, deck_starting_with(&[2, 3, 4, 5, 1, 6, 11, 16, 21])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "empty-city".to_string(),
            cards: vec![card(2), card(3)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(7));

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "five-streams-unite".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        record
            .state()
            .turn_draw_bonus_by_player
            .get(&PlayerId::new("p2")),
        Some(&1)
    );
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(40)
    );

    record.advance_automatic().unwrap();
    assert!(matches!(
        &record.state().pending_choice,
        Some(PendingChoice {
            player,
            kind: PendingChoiceKind::TurnDrawDiscard { drawn_cards, .. },
        }) if player == &PlayerId::new("p2") && drawn_cards.len() == 4
    ));
    record
        .handle(Command::ChooseTurnDiscard {
            player: PlayerId::new("p2"),
            discard: card(10),
        })
        .unwrap();
    record.advance_automatic().unwrap();

    let state = record.state().clone();
    assert_eq!(
        state
            .hand(&PlayerId::new("p2"))
            .map(<[CardInstanceId]>::len),
        Some(3)
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn immediate_active_spell_resolves_through_perform_formation() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[2, 7, 1, 4])).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "barrier".to_string(),
                cards: vec![card(2), card(7), card(1), card(4)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "barrier".to_string(),
                used_cards: vec![card(2), card(7), card(1), card(4)],
                declared_targets: Vec::new(),
            },
            GameEvent::ShieldChanged {
                player: PlayerId::new("p1"),
                old_value: 0,
                delta: 44,
                new_value: 44,
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(state.hand(&PlayerId::new("p1")), Some([].as_slice()));
    assert_eq!(state.discard, vec![card(2), card(7), card(1), card(4)]);
    assert_eq!(state.shield(&PlayerId::new("p1")), Some(44));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn generating_formation_heals_current_players_team_through_public_command_flow() {
    let mut record =
        GameRecord::start(two_player_setup_with_hp(20), deck_starting_with(&[1, 3, 2])).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "generating-formation".to_string(),
                cards: vec![card(1), card(3), card(2)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "generating-formation".to_string(),
                used_cards: vec![card(1), card(3), card(2)],
                declared_targets: Vec::new(),
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 20,
                    delta: 18,
                    new_hp: 20,
                    effective_delta: 0,
                },
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(20)
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn recovery_cannot_raise_team_hp_above_its_initial_value() {
    let mut record =
        GameRecord::start(two_player_setup_with_hp(20), deck_starting_with(&[1, 3, 2])).unwrap();
    record.advance_automatic().unwrap();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "generating-formation".to_string(),
            cards: vec![card(1), card(3), card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(20)
    );
    assert_eq!(state.discard, vec![card(1), card(3), card(2)]);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn generating_formation_targets_own_side_in_team_mode_without_declared_targets() {
    let card_setup = two_player_setup();
    let setup = GameSetup::team_mode(
        TeamId::new("A"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("B"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        20,
    )
    .with_cards(card_setup.card_defs, card_setup.card_instances);
    let mut record = GameRecord::start(setup, deck_starting_with(&[1, 3, 2])).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "generating-formation".to_string(),
                cards: vec![card(1), card(3), card(2)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "generating-formation".to_string(),
                used_cards: vec![card(1), card(3), card(2)],
                declared_targets: Vec::new(),
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("A"),
                    old_hp: 20,
                    delta: 18,
                    new_hp: 20,
                    effective_delta: 0,
                },
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 20,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 20,
            },
        ]
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn overcoming_formation_damages_previous_players_team_through_public_command_flow() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(2), card(5)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p2"),
            old_value: 0,
            delta: 12,
            new_value: 12,
        },
    );

    assert_eq!(
        handle_command(
            &state,
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "overcoming-formation".to_string(),
                cards: vec![card(1), card(2), card(5)],
                declared_targets: Vec::new(),
            }
        )
        .unwrap(),
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "overcoming-formation".to_string(),
                used_cards: vec![card(1), card(2), card(5)],
                declared_targets: Vec::new(),
            },
            GameEvent::ShieldChanged {
                player: PlayerId::new("p2"),
                old_value: 12,
                delta: -12,
                new_value: 0,
            },
        ]
    );

    for event in handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "overcoming-formation".to_string(),
            cards: vec![card(1), card(2), card(5)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }
    assert_eq!(state.shield(&PlayerId::new("p2")), Some(0));
}

#[test]
fn radiance_prevents_next_player_action_and_draw_for_two_turns() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.hands = vec![
        PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(6), card(4), card(3)],
        ),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "radiance".to_string(),
            cards: vec![card(1), card(6), card(4), card(3)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(
        events,
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "radiance".to_string(),
                used_cards: vec![card(1), card(6), card(4), card(3)],
                declared_targets: Vec::new(),
            },
            GameEvent::HandInspected {
                viewer: PlayerId::new("p1"),
                target: PlayerId::new("p2"),
                cards: Vec::new(),
            },
            GameEvent::StatusAdded {
                status: StatusEffect {
                    id: "radiance-cannot-act-p2-turn-1".to_string(),
                    owner: StatusOwner::Player(PlayerId::new("p2")),
                    kind: "CannotAct".to_string(),
                    value: None,
                    duration: StatusDuration::UntilTurnEndNumber {
                        player: PlayerId::new("p2"),
                        turn_number: 4,
                    },
                },
            },
            GameEvent::StatusAdded {
                status: StatusEffect {
                    id: "radiance-cannot-draw-p2-turn-1".to_string(),
                    owner: StatusOwner::Player(PlayerId::new("p2")),
                    kind: "CannotDraw".to_string(),
                    value: None,
                    duration: StatusDuration::UntilTurnEndNumber {
                        player: PlayerId::new("p2"),
                        turn_number: 4,
                    },
                },
            },
        ]
    );

    for event in events {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.statuses.len(), 2);
    assert_eq!(state.phase, Phase::TurnDraw);
}

#[test]
fn radiance_records_a_private_snapshot_of_the_next_players_hand() {
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 4, 3, 2, 5, 7, 8, 9]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    let events = record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "radiance".to_string(),
            cards: vec![card(1), card(6), card(4), card(3)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let inspected = events
        .iter()
        .find(|event| matches!(event, GameEvent::HandInspected { .. }))
        .unwrap();

    assert_eq!(
        public_view::event_for(inspected, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::HandInspected {
            viewer: PlayerId::new("p1"),
            target: PlayerId::new("p2"),
            cards: PublicCardRefs::Known(vec![card(2), card(5), card(7), card(8), card(9),]),
        }
    );
    for viewer in [Viewer::Player(PlayerId::new("p2")), Viewer::Observer] {
        assert_eq!(
            public_view::event_for(inspected, viewer),
            PublicGameEvent::HandInspected {
                viewer: PlayerId::new("p1"),
                target: PlayerId::new("p2"),
                cards: PublicCardRefs::Hidden { count: 5 },
            }
        );
    }
    assert_eq!(record.replay().unwrap(), record.state().clone());
}

#[test]
fn metamorphosis_copies_previous_players_last_base_formation_effect() {
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 2, 3, 5, 10, 4, 7, 8]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(9));

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "metamorphosis".to_string(),
                cards: vec![card(5), card(10)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p2"),
                formation_id: "metamorphosis".to_string(),
                used_cards: vec![card(5), card(10)],
                declared_targets: Vec::new(),
            },
            GameEvent::FormationEffectCopied {
                player: PlayerId::new("p2"),
                effect_id: "weapon".to_string(),
            },
            GameEvent::AttackResolved {
                attacker: PlayerId::new("p2"),
                target: PlayerId::new("p1"),
                formation_id: "weapon".to_string(),
                used_cards: vec![card(5), card(10)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 20,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 20,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: -20,
                    new_hp: 10,
                    effective_delta: -20,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: None,
            },
        ]
    );

    let state = record.state().clone();
    assert!(state.pending_choice.is_none());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(state.shield(&PlayerId::new("p2")), Some(0));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn metamorphosis_recomputes_a_copied_elemental_attack_and_keeps_its_own_identity() {
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 2, 3, 4, 5, 10, 6, 7, 8]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(9));

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(23)
    );
    assert_eq!(
        state
            .last_formation_by_player
            .get(&PlayerId::new("p2"))
            .map(|formation| formation.formation_id.as_str()),
        Some("metamorphosis")
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn metamorphosis_chain_copies_the_previous_resolved_effect_without_changing_names() {
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 5, 10, 15, 20, 2, 3, 4]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(7));
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(15), card(20)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::ChooseTurnDiscard {
            player: PlayerId::new("p2"),
            discard: card(11),
        })
        .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    for player in ["p1", "p2"] {
        let formation = state
            .last_formation_by_player
            .get(&PlayerId::new(player))
            .unwrap();
        assert_eq!(formation.formation_id, "metamorphosis");
        assert_eq!(formation.effective_effect_id(), "weapon");
    }
    assert_eq!(
        state.status,
        GameStatus::Finished {
            outcome: GameOutcome::Team(TeamId::new("team:p1")),
        }
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn metamorphosis_copies_five_streams_unites_damage_and_draw_bonus() {
    let mut state = GameState::from_setup(&two_player_setup_with_hp(100));
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(2), card(3)]),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5), card(10)]),
    ];
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "five-streams-unite".to_string(),
            resolved_effect_id: "five-streams-unite".to_string(),
            used_cards: Vec::new(),
            resolved_turn: 1,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(55)
    );
    assert_eq!(
        state.turn_draw_bonus_by_player.get(&PlayerId::new("p2")),
        Some(&1)
    );
    let formation = state
        .last_formation_by_player
        .get(&PlayerId::new("p2"))
        .unwrap();
    assert_eq!(formation.formation_id, "metamorphosis");
    assert_eq!(formation.effective_effect_id(), "five-streams-unite");
}

#[test]
fn metamorphosis_copies_an_active_spell_and_records_its_resolved_effect() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5), card(10)]),
    ];
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "barrier".to_string(),
            resolved_effect_id: "barrier".to_string(),
            used_cards: Vec::new(),
            resolved_turn: 1,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(state.shield(&PlayerId::new("p2")), Some(40));
    let formation = state
        .last_formation_by_player
        .get(&PlayerId::new("p2"))
        .unwrap();
    assert_eq!(formation.formation_id, "metamorphosis");
    assert_eq!(formation.effective_effect_id(), "barrier");
}

#[test]
fn metamorphosis_copies_a_passive_effect_as_a_public_delayed_counter() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5), card(10)]),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "defense".to_string(),
        cards: vec![card(2), card(7)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "defense".to_string(),
            resolved_effect_id: "defense".to_string(),
            used_cards: vec![card(2), card(7)],
            resolved_turn: 1,
        },
    );

    let copy_events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &copy_events {
        apply_event(&mut state, event);
    }

    assert!(state.covered_passives.is_empty());
    assert_eq!(state.counter_effects.len(), 1);
    assert_eq!(state.counter_effects[0].owner, PlayerId::new("p2"));
    assert_eq!(state.counter_effects[0].effect_id, "defense");
    for used_card in [card(2), card(7), card(5), card(10)] {
        assert!(state.discard.contains(&used_card));
    }

    state.phase = Phase::Main;
    state.current_turn_index = 0;
    state.turn_number = 3;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    let attack_events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &attack_events {
        apply_event(&mut state, event);
    }

    assert!(state.counter_effects.is_empty());
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p2"))
            .map(|team_hp| team_hp.hp),
        Some(30)
    );
}

#[test]
fn metamorphosis_copying_empty_city_does_not_create_a_counter_effect() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5), card(10)]),
    ];
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "empty-city".to_string(),
            resolved_effect_id: "empty-city".to_string(),
            used_cards: vec![card(1), card(2)],
            resolved_turn: 1,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert!(state.counter_effects.is_empty());
    assert_eq!(
        state
            .last_formation_by_player
            .get(&PlayerId::new("p2"))
            .map(LastFormationUse::effective_effect_id),
        Some("empty-city")
    );
}

#[test]
fn chaos_requests_two_next_player_hand_cards_and_returns_them_to_deck_top() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 2, 1])).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "chaos".to_string(),
                cards: vec![card(5), card(10), card(2), card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "chaos".to_string(),
                used_cards: vec![card(5), card(10), card(2), card(1)],
                declared_targets: Vec::new(),
            },
            GameEvent::HandInspected {
                viewer: PlayerId::new("p1"),
                target: PlayerId::new("p2"),
                cards: vec![card(3), card(4), card(6), card(7), card(8)],
            },
            GameEvent::EffectChoiceRequested {
                player: PlayerId::new("p1"),
                kind: PendingChoiceKind::EffectGenerated {
                    effect_id: "chaos".to_string(),
                    continuation_id: "chaos:return-two".to_string(),
                    allowed_cards: vec![card(3), card(4), card(6), card(7), card(8)],
                },
            },
        ]
    );

    assert_eq!(
        record.handle(Command::AnswerEffectChoiceTyped {
            player: PlayerId::new("p1"),
            answer: EffectChoiceAnswer::Cards {
                cards: vec![card(3)],
            },
        }),
        Err(GameError::Validation(
            ValidationError::InvalidEffectChoiceAnswer
        ))
    );
    assert_eq!(
        record.handle(Command::AnswerEffectChoiceTyped {
            player: PlayerId::new("p1"),
            answer: EffectChoiceAnswer::Cards {
                cards: vec![card(3), card(3)],
            },
        }),
        Err(GameError::Validation(
            ValidationError::InvalidEffectChoiceAnswer
        ))
    );

    assert_eq!(
        record
            .handle(Command::AnswerEffectChoiceTyped {
                player: PlayerId::new("p1"),
                answer: EffectChoiceAnswer::Cards {
                    cards: vec![card(3), card(4)],
                },
            })
            .unwrap(),
        vec![
            GameEvent::TypedEffectChoiceAnswered {
                player: PlayerId::new("p1"),
                effect_id: "chaos".to_string(),
                continuation_id: "chaos:return-two".to_string(),
                answer: EffectChoiceAnswer::Cards {
                    cards: vec![card(3), card(4)],
                },
            },
            GameEvent::CardsMoved {
                card_moves: vec![
                    CardMoveDelta {
                        card: card(4),
                        from: CardZone::Hand(PlayerId::new("p2")),
                        to: CardZone::DeckTop,
                    },
                    CardMoveDelta {
                        card: card(3),
                        from: CardZone::Hand(PlayerId::new("p2")),
                        to: CardZone::DeckTop,
                    },
                ],
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(6), card(7), card(8)].as_slice())
    );
    assert_eq!(
        state.deck.iter().take(2).copied().collect::<Vec<_>>(),
        vec![card(3), card(4)]
    );
    assert!(state.pending_choice.is_none());
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn active_spell_intent_can_change_hp_through_public_command_flow() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[3, 8, 5, 2])).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "return-to-origin".to_string(),
                cards: vec![card(3), card(8), card(5), card(2)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "return-to-origin".to_string(),
                used_cards: vec![card(3), card(8), card(5), card(2)],
                declared_targets: Vec::new(),
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: 36,
                    new_hp: 30,
                    effective_delta: 0,
                },
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(30)
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn five_elements_cycle_exchanges_team_hp_through_public_command_flow() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.hp = vec![
        TeamHp {
            team: TeamId::new("team:p1"),
            hp: 12,
        },
        TeamHp {
            team: TeamId::new("team:p2"),
            hp: 27,
        },
    ];
    state.hands = vec![
        PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(2), card(3), card(4), card(5)],
        ),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "five-elements-cycle".to_string(),
            cards: vec![card(1), card(2), card(3), card(4), card(5)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(
        events,
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "five-elements-cycle".to_string(),
                used_cards: vec![card(1), card(2), card(3), card(4), card(5)],
                declared_targets: Vec::new(),
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 12,
                    delta: 15,
                    new_hp: 27,
                    effective_delta: 15,
                },
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 27,
                    delta: -15,
                    new_hp: 12,
                    effective_delta: -15,
                },
            },
        ]
    );

    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(state.hand(&PlayerId::new("p1")), Some([].as_slice()));
    assert_eq!(
        state.discard,
        vec![card(1), card(2), card(3), card(4), card(5)]
    );
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 27,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 12,
            },
        ]
    );
    assert_eq!(state.phase, Phase::TurnDraw);
}

#[test]
fn answering_effect_choice_resumes_resolution_deterministically() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 2, 1])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "chaos".to_string(),
            cards: vec![card(5), card(10), card(2), card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    assert_eq!(
        record
            .handle(Command::AnswerEffectChoice {
                player: PlayerId::new("p1"),
                selected_cards: vec![card(3), card(4)],
            })
            .unwrap(),
        vec![
            GameEvent::EffectChoiceAnswered {
                player: PlayerId::new("p1"),
                effect_id: "chaos".to_string(),
                continuation_id: "chaos:return-two".to_string(),
                selected_cards: vec![card(3), card(4)],
            },
            GameEvent::CardsMoved {
                card_moves: vec![
                    CardMoveDelta {
                        card: card(4),
                        from: CardZone::Hand(PlayerId::new("p2")),
                        to: CardZone::DeckTop,
                    },
                    CardMoveDelta {
                        card: card(3),
                        from: CardZone::Hand(PlayerId::new("p2")),
                        to: CardZone::DeckTop,
                    },
                ],
            },
        ]
    );

    let state = record.state().clone();
    assert!(state.pending_choice.is_none());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(6), card(7), card(8)].as_slice())
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn commands_and_automatic_advance_wait_while_effect_choice_is_pending() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 2, 1])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "chaos".to_string(),
            cards: vec![card(5), card(10), card(2), card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(
        record.handle(Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        }),
        Err(GameError::Validation(
            ValidationError::PendingChoiceInProgress {
                player: PlayerId::new("p1"),
            }
        ))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
    assert_eq!(record.advance_automatic().unwrap(), Vec::new());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn performing_passive_spell_covers_cards_and_consumes_action() {
    let mut record = GameRecord::start(two_player_setup(), defense_setup_deck()).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "defense".to_string(),
                cards: vec![card(2), card(7)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            star_substitution: None,
            sealed: false,
        }]
    );

    let state = record.state().clone();
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(1), card(3)].as_slice())
    );
    assert_eq!(
        state.covered_passives,
        vec![fewfc::domain::CoveredPassive {
            owner: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            star_substitution: None,
            sealed: false,
            covered_on_turn: 1,
            reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
        }]
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn empty_city_flips_with_its_intentional_no_effect_outcome_and_is_discarded() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[1, 2, 3, 4])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "empty-city".to_string(),
            cards: vec![card(1), card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(10));

    let events = record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            passive_id,
            outcome: PassiveFlipOutcome::NoEffect {
                reason: PassiveNoEffectReason::EmptyCity,
            },
            ..
        } if passive_id == "empty-city"
    )));
    let state = record.state().clone();
    assert!(state.covered_passives.is_empty());
    assert!(state.discard.contains(&card(1)));
    assert!(state.discard.contains(&card(2)));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn player_cannot_cover_second_passive_while_one_is_pending() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(2), card(7)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "seal".to_string(),
        cards: vec![card(3), card(8)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });
    let state_before = state.clone();

    assert_eq!(
        handle_command(
            &state,
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "defense".to_string(),
                cards: vec![card(2), card(7)],
                declared_targets: Vec::new(),
            },
        ),
        Err(GameError::Validation(
            ValidationError::PendingPassiveAlreadyCovered {
                player: PlayerId::new("p1"),
            }
        ))
    );
    assert_eq!(state, state_before);
}

#[test]
fn defense_prevents_incoming_attack_damage_and_records_action_modification() {
    let mut record = record_after_p1_covers_defense();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::PassiveFlipped {
                owner: PlayerId::new("p1"),
                incoming_player: PlayerId::new("p2"),
                passive_id: "defense".to_string(),
                cards: vec![card(2), card(7)],
                outcome: PassiveFlipOutcome::Applied {
                    effect_id: "defense".to_string(),
                    modifications: vec![ActionModification::PreventDamage],
                },
            },
            GameEvent::AttackResolved {
                attacker: PlayerId::new("p2"),
                target: PlayerId::new("p1"),
                formation_id: "fire-strike".to_string(),
                used_cards: vec![card(9)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 8,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 8,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: 0,
                    new_hp: 30,
                    effective_delta: 0,
                },
                shield_change: None,
                card_moves: vec![CardMoveDelta {
                    card: card(9),
                    from: CardZone::Hand(PlayerId::new("p2")),
                    to: CardZone::Discard,
                }],
                elemental_context_update: Some(LastElementalAttackUpdate {
                    player: PlayerId::new("p2"),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 2,
                    },
                }),
            },
        ]
    );

    let state = record.state().clone();
    assert!(state.covered_passives.is_empty());
    assert_eq!(state.discard, vec![card(10), card(2), card(7), card(9)]);
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 30,
            },
        ]
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn defense_prevents_five_streams_damage_but_not_its_draw_bonus() {
    let mut setup = two_player_setup_with_hp(100);
    setup.card_instances.push(card_instance(21, "metal"));
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(2), card(3)]),
        PlayerHand::new(
            PlayerId::new("p2"),
            vec![card(1), card(6), card(11), card(16), card(21)],
        ),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "defense".to_string(),
        cards: vec![card(7), card(12)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "five-streams-unite".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(100)
    );
    assert_eq!(
        state.turn_draw_bonus_by_player.get(&PlayerId::new("p2")),
        Some(&1)
    );
    for used_card in [
        card(7),
        card(12),
        card(1),
        card(6),
        card(11),
        card(16),
        card(21),
    ] {
        assert!(state.discard.contains(&used_card));
    }
}

#[test]
fn countershock_rounds_each_players_half_of_odd_attack_damage_up() {
    let mut record = record_after_p1_covers_countershock();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 26,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 26,
            },
        ]
    );
    assert!(state.covered_passives.is_empty());
    assert!(state.discard.contains(&card(4)));
    assert!(state.discard.contains(&card(9)));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn countershock_splits_before_the_defenders_shield_absorbs_physical_damage() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(1), card(6)]),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "countershock".to_string(),
        cards: vec![card(4), card(9)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });
    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p1"),
            old_value: 0,
            delta: 30,
            new_value: 30,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(state.shield(&PlayerId::new("p1")), Some(18));
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 24,
            },
        ]
    );
}

#[test]
fn countershock_splits_a_generating_attack_and_both_sides_recover_hp() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hp = vec![
        TeamHp {
            team: TeamId::new("team:p1"),
            hp: 20,
        },
        TeamHp {
            team: TeamId::new("team:p2"),
            hp: 20,
        },
    ];
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5)]),
    ];
    state.last_elemental_attack_by_player.insert(
        PlayerId::new("p1"),
        LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        },
    );
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "metal-strike".to_string(),
            resolved_effect_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            resolved_turn: 1,
        },
    );
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "countershock".to_string(),
        cards: vec![card(4), card(9)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 25,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 25,
            },
        ]
    );
}

#[test]
fn countershock_finishes_as_a_draw_when_both_sides_reach_zero() {
    let mut state = GameState::from_setup(&two_player_setup_with_hp(4));
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(6)]),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "countershock".to_string(),
        cards: vec![card(4), card(9)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(
        state.status,
        GameStatus::Finished {
            outcome: GameOutcome::Draw,
        }
    );
    assert!(state.hp.iter().all(|team_hp| team_hp.hp == 0));
}

#[test]
fn seal_passive_flips_as_no_effect_against_incoming_attack_and_is_discarded() {
    let mut record = record_after_p1_covers_seal();

    let events = record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    assert_eq!(
        events.first(),
        Some(&GameEvent::PassiveFlipped {
            owner: PlayerId::new("p1"),
            incoming_player: PlayerId::new("p2"),
            passive_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            outcome: PassiveFlipOutcome::NoEffect {
                reason: PassiveNoEffectReason::NotASpell,
            },
        })
    );

    let state = record.state().clone();
    assert!(state.covered_passives.is_empty());
    assert_eq!(state.discard, vec![card(10), card(3), card(8), card(9)]);
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 22,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 30,
            },
        ]
    );
}

#[test]
fn seal_cancels_incoming_active_spell_effects_and_consumes_the_action() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(
            PlayerId::new("p2"),
            vec![card(2), card(7), card(1), card(4)],
        ),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "seal".to_string(),
        cards: vec![card(3), card(8)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "barrier".to_string(),
            cards: vec![card(2), card(7), card(1), card(4)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert_eq!(
        events,
        vec![
            GameEvent::PassiveFlipped {
                owner: PlayerId::new("p1"),
                incoming_player: PlayerId::new("p2"),
                passive_id: "seal".to_string(),
                cards: vec![card(3), card(8)],
                outcome: PassiveFlipOutcome::Applied {
                    effect_id: "seal".to_string(),
                    modifications: vec![ActionModification::CancelSpell],
                },
            },
            GameEvent::FormationPerformed {
                player: PlayerId::new("p2"),
                formation_id: "barrier".to_string(),
                used_cards: vec![card(2), card(7), card(1), card(4)],
                declared_targets: Vec::new(),
            },
        ]
    );

    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(state.shield(&PlayerId::new("p2")), Some(0));
    assert_eq!(state.phase, Phase::TurnDraw);
    assert!(state.covered_passives.is_empty());
}

#[test]
fn seal_cancels_chaos_without_leaving_a_pending_choice() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(3), card(4)]),
        PlayerHand::new(
            PlayerId::new("p2"),
            vec![card(5), card(10), card(2), card(1)],
        ),
    ];
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "seal".to_string(),
        cards: vec![card(8), card(13)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "chaos".to_string(),
            cards: vec![card(5), card(10), card(2), card(1)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert!(state.pending_choice.is_none());
    assert!(state.covered_passives.is_empty());
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::EffectChoiceRequested { .. }))
    );
    for used_card in [card(8), card(13), card(5), card(10), card(2), card(1)] {
        assert!(state.discard.contains(&used_card));
    }
}

#[test]
fn seal_marks_incoming_passive_cover_as_sealed_without_exposing_the_marker() {
    let mut record = record_after_p1_covers_seal();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "countershock".to_string(),
                cards: vec![card(4), card(9)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::PassiveFlipped {
                owner: PlayerId::new("p1"),
                incoming_player: PlayerId::new("p2"),
                passive_id: "seal".to_string(),
                cards: vec![card(3), card(8)],
                outcome: PassiveFlipOutcome::Applied {
                    effect_id: "seal".to_string(),
                    modifications: vec![ActionModification::SealCoveredPassive],
                },
            },
            GameEvent::PassiveCovered {
                player: PlayerId::new("p2"),
                formation_id: "countershock".to_string(),
                cards: vec![card(4), card(9)],
                star_substitution: None,
                sealed: true,
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(state.discard, vec![card(10), card(3), card(8)]);
    assert_eq!(
        state.covered_passives,
        vec![fewfc::domain::CoveredPassive {
            owner: PlayerId::new("p2"),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            star_substitution: None,
            sealed: true,
            covered_on_turn: 2,
            reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
        }]
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p1"))).covered_passives,
        vec![PublicCoveredPassive {
            owner: PlayerId::new("p2"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p2"))).covered_passives,
        vec![PublicCoveredPassive {
            owner: PlayerId::new("p2"),
            formation_id: Some("countershock".to_string()),
            cards: PublicCardRefs::Known(vec![card(4), card(9)]),
            star_substitution: None,
        }]
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn sealed_passive_later_flips_as_no_effect_and_is_discarded() {
    let mut record = record_after_p1_covers_seal();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::ChooseTurnDiscard {
            player: PlayerId::new("p2"),
            discard: card(13),
        })
        .unwrap();
    record.advance_automatic().unwrap();

    let events = record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    assert_eq!(
        events.first(),
        Some(&GameEvent::PassiveFlipped {
            owner: PlayerId::new("p2"),
            incoming_player: PlayerId::new("p1"),
            passive_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            outcome: PassiveFlipOutcome::NoEffect {
                reason: PassiveNoEffectReason::Sealed,
            },
        })
    );

    let state = record.state().clone();
    assert!(state.covered_passives.is_empty());
    assert!(state.discard.contains(&card(4)));
    assert!(state.discard.contains(&card(9)));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn covered_passive_state_view_shows_cards_only_to_owner() {
    let mut record = GameRecord::start(two_player_setup(), defense_setup_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let state = record.state().clone();

    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p1"))).covered_passives,
        vec![PublicCoveredPassive {
            owner: PlayerId::new("p1"),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(vec![card(2), card(7)]),
            star_substitution: None,
        }]
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p2"))).covered_passives,
        vec![PublicCoveredPassive {
            owner: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Observer).covered_passives,
        vec![PublicCoveredPassive {
            owner: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    assert_eq!(state.covered_passives[0].cards, vec![card(2), card(7)]);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn public_state_view_includes_client_state_and_filters_hands_by_viewer() {
    let record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let state = record.state().clone();

    let p1_view = public_view::state_for(&state, Viewer::Player(PlayerId::new("p1")));
    assert_eq!(p1_view.status, GameStatus::InProgress);
    assert_eq!(p1_view.turn_number, 1);
    assert_eq!(p1_view.phase, Phase::TurnStart);
    assert_eq!(p1_view.current_player, Some(PlayerId::new("p1")));
    assert_eq!(p1_view.players, two_player_setup().players);
    assert_eq!(
        p1_view.turn_order,
        vec![PlayerId::new("p1"), PlayerId::new("p2")]
    );
    assert_eq!(
        p1_view.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 30,
            },
        ]
    );
    assert_eq!(
        p1_view.hands,
        vec![
            PublicPlayerHand {
                player: PlayerId::new("p1"),
                cards: PublicCardRefs::Known(vec![card(1), card(2), card(3), card(4)]),
            },
            PublicPlayerHand {
                player: PlayerId::new("p2"),
                cards: PublicCardRefs::Hidden { count: 5 },
            },
        ]
    );
    assert_eq!(p1_view.discard, Vec::<CardInstanceId>::new());
    assert_eq!(p1_view.covered_passives, Vec::new());
    assert_eq!(p1_view.pending_choice, None);
    assert_eq!(
        p1_view.shields,
        vec![
            PlayerShield {
                player: PlayerId::new("p1"),
                value: 0,
            },
            PlayerShield {
                player: PlayerId::new("p2"),
                value: 0,
            },
        ]
    );
    assert_eq!(p1_view.statuses, Vec::<StatusEffect>::new());

    let p2_view = public_view::state_for(&state, Viewer::Player(PlayerId::new("p2")));
    assert_eq!(
        p2_view.hands,
        vec![
            PublicPlayerHand {
                player: PlayerId::new("p1"),
                cards: PublicCardRefs::Hidden { count: 4 },
            },
            PublicPlayerHand {
                player: PlayerId::new("p2"),
                cards: PublicCardRefs::Known(vec![card(5), card(6), card(7), card(8), card(9)]),
            },
        ]
    );

    let observer_view = public_view::state_for(&state, Viewer::Observer);
    assert_eq!(
        observer_view.hands,
        vec![
            PublicPlayerHand {
                player: PlayerId::new("p1"),
                cards: PublicCardRefs::Hidden { count: 4 },
            },
            PublicPlayerHand {
                player: PlayerId::new("p2"),
                cards: PublicCardRefs::Hidden { count: 5 },
            },
        ]
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn public_state_view_exposes_only_the_previous_turns_formation() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.turn_number = 3;
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "metal-strike".to_string(),
            resolved_effect_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            resolved_turn: 1,
        },
    );
    state.last_formation_by_player.insert(
        PlayerId::new("p2"),
        LastFormationUse {
            formation_id: "wood-strike".to_string(),
            resolved_effect_id: "wood-strike".to_string(),
            used_cards: vec![card(2)],
            resolved_turn: 2,
        },
    );

    assert_eq!(
        public_view::state_for(&state, Viewer::Observer).previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: PlayerId::new("p2"),
            formation_id: Some("wood-strike".to_string()),
            cards: PublicCardRefs::Known(vec![card(2)]),
        })
    );
}

#[test]
fn previous_turn_covered_formation_hides_details_from_other_players() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.turn_number = 2;
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "defense".to_string(),
            resolved_effect_id: "defense".to_string(),
            used_cards: vec![card(2), card(7)],
            resolved_turn: 1,
        },
    );
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p1"),
        formation_id: "defense".to_string(),
        cards: vec![card(2), card(7)],
        star_substitution: None,
        sealed: false,
        covered_on_turn: 1,
        reveal_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
    });

    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p1"))).previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: PlayerId::new("p1"),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(vec![card(2), card(7)]),
        })
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p2"))).previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
        })
    );
}

#[test]
fn initial_deal_event_view_filters_cards_to_dealt_player() {
    let event = GameEvent::CardsDealt {
        player: PlayerId::new("p1"),
        cards: vec![card(1), card(2), card(3), card(4)],
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: PublicCardRefs::Known(vec![card(1), card(2), card(3), card(4)]),
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: PublicCardRefs::Hidden { count: 4 },
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: PublicCardRefs::Hidden { count: 4 },
        }
    );
    assert_eq!(
        event,
        GameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: vec![card(1), card(2), card(3), card(4)],
        }
    );
}

#[test]
fn deck_prepared_event_view_hides_deck_order_for_every_viewer() {
    let event = GameEvent::DeckPrepared {
        deck_order: vec![card(1), card(2), card(3)],
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::DeckPrepared {
            deck: PublicCardRefs::Hidden { count: 3 },
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::DeckPrepared {
            deck: PublicCardRefs::Hidden { count: 3 },
        }
    );
    assert_eq!(
        event,
        GameEvent::DeckPrepared {
            deck_order: vec![card(1), card(2), card(3)],
        }
    );
}

#[test]
fn pending_effect_choice_state_view_shows_options_only_to_choice_player() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 2, 1])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "chaos".to_string(),
            cards: vec![card(5), card(10), card(2), card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let state = record.state().clone();

    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p1"))).pending_choice,
        Some(PublicPendingChoice {
            player: PlayerId::new("p1"),
            purpose: "chaos".to_string(),
            presentation: PublicPendingChoicePresentation::Chaos,
            kind: PublicPendingChoiceKind::Known(PendingChoiceKind::EffectGenerated {
                effect_id: "chaos".to_string(),
                continuation_id: "chaos:return-two".to_string(),
                allowed_cards: vec![card(3), card(4), card(6), card(7), card(8)],
            }),
        })
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p2"))).pending_choice,
        Some(PublicPendingChoice {
            player: PlayerId::new("p1"),
            purpose: "chaos".to_string(),
            presentation: PublicPendingChoicePresentation::Chaos,
            kind: PublicPendingChoiceKind::Hidden,
        })
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Observer).pending_choice,
        Some(PublicPendingChoice {
            player: PlayerId::new("p1"),
            purpose: "chaos".to_string(),
            presentation: PublicPendingChoicePresentation::Chaos,
            kind: PublicPendingChoiceKind::Hidden,
        })
    );
    assert_eq!(
        state.pending_choice,
        Some(PendingChoice {
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::EffectGenerated {
                effect_id: "chaos".to_string(),
                continuation_id: "chaos:return-two".to_string(),
                allowed_cards: vec![card(3), card(4), card(6), card(7), card(8)],
            },
        })
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn passive_cover_event_view_filters_hidden_card_ids_without_changing_canonical_event() {
    let event = GameEvent::PassiveCovered {
        player: PlayerId::new("p1"),
        formation_id: "defense".to_string(),
        cards: vec![card(2), card(7)],
        star_substitution: None,
        sealed: true,
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(vec![card(2), card(7)]),
            star_substitution: None,
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }
    );
    assert_eq!(
        event,
        GameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            star_substitution: None,
            sealed: true,
        }
    );
}

#[test]
fn effect_choice_event_view_filters_options_and_preserves_canonical_continuation() {
    let event = GameEvent::EffectChoiceRequested {
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::EffectGenerated {
            effect_id: "metamorphosis".to_string(),
            continuation_id: "metamorphosis:choose-card".to_string(),
            allowed_cards: vec![card(1), card(2)],
        },
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::EffectChoiceRequested {
            player: PlayerId::new("p1"),
            purpose: "metamorphosis".to_string(),
            kind: PublicPendingChoiceKind::Known(PendingChoiceKind::EffectGenerated {
                effect_id: "metamorphosis".to_string(),
                continuation_id: "metamorphosis:choose-card".to_string(),
                allowed_cards: vec![card(1), card(2)],
            }),
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::EffectChoiceRequested {
            player: PlayerId::new("p1"),
            purpose: "metamorphosis".to_string(),
            kind: PublicPendingChoiceKind::Hidden,
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::EffectChoiceRequested {
            player: PlayerId::new("p1"),
            purpose: "metamorphosis".to_string(),
            kind: PublicPendingChoiceKind::Hidden,
        }
    );
    assert_eq!(
        event,
        GameEvent::EffectChoiceRequested {
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::EffectGenerated {
                effect_id: "metamorphosis".to_string(),
                continuation_id: "metamorphosis:choose-card".to_string(),
                allowed_cards: vec![card(1), card(2)],
            },
        }
    );
}

#[test]
fn turn_draw_choice_event_view_filters_choice_options_to_choice_player() {
    let event = GameEvent::CardsDrawnForTurnDiscardChoice {
        player: PlayerId::new("p1"),
        drawn_cards: vec![card(10), card(11), card(12)],
        allowed_discards: vec![card(10), card(11), card(12)],
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("p1"),
            drawn_cards: PublicCardRefs::Known(vec![card(10), card(11), card(12)]),
            allowed_discards: PublicCardRefs::Known(vec![card(10), card(11), card(12)]),
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("p1"),
            drawn_cards: PublicCardRefs::Hidden { count: 3 },
            allowed_discards: PublicCardRefs::Hidden { count: 3 },
        }
    );
    assert_eq!(
        event,
        GameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("p1"),
            drawn_cards: vec![card(10), card(11), card(12)],
            allowed_discards: vec![card(10), card(11), card(12)],
        }
    );
}

#[test]
fn record_event_feed_is_viewer_filtered_and_canonical_events_remain_replay_source() {
    let mut record = GameRecord::start(two_player_setup(), defense_setup_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    assert_eq!(
        record
            .public_events_for(Viewer::Player(PlayerId::new("p2")))
            .last(),
        Some(&PublicGameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        })
    );
    assert_eq!(
        record.events().last(),
        Some(&GameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            star_substitution: None,
            sealed: false,
        })
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
}

#[test]
fn invalid_perform_formation_commands_leave_events_and_state_unchanged() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let turn_start_events = record.events().to_vec();
    let turn_start_state = record.state().clone();

    assert_eq!(
        record.handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        }),
        Err(GameError::Validation(ValidationError::WrongPhase {
            expected: Phase::Main,
            actual: Phase::TurnStart,
        }))
    );
    assert_eq!(record.events(), turn_start_events.as_slice());
    assert_eq!(record.state().clone(), turn_start_state);

    record.advance_automatic().unwrap();

    let cases = [
        (
            Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(5)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::WrongPlayer {
                expected: PlayerId::new("p1"),
                actual: PlayerId::new("p2"),
            }),
        ),
        (
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "missing".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::UnknownFormation("missing".to_string())),
        ),
        (
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "weapon".to_string(),
                cards: vec![card(1), card(1)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::DuplicateSubmittedCard(card(1))),
        ),
        (
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(5)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::CardNotInHand(card(5))),
        ),
        (
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1), card(2)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::FormationPatternMismatch {
                formation_id: "metal-strike".to_string(),
            }),
        ),
    ];

    for (command, expected_error) in cases {
        let events_before = record.events().to_vec();
        let state_before = record.state().clone();

        assert_eq!(record.handle(command), Err(expected_error));
        assert_eq!(record.events(), events_before.as_slice());
        assert_eq!(record.state().clone(), state_before);
    }
}

#[test]
fn perform_formation_matches_cards_by_instance_definitions() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[1, 6, 2, 3])).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "weapon".to_string(),
                cards: vec![card(1), card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p1"),
            target: PlayerId::new("p2"),
            formation_id: "weapon".to_string(),
            used_cards: vec![card(1), card(6)],
            point_breakdown: AttackPointBreakdown {
                base_points: 12,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 12,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 30,
                delta: -12,
                new_hp: 18,
                effective_delta: -12,
            },
            shield_change: None,
            card_moves: vec![
                CardMoveDelta {
                    card: card(1),
                    from: CardZone::Hand(PlayerId::new("p1")),
                    to: CardZone::Discard,
                },
                CardMoveDelta {
                    card: card(6),
                    from: CardZone::Hand(PlayerId::new("p1")),
                    to: CardZone::Discard,
                },
            ],
            elemental_context_update: None,
        }]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(2), card(3)].as_slice())
    );
    assert_eq!(state.discard, vec![card(1), card(6)]);
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 18,
            },
        ]
    );
    assert!(state.last_elemental_attack_by_player.is_empty());
}

#[test]
fn attack_hp_delta_records_clamped_damage() {
    let mut record = GameRecord::start(two_player_setup_with_hp(5), official_deck()).unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p1"),
            target: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            point_breakdown: AttackPointBreakdown {
                base_points: 7,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 7,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 5,
                delta: -7,
                new_hp: 0,
                effective_delta: -5,
            },
            shield_change: None,
            card_moves: vec![CardMoveDelta {
                card: card(1),
                from: CardZone::Hand(PlayerId::new("p1")),
                to: CardZone::Discard,
            }],
            elemental_context_update: Some(LastElementalAttackUpdate {
                player: PlayerId::new("p1"),
                attack: LastElementalAttack {
                    element: Element::Metal,
                    resolved_turn: 1,
                },
            }),
        }]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 5,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 0,
            },
        ]
    );
}

#[test]
fn attack_that_reduces_a_team_to_zero_finishes_game_with_opposing_team_winner() {
    let mut record = GameRecord::start(two_player_setup_with_hp(5), official_deck()).unwrap();
    record.advance_automatic().unwrap();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert_eq!(
        state.status,
        GameStatus::Finished {
            outcome: GameOutcome::Team(TeamId::new("team:p1")),
        }
    );
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 5,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 0,
            },
        ]
    );
}

#[test]
fn commands_after_game_over_are_rejected_without_events_or_state_changes() {
    let mut record = GameRecord::start(two_player_setup_with_hp(5), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(
        record.handle(Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::CannotActByStatus,
        }),
        Err(GameError::Validation(ValidationError::GameFinished))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn automatic_advance_stops_after_game_over() {
    let mut record = GameRecord::start(two_player_setup_with_hp(5), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(record.advance_automatic().unwrap(), Vec::<GameEvent>::new());
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn hp_resolution_finishes_as_draw_when_no_team_remains_alive() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.hp = vec![
        TeamHp {
            team: TeamId::new("team:p1"),
            hp: 0,
        },
        TeamHp {
            team: TeamId::new("team:p2"),
            hp: 1,
        },
    ];

    apply_event(
        &mut state,
        &GameEvent::AttackResolved {
            attacker: PlayerId::new("p1"),
            target: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            used_cards: Vec::new(),
            point_breakdown: AttackPointBreakdown {
                base_points: 7,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 7,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 1,
                delta: -7,
                new_hp: 0,
                effective_delta: -1,
            },
            shield_change: None,
            card_moves: Vec::new(),
            elemental_context_update: None,
        },
    );

    assert_eq!(
        state.status,
        GameStatus::Finished {
            outcome: GameOutcome::Draw,
        }
    );
}

#[test]
fn elemental_attack_overcoming_previous_players_last_element_doubles_damage() {
    let mut record = record_after_p1_metal_attack_on_turn_1();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p2"),
            target: PlayerId::new("p1"),
            formation_id: "fire-strike".to_string(),
            used_cards: vec![card(9)],
            point_breakdown: AttackPointBreakdown {
                base_points: 8,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::Overcoming,
                damage_transform: DamageTransform::DoubleDamage,
                final_amount: 16,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p1"),
                old_hp: 30,
                delta: -16,
                new_hp: 14,
                effective_delta: -16,
            },
            shield_change: None,
            card_moves: vec![CardMoveDelta {
                card: card(9),
                from: CardZone::Hand(PlayerId::new("p2")),
                to: CardZone::Discard,
            }],
            elemental_context_update: Some(LastElementalAttackUpdate {
                player: PlayerId::new("p2"),
                attack: LastElementalAttack {
                    element: Element::Fire,
                    resolved_turn: 2,
                },
            }),
        }]
    );
}

#[test]
fn elemental_attack_generating_previous_players_last_element_heals_target_team() {
    let mut record = record_after_p1_metal_attack_on_turn_1();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "earth-strike".to_string(),
                cards: vec![card(5)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p2"),
            target: PlayerId::new("p1"),
            formation_id: "earth-strike".to_string(),
            used_cards: vec![card(5)],
            point_breakdown: AttackPointBreakdown {
                base_points: 9,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::Generating,
                damage_transform: DamageTransform::HealTarget,
                final_amount: 9,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p1"),
                old_hp: 30,
                delta: 9,
                new_hp: 30,
                effective_delta: 0,
            },
            shield_change: None,
            card_moves: vec![CardMoveDelta {
                card: card(5),
                from: CardZone::Hand(PlayerId::new("p2")),
                to: CardZone::Discard,
            }],
            elemental_context_update: Some(LastElementalAttackUpdate {
                player: PlayerId::new("p2"),
                attack: LastElementalAttack {
                    element: Element::Earth,
                    resolved_turn: 2,
                },
            }),
        }]
    );
}

#[test]
fn elemental_attack_same_as_previous_players_last_element_halves_damage_rounding_up() {
    let mut record = record_after_p1_metal_attack_on_turn_1();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p2"),
            target: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            used_cards: vec![card(6)],
            point_breakdown: AttackPointBreakdown {
                base_points: 7,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::Same,
                damage_transform: DamageTransform::HalfDamageRoundUp,
                final_amount: 4,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p1"),
                old_hp: 30,
                delta: -4,
                new_hp: 26,
                effective_delta: -4,
            },
            shield_change: None,
            card_moves: vec![CardMoveDelta {
                card: card(6),
                from: CardZone::Hand(PlayerId::new("p2")),
                to: CardZone::Discard,
            }],
            elemental_context_update: Some(LastElementalAttackUpdate {
                player: PlayerId::new("p2"),
                attack: LastElementalAttack {
                    element: Element::Metal,
                    resolved_turn: 2,
                },
            }),
        }]
    );
}

#[test]
fn elemental_attack_without_relationship_to_previous_players_last_element_uses_normal_damage() {
    let mut record = record_after_p1_metal_attack_on_turn_1();

    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(7)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p2"),
            target: PlayerId::new("p1"),
            formation_id: "wood-strike".to_string(),
            used_cards: vec![card(7)],
            point_breakdown: AttackPointBreakdown {
                base_points: 6,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 6,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p1"),
                old_hp: 30,
                delta: -6,
                new_hp: 24,
                effective_delta: -6,
            },
            shield_change: None,
            card_moves: vec![CardMoveDelta {
                card: card(7),
                from: CardZone::Hand(PlayerId::new("p2")),
                to: CardZone::Discard,
            }],
            elemental_context_update: Some(LastElementalAttackUpdate {
                player: PlayerId::new("p2"),
                attack: LastElementalAttack {
                    element: Element::Wood,
                    resolved_turn: 2,
                },
            }),
        }]
    );
}

#[test]
fn elemental_interaction_ignores_an_older_element_when_the_previous_formation_was_not_elemental() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::Main;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(9)]),
    ];
    state.last_elemental_attack_by_player.insert(
        PlayerId::new("p1"),
        LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        },
    );
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "weapon".to_string(),
            resolved_effect_id: "weapon".to_string(),
            used_cards: vec![card(1), card(6)],
            resolved_turn: 1,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(22)
    );
}

#[test]
fn shield_absorbs_attack_damage_before_hp_and_skips_element_interaction() {
    let mut state = record_after_p1_metal_attack_on_turn_1().state().clone();
    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p1"),
            old_value: 0,
            delta: 3,
            new_value: 3,
        },
    );

    assert_eq!(
        handle_command(
            &state,
            Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
                declared_targets: Vec::new(),
            },
        )
        .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p2"),
            target: PlayerId::new("p1"),
            formation_id: "fire-strike".to_string(),
            used_cards: vec![card(9)],
            point_breakdown: AttackPointBreakdown {
                base_points: 8,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 8,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p1"),
                old_hp: 30,
                delta: 0,
                new_hp: 30,
                effective_delta: 0,
            },
            shield_change: Some(ShieldChangeDelta {
                player: PlayerId::new("p1"),
                old_value: 3,
                delta: -8,
                new_value: 0,
            }),
            card_moves: vec![CardMoveDelta {
                card: card(9),
                from: CardZone::Hand(PlayerId::new("p2")),
                to: CardZone::Discard,
            }],
            elemental_context_update: Some(LastElementalAttackUpdate {
                player: PlayerId::new("p2"),
                attack: LastElementalAttack {
                    element: Element::Fire,
                    resolved_turn: 2,
                },
            }),
        }]
    );

    for event in handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.shield(&PlayerId::new("p1")), Some(0));
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 23,
            },
        ]
    );
}

#[test]
fn shield_change_replaces_existing_player_shield_amount() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);

    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p1"),
            old_value: 0,
            delta: 3,
            new_value: 3,
        },
    );
    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p1"),
            old_value: 3,
            delta: 2,
            new_value: 5,
        },
    );

    assert_eq!(state.shield(&PlayerId::new("p1")), Some(5));
}

#[test]
fn physical_attack_deals_double_damage_to_a_player_shield() {
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[2, 7, 1, 4, 6, 11, 3, 5, 8]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "barrier".to_string(),
            cards: vec![card(2), card(7), card(1), card(4)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(9));

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "weapon".to_string(),
            cards: vec![card(6), card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert_eq!(state.shield(&PlayerId::new("p1")), Some(20));
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(30)
    );
    assert_eq!(record.replay().unwrap(), state);
}
