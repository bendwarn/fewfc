use crate::domain::targeting::{RulePlayerTarget, TurnOrderTargets};
use crate::domain::{
    CardMoveDelta, CardOrigin, CardZone, ChoiceAnswer, ChoiceContinuation, ChoiceRequest,
    ECHO_MODULE_ID, EchoChoiceContinuation, EchoRandomnessContinuation, Element, GameError,
    GameEvent, GameResult, GameState, GameStatus, PendingChoiceKind, PlayerId,
    RandomnessContinuation, ScheduledEcho, ValidationError,
};
use crate::rules::{
    BaseFormationSpec, EffectDef, EffectPlan, FormationCategory, FormationDef, FormationPattern,
    PointFormula, SpellPlanDef, SubmittedCardFacts,
};

pub(crate) const RINGING_METAL: &str = "echo:ringing-metal";
pub(crate) const FALLING_WOOD: &str = "echo:falling-wood";
pub(crate) const FLOWING_WATER: &str = "echo:flowing-water";
pub(crate) const WAR_FIRE: &str = "echo:war-fire";
pub(crate) const SPLIT_EARTH: &str = "echo:split-earth";
pub(crate) const PURE_FIRE: &str = "echo:pure-fire";
pub(crate) const PLANT_EARTH: &str = "echo:plant-earth";

