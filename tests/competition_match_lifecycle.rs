use fewfc::competition_clock::{
    ClockConfig, CompetitionClock, CompetitionRuling, EliminationDrawLossReason, GameResultMode,
    NeedsDecisionReason, NewGameTime, RulingOutcome,
};
use fewfc::competition_match::{
    CompetitionMatch, CompetitionMatchConfig, CompetitionRulingRecord, DoubleLossProgression,
    FirstPlayerIntent, GameStartKind, GameTerminalEvidence, MatchError, MatchFormat, MatchOutcome,
    MatchStatus, RefereeAssignment, RefereeId, ResultSubmissionId, RulingRecordId, TimeoutExpiry,
    TrustedGameResult,
};
use fewfc::domain::{GameConclusion, GameEndCause, GameOutcome, TeamId};

fn team(id: &str) -> TeamId {
    TeamId::new(id)
}

fn assigned(team_id: &str) -> FirstPlayerIntent {
    FirstPlayerIntent::Assigned {
        team: team(team_id),
    }
}

fn draw_conclusion() -> GameConclusion {
    GameConclusion::new(
        GameOutcome::Draw,
        vec![GameEndCause::TeamHpDepleted {
            teams: vec![team("a"), team("b")],
        }],
    )
}

fn winner_conclusion(winner: &str) -> GameConclusion {
    GameConclusion::new(
        GameOutcome::Winner(team(winner)),
        vec![GameEndCause::DirectVictory {
            rule: "test".to_string(),
            team: team(winner),
        }],
    )
}

fn config(format: MatchFormat, first_game: FirstPlayerIntent) -> CompetitionMatchConfig {
    CompetitionMatchConfig {
        match_id: "match-1".into(),
        teams: [team("a"), team("b")],
        format,
        first_game,
        referees: vec![RefereeAssignment {
            referee_id: "neutral-referee".into(),
            conflicted_team: None,
        }],
    }
}

fn submit(
    match_: &mut CompetitionMatch,
    submission_id: &str,
    game_id: &str,
    ruling_id: &str,
    start_kind: GameStartKind,
    first_player: &str,
    conclusion: GameConclusion,
    ruling: CompetitionRuling,
) {
    submit_evidence(
        match_,
        submission_id,
        game_id,
        ruling_id,
        start_kind,
        first_player,
        GameTerminalEvidence::CanonicalGameEnded { conclusion },
        ruling,
    );
}

fn submit_evidence(
    match_: &mut CompetitionMatch,
    submission_id: &str,
    game_id: &str,
    ruling_id: &str,
    start_kind: GameStartKind,
    first_player: &str,
    terminal_evidence: GameTerminalEvidence,
    ruling: CompetitionRuling,
) {
    match_
        .submit_game(TrustedGameResult {
            submission_id: ResultSubmissionId::from(submission_id),
            game_record_id: game_id.into(),
            ruling_record_id: RulingRecordId::from(ruling_id),
            start_kind,
            first_player: team(first_player),
            terminal_evidence,
            ruling,
        })
        .unwrap();
}

fn ordinary() -> GameStartKind {
    GameStartKind::OrdinaryNewGame
}

fn game_accepted_winner(winner: &str) -> CompetitionRuling {
    CompetitionRuling::GameAccepted {
        outcome: RulingOutcome::Winner(team(winner)),
    }
}

fn timeout_evidence(
    timeout_id: &str,
    expiries: Vec<(&str, u64)>,
    observed_at_monotonic_ms: u64,
) -> GameTerminalEvidence {
    GameTerminalEvidence::Timeout {
        timeout_id: timeout_id.to_string(),
        expiries: expiries
            .into_iter()
            .map(|(team_id, deadline_monotonic_ms)| TimeoutExpiry {
                team: team(team_id),
                deadline_monotonic_ms,
            })
            .collect(),
        observed_at_monotonic_ms,
        clock_checkpoint_digest: "digest-1".to_string(),
    }
}

