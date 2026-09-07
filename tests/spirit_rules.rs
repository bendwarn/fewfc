use fewfc::application::{GameRecord, apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, ChoiceAnswer, ChoiceId, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID,
    FormationAreaState, FormationInArea, GameError, GameEvent, GameOutcome, GameSetup, GameState,
    GameStatus, HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID, PassiveTriggerTiming,
    PendingChoice, PendingChoiceKind, PendingResolution, Phase, Player, PlayerId, PlayerProfession,
    PlayerSpirit, ProfessionId, RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID, SpiritKind,
    SpiritSkill, StarKind, StatusDuration, StatusEffect, StatusOwner, TargetDecl, TeamId, TeamStar,
    ValidationError,
};
use fewfc::public_view::{PublicGameEvent, Viewer, event_for, state_for};
use fewfc::rules::OfficialRules;

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

fn cover(state: &mut GameState, owner: &str, formation_id: &str, cards: Vec<CardInstanceId>) {
    state
        .formation_area_mut(&PlayerId::new(owner))
        .unwrap()
        .formation = Some(FormationInArea {
        formation_id: formation_id.to_string(),
        cards,
        star_substitution: None,
        state: FormationAreaState::FaceDownWaiting {
            sealed: false,
            revealed: false,
            neutralized: false,
            trigger_timing: PassiveTriggerTiming::NextPlayerActionStart,
        },
    });
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

fn give_spirit(state: &mut GameState, spirit: SpiritKind, power: u32) {
    apply_event(
        state,
        &GameEvent::SpiritSummoned {
            player: PlayerId::new("p1"),
            previous: state
                .spirit_for(&PlayerId::new("p1"))
                .map(|owned| owned.spirit),
            spirit,
        },
    );
    state.spirits[0].power = power;
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
    let mut state = spirit_state();
    let metal_cards = vec![
        card(&state, Element::Metal, 1),
        card(&state, Element::Metal, 2),
    ];
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = metal_cards.clone();

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-spirit-summoning".to_string(),
            cards: metal_cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(matches!(
        events
            .iter()
            .find(|event| matches!(event, GameEvent::SpiritSummoned { .. })),
        Some(&GameEvent::SpiritSummoned {
            previous: None,
            spirit: SpiritKind::Metal,
            ..
        })
    ));
    apply_all(&mut state, &events);
    assert_eq!(state.spirit_for(&PlayerId::new("p1")).unwrap().power, 2);

    state.phase = Phase::ActiveEffects;
    let wood_cards = vec![
        card(&state, Element::Wood, 1),
        card(&state, Element::Wood, 2),
    ];
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = wood_cards.clone();
    let replacement = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "wood-spirit-summoning".to_string(),
            cards: wood_cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(matches!(
        replacement
            .iter()
            .find(|event| matches!(event, GameEvent::SpiritSummoned { .. })),
        Some(&GameEvent::SpiritSummoned {
            previous: Some(SpiritKind::Metal),
            spirit: SpiritKind::Wood,
            ..
        })
    ));
    apply_all(&mut state, &replacement);
    assert_eq!(
        state.spirit_for(&PlayerId::new("p1")).unwrap().spirit,
        SpiritKind::Wood
    );
    assert_eq!(state.spirit_for(&PlayerId::new("p1")).unwrap().power, 2);
}

#[test]
fn matching_turn_discard_charges_only_the_owners_spirit_to_six() {
    let mut state = spirit_state();
    apply_event(
        &mut state,
        &GameEvent::SpiritSummoned {
            player: PlayerId::new("p1"),
            previous: None,
            spirit: SpiritKind::Metal,
        },
    );
    state.spirits[0].power = 5;
    let discard = card(&state, Element::Metal, 3);
    state.turn_draw_pool = vec![discard];
    state.phase = Phase::TurnDraw;
    state.pending_choice = Some(PendingChoice {
        choice_id: ChoiceId::new(1),
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::Card {
            cards: vec![discard],
            minimum: 1,
            maximum: 1,
            can_decline: false,
        },
    });
    state.pending_resolution = Some(PendingResolution::TurnDrawDiscard);

    let events = handle_command(
        &state,
        Command::AnswerChoice {
            player: PlayerId::new("p1"),
            choice_id: ChoiceId::new(1),
            answer: ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        },
    )
    .unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::ChoiceMade { .. },
            GameEvent::TurnDrawResolved { .. },
            GameEvent::SpiritPowerChanged {
                old_power: 5,
                new_power: 6,
                ..
            }
        ]
    ));
    apply_all(&mut state, &events);
    assert_eq!(state.spirit_for(&PlayerId::new("p1")).unwrap().power, 6);
    assert!(state.spirit_for(&PlayerId::new("p2")).is_none());
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
    let rules = OfficialRules::new();
    let setup = rules
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
    let initial = GameState::from_setup(&setup);
    let cards = vec![
        card(&initial, Element::Fire, 1),
        card(&initial, Element::Fire, 2),
    ];
    let mut first_cards = cards.clone();
    first_cards.extend([
        card(&initial, Element::Metal, 1),
        card(&initial, Element::Wood, 1),
    ]);
    let mut record = GameRecord::start(setup.clone(), deck_starting_with(&setup, &first_cards))
        .expect("Spirit game should start");
    record.advance_until_decision().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "fire-spirit-summoning".to_string(),
            cards,
            declared_targets: Vec::new(),
        })
        .unwrap();

    assert_eq!(
        record
            .state()
            .spirit_for(&PlayerId::new("p1"))
            .map(|owned| owned.spirit),
        Some(SpiritKind::Fire)
    );
    assert_eq!(record.replay().unwrap(), *record.state());
    assert!(record.verify_replay().is_ok());

    let mut replayed = record.state().clone();
    apply_event(
        &mut replayed,
        &GameEvent::SpiritSummoned {
            player: PlayerId::new("p3"),
            previous: None,
            spirit: SpiritKind::Fire,
        },
    );
    assert_eq!(
        replayed
            .spirits
            .iter()
            .filter(|owned| owned.spirit == SpiritKind::Fire)
            .count(),
        2
    );
}