pub(crate) fn matches_pure_fire(submitted: &[SubmittedCardFacts]) -> bool {
    submitted.len() == 2
        && submitted.iter().any(|card| card.element == Element::Fire)
        && submitted.iter().any(|card| card.element == Element::Water)
        && submitted.iter().map(|card| card.level).sum::<u32>() >= 7
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EchoPolicy {
    OptionalCost {
        allowed_printed_elements: [Element; 2],
    },
    Automatic,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum MelodyExecutionOrigin {
    FormationUse,
    Echo,
    PlantedEarth,
}

impl MelodyExecutionOrigin {
    pub(crate) fn can_schedule_echo(self) -> bool {
        matches!(self, Self::FormationUse)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct MelodyDef {
    pub id: &'static str,
    pub name: &'static str,
    pub element: Element,
    pub custom_pattern: Option<&'static str>,
    pub echo_policy: EchoPolicy,
}

pub(crate) fn melody_catalog() -> Vec<MelodyDef> {
    debug_assert_eq!(ECHO_MODULE_ID, "echo");
    debug_assert!(MelodyExecutionOrigin::FormationUse.can_schedule_echo());
    debug_assert!(!MelodyExecutionOrigin::Echo.can_schedule_echo());
    debug_assert!(!MelodyExecutionOrigin::PlantedEarth.can_schedule_echo());
    let _supported_future_policies = [EchoPolicy::Automatic, EchoPolicy::None];
    vec![
        melody_definition(
            RINGING_METAL,
            "商調‧鳴金",
            Element::Metal,
            [Element::Metal, Element::Earth],
        ),
        MelodyDef {
            id: PURE_FIRE,
            name: "變徵‧淨火",
            element: Element::Fire,
            custom_pattern: Some(PURE_FIRE),
            echo_policy: EchoPolicy::Automatic,
        },
        MelodyDef {
            id: PLANT_EARTH,
            name: "變宮‧植土",
            element: Element::Earth,
            custom_pattern: Some(PLANT_EARTH),
            echo_policy: EchoPolicy::None,
        },
        melody_definition(
            FALLING_WOOD,
            "角調‧落木",
            Element::Wood,
            [Element::Wood, Element::Water],
        ),
        melody_definition(
            FLOWING_WATER,
            "羽調‧流水",
            Element::Water,
            [Element::Water, Element::Metal],
        ),
        melody_definition(
            WAR_FIRE,
            "徵調‧戰火",
            Element::Fire,
            [Element::Fire, Element::Wood],
        ),
        melody_definition(
            SPLIT_EARTH,
            "宮調‧裂土",
            Element::Earth,
            [Element::Earth, Element::Fire],
        ),
    ]
}

pub(crate) fn melody(id: &str) -> Option<MelodyDef> {
    melody_catalog().into_iter().find(|melody| melody.id == id)
}

pub(crate) fn formation_specs() -> Vec<BaseFormationSpec> {
    melody_catalog()
        .into_iter()
        .map(|melody| BaseFormationSpec {
            formation: FormationDef {
                id: melody.id.to_string(),
                name: melody.name.to_string(),
                rule_text: rule_text(&melody).to_string(),
                category: FormationCategory::Spell,
                pattern: melody.custom_pattern.map_or_else(
                    || FormationPattern::ExactElements(vec![melody.element, melody.element]),
                    |pattern| FormationPattern::Custom(pattern.to_string()),
                ),
                effect_id: melody.id.to_string(),
                point_formula: PointFormula::Fixed(0),
            },
            effect: EffectDef {
                id: melody.id.to_string(),
                plan: EffectPlan::ActiveSpell(SpellPlanDef {
                    resolver_id: melody.id.to_string(),
                }),
            },
        })
        .collect()
}

fn melody_definition(
    id: &'static str,
    name: &'static str,
    element: Element,
    allowed_printed_elements: [Element; 2],
) -> MelodyDef {
    MelodyDef {
        id,
        name,
        element,
        custom_pattern: None,
        echo_policy: EchoPolicy::OptionalCost {
            allowed_printed_elements,
        },
    }
}

fn rule_text(melody: &MelodyDef) -> &'static str {
    match melody.id {
        RINGING_METAL => "金金；檢索自身牌組一張牌，展示並放到洗牌後牌組最上方",
        FALLING_WOOD => "木木；自身隊伍回復１５點生命",
        FLOWING_WATER => "水水；自身獲得一層流水狀態",
        WAR_FIRE => "火火；上家隊伍扣除１５點生命",
        SPLIT_EARTH => "土土；檢視下家手牌並選擇其下回合無效的一個陣法",
        PURE_FIRE => "火水且等級合計７以上；指定玩家的合格時效效果減少１回合或１層",
        PLANT_EARTH => "土木且等級合計７以上；下次自己回合開始選擇一種基礎曲調主效果",
        _ => unreachable!("rule text is defined for every Melody"),
    }
}

pub(crate) fn formation_main_effect_events(
    _state: &GameState,
    post_formation_state: &GameState,
    player: &PlayerId,
    melody_id: &str,
) -> GameResult<Option<Vec<GameEvent>>> {
    let Some(melody) = melody(melody_id) else {
        return Ok(None);
    };
    let mut events = main_effect_events(
        post_formation_state,
        player,
        &melody,
        MelodyExecutionOrigin::FormationUse,
    )?;
    let mut projected = post_formation_state.clone();
    for event in &events {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    if !events.iter().any(|event| {
        matches!(
            event,
            GameEvent::ChoiceRequested { .. } | GameEvent::RandomnessRequested { .. }
        )
    }) && !matches!(projected.status, GameStatus::Finished { .. })
        && melody
            .echo_policy
            .is_schedulable_from(MelodyExecutionOrigin::FormationUse)
    {
        events.push(echo_cost_choice(&projected, player, &melody)?);
    }
    Ok(Some(events))
}

pub(crate) fn answer_choice(
    state: &GameState,
    player: &PlayerId,
    continuation: &EchoChoiceContinuation,
    answer: &ChoiceAnswer,
) -> GameResult<Option<Vec<GameEvent>>> {
    match continuation {
        EchoChoiceContinuation::SplitEarthFormation => {
            let ChoiceAnswer::Formation { formation_id } = answer else {
                return Err(GameError::Validation(ValidationError::InvalidChoiceAnswer));
            };
            let target =
                TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::NextPlayer)?;
            let mut events = vec![GameEvent::FormationSuppressionSet {
                suppression: crate::domain::FormationSuppression {
                    source: player.clone(),
                    target,
                    formation_id: formation_id.clone(),
                    expires_on_turn_number: state.turn_number + 1,
                },
            }];
            if let Some(active) = &state.active_plant_earth_resolution {
                events.push(GameEvent::PlantEarthResolutionCompleted {
                    player: player.clone(),
                    due_turn_number: active.due_turn_number,
                    melody_id: SPLIT_EARTH.to_string(),
                });
            } else if state
                .active_echo_resolution
                .as_ref()
                .is_some_and(|active| active.player == *player && active.melody_id == SPLIT_EARTH)
            {
                let active = state
                    .active_echo_resolution
                    .as_ref()
                    .expect("matched active Echo");
                events.push(GameEvent::EchoResolutionCompleted {
                    player: player.clone(),
                    melody_id: SPLIT_EARTH.to_string(),
                    due_turn_number: active.due_turn_number,
                });
            } else {
                let melody = melody(SPLIT_EARTH).expect("Split Earth is an official Melody");
                events.push(echo_cost_choice(state, player, &melody)?);
            }
            Ok(Some(events))
        }
        EchoChoiceContinuation::PlantEarthMelody => {
            let ChoiceAnswer::Formation { formation_id } = answer else {
                return Err(GameError::Validation(ValidationError::InvalidChoiceAnswer));
            };
            let selected = melody_catalog()
                .into_iter()
                .find(|candidate| candidate.id == formation_id)
                .filter(|selected| {
                    matches!(
                        selected.id,
                        RINGING_METAL | FALLING_WOOD | FLOWING_WATER | WAR_FIRE | SPLIT_EARTH
                    )
                });
            let Some(selected) = selected else {
                return Err(GameError::Validation(ValidationError::InvalidChoiceAnswer));
            };
            let mut events = main_effect_events(
                state,
                player,
                &selected,
                MelodyExecutionOrigin::PlantedEarth,
            )?;
            if !events.iter().any(|event| {
                matches!(
                    event,
                    GameEvent::ChoiceRequested { .. } | GameEvent::RandomnessRequested { .. }
                )
            }) {
                let active = state
                    .active_plant_earth_resolution
                    .as_ref()
                    .ok_or_else(|| {
                        GameError::RuleImplementation(
                            crate::domain::RuleImplementationError::EffectNotImplemented(
                                "echo:plant-earth:missing-active-resolution".to_string(),
                            ),
                        )
                    })?;
                events.push(GameEvent::PlantEarthResolutionCompleted {
                    player: player.clone(),
                    due_turn_number: active.due_turn_number,
                    melody_id: selected.id.to_string(),
                });
            }
            Ok(Some(events))
        }
        EchoChoiceContinuation::PureFireTarget => {
            let ChoiceAnswer::Player { player: target } = answer else {
                return Err(GameError::Validation(ValidationError::InvalidChoiceAnswer));
            };
            let mut events = vec![GameEvent::TimedEffectsReduced {
                source: player.clone(),
                target: target.clone(),
                reductions: timed_effect_reductions(state, target),
            }];
            if let Some(active) = state
                .active_echo_resolution
                .as_ref()
                .filter(|active| active.player == *player && active.melody_id == PURE_FIRE)
            {
                events.push(GameEvent::EchoResolutionCompleted {
                    player: player.clone(),
                    melody_id: PURE_FIRE.to_string(),
                    due_turn_number: active.due_turn_number,
                });
            } else {
                events.push(GameEvent::EchoScheduled {
                    schedule: ScheduledEcho {
                        player: player.clone(),
                        melody_id: PURE_FIRE.to_string(),
                        due_turn_number: state.turn_number + state.turn_order.len() as u64,
                    },
                });
            }
            Ok(Some(events))
        }
        EchoChoiceContinuation::RingingMetalDeckCard => {
            let ChoiceAnswer::Cards { cards } = answer else {
                return Err(GameError::Validation(ValidationError::InvalidChoiceAnswer));
            };
            let card = cards[0];
            let selection = crate::domain::RingingMetalSelection {
                player: player.clone(),
                card,
                deck: deck_kind(state, player),
            };
            let mut projected = state.clone();
            let revealed = GameEvent::RingingMetalCardRevealed {
                selection: selection.clone(),
            };
            crate::rules::projection::apply_event(&mut projected, &revealed);
            let mut events = vec![revealed];
            let remainder = projected
                .deck_for(player)
                .expect("choice Player must have a Deck")
                .to_vec();
            if remainder.is_empty() {
                events.extend(ringing_metal_completion_events(&projected, selection)?);
            } else {
                events.push(GameEvent::RandomnessRequested {
                    request: crate::domain::PendingRandomness {
                        request_id: format!(
                            "echo:ringing-metal:post-search:{}:{}",
                            state.turn_number,
                            player.as_str()
                        ),
                        operation: crate::domain::RandomnessOperation::DeckShuffle {
                            deck: deck_kind(state, player),
                        },
                        continuation: RandomnessContinuation::Echo(
                            EchoRandomnessContinuation::RingingMetalPostSearch,
                        ),
                        current_order: remainder,
                    },
                });
            }
            Ok(Some(events))
        }
        EchoChoiceContinuation::Cost { melody_id } => {
            let Some(melody) = melody(melody_id) else {
                return Ok(None);
            };
            let mut events = match answer {
                ChoiceAnswer::Decline => vec![GameEvent::EchoDeclined {
                    player: player.clone(),
                    melody_id: melody.id.to_string(),
                }],
                ChoiceAnswer::Cards { cards } if cards.len() == 1 => {
                    let card = cards[0];
                    vec![GameEvent::EchoCostPaid {
                        player: player.clone(),
                        melody_id: melody.id.to_string(),
                        card_move: CardMoveDelta {
                            card,
                            from: CardZone::Hand(player.clone()),
                            to: discard_zone_for_card(state, card),
                        },
                    }]
                }
                _ => {
                    return Err(GameError::Validation(ValidationError::InvalidChoiceAnswer));
                }
            };
            if matches!(answer, ChoiceAnswer::Cards { .. }) {
                events.push(GameEvent::EchoScheduled {
                    schedule: ScheduledEcho {
                        player: player.clone(),
                        melody_id: melody.id.to_string(),
                        due_turn_number: state.turn_number + state.turn_order.len() as u64,
                    },
                });
            }
            Ok(Some(events))
        }
    }
}

pub(crate) fn turn_start_events(state: &GameState) -> GameResult<Vec<GameEvent>> {
    let Some(player) = state.current_player() else {
        return Err(GameError::Validation(ValidationError::EmptyTurnOrder));
    };
    let Some(schedule) = state
        .scheduled_echoes
        .iter()
        .find(|schedule| {
            &schedule.player == player && schedule.due_turn_number <= state.turn_number
        })
        .cloned()
    else {
        return plant_earth_turn_start_events(state);
    };
    let melody = melody(&schedule.melody_id).ok_or_else(|| {
        GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(
                schedule.melody_id.clone(),
            ),
        )
    })?;
    let mut events = vec![GameEvent::EchoResolutionStarted {
        schedule: schedule.clone(),
    }];
    events.extend(main_effect_events(
        state,
        player,
        &melody,
        MelodyExecutionOrigin::Echo,
    )?);
    if !events.iter().any(|event| {
        matches!(
            event,
            GameEvent::ChoiceRequested { .. } | GameEvent::RandomnessRequested { .. }
        )
    }) {
        events.push(GameEvent::EchoResolutionCompleted {
            player: player.clone(),
            melody_id: melody.id.to_string(),
            due_turn_number: schedule.due_turn_number,
        });
    }
    Ok(events)
}

pub(crate) fn flow_trigger_event(state: &GameState, player: &PlayerId) -> Option<GameEvent> {
    let old_layers = state
        .flow_layers_by_player
        .get(player)
        .copied()
        .unwrap_or(0);
    if old_layers == 0 {
        return None;
    }
    if state
        .flow_triggered_turn_by_player
        .get(player)
        .is_some_and(|turn| *turn == state.turn_number)
    {
        return None;
    }
    let hand_count = state.hand(player)?.len();
    let available_space = state.hand_limit.saturating_sub(hand_count);
    let old_draw_bonus = state
        .turn_draw_bonus_by_player
        .get(player)
        .copied()
        .unwrap_or(0);
    (state.base_draw + old_draw_bonus < available_space).then_some(GameEvent::FlowStateTriggered {
        player: player.clone(),
        old_layers,
        new_layers: old_layers - 1,
        old_draw_bonus,
        new_draw_bonus: old_draw_bonus + 1,
    })
}

fn main_effect_events(
    state: &GameState,
    player: &PlayerId,
    melody: &MelodyDef,
    _origin: MelodyExecutionOrigin,
) -> GameResult<Vec<GameEvent>> {
    match melody.id {
        FALLING_WOOD => hp_event(state, player, 15).map(|event| event.into_iter().collect()),
        FLOWING_WATER => {
            let old_layers = state
                .flow_layers_by_player
                .get(player)
                .copied()
                .unwrap_or(0);
            Ok(vec![GameEvent::FlowStateChanged {
                player: player.clone(),
                old_layers,
                new_layers: old_layers + 1,
            }])
        }
        WAR_FIRE => {
            let target = TurnOrderTargets::new(state)
                .player_target(player, RulePlayerTarget::PreviousPlayer)?;
            hp_event(state, &target, -15).map(|event| event.into_iter().collect())
        }
        PLANT_EARTH if matches!(_origin, MelodyExecutionOrigin::FormationUse) => {
            Ok(vec![GameEvent::PlantEarthScheduled {
                schedule: crate::domain::ScheduledPlantEarth {
                    player: player.clone(),
                    due_turn_number: state.turn_number + state.turn_order.len() as u64,
                },
            }])
        }
        PURE_FIRE => Ok(vec![crate::rules::pending_choice::request_event(
            state,
            ChoiceRequest {
                player: player.clone(),
                kind: PendingChoiceKind::Player {
                    players: state.players.iter().map(|entry| entry.id.clone()).collect(),
                    can_decline: false,
                },
                continuation: ChoiceContinuation::Echo(EchoChoiceContinuation::PureFireTarget),
            },
        )?]),
        PLANT_EARTH => Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(melody.id.to_string()),
        )),
        SPLIT_EARTH => {
            let target =
                TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::NextPlayer)?;
            let cards = state
                .hand(&target)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
                })?
                .to_vec();
            let mut formations =
                crate::rules::official_formation_registry(&state.enabled_rule_modules)
                    .formations()
                    .into_iter()
                    .map(|formation| formation.id.clone())
                    .collect::<Vec<_>>();
            formations.sort();
            Ok(vec![
                GameEvent::HandInspected {
                    viewer: player.clone(),
                    target,
                    cards,
                },
                crate::rules::pending_choice::request_event(
                    state,
                    ChoiceRequest {
                        player: player.clone(),
                        kind: PendingChoiceKind::Formation {
                            formations,
                            can_decline: false,
                        },
                        continuation: ChoiceContinuation::Echo(
                            EchoChoiceContinuation::SplitEarthFormation,
                        ),
                    },
                )?,
            ])
        }
        RINGING_METAL => ringing_metal_start_events(state, player),
        _ => unreachable!("every Melody has one main effect"),
    }
}

