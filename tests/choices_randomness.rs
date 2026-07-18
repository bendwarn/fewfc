use fewfc::application::{
    advance_automatic, apply_event, handle_command, replay, resolve_trusted_randomness,
};
use fewfc::domain::{
    CardInstanceId, ChoiceAnswer, ChoiceId, Command, GameError, GameEvent, GameSetup, GameState,
    PassActionReason, PendingChoiceKind, PendingRandomness, PlayerId, PouchRandomnessContinuation,
    RandomnessContinuation, RandomnessDeck, TrustedRandomnessAnswer, ValidationError,
};
use fewfc::public_view::{PublicGameEvent, Viewer, event_for, state_for};

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

fn setup() -> GameSetup {
    GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30)
}

fn state_with_pending_randomness() -> GameState {
    let mut state = GameState::from_setup(&setup());
    state.deck = vec![card(1), card(2), card(3)];
    apply_event(
        &mut state,
        &GameEvent::RandomnessRequested {
            request: PendingRandomness {
                request_id: "shuffle-1".to_string(),
                operation: fewfc::domain::RandomnessOperation::DeckShuffle {
                    deck: RandomnessDeck::Shared,
                },
                continuation: RandomnessContinuation::Pouch(
                    PouchRandomnessContinuation::SheepStealing {
                        source_card: card(1),
                        owner: None,
                    },
                ),
                current_order: vec![card(1), card(2), card(3)],
            },
        },
    );
    state
}

#[test]
fn pending_randomness_blocks_commands_and_automatic_advancement() {
    let state = state_with_pending_randomness();

    assert_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::NoCardsInHand,
            },
        ),
        Err(GameError::Validation(
            ValidationError::PendingRandomnessInProgress {
                request_id: "shuffle-1".to_string(),
            }
        ))
    );
    assert_eq!(advance_automatic(&state), Ok(Vec::new()));
}

#[test]
fn trusted_randomness_requires_the_exact_current_permutation() {
    let state = state_with_pending_randomness();

    for shuffled_order in [
        vec![card(1), card(2)],
        vec![card(1), card(1), card(3)],
        vec![card(1), card(2), card(99)],
    ] {
        assert_eq!(
            resolve_trusted_randomness(
                &state,
                &TrustedRandomnessAnswer {
                    request_id: "shuffle-1".to_string(),
                    shuffled_order,
                },
            ),
            Err(GameError::Validation(
                ValidationError::InvalidRandomnessPermutation
            ))
        );
        assert!(state.pending_randomness.is_some());
    }

    let mut stale = state.clone();
    stale.deck.swap(0, 1);
    assert_eq!(
        resolve_trusted_randomness(
            &stale,
            &TrustedRandomnessAnswer {
                request_id: "shuffle-1".to_string(),
                shuffled_order: vec![card(3), card(2), card(1)],
            },
        ),
        Err(GameError::Validation(
            ValidationError::StalePendingRandomness
        ))
    );
}

#[test]
fn accepted_shuffle_is_canonical_and_replay_uses_the_recorded_order() {
    let mut state = state_with_pending_randomness();
    let events = resolve_trusted_randomness(
        &state,
        &TrustedRandomnessAnswer {
            request_id: "shuffle-1".to_string(),
            shuffled_order: vec![card(3), card(1), card(2)],
        },
    )
    .unwrap();
    apply_event(&mut state, &events[0]);
    assert_eq!(state.deck, vec![card(3), card(1), card(2)]);
    assert!(state.pending_randomness.is_none());

    let replayed = replay(
        &setup(),
        &[
            GameEvent::RandomnessRequested {
                request: PendingRandomness {
                    request_id: "shuffle-1".to_string(),
                    operation: fewfc::domain::RandomnessOperation::DeckShuffle {
                        deck: RandomnessDeck::Shared,
                    },
                    continuation: RandomnessContinuation::Pouch(
                        PouchRandomnessContinuation::SheepStealing {
                            source_card: card(1),
                            owner: None,
                        },
                    ),
                    current_order: vec![card(1), card(2), card(3)],
                },
            },
            events[0].clone(),
        ],
    )
    .unwrap();
    assert_eq!(replayed.deck, vec![card(3), card(1), card(2)]);
}

#[test]
fn public_randomness_views_never_expose_the_order() {
    let state = state_with_pending_randomness();
    let public = state_for(&state, Viewer::Observer);
    let state_json = serde_json::to_string(&public).unwrap();
    assert!(!state_json.contains("current_order"));
    assert!(!state_json.contains("[1,2,3]"));
    assert!(state_json.contains("\"requestId\""));
    assert!(state_json.contains("\"cardCount\""));
    assert!(state_json.contains("\"operation\":\"deckShuffle\""));

    let event = GameEvent::RandomnessRequested {
        request: state.pending_randomness.unwrap(),
    };
    let public_event = event_for(&event, Viewer::Observer);
    assert!(matches!(
        public_event,
        PublicGameEvent::RandomnessRequested { card_count: 3, .. }
    ));
    assert!(
        !serde_json::to_string(&public_event)
            .unwrap()
            .contains("[1,2,3]")
    );
}

