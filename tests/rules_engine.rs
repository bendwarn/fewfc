// 單一事件預期維持為 Vec<GameEvent>，以符合事件日誌斷言輔助工具。
#![allow(clippy::useless_vec)]

use fewfc::application::{
    AutomaticReason, CommandContext, CommandKind, EventSource, GameRecord, StartGame,
    advance_automatic as advance_state_automatic, apply_event, handle_command,
};
use fewfc::domain::{
    ActionModification, AttackPointBreakdown, AttackResolutionEffects,
    CannotPerformFormationReason, CardDef, CardDefId, CardInstanceDef, CardInstanceId,
    CardMoveDelta, CardZone, ChoiceAnswer, ChoiceContinuation, ChoiceId, Command, CommandId,
    DamageTransform, ElementInteraction, EngineInvariantError, EnvironmentAttackEffect,
    FormationAreaState, FormationInArea, GameConclusion, GameEndCause, GameError, GameEvent,
    GameOutcome, GameSetup, GameState, GameStatus, HpChangeDelta, LastElementalAttack,
    LastElementalAttackUpdate, LastFormationUse, PassActionReason, PassiveFlipOutcome,
    PassiveNoEffectGround, PendingChoice, PendingChoiceKind, Phase, Player, PlayerFormationArea,
    PlayerHand, PlayerId, PlayerShield, RuleModuleId, RulesetId, ShieldChangeDelta, StatusDuration,
    StatusEffect, StatusExpiryTiming, StatusOwner, TeamHp, TeamId, TurnDrawSkipReason,
    ValidationError,
};
use fewfc::public_view::{
    self, PublicCardRefs, PublicCoveredPassive, PublicGameEvent, PublicPendingChoice,
    PublicPendingChoicePresentation, PublicPlayerHand, PublicPreviousTurnFormation, Viewer,
};
use fewfc::rules::Element;

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

macro_rules! atomic_context {
    ($update:expr) => {
        Some(AttackResolutionEffects::with_elemental_context($update))
    };
}

/// 舊行為斷言描述命令的語意效果，而不是新的區域/終止生命週期事實。兩側都
/// 比較該不變量視圖，生命週期本身則由下方的專用測試斷言。
fn semantic_events(events: &[GameEvent]) -> Vec<GameEvent> {
    let mut semantic = Vec::new();
    for event in events {
        match event {
            GameEvent::FormationCommitted { .. }
            | GameEvent::FormationCardsDiscarded { .. }
            | GameEvent::FormationPerformed { .. }
            | GameEvent::ActionStarted { .. }
            | GameEvent::GameEnded { .. } => continue,
            GameEvent::TurnDrawResolved {
                player, discard, ..
            } => semantic.push(GameEvent::TurnDiscardChosen {
                player: player.clone(),
                discard: *discard,
            }),
            GameEvent::AttackResolved { .. } => {
                let mut attack = event.clone();
                let mut environment_transfers = Vec::new();
                if let GameEvent::AttackResolved {
                    card_moves,
                    elemental_context_update,
                    ..
                } = &mut attack
                {
                    // 陣形卡牌現在會經過陣形區，而不是使用攻擊舊有的手牌到棄牌堆
                    // 負載。
                    card_moves.clear();
                    if let Some(effects) = elemental_context_update {
                        effects.outcome = Default::default();
                        environment_transfers = std::mem::take(&mut effects.environment_transfers)
                            .into_iter()
                            .map(|transfer| GameEvent::EnvironmentTransferred {
                                player: transfer.player,
                                formation_id: transfer.formation_id,
                                from: transfer.from,
                                to: transfer.to,
                            })
                            .collect();
                        if effects.is_empty() {
                            *elemental_context_update = None;
                        }
                    }
                }
                semantic.push(attack);
                semantic.extend(environment_transfers);
            }
            _ => semantic.push(event.clone()),
        }
    }
    semantic
}

macro_rules! assert_event_semantics_eq {
    ($actual:expr, $expected:expr $(,)?) => {
        assert_eq!(semantic_events(&$actual), semantic_events(&$expected));
    };
}

fn cover(
    state: &mut GameState,
    owner: &str,
    formation_id: &str,
    cards: Vec<CardInstanceId>,
    sealed: bool,
) {
    state
        .formation_area_mut(&PlayerId::new(owner))
        .unwrap()
        .formation = Some(FormationInArea {
        formation_id: formation_id.to_string(),
        cards,
        star_substitution: None,
        state: FormationAreaState::FaceDownWaiting {
            sealed,
            revealed: false,
            neutralized: false,
            trigger_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
        },
    });
}

fn no_covered(state: &GameState) -> bool {
    state
        .formation_areas
        .iter()
        .all(|area| area.formation.is_none())
}

fn card_def(id: &str, element: Element) -> CardDef {
    CardDef {
        id: CardDefId::new(id),
        name: id.to_string(),
        element,
        level: fewfc::domain::PrintedCardLevel::new(match id {
            "metal" => 3,
            "wood" => 2,
            "water" => 1,
            "fire" => 4,
            "earth" => 5,
            _ => 1,
        }),
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

fn advance_record_to_next_main_after_turn_draw(
    record: &mut GameRecord,
    discard: CardInstanceId,
) -> Vec<GameEvent> {
    let mut events = record.advance_automatic().unwrap();
    events.extend(
        answer_record_choice(
            record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap(),
    );
    events.extend(record.advance_automatic().unwrap());
    events
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
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
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
fn elemental_interaction_ignores_an_elemental_attack_from_an_older_turn() {
    let mut state = GameState::from_setup(&two_player_setup_with_hp(100));
    state.phase = Phase::ActiveEffects;
    state.turn_number = 3;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(5)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    state.last_formation_by_player.insert(
        PlayerId::new("p2"),
        LastFormationUse {
            formation_id: "metal-strike".to_string(),
            resolved_effect_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            resolved_turn: 1,
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
        semantic_events(&events).as_slice(),
        [GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown {
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 9,
                ..
            },
            hp_change: HpChangeDelta { new_hp: 91, .. },
            ..
        }]
    ));
}

#[test]
fn matching_environment_damage_stacks_with_overcoming_interaction() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
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
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
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
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
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
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
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
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
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
    state.phase = Phase::ActiveEffects;
    state.hands = vec![
        PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(6), card(11), card(16), card(21)],
        ),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    cover(&mut state, "p2", "defense", vec![card(2), card(7)], false);

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

    let semantic = semantic_events(&events);
    assert!(matches!(
        semantic.as_slice(),
        [
            GameEvent::PassiveFlipped {
                outcome: PassiveFlipOutcome::NoEffect { .. },
                ..
            },
            GameEvent::AttackResolved {
                hp_change: HpChangeDelta { new_hp: 119, .. },
                ..
            },
            GameEvent::EnvironmentTransferred { .. },
        ]
    ));
    assert!(matches!(
        &semantic[0],
        GameEvent::PassiveFlipped {
            outcome: PassiveFlipOutcome::NoEffect { grounds },
            ..
        } if grounds == &vec![PassiveNoEffectGround::IgnoredBySacredBeast]
    ));
}

#[test]
fn ineffective_barrier_is_performed_without_replacing_an_existing_shield() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
        [GameEvent::FormationEffectIgnored {
            reason: fewfc::domain::FormationNoEffectReason::IneffectiveInEnvironment {
                environment: Element::Metal,
            },
            ..
        }]
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
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
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
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.environment = Some(Element::Metal);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(1)]),
    ];
    cover(&mut state, "p1", "defense", vec![card(2), card(7)], false);

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

    let semantic = semantic_events(&events);
    assert!(matches!(
        semantic.as_slice(),
        [
            GameEvent::PassiveFlipped {
                outcome: PassiveFlipOutcome::NoEffect { .. },
                ..
            },
            GameEvent::AttackResolved {
                hp_change: HpChangeDelta { new_hp: 86, .. },
                ..
            },
        ]
    ));
    assert!(matches!(
        &semantic[0],
        GameEvent::PassiveFlipped {
            outcome: PassiveFlipOutcome::NoEffect { grounds },
            ..
        } if grounds == &vec![PassiveNoEffectGround::IneffectiveInEnvironment {
            environment: Element::Metal,
        }]
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
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
        [GameEvent::EnvironmentCleared {
                player,
                formation_id,
                environment: Element::Fire,
                hp_changes,
            }] if player == &PlayerId::new("p1")
            && formation_id == "void-meridian-severing"
            && hp_changes.len() == 2
            && hp_changes.iter().all(|change| change.new_hp == 0)
    ));

    for event in &events {
        apply_event(&mut state, event);
    }
    assert_eq!(state.environment, None);
    assert!(matches!(
        state.status,
        GameStatus::Finished { ref conclusion } if conclusion.outcome == GameOutcome::Draw
    ));
}

#[test]
fn void_meridian_severing_without_environment_does_not_change_hp() {
    let setup = two_player_setup_with_hp(100)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
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

    assert!(semantic_events(&events).is_empty());
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
    state.phase = Phase::ActiveEffects;
    state.environment = Some(Element::Fire);
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(6), card(11)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    cover(&mut state, "p2", "seal", vec![card(3), card(8)], false);

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
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).as_slice(),
        [GameEvent::EnvironmentCleared { hp_changes, .. }] if hp_changes.len() == 2
            && hp_changes.iter().all(|change| change.old_hp == 250 && change.new_hp == 230)
    ));
}

