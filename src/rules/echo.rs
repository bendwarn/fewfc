use crate::domain::{ECHO_MODULE_ID, Element};
use crate::rules::{
    BaseFormationSpec, EffectDef, EffectPlan, FormationCategory, FormationDef, FormationPattern,
    PointFormula, SpellPlanDef,
};

pub(crate) const RINGING_METAL: &str = "echo:ringing-metal";
pub(crate) const FALLING_WOOD: &str = "echo:falling-wood";
pub(crate) const FLOWING_WATER: &str = "echo:flowing-water";
pub(crate) const WAR_FIRE: &str = "echo:war-fire";
pub(crate) const SPLIT_EARTH: &str = "echo:split-earth";

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
    pub echo_policy: EchoPolicy,
}

pub(crate) fn melody_catalog() -> Vec<MelodyDef> {
    debug_assert_eq!(ECHO_MODULE_ID, "echo");
    debug_assert!(MelodyExecutionOrigin::FormationUse.can_schedule_echo());
    debug_assert!(!MelodyExecutionOrigin::Echo.can_schedule_echo());
    debug_assert!(!MelodyExecutionOrigin::PlantedEarth.can_schedule_echo());
    let _supported_future_policies = [EchoPolicy::Automatic, EchoPolicy::None];
    vec![
        melody(
            RINGING_METAL,
            "商調‧鳴金",
            Element::Metal,
            [Element::Metal, Element::Earth],
        ),
        melody(
            FALLING_WOOD,
            "角調‧落木",
            Element::Wood,
            [Element::Wood, Element::Water],
        ),
        melody(
            FLOWING_WATER,
            "羽調‧流水",
            Element::Water,
            [Element::Water, Element::Metal],
        ),
        melody(
            WAR_FIRE,
            "徵調‧戰火",
            Element::Fire,
            [Element::Fire, Element::Wood],
        ),
        melody(
            SPLIT_EARTH,
            "宮調‧裂土",
            Element::Earth,
            [Element::Earth, Element::Fire],
        ),
    ]
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
                pattern: FormationPattern::ExactElements(vec![melody.element, melody.element]),
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

fn melody(
    id: &'static str,
    name: &'static str,
    element: Element,
    allowed_printed_elements: [Element; 2],
) -> MelodyDef {
    MelodyDef {
        id,
        name,
        element,
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
        _ => unreachable!("rule text is defined for every Melody"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn catalog_matches_the_five_published_basic_melodies() {
        let catalog = melody_catalog();
        assert_eq!(catalog.len(), 5);
        assert_eq!(
            catalog
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
        assert!(
            catalog
                .iter()
                .all(|melody| matches!(melody.echo_policy, EchoPolicy::OptionalCost { .. }))
        );
    }

    #[test]
    fn delayed_origins_cannot_schedule_recursive_echo() {
        assert!(MelodyExecutionOrigin::FormationUse.can_schedule_echo());
        assert!(!MelodyExecutionOrigin::Echo.can_schedule_echo());
        assert!(!MelodyExecutionOrigin::PlantedEarth.can_schedule_echo());
        let _future_policies = [EchoPolicy::Automatic, EchoPolicy::None];
        assert_eq!(ECHO_MODULE_ID, "echo");
    }
}
