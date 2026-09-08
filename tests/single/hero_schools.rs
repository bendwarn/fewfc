use fewfc::application::{apply_event, handle_command, resolve_trusted_randomness};
use fewfc::domain::{
    CardInstanceId, CardOrigin, ChoiceAnswer, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID,
    GameError, GameEvent, GameState, HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID,
    PassiveTriggerTiming, Phase, Player, PlayerId, PlayerProfession, ProfessionId, RuleModuleId,
    STAR_MODULE_ID, StatusDuration, StatusEffect, StatusOwner, TargetDecl, TeamId,
    TrustedRandomnessAnswer, ValidationError,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{ActionInputRequirement, OfficialRules, PlayableAction};

fn game_state(modules: &[&str]) -> GameState {
    let setup = OfficialRules::new()
        .configure_game(
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
            modules
                .iter()
                .map(|module| RuleModuleId::new(*module))
                .collect(),
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
    *state.hand_mut(&PlayerId::new(player)).unwrap() = cards;
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

fn give_temporary_profession_ability_loss(state: &mut GameState, player: &str) {
    let player = PlayerId::new(player);
    state.statuses.push(StatusEffect {
        id: format!("test-lure-{}", player.as_str()),
        owner: StatusOwner::Player(player.clone()),
        kind: "PouchLurePlayer".to_string(),
        value: None,
        duration: StatusDuration::UntilTurnEnd {
            player: player.clone(),
        },
    });
}

fn apply_all(state: &mut GameState, events: &[GameEvent]) {
    for event in events {
        apply_event(state, event);
    }
}

fn formation_candidates(
    state: &GameState,
    selected: &[CardInstanceId],
    formation_id: &str,
) -> Vec<fewfc::rules::FormationCandidate> {
    OfficialRules::new()
        .playable_actions(state, &PlayerId::new("p1"), selected)
        .unwrap()
        .into_iter()
        .filter_map(|action| match action {
            PlayableAction::PerformFormation(candidate)
                if candidate.formation_id == formation_id =>
            {
                Some(candidate)
            }
            _ => None,
        })
        .collect()
}

fn perform(
    state: &GameState,
    formation_id: &str,
    cards: Vec<CardInstanceId>,
    declared_targets: Vec<TargetDecl>,
) -> Result<Vec<GameEvent>, GameError> {
    handle_command(
        state,
        Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: formation_id.to_string(),
            cards,
            declared_targets,
        },
    )
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

#[test]
fn immortal_chaos_grants_one_draw_after_chaos_return_two_completes() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "immortal");
    let chaos = cards(
        &state,
        &[
            (Element::Earth, 1),
            (Element::Earth, 2),
            (Element::Wood, 1),
            (Element::Metal, 1),
        ],
    );
    let selected_for_return = cards(&state, &[(Element::Fire, 1), (Element::Water, 1)]);
    set_hand(&mut state, "p1", chaos.clone());
    set_hand(&mut state, "p2", selected_for_return.clone());

    let formation_events = perform(&state, "chaos", chaos, Vec::new()).unwrap();
    assert!(
        !formation_events
            .iter()
            .any(|event| matches!(event, GameEvent::TurnDrawBonusChanged { .. }))
    );
    apply_all(&mut state, &formation_events);

    let completion_events = answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: selected_for_return,
        },
    )
    .unwrap();
    assert_eq!(
        completion_events
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::TurnDrawBonusChanged {
                    player,
                    old_value: 0,
                    delta: 1,
                    new_value: 1,
                } if player == &PlayerId::new("p1")
            ))
            .count(),
        1
    );
    apply_all(&mut state, &completion_events);
    assert_eq!(
        state.turn_draw_bonus_by_player.get(&PlayerId::new("p1")),
        Some(&1)
    );
}