fn timed_effect_reductions(
    state: &GameState,
    target: &PlayerId,
) -> Vec<crate::domain::TimedEffectReduction> {
    let mut reductions = crate::rules::base::timed_effect_reductions(state, target);
    reductions.extend(echo_timed_effect_reductions(state, target));
    reductions.extend(crate::rules::hero::timed_effect_reductions(state, target));
    reductions.extend(crate::rules::jianghu::timed_effect_reductions(
        state, target,
    ));
    reductions.extend(crate::rules::confluence::timed_effect_reductions(
        state, target,
    ));
    reductions.extend(crate::rules::dark::timed_effect_reductions(state, target));
    reductions
}

fn echo_timed_effect_reductions(
    state: &GameState,
    target: &PlayerId,
) -> Vec<crate::domain::TimedEffectReduction> {
    let mut reductions = Vec::new();
    if let Some(old_layers) = state
        .flow_layers_by_player
        .get(target)
        .copied()
        .filter(|layers| *layers > 0)
    {
        reductions.push(crate::domain::TimedEffectReduction::FlowState {
            player: target.clone(),
            old_layers,
            new_layers: old_layers - 1,
        });
    }
    reductions.extend(
        state
            .formation_suppressions
            .iter()
            .filter(|suppression| suppression.target == *target)
            .map(
                |suppression| crate::domain::TimedEffectReduction::FormationSuppression {
                    target: suppression.target.clone(),
                    formation_id: suppression.formation_id.clone(),
                },
            ),
    );
    reductions
}

