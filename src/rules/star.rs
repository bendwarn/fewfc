use crate::domain::{Element, StarKind};

use super::{
    AttackCategory, AttackPlanDef, BaseFormationSpec, DamageTarget, EffectDef, EffectPlan,
    FormationDef, FormationEffect, FormationPattern, PointFormula, SpellPlanDef,
};

pub(super) fn specs() -> Vec<BaseFormationSpec> {
    let mut specs = Vec::new();
    for star in all_stars() {
        specs.push(star_strike(star));
        specs.push(three_card_star_formation(star));
    }
    specs.push(void_star_breaking());
    specs
}

pub(crate) fn all_stars() -> [StarKind; 5] {
    [
        StarKind::Metal,
        StarKind::Wood,
        StarKind::Water,
        StarKind::Fire,
        StarKind::Earth,
    ]
}

pub(crate) fn element(star: StarKind) -> Element {
    match star {
        StarKind::Metal => Element::Metal,
        StarKind::Wood => Element::Wood,
        StarKind::Water => Element::Water,
        StarKind::Fire => Element::Fire,
        StarKind::Earth => Element::Earth,
    }
}

pub(crate) fn companion_element(star: StarKind) -> Element {
    match star {
        StarKind::Metal => Element::Earth,
        StarKind::Wood => Element::Water,
        StarKind::Water => Element::Metal,
        StarKind::Fire => Element::Wood,
        StarKind::Earth => Element::Fire,
    }
}

pub(crate) fn opposing_star(star: StarKind) -> StarKind {
    match star {
        StarKind::Metal => StarKind::Wood,
        StarKind::Wood => StarKind::Earth,
        StarKind::Water => StarKind::Fire,
        StarKind::Fire => StarKind::Metal,
        StarKind::Earth => StarKind::Water,
    }
}

pub(crate) fn summoning_formation_star(formation_id: &str) -> Option<StarKind> {
    match formation_id {
        "triple-metal" => Some(StarKind::Metal),
        "triple-wood" => Some(StarKind::Wood),
        "triple-water" => Some(StarKind::Water),
        "triple-fire" => Some(StarKind::Fire),
        "triple-earth" => Some(StarKind::Earth),
        _ => None,
    }
}

pub(crate) fn required_star(formation_id: &str) -> Option<StarKind> {
    all_stars()
        .into_iter()
        .find(|star| formation_id == strike_id(*star) || formation_id == formation_id_for(*star))
}

pub(crate) fn three_card_formation_star(formation_id: &str) -> Option<StarKind> {
    all_stars()
        .into_iter()
        .find(|star| formation_id == formation_id_for(*star))
}

pub(crate) fn star_name(star: StarKind) -> &'static str {
    match star {
        StarKind::Metal => "金星‧太白",
        StarKind::Wood => "木星‧歲星",
        StarKind::Water => "水星‧辰星",
        StarKind::Fire => "火星‧熒惑",
        StarKind::Earth => "土星‧鎮星",
    }
}

fn strike_id(star: StarKind) -> &'static str {
    match star {
        StarKind::Metal => "taibai-star-strike",
        StarKind::Wood => "suixing-star-strike",
        StarKind::Water => "chenxing-star-strike",
        StarKind::Fire => "yinghuo-star-strike",
        StarKind::Earth => "zhenxing-star-strike",
    }
}

fn formation_id_for(star: StarKind) -> &'static str {
    match star {
        StarKind::Metal => "taibai-heaven-forging",
        StarKind::Wood => "suixing-heaven-pillar",
        StarKind::Water => "chenxing-heaven-quenching",
        StarKind::Fire => "yinghuo-heaven-blazing",
        StarKind::Earth => "zhenxing-heaven-bearing",
    }
}

fn strike_name(star: StarKind) -> &'static str {
    match star {
        StarKind::Metal => "太白星擊",
        StarKind::Wood => "歲星星擊",
        StarKind::Water => "辰星星擊",
        StarKind::Fire => "熒惑星擊",
        StarKind::Earth => "鎮星星擊",
    }
}

fn formation_name(star: StarKind) -> &'static str {
    match star {
        StarKind::Metal => "太白鍊天陣",
        StarKind::Wood => "歲星柱天陣",
        StarKind::Water => "辰星淬天陣",
        StarKind::Fire => "熒惑燎天陣",
        StarKind::Earth => "鎮星堪天陣",
    }
}

fn star_strike(star: StarKind) -> BaseFormationSpec {
    attack_spec(
        strike_id(star),
        strike_name(star),
        format!("{}行攻擊，點數＝１０", element_label(element(star))),
        FormationPattern::ExactElements(vec![element(star)]),
        element(star),
        PointFormula::Fixed(10),
    )
}

fn three_card_star_formation(star: StarKind) -> BaseFormationSpec {
    attack_spec(
        formation_id_for(star),
        formation_name(star),
        format!(
            "{}行攻擊，點數＝等級總和×３，本回合抽牌＋１，破除{}",
            element_label(element(star)),
            star_name(star)
        ),
        FormationPattern::ExactElements(vec![
            element(star),
            element(star),
            companion_element(star),
        ]),
        element(star),
        PointFormula::LevelSumTimes(3),
    )
}

fn attack_spec(
    id: &str,
    name: &str,
    rule_text: String,
    pattern: FormationPattern,
    element: Element,
    point_formula: PointFormula,
) -> BaseFormationSpec {
    BaseFormationSpec {
        formation: FormationDef {
            id: id.to_string(),
            name: name.to_string(),
            rule_text,
            category: super::FormationCategory::Attack,
            pattern,
            effect_id: id.to_string(),
            point_formula: point_formula.clone(),
        },
        effect: EffectDef {
            id: id.to_string(),
            plan: EffectPlan::Attack(AttackPlanDef {
                category: AttackCategory::Elemental(element),
                point_formula,
                damage_target: DamageTarget::PreviousPlayer,
            }),
        },
    }
}

fn void_star_breaking() -> BaseFormationSpec {
    BaseFormationSpec {
        formation: FormationDef {
            id: "void-star-breaking".to_string(),
            name: "虛空破星術".to_string(),
            rule_text: "主動術式，破除所有星辰，星辰被破除方扣除２０點生命".to_string(),
            category: super::FormationCategory::Spell,
            pattern: FormationPattern::Custom("three-same-level".to_string()),
            effect_id: "void-star-breaking".to_string(),
            point_formula: PointFormula::Fixed(0),
        },
        effect: EffectDef {
            id: "void-star-breaking".to_string(),
            plan: EffectPlan::ActiveSpell(SpellPlanDef {
                resolver_id: "void-star-breaking".to_string(),
                player_facing_effect: FormationEffect::BreakStars,
            }),
        },
    }
}

fn element_label(element: Element) -> &'static str {
    match element {
        Element::Metal => "金",
        Element::Wood => "木",
        Element::Water => "水",
        Element::Fire => "火",
        Element::Earth => "土",
    }
}
