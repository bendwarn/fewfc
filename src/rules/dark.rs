use crate::domain::{
    CardInstanceId, DARK_GLIMMER_MODULE_ID, Element, GameError, GameEvent, GameResult, GameState,
    HpChangeDelta, PlayerId, ProfessionId, RuleModuleId, SpiritKind, StatusDuration, StatusEffect,
    StatusOwner, ValidationError,
    targeting::{RulePlayerTarget, TurnOrderTargets},
};
use crate::rules::{
    AttackCategory, AttackPlanDef, BaseFormationSpec, ConsequenceCertainty, DamageTarget,
    EffectDef, EffectPlan, FormationCategory, FormationDef, FormationEffect, FormationPattern,
    PointFormula, ProfessionAbilityCandidate, ProfessionAbilityEffect, ProfessionChangeCandidate,
    RuleConsequence, SpellPlanDef, SubmittedCardFacts, TrustedRandomness,
};

pub(crate) fn timed_effect_reductions(
    state: &GameState,
    target: &PlayerId,
) -> Vec<crate::domain::TimedEffectReduction> {
    if !state.has_rule_module(crate::domain::DARK_GLIMMER_MODULE_ID) {
        return Vec::new();
    }
    crate::rules::timed_effect::status_reductions(state, target, |id| id.starts_with("dark-"))
}

pub(crate) const DARK_WALKER_ID: &str = "dark:dark-walker";
pub(crate) const DARK_SPIRIT_ENVOY_ID: &str = "dark:dark-spirit-envoy";
pub(crate) const SHADOW_WARRIOR_ID: &str = "dark:shadow-warrior";
pub(crate) const SHADOW_BERSERKER_ID: &str = "dark:shadow-berserker";
pub(crate) const DEMON_SPIRIT_MASTER_ID: &str = "dark:demon-spirit-master";

pub(crate) const DARK_RADIANCE: &str = "dark:dark-radiance";
pub(crate) const DARK_BARRIER: &str = "dark:dark-barrier";
pub(crate) const DARK_RETURN_TO_ORIGIN: &str = "dark:dark-return-to-origin";
pub(crate) const DARK_SHOCK_BURST: &str = "dark:dark-shock-burst";
pub(crate) const DARK_CHAOS: &str = "dark:dark-chaos";
pub(crate) const DARK_CYCLE: &str = "dark:dark-cycle";
pub(crate) const AFTERIMAGE_SLASH: &str = "dark:afterimage-slash";
pub(crate) const BERSERK_AFTERIMAGE_SLASH: &str = "dark:berserk-afterimage-slash";
pub(crate) const EVIL_SPIRIT_SUMMONING: &str = "dark:evil-spirit-summoning";
pub(crate) const DEATH_SPIRIT_SUMMONING: &str = "dark:death-spirit-summoning";

pub(crate) fn profession_catalog_entries() -> Vec<crate::rules::profession::ProfessionCatalogEntry>
{
    vec![
        entry(
            DARK_WALKER_ID,
            "暗行者",
            "目前無職業；一張２級以下牌",
            &[],
            &["dark:dark-walking"],
        ),
        entry(
            DARK_SPIRIT_ENVOY_ID,
            "暗靈使",
            "僅能由暗行能力轉職",
            &[DARK_WALKER_ID],
            &["dark:dark-spirit", "dark:dark-realm"],
        ),
        entry(
            SHADOW_WARRIOR_ID,
            "影戰士",
            "風行者以金行６，或戰士以土行６轉職",
            &[
                crate::rules::hero::WINDWALKER_ID,
                crate::rules::hero::WARRIOR_ID,
            ],
            &[],
        ),
        entry(
            SHADOW_BERSERKER_ID,
            "影戰狂",
            "目前為影戰士；土５金５",
            &[SHADOW_WARRIOR_ID],
            &["dark:berserk-shadow"],
        ),
        entry(
            DEMON_SPIRIT_MASTER_ID,
            "魔靈師",
            "目前擁有精靈；以２２或４４轉職",
            &[],
            &["dark:demon-spirit-possession", "dark:demon-spirit-revival"],
        ),
    ]
}