#[test]
fn fixed_game_series_keeps_raw_conclusion_and_separate_ruling_record() {
    let mut match_ =
        CompetitionMatch::new(config(MatchFormat::FixedGames(3), assigned("a"))).unwrap();
    assert_eq!(ordinary().required_clock_time(), NewGameTime::Reset);
    assert_eq!(
        MatchFormat::FixedGames(3).game_result_mode(),
        GameResultMode::RetainDraw
    );

    submit(
        &mut match_,
        "submission-1",
        "game-1",
        "ruling-1",
        ordinary(),
        "a",
        draw_conclusion(),
        CompetitionRuling::GameAccepted {
            outcome: RulingOutcome::Draw,
        },
    );
    assert_eq!(match_.status(), MatchStatus::Open);
    assert!(matches!(
        match_.next_game(),
        Some(intent)
            if matches!(&intent.first_player, FirstPlayerIntent::Assigned { team } if team.as_str() == "b")
    ));
    match_
        .assign_next_first_player(team("b"), "公告指定下一局先手")
        .unwrap();
    submit(
        &mut match_,
        "submission-2",
        "game-2",
        "ruling-2",
        ordinary(),
        "b",
        winner_conclusion("a"),
        game_accepted_winner("a"),
    );
    match_
        .assign_next_first_player(team("a"), "公告指定下一局先手")
        .unwrap();
    submit(
        &mut match_,
        "submission-3",
        "game-3",
        "ruling-3",
        ordinary(),
        "a",
        winner_conclusion("a"),
        game_accepted_winner("a"),
    );

    assert_eq!(match_.status(), MatchStatus::Completed);
    assert_eq!(match_.score(), [2, 0]);
    assert!(matches!(
        match_.outcome(),
        Some(MatchOutcome::Winner { winner_id, loser_id, score })
            if winner_id == &team("a") && loser_id == &team("b") && *score == [2, 0]
    ));
    assert_eq!(match_.games().len(), 3);
    assert_eq!(match_.rulings().len(), 3);
    assert_eq!(
        match_.games()[0].ruling_record_id,
        RulingRecordId::from("ruling-1")
    );
    assert!(matches!(
        &match_.games()[0].terminal_evidence,
        GameTerminalEvidence::CanonicalGameEnded { conclusion }
            if conclusion.outcome == GameOutcome::Draw
    ));
    let wire = serde_json::to_value(&match_).unwrap();
    assert!(wire["games"][0]["terminalEvidence"].is_object());
    assert_eq!(wire["games"][0]["rulingRecordId"], "ruling-1");
    assert_eq!(wire["rulings"][0]["rulingRecordId"], "ruling-1");
    assert!(matches!(
        match_.ruling_for_game(&"game-1".into()),
        Some(CompetitionRulingRecord {
            ruling,
            game_record_id,
            ..
        }) if game_record_id == &"game-1".into()
            && matches!(ruling, CompetitionRuling::GameAccepted { outcome: RulingOutcome::Draw })
    ));
}

#[test]
fn fixed_games_finish_using_the_number_of_games_already_replayed() {
    let mut match_ =
        CompetitionMatch::new(config(MatchFormat::FixedGames(2), assigned("a"))).unwrap();
    submit(
        &mut match_,
        "submission-1",
        "game-1",
        "ruling-1",
        ordinary(),
        "a",
        winner_conclusion("a"),
        game_accepted_winner("a"),
    );
    match_
        .assign_next_first_player(team("b"), "公告指定下一局先手")
        .unwrap();
    submit(
        &mut match_,
        "submission-2",
        "game-2",
        "ruling-2",
        ordinary(),
        "b",
        winner_conclusion("a"),
        game_accepted_winner("a"),
    );

    assert_eq!(match_.status(), MatchStatus::Completed);
    assert!(matches!(
        match_.outcome(),
        Some(MatchOutcome::Winner { winner_id, score, .. })
            if winner_id == &team("a") && *score == [2, 0]
    ));
}

