use fewfc::rules::{
    AttackCategory, AttackPlanDef, DamageTarget, EffectDef, EffectPlan, Element, FormationCategory,
    FormationDef, FormationPattern, FormationRegistry, PointFormula, base_formation_matcher,
    base_formation_registry,
};

#[test]
fn registry_links_formation_schema_to_effect_plan() {
    let registry = FormationRegistry::new(
        vec![FormationDef {
            id: "metal-strike".to_string(),
            name: "Metal Strike".to_string(),
            category: FormationCategory::Attack,
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
                category: AttackCategory::Elemental(Element::Metal),
                point_formula: PointFormula::FormationPoints,
                damage_target: DamageTarget::PreviousPlayer,
            }),
        }],
    )
    .unwrap();

    let formation = registry.formation("metal-strike").unwrap();
    let effect = registry.effect_for(formation).unwrap();

    assert_eq!(formation.effect_id, "deal-formation-damage");
    assert_eq!(formation.category, FormationCategory::Attack);
    assert_eq!(effect.id, "deal-formation-damage");
    assert_eq!(
        effect.plan,
        EffectPlan::Attack(AttackPlanDef {
            category: AttackCategory::Elemental(Element::Metal),
            point_formula: PointFormula::FormationPoints,
            damage_target: DamageTarget::PreviousPlayer,
        })
    );
}

#[test]
fn base_registry_exposes_all_25_base_formations_with_linked_effects() {
    let registry = base_formation_registry();

    assert_eq!(registry.formation_count(), 25);
    assert_eq!(registry.effect_count(), 25);

    let metal_strike = registry.formation("metal-strike").unwrap();
    assert_eq!(metal_strike.name, "金擊術");
    assert_eq!(metal_strike.category, FormationCategory::Attack);
    assert_eq!(
        metal_strike.pattern,
        FormationPattern::ExactElements(vec![Element::Metal])
    );
    assert_eq!(
        registry.effect_for(metal_strike).unwrap().id,
        "metal-strike"
    );

    let defense = registry.formation("defense").unwrap();
    assert_eq!(defense.name, "防禦");
    assert_eq!(defense.category, FormationCategory::Spell);
    assert!(matches!(
        registry.effect_for(defense).unwrap().plan,
        EffectPlan::PassiveSpell(_)
    ));
    assert_eq!(registry.effect_for(defense).unwrap().id, "defense");

    let five_streams = registry.formation("five-streams-unite").unwrap();
    assert_eq!(five_streams.name, "五流歸一");
    assert_eq!(five_streams.category, FormationCategory::Attack);
    assert!(matches!(
        registry.effect_for(five_streams).unwrap().plan,
        EffectPlan::Attack(AttackPlanDef {
            category: AttackCategory::Special,
            ..
        })
    ));
    assert_eq!(
        registry.effect_for(five_streams).unwrap().id,
        "five-streams-unite"
    );
}

#[test]
fn base_registry_documents_all_base_formation_ids_and_names() {
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

    for (id, name) in expected {
        assert_eq!(
            registry
                .formation(id)
                .map(|formation| formation.name.as_str()),
            Some(name)
        );
    }
}

#[test]
fn base_registry_patterns_match_representative_base_formations() {
    let registry = base_formation_registry();
    let matcher = base_formation_matcher();

    assert!(matcher.matches(
        &registry.formation("radiance").unwrap().pattern,
        &[
            Element::Water,
            Element::Metal,
            Element::Fire,
            Element::Metal
        ],
    ));
    assert!(matcher.matches(
        &registry.formation("generating-formation").unwrap().pattern,
        &[Element::Earth, Element::Wood, Element::Fire],
    ));
    assert!(matcher.matches(
        &registry.formation("overcoming-formation").unwrap().pattern,
        &[Element::Water, Element::Wood, Element::Earth],
    ));
    assert!(matcher.matches(
        &registry.formation("empty-city").unwrap().pattern,
        &[Element::Metal, Element::Wood],
    ));
    assert!(!matcher.matches(
        &registry.formation("empty-city").unwrap().pattern,
        &[Element::Metal, Element::Metal],
    ));
    assert!(matcher.matches(
        &registry.formation("five-streams-unite").unwrap().pattern,
        &[
            Element::Metal,
            Element::Wood,
            Element::Water,
            Element::Fire,
            Element::Earth,
        ],
    ));
}

#[test]
fn base_registry_effect_plans_match_formation_categories() {
    let registry = base_formation_registry();

    assert_eq!(
        registry.effect("triple-fire").unwrap().plan,
        EffectPlan::Attack(AttackPlanDef {
            category: AttackCategory::Elemental(Element::Fire),
            point_formula: PointFormula::LevelSumTimes(3),
            damage_target: DamageTarget::PreviousPlayer,
        })
    );
    assert_eq!(
        registry.effect("shock-burst").unwrap().plan,
        EffectPlan::Attack(AttackPlanDef {
            category: AttackCategory::Physical,
            point_formula: PointFormula::LevelSumTimes(4),
            damage_target: DamageTarget::PreviousPlayer,
        })
    );
    assert_eq!(
        registry.effect("barrier").unwrap().plan,
        EffectPlan::ActiveSpell(fewfc::rules::SpellPlanDef {
            resolver_id: "barrier".to_string(),
        })
    );
    assert_eq!(
        registry.effect("seal").unwrap().plan,
        EffectPlan::PassiveSpell(fewfc::rules::SpellPlanDef {
            resolver_id: "seal".to_string(),
        })
    );
}