pub(crate) fn playable_profession_changes(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionChangeCandidate>> {
    if !state.has_rule_module(DARK_GLIMMER_MODULE_ID) {
        return Ok(Vec::new());
    }
    let facts = submitted_card_facts(state, player, cards)?;
    Ok(profession_catalog_entries()
        .into_iter()
        .filter(|profession| change_matches(state, player, &profession.id, &facts))
        .map(|profession| ProfessionChangeCandidate {
            profession_id: profession.id,
            profession_name: profession.name.to_string(),
            cards: cards.to_vec(),
            detail: crate::rules::PlayerFacingActionDetail::pending_composition(),
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
    match target.as_str() {
        DARK_WALKER_ID => current.is_none() && facts.len() == 1 && facts[0].level <= 2,
        DARK_SPIRIT_ENVOY_ID => false,
        SHADOW_WARRIOR_ID => match current.map(ProfessionId::as_str) {
            Some(crate::rules::hero::WINDWALKER_ID) => {
                one_element_minimum(facts, Element::Metal, 6)
            }
            Some(crate::rules::hero::WARRIOR_ID) => one_element_minimum(facts, Element::Earth, 6),
            _ => false,
        },
        SHADOW_BERSERKER_ID => {
            current.is_some_and(|profession| profession.as_str() == SHADOW_WARRIOR_ID)
                && facts
                    .iter()
                    .all(|card| matches!(card.element, Element::Metal | Element::Earth))
                && element_minimum(facts, Element::Metal, 5)
                && element_minimum(facts, Element::Earth, 5)
        }
        DEMON_SPIRIT_MASTER_ID => {
            state.spirit_for(player).is_some()
                && facts.len() == 2
                && matches!(facts[0].level, 2 | 4)
                && facts[1].level == facts[0].level
        }
        _ => false,
    }
}

pub(crate) fn formation_specs() -> Vec<BaseFormationSpec> {
    vec![
        spell(
            DARK_RADIANCE,
            "暗黑光芒",
            "金金火水，等級和１０以下；下家扣１５且無法行動及抽牌２回合",
        ),
        spell(
            DARK_BARRIER,
            "暗黑氣壁",
            "木木金火，等級和１０以下；防護罩＝等級和×６",
        ),
        spell(
            DARK_RETURN_TO_ORIGIN,
            "暗黑歸元",
            "水水土木，等級和１０以下；回復＝等級和×６",
        ),
        attack(
            DARK_SHOCK_BURST,
            "暗黑震暴",
            "火火水土，等級和１０以下；物理攻擊＝等級和×６",
            AttackCategory::Physical,
            PointFormula::LevelSumTimes(6),
        ),
        spell(
            DARK_CHAOS,
            "暗黑混沌",
            "土土木金，等級和１０以下；下家扣１５並隨機將兩張手牌放回牌堆頂",
        ),
        spell(
            DARK_CYCLE,
            "暗行輪迴",
            "金木水火土，等級和１０以下；下家扣５０，我方回復５０並破除此職業",
        ),
        attack(
            AFTERIMAGE_SLASH,
            "殘影斬",
            "土＋任意牌；物理攻擊８點，上家另扣８點生命",
            AttackCategory::Physical,
            PointFormula::Fixed(8),
        ),
        attack(
            BERSERK_AFTERIMAGE_SLASH,
            "殘影狂刃斬",
            "金土，等級和為偶級；物理攻擊＝等級和×３，上家另扣等量生命",
            AttackCategory::Physical,
            PointFormula::LevelSumTimes(3),
        ),
        spell(
            EVIL_SPIRIT_SUMMONING,
            "惡靈召喚陣",
            "２２；召喚惡精靈，已有時靈力＋２",
        ),
        spell(
            DEATH_SPIRIT_SUMMONING,
            "死靈召喚陣",
            "４４；召喚死精靈，已有時靈力＋２",
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
                player_facing_effect: player_facing_formation_effect(id),
            }),
        },
    }
}

fn player_facing_formation_effect(id: &str) -> FormationEffect {
    match id {
        DARK_RADIANCE | DARK_CHAOS => FormationEffect::InspectHand,
        DARK_BARRIER => FormationEffect::CreateShield,
        DARK_RETURN_TO_ORIGIN => FormationEffect::RecoverHp,
        DARK_CYCLE => FormationEffect::BreakProfession,
        EVIL_SPIRIT_SUMMONING | DEATH_SPIRIT_SUMMONING => FormationEffect::SummonSpirit,
        _ => panic!("Dark formation `{id}` is missing a player-facing effect fact"),
    }
}

pub(crate) fn formation_action_detail_consequences(id: &str) -> Option<Vec<RuleConsequence>> {
    (id == DARK_CHAOS).then(|| {
        vec![RuleConsequence::TrustedRandomness {
            certainty: ConsequenceCertainty::Random,
            operation: TrustedRandomness::SelectHiddenHandCards { count: 2 },
        }]
    })
}

fn attack(
    id: &str,
    name: &str,
    rule_text: &str,
    category: AttackCategory,
    point_formula: PointFormula,
) -> BaseFormationSpec {
    BaseFormationSpec {
        formation: FormationDef {
            id: id.to_string(),
            name: name.to_string(),
            rule_text: rule_text.to_string(),
            category: FormationCategory::Attack,
            pattern: FormationPattern::Custom(id.to_string()),
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

pub(crate) fn is_profession_formation(id: &str) -> bool {
    id.starts_with("dark:") && formation_specs().iter().any(|spec| spec.formation.id == id)
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
        DARK_RADIANCE | DARK_BARRIER | DARK_RETURN_TO_ORIGIN | DARK_SHOCK_BURST | DARK_CHAOS => {
            inherits(DARK_WALKER_ID)
        }
        DARK_CYCLE => profession.as_str() == DARK_SPIRIT_ENVOY_ID,
        AFTERIMAGE_SLASH => inherits(SHADOW_WARRIOR_ID),
        BERSERK_AFTERIMAGE_SLASH => profession.as_str() == SHADOW_BERSERKER_ID,
        EVIL_SPIRIT_SUMMONING | DEATH_SPIRIT_SUMMONING => {
            profession.as_str() == DEMON_SPIRIT_MASTER_ID
        }
        _ => false,
    }
}

pub(crate) fn formation_matches(id: &str, cards: &[SubmittedCardFacts]) -> bool {
    let exact_and_max = |elements: &[Element], maximum| {
        cards.len() == elements.len()
            && element_counts(cards) == element_slice_counts(elements)
            && cards.iter().map(|card| card.level).sum::<u32>() <= maximum
    };
    match id {
        DARK_RADIANCE => exact_and_max(
            &[
                Element::Metal,
                Element::Metal,
                Element::Fire,
                Element::Water,
            ],
            10,
        ),
        DARK_BARRIER => exact_and_max(
            &[Element::Wood, Element::Wood, Element::Metal, Element::Fire],
            10,
        ),
        DARK_RETURN_TO_ORIGIN => exact_and_max(
            &[
                Element::Water,
                Element::Water,
                Element::Earth,
                Element::Wood,
            ],
            10,
        ),
        DARK_SHOCK_BURST => exact_and_max(
            &[Element::Fire, Element::Fire, Element::Water, Element::Earth],
            10,
        ),
        DARK_CHAOS => exact_and_max(
            &[
                Element::Earth,
                Element::Earth,
                Element::Wood,
                Element::Metal,
            ],
            10,
        ),
        DARK_CYCLE => exact_and_max(
            &[
                Element::Metal,
                Element::Wood,
                Element::Water,
                Element::Fire,
                Element::Earth,
            ],
            10,
        ),
        AFTERIMAGE_SLASH => {
            cards.len() == 2 && cards.iter().any(|card| card.element == Element::Earth)
        }
        BERSERK_AFTERIMAGE_SLASH => {
            cards.len() == 2
                && cards.iter().any(|card| card.element == Element::Metal)
                && cards.iter().any(|card| card.element == Element::Earth)
                && cards.iter().map(|card| card.level).sum::<u32>() % 2 == 0
        }
        EVIL_SPIRIT_SUMMONING => cards.len() == 2 && cards.iter().all(|card| card.level == 2),
        DEATH_SPIRIT_SUMMONING => cards.len() == 2 && cards.iter().all(|card| card.level == 4),
        _ => false,
    }
}

pub(crate) fn profession_acquired_events(
    state: &GameState,
    player: &PlayerId,
    profession: &ProfessionId,
    cards: &[CardInstanceId],
) -> Vec<GameEvent> {
    if profession.as_str() != DEMON_SPIRIT_MASTER_ID {
        return Vec::new();
    }
    let Some(owned) = state.spirit_for(player) else {
        return Vec::new();
    };
    let target = cards
        .first()
        .and_then(|card| state.card_level_for(player, *card))
        .and_then(|level| match level {
            2 => Some(SpiritKind::Evil),
            4 => Some(SpiritKind::Death),
            _ => None,
        });
    target
        .map(|spirit| GameEvent::SpiritTransformed {
            player: player.clone(),
            previous: owned.spirit,
            spirit,
            power: owned.power,
        })
        .into_iter()
        .collect()
}

pub(crate) fn pre_formation_events(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) -> Vec<GameEvent> {
    if !crate::rules::pouch::profession_is_suppressed(state, player)
        && is_dark_formation(formation_id)
        && state
            .profession_for(player)
            .is_some_and(|profession| profession.as_str() == DARK_WALKER_ID)
    {
        vec![GameEvent::ProfessionTransformed {
            player: player.clone(),
            previous: Some(ProfessionId::new(DARK_WALKER_ID)),
            profession: ProfessionId::new(DARK_SPIRIT_ENVOY_ID),
            reason: "dark:dark-walking".to_string(),
        }]
    } else {
        Vec::new()
    }
}

pub(crate) fn is_dark_formation(id: &str) -> bool {
    matches!(
        id,
        DARK_RADIANCE
            | DARK_BARRIER
            | DARK_RETURN_TO_ORIGIN
            | DARK_SHOCK_BURST
            | DARK_CHAOS
            | DARK_CYCLE
    )
}

pub(crate) fn ignores_environment(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) -> bool {
    !crate::rules::pouch::profession_is_suppressed(state, player)
        && is_dark_formation(formation_id)
        && state.profession_for(player).is_some_and(|profession| {
            matches!(profession.as_str(), DARK_WALKER_ID | DARK_SPIRIT_ENVOY_ID)
        })
}

pub(crate) fn modify_attack_points(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
    cards: &[CardInstanceId],
    points: i32,
) -> i32 {
    let berserk = crate::rules::profession::ability_ids_in_effect(state, player)
        .contains(&"dark:berserk-shadow");
    if !berserk {
        return points;
    }
    let all_element = |element| {
        cards.iter().all(|card| {
            state
                .card_def(*card)
                .is_some_and(|definition| definition.element == element)
        })
    };
    if (formation_id == "weapon" && cards.len() == 2 && all_element(Element::Metal))
        || (formation_id == AFTERIMAGE_SLASH && cards.len() == 2 && all_element(Element::Earth))
    {
        points * 2
    } else {
        points
    }
}

pub(crate) fn active_spell_events(
    state: &GameState,
    player: &PlayerId,
    resolver_id: &str,
    used_cards: &[CardInstanceId],
    trusted_random_cards: Option<&[CardInstanceId]>,
) -> GameResult<Option<Vec<GameEvent>>> {
    let next = TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::NextPlayer)?;
    let events = match resolver_id {
        DARK_RADIANCE => {
            if crate::rules::hero::target_ignores_disruptive_spell(state, player, resolver_id)? {
                return Ok(Some(Vec::new()));
            }
            let mut events = vec![hp_event_for_player(state, &next, -15)?];
            events.extend(cannot_act_or_draw(state, &next, "dark-radiance", 2)?);
            events
        }
        DARK_BARRIER => vec![set_shield_event(
            state,
            player,
            level_sum(state, player, used_cards)? * 6,
        )],
        DARK_RETURN_TO_ORIGIN => vec![hp_event_for_player(
            state,
            player,
            level_sum(state, player, used_cards)? * 6,
        )?],
        DARK_CHAOS => {
            if crate::rules::hero::target_ignores_disruptive_spell(state, player, resolver_id)? {
                return Ok(Some(Vec::new()));
            }
            let mut events = vec![hp_event_for_player(state, &next, -15)?];
            let returned =
                validate_trusted_random_hand_cards(state, &next, trusted_random_cards, DARK_CHAOS)?;
            if !returned.is_empty() {
                events.push(GameEvent::CardsMoved {
                    card_moves: returned
                        .into_iter()
                        .rev()
                        .map(|card| crate::domain::CardMoveDelta {
                            card,
                            from: crate::domain::CardZone::Hand(next.clone()),
                            to: if state.uses_personal_decks() {
                                crate::domain::CardZone::PlayerDeckTop(next.clone())
                            } else {
                                crate::domain::CardZone::DeckTop
                            },
                        })
                        .collect(),
                });
            }
            events
        }
        DARK_CYCLE => vec![
            hp_event_for_player(state, &next, -50)?,
            hp_event_for_player(state, player, 50)?,
            GameEvent::ProfessionBroken {
                player: player.clone(),
                profession: ProfessionId::new(DARK_SPIRIT_ENVOY_ID),
            },
        ],
        EVIL_SPIRIT_SUMMONING => summon_or_charge_events(state, player, SpiritKind::Evil),
        DEATH_SPIRIT_SUMMONING => summon_or_charge_events(state, player, SpiritKind::Death),
        _ => return Ok(None),
    };
    Ok(Some(events))
}

pub(crate) fn validate_trusted_random_hand_cards(
    state: &GameState,
    target: &PlayerId,
    selected: Option<&[CardInstanceId]>,
    effect_id: &str,
) -> GameResult<Vec<CardInstanceId>> {
    let hand = state
        .hand(target)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(target.clone())))?;
    let selected = selected.ok_or_else(|| {
        GameError::Validation(ValidationError::TrustedRandomSelectionRequired {
            effect_id: effect_id.to_string(),
        })
    })?;
    let required = hand.len().min(2);
    if selected.len() != required {
        return Err(GameError::Validation(
            ValidationError::TrustedRandomSelectionRequired {
                effect_id: effect_id.to_string(),
            },
        ));
    }
    let mut unique = std::collections::HashSet::new();
    for card in selected {
        if !unique.insert(*card) {
            return Err(GameError::Validation(ValidationError::DuplicateChoiceCard(
                *card,
            )));
        }
        if !hand.contains(card) {
            return Err(GameError::Validation(ValidationError::IllegalChoiceCard(
                *card,
            )));
        }
    }
    Ok(selected.to_vec())
}

pub(crate) fn post_attack_events(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
    used_cards: &[CardInstanceId],
) -> GameResult<Vec<GameEvent>> {
    let previous =
        TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)?;
    match formation_id {
        AFTERIMAGE_SLASH => Ok(vec![hp_event_for_player(state, &previous, -8)?]),
        BERSERK_AFTERIMAGE_SLASH => {
            let amount = level_sum(state, player, used_cards)? * 3;
            let mut events = vec![hp_event_for_player(state, &previous, -amount)?];
            for target in state
                .players
                .iter()
                .map(|entry| &entry.id)
                .filter(|target| *target != player)
            {
                events.extend(cannot_act_or_draw(state, target, "berserk-afterimage", 1)?);
            }
            events.push(GameEvent::ProfessionBroken {
                player: player.clone(),
                profession: ProfessionId::new(SHADOW_BERSERKER_ID),
            });
            Ok(events)
        }
        _ => Ok(Vec::new()),
    }
}

