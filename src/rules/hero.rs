use crate::domain::{
    CardInstanceId, CardMoveDelta, CardOrigin, CardZone, Element, GameError, GameEvent, GameResult,
    GameState, HERO_SCHOOLS_MODULE_ID, HeroRandomnessContinuation, HpChangeDelta, PlayerId,
    ProfessionId, RandomnessContinuation, RandomnessDeck, RandomnessOperation, StatusDuration,
    StatusEffect, StatusOwner, ValidationError,
    targeting::{RulePlayerTarget, TurnOrderTargets},
};

use super::{
    ActionInputRequirement, AttackCategory, AttackPlanDef, BaseFormationSpec, DamageTarget,
    EffectDef, EffectPlan, FormationCategory, FormationDef, FormationEffect, FormationPattern,
    PointFormula, ProfessionAbilityCandidate, ProfessionAbilityEffect, ProfessionChangeCandidate,
    SubmittedCardFacts, VirtualFormationScope,
};

pub(crate) const WARRIOR_ID: &str = "warrior";
pub(crate) const WAR_GOD_ID: &str = "war-god";
pub(crate) const HERO_ID: &str = "hero";
pub(crate) const SEEKER_ID: &str = "seeker";
pub(crate) const EXPOUNDER_ID: &str = "expounder";
pub(crate) const BENEVOLENT_ID: &str = "benevolent";
pub(crate) const MESMER_ID: &str = "mesmer";
pub(crate) const SPIRIT_MESMER_ID: &str = "spirit-mesmer";
pub(crate) const HERMIT_ID: &str = "hermit";
pub(crate) const MAGE_ID: &str = "mage";
pub(crate) const MAGE_GUIDE_ID: &str = "mage-guide";
pub(crate) const SAGE_ID: &str = "sage";
pub(crate) const WINDWALKER_ID: &str = "windwalker";
pub(crate) const SHADOW_WALKER_ID: &str = "shadow-walker";
pub(crate) const MARTIAL_ARTIST_ID: &str = "martial-artist";
pub(crate) const FIRST_WANDERER_ID: &str = "first-wanderer";
pub(crate) const IMMORTAL_ID: &str = "immortal";
pub(crate) const SAINT_ID: &str = "saint";

