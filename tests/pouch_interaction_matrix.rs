use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, ChoiceAnswer, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent,
    GamePreparationStage, GameStatus, HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID,
    POUCH_MODULE_ID, PassiveNoEffectGround, Player, PlayerId, RuleModuleId, SPIRIT_MODULE_ID,
    STAR_MODULE_ID, SecretStrategy, TeamId,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::OfficialRules;

const POUCH_MODULES: [&str; 6] = [
    PERSONAL_DECK_MODULE_ID,
    STAR_MODULE_ID,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID,
    HERO_SCHOOLS_MODULE_ID,
    SPIRIT_MODULE_ID,
    POUCH_MODULE_ID,
];

struct PouchPreparationScenario {
    record: GameRecord,
    turn_order: Vec<PlayerId>,
}

impl PouchPreparationScenario {
    fn two_player() -> Self {
        Self::new(vec![
            Player {
                id: PlayerId::new("p1"),
                team: TeamId::new("team:p1"),
            },
            Player {
                id: PlayerId::new("p2"),
                team: TeamId::new("team:p2"),
            },
        ])
    }

    fn four_player() -> Self {
        Self::new(vec![
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
        ])
    }

    fn new(players: Vec<Player>) -> Self {
        let turn_order = players
            .iter()
            .map(|player| player.id.clone())
            .collect::<Vec<_>>();
        let setup = OfficialRules::new()
            .configure_game(
                players,
                turn_order.clone(),
                POUCH_MODULES.into_iter().map(RuleModuleId::new).collect(),
            )
            .unwrap();
        Self {
            record: GameRecord::start(setup, Vec::new()).unwrap(),
            turn_order,
        }
    }

    fn choose(&mut self, player: &PlayerId) -> Vec<GameEvent> {
        let card = self.record.state().deck_for(player).unwrap()[0];
        self.record
            .handle(Command::ChooseInitialPouch {
                player: player.clone(),
                card,
            })
            .unwrap()
    }

