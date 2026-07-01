//! Rule registries: formations, effects, matchers, and formula resolvers.

pub(crate) mod base;
mod official;
pub(crate) mod projection;
pub(crate) mod star;

pub use official::OfficialRules;

pub use crate::domain::Element;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormationDef {
    pub id: String,
    pub name: String,
    pub rule_text: String,
    pub category: FormationCategory,
    pub pattern: FormationPattern,
    pub effect_id: String,
    pub point_formula: PointFormula,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormationCategory {
    Attack,
    Spell,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AttackCategory {
    Elemental(Element),
    Physical,
    Special,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FormationPattern {
    ExactElements(Vec<Element>),
    GeneratingSequence { length: usize },
    OvercomingSequence { length: usize },
    Custom(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SubmittedCardFacts {
    pub element: Element,
    pub level: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormationCandidate {
    pub formation_id: String,
    pub formation_name: String,
    pub rule_text: String,
    pub category: FormationCategory,
    pub cards: Vec<crate::domain::CardInstanceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayableAction {
    PerformFormation(FormationCandidate),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PointFormula {
    Fixed(u32),
    LevelPlus(u32),
    LevelSumTimes(u32),
    TargetHandCountTimes(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectDef {
    pub id: String,
    pub plan: EffectPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EffectPlan {
    Attack(AttackPlanDef),
    ActiveSpell(SpellPlanDef),
    PassiveSpell(SpellPlanDef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AttackPlanDef {
    pub category: AttackCategory,
    pub point_formula: PointFormula,
    pub damage_target: DamageTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpellPlanDef {
    pub resolver_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DamageTarget {
    PreviousPlayer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormationRegistry {
    formations: HashMap<String, FormationDef>,
    effects: HashMap<String, EffectDef>,
}

impl FormationRegistry {
    pub(crate) fn new(
        formations: Vec<FormationDef>,
        effects: Vec<EffectDef>,
    ) -> Result<Self, RegistryError> {
        let effects = effects
            .into_iter()
            .map(|effect| (effect.id.clone(), effect))
            .collect::<HashMap<_, _>>();

        let mut formation_map = HashMap::new();
        for formation in formations {
            if !effects.contains_key(&formation.effect_id) {
                return Err(RegistryError::MissingEffect {
                    formation_id: formation.id,
                    effect_id: formation.effect_id,
                });
            }

            if formation_map
                .insert(formation.id.clone(), formation)
                .is_some()
            {
                return Err(RegistryError::DuplicateFormation);
            }
        }

        Ok(Self {
            formations: formation_map,
            effects,
        })
    }

    pub(crate) fn formation(&self, id: &str) -> Option<&FormationDef> {
        self.formations.get(id)
    }

    pub(crate) fn effect(&self, id: &str) -> Option<&EffectDef> {
        self.effects.get(id)
    }

    pub(crate) fn effect_for(&self, formation: &FormationDef) -> Option<&EffectDef> {
        self.effect(&formation.effect_id)
    }

    pub(crate) fn formations(&self) -> Vec<&FormationDef> {
        let mut formations = self.formations.values().collect::<Vec<_>>();
        formations.sort_by(|left, right| left.id.cmp(&right.id));
        formations
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RegistryError {
    DuplicateFormation,
    MissingEffect {
        formation_id: String,
        effect_id: String,
    },
}

#[derive(Default)]
pub(crate) struct FormationMatcher<'a> {
    custom_matchers: HashMap<String, CustomMatcher<'a>>,
}

type CustomMatcher<'a> = Box<dyn Fn(&[SubmittedCardFacts]) -> bool + 'a>;

impl<'a> FormationMatcher<'a> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_custom(
        mut self,
        id: impl Into<String>,
        matcher: impl Fn(&[SubmittedCardFacts]) -> bool + 'a,
    ) -> Self {
        self.custom_matchers.insert(id.into(), Box::new(matcher));
        self
    }

    pub(crate) fn matches(
        &self,
        pattern: &FormationPattern,
        submitted: &[SubmittedCardFacts],
    ) -> bool {
        let submitted_elements = submitted_elements(submitted);
        match pattern {
            FormationPattern::ExactElements(elements) => {
                element_counts(elements) == element_counts(&submitted_elements)
            }
            FormationPattern::GeneratingSequence { length } => {
                matches_unordered_sequence(&submitted_elements, *length, &GENERATING_CYCLE)
            }
            FormationPattern::OvercomingSequence { length } => {
                matches_unordered_sequence(&submitted_elements, *length, &OVERCOMING_CYCLE)
            }
            FormationPattern::Custom(id) => self
                .custom_matchers
                .get(id)
                .is_some_and(|matcher| matcher(submitted)),
        }
    }
}

fn submitted_elements(submitted: &[SubmittedCardFacts]) -> Vec<Element> {
    submitted.iter().map(|card| card.element).collect()
}

fn element_counts(elements: &[Element]) -> HashMap<Element, usize> {
    let mut counts = HashMap::new();
    for element in elements {
        *counts.entry(*element).or_default() += 1;
    }
    counts
}

const GENERATING_CYCLE: [Element; 5] = [
    Element::Wood,
    Element::Fire,
    Element::Earth,
    Element::Metal,
    Element::Water,
];

const OVERCOMING_CYCLE: [Element; 5] = [
    Element::Wood,
    Element::Earth,
    Element::Water,
    Element::Fire,
    Element::Metal,
];

fn matches_unordered_sequence(submitted: &[Element], length: usize, cycle: &[Element; 5]) -> bool {
    if submitted.len() != length || length == 0 || length > cycle.len() {
        return false;
    }

    let submitted_counts = element_counts(submitted);

    (0..cycle.len()).any(|start| {
        let sequence = (0..length)
            .map(|offset| cycle[(start + offset) % cycle.len()])
            .collect::<Vec<_>>();
        element_counts(&sequence) == submitted_counts
    })
}

pub(crate) fn base_formation_registry() -> FormationRegistry {
    FormationRegistry::new(base_formations(), base_effects())
        .expect("base formation registry must be internally consistent")
}

pub(crate) fn official_formation_registry(
    modules: &[crate::domain::RuleModuleId],
) -> FormationRegistry {
    let mut specs = base_specs();
    if modules
        .iter()
        .any(|module| module.as_str() == crate::domain::FIVE_DIRECTIONS_LEGEND_MODULE_ID)
    {
        specs.extend(five_directions_legend_specs());
    }
    if modules
        .iter()
        .any(|module| module.as_str() == crate::domain::STAR_MODULE_ID)
    {
        specs.extend(star::specs());
    }
    FormationRegistry::new(
        specs.iter().map(|spec| spec.formation.clone()).collect(),
        specs.into_iter().map(|spec| spec.effect).collect(),
    )
    .expect("official formation registry must be internally consistent")
}

pub(crate) fn sacred_beast_element(formation_id: &str) -> Option<Element> {
    match formation_id {
        "east-azure-dragon" => Some(Element::Wood),
        "west-white-tiger" => Some(Element::Metal),
        "south-vermilion-bird" => Some(Element::Fire),
        "north-black-tortoise" => Some(Element::Water),
        "center-yellow-serpent" => Some(Element::Earth),
        _ => None,
    }
}

pub(crate) fn environment_makes_formation_ineffective(
    state: &crate::domain::GameState,
    formation_id: &str,
) -> bool {
    if !state.has_rule_module(crate::domain::FIVE_DIRECTIONS_LEGEND_MODULE_ID) {
        return false;
    }

    matches!(
        (state.environment, formation_id),
        (Some(Element::Metal), "defense" | "barrier")
            | (Some(Element::Wood), "metamorphosis" | "chaos")
            | (Some(Element::Water), "countershock" | "shock-burst")
            | (Some(Element::Fire), "weapon" | "radiance")
            | (Some(Element::Earth), "seal" | "return-to-origin")
    )
}

pub(crate) fn base_formation_matcher<'a>() -> FormationMatcher<'a> {
    FormationMatcher::new()
        .with_custom("two-different-elements", |submitted| {
            submitted.len() == 2 && submitted[0].element != submitted[1].element
        })
        .with_custom("five-same-level", |submitted| {
            let Some(first_card) = submitted.first() else {
                return false;
            };
            submitted.len() == 5 && submitted.iter().all(|card| card.level == first_card.level)
        })
        .with_custom("three-same-level", |submitted| {
            let Some(first_card) = submitted.first() else {
                return false;
            };
            submitted.len() == 3 && submitted.iter().all(|card| card.level == first_card.level)
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct BaseFormationSpec {
    formation: FormationDef,
    effect: EffectDef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpellTiming {
    Active,
    Passive,
}

fn base_formations() -> Vec<FormationDef> {
    base_specs()
        .into_iter()
        .map(|spec| spec.formation)
        .collect()
}

fn base_effects() -> Vec<EffectDef> {
    base_specs().into_iter().map(|spec| spec.effect).collect()
}

fn base_formation_rule_text(id: &str) -> &'static str {
    match id {
        "metal-strike" => "金行攻擊，點數＝等級＋４",
        "wood-strike" => "木行攻擊，點數＝等級＋４",
        "water-strike" => "水行攻擊，點數＝等級＋４",
        "fire-strike" => "火行攻擊，點數＝等級＋４",
        "earth-strike" => "土行攻擊，點數＝等級＋４",
        "weapon" => "物理攻擊，點數＝等級總和×２",
        "defense" => "被動術式，反制：下家下回合攻擊之傷害無效",
        "seal" => "被動術式，反制：下家下回合術式無效",
        "countershock" => "被動術式，反制：下家下回合攻擊之傷害由目標與施展者平分",
        "metamorphosis" => "主動術式，複製上家上回合施展之基礎規則陣法之類別與效果",
        "empty-city" => "被動術式，無效果",
        "triple-metal" => "金行攻擊，點數＝等級總和×３",
        "triple-wood" => "木行攻擊，點數＝等級總和×３",
        "triple-water" => "水行攻擊，點數＝等級總和×３",
        "triple-fire" => "火行攻擊，點數＝等級總和×３",
        "triple-earth" => "土行攻擊，點數＝等級總和×３",
        "generating-formation" => "主動術式，回復生命值，點數＝等級總和×３",
        "overcoming-formation" => "主動術式，扣除下家防護罩，點數＝等級總和×３",
        "radiance" => "主動術式，檢視下家手牌，下家無法行動及抽牌２回合",
        "barrier" => "主動術式，建構防護罩，點數＝等級總和×４",
        "return-to-origin" => "主動術式，回復生命值，點數＝等級總和×４",
        "shock-burst" => "物理攻擊，點數＝等級總和×４",
        "chaos" => "主動術式，檢視下家手牌，將其中兩張放回牌堆最上方",
        "five-elements-cycle" => "主動術式，雙方生命值交換",
        "five-streams-unite" => "特殊攻擊，點數＝目標手牌數×１５，本回合抽牌＋１",
        _ => panic!("missing base formation rule text for {id}"),
    }
}

fn base_specs() -> Vec<BaseFormationSpec> {
    vec![
        elemental_attack(
            "metal-strike",
            "金擊術",
            Element::Metal,
            1,
            PointFormula::LevelPlus(4),
        ),
        elemental_attack(
            "wood-strike",
            "木擊術",
            Element::Wood,
            1,
            PointFormula::LevelPlus(4),
        ),
        elemental_attack(
            "water-strike",
            "水擊術",
            Element::Water,
            1,
            PointFormula::LevelPlus(4),
        ),
        elemental_attack(
            "fire-strike",
            "火擊術",
            Element::Fire,
            1,
            PointFormula::LevelPlus(4),
        ),
        elemental_attack(
            "earth-strike",
            "土擊術",
            Element::Earth,
            1,
            PointFormula::LevelPlus(4),
        ),
        attack(
            "weapon",
            "武器",
            AttackCategory::Physical,
            FormationPattern::ExactElements(vec![Element::Metal, Element::Metal]),
            PointFormula::LevelSumTimes(2),
        ),
        spell(
            "defense",
            "防禦",
            SpellTiming::Passive,
            FormationPattern::ExactElements(vec![Element::Wood, Element::Wood]),
        ),
        spell(
            "seal",
            "封印",
            SpellTiming::Passive,
            FormationPattern::ExactElements(vec![Element::Water, Element::Water]),
        ),
        spell(
            "countershock",
            "反震",
            SpellTiming::Passive,
            FormationPattern::ExactElements(vec![Element::Fire, Element::Fire]),
        ),
        spell(
            "metamorphosis",
            "幻化",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![Element::Earth, Element::Earth]),
        ),
        spell(
            "empty-city",
            "空城",
            SpellTiming::Passive,
            FormationPattern::Custom("two-different-elements".to_string()),
        ),
        elemental_attack(
            "triple-metal",
            "鍠金",
            Element::Metal,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        elemental_attack(
            "triple-wood",
            "樸木",
            Element::Wood,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        elemental_attack(
            "triple-water",
            "洄水",
            Element::Water,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        elemental_attack(
            "triple-fire",
            "熾火",
            Element::Fire,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        elemental_attack(
            "triple-earth",
            "坱土",
            Element::Earth,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        spell(
            "generating-formation",
            "生陣",
            SpellTiming::Active,
            FormationPattern::GeneratingSequence { length: 3 },
        ),
        spell(
            "overcoming-formation",
            "剋陣",
            SpellTiming::Active,
            FormationPattern::OvercomingSequence { length: 3 },
        ),
        spell(
            "radiance",
            "光芒",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Metal,
                Element::Metal,
                Element::Fire,
                Element::Water,
            ]),
        ),
        spell(
            "barrier",
            "氣壁",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Wood,
                Element::Wood,
                Element::Metal,
                Element::Fire,
            ]),
        ),
        spell(
            "return-to-origin",
            "歸元",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Water,
                Element::Water,
                Element::Earth,
                Element::Wood,
            ]),
        ),
        attack(
            "shock-burst",
            "震暴",
            AttackCategory::Physical,
            FormationPattern::ExactElements(vec![
                Element::Fire,
                Element::Fire,
                Element::Water,
                Element::Earth,
            ]),
            PointFormula::LevelSumTimes(4),
        ),
        spell(
            "chaos",
            "混沌",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Earth,
                Element::Earth,
                Element::Wood,
                Element::Metal,
            ]),
        ),
        spell(
            "five-elements-cycle",
            "五行輪迴",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Metal,
                Element::Wood,
                Element::Water,
                Element::Fire,
                Element::Earth,
            ]),
        ),
        attack(
            "five-streams-unite",
            "五流歸一",
            AttackCategory::Special,
            FormationPattern::Custom("five-same-level".to_string()),
            PointFormula::TargetHandCountTimes(15),
        ),
    ]
}

fn five_directions_legend_specs() -> Vec<BaseFormationSpec> {
    vec![
        sacred_beast("east-azure-dragon", "東‧青龍", Element::Wood),
        sacred_beast("west-white-tiger", "西‧白虎", Element::Metal),
        sacred_beast("south-vermilion-bird", "南‧朱雀", Element::Fire),
        sacred_beast("north-black-tortoise", "北‧玄武", Element::Water),
        sacred_beast("center-yellow-serpent", "中‧黃蛇", Element::Earth),
        BaseFormationSpec {
            formation: FormationDef {
                id: "void-meridian-severing".to_string(),
                name: "虛空斷脈術".to_string(),
                rule_text: "主動術式，破除環境，環境被破除時雙方扣除２０點生命".to_string(),
                category: FormationCategory::Spell,
                pattern: FormationPattern::Custom("three-same-level".to_string()),
                effect_id: "void-meridian-severing".to_string(),
                point_formula: PointFormula::Fixed(0),
            },
            effect: EffectDef {
                id: "void-meridian-severing".to_string(),
                plan: EffectPlan::ActiveSpell(SpellPlanDef {
                    resolver_id: "void-meridian-severing".to_string(),
                }),
            },
        },
    ]
}

fn sacred_beast(id: &str, name: &str, element: Element) -> BaseFormationSpec {
    let formation = FormationDef {
        id: id.to_string(),
        name: name.to_string(),
        rule_text: format!(
            "{}行攻擊，點數＝８１，傷害後轉移環境，本陣法不受其他陣法效果影響",
            match element {
                Element::Metal => "金",
                Element::Wood => "木",
                Element::Water => "水",
                Element::Fire => "火",
                Element::Earth => "土",
            }
        ),
        category: FormationCategory::Attack,
        pattern: FormationPattern::ExactElements(vec![element; 5]),
        effect_id: id.to_string(),
        point_formula: PointFormula::Fixed(81),
    };
    let effect = EffectDef {
        id: id.to_string(),
        plan: EffectPlan::Attack(AttackPlanDef {
            category: AttackCategory::Elemental(element),
            point_formula: PointFormula::Fixed(81),
            damage_target: DamageTarget::PreviousPlayer,
        }),
    };

    BaseFormationSpec { formation, effect }
}

fn elemental_attack(
    id: &str,
    name: &str,
    element: Element,
    count: usize,
    point_formula: PointFormula,
) -> BaseFormationSpec {
    attack(
        id,
        name,
        AttackCategory::Elemental(element),
        FormationPattern::ExactElements(vec![element; count]),
        point_formula,
    )
}

fn attack(
    id: &str,
    name: &str,
    category: AttackCategory,
    pattern: FormationPattern,
    point_formula: PointFormula,
) -> BaseFormationSpec {
    let formation = FormationDef {
        id: id.to_string(),
        name: name.to_string(),
        rule_text: base_formation_rule_text(id).to_string(),
        category: FormationCategory::Attack,
        pattern,
        effect_id: id.to_string(),
        point_formula: point_formula.clone(),
    };
    let effect = EffectDef {
        id: id.to_string(),
        plan: EffectPlan::Attack(AttackPlanDef {
            category,
            point_formula,
            damage_target: DamageTarget::PreviousPlayer,
        }),
    };

    BaseFormationSpec { formation, effect }
}

fn spell(
    id: &str,
    name: &str,
    timing: SpellTiming,
    pattern: FormationPattern,
) -> BaseFormationSpec {
    let formation = FormationDef {
        id: id.to_string(),
        name: name.to_string(),
        rule_text: base_formation_rule_text(id).to_string(),
        category: FormationCategory::Spell,
        pattern,
        effect_id: id.to_string(),
        point_formula: PointFormula::Fixed(0),
    };
    let plan = match timing {
        SpellTiming::Active => EffectPlan::ActiveSpell(SpellPlanDef {
            resolver_id: id.to_string(),
        }),
        SpellTiming::Passive => EffectPlan::PassiveSpell(SpellPlanDef {
            resolver_id: id.to_string(),
        }),
    };
    let effect = EffectDef {
        id: id.to_string(),
        plan,
    };

    BaseFormationSpec { formation, effect }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(element: Element) -> SubmittedCardFacts {
        leveled_card(element, 1)
    }

    fn leveled_card(element: Element, level: u32) -> SubmittedCardFacts {
        SubmittedCardFacts { element, level }
    }

    #[test]
    fn matcher_handles_unordered_exact_counts_and_sequences() {
        let matcher = FormationMatcher::default();

        assert!(matcher.matches(
            &FormationPattern::ExactElements(vec![
                Element::Metal,
                Element::Metal,
                Element::Fire,
                Element::Water,
            ]),
            &[
                card(Element::Water),
                card(Element::Metal),
                card(Element::Fire),
                card(Element::Metal),
            ],
        ));
        assert!(matcher.matches(
            &FormationPattern::GeneratingSequence { length: 3 },
            &[
                card(Element::Earth),
                card(Element::Wood),
                card(Element::Fire),
            ],
        ));
        assert!(matcher.matches(
            &FormationPattern::OvercomingSequence { length: 3 },
            &[
                card(Element::Water),
                card(Element::Wood),
                card(Element::Earth),
            ],
        ));
    }

    #[test]
    fn matcher_rejects_incomplete_or_wrong_patterns() {
        let matcher = FormationMatcher::default();

        assert!(!matcher.matches(
            &FormationPattern::ExactElements(vec![Element::Metal, Element::Fire, Element::Water,]),
            &[card(Element::Metal), card(Element::Fire)],
        ));
        assert!(!matcher.matches(
            &FormationPattern::GeneratingSequence { length: 3 },
            &[
                card(Element::Metal),
                card(Element::Fire),
                card(Element::Water),
            ],
        ));
        assert!(!matcher.matches(
            &FormationPattern::OvercomingSequence { length: 3 },
            &[
                card(Element::Wood),
                card(Element::Fire),
                card(Element::Earth),
            ],
        ));
    }

    #[test]
    fn matcher_keeps_custom_hooks_internal() {
        let matcher = FormationMatcher::new().with_custom("all-water", |submitted| {
            submitted.len() == 3
                && submitted
                    .iter()
                    .all(|card| card.element == Element::Water && card.level > 1)
        });
        let pattern = FormationPattern::Custom("all-water".to_string());

        assert!(matcher.matches(
            &pattern,
            &[
                leveled_card(Element::Water, 2),
                leveled_card(Element::Water, 3),
                leveled_card(Element::Water, 4),
            ],
        ));
        assert!(!matcher.matches(
            &pattern,
            &[
                leveled_card(Element::Water, 2),
                leveled_card(Element::Water, 1),
                leveled_card(Element::Water, 4),
            ],
        ));
    }

    #[test]
    fn registry_links_formation_schema_to_effect_plan() {
        let registry = FormationRegistry::new(
            vec![FormationDef {
                id: "metal-strike".to_string(),
                name: "Metal Strike".to_string(),
                rule_text: "Elemental attack".to_string(),
                category: FormationCategory::Attack,
                pattern: FormationPattern::ExactElements(vec![Element::Metal]),
                effect_id: "deal-formation-damage".to_string(),
                point_formula: PointFormula::Fixed(3),
            }],
            vec![EffectDef {
                id: "deal-formation-damage".to_string(),
                plan: EffectPlan::Attack(AttackPlanDef {
                    category: AttackCategory::Elemental(Element::Metal),
                    point_formula: PointFormula::LevelSumTimes(1),
                    damage_target: DamageTarget::PreviousPlayer,
                }),
            }],
        )
        .unwrap();

        let formation = registry.formation("metal-strike").unwrap();
        let effect = registry.effect_for(formation).unwrap();

        assert_eq!(formation.effect_id, "deal-formation-damage");
        assert_eq!(effect.id, "deal-formation-damage");
        assert!(matches!(effect.plan, EffectPlan::Attack(_)));
    }

    #[test]
    fn base_registry_contains_every_documented_formation() {
        let registry = base_formation_registry();
        let expected = [
            ("metal-strike", "金擊術"),
            ("wood-strike", "木擊術"),
            ("water-strike", "水擊術"),
            ("fire-strike", "火擊術"),
            ("earth-strike", "土擊術"),
            ("weapon", "武器"),
            ("defense", "防禦"),
            ("seal", "封印"),
            ("countershock", "反震"),
            ("metamorphosis", "幻化"),
            ("empty-city", "空城"),
            ("triple-metal", "鍠金"),
            ("triple-wood", "樸木"),
            ("triple-water", "洄水"),
            ("triple-fire", "熾火"),
            ("triple-earth", "坱土"),
            ("generating-formation", "生陣"),
            ("overcoming-formation", "剋陣"),
            ("radiance", "光芒"),
            ("barrier", "氣壁"),
            ("return-to-origin", "歸元"),
            ("shock-burst", "震暴"),
            ("chaos", "混沌"),
            ("five-elements-cycle", "五行輪迴"),
            ("five-streams-unite", "五流歸一"),
        ];

        assert_eq!(registry.formations().len(), expected.len());
        assert_eq!(registry.effects.len(), expected.len());
        for (id, name) in expected {
            let formation = registry.formation(id).expect("base formation must exist");
            assert_eq!(formation.name, name);
            assert!(!formation.rule_text.is_empty());
            assert!(registry.effect_for(formation).is_some());
        }
    }

    #[test]
    fn base_registry_patterns_and_effect_plans_are_consistent() {
        let registry = base_formation_registry();
        let matcher = base_formation_matcher();

        assert!(matcher.matches(
            &registry.formation("empty-city").unwrap().pattern,
            &[card(Element::Metal), card(Element::Wood)],
        ));
        assert!(!matcher.matches(
            &registry.formation("empty-city").unwrap().pattern,
            &[card(Element::Metal), card(Element::Metal)],
        ));
        assert!(matcher.matches(
            &registry.formation("five-streams-unite").unwrap().pattern,
            &[
                leveled_card(Element::Metal, 3),
                leveled_card(Element::Wood, 3),
                leveled_card(Element::Water, 3),
                leveled_card(Element::Fire, 3),
                leveled_card(Element::Earth, 3),
            ],
        ));
        assert_eq!(
            registry.effect("triple-fire").unwrap().plan,
            EffectPlan::Attack(AttackPlanDef {
                category: AttackCategory::Elemental(Element::Fire),
                point_formula: PointFormula::LevelSumTimes(3),
                damage_target: DamageTarget::PreviousPlayer,
            }),
        );
        assert_eq!(
            registry.effect("barrier").unwrap().plan,
            EffectPlan::ActiveSpell(SpellPlanDef {
                resolver_id: "barrier".to_string(),
            }),
        );
    }
}
