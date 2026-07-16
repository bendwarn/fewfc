use fewfc::application::{apply_event, handle_command, resolve_trusted_randomness};
use fewfc::domain::{
    CardInstanceId, CardOrigin, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError,
    GameEvent, GameState, HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID, PassiveTriggerTiming,
    Phase, Player, PlayerId, PlayerProfession, ProfessionId, RuleModuleId, STAR_MODULE_ID,
    TargetDecl, TeamId, TrustedRandomnessAnswer, ValidationError,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};

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
        GameEvent::HpChanged { change } if change.delta == 100
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
        [GameEvent::DiscardRetrieved { hp_change, .. }] if hp_change.delta == -5
    ));

    let mut state = game_state(&[HERO_SCHOOLS_MODULE_ID]);
    set_profession(&mut state, "p1", "expounder");
    let dao = cards(&state, &[(Element::Wood, 3), (Element::Fire, 3)]);
    set_hand(&mut state, "p1", dao.clone());
    let events = perform(&state, "dao-defense", dao, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::CounterEffectEstablished { effect_id, .. } if effect_id == "dao-defense"
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
                reason: fewfc::domain::PassiveNoEffectReason::IgnoredByProfessionAbility,
            },
            ..
        }
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
        GameEvent::AttackResolved { hp_change, .. } if hp_change.delta == 0
    )));
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
        GameEvent::StatusAdded { status } if status.kind == "CannotDraw"
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
        GameEvent::HpChanged { change } if change.delta == -24
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
        GameEvent::HpChanged { change } if change.new_hp == 100 && change.delta == -100
    )));
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
    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::AttackResolved { hp_change, .. },
            GameEvent::EnvironmentTransferred { .. }
        ] if hp_change.delta == 0
    ));

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
    state.covered_passives.push(fewfc::domain::CoveredPassive {
        owner: PlayerId::new("p2"),
        formation_id: "defense".to_string(),
        cards: cards(&state, &[(Element::Wood, 1), (Element::Wood, 2)]),
        star_substitution: None,
        sealed: false,
        covered_on_turn: 0,
        reveal_timing: PassiveTriggerTiming::NextPlayerActionStart,
    });
    let strike = cards(&state, &[(Element::Metal, 1)]);
    set_hand(&mut state, "p1", strike.clone());
    let events = perform(&state, "metal-strike", strike, Vec::new()).unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. } if hp_change.delta < 0
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
        GameEvent::HpChanged { change } if change.delta == -10
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
    assert!(matches!(
        events.as_slice(),
        [GameEvent::VoidReversionResolved {
            hp_change,
            broken_professions,
            retained_legendary_professions,
            ..
        }] if hp_change.delta == -20
            && broken_professions.len() == 1
            && retained_legendary_professions.len() == 1
    ));
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
                .is_some_and(|definition| definition.level >= 4)
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
        GameEvent::RandomnessRequested { request }
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
    assert!(matches!(
        events.as_slice(),
        [GameEvent::VoidReversionResolved {
            hp_change,
            broken_professions,
            retained_legendary_professions,
            ..
        }] if hp_change.new_hp == 0
            && broken_professions.len() == 2
            && retained_legendary_professions.is_empty()
    ));
    apply_all(&mut state, &events);
    assert!(state.professions.is_empty());
    assert!(matches!(
        state.status,
        fewfc::domain::GameStatus::Finished { .. }
    ));
}
