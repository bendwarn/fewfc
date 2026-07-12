use crate::domain::{
    CardInstanceId, Element, GameError, GameEvent, GameResult, GameState, HpChangeDelta,
    JIANGHU_MODULE_ID, JianghuState, JianghuStateKind, PlayerId, ProfessionId, StatusDuration,
    StatusEffect, StatusOwner, ValidationError,
    targeting::{RulePlayerTarget, TurnOrderTargets},
};
use crate::rules::{
    AttackCategory, AttackPlanDef, BaseFormationSpec, DamageTarget, EffectDef, EffectPlan,
    FormationCategory, FormationDef, FormationPattern, PointFormula, SpellPlanDef,
    formation_resolved_on_previous_turn,
};
use crate::rules::{ProfessionAbilityCandidate, ProfessionChangeCandidate, SubmittedCardFacts};

pub(crate) const LONE_WANDERER_ID: &str = "jianghu:lone-wanderer";
pub(crate) const SWORDSMAN_ID: &str = "jianghu:swordsman";
pub(crate) const SWORD_SAGE_ID: &str = "jianghu:sword-sage";
pub(crate) const QI_CULTIVATOR_ID: &str = "jianghu:qi-cultivator";
pub(crate) const QI_GRANDMASTER_ID: &str = "jianghu:qi-grandmaster";
pub(crate) const INK_SEEKER_ID: &str = "jianghu:ink-seeker";
pub(crate) const BOOK_IMMORTAL_ID: &str = "jianghu:book-immortal";
pub(crate) const POISONER_ID: &str = "jianghu:poisoner";
pub(crate) const POISON_SAINT_ID: &str = "jianghu:poison-saint";

pub(crate) const THOUSAND_BLADES_SWORD_ART: &str = "jianghu:thousand-blades-sword-art";
pub(crate) const SNOW_TREADING_SWORD_ART: &str = "jianghu:snow-treading-sword-art";
pub(crate) const FLOWING_SHADOW_SWORD: &str = "jianghu:flowing-shadow-remnant-light-sword";
pub(crate) const THOUSAND_BLADES_FLYING_FEATHER: &str = "jianghu:thousand-blades-flying-feather";
pub(crate) const SNOW_TREADING_REFLECTED_MOON: &str = "jianghu:snow-treading-reflected-moon";
pub(crate) const FLOWING_SHADOW_CLOUD_BREAKING: &str =
    "jianghu:flowing-shadow-cloud-breaking-sword";
pub(crate) const WATER_DOTTING_FAN: &str = "jianghu:water-dotting-fan";
pub(crate) const WIND_RIDING_FAN: &str = "jianghu:wind-riding-fan";
pub(crate) const FAN_BEYOND_HEAVEN: &str = "jianghu:fan-beyond-heaven";
pub(crate) const POISON_DART: &str = "jianghu:poison-dart";
pub(crate) const POISON_SMOKE: &str = "jianghu:poison-smoke";
pub(crate) const THOUSAND_POISON_HAND: &str = "jianghu:thousand-poison-hand";
pub(crate) const LINGERING_FROST_HAND: &str = "jianghu:lingering-frost-hand";
pub(crate) const KING_YAMA_DECREE: &str = "jianghu:king-yama-decree";

pub(crate) fn timed_effect_reductions(
    state: &GameState,
    target: &PlayerId,
) -> Vec<crate::domain::TimedEffectReduction> {
    if !state.has_rule_module(JIANGHU_MODULE_ID) {
        return Vec::new();
    }
    let mut reductions = crate::rules::timed_effect::status_reductions(state, target, |id| {
        id.starts_with("jianghu-fan-beyond-heaven-")
            || id.starts_with("jianghu-lingering-frost-")
            || id.starts_with("jianghu-snow-treading-")
            || id.starts_with("jianghu-water-dotting-fan-")
    });
    reductions.extend(
        state
            .jianghu_states
            .iter()
            .filter(|active| active.owner == *target)
            .map(|active| {
                let (new_remaining_turns, new_expires_on_turn) =
                    if active.kind == JianghuStateKind::Poison {
                        (active.remaining_turns.saturating_sub(1), None)
                    } else {
                        (active.remaining_turns, None)
                    };
                crate::domain::TimedEffectReduction::JianghuState {
                    owner: active.owner.clone(),
                    kind: active.kind,
                    old_remaining_turns: active.remaining_turns,
                    new_remaining_turns,
                    old_expires_on_turn: active.expires_on_turn,
                    new_expires_on_turn,
                }
            }),
    );
    reductions
}

