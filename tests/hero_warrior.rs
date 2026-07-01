use fewfc::application::{apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, Command, Element, GameError, GameEvent, GameState, HERO_SCHOOLS_MODULE_ID,
    Phase, Player, PlayerId, PlayerProfession, ProfessionId, RuleModuleId, TeamId, ValidationError,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};

fn state(player_count: usize, hero_enabled: bool) -> GameState {
    let players = (1..=player_count)
        .map(|index| Player {
            id: PlayerId::new(format!("p{index}")),
            team: TeamId::new(if index % 2 == 1 { "team:a" } else { "team:b" }),
        })
        .collect::<Vec<_>>();
    let turn_order = players.iter().map(|player| player.id.clone()).collect();
    let modules = hero_enabled
        .then(|| vec![RuleModuleId::new(HERO_SCHOOLS_MODULE_ID)])
        .unwrap_or_default();
    let setup = OfficialRules::new()
        .configure_game(players, turn_order, modules)
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

fn change_profession(
    state: &mut GameState,
    profession: &str,
    cards: Vec<CardInstanceId>,
) -> Vec<GameEvent> {
    let events = handle_command(
        state,
        Command::ChangeProfession {
            player: PlayerId::new("p1"),
            profession: ProfessionId::new(profession),
            cards,
        },
    )
    .unwrap();
    for event in &events {
        apply_event(state, event);
    }
    events
}

fn perform(state: &GameState, formation_id: &str, cards: Vec<CardInstanceId>) -> Vec<GameEvent> {
    handle_command(
        state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: formation_id.to_string(),
            cards,
            declared_targets: Vec::new(),
        },
    )
    .unwrap()
}

#[test]
fn profession_change_progresses_in_order_and_replays_explicit_card_moves() {
    let mut state = state(2, true);
    assert!(state.professions.is_empty());
    assert!(state_for(&state, Viewer::Observer).professions.is_empty());

    let warrior_cards = cards(&state, &[(Element::Metal, 1), (Element::Metal, 2)]);
    set_hand(&mut state, "p1", warrior_cards.clone());
    let candidates = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &warrior_cards)
        .unwrap();
    assert!(candidates.iter().any(|candidate| matches!(
        candidate,
        PlayableAction::ChangeProfession(candidate)
            if candidate.profession_id == ProfessionId::new("warrior")
                && candidate.cards == warrior_cards
    )));

    let before = state.clone();
    let events = change_profession(&mut state, "warrior", warrior_cards.clone());
    assert!(matches!(
        events.last(),
        Some(GameEvent::ProfessionChanged {
            previous: None,
            profession,
            card_moves,
            ..
        }) if profession == &ProfessionId::new("warrior")
            && card_moves.len() == warrior_cards.len()
    ));
    let mut replayed = before;
    let persisted = serde_json::to_string(&events).unwrap();
    let restored: Vec<GameEvent> = serde_json::from_str(&persisted).unwrap();
    for event in &restored {
        apply_event(&mut replayed, event);
    }
    assert_eq!(replayed, state);
    assert_eq!(
        state.profession_for(&PlayerId::new("p1")),
        Some(&ProfessionId::new("warrior"))
    );

    state.phase = Phase::Main;
    let war_god_cards = cards(&state, &[(Element::Metal, 1), (Element::Metal, 5)]);
    set_hand(&mut state, "p1", war_god_cards.clone());
    change_profession(&mut state, "war-god", war_god_cards);
    assert_eq!(
        state.profession_for(&PlayerId::new("p1")),
        Some(&ProfessionId::new("war-god"))
    );

    state.phase = Phase::Main;
    let hero_cards = cards(&state, &[(Element::Metal, 4), (Element::Metal, 5)]);
    set_hand(&mut state, "p1", hero_cards.clone());
    change_profession(&mut state, "hero", hero_cards);
    assert_eq!(
        state.profession_for(&PlayerId::new("p1")),
        Some(&ProfessionId::new("hero"))
    );

    let public = state_for(&state, Viewer::Observer);
    assert_eq!(public.professions, state.professions);
}

#[test]
fn profession_change_rejects_skipping_and_hero_disabled_games() {
    let mut enabled = state(2, true);
    let metal = cards(&enabled, &[(Element::Metal, 5), (Element::Metal, 5)]);
    set_hand(&mut enabled, "p1", metal.clone());
    assert!(matches!(
        handle_command(
            &enabled,
            Command::ChangeProfession {
                player: PlayerId::new("p1"),
                profession: ProfessionId::new("war-god"),
                cards: metal,
            },
        ),
        Err(GameError::Validation(
            ValidationError::ProfessionPrerequisiteNotMet { .. }
        ))
    ));

    let mut disabled = state(2, false);
    let metal = cards(&disabled, &[(Element::Metal, 3)]);
    set_hand(&mut disabled, "p1", metal.clone());
    assert!(matches!(
        handle_command(
            &disabled,
            Command::ChangeProfession {
                player: PlayerId::new("p1"),
                profession: ProfessionId::new("warrior"),
                cards: metal,
            },
        ),
        Err(GameError::Validation(ValidationError::HeroSchoolsDisabled))
    ));
}

