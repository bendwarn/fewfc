use crate::domain::{
    CONFLUENCE_GENERATION_MODULE_ID, CardInstanceId, ConfluenceRandomnessContinuation, Element,
    GameError, GameEvent, GameResult, GameState, HpChangeDelta, PlayerId, ProfessionId,
    RandomnessContinuation, RandomnessDeck, RandomnessOperation, RuleModuleId, StatusDuration,
    StatusEffect, StatusOwner, ValidationError,
    targeting::{RulePlayerTarget, TurnOrderTargets},
};
use crate::rules::{
    BaseFormationSpec, EffectDef, EffectPlan, FormationCategory, FormationDef, FormationPattern,
    PointFormula, SpellPlanDef,
};

pub(crate) fn timed_effect_reductions(
    state: &GameState,
    target: &PlayerId,
) -> Vec<crate::domain::TimedEffectReduction> {
    if !state.has_rule_module(crate::domain::CONFLUENCE_GENERATION_MODULE_ID) {
        return Vec::new();
    }
    crate::rules::timed_effect::status_reductions(state, target, |id| id.starts_with("confluence-"))
}
use crate::rules::{ProfessionAbilityCandidate, ProfessionChangeCandidate, SubmittedCardFacts};

pub(crate) const TUNER_ID: &str = "confluence:tuner";
pub(crate) const STRING_CHANGER_ID: &str = "confluence:string-changer";
pub(crate) const HEAVENLY_RESONATOR_ID: &str = "confluence:heavenly-resonator";
pub(crate) const DAO_MAGE_ID: &str = "confluence:dao-mage";
pub(crate) const DAO_SAINT_ID: &str = "confluence:dao-saint";
pub(crate) const CLEAR_WIND_ADEPT_ID: &str = "confluence:clear-wind-adept";
pub(crate) const CLEAR_WIND_ENVOY_ID: &str = "confluence:clear-wind-envoy";
pub(crate) const VOID_SEEKER_ID: &str = "confluence:void-seeker";
pub(crate) const VOID_DESTROYER_ID: &str = "confluence:void-destroyer";

pub(crate) const HEAVENLY_RESONANCE_USE: &str = "confluence:heavenly-resonance";
pub(crate) const IMPRISONING_ARRAY_USE: &str = "confluence:imprisoning-array";
pub(crate) const TAILWIND_USE: &str = "confluence:tailwind";
pub(crate) const VOID_REALM_USE: &str = "confluence:void-realm";

pub(crate) const MIRROR_RESONANCE: &str = "confluence:mirror-resonance";
pub(crate) const FOREST_RESONANCE: &str = "confluence:forest-resonance";
pub(crate) const STREAM_RESONANCE: &str = "confluence:stream-resonance";
pub(crate) const BLAZE_RESONANCE: &str = "confluence:blaze-resonance";
pub(crate) const EARTH_RESONANCE: &str = "confluence:earth-resonance";
pub(crate) const THOUSAND_RESONANCE: &str = "confluence:thousand-resonance";
pub(crate) const MYRIAD_RESONANCE: &str = "confluence:myriad-resonance";
pub(crate) const IMPRISONING_ARRAY: &str = "confluence:imprisoning-array";
pub(crate) const ENDLESS_ARRAY: &str = "confluence:endless-array";
pub(crate) const WIND_DANCE: &str = "confluence:wind-dance";
pub(crate) const CLEAR_WIND_TEN_THOUSAND_MILES: &str = "confluence:clear-wind-ten-thousand-miles";
pub(crate) const VOID_BARRIER: &str = "confluence:void-barrier";
pub(crate) const VOID_RETURN_TO_NOTHING: &str = "confluence:void-return-to-nothing";

pub(crate) fn formation_specs() -> Vec<BaseFormationSpec> {
    vec![
        spell(
            MIRROR_RESONANCE,
            "鏡鳴",
            "餘行為金；金行牌＋餘級牌；檢視上家手牌並捨棄一張",
        ),
        spell(
            FOREST_RESONANCE,
            "森鳴",
            "餘行為木；木行牌＋餘級牌；回復２０點生命",
        ),
        spell(
            STREAM_RESONANCE,
            "淙鳴",
            "餘行為水；水行牌＋餘級牌；本回合抽牌＋２",
        ),
        spell(
            BLAZE_RESONANCE,
            "煌鳴",
            "餘行為火；火行牌＋餘級牌；上家扣除２０點生命",
        ),
        spell(
            EARTH_RESONANCE,
            "垠鳴",
            "餘行為土；土行牌＋餘級牌；建構１５點防護罩",
        ),
        spell(
            THOUSAND_RESONANCE,
            "千鳴",
            "餘行牌＋餘級牌＋餘行餘級牌；同時發動餘行五鳴術與指定五鳴術",
        ),
        spell(
            MYRIAD_RESONANCE,
            "萬鳴",
            "金木水火土，含餘行餘級牌；五種五鳴術效果",
        ),
        spell(
            IMPRISONING_ARRAY,
            "禁錮法陣",
            "５級牌；所有其他玩家無法行動及抽牌１回合",
        ),
        spell(
            ENDLESS_ARRAY,
            "無盡法陣",
            "５級牌；回復禁錮法陣全部使用次數",
        ),
        spell(WIND_DANCE, "舞風", "連續兩級牌；本回合抽牌＋１"),
        spell(
            CLEAR_WIND_TEN_THOUSAND_MILES,
            "晴風萬里",
            "連續四級牌；抽十張牌，保留任意牌，其餘捨棄",
        ),
        spell(
            VOID_BARRIER,
            "虛空絕壁術",
            "三張同級牌；所有玩家防護罩扣除２０，被扣除者扣除２０點生命",
        ),
        spell(
            VOID_RETURN_TO_NOTHING,
            "虛空歸無術",
            "三張同級牌；雙方生命值減半，對方再扣除２０點生命",
        ),
    ]
}

fn spell(id: &str, name: &str, rule_text: &str) -> BaseFormationSpec {
    BaseFormationSpec {
        formation: FormationDef {
            id: id.to_string(),
            name: name.to_string(),
            rule_text: rule_text.to_string(),
            category: FormationCategory::Spell,
            pattern: FormationPattern::Custom(id.to_string()),
            effect_id: id.to_string(),
            point_formula: PointFormula::Fixed(0),
        },
        effect: EffectDef {
            id: id.to_string(),
            plan: EffectPlan::ActiveSpell(SpellPlanDef {
                resolver_id: id.to_string(),
            }),
        },
    }
}

pub(crate) fn is_profession_formation(id: &str) -> bool {
    id.starts_with("confluence:") && formation_specs().iter().any(|spec| spec.formation.id == id)
}

