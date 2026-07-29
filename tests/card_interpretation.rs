use fewfc::application::{apply_event, replay};
use fewfc::domain::{
    CardDef, CardDefId, CardInstanceDef, CardInstanceId, CardInterpretationLayer,
    CardInterpretationSource, CardLevelInterpretation, EffectiveCardLevel, Element, GameEvent,
    GameSetup, GameState, HERO_SCHOOLS_MODULE_ID, Phase, PlayerId, PlayerProfession,
    PouchLevelBonus, PrintedCardLevel, ProfessionId, RuleModuleId,
};
use fewfc::rules::{OfficialRules, PlayableAction};
use proptest::prelude::*;

fn setup_with_levels(levels: &[u32]) -> GameSetup {
    let definitions = levels
        .iter()
        .enumerate()
        .map(|(index, level)| CardDef {
            id: CardDefId::new(format!("card-{index}")),
            name: format!("card-{index}"),
            element: Element::Wood,
            level: PrintedCardLevel::new(*level),
        })
        .collect::<Vec<_>>();
    let instances = levels
        .iter()
        .enumerate()
        .map(|(index, _)| CardInstanceDef {
            instance: CardInstanceId::new(index as u64 + 1),
            definition: CardDefId::new(format!("card-{index}")),
            origin: Default::default(),
        })
        .collect();
    GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30)
        .with_cards(definitions, instances)
}

fn resolver_state(printed: u32, operations: &[CardLevelInterpretation]) -> GameState {
    let mut state = GameState::from_setup(&setup_with_levels(&[printed]));
    state.card_interpretation_layers = operations
        .iter()
        .enumerate()
        .map(|(_, level)| CardInterpretationLayer {
            source: CardInterpretationSource::PouchLevelBonusGranted,
            player: PlayerId::new("p1"),
            card: CardInstanceId::new(1),
            applied_on_turn: state.turn_number,
            element: None,
            level: Some(*level),
        })
        .collect();
    state
}

proptest! {
    #[test]
    fn effective_level_is_bounded_and_clamped_after_full_composition(
        printed in 1_u32..=5,
        operations in prop::collection::vec(
            prop_oneof![
                (1_u32..=5).prop_map(|level| CardLevelInterpretation::Set(EffectiveCardLevel::new(level))),
                (-12_i32..=12).prop_map(CardLevelInterpretation::Adjust),
            ],
            0..24,
        ),
    ) {
        let state = resolver_state(printed, &operations);
        let resolved = state
            .effective_card_facts(&PlayerId::new("p1"), CardInstanceId::new(1))
            .expect("known physical Card resolves");
        let composed = operations.iter().fold(printed as i32, |current, operation| match operation {
            CardLevelInterpretation::Set(level) => level.value() as i32,
            CardLevelInterpretation::Adjust(delta) => current + delta,
        });

        prop_assert!((1..=5).contains(&resolved.level.value()));
        prop_assert_eq!(resolved.level.value(), composed.clamp(1, 5) as u32);
    }

    #[test]
    fn out_of_range_set_is_rejected_from_canonical_event_json(level in prop_oneof![Just(0_u32), 6_u32..=u32::MAX]) {
        let raw = serde_json::json!({
            "SpiritLevelInterpreted": {
                "player": "p1",
                "skill": null,
                "card": 1,
                "level": level,
                "applied_on_turn": 1,
                "interpretation_revision": 1,
            }
        });
        prop_assert!(serde_json::from_value::<GameEvent>(raw).is_err());
    }
}

#[test]
fn clamp_is_not_applied_between_relative_layers() {
    let state = resolver_state(
        5,
        &[
            CardLevelInterpretation::Adjust(4),
            CardLevelInterpretation::Adjust(-4),
        ],
    );

    assert_eq!(
        state
            .effective_card_facts(&PlayerId::new("p1"), CardInstanceId::new(1))
            .unwrap()
            .level,
        EffectiveCardLevel::new(5),
    );
}