pub(crate) fn after_randomness_events(
    state: &GameState,
    continuation: &EchoRandomnessContinuation,
) -> GameResult<Vec<GameEvent>> {
    match continuation {
        EchoRandomnessContinuation::RingingMetalRecycleDiscard => {
            ringing_metal_search_choice(state)
        }
        EchoRandomnessContinuation::RingingMetalPostSearch => {
            let selection = state.ringing_metal_selection.clone().ok_or_else(|| {
                GameError::RuleImplementation(
                    crate::domain::RuleImplementationError::EffectNotImplemented(
                        "echo:ringing-metal:missing-selection".to_string(),
                    ),
                )
            })?;
            ringing_metal_completion_events(state, selection)
        }
    }
}

fn ringing_metal_start_events(state: &GameState, player: &PlayerId) -> GameResult<Vec<GameEvent>> {
    let deck = state
        .deck_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if !deck.is_empty() {
        return ringing_metal_search_choice(state);
    }
    let discard = state
        .discard_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if discard.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![GameEvent::RandomnessRequested {
        request: crate::domain::PendingRandomness {
            request_id: format!(
                "echo:ringing-metal:recycle:{}:{}",
                state.turn_number,
                player.as_str()
            ),
            operation: crate::domain::RandomnessOperation::DiscardShuffle {
                pile: deck_kind(state, player),
                placement: crate::domain::DeckPlacement::Bottom,
            },
            continuation: RandomnessContinuation::Echo(
                EchoRandomnessContinuation::RingingMetalRecycleDiscard,
            ),
            current_order: discard.to_vec(),
        },
    }])
}

