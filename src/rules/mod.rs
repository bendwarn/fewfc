//! Rule registries: formations, effects, matchers, and formula resolvers.

use std::collections::HashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    Metal,
    Wood,
    Water,
    Fire,
    Earth,
}

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
    Attack(AttackCategory),
    Spell(SpellCategory),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttackCategory {
    Elemental(Element),
    Physical,
    Special,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SpellCategory {
    Active,
    Passive,
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
