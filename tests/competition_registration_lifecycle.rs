use fewfc::competition_match::MatchFormat;
use fewfc::competition_clock::ClockConfig;
use fewfc::competition_registration::*;
use fewfc::domain::{Player, PlayerId, RuleModuleId, RuleVersion, RulesetId, TeamId};
use fewfc::rules::OfficialRules;
use fewfc::tournament::{
    AnnouncedStageQualification, AnnouncedTournamentStage, CompetitionAnnouncementPlan,
    CompetitionRankingPolicy, ExactRankingTiePolicy, InitialSeedPolicy, StageId,
    SwissPairingPolicy, TournamentFormat,
};

fn announcement_plan() -> CompetitionAnnouncementPlan {
    CompetitionAnnouncementPlan {
        initial_seed_policy: InitialSeedPolicy::RandomDrawAfterRegistrationDeadline,
        swiss_pairing_policy: SwissPairingPolicy::PublishedRulesWithPersistedRandomSeed,
        ranking_policy: CompetitionRankingPolicy::MatchPointsThenOpponentWinsThenOpponentLosses,
        exact_ranking_tie_policy: ExactRankingTiePolicy::OrganizerOrUnconflictedRefereeWithPublicReason,
        stages: vec![AnnouncedTournamentStage {
            stage_id: StageId::from("stage-1"),
            format: TournamentFormat::RoundRobin,
            match_format: MatchFormat::FixedGames(1),
            clock_config: ClockConfig {
                main_time_ms: Some(300_000),
                action_time_ms: Some(30_000),
                out_of_turn_choice_time_ms: Some(30_000),
            },
            swiss_rounds: None,
            swiss_policy: None,
            qualification: AnnouncedStageQualification::AllEligibleTeams,
        }],
    }
}

#[test]
fn e6a3a_settings_core_uses_actual_team_seating_and_official_hp() {
    for version in [RuleVersion::V5_16, RuleVersion::V5_17] {
        for team_size in [1_u16, 2, 3] {
            for modules in [
                vec![],
                vec![RuleModuleId::new("star")],
                vec![RuleModuleId::new("personal-deck")],
            ] {
                let mut players = Vec::new();
                let mut order = Vec::new();
                for index in 0..team_size {
                    for team in ["a", "b"] {
                        let id = PlayerId::new(format!("{team}-{index}"));
                        players.push(Player {
                            id: id.clone(),
                            team: TeamId::new(team),
                        });
                        order.push(id);
                    }
                }
                let setup = OfficialRules::new()
                    .configure_versioned_game_with_decks(
                        version,
                        players,
                        order,
                        modules.clone(),
                        Vec::new(),
                    )
                    .unwrap();
                let settings = CompetitionRulesSettings {
                    rule_version: version,
                    ruleset: RulesetId::base(),
                    enabled_rule_modules: modules,
                    team_size,
                };
                let before = settings.clone();
                let result = resolve_competition_settings(&settings).unwrap();
                assert!(result.initial_hp > 0);
                assert_eq!(setup.players.len(), usize::from(team_size) * 2);
                assert!(setup.hp.iter().all(|hp| hp.hp == result.initial_hp));
                assert_eq!(settings, before);
                assert_eq!(resolve_competition_settings(&settings).unwrap(), result);
            }
        }
    }
}

