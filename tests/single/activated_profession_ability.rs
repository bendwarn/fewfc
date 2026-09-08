use fewfc::application::{apply_event, handle_command, resolve_trusted_randomness};
use fewfc::domain::{
    CONFLUENCE_GENERATION_MODULE_ID, CardInstanceId, Command, DARK_GLIMMER_MODULE_ID, Element,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError, GameEvent, GameState, HERO_SCHOOLS_MODULE_ID,
    JIANGHU_MODULE_ID, LimitedUse, PERSONAL_DECK_MODULE_ID, PendingResolution, Phase, Player,
    PlayerId, PlayerProfession, PlayerSpirit, ProfessionId, RuleModuleId, SPIRIT_MODULE_ID,
    STAR_MODULE_ID, SpiritKind, TeamId, TrustedRandomnessAnswer, ValidationError,
};
use fewfc::rules::{ActionInputRequirement, OfficialRules, PlayableAction};

fn state() -> GameState {
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
            ],
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            [
                STAR_MODULE_ID,
                FIVE_DIRECTIONS_LEGEND_MODULE_ID,
                HERO_SCHOOLS_MODULE_ID,
                SPIRIT_MODULE_ID,
                JIANGHU_MODULE_ID,
                CONFLUENCE_GENERATION_MODULE_ID,
                DARK_GLIMMER_MODULE_ID,
            ]
            .into_iter()
            .map(RuleModuleId::new)
            .collect(),
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    state
}

fn card(
    state: &GameState,
    element: Element,
    level: u32,
    excluded: &[CardInstanceId],
) -> CardInstanceId {
    state
        .card_instances
        .iter()
        .map(|instance| instance.instance)
        .find(|card| {
            !excluded.contains(card)
                && state.card_def(*card).is_some_and(|definition| {
                    definition.element == element && definition.level.value() == level
                })
        })
        .unwrap()
}

fn set_profession(state: &mut GameState, profession: &str) {
    state
        .professions
        .retain(|owned| owned.player != PlayerId::new("p1"));
    state.professions.push(PlayerProfession {
        player: PlayerId::new("p1"),
        profession: ProfessionId::new(profession),
    });
}

fn set_hand(state: &mut GameState, cards: Vec<CardInstanceId>) {
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = cards;
}

fn activate(
    state: &GameState,
    ability_id: &str,
    cards: Vec<CardInstanceId>,
    target_card: Option<CardInstanceId>,
    declared_element: Option<Element>,
    declared_level: Option<u32>,
) -> Result<Vec<GameEvent>, GameError> {
    handle_command(
        state,
        Command::ActivateProfessionAbility {
            player: PlayerId::new("p1"),
            ability_id: ability_id.to_string(),
            cards,
            target_card,
            declared_element,
            declared_level,
        },
    )
}

#[test]
fn offer_completion_is_accepted_and_activation_is_first_and_unique() {
    let mut game = state();
    set_profession(&mut game, "mesmer");
    let first = card(&game, Element::Metal, 1, &[]);
    let second = card(&game, Element::Wood, 1, &[first]);
    set_hand(&mut game, vec![first, second]);

    let candidate = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p1"), &[first, second])
        .unwrap()
        .into_iter()
        .find_map(|action| match action {
            PlayableAction::ActivateProfessionAbility(candidate)
                if candidate.ability_id == "illusion" =>
            {
                Some(candidate)
            }
            _ => None,
        })
        .expect("Illusion is offered");
    assert!(matches!(
        candidate.input_requirement,
        Some(ActionInputRequirement::VirtualFormationCard { .. })
    ));

    let events = activate(
        &game,
        "illusion",
        vec![first, second],
        None,
        Some(Element::Water),
        Some(3),
    )
    .unwrap();
    assert!(matches!(
        events.first(),
        Some(GameEvent::ProfessionAbilityActivated { ability_id, .. }) if ability_id == "illusion"
    ));
    assert_eq!(
        events
            .iter()
            .filter(|event| matches!(event, GameEvent::ProfessionAbilityActivated { .. }))
            .count(),
        1
    );
}