    fn resolve_initial_shuffles(&mut self) -> Vec<PlayerId> {
        let mut shuffled_players = Vec::new();
        while let Some(request) = self.record.state().pending_randomness.clone() {
            let fewfc::domain::RandomnessOperation::DeckShuffle {
                deck: fewfc::domain::RandomnessDeck::Player(player),
            } = request.operation
            else {
                unreachable!("initial Pouch preparation only shuffles Player Decks");
            };
            shuffled_players.push(player);
            let mut shuffled_order = request.current_order;
            shuffled_order.reverse();
            self.record
                .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order,
                })
                .unwrap();
        }
        shuffled_players
    }

    fn assert_completed(&self) {
        assert_eq!(self.record.state().status, GameStatus::InProgress);
        for (index, player) in self.turn_order.iter().enumerate() {
            assert!(self.record.state().pouch_for(player).is_some());
            assert_eq!(
                self.record.state().hand(player).unwrap().len(),
                if index == 0 { 4 } else { 5 }
            );
        }
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

struct PouchWatchFireScenario {
    record: GameRecord,
    p1_watch_pouch: CardInstanceId,
    p2_golden_pouch: CardInstanceId,
}

impl PouchWatchFireScenario {
    fn new() -> Self {
        let rules = OfficialRules::new();
        let p1 = PlayerId::new("p1");
        let p2 = PlayerId::new("p2");
        let setup = rules
            .configure_game(
                vec![
                    Player {
                        id: p1.clone(),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: p2.clone(),
                        team: TeamId::new("team:p2"),
                    },
                ],
                vec![p1.clone(), p2.clone()],
                POUCH_MODULES.into_iter().map(RuleModuleId::new).collect(),
            )
            .unwrap();
        let mut record = GameRecord::start(setup, Vec::new()).unwrap();

        let p1_watch_pouch = card_with(&record, &p1, Element::Fire, None);
        record
            .handle(Command::ChooseInitialPouch {
                player: p1.clone(),
                card: p1_watch_pouch,
            })
            .unwrap();
        let p2_golden_pouch = card_with(&record, &p2, Element::Metal, None);
        record
            .handle(Command::ChooseInitialPouch {
                player: p2.clone(),
                card: p2_golden_pouch,
            })
            .unwrap();

        while let Some(request) = record.state().pending_randomness.clone() {
            let fewfc::domain::RandomnessOperation::DeckShuffle {
                deck: fewfc::domain::RandomnessDeck::Player(player),
            } = request.operation
            else {
                unreachable!("Pouch preparation only shuffles Player Decks");
            };
            let mut remaining = request.current_order;
            let desired = if player == p1 {
                vec![
                    (Element::Wood, None),
                    (Element::Wood, None),
                    (Element::Metal, None),
                    (Element::Metal, None),
                    (Element::Fire, None),
                ]
            } else {
                vec![
                    (Element::Metal, None),
                    (Element::Wood, Some(1)),
                    (Element::Fire, Some(2)),
                    (Element::Earth, Some(3)),
                    (Element::Metal, None),
                ]
            };
            let mut ordered = desired
                .into_iter()
                .map(|(element, level)| take_card(record.state(), &mut remaining, element, level))
                .collect::<Vec<_>>();
            ordered.extend(remaining);
            record
                .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order: ordered,
                })
                .unwrap();
        }
        record.advance_automatic().unwrap();
        assert_eq!(record.state().current_player(), Some(&p1));

        Self {
            record,
            p1_watch_pouch,
            p2_golden_pouch,
        }
    }

    fn establish_watch_fire_and_advance_to_p2(&mut self) -> Vec<GameEvent> {
        let p1 = PlayerId::new("p1");
        let p2 = PlayerId::new("p2");
        let watch_events = self
            .record
            .handle(trigger_secret_strategy(&p1, SecretStrategy::WatchTheFire))
            .unwrap();
        assert!(matches!(
            watch_events.as_slice(),
            [
                GameEvent::PouchRevealed {
                    player,
                    card,
                    strategy: SecretStrategy::WatchTheFire,
                    ..
                },
                GameEvent::StatusAdded { status },
                GameEvent::PouchConsumed { card: consumed, .. },
            ] if player == &p1
                && *card == self.p1_watch_pouch
                && *consumed == self.p1_watch_pouch
                && status.owner == fewfc::domain::StatusOwner::Player(p2.clone())
                && status.kind == "PouchWatchFire"
        ));

        self.perform_metal_strike(&p1);
        self.finish_turn(&p1);
        assert_eq!(self.record.state().current_player(), Some(&p2));
        watch_events
    }

    fn establish_golden_cicada(&mut self) -> Vec<GameEvent> {
        let p2 = PlayerId::new("p2");
        let events = self
            .record
            .handle(trigger_secret_strategy(&p2, SecretStrategy::GoldenCicada))
            .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::PouchRevealed {
                    player,
                    card,
                    strategy: SecretStrategy::GoldenCicada,
                    ..
                },
                GameEvent::StatusAdded { status },
                GameEvent::PouchConsumed { card: consumed, .. },
            ] if player == &p2
                && *card == self.p2_golden_pouch
                && *consumed == self.p2_golden_pouch
                && status.owner == fewfc::domain::StatusOwner::Player(p2.clone())
                && status.kind == "PouchGoldenCicada"
        ));
        events
    }

    fn perform_metal_strike(&mut self, player: &PlayerId) -> Vec<GameEvent> {
        let card = card_in_hand_with(&self.record, player, Element::Metal);
        self.record
            .handle(Command::PerformFormation {
                player: player.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card],
                declared_targets: Vec::new(),
            })
            .unwrap()
    }

    fn perform_generating_formation(
        &mut self,
        player: &PlayerId,
    ) -> (Vec<CardInstanceId>, Vec<GameEvent>) {
        let cards = vec![
            card_with_level_in_hand(&self.record, player, Element::Wood, 1),
            card_with_level_in_hand(&self.record, player, Element::Fire, 2),
            card_with_level_in_hand(&self.record, player, Element::Earth, 3),
        ];
        let events = self
            .record
            .handle(Command::PerformFormation {
                player: player.clone(),
                formation_id: "generating-formation".to_string(),
                cards: cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap();
        (cards, events)
    }

    fn finish_turn(&mut self, player: &PlayerId) {
        self.record.advance_automatic().unwrap();
        let choice = self
            .record
            .state()
            .pending_choice
            .as_ref()
            .expect("a legal formation advances to the turn-draw discard choice");
        let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
            panic!("turn draw must request a card discard");
        };
        self.record
            .handle(Command::AnswerChoice {
                player: player.clone(),
                choice_id: choice.choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![cards[0]],
                },
            })
            .unwrap();
        self.record.advance_automatic().unwrap();
    }

    fn assert_replay_and_public_lifecycle(&self, expect_watch_fire: bool, expect_golden: bool) {
        let p2 = PlayerId::new("p2");
        let statuses = &self.record.state().statuses;
        assert_eq!(
            statuses.iter().any(|status| {
                status.owner == fewfc::domain::StatusOwner::Player(p2.clone())
                    && status.kind == "PouchWatchFire"
            }),
            expect_watch_fire
        );
        assert_eq!(
            statuses
                .iter()
                .any(|status| status.kind == "PouchGoldenCicada"),
            expect_golden
        );
        let public = state_for(self.record.state(), Viewer::Player(p2));
        assert_eq!(
            public
                .statuses
                .iter()
                .any(|status| status.kind == "PouchWatchFire"),
            expect_watch_fire
        );
        assert_eq!(
            public
                .statuses
                .iter()
                .any(|status| status.kind == "PouchGoldenCicada"),
            expect_golden
        );
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

fn trigger_secret_strategy(player: &PlayerId, strategy: SecretStrategy) -> Command {
    Command::TriggerSecretStrategy {
        player: player.clone(),
        strategy,
        target_player: None,
        star: None,
        break_star: false,
        discard_card: None,
        deck_cards: Vec::new(),
        discard_cards: Vec::new(),
    }
}

fn card_with(
    record: &GameRecord,
    player: &PlayerId,
    element: Element,
    level: Option<u32>,
) -> CardInstanceId {
    record
        .state()
        .deck_for(player)
        .unwrap()
        .iter()
        .copied()
        .find(|card| {
            record.state().card_def(*card).is_some_and(|definition| {
                definition.element == element
                    && level.is_none_or(|level| definition.level.value() == level)
            })
        })
        .unwrap()
}

fn card_in_hand_with(record: &GameRecord, player: &PlayerId, element: Element) -> CardInstanceId {
    record
        .state()
        .hand(player)
        .unwrap()
        .iter()
        .copied()
        .find(|card| {
            record
                .state()
                .card_def(*card)
                .is_some_and(|definition| definition.element == element)
        })
        .unwrap()
}

fn card_with_level_in_hand(
    record: &GameRecord,
    player: &PlayerId,
    element: Element,
    level: u32,
) -> CardInstanceId {
    record
        .state()
        .hand(player)
        .unwrap()
        .iter()
        .copied()
        .find(|card| {
            record.state().card_def(*card).is_some_and(|definition| {
                definition.element == element && definition.level.value() == level
            })
        })
        .unwrap()
}

fn card_in_hand_with_after(
    record: &GameRecord,
    player: &PlayerId,
    element: Element,
    skip: usize,
) -> CardInstanceId {
    record
        .state()
        .hand(player)
        .unwrap()
        .iter()
        .copied()
        .filter(|card| {
            record
                .state()
                .card_def(*card)
                .is_some_and(|definition| definition.element == element)
        })
        .nth(skip)
        .unwrap()
}

fn assert_ground_set(
    actual: &[PassiveNoEffectGround],
    expected: impl IntoIterator<Item = PassiveNoEffectGround>,
) {
    let expected = expected.into_iter().collect::<Vec<_>>();
    assert_eq!(actual.len(), expected.len());
    let actual = actual
        .iter()
        .copied()
        .collect::<std::collections::HashSet<_>>();
    let expected = expected
        .into_iter()
        .collect::<std::collections::HashSet<_>>();
    assert_eq!(actual, expected);
}

fn hp_for_player(record: &GameRecord, player: &PlayerId) -> i32 {
    let team = &record
        .state()
        .players
        .iter()
        .find(|candidate| &candidate.id == player)
        .unwrap()
        .team;
    record
        .state()
        .hp
        .iter()
        .find(|entry| &entry.team == team)
        .unwrap()
        .hp
}

fn take_card(
    state: &fewfc::domain::GameState,
    cards: &mut Vec<CardInstanceId>,
    element: Element,
    level: Option<u32>,
) -> CardInstanceId {
    let position = cards
        .iter()
        .position(|card| {
            state.card_def(*card).is_some_and(|definition| {
                definition.element == element
                    && level.is_none_or(|level| definition.level.value() == level)
            })
        })
        .unwrap();
    cards.remove(position)
}

fn ordered_pouch_record_for_benevolent_seal_matrix() -> GameRecord {
    let rules = OfficialRules::new();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let setup = rules
        .configure_game(
            vec![
                Player {
                    id: p1.clone(),
                    team: TeamId::new("team:p1"),
                },
                Player {
                    id: p2.clone(),
                    team: TeamId::new("team:p2"),
                },
            ],
            vec![p1.clone(), p2.clone()],
            POUCH_MODULES.into_iter().map(RuleModuleId::new).collect(),
        )
        .unwrap();
    let mut record = GameRecord::start(setup, Vec::new()).unwrap();
    let p1_watch = card_with(&record, &p1, Element::Fire, None);
    record
        .handle(Command::ChooseInitialPouch {
            player: p1.clone(),
            card: p1_watch,
        })
        .unwrap();
    let p2_golden = card_with(&record, &p2, Element::Metal, None);
    record
        .handle(Command::ChooseInitialPouch {
            player: p2.clone(),
            card: p2_golden,
        })
        .unwrap();

    while let Some(request) = record.state().pending_randomness.clone() {
        let fewfc::domain::RandomnessOperation::DeckShuffle {
            deck: fewfc::domain::RandomnessDeck::Player(player),
        } = request.operation
        else {
            unreachable!("Pouch preparation only shuffles Player Decks");
        };
        let mut remaining = request.current_order;
        let desired = if player == p1 {
            vec![
                (Element::Metal, None),
                (Element::Metal, None),
                (Element::Water, None),
                (Element::Water, None),
                (Element::Fire, None),
                (Element::Metal, None),
                (Element::Earth, None),
                (Element::Fire, None),
                (Element::Metal, None),
            ]
        } else {
            vec![
                (Element::Wood, Some(3)),
                (Element::Wood, Some(1)),
                (Element::Wood, Some(5)),
                (Element::Fire, Some(2)),
                (Element::Earth, Some(3)),
                (Element::Metal, None),
                (Element::Wood, Some(4)),
                (Element::Metal, None),
                (Element::Wood, Some(5)),
                (Element::Metal, None),
                (Element::Wood, Some(1)),
            ]
        };
        let mut ordered = desired
            .into_iter()
            .map(|(element, level)| take_card(record.state(), &mut remaining, element, level))
            .collect::<Vec<_>>();
        ordered.extend(remaining);
        record
            .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order: ordered,
            })
            .unwrap();
    }
    record.advance_automatic().unwrap();
    record
}

