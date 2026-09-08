//! 從標準遊戲資料衍生經檢視者過濾的公開視圖。

use crate::domain::{
    CardInstanceId, ChoiceId, CounterEffect, Element, FormationSuppression, GameEvent, GameState,
    GameStatus, JianghuState, LimitedUse, PendingChoice, PendingChoiceKind, PendingResolution,
    Phase, Player, PlayerId, PlayerProfession, PlayerShield, PlayerStarHistory, RandomnessDeck,
    RandomnessOperation, RuleModuleId, ScheduledEcho, ScheduledPlantEarth, StatusEffect, TeamHp,
    TeamStar,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Viewer {
    Player(PlayerId),
    Observer,
    /// 已完成對局的唯讀檢視器。它可以查看玩家擁有的歷史資訊，但永遠不能
    /// 查看牌堆順序或受信任的隨機性答案。
    Replay,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicGameState {
    pub rule_version: crate::domain::RuleVersion,
    pub enabled_rule_modules: Vec<RuleModuleId>,
    pub status: GameStatus,
    pub turn_number: u64,
    pub phase: Phase,
    pub current_player: Option<PlayerId>,
    pub players: Vec<Player>,
    pub turn_order: Vec<PlayerId>,
    pub hp: Vec<TeamHp>,
    pub hands: Vec<PublicPlayerHand>,
    pub deck_count: usize,
    pub discard: Vec<CardInstanceId>,
    pub player_decks: Vec<PublicPlayerDeck>,
    pub player_discards: Vec<PublicPlayerDiscard>,
    pub pouches: Vec<PublicPouch>,
    pub initial_pouch_selection: Option<PublicInitialPouchSelection>,
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
    pub totems: Vec<crate::domain::PlayerTotem>,
    pub previous_turn_formation: Option<PublicPreviousTurnFormation>,
    pub last_completed_turn_discards: Vec<PublicLastCompletedTurnDiscard>,
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

/// 共用初始袋牌選擇階段的公開進度。此處只攜帶完成狀態；選取的卡牌仍留在
/// 一般袋牌隱私邊界內。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PublicInitialPouchSelection {
    pub remaining_players: Vec<PlayerId>,
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

/// 每位玩家最近一個已完成回合的 Turn Draw 棄牌。`player` 是執行該回合的
/// 玩家；卡牌的不可變來源不會改變這個回合歸屬。
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicLastCompletedTurnDiscard {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub turn_number: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicCardRefs {
    Known(Vec<CardInstanceId>),
    Hidden { count: usize },
    PartiallyKnown { cards: Vec<Option<CardInstanceId>> },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(
    tag = "visibility",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PublicPendingChoice {
    Visible {
        choice_id: ChoiceId,
        player: PlayerId,
        reason: PublicPendingChoicePresentation,
        choice: PendingChoiceKind,
    },
    Hidden {
        player: PlayerId,
        reason: PublicPendingChoicePresentation,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PublicPendingChoicePresentation {
    TurnDrawDiscard,
    HolyWind,
    Chaos,
    Revelation,
    AzureCloudStep,
    ClearWind,
    ClearWindTenThousandMiles,
    MirrorResonance,
    MyriadResonance,
    ThousandResonance,
    EchoRingingMetalDeckCard,
    SouthSpiritArrayElement,
    CentralSpiritArrayCard,
    DragonSearchDeckCard,
    EchoCost {
        melody: PublicEchoMelodyPresentation,
    },
    EchoSplitEarthFormation,
    EchoPureFirePlayer,
    EchoPlantEarthMelody,
    EarthRendingEnvironment,
    EarthRendingCard,
    Chain,
    SheepStealing,
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
#[serde(rename_all = "camelCase")]
pub struct PublicPendingRandomness {
    pub request_id: String,
    pub deck: RandomnessDeck,
    pub operation: PublicRandomnessOperation,
    pub card_count: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PublicRandomnessOperation {
    DeckShuffle,
    DiscardShuffle,
}

fn public_randomness_operation(operation: &RandomnessOperation) -> PublicRandomnessOperation {
    match operation {
        RandomnessOperation::DeckShuffle { .. } => PublicRandomnessOperation::DeckShuffle,
        RandomnessOperation::DiscardShuffle { .. } => PublicRandomnessOperation::DiscardShuffle,
    }
}

// 以值保留標準事件負載，讓公開投影維持記錄的透明且零配置視圖。
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PublicGameEvent {
    Public(GameEvent),
    GamePreparationStarted,
    InitialPouchChosen {
        player: PlayerId,
    },
    InitialPouchSelectionCompleted,
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
    FormationCommitted {
        player: PlayerId,
        formation_id: Option<String>,
        cards: PublicCardRefs,
    },
    CardsDrawnForTurnDiscardChoice {
        player: PlayerId,
        drawn_cards: PublicCardRefs,
        allowed_discards: PublicCardRefs,
    },
    TurnDrawResolved {
        player: PlayerId,
        discard: CardInstanceId,
        kept_cards: PublicCardRefs,
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
    CardsMoved {
        cards: PublicCardRefs,
    },
    FormationRequirementSet {
        player: PlayerId,
        virtual_card: Option<crate::domain::VirtualFormationCard>,
    },
    FormationRequirementFulfilled {
        player: PlayerId,
        formation_id: String,
        virtual_card: Option<crate::domain::VirtualFormationCard>,
    },
    ChoiceRequested {
        choice: PublicPendingChoice,
    },
    ChoiceMade {
        player: PlayerId,
    },
    RandomnessRequested {
        request_id: String,
        deck: RandomnessDeck,
        operation: PublicRandomnessOperation,
        card_count: usize,
    },
    RandomnessResolved {
        request_id: String,
        deck: RandomnessDeck,
        operation: PublicRandomnessOperation,
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
            let remains_covered = state.covered_passive(player).is_some_and(|passive| {
                passive.formation_id == formation.formation_id
                    && passive.cards == formation.used_cards
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
    let initial_pouch_selection = matches!(
        &state.status,
        GameStatus::Preparing {
            stage: crate::domain::GamePreparationStage::InitialPouchSelection,
        }
    )
    .then(|| PublicInitialPouchSelection {
        remaining_players: state
            .turn_order
            .iter()
            .filter(|player| state.pouch_for(player).is_none())
            .cloned()
            .collect(),
    });
    let last_completed_turn_discards = public_last_completed_turn_discards(state);

    PublicGameState {
        rule_version: state.rule_version,
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
        deck_count: state.deck.len(),
        discard: state.discard.clone(),
        player_decks: state
            .player_decks
            .iter()
            .filter(|_| uses_personal_decks)
            .map(|pile| {
                // 回放特意揭露玩家區域，但仍將每個牌堆保留為無順序、只有數量的牌堆。
                let visible = !matches!(&policy.viewer, Viewer::Replay)
                    && state.has_rule_module(crate::domain::POUCH_MODULE_ID)
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
                    Viewer::Replay => Some(pouch.card),
                    Viewer::Player(viewer) if pouch.known_by.contains(viewer) => Some(pouch.card),
                    _ => None,
                },
            })
            .collect(),
        initial_pouch_selection,
        covered_passives: state
            .formation_areas
            .iter()
            .filter_map(|area| {
                let passive = area.formation.as_ref()?;
                let crate::domain::FormationAreaState::FaceDownWaiting { revealed, .. } =
                    passive.state
                else {
                    return None;
                };
                let visible = policy.can_see_player_hidden_cards(&area.player) || revealed;
                Some(PublicCoveredPassive {
                    owner: area.player.clone(),
                    formation_id: visible.then(|| passive.formation_id.clone()),
                    cards: if visible {
                        PublicCardRefs::Known(passive.cards.clone())
                    } else {
                        PublicCardRefs::Hidden {
                            count: passive.cards.len(),
                        }
                    },
                    star_substitution: visible.then(|| passive.star_substitution.clone()).flatten(),
                })
            })
            .collect(),
        counter_effects: state.counter_effects.clone(),
        pending_choice: state.pending_choice.as_ref().map(|choice| {
            public_pending_choice(
                choice,
                state
                    .pending_resolution
                    .as_ref()
                    .expect("pending choice must have a pending resolution"),
                &policy,
            )
        }),
        pending_randomness: state.pending_randomness.as_ref().map(|request| {
            PublicPendingRandomness {
                request_id: request.request_id.clone(),
                deck: request.operation.destination_deck().clone(),
                operation: public_randomness_operation(&request.operation),
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
                level: prepared.level.value(),
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
                        level: interpretation.level.value(),
                    }),
            )
            .collect(),
        spirits: if state.has_rule_module(crate::domain::SPIRIT_MODULE_ID) {
            state.spirits.clone()
        } else {
            Vec::new()
        },
        totems: state.totems.clone(),
        previous_turn_formation,
        last_completed_turn_discards,
    }
}

fn public_last_completed_turn_discards(state: &GameState) -> Vec<PublicLastCompletedTurnDiscard> {
    let player_count = state.turn_order.len() as u64;
    if player_count == 0 {
        return Vec::new();
    }

    state
        .turn_order
        .iter()
        .enumerate()
        .filter_map(|(index, player)| {
            // 當前玩家的本回合尚未完成；其他玩家則依座次回推至最近完成的回合。
            let distance = (state.current_turn_index + state.turn_order.len() - index)
                % state.turn_order.len();
            let completed_turn = state.turn_number.checked_sub(if distance == 0 {
                player_count
            } else {
                distance as u64
            })?;
            let discard = state.last_turn_discard_by_player.get(player)?;
            (discard.turn_number == completed_turn).then(|| PublicLastCompletedTurnDiscard {
                player: player.clone(),
                card: discard.card,
                turn_number: discard.turn_number,
            })
        })
        .collect()
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
        GameEvent::InitialPouchSelectionCompleted => {
            PublicGameEvent::InitialPouchSelectionCompleted
        }
        GameEvent::PouchPlaced {
            owner,
            card,
            known_by,
            ..
        } => PublicGameEvent::PouchPlaced {
            owner: owner.clone(),
            card: match &policy.viewer {
                Viewer::Replay => Some(*card),
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
            ..
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
        GameEvent::FormationCommitted {
            player,
            formation_id,
            cards,
            state,
            ..
        } => {
            let hidden = matches!(
                state,
                crate::domain::FormationAreaState::FaceDownResolving
                    | crate::domain::FormationAreaState::FaceDownWaiting { .. }
            ) && !policy.can_see_player_hidden_cards(player);
            PublicGameEvent::FormationCommitted {
                player: player.clone(),
                formation_id: (!hidden).then(|| formation_id.clone()),
                cards: if hidden {
                    PublicCardRefs::Hidden { count: cards.len() }
                } else {
                    PublicCardRefs::Known(cards.clone())
                },
            }
        }
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
        GameEvent::TurnDrawResolved {
            player,
            discard,
            kept_cards,
        } => PublicGameEvent::TurnDrawResolved {
            player: player.clone(),
            discard: *discard,
            kept_cards: if policy.can_see_player_hidden_cards(player) {
                PublicCardRefs::Known(kept_cards.clone())
            } else {
                PublicCardRefs::Hidden {
                    count: kept_cards.len(),
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
        GameEvent::ChoiceRequested { choice, resolution } => PublicGameEvent::ChoiceRequested {
            choice: public_pending_choice(choice, resolution, &policy),
        },
        GameEvent::ChoiceMade { player, .. } => PublicGameEvent::ChoiceMade {
            player: player.clone(),
        },
        GameEvent::RandomnessRequested { request, .. } => PublicGameEvent::RandomnessRequested {
            request_id: request.request_id.clone(),
            deck: request.operation.destination_deck().clone(),
            operation: public_randomness_operation(&request.operation),
            card_count: request.current_order.len(),
        },
        GameEvent::RandomnessResolved {
            request_id,
            operation,
            shuffled_order,
        } => PublicGameEvent::RandomnessResolved {
            request_id: request_id.clone(),
            deck: operation.destination_deck().clone(),
            operation: public_randomness_operation(operation),
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
            level: level.value(),
            applied_on_turn: *applied_on_turn,
        },
        GameEvent::CardsMoved { card_moves } => PublicGameEvent::CardsMoved {
            cards: public_moved_cards(card_moves, &policy),
        },
        GameEvent::FormationRequirementSet { requirement } => {
            PublicGameEvent::FormationRequirementSet {
                player: requirement.player.clone(),
                // 暗靈目標在一般卡牌移動揭露前仍保持隱藏；虛擬事實在建立時即為公開。
                virtual_card: requirement.virtual_card.clone(),
            }
        }
        GameEvent::FormationRequirementFulfilled {
            player,
            formation_id,
            composition,
        } => PublicGameEvent::FormationRequirementFulfilled {
            player: player.clone(),
            formation_id: formation_id.clone(),
            virtual_card: composition.virtual_card.clone(),
        },
        GameEvent::TurnStarted { .. }
        | GameEvent::GamePreparationCompleted
        | GameEvent::PouchRevealed { .. }
        | GameEvent::PouchConsumed { .. }
        | GameEvent::PouchLevelBonusGranted { .. }
        | GameEvent::TemporaryStarEffectGranted { .. }
        | GameEvent::SpiritRevived { .. }
        | GameEvent::ActionStarted { .. }
        | GameEvent::ActionPassed { .. }
        | GameEvent::FormationCardsDiscarded { .. }
        | GameEvent::ProfessionChanged { .. }
        | GameEvent::ProfessionTransformed { .. }
        | GameEvent::ProfessionBroken { .. }
        | GameEvent::ProfessionAbilityActivated { .. }
        | GameEvent::SpiritSummoned { .. }
        | GameEvent::TotemChanged { .. }
        | GameEvent::DragonSearchRevealed { .. }
        | GameEvent::DragonSearchCompleted { .. }
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
        | GameEvent::KingYamaDecreeVictoryAchieved { .. }
        | GameEvent::GameEnded { .. }
        | GameEvent::TurnDrawBonusChanged { .. }
        | GameEvent::ShieldChanged { .. }
        | GameEvent::HpChanged { .. }
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
        | GameEvent::DiscardRetrieved { .. }
        | GameEvent::TurnEnded { .. } => PublicGameEvent::Public(event.clone()),
    }
}

fn public_moved_cards(
    card_moves: &[crate::domain::CardMoveDelta],
    policy: &RedactionPolicy,
) -> PublicCardRefs {
    let cards = card_moves
        .iter()
        .map(|movement| {
            movement_card_is_public(movement)
                .then_some(movement.card)
                .or_else(|| {
                    movement_zone_owner(&movement.from)
                        .into_iter()
                        .chain(movement_zone_owner(&movement.to))
                        .any(|player| policy.can_see_player_hidden_cards(player))
                        .then_some(movement.card)
                })
        })
        .collect::<Vec<_>>();
    let known = cards.iter().flatten().count();
    if known == cards.len() {
        PublicCardRefs::Known(cards.into_iter().flatten().collect())
    } else if known == 0 {
        PublicCardRefs::Hidden { count: cards.len() }
    } else {
        PublicCardRefs::PartiallyKnown { cards }
    }
}

fn movement_card_is_public(movement: &crate::domain::CardMoveDelta) -> bool {
    matches!(
        movement.from,
        crate::domain::CardZone::Discard | crate::domain::CardZone::PlayerDiscard(_)
    ) || matches!(
        movement.to,
        crate::domain::CardZone::Discard | crate::domain::CardZone::PlayerDiscard(_)
    )
}

fn movement_zone_owner(zone: &crate::domain::CardZone) -> Option<&PlayerId> {
    match zone {
        crate::domain::CardZone::Hand(player)
        | crate::domain::CardZone::PlayerDeckTop(player)
        | crate::domain::CardZone::Pouch(player) => Some(player),
        crate::domain::CardZone::DeckTop
        | crate::domain::CardZone::Discard
        | crate::domain::CardZone::PlayerDiscard(_) => None,
    }
}

fn public_pending_choice(
    choice: &PendingChoice,
    resolution: &PendingResolution,
    policy: &RedactionPolicy,
) -> PublicPendingChoice {
    let reason = pending_choice_presentation(resolution);
    if policy.can_see_player_hidden_cards(&choice.player) {
        PublicPendingChoice::Visible {
            choice_id: choice.choice_id,
            player: choice.player.clone(),
            reason,
            choice: choice.kind.clone(),
        }
    } else {
        PublicPendingChoice::Hidden {
            player: choice.player.clone(),
            reason,
        }
    }
}

fn pending_choice_presentation(resolution: &PendingResolution) -> PublicPendingChoicePresentation {
    use PublicEchoMelodyPresentation as Melody;
    use PublicPendingChoicePresentation as Presentation;
    match resolution {
        PendingResolution::TurnDrawDiscard => Presentation::TurnDrawDiscard,
        PendingResolution::HolyWindTakeHighest => Presentation::HolyWind,
        PendingResolution::SouthSpiritArrayElement { .. } => Presentation::SouthSpiritArrayElement,
        PendingResolution::CentralSpiritArrayCard => Presentation::CentralSpiritArrayCard,
        PendingResolution::DragonSearchDeckCard => Presentation::DragonSearchDeckCard,
        PendingResolution::ChaosReturnTwo => Presentation::Chaos,
        PendingResolution::HeroRevelationKeepOne => Presentation::Revelation,
        PendingResolution::JianghuAzureCloudStepReturnOne => Presentation::AzureCloudStep,
        PendingResolution::ConfluenceClearWindDiscardTop => Presentation::ClearWind,
        PendingResolution::ConfluenceClearWindKeepCards => Presentation::ClearWindTenThousandMiles,
        PendingResolution::ConfluenceDiscardInspectedCard { resonance, .. } => match resonance {
            crate::domain::ConfluenceResonance::Mirror => Presentation::MirrorResonance,
            crate::domain::ConfluenceResonance::Myriad => Presentation::MyriadResonance,
            crate::domain::ConfluenceResonance::Thousand => Presentation::ThousandResonance,
        },
        PendingResolution::MelodyRingingMetalDeckCard { .. } => {
            Presentation::EchoRingingMetalDeckCard
        }
        PendingResolution::MelodyCost { melody_id, .. } => {
            let melody = match melody_id.as_str() {
                "echo:ringing-metal" => Melody::RingingMetal,
                "echo:falling-wood" => Melody::FallingWood,
                "echo:flowing-water" => Melody::FlowingWater,
                "echo:war-fire" => Melody::WarFire,
                _ => Melody::SplitEarth,
            };
            Presentation::EchoCost { melody }
        }
        PendingResolution::MelodySplitEarthFormation { .. } => {
            Presentation::EchoSplitEarthFormation
        }
        PendingResolution::MelodyPureFireTarget { .. } => Presentation::EchoPureFirePlayer,
        PendingResolution::MelodyPlantEarthMelody { .. } => Presentation::EchoPlantEarthMelody,
        PendingResolution::TribulationEarthRendingEnvironment => {
            Presentation::EarthRendingEnvironment
        }
        PendingResolution::TribulationEarthRendingCard => Presentation::EarthRendingCard,
        PendingResolution::PouchChain => Presentation::Chain,
        PendingResolution::PouchSheepStealingChoice => Presentation::SheepStealing,
        _ => unreachable!("non-choice resolution cannot own a Pending Choice"),
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
        matches!(&self.viewer, Viewer::Replay)
            || matches!(&self.viewer, Viewer::Player(viewer) if viewer == player)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dragon_search_reveal_is_public_but_does_not_expose_a_later_hand() {
        let alice = PlayerId::new("alice");
        let card = CardInstanceId::new(1);
        let reveal = GameEvent::DragonSearchRevealed {
            player: alice.clone(),
            card,
        };
        assert_eq!(
            event_for(&reveal, Viewer::Observer),
            PublicGameEvent::Public(reveal.clone())
        );
        let mut state = GameState::from_setup(&crate::domain::GameSetup::two_player(
            alice.clone(),
            PlayerId::new("bob"),
            30,
        ));
        state.hands[0].cards = vec![card];
        assert_eq!(
            state_for(&state, Viewer::Observer).hands[0].cards,
            PublicCardRefs::Hidden { count: 1 }
        );
        assert_eq!(
            state_for(&state, Viewer::Player(alice)).hands[0].cards,
            PublicCardRefs::Known(vec![card])
        );
    }

    #[test]
    fn cover_time_environment_ground_is_hidden_until_passive_flip() {
        let alice = PlayerId::new("alice");
        let covered = GameEvent::PassiveCovered {
            player: alice,
            formation_id: "defense".to_string(),
            cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
            star_substitution: None,
            sealed: false,
            ineffective_environment: Some(Element::Metal),
        };
        let public = event_for(&covered, Viewer::Observer);
        assert!(matches!(
            public,
            PublicGameEvent::PassiveCovered {
                formation_id: None,
                cards: PublicCardRefs::Hidden { count: 2 },
                ..
            }
        ));
        let json = serde_json::to_string(&public).unwrap();
        assert!(!json.contains("Metal"));
        assert!(!json.contains("defense"));
    }

    #[test]
    fn replay_viewer_reveals_player_areas_but_keeps_decks_count_only() {
        let alice = PlayerId::new("alice");
        let mut state = GameState::from_setup(&crate::domain::GameSetup::two_player(
            alice.clone(),
            PlayerId::new("bob"),
            30,
        ));
        state
            .enabled_rule_modules
            .push(crate::domain::RuleModuleId::new(
                crate::domain::PERSONAL_DECK_MODULE_ID,
            ));
        state.hands[0].cards = vec![CardInstanceId::new(1)];
        state.player_decks = vec![crate::domain::PlayerCardPile {
            player: alice.clone(),
            cards: vec![CardInstanceId::new(2), CardInstanceId::new(3)],
        }];

        let view = state_for(&state, Viewer::Replay);
        assert_eq!(
            view.hands[0].cards,
            PublicCardRefs::Known(vec![CardInstanceId::new(1)])
        );
        assert_eq!(
            view.player_decks[0].cards,
            PublicCardRefs::Hidden { count: 2 }
        );
    }

    #[test]
    fn card_movements_reveal_public_discards_without_leaking_hidden_returns() {
        let revealed = GameEvent::CardsMoved {
            card_moves: vec![crate::domain::CardMoveDelta {
                card: CardInstanceId::new(7),
                from: crate::domain::CardZone::DeckTop,
                to: crate::domain::CardZone::Discard,
            }],
        };
        assert_eq!(
            event_for(&revealed, Viewer::Observer),
            PublicGameEvent::CardsMoved {
                cards: PublicCardRefs::Known(vec![CardInstanceId::new(7)]),
            }
        );

        let alice = PlayerId::new("alice");
        let hidden = GameEvent::CardsMoved {
            card_moves: vec![crate::domain::CardMoveDelta {
                card: CardInstanceId::new(8),
                from: crate::domain::CardZone::Hand(alice.clone()),
                to: crate::domain::CardZone::DeckTop,
            }],
        };
        assert_eq!(
            event_for(&hidden, Viewer::Observer),
            PublicGameEvent::CardsMoved {
                cards: PublicCardRefs::Hidden { count: 1 },
            }
        );
        assert_eq!(
            event_for(&hidden, Viewer::Player(alice)),
            PublicGameEvent::CardsMoved {
                cards: PublicCardRefs::Known(vec![CardInstanceId::new(8)]),
            }
        );
    }

    #[test]
    fn initial_pouch_progress_is_public_without_the_selected_card_identity() {
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let mut state = GameState::from_setup(&crate::domain::GameSetup::two_player(
            alice.clone(),
            bob.clone(),
            30,
        ));
        state.status = GameStatus::Preparing {
            stage: crate::domain::GamePreparationStage::InitialPouchSelection,
        };
        state.pouches.push(crate::domain::PlayerPouch {
            owner: alice.clone(),
            card: CardInstanceId::new(7),
            known_by: vec![alice],
        });

        let observer = state_for(&state, Viewer::Observer);
        assert_eq!(
            observer.initial_pouch_selection,
            Some(PublicInitialPouchSelection {
                remaining_players: vec![bob],
            }),
        );
        assert_eq!(observer.pouches[0].card, None);
        assert_eq!(
            serde_json::to_value(event_for(
                &GameEvent::InitialPouchSelectionCompleted,
                Viewer::Observer,
            ))
            .unwrap(),
            serde_json::json!({"type": "initialPouchSelectionCompleted"}),
        );
    }

    #[test]
    fn formation_commit_and_turn_draw_resolution_keep_owner_cards_private() {
        let alice = PlayerId::new("alice");
        let commit = GameEvent::FormationCommitted {
            player: alice.clone(),
            formation_id: "defense".to_string(),
            cards: vec![CardInstanceId::new(8)],
            star_substitution: None,
            state: crate::domain::FormationAreaState::FaceDownResolving,
        };
        assert_eq!(
            event_for(&commit, Viewer::Observer),
            PublicGameEvent::FormationCommitted {
                player: alice.clone(),
                formation_id: None,
                cards: PublicCardRefs::Hidden { count: 1 },
            }
        );

        let draw = GameEvent::TurnDrawResolved {
            player: alice,
            discard: CardInstanceId::new(1),
            kept_cards: vec![CardInstanceId::new(2), CardInstanceId::new(3)],
        };
        assert_eq!(
            event_for(&draw, Viewer::Observer),
            PublicGameEvent::TurnDrawResolved {
                player: PlayerId::new("alice"),
                discard: CardInstanceId::new(1),
                kept_cards: PublicCardRefs::Hidden { count: 2 },
            }
        );
        let json = serde_json::to_value(event_for(&draw, Viewer::Observer)).unwrap();
        assert_eq!(json["type"], "turnDrawResolved");
        assert_eq!(json["keptCards"]["Hidden"]["count"], 2);
        assert!(json.get("kept_cards").is_none());
    }

    #[test]
    fn last_completed_turn_discards_are_turn_ordered_and_keep_the_turn_actor() {
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let mut state = GameState::from_setup(&crate::domain::GameSetup::two_player(
            alice.clone(),
            bob.clone(),
            30,
        ));
        state
            .enabled_rule_modules
            .push(crate::domain::RuleModuleId::new(
                crate::domain::PERSONAL_DECK_MODULE_ID,
            ));
        state.card_instances.push(crate::domain::CardInstanceDef {
            instance: CardInstanceId::new(7),
            definition: crate::domain::CardDefId::new("foreign-origin"),
            origin: crate::domain::CardOrigin::Player(bob.clone()),
        });
        state.turn_number = 6;
        state.current_turn_index = 0;
        state.last_turn_discard_by_player.insert(
            alice.clone(),
            crate::domain::LastTurnDiscard {
                // 此牌來源是 bob，但回合棄牌者仍是 alice。
                card: CardInstanceId::new(7),
                turn_number: 4,
            },
        );
        state.last_turn_discard_by_player.insert(
            bob.clone(),
            crate::domain::LastTurnDiscard {
                card: CardInstanceId::new(8),
                turn_number: 5,
            },
        );

        let view = state_for(&state, Viewer::Observer);
        assert_eq!(
            view.last_completed_turn_discards,
            vec![
                PublicLastCompletedTurnDiscard {
                    player: alice,
                    card: CardInstanceId::new(7),
                    turn_number: 4,
                },
                PublicLastCompletedTurnDiscard {
                    player: bob,
                    card: CardInstanceId::new(8),
                    turn_number: 5,
                },
            ]
        );
    }

    #[test]
    fn skipped_turn_does_not_project_a_stale_last_turn_discard() {
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let mut state = GameState::from_setup(&crate::domain::GameSetup::two_player(
            alice.clone(),
            bob.clone(),
            30,
        ));
        state.turn_number = 6;
        state.current_turn_index = 0;
        // alice 在第 4 回合跳過 Turn Draw，因此僅剩更早的舊記錄。
        state.last_turn_discard_by_player.insert(
            alice,
            crate::domain::LastTurnDiscard {
                card: CardInstanceId::new(7),
                turn_number: 2,
            },
        );
        state.last_turn_discard_by_player.insert(
            bob.clone(),
            crate::domain::LastTurnDiscard {
                card: CardInstanceId::new(8),
                turn_number: 5,
            },
        );

        let live = state_for(&state, Viewer::Observer);
        let replay = state_for(&state, Viewer::Replay);
        assert_eq!(
            live.last_completed_turn_discards,
            replay.last_completed_turn_discards
        );
        assert_eq!(
            live.last_completed_turn_discards,
            vec![PublicLastCompletedTurnDiscard {
                player: bob,
                card: CardInstanceId::new(8),
                turn_number: 5,
            }]
        );
    }

    #[test]
    fn every_official_pending_choice_path_has_a_typed_presentation() {
        use crate::domain::{ConfluenceResonance, PendingResolution};

        let paths = vec![
            PendingResolution::TurnDrawDiscard,
            PendingResolution::SouthSpiritArrayElement {
                damage_prevented: false,
                split_attack_damage: false,
            },
            PendingResolution::CentralSpiritArrayCard,
            PendingResolution::DragonSearchDeckCard,
            PendingResolution::HolyWindTakeHighest,
            PendingResolution::ChaosReturnTwo,
            PendingResolution::HeroRevelationKeepOne,
            PendingResolution::JianghuAzureCloudStepReturnOne,
            PendingResolution::ConfluenceClearWindDiscardTop,
            PendingResolution::ConfluenceClearWindKeepCards,
            PendingResolution::ConfluenceDiscardInspectedCard {
                resonance: ConfluenceResonance::Mirror,
                after: None,
            },
            PendingResolution::ConfluenceDiscardInspectedCard {
                resonance: ConfluenceResonance::Myriad,
                after: None,
            },
            PendingResolution::ConfluenceDiscardInspectedCard {
                resonance: ConfluenceResonance::Thousand,
                after: Some(Element::Fire),
            },
            PendingResolution::MelodyRingingMetalDeckCard {
                origin: crate::domain::MelodyExecutionOrigin::Echo,
            },
            PendingResolution::MelodyCost {
                melody_id: "echo:ringing-metal".to_string(),
                origin: crate::domain::MelodyExecutionOrigin::FormationUse,
            },
            PendingResolution::MelodySplitEarthFormation {
                origin: crate::domain::MelodyExecutionOrigin::Echo,
            },
            PendingResolution::MelodyPureFireTarget {
                origin: crate::domain::MelodyExecutionOrigin::Echo,
            },
            PendingResolution::MelodyPlantEarthMelody {
                origin: crate::domain::MelodyExecutionOrigin::PlantedEarth,
            },
            PendingResolution::TribulationEarthRendingEnvironment,
            PendingResolution::TribulationEarthRendingCard,
            PendingResolution::PouchChain,
            PendingResolution::PouchSheepStealingChoice,
        ];

        for resolution in paths {
            let presentation = pending_choice_presentation(&resolution);
            assert_ne!(presentation, PublicPendingChoicePresentation::Unclassified);
            assert!(!serde_json::to_string(&presentation).unwrap().is_empty());
        }
    }
}