#[test]
fn new_choice_and_randomness_fields_serialize_as_camel_case() {
    assert_eq!(
        serde_json::to_value(ChoiceAnswer::Formation {
            formation_id: "formation".to_string(),
        })
        .unwrap(),
        serde_json::json!({"type": "formation", "formationId": "formation"})
    );
    assert_eq!(
        serde_json::to_value(ChoiceAnswer::SheepStealing {
            deck_cards: vec![card(2), card(3)],
            discard_cards: vec![card(4), card(5)],
        })
        .unwrap(),
        serde_json::json!({
            "type": "sheepStealing",
            "deckCards": [2, 3],
            "discardCards": [4, 5]
        })
    );
    assert_eq!(
        serde_json::to_value(ChoiceAnswer::Chain {
            pouch_owner: PlayerId::new("p2"),
            pouch_card: card(2),
            trigger_card: Some(card(3)),
            strategy: Some(fewfc::domain::SecretStrategy::DeceiveHeaven),
            target_player: Some(PlayerId::new("p1")),
            star: Some(fewfc::domain::StarKind::Fire),
            break_star: true,
            discard_card: Some(card(4)),
        })
        .unwrap(),
        serde_json::json!({
            "type": "chain",
            "pouchOwner": "p2",
            "pouchCard": 2,
            "triggerCard": 3,
            "strategy": "DeceiveHeaven",
            "targetPlayer": "p1",
            "star": "Fire",
            "breakStar": true,
            "discardCard": 4
        })
    );
    assert_eq!(
        serde_json::to_value(PendingChoiceKind::SheepStealing {
            source_card: card(9),
            owner: Some(PlayerId::new("p1")),
            deck_cards: vec![card(2), card(3)],
            discard_cards: vec![card(4), card(5)],
        })
        .unwrap(),
        serde_json::json!({
            "type": "sheepStealing",
            "sourceCard": 9,
            "owner": "p1",
            "deckCards": [2, 3],
            "discardCards": [4, 5]
        })
    );
    assert_eq!(
        serde_json::to_value(Command::AnswerChoice {
            player: PlayerId::new("p1"),
            choice_id: ChoiceId::new(7),
            answer: ChoiceAnswer::Chain {
                pouch_owner: PlayerId::new("p2"),
                pouch_card: card(2),
                trigger_card: Some(card(3)),
                strategy: None,
                target_player: Some(PlayerId::new("p1")),
                star: None,
                break_star: true,
                discard_card: Some(card(4)),
            },
        })
        .unwrap(),
        serde_json::json!({
            "answerChoice": {
                "player": "p1",
                "choiceId": 7,
                "answer": {
                    "type": "chain",
                    "pouchOwner": "p2",
                    "pouchCard": 2,
                    "triggerCard": 3,
                    "targetPlayer": "p1",
                    "breakStar": true,
                    "discardCard": 4
                }
            }
        })
    );
    assert_eq!(
        serde_json::to_value(TrustedRandomnessAnswer {
            request_id: "request".to_string(),
            shuffled_order: vec![card(2), card(1)],
        })
        .unwrap(),
        serde_json::json!({"requestId": "request", "shuffledOrder": [2, 1]})
    );
    assert_eq!(
        serde_json::to_value(PendingRandomness {
            request_id: "request".to_string(),
            operation: fewfc::domain::RandomnessOperation::DeckShuffle {
                deck: RandomnessDeck::Shared,
            },
            continuation: RandomnessContinuation::Echo(
                fewfc::domain::EchoRandomnessContinuation::RingingMetalPostSearch,
            ),
            current_order: vec![card(1), card(2)],
        })
        .unwrap(),
        serde_json::json!({
            "requestId": "request",
            "operation": {
                "type": "deckShuffle",
                "deck": "Shared"
            },
            "continuation": {
                "type": "echo",
                "kind": "ringingMetalPostSearch"
            },
            "currentOrder": [1, 2],
        })
    );
    assert_eq!(
        serde_json::to_value(PouchRandomnessContinuation::ChainRecycle).unwrap(),
        serde_json::json!("chainRecycle")
    );
    assert_eq!(
        serde_json::to_value(PendingRandomness {
            request_id: "pouch-recycle".to_string(),
            operation: fewfc::domain::RandomnessOperation::DiscardShuffle {
                pile: RandomnessDeck::Player(PlayerId::new("p1")),
                placement: fewfc::domain::DeckPlacement::Bottom,
            },
            continuation: RandomnessContinuation::Pouch(
                PouchRandomnessContinuation::SheepStealingRecycle {
                    source_card: card(9),
                },
            ),
            current_order: vec![card(1)],
        })
        .unwrap(),
        serde_json::json!({
            "requestId": "pouch-recycle",
            "operation": {
                "type": "discardShuffle",
                "pile": { "Player": "p1" },
                "placement": "Bottom"
            },
            "continuation": {
                "type": "pouch",
                "kind": { "sheepStealingRecycle": { "sourceCard": 9 } }
            },
            "currentOrder": [1]
        })
    );
    assert_eq!(
        serde_json::to_value(PouchRandomnessContinuation::SheepStealing {
            source_card: card(9),
            owner: Some(PlayerId::new("p1")),
        })
        .unwrap(),
        serde_json::json!({
            "sheepStealing": {
                "sourceCard": 9,
                "owner": "p1"
            }
        })
    );
    assert_eq!(
        serde_json::to_value(fewfc::domain::RandomnessOperation::DiscardShuffle {
            pile: RandomnessDeck::Player(PlayerId::new("p1")),
            placement: fewfc::domain::DeckPlacement::Bottom,
        })
        .unwrap(),
        serde_json::json!({
            "type": "discardShuffle",
            "pile": { "Player": "p1" },
            "placement": "Bottom"
        })
    );
}
