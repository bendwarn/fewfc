use fewfc::application::{
    AutomaticReason, EventSource, GameRecord, advance_automatic, apply_event, handle_command,
    resolve_trusted_randomness,
};
use fewfc::domain::{
    CONFLUENCE_GENERATION_MODULE_ID, CardInstanceId, ChoiceAnswer, Command, CounterEffect,
    DARK_GLIMMER_MODULE_ID, ECHO_MODULE_ID, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID,
    FormationAreaState, FormationInArea, FormationSuppression, GameError, GameEvent, GameSetup,
    GameState, HERO_SCHOOLS_MODULE_ID, JIANGHU_MODULE_ID, JianghuState, JianghuStateKind,
    PERSONAL_DECK_MODULE_ID, PassiveFlipOutcome, PassiveNoEffectGround, PassiveTriggerTiming,
    PendingResolution, Phase, PlayerId, PreparedProfessionAbility, RuleModuleId, SPIRIT_MODULE_ID,
    STAR_MODULE_ID, ScheduledEcho, StarKind, StatusDuration, StatusEffect, StatusOwner, TeamId,
    TeamStar, TimedEffectReduction, TrustedRandomnessAnswer, ValidationError,
};
use fewfc::public_view::{PublicPendingChoice, Viewer, state_for};
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
    let rules = OfficialRules::new();
    let mut modules = dependencies();
    modules.push(RuleModuleId::new(ECHO_MODULE_ID));
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
        GameEvent::HpChanged { change } if change.old_hp == 10 && change.new_hp == 0
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
}

#[test]
fn flow_layers_stack_but_only_one_can_trigger_during_a_turn_draw() {
    let mut state = configured_state();
    state.flow_layers_by_player.insert(PlayerId::new("p1"), 2);
    state.phase = Phase::TurnDraw;
    state.hands[0].cards = vec![card(1)];
    state.deck = (2..=18).map(card).collect();

    let events = advance_automatic(&state).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GameEvent::FlowStateTriggered { .. }))
            .count(),
        1
    );
    let mut projected = state.clone();
    apply_all(&mut projected, &events);
    assert_eq!(
        projected.flow_layers_by_player.get(&PlayerId::new("p1")),
        Some(&1)
    );
    assert_eq!(
        projected
            .turn_draw_bonus_by_player
            .get(&PlayerId::new("p1")),
        Some(&1)
    );
}

#[test]
fn split_earth_selects_a_catalog_formation_and_only_its_next_use_is_ineffective() {
    let mut state = configured_state();
    state.hands[0].cards = vec![card(73), card(74), card(75)];
    state.hands[1].cards = vec![card(1), card(2)];

    let performed = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:split-earth".to_string(),
            cards: vec![card(73), card(74)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(performed
        .iter()
        .any(|event| matches!(event, GameEvent::HandInspected { target, .. } if target == &PlayerId::new("p2"))));
    apply_all(&mut state, &performed);

    let selected = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Formation {
            formation_id: "weapon".to_string(),
        },
    )
    .unwrap();
    assert!(selected.iter().any(|event| matches!(
        event,
        GameEvent::FormationSuppressionSet { suppression }
            if suppression.target == PlayerId::new("p2")
                && suppression.formation_id == "weapon"
                && suppression.expires_on_turn_number == 2
    )));
    assert!(
        selected
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
    apply_all(&mut state, &selected);

    let declined = answer_choice(&state, PlayerId::new("p1"), ChoiceAnswer::Decline).unwrap();
    apply_all(&mut state, &declined);

    state.current_turn_index = 1;
    state.turn_number = 2;
    state.phase = Phase::ActiveEffects;
    let old_hp = state.hp[0].hp;
    let suppressed = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(2)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(
        suppressed
            .iter()
            .any(|event| matches!(event, GameEvent::FormationEffectIgnored { .. }))
    );
    let mut projected = state.clone();
    apply_all(&mut projected, &suppressed);
    assert_eq!(projected.hp[0].hp, old_hp);
    assert_eq!(projected.phase, Phase::TurnDraw);
}

#[test]
fn an_ineffective_melody_neither_runs_its_main_effect_nor_offers_echo() {
    let mut state = configured_state();
    state.hp[0].hp = 170;
    state.hands[0].cards = vec![card(19), card(20), card(21)];
    state.formation_suppressions.push(FormationSuppression {
        source: PlayerId::new("p2"),
        target: PlayerId::new("p1"),
        formation_id: "echo:falling-wood".to_string(),
        expires_on_turn_number: 1,
    });

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:falling-wood".to_string(),
            cards: vec![card(19), card(20)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationEffectIgnored { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::HpChanged { .. }))
    );
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
}