#[test]
fn metal_and_wood_skills_change_hp_consume_power_and_allow_ineffective_recovery() {
    let mut metal = spirit_state();
    give_spirit(&mut metal, SpiritKind::Metal, 6);
    let events = use_skill(&metal, SpiritSkill::FlyingBlade, None, None).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.delta() == -10
    )));
    apply_all(&mut metal, &events);
    assert_eq!(metal.spirit_for(&PlayerId::new("p1")).unwrap().power, 4);
    assert!(matches!(
        use_skill(&metal, SpiritSkill::FlyingBlade, None, None),
        Err(GameError::Validation(
            ValidationError::SpiritSkillAlreadyUsed { .. }
        ))
    ));

    let mut sword_rain = spirit_state();
    give_spirit(&mut sword_rain, SpiritKind::Metal, 6);
    let events = use_skill(&sword_rain, SpiritSkill::SwordRain, None, None).unwrap();
    assert!(matches!(
        events.last(),
        Some(GameEvent::SpiritBroken {
            spirit: SpiritKind::Metal,
            ..
        })
    ));
    apply_all(&mut sword_rain, &events);
    assert!(sword_rain.spirits.is_empty());

    let mut wood = spirit_state();
    give_spirit(&mut wood, SpiritKind::Wood, 2);
    let events = use_skill(&wood, SpiritSkill::Fragrance, None, None).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.effective_delta() == 0
    )));
    apply_all(&mut wood, &events);
    assert!(wood.spirits.is_empty());
}

#[test]
fn water_skills_move_cards_and_add_turn_draw_bonus() {
    let mut state = spirit_state();
    give_spirit(&mut state, SpiritKind::Water, 3);
    let selected = card(&state, Element::Fire, 1);
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = vec![selected];
    let events = use_skill(&state, SpiritSkill::Flow, Some(selected), None).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::CardsMoved { card_moves }
            if card_moves.iter().any(|card_move| card_move.card == selected)
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::TurnDrawBonusChanged { new_value: 1, .. }))
    );
    apply_all(&mut state, &events);
    assert!(
        !state
            .hand(&PlayerId::new("p1"))
            .unwrap()
            .contains(&selected)
    );
    assert!(state.discard.contains(&selected));

    let mut vastness = spirit_state();
    give_spirit(&mut vastness, SpiritKind::Water, 3);
    let events = use_skill(&vastness, SpiritSkill::Vastness, None, None).unwrap();
    apply_all(&mut vastness, &events);
    assert_eq!(
        vastness.turn_draw_bonus_by_player.get(&PlayerId::new("p1")),
        Some(&1)
    );
    assert!(vastness.spirits.is_empty());
}

