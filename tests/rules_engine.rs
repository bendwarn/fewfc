use fewfc::application::{
    GameRecord, advance_automatic as advance_state_automatic, apply_event, handle_command,
};
use fewfc::domain::{
    CardInstanceId, Command, DeckPlacement, EventSource, GameError, GameEvent, GameSetup,
    GameStatus, PassActionReason, PendingChoice, PendingChoiceKind, Phase, Player, PlayerHand,
    PlayerId, StatusDuration, StatusEffect, StatusOwner, TeamHp, TeamId, TurnDrawSkipReason,
};

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

fn two_player_setup() -> GameSetup {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    GameSetup::two_player(
        p1.clone(),
        p2.clone(),
        30,
        vec![
            (p1, vec![card(1), card(2), card(3), card(4)]),
            (p2, vec![card(5), card(6), card(7), card(8), card(9)]),
        ],
    )
}

fn two_player_setup_with_full_first_hand() -> GameSetup {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    GameSetup::two_player(
        p1.clone(),
        p2.clone(),
        30,
        vec![
            (p1, vec![card(1), card(2), card(3), card(4), card(5)]),
            (p2, vec![card(6), card(7), card(8), card(9), card(10)]),
        ],
    )
}

fn two_player_setup_with_empty_hands() -> GameSetup {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    GameSetup::two_player(
        p1.clone(),
        p2.clone(),
        30,
        vec![(p1, Vec::new()), (p2, Vec::new())],
    )
}

fn two_player_setup_with_empty_first_hand() -> GameSetup {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    GameSetup::two_player(
        p1.clone(),
        p2.clone(),
        30,
        vec![
            (p1, Vec::new()),
            (p2, vec![card(1), card(2), card(3), card(4), card(5)]),
        ],
    )
}

#[test]
fn new_game_persists_deck_order_and_replay_matches_current_state() {
    let deck = vec![card(10), card(11), card(12)];
    let record = GameRecord::start(two_player_setup(), deck.clone()).unwrap();

    assert_eq!(
        record.events().first(),
        Some(&GameEvent::DeckPrepared {
            deck_order: deck.clone()
        })
    );

    let state = record.state().unwrap();
    assert_eq!(state.phase, Phase::TurnStart);
    assert_eq!(state.current_player(), Some(&PlayerId::new("p1")));
    assert_eq!(state.deck, deck);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn game_record_exposes_recorded_events_with_sequence_metadata() {
    let deck = vec![card(10), card(11)];
    let record = GameRecord::start(two_player_setup(), deck.clone()).unwrap();

    let recorded_events = record.recorded_events();

    assert_eq!(recorded_events.len(), 1);
    assert_eq!(recorded_events[0].metadata.sequence, 1);
    assert_eq!(recorded_events[0].metadata.source, EventSource::System);
    assert_eq!(
        recorded_events[0].event,
        GameEvent::DeckPrepared { deck_order: deck }
    );
}

#[test]
fn apply_event_projects_canonical_events_without_returning_validation_errors() {
    let setup = two_player_setup();
    let mut state = fewfc::domain::GameState::from_setup(&setup);

    let projected: () = apply_event(
        &mut state,
        &GameEvent::DeckPrepared {
            deck_order: vec![card(10), card(11)],
        },
    );

    assert_eq!(projected, ());
    assert_eq!(state.deck, vec![card(10), card(11)]);
}

#[test]
fn setup_validation_rejects_duplicate_card_instances_across_hands_and_deck() {
    assert_eq!(
        GameRecord::start(two_player_setup(), vec![card(4)]),
        Err(GameError::DuplicateCard(card(4)))
    );
}

#[test]
fn setup_validation_requires_hp_for_every_team() {
    let setup = GameSetup {
        players: vec![Player {
            id: PlayerId::new("p1"),
            team: TeamId::new("A"),
        }],
        turn_order: vec![PlayerId::new("p1")],
        hp: Vec::new(),
        starting_hands: vec![PlayerHand::new(PlayerId::new("p1"), vec![card(1)])],
        hand_limit: 5,
        base_draw: 2,
    };

    assert_eq!(
        GameRecord::start(setup, vec![card(2)]),
        Err(GameError::MissingTeamHp(TeamId::new("A")))
    );
}

#[test]
fn setup_validation_rejects_team_mode_turn_order_that_is_not_alternating() {
    let setup = GameSetup {
        players: vec![
            Player {
                id: PlayerId::new("p1"),
                team: TeamId::new("A"),
            },
            Player {
                id: PlayerId::new("p2"),
                team: TeamId::new("A"),
            },
            Player {
                id: PlayerId::new("p3"),
                team: TeamId::new("B"),
            },
            Player {
                id: PlayerId::new("p4"),
                team: TeamId::new("B"),
            },
        ],
        turn_order: vec![
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            PlayerId::new("p3"),
            PlayerId::new("p4"),
        ],
        hp: vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 30,
            },
        ],
        starting_hands: vec![
            PlayerHand::new(PlayerId::new("p1"), vec![card(1)]),
            PlayerHand::new(PlayerId::new("p2"), vec![card(2)]),
            PlayerHand::new(PlayerId::new("p3"), vec![card(3)]),
            PlayerHand::new(PlayerId::new("p4"), vec![card(4)]),
        ],
        hand_limit: 5,
        base_draw: 2,
    };

    assert_eq!(
        GameRecord::start(setup, vec![card(5)]),
        Err(GameError::TeamSeatingNotAlternating {
            previous_player: PlayerId::new("p1"),
            player: PlayerId::new("p2"),
            team: TeamId::new("A"),
        })
    );
}