#[test]
fn e6a3a_settings_reuses_rules_errors_and_keeps_registration_unchanged() {
    let mut registered = state();
    register(&mut registered);
    let public = registered.public_view();
    let private = registered.private_view(&player("captain"));
    let base = CompetitionRulesSettings {
        rule_version: RuleVersion::V5_17,
        ruleset: RulesetId::base(),
        enabled_rule_modules: vec![],
        team_size: 2,
    };
    assert!(resolve_competition_settings(&base).is_ok());
    assert_eq!(registered.public_view(), public);
    assert_eq!(registered.private_view(&player("captain")), private);
    // 完整公告仍是既有可信契約，本 foundation 不改其生命或報名歷史。
    assert_eq!(registered.public_view().announcement.initial_hp, 30);
    for invalid in [
        CompetitionRulesSettings {
            team_size: 0,
            ..base.clone()
        },
        CompetitionRulesSettings {
            ruleset: RulesetId::new("fewfc-base"),
            ..base.clone()
        },
        CompetitionRulesSettings {
            enabled_rule_modules: vec![RuleModuleId::new("spirit")],
            ..base.clone()
        },
        CompetitionRulesSettings {
            enabled_rule_modules: vec![RuleModuleId::new("star"), RuleModuleId::new("star")],
            ..base.clone()
        },
        CompetitionRulesSettings {
            rule_version: RuleVersion::V5_16,
            enabled_rule_modules: vec![RuleModuleId::new("totem-formation")],
            ..base.clone()
        },
        // 非法模組在座次配置前拒絕，不對巨大有效 u16 建立牌組或自創人數上限。
        CompetitionRulesSettings {
            team_size: u16::MAX,
            enabled_rule_modules: vec![RuleModuleId::new("unknown")],
            ..base.clone()
        },
    ] {
        assert!(matches!(
            resolve_competition_settings(&invalid),
            Err(RegistrationError::RulesValidation { .. })
        ));
        assert_eq!(registered.public_view(), public);
        assert_eq!(registered.private_view(&player("captain")), private);
    }
}

#[test]
fn e6a3a_settings_dto_is_exact_camel_case_without_caller_hp() {
    let wire = serde_json::json!({
        "ruleVersion": "5.17", "ruleset": "base", "enabledRuleModules": ["star"], "teamSize": 2,
    });
    let settings: CompetitionRulesSettings = serde_json::from_value(wire.clone()).unwrap();
    assert_eq!(serde_json::to_value(&settings).unwrap(), wire);
    let resolved = resolve_competition_settings(&settings).unwrap();
    let value = serde_json::to_value(&resolved).unwrap();
    assert_eq!(value, serde_json::json!({"initialHp": resolved.initial_hp}));
    assert_eq!(value.as_object().unwrap().len(), 1);
    for field in [
        "initialHp",
        "initial_hp",
        "players",
        "private",
        "announcement",
        "team_size",
    ] {
        let mut extra = wire.clone();
        extra[field] = serde_json::json!(1);
        assert!(serde_json::from_value::<CompetitionRulesSettings>(extra).is_err());
    }
    for size in [
        serde_json::json!(65536_u64),
        serde_json::json!(u64::MAX),
        serde_json::json!(-1),
        serde_json::json!(1.5),
        serde_json::json!("2"),
    ] {
        let mut invalid = wire.clone();
        invalid["teamSize"] = size;
        assert!(serde_json::from_value::<CompetitionRulesSettings>(invalid).is_err());
    }
}

fn player(id: &str) -> RegistrationPlayerId {
    RegistrationPlayerId::new(id)
}
fn team() -> RegistrationTeamId {
    RegistrationTeamId::new("team-a")
}

fn state() -> RegistrationState {
    let mut state = RegistrationState::new(CompetitionAnnouncement {
        organizer: player("organizer"),
        rule_version: RuleVersion::default(),
        ruleset: RulesetId::new("base"),
        enabled_rule_modules: vec![],
        team_size: 1,
        initial_hp: 30,
        registration_mode: RegistrationMode::Public,
        personal_decks: PersonalDeckPolicy::Disabled,
        registration_deadline: TrustedTimestamp::from_millis(60_000),
        check_in_deadline: TrustedTimestamp::from_millis(120_000),
        plan: announcement_plan(),
    })
    .unwrap();
    apply(
        &mut state,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Open,
        0,
    )
    .unwrap();
    state
}

fn apply(
    state: &mut RegistrationState,
    actor: RegistrationActor,
    action: RegistrationAction,
    now: u64,
) -> Result<RegistrationCommandResult, RegistrationError> {
    state.execute(
        RegistrationCommand {
            command_id: format!("cmd-{}", state.revision()),
            expected_revision: state.revision(),
            actor,
            action,
        },
        TrustedTimestamp::from_millis(now),
    )
}

fn register(state: &mut RegistrationState) {
    apply(
        state,
        RegistrationActor::Player(player("captain")),
        RegistrationAction::RegisterTeam {
            team_id: team(),
            captain: player("captain"),
            members: vec![player("captain")],
        },
        1,
    )
    .unwrap();
}

