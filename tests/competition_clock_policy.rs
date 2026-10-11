use fewfc::competition_clock::{
    CheckpointWindow, ClockConfig, ClockError, ClockUpdate, CompetitionClock,
    CompetitionClockCheckpoint, CompetitionRuling, GameResultMode, NewGameTime,
};
use fewfc::domain::{GameConclusion, GameEndCause, GameOutcome, TeamId};

#[test]
fn zero_main_time_configuration_is_unlimited_during_preparation() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let start_ms = 50_000;
    let mut clock = CompetitionClock::new(
        [team_a.clone(), team_b],
        ClockConfig {
            main_time_ms: Some(0),
            action_time_ms: Some(30_000),
            ..ClockConfig::default()
        },
        start_ms,
    )
    .unwrap();

    assert_eq!(clock.begin_preparation(start_ms), Ok(ClockUpdate::Advanced));
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(None));
    assert_eq!(
        clock.observe(start_ms + 86_400_000),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(clock.needs_decision(), None);
    assert_eq!(clock.expired_team(), None);
    assert_eq!(clock.timeout_ruling(), None);
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(None));
    assert!(clock.snapshot().iter().all(|team| {
        !team.action_byoyomi_started
            && !team.entered_byoyomi
            && team.action_time_remaining_ms.is_none()
            && team.byoyomi_entry_sequence.is_none()
    }));
}

#[test]
fn finite_main_time_expiry_keeps_byoyomi_deadline_and_times_out() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let mut clock = CompetitionClock::new(
        [team_a.clone(), team_b],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        1_000,
    )
    .unwrap();

    assert_eq!(
        clock.begin_action(&team_a, 1, 1_000),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(1_130)));
    assert_eq!(
        clock.observe(1_100),
        Ok(ClockUpdate::EnteredByoyomi {
            team: team_a.clone()
        })
    );
    let after_main_expiry = clock.snapshot();
    assert_eq!(after_main_expiry[0].main_time_remaining_ms, Some(0));
    assert_eq!(after_main_expiry[0].action_time_remaining_ms, Some(30));
    assert!(after_main_expiry[0].action_byoyomi_started);
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(1_130)));

    assert_eq!(clock.observe(1_129), Ok(ClockUpdate::Advanced));
    assert_eq!(clock.expired_team(), None);
    assert_eq!(clock.snapshot()[0].action_time_remaining_ms, Some(1));
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(1_130)));
    assert_eq!(
        clock.observe(1_130),
        Ok(ClockUpdate::TimedOut { team: team_a.clone() })
    );
    assert_eq!(clock.expired_team(), Some(&team_a));
}

#[test]
fn zero_main_time_configuration_is_unlimited_during_action() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let start_ms = 20_000;
    let mut clock = CompetitionClock::new(
        [team_a.clone(), team_b],
        ClockConfig {
            main_time_ms: Some(0),
            action_time_ms: Some(30_000),
            ..ClockConfig::default()
        },
        start_ms,
    )
    .unwrap();

    assert_eq!(
        clock.begin_action(&team_a, 1, start_ms),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(None));
    assert_eq!(
        clock.observe(start_ms + 86_400_000),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(clock.needs_decision(), None);
    assert_eq!(clock.expired_team(), None);
    assert_eq!(clock.timeout_ruling(), None);
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(None));
    assert!(clock.snapshot().iter().all(|team| {
        !team.action_byoyomi_started
            && !team.entered_byoyomi
            && team.action_time_remaining_ms.is_none()
            && team.byoyomi_entry_sequence.is_none()
    }));
}