pub(crate) fn append_shared_fate_events(
    state: &GameState,
    performer: &PlayerId,
    formation_id: &str,
    events: &mut Vec<GameEvent>,
) -> GameResult<()> {
    if formation_id == "void-spirit-shattering" {
        return Ok(());
    }
    let mut affected = std::collections::HashSet::new();
    for event in events.iter() {
        match event {
            GameEvent::AttackResolved {
                target, hp_change, ..
            } if hp_change.effective_delta < 0 => {
                affected.insert(target.clone());
            }
            GameEvent::HpChanged { change } if change.effective_delta < 0 => {
                affected.extend(affected_players_for_hp_change(
                    state,
                    performer,
                    formation_id,
                    &change.team,
                )?);
            }
            GameEvent::EnvironmentCleared { hp_changes, .. } => {
                for change in hp_changes
                    .iter()
                    .filter(|change| change.effective_delta < 0)
                {
                    affected.extend(players_for_team(state, &change.team));
                }
            }
            GameEvent::StarBroken {
                team,
                hp_change: Some(change),
                ..
            } if change.effective_delta < 0 => {
                affected.extend(players_for_team(state, team));
            }
            GameEvent::VoidReversionResolved { hp_change, .. } if hp_change.effective_delta < 0 => {
                affected.insert(performer.clone());
            }
            _ => {}
        }
    }
    let death_owners = state
        .spirits
        .iter()
        .filter(|owned| owned.spirit == SpiritKind::Death && affected.contains(&owned.player))
        .map(|owned| owned.player.clone())
        .collect::<Vec<_>>();
    if death_owners.is_empty() {
        return Ok(());
    }
    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    let mut counts = std::collections::HashMap::new();
    for owner in death_owners {
        let target =
            TurnOrderTargets::new(state).player_target(&owner, RulePlayerTarget::NextPlayer)?;
        let team = TurnOrderTargets::new(state).team_of(&target)?;
        *counts.entry(team).or_insert(0_i32) += 1;
    }
    for (team, count) in counts {
        let old_hp = projected
            .hp
            .iter()
            .find(|entry| entry.team == team)
            .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?
            .hp;
        let delta = count.saturating_mul(-10);
        let new_hp = (old_hp + delta).max(0);
        let event = GameEvent::HpChanged {
            change: HpChangeDelta {
                team,
                old_hp,
                delta,
                new_hp,
                effective_delta: new_hp - old_hp,
            },
        };
        crate::rules::projection::apply_event(&mut projected, &event);
        events.push(event);
    }
    Ok(())
}