#[test]
fn untrusted_command_time_cannot_reopen_registration() {
    let mut state = state();
    let before = serde_json::to_value(state.public_view()).unwrap();
    assert!(
        apply(
            &mut state,
            RegistrationActor::Player(player("captain")),
            RegistrationAction::RegisterTeam {
                team_id: team(),
                captain: player("captain"),
                members: vec![player("captain")],
            },
            60_000
        )
        .is_err()
    );
    assert_eq!(serde_json::to_value(state.public_view()).unwrap(), before);
}

#[test]
fn another_organizer_cannot_withdraw_a_registered_team() {
    let mut state = state();
    register(&mut state);
    let before = serde_json::to_value(state.public_view()).unwrap();
    assert!(
        apply(
            &mut state,
            RegistrationActor::Organizer(player("impostor")),
            RegistrationAction::WithdrawTeam { team_id: team() },
            2
        )
        .is_err()
    );
    assert_eq!(serde_json::to_value(state.public_view()).unwrap(), before);
}

#[test]
fn confirmed_checked_in_team_is_exported_and_cannot_change_after_lock() {
    let mut state = state();
    register(&mut state);
    apply(
        &mut state,
        RegistrationActor::Player(player("captain")),
        RegistrationAction::ConfirmMembership {
            team_id: team(),
            roster_version: 1,
        },
        2,
    )
    .unwrap();
    apply(
        &mut state,
        RegistrationActor::Player(player("captain")),
        RegistrationAction::CheckInTeam {
            team_id: team(),
            roster_version: 1,
        },
        3,
    )
    .unwrap();
    apply(
        &mut state,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Lock {},
        60_000,
    )
    .unwrap();
    assert_eq!(state.export_locked_roster().unwrap().teams.len(), 1);
    let before = serde_json::to_value(state.public_view()).unwrap();
    assert!(
        apply(
            &mut state,
            RegistrationActor::Player(player("captain")),
            RegistrationAction::ReplaceRoster {
                team_id: team(),
                expected_roster_version: 1,
                members: vec![player("captain")],
            },
            201
        )
        .is_err()
    );
    assert_eq!(serde_json::to_value(state.public_view()).unwrap(), before);
}

#[test]
fn locked_teams_feed_schedule_and_clock_without_rewriting_game_draw() {
    use fewfc::competition_clock::{
        ClockConfig, CompetitionClock, CompetitionRuling, GameResultMode,
    };
    use fewfc::domain::{GameConclusion, GameEndCause, GameOutcome, TeamId};
    use fewfc::tournament as schedule;

    let mut state = state();
    for id in ["a", "b"] {
        let team_id = RegistrationTeamId::new(id);
        let actor = RegistrationActor::Player(player(id));
        apply(
            &mut state,
            actor.clone(),
            RegistrationAction::RegisterTeam {
                team_id: team_id.clone(),
                captain: player(id),
                members: vec![player(id)],
            },
            1,
        )
        .unwrap();
        apply(
            &mut state,
            actor.clone(),
            RegistrationAction::ConfirmMembership {
                team_id: team_id.clone(),
                roster_version: 1,
            },
            1,
        )
        .unwrap();
        apply(
            &mut state,
            actor,
            RegistrationAction::CheckInTeam {
                team_id,
                roster_version: 1,
            },
            1,
        )
        .unwrap();
    }
    apply(
        &mut state,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Lock {},
        60_000,
    )
    .unwrap();
    let locked = state.export_locked_roster().unwrap();
    let stage_id = schedule::StageId("final".into());
    let mut tournament = schedule::Tournament::new(schedule::TournamentConfig {
        tournament_id: "event".into(),
        stage: schedule::StageConfig {
            stage_id: stage_id.clone(),
            format: schedule::TournamentFormat::SingleElimination,
            teams: locked
                .teams
                .iter()
                .enumerate()
                .map(|(index, team)| schedule::SeededTeam {
                    team_id: team.team_id.as_str().into(),
                    seed: index as u32 + 1,
                })
                .collect(),
            random_seed: 1,
            match_format: MatchFormat::FirstToWins(1),
            clock_config: ClockConfig::default(),
            swiss_rounds: None,
            swiss_policy: None,
        },
    })
    .unwrap();
    let first = TeamId::new(locked.teams[0].team_id.as_str());
    let second = TeamId::new(locked.teams[1].team_id.as_str());
    let mut clock =
        CompetitionClock::new([first.clone(), second.clone()], ClockConfig::default(), 0).unwrap();
    clock.begin_action(&first, 1, 0).unwrap();
    clock.end_window(300_001).unwrap();
    let original = GameConclusion::new(
        GameOutcome::Draw,
        vec![GameEndCause::TeamHpDepleted {
            teams: vec![first.clone(), second.clone()],
        }],
    );
    let original_json = serde_json::to_value(&original).unwrap();
    let ruling = clock
        .rule_game(&original, GameResultMode::Elimination)
        .unwrap();
    assert!(
        matches!(ruling, CompetitionRuling::EliminationDrawLoss { loser, .. } if loser == first)
    );
    let match_id = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0]
        .match_id
        .clone();
    tournament
        .submit_result(
            &stage_id,
            &match_id,
            schedule::MatchResult::win(second.as_str(), first.as_str()),
        )
        .unwrap();
    assert_eq!(
        tournament.stage(&stage_id).unwrap().winner(),
        Some(second.as_str().into())
    );
    assert_eq!(serde_json::to_value(original).unwrap(), original_json);
}

