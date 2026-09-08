use fewfc::application::GameRecord;
use fewfc::domain::{
    GameError, GameSetup, GameState, Player, PlayerId, RuleModuleId, RulesetId, TeamId,
    ValidationError,
};
use fewfc::rules::{OfficialRuleModuleCategory, OfficialRules};
mod support;
use support::{CardSelector, ScenarioPlan, ScenarioStep};

fn players() -> (Vec<Player>, Vec<PlayerId>) {
    (
        vec![
            Player {
                id: PlayerId::new("p1"),
                team: TeamId::new("team:p1"),
            },
            Player {
                id: PlayerId::new("p2"),
                team: TeamId::new("team:p2"),
            },
        ],
        vec![PlayerId::new("p1"), PlayerId::new("p2")],
    )
}

#[test]
fn game_state_projects_ruleset_and_enabled_modules_from_setup() {
    let module = RuleModuleId::new("future-module");
    let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 20)
        .with_rule_modules(vec![module.clone()]);

    let state = GameState::from_setup(&setup);

    assert_eq!(state.ruleset, RulesetId::base());
    assert_eq!(state.enabled_rule_modules, vec![module]);
}

#[test]
fn official_rules_reject_an_unknown_rule_module_before_starting() {
    let module = RuleModuleId::new("unknown");
    let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 20)
        .with_rule_modules(vec![module.clone()]);

    assert_eq!(
        GameRecord::start(setup, Vec::new()),
        Err(GameError::Validation(ValidationError::UnknownRuleModule(
            module
        )))
    );
}

#[test]
fn official_rules_reject_duplicate_rule_modules_before_starting() {
    let module = RuleModuleId::new("duplicate");
    let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 20)
        .with_rule_modules(vec![module.clone(), module.clone()]);

    assert_eq!(
        GameRecord::start(setup, Vec::new()),
        Err(GameError::Validation(ValidationError::DuplicateRuleModule(
            module
        )))
    );
}

#[test]
fn official_rules_reject_an_unsupported_ruleset_before_starting() {
    let mut setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 20);
    let unsupported = RulesetId::new("alternate-base");
    setup.ruleset = unsupported.clone();

    assert_eq!(
        GameRecord::start(setup, Vec::new()),
        Err(GameError::Validation(ValidationError::UnsupportedRuleset(
            unsupported
        )))
    );
}

#[test]
fn base_only_game_runs_through_the_official_rules_interface() {
    let mut scenario = ScenarioPlan::two_player(&[])
        .start()
        .expect("base game should start");
    scenario
        .step(ScenarioStep::Advance)
        .expect("base game should advance to a decision");
    let current_player = scenario
        .current_player()
        .expect("scenario has a current player")
        .clone();
    let selected_card = scenario
        .select_card(
            CardSelector::in_hand(current_player.clone(), fewfc::domain::Element::Metal, 1)
                .occurrence(0),
        )
        .expect("first official hand contains a metal level-one card");

    assert_eq!(scenario.state().ruleset, RulesetId::base());
    assert!(scenario.state().enabled_rule_modules.is_empty());
    assert!(
        scenario
            .playable_actions(&current_player, &[selected_card])
            .is_ok()
    );
    scenario
        .assert_replay_evidence()
        .expect("scenario should replay");
}

#[test]
fn official_setup_uses_rulebook_hp_for_base_and_advanced_games() {
    let rules = OfficialRules::new();
    let (base_players, base_turn_order) = players();
    let base = rules
        .configure_game(base_players, base_turn_order, Vec::new())
        .unwrap();
    assert!(base.hp.iter().all(|team_hp| team_hp.hp == 100));

    let (advanced_players, advanced_turn_order) = players();
    let advanced = rules
        .configure_game(
            advanced_players,
            advanced_turn_order,
            vec![RuleModuleId::new("five-directions-legend")],
        )
        .unwrap();
    assert!(advanced.hp.iter().all(|team_hp| team_hp.hp == 200));
}

#[test]
fn official_rule_module_catalog_is_the_authoritative_configuration_contract() {
    let catalog = OfficialRules::new().rule_module_catalog();
    assert_eq!(
        catalog
            .iter()
            .map(|module| (
                module.id.as_str(),
                module.category,
                module.default_enabled,
                module
                    .dependencies
                    .iter()
                    .map(|dependency| dependency.as_str())
                    .collect::<Vec<_>>(),
            ))
            .collect::<Vec<_>>(),
        vec![
            (
                "totem-formation",
                OfficialRuleModuleCategory::Theme,
                true,
                vec!["star", "five-directions-legend", "hero-schools"],
            ),
            (
                "discard-retrieval",
                OfficialRuleModuleCategory::Optional,
                true,
                vec![]
            ),
            (
                "personal-deck",
                OfficialRuleModuleCategory::Optional,
                true,
                vec![]
            ),
            (
                "five-directions-legend",
                OfficialRuleModuleCategory::Advanced,
                true,
                vec![]
            ),
            ("star", OfficialRuleModuleCategory::Advanced, true, vec![]),
            (
                "hero-schools",
                OfficialRuleModuleCategory::Advanced,
                true,
                vec![]
            ),
            (
                "spirit",
                OfficialRuleModuleCategory::Theme,
                true,
                vec!["star", "five-directions-legend", "hero-schools"],
            ),
            (
                "jianghu",
                OfficialRuleModuleCategory::Theme,
                true,
                vec!["star", "five-directions-legend", "hero-schools"],
            ),
            (
                "confluence-generation",
                OfficialRuleModuleCategory::Theme,
                true,
                vec!["star", "five-directions-legend", "hero-schools"],
            ),
            (
                "dark-glimmer",
                OfficialRuleModuleCategory::Theme,
                true,
                vec!["spirit"]
            ),
            (
                "echo",
                OfficialRuleModuleCategory::Theme,
                true,
                vec!["star", "five-directions-legend", "hero-schools"],
            ),
            (
                "tribulation",
                OfficialRuleModuleCategory::Theme,
                true,
                vec!["star", "five-directions-legend", "hero-schools"],
            ),
            (
                "pouch",
                OfficialRuleModuleCategory::Theme,
                true,
                vec!["personal-deck", "spirit"],
            ),
        ]
    );
}