#[test]
fn flow_moves_personal_cards_to_their_origin_discard() {
    let (players, turn_order) = players();
    let mut modules = spirit_modules();
    modules.push(RuleModuleId::new(PERSONAL_DECK_MODULE_ID));
    let setup = OfficialRules::new()
        .configure_game_with_decks(players, turn_order, modules, Vec::new())
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    give_spirit(&mut state, SpiritKind::Water, 1);
    let selected = state
        .card_instances
        .iter()
        .find(|instance| {
            matches!(
                &instance.origin,
                fewfc::domain::CardOrigin::Player(player)
                    if player == &PlayerId::new("p1")
            )
        })
        .unwrap()
        .instance;
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = vec![selected];

    let events = use_skill(&state, SpiritSkill::Flow, Some(selected), None).unwrap();
    apply_all(&mut state, &events);
    assert!(
        state
            .discard_for(&PlayerId::new("p1"))
            .unwrap()
            .contains(&selected)
    );
    assert!(!state.discard.contains(&selected));
}

#[test]
fn fire_interpretation_changes_points_star_qualification_and_redacts_the_card() {
    let mut state = spirit_state();
    give_spirit(&mut state, SpiritKind::Fire, 3);
    let interpreted = card(&state, Element::Metal, 1);
    let cards = vec![
        interpreted,
        card(&state, Element::Metal, 3),
        card(&state, Element::Metal, 5),
    ];
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = cards.clone();
    let skill_events =
        use_skill(&state, SpiritSkill::Splendor, Some(interpreted), Some(5)).unwrap();
    let canonical = skill_events
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
    assert!(matches!(
        event_for(canonical, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::SpiritSkillUsed {
            selected_card: Some(card),
            ..
        } if card == interpreted
    ));
    apply_all(&mut state, &skill_events);

    let attack_events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "triple-metal".to_string(),
            cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(attack_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { point_breakdown, .. }
            if point_breakdown.base_points == 39
    )));
    assert!(attack_events.iter().any(|event| matches!(
        event,
        GameEvent::StarSummoned {
            star: fewfc::domain::StarKind::Metal,
            ..
        }
    )));
}

#[test]
fn sacred_art_composes_with_fire_level_but_rejects_star_substitution() {
    let mut state = spirit_state();
    state.professions.push(PlayerProfession {
        player: PlayerId::new("p1"),
        profession: ProfessionId::new("saint"),
    });
    state.team_stars.push(TeamStar {
        team: TeamId::new("team:p1"),
        star: StarKind::Water,
    });
    give_spirit(&mut state, SpiritKind::Fire, 3);
    let wood = card(&state, Element::Wood, 1);
    let metal = card(&state, Element::Metal, 2);
    let fire = card(&state, Element::Fire, 3);
    let cards = vec![wood, metal, fire];
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = cards.clone();
    let skill_events = use_skill(&state, SpiritSkill::Splendor, Some(wood), Some(4)).unwrap();
    apply_all(&mut state, &skill_events);

    let actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &cards)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        fewfc::rules::PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "barrier"
                && candidate.declared_targets == vec![TargetDecl::CardMultiplicity {
                    card: wood,
                    slots: 2,
                }]
    )));

    assert!(matches!(
        handle_command(
            &state,
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "barrier".to_string(),
                cards,
                declared_targets: vec![
                    TargetDecl::CardMultiplicity {
                        card: wood,
                        slots: 2,
                    },
                    TargetDecl::Card(metal),
                ],
            },
        ),
        Err(GameError::Validation(
            ValidationError::FormationPatternMismatch { .. }
        ))
    ));
}