#[test]
fn fixed_games_last_draw_uses_the_accumulated_score() {
    let mut match_ =
        CompetitionMatch::new(config(MatchFormat::FixedGames(2), assigned("a"))).unwrap();
    submit(
        &mut match_,
        "submission-1",
        "game-1",
        "ruling-1",
        ordinary(),
        "a",
        winner_conclusion("a"),
        game_accepted_winner("a"),
    );
    match_
        .assign_next_first_player(team("b"), "公告指定下一局先手")
        .unwrap();
    submit(
        &mut match_,
        "submission-2",
        "game-2",
        "ruling-2",
        ordinary(),
        "b",
        draw_conclusion(),
        CompetitionRuling::GameAccepted {
            outcome: RulingOutcome::Draw,
        },
    );

    assert_eq!(match_.status(), MatchStatus::Completed);
    assert!(matches!(
        match_.outcome(),
        Some(MatchOutcome::Winner { winner_id, score, .. })
            if winner_id == &team("a") && *score == [1, 0]
    ));
}

#[test]
fn assigned_first_player_cannot_be_overwritten_but_same_value_retry_is_idempotent() {
    let mut match_ =
        CompetitionMatch::new(config(MatchFormat::FixedGames(1), assigned("a"))).unwrap();
    match_
        .assign_next_first_player(team("a"), "相同先手的重試")
        .unwrap();
    let error = match_.assign_next_first_player(team("b"), "不可覆寫初局先手");
    assert!(matches!(
        error,
        Err(MatchError::FirstPlayerAlreadyAssigned { expected, received })
            if expected == team("a") && received == team("b")
    ));
    assert!(matches!(
        match_.next_game(),
        Some(intent)
            if matches!(&intent.first_player, FirstPlayerIntent::Assigned { team } if team.as_str() == "a")
    ));
}

#[test]
fn draw_only_pending_decisions_reject_an_original_winner_and_zero_time_stays_pending() {
    for reason in [
        NeedsDecisionReason::SimultaneousByoyomiEntry,
        NeedsDecisionReason::UnlimitedEliminationDraw,
        NeedsDecisionReason::ZeroTimeEliminationDraw,
    ] {
        let mut match_ =
            CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
        let result = TrustedGameResult {
            submission_id: "submission-invalid".into(),
            game_record_id: "game-invalid".into(),
            ruling_record_id: "ruling-invalid".into(),
            start_kind: ordinary(),
            first_player: team("a"),
            terminal_evidence: GameTerminalEvidence::CanonicalGameEnded {
                conclusion: winner_conclusion("a"),
            },
            ruling: CompetitionRuling::NeedsDecision { reason },
        };
        assert!(matches!(
            match_.submit_game(result),
            Err(MatchError::InvalidRuling(_))
        ));
    }

    let mut match_ =
        CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
    let result = TrustedGameResult {
        submission_id: "submission-zero-time".into(),
        game_record_id: "game-zero-time".into(),
        ruling_record_id: "ruling-zero-time".into(),
        start_kind: ordinary(),
        first_player: team("a"),
        terminal_evidence: GameTerminalEvidence::CanonicalGameEnded {
            conclusion: winner_conclusion("a"),
        },
        ruling: CompetitionRuling::NeedsDecision {
            reason: NeedsDecisionReason::ZeroTimeControl,
        },
    };
    match_.submit_game(result).unwrap();
    assert_eq!(match_.status(), MatchStatus::AwaitingReferee);
    assert!(matches!(
        match_.outcome(),
        Some(MatchOutcome::PendingReferee {
            reason: NeedsDecisionReason::ZeroTimeControl,
            ..
        })
    ));
}