#[test]
fn new_game_state_exposes_core_status_shields_and_passive_zones() {
    let record = GameRecord::start(two_player_setup(), vec![card(10), card(11)]).unwrap();
    let state = record.state().unwrap();

    assert_eq!(state.status, GameStatus::InProgress);
    assert_eq!(state.shield(&PlayerId::new("p1")), Some(0));
    assert_eq!(state.shield(&PlayerId::new("p2")), Some(0));
    assert!(state.covered_passives.is_empty());
    assert!(state.statuses.is_empty());
}

#[test]
fn pending_choices_can_store_effect_generated_continuations() {
    let choice = PendingChoice {
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::EffectGenerated {
            effect_id: "choose-card-to-seal".to_string(),
            continuation_id: "seal-resolution".to_string(),
            allowed_cards: vec![card(1), card(2)],
        },
    };

    assert_eq!(choice.clone(), choice);
}

#[test]
fn invalid_command_returns_error_without_appending_events_or_changing_state() {
    let mut record = GameRecord::start(two_player_setup(), vec![card(10), card(11)]).unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().unwrap();

    let result = record.handle(Command::ChooseTurnDiscard {
        player: PlayerId::new("p1"),
        discard: card(1),
    });

    assert_eq!(
        result,
        Err(GameError::WrongPhase {
            expected: Phase::TurnDrawDiscardChoice,
            actual: Phase::TurnStart,
        })
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().unwrap(), state_before);
}

#[test]
fn automatic_advance_stops_at_main_after_turn_start() {
    let mut record = GameRecord::start(two_player_setup(), vec![card(10), card(11)]).unwrap();

    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![GameEvent::TurnStarted {
            player: PlayerId::new("p1"),
            turn_number: 1,
        }]
    );

    let state = record.state().unwrap();
    assert_eq!(state.phase, Phase::Main);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn pass_action_with_no_cards_consumes_the_turn_action_and_enters_turn_draw() {
    let mut record = GameRecord::start(
        two_player_setup_with_empty_first_hand(),
        vec![card(10), card(11), card(12)],
    )
    .unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::NoCardsInHand,
            })
            .unwrap(),
        vec![GameEvent::ActionPassed {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        }]
    );

    let state = record.state().unwrap();
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn pass_action_with_cards_is_rejected_without_changing_state() {
    let mut record = GameRecord::start(two_player_setup(), vec![card(10), card(11)]).unwrap();
    record.advance_automatic().unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().unwrap();

    let result = record.handle(Command::PassAction {
        player: PlayerId::new("p1"),
        reason: PassActionReason::NoCardsInHand,
    });

    assert_eq!(
        result,
        Err(GameError::CannotPassAction {
            reason: PassActionReason::NoCardsInHand,
        })
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().unwrap(), state_before);
}

#[test]
fn pass_action_is_allowed_when_player_cannot_act_by_status() {
    let mut record = GameRecord::start(two_player_setup(), vec![card(10), card(11)]).unwrap();
    record.advance_automatic().unwrap();
    let mut state = record.state().unwrap();
    state.statuses.push(StatusEffect {
        id: "cannot-act-p1".to_string(),
        owner: StatusOwner::Player(PlayerId::new("p1")),
        kind: "CannotAct".to_string(),
        value: None,
        duration: StatusDuration::UntilNextAction,
    });

    assert_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::CannotActByStatus,
            },
        )
        .unwrap(),
        vec![GameEvent::ActionPassed {
            player: PlayerId::new("p1"),
            reason: PassActionReason::CannotActByStatus,
        }]
    );
}

