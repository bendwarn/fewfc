use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, ChoiceAnswer, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent,
    HERO_SCHOOLS_MODULE_ID, JIANGHU_MODULE_ID, JianghuStateKind, PERSONAL_DECK_MODULE_ID,
    POUCH_MODULE_ID, PassiveFlipOutcome, PassiveNoEffectGround, Player, PlayerId, RuleModuleId,
    SPIRIT_MODULE_ID, STAR_MODULE_ID, SecretStrategy, TeamId,
};
use fewfc::rules::OfficialRules;

const POISON_SMOKE_MODULES: [&str; 7] = [
    PERSONAL_DECK_MODULE_ID,
    STAR_MODULE_ID,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID,
    HERO_SCHOOLS_MODULE_ID,
    SPIRIT_MODULE_ID,
    POUCH_MODULE_ID,
    JIANGHU_MODULE_ID,
];

struct PoisonSmokeScenario {
    record: GameRecord,
    p1: PlayerId,
    p2: PlayerId,
    p2_golden_pouch: CardInstanceId,
    poison_smoke_cards: Vec<CardInstanceId>,
}

impl PoisonSmokeScenario {
    fn new() -> Self {
        let p1 = PlayerId::new("p1");
        let p2 = PlayerId::new("p2");
        let setup = OfficialRules::new()
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
                POISON_SMOKE_MODULES
                    .into_iter()
                    .map(RuleModuleId::new)
                    .collect(),
            )
            .unwrap();
        let mut record = GameRecord::start(setup, Vec::new()).unwrap();