#[test]
fn elimination_draw_produces_a_distinct_carry_forward_continuation_intent() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 1).unwrap();
    clock.end_window(14).unwrap();
    clock.begin_action(&team("b"), 2, 14).unwrap();
    clock.end_window(41).unwrap();
    let conclusion = draw_conclusion();
    let ruling = clock
        .rule_game(&conclusion, GameResultMode::Elimination)
        .unwrap();
    let continuation = match &ruling {
        CompetitionRuling::NeedsTiebreak { continuation } => continuation.clone(),
        other => panic!("預期和局加賽判定，收到 {other:?}"),
    };
    assert_eq!(continuation.teams[0].team, team("a"));
    assert_eq!(continuation.teams[0].main_time_remaining_ms, Some(87));
    assert_eq!(continuation.teams[1].team, team("b"));
    assert_eq!(continuation.teams[1].main_time_remaining_ms, Some(73));

    let mut match_ =
        CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
    assert_eq!(
        MatchFormat::FirstToWins(1).game_result_mode(),
        GameResultMode::Elimination
    );
    submit(
        &mut match_,
        "submission-1",
        "game-1",
        "ruling-1",
        ordinary(),
        "a",
        conclusion,
        ruling,
    );
    assert!(matches!(
        match_.next_game(),
        Some(intent) if intent.game_number == 2
            && matches!(intent.kind, GameStartKind::EliminationDrawContinuation { .. })
    ));
    assert_eq!(match_.status(), MatchStatus::Open);
    assert!(matches!(
        match_.next_game().map(|intent| &intent.first_player),
        Some(FirstPlayerIntent::Assigned { team: assigned }) if assigned == &team("b")
    ));
    assert_eq!(
        match_.next_game().unwrap().kind.required_clock_time(),
        NewGameTime::CarryForward
    );

    clock = clock.new_game(NewGameTime::CarryForward, 100).unwrap();
    clock.begin_action(&team("b"), 1, 100).unwrap();
    clock.end_window(119).unwrap();
    clock.begin_action(&team("a"), 2, 119).unwrap();
    clock.end_window(130).unwrap();
    let second_ruling = clock
        .rule_game(&draw_conclusion(), GameResultMode::Elimination)
        .unwrap();
    let second_continuation = match &second_ruling {
        CompetitionRuling::NeedsTiebreak { continuation } => continuation.clone(),
        other => panic!("預期第二局和局加賽判定，收到 {other:?}"),
    };
    assert_eq!(second_continuation.teams[0].team, team("a"));
    assert_eq!(
        second_continuation.teams[0].main_time_remaining_ms,
        Some(76)
    );
    assert_eq!(second_continuation.teams[1].team, team("b"));
    assert_eq!(
        second_continuation.teams[1].main_time_remaining_ms,
        Some(54)
    );
    submit(
        &mut match_,
        "submission-2",
        "game-2",
        "ruling-2",
        GameStartKind::EliminationDrawContinuation { continuation },
        "b",
        draw_conclusion(),
        second_ruling,
    );
    assert_eq!(match_.status(), MatchStatus::Open);
    assert!(matches!(
        match_.next_game().map(|intent| &intent.first_player),
        Some(FirstPlayerIntent::Assigned { team: assigned }) if assigned == &team("a")
    ));
    assert!(matches!(
        match_.next_game(),
        Some(intent) if intent.game_number == 3
            && matches!(&intent.kind, GameStartKind::EliminationDrawContinuation { continuation }
                if continuation == &second_continuation)
    ));
    submit(
        &mut match_,
        "submission-3",
        "game-3",
        "ruling-3",
        GameStartKind::EliminationDrawContinuation {
            continuation: second_continuation,
        },
        "a",
        winner_conclusion("b"),
        game_accepted_winner("b"),
    );
    assert_eq!(match_.status(), MatchStatus::Completed);
    assert!(matches!(
        match_.outcome(),
        Some(MatchOutcome::Winner { winner_id, .. }) if winner_id == &team("b")
    ));
}