#[test]
fn immortal_barrier_grants_one_draw_through_the_completed_active_spell_boundary() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "immortal");
    let barrier = cards(
        &state,
        &[
            (Element::Wood, 1),
            (Element::Wood, 2),
            (Element::Metal, 1),
            (Element::Fire, 1),
        ],
    );
    set_hand(&mut state, "p1", barrier.clone());

    let events = perform(&state, "barrier", barrier, Vec::new()).unwrap();
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(
                event,
                GameEvent::TurnDrawBonusChanged {
                    player,
                    old_value: 0,
                    delta: 1,
                    new_value: 1,
                } if player == &PlayerId::new("p1")
            ))
            .count(),
        1
    );
    apply_all(&mut state, &events);
    assert_eq!(
        state.turn_draw_bonus_by_player.get(&PlayerId::new("p1")),
        Some(&1)
    );
}

#[test]
fn immortal_radiance_and_return_to_origin_grant_one_draw_as_completed_active_spells() {
    for (formation_id, formation_cards) in [
        (
            "radiance",
            vec![
                (Element::Metal, 1),
                (Element::Metal, 2),
                (Element::Fire, 1),
                (Element::Water, 1),
            ],
        ),
        (
            "return-to-origin",
            vec![
                (Element::Water, 1),
                (Element::Water, 2),
                (Element::Earth, 1),
                (Element::Wood, 1),
            ],
        ),
    ] {
        let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
        set_profession(&mut state, "p1", "immortal");
        let selected = cards(&state, &formation_cards);
        set_hand(&mut state, "p1", selected.clone());

        let events = perform(&state, formation_id, selected, Vec::new()).unwrap();
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    GameEvent::TurnDrawBonusChanged {
                        player,
                        old_value: 0,
                        delta: 1,
                        new_value: 1,
                    } if player == &PlayerId::new("p1")
                ))
                .count(),
            1,
            "{formation_id} must emit exactly one canonical draw bonus"
        );
        apply_all(&mut state, &events);
        assert_eq!(
            state.turn_draw_bonus_by_player.get(&PlayerId::new("p1")),
            Some(&1),
            "{formation_id} must project its draw bonus"
        );
    }
}

#[test]
fn immortal_shock_burst_keeps_one_draw_in_the_atomic_attack_resolution() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "immortal");
    let shock_burst = cards(
        &state,
        &[
            (Element::Fire, 1),
            (Element::Fire, 2),
            (Element::Water, 1),
            (Element::Earth, 1),
        ],
    );
    set_hand(&mut state, "p1", shock_burst.clone());

    let events = perform(&state, "shock-burst", shock_burst, Vec::new()).unwrap();
    assert_eq!(
        events
            .iter()
            .filter_map(|event| match event {
                GameEvent::AttackResolved {
                    elemental_context_update: Some(effects),
                    ..
                } => Some(&effects.turn_draw_bonus_changes),
                _ => None,
            })
            .flatten()
            .filter(|change| {
                change.player == PlayerId::new("p1")
                    && change.old_value == 0
                    && change.delta == 1
                    && change.new_value == 1
            })
            .count(),
        1
    );
    apply_all(&mut state, &events);
    assert_eq!(
        state.turn_draw_bonus_by_player.get(&PlayerId::new("p1")),
        Some(&1)
    );
}

