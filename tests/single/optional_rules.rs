use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, CardOrigin, ChoiceAnswer, Command, DISCARD_RETRIEVAL_MODULE_ID, Element,
    GameError, GameEvent, GameSetup, GameState, PERSONAL_DECK_MODULE_ID, Player, PlayerId,
    RuleModuleId, StatusOwner, TeamId, ValidationError,
};
use fewfc::public_view::{PublicGameEvent, Viewer, state_for};
use fewfc::rules::OfficialRules;

fn players() -> (Vec<Player>, Vec<PlayerId>) {
    (
        vec![
            Player {
                id: PlayerId::new("p1"),
                team: TeamId::new("team:p1"),
            },
            Player {
                id: PlayerId::new("p2"),
                team: TeamId::new("team:p2"),
            },
        ],
        vec![PlayerId::new("p1"), PlayerId::new("p2")],
    )
}

fn personal_setup() -> GameSetup {
    let (players, turn_order) = players();
    OfficialRules::new()
        .configure_game(
            players,
            turn_order,
            vec![
                RuleModuleId::new(DISCARD_RETRIEVAL_MODULE_ID),
                RuleModuleId::new(PERSONAL_DECK_MODULE_ID),
            ],
        )
        .unwrap()
}

fn shared_discard_retrieval_setup() -> GameSetup {
    let (players, turn_order) = players();
    OfficialRules::new()
        .configure_game(
            players,
            turn_order,
            vec![RuleModuleId::new(DISCARD_RETRIEVAL_MODULE_ID)],
        )
        .unwrap()
}

fn take_element_card(
    setup: &GameSetup,
    remaining: &mut Vec<CardInstanceId>,
    element: Element,
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
            definition.element == element
        })
        .unwrap();
    remaining.remove(position)
}

/// 固定牌堆順序僅是背景：它將合法的四張 Radiance 組合發給 P1，為 P2 留下五張
/// 初始手牌，並給 P1 一張已知的回合抽牌棄牌。下方測試的效果仍由真正的
/// 陣形/回合抽牌命令建立。
fn shared_deck_order_for_radiance(setup: &GameSetup) -> (Vec<CardInstanceId>, CardInstanceId) {
    let mut remaining = OfficialRules::new().official_deck_order(setup).unwrap();
    let mut deck = vec![
        take_element_card(setup, &mut remaining, Element::Metal),
        take_element_card(setup, &mut remaining, Element::Metal),
        take_element_card(setup, &mut remaining, Element::Fire),
        take_element_card(setup, &mut remaining, Element::Water),
    ];
    // P2 的正常初始發牌是此互動無關的背景。
    for _ in 0..5 {
        deck.push(remaining.remove(0));
    }
    let turn_draw_discard = remaining.remove(0);
    deck.push(turn_draw_discard);
    deck.push(remaining.remove(0));
    deck.extend(remaining);
    (deck, turn_draw_discard)
}