pub(crate) fn can_use_profession_formation(
    modules: &[RuleModuleId],
    profession: Option<&ProfessionId>,
    id: &str,
) -> bool {
    let Some(profession) = profession else {
        return false;
    };
    let inherits = |ancestor| {
        crate::rules::profession::inherits_from(modules, profession, &ProfessionId::new(ancestor))
    };
    match id {
        MIRROR_RESONANCE | FOREST_RESONANCE | STREAM_RESONANCE | BLAZE_RESONANCE
        | EARTH_RESONANCE => inherits(TUNER_ID),
        THOUSAND_RESONANCE => inherits(STRING_CHANGER_ID),
        MYRIAD_RESONANCE => profession.as_str() == HEAVENLY_RESONATOR_ID,
        IMPRISONING_ARRAY => inherits(DAO_MAGE_ID),
        ENDLESS_ARRAY => profession.as_str() == DAO_SAINT_ID,
        WIND_DANCE => inherits(CLEAR_WIND_ADEPT_ID),
        CLEAR_WIND_TEN_THOUSAND_MILES => profession.as_str() == CLEAR_WIND_ENVOY_ID,
        VOID_BARRIER => inherits(VOID_SEEKER_ID),
        VOID_RETURN_TO_NOTHING => profession.as_str() == VOID_DESTROYER_ID,
        _ => false,
    }
}

pub(crate) fn formation_matches(
    id: &str,
    residual: Option<(Element, u32)>,
    cards: &[SubmittedCardFacts],
) -> bool {
    let resonance_element = match id {
        MIRROR_RESONANCE => Some(Element::Metal),
        FOREST_RESONANCE => Some(Element::Wood),
        STREAM_RESONANCE => Some(Element::Water),
        BLAZE_RESONANCE => Some(Element::Fire),
        EARTH_RESONANCE => Some(Element::Earth),
        _ => None,
    };
    if let Some(required_element) = resonance_element {
        return residual.is_some_and(|(element, level)| {
            element == required_element
                && cards.len() == 2
                && cards.iter().enumerate().any(|(element_index, card)| {
                    card.element == required_element
                        && cards.iter().enumerate().any(|(level_index, card)| {
                            level_index != element_index && card.level == level
                        })
                })
        });
    }
    match id {
        THOUSAND_RESONANCE => residual.is_some_and(|(element, level)| {
            cards.len() == 3
                && (0..cards.len()).any(|both| {
                    cards[both].element == element
                        && cards[both].level == level
                        && (0..cards.len()).any(|element_card| {
                            element_card != both
                                && cards[element_card].element == element
                                && (0..cards.len()).any(|level_card| {
                                    level_card != both
                                        && level_card != element_card
                                        && cards[level_card].level == level
                                })
                        })
                })
        }),
        MYRIAD_RESONANCE => residual.is_some_and(|(element, level)| {
            cards.len() == 5
                && cards
                    .iter()
                    .map(|card| card.element)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == 5
                && cards
                    .iter()
                    .any(|card| card.element == element && card.level == level)
        }),
        IMPRISONING_ARRAY | ENDLESS_ARRAY => cards.len() == 1 && cards[0].level == 5,
        WIND_DANCE => consecutive_levels(cards, 2),
        CLEAR_WIND_TEN_THOUSAND_MILES => consecutive_levels(cards, 4),
        VOID_BARRIER | VOID_RETURN_TO_NOTHING => {
            cards.len() == 3 && cards.iter().all(|card| card.level == cards[0].level)
        }
        _ => false,
    }
}

pub(crate) fn formation_role_options(
    id: &str,
    residual: Option<(Element, u32)>,
    cards: &[CardInstanceId],
) -> Vec<(crate::domain::TargetDecl, String)> {
    if id != THOUSAND_RESONANCE || cards.is_empty() {
        return Vec::new();
    }
    let Some((residual_element, _)) = residual else {
        return Vec::new();
    };
    [
        (Element::Metal, "鏡鳴"),
        (Element::Wood, "森鳴"),
        (Element::Water, "淙鳴"),
        (Element::Fire, "煌鳴"),
        (Element::Earth, "垠鳴"),
    ]
    .into_iter()
    .filter(|(element, _)| *element != residual_element)
    .map(|(element, label)| {
        (
            crate::domain::TargetDecl::FormationRole {
                role: format!("confluence:thousand-resonance:{element:?}"),
                card: cards[0],
            },
            format!("同時發動餘行五鳴術與{label}"),
        )
    })
    .collect()
}

pub(crate) fn profession_formation_summaries(
    modules: &[RuleModuleId],
    profession: &ProfessionId,
) -> Vec<(String, String)> {
    formation_specs()
        .into_iter()
        .filter(|spec| can_use_profession_formation(modules, Some(profession), &spec.formation.id))
        .map(|spec| (spec.formation.name, spec.formation.rule_text))
        .collect()
}

pub(crate) fn formation_available_for_selection(
    uses: &[crate::domain::LimitedUse],
    id: &str,
) -> bool {
    match id {
        IMPRISONING_ARRAY => uses
            .iter()
            .find(|use_count| use_count.key == IMPRISONING_ARRAY_USE)
            .is_some_and(|use_count| use_count.remaining > 0),
        _ => true,
    }
}