#[test]
fn zero_main_time_configuration_is_unlimited_during_out_of_turn_choice() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let start_ms = 40_000;
    let config = ClockConfig {
        main_time_ms: Some(0),
        action_time_ms: Some(30_000),
        out_of_turn_choice_time_ms: Some(15_000),
    };
    let mut clock = CompetitionClock::new([team_a.clone(), team_b], config, start_ms).unwrap();

    assert_eq!(clock.config(), config);
    assert_eq!(
        clock.begin_out_of_turn_choice(&team_a, "reaction-choice", start_ms),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(None));

    let observed_at_ms = start_ms + 86_400_000;
    assert_eq!(clock.observe(observed_at_ms), Ok(ClockUpdate::Advanced));
    assert_eq!(clock.needs_decision(), None);
    assert_eq!(clock.expired_team(), None);
    assert_eq!(clock.timeout_ruling(), None);
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(None));
    let display = clock.display(observed_at_ms).unwrap();
    assert!(!display.awaiting_authority);
    assert!(matches!(
        display.running,
        Some(CheckpointWindow::OutOfTurnChoice {
            team,
            choice_id,
            remaining_ms: None,
        }) if team == team_a && choice_id == "reaction-choice"
    ));
}

#[test]
fn zero_main_time_out_of_turn_choice_checkpoint_round_trip_preserves_unlimited_window() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let start_ms = 80_000;
    let config = ClockConfig {
        main_time_ms: Some(0),
        action_time_ms: Some(30_000),
        out_of_turn_choice_time_ms: Some(15_000),
    };
    let mut clock = CompetitionClock::new([team_a.clone(), team_b], config, start_ms).unwrap();

    assert_eq!(
        clock.begin_out_of_turn_choice(&team_a, "reaction-choice", start_ms),
        Ok(ClockUpdate::Advanced)
    );
    let checkpoint = clock.checkpoint();
    assert_eq!(checkpoint.config, config);
    assert!(matches!(
        checkpoint.running.as_ref(),
        Some(CheckpointWindow::OutOfTurnChoice {
            team,
            choice_id,
            remaining_ms: None,
        }) if team == &team_a && choice_id == "reaction-choice"
    ));

    let serialized = serde_json::to_string(&checkpoint).unwrap();
    let decoded: CompetitionClockCheckpoint = serde_json::from_str(&serialized).unwrap();
    assert_eq!(decoded, checkpoint);
    let mut restored = CompetitionClock::from_checkpoint(decoded).unwrap();

    assert_eq!(restored.config(), config);
    assert_eq!(restored.next_timeout_deadline_ms(), Ok(None));
    let observed_at_ms = start_ms + 86_400_000;
    assert_eq!(restored.observe(observed_at_ms), Ok(ClockUpdate::Advanced));
    assert_eq!(restored.needs_decision(), None);
    assert_eq!(restored.expired_team(), None);
    assert_eq!(restored.timeout_ruling(), None);
    assert_eq!(restored.next_timeout_deadline_ms(), Ok(None));
    let display = restored.display(observed_at_ms).unwrap();
    assert!(!display.awaiting_authority);
    assert!(matches!(
        display.running,
        Some(CheckpointWindow::OutOfTurnChoice {
            team,
            choice_id,
            remaining_ms: None,
        }) if team == team_a && choice_id == "reaction-choice"
    ));
}