#[test]
fn same_element_sacred_beast_still_records_environment_transfer() {
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.push(card_instance(21, "metal"));
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
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
        semantic_events(&events).last(),
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
            level: fewfc::domain::PrintedCardLevel::new(3),
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
        semantic_events(command_events.events()).as_slice(),
        [GameEvent::AttackResolved { .. }]
    ));

    let view = record.public_view(Viewer::Observer).unwrap();
    assert_eq!(view.phase, Phase::TurnDraw);
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn base_start_turn_lifecycle_matrix_commits_an_action_then_resolves_draw_choice_and_next_turn() {
    // 這是基礎規則開始邊界的命令層級生命週期矩陣。固定牌堆順序僅是背景：Start
    // 之後的每個轉換都是實際命令或自動標準轉換。
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    assert_eq!(
        record.events(),
        [
            GameEvent::DeckPrepared {
                deck_order: official_deck(),
            },
            GameEvent::CardsDealt {
                player: p1.clone(),
                cards: vec![card(1), card(2), card(3), card(4)],
            },
            GameEvent::CardsDealt {
                player: p2.clone(),
                cards: vec![card(5), card(6), card(7), card(8), card(9)],
            },
        ]
    );

    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![GameEvent::TurnStarted {
            player: p1.clone(),
            turn_number: 1,
        }]
    );
    let action = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        action.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                state: FormationAreaState::FaceUpResolving,
                ..
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                formation_id: attack_formation,
                point_breakdown: AttackPointBreakdown { final_amount: 7, .. },
                hp_change: HpChangeDelta { old_hp: 30, delta: -7, new_hp: 23, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded,
                cards: discarded_cards,
            },
        ] if player == &p1
            && formation_id == "metal-strike"
            && cards == &vec![card(1)]
            && attacker == &p1
            && target == &p2
            && attack_formation == "metal-strike"
            && discarded_by == &p1
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(1)]
    ));
    assert_eq!(record.state().phase, Phase::TurnDraw);
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(23)
    );
    assert!(record.state().discard.contains(&card(1)));

    let draw_events = record.advance_automatic().unwrap();
    let (choice_id, drawn_cards) = match draw_events.as_slice() {
        [
            GameEvent::CardsDrawnForTurnDiscardChoice {
                player,
                drawn_cards,
                allowed_discards,
            },
            GameEvent::ChoiceRequested { choice },
        ] if player == &p1 && drawn_cards == allowed_discards && choice.player == p1 => {
            (choice.choice_id, drawn_cards.clone())
        }
        _ => panic!("expected canonical turn-draw Card choice, got {draw_events:?}"),
    };
    assert_eq!(drawn_cards, vec![card(10), card(11), card(12)]);

    let chosen_discard = card(12);
    assert_eq!(
        record
            .handle(Command::AnswerChoice {
                player: p1.clone(),
                choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![chosen_discard],
                },
            })
            .unwrap(),
        vec![
            GameEvent::ChoiceMade {
                player: p1.clone(),
                choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![chosen_discard],
                },
            },
            GameEvent::TurnDrawResolved {
                player: p1.clone(),
                discard: chosen_discard,
                kept_cards: vec![card(10), card(11)],
            },
        ]
    );
    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![
            GameEvent::TurnEnded { player: p1.clone() },
            GameEvent::TurnStarted {
                player: p2.clone(),
                turn_number: 2,
            },
        ]
    );
    assert_eq!(record.state().current_player(), Some(&p2));
    assert_eq!(record.state().phase, Phase::ActiveEffects);
    assert_eq!(
        record.state().hand(&p1),
        Some(vec![card(2), card(3), card(4), card(10), card(11)].as_slice())
    );
    assert!(record.state().discard.contains(&chosen_discard));
    assert_eq!(record.replay().unwrap(), record.state().clone());
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

    assert_event_semantics_eq!(
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
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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
    state.phase = Phase::ActiveEffects;
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
    assert!(no_covered(&state));
    assert!(state.statuses.is_empty());
}

#[test]
fn pending_choices_store_typed_continuations_and_ids() {
    let choice = PendingChoice {
        choice_id: ChoiceId::new(7),
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::Card {
            cards: vec![card(1), card(2)],
            minimum: 1,
            maximum: 1,
            can_decline: false,
        },
        continuation: ChoiceContinuation::Base(
            fewfc::domain::BaseChoiceContinuation::ChaosReturnTwo,
        ),
    };

    assert_eq!(choice.clone(), choice);
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
    state.phase = Phase::ActiveEffects;
    cover(&mut state, "p1", "defense", vec![card(2), card(7)], false);
    cover(&mut state, "p1", "seal", vec![card(3), card(8)], false);
    state.formation_areas.push(PlayerFormationArea {
        player: PlayerId::new("p1"),
        formation: state
            .formation_area(&PlayerId::new("p1"))
            .unwrap()
            .formation
            .clone(),
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
            EngineInvariantError::DuplicateFormationArea {
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
    assert_eq!(state.phase, Phase::ActiveEffects);
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
    assert_eq!(state.phase, Phase::ActiveEffects);
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
    assert_event_semantics_eq!(
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

    let result = record.handle(Command::AnswerChoice {
        player: PlayerId::new("p1"),
        choice_id: ChoiceId::new(1),
        answer: ChoiceAnswer::Cards {
            cards: vec![card(1)],
        },
    });

    assert_eq!(
        result,
        Err(GameError::Validation(ValidationError::MissingPendingChoice))
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
    assert_eq!(state.phase, Phase::ActiveEffects);
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

    assert_event_semantics_eq!(
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

    assert_event_semantics_eq!(
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
                grounds,
            },
            ..
        } if owner == &PlayerId::new("p1")
            && passive_id == "defense"
            && grounds == &vec![PassiveNoEffectGround::NotAnAttack]
    )));
    assert!(no_covered(&state));
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
        vec![
            GameEvent::CardsDrawnForTurnDiscardChoice {
                player: PlayerId::new("p1"),
                drawn_cards: vec![card(10), card(11)],
                allowed_discards: vec![card(10), card(11)],
            },
            GameEvent::ChoiceRequested {
                choice: PendingChoice {
                    choice_id: ChoiceId::new(1),
                    player: PlayerId::new("p1"),
                    kind: PendingChoiceKind::Card {
                        cards: vec![card(10), card(11)],
                        minimum: 1,
                        maximum: 1,
                        can_decline: false,
                    },
                    continuation: ChoiceContinuation::Base(
                        fewfc::domain::BaseChoiceContinuation::TurnDrawDiscard,
                    ),
                },
            }
        ]
    );

    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(1), card(2), card(3), card(4)].as_slice())
    );
    assert_eq!(state.turn_draw_pool, vec![card(10), card(11)]);
    assert_eq!(state.deck, (12..=20).map(card).collect::<Vec<_>>());
    assert_eq!(
        state.pending_choice,
        Some(PendingChoice {
            choice_id: ChoiceId::new(1),
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::Card {
                cards: vec![card(10), card(11)],
                minimum: 1,
                maximum: 1,
                can_decline: false,
            },
            continuation: ChoiceContinuation::Base(
                fewfc::domain::BaseChoiceContinuation::TurnDrawDiscard,
            ),
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
        answer_choice(
            &state,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(10)]
            },
        )
        .unwrap(),
        vec![
            GameEvent::ChoiceMade {
                player: PlayerId::new("p1"),
                choice_id: ChoiceId::new(1),
                answer: ChoiceAnswer::Cards {
                    cards: vec![card(10)]
                },
            },
            GameEvent::TurnDrawResolved {
                player: PlayerId::new("p1"),
                discard: card(10),
                kept_cards: vec![card(11)],
            },
        ]
    );

    for event in answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: vec![card(10)],
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
        answer_choice(
            &state,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(2)]
            },
        ),
        Err(GameError::Validation(ValidationError::InvalidChoiceAnswer))
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
    for event in answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: vec![card(10)],
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

    assert_eq!(state.phase, Phase::ActiveEffects);
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
    for event in answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: vec![card(10)],
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

    assert_eq!(state.phase, Phase::ActiveEffects);
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

    let events = advance_state_automatic(&state).unwrap();
    assert!(matches!(
        semantic_events(&events).as_slice(),
        [GameEvent::RandomnessRequested { request }]
            if request.operation.is_discard_shuffle()
                && request.current_order == vec![card(2)]
    ));
}