fn ringing_metal_search_choice(state: &GameState) -> GameResult<Vec<GameEvent>> {
    let player = state
        .active_echo_resolution
        .as_ref()
        .map(|active| active.player.clone())
        .or_else(|| state.current_player().cloned())
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;
    let allowed_cards = state
        .deck_for(&player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
        .to_vec();
    if allowed_cards.is_empty() {
        return Ok(Vec::new());
    }
    Ok(vec![crate::rules::pending_choice::request_event(
        state,
        ChoiceRequest {
            player: player.clone(),
            kind: PendingChoiceKind::Card {
                cards: allowed_cards,
                minimum: 1,
                maximum: 1,
                can_decline: false,
            },
            continuation: ChoiceContinuation::Echo(EchoChoiceContinuation::RingingMetalDeckCard),
        },
    )?])
}

fn ringing_metal_completion_events(
    state: &GameState,
    selection: crate::domain::RingingMetalSelection,
) -> GameResult<Vec<GameEvent>> {
    let mut events = vec![GameEvent::RingingMetalCompleted {
        selection: selection.clone(),
    }];
    if let Some(active) = &state.active_echo_resolution {
        events.push(GameEvent::EchoResolutionCompleted {
            player: active.player.clone(),
            melody_id: active.melody_id.clone(),
            due_turn_number: active.due_turn_number,
        });
    } else if let Some(active) = &state.active_plant_earth_resolution {
        events.push(GameEvent::PlantEarthResolutionCompleted {
            player: active.player.clone(),
            due_turn_number: active.due_turn_number,
            melody_id: RINGING_METAL.to_string(),
        });
    } else {
        let mut projected = state.clone();
        crate::rules::projection::apply_event(&mut projected, &events[0]);
        let melody = melody(RINGING_METAL).expect("Ringing Metal is in the catalog");
        events.push(echo_cost_choice(&projected, &selection.player, &melody)?);
    }
    Ok(events)
}

fn plant_earth_turn_start_events(state: &GameState) -> GameResult<Vec<GameEvent>> {
    let player = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;
    let Some(schedule) = state
        .scheduled_plant_earth
        .iter()
        .find(|schedule| {
            &schedule.player == player && schedule.due_turn_number <= state.turn_number
        })
        .cloned()
    else {
        return Ok(Vec::new());
    };
    Ok(vec![
        GameEvent::PlantEarthResolutionStarted {
            schedule: schedule.clone(),
        },
        crate::rules::pending_choice::request_event(
            state,
            ChoiceRequest {
                player: player.clone(),
                kind: PendingChoiceKind::Formation {
                    formations: vec![
                        RINGING_METAL.to_string(),
                        FALLING_WOOD.to_string(),
                        FLOWING_WATER.to_string(),
                        WAR_FIRE.to_string(),
                        SPLIT_EARTH.to_string(),
                    ],
                    can_decline: false,
                },
                continuation: ChoiceContinuation::Echo(EchoChoiceContinuation::PlantEarthMelody),
            },
        )?,
    ])
}

fn deck_kind(state: &GameState, player: &PlayerId) -> crate::domain::RandomnessDeck {
    if state.uses_personal_decks() {
        crate::domain::RandomnessDeck::Player(player.clone())
    } else {
        crate::domain::RandomnessDeck::Shared
    }
}

pub(crate) fn formation_use_is_suppressed(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) -> bool {
    state.formation_suppressions.iter().any(|suppression| {
        &suppression.target == player
            && suppression.formation_id == formation_id
            && suppression.expires_on_turn_number == state.turn_number
    })
}

pub(crate) fn turn_end_expiry_event(state: &GameState) -> Option<GameEvent> {
    let player = state.current_player()?;
    let suppression = state.formation_suppressions.iter().find(|suppression| {
        &suppression.target == player && suppression.expires_on_turn_number <= state.turn_number
    })?;
    Some(GameEvent::FormationSuppressionExpired {
        target: player.clone(),
        formation_id: suppression.formation_id.clone(),
        expired_on_turn_number: state.turn_number,
    })
}

fn hp_event(
    state: &GameState,
    affected_player: &PlayerId,
    delta: i32,
) -> GameResult<Option<GameEvent>> {
    let team = TurnOrderTargets::new(state).team_of(affected_player)?;
    if delta > 0 && crate::rules::jianghu::team_has_poison(state, &team) {
        return Ok(None);
    }
    let old_hp = state
        .hp
        .iter()
        .find(|entry| entry.team == team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?
        .hp;
    let initial_hp = state
        .initial_hp(&team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;
    let new_hp = (old_hp + delta).clamp(0, initial_hp);
    Ok((new_hp != old_hp).then_some(GameEvent::HpChanged {
        change: crate::domain::HpChangeDelta {
            team,
            old_hp,
            delta,
            new_hp,
            effective_delta: new_hp - old_hp,
        },
    }))
}

fn echo_cost_choice(
    state: &GameState,
    player: &PlayerId,
    melody: &MelodyDef,
) -> GameResult<GameEvent> {
    let EchoPolicy::OptionalCost {
        allowed_printed_elements,
    } = melody.echo_policy
    else {
        return Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(
                "echo:unsupported-policy".to_string(),
            ),
        ));
    };
    let allowed_cards = state
        .hand(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
        .iter()
        .copied()
        .filter(|card| {
            state
                .card_def(*card)
                .is_some_and(|definition| allowed_printed_elements.contains(&definition.element))
        })
        .collect();
    crate::rules::pending_choice::request_event(
        state,
        ChoiceRequest {
            player: player.clone(),
            kind: PendingChoiceKind::Card {
                cards: allowed_cards,
                minimum: 1,
                maximum: 1,
                can_decline: true,
            },
            continuation: ChoiceContinuation::Echo(EchoChoiceContinuation::Cost {
                melody_id: melody.id.to_string(),
            }),
        },
    )
}

fn discard_zone_for_card(state: &GameState, card: crate::domain::CardInstanceId) -> CardZone {
    if state.uses_personal_decks() {
        match state.card_origin(card) {
            Some(CardOrigin::Player(owner)) => CardZone::PlayerDiscard(owner.clone()),
            _ => CardZone::Discard,
        }
    } else {
        CardZone::Discard
    }
}

impl EchoPolicy {
    fn is_schedulable_from(self, origin: MelodyExecutionOrigin) -> bool {
        origin.can_schedule_echo() && !matches!(self, Self::None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_all_seven_published_melodies() {
        let catalog = melody_catalog();
        assert_eq!(catalog.len(), 7);
        let basic = catalog
            .iter()
            .filter(|melody| matches!(melody.echo_policy, EchoPolicy::OptionalCost { .. }))
            .collect::<Vec<_>>();
        assert_eq!(basic.len(), 5);
        assert_eq!(
            basic
                .iter()
                .map(|melody| (melody.name, melody.element))
                .collect::<Vec<_>>(),
            vec![
                ("商調‧鳴金", Element::Metal),
                ("角調‧落木", Element::Wood),
                ("羽調‧流水", Element::Water),
                ("徵調‧戰火", Element::Fire),
                ("宮調‧裂土", Element::Earth),
            ]
        );
        assert_eq!(
            basic
                .iter()
                .map(|melody| match melody.echo_policy {
                    EchoPolicy::OptionalCost {
                        allowed_printed_elements,
                    } => allowed_printed_elements,
                    _ => unreachable!(),
                })
                .collect::<Vec<_>>(),
            vec![
                [Element::Metal, Element::Earth],
                [Element::Wood, Element::Water],
                [Element::Water, Element::Metal],
                [Element::Fire, Element::Wood],
                [Element::Earth, Element::Fire],
            ]
        );
        assert!(matches!(
            melody(PURE_FIRE).unwrap().echo_policy,
            EchoPolicy::Automatic
        ));
        assert!(matches!(
            melody(PLANT_EARTH).unwrap().echo_policy,
            EchoPolicy::None
        ));
    }

    #[test]
    fn delayed_origins_cannot_schedule_recursive_echo() {
        assert!(MelodyExecutionOrigin::FormationUse.can_schedule_echo());
        assert!(!MelodyExecutionOrigin::Echo.can_schedule_echo());
        assert!(!MelodyExecutionOrigin::PlantedEarth.can_schedule_echo());
        assert_eq!(ECHO_MODULE_ID, "echo");
    }
}