#[test]
fn ringing_metal_reveals_then_shuffles_the_remainder_before_placing_it_on_top() {
    let mut state = configured_state();
    state.hands[0].cards = vec![card(1), card(2), card(19)];
    state.deck = vec![card(3), card(4), card(5)];

    let performed = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:ringing-metal".to_string(),
            cards: vec![card(1), card(2)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &performed);
    assert!(matches!(
        state_for(&state, Viewer::Observer).pending_choice,
        Some(PublicPendingChoice::Hidden { .. })
    ));
    assert!(matches!(
        state_for(&state, Viewer::Player(PlayerId::new("p1"))).pending_choice,
        Some(PublicPendingChoice::Visible { .. })
    ));

    let selected = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: vec![card(4)],
        },
    )
    .unwrap();
    apply_all(&mut state, &selected);
    assert_eq!(
        state
            .pending_randomness
            .as_ref()
            .map(|request| request.current_order.clone()),
        Some(vec![card(3), card(5)])
    );

    let request = state.pending_randomness.clone().unwrap();
    let shuffled = resolve_trusted_randomness(
        &state,
        &TrustedRandomnessAnswer {
            request_id: request.request_id,
            shuffled_order: vec![card(5), card(3)],
        },
    )
    .unwrap();
    apply_all(&mut state, &shuffled);
    assert_eq!(state.deck, vec![card(4), card(5), card(3)]);
    assert!(
        shuffled
            .iter()
            .any(|event| matches!(event, GameEvent::RingingMetalCompleted { .. }))
    );
    assert!(
        shuffled
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
}

#[test]
fn ringing_metal_empty_deck_recycles_before_an_independent_post_search_shuffle() {
    let mut state = configured_state();
    state.hands[0].cards = vec![card(1), card(2), card(19)];
    state.deck.clear();
    // 隨機性請求等待期間，陣形卡牌仍留在陣形區，因此只有可回收的棄牌必須
    // 滿足要求的抽牌數量。
    state.discard = vec![card(3), card(4), card(5), card(6)];

    let performed = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:ringing-metal".to_string(),
            cards: vec![card(1), card(2)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &performed);
    let recycle = state.pending_randomness.clone().unwrap();
    assert_eq!(
        recycle.current_order,
        vec![card(3), card(4), card(5), card(6)]
    );

    let recycled = resolve_trusted_randomness(
        &state,
        &TrustedRandomnessAnswer {
            request_id: recycle.request_id,
            shuffled_order: vec![card(4), card(3), card(5), card(6)],
        },
    )
    .unwrap();
    apply_all(&mut state, &recycled);
    assert!(state.discard.is_empty());
    assert!(state.pending_choice.is_some());

    let selected = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: vec![card(4)],
        },
    )
    .unwrap();
    apply_all(&mut state, &selected);
    let post_search = state.pending_randomness.as_ref().unwrap();
    assert_eq!(
        state.pending_resolution,
        Some(PendingResolution::EchoRingingMetalPostSearch)
    );
    assert_eq!(post_search.current_order, vec![card(3), card(5), card(6)]);
}