pub(crate) fn formation_specs() -> Vec<BaseFormationSpec> {
    vec![
        attack(
            THOUSAND_BLADES_SWORD_ART,
            "千鋒劍訣",
            "金行牌＋同級牌；物理攻擊，點數＝等級總和×３；進入千鋒",
            AttackCategory::Physical,
            FormationPattern::Custom("metal-and-same-level".to_string()),
            PointFormula::LevelSumTimes(3),
        ),
        attack(
            SNOW_TREADING_SWORD_ART,
            "踏雪劍訣",
            "水行牌＋同級牌；特殊攻擊，點數＝等級總和×３；進入踏雪",
            AttackCategory::Special,
            FormationPattern::Custom("water-and-same-level".to_string()),
            PointFormula::LevelSumTimes(3),
        ),
        attack(
            FLOWING_SHADOW_SWORD,
            "流影殘光劍",
            "三張不同等級牌；特殊攻擊，點數＝等級總和×３",
            AttackCategory::Special,
            FormationPattern::Custom("three-different-levels".to_string()),
            PointFormula::LevelSumTimes(3),
        ),
        attack(
            THOUSAND_BLADES_FLYING_FEATHER,
            "千鋒飛羽",
            "金金＋５級牌；物理攻擊，固定３０點",
            AttackCategory::Physical,
            FormationPattern::Custom("metal-metal-and-level-five".to_string()),
            PointFormula::Fixed(30),
        ),
        attack(
            SNOW_TREADING_REFLECTED_MOON,
            "踏雪映月",
            "水水＋５級牌；特殊攻擊，固定３０點",
            AttackCategory::Special,
            FormationPattern::Custom("water-water-and-level-five".to_string()),
            PointFormula::Fixed(30),
        ),
        attack(
            FLOWING_SHADOW_CLOUD_BREAKING,
            "流影崩雲劍",
            "１至５級牌各一張；特殊攻擊，固定８０點",
            AttackCategory::Special,
            FormationPattern::Custom("levels-one-through-five".to_string()),
            PointFormula::Fixed(80),
        ),
        attack(
            WATER_DOTTING_FAN,
            "點水扇",
            "１級牌；特殊攻擊，固定５點",
            AttackCategory::Special,
            FormationPattern::Custom("single-level-one".to_string()),
            PointFormula::Fixed(5),
        ),
        attack(
            WIND_RIDING_FAN,
            "凌風扇",
            "３級牌；特殊攻擊，固定１０點",
            AttackCategory::Special,
            FormationPattern::Custom("single-level-three".to_string()),
            PointFormula::Fixed(10),
        ),
        attack(
            FAN_BEYOND_HEAVEN,
            "天外飛扇",
            "５級牌；特殊攻擊，固定０點；上家下回合行動後扣２０",
            AttackCategory::Special,
            FormationPattern::Custom("single-level-five".to_string()),
            PointFormula::Fixed(0),
        ),
        spell(
            POISON_DART,
            "毒鏢",
            "金行牌；下家中毒１回合",
            false,
            FormationPattern::ExactElements(vec![Element::Metal]),
        ),
        spell(
            POISON_SMOKE,
            "毒煙",
            "火行牌＋木行牌；翻開時下家中毒１回合",
            true,
            FormationPattern::ExactElements(vec![Element::Fire, Element::Wood]),
        ),
        spell(
            THOUSAND_POISON_HAND,
            "千毒手",
            "三張不同行同級牌；下家扣１０並中毒２回合",
            false,
            FormationPattern::Custom("three-different-elements-same-level".to_string()),
        ),
        spell(
            LINGERING_FROST_HAND,
            "殘霜手",
            "三張水行牌；下家無法行動及抽牌１回合，並中毒２回合",
            false,
            FormationPattern::Custom("three-water".to_string()),
        ),
        spell(
            KING_YAMA_DECREE,
            "閻王令",
            "１級牌；對方生命值４０以下時直接獲勝",
            false,
            FormationPattern::Custom("single-level-one".to_string()),
        ),
    ]
}

fn attack(
    id: &str,
    name: &str,
    rule_text: &str,
    category: AttackCategory,
    pattern: FormationPattern,
    point_formula: PointFormula,
) -> BaseFormationSpec {
    BaseFormationSpec {
        formation: FormationDef {
            id: id.to_string(),
            name: name.to_string(),
            rule_text: rule_text.to_string(),
            category: FormationCategory::Attack,
            pattern,
            effect_id: id.to_string(),
            point_formula: point_formula.clone(),
        },
        effect: EffectDef {
            id: id.to_string(),
            plan: EffectPlan::Attack(AttackPlanDef {
                category,
                point_formula,
                damage_target: DamageTarget::PreviousPlayer,
            }),
        },
    }
}

fn spell(
    id: &str,
    name: &str,
    rule_text: &str,
    passive: bool,
    pattern: FormationPattern,
) -> BaseFormationSpec {
    let plan = SpellPlanDef {
        resolver_id: id.to_string(),
    };
    BaseFormationSpec {
        formation: FormationDef {
            id: id.to_string(),
            name: name.to_string(),
            rule_text: rule_text.to_string(),
            category: FormationCategory::Spell,
            pattern,
            effect_id: id.to_string(),
            point_formula: PointFormula::Fixed(0),
        },
        effect: EffectDef {
            id: id.to_string(),
            plan: if passive {
                EffectPlan::PassiveSpell(plan)
            } else {
                EffectPlan::ActiveSpell(plan)
            },
        },
    }
}

pub(crate) fn is_profession_formation(formation_id: &str) -> bool {
    matches!(
        formation_id,
        THOUSAND_BLADES_SWORD_ART
            | SNOW_TREADING_SWORD_ART
            | FLOWING_SHADOW_SWORD
            | THOUSAND_BLADES_FLYING_FEATHER
            | SNOW_TREADING_REFLECTED_MOON
            | FLOWING_SHADOW_CLOUD_BREAKING
            | WATER_DOTTING_FAN
            | WIND_RIDING_FAN
            | FAN_BEYOND_HEAVEN
            | POISON_DART
            | POISON_SMOKE
            | THOUSAND_POISON_HAND
            | LINGERING_FROST_HAND
            | KING_YAMA_DECREE
    )
}

pub(crate) fn can_use_profession_formation(
    enabled_modules: &[crate::domain::RuleModuleId],
    profession: Option<&ProfessionId>,
    formation_id: &str,
) -> bool {
    let Some(profession) = profession else {
        return false;
    };
    let inherits = |ancestor: &str| {
        crate::rules::profession::inherits_from(
            enabled_modules,
            profession,
            &ProfessionId::new(ancestor),
        )
    };
    match formation_id {
        THOUSAND_BLADES_SWORD_ART | SNOW_TREADING_SWORD_ART | FLOWING_SHADOW_SWORD => {
            inherits(SWORDSMAN_ID)
        }
        THOUSAND_BLADES_FLYING_FEATHER
        | SNOW_TREADING_REFLECTED_MOON
        | FLOWING_SHADOW_CLOUD_BREAKING => profession.as_str() == SWORD_SAGE_ID,
        WATER_DOTTING_FAN | WIND_RIDING_FAN => inherits(INK_SEEKER_ID),
        FAN_BEYOND_HEAVEN => profession.as_str() == BOOK_IMMORTAL_ID,
        POISON_DART | POISON_SMOKE | THOUSAND_POISON_HAND => inherits(POISONER_ID),
        LINGERING_FROST_HAND | KING_YAMA_DECREE => profession.as_str() == POISON_SAINT_ID,
        _ => false,
    }
}

