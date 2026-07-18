use crate::domain::{
    ChoiceAnswer, ChoiceId, ChoiceRequest, GameError, GameEvent, GameResult, GameState,
    PendingChoice, PendingChoiceKind, PlayerId, ValidationError,
};
use std::collections::HashSet;

/// The single owner of canonical Pending Choice allocation and answer-shape
/// validation. Rule modules submit `ChoiceRequest`; they never allocate IDs or
/// assemble a waiting state themselves.
pub(crate) fn request_event(state: &GameState, request: ChoiceRequest) -> GameResult<GameEvent> {
    if state.pending_choice.is_some() {
        return Err(GameError::EngineInvariant(
            crate::domain::EngineInvariantError::InvalidPendingChoice,
        ));
    }
    validate_request(&request)?;
    Ok(GameEvent::ChoiceRequested {
        choice: PendingChoice {
            choice_id: state.next_choice_id,
            player: request.player,
            kind: request.kind,
            continuation: request.continuation,
        },
    })
}

pub(crate) fn validate_answer(
    choice: &PendingChoice,
    player: &PlayerId,
    choice_id: ChoiceId,
    answer: &ChoiceAnswer,
) -> GameResult<()> {
    if &choice.player != player {
        return Err(GameError::Validation(ValidationError::WrongPlayer {
            expected: choice.player.clone(),
            actual: player.clone(),
        }));
    }
    if choice.choice_id != choice_id {
        return Err(GameError::Validation(ValidationError::StaleChoiceId {
            expected: choice.choice_id,
            actual: choice_id,
        }));
    }

    let valid = match (&choice.kind, answer) {
        (
            PendingChoiceKind::Card {
                cards,
                minimum,
                maximum,
                ..
            },
            ChoiceAnswer::Cards { cards: selected },
        ) => cards_are_valid(cards, *minimum, *maximum, selected),
        (
            PendingChoiceKind::Card {
                can_decline: true, ..
            },
            ChoiceAnswer::Decline,
        )
        | (
            PendingChoiceKind::Player {
                can_decline: true, ..
            },
            ChoiceAnswer::Decline,
        )
        | (
            PendingChoiceKind::Formation {
                can_decline: true, ..
            },
            ChoiceAnswer::Decline,
        )
        | (
            PendingChoiceKind::Environment {
                can_decline: true, ..
            },
            ChoiceAnswer::Decline,
        ) => true,
        (PendingChoiceKind::Player { players, .. }, ChoiceAnswer::Player { player }) => {
            players.contains(player)
        }
        (
            PendingChoiceKind::Formation { formations, .. },
            ChoiceAnswer::Formation { formation_id },
        ) => formations.contains(formation_id),
        (
            PendingChoiceKind::Environment { environments, .. },
            ChoiceAnswer::Environment { environment },
        ) => environments.contains(environment),
        (
            PendingChoiceKind::Chain {
                pouch_owners,
                deck_cards,
            },
            ChoiceAnswer::Chain {
                pouch_owner,
                pouch_card,
                trigger_card,
                ..
            },
        ) => {
            pouch_owners.contains(pouch_owner)
                && deck_cards.contains(pouch_card)
                && trigger_card.is_none_or(|card| deck_cards.contains(&card) && card != *pouch_card)
        }
        (
            PendingChoiceKind::SheepStealing {
                deck_cards,
                discard_cards,
                ..
            },
            ChoiceAnswer::SheepStealing {
                deck_cards: selected_deck,
                discard_cards: selected_discard,
            },
        ) => {
            cards_are_valid(deck_cards, 2, 2, selected_deck)
                // Sheep Stealing first moves the selected deck cards to the
                // discard pile, then lets the player choose two cards to
                // return. The typed answer therefore permits the original
                // discard cards plus precisely those selected deck cards.
                && cards_are_valid(
                    &discard_cards
                        .iter()
                        .chain(selected_deck)
                        .copied()
                        .collect::<Vec<_>>(),
                    2,
                    2,
                    selected_discard,
                )
        }
        _ => false,
    };

    valid
        .then_some(())
        .ok_or(GameError::Validation(ValidationError::InvalidChoiceAnswer))
}

/// Dispatches the one public choice-answer command after validating the active
/// `ChoiceId`, owner, and typed answer. Consequence planning remains in the
/// rule modules, but every continuation is entered through this lifecycle
/// boundary so a raw continuation can never be invoked directly by a command.
pub(crate) fn answer_events(
    state: &GameState,
    player: PlayerId,
    choice_id: ChoiceId,
    answer: ChoiceAnswer,
) -> GameResult<Vec<GameEvent>> {
    let Some(choice) = state.pending_choice.as_ref() else {
        return if choice_id < state.next_choice_id {
            Err(GameError::Validation(ValidationError::StaleChoiceId {
                expected: state.next_choice_id,
                actual: choice_id,
            }))
        } else {
            Err(GameError::Validation(ValidationError::MissingPendingChoice))
        };
    };
    validate_answer(choice, &player, choice_id, &answer)?;
    crate::rules::base::resolve_answered_choice(state, choice, player, choice_id, answer)
}

fn validate_request(request: &ChoiceRequest) -> GameResult<()> {
    if let PendingChoiceKind::Card {
        cards,
        minimum,
        maximum,
        can_decline,
    } = &request.kind
    {
        if minimum > maximum || (!can_decline && (*maximum > cards.len() || *minimum > cards.len()))
        {
            return Err(GameError::EngineInvariant(
                crate::domain::EngineInvariantError::InvalidPendingChoice,
            ));
        }
    }
    Ok(())
}