#[test]
fn turn_draw_creates_pending_discard_choice_after_drawing_available_space_plus_one() {
    let mut record = GameRecord::start(
        two_player_setup_with_empty_first_hand(),
        vec![card(10), card(11), card(12), card(13)],
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        })
        .unwrap();

    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![GameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("p1"),
            drawn_cards: vec![card(10), card(11), card(12)],
            allowed_discards: vec![card(10), card(11), card(12)],
        }]
    );

    let state = record.state().unwrap();
    assert_eq!(state.phase, Phase::TurnDrawDiscardChoice);
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(10), card(11), card(12)].as_slice())
    );
    assert_eq!(state.deck, vec![card(13)]);
    assert!(state.pending_choice.is_some());
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn choosing_turn_discard_finishes_draw_choice_and_moves_card_to_discard() {
    let mut record = GameRecord::start(
        two_player_setup_with_empty_first_hand(),
        vec![card(10), card(11), card(12), card(13)],
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        })
        .unwrap();
    record.advance_automatic().unwrap();

    assert_eq!(
        record
            .handle(Command::ChooseTurnDiscard {
                player: PlayerId::new("p1"),
                discard: card(10),
            })
            .unwrap(),
        vec![GameEvent::TurnDiscardChosen {
            player: PlayerId::new("p1"),
            discard: card(10),
        }]
    );

    let state = record.state().unwrap();
    assert_eq!(state.phase, Phase::TurnEnd);
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(11), card(12)].as_slice())
    );
    assert_eq!(state.discard, vec![card(10)]);
    assert!(state.pending_choice.is_none());
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn turn_end_automatic_advance_starts_next_players_turn() {
    let mut record = GameRecord::start(
        two_player_setup_with_empty_first_hand(),
        vec![card(10), card(11), card(12), card(13)],
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        })
        .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::ChooseTurnDiscard {
            player: PlayerId::new("p1"),
            discard: card(10),
        })
        .unwrap();

    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![
            GameEvent::TurnEnded {
                player: PlayerId::new("p1"),
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p2"),
                turn_number: 2,
            },
        ]
    );

    let state = record.state().unwrap();
    assert_eq!(state.phase, Phase::Main);
    assert_eq!(state.current_player(), Some(&PlayerId::new("p2")));
    assert_eq!(state.turn_number, 2);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn turn_draw_is_skipped_when_hand_is_already_at_limit() {
    let mut record =
        GameRecord::start(two_player_setup_with_full_first_hand(), vec![card(11)]).unwrap();
    record.advance_automatic().unwrap();
    let mut state = record.state().unwrap();
    state.statuses.push(StatusEffect {
        id: "cannot-act-p1".to_string(),
        owner: StatusOwner::Player(PlayerId::new("p1")),
        kind: "CannotAct".to_string(),
        value: None,
        duration: StatusDuration::UntilNextAction,
    });
    for event in handle_command(
        &state,
        Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::CannotActByStatus,
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }

    assert_eq!(
        advance_state_automatic(&state).unwrap(),
        vec![
            GameEvent::TurnDrawSkipped {
                player: PlayerId::new("p1"),
                reason: TurnDrawSkipReason::HandLimitReached,
            },
            GameEvent::TurnEnded {
                player: PlayerId::new("p1"),
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p2"),
                turn_number: 2,
            },
        ]
    );

    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.phase, Phase::Main);
    assert_eq!(state.current_player(), Some(&PlayerId::new("p2")));
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(1), card(2), card(3), card(4), card(5)].as_slice())
    );
    assert_eq!(state.deck, vec![card(11)]);
    assert!(state.pending_choice.is_none());
}

#[test]
fn turn_draw_recycles_discard_to_deck_bottom_when_deck_is_insufficient() {
    let mut record = GameRecord::start(
        two_player_setup_with_empty_hands(),
        vec![card(10), card(11), card(12), card(13), card(14)],
    )
    .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        })
        .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::ChooseTurnDiscard {
            player: PlayerId::new("p1"),
            discard: card(10),
        })
        .unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PassAction {
            player: PlayerId::new("p2"),
            reason: PassActionReason::NoCardsInHand,
        })
        .unwrap();

    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![
            GameEvent::DiscardRecycledIntoDeck {
                shuffled_order: vec![card(10)],
                placement: DeckPlacement::Bottom,
            },
            GameEvent::CardsDrawnForTurnDiscardChoice {
                player: PlayerId::new("p2"),
                drawn_cards: vec![card(13), card(14), card(10)],
                allowed_discards: vec![card(13), card(14), card(10)],
            },
        ]
    );

    let state = record.state().unwrap();
    assert_eq!(state.phase, Phase::TurnDrawDiscardChoice);
    assert_eq!(state.deck, Vec::<CardInstanceId>::new());
    assert_eq!(state.discard, Vec::<CardInstanceId>::new());
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(13), card(14), card(10)].as_slice())
    );
    assert_eq!(record.replay().unwrap(), state);
}