pub(crate) fn has_state(state: &GameState, player: &PlayerId, kind: JianghuStateKind) -> bool {
    state
        .jianghu_states
        .iter()
        .any(|active| &active.owner == player && active.kind == kind)
}

pub(crate) fn team_has_poison(state: &GameState, team: &crate::domain::TeamId) -> bool {
    state.jianghu_states.iter().any(|active| {
        active.kind == JianghuStateKind::Poison
            && state
                .players
                .iter()
                .any(|player| player.id == active.owner && &player.team == team)
    })
}

pub(crate) fn player_has_poison(state: &GameState, player: &PlayerId) -> bool {
    state
        .jianghu_states
        .iter()
        .any(|active| active.kind == JianghuStateKind::Poison && active.owner == *player)
}

pub(crate) fn ignores_other_formation_effects(state: &GameState, player: &PlayerId) -> bool {
    has_state(state, player, JianghuStateKind::SnowTreading)
}

pub(crate) fn modify_attack_points(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
    formula: &PointFormula,
    cards: &[CardInstanceId],
    points: i32,
) -> i32 {
    let abilities = crate::rules::profession::ability_ids_in_effect(state, player);
    let points = if formation_id == "five-streams-unite"
        && (abilities.contains(&"jianghu:pure-yang-force")
            || abilities.contains(&"jianghu:extreme-yang-force"))
    {
        points + 15
    } else {
        points
    };
    if formation_id != THOUSAND_BLADES_SWORD_ART
        && !has_state(state, player, JianghuStateKind::ThousandBlades)
    {
        return points;
    }
    let raised = cards
        .iter()
        .filter_map(|card| state.card_level_for(player, *card))
        .filter(|level| *level < 5)
        .count() as i32;
    match formula {
        PointFormula::LevelPlus(_) => points + raised.min(1),
        PointFormula::LevelSumTimes(multiplier) => points + raised * *multiplier as i32,
        PointFormula::ElementProductTimes {
            element,
            multiplier,
        } => {
            let matching_raised = cards
                .iter()
                .filter(|card| {
                    state
                        .card_def(**card)
                        .is_some_and(|definition| definition.element == *element)
                        && state
                            .card_level_for(player, **card)
                            .is_some_and(|level| level < 5)
                })
                .count() as i32;
            points + matching_raised * *multiplier as i32
        }
        PointFormula::Fixed(_) | PointFormula::TargetHandCountTimes(_) => points,
    }
}

pub(crate) fn halves_incoming_damage(
    state: &GameState,
    player: &PlayerId,
    category: &AttackCategory,
) -> bool {
    if crate::rules::pouch::profession_is_suppressed(state, player)
        || !matches!(
            category,
            AttackCategory::Elemental(Element::Wood | Element::Fire)
        )
    {
        return false;
    }
    state.statuses.iter().any(|status| {
        status.owner == StatusOwner::Player(player.clone()) && status.kind == "JianghuYangAura"
    })
}

pub(crate) fn extreme_yang_applies(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) -> bool {
    formation_id == "five-streams-unite"
        && crate::rules::profession::ability_ids_in_effect(state, player)
            .contains(&"jianghu:extreme-yang-force")
}

pub(crate) fn post_attack_events(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) -> GameResult<Vec<GameEvent>> {
    let mut events = Vec::new();
    if formation_id == THOUSAND_BLADES_SWORD_ART {
        events.push(state_applied_event(
            state,
            player,
            JianghuStateKind::ThousandBlades,
            0,
        )?);
    }
    if formation_id == SNOW_TREADING_SWORD_ART {
        events.push(state_applied_event(
            state,
            player,
            JianghuStateKind::SnowTreading,
            0,
        )?);
    }
    if formation_id == FLOWING_SHADOW_SWORD {
        events.push(GameEvent::CounterEffectEstablished {
            owner: player.clone(),
            effect_id: FLOWING_SHADOW_SWORD.to_string(),
        });
    }
    let thousand = has_state(state, player, JianghuStateKind::ThousandBlades);
    let snow = has_state(state, player, JianghuStateKind::SnowTreading);
    if matches!(
        formation_id,
        THOUSAND_BLADES_FLYING_FEATHER | FLOWING_SHADOW_CLOUD_BREAKING
    ) && thousand
    {
        events.push(hp_loss_to_previous_player(state, player, 20)?);
    }
    if matches!(
        formation_id,
        SNOW_TREADING_REFLECTED_MOON | FLOWING_SHADOW_CLOUD_BREAKING
    ) && snow
    {
        for target in state
            .players
            .iter()
            .map(|entry| &entry.id)
            .filter(|target| *target != player)
        {
            events.extend(cannot_act_or_draw_events(
                state,
                target,
                "snow-treading",
                1,
            )?);
        }
    }
    let abilities = crate::rules::profession::ability_ids_in_effect(state, player);
    if formation_id == "five-streams-unite" && abilities.contains(&"jianghu:extreme-yang-force") {
        for target in state
            .players
            .iter()
            .map(|entry| &entry.id)
            .filter(|target| *target != player)
        {
            events.extend(
                cannot_act_or_draw_events(state, target, "extreme-yang", 1)?
                    .into_iter()
                    .filter(|event| {
                        matches!(
                            event,
                            GameEvent::StatusAdded { status } if status.kind == "CannotAct"
                        )
                    }),
            );
        }
    }
    if state.statuses.iter().any(|status| {
        status.owner == StatusOwner::Player(player.clone()) && status.kind == "JianghuDancingYang"
    }) && cards_are_wood_or_fire_attack(state, player, formation_id)
    {
        events.push(turn_draw_bonus_event(state, player));
    }
    if is_meteor_active(state, player) {
        if formation_id == WIND_RIDING_FAN {
            events.push(turn_draw_bonus_event(state, player));
        }
        if formation_id == FAN_BEYOND_HEAVEN {
            events.push(hp_loss_to_previous_player(state, player, 20)?);
        }
    } else if formation_id == FAN_BEYOND_HEAVEN {
        let previous =
            TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
        let expires_on = TurnOrderTargets::new(state).nth_future_turn_for_player(&previous, 1)?;
        events.push(GameEvent::StatusAdded {
            status: StatusEffect {
                id: format!("jianghu-fan-beyond-heaven-{}", state.turn_number),
                owner: StatusOwner::Player(previous.clone()),
                kind: "JianghuFanBeyondHeaven".to_string(),
                value: Some(20),
                duration: StatusDuration::UntilTurnEndNumber {
                    player: previous,
                    turn_number: expires_on,
                },
            },
        });
    }
    if formation_id == WATER_DOTTING_FAN {
        let previous =
            TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
        let threshold = if is_meteor_active(state, player) {
            3
        } else {
            4
        };
        if formation_resolved_on_previous_turn(state, &previous)
            .is_some_and(|last| last.used_cards.len() >= threshold)
        {
            events.extend(cannot_act_or_draw_events(
                state,
                &previous,
                "water-dotting-fan",
                1,
            )?);
        }
    }
    Ok(events)
}

