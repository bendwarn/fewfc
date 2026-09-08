use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, ChoiceAnswer, Command, DARK_GLIMMER_MODULE_ID, Element,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent, GameSetup, HERO_SCHOOLS_MODULE_ID,
    JIANGHU_MODULE_ID, JianghuStateKind, PERSONAL_DECK_MODULE_ID, POUCH_MODULE_ID,
    PassiveFlipOutcome, PassiveNoEffectGround, PlayerId, RuleModuleId, SPIRIT_MODULE_ID,
    STAR_MODULE_ID, SecretStrategy, SpiritKind,
};
use fewfc::rules::OfficialRules;
mod support;
use support::ScenarioPlan;

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
        let setup = ScenarioPlan::two_player(&POISON_SMOKE_MODULES)
            .setup()
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

fn finish_turn_preserving(
    record: &mut GameRecord,
    player: &PlayerId,
    preserved: &[CardInstanceId],
) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("a legal action advances to the turn-draw discard choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("turn draw must request a card discard");
    };
    let discard = cards
        .iter()
        .rev()
        .copied()
        .find(|card| !preserved.contains(card))
        .expect("the legal discard must not consume a card reserved for a later public command");
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
        GameEvent::AttackResolved { hp_changes, .. }
            if hp_changes.iter().any(|resolved| resolved.change.effective_delta() < 0)
    )));
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, .. }
            if player == p2 && formation_id == "metal-strike"
    )));
}

fn card_from_setup(
    setup: &fewfc::domain::GameSetup,
    element: Element,
    level: u32,
) -> CardInstanceId {
    setup
        .card_instances
        .iter()
        .find_map(|instance| {
            setup
                .card_defs
                .iter()
                .find(|definition| definition.id == instance.definition)
                .filter(|definition| {
                    definition.element == element && definition.level.value() == level
                })
                .map(|_| instance.instance)
        })
        .expect("official Rules have each requested Card")
}