#[test]
fn activated_abilities_share_one_allowance_across_rule_modules() {
    let mut game = state();
    set_profession(&mut game, "shadow-walker");
    let cost = card(&game, Element::Earth, 1, &[]);
    set_hand(&mut game, vec![cost]);
    let events = activate(&game, "shadow-cut", vec![cost], None, None, None).unwrap();
    for event in &events {
        apply_event(&mut game, event);
    }

    set_profession(&mut game, "jianghu:qi-grandmaster");
    assert!(
        OfficialRules::new()
            .playable_actions(&game, &PlayerId::new("p1"), &[])
            .unwrap()
            .into_iter()
            .all(|action| !matches!(action, PlayableAction::ActivateProfessionAbility(_)))
    );
    assert!(matches!(
        activate(
            &game,
            "jianghu:dancing-yang-art",
            Vec::new(),
            None,
            None,
            None,
        ),
        Err(GameError::Validation(
            ValidationError::ProfessionAbilityAlreadyActivated { .. }
        ))
    ));
}

#[test]
fn lethal_shadow_cut_blooms_in_the_same_command_and_applies_the_hp_chain() {
    let mut game = state();
    set_profession(&mut game, "shadow-walker");
    game.spirits.push(PlayerSpirit {
        player: PlayerId::new("p2"),
        spirit: SpiritKind::Wood,
        power: 6,
    });
    game.hp
        .iter_mut()
        .find(|owned| owned.team == TeamId::new("team:b"))
        .unwrap()
        .hp = 10;
    let cost = card(&game, Element::Earth, 5, &[]);
    set_hand(&mut game, vec![cost]);

    let events = activate(&game, "shadow-cut", vec![cost], None, None, None).unwrap();
    let hp_change_index = events
        .iter()
        .position(|event| {
            matches!(
                event,
                GameEvent::HpChanged { change }
                    if change.team() == &TeamId::new("team:b")
                        && change.old_hp() == 10
                        && change.delta() == -10
                        && change.new_hp() == 0
            )
        })
        .expect("影切必須在同一命令內使 team:b 歸零");
    assert!(matches!(
        events.get(hp_change_index + 1),
        Some(GameEvent::AutomaticBloomsResolved { resolutions })
            if matches!(resolutions.as_slice(), [resolution]
                if resolution.team == TeamId::new("team:b")
                    && resolution.hp_change.old_hp() == 0
                    && resolution.hp_change.new_hp() == 40)
    ));

    for event in &events {
        apply_event(&mut game, event);
    }
    assert_eq!(
        game.hp
            .iter()
            .find(|owned| owned.team == TeamId::new("team:b"))
            .unwrap()
            .hp,
        40
    );
    assert!(
        game.spirit_for(&PlayerId::new("p2")).is_none(),
        "綻放耗盡的木精靈必須在套用事件後被破除"
    );
}

#[test]
fn selection_is_validated_once_for_duplicates_and_hand_ownership() {
    let mut game = state();
    set_profession(&mut game, "shadow-walker");
    let in_hand = card(&game, Element::Earth, 1, &[]);
    let outside_hand = card(&game, Element::Fire, 1, &[in_hand]);
    set_hand(&mut game, vec![in_hand]);

    assert!(matches!(
        activate(
            &game,
            "shadow-cut",
            vec![in_hand, in_hand],
            None,
            None,
            None,
        ),
        Err(GameError::Validation(
            ValidationError::DuplicateSubmittedCard(card)
        )) if card == in_hand
    ));
    assert!(matches!(
        activate(
            &game,
            "shadow-cut",
            vec![outside_hand],
            None,
            None,
            None,
        ),
        Err(GameError::Validation(ValidationError::CardNotInHand(card))) if card == outside_hand
    ));
}

