use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, CardMoveDelta, CardZone, ChoiceAnswer, Command, ECHO_MODULE_ID, Element,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID, FormationAreaState, GameEvent, HERO_SCHOOLS_MODULE_ID,
    PlayerId, RuleModuleId, STAR_MODULE_ID, ScheduledEcho,
};
use fewfc::rules::OfficialRules;

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

fn echo_setup() -> fewfc::domain::GameSetup {
    let mut modules = [
        STAR_MODULE_ID,
        FIVE_DIRECTIONS_LEGEND_MODULE_ID,
        HERO_SCHOOLS_MODULE_ID,
    ]
    .into_iter()
    .map(RuleModuleId::new)
    .collect::<Vec<_>>();
    modules.push(RuleModuleId::new(ECHO_MODULE_ID));
    OfficialRules::new()
        .configure_game(
            fewfc::domain::GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30)
                .players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            modules,
        )
        .unwrap()
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

fn finish_turn_draw(record: &mut GameRecord, player: &PlayerId, discard: CardInstanceId) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("the completed action must reach a canonical Turn Draw choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("Turn Draw must request a Card discard");
    };
    assert!(cards.contains(&discard));
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
fn falling_wood_echo_matrix_pays_cost_schedules_then_resolves_at_its_next_legal_turn_start() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let setup = echo_setup();
    let leading = (19..=21).chain(1..=18).map(card).collect::<Vec<_>>();
    let mut record =
        GameRecord::start(setup.clone(), deck_starting_with(&setup, &leading)).unwrap();
    record.advance_automatic().unwrap();

    // Baseline: the Formation itself resolves its recovery and exposes an
    // optional printed-element cost through the canonical pending choice. No
    // schedule or delayed effect is inserted by a fixture.
    let falling_wood = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "echo:falling-wood".to_string(),
            cards: vec![card(19), card(20)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        falling_wood.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::ChoiceRequested { choice },
        ] if player == &p1
            && formation_id == "echo:falling-wood"
            && cards == &vec![card(19), card(20)]
            && choice.player == p1
            && matches!(choice.continuation, fewfc::domain::ChoiceContinuation::Echo(fewfc::domain::EchoChoiceContinuation::Cost { .. }))
    ));
    let cost_choice_id = record.state().pending_choice.as_ref().unwrap().choice_id;
    let paid = record
        .handle(Command::AnswerChoice {
            player: p1.clone(),
            choice_id: cost_choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![card(21)],
            },
        })
        .unwrap();
    assert_eq!(
        paid,
        vec![
            GameEvent::ChoiceMade {
                player: p1.clone(),
                choice_id: cost_choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![card(21)],
                },
            },
            GameEvent::EchoCostPaid {
                player: p1.clone(),
                melody_id: "echo:falling-wood".to_string(),
                card_move: CardMoveDelta {
                    card: card(21),
                    from: CardZone::Hand(p1.clone()),
                    to: CardZone::Discard,
                },
            },
            GameEvent::EchoScheduled {
                schedule: ScheduledEcho {
                    player: p1.clone(),
                    melody_id: "echo:falling-wood".to_string(),
                    due_turn_number: 3,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "echo:falling-wood".to_string(),
                cards: vec![card(19), card(20)],
            },
        ]
    );
    assert_eq!(
        record.state().scheduled_echoes,
        vec![ScheduledEcho {
            player: p1.clone(),
            melody_id: "echo:falling-wood".to_string(),
            due_turn_number: 3,
        }]
    );

    // Establish a real, affected sibling: P2's legal elemental Attack lowers
    // P1 before turn three. The Echo recovery then visibly changes HP rather
    // than being hidden by the 200-point cap.
    finish_turn_draw(&mut record, &p1, card(7));
    let p2_attack_card = record.state().hand(&p2).unwrap()[0];
    let p2_attack = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: elemental_strike(record.state().card_element(p2_attack_card).unwrap())
                .to_string(),
            cards: vec![p2_attack_card],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let hp_before_echo = p2_attack
        .iter()
        .find_map(|event| match event {
            GameEvent::AttackResolved { hp_change, .. } => Some(hp_change.new_hp),
            _ => None,
        })
        .expect("P2's legal elemental Formation must resolve an attack");
    assert!(hp_before_echo < 200);
    record.advance_automatic().unwrap();
    let p2_draw_choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("P2's action must enter its canonical Turn Draw choice");
    record
        .handle(Command::AnswerChoice {
            player: p2.clone(),
            choice_id: p2_draw_choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![card(10)],
            },
        })
        .unwrap();

    // Interaction: automatic Turn Start resolves exactly the schedule created
    // above, never offers a fresh Echo cost, completes the record, then starts
    // P1's Turn normally.
    let echoed = record.advance_automatic().unwrap();
    assert!(matches!(
        echoed.as_slice(),
        [
            GameEvent::TurnEnded { player: ended },
            GameEvent::EchoResolutionStarted { schedule },
            GameEvent::HpChanged { change },
            GameEvent::EchoResolutionCompleted { player, melody_id, due_turn_number: 3 },
            GameEvent::TurnStarted { player: started, turn_number: 3 },
        ] if ended == &p2
            && schedule == &ScheduledEcho {
                player: p1.clone(),
                melody_id: "echo:falling-wood".to_string(),
                due_turn_number: 3,
            }
            && change.team == fewfc::domain::TeamId::new("team:p1")
            && change.old_hp == hp_before_echo
            && change.delta == 15
            && change.new_hp == (hp_before_echo + 15).min(200)
            && change.effective_delta == (hp_before_echo + 15).min(200) - hp_before_echo
            && player == &p1
            && melody_id == "echo:falling-wood"
            && started == &p1
    ));
    assert!(record.state().scheduled_echoes.is_empty());
    assert!(record.state().pending_choice.is_none());
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}