fn deck_starting_with(
    setup: &fewfc::domain::GameSetup,
    first: &[CardInstanceId],
) -> Vec<CardInstanceId> {
    let mut deck = first.to_vec();
    deck.extend(
        OfficialRules::new()
            .official_deck_order(setup)
            .unwrap()
            .into_iter()
            .filter(|card| !first.contains(card)),
    );
    deck
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

#[test]
fn thousand_poison_hand_immediately_triggers_shared_fate_and_applies_poison() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = OfficialRules::new()
        .configure_game(
            GameSetup::two_player(p1.clone(), p2.clone(), 200).players,
            vec![p1.clone(), p2.clone()],
            [
                STAR_MODULE_ID,
                FIVE_DIRECTIONS_LEGEND_MODULE_ID,
                HERO_SCHOOLS_MODULE_ID,
                SPIRIT_MODULE_ID,
                DARK_GLIMMER_MODULE_ID,
                JIANGHU_MODULE_ID,
            ]
            .into_iter()
            .map(RuleModuleId::new)
            .collect(),
        )
        .unwrap();
    // 前置的合法回合需要保留轉職與千毒手的卡牌，避免以測試資料捏造手牌。
    setup.base_draw = 5;
    setup.hand_limit = 10;

    let first_wanderer = card_from_setup(&setup, Element::Metal, 1);
    let poisoner_water = card_from_setup(&setup, Element::Water, 1);
    let poisoner_earth = card_from_setup(&setup, Element::Earth, 1);
    let thousand_poison_cards = vec![
        card_from_setup(&setup, Element::Metal, 3),
        card_from_setup(&setup, Element::Wood, 3),
        card_from_setup(&setup, Element::Fire, 3),
    ];
    let fire_spirit_cards = vec![
        card_from_setup(&setup, Element::Fire, 1),
        card_from_setup(&setup, Element::Fire, 2),
    ];
    let demon_spirit_master_cards = vec![
        card_from_setup(&setup, Element::Fire, 4),
        card_from_setup(&setup, Element::Wood, 4),
    ];
    let death_spirit_cards = vec![
        card_from_setup(&setup, Element::Earth, 4),
        card_from_setup(&setup, Element::Metal, 4),
    ];
    let opening = vec![
        first_wanderer,
        poisoner_water,
        poisoner_earth,
        card_from_setup(&setup, Element::Wood, 2),
        fire_spirit_cards[0],
        fire_spirit_cards[1],
        demon_spirit_master_cards[0],
        demon_spirit_master_cards[1],
        card_from_setup(&setup, Element::Water, 2),
        // P1 首次回合抽六棄一；最後一張是合法棄牌。
        thousand_poison_cards[0],
        thousand_poison_cards[1],
        thousand_poison_cards[2],
        card_from_setup(&setup, Element::Wood, 5),
        card_from_setup(&setup, Element::Metal, 5),
        card_from_setup(&setup, Element::Earth, 5),
        // P2 先召喚火精靈，再合法轉為魔靈師，最後才以公開命令召喚死精靈。
        death_spirit_cards[0],
        death_spirit_cards[1],
    ];
    let mut record =
        GameRecord::start(setup.clone(), deck_starting_with(&setup, &opening)).unwrap();
    record.advance_automatic().unwrap();

    change_profession(&mut record, &p1, "first-wanderer", vec![first_wanderer]);
    finish_turn(&mut record, &p1);

    let fire_summoned = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-spirit-summoning".to_string(),
            cards: fire_spirit_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        fire_summoned.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::SpiritSummoned { player: summoner, previous: None, spirit: SpiritKind::Fire },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-spirit-summoning"
            && cards == &fire_spirit_cards
            && summoner == &p2
            && discarded_by == &p2
            && discarded == "fire-spirit-summoning"
            && discarded_cards == &fire_spirit_cards
    ));
    finish_turn_preserving(&mut record, &p2, &death_spirit_cards);

    change_profession(
        &mut record,
        &p1,
        "jianghu:poisoner",
        vec![poisoner_water, poisoner_earth],
    );
    finish_turn(&mut record, &p1);

    change_profession(
        &mut record,
        &p2,
        "dark:demon-spirit-master",
        demon_spirit_master_cards,
    );
    finish_turn_preserving(&mut record, &p2, &death_spirit_cards);

    let defense_cards = vec![
        card_from_setup(&setup, Element::Wood, 2),
        card_from_setup(&setup, Element::Wood, 5),
    ];
    record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut record, &p1);

    assert!(
        death_spirit_cards
            .iter()
            .all(|card| record.state().hand(&p2).unwrap().contains(card)),
        "P2 death Cards must survive the legal setup: hand={:?}, death={death_spirit_cards:?}",
        record.state().hand(&p2).unwrap(),
    );
    let summoned = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "dark:death-spirit-summoning".to_string(),
            cards: death_spirit_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        summoned
            .iter()
            .find(|event| matches!(event, GameEvent::SpiritPowerChanged { .. })),
        Some(GameEvent::SpiritPowerChanged {
            player: owner,
            spirit: SpiritKind::Death,
            old_power: 2,
            delta: 2,
            new_power: 4,
            ..
        }) if owner == &p2
    ));
    finish_turn(&mut record, &p2);
    assert_eq!(
        record.state().spirit_for(&p2),
        Some(&fewfc::domain::PlayerSpirit {
            player: p2.clone(),
            spirit: SpiritKind::Death,
            power: 4,
        })
    );

    let events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "jianghu:thousand-poison-hand".to_string(),
            cards: thousand_poison_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::HpChanged { change: primary },
            GameEvent::JianghuStateApplied { state },
            GameEvent::HpChanged { change: shared_fate },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "jianghu:thousand-poison-hand"
            && cards == &thousand_poison_cards
            && primary.team().as_str() == "team:p2"
            && primary.old_hp() == 200
            && primary.delta() == -10
            && primary.new_hp() == 190
            && primary.effective_delta() == -10
            && state.owner == p2
            && state.kind == JianghuStateKind::Poison
            && state.remaining_turns == 2
            && shared_fate.team().as_str() == "team:p1"
            && shared_fate.old_hp() == 200
            && shared_fate.delta() == -10
            && shared_fate.new_hp() == 190
            && shared_fate.effective_delta() == -10
            && discarded_by == &p1
            && discarded == "jianghu:thousand-poison-hand"
            && discarded_cards == &thousand_poison_cards
    ));
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}