#[test]
fn all_school_transitions_and_unaffiliated_routes_are_queryable() {
    let schools = [
        (Element::Metal, "warrior", "war-god", "hero"),
        (Element::Wood, "seeker", "expounder", "benevolent"),
        (Element::Water, "mesmer", "spirit-mesmer", "hermit"),
        (Element::Fire, "mage", "mage-guide", "sage"),
        (
            Element::Earth,
            "windwalker",
            "shadow-walker",
            "martial-artist",
        ),
    ];
    for (element, first, second, third) in schools {
        let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
        let selected = cards(&state, &[(element, 3)]);
        set_hand(&mut state, "p1", selected.clone());
        assert!(
            OfficialRules::new()
                .playable_actions(&state, &PlayerId::new("p1"), &selected)
                .unwrap()
                .iter()
                .any(|action| matches!(
                    action,
                    PlayableAction::ChangeProfession(candidate)
                        if candidate.profession_id == ProfessionId::new(first)
                ))
        );

        set_profession(&mut state, "p1", first);
        let selected = cards(&state, &[(element, 1), (element, 5)]);
        set_hand(&mut state, "p1", selected.clone());
        assert!(
            OfficialRules::new()
                .playable_actions(&state, &PlayerId::new("p1"), &selected)
                .unwrap()
                .iter()
                .any(|action| matches!(
                    action,
                    PlayableAction::ChangeProfession(candidate)
                        if candidate.profession_id == ProfessionId::new(second)
                ))
        );

        set_profession(&mut state, "p1", second);
        let selected = cards(&state, &[(element, 4), (element, 5)]);
        set_hand(&mut state, "p1", selected.clone());
        assert!(
            OfficialRules::new()
                .playable_actions(&state, &PlayerId::new("p1"), &selected)
                .unwrap()
                .iter()
                .any(|action| matches!(
                    action,
                    PlayableAction::ChangeProfession(candidate)
                        if candidate.profession_id == ProfessionId::new(third)
                ))
        );
    }

    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    let one = cards(&state, &[(Element::Water, 1)]);
    set_hand(&mut state, "p1", one.clone());
    assert!(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &one)
            .unwrap()
            .iter()
            .any(|action| matches!(
                action,
                PlayableAction::ChangeProfession(candidate)
                    if candidate.profession_id == ProfessionId::new("first-wanderer")
            ))
    );
    set_profession(&mut state, "p1", "first-wanderer");
    assert!(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &one)
            .unwrap()
            .iter()
            .any(|action| matches!(
                action,
                PlayableAction::ChangeProfession(candidate)
                    if candidate.profession_id == ProfessionId::new("seeker")
            ))
    );

    let breakthrough = cards(
        &state,
        &[(Element::Water, 1), (Element::Fire, 1), (Element::Fire, 5)],
    );
    set_hand(&mut state, "p1", breakthrough.clone());
    assert!(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &breakthrough)
            .unwrap()
            .iter()
            .any(|action| matches!(
                action,
                PlayableAction::ChangeProfession(candidate)
                    if candidate.profession_id == ProfessionId::new("mage-guide")
            ))
    );

    set_profession(&mut state, "p1", "mage-guide");
    let fives = cards(
        &state,
        &[(Element::Metal, 5), (Element::Wood, 5), (Element::Fire, 5)],
    );
    set_hand(&mut state, "p1", fives.clone());
    assert!(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &fives)
            .unwrap()
            .iter()
            .any(|action| matches!(
                action,
                PlayableAction::ChangeProfession(candidate)
                    if candidate.profession_id == ProfessionId::new("immortal")
            ))
    );
}

#[test]
fn temporary_ability_loss_removes_first_wanderer_choice_and_breakthrough_paths() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "first-wanderer");
    let choice = cards(&state, &[(Element::Water, 1)]);
    set_hand(&mut state, "p1", choice.clone());
    give_temporary_profession_ability_loss(&mut state, "p1");

    let actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &choice)
        .unwrap();
    assert!(!actions.iter().any(|action| {
        matches!(
            action,
            PlayableAction::ChangeProfession(candidate)
                if candidate.profession_id == ProfessionId::new("seeker")
        )
    }));
    let state_before_choice = state.clone();
    assert!(matches!(
        handle_command(
            &state,
            Command::ChangeProfession {
                player: PlayerId::new("p1"),
                profession: ProfessionId::new("seeker"),
                cards: choice,
            },
        ),
        Err(GameError::Validation(
            ValidationError::ProfessionChangePatternMismatch { profession }
        )) if profession == ProfessionId::new("seeker")
    ));
    assert_eq!(state, state_before_choice);

    let breakthrough = cards(
        &state,
        &[(Element::Water, 1), (Element::Fire, 1), (Element::Fire, 5)],
    );
    set_hand(&mut state, "p1", breakthrough.clone());
    let actions = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &breakthrough)
        .unwrap();
    assert!(!actions.iter().any(|action| {
        matches!(
            action,
            PlayableAction::ChangeProfession(candidate)
                if candidate.profession_id == ProfessionId::new("mage-guide")
        )
    }));
    let state_before_breakthrough = state.clone();
    assert!(matches!(
        handle_command(
            &state,
            Command::ChangeProfession {
                player: PlayerId::new("p1"),
                profession: ProfessionId::new("mage-guide"),
                cards: breakthrough,
            },
        ),
        Err(GameError::Validation(
            ValidationError::ProfessionChangePatternMismatch { profession }
        )) if profession == ProfessionId::new("mage-guide")
    ));
    assert_eq!(state, state_before_breakthrough);
}

