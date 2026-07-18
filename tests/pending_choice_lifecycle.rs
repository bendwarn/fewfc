use fewfc::application::{apply_event, handle_command};
use fewfc::domain::{
    BaseChoiceContinuation, CardInstanceId, ChoiceAnswer, ChoiceContinuation, ChoiceId, Command,
    GameEvent, GameSetup, GameState, PendingChoice, PendingChoiceKind, Phase, PlayerId,
    ValidationError,
};
use fewfc::public_view::{PublicPendingChoice, Viewer, state_for};

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

#[test]
fn accepted_answer_records_choice_made_before_domain_consequences_and_releases_the_lifecycle() {
    let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::TurnDrawDiscardChoice;
    state.hands[0].cards = vec![card(1)];
    state.pending_choice = Some(PendingChoice {
        choice_id: ChoiceId::new(1),
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::Card {
            cards: vec![card(1)],
            minimum: 1,
            maximum: 1,
            can_decline: false,
        },
        continuation: ChoiceContinuation::Base(BaseChoiceContinuation::TurnDrawDiscard),
    });
    state.next_choice_id = ChoiceId::new(2);

    assert_eq!(
        handle_command(
            &state,
            Command::AnswerChoice {
                player: PlayerId::new("p1"),
                choice_id: ChoiceId::new(99),
                answer: ChoiceAnswer::Cards {
                    cards: vec![card(1)]
                },
            },
        ),
        Err(fewfc::domain::GameError::Validation(
            ValidationError::StaleChoiceId {
                expected: ChoiceId::new(1),
                actual: ChoiceId::new(99),
            }
        ))
    );
    assert!(state.pending_choice.is_some());

    let events = handle_command(
        &state,
        Command::AnswerChoice {
            player: PlayerId::new("p1"),
            choice_id: ChoiceId::new(1),
            answer: ChoiceAnswer::Cards {
                cards: vec![card(1)],
            },
        },
    )
    .unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::ChoiceMade { choice_id, .. },
            GameEvent::TurnDiscardChosen { .. },
            ..
        ] if *choice_id == ChoiceId::new(1)
    ));

    for event in events {
        apply_event(&mut state, &event);
    }
    assert!(state.pending_choice.is_none());
    assert_eq!(state.phase, Phase::TurnEnd);
    assert_eq!(state.next_choice_id, ChoiceId::new(2));
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
        Err(fewfc::domain::GameError::Validation(
            ValidationError::StaleChoiceId {
                expected: ChoiceId::new(2),
                actual: ChoiceId::new(1),
            }
        ))
    );
}

#[test]
fn public_pending_choice_redacts_choice_id_options_and_continuation_from_non_owners() {
    let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
    let mut state = GameState::from_setup(&setup);
    state.pending_choice = Some(PendingChoice {
        choice_id: ChoiceId::new(7),
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::Card {
            cards: vec![card(1), card(2)],
            minimum: 2,
            maximum: 2,
            can_decline: false,
        },
        continuation: ChoiceContinuation::Base(BaseChoiceContinuation::ChaosReturnTwo),
    });

    assert!(matches!(
        state_for(&state, Viewer::Player(PlayerId::new("p1"))).pending_choice,
        Some(PublicPendingChoice::Visible { choice_id, .. }) if choice_id == ChoiceId::new(7)
    ));
    let hidden = state_for(&state, Viewer::Player(PlayerId::new("p2")))
        .pending_choice
        .expect("opponent sees pending activity");
    assert!(matches!(hidden, PublicPendingChoice::Hidden { .. }));
    let json = serde_json::to_value(hidden).unwrap();
    assert!(json.get("choiceId").is_none());
    assert!(json.get("choice").is_none());
    assert!(json.get("continuation").is_none());
}

#[test]
fn unified_choice_answer_command_uses_the_choice_id_and_camel_case_answer_fields() {
    let command = Command::AnswerChoice {
        player: PlayerId::new("p1"),
        choice_id: ChoiceId::new(7),
        answer: ChoiceAnswer::SheepStealing {
            deck_cards: vec![card(2), card(3)],
            discard_cards: vec![card(4), card(5)],
        },
    };

    assert_eq!(
        serde_json::to_value(command).unwrap(),
        serde_json::json!({
            "answerChoice": {
                "player": "p1",
                "choiceId": 7,
                "answer": {
                    "type": "sheepStealing",
                    "deckCards": [2, 3],
                    "discardCards": [4, 5]
                }
            }
        })
    );
}
