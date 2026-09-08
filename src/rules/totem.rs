//! 圖騰法陣及圖騰的環境例外。
use crate::domain::hp::HpChangePlan;
use crate::domain::targeting::{RulePlayerTarget, TurnOrderTargets};
use crate::domain::*;
use crate::rules::*;

pub(crate) const EAST: &str = "east-spirit-array";
pub(crate) const WEST: &str = "west-spirit-array";
pub(crate) const SOUTH: &str = "south-spirit-array";
pub(crate) const NORTH: &str = "north-spirit-array";
pub(crate) const CENTRAL: &str = "central-spirit-array";
pub(crate) const SEARCH: &str = "dragon-search";

pub(crate) fn matches_array(cards: &[SubmittedCardFacts], element: Element) -> bool {
    cards.len() == 3
        && cards.iter().all(|card| card.element == element)
        && cards.iter().any(|card| card.level.value() == 5)
}

pub(crate) fn formation_specs() -> Vec<BaseFormationSpec> {
    [
        (EAST, "東靈陣‧青角", "木木＋木５；回復１５生命、建構１５防護罩，轉換木行環境並獲得青角圖騰", FormationEffect::TotemAzureHorn),
        (WEST, "西靈陣‧白牙", "金金＋金５；上家扣３０生命，轉換金行環境並獲得白牙圖騰", FormationEffect::TotemWhiteFang),
        (SOUTH, "南靈陣‧朱羽", "火火＋火５；選擇五行屬性攻擊２０，傷害後轉換火行環境並獲得朱羽圖騰", FormationEffect::TotemVermilionFeather),
        (NORTH, "北靈陣‧玄甲", "水水＋水５；本回合抽牌＋１，檢視下家手牌，下家下回合無法行動及抽牌，轉換水行環境並獲得玄甲圖騰", FormationEffect::TotemBlackShell),
        (CENTRAL, "中靈陣‧黃鱗", "土土＋土５；檢視下家手牌並取一張，轉換土行環境並獲得黃鱗圖騰", FormationEffect::TotemYellowScales),
        (SEARCH, "尋龍", "兩張同行牌；檢索同行或所生屬性的５級牌，展示後洗剩餘牌堆並放回頂部；可放棄", FormationEffect::DragonSearch),
    ].into_iter().map(|(id,name,text,effect)| BaseFormationSpec {
        formation: FormationDef { id:id.into(), name:name.into(), rule_text:text.into(), category:FormationCategory::Spell, pattern:FormationPattern::Custom(id.into()), effect_id:id.into(), point_formula:PointFormula::Fixed(match id {EAST=>15,WEST=>30,SOUTH=>20,_=>0}) },
        effect:EffectDef {id:id.into(), plan:EffectPlan::ActiveSpell(SpellPlanDef {resolver_id:id.into(), player_facing_effect:effect})},
    }).collect()
}