#[test]
fn seeker_options_and_reincarnation_role_binding_are_exact() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "expounder");
    let generating = cards(&state, &[(Element::Water, 2), (Element::Wood, 3)]);
    set_hand(&mut state, "p1", generating.clone());
    assert_eq!(
        formation_candidates(&state, &generating, "generating-formation").len(),
        1
    );

    set_profession(&mut state, "p1", "benevolent");
    let unambiguous = cards(
        &state,
        &[
            (Element::Wood, 2),
            (Element::Wood, 3),
            (Element::Fire, 3),
            (Element::Earth, 4),
        ],
    );
    set_hand(&mut state, "p1", unambiguous.clone());
    let candidates = formation_candidates(&state, &unambiguous, "reincarnation");
    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].preview.as_deref(), Some("回復 50 點生命"));
    assert!(perform(&state, "reincarnation", unambiguous, Vec::new()).is_ok());

    let ambiguous = cards(
        &state,
        &[
            (Element::Wood, 3),
            (Element::Wood, 4),
            (Element::Fire, 3),
            (Element::Earth, 4),
        ],
    );
    set_hand(&mut state, "p1", ambiguous.clone());
    let candidates = formation_candidates(&state, &ambiguous, "reincarnation");
    assert_eq!(candidates.len(), 2);
    assert!(matches!(
        perform(&state, "reincarnation", ambiguous.clone(), Vec::new()),
        Err(GameError::Validation(
            ValidationError::FormationMatchOptionRequired { .. }
        ))
    ));
    let events = perform(
        &state,
        "reincarnation",
        ambiguous,
        candidates[1].declared_targets.clone(),
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.delta() == 100
    )));
}

#[test]
fn seeker_cost_counter_resistance_and_spell_protection_are_typed() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID, "discard-retrieval"]);
    set_profession(&mut state, "p1", "seeker");
    let retrieved = cards(&state, &[(Element::Fire, 5)])[0];
    state.discard.push(retrieved);
    state.last_turn_discard_by_player.insert(
        PlayerId::new("p2"),
        fewfc::domain::LastTurnDiscard {
            card: retrieved,
            turn_number: 0,
        },
    );
    let events = handle_command(
        &state,
        Command::RetrievePreviousTurnDiscard {
            player: PlayerId::new("p1"),
        },
    )
    .unwrap();
    assert!(matches!(
        events.as_slice(),
        [GameEvent::DiscardRetrieved { hp_change, .. }] if hp_change.delta() == -5
    ));

    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "expounder");
    let dao = cards(&state, &[(Element::Wood, 3), (Element::Fire, 3)]);
    set_hand(&mut state, "p1", dao.clone());
    let events = perform(&state, "dao-defense", dao, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(effects),
            ..
        } if effects.counter_effects_established.iter().any(|effect| effect.effect_id == "dao-defense")
    )));

    set_profession(&mut state, "p1", "benevolent");
    set_profession(&mut state, "p2", "benevolent");
    state.counter_effects.push(fewfc::domain::CounterEffect {
        owner: PlayerId::new("p2"),
        effect_id: "magic-seal".to_string(),
        established_on_turn: 0,
    });
    let spell = cards(
        &state,
        &[(Element::Wood, 1), (Element::Fire, 2), (Element::Earth, 3)],
    );
    set_hand(&mut state, "p1", spell.clone());
    let events = perform(&state, "generating-formation", spell, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::CounterEffectResolved {
            outcome: fewfc::domain::PassiveFlipOutcome::NoEffect {
                grounds,
            },
            ..
        } if grounds == &vec![fewfc::domain::PassiveNoEffectGround::IgnoredByProfessionAbility]
    )));

    let wood = cards(&state, &[(Element::Wood, 5)]);
    set_hand(&mut state, "p2", wood.clone());
    state.current_turn_index = 1;
    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "wood-strike".to_string(),
            cards: wood,
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_changes, .. }
            if hp_changes.iter().any(|resolved| resolved.change.delta() == 0)
    )));
}