#[test]
fn timeout_counts_as_a_win_but_double_loss_has_no_winner_and_needs_referee() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(10),
            action_time_ms: Some(5),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    let timeout_ruling = match clock.observe(15).unwrap() {
        fewfc::competition_clock::ClockUpdate::TimedOut { .. } => clock.timeout_ruling().unwrap(),
        update => panic!("預期單方超時，收到 {update:?}"),
    };
    let mut timeout_match =
        CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
    submit_evidence(
        &mut timeout_match,
        "submission-timeout",
        "game-timeout",
        "ruling-timeout",
        ordinary(),
        "a",
        timeout_evidence("timeout-single", vec![("a", 15)], 15),
        timeout_ruling,
    );
    assert!(matches!(
        timeout_match.outcome(),
        Some(MatchOutcome::Winner { winner_id, .. }) if winner_id == &team("b")
    ));
    let timeout_wire = serde_json::to_value(&timeout_match.games()[0].terminal_evidence).unwrap();
    assert_eq!(timeout_wire["type"], "timeout");
    assert_eq!(timeout_wire["timeoutId"], "timeout-single");
    assert!(timeout_wire.get("conclusion").is_none());
    let correction = timeout_match.correct_ruling(
        "timeout-correction".into(),
        "neutral-referee".into(),
        "game-timeout".into(),
        "timeout-replacement".into(),
        game_accepted_winner("b"),
        "超時證據不可改判為一般遊戲結論",
    );
    assert!(matches!(correction, Err(MatchError::InvalidRuling(_))));

    let mut double_clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(10),
            action_time_ms: Some(5),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    double_clock.begin_preparation(0).unwrap();
    assert!(matches!(
        double_clock.observe(15).unwrap(),
        fewfc::competition_clock::ClockUpdate::DoubleTimedOut { .. }
    ));
    let mut double_match =
        CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
    let double_result = TrustedGameResult {
        submission_id: "submission-double".into(),
        game_record_id: "game-double".into(),
        ruling_record_id: "ruling-double".into(),
        start_kind: ordinary(),
        first_player: team("a"),
        terminal_evidence: timeout_evidence("timeout-double", vec![("a", 15), ("b", 15)], 15),
        ruling: double_clock.timeout_ruling().unwrap(),
    };
    double_match.submit_game(double_result.clone()).unwrap();
    assert_eq!(double_match.status(), MatchStatus::AwaitingReferee);
    assert!(matches!(
        double_match.outcome(),
        Some(MatchOutcome::DoubleLoss {
            teams,
            progression: DoubleLossProgression::PendingReferee { .. }
        }) if teams == &[team("a"), team("b")]
    ));
    assert_eq!(
        double_match.submit_game(double_result).unwrap(),
        fewfc::competition_match::SubmissionOutcome::AlreadyRecorded
    );
}