pub(crate) fn append_mischief_events(
    state: &GameState,
    events: &mut Vec<GameEvent>,
) -> GameResult<()> {
    let inspected = events
        .iter()
        .filter_map(|event| match event {
            GameEvent::HandInspected {
                viewer,
                target,
                cards,
            } if state
                .spirit_for(viewer)
                .is_some_and(|owned| owned.spirit == SpiritKind::Evil) =>
            {
                Some((viewer.clone(), target.clone(), cards.clone()))
            }
            _ => None,
        })
        .collect::<Vec<_>>();
    if inspected.is_empty() {
        return Ok(());
    }
    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    for (_, target, cards) in inspected {
        let highest = cards
            .iter()
            .filter_map(|card| state.card_def(*card).map(|definition| definition.level))
            .max()
            .unwrap_or(0);
        if highest == 0 {
            continue;
        }
        let event = hp_event_for_player(&projected, &target, -(highest as i32 * 2))?;
        crate::rules::projection::apply_event(&mut projected, &event);
        events.push(event);
    }
    Ok(())
}

fn affected_players_for_hp_change(
    state: &GameState,
    performer: &PlayerId,
    formation_id: &str,
    changed_team: &crate::domain::TeamId,
) -> GameResult<Vec<PlayerId>> {
    let direct_target = if matches!(
        formation_id,
        DARK_RADIANCE
            | DARK_CHAOS
            | DARK_CYCLE
            | crate::rules::jianghu::THOUSAND_POISON_HAND
            | crate::rules::jianghu::KING_YAMA_DECREE
            | "holy-wind"
    ) {
        Some(TurnOrderTargets::new(state).player_target(performer, RulePlayerTarget::NextPlayer)?)
    } else if matches!(
        formation_id,
        AFTERIMAGE_SLASH
            | BERSERK_AFTERIMAGE_SLASH
            | crate::rules::jianghu::THOUSAND_BLADES_FLYING_FEATHER
            | crate::rules::jianghu::FLOWING_SHADOW_CLOUD_BREAKING
            | crate::rules::jianghu::FAN_BEYOND_HEAVEN
            | crate::rules::confluence::BLAZE_RESONANCE
            | crate::rules::confluence::THOUSAND_RESONANCE
            | crate::rules::confluence::MYRIAD_RESONANCE
            | "shadow-assault"
            | "instant-shadow-death"
    ) {
        Some(
            TurnOrderTargets::new(state)
                .player_target(performer, RulePlayerTarget::PreviousPlayer)?,
        )
    } else {
        None
    };
    Ok(direct_target
        .filter(|target| {
            state
                .players
                .iter()
                .find(|entry| &entry.id == target)
                .is_some_and(|entry| &entry.team == changed_team)
        })
        .map(|target| vec![target])
        .unwrap_or_else(|| players_for_team(state, changed_team)))
}

