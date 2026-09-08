use fewfc::domain::{GameSetup, PlayerId, RuleModuleId, RuleVersion};
use fewfc::rules::OfficialRules;

#[test]
fn unversioned_setup_preserves_legacy_modules_and_version() {
    let setup = GameSetup::two_player(PlayerId::new("alice"), PlayerId::new("bob"), 100)
        .with_rule_modules(vec![RuleModuleId::new("echo")]);
    let mut value = serde_json::to_value(&setup).unwrap();
    value.as_object_mut().unwrap().remove("rule_version");
    let restored: GameSetup = serde_json::from_value(value).unwrap();
    assert_eq!(restored.rule_version, RuleVersion::V5_16);
    assert_eq!(restored.enabled_rule_modules, setup.enabled_rule_modules);
}

#[test]
fn version_catalogs_reject_cross_version_themes_and_default_to_available_themes() {
    let rules = OfficialRules::new();
    let latest = rules
        .resolve_version_modules(RuleVersion::V5_17, None)
        .unwrap();
    assert!(latest.contains(&RuleModuleId::new("totem-formation")));
    assert!(!latest.contains(&RuleModuleId::new("echo")));
    assert!(!latest.contains(&RuleModuleId::new("tribulation")));
    assert!(latest.contains(&RuleModuleId::new("personal-deck")));
    let old = rules
        .resolve_version_modules(RuleVersion::V5_16, None)
        .unwrap();
    assert!(old.contains(&RuleModuleId::new("echo")));
    assert!(!old.contains(&RuleModuleId::new("totem-formation")));
    assert!(
        rules
            .resolve_version_modules(RuleVersion::V5_17, Some(old))
            .is_err()
    );
    assert!(
        rules
            .resolve_version_modules(RuleVersion::V5_16, Some(latest))
            .is_err()
    );
}

#[test]
fn latest_version_is_applied_before_setup_validation() {
    let rules = OfficialRules::new();
    let base = GameSetup::two_player(PlayerId::new("alice"), PlayerId::new("bob"), 100);
    let setup = rules
        .configure_versioned_game_with_decks(
            RuleVersion::V5_17,
            base.players,
            base.turn_order,
            rules
                .resolve_version_modules(RuleVersion::V5_17, None)
                .unwrap(),
            Vec::new(),
        )
        .unwrap();
    assert_eq!(setup.rule_version, RuleVersion::V5_17);
}

#[test]
fn web_rule_version_contract_uses_camel_case_and_defaults_new_resolution_to_latest() {
    let latest: serde_json::Value =
        serde_json::from_str(&fewfc::web_api::resolve_rule_modules_json("{}").unwrap()).unwrap();
    assert!(
        latest["modules"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("totem-formation"))
    );
    let old: serde_json::Value = serde_json::from_str(
        &fewfc::web_api::resolve_rule_modules_json(r#"{"ruleVersion":"5.16"}"#).unwrap(),
    )
    .unwrap();
    assert!(
        old["modules"]
            .as_array()
            .unwrap()
            .contains(&serde_json::json!("echo"))
    );
    assert!(fewfc::web_api::resolve_rule_modules_json(r#"{"ruleVersion":"5.18"}"#).is_err());
}

#[test]
fn snapshots_and_public_views_retain_selected_version_with_partial_modules() {
    let setup = GameSetup::two_player(PlayerId::new("alice"), PlayerId::new("bob"), 100)
        .with_rule_version(RuleVersion::V5_17);
    let state = fewfc::domain::GameState::from_setup(&setup);
    let serialized = serde_json::to_value(&state).unwrap();
    let restored: fewfc::domain::GameState = serde_json::from_value(serialized.clone()).unwrap();
    assert_eq!(restored.rule_version, RuleVersion::V5_17);
    assert_eq!(
        fewfc::public_view::state_for(&restored, fewfc::public_view::Viewer::Observer).rule_version,
        RuleVersion::V5_17
    );
    let mut legacy = serialized;
    legacy.as_object_mut().unwrap().remove("rule_version");
    let restored: fewfc::domain::GameState = serde_json::from_value(legacy).unwrap();
    assert_eq!(restored.rule_version, RuleVersion::V5_16);
    assert!(restored.enabled_rule_modules.is_empty());
}