#[test]
fn configured_zero_windows_round_trip_running_and_paused_checkpoints() {
    #[derive(Clone, Copy)]
    enum WindowKind {
        Preparation,
        Action,
        OutOfTurnChoice,
    }

    let config = ClockConfig {
        main_time_ms: Some(0),
        action_time_ms: Some(30_000),
        out_of_turn_choice_time_ms: Some(15_000),
    };
    for kind in [
        WindowKind::Preparation,
        WindowKind::Action,
        WindowKind::OutOfTurnChoice,
    ] {
        for paused in [false, true] {
            let team_a = TeamId::new("team-a");
            let team_b = TeamId::new("team-b");
            let start_ms = 100_000;
            let mut clock =
                CompetitionClock::new([team_a.clone(), team_b.clone()], config, start_ms).unwrap();
            let started = match kind {
                WindowKind::Preparation => clock.begin_preparation(start_ms),
                WindowKind::Action => clock.begin_action(&team_a, 1, start_ms),
                WindowKind::OutOfTurnChoice => {
                    clock.begin_out_of_turn_choice(&team_a, "reaction-choice", start_ms)
                }
            };
            assert_eq!(started, Ok(ClockUpdate::Advanced));
            if paused {
                assert_eq!(clock.pause(start_ms), Ok(ClockUpdate::Paused));
            }

            let checkpoint = clock.checkpoint();
            assert_eq!(checkpoint.config, config);
            let effective_window = if paused {
                checkpoint.paused.as_ref()
            } else {
                checkpoint.running.as_ref()
            };
            let expected_window = match kind {
                WindowKind::Preparation => matches!(
                    effective_window,
                    Some(CheckpointWindow::Preparation { teams })
                        if teams.len() == 2
                            && teams.contains(&team_a)
                            && teams.contains(&team_b)
                ),
                WindowKind::Action => matches!(
                    effective_window,
                    Some(CheckpointWindow::Action { team, turn })
                        if team == &team_a && *turn == 1
                ),
                WindowKind::OutOfTurnChoice => matches!(
                    effective_window,
                    Some(CheckpointWindow::OutOfTurnChoice {
                        team,
                        choice_id,
                        remaining_ms: None,
                    }) if team == &team_a && choice_id == "reaction-choice"
                ),
            };
            assert!(expected_window);

            let serialized = serde_json::to_string(&checkpoint).unwrap();
            let decoded: CompetitionClockCheckpoint = serde_json::from_str(&serialized).unwrap();
            assert_eq!(decoded, checkpoint);
            let mut restored = CompetitionClock::from_checkpoint(decoded).unwrap();
            assert_eq!(restored.config(), config);
            assert_eq!(restored.checkpoint(), checkpoint);
            assert_eq!(restored.next_timeout_deadline_ms(), Ok(None));
            if paused {
                let resumed_at_ms = start_ms + 1;
                assert_eq!(
                    restored.resume(resumed_at_ms),
                    Ok(ClockUpdate::Resumed)
                );
                let observed_at_ms = resumed_at_ms + 86_400_000;
                assert_eq!(
                    restored.observe(observed_at_ms),
                    Ok(ClockUpdate::Advanced)
                );
                assert_eq!(restored.config(), config);
                assert_eq!(restored.needs_decision(), None);
                assert_eq!(restored.expired_team(), None);
                assert_eq!(restored.timeout_ruling(), None);
                assert_eq!(restored.next_timeout_deadline_ms(), Ok(None));
                assert!(!restored.display(observed_at_ms).unwrap().awaiting_authority);
                assert!(restored.snapshot().iter().all(|team| {
                    team.main_time_remaining_ms == Some(0) && !team.entered_byoyomi
                }));
                let resumed_checkpoint = restored.checkpoint();
                assert_eq!(resumed_checkpoint.config, config);
                assert!(resumed_checkpoint.paused.is_none());
                assert!(resumed_checkpoint.running.is_some());
                if matches!(kind, WindowKind::OutOfTurnChoice) {
                    assert!(matches!(
                        resumed_checkpoint.running.as_ref(),
                        Some(CheckpointWindow::OutOfTurnChoice {
                            remaining_ms: None,
                            ..
                        })
                    ));
                }
            }
        }
    }
}

#[test]
fn finite_main_time_checkpoint_rejects_unlimited_choice_remaining() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let start_ms = 120_000;
    let config = ClockConfig {
        main_time_ms: Some(60_000),
        action_time_ms: Some(30_000),
        out_of_turn_choice_time_ms: Some(15_000),
    };
    let mut clock = CompetitionClock::new([team_a.clone(), team_b], config, start_ms).unwrap();
    assert_eq!(
        clock.begin_out_of_turn_choice(&team_a, "reaction-choice", start_ms),
        Ok(ClockUpdate::Advanced)
    );

    let mut checkpoint = clock.checkpoint();
    match checkpoint.running.as_mut() {
        Some(CheckpointWindow::OutOfTurnChoice { remaining_ms, .. }) => *remaining_ms = None,
        _ => panic!("expected a running out-of-turn choice checkpoint"),
    }

    assert!(matches!(
        CompetitionClock::from_checkpoint(checkpoint),
        Err(ClockError::InvalidCheckpoint(
            "choice remaining time does not match configured allowance"
        ))
    ));
}