#[test]
fn ringing_metal_uses_the_performing_players_personal_deck() {
    let mut state = configured_state();
    state
        .enabled_rule_modules
        .push(RuleModuleId::new(PERSONAL_DECK_MODULE_ID));
    state.hands[0].cards = vec![card(1), card(2), card(19)];
    state.deck = vec![card(70)];
    state.player_decks[0].cards = vec![card(3), card(4)];

    let performed = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:ringing-metal".to_string(),
            cards: vec![card(1), card(2)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    apply_all(&mut state, &performed);
    let choice = state.pending_choice.as_ref().unwrap();
    assert!(matches!(
        &choice.kind,
        fewfc::domain::PendingChoiceKind::Card { cards, .. }
            if cards == &vec![card(3), card(4)]
    ));

    let selected = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: vec![card(3)],
        },
    )
    .unwrap();
    apply_all(&mut state, &selected);
    assert!(matches!(
        state
            .pending_randomness
            .as_ref()
            .map(|request| request.operation.destination_deck()),
        Some(fewfc::domain::RandomnessDeck::Player(player))
            if player == &PlayerId::new("p1")
    ));
    assert_eq!(state.deck, vec![card(70)]);
}

#[test]
fn split_earth_echo_resolves_a_fresh_choice_without_scheduling_again() {
    let mut state = configured_state();
    state.phase = Phase::TurnStart;
    state.hands[1].cards = vec![card(1), card(2)];
    state.scheduled_echoes.push(ScheduledEcho {
        player: PlayerId::new("p1"),
        melody_id: "echo:split-earth".to_string(),
        due_turn_number: 1,
    });

    let started = advance_automatic(&state).unwrap();
    assert!(
        started
            .iter()
            .any(|event| matches!(event, GameEvent::EchoResolutionStarted { .. }))
    );
    assert!(
        started
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
    assert!(
        !started
            .iter()
            .any(|event| matches!(event, GameEvent::TurnStarted { .. }))
    );
    apply_all(&mut state, &started);

    let answered = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Formation {
            formation_id: "weapon".to_string(),
        },
    )
    .unwrap();
    assert!(
        answered
            .iter()
            .any(|event| matches!(event, GameEvent::EchoResolutionCompleted { .. }))
    );
    assert!(
        !answered
            .iter()
            .any(|event| matches!(event, GameEvent::EchoScheduled { .. }))
    );
    assert!(
        !answered
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
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
    let rules = OfficialRules::new();
    let mut modules = dependencies();
    modules.push(RuleModuleId::new(ECHO_MODULE_ID));
    let shape = GameSetup::team_mode(
        TeamId::new("a"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("b"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        30,
    );
    let setup = rules
        .configure_game(shape.players, shape.turn_order, modules)
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    state.hands[0].cards = vec![card(73), card(74)];
    state.hands[1].cards = vec![card(1)];

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:split-earth".to_string(),
            cards: vec![card(73), card(74)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HandInspected { target, .. } if target == &PlayerId::new("p2")
    )));
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
    let mut state = configured_state();
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

    let answered = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Player {
            player: PlayerId::new("p2"),
        },
    )
    .unwrap();
    assert!(matches!(
        answered.as_slice(),
        [
            GameEvent::ChoiceMade { .. },
            GameEvent::TimedEffectsReduced { reductions, .. },
            GameEvent::EchoScheduled { .. },
            ..
        ] if reductions.is_empty()
    ));
    assert!(
        !answered
            .iter()
            .any(|event| matches!(event, GameEvent::EchoCostPaid { .. }))
    );
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
    let mut state = configured_state();
    state.scheduled_echoes.push(ScheduledEcho {
        player: PlayerId::new("p1"),
        melody_id: "echo:pure-fire".to_string(),
        due_turn_number: 3,
    });
    state.flow_layers_by_player.insert(PlayerId::new("p2"), 1);
    state.turn_number = 3;
    state.current_turn_index = 0;
    state.phase = Phase::TurnStart;

    let started = advance_automatic(&state).unwrap();
    apply_all(&mut state, &started);
    let answered = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Player {
            player: PlayerId::new("p2"),
        },
    )
    .unwrap();
    assert!(answered.iter().any(|event| matches!(
        event,
        GameEvent::TimedEffectsReduced {
            target,
            reductions,
            ..
        } if target == &PlayerId::new("p2") && reductions.len() == 1
    )));
    assert!(
        answered
            .iter()
            .any(|event| matches!(event, GameEvent::EchoResolutionCompleted { .. }))
    );
    assert!(
        !answered
            .iter()
            .any(|event| matches!(event, GameEvent::EchoScheduled { .. }))
    );
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
        GameEvent::HpChanged { change } if change.new_hp == 0
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
    for melody_id in [
        "echo:ringing-metal",
        "echo:falling-wood",
        "echo:flowing-water",
        "echo:war-fire",
        "echo:split-earth",
    ] {
        let mut state = configured_state();
        state
            .scheduled_plant_earth
            .push(fewfc::domain::ScheduledPlantEarth {
                player: PlayerId::new("p1"),
                due_turn_number: 3,
            });
        state.turn_number = 3;
        state.current_turn_index = 0;
        state.phase = Phase::TurnStart;
        state.deck = vec![card(1), card(2)];
        state.hands[1].cards = vec![card(3)];

        let started = advance_automatic(&state).unwrap();
        apply_all(&mut state, &started);
        let selected = answer_choice(
            &state,
            PlayerId::new("p1"),
            ChoiceAnswer::Formation {
                formation_id: melody_id.to_string(),
            },
        )
        .unwrap();
        apply_all(&mut state, &selected);

        match melody_id {
            "echo:ringing-metal" => {
                let selected_card = answer_choice(
                    &state,
                    PlayerId::new("p1"),
                    ChoiceAnswer::Cards {
                        cards: vec![card(1)],
                    },
                )
                .unwrap();
                apply_all(&mut state, &selected_card);
                let request = state
                    .pending_randomness
                    .clone()
                    .expect("Ringing Metal must shuffle the remaining deck");
                let shuffled = resolve_trusted_randomness(
                    &state,
                    &TrustedRandomnessAnswer {
                        request_id: request.request_id,
                        shuffled_order: request.current_order,
                    },
                )
                .unwrap();
                apply_all(&mut state, &shuffled);
            }
            "echo:split-earth" => {
                let nested = answer_choice(
                    &state,
                    PlayerId::new("p1"),
                    ChoiceAnswer::Formation {
                        formation_id: "weapon".to_string(),
                    },
                )
                .unwrap();
                apply_all(&mut state, &nested);
            }
            _ => {}
        }

        assert!(
            state.active_plant_earth_resolution.is_none(),
            "{melody_id} must complete Plant Earth"
        );
        assert!(
            state.scheduled_echoes.is_empty(),
            "{melody_id} must not schedule Echo"
        );
    }
}