#[test]
fn completion_contract_rejects_extra_fields_in_three_rule_modules() {
    let mut hero = state();
    set_profession(&mut hero, "shadow-walker");
    let hero_card = card(&hero, Element::Earth, 1, &[]);
    set_hand(&mut hero, vec![hero_card]);
    assert_cannot_resolve(activate(
        &hero,
        "shadow-cut",
        vec![hero_card],
        None,
        Some(Element::Earth),
        None,
    ));

    let mut jianghu = state();
    set_profession(&mut jianghu, "jianghu:qi-grandmaster");
    assert_cannot_resolve(activate(
        &jianghu,
        "jianghu:dancing-yang-art",
        Vec::new(),
        None,
        None,
        Some(1),
    ));

    let mut confluence = state();
    set_profession(&mut confluence, "confluence:clear-wind-adept");
    let deck_card = card(&confluence, Element::Water, 1, &[]);
    confluence.deck = vec![deck_card];
    assert_cannot_resolve(activate(
        &confluence,
        "confluence:clear-wind",
        Vec::new(),
        None,
        Some(Element::Water),
        None,
    ));
}

#[test]
fn azure_short_deck_shuffle_includes_the_just_discarded_cost_then_resumes_to_choice() {
    let mut game = state();
    assert!(!game.has_rule_module(PERSONAL_DECK_MODULE_ID));
    set_profession(&mut game, "jianghu:ink-seeker");
    let cost = card(&game, Element::Wood, 1, &[]);
    let deck_card = card(&game, Element::Metal, 1, &[cost]);
    let old_discard = card(&game, Element::Fire, 1, &[cost, deck_card]);
    set_hand(&mut game, vec![cost]);
    game.deck = vec![deck_card];
    game.discard = vec![old_discard];

    let events = activate(
        &game,
        "jianghu:azure-cloud-step",
        vec![cost],
        None,
        None,
        None,
    )
    .unwrap();
    let (request_id, current_order) = match events.last() {
        Some(GameEvent::RandomnessRequested {
            request,
            resolution: PendingResolution::JianghuAzureCloudStepDraw,
        }) => (request.request_id.clone(), request.current_order.clone()),
        other => panic!("Azure Cloud Step must end at Randomness: {other:?}"),
    };
    assert_eq!(current_order.len(), 2);
    assert!(current_order.contains(&cost));
    assert!(current_order.contains(&old_discard));
    for event in &events {
        apply_event(&mut game, event);
    }

    let resumed = resolve_trusted_randomness(
        &game,
        &TrustedRandomnessAnswer {
            request_id,
            shuffled_order: current_order,
        },
    )
    .unwrap();
    assert!(matches!(
        resumed.last(),
        Some(GameEvent::ChoiceRequested {
            resolution: PendingResolution::JianghuAzureCloudStepReturnOne,
            ..
        })
    ));
    assert!(resumed.iter().any(|event| matches!(
        event,
        GameEvent::CardsDrawnForProfessionChoice { ability_id, cards, .. }
            if ability_id == "jianghu:azure-cloud-step" && cards.len() == 2
    )));
}

#[test]
fn limited_use_presentation_comes_from_the_provider_plan() {
    let mut game = state();
    set_profession(&mut game, "confluence:clear-wind-envoy");
    game.deck = vec![card(&game, Element::Metal, 1, &[])];
    game.limited_uses.push(LimitedUse {
        owner: PlayerId::new("p1"),
        key: "confluence:tailwind".to_string(),
        remaining: 1,
        maximum: 1,
    });

    let candidate = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p1"), &[])
        .unwrap()
        .into_iter()
        .find_map(|action| match action {
            PlayableAction::ActivateProfessionAbility(candidate)
                if candidate.ability_id == "confluence:tailwind" =>
            {
                Some(candidate)
            }
            _ => None,
        })
        .expect("Tailwind is offered");
    assert!(
        candidate
            .detail
            .consequences
            .iter()
            .any(|consequence| matches!(
                consequence,
                fewfc::rules::RuleConsequence::RuleException {
                    exception: fewfc::rules::RuleException::LimitedUse { key, .. },
                    ..
                } if key == "confluence:tailwind"
            ))
    );
}

fn assert_cannot_resolve(result: Result<Vec<GameEvent>, GameError>) {
    assert!(matches!(
        result,
        Err(GameError::Validation(
            ValidationError::ProfessionAbilityCannotResolve(_)
        ))
    ));
}