fn advance_after_p1_turn_draw(
    record: &mut GameRecord,
    p1: &PlayerId,
    turn_draw_discard: CardInstanceId,
) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("the legal Formation must enter P1's Turn Draw discard choice");
    record
        .handle(Command::AnswerChoice {
            player: p1.clone(),
            choice_id: choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![turn_draw_discard],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn perform_first_elemental_attack(record: &mut GameRecord, player: &PlayerId) {
    let card = record.state().hand(player).unwrap()[0];
    let formation_id = match record.state().card_element(card).unwrap() {
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

fn finish_turn_discarding(record: &mut GameRecord, player: &PlayerId, discard: CardInstanceId) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("a legal action must create its Turn Draw discard choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("turn draw must ask the acting Player to discard a Card");
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

#[test]
fn shared_deck_public_view_does_not_expose_inactive_player_piles() {
    let (players, turn_order) = players();
    let setup = OfficialRules::new()
        .configure_game(players, turn_order, Vec::new())
        .unwrap();
    let public = state_for(&GameState::from_setup(&setup), Viewer::Observer);

    assert!(public.player_decks.is_empty());
    assert!(public.player_discards.is_empty());
}

#[test]
fn personal_deck_falls_back_to_the_balanced_preconstructed_list() {
    let setup = personal_setup();

    assert_eq!(setup.deck_lists.len(), 2);
    assert!(setup.deck_lists.iter().all(|deck| deck.cards.len() == 60));
    assert_eq!(setup.card_instances.len(), 120);

    for deck in &setup.deck_lists {
        let level_total = deck
            .cards
            .iter()
            .map(|card| {
                setup
                    .card_defs
                    .iter()
                    .find(|definition| definition.id == *card)
                    .unwrap()
                    .level
                    .value()
            })
            .sum::<u32>();
        assert_eq!(level_total, 170);
    }
}

#[test]
fn personal_deck_start_matrix_prepares_owned_piles_deals_opening_hands_and_replays() {
    // 牌堆順序是固定背景。斷言的證據是標準 Start 生命週期：每張起手卡牌都來自
    // 其擁有者的牌堆，絕不來自共用牌堆，且回放會抵達目前狀態。
    let rules = OfficialRules::new();
    let setup = personal_setup();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck_order = rules.official_deck_order(&setup).unwrap();
    let owned_order = |owner: &PlayerId| {
        deck_order
            .iter()
            .copied()
            .filter(|card| {
                setup
                    .card_instances
                    .iter()
                    .find(|instance| instance.instance == *card)
                    .is_some_and(|instance| instance.origin == CardOrigin::Player(owner.clone()))
            })
            .collect::<Vec<_>>()
    };
    let p1_order = owned_order(&p1);
    let p2_order = owned_order(&p2);
    assert_eq!(p1_order.len(), 60);
    assert_eq!(p2_order.len(), 60);

    let record = GameRecord::start(setup, deck_order).unwrap();
    assert_eq!(
        record.events(),
        [
            GameEvent::PlayerDeckPrepared {
                player: p1.clone(),
                deck_order: p1_order.clone(),
            },
            GameEvent::CardsDealt {
                player: p1.clone(),
                cards: p1_order[..4].to_vec(),
            },
            GameEvent::PlayerDeckPrepared {
                player: p2.clone(),
                deck_order: p2_order.clone(),
            },
            GameEvent::CardsDealt {
                player: p2.clone(),
                cards: p2_order[..5].to_vec(),
            },
        ]
    );
    assert!(record.state().deck.is_empty());
    assert_eq!(record.state().hand(&p1), Some(p1_order[..4].as_ref()));
    assert_eq!(record.state().hand(&p2), Some(p2_order[..5].as_ref()));
    assert_eq!(record.state().deck_for(&p1), Some(p1_order[4..].as_ref()));
    assert_eq!(record.state().deck_for(&p2), Some(p2_order[5..].as_ref()));
    assert!(record.state().discard_for(&p1).unwrap().is_empty());
    assert!(record.state().discard_for(&p2).unwrap().is_empty());
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn radiance_discard_retrieval_matrix_keeps_the_active_effect_legal_under_cannot_act() {
    let setup = shared_discard_retrieval_setup();
    let (deck_order, turn_draw_discard) = shared_deck_order_for_radiance(&setup);
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：沒有 Radiance 時，可以在 P2 的作用中效果中取回緊接之前的合法回合
    // 抽牌棄牌。
    let mut baseline = GameRecord::start(setup.clone(), deck_order.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    let metal = baseline.state().hand(&p1).unwrap()[0];
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![metal],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_after_p1_turn_draw(&mut baseline, &p1, turn_draw_discard);
    assert_eq!(baseline.state().current_player(), Some(&p2));
    let baseline_events = baseline
        .handle(Command::RetrievePreviousTurnDiscard { player: p2.clone() })
        .unwrap();
    assert!(matches!(
        baseline_events.as_slice(),
        [GameEvent::DiscardRetrieved {
            player,
            previous_player,
            card,
            card_move,
            hp_change,
        }] if player == &p2
            && previous_player == &p1
            && *card == turn_draw_discard
            && card_move.from == fewfc::domain::CardZone::Discard
            && card_move.to == fewfc::domain::CardZone::DeckTop
            && hp_change.effective_delta() < 0
    ));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾與互動：只有合法的 Radiance 陣形會建立 CannotAct/CannotDraw。P2 不能
    // 執行陣形行動，但分開定義的棄牌取回仍可用且公開。
    let mut interaction = GameRecord::start(setup, deck_order).unwrap();
    interaction.advance_automatic().unwrap();
    let radiance_cards = interaction.state().hand(&p1).unwrap().to_vec();
    let radiance_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "radiance".to_string(),
            cards: radiance_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(radiance_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { formation_id, cards, .. }
            if formation_id == "radiance" && cards == &radiance_cards
    )));
    assert!(radiance_events.iter().any(|event| matches!(
        event,
        GameEvent::StatusAdded { status }
            if status.owner == StatusOwner::Player(p2.clone())
                && matches!(status.kind.as_str(), "CannotAct" | "CannotDraw")
    )));
    assert!(radiance_events.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { formation_id, cards, .. }
            if formation_id == "radiance" && cards == &radiance_cards
    )));

    advance_after_p1_turn_draw(&mut interaction, &p1, turn_draw_discard);
    assert_eq!(interaction.state().current_player(), Some(&p2));
    assert!(
        interaction
            .state()
            .statuses
            .iter()
            .any(|status| status.owner == StatusOwner::Player(p2.clone())
                && status.kind == "CannotAct")
    );
    // CannotDraw 讓 P2 的五張起手牌保持不變；它不會抑制下方的非行動取回命令。
    assert_eq!(interaction.state().hand(&p2).unwrap().len(), 5);

    let interaction_events = interaction
        .handle(Command::RetrievePreviousTurnDiscard { player: p2.clone() })
        .unwrap();
    assert!(matches!(
        interaction_events.as_slice(),
        [GameEvent::DiscardRetrieved {
            player,
            previous_player,
            card,
            card_move,
            hp_change,
        }] if player == &p2
            && previous_player == &p1
            && *card == turn_draw_discard
            && card_move.from == fewfc::domain::CardZone::Discard
            && card_move.to == fewfc::domain::CardZone::DeckTop
            && hp_change.effective_delta() < 0
    ));
    assert_eq!(interaction.state().deck.first(), Some(&turn_draw_discard));
    assert!(!interaction.state().discard.contains(&turn_draw_discard));
    assert!(matches!(
        interaction.public_events_for(Viewer::Observer).last(),
        Some(PublicGameEvent::Public(GameEvent::DiscardRetrieved { card, .. }))
            if *card == turn_draw_discard
    ));
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn discard_retrieval_matrix_uses_only_the_immediate_previous_turn_discard() {
    let setup = shared_discard_retrieval_setup();
    let (deck_order, _) = shared_deck_order_for_radiance(&setup);
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = GameRecord::start(setup, deck_order).unwrap();
    record.advance_automatic().unwrap();

    // P1 的第一個合法回合建立較舊的棄牌。P2 接著完成正常回合，因此 P1 的下一個
    // 合法回合可以建立符合資格的同隊卡牌。
    perform_first_elemental_attack(&mut record, &p1);
    record.advance_automatic().unwrap();
    let first_p1_discard = match &record.state().pending_choice {
        Some(choice) => match &choice.kind {
            fewfc::domain::PendingChoiceKind::Card { cards, .. } => cards[0],
            _ => panic!("turn draw must present a Card choice"),
        },
        None => panic!("P1's legal turn must reach Turn Draw"),
    };
    finish_turn_discarding(&mut record, &p1, first_p1_discard);
    assert_eq!(record.state().current_player(), Some(&p2));
    perform_first_elemental_attack(&mut record, &p2);
    record.advance_automatic().unwrap();
    let p2_discard = match &record.state().pending_choice {
        Some(choice) => match &choice.kind {
            fewfc::domain::PendingChoiceKind::Card { cards, .. } => cards[0],
            _ => panic!("turn draw must present a Card choice"),
        },
        None => panic!("P2's legal turn must reach Turn Draw"),
    };
    finish_turn_discarding(&mut record, &p2, p2_discard);
    assert_eq!(record.state().current_player(), Some(&p1));

    perform_first_elemental_attack(&mut record, &p1);
    record.advance_automatic().unwrap();
    let latest_p1_discard = match &record.state().pending_choice {
        Some(choice) => match &choice.kind {
            fewfc::domain::PendingChoiceKind::Card { cards, .. } => cards[0],
            _ => panic!("turn draw must present a Card choice"),
        },
        None => panic!("P1's second legal turn must reach Turn Draw"),
    };
    finish_turn_discarding(&mut record, &p1, latest_p1_discard);
    assert_eq!(record.state().current_player(), Some(&p2));
    assert!(record.state().discard.contains(&first_p1_discard));
    assert!(record.state().discard.contains(&latest_p1_discard));

    // 命令會推導目前 P1 的棄牌，而不是接受卡牌參數：只有緊接前一回合的卡牌會
    // 移回牌堆。
    let retrieve_events = record
        .handle(Command::RetrievePreviousTurnDiscard { player: p2.clone() })
        .unwrap();
    assert!(matches!(
        retrieve_events.as_slice(),
        [GameEvent::DiscardRetrieved {
            previous_player,
            card,
            card_move,
            ..
        }] if previous_player == &p1
            && *card == latest_p1_discard
            && card_move.from == fewfc::domain::CardZone::Discard
            && card_move.to == fewfc::domain::CardZone::DeckTop
    ));
    assert_eq!(record.state().deck.first(), Some(&latest_p1_discard));
    assert!(record.state().discard.contains(&first_p1_discard));
    assert!(!record.state().discard.contains(&latest_p1_discard));

    // 符合資格的卡牌移動後，較舊卡牌本身不能成為後備候選；具型別的拒絕不會
    // 記錄決策或改變狀態。
    let state_before_rejection = record.state().clone();
    let decisions_before_rejection = record.recorded_decisions();
    assert_eq!(
        record.handle(Command::RetrievePreviousTurnDiscard { player: p2 }),
        Err(GameError::Validation(
            ValidationError::NoRetrievableDiscard {
                previous_player: p1.clone(),
            }
        ))
    );
    assert_eq!(record.state(), &state_before_rejection);
    assert_eq!(record.recorded_decisions(), decisions_before_rejection);
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}
