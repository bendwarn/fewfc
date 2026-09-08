use fewfc::application::GameRecord;
use fewfc::domain::*;
use fewfc::rules::OfficialRules;

fn player(id: &str) -> PlayerId {
    PlayerId::new(id)
}
fn card(setup: &GameSetup, owner: &PlayerId, element: Element, level: u32) -> CardInstanceId {
    setup
        .card_instances
        .iter()
        .find(|instance| {
            instance.origin == CardOrigin::Player(owner.clone())
                && setup.card_defs.iter().any(|definition| {
                    definition.id == instance.definition
                        && definition.element == element
                        && definition.level.value() == level
                })
        })
        .unwrap()
        .instance
}
fn perform(
    record: &mut GameRecord,
    owner: &PlayerId,
    formation: &str,
    cards: Vec<CardInstanceId>,
) -> Vec<GameEvent> {
    record
        .handle(Command::PerformFormation {
            player: owner.clone(),
            formation_id: formation.into(),
            cards,
            declared_targets: vec![],
        })
        .unwrap()
}
fn finish(record: &mut GameRecord) {
    record.advance_automatic().unwrap();
    let choice = record.state().pending_choice.clone().expect("抽牌棄牌選擇");
    let PendingChoiceKind::Card { cards, .. } = choice.kind else {
        panic!("抽牌選擇")
    };
    record
        .handle(Command::AnswerChoice {
            player: choice.player,
            choice_id: choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![*cards.last().unwrap()],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}
fn retreat(
    record: &mut GameRecord,
    owner: &PlayerId,
    source: CardInstanceId,
    operation: SecretStrategyEnvironmentOperation,
) {
    record
        .handle(Command::TriggerSecretStrategy {
            player: owner.clone(),
            decision: SecretStrategyDecision::Environment {
                source_card: source,
                operation,
            },
        })
        .unwrap();
}
fn scenario(azure: bool) -> (GameRecord, GameSetup) {
    scenario_with_meridian(azure, false)
}

fn scenario_with_meridian(azure: bool, meridian: bool) -> (GameRecord, GameSetup) {
    let base = GameSetup::two_player(player("p1"), player("p2"), 200);
    let setup = OfficialRules::new()
        .configure_versioned_game_with_decks(
            RuleVersion::V5_17,
            base.players,
            base.turn_order,
            [
                STAR_MODULE_ID,
                FIVE_DIRECTIONS_LEGEND_MODULE_ID,
                HERO_SCHOOLS_MODULE_ID,
                SPIRIT_MODULE_ID,
                PERSONAL_DECK_MODULE_ID,
                POUCH_MODULE_ID,
                TOTEM_FORMATION_MODULE_ID,
            ]
            .map(RuleModuleId::new)
            .to_vec(),
            vec![],
        )
        .unwrap();
    let mut record = GameRecord::start(setup.clone(), vec![]).unwrap();
    for owner in [player("p1"), player("p2")] {
        record
            .handle(Command::ChooseInitialPouch {
                player: owner.clone(),
                card: card(&setup, &owner, Element::Fire, 5),
            })
            .unwrap();
    }
    while let Some(request) = record.state().pending_randomness.clone() {
        let owner = if request
            .current_order
            .iter()
            .any(|id| *id == card(&setup, &player("p1"), Element::Wood, 1))
        {
            player("p1")
        } else {
            player("p2")
        };
        let planned = if owner == player("p1") {
            if meridian {
                vec![
                    (Element::Wood, 1),
                    (Element::Wood, 2),
                    (Element::Wood, 5),
                    (Element::Metal, 3),
                    (Element::Wood, 3),
                    (Element::Water, 3),
                    (Element::Earth, 1),
                ]
            } else if azure {
                vec![
                    (Element::Wood, 1),
                    (Element::Wood, 2),
                    (Element::Wood, 5),
                    (Element::Wood, 3),
                    (Element::Wood, 4),
                    (Element::Earth, 1),
                    (Element::Water, 1),
                ]
            } else {
                vec![
                    (Element::Wood, 1),
                    (Element::Wood, 2),
                    (Element::Metal, 1),
                    (Element::Water, 1),
                ]
            }
        } else if azure {
            vec![
                (Element::Metal, 1),
                (Element::Metal, 2),
                (Element::Metal, 5),
                (Element::Earth, 1),
                (Element::Water, 1),
            ]
        } else {
            vec![
                (Element::Metal, 1),
                (Element::Fire, 1),
                (Element::Earth, 1),
                (Element::Water, 1),
                (Element::Water, 2),
            ]
        };
        let mut order: Vec<_> = planned
            .into_iter()
            .map(|(element, level)| card(&setup, &owner, element, level))
            .collect();
        let remaining: Vec<_> = request
            .current_order
            .iter()
            .copied()
            .filter(|id| !order.contains(id))
            .collect();
        order.extend(remaining);
        record
            .resolve_randomness(TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order: order,
            })
            .unwrap();
    }
    record.advance_automatic().unwrap();
    (record, setup)
}
fn assert_defense(record: &mut GameRecord, setup: &GameSetup, effective: bool) {
    let attack = card(setup, &player("p2"), Element::Earth, 1);
    let events = perform(record, &player("p2"), "earth-strike", vec![attack]);
    assert!(events.iter().any(|event| matches!(event, GameEvent::AttackResolved { formation_id, hp_changes, .. } if formation_id == "earth-strike" && hp_changes.is_empty() == effective)), "{events:#?}");
    assert_eq!(record.replay().unwrap(), *record.state());
    assert_eq!(record.verify_replay().unwrap(), *record.state());
}

#[test]
fn defense_effective_when_covered_stays_effective_after_retreat_transfers_metal() {
    let (mut record, setup) = scenario(false);
    perform(
        &mut record,
        &player("p1"),
        "defense",
        [1, 2]
            .map(|level| card(&setup, &player("p1"), Element::Wood, level))
            .to_vec(),
    );
    finish(&mut record);
    retreat(
        &mut record,
        &player("p2"),
        card(&setup, &player("p2"), Element::Fire, 5),
        SecretStrategyEnvironmentOperation::TransferByDiscard {
            card: card(&setup, &player("p2"), Element::Metal, 1),
        },
    );
    assert_eq!(record.state().environment, Some(Element::Metal));
    assert_defense(&mut record, &setup, true);
}

#[test]
fn defense_ineffective_when_covered_in_metal_stays_ineffective_after_retreat_clears_it() {
    let (mut record, setup) = scenario(false);
    retreat(
        &mut record,
        &player("p1"),
        card(&setup, &player("p1"), Element::Fire, 5),
        SecretStrategyEnvironmentOperation::TransferByDiscard {
            card: card(&setup, &player("p1"), Element::Metal, 1),
        },
    );
    perform(
        &mut record,
        &player("p1"),
        "defense",
        [1, 2]
            .map(|level| card(&setup, &player("p1"), Element::Wood, level))
            .to_vec(),
    );
    finish(&mut record);
    retreat(
        &mut record,
        &player("p2"),
        card(&setup, &player("p2"), Element::Fire, 5),
        SecretStrategyEnvironmentOperation::Clear,
    );
    assert_eq!(record.state().environment, None);
    assert_defense(&mut record, &setup, false);
}

#[test]
fn azure_horn_exemption_at_cover_survives_retreat_clearing_metal_before_flip() {
    let (mut record, setup) = scenario(true);
    perform(
        &mut record,
        &player("p1"),
        "east-spirit-array",
        [1, 2, 5]
            .map(|level| card(&setup, &player("p1"), Element::Wood, level))
            .to_vec(),
    );
    assert_eq!(record.state().totems[0].totem, TotemKind::AzureHorn);
    finish(&mut record);
    perform(
        &mut record,
        &player("p2"),
        "west-spirit-array",
        [1, 2, 5]
            .map(|level| card(&setup, &player("p2"), Element::Metal, level))
            .to_vec(),
    );
    finish(&mut record);
    assert_eq!(record.state().environment, Some(Element::Metal));
    perform(
        &mut record,
        &player("p1"),
        "defense",
        [3, 4]
            .map(|level| card(&setup, &player("p1"), Element::Wood, level))
            .to_vec(),
    );
    finish(&mut record);
    retreat(
        &mut record,
        &player("p2"),
        card(&setup, &player("p2"), Element::Fire, 5),
        SecretStrategyEnvironmentOperation::Clear,
    );
    assert_eq!(
        record.state().totems,
        vec![
            PlayerTotem {
                player: player("p1"),
                totem: TotemKind::AzureHorn
            },
            PlayerTotem {
                player: player("p2"),
                totem: TotemKind::WhiteFang
            }
        ]
    );
    assert_defense(&mut record, &setup, true);
}

#[test]
fn void_meridian_removes_all_totems_only_when_it_clears_an_existing_environment() {
    for clear_first_with_retreat in [false, true] {
        let (mut record, setup) = scenario_with_meridian(true, true);
        perform(
            &mut record,
            &player("p1"),
            "east-spirit-array",
            [1, 2, 5]
                .map(|level| card(&setup, &player("p1"), Element::Wood, level))
                .to_vec(),
        );
        finish(&mut record);
        perform(
            &mut record,
            &player("p2"),
            "west-spirit-array",
            [1, 2, 5]
                .map(|level| card(&setup, &player("p2"), Element::Metal, level))
                .to_vec(),
        );
        finish(&mut record);
        let totems = record.state().totems.clone();
        assert_eq!(totems.len(), 2);
        if clear_first_with_retreat {
            retreat(
                &mut record,
                &player("p1"),
                card(&setup, &player("p1"), Element::Fire, 5),
                SecretStrategyEnvironmentOperation::Clear,
            );
            assert_eq!(record.state().totems, totems);
            assert_eq!(record.state().environment, None);
        } else {
            assert_eq!(record.state().environment, Some(Element::Metal));
        }
        let events = perform(
            &mut record,
            &player("p1"),
            "void-meridian-severing",
            [Element::Metal, Element::Wood, Element::Water]
                .map(|element| card(&setup, &player("p1"), element, 3))
                .to_vec(),
        );
        assert_eq!(record.state().environment, None);
        if clear_first_with_retreat {
            assert_eq!(record.state().totems, totems);
            assert!(!events.iter().any(|event| matches!(
                event,
                GameEvent::TotemChanged { .. } | GameEvent::EnvironmentCleared { .. }
            )));
        } else {
            assert!(record.state().totems.is_empty());
            assert_eq!(
                events
                    .iter()
                    .filter(|event| matches!(
                        event,
                        GameEvent::TotemChanged {
                            reason: TotemChangeReason::EnvironmentCleared,
                            ..
                        }
                    ))
                    .count(),
                2
            );
        }
        assert_eq!(record.replay().unwrap(), *record.state());
        assert_eq!(record.verify_replay().unwrap(), *record.state());
    }
}
