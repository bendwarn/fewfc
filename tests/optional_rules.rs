use fewfc::application::{GameRecord, apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, CardOrigin, ChoiceAnswer, Command, DISCARD_RETRIEVAL_MODULE_ID, Element,
    GameError, GameEvent, GameSetup, GameState, PERSONAL_DECK_MODULE_ID, Phase, Player, PlayerId,
    RuleModuleId, StatusDuration, StatusEffect, StatusOwner, TeamId, ValidationError,
};
use fewfc::public_view::{PublicCardRefs, PublicGameEvent, Viewer, state_for};
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

/// Fixed Deck order is background only: it deals the legal four-card Radiance
/// composition to P1, leaves five Cards for P2's initial hand, and gives P1 a
/// known Turn Draw discard.  The tested effect is still established by real
/// Formation/Turn Draw Commands below.
fn shared_deck_order_for_radiance(setup: &GameSetup) -> (Vec<CardInstanceId>, CardInstanceId) {
    let mut remaining = OfficialRules::new().official_deck_order(setup).unwrap();
    let mut deck = vec![
        take_element_card(setup, &mut remaining, Element::Metal),
        take_element_card(setup, &mut remaining, Element::Metal),
        take_element_card(setup, &mut remaining, Element::Fire),
        take_element_card(setup, &mut remaining, Element::Water),
    ];
    // P2's normal initial deal is unrelated background for this interaction.
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
fn personal_deck_start_prepares_and_deals_from_each_players_pile() {
    let rules = OfficialRules::new();
    let setup = personal_setup();
    let deck_order = rules.official_deck_order(&setup).unwrap();
    let record = GameRecord::start(setup, deck_order).unwrap();

    assert!(record.state().deck.is_empty());
    assert_eq!(
        record.state().deck_for(&PlayerId::new("p1")).unwrap().len(),
        56
    );
    assert_eq!(
        record.state().deck_for(&PlayerId::new("p2")).unwrap().len(),
        55
    );
    assert_eq!(record.state().hand(&PlayerId::new("p1")).unwrap().len(), 4);
    assert_eq!(record.state().hand(&PlayerId::new("p2")).unwrap().len(), 5);
    assert!(record.verify_replay().is_ok());
}

#[test]
fn personal_deck_start_matrix_prepares_owned_piles_deals_opening_hands_and_replays() {
    // The deck order is fixed background. The asserted evidence is the
    // canonical Start lifecycle: every opening Card comes from its owner's
    // pile, never the shared Deck, and replay reaches the live state.
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
fn discard_retrieval_is_derived_and_remains_legal_under_cannot_act() {
    let setup = personal_setup();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let card = setup
        .card_instances
        .iter()
        .find(|instance| instance.origin == CardOrigin::Player(p1.clone()))
        .unwrap()
        .instance;
    let level = setup
        .card_defs
        .iter()
        .find(|definition| {
            setup
                .card_instances
                .iter()
                .find(|instance| instance.instance == card)
                .unwrap()
                .definition
                == definition.id
        })
        .unwrap()
        .level
        .value() as i32;
    let mut state = GameState::from_setup(&setup);
    state.current_turn_index = 1;
    state.phase = Phase::ActiveEffects;
    state.discard_for_mut(&p1).unwrap().push(card);
    state.last_turn_discard_by_player.insert(
        p1.clone(),
        fewfc::domain::LastTurnDiscard {
            card,
            turn_number: 1,
        },
    );
    state.turn_number = 2;
    state.statuses.push(StatusEffect {
        id: "cannot-act".to_string(),
        owner: StatusOwner::Player(p2.clone()),
        kind: "CannotAct".to_string(),
        value: None,
        duration: StatusDuration::Permanent,
    });

    let events = handle_command(
        &state,
        Command::RetrievePreviousTurnDiscard { player: p2.clone() },
    )
    .unwrap();
    assert_eq!(events.len(), 1);
    let GameEvent::DiscardRetrieved {
        player,
        previous_player,
        card: retrieved,
        hp_change,
        ..
    } = &events[0]
    else {
        panic!("expected discard retrieval");
    };
    assert_eq!(player, &p2);
    assert_eq!(previous_player, &p1);
    assert_eq!(*retrieved, card);
    assert_eq!(hp_change.delta, -(level * 2));

    apply_event(&mut state, &events[0]);
    assert_eq!(state.deck_for(&p2).unwrap().first(), Some(&card));
    assert!(!state.discard_for(&p1).unwrap().contains(&card));
    assert!(state.exposed_foreign_cards.contains(&card));

    let public = state_for(&state, Viewer::Observer);
    let p2_deck = public
        .player_decks
        .iter()
        .find(|pile| pile.player == p2)
        .unwrap();
    assert!(matches!(
        &p2_deck.cards,
        PublicCardRefs::PartiallyKnown { cards } if cards.first() == Some(&Some(card))
    ));
}

#[test]
fn radiance_discard_retrieval_matrix_keeps_the_active_effect_legal_under_cannot_act() {
    let setup = shared_discard_retrieval_setup();
    let (deck_order, turn_draw_discard) = shared_deck_order_for_radiance(&setup);
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // Baseline: the immediately preceding legal Turn Draw discard can be
    // retrieved in P2's Active Effects without Radiance.
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
            && hp_change.effective_delta < 0
    ));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // Modifier and interaction: only the legal Radiance Formation establishes
    // CannotAct/CannotDraw.  P2 cannot take a Formation action, but the
    // separately scoped Discard Retrieval remains available and public.
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
    // CannotDraw leaves P2's opening five-card hand unchanged; it does not
    // suppress the non-Action retrieval command below.
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
            && hp_change.effective_delta < 0
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
fn discard_retrieval_does_not_use_an_older_turns_discard() {
    let setup = personal_setup();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let card = setup
        .card_instances
        .iter()
        .find(|instance| instance.origin == CardOrigin::Player(p1.clone()))
        .unwrap()
        .instance;
    let mut state = GameState::from_setup(&setup);
    state.current_turn_index = 1;
    state.turn_number = 4;
    state.phase = Phase::ActiveEffects;
    state.discard_for_mut(&p1).unwrap().push(card);
    state.last_turn_discard_by_player.insert(
        p1.clone(),
        fewfc::domain::LastTurnDiscard {
            card,
            turn_number: 1,
        },
    );

    assert!(handle_command(&state, Command::RetrievePreviousTurnDiscard { player: p2 },).is_err());
}

#[test]
fn discard_retrieval_matrix_uses_only_the_immediate_previous_turn_discard() {
    let setup = shared_discard_retrieval_setup();
    let (deck_order, _) = shared_deck_order_for_radiance(&setup);
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = GameRecord::start(setup, deck_order).unwrap();
    record.advance_automatic().unwrap();

    // P1's first legal turn creates an older discard. P2 then completes a
    // normal turn, so P1's next legal turn can establish the eligible sibling.
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

    // The command derives the current P1 discard rather than accepting a Card
    // parameter: only the immediately previous turn's Card moves to the deck.
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

    // The older card alone must not become a fallback candidate after the
    // eligible Card has moved; the typed rejection records no decision or
    // state mutation.
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