#[test]
fn illusion_and_phantasm_are_single_offers_with_virtual_card_input() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "hermit");
    let selected = cards(&state, &[(Element::Metal, 1), (Element::Wood, 1)]);
    set_hand(&mut state, "p1", selected.clone());

    let offers = OfficialRules::new()
        .playable_actions(&state, &PlayerId::new("p1"), &selected)
        .unwrap()
        .into_iter()
        .filter_map(|action| match action {
            PlayableAction::ActivateProfessionAbility(candidate)
                if matches!(candidate.ability_id.as_str(), "illusion" | "phantasm") =>
            {
                Some(candidate)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    assert_eq!(offers.len(), 2);
    for ability_id in ["illusion", "phantasm"] {
        let offer = offers
            .iter()
            .find(|offer| offer.ability_id == ability_id)
            .expect("each Mesmer ability should have one offer");
        assert_eq!(offer.cards, selected);
        assert_eq!(offer.declared_element, None);
        assert_eq!(offer.declared_level, None);
        assert_eq!(
            offer.input_requirement,
            Some(ActionInputRequirement::VirtualFormationCard {
                elements: vec![
                    Element::Metal,
                    Element::Wood,
                    Element::Water,
                    Element::Fire,
                    Element::Earth,
                ],
                levels: vec![1, 2, 3, 4, 5],
            })
        );
    }
}

#[test]
fn mesmer_preparation_is_public_shared_and_clears_after_action() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "hermit");
    let selected = cards(
        &state,
        &[(Element::Metal, 1), (Element::Wood, 1), (Element::Earth, 2)],
    );
    set_hand(&mut state, "p1", selected.clone());
    let events = handle_command(
        &state,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p1"),
            ability_id: "illusion".to_string(),
            cards: selected[..2].to_vec(),
            target_card: None,
            declared_element: Some(Element::Fire),
            declared_level: Some(3),
        },
    )
    .unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::TurnDrawBonusChanged { delta: 1, .. }))
    );
    apply_all(&mut state, &events);
    assert!(
        state_for(&state, Viewer::Observer)
            .card_interpretations
            .is_empty()
    );
    assert!(
        matches!(state.formation_requirements.as_slice(), [requirement]
        if requirement.virtual_card.as_ref().is_some_and(|card| card.element == Element::Fire && card.level == 3))
    );
    assert!(matches!(
        handle_command(
            &state,
            Command::ActivateProfessionAbility {
                player: PlayerId::new("p1"),
                ability_id: "phantasm".to_string(),
                cards: selected[..2].to_vec(),
                target_card: None,
                declared_element: Some(Element::Fire),
                declared_level: Some(3),
            },
        ),
        Err(GameError::Validation(
            ValidationError::ProfessionAbilityAlreadyActivated { .. }
        ))
    ));

    let candidates = formation_candidates(&state, &[], "fire-strike");
    assert!(!candidates.is_empty());
    let events = perform(&state, "fire-strike", vec![], Vec::new()).unwrap();
    apply_all(&mut state, &events);
    assert!(state.prepared_profession_abilities.is_empty());
    assert!(state.formation_requirements.is_empty());
}

#[test]
fn phantasm_does_not_trigger_illusion_refinement_and_mesmer_formations_resolve() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "hermit");
    let selected = cards(
        &state,
        &[(Element::Metal, 1), (Element::Wood, 1), (Element::Earth, 2)],
    );
    set_hand(&mut state, "p1", selected.clone());
    let events = handle_command(
        &state,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p1"),
            ability_id: "phantasm".to_string(),
            cards: selected[..2].to_vec(),
            target_card: None,
            declared_element: Some(Element::Fire),
            declared_level: Some(3),
        },
    )
    .unwrap();
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::TurnDrawBonusChanged { .. }))
    );

    apply_all(&mut state, &events);
    let shield = cards(
        &state,
        &[(Element::Wood, 1), (Element::Wood, 2), (Element::Metal, 1)],
    );
    set_hand(&mut state, "p1", shield.clone());
    let events = perform(&state, "barrier", shield, Vec::new()).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::ShieldChanged { new_value: 28, .. }))
    );
    assert!(events.iter().any(
        |event| matches!(event, GameEvent::FormationRequirementFulfilled { composition, .. }
        if composition.physical_cards.len() == 3 && composition.virtual_card.is_some())
    ));
}

