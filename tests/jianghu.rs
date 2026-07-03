use fewfc::application::{advance_automatic, apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent, GameState,
    HERO_SCHOOLS_MODULE_ID, JIANGHU_MODULE_ID, JianghuState, JianghuStateKind, Phase, Player,
    PlayerId, PlayerProfession, ProfessionId, RuleModuleId, STAR_MODULE_ID, TeamId,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};

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
    state.phase = Phase::Main;
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
                            definition.element == *element && definition.level == *level
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
fn thousand_blades_applies_immediately_and_replays_as_typed_state() {
    let mut game = state();
    set_profession(&mut game, "p1", "jianghu:swordsman");
    let used = cards(&game, &[(Element::Metal, 2), (Element::Wood, 2)]);
    set_hand(&mut game, "p1", used.clone());

    let events = handle_command(
        &game,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "jianghu:thousand-blades-sword-art".to_string(),
            cards: used,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { point_breakdown, .. }
            if point_breakdown.base_points == 18
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::JianghuStateApplied { state }
            if state.kind == JianghuStateKind::ThousandBlades
    )));

    for event in &events {
        apply_event(&mut game, event);
    }
    assert!(game.jianghu_states.iter().any(|state| {
        state.owner == PlayerId::new("p1") && state.kind == JianghuStateKind::ThousandBlades
    }));
}

#[test]
fn poison_stacks_ticks_at_affected_players_turn_end_and_uses_poison_mastery() {
    let mut game = state();
    set_profession(&mut game, "p1", "jianghu:poison-saint");
    game.current_turn_index = 1;
    game.phase = Phase::TurnEnd;
    game.jianghu_states.push(JianghuState {
        owner: PlayerId::new("p2"),
        kind: JianghuStateKind::Poison,
        remaining_turns: 3,
        expires_on_turn: None,
        last_resolved_turn: None,
    });

    let events = advance_automatic(&game).unwrap();
    assert!(matches!(
        events.first(),
        Some(GameEvent::JianghuPoisonTicked {
            owner,
            damage: 15,
            remaining_turns: 2,
            ..
        }) if owner == &PlayerId::new("p2")
    ));
    for event in &events {
        apply_event(&mut game, event);
    }
    assert_eq!(
        game.jianghu_states
            .iter()
            .find(|active| active.kind == JianghuStateKind::Poison)
            .unwrap()
            .remaining_turns,
        2
    );
}