pub(crate) fn active_spell_events(
    state: &GameState,
    player: &PlayerId,
    resolver_id: &str,
    declared_targets: &[crate::domain::TargetDecl],
) -> GameResult<Option<Vec<GameEvent>>> {
    let previous =
        TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
    let events = match resolver_id {
        MIRROR_RESONANCE => inspect_and_discard_events(state, player, &previous, resolver_id)?,
        FOREST_RESONANCE => vec![change_hp_event(state, player, 20)?],
        STREAM_RESONANCE => vec![turn_draw_bonus_event(state, player, 2)],
        BLAZE_RESONANCE => vec![change_hp_event(state, &previous, -20)?],
        EARTH_RESONANCE => vec![set_shield_event(state, player, 15)],
        MYRIAD_RESONANCE => {
            let mut events = vec![
                change_hp_event(state, &previous, -20)?,
                change_hp_event(state, player, 20)?,
                set_shield_event(state, player, 15),
                turn_draw_bonus_event(state, player, 1),
            ];
            events.extend(inspect_and_discard_events(
                state,
                player,
                &previous,
                resolver_id,
            )?);
            events
        }
        IMPRISONING_ARRAY => {
            let use_count = limited_use(state, player, IMPRISONING_ARRAY_USE)
                .filter(|use_count| use_count.remaining > 0)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::ProfessionAbilityCannotResolve(
                        IMPRISONING_ARRAY.to_string(),
                    ))
                })?;
            let mut events = vec![GameEvent::LimitedUseChanged {
                owner: player.clone(),
                key: IMPRISONING_ARRAY_USE.to_string(),
                old_remaining: use_count.remaining,
                new_remaining: use_count.remaining - 1,
                maximum: use_count.maximum,
            }];
            for target in state
                .players
                .iter()
                .map(|entry| &entry.id)
                .filter(|target| *target != player)
            {
                events.extend(cannot_act_or_draw(state, target, "imprisoning-array")?);
            }
            events
        }
        ENDLESS_ARRAY => {
            let maximum = limited_use(state, player, IMPRISONING_ARRAY_USE)
                .map_or(2, |use_count| use_count.maximum.max(2));
            vec![set_limited_use(
                state,
                player,
                IMPRISONING_ARRAY_USE,
                maximum,
                maximum,
            )]
        }
        WIND_DANCE => vec![turn_draw_bonus_event(state, player, 1)],
        CLEAR_WIND_TEN_THOUSAND_MILES => {
            let deck_count = state.deck_for(player).map_or(0, |deck| deck.len());
            let discard = state.discard_for(player).map_or(&[][..], |cards| cards);
            if deck_count < 10 && !discard.is_empty() {
                let pile = deck_kind(state, player);
                vec![GameEvent::RandomnessRequested {
                    request: crate::domain::PendingRandomness {
                        request_id: format!(
                            "confluence:clear-wind-ten-thousand-miles:{}:{}",
                            state.turn_number,
                            player.as_str()
                        ),
                        operation: RandomnessOperation::DiscardShuffle {
                            pile,
                            placement: crate::domain::DeckPlacement::Bottom,
                        },
                        continuation: RandomnessContinuation::Confluence(
                            ConfluenceRandomnessContinuation::ClearWindTenThousandMiles,
                        ),
                        current_order: discard.to_vec(),
                    },
                }]
            } else {
                clear_wind_ten_thousand_miles_after_shuffle(state, player)?
            }
        }
        VOID_BARRIER => {
            let mut events = Vec::new();
            let mut hp_by_team = state
                .hp
                .iter()
                .map(|entry| (entry.team.clone(), entry.hp))
                .collect::<std::collections::HashMap<_, _>>();
            for shield in &state.shields {
                let new_value = (shield.value - 20).max(0);
                if new_value != shield.value {
                    events.push(GameEvent::ShieldChanged {
                        player: shield.player.clone(),
                        old_value: shield.value,
                        delta: new_value - shield.value,
                        new_value,
                    });
                    let team = TurnOrderTargets::new(state).team_of(&shield.player)?;
                    let old_hp = *hp_by_team.get(&team).expect("known Player Team has HP");
                    let new_hp = (old_hp - 20).max(0);
                    hp_by_team.insert(team.clone(), new_hp);
                    events.push(GameEvent::HpChanged {
                        change: HpChangeDelta {
                            team,
                            old_hp,
                            delta: -20,
                            new_hp,
                            effective_delta: new_hp - old_hp,
                        },
                    });
                }
            }
            events
        }
        VOID_RETURN_TO_NOTHING => {
            let own_team = TurnOrderTargets::new(state).team_of(player)?;
            state
                .hp
                .iter()
                .map(|team_hp| {
                    let additional = if team_hp.team == own_team { 0 } else { 20 };
                    let new_hp = (team_hp.hp / 2 - additional).max(0);
                    Ok(GameEvent::HpChanged {
                        change: HpChangeDelta {
                            team: team_hp.team.clone(),
                            old_hp: team_hp.hp,
                            delta: new_hp - team_hp.hp,
                            new_hp,
                            effective_delta: new_hp - team_hp.hp,
                        },
                    })
                })
                .collect::<GameResult<Vec<_>>>()?
        }
        THOUSAND_RESONANCE => {
            let selected = declared_targets.iter().find_map(|target| match target {
                crate::domain::TargetDecl::FormationRole { role, .. } => {
                    role.strip_prefix("confluence:thousand-resonance:")
                }
                _ => None,
            });
            if residual_card_facts(state, player)
                .is_some_and(|(element, _)| element == Element::Metal)
            {
                let previous = TurnOrderTargets::new(state)
                    .player_target(player, RulePlayerTarget::PreviousPlayer)?;
                let continuation_id = selected
                    .map(|element| format!("confluence:thousand-resonance-after:{element}"))
                    .unwrap_or_else(|| "confluence:discard-inspected-card".to_string());
                return Ok(Some(inspect_and_discard_events_with_continuation(
                    state,
                    player,
                    &previous,
                    resolver_id,
                    &continuation_id,
                )?));
            }
            let mut events = resonance_primary_events(state, player)?;
            if let Some(selected) = selected {
                events.extend(resonance_element_events(state, player, selected)?);
            }
            events
        }
        _ => return Ok(None),
    };
    Ok(Some(events))
}

fn resonance_element_events(
    state: &GameState,
    player: &PlayerId,
    element: &str,
) -> GameResult<Vec<GameEvent>> {
    let previous =
        TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
    match element {
        "Metal" => inspect_and_discard_events(state, player, &previous, THOUSAND_RESONANCE),
        "Wood" => Ok(vec![change_hp_event(state, player, 20)?]),
        "Water" => Ok(vec![turn_draw_bonus_event(state, player, 2)]),
        "Fire" => Ok(vec![change_hp_event(state, &previous, -20)?]),
        "Earth" => Ok(vec![set_shield_event(state, player, 15)]),
        _ => Ok(Vec::new()),
    }
}

fn resonance_primary_events(state: &GameState, player: &PlayerId) -> GameResult<Vec<GameEvent>> {
    let Some((element, _)) = residual_card_facts(state, player) else {
        return Ok(Vec::new());
    };
    let previous =
        TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
    match element {
        Element::Metal => inspect_and_discard_events(state, player, &previous, THOUSAND_RESONANCE),
        Element::Wood => Ok(vec![change_hp_event(state, player, 20)?]),
        Element::Water => Ok(vec![turn_draw_bonus_event(state, player, 2)]),
        Element::Fire => Ok(vec![change_hp_event(state, &previous, -20)?]),
        Element::Earth => Ok(vec![set_shield_event(state, player, 15)]),
    }
}

fn inspect_and_discard_events(
    state: &GameState,
    player: &PlayerId,
    target: &PlayerId,
    effect_id: &str,
) -> GameResult<Vec<GameEvent>> {
    inspect_and_discard_events_with_continuation(
        state,
        player,
        target,
        effect_id,
        "confluence:discard-inspected-card",
    )
}

fn inspect_and_discard_events_with_continuation(
    state: &GameState,
    player: &PlayerId,
    target: &PlayerId,
    effect_id: &str,
    continuation_id: &str,
) -> GameResult<Vec<GameEvent>> {
    let cards = state
        .hand(target)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(target.clone())))?
        .to_vec();
    let mut events = vec![GameEvent::HandInspected {
        viewer: player.clone(),
        target: target.clone(),
        cards: cards.clone(),
    }];
    if !cards.is_empty() {
        events.push(GameEvent::EffectChoiceRequested {
            player: player.clone(),
            kind: crate::domain::PendingChoiceKind::EffectGenerated {
                effect_id: effect_id.to_string(),
                continuation_id: continuation_id.to_string(),
                allowed_cards: cards,
            },
        });
    }
    Ok(events)
}