        let p1_pouch = card_in_deck(&record, &p1, Element::Earth, 5);
        let p2_golden_pouch = card_in_deck(&record, &p2, Element::Metal, 5);
        record
            .handle(Command::ChooseInitialPouch {
                player: p1.clone(),
                card: p1_pouch,
            })
            .unwrap();
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
                unreachable!("Pouch preparation only requests Personal Deck shuffles");
            };
            let mut remaining = request.current_order;
            let desired = if player == p1 {
                vec![
                    (Element::Metal, 1),
                    (Element::Water, 1),
                    (Element::Fire, 2),
                    (Element::Wood, 2),
                    (Element::Earth, 1),
                    (Element::Metal, 5),
                ]
            } else {
                vec![
                    (Element::Earth, 2),
                    (Element::Water, 2),
                    (Element::Metal, 1),
                    (Element::Fire, 4),
                    (Element::Wood, 4),
                ]
            };
            let mut ordered = desired
                .into_iter()
                .map(|(element, level)| take_card(&record, &mut remaining, element, level))
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

        let first_wanderer_card = card_in_hand(&record, &p1, Element::Metal, 1);
        change_profession(
            &mut record,
            &p1,
            "first-wanderer",
            vec![first_wanderer_card],
        );
        finish_turn(&mut record, &p1);

        perform_strike(&mut record, &p2, Element::Earth, 2);
        finish_turn(&mut record, &p2);

        let poisoner_cards = vec![
            card_in_hand(&record, &p1, Element::Water, 1),
            card_in_hand(&record, &p1, Element::Earth, 1),
        ];
        change_profession(&mut record, &p1, "jianghu:poisoner", poisoner_cards);
        finish_turn(&mut record, &p1);

        perform_strike(&mut record, &p2, Element::Water, 2);
        finish_turn(&mut record, &p2);

        let poison_smoke_cards = vec![
            card_in_hand(&record, &p1, Element::Fire, 2),
            card_in_hand(&record, &p1, Element::Wood, 2),
        ];
        let cover_events = record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "jianghu:poison-smoke".to_string(),
                cards: poison_smoke_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(matches!(
            cover_events.as_slice(),
            [
                GameEvent::FormationCommitted { player, formation_id, cards, .. },
                GameEvent::PassiveCovered { player: covered, formation_id: passive_id, cards: covered_cards, .. },
            ] if player == &p1
                && formation_id == "jianghu:poison-smoke"
                && cards == &poison_smoke_cards
                && covered == &p1
                && passive_id == "jianghu:poison-smoke"
                && covered_cards == &poison_smoke_cards
        ));
        assert!(record.state().covered_passive(&p1).is_some());
        finish_turn(&mut record, &p1);
        assert_eq!(record.state().current_player(), Some(&p2));

        Self {
            record,
            p1,
            p2,
            p2_golden_pouch,
            poison_smoke_cards,
        }
    }

    fn trigger_golden_cicada(&mut self) -> Vec<GameEvent> {
        let events = self
            .record
            .handle(Command::TriggerSecretStrategy {
                player: self.p2.clone(),
                decision: fewfc::domain::SecretStrategyDecision::NoInput {
                    source_card: self.p2_golden_pouch,
                    strategy: SecretStrategy::GoldenCicada,
                },
            })
            .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::PouchRevealed { player, card, strategy: SecretStrategy::GoldenCicada, .. },
                GameEvent::StatusAdded { status },
                GameEvent::PouchConsumed { owner: Some(owner), card: consumed },
            ] if player == &self.p2
                && *card == self.p2_golden_pouch
                && status.owner == fewfc::domain::StatusOwner::Player(self.p2.clone())
                && status.kind == "PouchGoldenCicada"
                && owner == &self.p2
                && *consumed == self.p2_golden_pouch
        ));
        assert!(
            self.record
                .state()
                .discard_for(&self.p2)
                .unwrap()
                .contains(&self.p2_golden_pouch)
        );
        events
    }

    fn trigger_poison_smoke_with_metal_strike(&mut self) -> Vec<GameEvent> {
        let metal = card_in_hand(&self.record, &self.p2, Element::Metal, 1);
        self.record
            .handle(Command::PerformFormation {
                player: self.p2.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![metal],
                declared_targets: Vec::new(),
            })
            .unwrap()
    }

    fn assert_passive_cards_moved_and_replayed(&self) {
        assert!(self.record.state().covered_passive(&self.p1).is_none());
        assert!(self.poison_smoke_cards.iter().all(|card| {
            self.record
                .state()
                .discard_for(&self.p1)
                .unwrap()
                .contains(card)
        }));
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

fn change_profession(
    record: &mut GameRecord,
    player: &PlayerId,
    profession: &str,
    cards: Vec<CardInstanceId>,
) {
    let events = record
        .handle(Command::ChangeProfession {
            player: player.clone(),
            profession: fewfc::domain::ProfessionId::new(profession),
            cards,
        })
        .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::ProfessionChanged { player: changed, profession: changed_to, .. }
            if changed == player && changed_to.as_str() == profession
    )));
}

fn finish_turn(record: &mut GameRecord, player: &PlayerId) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("a legal action advances to the turn-draw discard choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("turn draw must request a card discard");
    };
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id: choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![*cards.last().unwrap()],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn perform_strike(record: &mut GameRecord, player: &PlayerId, element: Element, level: u32) {
    let card = card_in_hand(record, player, element, level);
    let formation_id = match element {
        Element::Metal => "metal-strike",
        Element::Wood => "wood-strike",
        Element::Water => "water-strike",
        Element::Fire => "fire-strike",
        Element::Earth => "earth-strike",
    };
    record
        .handle(Command::PerformFormation {
            player: player.clone(),
            formation_id: formation_id.to_string(),
            cards: vec![card],
            declared_targets: Vec::new(),
        })
        .unwrap();
}

fn card_in_deck(
    record: &GameRecord,
    player: &PlayerId,
    element: Element,
    level: u32,
) -> CardInstanceId {
    find_card(
        record,
        record.state().deck_for(player).unwrap(),
        element,
        level,
    )
}

fn card_in_hand(
    record: &GameRecord,
    player: &PlayerId,
    element: Element,
    level: u32,
) -> CardInstanceId {
    find_card(record, record.state().hand(player).unwrap(), element, level)
}