pub(crate) fn shorten_enemy_status(state: &GameState, mut status: StatusEffect) -> StatusEffect {
    let StatusOwner::Player(owner) = &status.owner else {
        return status;
    };
    let Some(current) = state.current_player() else {
        return status;
    };
    if current == owner {
        return status;
    }
    let has_righteous_spirit = crate::rules::profession::ability_ids_in_effect(state, owner)
        .contains(&"jianghu:righteous-spirit");
    if !has_righteous_spirit {
        return status;
    }
    if let StatusDuration::UntilTurnEndNumber {
        player,
        turn_number,
    } = &mut status.duration
        && let Ok(next_turn) = TurnOrderTargets::new(state).nth_future_turn_for_player(player, 1)
        && *turn_number > next_turn
    {
        *turn_number = next_turn;
    }
    status
}

pub(crate) fn active_spell_events(
    state: &GameState,
    player: &PlayerId,
    resolver_id: &str,
) -> GameResult<Option<Vec<GameEvent>>> {
    let target =
        TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::NextPlayer)?;
    let events = match resolver_id {
        POISON_DART => vec![poison_event(state, &target, 1)],
        THOUSAND_POISON_HAND => {
            vec![
                hp_loss_for_player(state, &target, 10)?,
                poison_event(state, &target, 2),
            ]
        }
        LINGERING_FROST_HAND => {
            let mut events = cannot_act_or_draw_events(state, &target, "lingering-frost", 1)?;
            events.push(poison_event(state, &target, 2));
            events
        }
        KING_YAMA_DECREE => {
            let opposing_team = TurnOrderTargets::new(state).team_of(&target)?;
            let hp = state
                .hp
                .iter()
                .find(|entry| entry.team == opposing_team)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::MissingTeamHp(opposing_team.clone()))
                })?
                .hp;
            if hp > 40 {
                Vec::new()
            } else {
                vec![hp_loss_for_player(state, &target, hp)?]
            }
        }
        _ => return Ok(None),
    };
    Ok(Some(events))
}

pub(crate) fn poison_smoke_flip_events(state: &GameState, events: &[GameEvent]) -> Vec<GameEvent> {
    events
        .iter()
        .filter_map(|event| match event {
            GameEvent::PassiveFlipped {
                incoming_player,
                passive_id,
                ..
            } if passive_id == POISON_SMOKE => Some(poison_event(state, incoming_player, 1)),
            _ => None,
        })
        .collect()
}

