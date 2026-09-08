use fewfc::application::GameRecord;
use fewfc::domain::*;
use fewfc::public_view::{PublicCardRefs, PublicGameEvent, Viewer, event_for, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};

fn p1() -> PlayerId {
    PlayerId::new("p1")
}
fn p2() -> PlayerId {
    PlayerId::new("p2")
}

fn setup(personal: bool) -> GameSetup {
    let base = GameSetup::two_player(p1(), p2(), 100);
    let mut modules = vec![
        STAR_MODULE_ID,
        FIVE_DIRECTIONS_LEGEND_MODULE_ID,
        HERO_SCHOOLS_MODULE_ID,
        TOTEM_FORMATION_MODULE_ID,
    ];
    if personal {
        modules.push(PERSONAL_DECK_MODULE_ID);
    }
    OfficialRules::new()
        .configure_versioned_game_with_decks(
            RuleVersion::V5_17,
            base.players,
            base.turn_order,
            modules.into_iter().map(RuleModuleId::new).collect(),
            vec![],
        )
        .unwrap()
}

fn card(setup: &GameSetup, owner: &PlayerId, element: Element, level: u32) -> CardInstanceId {
    setup
        .card_instances
        .iter()
        .find(|instance| {
            (!setup
                .enabled_rule_modules
                .iter()
                .any(|module| module.as_str() == PERSONAL_DECK_MODULE_ID)
                || instance.origin == CardOrigin::Player(owner.clone()))
                && setup.card_defs.iter().any(|definition| {
                    definition.id == instance.definition
                        && definition.element == element
                        && definition.level.value() == level
                })
        })
        .unwrap()
        .instance
}

fn start(setup: GameSetup, leading: Vec<CardInstanceId>) -> GameRecord {
    let mut deck = leading.clone();
    deck.extend(
        OfficialRules::new()
            .official_deck_order(&setup)
            .unwrap()
            .into_iter()
            .filter(|card| !leading.contains(card)),
    );
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();
    record
}

fn array_record(element: Element) -> (GameRecord, Vec<CardInstanceId>) {
    let setup = setup(false);
    let cards = [1, 2, 5]
        .map(|level| card(&setup, &p1(), element, level))
        .to_vec();
    (start(setup, cards.clone()), cards)
}

fn perform(
    record: &mut GameRecord,
    player: PlayerId,
    id: &str,
    cards: Vec<CardInstanceId>,
) -> Vec<GameEvent> {
    record
        .handle(Command::PerformFormation {
            player,
            formation_id: id.into(),
            cards,
            declared_targets: vec![],
        })
        .unwrap()
}
fn answer(record: &mut GameRecord, answer: ChoiceAnswer) -> Vec<GameEvent> {
    let choice = record.state().pending_choice.as_ref().unwrap();
    record
        .handle(Command::AnswerChoice {
            player: choice.player.clone(),
            choice_id: choice.choice_id,
            answer,
        })
        .unwrap()
}
fn replay(record: &GameRecord) {
    assert_eq!(record.replay().unwrap(), *record.state());
    assert_eq!(record.verify_replay().unwrap(), *record.state());
}