#[test]
fn mage_point_modifiers_and_star_qualification_use_attack_points() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID, STAR_MODULE_ID]);
    set_profession(&mut state, "p1", "mage");
    let selected = cards(
        &state,
        &[(Element::Metal, 3), (Element::Metal, 4), (Element::Wood, 1)],
    );
    set_hand(&mut state, "p1", selected.clone());
    let events = perform(&state, "triple-metal", selected, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { point_breakdown, .. }
            if point_breakdown.base_points == 18
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::StarSummoned { .. }))
    );

    set_profession(&mut state, "p1", "sage");
    let selected = cards(
        &state,
        &[(Element::Metal, 5), (Element::Metal, 5), (Element::Wood, 5)],
    );
    set_hand(&mut state, "p1", selected.clone());
    let events = perform(&state, "triple-metal", selected, Vec::new()).unwrap();
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::StarSummoned { .. }))
    );
}

#[test]
fn mage_and_windwalker_profession_formations_keep_stage_semantics() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "sage");
    let flash = cards(
        &state,
        &[
            (Element::Fire, 3),
            (Element::Fire, 4),
            (Element::Metal, 1),
            (Element::Water, 1),
        ],
    );
    set_hand(&mut state, "p1", flash.clone());
    let events = perform(&state, "magic-reflection-flash", flash, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { point_breakdown, .. }
            if point_breakdown.base_points == 60
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(effects),
            ..
        } if effects.statuses_added.iter().any(|status| status.kind == "CannotDraw")
    )));

    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "martial-artist");
    state
        .shields
        .iter_mut()
        .find(|shield| shield.player == PlayerId::new("p2"))
        .unwrap()
        .value = 50;
    let assault = cards(&state, &[(Element::Earth, 4), (Element::Metal, 4)]);
    set_hand(&mut state, "p1", assault.clone());
    let events = perform(&state, "shadow-assault", assault, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.delta() == -24
    )));
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::ShieldChanged { .. }))
    );

    let death = cards(&state, &[(Element::Earth, 5), (Element::Earth, 5)]);
    set_hand(&mut state, "p1", death.clone());
    let events = perform(&state, "instant-shadow-death", death, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.new_hp() == 100 && change.delta() == -100
    )));
}

#[test]
fn instant_shadow_death_rounds_down_odd_hp_in_its_command_event() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "martial-artist");
    state
        .hp
        .iter_mut()
        .find(|owned| owned.team == TeamId::new("team:p2"))
        .unwrap()
        .hp = 101;
    let used = cards(&state, &[(Element::Earth, 5), (Element::Earth, 5)]);
    set_hand(&mut state, "p1", used.clone());

    let events = perform(&state, "instant-shadow-death", used, Vec::new()).unwrap();

    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.team() == &TeamId::new("team:p2")
                && change.old_hp() == 101
                && change.delta() == -51
                && change.effective_delta() == -51
                && change.new_hp() == 50
    )));
}

#[test]
fn temporary_ability_loss_removes_profession_formation_options_and_stale_commands() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "sage");
    let cards = cards(
        &state,
        &[
            (Element::Fire, 3),
            (Element::Fire, 4),
            (Element::Metal, 1),
            (Element::Water, 1),
        ],
    );
    set_hand(&mut state, "p1", cards.clone());
    give_temporary_profession_ability_loss(&mut state, "p1");

    assert!(formation_candidates(&state, &cards, "magic-reflection-flash").is_empty());
    let state_before_stale_command = state.clone();
    assert!(matches!(
        perform(&state, "magic-reflection-flash", cards, Vec::new()),
        Err(GameError::Validation(
            ValidationError::FormationPatternMismatch { formation_id }
        )) if formation_id == "magic-reflection-flash"
    ));
    assert_eq!(state, state_before_stale_command);
}