#[test]
fn profession_breaking_is_public_and_replayable() {
    let mut state = state(2, true);
    set_profession(&mut state, "p1", "warrior");
    let event = GameEvent::ProfessionBroken {
        player: PlayerId::new("p1"),
        profession: ProfessionId::new("warrior"),
    };

    assert!(matches!(
        fewfc::public_view::event_for(&event, Viewer::Observer),
        fewfc::public_view::PublicGameEvent::Public(GameEvent::ProfessionBroken { .. })
    ));
    apply_event(&mut state, &event);
    assert_eq!(state.profession_for(&PlayerId::new("p1")), None);
    assert!(state_for(&state, Viewer::Observer).professions.is_empty());
}

#[test]
fn warrior_proficiencies_keep_original_formation_identity_and_effect() {
    let mut state = state(2, true);
    set_profession(&mut state, "p1", "war-god");
    let weapon_cards = cards(&state, &[(Element::Metal, 1), (Element::Wood, 1)]);
    set_hand(&mut state, "p1", weapon_cards.clone());

    let actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &weapon_cards)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "weapon"
    )));
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "divine-weapon"
    )));

    let events = perform(&state, "weapon", weapon_cards);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { formation_id, .. } if formation_id == "weapon"
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::TurnDrawBonusChanged { delta: 1, .. }))
    );
}

#[test]
fn warrior_and_hero_damage_hooks_modify_only_matching_damage() {
    let mut warrior = state(2, true);
    set_profession(&mut warrior, "p2", "warrior");
    let shock = cards(
        &warrior,
        &[
            (Element::Fire, 1),
            (Element::Fire, 2),
            (Element::Water, 1),
            (Element::Earth, 1),
        ],
    );
    set_hand(&mut warrior, "p1", shock.clone());
    let events = perform(&warrior, "shock-burst", shock);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown,
            hp_change,
            ..
        } if point_breakdown.base_points == 20
            && point_breakdown.final_amount == 10
            && hp_change.delta == -10
    )));

    let mut hero = state(2, true);
    set_profession(&mut hero, "p2", "hero");
    let metal = cards(&hero, &[(Element::Metal, 1)]);
    set_hand(&mut hero, "p1", metal.clone());
    let events = perform(&hero, "metal-strike", metal);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown,
            hp_change,
            ..
        } if point_breakdown.final_amount == 0 && hp_change.delta == 0
    )));
}

#[test]
fn shield_receives_unreduced_damage_before_warrior_resistance() {
    let mut state = state(2, true);
    set_profession(&mut state, "p2", "warrior");
    state
        .shields
        .iter_mut()
        .find(|shield| shield.player == PlayerId::new("p2"))
        .unwrap()
        .value = 50;
    let shock = cards(
        &state,
        &[
            (Element::Fire, 1),
            (Element::Fire, 2),
            (Element::Water, 1),
            (Element::Earth, 1),
        ],
    );
    set_hand(&mut state, "p1", shock.clone());

    assert!(
        perform(&state, "shock-burst", shock)
            .iter()
            .any(|event| matches!(
                event,
                GameEvent::AttackResolved {
                    point_breakdown,
                    hp_change,
                    shield_change: Some(shield_change),
                    ..
                } if point_breakdown.final_amount == 20
                    && hp_change.delta == 0
                    && shield_change.delta == -40
            ))
    );
}

#[test]
fn hero_formations_and_battle_soul_work_in_four_player_games() {
    let mut state = state(4, true);
    set_profession(&mut state, "p1", "hero");
    let slash = cards(
        &state,
        &[
            (Element::Metal, 1),
            (Element::Metal, 2),
            (Element::Fire, 1),
            (Element::Fire, 2),
        ],
    );
    set_hand(&mut state, "p1", slash.clone());
    assert!(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &slash)
            .unwrap()
            .iter()
            .any(|action| matches!(
                action,
                PlayableAction::PerformFormation(candidate)
                    if candidate.formation_id == "falling-light-slash"
            ))
    );
    assert!(
        perform(&state, "falling-light-slash", slash)
            .iter()
            .any(|event| matches!(
                event,
                GameEvent::AttackResolved {
                    point_breakdown,
                    ..
                } if point_breakdown.base_points == 36
            ))
    );

    let shock = cards(
        &state,
        &[
            (Element::Fire, 1),
            (Element::Fire, 2),
            (Element::Water, 1),
            (Element::Metal, 1),
        ],
    );
    set_hand(&mut state, "p1", shock.clone());
    let actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &shock)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "shock-burst"
    )));
    let events = perform(&state, "shock-burst", shock);
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GameEvent::StatusAdded { .. }))
            .count(),
        6
    );
}