#[test]
fn spirit_skills_remain_legal_under_cannot_act_and_replacement_resets_the_allowance() {
    let mut state = spirit_state();
    give_spirit(&mut state, SpiritKind::Metal, 4);
    state.statuses.push(StatusEffect {
        id: "cannot-act".to_string(),
        owner: StatusOwner::Player(PlayerId::new("p1")),
        kind: "CannotAct".to_string(),
        value: None,
        duration: StatusDuration::UntilTurnEnd {
            player: PlayerId::new("p1"),
        },
    });
    let actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &[])
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        fewfc::rules::PlayableAction::UseSpiritSkill(candidate)
            if candidate.skill == SpiritSkill::FlyingBlade
    )));
    let first = use_skill(&state, SpiritSkill::FlyingBlade, None, None).unwrap();
    apply_all(&mut state, &first);
    apply_event(
        &mut state,
        &GameEvent::SpiritSummoned {
            player: PlayerId::new("p1"),
            previous: Some(SpiritKind::Metal),
            spirit: SpiritKind::Metal,
        },
    );
    assert!(use_skill(&state, SpiritSkill::FlyingBlade, None, None).is_ok());
}

#[test]
fn earth_skills_replace_shields_and_stone_shield_prevents_sacred_beast_damage_only() {
    let mut rock_wall = spirit_state();
    give_spirit(&mut rock_wall, SpiritKind::Earth, 6);
    rock_wall
        .shields
        .iter_mut()
        .find(|shield| shield.player == PlayerId::new("p1"))
        .unwrap()
        .value = 60;
    let wall_events = use_skill(&rock_wall, SpiritSkill::RockWall, None, None).unwrap();
    assert!(wall_events.iter().any(|event| matches!(
        event,
        GameEvent::ShieldChanged {
            old_value: 60,
            new_value: 40,
            ..
        }
    )));

    let mut stone = spirit_state();
    give_spirit(&mut stone, SpiritKind::Earth, 2);
    let stone_events = use_skill(&stone, SpiritSkill::StoneShield, None, None).unwrap();
    apply_all(&mut stone, &stone_events);
    stone.current_turn_index = 1;
    stone.turn_number += 1;
    stone.phase = Phase::ActiveEffects;
    let beast_cards = [1, 2, 3, 4, 5]
        .map(|level| card(&stone, Element::Metal, level))
        .to_vec();
    *stone.hand_mut(&PlayerId::new("p2")).unwrap() = beast_cards.clone();
    let attack = handle_command(
        &stone,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "west-white-tiger".to_string(),
            cards: beast_cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_changes, .. } if hp_changes.is_empty()
    )));
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(effects),
            ..
        } if effects.environment_transfers.iter().any(|transfer| transfer.to == Element::Metal)
    )));
}

#[test]
fn all_eligible_teammate_wood_spirits_bloom_atomically_and_cap_recovery() {
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
        PlayerSpirit {
            player: PlayerId::new("p4"),
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
    state
        .initial_hp
        .iter_mut()
        .find(|owned| owned.team == TeamId::new("team:b"))
        .unwrap()
        .hp = 50;

    let events = use_skill(&state, SpiritSkill::FlyingBlade, None, None).unwrap();
    assert!(matches!(
        events.last(),
        Some(GameEvent::AutomaticBloomsResolved { resolutions })
            if resolutions.len() == 1
                && resolutions[0].spirit_changes.len() == 2
                && resolutions[0].hp_change.delta() == 80
                && resolutions[0].hp_change.new_hp() == 50
    ));
    apply_all(&mut state, &events);
    assert_eq!(
        state
            .hp
            .iter()
            .find(|owned| owned.team == TeamId::new("team:b"))
            .unwrap()
            .hp,
        50
    );
    assert!(matches!(state.status, GameStatus::InProgress));
    assert!(state.spirits.is_empty());
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
    let mut state = team_spirit_state();
    state.spirits.push(PlayerSpirit {
        player: PlayerId::new("p1"),
        spirit: SpiritKind::Wood,
        power: 6,
    });
    state
        .hp
        .iter_mut()
        .find(|owned| owned.team == TeamId::new("team:a"))
        .unwrap()
        .hp = 5;
    state
        .star_histories
        .iter_mut()
        .find(|history| history.player == PlayerId::new("p1"))
        .unwrap()
        .stars = vec![
        StarKind::Wood,
        StarKind::Water,
        StarKind::Fire,
        StarKind::Earth,
    ];
    cover(&mut state, "p4", "countershock", Vec::new());
    let cards = vec![
        card(&state, Element::Metal, 3),
        card(&state, Element::Metal, 4),
        card(&state, Element::Metal, 5),
    ];
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = cards.clone();

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "triple-metal".to_string(),
            cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::FiveStarAlignmentAchieved { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::AutomaticBloomsResolved { .. }))
    );
    apply_all(&mut state, &events);
    assert!(matches!(
        state.status,
        GameStatus::Finished {
            ref conclusion
        } if conclusion.outcome == GameOutcome::Winner(TeamId::new("team:a"))
    ));
    assert_eq!(state.spirit_for(&PlayerId::new("p1")).unwrap().power, 6);
}

