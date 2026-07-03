use fewfc::domain::{
    CardInstanceId, ECHO_MODULE_ID, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError, GameState,
    HERO_SCHOOLS_MODULE_ID, Phase, PlayerId, RuleModuleId, STAR_MODULE_ID, StarKind, TeamStar,
    ValidationError,
};
use fewfc::rules::{OfficialRules, PlayableAction};

fn dependencies() -> Vec<RuleModuleId> {
    [
        STAR_MODULE_ID,
        FIVE_DIRECTIONS_LEGEND_MODULE_ID,
        HERO_SCHOOLS_MODULE_ID,
    ]
    .into_iter()
    .map(RuleModuleId::new)
    .collect()
}

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

fn configured_state() -> GameState {
    let rules = OfficialRules::new();
    let mut modules = dependencies();
    modules.push(RuleModuleId::new(ECHO_MODULE_ID));
    let setup = rules
        .configure_game(
            fewfc::domain::GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30)
                .players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            modules,
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::Main;
    state
}

fn formation_ids(actions: Vec<PlayableAction>) -> Vec<String> {
    actions
        .into_iter()
        .filter_map(|action| match action {
            PlayableAction::PerformFormation(candidate) => Some(candidate.formation_id),
            _ => None,
        })
        .collect()
}

#[test]
fn echo_is_stable_non_default_and_requires_all_advanced_modules() {
    let rules = OfficialRules::new();
    assert!(
        !rules
            .default_rule_modules()
            .iter()
            .any(|module| module.as_str() == ECHO_MODULE_ID)
    );

    let players =
        fewfc::domain::GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).players;
    assert!(matches!(
        rules.configure_game(
            players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            vec![RuleModuleId::new(ECHO_MODULE_ID)],
        ),
        Err(GameError::Validation(
            ValidationError::MissingRuleModuleDependencies { .. }
        ))
    ));
}

#[test]
fn melody_and_base_formation_remain_explicit_overlapping_declarations() {
    let mut state = configured_state();
    state.hands[0].cards = vec![card(1), card(2)];

    let ids = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(1), card(2)])
            .unwrap(),
    );
    assert!(ids.contains(&"weapon".to_string()));
    assert!(ids.contains(&"echo:ringing-metal".to_string()));
}

#[test]
fn star_substitution_does_not_make_a_melody_legal() {
    let mut state = configured_state();
    state.hands[0].cards = vec![card(1), card(73)];
    state.team_stars.push(TeamStar {
        team: state.players[0].team.clone(),
        star: StarKind::Metal,
    });

    let ids = formation_ids(
        OfficialRules::new()
            .playable_actions(&state, &PlayerId::new("p1"), &[card(1), card(73)])
            .unwrap(),
    );
    assert!(ids.contains(&"weapon".to_string()));
    assert!(!ids.contains(&"echo:ringing-metal".to_string()));
}