#[test]
fn configured_zero_elimination_draw_carries_unlimited_time_without_a_cap() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let start_ms = 200_000;
    let config = ClockConfig {
        main_time_ms: Some(0),
        action_time_ms: Some(30_000),
        out_of_turn_choice_time_ms: Some(15_000),
    };
    let clock = CompetitionClock::new([team_a.clone(), team_b.clone()], config, start_ms).unwrap();
    let conclusion = GameConclusion::new(
        GameOutcome::Draw,
        vec![GameEndCause::TeamHpDepleted {
            teams: vec![team_a.clone(), team_b.clone()],
        }],
    );
    let original_conclusion = conclusion.clone();

    assert!(clock.snapshot().iter().all(|team| !team.entered_byoyomi));
    let first_ruling = clock
        .rule_game(&conclusion, GameResultMode::Elimination)
        .unwrap();
    assert!(matches!(
        first_ruling,
        CompetitionRuling::NeedsTiebreak { .. }
    ));
    assert_eq!(conclusion, original_conclusion);

    let mut tiebreak = clock
        .new_game(NewGameTime::CarryForward, start_ms)
        .unwrap();
    assert_eq!(tiebreak.config(), config);
    assert!(tiebreak
        .snapshot()
        .iter()
        .all(|team| team.main_time_remaining_ms == Some(0)));
    assert_eq!(
        tiebreak.begin_action(&team_a, 1, start_ms),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(tiebreak.next_timeout_deadline_ms(), Ok(None));
    let after_action_ms = start_ms + 60_000;
    assert_eq!(
        tiebreak.observe(after_action_ms),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(tiebreak.next_timeout_deadline_ms(), Ok(None));
    assert!(tiebreak.snapshot().iter().all(|team| !team.entered_byoyomi));
    assert_eq!(
        tiebreak.end_window(after_action_ms),
        Ok(ClockUpdate::Unchanged)
    );

    let second_ruling = tiebreak
        .rule_game(&conclusion, GameResultMode::Elimination)
        .unwrap();
    assert!(matches!(
        second_ruling,
        CompetitionRuling::NeedsTiebreak { .. }
    ));
    assert_eq!(conclusion, original_conclusion);
}

#[test]
fn raw_unlimited_main_time_elimination_draw_carries_unlimited_time() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let start_ms = 300_000;
    let config = ClockConfig {
        main_time_ms: None,
        action_time_ms: Some(30_000),
        out_of_turn_choice_time_ms: Some(15_000),
    };
    let clock = CompetitionClock::new([team_a.clone(), team_b.clone()], config, start_ms).unwrap();
    let conclusion = GameConclusion::new(
        GameOutcome::Draw,
        vec![GameEndCause::TeamHpDepleted {
            teams: vec![team_a.clone(), team_b.clone()],
        }],
    );
    let original_conclusion = conclusion.clone();

    let first_ruling = clock
        .rule_game(&conclusion, GameResultMode::Elimination)
        .unwrap();
    let continuation = match first_ruling {
        CompetitionRuling::NeedsTiebreak { continuation } => continuation,
        other => panic!("expected an unlimited tiebreak continuation, got {other:?}"),
    };
    assert_eq!(continuation.teams.len(), 2);
    assert!(continuation
        .teams
        .iter()
        .all(|team| team.main_time_remaining_ms.is_none()));
    assert_eq!(conclusion, original_conclusion);

    let mut tiebreak = clock
        .new_game(NewGameTime::CarryForward, start_ms)
        .unwrap();
    assert_eq!(tiebreak.config(), config);
    assert!(tiebreak
        .snapshot()
        .iter()
        .all(|team| team.main_time_remaining_ms.is_none()));
    assert_eq!(
        tiebreak.begin_action(&team_a, 1, start_ms),
        Ok(ClockUpdate::Advanced)
    );
    let after_action_ms = start_ms + 60_000;
    assert_eq!(
        tiebreak.observe(after_action_ms),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(tiebreak.next_timeout_deadline_ms(), Ok(None));
    assert!(tiebreak.snapshot().iter().all(|team| !team.entered_byoyomi));
    assert_eq!(
        tiebreak.end_window(after_action_ms),
        Ok(ClockUpdate::Unchanged)
    );

    assert!(matches!(
        tiebreak
            .rule_game(&conclusion, GameResultMode::Elimination)
            .unwrap(),
        CompetitionRuling::NeedsTiebreak { .. }
    ));
    assert_eq!(conclusion, original_conclusion);
}

#[test]
fn configured_zero_action_checkpoint_rejects_recorded_byoyomi_entry() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let start_ms = 400_000;
    let config = ClockConfig {
        main_time_ms: Some(0),
        action_time_ms: Some(30_000),
        out_of_turn_choice_time_ms: Some(15_000),
    };
    let mut clock = CompetitionClock::new([team_a.clone(), team_b], config, start_ms).unwrap();
    assert_eq!(
        clock.begin_action(&team_a, 1, start_ms),
        Ok(ClockUpdate::Advanced)
    );

    let mut checkpoint = clock.checkpoint();
    assert_eq!(checkpoint.config, config);
    assert!(matches!(
        checkpoint.running.as_ref(),
        Some(CheckpointWindow::Action { team, turn }) if team == &team_a && *turn == 1
    ));
    assert!(checkpoint.teams.iter().all(|team| {
        team.main_time_remaining_ms == Some(0)
            && !team.entered_byoyomi
            && team.byoyomi_entry_sequence.is_none()
            && !team.action_byoyomi_started
            && team.action_time_remaining_ms.is_none()
    }));
    assert_eq!(checkpoint.next_byoyomi_entry_sequence, 0);

    let team_a_entry = checkpoint
        .teams
        .iter_mut()
        .find(|team| team.team == team_a)
        .expect("公開 Action checkpoint 應包含第一隊");
    team_a_entry.entered_byoyomi = true;
    team_a_entry.byoyomi_entry_sequence = Some(0);
    checkpoint.next_byoyomi_entry_sequence = 1;

    let serialized = serde_json::to_string(&checkpoint).unwrap();
    let decoded: CompetitionClockCheckpoint = serde_json::from_str(&serialized).unwrap();
    assert_eq!(decoded, checkpoint);
    let restored = CompetitionClock::from_checkpoint(decoded);
    assert!(
        matches!(&restored, Err(ClockError::InvalidCheckpoint(_))),
        "配置零的不限時時計不能恢復讀秒進入事實，實際結果：{restored:?}"
    );
}