#[test]
fn timeout_terminal_evidence_is_fail_closed_and_cannot_be_paired_with_other_rulings() {
    let cases = [
        (
            timeout_evidence("", vec![("a", 15)], 15),
            CompetitionRuling::TimeoutLoss { loser: team("a") },
        ),
        (
            GameTerminalEvidence::Timeout {
                timeout_id: "timeout-blank-digest".to_string(),
                expiries: vec![TimeoutExpiry {
                    team: team("a"),
                    deadline_monotonic_ms: 15,
                }],
                observed_at_monotonic_ms: 15,
                clock_checkpoint_digest: "   ".to_string(),
            },
            CompetitionRuling::TimeoutLoss { loser: team("a") },
        ),
        (
            timeout_evidence("timeout-late", vec![("a", 16)], 15),
            CompetitionRuling::TimeoutLoss { loser: team("a") },
        ),
        (
            timeout_evidence("timeout-empty-expiry", vec![], 15),
            CompetitionRuling::TimeoutLoss { loser: team("a") },
        ),
        (
            timeout_evidence("timeout-wrong-loser", vec![("a", 15)], 15),
            CompetitionRuling::TimeoutLoss { loser: team("b") },
        ),
        (
            timeout_evidence(
                "timeout-different-deadlines",
                vec![("a", 15), ("b", 16)],
                16,
            ),
            CompetitionRuling::DoubleTimeoutLoss {
                teams: [team("a"), team("b")],
            },
        ),
        (
            timeout_evidence("timeout-duplicate-team", vec![("a", 15), ("a", 15)], 15),
            CompetitionRuling::DoubleTimeoutLoss {
                teams: [team("a"), team("b")],
            },
        ),
        (
            timeout_evidence("timeout-one-for-double", vec![("a", 15)], 15),
            CompetitionRuling::DoubleTimeoutLoss {
                teams: [team("a"), team("b")],
            },
        ),
        (
            timeout_evidence("timeout-two-for-single", vec![("a", 15), ("b", 15)], 15),
            CompetitionRuling::TimeoutLoss { loser: team("a") },
        ),
        (
            timeout_evidence("timeout-other-ruling", vec![("a", 15)], 15),
            game_accepted_winner("b"),
        ),
    ];

    for (terminal_evidence, ruling) in cases {
        let mut match_ =
            CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
        let result = match_.submit_game(TrustedGameResult {
            submission_id: "submission-invalid-timeout".into(),
            game_record_id: "game-invalid-timeout".into(),
            ruling_record_id: "ruling-invalid-timeout".into(),
            start_kind: ordinary(),
            first_player: team("a"),
            terminal_evidence,
            ruling,
        });
        assert!(matches!(
            result,
            Err(MatchError::InvalidTerminalEvidence(_)) | Err(MatchError::InvalidRuling(_))
        ));
    }

    let mut canonical_match =
        CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
    let result = canonical_match.submit_game(TrustedGameResult {
        submission_id: "submission-canonical-timeout".into(),
        game_record_id: "game-canonical-timeout".into(),
        ruling_record_id: "ruling-canonical-timeout".into(),
        start_kind: ordinary(),
        first_player: team("a"),
        terminal_evidence: GameTerminalEvidence::CanonicalGameEnded {
            conclusion: winner_conclusion("a"),
        },
        ruling: CompetitionRuling::TimeoutLoss { loser: team("b") },
    });
    assert!(matches!(result, Err(MatchError::InvalidRuling(_))));

    let mut wrong_team_match =
        CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
    let result = wrong_team_match.submit_game(TrustedGameResult {
        submission_id: "submission-wrong-team".into(),
        game_record_id: "game-wrong-team".into(),
        ruling_record_id: "ruling-wrong-team".into(),
        start_kind: ordinary(),
        first_player: team("a"),
        terminal_evidence: timeout_evidence("timeout-wrong-team", vec![("c", 15)], 15),
        ruling: CompetitionRuling::TimeoutLoss { loser: team("c") },
    });
    assert!(matches!(result, Err(MatchError::TeamNotInMatch(_))));
}

#[test]
fn retry_is_idempotent_and_different_payload_with_same_id_is_rejected() {
    let mut match_ =
        CompetitionMatch::new(config(MatchFormat::FirstToWins(1), assigned("a"))).unwrap();
    let result = TrustedGameResult {
        submission_id: "submission-1".into(),
        game_record_id: "game-1".into(),
        ruling_record_id: "ruling-1".into(),
        start_kind: ordinary(),
        first_player: team("a"),
        terminal_evidence: GameTerminalEvidence::CanonicalGameEnded {
            conclusion: winner_conclusion("a"),
        },
        ruling: game_accepted_winner("a"),
    };
    assert_eq!(
        match_.submit_game(result.clone()).unwrap(),
        fewfc::competition_match::SubmissionOutcome::Applied
    );
    assert_eq!(
        match_.submit_game(result.clone()).unwrap(),
        fewfc::competition_match::SubmissionOutcome::AlreadyRecorded
    );
    let mut different = result;
    different.terminal_evidence = GameTerminalEvidence::CanonicalGameEnded {
        conclusion: winner_conclusion("b"),
    };
    different.ruling = game_accepted_winner("b");
    assert!(matches!(
        match_.submit_game(different),
        Err(MatchError::DuplicateSubmissionIdDifferentPayload(id)) if id == ResultSubmissionId::from("submission-1")
    ));
}

