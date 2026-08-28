use fewfc::application::{
    advance_automatic, apply_event, handle_command, replay, resolve_trusted_randomness,
};
use fewfc::domain::{
    CardInstanceId, ChainPouchDecision, ChoiceAnswer, ChoiceId, Command, EngineInvariantError,
    GameError, GameEvent, GameSetup, GameState, PassActionReason, PendingChoice, PendingChoiceKind,
    PendingRandomness, PendingResolution, PlayerId, RandomnessDeck, SecretStrategyDecision,
    SecretStrategyStarOperation, TrustedRandomnessAnswer, ValidationError,
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
                current_order: vec![card(1), card(2), card(3)],
            },
            resolution: PendingResolution::PouchSheepStealing {
                source_card: card(1),
                owner: None,
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
fn randomness_without_its_semantic_pending_resolution_is_an_engine_invariant() {
    let mut state = state_with_pending_randomness();
    state.pending_resolution = None;

    assert_eq!(
        resolve_trusted_randomness(
            &state,
            &TrustedRandomnessAnswer {
                request_id: "shuffle-1".to_string(),
                shuffled_order: vec![card(3), card(2), card(1)],
            },
        ),
        Err(GameError::EngineInvariant(
            EngineInvariantError::InvalidPendingResolution
        ))
    );
}

#[test]
fn randomness_cannot_resume_a_choice_only_pending_resolution() {
    let mut state = state_with_pending_randomness();
    state.pending_resolution = Some(PendingResolution::TurnDrawDiscard);

    assert_eq!(
        resolve_trusted_randomness(
            &state,
            &TrustedRandomnessAnswer {
                request_id: "shuffle-1".to_string(),
                shuffled_order: vec![card(3), card(2), card(1)],
            },
        ),
        Err(GameError::EngineInvariant(
            EngineInvariantError::InvalidPendingResolution
        ))
    );
}

#[test]
fn choice_cannot_resume_a_randomness_only_pending_resolution() {
    let mut state = GameState::from_setup(&setup());
    apply_event(
        &mut state,
        &GameEvent::ChoiceRequested {
            choice: PendingChoice {
                choice_id: ChoiceId::new(1),
                player: PlayerId::new("p1"),
                kind: PendingChoiceKind::Card {
                    cards: vec![card(1)],
                    minimum: 1,
                    maximum: 1,
                    can_decline: false,
                },
            },
            resolution: PendingResolution::TurnDraw,
        },
    );

    assert_eq!(
        handle_command(
            &state,
            Command::AnswerChoice {
                player: PlayerId::new("p1"),
                choice_id: ChoiceId::new(1),
                answer: ChoiceAnswer::Cards {
                    cards: vec![card(1)],
                },
            },
        ),
        Err(GameError::EngineInvariant(
            EngineInvariantError::InvalidPendingResolution
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
                    current_order: vec![card(1), card(2), card(3)],
                },
                resolution: PendingResolution::PouchSheepStealing {
                    source_card: card(1),
                    owner: None,
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
        resolution: state.pending_resolution.unwrap(),
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
            decision: ChainPouchDecision::PlaceAndTrigger {
                pouch_owner: PlayerId::new("p2"),
                pouch_card: card(2),
                decision: SecretStrategyDecision::Star {
                    source_card: card(3),
                    operation: SecretStrategyStarOperation::Break {
                        star: fewfc::domain::StarKind::Fire,
                    },
                },
            },
        })
        .unwrap(),
        serde_json::json!({
            "type": "chain",
            "decision": {
                "type": "placeAndTrigger",
                "pouchOwner": "p2",
                "pouchCard": 2,
                "decision": {
                    "type": "star",
                    "sourceCard": 3,
                    "operation": { "type": "break", "star": "Fire" }
                }
            }
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
                decision: ChainPouchDecision::PlaceOnly {
                    pouch_owner: PlayerId::new("p2"),
                    pouch_card: card(2),
                },
            },
        })
        .unwrap(),
        serde_json::json!({
            "answerChoice": {
                "player": "p1",
                "choiceId": 7,
                "answer": {
                    "type": "chain",
                    "decision": {
                        "type": "placeOnly",
                        "pouchOwner": "p2",
                        "pouchCard": 2
                    }
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
            current_order: vec![card(1), card(2)],
        })
        .unwrap(),
        serde_json::json!({
            "requestId": "request",
            "operation": {
                "type": "deckShuffle",
                "deck": "Shared"
            },
            "currentOrder": [1, 2],
        })
    );
    assert_eq!(
        serde_json::to_value(PendingResolution::SpiritDeathOmen {
            player: PlayerId::new("p1"),
        })
        .unwrap(),
        serde_json::json!({
            "type": "spiritDeathOmen",
            "player": "p1"
        })
    );
    assert_eq!(
        serde_json::to_value(PendingRandomness {
            request_id: "pouch-recycle".to_string(),
            operation: fewfc::domain::RandomnessOperation::DiscardShuffle {
                pile: RandomnessDeck::Player(PlayerId::new("p1")),
                placement: fewfc::domain::DeckPlacement::Bottom,
            },
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
            "currentOrder": [1]
        })
    );
    assert_eq!(
        serde_json::to_value(PendingResolution::PouchSheepStealing {
            source_card: card(9),
            owner: Some(PlayerId::new("p1")),
        })
        .unwrap(),
        serde_json::json!({
            "type": "pouchSheepStealing",
            "sourceCard": 9,
            "owner": "p1"
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