#[test]
fn five_arrays_match_exact_level_five_and_resolve_their_public_effects() {
    for (id, element, totem) in [
        ("east-spirit-array", Element::Wood, TotemKind::AzureHorn),
        ("west-spirit-array", Element::Metal, TotemKind::WhiteFang),
        (
            "south-spirit-array",
            Element::Fire,
            TotemKind::VermilionFeather,
        ),
        ("north-spirit-array", Element::Water, TotemKind::BlackShell),
        (
            "central-spirit-array",
            Element::Earth,
            TotemKind::YellowScales,
        ),
    ] {
        let setup = setup(false);
        let leading = [1, 2, 5, 4].map(|level| card(&setup, &p1(), element, level));
        let mut record = start(setup, leading.to_vec());
        let offered = |cards: &[CardInstanceId]| {
            OfficialRules::new().playable_actions(record.state(), &p1(), cards).unwrap().iter().any(|action| matches!(action, PlayableAction::PerformFormation(candidate) if candidate.formation_id == id))
        };
        assert!(offered(&leading[..3]), "{id}");
        assert!(
            !offered(&[leading[0], leading[1], leading[3]]),
            "level four cannot replace exact five: {id}"
        );
        let before_hp = record.state().hp.clone();
        let events = perform(&mut record, p1(), id, leading[..3].to_vec());
        if id == "south-spirit-array" {
            answer(
                &mut record,
                ChoiceAnswer::Environment {
                    environment: Element::Fire,
                },
            );
        }
        if id == "central-spirit-array" {
            let chosen = record.state().hand(&p2()).unwrap()[0];
            answer(
                &mut record,
                ChoiceAnswer::Cards {
                    cards: vec![chosen],
                },
            );
            assert!(record.state().hand(&p1()).unwrap().contains(&chosen));
            assert!(!record.state().hand(&p2()).unwrap().contains(&chosen));
        }
        assert_eq!(record.state().environment, Some(element));
        assert_eq!(
            record.state().totems,
            vec![PlayerTotem {
                player: p1(),
                totem
            }]
        );
        if id == "east-spirit-array" {
            assert_eq!(record.state().shield(&p1()), Some(15));
            assert!(events.iter().any(
                |event| matches!(event, GameEvent::HpChanged { change } if change.delta() == 15)
            ));
        }
        if id == "west-spirit-array" {
            assert_eq!(record.state().hp[1].hp, before_hp[1].hp - 30);
        }
        if id == "north-spirit-array" {
            assert_eq!(
                record.state().turn_draw_bonus_by_player.get(&p1()),
                Some(&1)
            );
            assert!(events.iter().any(
                |event| matches!(event, GameEvent::HandInspected { target, .. } if target == &p2())
            ));
            for kind in ["CannotAct", "CannotDraw"] {
                assert!(
                    record
                        .state()
                        .statuses
                        .iter()
                        .any(|status| status.kind == kind
                            && status.owner == StatusOwner::Player(p2()))
                );
            }
        }
        assert!(record.state().pending_choice.is_none());
        replay(&record);
    }
}

#[test]
fn yellow_nonempty_hand_choice_cannot_be_declined_or_answered_empty() {
    let (mut record, cards) = array_record(Element::Earth);
    perform(&mut record, p1(), "central-spirit-array", cards);
    let choice = record.state().pending_choice.as_ref().unwrap().clone();
    assert!(matches!(
        choice.kind,
        PendingChoiceKind::Card {
            minimum: 1,
            maximum: 1,
            can_decline: false,
            ..
        }
    ));
    let before = record.state().clone();
    for answer in [ChoiceAnswer::Decline, ChoiceAnswer::Cards { cards: vec![] }] {
        assert!(
            record
                .handle(Command::AnswerChoice {
                    player: p1(),
                    choice_id: choice.choice_id,
                    answer
                })
                .is_err()
        );
        assert_eq!(record.state(), &before);
    }
    let chosen = record.state().hand(&p2()).unwrap()[0];
    answer(
        &mut record,
        ChoiceAnswer::Cards {
            cards: vec![chosen],
        },
    );
    replay(&record);
}