#[test]
fn perform_attack_formation_damages_previous_players_team_and_moves_cards_to_discard() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();

    assert_event_semantics_eq!(
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
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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
            kind: PendingChoiceKind::Card { cards, .. },
            ..
        }) if player == &PlayerId::new("p2") && cards.len() == 4
    ));
    answer_record_choice(
        &mut record,
        PlayerId::new("p2"),
        ChoiceAnswer::Cards {
            cards: vec![card(10)],
        },
    )
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

    assert_event_semantics_eq!(
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

    assert_event_semantics_eq!(
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

    assert_event_semantics_eq!(
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
    state.phase = Phase::ActiveEffects;
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

    assert_event_semantics_eq!(
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
    state.phase = Phase::ActiveEffects;
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

    assert_event_semantics_eq!(
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
fn radiance_cannot_act_matrix_blocks_a_usable_formation_but_keeps_status_specific_pass_legal() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：P1 完成普通回合後，P2 擁有合法的 Wood Strike。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 4, 3, 2, 5, 7, 8, 9]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let baseline_events = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "wood-strike".to_string(),
            cards: vec![card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        baseline_events
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：Radiance 透過完整陣形命令建立；P2 仍持有相同可用的木卡，但只能
    // 執行狀態專屬的 Pass 行動。
    let mut radiance = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 4, 3, 2, 5, 7, 8, 9]),
    )
    .unwrap();
    radiance.advance_automatic().unwrap();
    let radiance_cards = vec![card(1), card(6), card(4), card(3)];
    let radiance_events = radiance
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "radiance".to_string(),
            cards: radiance_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(radiance_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards, .. }
            if formation_id == "radiance" && cards == &radiance_cards
    )));
    assert!(radiance_events.iter().any(|event| matches!(
        event,
        GameEvent::StatusAdded { status }
            if status.owner == StatusOwner::Player(p2.clone()) && status.kind == "CannotAct"
    )));
    assert!(radiance_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards, .. }
            if formation_id == "radiance" && cards == &radiance_cards
    )));
    // 讓副本經過合法 Radiance 命令造成的標準事件。在 GameRecord 的便利迴圈提交
    // 強制 pass 前，它會抵達 P2 的 ActiveEffects，沒有手動製造狀態。
    let mut p2_turn = radiance.state().clone();
    while p2_turn.current_player() != Some(&p2) || p2_turn.phase != Phase::ActiveEffects {
        let automatic_events = advance_state_automatic(&p2_turn).unwrap();
        assert!(
            !automatic_events.is_empty(),
            "automatic progression must reach P2's command boundary"
        );
        for event in automatic_events {
            apply_event(&mut p2_turn, &event);
        }
        if let Some(choice) = p2_turn.pending_choice.clone() {
            let PendingChoiceKind::Card { cards, .. } = choice.kind else {
                panic!("P1's normal Turn Draw must use a Card choice");
            };
            for event in answer_choice(
                &p2_turn,
                choice.player,
                ChoiceAnswer::Cards {
                    cards: vec![cards[0]],
                },
            )
            .unwrap()
            {
                apply_event(&mut p2_turn, &event);
            }
        }
    }
    assert!(p2_turn.hand(&p2).unwrap().contains(&card(2)));
    assert_eq!(
        handle_command(
            &p2_turn,
            Command::PerformFormation {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(2)],
                declared_targets: Vec::new(),
            },
        ),
        Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::CannotActByStatus { player: p2.clone() },
            }
        ))
    );

    let progression_events = advance_record_to_next_main_after_turn_draw(&mut radiance, card(10));
    assert!(progression_events.iter().any(|event| matches!(
        event,
        GameEvent::ActionPassed { player, reason }
            if player == &p2 && reason == &PassActionReason::CannotActByStatus
    )));
    assert_eq!(radiance.replay().unwrap(), radiance.state().clone());
    assert_eq!(radiance.verify_replay().unwrap(), radiance.state().clone());
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
fn metamorphosis_ignores_a_base_formation_from_an_older_turn() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.turn_number = 4;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5), card(10)]),
    ];
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "barrier".to_string(),
            resolved_effect_id: "barrier".to_string(),
            used_cards: vec![card(1), card(6)],
            resolved_turn: 2,
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

    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationEffectCopied { .. }))
    );
    for event in &events {
        apply_event(&mut state, event);
    }
    assert_eq!(state.shield(&PlayerId::new("p2")), Some(0));
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

    assert_event_semantics_eq!(
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
    answer_record_choice(
        &mut record,
        PlayerId::new("p2"),
        ChoiceAnswer::Cards {
            cards: vec![card(11)],
        },
    )
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
    assert!(matches!(
        state.status,
        GameStatus::Finished { ref conclusion }
            if conclusion.outcome == GameOutcome::Winner(TeamId::new("team:p1"))
    ));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn metamorphosis_copies_five_streams_unites_damage_and_draw_bonus() {
    let mut state = GameState::from_setup(&two_player_setup_with_hp(100));
    state.phase = Phase::ActiveEffects;
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
    state.phase = Phase::ActiveEffects;
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
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5), card(10)]),
    ];
    cover(&mut state, "p1", "defense", vec![card(2), card(7)], false);
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

    assert!(no_covered(&state));
    assert_eq!(state.counter_effects.len(), 1);
    assert_eq!(state.counter_effects[0].owner, PlayerId::new("p2"));
    assert_eq!(state.counter_effects[0].effect_id, "defense");
    for used_card in [card(2), card(7), card(5), card(10)] {
        assert!(state.discard.contains(&used_card));
    }

    state.phase = Phase::ActiveEffects;
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
    state.phase = Phase::ActiveEffects;
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

    assert_event_semantics_eq!(
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
            GameEvent::ChoiceRequested {
                choice: PendingChoice {
                    choice_id: ChoiceId::new(1),
                    player: PlayerId::new("p1"),
                    kind: PendingChoiceKind::Card {
                        cards: vec![card(3), card(4), card(6), card(7), card(8)],
                        minimum: 2,
                        maximum: 2,
                        can_decline: false,
                    },
                    continuation: ChoiceContinuation::Base(
                        fewfc::domain::BaseChoiceContinuation::ChaosReturnTwo,
                    ),
                },
            },
        ]
    );

    assert_eq!(
        answer_record_choice(
            &mut record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(3)]
            },
        ),
        Err(GameError::Validation(ValidationError::InvalidChoiceAnswer))
    );
    assert_eq!(
        answer_record_choice(
            &mut record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(3), card(3)]
            },
        ),
        Err(GameError::Validation(ValidationError::InvalidChoiceAnswer))
    );

    assert_event_semantics_eq!(
        answer_record_choice(
            &mut record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(3), card(4)]
            },
        )
        .unwrap(),
        vec![
            GameEvent::ChoiceMade {
                player: PlayerId::new("p1"),
                choice_id: ChoiceId::new(1),
                answer: ChoiceAnswer::Cards {
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

    assert_event_semantics_eq!(
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
    state.phase = Phase::ActiveEffects;
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

    assert_event_semantics_eq!(
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

    assert_event_semantics_eq!(
        answer_record_choice(
            &mut record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(3), card(4)]
            },
        )
        .unwrap(),
        vec![
            GameEvent::ChoiceMade {
                player: PlayerId::new("p1"),
                choice_id: ChoiceId::new(1),
                answer: ChoiceAnswer::Cards {
                    cards: vec![card(3), card(4)]
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

    assert_event_semantics_eq!(
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
    let passive = state.covered_passive(&PlayerId::new("p1")).unwrap();
    assert_eq!(passive.formation_id, "defense");
    assert_eq!(passive.cards, vec![card(2), card(7)]);
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
                grounds,
            },
            ..
        } if passive_id == "empty-city" && grounds == &vec![PassiveNoEffectGround::EmptyCity]
    )));
    let state = record.state().clone();
    assert!(no_covered(&state));
    assert!(state.discard.contains(&card(1)));
    assert!(state.discard.contains(&card(2)));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn empty_city_next_action_matrix_consumes_its_intentional_no_effect_while_attack_resolves() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

    // 基準：下一個合法 Fire Strike 沒有被動可觸發，P1 完成普通行動/回合抽牌後
    // 正常造成傷害。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let baseline_attack = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(!baseline_attack
        .iter()
        .any(|event| matches!(event, GameEvent::PassiveFlipped { .. })));
    assert!(baseline_attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change: HpChangeDelta { effective_delta, .. }, .. }
            if *effective_delta < 0
    )));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // Empty City 由合法被動命令建立。它刻意的無效果結果不能抑制下一個 incoming
    // Attack。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    let cover = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(1), card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        cover.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p1
            && formation_id == "empty-city"
            && cards == &vec![card(1), card(2)]
            && owner == &p1
            && covered == "empty-city"
            && covered_cards == &vec![card(1), card(2)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(10));
    let hp_before = interaction
        .state()
        .hp
        .iter()
        .find(|team| team.team == TeamId::new("team:p1"))
        .map(|team| team.hp)
        .unwrap();
    let attack = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: passive_cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            },
            GameEvent::AttackResolved { attacker, target, hp_change: HpChangeDelta { team, effective_delta, .. }, .. },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-strike"
            && cards == &vec![card(9)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "empty-city"
            && passive_cards == &vec![card(1), card(2)]
            && grounds == &vec![PassiveNoEffectGround::EmptyCity]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && *effective_delta < 0
            && discarded_by == &p2
            && discarded == "fire-strike"
            && discarded_cards == &vec![card(9)]
    ));
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert!(interaction
        .state()
        .hp
        .iter()
        .find(|team| team.team == TeamId::new("team:p1"))
        .is_some_and(|team| team.hp < hp_before));
    for card in [card(1), card(2), card(9)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn player_cannot_cover_second_passive_while_one_is_pending() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(2), card(7)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    cover(&mut state, "p1", "seal", vec![card(3), card(8)], false);
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
fn defense_attack_matrix_preserves_formation_lifecycle_while_preventing_damage() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：P1 使用正常合法行動並完成回合抽牌後，同一個 incoming Fire Strike
    // 會傷害 P1。
    let mut baseline = GameRecord::start(two_player_setup(), defense_setup_deck()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let baseline_hp = baseline
        .state()
        .hp
        .iter()
        .find(|entry| entry.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let baseline_events = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. } if hp_change.effective_delta < 0
    )));
    assert!(baseline_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards, .. }
            if formation_id == "fire-strike" && cards == &vec![card(9)]
    )));
    assert!(baseline_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards, .. }
            if formation_id == "fire-strike" && cards == &vec![card(9)]
    )));
    assert!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .unwrap()
            .hp
            < baseline_hp
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：Defense 只能由 P1 的合法被動陣形建立。
    let mut interaction = GameRecord::start(two_player_setup(), defense_setup_deck()).unwrap();
    interaction.advance_automatic().unwrap();
    let defense_cards = vec![card(2), card(7)];
    let defense_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense_events.as_slice(),
        [
            GameEvent::FormationCommitted { formation_id, cards, .. },
            GameEvent::PassiveCovered { player, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if formation_id == "defense"
            && cards == &defense_cards
            && player == &p1
            && covered == "defense"
            && covered_cards == &defense_cards
    ));
    assert!(matches!(
        public_view::state_for(interaction.state(), Viewer::Player(p1.clone())).covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: Some(formation_id), cards: PublicCardRefs::Known(cards), .. }]
            if owner == &p1 && formation_id == "defense" && cards == &defense_cards
    ));
    assert!(matches!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone())).covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: None, cards: PublicCardRefs::Hidden { count: 2 }, .. }]
            if owner == &p1
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(10));
    assert_eq!(interaction.state().current_player(), Some(&p2));

    // 互動：被動恰好翻開一次，只防止傷害，仍讓 incoming Formation 提交並移動其
    // 卡牌。
    let interaction_hp = interaction
        .state()
        .hp
        .iter()
        .find(|entry| entry.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let interaction_events = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(interaction_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            incoming_player,
            passive_id,
            cards,
            outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
        } if owner == &p1
            && incoming_player == &p2
            && passive_id == "defense"
            && cards == &defense_cards
            && effect_id == "defense"
            && modifications == &vec![ActionModification::PreventDamage]
    )));
    assert!(interaction_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. }
            if hp_change.delta == 0 && hp_change.effective_delta == 0
    )));
    assert!(interaction_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards, .. }
            if formation_id == "fire-strike" && cards == &vec![card(9)]
    )));
    assert!(interaction_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards, .. }
            if formation_id == "fire-strike" && cards == &vec![card(9)]
    )));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        interaction_hp
    );
    assert!(interaction.state().covered_passive(&p1).is_none());
    for card in defense_cards.into_iter().chain([card(9)]) {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn defense_metal_environment_matrix_records_the_ground_but_keeps_attack_damage() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup
        .card_instances
        .extend([card_instance(21, "metal"), card_instance(26, "metal")]);
    let opening = vec![
        card(1),
        card(2),
        card(3),
        card(4), // P1 起手：合法的初始 Metal Strike
        card(6),
        card(11),
        card(16),
        card(21),
        card(26), // P2 起手：合法的 West White Tiger
        card(7),
        card(8), // P1 第一次回合抽牌；保留兩張木卡
        card(9), // P1 必須進行的第三次抽牌/棄牌候選
        card(14),
        card(10),
        card(12), // Sacred Beast 後 P2 抽牌；保留 Fire 14
        card(13),
        card(19), // P1 測試行動後的抽牌
    ];
    let mut deck = opening.clone();
    deck.extend(
        (1..=20)
            .map(card)
            .filter(|candidate| !opening.contains(candidate)),
    );

    fn finish_turn(record: &mut GameRecord, player: &PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player.clone(),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    fn establish_metal_environment(record: &mut GameRecord, p1: &PlayerId, p2: &PlayerId) {
        record.advance_automatic().unwrap();
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(record, p1, card(8));
        let sacred_beast = record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "west-white-tiger".to_string(),
                cards: vec![card(6), card(11), card(16), card(21), card(26)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(sacred_beast.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            } if environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p2.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
        )));
        assert_eq!(record.state().environment, Some(Element::Metal));
        finish_turn(record, p2, card(12));
        assert_eq!(record.state().current_player(), Some(p1));
    }

    // 僅修飾分支：Metal Environment 是合法 Sacred-Beast 結果。無關的 Empty City
    // 仍有正常且刻意的生命週期，而後續 Fire Attack 在 Metal Environment 下仍會
    // 正常傷害 P1。
    let mut environment_only = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    establish_metal_environment(&mut environment_only, &p1, &p2);
    environment_only
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(3), card(4)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut environment_only, &p1, card(19));
    let environment_attack = environment_only
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(14)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(environment_attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown { base_points: 8, final_amount: 8, .. },
            hp_change: HpChangeDelta { team, effective_delta: -8, .. },
            ..
        } if team == &TeamId::new("team:p1")
    )));

    // 互動：相同的合法環境現在與合法覆蓋的 Defense 共存。Defense 會隨環境地面
    // 恰好翻開一次；它不會防止 incoming Fire Attack，也不會抑制其生命週期。
    let mut interaction = GameRecord::start(setup, deck).unwrap();
    establish_metal_environment(&mut interaction, &p1, &p2);
    let defense = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, .. },
        ] if player == &p1
            && formation_id == "defense"
            && cards == &vec![card(2), card(7)]
            && owner == &p1
            && covered == "defense"
            && covered_cards == &vec![card(2), card(7)]
    ));
    finish_turn(&mut interaction, &p1, card(19));
    let attack = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(14)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: defense_cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 8, final_amount: 8, .. },
                hp_change: HpChangeDelta { team, effective_delta: -8, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-strike"
            && cards == &vec![card(14)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "defense"
            && defense_cards == &vec![card(2), card(7)]
            && grounds == &vec![PassiveNoEffectGround::IneffectiveInEnvironment { environment: Element::Metal }]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "fire-strike"
            && discarded_cards == &vec![card(14)]
    ));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert!(interaction.state().covered_passive(&p1).is_none());
    for card in [card(2), card(7), card(14)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn defense_sacred_beast_matrix_consumes_defense_but_keeps_the_beasts_damage_and_environment() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup
        .card_instances
        .extend([
            card_instance(21, "metal"),
            card_instance(26, "metal"),
            card_instance(31, "metal"),
        ]);
    let deck = {
        let opening = vec![
            card(2),
            card(7),
            card(1),
            card(31), // P1：合法 Defense 或基準實體 Weapon
            card(6),
            card(11),
            card(16),
            card(21),
            card(26), // P2：合法 West White Tiger
            card(3),
            card(4),
            card(5), // P1 回合抽牌
        ];
        let mut cards = opening.clone();
        cards.extend(
            (1..=20)
                .map(card)
                .filter(|candidate| !opening.contains(candidate)),
        );
        cards
    };
    let beast_cards = vec![card(6), card(11), card(16), card(21), card(26)];

    // 基準：沒有前一位玩家被動時，Sacred Beast 具有正常的 81 點攻擊與標準環境
    // 轉移。P1 的實體 Weapon 特意不留下元素前置行動脈絡。
    let mut baseline = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(31)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(4));
    let baseline_beast = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: beast_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_beast.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown { base_points: 81, final_amount: 81, .. },
            hp_change: HpChangeDelta { team, effective_delta: -81, .. },
            elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
            ..
        } if team == &TeamId::new("team:p1")
            && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p2.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
    )));
    assert_eq!(baseline.state().environment, Some(Element::Metal));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾本身：Defense 由真正的被動命令建立，接著對非擁有者保持隱藏，直到
    // P2 的下一個行動。
    let mut interaction = GameRecord::start(setup, deck).unwrap();
    interaction.advance_automatic().unwrap();
    let defense = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p1
            && formation_id == "defense"
            && cards == &vec![card(2), card(7)]
            && owner == &p1
            && covered == "defense"
            && covered_cards == &vec![card(2), card(7)]
    ));
    assert!(matches!(
        interaction.public_view(Viewer::Player(p2.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: None, cards: PublicCardRefs::Hidden { count: 2 }, star_substitution: None }]
            if owner == &p1
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(4));

    // 互動：Beast 的規則例外讓 Defense 產生單一無效果結果，但無關的 81 點傷害、
    // 轉移、提交與實體卡牌移動都保持完整。
    let beast = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: beast_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        beast.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: defense_cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 81, final_amount: 81, .. },
                hp_change: HpChangeDelta { team, effective_delta: -81, .. },
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "west-white-tiger"
            && cards == &beast_cards
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "defense"
            && defense_cards == &vec![card(2), card(7)]
            && grounds == &vec![PassiveNoEffectGround::IgnoredBySacredBeast]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p2.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
            && discarded_by == &p2
            && discarded == "west-white-tiger"
            && discarded_cards == &beast_cards
    ));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert!(interaction.state().covered_passive(&p1).is_none());
    for used in [card(2), card(7), card(6), card(11), card(16), card(21), card(26)] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn metal_environment_barrier_matrix_keeps_commitment_and_cards_when_its_shield_is_ineffective() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
    ]);
    let barrier_cards = vec![card(2), card(7), card(31), card(4)];

    // 基準：Barrier 本身套用 44 點護盾，並在沒有 Environment 修飾時完成正常的
    // 陣形/卡牌生命週期。
    let mut baseline = GameRecord::start(
        setup.clone(),
        {
            let opening = vec![
                card(2),
                card(7),
                card(31),
                card(4), // P1：Barrier
                card(1),
                card(6),
                card(11),
                card(16),
                card(21),
                card(26),
            ];
            let mut cards = opening.clone();
            cards.extend(
                (1..=20)
                    .map(card)
                    .filter(|candidate| !opening.contains(candidate)),
            );
            cards
        },
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    let baseline_barrier = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: barrier_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_barrier.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::ShieldChanged { player: shield_owner, old_value: 0, delta: 44, new_value: 44 },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "barrier"
            && cards == &barrier_cards
            && shield_owner == &p1
            && discarded_by == &p1
            && discarded == "barrier"
            && discarded_cards == &barrier_cards
    ));
    assert_eq!(baseline.state().shield(&p1), Some(44));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾：P2 的合法 Sacred Beast 是共用 Metal Environment 的唯一來源。P1 接著
    // 仍在下一次行動中使用相同且實體上合法的 Barrier 卡牌。
    let mut interaction = GameRecord::start(
        setup,
        {
            let opening = vec![
                card(1),
                card(2),
                card(7),
                card(4), // P1：第一次行動，接著在回合抽牌後使用 Barrier
                card(6),
                card(11),
                card(16),
                card(21),
                card(26), // P2：West White Tiger
                card(31),
                card(8),
                card(9), // P1 回合抽牌；保留 Metal 31
                card(10),
                card(12),
                card(13), // P2 回合抽牌
            ];
            let mut cards = opening.clone();
            cards.extend(
                (1..=20)
                    .map(card)
                    .filter(|candidate| !opening.contains(candidate)),
            );
            cards
        },
    )
    .unwrap();
    interaction.advance_automatic().unwrap();
    interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(8));
    let sacred_beast = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: vec![card(6), card(11), card(16), card(21), card(26)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(sacred_beast.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
            ..
        } if environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            from: None,
            to: Element::Metal,
        }]
    )));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    interaction.advance_automatic().unwrap();
    answer_record_choice(
        &mut interaction,
        p2.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(10)],
        },
    )
    .unwrap();
    interaction.advance_automatic().unwrap();

    // 互動：Metal Environment 讓 Barrier 的護盾效果不適用，但不能撤銷已接受的
    // 命令、陣形提交或棄牌。
    let ignored_barrier = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: barrier_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(ignored_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if player == &p1 && formation_id == "barrier" && cards == &barrier_cards
    )));
    assert!(ignored_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationEffectIgnored {
            player,
            formation_id,
            reason: fewfc::domain::FormationNoEffectReason::IneffectiveInEnvironment {
                environment: Element::Metal,
            },
        } if player == &p1 && formation_id == "barrier"
    )));
    assert!(ignored_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p1 && formation_id == "barrier" && cards == &barrier_cards
    )));
    assert!(!ignored_barrier.iter().any(|event| matches!(
        event,
        GameEvent::ShieldChanged { player, .. } if player == &p1
    )));
    assert_eq!(interaction.state().shield(&p1), Some(0));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert_eq!(
        interaction
            .public_view(Viewer::Player(p1.clone()))
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    assert_eq!(
        interaction
            .public_view(Viewer::Player(p2.clone()))
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    for used in barrier_cards {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn defense_prevents_incoming_attack_damage_and_records_action_modification() {
    let mut record = record_after_p1_covers_defense();

    assert_event_semantics_eq!(
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
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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
    assert!(no_covered(&state));
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
fn five_streams_defense_matrix_keeps_the_turn_draw_bonus_when_damage_is_prevented() {
    // 牌堆順序只提供每位玩家合法命令所需的卡牌。P1 透過 Defense 建立修飾；此
    // 矩陣沒有注入覆蓋被動或行動結果。
    let mut setup = two_player_setup_with_hp(100);
    setup.card_instances.push(card_instance(21, "metal"));
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：五張同等級卡牌組成 Five Streams 並造成傷害，同時提交陣形並記錄其
    // 抽牌獎勵。
    let mut baseline = GameRecord::start(
        setup.clone(),
        deck_starting_with(&[2, 3, 4, 5, 1, 6, 11, 16, 21, 7, 8]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(2), card(3)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(7));
    let baseline_hp = baseline
        .state()
        .hp
        .iter()
        .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let baseline_events = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "five-streams-unite".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. } if hp_change.effective_delta < 0
    )));
    assert!(baseline_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, .. } if formation_id == "five-streams-unite"
    )));
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        baseline_hp - 60
    );
    assert_eq!(
        baseline.state().turn_draw_bonus_by_player.get(&p2),
        Some(&1)
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾與互動：P1 合法覆蓋 Defense。下一次攻擊會恰好翻開該被動，防止受影響
    // 的生命值損失，並保留 Five Streams 獨立的回合抽牌獎勵。
    let mut interaction = GameRecord::start(
        setup,
        deck_starting_with(&[2, 7, 4, 5, 1, 6, 11, 16, 21, 3, 8]),
    )
    .unwrap();
    interaction.advance_automatic().unwrap();
    let defense_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense_events.as_slice(),
        [
            GameEvent::FormationCommitted { formation_id, cards, .. },
            GameEvent::PassiveCovered { player, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if formation_id == "defense"
            && cards == &vec![card(2), card(7)]
            && player == &p1
            && covered == "defense"
            && covered_cards == &vec![card(2), card(7)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(3));
    assert_eq!(interaction.state().current_player(), Some(&p2));
    let interaction_hp = interaction
        .state()
        .hp
        .iter()
        .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let interaction_events = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "five-streams-unite".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(interaction_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            passive_id,
            outcome: PassiveFlipOutcome::Applied { .. },
            ..
        } if owner == &p1 && passive_id == "defense"
    )));
    assert!(interaction_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. }
            if hp_change.delta == 0 && hp_change.effective_delta == 0
    )));
    assert!(interaction_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards, .. }
            if formation_id == "five-streams-unite"
                && cards == &vec![card(1), card(6), card(11), card(16), card(21)]
    )));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        interaction_hp
    );
    assert_eq!(
        interaction.state().turn_draw_bonus_by_player.get(&p2),
        Some(&1)
    );
    interaction.advance_automatic().unwrap();
    assert!(matches!(
        &interaction.state().pending_choice,
        Some(PendingChoice {
            player,
            kind: PendingChoiceKind::Card { cards, .. },
            ..
        }) if player == &p2 && cards.len() == 4
    ));
    assert!(interaction.state().discard.contains(&card(2)));
    assert!(interaction.state().discard.contains(&card(7)));
    assert!(interaction.state().formation_area(&p2).is_some());
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn countershock_attack_matrix_splits_damage_after_legal_cover_and_preserves_attack_lifecycle() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[4, 9, 1, 2, 5, 10, 3, 6, 7]);

    // 基準：沒有修飾時，相同的普通 Metal Strike 只會傷害 P1。P1 的 Fire Strike
    // 是合法的先前行動，不是注入的回合歷史。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(4)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(8));
    let baseline_attack = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 7, final_amount: 7, .. },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -7, new_hp: 23, effective_delta: -7 },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(6)]
    ));
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(23)
    );
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(22)
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：Countershock 由 P1 的合法陣形命令建立。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    let cover_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        cover_events.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, star_substitution: None, sealed: false },
        ] if player == &p1
            && formation_id == "countershock"
            && cards == &vec![card(4), card(9)]
            && covered_by == &p1
            && covered == "countershock"
            && covered_cards == &vec![card(4), card(9)]
    ));
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p1.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: Some("countershock".to_string()),
            cards: PublicCardRefs::Known(vec![card(4), card(9)]),
            star_substitution: None,
        }]
    );
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(8));

    // 互動：相同的 Attack 會翻開 Countershock。它將七點分成兩個向上取整的四點
    // 損失，同時保留攻擊提交與雙方標準卡牌移動。
    let attack = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: passive_cards,
                outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                formation_id: attack_formation,
                used_cards,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -4, new_hp: 26, effective_delta: -4 },
                shield_change: None,
                card_moves,
                elemental_context_update: Some(AttackResolutionEffects {
                    outcome: fewfc::domain::AttackOutcome::Resolved,
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Metal, resolved_turn: 2 },
                    }),
                    hp_changes,
                    shield_changes,
                    card_moves: effect_card_moves,
                    statuses_added,
                    statuses_removed,
                    counter_effects_established,
                    turn_draw_bonus_changes,
                    environment_transfers,
                }),
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "countershock"
            && passive_cards == &vec![card(4), card(9)]
            && effect_id == "countershock"
            && modifications == &vec![ActionModification::SplitAttackDamage]
            && attacker == &p2
            && target == &p1
            && attack_formation == "metal-strike"
            && used_cards == &vec![card(6)]
            && team == &TeamId::new("team:p1")
            && card_moves.is_empty()
            && context_player == &p2
            && hp_changes == &vec![HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 30,
                delta: -4,
                new_hp: 26,
                effective_delta: -4,
            }]
            && shield_changes.is_empty()
            && effect_card_moves.is_empty()
            && statuses_added.is_empty()
            && statuses_removed.is_empty()
            && counter_effects_established.is_empty()
            && turn_draw_bonus_changes.is_empty()
            && environment_transfers.is_empty()
            && discarded_by == &p2
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(6)]
    ));
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(26)
    );
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(26)
    );
    for card in [card(4), card(9), card(6)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn countershock_shield_matrix_splits_before_physical_shield_absorption_through_legal_turns() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[
        2, 7, 1, 4, // P1 Barrier
        6, 11, 16, 5, 10, // P2 起手；先 Metal Strike 再 Weapon
        9, 14, 3, // P1 收到 Fire/Fire，另有一張回合抽牌棄牌
        8, 12, 13, // P2 第一次回合抽牌
        15, 17, 18, // P1 第二次回合抽牌
    ]);

    fn advance_p2_turn(record: &mut GameRecord, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            PlayerId::new("p2"),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    fn establish_barrier_and_take_opening_attack(
        record: &mut GameRecord,
        p1: &PlayerId,
        p2: &PlayerId,
    ) {
        record.advance_automatic().unwrap();
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "barrier".to_string(),
                cards: vec![card(2), card(7), card(1), card(4)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert_eq!(record.state().shield(p1), Some(44));
        advance_record_to_next_main_after_turn_draw(record, card(3));
        let opening_attack = record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(opening_attack.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                hp_change: HpChangeDelta { delta: 0, effective_delta: 0, .. },
                shield_change: Some(ShieldChangeDelta { player, old_value: 44, delta: -7, new_value: 37 }),
                ..
            } if player == p1
        )));
        advance_p2_turn(record, card(8));
        assert_eq!(record.state().current_player(), Some(p1));
        assert_eq!(record.state().shield(p1), Some(37));
    }

    // 基準：只有 Barrier 時，最終實體 Weapon 會讓護盾消耗兩倍的十二點傷害，
    // 並讓兩隊生命值保持不變；P1 中間的普通 Fire Strike 除外。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    establish_barrier_and_take_opening_attack(&mut baseline, &p1, &p2);
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(15));
    let baseline_weapon = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(11), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown { base_points: 12, final_amount: 12, .. },
                hp_change: HpChangeDelta { team, delta: 0, effective_delta: 0, .. },
                shield_change: Some(ShieldChangeDelta { player: shield_owner, old_value: 37, delta: -24, new_value: 13 }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "weapon"
            && cards == &vec![card(11), card(16)]
            && team == &TeamId::new("team:p1")
            && shield_owner == &p1
            && discarded_by == &p2
            && discarded == "weapon"
            && discarded_cards == &vec![card(11), card(16)]
    ));
    assert_eq!(baseline.state().shield(&p1), Some(13));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：相同 Barrier 與中間的 P2 回合後，P1 透過合法被動陣形覆蓋 Countershock；
    // 沒有注入護盾或覆蓋狀態。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    establish_barrier_and_take_opening_attack(&mut interaction, &p1, &p2);
    let countershock = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(9), card(14)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        countershock.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, .. },
        ] if player == &p1
            && formation_id == "countershock"
            && cards == &vec![card(9), card(14)]
            && covered_by == &p1
            && covered == "countershock"
            && covered_cards == &vec![card(9), card(14)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(15));

    // 互動：Countershock 先將 incoming 的十二點減半。目標方的一半接著是對護盾
    // 的實體傷害，因此加倍為十二點；另一半傷害 P2 的生命值。兩個陣形生命週期
    // 都保持不變。
    let weapon = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(11), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(weapon.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            passive_id,
            outcome: PassiveFlipOutcome::Applied { modifications, .. },
            ..
        } if owner == &p1
            && passive_id == "countershock"
            && modifications == &vec![ActionModification::SplitAttackDamage]
    )));
    assert!(weapon.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown { base_points: 12, final_amount: 12, .. },
            hp_change: HpChangeDelta { team, delta: 0, effective_delta: 0, .. },
            shield_change: Some(ShieldChangeDelta { player, old_value: 37, delta: -12, new_value: 25 }),
            elemental_context_update: Some(effects),
            ..
        } if team == &TeamId::new("team:p1")
            && effects.hp_changes == vec![HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 30,
                delta: -6,
                new_hp: 24,
                effective_delta: -6,
            }]
    )));
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert_eq!(interaction.state().shield(&p1), Some(25));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(30)
    );
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(24)
    );
    for card in [card(9), card(14), card(11), card(16)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn countershock_lethal_matrix_resolves_both_split_losses_before_declaring_a_draw() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[
        4, 9, 1, 2, // P1：合法 Countershock
        6, 11, 16, 5, 10, // P2：合法 Metal Strike
        3, 7, // P1 回合抽牌，以下棄置卡牌 7
    ]);
    let mut record = GameRecord::start(two_player_setup_with_hp(4), deck).unwrap();
    record.advance_automatic().unwrap();

    // 修飾：覆蓋的 Countershock 透過實際回合抽牌生命週期抵達下一位玩家的行動。
    let covered = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        covered.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered_id, cards: covered_cards, .. },
        ] if player == &p1
            && formation_id == "countershock"
            && cards == &vec![card(4), card(9)]
            && covered_by == &p1
            && covered_id == "countershock"
            && covered_cards == &vec![card(4), card(9)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut record, card(7));
    assert_eq!(record.state().current_player(), Some(&p2));

    // 互動：七點 Attack 分成兩個向上取整的四點損失。兩個差異都必須在終止抽牌
    // 前記錄，同時被動與 incoming Formation 卡牌仍會抵達棄牌堆。
    let attack = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                outcome: PassiveFlipOutcome::Applied { modifications, .. },
                ..
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 7, final_amount: 7, .. },
                hp_change: HpChangeDelta { team, old_hp: 4, delta: -4, new_hp: 0, effective_delta: -4 },
                elemental_context_update: Some(AttackResolutionEffects { hp_changes, .. }),
                ..
            },
            GameEvent::GameEnded { conclusion },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "countershock"
            && modifications == &vec![ActionModification::SplitAttackDamage]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && hp_changes == &vec![HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 4,
                delta: -4,
                new_hp: 0,
                effective_delta: -4,
            }]
            && conclusion.outcome == GameOutcome::Draw
            && conclusion.causes == vec![GameEndCause::TeamHpDepleted {
                teams: vec![TeamId::new("team:p1"), TeamId::new("team:p2")],
            }]
    ));
    assert!(matches!(
        record.state().status,
        GameStatus::Finished { ref conclusion } if conclusion.outcome == GameOutcome::Draw
    ));
    assert!(record.state().hp.iter().all(|team| team.hp == 0));
    assert!(record.state().covered_passive(&p1).is_none());
    for card in [card(4), card(9)] {
        assert!(record.state().discard.contains(&card));
    }
    assert!(matches!(
        record.state().formation_area(&p2),
        Some(PlayerFormationArea {
            formation: Some(FormationInArea { formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }),
            ..
        }) if formation_id == "metal-strike" && cards == &vec![card(6)]
    ));
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
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
    assert!(no_covered(&state));
    assert!(state.discard.contains(&card(4)));
    assert!(state.discard.contains(&card(9)));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn countershock_splits_before_the_defenders_shield_absorbs_physical_damage() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(1), card(6)]),
    ];
    cover(
        &mut state,
        "p1",
        "countershock",
        vec![card(4), card(9)],
        false,
    );
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
    state.phase = Phase::ActiveEffects;
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
    cover(
        &mut state,
        "p1",
        "countershock",
        vec![card(4), card(9)],
        false,
    );

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
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(6)]),
    ];
    cover(
        &mut state,
        "p1",
        "countershock",
        vec![card(4), card(9)],
        false,
    );

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

    assert!(matches!(
        state.status,
        GameStatus::Finished { ref conclusion } if conclusion.outcome == GameOutcome::Draw
    ));
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
        semantic_events(&events).first(),
        Some(&GameEvent::PassiveFlipped {
            owner: PlayerId::new("p1"),
            incoming_player: PlayerId::new("p2"),
            passive_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            outcome: PassiveFlipOutcome::NoEffect {
                grounds: vec![PassiveNoEffectGround::NotASpell],
            },
        })
    );

    let state = record.state().clone();
    assert!(no_covered(&state));
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
fn seal_attack_matrix_records_not_a_spell_but_keeps_the_attack_outcome() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[3, 8, 1, 6, 9, 10, 4, 5, 7, 11, 12]);

    // 基準：合法 Fire Strike 在沒有覆蓋被動時正常解析，建立確切的未受影響攻擊
    // 結果。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(11));
    let baseline_attack = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown { base_points: 8, final_amount: 8, .. },
            hp_change: HpChangeDelta { team, old_hp: 30, delta: -8, new_hp: 22, effective_delta: -8 },
            ..
        } if team == &TeamId::new("team:p1")
    )));

    // 修飾：P1 在相同的 incoming Attack 前合法覆蓋 Seal。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(11));
    let attack = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: seal_cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 8, final_amount: 8, .. },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -8, new_hp: 22, effective_delta: -8 },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-strike"
            && cards == &vec![card(9)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && seal_cards == &vec![card(3), card(8)]
            && grounds == &vec![PassiveNoEffectGround::NotASpell]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "fire-strike"
            && discarded_cards == &vec![card(9)]
    ));
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(22)
    );
    for card in [card(3), card(8), card(9)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn seal_cancels_incoming_active_spell_effects_and_consumes_the_action() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(
            PlayerId::new("p2"),
            vec![card(2), card(7), card(1), card(4)],
        ),
    ];
    cover(&mut state, "p1", "seal", vec![card(3), card(8)], false);

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

    assert_event_semantics_eq!(
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
    assert!(no_covered(&state));
}