#[test]
fn sacred_beast_resistance_applies_only_after_shield_absorption() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID, FIVE_DIRECTIONS_LEGEND_MODULE_ID]);
    set_profession(&mut state, "p2", "hero");
    let beast = cards(
        &state,
        &[
            (Element::Metal, 1),
            (Element::Metal, 2),
            (Element::Metal, 3),
            (Element::Metal, 4),
            (Element::Metal, 5),
        ],
    );
    set_hand(&mut state, "p1", beast.clone());
    let events = perform(&state, "west-white-tiger", beast.clone(), Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            hp_changes,
            elemental_context_update: Some(effects),
            ..
        } if hp_changes.iter().any(|resolved| resolved.change.delta() == 0)
            && !effects.environment_transfers.is_empty()
    )));

    state
        .shields
        .iter_mut()
        .find(|shield| shield.player == PlayerId::new("p2"))
        .unwrap()
        .value = 100;
    set_hand(&mut state, "p1", beast.clone());
    let events = perform(&state, "west-white-tiger", beast, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            shield_change: Some(change),
            point_breakdown,
            ..
        } if change.delta == -81 && point_breakdown.final_amount == 81
    )));
}

#[test]
fn windwalker_and_unaffiliated_effects_use_shared_pipelines() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "windwalker");
    state
        .formation_area_mut(&PlayerId::new("p2"))
        .unwrap()
        .formation = Some(fewfc::domain::FormationInArea {
        formation_id: "defense".to_string(),
        cards: cards(&state, &[(Element::Wood, 1), (Element::Wood, 2)]),
        star_substitution: None,
        state: fewfc::domain::FormationAreaState::FaceDownWaiting {
            ineffective_environment: None,
            sealed: false,
            revealed: false,
            neutralized: false,
            trigger_timing: PassiveTriggerTiming::NextPlayerActionStart,
        },
    });
    let strike = cards(&state, &[(Element::Metal, 1)]);
    set_hand(&mut state, "p1", strike.clone());
    let events = perform(&state, "metal-strike", strike, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_changes, .. }
            if hp_changes.iter().any(|resolved| resolved.change.delta() < 0)
    )));

    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "shadow-walker");
    let cut = cards(&state, &[(Element::Fire, 5)]);
    set_hand(&mut state, "p1", cut.clone());
    let events = handle_command(
        &state,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p1"),
            ability_id: "shadow-cut".to_string(),
            cards: cut,
            target_card: None,
            declared_element: None,
            declared_level: None,
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.delta() == -10
    )));

    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "saint");
    let sacred = cards(
        &state,
        &[(Element::Fire, 4), (Element::Water, 2), (Element::Earth, 3)],
    );
    set_hand(&mut state, "p1", sacred.clone());
    let candidates = formation_candidates(&state, &sacred, "shock-burst");
    let sacred_option = candidates
        .iter()
        .find(|candidate| {
            candidate
                .declared_targets
                .iter()
                .any(|target| matches!(target, TargetDecl::CardMultiplicity { .. }))
        })
        .unwrap();
    let events = perform(
        &state,
        "shock-burst",
        sacred,
        sacred_option.declared_targets.clone(),
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { point_breakdown, used_cards, .. }
            if point_breakdown.base_points == 36 && used_cards.len() == 3
    )));
}

#[test]
fn void_reversion_is_atomic_and_protects_low_level_legendary_professions() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "warrior");
    set_profession(&mut state, "p2", "hero");
    let selected = cards(
        &state,
        &[(Element::Metal, 2), (Element::Wood, 2), (Element::Fire, 2)],
    );
    set_hand(&mut state, "p1", selected.clone());
    let events = perform(&state, "void-reversion", selected, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::VoidReversionResolved {
            hp_change,
            broken_professions,
            retained_legendary_professions,
            ..
        } if hp_change.delta() == -20
            && broken_professions.len() == 1
            && retained_legendary_professions.len() == 1
    )));
    apply_all(&mut state, &events);
    assert_eq!(
        state.profession_for(&PlayerId::new("p2")),
        Some(&ProfessionId::new("hero"))
    );
    assert_eq!(state.profession_for(&PlayerId::new("p1")), None);
}

