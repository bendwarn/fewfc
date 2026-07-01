//! Viewer-filtered Public View derivation from canonical game data.

use crate::domain::{
    CardInstanceId, CounterEffect, Element, GameEvent, GameState, GameStatus, PendingChoiceKind,
    Phase, Player, PlayerId, PlayerProfession, PlayerShield, PlayerStarHistory, RuleModuleId,
    StatusEffect, TeamHp, TeamStar,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Viewer {
    Player(PlayerId),
    Observer,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicGameState {
    pub enabled_rule_modules: Vec<RuleModuleId>,
    pub status: GameStatus,
    pub turn_number: u64,
    pub phase: Phase,
    pub current_player: Option<PlayerId>,
    pub players: Vec<Player>,
    pub turn_order: Vec<PlayerId>,
    pub hp: Vec<TeamHp>,
    pub hands: Vec<PublicPlayerHand>,
    pub discard: Vec<CardInstanceId>,
    pub player_decks: Vec<PublicPlayerDeck>,
    pub player_discards: Vec<PublicPlayerDiscard>,
    pub covered_passives: Vec<PublicCoveredPassive>,
    pub counter_effects: Vec<CounterEffect>,
    pub pending_choice: Option<PublicPendingChoice>,
    pub shields: Vec<PlayerShield>,
    pub statuses: Vec<StatusEffect>,
    pub environment: Option<Element>,
    pub team_stars: Vec<TeamStar>,
    pub star_histories: Vec<PlayerStarHistory>,
    pub five_star_alignment: Option<crate::domain::FiveStarAlignment>,
    pub professions: Vec<PlayerProfession>,
    pub prepared_profession_abilities: Vec<crate::domain::PreparedProfessionAbility>,
    pub previous_turn_formation: Option<PublicPreviousTurnFormation>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPlayerHand {
    pub player: PlayerId,
    pub cards: PublicCardRefs,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPlayerDeck {
    pub player: PlayerId,
    pub cards: PublicCardRefs,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPlayerDiscard {
    pub player: PlayerId,
    pub cards: Vec<CardInstanceId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicCoveredPassive {
    pub owner: PlayerId,
    pub formation_id: Option<String>,
    pub cards: PublicCardRefs,
    pub star_substitution: Option<crate::domain::StarElementSubstitution>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPreviousTurnFormation {
    pub player: PlayerId,
    pub formation_id: Option<String>,
    pub cards: PublicCardRefs,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicCardRefs {
    Known(Vec<CardInstanceId>),
    Hidden { count: usize },
    PartiallyKnown { cards: Vec<Option<CardInstanceId>> },
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
    PlayerDeckPrepared {
        player: PlayerId,
        deck: PublicCardRefs,
    },
    CardsDealt {
        player: PlayerId,
        cards: PublicCardRefs,
    },
    PassiveCovered {
        player: PlayerId,
        formation_id: Option<String>,
        cards: PublicCardRefs,
        star_substitution: Option<crate::domain::StarElementSubstitution>,
    },
    CardsDrawnForTurnDiscardChoice {
        player: PlayerId,
        drawn_cards: PublicCardRefs,
        allowed_discards: PublicCardRefs,
    },
    CardsDrawnForProfessionChoice {
        player: PlayerId,
        ability_id: String,
        cards: PublicCardRefs,
    },
    EffectChoiceRequested {
        player: PlayerId,
        kind: PublicPendingChoiceKind,
    },
    HandInspected {
        viewer: PlayerId,
        target: PlayerId,
        cards: PublicCardRefs,
    },
}

pub fn state_for(state: &GameState, viewer: Viewer) -> PublicGameState {
    let policy = RedactionPolicy::new(viewer);
    let previous_turn_formation = state
        .turn_number
        .checked_sub(1)
        .and_then(|previous_turn| {
            state
                .last_formation_by_player
                .iter()
                .find(|(_, formation)| formation.resolved_turn == previous_turn)
        })
        .map(|(player, formation)| {
            let remains_covered = state.covered_passives.iter().any(|passive| {
                passive.owner == *player
                    && passive.formation_id == formation.formation_id
                    && passive.covered_on_turn == formation.resolved_turn
            });
            let can_see_details = !remains_covered || policy.can_see_player_hidden_cards(player);

            PublicPreviousTurnFormation {
                player: player.clone(),
                formation_id: can_see_details.then(|| formation.formation_id.clone()),
                cards: if can_see_details {
                    PublicCardRefs::Known(formation.used_cards.clone())
                } else {
                    PublicCardRefs::Hidden {
                        count: formation.used_cards.len(),
                    }
                },
            }
        });

    let uses_personal_decks = state.uses_personal_decks();
    let uses_stars = state.has_rule_module(crate::domain::STAR_MODULE_ID);

    PublicGameState {
        enabled_rule_modules: state.enabled_rule_modules.clone(),
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
                cards: public_cards(
                    &hand.cards,
                    policy.can_see_player_hidden_cards(&hand.player),
                    &state.exposed_foreign_cards,
                ),
            })
            .collect(),
        discard: state.discard.clone(),
        player_decks: state
            .player_decks
            .iter()
            .filter(|_| uses_personal_decks)
            .map(|pile| PublicPlayerDeck {
                player: pile.player.clone(),
                cards: public_cards(&pile.cards, false, &state.exposed_foreign_cards),
            })
            .collect(),
        player_discards: state
            .player_discards
            .iter()
            .filter(|_| uses_personal_decks)
            .map(|pile| PublicPlayerDiscard {
                player: pile.player.clone(),
                cards: pile.cards.clone(),
            })
            .collect(),
        covered_passives: state
            .covered_passives
            .iter()
            .map(|passive| {
                let visible = policy.can_see_player_hidden_cards(&passive.owner)
                    || state
                        .revealed_covered_passive_owners
                        .contains(&passive.owner);
                PublicCoveredPassive {
                    owner: passive.owner.clone(),
                    formation_id: visible.then(|| passive.formation_id.clone()),
                    cards: if visible {
                        PublicCardRefs::Known(passive.cards.clone())
                    } else {
                        PublicCardRefs::Hidden {
                            count: passive.cards.len(),
                        }
                    },
                    star_substitution: visible.then(|| passive.star_substitution.clone()).flatten(),
                }
            })
            .collect(),
        counter_effects: state.counter_effects.clone(),
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
        environment: state.environment,
        team_stars: state
            .team_stars
            .iter()
            .filter(|_| uses_stars)
            .cloned()
            .collect(),
        star_histories: state
            .star_histories
            .iter()
            .filter(|_| uses_stars)
            .cloned()
            .collect(),
        five_star_alignment: uses_stars
            .then(|| state.five_star_alignment.clone())
            .flatten(),
        professions: state
            .has_rule_module(crate::domain::HERO_SCHOOLS_MODULE_ID)
            .then(|| state.professions.clone())
            .unwrap_or_default(),
        prepared_profession_abilities: state
            .has_rule_module(crate::domain::HERO_SCHOOLS_MODULE_ID)
            .then(|| state.prepared_profession_abilities.clone())
            .unwrap_or_default(),
        previous_turn_formation,
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
        GameEvent::PlayerDeckPrepared { player, deck_order } => {
            PublicGameEvent::PlayerDeckPrepared {
                player: player.clone(),
                deck: PublicCardRefs::Hidden {
                    count: deck_order.len(),
                },
            }
        }
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
            star_substitution,
            sealed: _,
        } => PublicGameEvent::PassiveCovered {
            player: player.clone(),
            formation_id: policy
                .can_see_player_hidden_cards(player)
                .then(|| formation_id.clone()),
            cards: if policy.can_see_player_hidden_cards(player) {
                PublicCardRefs::Known(cards.clone())
            } else {
                PublicCardRefs::Hidden { count: cards.len() }
            },
            star_substitution: policy
                .can_see_player_hidden_cards(player)
                .then(|| star_substitution.clone())
                .flatten(),
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
        GameEvent::CardsDrawnForProfessionChoice {
            player,
            ability_id,
            cards,
        } => PublicGameEvent::CardsDrawnForProfessionChoice {
            player: player.clone(),
            ability_id: ability_id.clone(),
            cards: if policy.can_see_player_hidden_cards(player) {
                PublicCardRefs::Known(cards.clone())
            } else {
                PublicCardRefs::Hidden { count: cards.len() }
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
        GameEvent::HandInspected {
            viewer,
            target,
            cards,
        } => PublicGameEvent::HandInspected {
            viewer: viewer.clone(),
            target: target.clone(),
            cards: if policy.can_see_player_hidden_cards(viewer) {
                PublicCardRefs::Known(cards.clone())
            } else {
                PublicCardRefs::Hidden { count: cards.len() }
            },
        },
        GameEvent::TurnStarted { .. }
        | GameEvent::ActionPassed { .. }
        | GameEvent::ProfessionChanged { .. }
        | GameEvent::ProfessionBroken { .. }
        | GameEvent::ProfessionAbilityActivated { .. }
        | GameEvent::PassiveCoverRevealed { .. }
        | GameEvent::TurnDiscardChosen { .. }
        | GameEvent::TurnDrawSkipped { .. }
        | GameEvent::FormationPerformed { .. }
        | GameEvent::FormationMatchOptionDeclared { .. }
        | GameEvent::FormationEffectCopied { .. }
        | GameEvent::FormationEffectIgnored { .. }
        | GameEvent::CounterEffectEstablished { .. }
        | GameEvent::CounterEffectResolved { .. }
        | GameEvent::AttackResolved { .. }
        | GameEvent::EnvironmentTransferred { .. }
        | GameEvent::EnvironmentCleared { .. }
        | GameEvent::StarBroken { .. }
        | GameEvent::StarSummoned { .. }
        | GameEvent::VoidStarBreakingCompleted { .. }
        | GameEvent::VoidReversionResolved { .. }
        | GameEvent::FiveStarAlignmentAchieved { .. }
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
        | GameEvent::PlayerDiscardRecycledIntoDeck { .. }
        | GameEvent::DiscardRetrieved { .. }
        | GameEvent::TurnEnded { .. } => PublicGameEvent::Public(event.clone()),
    }
}

fn public_cards(
    cards: &[CardInstanceId],
    can_see_all: bool,
    exposed_cards: &[CardInstanceId],
) -> PublicCardRefs {
    if can_see_all {
        return PublicCardRefs::Known(cards.to_vec());
    }

    if cards.iter().any(|card| exposed_cards.contains(card)) {
        PublicCardRefs::PartiallyKnown {
            cards: cards
                .iter()
                .map(|card| exposed_cards.contains(card).then_some(*card))
                .collect(),
        }
    } else {
        PublicCardRefs::Hidden { count: cards.len() }
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