pub(crate) fn owned(state: &GameState, player: &PlayerId) -> Option<TotemKind> {
    state
        .has_rule_module(TOTEM_FORMATION_MODULE_ID)
        .then(|| {
            state
                .totems
                .iter()
                .find(|owned| &owned.player == player)
                .map(|owned| owned.totem)
        })
        .flatten()
}
pub(crate) fn element(totem: TotemKind) -> Element {
    match totem {
        TotemKind::AzureHorn => Element::Wood,
        TotemKind::WhiteFang => Element::Metal,
        TotemKind::VermilionFeather => Element::Fire,
        TotemKind::BlackShell => Element::Water,
        TotemKind::YellowScales => Element::Earth,
    }
}
pub(crate) fn environment_ineffective(
    state: &GameState,
    player: &PlayerId,
    formation: &str,
) -> bool {
    let exempt = matches!(
        (owned(state, player), formation),
        (Some(TotemKind::AzureHorn), "defense" | "barrier")
            | (Some(TotemKind::WhiteFang), "weapon" | "radiance")
            | (
                Some(TotemKind::VermilionFeather),
                "countershock" | "shock-burst"
            )
            | (Some(TotemKind::BlackShell), "seal" | "return-to-origin")
            | (Some(TotemKind::YellowScales), "metamorphosis" | "chaos")
    );
    !exempt && environment_makes_formation_ineffective(state, formation)
}
fn granted(state: &GameState, player: &PlayerId, id: &str) -> Vec<GameEvent> {
    let totem = match id {
        EAST => TotemKind::AzureHorn,
        WEST => TotemKind::WhiteFang,
        SOUTH => TotemKind::VermilionFeather,
        NORTH => TotemKind::BlackShell,
        CENTRAL => TotemKind::YellowScales,
        _ => return vec![],
    };
    vec![
        GameEvent::EnvironmentTransferred {
            player: player.clone(),
            formation_id: id.into(),
            from: state.environment,
            to: element(totem),
        },
        GameEvent::TotemChanged {
            player: player.clone(),
            previous: owned(state, player),
            totem: Some(totem),
            reason: TotemChangeReason::Granted,
        },
    ]
}
fn choice(
    state: &GameState,
    player: &PlayerId,
    kind: PendingChoiceKind,
    resolution: PendingResolution,
) -> GameResult<GameEvent> {
    crate::rules::pending_choice::request_event(
        state,
        ChoiceRequest {
            player: player.clone(),
            kind,
            resolution,
        },
    )
}
fn inspect(state: &GameState, player: &PlayerId, target: &PlayerId) -> GameEvent {
    GameEvent::HandInspected {
        viewer: player.clone(),
        target: target.clone(),
        cards: state.hand(target).unwrap_or_default().to_vec(),
    }
}
pub(crate) fn active_events(
    state: &GameState,
    player: &PlayerId,
    id: &str,
    cards: &[CardInstanceId],
    damage_prevented: bool,
    split_attack_damage: bool,
    hp: &mut HpChangePlan,
) -> GameResult<Vec<GameEvent>> {
    let next = TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::NextPlayer)?;
    let mut events = vec![];
    match id {
        EAST | WEST => {
            let target = if id == EAST {
                player.clone()
            } else {
                TurnOrderTargets::new(state)
                    .player_target(player, RulePlayerTarget::PreviousPlayer)?
            };
            let team = TurnOrderTargets::new(state).team_of(&target)?;
            let mut effect = hp.begin_formation_effect();
            events.push(GameEvent::HpChanged {
                change: effect.plan(
                    &team,
                    crate::rules::base::formation_use::formation_hp_request(
                        state,
                        player,
                        &team,
                        if id == EAST { 15 } else { -30 },
                    ),
                )?,
            });
            if id == EAST {
                let old_value = state.shield(player).unwrap_or(0);
                events.push(GameEvent::ShieldChanged {
                    player: player.clone(),
                    old_value,
                    delta: 15 - old_value,
                    new_value: 15,
                });
            }
            events.extend(granted(state, player, id));
            let mut sequence =
                crate::rules::formation_effect_sequence::FormationEffectSequence::new(state);
            let outcome = effect.finish();
            sequence.append(
                hp,
                crate::rules::formation_effect_sequence::ResolvedFormationEffect::new(
                    events, outcome,
                ),
            )?;
            return Ok(sequence.into_events());
        }
        SOUTH => {
            return Ok(vec![choice(
                state,
                player,
                PendingChoiceKind::Environment {
                    environments: vec![
                        Element::Metal,
                        Element::Wood,
                        Element::Water,
                        Element::Fire,
                        Element::Earth,
                    ],
                    can_decline: false,
                },
                PendingResolution::SouthSpiritArrayElement {
                    damage_prevented,
                    split_attack_damage,
                },
            )?]);
        }
        NORTH => {
            let old_value = state
                .turn_draw_bonus_by_player
                .get(player)
                .copied()
                .unwrap_or(0);
            events.push(GameEvent::TurnDrawBonusChanged {
                player: player.clone(),
                old_value,
                delta: 1,
                new_value: old_value + 1,
            });
            events.push(inspect(state, player, &next));
            let turn_number = TurnOrderTargets::new(state).nth_future_turn_for_player(&next, 1)?;
            for kind in ["CannotAct", "CannotDraw"] {
                events.push(GameEvent::StatusAdded {
                    status: crate::rules::jianghu::shorten_enemy_status(
                        state,
                        StatusEffect {
                            id: format!("totem-north-{}-{kind}", state.turn_number),
                            owner: StatusOwner::Player(next.clone()),
                            kind: kind.into(),
                            value: None,
                            duration: StatusDuration::UntilTurnEndNumber {
                                player: next.clone(),
                                turn_number,
                            },
                        },
                    ),
                });
            }
        }
        CENTRAL => {
            events.push(inspect(state, player, &next));
            let cards = state.hand(&next).unwrap_or_default().to_vec();
            if !cards.is_empty() {
                events.push(choice(
                    state,
                    player,
                    PendingChoiceKind::Card {
                        cards,
                        minimum: 1,
                        maximum: 1,
                        can_decline: false,
                    },
                    PendingResolution::CentralSpiritArrayCard,
                )?);
                return Ok(events);
            }
        }
        SEARCH => {
            let source = state
                .effective_card_facts(player, cards[0])
                .expect("validated composition")
                .element;
            return search_choice(state, player, source, true);
        }
        _ => unreachable!("validated Totem formation"),
    }
    events.extend(granted(state, player, id));
    Ok(events)
}
fn generated(element: Element) -> Element {
    match element {
        Element::Metal => Element::Water,
        Element::Water => Element::Wood,
        Element::Wood => Element::Fire,
        Element::Fire => Element::Earth,
        Element::Earth => Element::Metal,
    }
}
pub(crate) fn answer(
    state: &GameState,
    player: &PlayerId,
    resolution: &PendingResolution,
    answer: &ChoiceAnswer,
    hp: &mut HpChangePlan,
) -> GameResult<Vec<GameEvent>> {
    match resolution {
        PendingResolution::SouthSpiritArrayElement {
            damage_prevented,
            split_attack_damage,
        } => {
            let ChoiceAnswer::Environment { environment } = answer else {
                unreachable!("validated element answer")
            };
            let cards = state
                .formation_area(player)
                .and_then(|area| area.formation.as_ref())
                .expect("committed formation")
                .cards
                .clone();
            let mut events = crate::rules::base::attack_resolution::resolve_with_plan(
                state,
                crate::rules::base::attack_resolution::AttackRequest {
                    attacker: player.clone(),
                    formation_id: SOUTH.into(),
                    category: AttackCategory::Elemental(*environment),
                    point_formula: PointFormula::Fixed(20),
                    used_cards: cards,
                    damage_prevented: *damage_prevented,
                    split_attack_damage: *split_attack_damage,
                    mode: crate::rules::base::attack_resolution::AttackResolutionMode::FormationUse,
                    pre_resolution_effects: Default::default(),
                    trailing_hp_role: HpChangeRole::FormationEffect,
                },
                hp,
            )?;
            let projected = project(state, &events);
            events.extend(granted(&projected, player, SOUTH));
            crate::rules::base::attack_resolution::absorb_simultaneous_events(
                &mut events,
                HpChangeRole::TriggeredEffect,
            );
            Ok(events)
        }
        PendingResolution::CentralSpiritArrayCard => {
            let ChoiceAnswer::Cards { cards } = answer else {
                unreachable!("validated card answer")
            };
            let next =
                TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::NextPlayer)?;
            let mut events = vec![GameEvent::CardsMoved {
                card_moves: vec![CardMoveDelta {
                    card: cards[0],
                    from: CardZone::Hand(next),
                    to: CardZone::Hand(player.clone()),
                }],
            }];
            events.extend(granted(state, player, CENTRAL));
            Ok(events)
        }
        PendingResolution::DragonSearchDeckCard => {
            let card = match answer {
                ChoiceAnswer::Cards { cards } => cards.first().copied(),
                ChoiceAnswer::Decline => None,
                _ => unreachable!("validated search answer"),
            };
            let mut events = vec![];
            if let Some(card) = card {
                events.push(GameEvent::DragonSearchRevealed {
                    player: player.clone(),
                    card,
                });
            }
            let projected = project(state, &events);
            let current_order = projected.deck_for(player).unwrap_or_default().to_vec();
            if current_order.len() > 1 {
                events.push(GameEvent::RandomnessRequested {
                    request: PendingRandomness {
                        request_id: format!(
                            "dragon-search:{}:{}",
                            state.turn_number,
                            player.as_str()
                        ),
                        operation: RandomnessOperation::DeckShuffle {
                            deck: if state.uses_personal_decks() {
                                RandomnessDeck::Player(player.clone())
                            } else {
                                RandomnessDeck::Shared
                            },
                        },
                        current_order,
                    },
                    resolution: PendingResolution::DragonSearchShuffle {
                        player: player.clone(),
                        card,
                    },
                });
            } else {
                events.push(GameEvent::DragonSearchCompleted {
                    player: player.clone(),
                    card,
                });
            }
            Ok(events)
        }
        _ => unreachable!("Totem continuation"),
    }
}