#[test]
fn referee_correction_keeps_history_and_cannot_be_done_by_a_participant() {
    let mut match_ = CompetitionMatch::new(CompetitionMatchConfig {
        referees: vec![
            RefereeAssignment {
                referee_id: RefereeId::from("team-a-player"),
                conflicted_team: Some(team("a")),
            },
            RefereeAssignment {
                referee_id: RefereeId::from("neutral-referee"),
                conflicted_team: None,
            },
        ],
        ..config(MatchFormat::FirstToWins(1), assigned("a"))
    })
    .unwrap();
    let original_ruling = CompetitionRuling::NeedsDecision {
        reason: NeedsDecisionReason::ZeroTimeControl,
    };
    submit(
        &mut match_,
        "submission-1",
        "game-1",
        "ruling-1",
        ordinary(),
        "a",
        draw_conclusion(),
        original_ruling.clone(),
    );
    let rejected = match_.correct_ruling(
        "correction-self".into(),
        "team-a-player".into(),
        "game-1".into(),
        "ruling-self".into(),
        CompetitionRuling::EliminationDrawLoss {
            loser: team("a"),
            reason: EliminationDrawLossReason::EnteredByoyomiFirst,
        },
        "self conflict",
    );
    assert!(matches!(
        rejected,
        Err(MatchError::SelfMatchRefereeConflict(id)) if id == RefereeId::from("team-a-player")
    ));

    match_
        .correct_ruling(
            "correction-1".into(),
            "neutral-referee".into(),
            "game-1".into(),
            "ruling-2".into(),
            CompetitionRuling::EliminationDrawLoss {
                loser: team("a"),
                reason: EliminationDrawLossReason::EnteredByoyomiFirst,
            },
            "權威時鐘紀錄確認 A 先進入讀秒",
        )
        .unwrap();
    assert_eq!(match_.status(), MatchStatus::Completed);
    assert!(matches!(
        match_.outcome(),
        Some(MatchOutcome::Winner { winner_id, .. }) if winner_id == &team("b")
    ));
    assert_eq!(
        match_.games()[0].terminal_evidence,
        GameTerminalEvidence::CanonicalGameEnded {
            conclusion: draw_conclusion()
        }
    );
    assert_eq!(match_.rulings().len(), 2);
    assert_eq!(match_.corrections().len(), 1);
    assert!(matches!(
        match_.ruling_for_game(&"game-1".into()),
        Some(record) if record.ruling_record_id == RulingRecordId::from("ruling-2")
            && record.ruling != original_ruling
    ));
    assert_eq!(
        match_
            .correct_ruling(
                "correction-1".into(),
                "neutral-referee".into(),
                "game-1".into(),
                "ruling-2".into(),
                CompetitionRuling::EliminationDrawLoss {
                    loser: team("a"),
                    reason: EliminationDrawLossReason::EnteredByoyomiFirst,
                },
                "權威時鐘紀錄確認 A 先進入讀秒",
            )
            .unwrap(),
        fewfc::competition_match::CorrectionOutcome::AlreadyRecorded
    );
}

#[test]
fn unknown_first_player_policy_is_an_explicit_pending_state() {
    let mut match_ = CompetitionMatch::new(config(
        MatchFormat::FixedGames(1),
        FirstPlayerIntent::Pending {
            reason: "賽事公告尚未指定先手政策".to_string(),
        },
    ))
    .unwrap();
    assert_eq!(match_.status(), MatchStatus::AwaitingFirstPlayer);
    assert!(matches!(
        match_.next_game(),
        Some(intent) if matches!(&intent.first_player, FirstPlayerIntent::Pending { reason } if reason.contains("尚未指定"))
    ));
    let result = TrustedGameResult {
        submission_id: "submission-1".into(),
        game_record_id: "game-1".into(),
        ruling_record_id: "ruling-1".into(),
        start_kind: ordinary(),
        first_player: team("a"),
        terminal_evidence: GameTerminalEvidence::CanonicalGameEnded {
            conclusion: winner_conclusion("a"),
        },
        ruling: game_accepted_winner("a"),
    };
    assert!(matches!(
        match_.submit_game(result),
        Err(MatchError::FirstPlayerPending { game_number: 1, .. })
    ));
}