fn opening_registration_state(
    team_size: u16,
    personal_decks: PersonalDeckPolicy,
) -> RegistrationState {
    let mut state = RegistrationState::new(CompetitionAnnouncement {
        organizer: player("organizer"),
        rule_version: RuleVersion::default(),
        ruleset: RulesetId::base(),
        enabled_rule_modules: if personal_decks == PersonalDeckPolicy::Required {
            vec![RuleModuleId::new("personal-deck")]
        } else {
            Vec::new()
        },
        team_size,
        initial_hp: 30,
        registration_mode: RegistrationMode::Public,
        personal_decks,
        registration_deadline: TrustedTimestamp::from_millis(60_000),
        check_in_deadline: TrustedTimestamp::from_millis(120_000),
        plan: announcement_plan(),
    })
    .unwrap();
    apply(
        &mut state,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Open,
        0,
    )
    .unwrap();
    state
}

fn register_opening_team(state: &mut RegistrationState, members: &[&str]) {
    apply(
        state,
        RegistrationActor::Player(player("captain")),
        RegistrationAction::RegisterTeam {
            team_id: team(),
            captain: player("captain"),
            members: members.iter().map(|id| player(id)).collect(),
        },
        1,
    )
    .unwrap();
}

fn confirm_opening_members(state: &mut RegistrationState, members: &[&str]) {
    for id in members {
        apply(
            state,
            RegistrationActor::Player(player(id)),
            RegistrationAction::ConfirmMembership {
                team_id: team(),
                roster_version: 1,
            },
            2,
        )
        .unwrap();
    }
}

fn opening_deck_action(id: &str) -> RegistrationAction {
    let deck = OfficialRules::new().preconstructed_deck(PlayerId::new(id));
    RegistrationAction::SubmitDeck {
        team_id: team(),
        roster_version: 1,
        name: deck.name,
        cards: deck.cards.iter().map(|card| card.as_str().to_string()).collect(),
    }
}

fn submit_opening_decks(state: &mut RegistrationState, members: &[&str]) {
    for id in members {
        apply(
            state,
            RegistrationActor::Player(player(id)),
            opening_deck_action(id),
            3,
        )
        .unwrap();
    }
}