fn ordered_pouch_record_for_benevolent_magic_seal_matrix() -> GameRecord {
    let rules = OfficialRules::new();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let setup = rules
        .configure_game(
            vec![
                Player {
                    id: p1.clone(),
                    team: TeamId::new("team:p1"),
                },
                Player {
                    id: p2.clone(),
                    team: TeamId::new("team:p2"),
                },
            ],
            vec![p1.clone(), p2.clone()],
            POUCH_MODULES.into_iter().map(RuleModuleId::new).collect(),
        )
        .unwrap();
    let mut record = GameRecord::start(setup, Vec::new()).unwrap();
    let p1_watch = card_with(&record, &p1, Element::Fire, None);
    record
        .handle(Command::ChooseInitialPouch {
            player: p1.clone(),
            card: p1_watch,
        })
        .unwrap();
    let p2_golden = card_with(&record, &p2, Element::Metal, None);
    record
        .handle(Command::ChooseInitialPouch {
            player: p2.clone(),
            card: p2_golden,
        })
        .unwrap();

    while let Some(request) = record.state().pending_randomness.clone() {
        let fewfc::domain::RandomnessOperation::DeckShuffle {
            deck: fewfc::domain::RandomnessDeck::Player(player),
        } = request.operation
        else {
            unreachable!("Pouch preparation only shuffles Player Decks");
        };
        let mut remaining = request.current_order;
        let desired = if player == p1 {
            vec![
                (Element::Water, Some(3)),
                (Element::Water, Some(1)),
                (Element::Water, Some(5)),
                (Element::Metal, None),
                (Element::Metal, None),
                (Element::Water, Some(3)),
                (Element::Fire, None),
                (Element::Water, Some(3)),
                (Element::Fire, None),
                (Element::Fire, None),
            ]
        } else {
            vec![
                (Element::Wood, Some(3)),
                (Element::Wood, Some(1)),
                (Element::Wood, Some(5)),
                (Element::Fire, Some(2)),
                (Element::Earth, Some(3)),
                (Element::Metal, None),
                (Element::Wood, Some(4)),
                (Element::Metal, None),
                (Element::Wood, Some(5)),
                (Element::Metal, None),
                (Element::Wood, Some(1)),
            ]
        };
        let mut ordered = desired
            .into_iter()
            .map(|(element, level)| take_card(record.state(), &mut remaining, element, level))
            .collect::<Vec<_>>();
        ordered.extend(remaining);
        record
            .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order: ordered,
            })
            .unwrap();
    }
    record.advance_automatic().unwrap();
    record
}

