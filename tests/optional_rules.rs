use fewfc::application::{GameRecord, apply_event, handle_command};
use fewfc::domain::{
    CardOrigin, Command, DISCARD_RETRIEVAL_MODULE_ID, GameEvent, GameSetup, GameState,
    PERSONAL_DECK_MODULE_ID, Phase, Player, PlayerId, RuleModuleId, StatusDuration, StatusEffect,
    StatusOwner, TeamId,
};
use fewfc::public_view::{PublicCardRefs, Viewer, state_for};
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