#[test]
fn opening_freeze_all_data_actions_share_exclusive_registration_deadline() {
    let cases = [
        (
            RegistrationActor::Player(player("other-captain")),
            RegistrationAction::RegisterTeam {
                team_id: RegistrationTeamId::new("other-team"),
                captain: player("other-captain"),
                members: vec![player("other-captain")],
            },
        ),
        (
            RegistrationActor::Player(player("captain")),
            RegistrationAction::ReplaceRoster {
                team_id: team(),
                expected_roster_version: 1,
                members: vec![player("captain")],
            },
        ),
        (
            RegistrationActor::Player(player("captain")),
            RegistrationAction::WithdrawTeam { team_id: team() },
        ),
        (
            RegistrationActor::Player(player("captain")),
            RegistrationAction::ConfirmMembership {
                team_id: team(),
                roster_version: 1,
            },
        ),
        (
            RegistrationActor::Player(player("captain")),
            opening_deck_action("captain"),
        ),
        (
            RegistrationActor::Player(player("captain")),
            RegistrationAction::CheckInTeam {
                team_id: team(),
                roster_version: 1,
            },
        ),
    ];
    for (actor, action) in cases {
        for time in [59_999, 60_000, 60_001] {
            let mut state = opening_registration_state(1, PersonalDeckPolicy::Required);
            register_opening_team(&mut state, &["captain"]);
            if matches!(&action, RegistrationAction::CheckInTeam { .. }) {
                confirm_opening_members(&mut state, &["captain"]);
                submit_opening_decks(&mut state, &["captain"]);
            }
            let before = state.public_view();
            let private = state.private_view(&player("captain"));
            let result = apply(&mut state, actor.clone(), action.clone(), time);
            if time == 59_999 {
                assert_eq!(result.unwrap().revision, before.revision + 1);
            } else {
                assert!(matches!(result, Err(RegistrationError::InvalidTimestamp { .. })));
                assert_eq!(state.public_view(), before);
                assert_eq!(state.private_view(&player("captain")), private);
            }
            assert_eq!(state.lifecycle(), RegistrationLifecycle::Open);
        }
    }
}

fn complete_opening_team(
    state: &mut RegistrationState,
    members: &[&str],
    personal_decks: PersonalDeckPolicy,
) {
    confirm_opening_members(state, members);
    if personal_decks == PersonalDeckPolicy::Required {
        submit_opening_decks(state, members);
    }
    apply(
        state,
        RegistrationActor::Player(player("captain")),
        RegistrationAction::CheckInTeam {
            team_id: team(),
            roster_version: 1,
        },
        4,
    )
    .unwrap();
}

#[test]
fn opening_freeze_lock_keeps_deadline_authority_and_failure_atomicity() {
    let mut state = opening_registration_state(2, PersonalDeckPolicy::Required);
    register_opening_team(&mut state, &["captain", "member"]);
    complete_opening_team(&mut state, &["captain", "member"], PersonalDeckPolicy::Required);
    let before = state.public_view();
    let private = state.private_view(&player("captain"));
    let early = apply(
        &mut state,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Lock {},
        59_999,
    );
    assert!(matches!(early, Err(RegistrationError::InvalidTimestamp { .. })));
    assert_eq!(state.public_view(), before);
    assert_eq!(state.private_view(&player("captain")), private);
    for actor in [
        RegistrationActor::Organizer(player("impostor")),
        RegistrationActor::Player(player("organizer")),
        RegistrationActor::Player(player("captain")),
    ] {
        assert!(matches!(
            apply(&mut state, actor, RegistrationAction::Lock {}, 60_000),
            Err(RegistrationError::Unauthorized)
        ));
        assert_eq!(state.public_view(), before);
        assert_eq!(state.private_view(&player("captain")), private);
    }
    apply(
        &mut state,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Lock {},
        60_000,
    )
    .unwrap();
    assert_eq!(state.lifecycle(), RegistrationLifecycle::Locked);
    assert_eq!(state.revision(), before.revision + 1);
    assert_eq!(state.export_locked_roster().unwrap().teams.len(), 1);
}

#[test]
fn opening_freeze_late_lock_exports_the_same_fixed_snapshot() {
    let mut before_deadline = opening_registration_state(2, PersonalDeckPolicy::Required);
    register_opening_team(&mut before_deadline, &["captain", "member"]);
    complete_opening_team(
        &mut before_deadline,
        &["captain", "member"],
        PersonalDeckPolicy::Required,
    );
    let mut on_deadline = before_deadline.clone();
    apply(
        &mut on_deadline,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Lock {},
        60_000,
    )
    .unwrap();
    let expected = on_deadline.export_locked_roster().unwrap();
    assert_eq!(expected.announcement.check_in_deadline, TrustedTimestamp::from_millis(120_000));
    assert_eq!(expected.teams[0].members, vec![player("captain"), player("member")]);
    assert_eq!(expected.teams[0].decks.len(), 2);
    for time in [60_050, 60_250] {
        let mut delayed = before_deadline.clone();
        apply(
            &mut delayed,
            RegistrationActor::Organizer(player("organizer")),
            RegistrationAction::Lock {},
            time,
        )
        .unwrap();
        assert_eq!(delayed.export_locked_roster().unwrap(), expected);
        assert_eq!(delayed.public_view(), on_deadline.public_view());
        assert_eq!(delayed.private_view(&player("member")), on_deadline.private_view(&player("member")));
    }
}

