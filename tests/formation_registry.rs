use fewfc::rules::{
    AttackCategory, AttackPlanDef, DamageTarget, EffectDef, EffectPlan, Element, FormationCategory,
    FormationDef, FormationPattern, FormationRegistry, PointFormula,
};

#[test]
fn registry_links_formation_schema_to_effect_plan() {
    let registry = FormationRegistry::new(
        vec![FormationDef {
            id: "metal-strike".to_string(),
            name: "Metal Strike".to_string(),
            category: FormationCategory::Attack(AttackCategory::Elemental(Element::Metal)),
            pattern: FormationPattern::ExactElements(vec![
                Element::Metal,
                Element::Metal,
                Element::Fire,
                Element::Water,
            ]),
            effect_id: "deal-formation-damage".to_string(),
            point_formula: PointFormula::Fixed(3),
        }],
        vec![EffectDef {
            id: "deal-formation-damage".to_string(),
            plan: EffectPlan::Attack(AttackPlanDef {
                point_formula: PointFormula::FormationPoints,
                damage_target: DamageTarget::PreviousPlayer,
            }),
        }],
    )
    .unwrap();

    let formation = registry.formation("metal-strike").unwrap();
    let effect = registry.effect_for(formation).unwrap();

    assert_eq!(formation.effect_id, "deal-formation-damage");
    assert_eq!(
        formation.category,
        FormationCategory::Attack(AttackCategory::Elemental(Element::Metal))
    );
    assert_eq!(effect.id, "deal-formation-damage");
    assert_eq!(
        effect.plan,
        EffectPlan::Attack(AttackPlanDef {
            point_formula: PointFormula::FormationPoints,
            damage_target: DamageTarget::PreviousPlayer,
        })
    );
}