#[test]
fn dragon_search_reveals_then_shuffles_and_draws_into_an_ordinary_hidden_hand() {
    for personal in [false, true] {
        let setup = setup(personal);
        let leading = [1, 2, 3, 4].map(|level| card(&setup, &p1(), Element::Metal, level));
        let mut record = start(setup, leading.to_vec());
        perform(&mut record, p1(), "dragon-search", leading[..2].to_vec());
        let choice = record.state().pending_choice.as_ref().unwrap();
        let PendingChoiceKind::Card {
            cards,
            minimum: 1,
            maximum: 1,
            can_decline: true,
        } = &choice.kind
        else {
            panic!("search choice")
        };
        assert!(!cards.is_empty());
        for selected in cards {
            assert_eq!(record.state().card_def(*selected).unwrap().level.value(), 5);
            assert!(matches!(
                record.state().card_element(*selected),
                Some(Element::Metal | Element::Water)
            ));
        }
        let selected = cards[0];
        let events = answer(
            &mut record,
            ChoiceAnswer::Cards {
                cards: vec![selected],
            },
        );
        let reveal = events.iter().find(|event| matches!(event, GameEvent::DragonSearchRevealed { card, .. } if *card == selected)).unwrap();
        assert_eq!(
            event_for(reveal, Viewer::Observer),
            PublicGameEvent::Public(reveal.clone())
        );
        let request = record.state().pending_randomness.as_ref().unwrap().clone();
        assert!(
            !request.current_order.contains(&selected),
            "revealed card must be removed before shuffling remaining deck"
        );
        let mut order = request.current_order;
        order.reverse();
        record
            .resolve_randomness(TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order: order,
            })
            .unwrap();
        assert_eq!(
            record.state().deck_for(&p1()).unwrap().first(),
            Some(&selected)
        );
        record.advance_automatic().unwrap();
        let choice = record.state().pending_choice.as_ref().unwrap();
        let PendingChoiceKind::Card { cards, .. } = &choice.kind else {
            panic!("turn draw")
        };
        assert!(cards.contains(&selected));
        let discarded = *cards.iter().find(|card| **card != selected).unwrap();
        answer(
            &mut record,
            ChoiceAnswer::Cards {
                cards: vec![discarded],
            },
        );
        assert!(record.state().hand(&p1()).unwrap().contains(&selected));
        let public = state_for(record.state(), Viewer::Observer);
        assert!(matches!(
            public
                .hands
                .iter()
                .find(|hand| hand.player == p1())
                .unwrap()
                .cards,
            PublicCardRefs::Hidden { .. }
        ));
        replay(&record);
    }
}

#[test]
fn south_lethal_attack_completes_environment_transfer_and_totem_acquisition() {
    let mut setup = setup(false);
    setup.hp[1].hp = 10;
    let cards = [1, 2, 5]
        .map(|level| card(&setup, &p1(), Element::Fire, level))
        .to_vec();
    let mut record = start(setup, cards.clone());
    perform(&mut record, p1(), "south-spirit-array", cards);
    answer(
        &mut record,
        ChoiceAnswer::Environment {
            environment: Element::Wood,
        },
    );
    assert_eq!(record.state().hp[1].hp, 0);
    assert_eq!(record.state().environment, Some(Element::Fire));
    assert_eq!(
        record.state().totems,
        vec![PlayerTotem {
            player: p1(),
            totem: TotemKind::VermilionFeather
        }]
    );
    replay(&record);
}

#[test]
fn yellow_empty_hand_skips_the_choice_but_still_grants_environment_and_totem() {
    let mut setup = setup(false);
    // 開局設定手牌上限為零；既有初始手牌仍可施展，回合抽牌依上限正式跳過。
    setup.hand_limit = 0;
    let earth = [1, 2, 5]
        .map(|level| card(&setup, &p1(), Element::Earth, level))
        .to_vec();
    let spare = card(&setup, &p1(), Element::Fire, 1);
    let opponent = [
        Element::Metal,
        Element::Wood,
        Element::Water,
        Element::Fire,
        Element::Earth,
    ]
    .map(|element| card(&setup, &p2(), element, 3))
    .to_vec();
    let mut leading = earth.clone();
    leading.push(spare);
    leading.extend(opponent.clone());
    let mut record = start(setup, leading);
    perform(&mut record, p1(), "fire-strike", vec![spare]);
    record.advance_automatic().unwrap();
    perform(&mut record, p2(), "five-streams-unite", opponent);
    record.advance_automatic().unwrap();
    assert!(record.state().hand(&p2()).unwrap().is_empty());
    perform(&mut record, p1(), "central-spirit-array", earth);
    assert!(record.state().pending_choice.is_none());
    assert_eq!(record.state().environment, Some(Element::Earth));
    assert_eq!(
        record.state().totems,
        vec![PlayerTotem {
            player: p1(),
            totem: TotemKind::YellowScales
        }]
    );
    replay(&record);
}