fn finish_turn_discarding_element(record: &mut GameRecord, player: &PlayerId, element: Element) {
    record.advance_automatic().unwrap();
    let choice = record.state().pending_choice.as_ref().unwrap();
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("turn draw must request a card discard");
    };
    let discard = cards
        .iter()
        .copied()
        .find(|card| {
            record
                .state()
                .card_def(*card)
                .is_some_and(|definition| definition.element == element)
        })
        .unwrap_or(cards[0]);
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id: choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn perform_elemental_attack(record: &mut GameRecord, player: &PlayerId, element: Element) {
    let card = card_in_hand_with(record, player, element);
    record
        .handle(Command::PerformFormation {
            player: player.clone(),
            formation_id: format!(
                "{}-strike",
                match element {
                    Element::Metal => "metal",
                    Element::Wood => "wood",
                    Element::Water => "water",
                    Element::Fire => "fire",
                    Element::Earth => "earth",
                }
            ),
            cards: vec![card],
            declared_targets: Vec::new(),
        })
        .unwrap();
}

fn change_to(
    record: &mut GameRecord,
    player: &PlayerId,
    profession: &str,
    cards: Vec<CardInstanceId>,
) {
    record
        .handle(Command::ChangeProfession {
            player: player.clone(),
            profession: fewfc::domain::ProfessionId::new(profession),
            cards,
        })
        .unwrap();
}