fn cards_are_valid(
    allowed: &[crate::domain::CardInstanceId],
    minimum: usize,
    maximum: usize,
    selected: &[crate::domain::CardInstanceId],
) -> bool {
    selected.len() >= minimum
        && selected.len() <= maximum
        && selected.iter().all(|card| allowed.contains(card))
        && selected.iter().collect::<HashSet<_>>().len() == selected.len()
}

#[cfg(test)]
mod tests {
    use super::validate_answer;
    use crate::domain::{
        CardInstanceId, ChoiceAnswer, ChoiceContinuation, ChoiceId, ChoiceRequest, GameError,
        GameSetup, GameState, PendingChoice, PendingChoiceKind, PlayerId,
    };

    fn card(value: u64) -> CardInstanceId {
        CardInstanceId::new(value)
    }

    fn choice(kind: PendingChoiceKind) -> PendingChoice {
        PendingChoice {
            choice_id: ChoiceId::new(7),
            player: PlayerId::new("p1"),
            kind,
            continuation: ChoiceContinuation::Base(
                crate::domain::BaseChoiceContinuation::TurnDrawDiscard,
            ),
        }
    }

    #[test]
    fn validates_every_typed_choice_shape_before_continuation_dispatch() {
        let cases = vec![
            (
                choice(PendingChoiceKind::Card {
                    cards: vec![card(1), card(2)],
                    minimum: 1,
                    maximum: 2,
                    can_decline: false,
                }),
                ChoiceAnswer::Cards {
                    cards: vec![card(1)],
                },
                ChoiceAnswer::Cards {
                    cards: vec![card(3)],
                },
            ),
            (
                choice(PendingChoiceKind::Player {
                    players: vec![PlayerId::new("p2")],
                    can_decline: false,
                }),
                ChoiceAnswer::Player {
                    player: PlayerId::new("p2"),
                },
                ChoiceAnswer::Player {
                    player: PlayerId::new("p3"),
                },
            ),
            (
                choice(PendingChoiceKind::Formation {
                    formations: vec!["weapon".to_string()],
                    can_decline: false,
                }),
                ChoiceAnswer::Formation {
                    formation_id: "weapon".to_string(),
                },
                ChoiceAnswer::Formation {
                    formation_id: "barrier".to_string(),
                },
            ),
            (
                choice(PendingChoiceKind::Environment {
                    environments: vec![crate::domain::Element::Fire],
                    can_decline: false,
                }),
                ChoiceAnswer::Environment {
                    environment: crate::domain::Element::Fire,
                },
                ChoiceAnswer::Environment {
                    environment: crate::domain::Element::Water,
                },
            ),
            (
                choice(PendingChoiceKind::Chain {
                    pouch_owners: vec![PlayerId::new("p2")],
                    deck_cards: vec![card(1), card(2)],
                }),
                ChoiceAnswer::Chain {
                    pouch_owner: PlayerId::new("p2"),
                    pouch_card: card(1),
                    trigger_card: Some(card(2)),
                    strategy: None,
                    target_player: None,
                    star: None,
                    break_star: false,
                    discard_card: None,
                },
                ChoiceAnswer::Chain {
                    pouch_owner: PlayerId::new("p3"),
                    pouch_card: card(1),
                    trigger_card: None,
                    strategy: None,
                    target_player: None,
                    star: None,
                    break_star: false,
                    discard_card: None,
                },
            ),
            (
                choice(PendingChoiceKind::SheepStealing {
                    source_card: card(9),
                    owner: None,
                    deck_cards: vec![card(1), card(2)],
                    discard_cards: vec![card(3), card(4)],
                }),
                ChoiceAnswer::SheepStealing {
                    deck_cards: vec![card(1), card(2)],
                    discard_cards: vec![card(1), card(3)],
                },
                ChoiceAnswer::SheepStealing {
                    deck_cards: vec![card(1), card(2)],
                    discard_cards: vec![card(3), card(5)],
                },
            ),
        ];

        for (choice, valid, invalid) in cases {
            assert!(
                validate_answer(&choice, &PlayerId::new("p1"), ChoiceId::new(7), &valid).is_ok()
            );
            assert!(
                validate_answer(&choice, &PlayerId::new("p1"), ChoiceId::new(7), &invalid).is_err()
            );
        }
    }

    #[test]
    fn rejects_wrong_player_and_stale_choice_id_without_dispatching() {
        let choice = choice(PendingChoiceKind::Card {
            cards: vec![card(1)],
            minimum: 1,
            maximum: 1,
            can_decline: false,
        });
        let answer = ChoiceAnswer::Cards {
            cards: vec![card(1)],
        };

        assert!(validate_answer(&choice, &PlayerId::new("p2"), ChoiceId::new(7), &answer).is_err());
        assert!(validate_answer(&choice, &PlayerId::new("p1"), ChoiceId::new(8), &answer).is_err());
    }

    #[test]
    fn refuses_to_replace_an_active_canonical_choice() {
        let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30);
        let mut state = GameState::from_setup(&setup);
        state.pending_choice = Some(choice(PendingChoiceKind::Card {
            cards: vec![card(1)],
            minimum: 1,
            maximum: 1,
            can_decline: false,
        }));

        assert!(matches!(
            super::request_event(
                &state,
                ChoiceRequest {
                    player: PlayerId::new("p1"),
                    kind: PendingChoiceKind::Card {
                        cards: vec![card(2)],
                        minimum: 1,
                        maximum: 1,
                        can_decline: false,
                    },
                    continuation: ChoiceContinuation::Base(
                        crate::domain::BaseChoiceContinuation::TurnDrawDiscard,
                    ),
                },
            ),
            Err(GameError::EngineInvariant(
                crate::domain::EngineInvariantError::InvalidPendingChoice
            ))
        ));
    }
}