#[test]
fn seal_barrier_matrix_cancels_the_spell_but_keeps_formation_commitment_and_card_movement() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[3, 8, 1, 2, 7, 12, 4, 5, 6, 10, 11, 13]);
    let barrier_cards = vec![card(7), card(12), card(4), card(6)];

    // 基準：P1 普通合法的 Metal Strike 後，Barrier 自身效果給 P2 44 點護盾。
    // 它建立互動中必須在取消後仍存活的確切行動與卡牌生命週期。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let baseline_barrier = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "barrier".to_string(),
            cards: barrier_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_barrier.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::ShieldChanged { player: shield_owner, old_value: 0, delta: 44, new_value: 44 },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "barrier"
            && cards == &barrier_cards
            && shield_owner == &p2
            && discarded_by == &p2
            && discarded == "barrier"
            && discarded_cards == &barrier_cards
    ));
    assert_eq!(baseline.state().shield(&p2), Some(44));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：P1 合法覆蓋 Seal。其擁有者看得到覆蓋的卡牌，而 P2 在觸發前只看得到
    // 數量。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    let seal = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        seal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, star_substitution: None, sealed: false },
        ] if player == &p1
            && formation_id == "seal"
            && cards == &vec![card(3), card(8)]
            && covered_by == &p1
            && covered == "seal"
            && covered_cards == &vec![card(3), card(8)]
    ));
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(10));

    // 互動：Seal 翻開一次並取消 Barrier 的護盾結果。Barrier 仍會提交並棄置四張
    // 實體卡牌；Seal 本身被消耗，因此沒有潛在反制或捏造的護盾。
    let canceled_barrier = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "barrier".to_string(),
            cards: barrier_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(canceled_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if player == &p2 && formation_id == "barrier" && cards == &barrier_cards
    )));
    assert!(canceled_barrier.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            incoming_player,
            passive_id,
            cards,
            outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
        } if owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && cards == &vec![card(3), card(8)]
            && effect_id == "seal"
            && modifications == &vec![ActionModification::CancelSpell]
    )));
    assert!(canceled_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p2 && formation_id == "barrier" && cards == &barrier_cards
    )));
    assert!(!canceled_barrier.iter().any(|event| matches!(
        event,
        GameEvent::ShieldChanged { player, .. } if player == &p2
    )));
    assert_eq!(interaction.state().shield(&p2), Some(0));
    assert!(interaction.state().covered_passive(&p1).is_none());
    for card in [card(3), card(8), card(7), card(12), card(4), card(6)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn seal_cancels_chaos_without_leaving_a_pending_choice() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(3), card(4)]),
        PlayerHand::new(
            PlayerId::new("p2"),
            vec![card(5), card(10), card(2), card(1)],
        ),
    ];
    cover(&mut state, "p1", "seal", vec![card(8), card(13)], false);

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
    assert!(no_covered(&state));
    assert!(!events
        .iter()
        .any(|event| matches!(event, GameEvent::ChoiceRequested { .. })));
    for used_card in [card(8), card(13), card(5), card(10), card(2), card(1)] {
        assert!(state.discard.contains(&used_card));
    }
}