#[test]
fn pouch_preparation_matrix_selection_completion_shuffles_deal_and_replay() {
    let mut scenario = PouchPreparationScenario::two_player();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    let first_events = scenario.choose(&p1);
    assert!(matches!(
        first_events.as_slice(),
        [
            GameEvent::PouchPlaced { owner, .. },
            GameEvent::InitialPouchChosen { player: chosen, .. },
        ] if owner == &p1 && chosen == &p1
    ));
    assert!(matches!(
        scenario.record.state().status,
        GameStatus::Preparing {
            stage: GamePreparationStage::InitialPouchSelection
        }
    ));
    assert!(scenario.record.state().pending_randomness.is_none());

    let completion_events = scenario.choose(&p2);
    assert!(matches!(
        completion_events.as_slice(),
        [
            GameEvent::PouchPlaced { owner, .. },
            GameEvent::InitialPouchChosen { player: chosen, .. },
            GameEvent::InitialPouchSelectionCompleted,
            GameEvent::RandomnessRequested { request },
        ] if owner == &p2
            && chosen == &p2
            && request.request_id == "pouch:initial-shuffle:p1"
    ));
    assert_eq!(scenario.resolve_initial_shuffles(), vec![p1, p2]);
    scenario.assert_completed();
}

#[test]
fn pouch_preparation_matrix_independent_arrival_order_converges_in_two_and_four_player_games() {
    for mut scenario in [
        PouchPreparationScenario::two_player(),
        PouchPreparationScenario::four_player(),
    ] {
        let forward_order = scenario.turn_order.clone();
        let mut reverse_order = forward_order.clone();
        reverse_order.reverse();

        for order in [forward_order, reverse_order] {
            let mut run = PouchPreparationScenario::new(scenario.record.state().players.clone());
            for player in &order {
                run.choose(player);
            }
            assert_eq!(run.resolve_initial_shuffles(), run.turn_order);
            run.assert_completed();

            if order == scenario.turn_order {
                scenario = run;
            } else {
                assert_eq!(scenario.record.state(), run.record.state());
                assert_eq!(
                    scenario.record.replay().unwrap(),
                    run.record.replay().unwrap()
                );
            }
        }
    }
}

#[test]
fn pouch_watch_fire_matrix_modifier_prevents_damage_but_not_formation_commitment_or_cards() {
    let mut scenario = PouchWatchFireScenario::new();
    let p1 = PlayerId::new("p1");
    scenario.establish_watch_fire_and_advance_to_p2();

    let p2 = PlayerId::new("p2");
    let hp_before = hp_for_player(&scenario.record, &p1);
    let events = scenario.perform_metal_strike(&p2);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. }
            if hp_change.delta == 0 && hp_change.effective_delta == 0
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCommitted { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCardsDiscarded { .. }))
    );
    assert_eq!(hp_for_player(&scenario.record, &p1), hp_before);
    scenario.assert_replay_and_public_lifecycle(true, false);
    scenario.finish_turn(&p2);
    scenario.assert_replay_and_public_lifecycle(false, false);
}

#[test]
fn pouch_watch_fire_matrix_golden_cicada_restores_damage_without_consuming_watch_fire() {
    let mut scenario = PouchWatchFireScenario::new();
    let p1 = PlayerId::new("p1");
    scenario.establish_watch_fire_and_advance_to_p2();
    scenario.establish_golden_cicada();

    let p2 = PlayerId::new("p2");
    let hp_before = hp_for_player(&scenario.record, &p1);
    let events = scenario.perform_metal_strike(&p2);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. } if hp_change.effective_delta < 0
    )));
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCommitted { .. }))
    );
    assert!(
        events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCardsDiscarded { .. }))
    );
    assert!(hp_for_player(&scenario.record, &p1) < hp_before);
    scenario.assert_replay_and_public_lifecycle(true, true);
    scenario.finish_turn(&p2);
    scenario.assert_replay_and_public_lifecycle(false, false);
}

#[test]
fn pouch_watch_fire_matrix_baseline_formation_hp_change_resolves_normally() {
    let mut scenario = PouchWatchFireScenario::new();
    let p1 = PlayerId::new("p1");
    scenario.perform_metal_strike(&p1);
    scenario.finish_turn(&p1);

    let p2 = PlayerId::new("p2");
    let hp_before = hp_for_player(&scenario.record, &p2);
    let (cards, events) = scenario.perform_generating_formation(&p2);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.effective_delta > 0
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards: committed, .. }
            if formation_id == "generating-formation" && committed == &cards
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards: discarded, .. }
            if formation_id == "generating-formation" && discarded == &cards
    )));
    assert!(hp_for_player(&scenario.record, &p2) > hp_before);
    scenario.assert_replay_and_public_lifecycle(false, false);
}

#[test]
fn pouch_watch_fire_matrix_modifier_prevents_formation_hp_change_but_not_commitment_or_cards() {
    let mut scenario = PouchWatchFireScenario::new();
    scenario.establish_watch_fire_and_advance_to_p2();

    let p2 = PlayerId::new("p2");
    let hp_before = hp_for_player(&scenario.record, &p2);
    let (cards, events) = scenario.perform_generating_formation(&p2);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change }
            if change.delta == 0 && change.effective_delta == 0
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards: committed, .. }
            if formation_id == "generating-formation" && committed == &cards
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards: discarded, .. }
            if formation_id == "generating-formation" && discarded == &cards
    )));
    assert_eq!(hp_for_player(&scenario.record, &p2), hp_before);
    scenario.assert_replay_and_public_lifecycle(true, false);
    scenario.finish_turn(&p2);
    scenario.assert_replay_and_public_lifecycle(false, false);
}

