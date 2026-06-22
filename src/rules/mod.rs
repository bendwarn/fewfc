//! Rule registries: formations, effects, matchers, and formula resolvers.

pub use crate::domain::Element;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormationDef {
    pub id: String,
    pub name: String,
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
pub enum AttackCategory {
    Elemental(Element),
    Physical,
    Special,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormationPattern {
    ExactElements(Vec<Element>),
    ElementCounts(Vec<ElementCount>),
    GeneratingSequence { length: usize },
    OvercomingSequence { length: usize },
    Custom(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ElementCount {
    pub element: Element,
    pub count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PointFormula {
    Fixed(u32),
    CardCount,
    FormationPoints,
    LevelPlus(u32),
    LevelSumTimes(u32),
    TargetHandCountTimes(u32),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EffectDef {
    pub id: String,
    pub plan: EffectPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EffectPlan {
    Attack(AttackPlanDef),
    ActiveSpell(SpellPlanDef),
    PassiveSpell(SpellPlanDef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttackPlanDef {
    pub category: AttackCategory,
    pub point_formula: PointFormula,
    pub damage_target: DamageTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpellPlanDef {
    pub resolver_id: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DamageTarget {
    PreviousPlayer,
    DeclaredPlayer,
    TeamOfDeclaredPlayer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormationRegistry {
    formations: HashMap<String, FormationDef>,
    effects: HashMap<String, EffectDef>,
}

impl FormationRegistry {
    pub fn new(
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

    pub fn formation(&self, id: &str) -> Option<&FormationDef> {
        self.formations.get(id)
    }

    pub fn effect(&self, id: &str) -> Option<&EffectDef> {
        self.effects.get(id)
    }

    pub fn effect_for(&self, formation: &FormationDef) -> Option<&EffectDef> {
        self.effect(&formation.effect_id)
    }

    pub fn formation_count(&self) -> usize {
        self.formations.len()
    }

    pub fn effect_count(&self) -> usize {
        self.effects.len()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegistryError {
    DuplicateFormation,
    MissingEffect {
        formation_id: String,
        effect_id: String,
    },
}

#[derive(Default)]
pub struct FormationMatcher<'a> {
    custom_matchers: HashMap<String, CustomMatcher<'a>>,
}

type CustomMatcher<'a> = Box<dyn Fn(&[Element]) -> bool + 'a>;

impl<'a> FormationMatcher<'a> {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_custom(
        mut self,
        id: impl Into<String>,
        matcher: impl Fn(&[Element]) -> bool + 'a,
    ) -> Self {
        self.custom_matchers.insert(id.into(), Box::new(matcher));
        self
    }

    pub fn matches(&self, pattern: &FormationPattern, submitted: &[Element]) -> bool {
        match pattern {
            FormationPattern::ExactElements(elements) => {
                element_counts(elements) == element_counts(submitted)
            }
            FormationPattern::ElementCounts(required_counts) => {
                required_counts
                    .iter()
                    .map(|required| required.count)
                    .sum::<usize>()
                    == submitted.len()
                    && required_counts.iter().all(|required| {
                        submitted
                            .iter()
                            .filter(|element| **element == required.element)
                            .count()
                            == required.count
                    })
            }
            FormationPattern::GeneratingSequence { length } => {
                matches_unordered_sequence(submitted, *length, &GENERATING_CYCLE)
            }
            FormationPattern::OvercomingSequence { length } => {
                matches_unordered_sequence(submitted, *length, &OVERCOMING_CYCLE)
            }
            FormationPattern::Custom(id) => self
                .custom_matchers
                .get(id)
                .is_some_and(|matcher| matcher(submitted)),
        }
    }
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

pub fn base_formation_registry() -> FormationRegistry {
    FormationRegistry::new(base_formations(), base_effects())
        .expect("base formation registry must be internally consistent")
}

pub fn base_formation_matcher<'a>() -> FormationMatcher<'a> {
    FormationMatcher::new()
        .with_custom("two-different-elements", |submitted| {
            submitted.len() == 2 && submitted[0] != submitted[1]
        })
        .with_custom("five-same-level", |submitted| submitted.len() == 5)
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
