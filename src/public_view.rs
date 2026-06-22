//! Viewer-filtered Public View derivation from canonical game data.

use crate::domain::PlayerId;
use crate::domain::{
    GameEvent, GameState, PublicCardRefs, PublicCoveredPassive, PublicGameEvent, PublicGameState,
    PublicPendingChoice, PublicPendingChoiceKind, PublicPlayerHand, Viewer,
};

pub fn state_for(state: &GameState, viewer: Viewer) -> PublicGameState {
    PublicGameState {
        status: state.status.clone(),
        turn_number: state.turn_number,
        phase: state.phase,
        current_player: state.current_player().cloned(),
        players: state.players.clone(),
        turn_order: state.turn_order.clone(),
        hp: state.hp.clone(),
        hands: state
            .hands
            .iter()
            .map(|hand| PublicPlayerHand {
                player: hand.player.clone(),
                cards: if can_see_player_hidden_cards(&viewer, &hand.player) {
                    PublicCardRefs::Known(hand.cards.clone())
                } else {
                    PublicCardRefs::Hidden {
                        count: hand.cards.len(),
                    }
                },
            })
            .collect(),
        discard: state.discard.clone(),
        covered_passives: state
            .covered_passives
            .iter()
            .map(|passive| PublicCoveredPassive {
                owner: passive.owner.clone(),
                formation_id: passive.formation_id.clone(),
                cards: if can_see_player_hidden_cards(&viewer, &passive.owner) {
                    PublicCardRefs::Known(passive.cards.clone())
                } else {
                    PublicCardRefs::Hidden {
                        count: passive.cards.len(),
                    }
                },
            })
            .collect(),
        pending_choice: state
            .pending_choice
            .as_ref()
            .map(|choice| PublicPendingChoice {
                player: choice.player.clone(),
                kind: if can_see_player_hidden_cards(&viewer, &choice.player) {
                    PublicPendingChoiceKind::Known(choice.kind.clone())
                } else {
                    PublicPendingChoiceKind::Hidden
                },
            }),
        shields: state.shields.clone(),
        statuses: state.statuses.clone(),
    }
}

pub fn event_for(event: &GameEvent, viewer: Viewer) -> PublicGameEvent {
    match event {
        GameEvent::DeckPrepared { deck_order } => PublicGameEvent::DeckPrepared {
            deck: PublicCardRefs::Hidden {
                count: deck_order.len(),
            },
        },
        GameEvent::CardsDealt { player, cards } => PublicGameEvent::CardsDealt {
            player: player.clone(),
            cards: if can_see_player_hidden_cards(&viewer, player) {
                PublicCardRefs::Known(cards.clone())
            } else {
                PublicCardRefs::Hidden { count: cards.len() }
            },
        },
        GameEvent::PassiveCovered {
            player,
            formation_id,
            cards,
            sealed: _,
        } => PublicGameEvent::PassiveCovered {
            player: player.clone(),
            formation_id: formation_id.clone(),
            cards: if can_see_player_hidden_cards(&viewer, player) {
                PublicCardRefs::Known(cards.clone())
            } else {
                PublicCardRefs::Hidden { count: cards.len() }
            },
        },
        GameEvent::CardsDrawnForTurnDiscardChoice {
            player,
            drawn_cards,
            allowed_discards,
        } => PublicGameEvent::CardsDrawnForTurnDiscardChoice {
            player: player.clone(),
            drawn_cards: if can_see_player_hidden_cards(&viewer, player) {
                PublicCardRefs::Known(drawn_cards.clone())
            } else {
                PublicCardRefs::Hidden {
                    count: drawn_cards.len(),
                }
            },
            allowed_discards: if can_see_player_hidden_cards(&viewer, player) {
                PublicCardRefs::Known(allowed_discards.clone())
            } else {
                PublicCardRefs::Hidden {
                    count: allowed_discards.len(),
                }
            },
        },
        GameEvent::EffectChoiceRequested { player, kind } => {
            PublicGameEvent::EffectChoiceRequested {
                player: player.clone(),
                kind: if can_see_player_hidden_cards(&viewer, player) {
                    PublicPendingChoiceKind::Known(kind.clone())
                } else {
                    PublicPendingChoiceKind::Hidden
                },
            }
        }
        event => PublicGameEvent::Public(event.clone()),
    }
}

pub fn events_for<'a>(
    events: impl IntoIterator<Item = &'a GameEvent>,
    viewer: Viewer,
) -> Vec<PublicGameEvent> {
    events
        .into_iter()
        .map(|event| event_for(event, viewer.clone()))
        .collect()
}

fn can_see_player_hidden_cards(viewer: &Viewer, player: &PlayerId) -> bool {
    matches!(viewer, Viewer::Player(viewer) if viewer == player)
}
