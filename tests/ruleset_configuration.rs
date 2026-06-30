use fewfc::application::GameRecord;
use fewfc::domain::{
    GameError, GameSetup, GameState, Player, PlayerId, RuleModuleId, RulesetId, TeamId,
    ValidationError,
};
use fewfc::rules::OfficialRules;

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
    let rules = OfficialRules::new();
    let (players, turn_order) = players();
    let setup = rules
        .configure_game(players, turn_order, Vec::new())
        .expect("base setup should be supported");
    let deck_order = rules
        .official_deck_order(&setup)
        .expect("base deck should be available");
    let mut record = GameRecord::start(setup, deck_order).expect("base game should start");

    record
        .advance_until_decision()
        .expect("base game should advance to a decision");

    let current_player = record
        .state()
        .current_player()
        .expect("turn order should have a current player")
        .clone();
    let selected_card = record
        .state()
        .hand(&current_player)
        .and_then(|hand| hand.first())
        .copied()
        .expect("initial deal should give the current player a card");

    assert_eq!(record.state().ruleset, RulesetId::base());
    assert!(record.state().enabled_rule_modules.is_empty());
    assert!(
        record
            .playable_formations(&current_player, &[selected_card])
            .is_ok()
    );
    assert_eq!(
        record.replay().expect("record should replay"),
        record.state().clone()
    );
}
