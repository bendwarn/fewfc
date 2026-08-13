use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, ChoiceAnswer, Command, Element, GameEvent, HERO_SCHOOLS_MODULE_ID, Player,
    PlayerId, ProfessionId, RuleModuleId, TeamId,
};
use fewfc::rules::{OfficialRules, PlayableAction};

struct WarriorDefenseScenario {
    record: GameRecord,
    p1: PlayerId,
    p2: PlayerId,
    warrior_cards: Vec<CardInstanceId>,
    defense_cards: Vec<CardInstanceId>,
}

impl WarriorDefenseScenario {
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
                vec![RuleModuleId::new(HERO_SCHOOLS_MODULE_ID)],
            )
            .unwrap();
        let mut remaining = OfficialRules::new().official_deck_order(&setup).unwrap();
        let warrior_cards = vec![
            take_card(&setup, &mut remaining, Element::Metal, 1),
            take_card(&setup, &mut remaining, Element::Metal, 2),
        ];
        let defense_cards = vec![
            take_card(&setup, &mut remaining, Element::Wood, 1),
            take_card(&setup, &mut remaining, Element::Metal, 3),
        ];
        // P2's five opening Cards are unrelated fixed background; their later
        // elemental actions establish the trigger timing by legal Commands.
        let mut deck = warrior_cards
            .iter()
            .chain(defense_cards.iter())
            .copied()
            .collect::<Vec<_>>();
        for element in [
            Element::Fire,
            Element::Water,
            Element::Earth,
            Element::Metal,
            Element::Wood,
        ] {
            deck.push(take_card(&setup, &mut remaining, element, 1));
        }
        deck.extend(remaining);
        let record = GameRecord::start(setup, deck).unwrap();
        Self {
            record,
            p1,
            p2,
            warrior_cards,
            defense_cards,
        }
    }

    fn advance_to_first_action(&mut self) {
        self.record.advance_automatic().unwrap();
        assert_eq!(self.record.state().current_player(), Some(&self.p1));
    }

    fn change_to_warrior(&mut self) -> Vec<GameEvent> {
        self.record
            .handle(Command::ChangeProfession {
                player: self.p1.clone(),
                profession: ProfessionId::new("warrior"),
                cards: self.warrior_cards.clone(),
            })
            .unwrap()
    }

    fn finish_turn(&mut self, player: &PlayerId) {
        self.record.advance_automatic().unwrap();
        let choice = self
            .record
            .state()
            .pending_choice
            .as_ref()
            .expect("each legal action must reach a Turn Draw discard choice");
        let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
            panic!("Turn Draw must expose its canonical Card choice");
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

    fn perform_first_elemental_attack(&mut self, player: &PlayerId) -> Vec<GameEvent> {
        let card = self.record.state().hand(player).unwrap()[0];
        let formation_id = match self.record.state().card_element(card).unwrap() {
            Element::Metal => "metal-strike",
            Element::Wood => "wood-strike",
            Element::Water => "water-strike",
            Element::Fire => "fire-strike",
            Element::Earth => "earth-strike",
        };
        self.record
            .handle(Command::PerformFormation {
                player: player.clone(),
                formation_id: formation_id.to_string(),
                cards: vec![card],
                declared_targets: Vec::new(),
            })
            .unwrap()
    }

    fn assert_replay(&self) {
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

fn take_card(
    setup: &fewfc::domain::GameSetup,
    remaining: &mut Vec<CardInstanceId>,
    element: Element,
    level: u32,
) -> CardInstanceId {
    let position = remaining
        .iter()
        .position(|card| {
            let instance = setup
                .card_instances
                .iter()
                .find(|instance| instance.instance == *card)
                .unwrap();
            let definition = setup
                .card_defs
                .iter()
                .find(|definition| definition.id == instance.definition)
                .unwrap();
            definition.element == element && definition.level.value() == level
        })
        .unwrap();
    remaining.remove(position)
}

#[test]
fn warrior_defense_proficiency_matrix_establishes_and_consumes_defense_without_empty_city_fallback()
{
    // Baseline: the exact Wood+non-Wood selection is not basic Defense. It
    // legally resolves only as Empty City before the Warrior transition.
    let mut baseline = WarriorDefenseScenario::new();
    baseline.advance_to_first_action();
    let baseline_actions = baseline
        .record
        .playable_actions(&baseline.p1, &baseline.defense_cards)
        .unwrap();
    assert!(!baseline_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "defense"
    )));
    assert!(baseline_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "empty-city"
    )));
    let baseline_events = baseline
        .record
        .handle(Command::PerformFormation {
            player: baseline.p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: baseline.defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveCovered { formation_id, cards, .. }
            if formation_id == "empty-city" && cards == &baseline.defense_cards
    )));
    baseline.assert_replay();

    // Modifier: Warrior is reached through its own legal all-Metal profession
    // command; the later Wood+non-Wood pair now offers Defense and no longer
    // falls back to Empty City.
    let mut interaction = WarriorDefenseScenario::new();
    interaction.advance_to_first_action();
    let warrior_events = interaction.change_to_warrior();
    assert!(matches!(
        warrior_events.as_slice(),
        [
            GameEvent::ActionStarted { player: started },
            GameEvent::ProfessionChanged { player, previous: None, profession, card_moves },
        ] if started == &interaction.p1
            && player == &interaction.p1
                && profession == &ProfessionId::new("warrior")
                && card_moves.len() == interaction.warrior_cards.len()
    ));
    assert_eq!(
        interaction.record.state().profession_for(&interaction.p1),
        Some(&ProfessionId::new("warrior"))
    );
    let p1 = interaction.p1.clone();
    interaction.finish_turn(&p1);
    assert_eq!(
        interaction.record.state().current_player(),
        Some(&interaction.p2)
    );
    let p2 = interaction.p2.clone();
    interaction.perform_first_elemental_attack(&p2);
    interaction.finish_turn(&p2);
    assert_eq!(
        interaction.record.state().current_player(),
        Some(&interaction.p1)
    );

    assert!(interaction.defense_cards.iter().all(|card| {
        interaction
            .record
            .state()
            .hand(&interaction.p1)
            .unwrap()
            .contains(card)
    }));
    let warrior_actions = interaction
        .record
        .playable_actions(&interaction.p1, &interaction.defense_cards)
        .unwrap();
    assert!(warrior_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "defense"
    )));
    assert!(!warrior_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "empty-city"
    )));
    let defense_events = interaction
        .record
        .handle(Command::PerformFormation {
            player: interaction.p1.clone(),
            formation_id: "defense".to_string(),
            cards: interaction.defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense_events.as_slice(),
        [
            GameEvent::FormationCommitted { formation_id, cards, .. },
            GameEvent::PassiveCovered { player, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if formation_id == "defense"
            && cards == &interaction.defense_cards
            && player == &interaction.p1
            && covered == "defense"
            && covered_cards == &interaction.defense_cards
    ));
    interaction.finish_turn(&p1);
    assert_eq!(
        interaction.record.state().current_player(),
        Some(&interaction.p2)
    );

    // Interaction: P2's next real Attack flips and consumes the proficient
    // Defense, prevents only HP loss, and still commits/discards P2's Card.
    let hp_before = interaction
        .record
        .state()
        .hp
        .iter()
        .find(|entry| entry.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let attack_events = interaction.perform_first_elemental_attack(&p2);
    assert!(attack_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            passive_id,
            outcome: fewfc::domain::PassiveFlipOutcome::Applied { modifications, .. },
            ..
        } if owner == &interaction.p1
            && passive_id == "defense"
            && modifications == &vec![fewfc::domain::ActionModification::PreventDamage]
    )));
    assert!(attack_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. }
            if hp_change.delta == 0 && hp_change.effective_delta == 0
    )));
    assert!(
        attack_events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCommitted { .. }))
    );
    assert!(
        attack_events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCardsDiscarded { .. }))
    );
    assert_eq!(
        interaction
            .record
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        hp_before
    );
    assert!(
        interaction
            .record
            .state()
            .covered_passive(&interaction.p1)
            .is_none()
    );
    assert!(
        interaction.defense_cards.iter().all(|card| interaction
            .record
            .state()
            .discard
            .contains(card))
    );
    interaction.assert_replay();
}