#[test]
fn configured_zero_action_restore_rejects_corruption_and_accepts_legitimate_checkpoints() {
    #[derive(Clone, Copy, Debug)]
    enum Corruption {
        RecordedEntry,
        ActiveByoyomi,
        SingleExpiry,
    }
    let round_trip = |checkpoint: &CompetitionClockCheckpoint| {
        let serialized = serde_json::to_string(checkpoint).unwrap();
        let decoded: CompetitionClockCheckpoint = serde_json::from_str(&serialized).unwrap();
        assert_eq!(&decoded, checkpoint);
        decoded
    };
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let config = ClockConfig {
        main_time_ms: Some(0),
        action_time_ms: Some(30_000),
        out_of_turn_choice_time_ms: Some(15_000),
    };
    for paused in [false, true] {
        let start_ms = 500_000;
        let mut clock =
            CompetitionClock::new([team_a.clone(), team_b.clone()], config, start_ms).unwrap();
        assert_eq!(
            clock.begin_action(&team_a, 1, start_ms),
            Ok(ClockUpdate::Advanced)
        );
        if paused {
            assert_eq!(clock.pause(start_ms + 100), Ok(ClockUpdate::Paused));
        }
        let base = clock.checkpoint();
        let window = if paused {
            assert!(base.running.is_none());
            base.paused.as_ref()
        } else {
            assert!(base.paused.is_none());
            base.running.as_ref()
        };
        assert!(matches!(
            window,
            Some(CheckpointWindow::Action { team, turn }) if team == &team_a && *turn == 1
        ));
        assert_eq!(base.config, config);
        let restored = CompetitionClock::from_checkpoint(round_trip(&base)).unwrap();
        assert_eq!(restored.checkpoint(), base);
        assert_eq!(restored.next_timeout_deadline_ms(), Ok(None));
        assert_eq!(restored.expired_team(), None);
        assert!(restored.snapshot().iter().all(|team| {
            team.main_time_remaining_ms == Some(0)
                && !team.entered_byoyomi
                && team.byoyomi_entry_sequence.is_none()
                && !team.action_byoyomi_started
                && team.action_time_remaining_ms.is_none()
        }));

        for corruption in [
            Corruption::RecordedEntry,
            Corruption::ActiveByoyomi,
            Corruption::SingleExpiry,
        ] {
            let mut candidate = base.clone();
            let entry = candidate
                .teams
                .iter_mut()
                .find(|team| team.team == team_a)
                .expect("公開 Action checkpoint 應包含第一隊");
            entry.entered_byoyomi = true;
            entry.byoyomi_entry_sequence = Some(0);
            if matches!(
                corruption,
                Corruption::ActiveByoyomi | Corruption::SingleExpiry
            ) {
                entry.action_byoyomi_started = true;
                entry.action_time_remaining_ms = Some(match corruption {
                    Corruption::SingleExpiry => 0,
                    _ => 30_000,
                });
            }
            candidate.next_byoyomi_entry_sequence = 1;
            if matches!(corruption, Corruption::SingleExpiry) {
                candidate.expired_team = Some(team_a.clone());
            }
            let expected_reason = if paused && matches!(corruption, Corruption::SingleExpiry) {
                // 暫停終局先被既有通用驗證拒絕，不能當成配置零 guard 的獨立證據。
                "paused clock cannot be terminal or require a decision"
            } else {
                "configured zero main time cannot have byoyomi history"
            };
            assert_eq!(
                CompetitionClock::from_checkpoint(round_trip(&candidate)),
                Err(ClockError::InvalidCheckpoint(expected_reason)),
                "paused={paused}, corruption={corruption:?}"
            );
        }
    }

    let finite_config = ClockConfig {
        main_time_ms: Some(100),
        action_time_ms: Some(30),
        out_of_turn_choice_time_ms: Some(20),
    };
    let mut finite =
        CompetitionClock::new([team_a.clone(), team_b], finite_config, 600_000).unwrap();
    assert_eq!(
        finite.begin_action(&team_a, 1, 600_000),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(
        finite.observe(600_100),
        Ok(ClockUpdate::EnteredByoyomi {
            team: team_a.clone()
        })
    );
    let byoyomi_checkpoint = finite.checkpoint();
    let byoyomi_team = byoyomi_checkpoint
        .teams
        .iter()
        .find(|team| team.team == team_a)
        .unwrap();
    assert_eq!(byoyomi_team.main_time_remaining_ms, Some(0));
    assert_eq!(byoyomi_team.action_time_remaining_ms, Some(30));
    assert!(byoyomi_team.entered_byoyomi && byoyomi_team.action_byoyomi_started);
    let restored = CompetitionClock::from_checkpoint(round_trip(&byoyomi_checkpoint)).unwrap();
    assert_eq!(restored.config(), finite_config);
    assert_eq!(restored.checkpoint(), byoyomi_checkpoint);
    assert_eq!(restored.next_timeout_deadline_ms(), Ok(Some(600_130)));

    assert_eq!(
        finite.observe(600_130),
        Ok(ClockUpdate::TimedOut {
            team: team_a.clone()
        })
    );
    let expired_checkpoint = finite.checkpoint();
    assert_eq!(expired_checkpoint.expired_team, Some(team_a.clone()));
    let restored = CompetitionClock::from_checkpoint(round_trip(&expired_checkpoint)).unwrap();
    assert_eq!(restored.config(), finite_config);
    assert_eq!(restored.checkpoint(), expired_checkpoint);
    assert_eq!(
        restored.timeout_ruling(),
        Some(CompetitionRuling::TimeoutLoss { loser: team_a })
    );
}

#[test]
fn finite_other_team_out_of_turn_choice_uses_main_time_before_independent_byoyomi() {
    let team_a = TeamId::new("team-a");
    let team_b = TeamId::new("team-b");
    let mut clock = CompetitionClock::new(
        [team_a.clone(), team_b.clone()],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(30),
        },
        1_000,
    )
    .unwrap();

    // 先經公開介面停止 A 的扣時視窗，再讓他隊 B 作答；不在 choice 期間另開 action。
    assert_eq!(
        clock.begin_action(&team_a, 1, 1_000),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(clock.end_window(1_010), Ok(ClockUpdate::Advanced));
    let team_a_after_action = clock.snapshot()[0].clone();
    assert_eq!(team_a_after_action.main_time_remaining_ms, Some(90));
    assert_eq!(clock.snapshot()[1].main_time_remaining_ms, Some(100));
    assert_eq!(
        clock.begin_out_of_turn_choice(&team_b, "other-team-choice-1", 1_010),
        Ok(ClockUpdate::Advanced)
    );

    // 第一個政策斷言：B 先用主時間 100，再接獨立讀秒 30；跳過主時間的 1040 不合法。
    assert_eq!(
        clock.next_timeout_deadline_ms(),
        Ok(Some(1_140)),
        "他隊回合外選擇應在 1140 才耗盡；1040 表示跳過 B 的主時間"
    );
    assert_eq!(
        clock.observe(1_110),
        Ok(ClockUpdate::EnteredByoyomi {
            team: team_b.clone()
        })
    );
    let after_main_expiry = clock.snapshot();
    assert_eq!(after_main_expiry[0], team_a_after_action);
    assert_eq!(after_main_expiry[1].main_time_remaining_ms, Some(0));
    assert!(after_main_expiry[1].entered_byoyomi);
    assert_eq!(clock.expired_team(), None);
    assert_eq!(clock.timeout_ruling(), None);
    assert!(matches!(
        clock.checkpoint().running,
        Some(CheckpointWindow::OutOfTurnChoice {
            team,
            choice_id,
            remaining_ms: Some(30),
        }) if team == team_b && choice_id == "other-team-choice-1"
    ));
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(1_140)));

    assert_eq!(clock.observe(1_120), Ok(ClockUpdate::Advanced));
    assert_eq!(clock.snapshot()[0], team_a_after_action);
    assert_eq!(clock.snapshot()[1].main_time_remaining_ms, Some(0));
    let checkpoint_at_1120 = clock.checkpoint();
    assert!(matches!(
        checkpoint_at_1120.running.as_ref(),
        Some(CheckpointWindow::OutOfTurnChoice {
            team,
            choice_id,
            remaining_ms: Some(20),
        }) if team == &team_b && choice_id == "other-team-choice-1"
    ));
    let display_at_1120 = clock.display(1_120).unwrap();
    assert_eq!(display_at_1120.teams[0].main_time_remaining_ms, Some(90));
    assert_eq!(display_at_1120.teams[1].main_time_remaining_ms, Some(0));
    assert_eq!(display_at_1120.running, checkpoint_at_1120.running);
    assert!(!display_at_1120.awaiting_authority);
    assert_eq!(clock.checkpoint(), checkpoint_at_1120);
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(1_140)));

    assert_eq!(clock.observe(1_139), Ok(ClockUpdate::Advanced));
    assert_eq!(clock.snapshot()[0], team_a_after_action);
    assert_eq!(clock.snapshot()[1].main_time_remaining_ms, Some(0));
    assert!(matches!(
        clock.checkpoint().running,
        Some(CheckpointWindow::OutOfTurnChoice {
            team,
            choice_id,
            remaining_ms: Some(1),
        }) if team == team_b && choice_id == "other-team-choice-1"
    ));
    assert_eq!(clock.expired_team(), None);
    assert_eq!(clock.timeout_ruling(), None);
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(1_140)));

    assert_eq!(
        clock.observe(1_140),
        Ok(ClockUpdate::TimedOut {
            team: team_b.clone()
        })
    );
    assert_eq!(clock.snapshot()[0], team_a_after_action);
    assert_eq!(clock.snapshot()[1].main_time_remaining_ms, Some(0));
    assert!(matches!(
        clock.checkpoint().running,
        Some(CheckpointWindow::OutOfTurnChoice {
            team,
            choice_id,
            remaining_ms: Some(0),
        }) if team == team_b && choice_id == "other-team-choice-1"
    ));
    assert_eq!(clock.expired_team(), Some(&team_b));
    assert_eq!(
        clock.timeout_ruling(),
        Some(CompetitionRuling::TimeoutLoss { loser: team_b })
    );
}
