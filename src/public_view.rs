//! Viewer-filtered Public View derivation from canonical game data.

use crate::domain::{
    CardInstanceId, GameEvent, GameState, GameStatus, PendingChoiceKind, Phase, Player, PlayerId,
    PlayerShield, StatusEffect, TeamHp,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Viewer {
    Player(PlayerId),
    Observer,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicGameState {
    pub status: GameStatus,
    pub turn_number: u64,
    pub phase: Phase,
    pub current_player: Option<PlayerId>,
    pub players: Vec<Player>,
    pub turn_order: Vec<PlayerId>,
    pub hp: Vec<TeamHp>,
    pub hands: Vec<PublicPlayerHand>,
    pub discard: Vec<CardInstanceId>,
    pub covered_passives: Vec<PublicCoveredPassive>,
    pub pending_choice: Option<PublicPendingChoice>,
    pub shields: Vec<PlayerShield>,
    pub statuses: Vec<StatusEffect>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPlayerHand {
    pub player: PlayerId,
    pub cards: PublicCardRefs,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicCoveredPassive {
    pub owner: PlayerId,
    pub formation_id: String,
    pub cards: PublicCardRefs,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicCardRefs {
    Known(Vec<CardInstanceId>),
    Hidden { count: usize },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPendingChoice {
    pub player: PlayerId,
    pub kind: PublicPendingChoiceKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicPendingChoiceKind {
    Known(PendingChoiceKind),
    Hidden,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicGameEvent {
    Public(GameEvent),
    DeckPrepared {
        deck: PublicCardRefs,
    },
    CardsDealt {
        player: PlayerId,
        cards: PublicCardRefs,
    },
    PassiveCovered {
        player: PlayerId,
        formation_id: String,
        cards: PublicCardRefs,
    },
    CardsDrawnForTurnDiscardChoice {
        player: PlayerId,
        drawn_cards: PublicCardRefs,
        allowed_discards: PublicCardRefs,
    },
    EffectChoiceRequested {
        player: PlayerId,
        kind: PublicPendingChoiceKind,
    },
}

pub fn state_for(state: &GameState, viewer: Viewer) -> PublicGameState {
    let policy = RedactionPolicy::new(viewer);
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
                cards: if policy.can_see_player_hidden_cards(&hand.player) {
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
                cards: if policy.can_see_player_hidden_cards(&passive.owner) {
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
                kind: if policy.can_see_player_hidden_cards(&choice.player) {
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
    let policy = RedactionPolicy::new(viewer);
    match event {
        GameEvent::DeckPrepared { deck_order } => PublicGameEvent::DeckPrepared {
            deck: PublicCardRefs::Hidden {
                count: deck_order.len(),
            },
        },
        GameEvent::CardsDealt { player, cards } => PublicGameEvent::CardsDealt {
            player: player.clone(),
            cards: if policy.can_see_player_hidden_cards(player) {
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
            cards: if policy.can_see_player_hidden_cards(player) {
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
            drawn_cards: if policy.can_see_player_hidden_cards(player) {
                PublicCardRefs::Known(drawn_cards.clone())
            } else {
                PublicCardRefs::Hidden {
                    count: drawn_cards.len(),
                }
            },
            allowed_discards: if policy.can_see_player_hidden_cards(player) {
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
                kind: if policy.can_see_player_hidden_cards(player) {
                    PublicPendingChoiceKind::Known(kind.clone())
                } else {
                    PublicPendingChoiceKind::Hidden
                },
            }
        }
        GameEvent::TurnStarted { .. }
        | GameEvent::ActionPassed { .. }
        | GameEvent::TurnDiscardChosen { .. }
        | GameEvent::TurnDrawSkipped { .. }
        | GameEvent::FormationPerformed { .. }
        | GameEvent::AttackResolved { .. }
        | GameEvent::TurnDrawBonusChanged { .. }
        | GameEvent::ShieldChanged { .. }
        | GameEvent::HpChanged { .. }
        | GameEvent::CardsMoved { .. }
        | GameEvent::StatusAdded { .. }
        | GameEvent::StatusExpired { .. }
        | GameEvent::StatusRemoved { .. }
        | GameEvent::EffectChoiceAnswered { .. }
        | GameEvent::PassiveFlipped { .. }
        | GameEvent::DiscardRecycledIntoDeck { .. }
        | GameEvent::TurnEnded { .. } => PublicGameEvent::Public(event.clone()),
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct RedactionPolicy {
    viewer: Viewer,
}

impl RedactionPolicy {
    fn new(viewer: Viewer) -> Self {
        Self { viewer }
    }

    fn can_see_player_hidden_cards(&self, player: &PlayerId) -> bool {
        matches!(&self.viewer, Viewer::Player(viewer) if viewer == player)
    }
}