#[test]
fn semantic_events_project_to_the_unified_ordered_layers() {
    let setup = setup_with_levels(&[2, 3]);
    let player = PlayerId::new("p1");
    let events = vec![
        GameEvent::PouchLevelBonusGranted {
            bonus: PouchLevelBonus {
                player: player.clone(),
                cards: vec![CardInstanceId::new(1)],
                applied_on_turn: 1,
            },
        },
        GameEvent::SpiritLevelInterpreted {
            player: player.clone(),
            skill: None,
            card: CardInstanceId::new(1),
            level: EffectiveCardLevel::new(4),
            applied_on_turn: 1,
            interpretation_revision: 1,
        },
        GameEvent::ProfessionAbilityActivated {
            player: player.clone(),
            ability_id: "test:prepared".to_string(),
            prepared: Some(fewfc::domain::PreparedProfessionAbility {
                player: player.clone(),
                ability_id: "test:prepared".to_string(),
                card: CardInstanceId::new(2),
                element: Element::Fire,
                level: EffectiveCardLevel::new(5),
                allowed_formation_scope: vec!["base".to_string()],
                prepared_on_turn: 1,
                interpretation_revision: 2,
            }),
        },
    ];

    let state = replay(&setup, &events).expect("valid semantic events replay");
    assert_eq!(
        state.card_interpretation_layers,
        vec![
            CardInterpretationLayer {
                source: CardInterpretationSource::PouchLevelBonusGranted,
                player: player.clone(),
                card: CardInstanceId::new(1),
                applied_on_turn: 1,
                element: None,
                level: Some(CardLevelInterpretation::Adjust(1)),
            },
            CardInterpretationLayer {
                source: CardInterpretationSource::SpiritLevelInterpreted,
                player: player.clone(),
                card: CardInstanceId::new(1),
                applied_on_turn: 1,
                element: None,
                level: Some(CardLevelInterpretation::Set(EffectiveCardLevel::new(4))),
            },
            CardInterpretationLayer {
                source: CardInterpretationSource::ProfessionAbilityActivated,
                player,
                card: CardInstanceId::new(2),
                applied_on_turn: 1,
                element: Some(Element::Fire),
                level: Some(CardLevelInterpretation::Set(EffectiveCardLevel::new(5))),
            },
        ],
    );
}

#[test]
fn steal_the_beam_turns_printed_five_five_four_into_immortal_levels() {
    let rules = OfficialRules::new();
    let setup = rules
        .configure_game(
            vec![
                fewfc::domain::Player {
                    id: PlayerId::new("p1"),
                    team: fewfc::domain::TeamId::new("team:p1"),
                },
                fewfc::domain::Player {
                    id: PlayerId::new("p2"),
                    team: fewfc::domain::TeamId::new("team:p2"),
                },
            ],
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            vec![RuleModuleId::new(HERO_SCHOOLS_MODULE_ID)],
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state.professions.push(PlayerProfession {
        player: PlayerId::new("p1"),
        profession: ProfessionId::new("mage-guide"),
    });
    let mut used = Vec::new();
    let cards = [5, 5, 4]
        .into_iter()
        .map(|level| {
            let card = state
                .card_instances
                .iter()
                .map(|instance| instance.instance)
                .find(|card| {
                    !used.contains(card)
                        && state.card_def(*card).is_some_and(|definition| {
                            definition.element == Element::Wood && definition.level.value() == level
                        })
                })
                .unwrap();
            used.push(card);
            card
        })
        .collect::<Vec<_>>();
    assert_eq!(
        cards
            .iter()
            .filter_map(|card| state
                .card_def(*card)
                .map(|definition| definition.level.value()))
            .collect::<Vec<_>>(),
        vec![5, 5, 4],
    );
    *state.hand_mut(&PlayerId::new("p1")).unwrap() = cards.clone();
    let current_turn = state.turn_number;
    apply_event(
        &mut state,
        &GameEvent::PouchLevelBonusGranted {
            bonus: PouchLevelBonus {
                player: PlayerId::new("p1"),
                cards: cards.clone(),
                applied_on_turn: current_turn,
            },
        },
    );

    assert_eq!(
        cards
            .iter()
            .map(|card| {
                state
                    .effective_card_facts(&PlayerId::new("p1"), *card)
                    .unwrap()
                    .level
            })
            .collect::<Vec<_>>(),
        vec![
            EffectiveCardLevel::new(5),
            EffectiveCardLevel::new(5),
            EffectiveCardLevel::new(5),
        ],
    );
    assert!(
        rules
            .playable_actions(&state, &PlayerId::new("p1"), &cards)
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
fn printed_and_effective_levels_remain_distinct_but_json_numbers() {
    fn normal_rule_consumer(_: EffectiveCardLevel) {}

    let printed = PrintedCardLevel::new(5);
    let effective = EffectiveCardLevel::new(5);
    normal_rule_consumer(effective);
    assert_eq!(serde_json::to_value(printed).unwrap(), serde_json::json!(5));
    assert_eq!(
        serde_json::to_value(effective).unwrap(),
        serde_json::json!(5)
    );
}
