//! Viewer-filtered Public View derivation from canonical game data.

use crate::domain::{
    CardInstanceId, CounterEffect, Element, FormationSuppression, GameEvent, GameState, GameStatus,
    JianghuState, LimitedUse, PendingChoiceKind, Phase, Player, PlayerId, PlayerProfession,
    PlayerShield, PlayerStarHistory, RandomnessDeck, RuleModuleId, ScheduledEcho,
    ScheduledPlantEarth, StatusEffect, TeamHp, TeamStar,
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
    pub pouches: Vec<PublicPouch>,
    pub covered_passives: Vec<PublicCoveredPassive>,
    pub counter_effects: Vec<CounterEffect>,
    pub pending_choice: Option<PublicPendingChoice>,
    pub pending_randomness: Option<PublicPendingRandomness>,
    pub shields: Vec<PlayerShield>,
    pub statuses: Vec<StatusEffect>,
    pub jianghu_states: Vec<JianghuState>,
    pub limited_uses: Vec<LimitedUse>,
    pub confluence_card_obligations: Vec<PublicConfluenceCardObligation>,
    pub scheduled_echoes: Vec<ScheduledEcho>,
    pub flow_states: Vec<PublicFlowState>,
    pub formation_suppressions: Vec<FormationSuppression>,
    pub scheduled_plant_earth: Vec<ScheduledPlantEarth>,
    pub environment: Option<Element>,
    pub team_stars: Vec<TeamStar>,
    pub star_histories: Vec<PlayerStarHistory>,
    pub five_star_alignment: Option<crate::domain::FiveStarAlignment>,
    pub professions: Vec<PlayerProfession>,
    pub card_interpretations: Vec<PublicCardInterpretation>,
    pub spirits: Vec<crate::domain::PlayerSpirit>,
    pub previous_turn_formation: Option<PublicPreviousTurnFormation>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicCardInterpretation {
    ProfessionAbility {
        player: PlayerId,
        ability_id: String,
        card: Option<CardInstanceId>,
        element: Element,
        level: u32,
    },
    SpiritSkill {
        player: PlayerId,
        skill: Option<crate::domain::SpiritSkill>,
        card: Option<CardInstanceId>,
        level: u32,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicConfluenceCardObligation {
    pub owner: PlayerId,
    pub card: Option<CardInstanceId>,
    pub allow_profession_formation: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicFlowState {
    pub player: PlayerId,
    pub layers: u32,
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
pub struct PublicPouch {
    pub owner: PlayerId,
    pub card: Option<CardInstanceId>,
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
    pub purpose: String,
    pub presentation: PublicPendingChoicePresentation,
    pub kind: PublicPendingChoiceKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PublicPendingChoicePresentation {
    TurnDrawDiscard,
    HolyWind,
    Chaos,
    Revelation,
    AzureCloudStep,
    ClearWindTenThousandMiles,
    MirrorResonance,
    MyriadResonance,
    ThousandResonance,
    EchoRingingMetalDeckCard,
    EchoCost {
        melody: PublicEchoMelodyPresentation,
    },
    EchoSplitEarthFormation,
    EchoPureFirePlayer,
    EchoPlantEarthMelody,
    EarthRendingEnvironment,
    EarthRendingCard,
    Metamorphosis,
    SealCard,
    Unclassified,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PublicEchoMelodyPresentation {
    RingingMetal,
    FallingWood,
    FlowingWater,
    WarFire,
    SplitEarth,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicPendingChoiceKind {
    Known(PendingChoiceKind),
    Hidden,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPendingRandomness {
    pub request_id: String,
    pub deck: RandomnessDeck,
    pub card_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicGameEvent {
    Public(GameEvent),
    GamePreparationStarted,
    InitialPouchChosen {
        player: PlayerId,
    },
    PouchPlaced {
        owner: PlayerId,
        card: Option<CardInstanceId>,
    },
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
    SpiritSkillUsed {
        player: PlayerId,
        spirit: crate::domain::SpiritKind,
        skill: crate::domain::SpiritSkill,
        old_power: u32,
        new_power: u32,
        selected_card: Option<CardInstanceId>,
        declared_level: Option<u32>,
    },
    SpiritLevelInterpreted {
        player: PlayerId,
        skill: Option<crate::domain::SpiritSkill>,
        card: Option<CardInstanceId>,
        level: u32,
        applied_on_turn: u64,
    },
    EffectChoiceRequested {
        player: PlayerId,
        purpose: String,
        kind: PublicPendingChoiceKind,
    },
    RandomnessRequested {
        request_id: String,
        deck: RandomnessDeck,
        card_count: usize,
    },
    RandomnessResolved {
        request_id: String,
        deck: RandomnessDeck,
        card_count: usize,
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
            .map(|pile| {
                let visible = state.has_rule_module(crate::domain::POUCH_MODULE_ID)
                    && policy.can_see_player_hidden_cards(&pile.player);
                let cards = if visible {
                    let mut cards = pile.cards.clone();
                    cards.sort();
                    PublicCardRefs::Known(cards)
                } else {
                    public_cards(&pile.cards, false, &state.exposed_foreign_cards)
                };
                PublicPlayerDeck {
                    player: pile.player.clone(),
                    cards,
                }
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
        pouches: state
            .pouches
            .iter()
            .map(|pouch| PublicPouch {
                owner: pouch.owner.clone(),
                card: match &policy.viewer {
                    Viewer::Player(viewer) if pouch.known_by.contains(viewer) => Some(pouch.card),
                    _ => None,
                },
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
                purpose: pending_choice_purpose(&choice.kind),
                presentation: pending_choice_presentation(&choice.kind),
                kind: if policy.can_see_player_hidden_cards(&choice.player) {
                    PublicPendingChoiceKind::Known(choice.kind.clone())
                } else {
                    PublicPendingChoiceKind::Hidden
                },
            }),
        pending_randomness: state.pending_randomness.as_ref().map(|request| {
            PublicPendingRandomness {
                request_id: request.request_id.clone(),
                deck: request.deck.clone(),
                card_count: request.current_order.len(),
            }
        }),
        shields: state.shields.clone(),
        statuses: state.statuses.clone(),
        jianghu_states: if state.has_rule_module(crate::domain::JIANGHU_MODULE_ID) {
            state.jianghu_states.clone()
        } else {
            Vec::new()
        },
        limited_uses: state.limited_uses.clone(),
        confluence_card_obligations: state
            .confluence_card_obligations
            .iter()
            .map(|obligation| PublicConfluenceCardObligation {
                owner: obligation.owner.clone(),
                card: policy
                    .can_see_player_hidden_cards(&obligation.owner)
                    .then_some(obligation.card),
                allow_profession_formation: obligation.allow_profession_formation,
            })
            .collect(),
        scheduled_echoes: state.scheduled_echoes.clone(),
        flow_states: public_flow_states(state),
        formation_suppressions: state.formation_suppressions.clone(),
        scheduled_plant_earth: state.scheduled_plant_earth.clone(),
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
        professions: if state.enabled_rule_modules.iter().any(|module| {
            matches!(
                module.as_str(),
                crate::domain::HERO_SCHOOLS_MODULE_ID | crate::domain::JIANGHU_MODULE_ID
            )
        }) {
            state.professions.clone()
        } else {
            Vec::new()
        },
        card_interpretations: state
            .prepared_profession_abilities
            .iter()
            .map(|prepared| PublicCardInterpretation::ProfessionAbility {
                player: prepared.player.clone(),
                ability_id: prepared.ability_id.clone(),
                card: policy
                    .can_see_player_hidden_cards(&prepared.player)
                    .then_some(prepared.card),
                element: prepared.element,
                level: prepared.level,
            })
            .chain(
                state
                    .spirit_level_interpretations
                    .iter()
                    .map(|interpretation| PublicCardInterpretation::SpiritSkill {
                        player: interpretation.player.clone(),
                        skill: interpretation.skill,
                        card: policy
                            .can_see_player_hidden_cards(&interpretation.player)
                            .then_some(interpretation.card),
                        level: interpretation.level,
                    }),
            )
            .collect(),
        spirits: if state.has_rule_module(crate::domain::SPIRIT_MODULE_ID) {
            state.spirits.clone()
        } else {
            Vec::new()
        },
        previous_turn_formation,
    }
}

fn public_flow_states(state: &GameState) -> Vec<PublicFlowState> {
    let mut flows = state
        .flow_layers_by_player
        .iter()
        .map(|(player, layers)| PublicFlowState {
            player: player.clone(),
            layers: *layers,
        })
        .collect::<Vec<_>>();
    flows.sort_by(|left, right| left.player.cmp(&right.player));
    flows
}

pub fn event_for(event: &GameEvent, viewer: Viewer) -> PublicGameEvent {
    let policy = RedactionPolicy::new(viewer);
    match event {
        GameEvent::GamePreparationStarted { .. } => PublicGameEvent::GamePreparationStarted,
        GameEvent::InitialPouchChosen { player, .. } => PublicGameEvent::InitialPouchChosen {
            player: player.clone(),
        },
        GameEvent::PouchPlaced {
            owner,
            card,
            known_by,
            ..
        } => PublicGameEvent::PouchPlaced {
            owner: owner.clone(),
            card: match &policy.viewer {
                Viewer::Player(viewer) if known_by.contains(viewer) => Some(*card),
                _ => None,
            },
        },
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
                purpose: pending_choice_purpose(kind),
                kind: if policy.can_see_player_hidden_cards(player) {
                    PublicPendingChoiceKind::Known(kind.clone())
                } else {
                    PublicPendingChoiceKind::Hidden
                },
            }
        }
        GameEvent::RandomnessRequested { request } => PublicGameEvent::RandomnessRequested {
            request_id: request.request_id.clone(),
            deck: request.deck.clone(),
            card_count: request.current_order.len(),
        },
        GameEvent::RandomnessResolved {
            request_id,
            deck,
            shuffled_order,
        } => PublicGameEvent::RandomnessResolved {
            request_id: request_id.clone(),
            deck: deck.clone(),
            card_count: shuffled_order.len(),
        },
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
        GameEvent::SpiritSkillUsed {
            player,
            spirit,
            skill,
            old_power,
            new_power,
            selected_card,
            declared_level,
        } => PublicGameEvent::SpiritSkillUsed {
            player: player.clone(),
            spirit: *spirit,
            skill: *skill,
            old_power: *old_power,
            new_power: *new_power,
            selected_card: policy
                .can_see_player_hidden_cards(player)
                .then_some(*selected_card)
                .flatten(),
            declared_level: *declared_level,
        },
        GameEvent::SpiritLevelInterpreted {
            player,
            skill,
            card,
            level,
            applied_on_turn,
            ..
        } => PublicGameEvent::SpiritLevelInterpreted {
            player: player.clone(),
            skill: *skill,
            card: policy.can_see_player_hidden_cards(player).then_some(*card),
            level: *level,
            applied_on_turn: *applied_on_turn,
        },
        GameEvent::TurnStarted { .. }
        | GameEvent::GamePreparationCompleted
        | GameEvent::PouchRevealed { .. }
        | GameEvent::PouchConsumed { .. }
        | GameEvent::PouchLevelBonusGranted { .. }
        | GameEvent::TemporaryStarEffectGranted { .. }
        | GameEvent::SpiritRevived { .. }
        | GameEvent::ActionPassed { .. }
        | GameEvent::ProfessionChanged { .. }
        | GameEvent::ProfessionTransformed { .. }
        | GameEvent::ProfessionBroken { .. }
        | GameEvent::ProfessionAbilityActivated { .. }
        | GameEvent::SpiritSummoned { .. }
        | GameEvent::SpiritTransformed { .. }
        | GameEvent::SpiritPowerChanged { .. }
        | GameEvent::SpiritBroken { .. }
        | GameEvent::AutomaticBloomsResolved { .. }
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
        | GameEvent::VoidSpiritShatteringResolved { .. }
        | GameEvent::FiveStarAlignmentAchieved { .. }
        | GameEvent::TurnDrawBonusChanged { .. }
        | GameEvent::ShieldChanged { .. }
        | GameEvent::HpChanged { .. }
        | GameEvent::CardsMoved { .. }
        | GameEvent::DeckTopRevealed { .. }
        | GameEvent::StatusAdded { .. }
        | GameEvent::StatusExpired { .. }
        | GameEvent::StatusRemoved { .. }
        | GameEvent::JianghuStateApplied { .. }
        | GameEvent::JianghuStateExpired { .. }
        | GameEvent::JianghuPoisonTicked { .. }
        | GameEvent::JianghuDelayedDamageResolved { .. }
        | GameEvent::LimitedUseChanged { .. }
        | GameEvent::ConfluenceCardObligationSet { .. }
        | GameEvent::ConfluenceCardObligationCleared { .. }
        | GameEvent::EffectChoiceAnswered { .. }
        | GameEvent::TypedEffectChoiceAnswered { .. }
        | GameEvent::EchoCostPaid { .. }
        | GameEvent::EchoDeclined { .. }
        | GameEvent::EchoScheduled { .. }
        | GameEvent::EchoResolutionStarted { .. }
        | GameEvent::EchoResolutionCompleted { .. }
        | GameEvent::TimedEffectsReduced { .. }
        | GameEvent::FlowStateChanged { .. }
        | GameEvent::FlowStateTriggered { .. }
        | GameEvent::FormationSuppressionSet { .. }
        | GameEvent::FormationSuppressionExpired { .. }
        | GameEvent::RingingMetalCardRevealed { .. }
        | GameEvent::RingingMetalCompleted { .. }
        | GameEvent::PlantEarthScheduled { .. }
        | GameEvent::PlantEarthResolutionStarted { .. }
        | GameEvent::PlantEarthResolutionCompleted { .. }
        | GameEvent::EarthRendingStarted { .. }
        | GameEvent::EarthRendingEnvironmentChosen { .. }
        | GameEvent::EarthRendingPlayerAnswered { .. }
        | GameEvent::HandRevealed { .. }
        | GameEvent::EarthRendingCompleted { .. }
        | GameEvent::RustedForestStarted { .. }
        | GameEvent::RustedForestCardsRevealed { .. }
        | GameEvent::RustedForestDeckProcessed { .. }
        | GameEvent::RustedForestCompleted { .. }
        | GameEvent::PassiveFlipped { .. }
        | GameEvent::DiscardRecycledIntoDeck { .. }
        | GameEvent::PlayerDiscardRecycledIntoDeck { .. }
        | GameEvent::DiscardRetrieved { .. }
        | GameEvent::TurnEnded { .. } => PublicGameEvent::Public(event.clone()),
    }
}

fn pending_choice_purpose(kind: &PendingChoiceKind) -> String {
    match kind {
        PendingChoiceKind::TurnDrawDiscard { .. } => "turn-draw-discard".to_string(),
        PendingChoiceKind::EffectGenerated { effect_id, .. }
        | PendingChoiceKind::CardSetChoice { effect_id, .. }
        | PendingChoiceKind::TypedEffect { effect_id, .. } => effect_id.clone(),
    }
}

fn pending_choice_presentation(kind: &PendingChoiceKind) -> PublicPendingChoicePresentation {
    let (effect_id, continuation_id) = match kind {
        PendingChoiceKind::TurnDrawDiscard { .. } => {
            return PublicPendingChoicePresentation::TurnDrawDiscard;
        }
        PendingChoiceKind::EffectGenerated {
            effect_id,
            continuation_id,
            ..
        }
        | PendingChoiceKind::CardSetChoice {
            effect_id,
            continuation_id,
            ..
        }
        | PendingChoiceKind::TypedEffect {
            effect_id,
            continuation_id,
            ..
        } => (effect_id.as_str(), continuation_id.as_str()),
    };

    use PublicEchoMelodyPresentation as Melody;
    use PublicPendingChoicePresentation as Presentation;
    match (effect_id, continuation_id) {
        ("holy-wind", _) => Presentation::HolyWind,
        ("chaos", _) => Presentation::Chaos,
        ("revelation", _) => Presentation::Revelation,
        ("jianghu:azure-cloud-step", _) => Presentation::AzureCloudStep,
        ("confluence:clear-wind-ten-thousand-miles", _) => Presentation::ClearWindTenThousandMiles,
        ("confluence:mirror-resonance", _) => Presentation::MirrorResonance,
        ("confluence:myriad-resonance", _) => Presentation::MyriadResonance,
        ("confluence:thousand-resonance", _) => Presentation::ThousandResonance,
        ("echo:ringing-metal", "echo:ringing-metal:select-card") => {
            Presentation::EchoRingingMetalDeckCard
        }
        ("echo:ringing-metal", "echo:cost") => Presentation::EchoCost {
            melody: Melody::RingingMetal,
        },
        ("echo:falling-wood", "echo:cost") => Presentation::EchoCost {
            melody: Melody::FallingWood,
        },
        ("echo:flowing-water", "echo:cost") => Presentation::EchoCost {
            melody: Melody::FlowingWater,
        },
        ("echo:war-fire", "echo:cost") => Presentation::EchoCost {
            melody: Melody::WarFire,
        },
        ("echo:split-earth", "echo:cost") => Presentation::EchoCost {
            melody: Melody::SplitEarth,
        },
        ("echo:split-earth", "echo:split-earth:formation") => Presentation::EchoSplitEarthFormation,
        ("echo:pure-fire", _) => Presentation::EchoPureFirePlayer,
        ("echo:plant-earth", _) => Presentation::EchoPlantEarthMelody,
        ("tribulation:earth-rending", "tribulation:earth-rending:environment") => {
            Presentation::EarthRendingEnvironment
        }
        ("tribulation:earth-rending", "tribulation:earth-rending:card") => {
            Presentation::EarthRendingCard
        }
        ("metamorphosis", _) => Presentation::Metamorphosis,
        ("choose-card-to-seal" | "seal", _) => Presentation::SealCard,
        _ => Presentation::Unclassified,
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_official_pending_choice_path_has_a_typed_presentation() {
        let paths = [
            ("holy-wind", "any"),
            ("chaos", "chaos:return-two"),
            ("revelation", "any"),
            ("jianghu:azure-cloud-step", "any"),
            ("confluence:clear-wind-ten-thousand-miles", "any"),
            ("confluence:mirror-resonance", "any"),
            ("confluence:myriad-resonance", "any"),
            ("confluence:thousand-resonance", "any"),
            ("echo:ringing-metal", "echo:ringing-metal:select-card"),
            ("echo:ringing-metal", "echo:cost"),
            ("echo:falling-wood", "echo:cost"),
            ("echo:flowing-water", "echo:cost"),
            ("echo:war-fire", "echo:cost"),
            ("echo:split-earth", "echo:cost"),
            ("echo:split-earth", "echo:split-earth:formation"),
            ("echo:pure-fire", "any"),
            ("echo:plant-earth", "any"),
            (
                "tribulation:earth-rending",
                "tribulation:earth-rending:environment",
            ),
            (
                "tribulation:earth-rending",
                "tribulation:earth-rending:card",
            ),
            ("metamorphosis", "any"),
            ("seal", "any"),
        ];

        assert_eq!(paths.len() + 1, 22);
        assert_eq!(
            pending_choice_presentation(&PendingChoiceKind::TurnDrawDiscard {
                drawn_cards: vec![],
                allowed_discards: vec![],
            }),
            PublicPendingChoicePresentation::TurnDrawDiscard
        );
        for (effect_id, continuation_id) in paths {
            let presentation = pending_choice_presentation(&PendingChoiceKind::EffectGenerated {
                effect_id: effect_id.to_string(),
                continuation_id: continuation_id.to_string(),
                allowed_cards: vec![],
            });
            assert_ne!(presentation, PublicPendingChoicePresentation::Unclassified);
            assert!(!serde_json::to_string(&presentation).unwrap().is_empty());
        }
    }
}