#[test]
fn seal_chaos_matrix_cancels_the_choice_but_keeps_spell_commitment_and_cards() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let chaos_cards = vec![card(5), card(10), card(7), card(6)];

    // 基準：Chaos 正常只向 P2 公開其具型別延續；P2 會從檢視的 P1 卡牌中精確
    // 選擇兩張並送回 DeckTop。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 2, 3, 4, 5, 10, 7, 6, 9, 11, 12, 13]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(13));
    let baseline_chaos = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "chaos".to_string(),
            cards: chaos_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_chaos.iter().any(|event| matches!(
        event,
        GameEvent::ChoiceRequested { choice }
            if choice.player == p2
                && matches!(choice.continuation, ChoiceContinuation::Base(fewfc::domain::BaseChoiceContinuation::ChaosReturnTwo))
    )));
    let baseline_choice = baseline
        .state()
        .pending_choice
        .as_ref()
        .expect("unsuppressed Chaos must create its canonical choice")
        .clone();
    let PendingChoiceKind::Card {
        cards: allowed,
        minimum,
        maximum,
        ..
    } = &baseline_choice.kind
    else {
        panic!("Chaos must request a typed Card choice");
    };
    assert_eq!((*minimum, *maximum), (2, 2));
    let chosen = vec![allowed[0], allowed[1]];
    let baseline_answer = baseline
        .handle(Command::AnswerChoice {
            player: p2.clone(),
            choice_id: baseline_choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: chosen.clone(),
            },
        })
        .unwrap();
    assert!(baseline_answer.iter().any(|event| matches!(
        event,
        GameEvent::CardsMoved { card_moves }
            if card_moves.len() == 2 && card_moves.iter().all(|movement| {
                movement.from == CardZone::Hand(p1.clone()) && movement.to == CardZone::DeckTop
            })
    )));
    assert!(baseline.state().pending_choice.is_none());
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 互動：P1 合法覆蓋 Seal。P2 相同的合法 Chaos 陣形會觸發它、消耗兩個陣形，
    // 且即使保留 Chaos 的提交/卡牌移動，也不能建立選擇或檢視 P1 手牌。
    let mut interaction = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[3, 8, 1, 2, 5, 10, 7, 6, 4, 9, 11, 12, 13]),
    )
    .unwrap();
    interaction.advance_automatic().unwrap();
    let seal = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        seal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, .. },
        ] if player == &p1
            && formation_id == "seal"
            && cards == &vec![card(3), card(8)]
            && owner == &p1
            && covered == "seal"
            && covered_cards == &vec![card(3), card(8)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(12));
    let p1_hand_before = interaction.state().hand(&p1).unwrap().to_vec();
    let canceled = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "chaos".to_string(),
            cards: chaos_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        canceled.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: seal_cards,
                outcome: PassiveFlipOutcome::Applied { modifications, .. },
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "chaos"
            && cards == &chaos_cards
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && seal_cards == &vec![card(3), card(8)]
            && modifications == &vec![ActionModification::CancelSpell]
            && discarded_by == &p2
            && discarded == "chaos"
            && discarded_cards == &chaos_cards
    ));
    assert!(!canceled.iter().any(|event| matches!(
        event,
        GameEvent::ChoiceRequested { .. } | GameEvent::HandInspected { .. }
    )));
    assert!(interaction.state().pending_choice.is_none());
    assert_eq!(
        interaction.state().hand(&p1),
        Some(p1_hand_before.as_slice())
    );
    assert!(interaction.state().covered_passive(&p1).is_none());
    for card in [card(3), card(8), card(5), card(10), card(7), card(6)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn seal_marks_incoming_passive_cover_as_sealed_without_exposing_the_marker() {
    let mut record = record_after_p1_covers_seal();

    assert_event_semantics_eq!(
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
    let passive = state.covered_passive(&PlayerId::new("p2")).unwrap();
    assert_eq!(passive.formation_id, "countershock");
    assert_eq!(passive.cards, vec![card(4), card(9)]);
    assert!(matches!(
        passive.state,
        FormationAreaState::FaceDownWaiting { sealed: true, .. }
    ));
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
    answer_record_choice(
        &mut record,
        PlayerId::new("p2"),
        ChoiceAnswer::Cards {
            cards: vec![card(13)],
        },
    )
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
        semantic_events(&events).first(),
        Some(&GameEvent::PassiveFlipped {
            owner: PlayerId::new("p2"),
            incoming_player: PlayerId::new("p1"),
            passive_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            outcome: PassiveFlipOutcome::NoEffect {
                grounds: vec![PassiveNoEffectGround::Sealed],
            },
        })
    );

    let state = record.state().clone();
    assert!(no_covered(&state));
    assert!(state.discard.contains(&card(4)));
    assert!(state.discard.contains(&card(9)));
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn seal_incoming_covered_passive_matrix_commits_sealed_counter_then_consumes_it_on_attack() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[3, 8, 1, 2, 4, 5, 6, 7, 9, 10, 11, 12, 13]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    // 修飾：P1 合法覆蓋 Seal，完成真正的回合抽牌，並讓 P2 的反制陣形成為下一個
    // 待測命令。
    let seal = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        seal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p1
            && formation_id == "seal"
            && cards == &vec![card(3), card(8)]
            && owner == &p1
            && covered == "seal"
            && covered_cards == &vec![card(3), card(8)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut record, card(10));
    assert_eq!(record.state().current_player(), Some(&p2));

    // 基準對照：沒有 incoming Seal 時，Countershock 正常覆蓋。這讓確切的合法
    // 使用可與下方的封印互動區分。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 2, 3, 11, 4, 9, 5, 6, 7, 8, 10]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(8));
    let unsealed = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        unsealed.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p2
            && formation_id == "countershock"
            && cards == &vec![card(4), card(9)]
            && owner == &p2
            && covered == "countershock"
            && covered_cards == &vec![card(4), card(9)]
    ));

    // 互動：Seal 被消耗，使合法覆蓋的 Countershock 被封印。其擁有者看得到識別
    // 與卡牌，而 P1 只看得到數量；兩個公開投影都不會洩漏封印標記。
    let countershock = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        countershock.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: seal_cards,
                outcome: PassiveFlipOutcome::Applied { modifications, .. },
            },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, sealed: true, .. },
        ] if player == &p2
            && formation_id == "countershock"
            && cards == &vec![card(4), card(9)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && seal_cards == &vec![card(3), card(8)]
            && modifications == &vec![ActionModification::SealCoveredPassive]
            && covered_by == &p2
            && covered == "countershock"
            && covered_cards == &vec![card(4), card(9)]
    ));
    assert!(matches!(
        record.public_view(Viewer::Player(p2.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: Some(formation_id), cards: PublicCardRefs::Known(cards), star_substitution: None }]
            if owner == &p2 && formation_id == "countershock" && cards == &vec![card(4), card(9)]
    ));
    assert!(matches!(
        record.public_view(Viewer::Player(p1.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: None, cards: PublicCardRefs::Hidden { count: 2 }, star_substitution: None }]
            if owner == &p2
    ));

    // 下一次 P1 Attack 會恰好翻開被封印的被動。它沒有 Countershock 修飾，因此
    // 攻擊者不承受反射傷害，而 P2 仍承受正常的 Metal Strike 損失。
    record.advance_automatic().unwrap();
    answer_record_choice(
        &mut record,
        p2.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(13)],
        },
    )
    .unwrap();
    record.advance_automatic().unwrap();
    let attack = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if player == &p1 && formation_id == "metal-strike" && cards == &vec![card(1)]
    )));
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            incoming_player,
            passive_id,
            cards: passive_cards,
            outcome: PassiveFlipOutcome::NoEffect { grounds },
        } if owner == &p2
            && incoming_player == &p1
            && passive_id == "countershock"
            && passive_cards == &vec![card(4), card(9)]
            && grounds == &vec![PassiveNoEffectGround::Sealed]
    )));
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            attacker,
            target,
            point_breakdown: AttackPointBreakdown { base_points: 7, final_amount: 7, .. },
            hp_change: HpChangeDelta { team, old_hp: 30, delta: -7, new_hp: 23, effective_delta: -7 },
            ..
        } if attacker == &p1 && target == &p2 && team == &TeamId::new("team:p2")
    )));
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p1 && formation_id == "metal-strike" && cards == &vec![card(1)]
    )));
    assert!(record.state().covered_passive(&p2).is_none());
    for card in [card(3), card(8), card(4), card(9), card(1)] {
        assert!(record.state().discard.contains(&card));
    }
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn seal_void_meridian_matrix_cancels_environment_clearing_but_keeps_spell_commitment_and_cards() {
    let p0 = PlayerId::new("p0");
    let p3 = PlayerId::new("p3");
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(300)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.players = vec![
        Player {
            id: p0.clone(),
            team: TeamId::new("team:a"),
        },
        Player {
            id: p3.clone(),
            team: TeamId::new("team:b"),
        },
        Player {
            id: p1.clone(),
            team: TeamId::new("team:a"),
        },
        Player {
            id: p2.clone(),
            team: TeamId::new("team:b"),
        },
    ];
    setup.turn_order = vec![p0.clone(), p3.clone(), p1.clone(), p2.clone()];
    setup.hp = vec![
        TeamHp {
            team: TeamId::new("team:a"),
            hp: 300,
        },
        TeamHp {
            team: TeamId::new("team:b"),
            hp: 300,
        },
    ];
    // 額外的三張金卡只是固定背景：它們共同的印製等級使後續合法的 Void Meridian
    // 使用有效。
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
        card_instance(36, "metal"),
        card_instance(41, "metal"),
        card_instance(46, "earth"),
        card_instance(47, "wood"),
        card_instance(48, "fire"),
    ]);
    let opening = vec![
        card(1),
        card(2),
        card(3),
        card(4), // P0 起手
        card(6),
        card(11),
        card(16),
        card(21),
        card(26), // P3 起手：West White Tiger
        card(8),
        card(13),
        card(5),
        card(7),
        card(9), // P1 起手：Seal
        card(31),
        card(36),
        card(41),
        card(14),
        card(15), // P2 起手：Void Meridian
        card(10),
        card(12),
        card(17),
        card(18),
        card(19),
        card(20), // P0/P3 回合抽牌
        card(46),
        card(47),
        card(48), // P1 互動回合抽牌
    ];
    let mut deck = opening.clone();
    deck.extend(
        (1..=20)
            .map(card)
            .filter(|candidate| !opening.contains(candidate)),
    );

    fn finish_turn(record: &mut GameRecord, player: PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player,
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    fn record_before_modifier(
        setup: &GameSetup,
        deck: &[CardInstanceId],
        p0: &PlayerId,
        p3: &PlayerId,
        p1: &PlayerId,
        p2: &PlayerId,
    ) -> GameRecord {
        let mut record = GameRecord::start(setup.clone(), deck.to_vec()).unwrap();
        record.advance_automatic().unwrap();

        // P0 只推進回合。接著 P3 建立 Environment，P1 取得獨立的合法行動，而 P2
        // 以真正已持有 Void Meridian 的起手牌抵達。
        record
            .handle(Command::PerformFormation {
                player: p0.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p0.clone(), card(10));

        // 共用 Metal Environment 只能由合法 Sacred Beast 命令建立，而不是由固定
        // 資料變更注入。
        let west_white_tiger = record
            .handle(Command::PerformFormation {
                player: p3.clone(),
                formation_id: "west-white-tiger".to_string(),
                cards: vec![card(6), card(11), card(16), card(21), card(26)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(west_white_tiger.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            } if environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p3.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
        )));
        assert_eq!(record.state().environment, Some(Element::Metal));
        finish_turn(&mut record, p3.clone(), card(18));

        assert_eq!(record.state().current_player(), Some(p1));
        assert_eq!(record.state().environment, Some(Element::Metal));
        assert!(record.state().hand(p2).unwrap().contains(&card(31)));
        assert!(record.state().hand(p2).unwrap().contains(&card(36)));
        assert!(record.state().hand(p2).unwrap().contains(&card(41)));
        record
    }

    // 基準：適用的 Void Meridian 法術會清除共用 Environment，並各改變兩隊生命值
    // 一次。其提交與卡牌移動是取消分支的參考生命週期。
    let mut baseline = record_before_modifier(&setup, &deck, &p0, &p3, &p1, &p2);
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "water-strike".to_string(),
            cards: vec![card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, p1.clone(), card(46));
    let baseline_before_void = baseline.state().hp.clone();
    let void_baseline = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(31), card(36), card(41)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        void_baseline.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::EnvironmentCleared { player: clearer, formation_id: cleared_by, environment: Element::Metal, hp_changes },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "void-meridian-severing"
            && cards == &vec![card(31), card(36), card(41)]
            && clearer == &p2
            && cleared_by == "void-meridian-severing"
            && hp_changes.len() == 2
            && hp_changes.iter().zip(&baseline_before_void).all(|(change, before)| {
                change.team == before.team
                    && change.old_hp == before.hp
                    && change.new_hp == before.hp - 20
                    && change.effective_delta == -20
            })
            && discarded_by == &p2
            && discarded == "void-meridian-severing"
            && discarded_cards == &vec![card(31), card(36), card(41)]
    ));
    assert_eq!(baseline.state().environment, None);
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾本身：Seal 透過完整合法命令建立，並維持私有識別，直到 P2 執行下一個
    // 行動。
    let mut interaction = record_before_modifier(&setup, &deck, &p0, &p3, &p1, &p2);
    let seal = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(8), card(13)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        seal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p1
            && formation_id == "seal"
            && cards == &vec![card(8), card(13)]
            && owner == &p1
            && covered == "seal"
            && covered_cards == &vec![card(8), card(13)]
    ));
    assert!(matches!(
        interaction.public_view(Viewer::Player(p1.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: Some(formation_id), cards: PublicCardRefs::Known(cards), star_substitution: None }]
            if owner == &p1 && formation_id == "seal" && cards == &vec![card(8), card(13)]
    ));
    assert!(matches!(
        interaction.public_view(Viewer::Player(p2.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: None, cards: PublicCardRefs::Hidden { count: 2 }, star_substitution: None }]
            if owner == &p1
    ));
    finish_turn(&mut interaction, p1.clone(), card(46));
    let interaction_before_void = interaction.state().hp.clone();

    // 互動：Seal 取消適用的法術效果，但不取消其標準提交或實體卡牌移動。因此
    // Environment 與兩隊生命值都維持在 Void Meridian 之前的狀態。
    let void_canceled = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(31), card(36), card(41)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(void_canceled.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if player == &p2
                && formation_id == "void-meridian-severing"
                && cards == &vec![card(31), card(36), card(41)]
    )));
    assert!(void_canceled.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            incoming_player,
            passive_id,
            cards,
            outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
        } if owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && cards == &vec![card(8), card(13)]
            && effect_id == "seal"
            && modifications == &vec![ActionModification::CancelSpell]
    )));
    assert!(!void_canceled
        .iter()
        .any(|event| matches!(event, GameEvent::EnvironmentCleared { .. })));
    assert!(void_canceled.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p2
                && formation_id == "void-meridian-severing"
                && cards == &vec![card(31), card(36), card(41)]
    )));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert_eq!(interaction.state().hp, interaction_before_void);
    assert!(interaction.state().covered_passive(&p1).is_none());
    for used in [card(8), card(13), card(31), card(36), card(41)] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
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
    assert_eq!(
        state.covered_passive(&PlayerId::new("p1")).unwrap().cards,
        vec![card(2), card(7)]
    );
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
    cover(&mut state, "p1", "defense", vec![card(2), card(7)], false);

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
        Some(PublicPendingChoice::Visible {
            choice_id: ChoiceId::new(1),
            player: PlayerId::new("p1"),
            reason: PublicPendingChoicePresentation::Chaos,
            choice: PendingChoiceKind::Card {
                cards: vec![card(3), card(4), card(6), card(7), card(8)],
                minimum: 2,
                maximum: 2,
                can_decline: false,
            },
        })
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p2"))).pending_choice,
        Some(PublicPendingChoice::Hidden {
            player: PlayerId::new("p1"),
            reason: PublicPendingChoicePresentation::Chaos,
        })
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Observer).pending_choice,
        Some(PublicPendingChoice::Hidden {
            player: PlayerId::new("p1"),
            reason: PublicPendingChoicePresentation::Chaos,
        })
    );
    assert_eq!(
        state.pending_choice,
        Some(PendingChoice {
            choice_id: ChoiceId::new(1),
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::Card {
                cards: vec![card(3), card(4), card(6), card(7), card(8)],
                minimum: 2,
                maximum: 2,
                can_decline: false,
            },
            continuation: ChoiceContinuation::Base(
                fewfc::domain::BaseChoiceContinuation::ChaosReturnTwo,
            ),
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
fn choice_requested_event_view_filters_options_and_continuations() {
    let event = GameEvent::ChoiceRequested {
        choice: PendingChoice {
            choice_id: ChoiceId::new(9),
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::Card {
                cards: vec![card(1), card(2)],
                minimum: 2,
                maximum: 2,
                can_decline: false,
            },
            continuation: ChoiceContinuation::Base(
                fewfc::domain::BaseChoiceContinuation::ChaosReturnTwo,
            ),
        },
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::ChoiceRequested {
            choice: PublicPendingChoice::Visible {
                choice_id: ChoiceId::new(9),
                player: PlayerId::new("p1"),
                reason: PublicPendingChoicePresentation::Chaos,
                choice: PendingChoiceKind::Card {
                    cards: vec![card(1), card(2)],
                    minimum: 2,
                    maximum: 2,
                    can_decline: false,
                },
            },
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::ChoiceRequested {
            choice: PublicPendingChoice::Hidden {
                player: PlayerId::new("p1"),
                reason: PublicPendingChoicePresentation::Chaos,
            },
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::ChoiceRequested {
            choice: PublicPendingChoice::Hidden {
                player: PlayerId::new("p1"),
                reason: PublicPendingChoicePresentation::Chaos,
            },
        }
    );
    assert_eq!(
        event,
        GameEvent::ChoiceRequested {
            choice: PendingChoice {
                choice_id: ChoiceId::new(9),
                player: PlayerId::new("p1"),
                kind: PendingChoiceKind::Card {
                    cards: vec![card(1), card(2)],
                    minimum: 2,
                    maximum: 2,
                    can_decline: false,
                },
                continuation: ChoiceContinuation::Base(
                    fewfc::domain::BaseChoiceContinuation::ChaosReturnTwo,
                ),
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
            expected: Phase::ActiveEffects,
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

    assert_event_semantics_eq!(
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

    assert_event_semantics_eq!(
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
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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
    assert!(matches!(
        state.status,
        GameStatus::Finished { ref conclusion }
            if conclusion.outcome == GameOutcome::Winner(TeamId::new("team:p1"))
    ));
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
    state.phase = Phase::ActiveEffects;
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
    apply_event(
        &mut state,
        &GameEvent::GameEnded {
            conclusion: GameConclusion::new(
                GameOutcome::Draw,
                vec![GameEndCause::TeamHpDepleted {
                    teams: vec![TeamId::new("team:p1"), TeamId::new("team:p2")],
                }],
                None,
            ),
        },
    );

    assert!(matches!(
        state.status,
        GameStatus::Finished { ref conclusion } if conclusion.outcome == GameOutcome::Draw
    ));
}

#[test]
fn elemental_attack_previous_element_matrix_distinguishes_physical_baseline_from_overcoming() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：真正的實體 Weapon 使用是立即的前一個陣形，但不建立元素脈絡。因此
    // 下一個 Fire Strike 會解析正常的八點傷害。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 2, 3, 9, 4, 5, 7, 8]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let ordinary_fire = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        ordinary_fire,
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
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
                    delta: -8,
                    new_hp: 22,
                    effective_delta: -8,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 2,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
            },
        ]
    );
    assert!(baseline
        .state()
        .last_elemental_attack_by_player
        .get(&p1)
        .is_none());
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：P1 使用相同的公開命令路徑建立立即的金元素脈絡。它不是手動寫入
    // 最後元素狀態。
    let mut interaction = record_after_p1_metal_attack_on_turn_1();
    assert_eq!(
        interaction.state().last_elemental_attack_by_player.get(&p1),
        Some(&LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        })
    );
    assert_eq!(interaction.state().current_player(), Some(&p2));

    // 互動：Fire 克制緊接之前的金攻擊，將相同的基礎八點精確加倍。陣形提交、
    // 卡牌移動、具型別脈絡、最終狀態與回放仍然是標準的。
    let overcoming_fire = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        overcoming_fire,
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
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
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 2,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
            },
        ]
    );
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(14)
    );
    assert!(interaction.state().discard.contains(&card(1)));
    assert!(interaction.state().discard.contains(&card(9)));
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn elemental_attack_overcoming_previous_players_last_element_doubles_damage() {
    let mut record = record_after_p1_metal_attack_on_turn_1();

    assert_event_semantics_eq!(
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
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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

    assert_event_semantics_eq!(
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
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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

    assert_event_semantics_eq!(
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
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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

    assert_event_semantics_eq!(
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
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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
    state.phase = Phase::ActiveEffects;
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

    assert_event_semantics_eq!(
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
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
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

#[test]
fn barrier_weapon_matrix_applies_physical_double_shield_damage_without_hp_loss() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[2, 7, 1, 4, 6, 11, 3, 5, 8]);

    // 基準：P1 普通合法行動後，相同的實體 Weapon 命令沒有護盾吸收其傷害。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(9));
    let baseline_weapon = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(6), card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                formation_id: attack_formation,
                used_cards,
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -12, new_hp: 18, effective_delta: -12 },
                shield_change: None,
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "weapon"
            && cards == &vec![card(6), card(11)]
            && attacker == &p2
            && target == &p1
            && attack_formation == "weapon"
            && used_cards == &vec![card(6), card(11)]
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "weapon"
            && discarded_cards == &vec![card(6), card(11)]
    ));
    assert_eq!(baseline.state().shield(&p1), Some(0));
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(18)
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：P1 透過相同的合法行動位置建立 Barrier；四張卡牌的等級總和提供
    // 44 點護盾。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    let barrier = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: vec![card(2), card(7), card(1), card(4)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        barrier.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::ShieldChanged { player: shield_owner, old_value: 0, delta: 44, new_value: 44 },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "barrier"
            && cards == &vec![card(2), card(7), card(1), card(4)]
            && shield_owner == &p1
            && discarded_by == &p1
            && discarded == "barrier"
            && discarded_cards == &vec![card(2), card(7), card(1), card(4)]
    ));
    assert_eq!(interaction.state().shield(&p1), Some(44));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(9));

    // 互動：實體傷害只會對護盾加倍。Weapon 陣形仍正常提交/棄置，並在將 44 降至
    // 20 的同時讓生命值保持不變。
    let weapon = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(6), card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                formation_id: attack_formation,
                used_cards,
                point_breakdown: AttackPointBreakdown { base_points: 12, final_amount: 12, .. },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: 0, new_hp: 30, effective_delta: 0 },
                shield_change: Some(ShieldChangeDelta { player: shield_owner, old_value: 44, delta: -24, new_value: 20 }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "weapon"
            && cards == &vec![card(6), card(11)]
            && attacker == &p2
            && target == &p1
            && attack_formation == "weapon"
            && used_cards == &vec![card(6), card(11)]
            && team == &TeamId::new("team:p1")
            && shield_owner == &p1
            && discarded_by == &p2
            && discarded == "weapon"
            && discarded_cards == &vec![card(6), card(11)]
    ));
    assert_eq!(interaction.state().shield(&p1), Some(20));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(30)
    );
    for card in [card(2), card(7), card(1), card(4), card(6), card(11)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}