#[test]
fn opening_freeze_successful_receipts_survive_deadline_and_lock_without_reapplying() {
    let mut state = opening_registration_state(1, PersonalDeckPolicy::Required);
    let actions = [
        RegistrationAction::RegisterTeam {
            team_id: team(),
            captain: player("captain"),
            members: vec![player("captain")],
        },
        RegistrationAction::ConfirmMembership {
            team_id: team(),
            roster_version: 1,
        },
        opening_deck_action("captain"),
        RegistrationAction::CheckInTeam {
            team_id: team(),
            roster_version: 1,
        },
    ];
    let mut receipts = Vec::new();
    for (time, action) in [1, 2, 3, 4].into_iter().zip(actions) {
        let command = RegistrationCommand {
            command_id: format!("opening-receipt-{time}"),
            expected_revision: state.revision(),
            actor: RegistrationActor::Player(player("captain")),
            action,
        };
        let result = state.execute(command.clone(), TrustedTimestamp::from_millis(time)).unwrap();
        receipts.push((command, result));
    }
    let lock = RegistrationCommand {
        command_id: "opening-lock-receipt".into(),
        expected_revision: state.revision(),
        actor: RegistrationActor::Organizer(player("organizer")),
        action: RegistrationAction::Lock {},
    };
    let locked = state.execute(lock.clone(), TrustedTimestamp::from_millis(60_000)).unwrap();
    receipts.push((lock, locked));
    let before = state.public_view();
    let private = state.private_view(&player("captain"));
    let export = state.export_locked_roster().unwrap();
    for (command, result) in &receipts {
        assert_eq!(
            state.execute(command.clone(), TrustedTimestamp::from_millis(1_000)).unwrap(),
            *result
        );
        assert_eq!(state.public_view(), before);
        assert_eq!(state.private_view(&player("captain")), private);
        assert_eq!(state.export_locked_roster().unwrap(), export);
    }
    let mut conflict = receipts[0].0.clone();
    conflict.action = RegistrationAction::WithdrawTeam { team_id: team() };
    assert!(matches!(
        state.execute(conflict, TrustedTimestamp::from_millis(1_000)),
        Err(RegistrationError::CommandIdConflict { .. })
    ));
    assert_eq!(state.public_view(), before);
    assert_eq!(state.private_view(&player("captain")), private);
}

#[test]
fn opening_freeze_exclusion_reasons_describe_only_missing_frozen_data() {
    use RegistrationExclusionReason::{
        MissingCaptainConfirmation, MissingMemberConfirmations, MissingRequiredDecks,
    };
    let cases = [
        (false, false, false, vec![MissingMemberConfirmations, MissingRequiredDecks, MissingCaptainConfirmation]),
        (false, true, false, vec![MissingMemberConfirmations, MissingCaptainConfirmation]),
        (true, false, false, vec![MissingRequiredDecks, MissingCaptainConfirmation]),
        (true, true, false, vec![MissingCaptainConfirmation]),
        (true, true, true, vec![]),
    ];
    for (confirmed, submitted, captain_complete, reasons) in cases {
        let mut state = opening_registration_state(1, PersonalDeckPolicy::Required);
        register_opening_team(&mut state, &["captain"]);
        if confirmed {
            confirm_opening_members(&mut state, &["captain"]);
        }
        if submitted {
            submit_opening_decks(&mut state, &["captain"]);
        }
        if captain_complete {
            apply(
                &mut state,
                RegistrationActor::Player(player("captain")),
                RegistrationAction::CheckInTeam { team_id: team(), roster_version: 1 },
                4,
            )
            .unwrap();
        }
        assert!(state.public_view().teams[0].exclusion_reasons.is_empty());
        apply(
            &mut state,
            RegistrationActor::Organizer(player("organizer")),
            RegistrationAction::Lock {},
            60_150,
        )
        .unwrap();
        let public = state.public_view();
        assert_eq!(public.teams[0].exclusion_reasons, reasons);
        if captain_complete {
            assert_eq!(public.teams[0].status, TeamStatus::CheckedIn);
            assert_eq!(state.export_locked_roster().unwrap().teams.len(), 1);
        } else {
            assert_eq!(public.teams[0].status, TeamStatus::NotCheckedIn);
            assert!(state.export_locked_roster().unwrap().teams.is_empty());
        }
        assert_eq!(state.private_view(&player("captain")).public_view, public);
    }
}

