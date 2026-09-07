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

    // 基準：陣形本身會解析恢復，並透過標準待選擇狀態公開可選的印製元素費用。
    // 測試固定資料沒有插入排程或延遲效果。
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
            GameEvent::HpChanged { change },
            GameEvent::ChoiceRequested { choice, resolution },
        ] if player == &p1
            && formation_id == "echo:falling-wood"
            && cards == &vec![card(19), card(20)]
            && change.delta() == 15
            && change.effective_delta() == 0
            && choice.player == p1
            && matches!(resolution, fewfc::domain::PendingResolution::MelodyCost { .. })
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

    // 建立真實且受影響的同隊玩家：P2 的合法元素攻擊會在第三回合前降低 P1 的
    // 生命值。如此 Echo 恢復會明確改變生命值，而不是被 200 點上限遮蔽。
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
            GameEvent::AttackResolved { hp_changes, .. } => {
                hp_changes.last().map(|resolved| resolved.change.new_hp())
            }
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

    // 互動：自動回合開始會精確解析上方建立的排程，絕不提供新的 Echo 費用，
    // 完成記錄後再正常開始 P1 的回合。
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
            && change.team() == &fewfc::domain::TeamId::new("team:p1")
            && change.old_hp() == hp_before_echo
            && change.delta() == 15
            && change.new_hp() == (hp_before_echo + 15).min(200)
            && change.effective_delta() == (hp_before_echo + 15).min(200) - hp_before_echo
            && player == &p1
            && melody_id == "echo:falling-wood"
            && started == &p1
    ));
    assert!(record.state().scheduled_echoes.is_empty());
    assert!(record.state().pending_choice.is_none());
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}