#[test]
fn pouch_watch_fire_matrix_golden_cicada_restores_formation_hp_change_without_consuming_watch_fire()
{
    let mut scenario = PouchWatchFireScenario::new();
    scenario.establish_watch_fire_and_advance_to_p2();
    scenario.establish_golden_cicada();

    let p2 = PlayerId::new("p2");
    let hp_before = hp_for_player(&scenario.record, &p2);
    let (cards, events) = scenario.perform_generating_formation(&p2);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.effective_delta > 0
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards: committed, .. }
            if formation_id == "generating-formation" && committed == &cards
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards: discarded, .. }
            if formation_id == "generating-formation" && discarded == &cards
    )));
    assert!(hp_for_player(&scenario.record, &p2) > hp_before);
    scenario.assert_replay_and_public_lifecycle(true, true);
    scenario.finish_turn(&p2);
    scenario.assert_replay_and_public_lifecycle(false, false);
}

#[test]
fn no_effect_ground_matrix_inapplicable_defense_does_not_collect_golden_cicada() {
    let mut scenario = PouchWatchFireScenario::new();
    let p1 = PlayerId::new("p1");
    let defense_cards = vec![
        card_in_hand_with(&scenario.record, &p1, Element::Wood),
        card_in_hand_with_after(&scenario.record, &p1, Element::Wood, 1),
    ];
    let cover_events = scenario
        .record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(cover_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveCovered { formation_id, cards, .. }
            if formation_id == "defense" && cards == &defense_cards
    )));
    scenario.finish_turn(&p1);

    let p2 = PlayerId::new("p2");
    scenario.establish_golden_cicada();
    let (_, events) = scenario.perform_generating_formation(&p2);
    let grounds = events
        .iter()
        .find_map(|event| match event {
            GameEvent::PassiveFlipped {
                passive_id,
                outcome: fewfc::domain::PassiveFlipOutcome::NoEffect { grounds },
                ..
            } if passive_id == "defense" => Some(grounds),
            _ => None,
        })
        .expect("Defense must flip once against the active spell");
    assert_ground_set(grounds, [PassiveNoEffectGround::NotAnAttack]);
    assert!(scenario.record.state().statuses.iter().any(|status| {
        status.owner == fewfc::domain::StatusOwner::Player(p2.clone())
            && status.kind == "PouchGoldenCicada"
    }));
    assert_eq!(
        scenario.record.replay().unwrap(),
        scenario.record.state().clone()
    );
    assert_eq!(
        scenario.record.verify_replay().unwrap(),
        scenario.record.state().clone()
    );
}