#[test]
fn activated_abilities_require_action_permission_and_revelation_recycles_personal_cards() {
    let mut blocked = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut blocked, "p1", "shadow-walker");
    blocked.statuses.push(fewfc::domain::StatusEffect {
        id: "blocked".to_string(),
        owner: fewfc::domain::StatusOwner::Player(PlayerId::new("p1")),
        kind: "CannotAct".to_string(),
        value: None,
        duration: fewfc::domain::StatusDuration::Permanent,
    });
    let card = cards(&blocked, &[(Element::Earth, 4)]);
    set_hand(&mut blocked, "p1", card.clone());
    assert!(matches!(
        handle_command(
            &blocked,
            Command::ActivateProfessionAbility {
                player: PlayerId::new("p1"),
                ability_id: "shadow-cut".to_string(),
                cards: card,
                target_card: None,
                declared_element: None,
                declared_level: None,
            }
        ),
        Err(GameError::Validation(
            ValidationError::ProfessionAbilityCannotResolve(_)
        ))
    ));

    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID]);
    set_profession(&mut state, "p1", "saint");
    let p1_cards = state
        .card_instances
        .iter()
        .filter(|instance| {
            matches!(
                &instance.origin,
                CardOrigin::Player(player) if player == &PlayerId::new("p1")
            )
        })
        .map(|instance| instance.instance)
        .collect::<Vec<_>>();
    let cost = p1_cards
        .iter()
        .copied()
        .find(|card| {
            state
                .card_def(*card)
                .is_some_and(|definition| definition.level.value() >= 4)
        })
        .unwrap();
    let draw = p1_cards
        .iter()
        .copied()
        .filter(|card| *card != cost)
        .take(3)
        .collect::<Vec<_>>();
    set_hand(&mut state, "p1", vec![cost]);
    state
        .player_decks
        .iter_mut()
        .find(|pile| pile.player == PlayerId::new("p1"))
        .unwrap()
        .cards = vec![draw[0]];
    state
        .player_discards
        .iter_mut()
        .find(|pile| pile.player == PlayerId::new("p1"))
        .unwrap()
        .cards = draw[1..].to_vec();

    let events = handle_command(
        &state,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p1"),
            ability_id: "revelation".to_string(),
            cards: vec![cost],
            target_card: None,
            declared_element: None,
            declared_level: None,
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::RandomnessRequested { request, .. }
            if request.operation.is_discard_shuffle()
                && request.current_order.len() == 3
                && request.current_order.contains(&cost)
    )));
    apply_all(&mut state, &events);
    assert!(state.pending_randomness.is_some());
    let request = state.pending_randomness.clone().unwrap();
    let resolved = resolve_trusted_randomness(
        &state,
        &TrustedRandomnessAnswer {
            request_id: request.request_id,
            shuffled_order: request.current_order.clone(),
        },
    )
    .unwrap();
    apply_all(&mut state, &resolved);
    assert!(state.pending_choice.is_some());
}

#[test]
fn high_level_void_reversion_breaks_legendary_professions_before_outcome() {
    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "saint");
    set_profession(&mut state, "p2", "hero");
    state
        .hp
        .iter_mut()
        .find(|hp| hp.team == TeamId::new("team:p1"))
        .unwrap()
        .hp = 20;
    let selected = cards(
        &state,
        &[(Element::Metal, 3), (Element::Wood, 3), (Element::Fire, 3)],
    );
    set_hand(&mut state, "p1", selected.clone());

    let events = perform(&state, "void-reversion", selected, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::VoidReversionResolved {
            hp_change,
            broken_professions,
            retained_legendary_professions,
            ..
        } if hp_change.new_hp() == 0
            && broken_professions.len() == 2
            && retained_legendary_professions.is_empty()
    )));
    assert!(matches!(events.last(), Some(GameEvent::GameEnded { .. })));
    apply_all(&mut state, &events);
    assert!(state.professions.is_empty());
    assert!(matches!(
        state.status,
        fewfc::domain::GameStatus::Finished { .. }
    ));
}