#[test]
fn dragon_search_decline_still_shuffles_without_a_public_card_reveal() {
    let setup = setup(false);
    let leading = [1, 2, 3, 4].map(|level| card(&setup, &p1(), Element::Metal, level));
    let mut record = start(setup, leading.to_vec());
    perform(&mut record, p1(), "dragon-search", leading[..2].to_vec());
    let events = answer(&mut record, ChoiceAnswer::Decline);
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::DragonSearchRevealed { .. }))
    );
    let request = record.state().pending_randomness.as_ref().unwrap().clone();
    let mut order = request.current_order;
    order.reverse();
    record
        .resolve_randomness(TrustedRandomnessAnswer {
            request_id: request.request_id,
            shuffled_order: order.clone(),
        })
        .unwrap();
    assert_eq!(record.state().deck_for(&p1()).unwrap(), order);
    assert!(record.state().pending_choice.is_none());
    assert!(record.state().pending_randomness.is_none());
    replay(&record);
}

#[test]
fn dragon_search_recycles_only_an_empty_deck_not_a_nonempty_deck_without_matches() {
    use fewfc::application::{apply_event, handle_command, resolve_trusted_randomness};
    for empty in [false, true] {
        let setup = setup(false);
        let selected = [1, 2]
            .map(|level| card(&setup, &p1(), Element::Metal, level))
            .to_vec();
        let eligible = card(&setup, &p1(), Element::Water, 5);
        let ineligible = card(&setup, &p1(), Element::Earth, 3);
        let mut state = GameState::from_setup(&setup);
        state.phase = Phase::ActiveEffects;
        state.hands[0].cards = selected.clone();
        // 待測檢索從固定牌堆／棄牌堆開始，供牌與選擇仍經正式命令及隨機性流程。
        state.deck = if empty { vec![] } else { vec![ineligible] };
        state.discard = vec![eligible];
        let baseline = state.clone();
        let events = handle_command(
            &state,
            Command::PerformFormation {
                player: p1(),
                formation_id: "dragon-search".into(),
                cards: selected,
                declared_targets: vec![],
            },
        )
        .unwrap();
        let mut all = events.clone();
        for event in events {
            apply_event(&mut state, &event);
        }
        if empty {
            let request = state
                .pending_randomness
                .as_ref()
                .expect("empty deck must recycle discard")
                .clone();
            let events = resolve_trusted_randomness(
                &state,
                &TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order: request.current_order,
                },
            )
            .unwrap();
            all.extend(events.clone());
            for event in events {
                apply_event(&mut state, &event);
            }
            let PendingChoiceKind::Card { cards, .. } =
                &state.pending_choice.as_ref().unwrap().kind
            else {
                panic!("search selection")
            };
            assert_eq!(cards, &vec![eligible]);
        } else {
            assert!(state.pending_randomness.is_none());
            let PendingChoiceKind::Card {
                cards,
                can_decline: true,
                ..
            } = &state.pending_choice.as_ref().unwrap().kind
            else {
                panic!("optional search selection")
            };
            assert!(cards.is_empty());
            assert_eq!(state.discard, vec![eligible]);
            assert_eq!(state.deck, vec![ineligible]);
        }
        let mut replayed = baseline;
        for event in all {
            apply_event(&mut replayed, &event);
        }
        assert_eq!(replayed, state);
    }
}