pub(crate) fn after_effect_choice_events(
    state: &GameState,
    player: &PlayerId,
    effect_id: &str,
    continuation_id: &str,
) -> GameResult<Vec<GameEvent>> {
    if effect_id != THOUSAND_RESONANCE {
        return Ok(Vec::new());
    }
    let Some(selected) = continuation_id.strip_prefix("confluence:thousand-resonance-after:")
    else {
        return Ok(Vec::new());
    };
    resonance_element_events(state, player, selected)
}

fn change_hp_event(state: &GameState, player: &PlayerId, delta: i32) -> GameResult<GameEvent> {
    let team = TurnOrderTargets::new(state).team_of(player)?;
    let old_hp = state
        .hp
        .iter()
        .find(|entry| entry.team == team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?
        .hp;
    let initial = state.initial_hp(&team).unwrap_or(old_hp.max(0));
    let effective_delta = if delta > 0 && crate::rules::jianghu::player_has_poison(state, player) {
        0
    } else {
        delta
    };
    let new_hp = (old_hp + effective_delta).clamp(0, initial);
    Ok(GameEvent::HpChanged {
        change: HpChangeDelta {
            team,
            old_hp,
            delta,
            new_hp,
            effective_delta: new_hp - old_hp,
        },
    })
}

fn set_shield_event(state: &GameState, player: &PlayerId, value: i32) -> GameEvent {
    let old_value = state.shield(player).unwrap_or(0);
    GameEvent::ShieldChanged {
        player: player.clone(),
        old_value,
        delta: value - old_value,
        new_value: value,
    }
}

fn turn_draw_bonus_event(state: &GameState, player: &PlayerId, amount: usize) -> GameEvent {
    let old_value = state
        .turn_draw_bonus_by_player
        .get(player)
        .copied()
        .unwrap_or(0);
    GameEvent::TurnDrawBonusChanged {
        player: player.clone(),
        old_value,
        delta: amount as i32,
        new_value: old_value + amount,
    }
}

fn cannot_act_or_draw(
    state: &GameState,
    player: &PlayerId,
    source: &str,
) -> GameResult<Vec<GameEvent>> {
    let expires_on = TurnOrderTargets::new(state).nth_future_turn_for_player(player, 1)?;
    Ok(["CannotAct", "CannotDraw"]
        .into_iter()
        .map(|kind| GameEvent::StatusAdded {
            status: crate::rules::jianghu::shorten_enemy_status(
                state,
                StatusEffect {
                    id: format!("confluence-{source}-{}-{kind}", state.turn_number),
                    owner: StatusOwner::Player(player.clone()),
                    kind: kind.to_string(),
                    value: None,
                    duration: StatusDuration::UntilTurnEndNumber {
                        player: player.clone(),
                        turn_number: expires_on,
                    },
                },
            ),
        })
        .collect())
}

pub(crate) fn profession_catalog_entries() -> Vec<crate::rules::profession::ProfessionCatalogEntry>
{
    vec![
        entry(
            TUNER_ID,
            "調律師",
            "目前無職業；以餘行牌等級總和３轉職",
            &[],
            &["confluence:tuning"],
        ),
        entry(
            STRING_CHANGER_ID,
            "易弦師",
            "目前為調律師；以餘行牌等級總和６轉職",
            &[TUNER_ID],
            &["confluence:string-changing"],
        ),
        entry(
            HEAVENLY_RESONATOR_ID,
            "天響師",
            "目前為易弦師；以餘行牌等級總和９轉職",
            &[STRING_CHANGER_ID],
            &["confluence:heavenly-resonance"],
        ),
        entry(
            DAO_MAGE_ID,
            "道法師",
            "尋道者以火行６，或法師以木行６轉職",
            &[crate::rules::hero::SEEKER_ID, crate::rules::hero::MAGE_ID],
            &[],
        ),
        entry(
            DAO_SAINT_ID,
            "道法聖",
            "目前為道法師；木５火５",
            &[DAO_MAGE_ID],
            &["confluence:living-dao"],
        ),
        entry(
            CLEAR_WIND_ADEPT_ID,
            "晴風士",
            "目前無職業；１２、２３、３４或４５",
            &[],
            &["confluence:clear-wind"],
        ),
        entry(
            CLEAR_WIND_ENVOY_ID,
            "晴風使",
            "目前為晴風士；１２３、２３４或３４５",
            &[CLEAR_WIND_ADEPT_ID],
            &["confluence:tailwind"],
        ),
        entry(
            VOID_SEEKER_ID,
            "虛空追尋者",
            "自身無職業、星辰、精靈；三張同級２至５級牌",
            &[],
            &["confluence:void-seeking"],
        ),
        entry(
            VOID_DESTROYER_ID,
            "虛空破滅者",
            "僅能由虛空追尋轉職",
            &[VOID_SEEKER_ID],
            &["confluence:void-destruction", "confluence:void-realm"],
        ),
    ]
}

pub(crate) fn playable_profession_changes(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionChangeCandidate>> {
    if !state.has_rule_module(CONFLUENCE_GENERATION_MODULE_ID) {
        return Ok(Vec::new());
    }
    let facts = submitted_card_facts(state, player, cards)?;
    Ok(profession_catalog_entries()
        .into_iter()
        .filter(|profession| change_matches(state, player, &profession.id, &facts))
        .map(|profession| ProfessionChangeCandidate {
            profession_id: profession.id,
            profession_name: profession.name.to_string(),
            rule_text: profession.rule_text.to_string(),
            cards: cards.to_vec(),
        })
        .collect())
}

pub(crate) fn validate_profession_change(
    state: &GameState,
    player: &PlayerId,
    target: &ProfessionId,
    cards: &[CardInstanceId],
) -> GameResult<()> {
    let facts = submitted_card_facts(state, player, cards)?;
    if change_matches(state, player, target, &facts) {
        Ok(())
    } else {
        Err(GameError::Validation(
            ValidationError::ProfessionChangePatternMismatch {
                profession: target.clone(),
            },
        ))
    }
}

fn change_matches(
    state: &GameState,
    player: &PlayerId,
    target: &ProfessionId,
    facts: &[SubmittedCardFacts],
) -> bool {
    let current = state.profession_for(player);
    let residual = residual_card_facts(state, player);
    match target.as_str() {
        TUNER_ID => {
            current.is_none()
                && residual.is_some_and(|(element, _)| {
                    !facts.is_empty()
                        && facts.iter().all(|card| card.element == element)
                        && facts.iter().map(|card| card.level).sum::<u32>() >= 3
                })
        }
        STRING_CHANGER_ID | HEAVENLY_RESONATOR_ID => {
            let (required, minimum) = if target.as_str() == STRING_CHANGER_ID {
                (TUNER_ID, 6)
            } else {
                (STRING_CHANGER_ID, 9)
            };
            current.is_some_and(|profession| profession.as_str() == required)
                && residual.is_some_and(|(element, _)| {
                    !facts.is_empty()
                        && facts.iter().all(|card| card.element == element)
                        && facts.iter().map(|card| card.level).sum::<u32>() >= minimum
                })
        }
        DAO_MAGE_ID => match current.map(ProfessionId::as_str) {
            Some(crate::rules::hero::SEEKER_ID) => one_element_minimum(facts, Element::Fire, 6),
            Some(crate::rules::hero::MAGE_ID) => one_element_minimum(facts, Element::Wood, 6),
            _ => false,
        },
        DAO_SAINT_ID => {
            current.is_some_and(|profession| profession.as_str() == DAO_MAGE_ID)
                && element_minimum(facts, Element::Wood, 5)
                && element_minimum(facts, Element::Fire, 5)
                && facts
                    .iter()
                    .all(|card| matches!(card.element, Element::Wood | Element::Fire))
        }
        CLEAR_WIND_ADEPT_ID => current.is_none() && consecutive_levels(facts, 2),
        CLEAR_WIND_ENVOY_ID => {
            current.is_some_and(|profession| profession.as_str() == CLEAR_WIND_ADEPT_ID)
                && consecutive_levels(facts, 3)
        }
        VOID_SEEKER_ID => {
            current.is_none()
                && state
                    .players
                    .iter()
                    .find(|entry| &entry.id == player)
                    .is_some_and(|entry| state.star_for_team(&entry.team).is_none())
                && state.spirit_for(player).is_none()
                && facts.len() == 3
                && (2..=5).contains(&facts[0].level)
                && facts.iter().all(|card| card.level == facts[0].level)
        }
        VOID_DESTROYER_ID => false,
        _ => false,
    }
}

pub(crate) fn residual_card_facts(state: &GameState, player: &PlayerId) -> Option<(Element, u32)> {
    if let Some(obligation) = state.confluence_card_obligations.iter().find(|obligation| {
        &obligation.owner == player && obligation.applied_on_turn == state.turn_number
    }) && let (Some(element), Some(level)) =
        (obligation.residual_element, obligation.residual_level)
    {
        return Some((element, level));
    }
    let previous = TurnOrderTargets::new(state)
        .player_target(player, RulePlayerTarget::PreviousPlayer)
        .ok()?;
    let discard = state
        .last_turn_discard_by_player
        .get(&previous)
        .filter(|discard| discard.turn_number + 1 == state.turn_number)?;
    let definition = state.card_def(discard.card)?;
    Some((definition.element, definition.level))
}

pub(crate) fn profession_change_satisfies_obligation(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> bool {
    state
        .confluence_card_obligations
        .iter()
        .find(|obligation| {
            &obligation.owner == player && obligation.applied_on_turn == state.turn_number
        })
        .is_none_or(|obligation| cards.contains(&obligation.card))
}

pub(crate) fn active_tuning_obligation<'a>(
    state: &'a GameState,
    player: &PlayerId,
) -> Option<&'a crate::domain::ConfluenceCardObligation> {
    state.confluence_card_obligations.iter().find(|obligation| {
        &obligation.owner == player && obligation.applied_on_turn == state.turn_number
    })
}

pub(crate) fn formation_selection_satisfies_obligation(
    obligation: Option<&crate::domain::ConfluenceCardObligation>,
    formation_id: &str,
    cards: &[CardInstanceId],
) -> bool {
    obligation.is_none_or(|obligation| {
        cards.contains(&obligation.card)
            && obligation.allow_profession_formation
            && is_profession_formation(formation_id)
    })
}

pub(crate) fn tuning_completion_available(
    state: &GameState,
    player: &PlayerId,
) -> GameResult<bool> {
    let Some(obligation) = active_tuning_obligation(state, player).cloned() else {
        return Ok(true);
    };
    let Some(hand) = state.hand(player) else {
        return Ok(false);
    };
    if !hand.contains(&obligation.card) || hand.len() >= usize::BITS as usize {
        return Ok(false);
    }
    for mask in 1usize..(1usize << hand.len()) {
        let selected = hand
            .iter()
            .enumerate()
            .filter_map(|(index, card)| ((mask >> index) & 1 == 1).then_some(*card))
            .collect::<Vec<_>>();
        if !selected.contains(&obligation.card) {
            continue;
        }
        if !crate::rules::profession::playable_profession_changes(state, player, &selected)?
            .is_empty()
        {
            return Ok(true);
        }
        if obligation.allow_profession_formation
            && tuning_profession_formation_available(state, player, &selected)?
        {
            return Ok(true);
        }
    }
    Ok(false)
}

pub(crate) fn obligation_completion_event(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> Option<GameEvent> {
    active_tuning_obligation(state, player)
        .filter(|obligation| cards.contains(&obligation.card))
        .map(|obligation| GameEvent::ConfluenceCardObligationCleared {
            owner: player.clone(),
            card: obligation.card,
        })
}

pub(crate) fn events_preserve_tuning_completion(
    state: &GameState,
    player: &PlayerId,
    events: &[GameEvent],
) -> GameResult<bool> {
    if active_tuning_obligation(state, player).is_none() {
        return Ok(true);
    }
    let mut projected = state.clone();
    for event in events {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    tuning_completion_available(&projected, player)
}

fn tuning_profession_formation_available(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<bool> {
    let facts = submitted_card_facts(state, player, cards)?;
    let residual = residual_card_facts(state, player);
    let limited_uses = state
        .limited_uses
        .iter()
        .filter(|use_count| &use_count.owner == player)
        .cloned()
        .collect::<Vec<_>>();
    Ok(formation_specs().into_iter().any(|spec| {
        can_use_profession_formation(
            &state.enabled_rule_modules,
            state.profession_for(player),
            &spec.formation.id,
        ) && formation_available_for_selection(&limited_uses, &spec.formation.id)
            && formation_matches(&spec.formation.id, residual, &facts)
    }))
}

pub(crate) fn profession_acquired_events(
    state: &GameState,
    player: &PlayerId,
    previous: Option<&ProfessionId>,
    profession: &ProfessionId,
) -> Vec<GameEvent> {
    let mut events = Vec::new();
    match profession.as_str() {
        HEAVENLY_RESONATOR_ID => {
            events.push(set_limited_use(state, player, HEAVENLY_RESONANCE_USE, 1, 1))
        }
        DAO_MAGE_ID => events.push(set_limited_use(state, player, IMPRISONING_ARRAY_USE, 1, 1)),
        DAO_SAINT_ID => {
            let current = limited_use(state, player, IMPRISONING_ARRAY_USE);
            events.push(GameEvent::LimitedUseChanged {
                owner: player.clone(),
                key: IMPRISONING_ARRAY_USE.to_string(),
                old_remaining: current.map_or(0, |use_count| use_count.remaining),
                new_remaining: current.map_or(1, |use_count| use_count.remaining + 1),
                maximum: 2,
            });
        }
        CLEAR_WIND_ENVOY_ID => events.push(set_limited_use(state, player, TAILWIND_USE, 1, 1)),
        VOID_DESTROYER_ID => events.push(set_limited_use(state, player, VOID_REALM_USE, 1, 1)),
        _ => {}
    }
    if previous == Some(profession) {
        // Reacquiring a Profession intentionally follows the same reset path.
    }
    events
}

pub(crate) fn void_transcendence_events(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<GameEvent>> {
    if crate::rules::pouch::profession_is_suppressed(state, player) {
        return Ok(Vec::new());
    }
    let is_void_profession = state.profession_for(player).is_some_and(|profession| {
        crate::rules::profession::inherits_from(
            &state.enabled_rule_modules,
            profession,
            &ProfessionId::new(VOID_SEEKER_ID),
        )
    });
    let uses_three_fives = cards.len() == 3
        && cards
            .iter()
            .all(|card| state.card_level_for(player, *card) == Some(5));
    if !is_void_profession || !uses_three_fives {
        return Ok(Vec::new());
    }
    let previous = state.profession_for(player).cloned();
    let profession = ProfessionId::new(VOID_DESTROYER_ID);
    let opposing =
        TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
    let mut events = vec![
        GameEvent::ProfessionTransformed {
            player: player.clone(),
            previous: previous.clone(),
            profession: profession.clone(),
            reason: "confluence:void-seeking".to_string(),
        },
        change_hp_event(state, &opposing, -20)?,
    ];
    events.extend(profession_acquired_events(
        state,
        player,
        previous.as_ref(),
        &profession,
    ));
    Ok(events)
}

pub(crate) fn void_realm_protects(state: &GameState, player: &PlayerId) -> bool {
    !crate::rules::pouch::profession_is_suppressed(state, player)
        && state
            .profession_for(player)
            .is_some_and(|profession| profession.as_str() == VOID_DESTROYER_ID)
        && limited_use(state, player, VOID_REALM_USE)
            .is_some_and(|use_count| use_count.remaining > 0)
}

pub(crate) fn void_realm_consumption_events(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> Vec<GameEvent> {
    if !cards.iter().all(|card| {
        state
            .card_level_for(player, *card)
            .is_some_and(|level| level >= 3)
    }) {
        return Vec::new();
    }
    state
        .players
        .iter()
        .filter(|entry| void_realm_protects(state, &entry.id))
        .map(|entry| {
            let use_count = limited_use(state, &entry.id, VOID_REALM_USE)
                .expect("protected Void Destroyer has a Limited Use");
            GameEvent::LimitedUseChanged {
                owner: entry.id.clone(),
                key: VOID_REALM_USE.to_string(),
                old_remaining: use_count.remaining,
                new_remaining: use_count.remaining - 1,
                maximum: use_count.maximum,
            }
        })
        .collect()
}

pub(crate) fn limited_use<'a>(
    state: &'a GameState,
    player: &PlayerId,
    key: &str,
) -> Option<&'a crate::domain::LimitedUse> {
    state
        .limited_uses
        .iter()
        .find(|use_count| &use_count.owner == player && use_count.key == key)
}

pub(crate) fn set_limited_use(
    state: &GameState,
    player: &PlayerId,
    key: &str,
    remaining: u32,
    maximum: u32,
) -> GameEvent {
    GameEvent::LimitedUseChanged {
        owner: player.clone(),
        key: key.to_string(),
        old_remaining: limited_use(state, player, key).map_or(0, |use_count| use_count.remaining),
        new_remaining: remaining,
        maximum,
    }
}

/// Explicit Tailwind recovery records.  This is called only after a canonical
/// Discard Shuffle has been projected, so replay never infers recovery from a
/// continuation or from a Deck Shuffle.
pub(crate) fn tailwind_recovery_events(
    state: &GameState,
    shuffled_deck: &RandomnessDeck,
) -> Vec<GameEvent> {
    let eligible = match shuffled_deck {
        RandomnessDeck::Shared => state
            .players
            .iter()
            .map(|player| player.id.clone())
            .collect(),
        RandomnessDeck::Player(player) => vec![player.clone()],
    };
    eligible
        .into_iter()
        .filter(|player| {
            !crate::rules::pouch::profession_is_suppressed(state, player)
                && state
                    .profession_for(player)
                    .is_some_and(|profession| profession.as_str() == CLEAR_WIND_ENVOY_ID)
        })
        .filter_map(|player| {
            let use_count = limited_use(state, &player, TAILWIND_USE)?;
            (use_count.key == TAILWIND_USE && use_count.remaining == 0 && use_count.maximum > 0)
                .then(|| GameEvent::LimitedUseChanged {
                    owner: player,
                    key: TAILWIND_USE.to_string(),
                    old_remaining: 0,
                    new_remaining: use_count.maximum,
                    maximum: use_count.maximum,
                })
        })
        .collect()
}

pub(crate) fn after_clear_wind_randomness_events(state: &GameState) -> GameResult<Vec<GameEvent>> {
    let player = state
        .current_player()
        .cloned()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;
    clear_wind_ten_thousand_miles_after_shuffle(state, &player)
}

fn clear_wind_ten_thousand_miles_after_shuffle(
    state: &GameState,
    player: &PlayerId,
) -> GameResult<Vec<GameEvent>> {
    let drawn = state
        .deck_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
        .iter()
        .take(10)
        .copied()
        .collect::<Vec<_>>();
    let available_hand_space = state.hand_limit.saturating_sub(
        state
            .hand(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
            .len(),
    );
    let maximum = drawn.len().min(available_hand_space);
    let mut events = vec![GameEvent::CardsDrawnForProfessionChoice {
        player: player.clone(),
        ability_id: CLEAR_WIND_TEN_THOUSAND_MILES.to_string(),
        cards: drawn.clone(),
    }];
    if !drawn.is_empty() {
        events.push(GameEvent::EffectChoiceRequested {
            player: player.clone(),
            kind: crate::domain::PendingChoiceKind::CardSetChoice {
                effect_id: CLEAR_WIND_TEN_THOUSAND_MILES.to_string(),
                continuation_id: "confluence:clear-wind:keep-cards".to_string(),
                allowed_cards: drawn,
                minimum: 0,
                maximum,
            },
        });
    }
    Ok(events)
}

fn deck_kind(state: &GameState, player: &PlayerId) -> RandomnessDeck {
    if state.uses_personal_decks() {
        RandomnessDeck::Player(player.clone())
    } else {
        RandomnessDeck::Shared
    }
}

fn submitted_card_facts(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<SubmittedCardFacts>> {
    let hand = state
        .hand(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    let mut seen = std::collections::HashSet::new();
    cards
        .iter()
        .map(|card| {
            if !seen.insert(*card) {
                return Err(GameError::Validation(
                    ValidationError::DuplicateSubmittedCard(*card),
                ));
            }
            if !hand.contains(card) {
                return Err(GameError::Validation(ValidationError::CardNotInHand(*card)));
            }
            let definition = state.card_def(*card).ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))?;
            Ok(SubmittedCardFacts {
                element: definition.element,
                level: state
                    .card_level_for(player, *card)
                    .unwrap_or(definition.level),
            })
        })
        .collect()
}

fn one_element_minimum(facts: &[SubmittedCardFacts], element: Element, minimum: u32) -> bool {
    !facts.is_empty()
        && facts.iter().all(|card| card.element == element)
        && facts.iter().map(|card| card.level).sum::<u32>() >= minimum
}

fn element_minimum(facts: &[SubmittedCardFacts], element: Element, minimum: u32) -> bool {
    facts
        .iter()
        .filter(|card| card.element == element)
        .map(|card| card.level)
        .sum::<u32>()
        >= minimum
}

fn consecutive_levels(facts: &[SubmittedCardFacts], count: usize) -> bool {
    if facts.len() != count {
        return false;
    }
    let mut levels = facts.iter().map(|card| card.level).collect::<Vec<_>>();
    levels.sort_unstable();
    levels.windows(2).all(|pair| pair[1] == pair[0] + 1)
}

fn entry(
    id: &'static str,
    name: &'static str,
    rule_text: &'static str,
    parents: &[&'static str],
    ability_ids: &[&'static str],
) -> crate::rules::profession::ProfessionCatalogEntry {
    crate::rules::profession::ProfessionCatalogEntry {
        id: ProfessionId::new(id),
        module_id: CONFLUENCE_GENERATION_MODULE_ID,
        name,
        rule_text,
        parents: parents
            .iter()
            .map(|parent| ProfessionId::new(*parent))
            .collect(),
        ability_ids: ability_ids.to_vec(),
    }
}

pub(crate) fn effective_ability_summaries(
    enabled_modules: &[RuleModuleId],
    id: &ProfessionId,
) -> Vec<&'static str> {
    crate::rules::profession::effective_ability_ids(enabled_modules, id)
        .into_iter()
        .filter_map(|ability| match ability {
            "confluence:tuning" => Some("調律：棄置高於餘級的牌，取得上家可回收棄牌"),
            "confluence:string-changing" => Some("易弦：調律牌亦可用於自身職業陣法"),
            "confluence:heavenly-resonance" => Some("天響：一次取得上家棄牌且不限制用途"),
            "confluence:living-dao" => Some("道法心生：禁錮法陣使用上限增加為二次"),
            "confluence:clear-wind" => Some("晴風：展示牌堆頂並選擇捨棄或放回"),
            "confluence:tailwind" => Some("順風：一次使本回合抽牌＋２；自身洗牌時回復"),
            "confluence:void-seeking" => Some("虛空追尋：以５５５施展虛空術時轉職"),
            "confluence:void-destruction" => Some("虛空破滅：轉職時對方扣除２０點生命"),
            "confluence:void-realm" => Some("虛空境界：一次防止本職業被破除"),
            _ => None,
        })
        .collect()
}

pub(crate) fn playable_profession_abilities(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionAbilityCandidate>> {
    if !state.has_rule_module(CONFLUENCE_GENERATION_MODULE_ID)
        || state
            .activated_profession_ability_turns
            .get(player)
            .is_some_and(|turn| *turn == state.turn_number)
    {
        return Ok(Vec::new());
    }
    validate_ability_cards(state, player, cards)?;
    let Some(profession) = state.profession_for(player) else {
        return Ok(Vec::new());
    };
    let abilities =
        crate::rules::profession::effective_ability_ids(&state.enabled_rule_modules, profession);
    let mut candidates = Vec::new();
    if cards.len() == 1
        && abilities.contains(&"confluence:tuning")
        && let Some((_, residual_level)) = residual_card_facts(state, player)
        && state
            .card_level_for(player, cards[0])
            .is_some_and(|level| level > residual_level)
        && tuning_card_can_be_used(state, player, cards[0])?
    {
        candidates.push(ability_candidate(
            "confluence:tuning",
            "調律",
            "捨棄此牌，取得上家可回收棄牌；該牌本回合須用於轉職",
            cards,
            None,
            None,
        ));
    }
    if cards.is_empty() {
        if abilities.contains(&"confluence:heavenly-resonance")
            && state.hand(player).is_some_and(|hand| hand.len() <= 4)
            && retrievable_discard(state, player).is_some()
            && limited_use(state, player, HEAVENLY_RESONANCE_USE)
                .is_some_and(|use_count| use_count.remaining > 0)
        {
            candidates.push(ability_candidate(
                "confluence:heavenly-resonance",
                "天響",
                "取得上家可回收棄牌；本局限一次",
                cards,
                None,
                None,
            ));
        }
        if abilities.contains(&"confluence:clear-wind")
            && state
                .deck_for(player)
                .and_then(|deck| deck.first())
                .is_some()
        {
            candidates.push(ability_candidate(
                "confluence:clear-wind",
                "晴風",
                "展示牌堆最上方牌，再選擇捨棄或放回",
                cards,
                None,
                None,
            ));
        }
        if abilities.contains(&"confluence:tailwind")
            && limited_use(state, player, TAILWIND_USE)
                .is_some_and(|use_count| use_count.remaining > 0)
        {
            candidates.push(ability_candidate(
                "confluence:tailwind",
                "順風",
                "本回合抽牌＋２；自身洗牌時回復",
                cards,
                None,
                None,
            ));
        }
    }
    Ok(candidates)
}

fn tuning_card_can_be_used(
    state: &GameState,
    player: &PlayerId,
    cost: CardInstanceId,
) -> GameResult<bool> {
    let Some((previous, retrieved)) = retrievable_discard(state, player) else {
        return Ok(false);
    };
    let mut projected = state.clone();
    projected
        .hand_mut(player)
        .expect("known Player has Hand")
        .retain(|card| *card != cost);
    projected
        .hand_mut(player)
        .expect("known Player has Hand")
        .push(retrieved);
    projected
        .discard_for_mut(&previous)
        .expect("known Player has Discard")
        .retain(|card| *card != retrieved);
    let definition = state
        .card_def(retrieved)
        .expect("retrievable Card has a definition");
    projected
        .confluence_card_obligations
        .push(crate::domain::ConfluenceCardObligation {
            owner: player.clone(),
            card: retrieved,
            allow_profession_formation: crate::rules::profession::ability_ids_in_effect(
                state, player,
            )
            .contains(&"confluence:string-changing"),
            applied_on_turn: state.turn_number,
            residual_element: Some(definition.element),
            residual_level: Some(definition.level),
        });
    tuning_completion_available(&projected, player)
}

pub(crate) fn activate_profession_ability(
    state: &GameState,
    player: &PlayerId,
    ability_id: &str,
    cards: &[CardInstanceId],
    target_card: Option<CardInstanceId>,
    _declared_element: Option<Element>,
    declared_level: Option<u32>,
) -> GameResult<Vec<GameEvent>> {
    let candidates = playable_profession_abilities(state, player, cards)?;
    if !candidates.iter().any(|candidate| {
        candidate.ability_id == ability_id
            && candidate.target_card == target_card
            && candidate.declared_level == declared_level
    }) {
        return Err(GameError::Validation(
            ValidationError::ProfessionAbilityCannotResolve(ability_id.to_string()),
        ));
    }
    let mut events = vec![GameEvent::ProfessionAbilityActivated {
        player: player.clone(),
        ability_id: ability_id.to_string(),
        prepared: None,
    }];
    match ability_id {
        "confluence:tuning" => {
            let retrieved = retrievable_discard(state, player)
                .expect("playable Tuning requires a retrievable discard");
            events.push(GameEvent::CardsMoved {
                card_moves: vec![
                    ability_discard_move(state, player, cards[0]),
                    crate::domain::CardMoveDelta {
                        card: retrieved.1,
                        from: discard_zone_for_player(state, &retrieved.0),
                        to: crate::domain::CardZone::Hand(player.clone()),
                    },
                ],
            });
            let allow_profession_formation =
                crate::rules::profession::ability_ids_in_effect(state, player)
                    .contains(&"confluence:string-changing");
            events.push(GameEvent::ConfluenceCardObligationSet {
                obligation: crate::domain::ConfluenceCardObligation {
                    owner: player.clone(),
                    card: retrieved.1,
                    allow_profession_formation,
                    applied_on_turn: state.turn_number,
                    residual_element: state.card_def(retrieved.1).map(|card| card.element),
                    residual_level: state.card_def(retrieved.1).map(|card| card.level),
                },
            });
        }
        "confluence:heavenly-resonance" => {
            let retrieved = retrievable_discard(state, player)
                .expect("playable Heavenly Resonance requires a discard");
            events.push(GameEvent::CardsMoved {
                card_moves: vec![crate::domain::CardMoveDelta {
                    card: retrieved.1,
                    from: discard_zone_for_player(state, &retrieved.0),
                    to: crate::domain::CardZone::Hand(player.clone()),
                }],
            });
            let use_count = limited_use(state, player, HEAVENLY_RESONANCE_USE).unwrap();
            events.push(GameEvent::LimitedUseChanged {
                owner: player.clone(),
                key: HEAVENLY_RESONANCE_USE.to_string(),
                old_remaining: use_count.remaining,
                new_remaining: use_count.remaining - 1,
                maximum: use_count.maximum,
            });
        }
        "confluence:clear-wind" => {
            let top = state
                .deck_for(player)
                .and_then(|deck| deck.first())
                .copied()
                .expect("playable Clear Wind requires a top Card");
            events.push(GameEvent::DeckTopRevealed {
                player: player.clone(),
                card: top,
            });
            events.push(GameEvent::EffectChoiceRequested {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::CardSetChoice {
                    effect_id: "confluence:clear-wind".to_string(),
                    continuation_id: "confluence:clear-wind:discard-top".to_string(),
                    allowed_cards: vec![top],
                    minimum: 0,
                    maximum: 1,
                },
            });
        }
        "confluence:tailwind" => {
            let use_count = limited_use(state, player, TAILWIND_USE).unwrap();
            events.push(GameEvent::LimitedUseChanged {
                owner: player.clone(),
                key: TAILWIND_USE.to_string(),
                old_remaining: use_count.remaining,
                new_remaining: use_count.remaining - 1,
                maximum: use_count.maximum,
            });
            events.push(turn_draw_bonus_event(state, player, 2));
        }
        _ => {
            return Err(GameError::Validation(
                ValidationError::UnknownProfessionAbility(ability_id.to_string()),
            ));
        }
    }
    Ok(events)
}

fn ability_candidate(
    id: &str,
    name: &str,
    rule_text: &str,
    cards: &[CardInstanceId],
    target_card: Option<CardInstanceId>,
    declared_level: Option<u32>,
) -> ProfessionAbilityCandidate {
    ProfessionAbilityCandidate {
        ability_id: id.to_string(),
        ability_name: name.to_string(),
        rule_text: rule_text.to_string(),
        cards: cards.to_vec(),
        target_card,
        declared_element: None,
        declared_level,
    }
}

fn validate_ability_cards(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<()> {
    let hand = state
        .hand(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    let mut seen = std::collections::HashSet::new();
    for card in cards {
        if !seen.insert(*card) {
            return Err(GameError::Validation(
                ValidationError::DuplicateSubmittedCard(*card),
            ));
        }
        if !hand.contains(card) {
            return Err(GameError::Validation(ValidationError::CardNotInHand(*card)));
        }
    }
    Ok(())
}

fn retrievable_discard(state: &GameState, player: &PlayerId) -> Option<(PlayerId, CardInstanceId)> {
    let previous = TurnOrderTargets::new(state)
        .player_target(player, RulePlayerTarget::PreviousPlayer)
        .ok()?;
    let discard = state
        .last_turn_discard_by_player
        .get(&previous)
        .filter(|discard| discard.turn_number + 1 == state.turn_number)?;
    state
        .discard_for(&previous)
        .filter(|pile| pile.contains(&discard.card))?;
    Some((previous, discard.card))
}

fn ability_discard_move(
    state: &GameState,
    player: &PlayerId,
    card: CardInstanceId,
) -> crate::domain::CardMoveDelta {
    crate::domain::CardMoveDelta {
        card,
        from: crate::domain::CardZone::Hand(player.clone()),
        to: discard_zone_for_card(state, card),
    }
}

fn discard_zone_for_player(state: &GameState, player: &PlayerId) -> crate::domain::CardZone {
    if state.uses_personal_decks() {
        crate::domain::CardZone::PlayerDiscard(player.clone())
    } else {
        crate::domain::CardZone::Discard
    }
}

fn discard_zone_for_card(state: &GameState, card: CardInstanceId) -> crate::domain::CardZone {
    if state.uses_personal_decks() {
        match state.card_origin(card) {
            Some(crate::domain::CardOrigin::Player(owner)) => {
                crate::domain::CardZone::PlayerDiscard(owner.clone())
            }
            _ => crate::domain::CardZone::Discard,
        }
    } else {
        crate::domain::CardZone::Discard
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_contains_every_published_profession_and_formation() {
        let professions = profession_catalog_entries()
            .into_iter()
            .map(|entry| entry.id.as_str().to_string())
            .collect::<Vec<_>>();
        assert_eq!(
            professions,
            [
                TUNER_ID,
                STRING_CHANGER_ID,
                HEAVENLY_RESONATOR_ID,
                DAO_MAGE_ID,
                DAO_SAINT_ID,
                CLEAR_WIND_ADEPT_ID,
                CLEAR_WIND_ENVOY_ID,
                VOID_SEEKER_ID,
                VOID_DESTROYER_ID,
            ]
        );

        let formations = formation_specs()
            .into_iter()
            .map(|spec| spec.formation.id)
            .collect::<Vec<_>>();
        assert_eq!(
            formations,
            [
                MIRROR_RESONANCE,
                FOREST_RESONANCE,
                STREAM_RESONANCE,
                BLAZE_RESONANCE,
                EARTH_RESONANCE,
                THOUSAND_RESONANCE,
                MYRIAD_RESONANCE,
                IMPRISONING_ARRAY,
                ENDLESS_ARRAY,
                WIND_DANCE,
                CLEAR_WIND_TEN_THOUSAND_MILES,
                VOID_BARRIER,
                VOID_RETURN_TO_NOTHING,
            ]
        );
    }
}
