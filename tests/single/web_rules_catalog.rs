use fewfc::web_api::rules_catalog_json;
use serde_json::Value;

#[test]
fn rules_catalog_serializes_the_versioned_web_contract() {
    let catalog: Value = serde_json::from_str(&rules_catalog_json().expect("rules catalog JSON"))
        .expect("valid rules catalog JSON");

    assert_eq!(catalog["version"], 1);
    assert_eq!(catalog["ruleModules"].as_array().unwrap().len(), 13);
    assert_eq!(
        catalog["deckComposition"]["sharedDeck"]["exactCardCount"],
        90
    );
    assert_eq!(
        catalog["deckComposition"]["cardDefinitions"]
            .as_array()
            .unwrap()
            .len(),
        25
    );
    assert_eq!(
        catalog["deckComposition"]["personalDeck"]["exactCardCount"],
        60
    );
    assert_eq!(
        catalog["deckComposition"]["personalDeck"]["maximumLevelTotal"],
        170
    );

    let spirit = catalog["ruleModules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["id"] == "spirit")
        .unwrap();
    assert_eq!(spirit["category"], "theme");
    assert_eq!(spirit["defaultEnabled"], true);
    assert_eq!(
        spirit["dependencies"],
        serde_json::json!(["star", "five-directions-legend", "hero-schools"])
    );

    let totem = catalog["ruleModules"]
        .as_array()
        .unwrap()
        .iter()
        .find(|module| module["id"] == "totem-formation")
        .unwrap();
    assert_eq!(totem["id"], "totem-formation");
    assert_eq!(totem["category"], "theme");
    assert_eq!(totem["defaultEnabled"], true);
    assert_eq!(
        totem["dependencies"],
        serde_json::json!(["star", "five-directions-legend", "hero-schools"])
    );
}

#[test]
fn personal_deck_resolution_serializes_the_authoritative_validation_result() {
    let catalog: Value = serde_json::from_str(&rules_catalog_json().unwrap()).unwrap();
    let cards = catalog["deckComposition"]["personalDeck"]["preconstructed"]["cards"].clone();
    let input = serde_json::json!({
        "player": "alice",
        "candidate": {
            "name": "candidate",
            "cards": cards,
        }
    });
    let resolved: Value = serde_json::from_str(
        &fewfc::web_api::resolve_personal_deck_json(&input.to_string()).unwrap(),
    )
    .unwrap();

    assert_eq!(resolved["source"], "custom");
    assert_eq!(resolved["candidateValidation"]["valid"], true);
    assert_eq!(resolved["effective"]["player"], "alice");
    assert_eq!(resolved["effective"]["cards"].as_array().unwrap().len(), 60);
}

#[test]
fn rule_module_resolution_rejects_incomplete_web_configuration() {
    let invalid = serde_json::json!({ "candidate": ["spirit"] });
    assert!(fewfc::web_api::resolve_rule_modules_json(&invalid.to_string()).is_err());

    let valid = serde_json::json!({
        "candidate": ["star", "five-directions-legend", "hero-schools", "spirit"]
    });
    let resolved: Value = serde_json::from_str(
        &fewfc::web_api::resolve_rule_modules_json(&valid.to_string()).unwrap(),
    )
    .unwrap();
    assert_eq!(resolved["modules"], valid["candidate"]);
}