fn find_card(
    record: &GameRecord,
    cards: &[CardInstanceId],
    element: Element,
    level: u32,
) -> CardInstanceId {
    cards
        .iter()
        .copied()
        .find(|card| {
            record.state().card_def(*card).is_some_and(|definition| {
                definition.element == element && definition.level.value() == level
            })
        })
        .unwrap()
}

fn take_card(
    record: &GameRecord,
    cards: &mut Vec<CardInstanceId>,
    element: Element,
    level: u32,
) -> CardInstanceId {
    let card = find_card(record, cards, element, level);
    cards.remove(
        cards
            .iter()
            .position(|candidate| *candidate == card)
            .unwrap(),
    );
    card
}

fn assert_ground_set(actual: &[PassiveNoEffectGround], expected: &[PassiveNoEffectGround]) {
    assert_eq!(actual.len(), expected.len());
    assert_eq!(
        actual
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>(),
        expected
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>()
    );
}

fn assert_unaffected_attack_lifecycle(events: &[GameEvent], p2: &PlayerId) {
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, .. }
            if player == p2 && formation_id == "metal-strike"
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. } if hp_change.effective_delta < 0
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, .. }
            if player == p2 && formation_id == "metal-strike"
    )));
}

#[test]
fn poison_smoke_matrix_applied_flip_creates_poison_after_legal_profession_and_cover() {
    let mut scenario = PoisonSmokeScenario::new();

    let events = scenario.trigger_poison_smoke_with_metal_strike();
    assert!(matches!(
        events.iter().find(|event| matches!(event, GameEvent::PassiveFlipped { .. })),
        Some(GameEvent::PassiveFlipped {
            owner,
            incoming_player,
            passive_id,
            cards,
            outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
        }) if owner == &scenario.p1
            && incoming_player == &scenario.p2
            && passive_id == "jianghu:poison-smoke"
            && cards == &scenario.poison_smoke_cards
            && effect_id == "jianghu:poison-smoke"
            && modifications.is_empty()
    ));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::JianghuStateApplied { state }
            if state.owner == scenario.p2
                && state.kind == JianghuStateKind::Poison
                && state.remaining_turns == 1
    )));
    assert_unaffected_attack_lifecycle(&events, &scenario.p2);
    assert!(scenario.record.state().jianghu_states.iter().any(|state| {
        state.owner == scenario.p2
            && state.kind == JianghuStateKind::Poison
            && state.remaining_turns == 1
    }));
    scenario.assert_passive_cards_moved_and_replayed();
}

#[test]
fn poison_smoke_matrix_golden_cicada_no_effect_prevents_poison_but_preserves_attack_lifecycle() {
    let mut scenario = PoisonSmokeScenario::new();
    scenario.trigger_golden_cicada();

    let events = scenario.trigger_poison_smoke_with_metal_strike();
    let grounds = events
        .iter()
        .find_map(|event| match event {
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            } if owner == &scenario.p1
                && incoming_player == &scenario.p2
                && passive_id == "jianghu:poison-smoke"
                && cards == &scenario.poison_smoke_cards =>
            {
                Some(grounds)
            }
            _ => None,
        })
        .expect("Golden Cicada must flip Poison Smoke exactly once with no effect");
    assert_ground_set(grounds, &[PassiveNoEffectGround::IgnoredByGoldenCicada]);
    assert!(!events.iter().any(|event| matches!(
        event,
        GameEvent::JianghuStateApplied { state }
            if state.owner == scenario.p2 && state.kind == JianghuStateKind::Poison
    )));
    assert_unaffected_attack_lifecycle(&events, &scenario.p2);
    assert!(
        !scenario
            .record
            .state()
            .jianghu_states
            .iter()
            .any(|state| { state.owner == scenario.p2 && state.kind == JianghuStateKind::Poison })
    );
    assert!(scenario.record.state().statuses.iter().any(|status| {
        status.owner == fewfc::domain::StatusOwner::Player(scenario.p2.clone())
            && status.kind == "PouchGoldenCicada"
    }));
    scenario.assert_passive_cards_moved_and_replayed();
}