fn project(state: &GameState, events: &[GameEvent]) -> GameState {
    let mut state = state.clone();
    for event in events {
        crate::rules::projection::apply_event(&mut state, event);
    }
    state
}

pub(crate) fn search_choice(
    state: &GameState,
    player: &PlayerId,
    source: Element,
    recycle: bool,
) -> GameResult<Vec<GameEvent>> {
    if recycle
        && state.deck_for(player).is_some_and(|deck| deck.is_empty())
        && state
            .discard_for(player)
            .is_some_and(|discard| !discard.is_empty())
    {
        let deck = if state.uses_personal_decks() {
            RandomnessDeck::Player(player.clone())
        } else {
            RandomnessDeck::Shared
        };
        if let Some(event) = crate::rules::deck_supply::request_if_needed(
            state,
            &deck,
            1,
            DeckPlacement::Bottom,
            format!(
                "dragon-search-recycle:{}:{}",
                state.turn_number,
                player.as_str()
            ),
            PendingResolution::DragonSearchRecycle { element: source },
        )? {
            return Ok(vec![event]);
        }
    }
    let cards = state
        .deck_for(player)
        .unwrap_or_default()
        .iter()
        .copied()
        .filter(|card| {
            state.card_def(*card).is_some_and(|card| {
                card.level.value() == 5
                    && (card.element == source || card.element == generated(source))
            })
        })
        .collect();
    Ok(vec![choice(
        state,
        player,
        PendingChoiceKind::Card {
            cards,
            minimum: 1,
            maximum: 1,
            can_decline: true,
        },
        PendingResolution::DragonSearchDeckCard,
    )?])
}