#[test]
fn void_spirit_shattering_resolves_power_breaking_hp_and_cards_as_one_event() {
    let mut state = team_spirit_state();
    state.spirits = vec![
        PlayerSpirit {
            player: PlayerId::new("p1"),
            spirit: SpiritKind::Wood,
            power: 6,
        },
        PlayerSpirit {
            player: PlayerId::new("p2"),
            spirit: SpiritKind::Metal,
            power: 2,
        },
        PlayerSpirit {
            player: PlayerId::new("p3"),
            spirit: SpiritKind::Fire,
            power: 4,
        },
        PlayerSpirit {
            player: PlayerId::new("p4"),
            spirit: SpiritKind::Earth,
            power: 1,
        },
    ];
    for team_hp in &mut state.hp {
        team_hp.hp = 40;
    }
    let cards = [Element::Metal, Element::Wood, Element::Water]
        .map(|element| card(&state, element, 3))
        .to_vec();
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = cards.clone();

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "void-spirit-shattering".to_string(),
            cards: cards.clone(),
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(matches!(
        events.iter().find(|event| matches!(event, GameEvent::VoidSpiritShatteringResolved { .. })),
        Some(GameEvent::VoidSpiritShatteringResolved {
            spirit_changes,
            broken_spirits,
            hp_changes,
            card_moves,
            ..
        }) if spirit_changes.len() == 4
            && broken_spirits.len() == 2
            && hp_changes.len() == 2
            && card_moves.is_empty()
    ));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::AutomaticBloomsResolved { .. }))
    );

    let mut replayed = state.clone();
    apply_all(&mut state, &events);
    apply_all(&mut replayed, &events);
    assert_eq!(replayed, state);
    assert!(matches!(
        state.status,
        GameStatus::Finished {
            ref conclusion
        } if conclusion.outcome == GameOutcome::Draw
    ));
    assert_eq!(state.spirit_for(&PlayerId::new("p1")).unwrap().power, 4);
    assert_eq!(state.spirit_for(&PlayerId::new("p3")).unwrap().power, 2);
    assert!(state.spirit_for(&PlayerId::new("p2")).is_none());
    assert!(state.spirit_for(&PlayerId::new("p4")).is_none());
    assert_eq!(
        state
            .formation_area(&PlayerId::new("p1"))
            .and_then(|area| area.formation.as_ref())
            .map(|formation| formation.cards.as_slice()),
        Some(cards.as_slice())
    );
}

#[test]
fn seal_cancels_void_spirit_shattering_but_still_discards_the_formation() {
    let mut state = team_spirit_state();
    state.spirits.push(PlayerSpirit {
        player: PlayerId::new("p1"),
        spirit: SpiritKind::Wood,
        power: 6,
    });
    cover(&mut state, "p4", "seal", Vec::new());
    let cards = [Element::Metal, Element::Wood, Element::Water]
        .map(|element| card(&state, element, 2))
        .to_vec();
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = cards.clone();

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "void-spirit-shattering".to_string(),
            cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCommitted { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::VoidSpiritShatteringResolved { .. }))
    );
}