pub(crate) fn turn_end_event(state: &GameState) -> GameResult<Option<GameEvent>> {
    let Some(player) = state.current_player() else {
        return Ok(None);
    };
    if let Some(status) = state.statuses.iter().find(|status| {
        status.owner == StatusOwner::Player(player.clone())
            && status.kind == "JianghuFanBeyondHeaven"
            && matches!(
                status.duration,
                StatusDuration::UntilTurnEndNumber { turn_number, .. }
                    if turn_number == state.turn_number
            )
    }) {
        let team = TurnOrderTargets::new(state).team_of(player)?;
        let old_hp = state
            .hp
            .iter()
            .find(|entry| entry.team == team)
            .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?
            .hp;
        let damage = status.value.unwrap_or(20);
        let new_hp = (old_hp - damage).max(0);
        let hp_change = HpChangeDelta {
            team: team.clone(),
            old_hp,
            delta: -damage,
            new_hp,
            effective_delta: new_hp - old_hp,
        };
        let shared_fate_hp_change = if hp_change.effective_delta < 0
            && state
                .spirit_for(player)
                .is_some_and(|owned| owned.spirit == crate::domain::SpiritKind::Death)
        {
            let target =
                TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::NextPlayer)?;
            let target_team = TurnOrderTargets::new(state).team_of(&target)?;
            let target_old_hp = if target_team == team {
                new_hp
            } else {
                state
                    .hp
                    .iter()
                    .find(|entry| entry.team == target_team)
                    .map_or(0, |entry| entry.hp)
            };
            let target_new_hp = (target_old_hp - 10).max(0);
            Some(HpChangeDelta {
                team: target_team,
                old_hp: target_old_hp,
                delta: -10,
                new_hp: target_new_hp,
                effective_delta: target_new_hp - target_old_hp,
            })
        } else {
            None
        };
        return Ok(Some(GameEvent::JianghuDelayedDamageResolved {
            owner: player.clone(),
            status_id: status.id.clone(),
            hp_change,
            shared_fate_hp_change,
        }));
    }
    if let Some(poison) = state.jianghu_states.iter().find(|active| {
        &active.owner == player
            && active.kind == JianghuStateKind::Poison
            && active.remaining_turns > 0
            && active.last_resolved_turn != Some(state.turn_number)
    }) {
        let previous =
            TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
        let damage = if state
            .profession_for(&previous)
            .is_some_and(|profession| profession.as_str() == POISON_SAINT_ID)
        {
            15
        } else {
            10
        };
        let team = TurnOrderTargets::new(state).team_of(player)?;
        let old_hp = state
            .hp
            .iter()
            .find(|entry| entry.team == team)
            .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?
            .hp;
        let new_hp = (old_hp - damage).max(0);
        let hp_change = HpChangeDelta {
            team: team.clone(),
            old_hp,
            delta: -damage,
            new_hp,
            effective_delta: new_hp - old_hp,
        };
        let shared_fate_hp_change = shared_fate_change(state, player, &hp_change)?;
        return Ok(Some(GameEvent::JianghuPoisonTicked {
            owner: player.clone(),
            damage,
            remaining_turns: poison.remaining_turns - 1,
            hp_change,
            shared_fate_hp_change,
        }));
    }
    if let Some(active) = state.jianghu_states.iter().find(|active| {
        &active.owner == player
            && active.kind != JianghuStateKind::Poison
            && active.expires_on_turn == Some(state.turn_number)
    }) {
        return Ok(Some(GameEvent::JianghuStateExpired {
            owner: player.clone(),
            kind: active.kind,
        }));
    }
    Ok(None)
}

fn shared_fate_change(
    state: &GameState,
    owner: &PlayerId,
    primary: &HpChangeDelta,
) -> GameResult<Option<HpChangeDelta>> {
    if primary.effective_delta >= 0
        || state
            .spirit_for(owner)
            .is_none_or(|owned| owned.spirit != crate::domain::SpiritKind::Death)
    {
        return Ok(None);
    }
    let target = TurnOrderTargets::new(state).player_target(owner, RulePlayerTarget::NextPlayer)?;
    let team = TurnOrderTargets::new(state).team_of(&target)?;
    let old_hp = if team == primary.team {
        primary.new_hp
    } else {
        state
            .hp
            .iter()
            .find(|entry| entry.team == team)
            .map_or(0, |entry| entry.hp)
    };
    let new_hp = (old_hp - 10).max(0);
    Ok(Some(HpChangeDelta {
        team,
        old_hp,
        delta: -10,
        new_hp,
        effective_delta: new_hp - old_hp,
    }))
}