pub(crate) fn timed_effect_reductions(
    state: &GameState,
    target: &PlayerId,
) -> Vec<crate::domain::TimedEffectReduction> {
    if !state.has_rule_module(HERO_SCHOOLS_MODULE_ID) {
        return Vec::new();
    }
    crate::rules::timed_effect::status_reductions(state, target, |id| {
        id.starts_with("magic-reflection-")
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ProfessionAbility {
    PhysicalDamageResistance,
    WeaponProficiency,
    DefenseProficiency,
    CountershockProficiency,
    WeaponMastery,
    ShockBurstProficiency,
    MetalResistance,
    BattleSoul,
    SeekerDiscount,
    GeneratingFormationProficiency,
    OvercomingFormationProficiency,
    ReturnToOriginProficiency,
    FiveElementsCycleProficiency,
    WoodResistance,
    SpellProtection,
    Illusion,
    SealProficiency,
    IllusionRefinement,
    BarrierProficiency,
    Phantasm,
    WaterResistance,
    TripleElementProficiency,
    RadianceProficiency,
    FiveStreamsUniteProficiency,
    FireResistance,
    ArcaneEssence,
    Windwalking,
    MetamorphosisProficiency,
    ShadowCut,
    ChaosProficiency,
    EarthResistance,
    ShadowEscape,
    Choice,
    Breakthrough,
    ImmortalDrawBonus,
    Meditation,
    SacredArt,
    Revelation,
}

impl ProfessionAbility {
    pub(crate) fn id(self) -> &'static str {
        match self {
            Self::PhysicalDamageResistance => "hero:physical-damage-resistance",
            Self::WeaponProficiency => "hero:weapon-proficiency",
            Self::DefenseProficiency => "hero:defense-proficiency",
            Self::CountershockProficiency => "hero:countershock-proficiency",
            Self::WeaponMastery => "hero:weapon-mastery",
            Self::ShockBurstProficiency => "hero:shock-burst-proficiency",
            Self::MetalResistance => "hero:metal-resistance",
            Self::BattleSoul => "hero:battle-soul",
            Self::SeekerDiscount => "hero:seeker-discount",
            Self::GeneratingFormationProficiency => "hero:generating-formation-proficiency",
            Self::OvercomingFormationProficiency => "hero:overcoming-formation-proficiency",
            Self::ReturnToOriginProficiency => "hero:return-to-origin-proficiency",
            Self::FiveElementsCycleProficiency => "hero:five-elements-cycle-proficiency",
            Self::WoodResistance => "hero:wood-resistance",
            Self::SpellProtection => "hero:spell-protection",
            Self::Illusion => "hero:illusion",
            Self::SealProficiency => "hero:seal-proficiency",
            Self::IllusionRefinement => "hero:illusion-refinement",
            Self::BarrierProficiency => "hero:barrier-proficiency",
            Self::Phantasm => "hero:phantasm",
            Self::WaterResistance => "hero:water-resistance",
            Self::TripleElementProficiency => "hero:triple-element-proficiency",
            Self::RadianceProficiency => "hero:radiance-proficiency",
            Self::FiveStreamsUniteProficiency => "hero:five-streams-unite-proficiency",
            Self::FireResistance => "hero:fire-resistance",
            Self::ArcaneEssence => "hero:arcane-essence",
            Self::Windwalking => "hero:windwalking",
            Self::MetamorphosisProficiency => "hero:metamorphosis-proficiency",
            Self::ShadowCut => "hero:shadow-cut",
            Self::ChaosProficiency => "hero:chaos-proficiency",
            Self::EarthResistance => "hero:earth-resistance",
            Self::ShadowEscape => "hero:shadow-escape",
            Self::Choice => "hero:choice",
            Self::Breakthrough => "hero:breakthrough",
            Self::ImmortalDrawBonus => "hero:immortal-draw-bonus",
            Self::Meditation => "hero:meditation",
            Self::SacredArt => "hero:sacred-art",
            Self::Revelation => "hero:revelation",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IncomingDamageModifier {
    None,
    HalfRoundUp,
    Prevent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PostFormationIntent {
    AddTurnDraw { player: PlayerId, amount: usize },
    AddStatus { status: StatusEffect },
    EstablishCounterEffect { owner: PlayerId, effect_id: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProfessionDef {
    pub(crate) id: ProfessionId,
    pub(crate) name: &'static str,
    pub(crate) rule_text: &'static str,
    pub(crate) prerequisite: Option<ProfessionId>,
    pub(crate) required_element: Element,
    pub(crate) minimum_level_sum: u32,
    pub(crate) parent: Option<ProfessionId>,
    pub(crate) abilities: Vec<ProfessionAbility>,
}

pub(crate) fn catalog() -> Vec<ProfessionDef> {
    vec![
        ProfessionDef {
            id: ProfessionId::new(WARRIOR_ID),
            name: "戰士",
            rule_text: "無前置職業；以任意張金行牌、等級總和３以上轉職",
            prerequisite: None,
            required_element: Element::Metal,
            minimum_level_sum: 3,
            parent: None,
            abilities: vec![
                ProfessionAbility::PhysicalDamageResistance,
                ProfessionAbility::WeaponProficiency,
                ProfessionAbility::DefenseProficiency,
                ProfessionAbility::CountershockProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(WAR_GOD_ID),
            name: "戰神",
            rule_text: "前置職業為戰士；以任意張金行牌、等級總和６以上轉職",
            prerequisite: Some(ProfessionId::new(WARRIOR_ID)),
            required_element: Element::Metal,
            minimum_level_sum: 6,
            parent: Some(ProfessionId::new(WARRIOR_ID)),
            abilities: vec![
                ProfessionAbility::WeaponMastery,
                ProfessionAbility::ShockBurstProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(HERO_ID),
            name: "勇者",
            rule_text: "前置職業為戰神；以任意張金行牌、等級總和９以上轉職",
            prerequisite: Some(ProfessionId::new(WAR_GOD_ID)),
            required_element: Element::Metal,
            minimum_level_sum: 9,
            parent: Some(ProfessionId::new(WAR_GOD_ID)),
            abilities: vec![
                ProfessionAbility::MetalResistance,
                ProfessionAbility::BattleSoul,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(SEEKER_ID),
            name: "尋道者",
            rule_text: "無前置職業；以任意張木行牌、等級總和３以上轉職",
            prerequisite: None,
            required_element: Element::Wood,
            minimum_level_sum: 3,
            parent: None,
            abilities: vec![
                ProfessionAbility::SeekerDiscount,
                ProfessionAbility::GeneratingFormationProficiency,
                ProfessionAbility::OvercomingFormationProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(EXPOUNDER_ID),
            name: "述道者",
            rule_text: "前置職業為尋道者；以任意張木行牌、等級總和６以上轉職",
            prerequisite: Some(ProfessionId::new(SEEKER_ID)),
            required_element: Element::Wood,
            minimum_level_sum: 6,
            parent: Some(ProfessionId::new(SEEKER_ID)),
            abilities: vec![
                ProfessionAbility::ReturnToOriginProficiency,
                ProfessionAbility::FiveElementsCycleProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(BENEVOLENT_ID),
            name: "仁者",
            rule_text: "前置職業為述道者；以任意張木行牌、等級總和９以上轉職",
            prerequisite: Some(ProfessionId::new(EXPOUNDER_ID)),
            required_element: Element::Wood,
            minimum_level_sum: 9,
            parent: Some(ProfessionId::new(EXPOUNDER_ID)),
            abilities: vec![
                ProfessionAbility::WoodResistance,
                ProfessionAbility::SpellProtection,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(MESMER_ID),
            name: "幻術師",
            rule_text: "無前置職業；以任意張水行牌、等級總和３以上轉職",
            prerequisite: None,
            required_element: Element::Water,
            minimum_level_sum: 3,
            parent: None,
            abilities: vec![
                ProfessionAbility::Illusion,
                ProfessionAbility::SealProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(SPIRIT_MESMER_ID),
            name: "幻靈使",
            rule_text: "前置職業為幻術師；以任意張水行牌、等級總和６以上轉職",
            prerequisite: Some(ProfessionId::new(MESMER_ID)),
            required_element: Element::Water,
            minimum_level_sum: 6,
            parent: Some(ProfessionId::new(MESMER_ID)),
            abilities: vec![
                ProfessionAbility::IllusionRefinement,
                ProfessionAbility::BarrierProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(HERMIT_ID),
            name: "隱者",
            rule_text: "前置職業為幻靈使；以任意張水行牌、等級總和９以上轉職",
            prerequisite: Some(ProfessionId::new(SPIRIT_MESMER_ID)),
            required_element: Element::Water,
            minimum_level_sum: 9,
            parent: Some(ProfessionId::new(SPIRIT_MESMER_ID)),
            abilities: vec![
                ProfessionAbility::WaterResistance,
                ProfessionAbility::Phantasm,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(MAGE_ID),
            name: "法師",
            rule_text: "無前置職業；以任意張火行牌、等級總和３以上轉職",
            prerequisite: None,
            required_element: Element::Fire,
            minimum_level_sum: 3,
            parent: None,
            abilities: vec![ProfessionAbility::TripleElementProficiency],
        },
        ProfessionDef {
            id: ProfessionId::new(MAGE_GUIDE_ID),
            name: "法導",
            rule_text: "前置職業為法師；以任意張火行牌、等級總和６以上轉職",
            prerequisite: Some(ProfessionId::new(MAGE_ID)),
            required_element: Element::Fire,
            minimum_level_sum: 6,
            parent: Some(ProfessionId::new(MAGE_ID)),
            abilities: vec![
                ProfessionAbility::RadianceProficiency,
                ProfessionAbility::FiveStreamsUniteProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(SAGE_ID),
            name: "智者",
            rule_text: "前置職業為法導；以任意張火行牌、等級總和９以上轉職",
            prerequisite: Some(ProfessionId::new(MAGE_GUIDE_ID)),
            required_element: Element::Fire,
            minimum_level_sum: 9,
            parent: Some(ProfessionId::new(MAGE_GUIDE_ID)),
            abilities: vec![
                ProfessionAbility::FireResistance,
                ProfessionAbility::ArcaneEssence,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(WINDWALKER_ID),
            name: "風行者",
            rule_text: "無前置職業；以任意張土行牌、等級總和３以上轉職",
            prerequisite: None,
            required_element: Element::Earth,
            minimum_level_sum: 3,
            parent: None,
            abilities: vec![
                ProfessionAbility::Windwalking,
                ProfessionAbility::MetamorphosisProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(SHADOW_WALKER_ID),
            name: "影行者",
            rule_text: "前置職業為風行者；以任意張土行牌、等級總和６以上轉職",
            prerequisite: Some(ProfessionId::new(WINDWALKER_ID)),
            required_element: Element::Earth,
            minimum_level_sum: 6,
            parent: Some(ProfessionId::new(WINDWALKER_ID)),
            abilities: vec![
                ProfessionAbility::ShadowCut,
                ProfessionAbility::ChaosProficiency,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(MARTIAL_ARTIST_ID),
            name: "武者",
            rule_text: "前置職業為影行者；以任意張土行牌、等級總和９以上轉職",
            prerequisite: Some(ProfessionId::new(SHADOW_WALKER_ID)),
            required_element: Element::Earth,
            minimum_level_sum: 9,
            parent: Some(ProfessionId::new(SHADOW_WALKER_ID)),
            abilities: vec![
                ProfessionAbility::EarthResistance,
                ProfessionAbility::ShadowEscape,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(FIRST_WANDERER_ID),
            name: "初行客",
            rule_text: "無前置職業；以一張１級牌轉職",
            prerequisite: None,
            required_element: Element::Metal,
            minimum_level_sum: 1,
            parent: None,
            abilities: vec![ProfessionAbility::Choice, ProfessionAbility::Breakthrough],
        },
        ProfessionDef {
            id: ProfessionId::new(IMMORTAL_ID),
            name: "仙者",
            rule_text: "前置職業為任一五大學派二階職業；以三張５級牌轉職",
            prerequisite: None,
            required_element: Element::Metal,
            minimum_level_sum: 15,
            parent: None,
            abilities: vec![
                ProfessionAbility::ImmortalDrawBonus,
                ProfessionAbility::Meditation,
            ],
        },
        ProfessionDef {
            id: ProfessionId::new(SAINT_ID),
            name: "聖者",
            rule_text: "前置職業為任一五大學派二階職業；以三張４級牌轉職",
            prerequisite: None,
            required_element: Element::Metal,
            minimum_level_sum: 12,
            parent: None,
            abilities: vec![ProfessionAbility::SacredArt, ProfessionAbility::Revelation],
        },
    ]
}

pub(crate) fn profession(id: &ProfessionId) -> Option<ProfessionDef> {
    catalog()
        .into_iter()
        .find(|profession| &profession.id == id)
}

pub(crate) fn profession_catalog_entries() -> Vec<crate::rules::profession::ProfessionCatalogEntry>
{
    catalog()
        .into_iter()
        .map(
            |profession| crate::rules::profession::ProfessionCatalogEntry {
                id: profession.id,
                module_id: HERO_SCHOOLS_MODULE_ID,
                name: profession.name,
                rule_text: profession.rule_text,
                parents: profession.parent.into_iter().collect(),
                ability_ids: profession
                    .abilities
                    .into_iter()
                    .map(ProfessionAbility::id)
                    .collect(),
            },
        )
        .collect()
}

fn effective_abilities(
    enabled_modules: &[crate::domain::RuleModuleId],
    id: &ProfessionId,
) -> Vec<ProfessionAbility> {
    let known = catalog()
        .into_iter()
        .flat_map(|profession| profession.abilities)
        .map(|ability| (ability.id(), ability))
        .collect::<std::collections::HashMap<_, _>>();
    crate::rules::profession::effective_ability_ids(enabled_modules, id)
        .into_iter()
        .filter_map(|id| known.get(id).copied())
        .collect()
}

pub(crate) fn effective_ability_summaries(
    enabled_modules: &[crate::domain::RuleModuleId],
    id: &ProfessionId,
) -> Vec<&'static str> {
    effective_abilities(enabled_modules, id)
        .into_iter()
        .map(|ability| match ability {
            ProfessionAbility::PhysicalDamageResistance => "卸勁：受到的物理傷害減半",
            ProfessionAbility::WeaponProficiency => "武器專精：金＋任意牌可組成武器",
            ProfessionAbility::DefenseProficiency => "防禦專精：木＋任意牌可組成防禦",
            ProfessionAbility::CountershockProficiency => "反震專精：火＋任意牌可組成反震",
            ProfessionAbility::WeaponMastery => "武器精研：施展武器的回合抽牌＋１",
            ProfessionAbility::ShockBurstProficiency => "震暴專精：火火水＋任意牌可組成震暴",
            ProfessionAbility::MetalResistance => "金行抗性：受到的金行傷害無效",
            ProfessionAbility::BattleSoul => "戰魄：施展震暴時，其他玩家無法行動及抽牌１回合",
            ProfessionAbility::SeekerDiscount => "尋道術：棄牌回收的生命代價減半",
            ProfessionAbility::GeneratingFormationProficiency => "生陣專精：兩張相生牌可組成生陣",
            ProfessionAbility::OvercomingFormationProficiency => "剋陣專精：兩張相剋牌可組成剋陣",
            ProfessionAbility::ReturnToOriginProficiency => "歸元專精：水水土＋任意牌可組成歸元",
            ProfessionAbility::FiveElementsCycleProficiency => {
                "五行輪迴專精：四張不同屬性牌可組成五行輪迴"
            }
            ProfessionAbility::WoodResistance => "木行抗性：受到的木行傷害無效",
            ProfessionAbility::SpellProtection => "道源：術式不受封印、幻印影響",
            ProfessionAbility::Illusion => "幻術：捨棄兩張牌，準備任意牌施展五行擊術",
            ProfessionAbility::SealProficiency => "封印專精：水＋任意牌可組成封印",
            ProfessionAbility::IllusionRefinement => "幻術精研：幻術產生３級以下牌時抽牌＋１",
            ProfessionAbility::BarrierProficiency => "氣壁專精：木木金＋任意牌可組成氣壁",
            ProfessionAbility::Phantasm => "幻朧：捨棄兩張牌，準備任意牌組成基礎陣法",
            ProfessionAbility::WaterResistance => "水行抗性：受到的水行傷害無效",
            ProfessionAbility::TripleElementProficiency => {
                "五行三張攻擊專精：兩張同行＋一張非同行牌，點數－６"
            }
            ProfessionAbility::RadianceProficiency => "光芒專精：金金火＋任意牌可組成光芒",
            ProfessionAbility::FiveStreamsUniteProficiency => {
                "五流歸一專精：四張同級牌可組成五流歸一，點數－１５"
            }
            ProfessionAbility::FireResistance => "火行抗性：受到的火行傷害無效",
            ProfessionAbility::ArcaneEssence => "法粹：專精組成的三張攻擊可召喚星辰",
            ProfessionAbility::Windwalking => "風行術：１５點以下攻擊不受其他陣法反制",
            ProfessionAbility::MetamorphosisProficiency => "幻化專精：土＋任意牌可組成幻化",
            ProfessionAbility::ShadowCut => "影切：捨棄一張牌，上家扣除該牌等級×２生命",
            ProfessionAbility::ChaosProficiency => "混沌專精：土土木＋任意牌可組成混沌",
            ProfessionAbility::EarthResistance => "土行抗性：受到的土行傷害無效",
            ProfessionAbility::ShadowEscape => "影遁：不受上家光芒、混沌及其幻化複製效果",
            ProfessionAbility::Choice => "抉擇：以１級牌轉職為任一五大學派一階職業",
            ProfessionAbility::Breakthrough => "突破：以１級牌加原需求直升任一五大學派二階職業",
            ProfessionAbility::ImmortalDrawBonus => "仙術：施展指定四張陣法時抽牌＋１",
            ProfessionAbility::Meditation => "冥思：捨棄一張４級以上牌，本回合抽牌＋１",
            ProfessionAbility::SacredArt => "聖術：基礎四張陣法可將一張４級以上牌視為兩張",
            ProfessionAbility::Revelation => "啟示：捨棄一張４級以上牌，抽三選一",
        })
        .collect()
}

pub(crate) fn profession_formation_summaries(
    enabled_modules: &[crate::domain::RuleModuleId],
    id: &ProfessionId,
) -> Vec<(String, String)> {
    formation_specs()
        .into_iter()
        .filter(|formation| {
            formation.formation.id != "void-reversion"
                && can_use_profession_formation(enabled_modules, Some(id), &formation.formation.id)
                && is_profession_formation(&formation.formation.id)
        })
        .map(|formation| (formation.formation.name, formation.formation.rule_text))
        .collect()
}

pub(crate) fn playable_profession_changes(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionChangeCandidate>> {
    if !state.has_rule_module(HERO_SCHOOLS_MODULE_ID) {
        return Ok(Vec::new());
    }

    let facts = submitted_card_facts(state, player, cards)?;
    Ok(catalog()
        .into_iter()
        .filter(|profession| profession_change_matches(state, player, profession, &facts))
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
    if !state.has_rule_module(HERO_SCHOOLS_MODULE_ID) {
        return Err(GameError::Validation(ValidationError::HeroSchoolsDisabled));
    }
    let profession = profession(target)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownProfession(target.clone())))?;
    let facts = submitted_card_facts(state, player, cards)?;
    if !profession_change_matches(state, player, &profession, &facts) {
        if let Some(required) = &profession.prerequisite
            && state.profession_for(player) != Some(required)
            && state.profession_for(player) != Some(&ProfessionId::new(FIRST_WANDERER_ID))
        {
            return Err(GameError::Validation(
                ValidationError::ProfessionPrerequisiteNotMet {
                    profession: target.clone(),
                    required: required.clone(),
                    actual: state.profession_for(player).cloned(),
                },
            ));
        }
        return Err(GameError::Validation(
            ValidationError::ProfessionChangePatternMismatch {
                profession: target.clone(),
            },
        ));
    }
    Ok(())
}

fn requirement_matches(profession: &ProfessionDef, facts: &[SubmittedCardFacts]) -> bool {
    !facts.is_empty()
        && facts
            .iter()
            .all(|card| card.element == profession.required_element)
        && facts.iter().map(|card| card.level).sum::<u32>() >= profession.minimum_level_sum
}

pub(crate) fn matches_initial_profession(
    profession_id: &str,
    facts: &[SubmittedCardFacts],
) -> bool {
    catalog().into_iter().any(|profession| {
        profession.id.as_str() == profession_id
            && profession.prerequisite.is_none()
            && requirement_matches(&profession, facts)
    })
}

fn profession_change_matches(
    state: &GameState,
    player: &PlayerId,
    profession: &ProfessionDef,
    facts: &[SubmittedCardFacts],
) -> bool {
    let current = state.profession_for(player);
    match profession.id.as_str() {
        FIRST_WANDERER_ID => facts.len() == 1 && facts[0].level == 1,
        IMMORTAL_ID => {
            current.is_some_and(is_second_tier)
                && facts.len() == 3
                && facts.iter().all(|card| card.level == 5)
        }
        SAINT_ID => {
            current.is_some_and(is_second_tier)
                && facts.len() == 3
                && facts.iter().all(|card| card.level == 4)
        }
        _ => {
            let normal_prerequisite = profession
                .prerequisite
                .as_ref()
                .is_none_or(|required| current == Some(required));
            if normal_prerequisite && requirement_matches(profession, facts) {
                return true;
            }
            if current != Some(&ProfessionId::new(FIRST_WANDERER_ID)) {
                return false;
            }
            if is_first_tier(&profession.id) {
                return facts.len() == 1 && facts[0].level == 1;
            }
            if is_second_tier(&profession.id) {
                return facts.iter().enumerate().any(|(choice_index, choice)| {
                    choice.level == 1
                        && facts
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| *index != choice_index)
                            .all(|(_, card)| card.element == profession.required_element)
                        && facts
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| *index != choice_index)
                            .map(|(_, card)| card.level)
                            .sum::<u32>()
                            >= profession.minimum_level_sum
                });
            }
            false
        }
    }
}

fn is_first_tier(profession: &ProfessionId) -> bool {
    matches!(
        profession.as_str(),
        WARRIOR_ID | SEEKER_ID | MESMER_ID | MAGE_ID | WINDWALKER_ID
    )
}

fn is_second_tier(profession: &ProfessionId) -> bool {
    matches!(
        profession.as_str(),
        WAR_GOD_ID | EXPOUNDER_ID | SPIRIT_MESMER_ID | MAGE_GUIDE_ID | SHADOW_WALKER_ID
    )
}

pub(crate) fn is_legendary(profession: &ProfessionId) -> bool {
    matches!(
        profession.as_str(),
        HERO_ID | BENEVOLENT_ID | HERMIT_ID | SAGE_ID | MARTIAL_ARTIST_ID | IMMORTAL_ID | SAINT_ID
    )
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
            state.card_def(*card).ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))?;
            let facts = state
                .effective_card_facts(player, *card)
                .expect("known Card has facts");
            Ok(SubmittedCardFacts {
                element: facts.element,
                level: facts.level,
            })
        })
        .collect()
}

pub(crate) fn matches_proficiency(
    enabled_modules: &[crate::domain::RuleModuleId],
    profession: Option<&ProfessionId>,
    formation_id: &str,
    cards: &[SubmittedCardFacts],
) -> bool {
    let Some(profession) = profession else {
        return false;
    };
    let abilities = effective_abilities(enabled_modules, profession);
    match formation_id {
        "weapon" if abilities.contains(&ProfessionAbility::WeaponProficiency) => {
            cards.len() == 2 && cards.iter().any(|card| card.element == Element::Metal)
        }
        "defense" if abilities.contains(&ProfessionAbility::DefenseProficiency) => {
            cards.len() == 2 && cards.iter().any(|card| card.element == Element::Wood)
        }
        "countershock" if abilities.contains(&ProfessionAbility::CountershockProficiency) => {
            cards.len() == 2 && cards.iter().any(|card| card.element == Element::Fire)
        }
        "shock-burst" if abilities.contains(&ProfessionAbility::ShockBurstProficiency) => {
            cards.len() == 4
                && cards
                    .iter()
                    .filter(|card| card.element == Element::Fire)
                    .count()
                    >= 2
                && cards.iter().any(|card| card.element == Element::Water)
        }
        "generating-formation"
            if abilities.contains(&ProfessionAbility::GeneratingFormationProficiency) =>
        {
            cards.len() == 2 && generates(cards[0].element, cards[1].element)
        }
        "overcoming-formation"
            if abilities.contains(&ProfessionAbility::OvercomingFormationProficiency) =>
        {
            cards.len() == 2 && overcomes(cards[0].element, cards[1].element)
        }
        "return-to-origin" if abilities.contains(&ProfessionAbility::ReturnToOriginProficiency) => {
            cards.len() == 4
                && count_element(cards, Element::Water) >= 2
                && count_element(cards, Element::Earth) >= 1
        }
        "five-elements-cycle"
            if abilities.contains(&ProfessionAbility::FiveElementsCycleProficiency) =>
        {
            cards.len() == 4
                && cards
                    .iter()
                    .map(|card| card.element)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == 4
        }
        "seal" if abilities.contains(&ProfessionAbility::SealProficiency) => {
            cards.len() == 2 && count_element(cards, Element::Water) >= 1
        }
        "barrier" if abilities.contains(&ProfessionAbility::BarrierProficiency) => {
            cards.len() == 4
                && count_element(cards, Element::Wood) >= 2
                && count_element(cards, Element::Metal) >= 1
        }
        formation_id
            if abilities.contains(&ProfessionAbility::TripleElementProficiency)
                && triple_formation_element(formation_id).is_some() =>
        {
            let element = triple_formation_element(formation_id).unwrap();
            cards.len() == 3 && count_element(cards, element) == 2
        }
        "radiance" if abilities.contains(&ProfessionAbility::RadianceProficiency) => {
            cards.len() == 4
                && count_element(cards, Element::Metal) >= 2
                && count_element(cards, Element::Fire) >= 1
        }
        "five-streams-unite"
            if abilities.contains(&ProfessionAbility::FiveStreamsUniteProficiency) =>
        {
            cards.len() == 4
                && cards
                    .first()
                    .is_some_and(|first| cards.iter().all(|card| card.level == first.level))
        }
        "metamorphosis" if abilities.contains(&ProfessionAbility::MetamorphosisProficiency) => {
            cards.len() == 2 && count_element(cards, Element::Earth) >= 1
        }
        "chaos" if abilities.contains(&ProfessionAbility::ChaosProficiency) => {
            cards.len() == 4
                && count_element(cards, Element::Earth) >= 2
                && count_element(cards, Element::Wood) >= 1
        }
        _ => false,
    }
}

fn count_element(cards: &[SubmittedCardFacts], element: Element) -> usize {
    cards.iter().filter(|card| card.element == element).count()
}

fn triple_formation_element(formation_id: &str) -> Option<Element> {
    match formation_id {
        "triple-metal" => Some(Element::Metal),
        "triple-wood" => Some(Element::Wood),
        "triple-water" => Some(Element::Water),
        "triple-fire" => Some(Element::Fire),
        "triple-earth" => Some(Element::Earth),
        _ => None,
    }
}

fn generates(first: Element, second: Element) -> bool {
    matches!(
        (first, second),
        (Element::Metal, Element::Water)
            | (Element::Water, Element::Metal)
            | (Element::Water, Element::Wood)
            | (Element::Wood, Element::Water)
            | (Element::Wood, Element::Fire)
            | (Element::Fire, Element::Wood)
            | (Element::Fire, Element::Earth)
            | (Element::Earth, Element::Fire)
            | (Element::Earth, Element::Metal)
            | (Element::Metal, Element::Earth)
    )
}

fn overcomes(first: Element, second: Element) -> bool {
    matches!(
        (first, second),
        (Element::Metal, Element::Wood)
            | (Element::Wood, Element::Metal)
            | (Element::Wood, Element::Earth)
            | (Element::Earth, Element::Wood)
            | (Element::Earth, Element::Water)
            | (Element::Water, Element::Earth)
            | (Element::Water, Element::Fire)
            | (Element::Fire, Element::Water)
            | (Element::Fire, Element::Metal)
            | (Element::Metal, Element::Fire)
    )
}

pub(crate) fn can_use_profession_formation(
    enabled_modules: &[crate::domain::RuleModuleId],
    profession: Option<&ProfessionId>,
    formation_id: &str,
) -> bool {
    if formation_id == "void-reversion" {
        return true;
    }
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
        "divine-weapon" => inherits(WAR_GOD_ID),
        "falling-light-slash" => profession.as_str() == HERO_ID,
        "dao-defense" => inherits(EXPOUNDER_ID),
        "reincarnation" => profession.as_str() == BENEVOLENT_ID,
        "magic-seal" => inherits(SPIRIT_MESMER_ID),
        "purple-light-shield" => profession.as_str() == HERMIT_ID,
        "magic-shock" => inherits(MAGE_GUIDE_ID),
        "magic-reflection-flash" => profession.as_str() == SAGE_ID,
        "shadow-assault" => inherits(SHADOW_WALKER_ID),
        "instant-shadow-death" => profession.as_str() == MARTIAL_ARTIST_ID,
        "condensed-void-arrow" | "sky-bow-roar" => profession.as_str() == IMMORTAL_ID,
        "holy-light-break" | "holy-wind" => profession.as_str() == SAINT_ID,
        _ => true,
    }
}

pub(crate) fn is_profession_formation(formation_id: &str) -> bool {
    matches!(
        formation_id,
        "divine-weapon"
            | "falling-light-slash"
            | "dao-defense"
            | "reincarnation"
            | "magic-seal"
            | "purple-light-shield"
            | "magic-shock"
            | "magic-reflection-flash"
            | "shadow-assault"
            | "instant-shadow-death"
            | "condensed-void-arrow"
            | "sky-bow-roar"
            | "holy-light-break"
            | "holy-wind"
            | "void-reversion"
    )
}

pub(crate) fn incoming_damage_modifier(
    state: &GameState,
    target: &PlayerId,
    category: &AttackCategory,
    is_damage: bool,
) -> IncomingDamageModifier {
    if !state.has_rule_module(HERO_SCHOOLS_MODULE_ID)
        || !is_damage
        || crate::rules::pouch::profession_is_suppressed(state, target)
    {
        return IncomingDamageModifier::None;
    }
    let Some(profession) = state.profession_for(target) else {
        return IncomingDamageModifier::None;
    };
    let abilities = effective_abilities(&state.enabled_rule_modules, profession);
    let resisted_element = match category {
        AttackCategory::Elemental(element) => Some(*element),
        AttackCategory::Physical | AttackCategory::Special => None,
    };
    if matches!(
        resisted_element,
        Some(Element::Metal) if abilities.contains(&ProfessionAbility::MetalResistance)
    ) || matches!(
        resisted_element,
        Some(Element::Wood) if abilities.contains(&ProfessionAbility::WoodResistance)
    ) || matches!(
        resisted_element,
        Some(Element::Water) if abilities.contains(&ProfessionAbility::WaterResistance)
    ) || matches!(
        resisted_element,
        Some(Element::Fire) if abilities.contains(&ProfessionAbility::FireResistance)
    ) || matches!(
        resisted_element,
        Some(Element::Earth) if abilities.contains(&ProfessionAbility::EarthResistance)
    ) {
        IncomingDamageModifier::Prevent
    } else if matches!(category, AttackCategory::Physical)
        && abilities.contains(&ProfessionAbility::PhysicalDamageResistance)
    {
        IncomingDamageModifier::HalfRoundUp
    } else {
        IncomingDamageModifier::None
    }
}

pub(crate) fn modify_attack_points(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
    cards: &[CardInstanceId],
    points: i32,
) -> i32 {
    if crate::rules::pouch::profession_is_suppressed(state, player) {
        return points;
    }
    let Some(profession) = state.profession_for(player) else {
        return points;
    };
    let abilities = effective_abilities(&state.enabled_rule_modules, profession);
    if triple_formation_element(formation_id).is_some()
        && abilities.contains(&ProfessionAbility::TripleElementProficiency)
        && cards.len() == 3
        && cards
            .iter()
            .filter_map(|card| state.card_def(*card))
            .count()
            == 3
        && triple_formation_element(formation_id).is_some_and(|element| {
            cards
                .iter()
                .filter_map(|card| state.card_def(*card))
                .filter(|card| card.element == element)
                .count()
                == 2
        })
    {
        (points - 6).max(0)
    } else if formation_id == "five-streams-unite"
        && abilities.contains(&ProfessionAbility::FiveStreamsUniteProficiency)
        && cards.len() == 4
    {
        (points - 15).max(0)
    } else {
        points
    }
}

pub(crate) fn star_summoning_allowed(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
    cards: &[CardInstanceId],
) -> bool {
    let Some(element) = triple_formation_element(formation_id) else {
        return true;
    };
    let used_mage_proficiency = cards.len() == 3
        && cards
            .iter()
            .filter_map(|card| state.card_def(*card))
            .filter(|card| card.element == element)
            .count()
            == 2;
    if !used_mage_proficiency {
        return true;
    }
    if crate::rules::pouch::profession_is_suppressed(state, player) {
        return false;
    }
    state.profession_for(player).is_some_and(|profession| {
        effective_abilities(&state.enabled_rule_modules, profession)
            .contains(&ProfessionAbility::ArcaneEssence)
    })
}

pub(crate) fn windwalking_applies(
    state: &GameState,
    player: &PlayerId,
    attack_points: i32,
) -> bool {
    !crate::rules::pouch::profession_is_suppressed(state, player)
        && attack_points <= 15
        && state.profession_for(player).is_some_and(|profession| {
            effective_abilities(&state.enabled_rule_modules, profession)
                .contains(&ProfessionAbility::Windwalking)
        })
}

pub(crate) fn spell_counter_immunity(state: &GameState, player: &PlayerId) -> bool {
    !crate::rules::pouch::profession_is_suppressed(state, player)
        && state.profession_for(player).is_some_and(|profession| {
            effective_abilities(&state.enabled_rule_modules, profession)
                .contains(&ProfessionAbility::SpellProtection)
        })
}

pub(crate) fn profession_has_ability(
    enabled_modules: &[crate::domain::RuleModuleId],
    profession: Option<&ProfessionId>,
    ability: ProfessionAbility,
) -> bool {
    profession.is_some_and(|profession| {
        effective_abilities(enabled_modules, profession).contains(&ability)
    })
}

pub(crate) fn discard_retrieval_cost(
    state: &GameState,
    player: &PlayerId,
    normal_cost: i32,
) -> i32 {
    if !crate::rules::pouch::profession_is_suppressed(state, player)
        && profession_has_ability(
            &state.enabled_rule_modules,
            state.profession_for(player),
            ProfessionAbility::SeekerDiscount,
        )
    {
        (normal_cost + 1) / 2
    } else {
        normal_cost
    }
}

pub(crate) fn target_ignores_disruptive_spell(
    state: &GameState,
    caster: &PlayerId,
    resolver_id: &str,
) -> GameResult<bool> {
    if !matches!(
        resolver_id,
        "radiance" | "chaos" | crate::rules::dark::DARK_RADIANCE | crate::rules::dark::DARK_CHAOS
    ) {
        return Ok(false);
    }
    let target =
        TurnOrderTargets::new(state).player_target(caster, RulePlayerTarget::NextPlayer)?;
    if crate::rules::pouch::profession_is_suppressed(state, &target) {
        return Ok(false);
    }
    Ok(profession_has_ability(
        &state.enabled_rule_modules,
        state.profession_for(&target),
        ProfessionAbility::ShadowEscape,
    ))
}

pub(crate) fn formation_role_options(
    formation_id: &str,
    cards: &[CardInstanceId],
    facts: &[SubmittedCardFacts],
) -> Vec<(crate::domain::TargetDecl, String)> {
    if formation_id != "reincarnation" {
        return Vec::new();
    }

    cards
        .iter()
        .enumerate()
        .filter_map(|(standalone_index, standalone)| {
            let standalone_fact = facts.get(standalone_index)?;
            if standalone_fact.element != Element::Wood {
                return None;
            }
            let sequence = facts
                .iter()
                .enumerate()
                .filter(|(index, _)| *index != standalone_index)
                .map(|(_, card)| card)
                .collect::<Vec<_>>();
            (sequence.len() == 3
                && sequence.iter().map(|card| card.level).sum::<u32>() >= 10
                && is_three_generating(
                    &sequence.iter().map(|card| card.element).collect::<Vec<_>>(),
                ))
            .then(|| {
                (
                    crate::domain::TargetDecl::FormationRole {
                        role: "standalone-wood".to_string(),
                        card: *standalone,
                    },
                    format!("回復 {} 點生命", standalone_fact.level.value() * 25),
                )
            })
        })
        .collect()
}

pub(crate) fn playable_profession_abilities(
    state: &GameState,
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> GameResult<Vec<ProfessionAbilityCandidate>> {
    if !state.has_rule_module(HERO_SCHOOLS_MODULE_ID)
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
    let abilities = effective_abilities(&state.enabled_rule_modules, profession);
    let mut candidates = Vec::new();
    if cards.len() == 1 {
        let effective_level =
            state
                .card_level_for(player, cards[0])
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(cards[0]),
                ))?;
        if abilities.contains(&ProfessionAbility::ShadowCut) {
            candidates.push(ProfessionAbilityCandidate {
                ability_id: "shadow-cut".to_string(),
                ability_name: "影切".to_string(),
                cards: cards.to_vec(),
                target_card: None,
                declared_element: None,
                declared_level: None,
                input_requirement: None,
                detail: crate::rules::PlayerFacingActionDetail::pending_composition(),
            });
        }
        if effective_level >= 4 && abilities.contains(&ProfessionAbility::Meditation) {
            candidates.push(ProfessionAbilityCandidate {
                ability_id: "meditation".to_string(),
                ability_name: "冥思".to_string(),
                cards: cards.to_vec(),
                target_card: None,
                declared_element: None,
                declared_level: None,
                input_requirement: None,
                detail: crate::rules::PlayerFacingActionDetail::pending_composition(),
            });
        }
        if effective_level >= 4
            && abilities.contains(&ProfessionAbility::Revelation)
            && state.deck_for(player).is_some_and(|deck| deck.len() >= 3)
        {
            candidates.push(ProfessionAbilityCandidate {
                ability_id: "revelation".to_string(),
                ability_name: "啟示".to_string(),
                cards: cards.to_vec(),
                target_card: None,
                declared_element: None,
                declared_level: None,
                input_requirement: None,
                detail: crate::rules::PlayerFacingActionDetail::pending_composition(),
            });
        }
    }
    if cards.len() == 2 {
        for (ability_id, ability_name, ability, _scope) in [
            ("illusion", "幻術", ProfessionAbility::Illusion, "五行擊術"),
            (
                "phantasm",
                "幻朧",
                ProfessionAbility::Phantasm,
                "基礎規則陣法",
            ),
        ] {
            if !abilities.contains(&ability) {
                continue;
            }
            candidates.push(ProfessionAbilityCandidate {
                ability_id: ability_id.to_string(),
                ability_name: ability_name.to_string(),
                cards: cards.to_vec(),
                target_card: None,
                declared_element: None,
                declared_level: None,
                input_requirement: Some(ActionInputRequirement::VirtualFormationCard {
                    elements: vec![
                        Element::Metal,
                        Element::Wood,
                        Element::Water,
                        Element::Fire,
                        Element::Earth,
                    ],
                    levels: (1..=5).collect(),
                }),
                detail: crate::rules::PlayerFacingActionDetail::pending_composition(),
            });
        }
    }
    Ok(candidates)
}

/// Mirrors the activated-ability resolver below.  The browser renders this
/// typed fact, never an ability-id prose lookup.
pub(crate) fn player_facing_ability_effect(id: &str) -> Option<ProfessionAbilityEffect> {
    Some(match id {
        "shadow-cut" => {
            ProfessionAbilityEffect::DamagePreviousTeamByCardLevelTimes { multiplier: 2 }
        }
        "meditation" => ProfessionAbilityEffect::IncreaseTurnDraw { amount: 1 },
        "revelation" => ProfessionAbilityEffect::DrawThreeThenChooseOne,
        "illusion" => ProfessionAbilityEffect::CreateVirtualFormationCard {
            scope: VirtualFormationScope::ElementalStrike,
        },
        "phantasm" => ProfessionAbilityEffect::CreateVirtualFormationCard {
            scope: VirtualFormationScope::BaseFormation,
        },
        _ => return None,
    })
}

pub(crate) fn player_facing_ability_discards_selected_cards(id: &str) -> bool {
    matches!(
        id,
        "shadow-cut" | "meditation" | "revelation" | "illusion" | "phantasm"
    )
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
    if !state.has_rule_module(HERO_SCHOOLS_MODULE_ID) {
        return Err(GameError::Validation(ValidationError::HeroSchoolsDisabled));
    }
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
    let profession = state.profession_for(player).ok_or_else(|| {
        GameError::Validation(ValidationError::ProfessionAbilityUnavailable(
            ability_id.to_string(),
        ))
    })?;
    let abilities = effective_abilities(&state.enabled_rule_modules, profession);
    let required_ability = match ability_id {
        "illusion" => ProfessionAbility::Illusion,
        "phantasm" => ProfessionAbility::Phantasm,
        "shadow-cut" => ProfessionAbility::ShadowCut,
        "meditation" => ProfessionAbility::Meditation,
        "revelation" => ProfessionAbility::Revelation,
        _ => {
            return Err(GameError::Validation(
                ValidationError::UnknownProfessionAbility(ability_id.to_string()),
            ));
        }
    };
    if !abilities.contains(&required_ability) {
        return Err(GameError::Validation(
            ValidationError::ProfessionAbilityUnavailable(ability_id.to_string()),
        ));
    }
    validate_ability_cards(state, player, cards)?;
    let mut events = Vec::new();
    match ability_id {
        "illusion" | "phantasm" => {
            if cards.len() != 2 {
                return cannot_resolve(ability_id);
            }
            if target_card.is_some() {
                return cannot_resolve(ability_id);
            }
            let element = declared_element.ok_or_else(|| {
                GameError::Validation(ValidationError::ProfessionAbilityCannotResolve(
                    ability_id.to_string(),
                ))
            })?;
            let level = declared_level
                .filter(|level| (1..=5).contains(level))
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::ProfessionAbilityCannotResolve(
                        ability_id.to_string(),
                    ))
                })?;
            let allowed_formation_scope = if ability_id == "illusion" {
                vec![format!("{}-strike", element_id(element))]
            } else {
                vec!["base".to_string()]
            };
            events.push(GameEvent::ProfessionAbilityActivated {
                player: player.clone(),
                ability_id: ability_id.to_string(),
                prepared: None,
            });
            events.push(GameEvent::FormationRequirementSet {
                requirement: crate::domain::FormationRequirement {
                    player: player.clone(),
                    physical_card: None,
                    virtual_card: Some(crate::domain::VirtualFormationCard {
                        source_ability_id: ability_id.to_string(),
                        element,
                        level: crate::domain::EffectiveCardLevel::try_from(level)
                            .expect("Virtual Formation Card levels are validated at command input"),
                    }),
                    allowed_formation_scope,
                    applied_on_turn: state.turn_number,
                },
            });
            events.push(GameEvent::CardsMoved {
                card_moves: ability_card_moves(state, player, cards),
            });
            if ability_id == "illusion"
                && level <= 3
                && abilities.contains(&ProfessionAbility::IllusionRefinement)
            {
                events.push(turn_draw_bonus_event(state, player, 1));
            }
        }
        "shadow-cut" => {
            if cards.len() != 1 {
                return cannot_resolve(ability_id);
            }
            let target = TurnOrderTargets::new(state)
                .player_target(player, RulePlayerTarget::PreviousPlayer)?;
            let team = TurnOrderTargets::new(state).team_of(&target)?;
            let level = state
                .card_level_for(player, cards[0])
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(cards[0]),
                ))?
                .value() as i32;
            events.push(GameEvent::ProfessionAbilityActivated {
                player: player.clone(),
                ability_id: ability_id.to_string(),
                prepared: None,
            });
            events.push(GameEvent::CardsMoved {
                card_moves: ability_card_moves(state, player, cards),
            });
            events.push(GameEvent::HpChanged {
                change: hp_change(state, team, -level * 2)?,
            });
        }
        "meditation" => {
            if cards.len() != 1
                || state
                    .card_level_for(player, cards[0])
                    .is_none_or(|level| level < 4)
            {
                return cannot_resolve(ability_id);
            }
            events.push(GameEvent::ProfessionAbilityActivated {
                player: player.clone(),
                ability_id: ability_id.to_string(),
                prepared: None,
            });
            events.push(GameEvent::CardsMoved {
                card_moves: ability_card_moves(state, player, cards),
            });
            events.push(turn_draw_bonus_event(state, player, 1));
        }
        "revelation" => {
            if cards.len() != 1
                || state
                    .card_level_for(player, cards[0])
                    .is_none_or(|level| level < 4)
            {
                return cannot_resolve(ability_id);
            }
            let deck = state.deck_for(player).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
            })?;
            let discard = state.discard_for(player).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
            })?;
            if deck.len() + discard.len() < 3 {
                return cannot_resolve(ability_id);
            }
            events.push(GameEvent::ProfessionAbilityActivated {
                player: player.clone(),
                ability_id: ability_id.to_string(),
                prepared: None,
            });
            events.push(GameEvent::CardsMoved {
                card_moves: ability_card_moves(state, player, cards),
            });
            if deck.len() < 3 {
                let mut projected = state.clone();
                for event in &events {
                    crate::rules::projection::apply_event(&mut projected, event);
                }
                let pile = deck_kind(state, player);
                events.push(GameEvent::RandomnessRequested {
                    request: crate::domain::PendingRandomness {
                        request_id: format!(
                            "hero:revelation:{}:{}",
                            state.turn_number,
                            player.as_str()
                        ),
                        operation: RandomnessOperation::DiscardShuffle {
                            pile,
                            placement: crate::domain::DeckPlacement::Bottom,
                        },
                        continuation: RandomnessContinuation::Hero(
                            HeroRandomnessContinuation::Revelation,
                        ),
                        current_order: projected
                            .discard_for(player)
                            .expect("known player has a discard pile")
                            .to_vec(),
                    },
                });
            } else {
                events.extend(revelation_choice_events(state, player)?);
            }
        }
        _ => unreachable!(),
    }
    Ok(events)
}

pub(crate) fn after_revelation_randomness_events(state: &GameState) -> GameResult<Vec<GameEvent>> {
    let player = state
        .current_player()
        .cloned()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;
    revelation_choice_events(state, &player)
}

fn revelation_choice_events(state: &GameState, player: &PlayerId) -> GameResult<Vec<GameEvent>> {
    let drawn_cards = state
        .deck_for(player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
        .iter()
        .take(3)
        .copied()
        .collect::<Vec<_>>();
    Ok(vec![
        GameEvent::CardsDrawnForProfessionChoice {
            player: player.clone(),
            ability_id: "revelation".to_string(),
            cards: drawn_cards.clone(),
        },
        crate::rules::pending_choice::request_event(
            state,
            crate::domain::ChoiceRequest {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::Card {
                    cards: drawn_cards,
                    minimum: 1,
                    maximum: 1,
                    can_decline: false,
                },
                continuation: crate::domain::ChoiceContinuation::Hero(
                    crate::domain::HeroChoiceContinuation::RevelationKeepOne,
                ),
            },
        )?,
    ])
}

fn deck_kind(state: &GameState, player: &PlayerId) -> RandomnessDeck {
    if state.uses_personal_decks() {
        RandomnessDeck::Player(player.clone())
    } else {
        RandomnessDeck::Shared
    }
}

fn cannot_resolve<T>(ability_id: &str) -> GameResult<T> {
    Err(GameError::Validation(
        ValidationError::ProfessionAbilityCannotResolve(ability_id.to_string()),
    ))
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
) -> Vec<CardMoveDelta> {
    cards
        .iter()
        .copied()
        .map(|card| CardMoveDelta {
            card,
            from: CardZone::Hand(player.clone()),
            to: if state.uses_personal_decks() {
                match state.card_origin(card) {
                    Some(CardOrigin::Player(owner)) => CardZone::PlayerDiscard(owner.clone()),
                    _ => CardZone::Discard,
                }
            } else {
                CardZone::Discard
            },
        })
        .collect()
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

fn hp_change(
    state: &GameState,
    team: crate::domain::TeamId,
    delta: i32,
) -> GameResult<HpChangeDelta> {
    let old_hp = state
        .hp
        .iter()
        .find(|hp| hp.team == team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?
        .hp;
    let initial_hp = state
        .initial_hp(&team)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;
    let new_hp = (old_hp + delta).clamp(0, initial_hp);
    Ok(HpChangeDelta {
        team,
        old_hp,
        delta,
        new_hp,
        effective_delta: new_hp - old_hp,
    })
}

fn element_id(element: Element) -> &'static str {
    match element {
        Element::Metal => "metal",
        Element::Wood => "wood",
        Element::Water => "water",
        Element::Fire => "fire",
        Element::Earth => "earth",
    }
}

fn is_three_generating(elements: &[Element]) -> bool {
    let cycle = [
        Element::Wood,
        Element::Fire,
        Element::Earth,
        Element::Metal,
        Element::Water,
    ];
    let counts = |values: &[Element]| {
        values.iter().fold(
            std::collections::HashMap::<Element, usize>::new(),
            |mut counts, element| {
                *counts.entry(*element).or_default() += 1;
                counts
            },
        )
    };
    (0..cycle.len()).any(|start| {
        counts(elements)
            == counts(
                &(0..3)
                    .map(|offset| cycle[(start + offset) % cycle.len()])
                    .collect::<Vec<_>>(),
            )
    })
}

pub(crate) fn post_formation_intents(
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) -> GameResult<Vec<PostFormationIntent>> {
    if !state.has_rule_module(HERO_SCHOOLS_MODULE_ID) {
        return Ok(Vec::new());
    }
    let Some(profession) = state.profession_for(player) else {
        return Ok(Vec::new());
    };
    let abilities = if crate::rules::pouch::profession_is_suppressed(state, player) {
        Vec::new()
    } else {
        effective_abilities(&state.enabled_rule_modules, profession)
    };
    let mut intents = Vec::new();
    if formation_id == "weapon" && abilities.contains(&ProfessionAbility::WeaponMastery) {
        intents.push(PostFormationIntent::AddTurnDraw {
            player: player.clone(),
            amount: 1,
        });
    }
    if formation_id == "shock-burst" && abilities.contains(&ProfessionAbility::BattleSoul) {
        for target in state
            .players
            .iter()
            .map(|entry| &entry.id)
            .filter(|id| *id != player)
        {
            let expires_on = nth_future_turn_for_player(state, target)?;
            for kind in ["CannotAct", "CannotDraw"] {
                intents.push(PostFormationIntent::AddStatus {
                    status: StatusEffect {
                        id: format!(
                            "battle-soul-{}-{}-turn-{}",
                            kind.to_ascii_lowercase(),
                            target.as_str(),
                            state.turn_number
                        ),
                        owner: StatusOwner::Player(target.clone()),
                        kind: kind.to_string(),
                        value: None,
                        duration: StatusDuration::UntilTurnEndNumber {
                            player: target.clone(),
                            turn_number: expires_on,
                        },
                    },
                });
            }
        }
    }
    if matches!(
        formation_id,
        "dao-defense" | "magic-seal" | "magic-shock" | "holy-light-break"
    ) {
        intents.push(PostFormationIntent::EstablishCounterEffect {
            owner: player.clone(),
            effect_id: formation_id.to_string(),
        });
    }
    if abilities.contains(&ProfessionAbility::ImmortalDrawBonus)
        && matches!(
            formation_id,
            "radiance" | "barrier" | "return-to-origin" | "shock-burst" | "chaos"
        )
    {
        intents.push(PostFormationIntent::AddTurnDraw {
            player: player.clone(),
            amount: 1,
        });
    }
    if formation_id == "magic-reflection-flash" {
        intents.push(PostFormationIntent::AddStatus {
            status: StatusEffect {
                id: format!(
                    "magic-reflection-cannot-draw-{}-turn-{}",
                    player.as_str(),
                    state.turn_number
                ),
                owner: StatusOwner::Player(player.clone()),
                kind: "CannotDraw".to_string(),
                value: None,
                duration: StatusDuration::UntilTurnEndNumber {
                    player: player.clone(),
                    turn_number: state.turn_number,
                },
            },
        });
    }
    if formation_id == "condensed-void-arrow" {
        intents.push(PostFormationIntent::AddTurnDraw {
            player: player.clone(),
            amount: 2,
        });
    }
    Ok(intents)
}

fn nth_future_turn_for_player(state: &GameState, player: &PlayerId) -> GameResult<u64> {
    use crate::domain::targeting::TurnOrderTargets;
    TurnOrderTargets::new(state).nth_future_turn_for_player(player, 1)
}

pub(crate) fn formation_specs() -> Vec<BaseFormationSpec> {
    vec![
        profession_attack(
            "divine-weapon",
            "神兵",
            "金＋同級牌／物理攻擊，點數＝等級總和×３",
            FormationPattern::Custom("metal-and-same-level".to_string()),
            3,
        ),
        profession_attack(
            "falling-light-slash",
            "落光斬",
            "金金火火／物理攻擊，點數＝等級總和×６",
            FormationPattern::ExactElements(vec![
                Element::Metal,
                Element::Metal,
                Element::Fire,
                Element::Fire,
            ]),
            6,
        ),
        profession_attack(
            "dao-defense",
            "道禦",
            "木＋同級牌／特殊攻擊，點數＝等級總和×３，反制攻擊傷害",
            FormationPattern::Custom("wood-and-same-level".to_string()),
            3,
        ),
        profession_spell(
            "reincarnation",
            "轉生術",
            "木＋三張連續相生且等級總和１０以上／回復生命，點數＝木×２５",
            FormationPattern::Custom("reincarnation".to_string()),
        ),
        profession_attack(
            "magic-seal",
            "幻印",
            "水＋同級牌／特殊攻擊，點數＝等級總和×３，反制術式",
            FormationPattern::Custom("water-and-same-level".to_string()),
            3,
        ),
        profession_spell(
            "purple-light-shield",
            "紫光防罩",
            "水水＋三張任意牌／建構防護罩，點數＝等級總和×５",
            FormationPattern::Custom("water-water-three-any".to_string()),
        ),
        profession_attack(
            "magic-shock",
            "法震",
            "火＋同級牌／特殊攻擊，點數＝等級總和×３，反制攻擊傷害平分",
            FormationPattern::Custom("fire-and-same-level".to_string()),
            3,
        ),
        profession_attack_with_formula(
            "magic-reflection-flash",
            "魔法反光閃",
            "火火＋兩張非火牌／特殊攻擊，點數＝火×火×５，本回合無法抽牌",
            FormationPattern::Custom("fire-fire-two-non-fire".to_string()),
            PointFormula::ElementProductTimes {
                element: Element::Fire,
                multiplier: 5,
            },
        ),
        profession_spell(
            "shadow-assault",
            "影襲",
            "土＋同級牌／主動術式，上家扣除生命，點數＝等級總和×３",
            FormationPattern::Custom("earth-and-same-level".to_string()),
        ),
        profession_spell(
            "instant-shadow-death",
            "刃影瞬殺",
            "任意張數土、等級總和１０以上／主動術式，上家生命減半",
            FormationPattern::Custom("earth-sum-ten".to_string()),
        ),
        profession_attack_with_formula(
            "condensed-void-arrow",
            "凝華斷空箭",
            "三張同級牌／特殊攻擊，點數＝２０，本回合抽牌＋２",
            FormationPattern::Custom("three-same-level".to_string()),
            PointFormula::Fixed(20),
        ),
        profession_attack_with_formula(
            "sky-bow-roar",
            "天弓嘯",
            "５５５／特殊攻擊，點數＝６０",
            FormationPattern::Custom("three-level-five".to_string()),
            PointFormula::Fixed(60),
        ),
        profession_attack_with_formula(
            "holy-light-break",
            "聖光破",
            "一張偶級牌／特殊攻擊，點數＝１０",
            FormationPattern::Custom("single-even-level".to_string()),
            PointFormula::Fixed(10),
        ),
        profession_spell(
            "holy-wind",
            "聖風",
            "４４４／主動術式，下家扣除２０點生命並取得其一張最高等級手牌",
            FormationPattern::Custom("three-level-four".to_string()),
        ),
        profession_spell(
            "void-reversion",
            "虛空返璞術",
            "三張同級牌／主動術式，自身扣除２０點生命並破除職業",
            FormationPattern::Custom("three-same-level".to_string()),
        ),
    ]
}

fn profession_attack(
    id: &str,
    name: &str,
    rule_text: &str,
    pattern: FormationPattern,
    multiplier: u32,
) -> BaseFormationSpec {
    profession_attack_with_formula(
        id,
        name,
        rule_text,
        pattern,
        PointFormula::LevelSumTimes(multiplier),
    )
}

fn profession_attack_with_formula(
    id: &str,
    name: &str,
    rule_text: &str,
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
                category: if matches!(id, "divine-weapon" | "falling-light-slash") {
                    AttackCategory::Physical
                } else {
                    AttackCategory::Special
                },
                point_formula,
                damage_target: DamageTarget::PreviousPlayer,
            }),
        },
    }
}

fn profession_spell(
    id: &str,
    name: &str,
    rule_text: &str,
    pattern: FormationPattern,
) -> BaseFormationSpec {
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
            plan: EffectPlan::ActiveSpell(super::SpellPlanDef {
                resolver_id: id.to_string(),
                player_facing_effect: player_facing_formation_effect(id),
            }),
        },
    }
}

fn player_facing_formation_effect(id: &str) -> FormationEffect {
    match id {
        "reincarnation" => FormationEffect::RecoverHp,
        "purple-light-shield" => FormationEffect::CreateShield,
        "shadow-assault" => FormationEffect::DamagePreviousTeamByLevelSumTimes { multiplier: 3 },
        "instant-shadow-death" => FormationEffect::HalvePreviousTeamHp,
        "holy-wind" => FormationEffect::DamageNextTeamAndTakeHighestLevelHandCard { damage: 20 },
        "void-reversion" => FormationEffect::BreakProfession,
        _ => panic!("Hero formation `{id}` is missing a player-facing effect fact"),
    }
}