#[test]
fn opening_freeze_disabled_decks_and_withdrawn_teams_have_no_false_reasons() {
    for withdrawn in [false, true] {
        let mut state = opening_registration_state(1, PersonalDeckPolicy::Disabled);
        register_opening_team(&mut state, &["captain"]);
        confirm_opening_members(&mut state, &["captain"]);
        if withdrawn {
            apply(
                &mut state,
                RegistrationActor::Player(player("captain")),
                RegistrationAction::WithdrawTeam { team_id: team() },
                3,
            )
            .unwrap();
        }
        apply(
            &mut state,
            RegistrationActor::Organizer(player("organizer")),
            RegistrationAction::Lock {},
                60_000,
        )
        .unwrap();
        let public = state.public_view();
        if withdrawn {
            assert_eq!(public.teams[0].status, TeamStatus::Withdrawn);
            assert!(public.teams[0].exclusion_reasons.is_empty());
        } else {
            assert_eq!(public.teams[0].status, TeamStatus::NotCheckedIn);
            assert_eq!(
                public.teams[0].exclusion_reasons,
                vec![RegistrationExclusionReason::MissingCaptainConfirmation]
            );
        }
        assert!(state.export_locked_roster().unwrap().teams.is_empty());
    }
    let mut checked_in = opening_registration_state(1, PersonalDeckPolicy::Disabled);
    register_opening_team(&mut checked_in, &["captain"]);
    complete_opening_team(&mut checked_in, &["captain"], PersonalDeckPolicy::Disabled);
    apply(
        &mut checked_in,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Lock {},
        60_000,
    )
    .unwrap();
    assert!(checked_in.public_view().teams[0].exclusion_reasons.is_empty());
    assert_eq!(checked_in.export_locked_roster().unwrap().teams.len(), 1);
}

#[test]
fn opening_freeze_public_exclusion_reasons_are_exact_camel_case_closed_and_private() {
    let mut state = opening_registration_state(2, PersonalDeckPolicy::Required);
    register_opening_team(&mut state, &["captain", "member"]);
    submit_opening_decks(&mut state, &["captain"]);
    apply(
        &mut state,
        RegistrationActor::Organizer(player("organizer")),
        RegistrationAction::Lock {},
        60_000,
    )
    .unwrap();
    let public = state.public_view();
    let wire = serde_json::to_value(&public.teams[0]).unwrap();
    assert_eq!(wire, serde_json::json!({
        "teamId": "team-a",
        "captain": "captain",
        "members": ["captain", "member"],
        "rosterVersion": 1,
        "confirmedMembers": [],
        "status": "notCheckedIn",
        "exclusionReasons": [
            "missingMemberConfirmations", "missingRequiredDecks", "missingCaptainConfirmation"
        ]
    }));
    assert_eq!(serde_json::from_value::<PublicTeamView>(wire.clone()).unwrap(), public.teams[0]);
    let mut missing = wire;
    missing.as_object_mut().unwrap().remove("exclusionReasons");
    assert!(serde_json::from_value::<PublicTeamView>(missing).is_err());
    for unknown in ["unknown", "missing_required_decks", "MissingCaptainConfirmation"] {
        assert!(serde_json::from_value::<RegistrationExclusionReason>(serde_json::json!(unknown)).is_err());
    }
    let private = state.private_view(&player("captain"));
    assert_eq!(private.public_view, public);
    assert_eq!(private.own_team.unwrap().own_deck.unwrap().cards.len(), 60);
    let outsider = state.private_view(&player("outsider"));
    assert!(outsider.own_team.is_none());
    assert_eq!(outsider.public_view, public);
}