pub(crate) fn playable_profession_abilities(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionAbilityCandidate>> {
    if !state.has_rule_module(JIANGHU_MODULE_ID)
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
    if cards.is_empty() && abilities.contains(&"jianghu:dancing-yang-art") {
        candidates.push(ability_candidate(
            "jianghu:dancing-yang-art",
            "舞陽訣",
            "本回合以至少三張木／火行牌攻擊時，抽牌＋１",
            cards,
            None,
        ));
    }
    if cards.is_empty() && abilities.contains(&"jianghu:divine-yang-aura") {
        candidates.push(ability_candidate(
            "jianghu:heavenly-yang-aura",
            "神陽罡",
            "不需展示手牌；一輪內受到木、火行攻擊的傷害減半",
            cards,
            None,
        ));
    }
    if cards.len() == 1 {
        let card = state.card_def(cards[0]).expect("validated card");
        if matches!(card.element, Element::Wood | Element::Fire) {
            if abilities.contains(&"jianghu:heavenly-yang-aura") {
                candidates.push(ability_candidate(
                    "jianghu:heavenly-yang-aura",
                    "天陽罡",
                    "亮出此牌；一輪內受到木、火行攻擊的傷害減半",
                    cards,
                    None,
                ));
            }
            if abilities.contains(&"jianghu:blazing-yang-art") {
                candidates.push(ability_candidate(
                    "jianghu:blazing-yang-art",
                    "烈陽訣",
                    "此牌本回合施展基礎陣法時等級＋２（最高５）",
                    cards,
                    Some(cards[0]),
                ));
            }
        }
        if matches!(card.element, Element::Wood | Element::Water)
            && abilities.contains(&"jianghu:azure-cloud-step")
            && state.deck_for(player).is_some_and(|deck| deck.len() >= 2)
        {
            candidates.push(ability_candidate(
                "jianghu:azure-cloud-step",
                "青雲步",
                "捨棄此牌，抽二張，再選一張放回牌堆頂",
                cards,
                None,
            ));
        }
        if abilities.contains(&"jianghu:meteor-step")
            && is_meteor_step_card(state, player, cards[0])
        {
            candidates.push(ability_candidate(
                "jianghu:meteor-step",
                "流星步",
                "捨棄此火行牌或星行牌；本回合陣法觸發流星效果",
                cards,
                None,
            ));
        }
    }
    Ok(candidates)
}

pub(crate) fn activate_profession_ability(
    state: &GameState,
    player: &PlayerId,
    ability_id: &str,
    cards: &[CardInstanceId],
    target_card: Option<CardInstanceId>,
    _declared_element: Option<Element>,
    _declared_level: Option<u32>,
) -> GameResult<Vec<GameEvent>> {
    if state
        .activated_profession_ability_turns
        .get(player)
        .is_some_and(|turn| *turn == state.turn_number)
    {
        return Err(GameError::Validation(
            ValidationError::ProfessionAbilityAlreadyActivated {
                player: player.clone(),
                turn_number: state.turn_number,
            },
        ));
    }
    let available = playable_profession_abilities(state, player, cards)?;
    let candidate = available
        .iter()
        .find(|candidate| {
            candidate.ability_id == ability_id
                && (candidate.target_card == target_card || target_card.is_none())
        })
        .ok_or_else(|| {
            GameError::Validation(ValidationError::ProfessionAbilityCannotResolve(
                ability_id.to_string(),
            ))
        })?;
    let mut events = vec![GameEvent::ProfessionAbilityActivated {
        player: player.clone(),
        ability_id: ability_id.to_string(),
        prepared: None,
    }];
    match ability_id {
        "jianghu:heavenly-yang-aura" => {
            let expires_on = TurnOrderTargets::new(state).nth_future_turn_for_player(player, 1)?;
            events.push(GameEvent::StatusAdded {
                status: StatusEffect {
                    id: format!("jianghu-heavenly-yang-aura-{}", state.turn_number),
                    owner: StatusOwner::Player(player.clone()),
                    kind: "JianghuYangAura".to_string(),
                    value: None,
                    duration: StatusDuration::UntilTurnEndNumber {
                        player: player.clone(),
                        turn_number: expires_on,
                    },
                },
            });
        }
        "jianghu:blazing-yang-art" => {
            let card_id = target_card.unwrap_or(candidate.cards[0]);
            let definition = state.card_def(card_id).expect("validated card");
            let level = state
                .card_level_for(player, card_id)
                .expect("validated card level")
                .saturating_add(2)
                .min(5);
            events[0] = GameEvent::ProfessionAbilityActivated {
                player: player.clone(),
                ability_id: ability_id.to_string(),
                prepared: Some(crate::domain::PreparedProfessionAbility {
                    player: player.clone(),
                    ability_id: ability_id.to_string(),
                    card: card_id,
                    element: definition.element,
                    level,
                    allowed_formation_scope: vec!["base".to_string()],
                    prepared_on_turn: state.turn_number,
                    interpretation_revision: state.card_interpretation_revision + 1,
                }),
            };
        }
        "jianghu:dancing-yang-art" | "jianghu:meteor-step" => {
            events.push(GameEvent::StatusAdded {
                status: StatusEffect {
                    id: format!("{ability_id}-{}", state.turn_number),
                    owner: StatusOwner::Player(player.clone()),
                    kind: if ability_id.ends_with("meteor-step") {
                        "JianghuMeteor".to_string()
                    } else {
                        "JianghuDancingYang".to_string()
                    },
                    value: None,
                    duration: StatusDuration::UntilTurnEndNumber {
                        player: player.clone(),
                        turn_number: state.turn_number,
                    },
                },
            });
            if ability_id.ends_with("meteor-step") {
                events.push(GameEvent::CardsMoved {
                    card_moves: ability_card_moves(state, player, cards),
                });
            }
        }
        "jianghu:azure-cloud-step" => {
            events.push(GameEvent::CardsMoved {
                card_moves: ability_card_moves(state, player, cards),
            });
            let drawn_cards = state
                .deck_for(player)
                .expect("validated deck")
                .iter()
                .take(2)
                .copied()
                .collect::<Vec<_>>();
            events.push(GameEvent::CardsDrawnForProfessionChoice {
                player: player.clone(),
                ability_id: ability_id.to_string(),
                cards: drawn_cards.clone(),
            });
            events.push(GameEvent::EffectChoiceRequested {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::EffectGenerated {
                    effect_id: ability_id.to_string(),
                    continuation_id: "jianghu:azure-cloud-step:return-one".to_string(),
                    allowed_cards: drawn_cards,
                },
            });
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
) -> ProfessionAbilityCandidate {
    ProfessionAbilityCandidate {
        ability_id: id.to_string(),
        ability_name: name.to_string(),
        rule_text: rule_text.to_string(),
        cards: cards.to_vec(),
        target_card,
        declared_element: None,
        declared_level: None,
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

fn ability_card_moves(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> Vec<crate::domain::CardMoveDelta> {
    cards
        .iter()
        .map(|card| crate::domain::CardMoveDelta {
            card: *card,
            from: crate::domain::CardZone::Hand(player.clone()),
            to: if state.uses_personal_decks() {
                state
                    .card_origin(*card)
                    .map_or(crate::domain::CardZone::Discard, |origin| match origin {
                        crate::domain::CardOrigin::Shared => crate::domain::CardZone::Discard,
                        crate::domain::CardOrigin::Player(owner) => {
                            crate::domain::CardZone::PlayerDiscard(owner.clone())
                        }
                    })
            } else {
                crate::domain::CardZone::Discard
            },
        })
        .collect()
}

fn is_meteor_step_card(state: &GameState, player: &PlayerId, card: CardInstanceId) -> bool {
    let Some(definition) = state.card_def(card) else {
        return false;
    };
    if definition.element == Element::Fire {
        return true;
    }
    let Some(team) = state
        .players
        .iter()
        .find(|entry| &entry.id == player)
        .map(|entry| &entry.team)
    else {
        return false;
    };
    state
        .star_for_team(team)
        .is_some_and(|star| definition.element == crate::rules::star::element(star))
}

fn is_meteor_active(state: &GameState, player: &PlayerId) -> bool {
    state.statuses.iter().any(|status| {
        status.owner == StatusOwner::Player(player.clone()) && status.kind == "JianghuMeteor"
    })
}

fn cards_are_wood_or_fire_attack(state: &GameState, player: &PlayerId, formation_id: &str) -> bool {
    state
        .last_formation_by_player
        .get(player)
        .filter(|last| last.formation_id == formation_id && last.used_cards.len() >= 3)
        .is_some_and(|last| {
            last.used_cards
                .iter()
                .filter(|card| {
                    state.card_def(**card).is_some_and(|definition| {
                        matches!(definition.element, Element::Wood | Element::Fire)
                    })
                })
                .count()
                >= 3
        })
}

fn turn_draw_bonus_event(state: &GameState, player: &PlayerId) -> GameEvent {
    let old_value = state
        .turn_draw_bonus_by_player
        .get(player)
        .copied()
        .unwrap_or(0);
    GameEvent::TurnDrawBonusChanged {
        player: player.clone(),
        old_value,
        delta: 1,
        new_value: old_value + 1,
    }
}

fn state_applied_event(
    state: &GameState,
    player: &PlayerId,
    kind: JianghuStateKind,
    remaining_turns: u32,
) -> GameResult<GameEvent> {
    Ok(GameEvent::JianghuStateApplied {
        state: JianghuState {
            owner: player.clone(),
            kind,
            remaining_turns,
            expires_on_turn: (kind != JianghuStateKind::Poison)
                .then(|| TurnOrderTargets::new(state).nth_future_turn_for_player(player, 1))
                .transpose()?,
            last_resolved_turn: None,
        },
    })
}

fn poison_event(state: &GameState, player: &PlayerId, turns: u32) -> GameEvent {
    let existing = state
        .jianghu_states
        .iter()
        .find(|active| &active.owner == player && active.kind == JianghuStateKind::Poison)
        .map_or(0, |active| active.remaining_turns);
    GameEvent::JianghuStateApplied {
        state: JianghuState {
            owner: player.clone(),
            kind: JianghuStateKind::Poison,
            remaining_turns: existing + turns,
            expires_on_turn: None,
            last_resolved_turn: None,
        },
    }
}

fn hp_loss_to_previous_player(
    state: &GameState,
    player: &PlayerId,
    amount: i32,
) -> GameResult<GameEvent> {
    let target =
        TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
    hp_loss_for_player(state, &target, amount)
}

fn hp_loss_for_player(state: &GameState, player: &PlayerId, amount: i32) -> GameResult<GameEvent> {
    let team = TurnOrderTargets::new(state).team_of(player)?;
    let old_hp = state
        .hp
        .iter()
        .find(|entry| entry.team == team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?
        .hp;
    let new_hp = (old_hp - amount).max(0);
    Ok(GameEvent::HpChanged {
        change: HpChangeDelta {
            team,
            old_hp,
            delta: -amount,
            new_hp,
            effective_delta: new_hp - old_hp,
        },
    })
}

fn cannot_act_or_draw_events(
    state: &GameState,
    player: &PlayerId,
    source: &str,
    turns: usize,
) -> GameResult<Vec<GameEvent>> {
    let expires_on = TurnOrderTargets::new(state).nth_future_turn_for_player(player, turns)?;
    Ok(["CannotAct", "CannotDraw"]
        .into_iter()
        .map(|kind| {
            let status = StatusEffect {
                id: format!(
                    "jianghu-{source}-{}-{}",
                    kind.to_ascii_lowercase(),
                    state.turn_number
                ),
                owner: StatusOwner::Player(player.clone()),
                kind: kind.to_string(),
                value: None,
                duration: StatusDuration::UntilTurnEndNumber {
                    player: player.clone(),
                    turn_number: expires_on,
                },
            };
            GameEvent::StatusAdded {
                status: shorten_enemy_status(state, status),
            }
        })
        .collect())
}

pub(crate) fn profession_catalog_entries() -> Vec<crate::rules::profession::ProfessionCatalogEntry>
{
    vec![
        entry(
            LONE_WANDERER_ID,
            "獨行客",
            "目前無職業；一張３級牌",
            &[],
            &[],
        ),
        entry(
            SWORDSMAN_ID,
            "劍客",
            "目前為初行客或獨行客；金３水３",
            &[],
            &[],
        ),
        entry(
            SWORD_SAGE_ID,
            "劍尊",
            "目前為劍客；金５水５",
            &[SWORDSMAN_ID],
            &[],
        ),
        entry(
            QI_CULTIVATOR_ID,
            "煉氣者",
            "目前為初行客或獨行客；木３火３",
            &[],
            &[
                "jianghu:heavenly-yang-aura",
                "jianghu:blazing-yang-art",
                "jianghu:pure-yang-force",
            ],
        ),
        entry(
            QI_GRANDMASTER_ID,
            "氣宗",
            "目前為煉氣者；木５火５",
            &[QI_CULTIVATOR_ID],
            &[
                "jianghu:divine-yang-aura",
                "jianghu:dancing-yang-art",
                "jianghu:extreme-yang-force",
            ],
        ),
        entry(
            INK_SEEKER_ID,
            "尋墨客",
            "目前為初行客或獨行客；兩張３級以上同行同級牌",
            &[],
            &["jianghu:azure-cloud-step"],
        ),
        entry(
            BOOK_IMMORTAL_ID,
            "書仙",
            "目前為尋墨客；兩張同行５級牌",
            &[INK_SEEKER_ID],
            &["jianghu:righteous-spirit", "jianghu:meteor-step"],
        ),
        entry(
            POISONER_ID,
            "毒師",
            "目前為初行客或獨行客；兩張不同行１級牌",
            &[],
            &[],
        ),
        entry(
            POISON_SAINT_ID,
            "毒聖",
            "目前為毒師；三張不同行１級牌",
            &[POISONER_ID],
            &["jianghu:poison-mastery"],
        ),
    ]
}

pub(crate) fn effective_ability_summaries(
    enabled_modules: &[crate::domain::RuleModuleId],
    id: &ProfessionId,
) -> Vec<&'static str> {
    crate::rules::profession::effective_ability_ids(enabled_modules, id)
        .into_iter()
        .filter_map(|ability| match ability {
            "jianghu:heavenly-yang-aura" => {
                Some("天陽罡：亮出木／火行牌，一輪內受到木、火行傷害減半")
            }
            "jianghu:blazing-yang-art" => {
                Some("烈陽訣：指定木／火行牌本回合施展基礎陣法時等級＋２")
            }
            "jianghu:pure-yang-force" => Some("純陽勁：五流歸一及其複製效果點數＋１５"),
            "jianghu:divine-yang-aura" => Some("神陽罡：天陽罡不需亮牌且常駐"),
            "jianghu:dancing-yang-art" => Some("舞陽訣：木／火牌三張以上攻擊時抽牌＋１"),
            "jianghu:extreme-yang-force" => {
                Some("極陽勁：五流歸一先破上家護盾，其他玩家無法行動１回合")
            }
            "jianghu:azure-cloud-step" => Some("青雲步：棄木／水牌，抽二張並將一張放回牌堆頂"),
            "jianghu:righteous-spirit" => {
                Some("浩然正氣：敵方陣法的二回合以上持續效果縮短為一回合")
            }
            "jianghu:meteor-step" => Some("流星步：棄火行牌或星行牌，本回合觸發流星效果"),
            "jianghu:poison-mastery" => Some("毒絕：自己造成的中毒每回合扣１５點生命"),
            _ => None,
        })
        .collect()
}

pub(crate) fn profession_formation_summaries(
    enabled_modules: &[crate::domain::RuleModuleId],
    id: &ProfessionId,
) -> Vec<(String, String)> {
    formation_specs()
        .into_iter()
        .filter(|spec| can_use_profession_formation(enabled_modules, Some(id), &spec.formation.id))
        .map(|spec| (spec.formation.name, spec.formation.rule_text))
        .collect()
}

pub(crate) fn playable_profession_changes(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionChangeCandidate>> {
    if !state.has_rule_module(JIANGHU_MODULE_ID) {
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
    let first_or_lone = current.is_some_and(|profession| {
        matches!(
            profession.as_str(),
            crate::rules::hero::FIRST_WANDERER_ID | LONE_WANDERER_ID
        )
    });
    match target.as_str() {
        LONE_WANDERER_ID => current.is_none() && facts.len() == 1 && facts[0].level == 3,
        SWORDSMAN_ID => {
            first_or_lone && dual_element_minimums(facts, Element::Metal, 3, Element::Water, 3)
        }
        SWORD_SAGE_ID => {
            current.is_some_and(|profession| profession.as_str() == SWORDSMAN_ID)
                && dual_element_minimums(facts, Element::Metal, 5, Element::Water, 5)
        }
        QI_CULTIVATOR_ID => {
            first_or_lone && dual_element_minimums(facts, Element::Wood, 3, Element::Fire, 3)
        }
        QI_GRANDMASTER_ID => {
            current.is_some_and(|profession| profession.as_str() == QI_CULTIVATOR_ID)
                && dual_element_minimums(facts, Element::Wood, 5, Element::Fire, 5)
        }
        INK_SEEKER_ID => {
            first_or_lone && facts.len() == 2 && facts[0] == facts[1] && facts[0].level >= 3
        }
        BOOK_IMMORTAL_ID => {
            current.is_some_and(|profession| profession.as_str() == INK_SEEKER_ID)
                && facts.len() == 2
                && facts[0] == facts[1]
                && facts[0].level == 5
        }
        POISONER_ID => {
            first_or_lone
                && facts.len() == 2
                && facts.iter().all(|card| card.level == 1)
                && facts[0].element != facts[1].element
        }
        POISON_SAINT_ID => {
            current.is_some_and(|profession| profession.as_str() == POISONER_ID)
                && facts.len() == 3
                && facts.iter().all(|card| card.level == 1)
                && facts
                    .iter()
                    .map(|card| card.element)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == 3
        }
        _ => false,
    }
}

fn dual_element_minimums(
    facts: &[SubmittedCardFacts],
    first: Element,
    first_minimum: u32,
    second: Element,
    second_minimum: u32,
) -> bool {
    !facts.is_empty()
        && facts
            .iter()
            .all(|card| matches!(card.element, element if element == first || element == second))
        && facts
            .iter()
            .filter(|card| card.element == first)
            .map(|card| card.level)
            .sum::<u32>()
            >= first_minimum
        && facts
            .iter()
            .filter(|card| card.element == second)
            .map(|card| card.level)
            .sum::<u32>()
            >= second_minimum
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
                    .expect("known Card must have an effective level"),
            })
        })
        .collect()
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
        module_id: JIANGHU_MODULE_ID,
        name,
        rule_text,
        parents: parents
            .iter()
            .map(|parent| ProfessionId::new(*parent))
            .collect(),
        ability_ids: ability_ids.to_vec(),
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
                LONE_WANDERER_ID,
                SWORDSMAN_ID,
                SWORD_SAGE_ID,
                QI_CULTIVATOR_ID,
                QI_GRANDMASTER_ID,
                INK_SEEKER_ID,
                BOOK_IMMORTAL_ID,
                POISONER_ID,
                POISON_SAINT_ID,
            ]
        );

        let formations = formation_specs()
            .into_iter()
            .map(|spec| spec.formation.id)
            .collect::<Vec<_>>();
        assert_eq!(
            formations,
            [
                THOUSAND_BLADES_SWORD_ART,
                SNOW_TREADING_SWORD_ART,
                FLOWING_SHADOW_SWORD,
                THOUSAND_BLADES_FLYING_FEATHER,
                SNOW_TREADING_REFLECTED_MOON,
                FLOWING_SHADOW_CLOUD_BREAKING,
                WATER_DOTTING_FAN,
                WIND_RIDING_FAN,
                FAN_BEYOND_HEAVEN,
                POISON_DART,
                POISON_SMOKE,
                THOUSAND_POISON_HAND,
                LINGERING_FROST_HAND,
                KING_YAMA_DECREE,
            ]
        );
    }
}