#[test]
fn no_effect_ground_matrix_benevolent_and_golden_cicada_both_preserve_sealed_generating_formation()
{
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = ordered_pouch_record_for_benevolent_seal_matrix();

    // 每個職業都在自己的回合透過合法命令取得。固定牌堆順序只提供三次合法
    // 轉換所需、與測試無關的背景卡牌。
    let transitions = ["seeker", "expounder", "benevolent"];
    for profession in transitions {
        perform_elemental_attack(&mut record, &p1, Element::Metal);
        finish_turn_discarding_element(&mut record, &p1, Element::Fire);
        assert_eq!(record.state().current_player(), Some(&p2));
        let cards = match profession {
            "seeker" => vec![card_with_level_in_hand(&record, &p2, Element::Wood, 3)],
            "expounder" => vec![
                card_with_level_in_hand(&record, &p2, Element::Wood, 1),
                card_with_level_in_hand(&record, &p2, Element::Wood, 5),
            ],
            "benevolent" => vec![
                card_with_level_in_hand(&record, &p2, Element::Wood, 4),
                card_with_level_in_hand(&record, &p2, Element::Wood, 5),
            ],
            _ => unreachable!(),
        };
        change_to(&mut record, &p2, profession, cards);
        finish_turn_discarding_element(&mut record, &p2, Element::Metal);
        assert_eq!(record.state().current_player(), Some(&p1));
    }

    let water_cards = vec![
        card_in_hand_with(&record, &p1, Element::Water),
        card_in_hand_with_after(&record, &p1, Element::Water, 1),
    ];
    let cover_events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: water_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(cover_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards, .. }
            if formation_id == "seal" && cards == &water_cards
    )));
    assert!(cover_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveCovered { formation_id, cards, .. }
            if formation_id == "seal" && cards == &water_cards
    )));
    finish_turn_discarding_element(&mut record, &p1, Element::Fire);
    assert_eq!(record.state().current_player(), Some(&p2));

    let golden_events = record
        .handle(trigger_secret_strategy(&p2, SecretStrategy::GoldenCicada))
        .unwrap();
    assert!(matches!(
        golden_events.as_slice(),
        [
            GameEvent::PouchRevealed {
                strategy: SecretStrategy::GoldenCicada,
                ..
            },
            GameEvent::StatusAdded { status },
            GameEvent::PouchConsumed { .. },
        ] if status.kind == "PouchGoldenCicada"
    ));

    let generating_cards = vec![
        card_with_level_in_hand(&record, &p2, Element::Wood, 1),
        card_with_level_in_hand(&record, &p2, Element::Fire, 2),
        card_with_level_in_hand(&record, &p2, Element::Earth, 3),
    ];
    let hp_before = hp_for_player(&record, &p2);
    let events = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "generating-formation".to_string(),
            cards: generating_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();

    let grounds = events
        .iter()
        .find_map(|event| match event {
            GameEvent::PassiveFlipped {
                passive_id,
                outcome: fewfc::domain::PassiveFlipOutcome::NoEffect { grounds },
                ..
            } if passive_id == "seal" => Some(grounds),
            _ => None,
        })
        .expect("Seal must flip once with its complete no-effect grounds");
    assert_ground_set(
        grounds,
        [
            PassiveNoEffectGround::IgnoredByProfessionAbility,
            PassiveNoEffectGround::IgnoredByGoldenCicada,
        ],
    );
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards, .. }
            if formation_id == "generating-formation" && cards == &generating_cards
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.effective_delta > 0
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards, .. }
            if formation_id == "generating-formation" && cards == &generating_cards
    )));
    assert!(record.state().covered_passive(&p1).is_none());
    assert!(
        water_cards
            .iter()
            .all(|card| record.state().discard_for(&p1).unwrap().contains(card))
    );
    assert!(
        record
            .public_events_for(Viewer::Player(p1.clone()))
            .iter()
            .any(|event| matches!(
                event,
                fewfc::public_view::PublicGameEvent::Public(GameEvent::PassiveFlipped {
                    passive_id,
                    outcome: fewfc::domain::PassiveFlipOutcome::NoEffect { grounds },
                    ..
                }) if passive_id == "seal"
                    && grounds.iter().copied().collect::<std::collections::HashSet<_>>()
                        == [
                            PassiveNoEffectGround::IgnoredByProfessionAbility,
                            PassiveNoEffectGround::IgnoredByGoldenCicada,
                        ].into_iter().collect()
            ))
    );
    assert!(hp_for_player(&record, &p2) > hp_before);
    assert!(record.state().statuses.iter().any(|status| {
        status.owner == fewfc::domain::StatusOwner::Player(p2.clone())
            && status.kind == "PouchGoldenCicada"
    }));
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn no_effect_ground_matrix_benevolent_and_golden_cicada_both_consume_magic_seal() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = ordered_pouch_record_for_benevolent_magic_seal_matrix();

    let mesmer = vec![card_with_level_in_hand(&record, &p1, Element::Water, 3)];
    change_to(&mut record, &p1, "mesmer", mesmer);
    finish_turn_discarding_element(&mut record, &p1, Element::Metal);
    let seeker = vec![card_with_level_in_hand(&record, &p2, Element::Wood, 3)];
    change_to(&mut record, &p2, "seeker", seeker);
    finish_turn_discarding_element(&mut record, &p2, Element::Metal);

    let spirit_mesmer = vec![
        card_with_level_in_hand(&record, &p1, Element::Water, 1),
        card_with_level_in_hand(&record, &p1, Element::Water, 5),
    ];
    change_to(&mut record, &p1, "spirit-mesmer", spirit_mesmer);
    finish_turn_discarding_element(&mut record, &p1, Element::Fire);
    let expounder = vec![
        card_with_level_in_hand(&record, &p2, Element::Wood, 1),
        card_with_level_in_hand(&record, &p2, Element::Wood, 5),
    ];
    change_to(&mut record, &p2, "expounder", expounder);
    finish_turn_discarding_element(&mut record, &p2, Element::Metal);

    perform_elemental_attack(&mut record, &p1, Element::Metal);
    finish_turn_discarding_element(&mut record, &p1, Element::Fire);
    let benevolent = vec![
        card_with_level_in_hand(&record, &p2, Element::Wood, 4),
        card_with_level_in_hand(&record, &p2, Element::Wood, 5),
    ];
    change_to(&mut record, &p2, "benevolent", benevolent);
    finish_turn_discarding_element(&mut record, &p2, Element::Metal);

    let magic_seal_cards = vec![
        card_with_level_in_hand(&record, &p1, Element::Water, 3),
        card_in_hand_with_after(&record, &p1, Element::Water, 1),
    ];
    let magic_seal_events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "magic-seal".to_string(),
            cards: magic_seal_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(magic_seal_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards, .. }
            if formation_id == "magic-seal" && cards == &magic_seal_cards
    )));
    assert!(magic_seal_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(effects),
            ..
        } if effects.counter_effects_established.iter().any(|counter| counter.effect_id == "magic-seal")
    )));
    finish_turn_discarding_element(&mut record, &p1, Element::Fire);

    record
        .handle(trigger_secret_strategy(&p2, SecretStrategy::GoldenCicada))
        .unwrap();
    let generating_cards = vec![
        card_with_level_in_hand(&record, &p2, Element::Wood, 1),
        card_with_level_in_hand(&record, &p2, Element::Fire, 2),
        card_with_level_in_hand(&record, &p2, Element::Earth, 3),
    ];
    let hp_before = hp_for_player(&record, &p2);
    let events = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "generating-formation".to_string(),
            cards: generating_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();

    let grounds = events
        .iter()
        .find_map(|event| match event {
            GameEvent::CounterEffectResolved {
                effect_id,
                outcome: fewfc::domain::PassiveFlipOutcome::NoEffect { grounds },
                ..
            } if effect_id == "magic-seal" => Some(grounds),
            _ => None,
        })
        .expect("Magic Seal must resolve once and expose every sufficient ground");
    assert_ground_set(
        grounds,
        [
            PassiveNoEffectGround::IgnoredByProfessionAbility,
            PassiveNoEffectGround::IgnoredByGoldenCicada,
        ],
    );
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.effective_delta > 0
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards, .. }
            if formation_id == "generating-formation" && cards == &generating_cards
    )));
    assert!(hp_for_player(&record, &p2) > hp_before);
    assert!(
        !record
            .state()
            .counter_effects
            .iter()
            .any(|counter| counter.effect_id == "magic-seal")
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn no_effect_ground_matrix_golden_cicada_alone_does_not_implicate_profession_ability() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = ordered_pouch_record_for_benevolent_magic_seal_matrix();

    let mesmer = vec![card_with_level_in_hand(&record, &p1, Element::Water, 3)];
    change_to(&mut record, &p1, "mesmer", mesmer);
    finish_turn_discarding_element(&mut record, &p1, Element::Metal);
    let seeker = vec![card_with_level_in_hand(&record, &p2, Element::Wood, 3)];
    change_to(&mut record, &p2, "seeker", seeker);
    finish_turn_discarding_element(&mut record, &p2, Element::Metal);

    let spirit_mesmer = vec![
        card_with_level_in_hand(&record, &p1, Element::Water, 1),
        card_with_level_in_hand(&record, &p1, Element::Water, 5),
    ];
    change_to(&mut record, &p1, "spirit-mesmer", spirit_mesmer);
    finish_turn_discarding_element(&mut record, &p1, Element::Fire);
    let expounder = vec![
        card_with_level_in_hand(&record, &p2, Element::Wood, 1),
        card_with_level_in_hand(&record, &p2, Element::Wood, 5),
    ];
    change_to(&mut record, &p2, "expounder", expounder);
    finish_turn_discarding_element(&mut record, &p2, Element::Metal);

    let magic_seal_cards = vec![
        card_with_level_in_hand(&record, &p1, Element::Water, 3),
        card_in_hand_with_after(&record, &p1, Element::Water, 1),
    ];
    let magic_seal_events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "magic-seal".to_string(),
            cards: magic_seal_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(magic_seal_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(effects),
            ..
        } if effects.counter_effects_established.iter().any(|counter| counter.effect_id == "magic-seal")
    )));
    finish_turn_discarding_element(&mut record, &p1, Element::Fire);

    record
        .handle(trigger_secret_strategy(&p2, SecretStrategy::GoldenCicada))
        .unwrap();
    let generating_cards = vec![
        card_in_hand_with(&record, &p2, Element::Wood),
        card_with_level_in_hand(&record, &p2, Element::Fire, 2),
        card_with_level_in_hand(&record, &p2, Element::Earth, 3),
    ];
    let hp_before = hp_for_player(&record, &p2);
    let events = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "generating-formation".to_string(),
            cards: generating_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    let grounds = events
        .iter()
        .find_map(|event| match event {
            GameEvent::CounterEffectResolved {
                effect_id,
                outcome: fewfc::domain::PassiveFlipOutcome::NoEffect { grounds },
                ..
            } if effect_id == "magic-seal" => Some(grounds),
            _ => None,
        })
        .expect("Magic Seal must resolve once against Golden Cicada");
    assert_ground_set(grounds, [PassiveNoEffectGround::IgnoredByGoldenCicada]);
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::HpChanged { change } if change.effective_delta > 0
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards, .. }
            if formation_id == "generating-formation" && cards == &generating_cards
    )));
    assert!(hp_for_player(&record, &p2) > hp_before);
    assert!(
        !record
            .state()
            .counter_effects
            .iter()
            .any(|counter| counter.effect_id == "magic-seal")
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn no_effect_outcome_serializes_all_grounds_without_a_primary_reason() {
    let outcome = fewfc::domain::PassiveFlipOutcome::NoEffect {
        grounds: vec![
            PassiveNoEffectGround::IgnoredByProfessionAbility,
            PassiveNoEffectGround::IgnoredByGoldenCicada,
        ],
    };
    assert_eq!(
        serde_json::to_value(outcome).unwrap(),
        serde_json::json!({
            "NoEffect": {
                "grounds": ["IgnoredByProfessionAbility", "IgnoredByGoldenCicada"]
            }
        })
    );
}
