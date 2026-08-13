use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, Command, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, FormationAreaState,
    GameEvent, HERO_SCHOOLS_MODULE_ID, HpChangeDelta, Player, PlayerId, RuleModuleId,
    SPIRIT_MODULE_ID, STAR_MODULE_ID, SpiritKind, SpiritSkill, TeamId,
};
use fewfc::rules::OfficialRules;

fn card_for(setup: &fewfc::domain::GameSetup, element: Element, level: u32) -> CardInstanceId {
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

fn finish_turn_with_legal_discard(record: &mut GameRecord, player: &PlayerId) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("a legal action must enter the canonical Turn Draw choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("turn draw must request Cards");
    };
    let discard = cards
        .iter()
        .copied()
        .find(|card| record.state().card_element(*card) != Some(Element::Metal))
        .unwrap_or_else(|| *cards.last().unwrap());
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id: choice.choice_id,
            answer: fewfc::domain::ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn elemental_strike(element: Element) -> &'static str {
    match element {
        Element::Metal => "metal-strike",
        Element::Wood => "wood-strike",
        Element::Water => "water-strike",
        Element::Fire => "fire-strike",
        Element::Earth => "earth-strike",
    }
}

#[test]
fn metal_spirit_flying_blade_matrix_summons_legally_then_spends_power_for_damage() {
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
            [
                STAR_MODULE_ID,
                FIVE_DIRECTIONS_LEGEND_MODULE_ID,
                HERO_SCHOOLS_MODULE_ID,
                SPIRIT_MODULE_ID,
            ]
            .into_iter()
            .map(RuleModuleId::new)
            .collect(),
        )
        .unwrap();
    let summoned_cards = vec![
        card_for(&setup, Element::Metal, 1),
        card_for(&setup, Element::Metal, 2),
    ];
    let opening = vec![
        summoned_cards[0],
        summoned_cards[1],
        card_for(&setup, Element::Wood, 1),
        card_for(&setup, Element::Fire, 1),
        card_for(&setup, Element::Earth, 1),
        card_for(&setup, Element::Water, 1),
        card_for(&setup, Element::Wood, 2),
        card_for(&setup, Element::Fire, 2),
        card_for(&setup, Element::Earth, 2),
    ];
    let deck = deck_starting_with(&setup, &opening);
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();

    // Baseline: the two Metal Cards establish the Spirit through the normal
    // Formation command; fixed opening Cards do not manufacture Spirit state.
    let summon = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-spirit-summoning".to_string(),
            cards: summoned_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        summon.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                state: FormationAreaState::FaceUpResolving,
                ..
            },
            GameEvent::SpiritSummoned {
                player: summoner,
                previous: None,
                spirit: SpiritKind::Metal,
            },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded,
                cards: discarded_cards,
            },
        ] if player == &p1
            && formation_id == "metal-spirit-summoning"
            && cards == &summoned_cards
            && summoner == &p1
            && discarded_by == &p1
            && discarded == "metal-spirit-summoning"
            && discarded_cards == &summoned_cards
    ));
    assert_eq!(
        record.state().spirit_for(&p1),
        Some(&fewfc::domain::PlayerSpirit {
            player: p1.clone(),
            spirit: SpiritKind::Metal,
            power: 2,
        })
    );

    // The summoning Formation used P1's action.  Advance with actual Turn
    // Draw choices and P2's ordinary Formation so the skill is available on
    // P1's next legal action, rather than resetting phase in a fixture.
    finish_turn_with_legal_discard(&mut record, &p1);
    let p2_card = record.state().hand(&p2).unwrap()[0];
    record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: elemental_strike(record.state().card_element(p2_card).unwrap())
                .to_string(),
            cards: vec![p2_card],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn_with_legal_discard(&mut record, &p2);

    // Interaction: Flying Blade is a legal, distinct skill command. It
    // consumes exactly two power, damages only the previous Team, and leaves
    // any legally acquired Turn Draw charge intact without undoing the
    // summoning card movement.
    let blade = record
        .handle(Command::UseSpiritSkill {
            player: p1.clone(),
            skill: SpiritSkill::FlyingBlade,
            selected_card: None,
            declared_level: None,
        })
        .unwrap();
    assert!(matches!(
        blade.as_slice(),
        [
            GameEvent::SpiritSkillUsed {
                player,
                spirit: SpiritKind::Metal,
                skill: SpiritSkill::FlyingBlade,
                old_power,
                new_power,
                selected_card: None,
                declared_level: None,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team,
                    old_hp: 200,
                    delta: -10,
                    new_hp: 190,
                    effective_delta: -10,
                },
            },
        ] if player == &p1 && old_power - new_power == 2 && team == &TeamId::new("team:p2")
    ));
    assert_eq!(
        record.state().spirit_for(&p1),
        Some(&fewfc::domain::PlayerSpirit {
            player: p1.clone(),
            spirit: SpiritKind::Metal,
            power: 1,
        })
    );
    assert!(record.state().discard.contains(&summoned_cards[0]));
    assert!(record.state().discard.contains(&summoned_cards[1]));
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(190)
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}