fn players_for_team(state: &GameState, team: &crate::domain::TeamId) -> Vec<PlayerId> {
    state
        .players
        .iter()
        .filter(|entry| &entry.team == team)
        .map(|entry| entry.id.clone())
        .collect()
}

pub(crate) fn playable_profession_abilities(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionAbilityCandidate>> {
    if cards.len() != 1
        || state
            .activated_profession_ability_turns
            .get(player)
            .is_some_and(|turn| *turn == state.turn_number)
    {
        return Ok(Vec::new());
    }
    let Some(profession) = state.profession_for(player) else {
        return Ok(Vec::new());
    };
    if !crate::rules::profession::effective_ability_ids(&state.enabled_rule_modules, profession)
        .contains(&"dark:dark-spirit")
    {
        return Ok(Vec::new());
    }
    let hand = state
        .hand(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if !hand.contains(&cards[0]) {
        return Err(GameError::Validation(ValidationError::CardNotInHand(
            cards[0],
        )));
    }
    let level = state.card_level_for(player, cards[0]).unwrap_or(1);
    Ok((1..=2)
        .filter(|target_level| *target_level < level)
        .map(|target_level| ProfessionAbilityCandidate {
            ability_id: "dark:dark-spirit".to_string(),
            ability_name: "暗靈".to_string(),
            cards: cards.to_vec(),
            target_card: Some(cards[0]),
            declared_element: state.card_def(cards[0]).map(|card| card.element),
            declared_level: Some(target_level),
            input_requirement: None,
            detail: crate::rules::PlayerFacingActionDetail::pending_composition(),
        })
        .collect())
}

/// The only currently activated Dark ability prepares a specific existing
/// card; its declared element and level are separate action-detail facts.
pub(crate) fn player_facing_ability_effect(id: &str) -> Option<ProfessionAbilityEffect> {
    (id == "dark:dark-spirit").then_some(ProfessionAbilityEffect::PrepareCardAtDeclaredLevel)
}

pub(crate) fn activate_profession_ability(
    state: &GameState,
    player: &PlayerId,
    ability_id: &str,
    cards: &[CardInstanceId],
    target_card: Option<CardInstanceId>,
    declared_element: Option<Element>,
    declared_level: Option<u32>,
) -> GameResult<Vec<GameEvent>> {
    let legal = playable_profession_abilities(state, player, cards)?;
    let candidate = legal
        .iter()
        .find(|candidate| {
            candidate.ability_id == ability_id
                && candidate.target_card == target_card
                && candidate.declared_level == declared_level
        })
        .ok_or_else(|| {
            GameError::Validation(ValidationError::ProfessionAbilityCannotResolve(
                ability_id.to_string(),
            ))
        })?;
    Ok(vec![
        GameEvent::ProfessionAbilityActivated {
            player: player.clone(),
            ability_id: ability_id.to_string(),
            prepared: Some(crate::domain::PreparedProfessionAbility {
                player: player.clone(),
                ability_id: ability_id.to_string(),
                card: candidate.target_card.unwrap(),
                element: declared_element.unwrap_or(candidate.declared_element.unwrap()),
                level: candidate.declared_level.unwrap(),
                allowed_formation_scope: vec!["all".to_string()],
                prepared_on_turn: state.turn_number,
                interpretation_revision: state.card_interpretation_revision + 1,
            }),
        },
        GameEvent::FormationRequirementSet {
            requirement: crate::domain::FormationRequirement {
                player: player.clone(),
                physical_card: candidate.target_card,
                virtual_card: None,
                allowed_formation_scope: vec!["all".to_string()],
                applied_on_turn: state.turn_number,
            },
        },
    ])
}

pub(crate) fn effective_ability_summaries(
    modules: &[RuleModuleId],
    profession: &ProfessionId,
) -> Vec<&'static str> {
    crate::rules::profession::effective_ability_ids(modules, profession)
        .into_iter()
        .filter_map(|ability| match ability {
            "dark:dark-walking" => Some("暗行：不能轉職；施展暗黑陣法時轉為暗靈使"),
            "dark:dark-spirit" => Some("暗靈：將手牌降為１或２級並於本回合施展"),
            "dark:dark-realm" => Some("暗境：暗黑陣法不因環境無效"),
            "dark:berserk-shadow" => Some("狂影：金金武器與土土殘影斬點數雙倍"),
            "dark:demon-spirit-possession" => Some("魔靈附體：轉職時保留靈力並轉化精靈"),
            "dark:demon-spirit-revival" => Some("魔靈復甦：碎靈破除惡／死精靈時召回並破職"),
            _ => None,
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

fn summon_or_charge_events(
    state: &GameState,
    player: &PlayerId,
    spirit: SpiritKind,
) -> Vec<GameEvent> {
    if let Some(owned) = state.spirit_for(player)
        && owned.spirit == spirit
    {
        let new_power = (owned.power + 2).min(6);
        vec![GameEvent::SpiritPowerChanged {
            player: player.clone(),
            spirit,
            old_power: owned.power,
            delta: new_power as i32 - owned.power as i32,
            new_power,
            reason: crate::domain::SpiritPowerChangeReason::SkillEffect,
        }]
    } else {
        vec![GameEvent::SpiritSummoned {
            player: player.clone(),
            previous: state.spirit_for(player).map(|owned| owned.spirit),
            spirit,
        }]
    }
}

fn level_sum(state: &GameState, player: &PlayerId, cards: &[CardInstanceId]) -> GameResult<i32> {
    cards.iter().try_fold(0, |sum, card| {
        state
            .card_level_for(player, *card)
            .map(|level| sum + level as i32)
            .ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))
    })
}

fn hp_event_for_player(state: &GameState, player: &PlayerId, delta: i32) -> GameResult<GameEvent> {
    let team = TurnOrderTargets::new(state).team_of(player)?;
    let old_hp = state
        .hp
        .iter()
        .find(|entry| entry.team == team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?
        .hp;
    let effective_delta = if delta > 0 && crate::rules::jianghu::team_has_poison(state, &team) {
        0
    } else {
        delta
    };
    let new_hp =
        (old_hp + effective_delta).clamp(0, state.initial_hp(&team).unwrap_or(old_hp.max(0)));
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

fn cannot_act_or_draw(
    state: &GameState,
    player: &PlayerId,
    source: &str,
    turns: usize,
) -> GameResult<Vec<GameEvent>> {
    let expires_on = TurnOrderTargets::new(state).nth_future_turn_for_player(player, turns)?;
    Ok(["CannotAct", "CannotDraw"]
        .into_iter()
        .map(|kind| GameEvent::StatusAdded {
            status: crate::rules::jianghu::shorten_enemy_status(
                state,
                StatusEffect {
                    id: format!("dark-{source}-{}-{kind}", state.turn_number),
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

fn element_counts(cards: &[SubmittedCardFacts]) -> std::collections::HashMap<Element, usize> {
    cards.iter().fold(Default::default(), |mut counts, card| {
        *counts.entry(card.element).or_default() += 1;
        counts
    })
}

fn element_slice_counts(elements: &[Element]) -> std::collections::HashMap<Element, usize> {
    elements
        .iter()
        .fold(Default::default(), |mut counts, element| {
            *counts.entry(*element).or_default() += 1;
            counts
        })
}

fn entry(
    id: &'static str,
    name: &'static str,
    rule_text: &'static str,
    parents: &[&'static str],
    abilities: &[&'static str],
) -> crate::rules::profession::ProfessionCatalogEntry {
    crate::rules::profession::ProfessionCatalogEntry {
        id: ProfessionId::new(id),
        module_id: DARK_GLIMMER_MODULE_ID,
        name,
        rule_text,
        parents: parents
            .iter()
            .map(|parent| ProfessionId::new(*parent))
            .collect(),
        ability_ids: abilities.to_vec(),
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
                DARK_WALKER_ID,
                DARK_SPIRIT_ENVOY_ID,
                SHADOW_WARRIOR_ID,
                SHADOW_BERSERKER_ID,
                DEMON_SPIRIT_MASTER_ID,
            ]
        );

        let formations = formation_specs()
            .into_iter()
            .map(|spec| spec.formation.id)
            .collect::<Vec<_>>();
        assert_eq!(
            formations,
            [
                DARK_RADIANCE,
                DARK_BARRIER,
                DARK_RETURN_TO_ORIGIN,
                DARK_SHOCK_BURST,
                DARK_CHAOS,
                DARK_CYCLE,
                AFTERIMAGE_SLASH,
                BERSERK_AFTERIMAGE_SLASH,
                EVIL_SPIRIT_SUMMONING,
                DEATH_SPIRIT_SUMMONING,
            ]
        );
    }
}