#[test]
fn plant_earth_schedules_a_fresh_turn_start_melody_without_echo() {
    let mut state = configured_state();
    state.hp[0].hp = 170;
    state.hands[0].cards = vec![card(27), card(85)];

    let performed = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "echo:plant-earth".to_string(),
            cards: vec![card(27), card(85)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(
        performed
            .iter()
            .any(|event| matches!(event, GameEvent::PlantEarthScheduled { .. }))
    );
    assert!(
        !performed
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
    apply_all(&mut state, &performed);

    state.turn_number = 3;
    state.current_turn_index = 0;
    state.phase = Phase::TurnStart;
    let started = advance_automatic(&state).unwrap();
    assert!(
        started
            .iter()
            .any(|event| matches!(event, GameEvent::PlantEarthResolutionStarted { .. }))
    );
    assert!(
        !started
            .iter()
            .any(|event| matches!(event, GameEvent::TurnStarted { .. }))
    );
    apply_all(&mut state, &started);

    let selected = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Formation {
            formation_id: "echo:falling-wood".to_string(),
        },
    )
    .unwrap();
    assert!(
        selected
            .iter()
            .any(|event| matches!(event, GameEvent::HpChanged { .. }))
    );
    assert!(
        selected
            .iter()
            .any(|event| matches!(event, GameEvent::PlantEarthResolutionCompleted { .. }))
    );
    assert!(
        !selected
            .iter()
            .any(|event| matches!(event, GameEvent::EchoScheduled { .. }))
    );
    assert!(
        !selected
            .iter()
            .any(|event| matches!(event, GameEvent::ChoiceRequested { .. }))
    );
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
