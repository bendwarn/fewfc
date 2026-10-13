use fewfc::competition_clock::ClockConfig;
use fewfc::competition_match::MatchFormat;
use fewfc::tournament::*;
use std::collections::BTreeSet;

#[test]
fn round_robin_double_loss_exits_both_teams_and_closes_their_original_future_matches() {
    let stage_id = StageId("round-robin-exit".into());
    let stage_config = StageConfig {
        stage_id: stage_id.clone(),
        format: TournamentFormat::RoundRobin,
        teams: (1..=4)
            .map(|seed| SeededTeam {
                team_id: TeamId(format!("team-{seed}")),
                seed,
            })
            .collect(),
        random_seed: 7,
        match_format: MatchFormat::FixedGames(1),
        clock_config: ClockConfig {
            main_time_ms: Some(300_000),
            action_time_ms: Some(30_000),
            out_of_turn_choice_time_ms: Some(30_000),
        },
        swiss_rounds: None,
        swiss_policy: None,
    };
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "round-robin-double-loss-exit".into(),
        stage: stage_config.clone(),
    })
    .expect("四隊循環、固定一局與有限時計設定應合法");

    let public_match = |stage: &Stage, match_id: &MatchId| {
        stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .find(|scheduled| &scheduled.match_id == match_id)
            .expect("原公開交手 ID 必須保留")
            .clone()
    };
    let public_stats = |stage: &Stage, team_id: &TeamId| {
        let row = stage
            .standings()
            .iter()
            .find(|row| &row.team_id == team_id)
            .expect("原參賽隊伍的統計必須保留");
        (
            row.wins,
            row.draws,
            row.losses,
            row.game_wins,
            row.game_losses,
            row.net_game_wins,
            row.match_points,
        )
    };

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(stage.config(), &stage_config);
    let original_rounds = stage.rounds().to_vec();
    assert_eq!(original_rounds.len(), 3);
    let expected_teams: BTreeSet<_> = stage_config
        .teams
        .iter()
        .map(|team| team.team_id.clone())
        .collect();
    let mut original_ids = BTreeSet::new();
    let mut original_pairs = BTreeSet::new();

    // 只驗公開排程：三輪六個不同隊伍組合，每輪各隊恰好一次，不猜配對算法。
    for (index, round) in original_rounds.iter().enumerate() {
        assert_eq!(round.round_number, (index + 1) as u32);
        assert_eq!(round.kind, RoundKind::Main);
        assert_eq!(round.matches.len(), 2);
        let mut round_teams = BTreeSet::new();
        for scheduled in &round.matches {
            assert_eq!(scheduled.stage_id, stage_id);
            assert_eq!(scheduled.round_number, round.round_number);
            assert_eq!(scheduled.kind, RoundKind::Main);
            assert_eq!(scheduled.participants.len(), 2);
            assert_ne!(scheduled.participants[0], scheduled.participants[1]);
            assert_eq!(scheduled.status, MatchStatus::Pending);
            assert_eq!(scheduled.result, None);
            assert_eq!(scheduled.cancellation_reason, None);
            assert!(!scheduled.started);
            assert!(original_ids.insert(scheduled.match_id.clone()));
            for team_id in &scheduled.participants {
                assert!(expected_teams.contains(team_id));
                assert!(round_teams.insert(team_id.clone()));
            }
            let mut pair = scheduled.participants.clone();
            pair.sort();
            assert!(original_pairs.insert(pair));
        }
        assert_eq!(round_teams, expected_teams);
    }
    assert_eq!(original_ids.len(), 6);
    assert_eq!(original_pairs.len(), 6);
    assert_eq!(stage.standings().len(), 4);
    for team_id in &expected_teams {
        assert_eq!(public_stats(stage, team_id), (0, 0, 0, 0, 0, 0, 0));
    }

    // 從真正首輪取得兩筆交手；未退出的兩隊已在另一筆首輪互為對手。
    let double_loss_match = original_rounds[0].matches[0].clone();
    let legal_match = original_rounds[0].matches[1].clone();
    let exited_teams = double_loss_match.participants.clone();
    let original_future_matches: Vec<_> = original_rounds
        .iter()
        .skip(1)
        .flat_map(|round| &round.matches)
        .filter(|scheduled| {
            scheduled
                .participants
                .iter()
                .any(|team_id| exited_teams.contains(team_id))
        })
        .cloned()
        .collect();
    assert_eq!(original_future_matches.len(), 4);

    assert_eq!(
        tournament.start_match(&stage_id, &double_loss_match.match_id),
        Ok(())
    );
    let started = public_match(
        tournament.stage(&stage_id).unwrap(),
        &double_loss_match.match_id,
    );
    assert_eq!(started.status, MatchStatus::InProgress);
    assert!(started.started);
    assert_eq!(started.result, None);
    assert_eq!(
        tournament
            .submit_result(
                &stage_id,
                &double_loss_match.match_id,
                MatchResult::DoubleLoss,
            )
            .unwrap(),
        UpdateOutcome::Applied
    );

    let stage = tournament.stage(&stage_id).unwrap();
    let played_double_loss = public_match(stage, &double_loss_match.match_id);
    assert_eq!(played_double_loss.status, MatchStatus::Completed);
    assert_eq!(played_double_loss.result, Some(MatchResult::DoubleLoss));
    assert!(played_double_loss.started);
    assert_eq!(played_double_loss.participants, exited_teams);
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(public_match(stage, &legal_match.match_id), legal_match);
    for team_id in &exited_teams {
        // 雙負只各記一負，不造單局比分、和局分或自願棄賽結果。
        assert_eq!(public_stats(stage, team_id), (0, 0, 1, 0, 0, 0, 0));
    }
    for team_id in &legal_match.participants {
        assert_eq!(public_stats(stage, team_id), (0, 0, 0, 0, 0, 0, 0));
    }

    // 第一政策 target：退出後原後續交手不能仍是可出賽狀態，不指定新關閉 enum。
    for original in &original_future_matches {
        let scheduled = public_match(stage, &original.match_id);
        assert!(
            !matches!(
                scheduled.status,
                MatchStatus::Pending | MatchStatus::InProgress
            ),
            "雙負退出隊伍的原後續交手仍可出賽：{} / {:?}",
            scheduled.match_id,
            scheduled.status
        );
        assert_eq!(scheduled.participants, original.participants);
        assert_eq!(scheduled.result, None);
        assert!(!scheduled.started);
    }

    // 真公開開賽必須拒絕，完整快照不動；不靠最後統計掩蓋仍可出賽的缺口。
    let blocked_match_id = &original_future_matches[0].match_id;
    let before_rejected_start = serde_json::to_value(&tournament).unwrap();
    assert!(tournament.start_match(&stage_id, blocked_match_id).is_err());
    assert_eq!(
        serde_json::to_value(&tournament).unwrap(),
        before_rejected_start
    );
    assert_eq!(
        public_match(tournament.stage(&stage_id).unwrap(), &legal_match.match_id),
        legal_match
    );

    // 保留原合法首輪配對，真正開賽並提交一比零，而非退出造成的假勝或輪空。
    assert_eq!(
        tournament.start_match(&stage_id, &legal_match.match_id),
        Ok(())
    );
    let started_legal = public_match(tournament.stage(&stage_id).unwrap(), &legal_match.match_id);
    assert_eq!(started_legal.status, MatchStatus::InProgress);
    assert!(started_legal.started);
    assert_eq!(started_legal.result, None);
    let legal_win = MatchResult::Win {
        winner_id: legal_match.participants[0].clone(),
        loser_id: legal_match.participants[1].clone(),
        winner_games: 1,
        loser_games: 0,
    };
    assert_eq!(
        tournament
            .submit_result(&stage_id, &legal_match.match_id, legal_win.clone())
            .unwrap(),
        UpdateOutcome::Applied
    );

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(stage.config(), &stage_config);
    assert_eq!(tournament.stages().len(), 1);
    assert!(stage.corrections().is_empty());
    assert!(stage.superseded_rounds().is_empty());
    assert_eq!(
        public_match(stage, &double_loss_match.match_id),
        played_double_loss
    );
    let played_legal = public_match(stage, &legal_match.match_id);
    assert_eq!(played_legal.status, MatchStatus::Completed);
    assert_eq!(played_legal.result, Some(legal_win));
    assert!(played_legal.started);
    for team_id in &exited_teams {
        assert_eq!(public_stats(stage, team_id), (0, 0, 1, 0, 0, 0, 0));
    }
    assert_eq!(
        public_stats(stage, &legal_match.participants[0]),
        (1, 0, 0, 1, 0, 1, 3)
    );
    assert_eq!(
        public_stats(stage, &legal_match.participants[1]),
        (0, 0, 1, 0, 1, -1, 0)
    );

    // 原三輪六筆 ID／位置／參賽者完整保留；四筆未玩交手不造任何賽果或 started。
    assert_eq!(stage.rounds().len(), 3);
    let final_ids: BTreeSet<_> = stage
        .rounds()
        .iter()
        .flat_map(|round| &round.matches)
        .map(|scheduled| scheduled.match_id.clone())
        .collect();
    assert_eq!(final_ids, original_ids);
    assert_eq!(stage.standings().len(), 4);
    for (round, original_round) in stage.rounds().iter().zip(&original_rounds) {
        assert_eq!(round.round_number, original_round.round_number);
        assert_eq!(round.kind, original_round.kind);
        assert_eq!(round.matches.len(), 2);
        for (scheduled, original) in round.matches.iter().zip(&original_round.matches) {
            assert_eq!(scheduled.match_id, original.match_id);
            assert_eq!(scheduled.stage_id, original.stage_id);
            assert_eq!(scheduled.round_number, original.round_number);
            assert_eq!(scheduled.sequence, original.sequence);
            assert_eq!(scheduled.kind, original.kind);
            assert_eq!(scheduled.participants, original.participants);
            if round.round_number > 1 {
                assert!(!matches!(
                    scheduled.status,
                    MatchStatus::Pending | MatchStatus::InProgress
                ));
                assert_eq!(scheduled.result, None);
                assert!(!scheduled.started);
            }
        }
    }
}

#[test]
fn active_round_robin_double_loss_correction_restores_original_unplayed_future_matches() {
    fn four_team_round_robin(stage_id: StageId) -> Tournament {
        Tournament::new(TournamentConfig {
            tournament_id: "active-round-robin-double-loss-correction".into(),
            stage: StageConfig {
                stage_id,
                format: TournamentFormat::RoundRobin,
                teams: (1..=4)
                    .map(|seed| SeededTeam {
                        team_id: TeamId(format!("team-{seed}")),
                        seed,
                    })
                    .collect(),
                random_seed: 7,
                match_format: MatchFormat::FixedGames(1),
                clock_config: ClockConfig {
                    main_time_ms: Some(300_000),
                    action_time_ms: Some(30_000),
                    out_of_turn_choice_time_ms: Some(30_000),
                },
                swiss_rounds: None,
                swiss_policy: None,
            },
        })
        .expect("四隊循環與固定一局設定應合法")
    }

    let stage_id = StageId("active-round-robin-correction".into());
    let mut tournament = four_team_round_robin(stage_id.clone());
    let public_match = |stage: &Stage, match_id: &MatchId| {
        stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .find(|scheduled| &scheduled.match_id == match_id)
            .expect("更正不能移除原公開交手 ID")
            .clone()
    };
    let stage = tournament.stage(&stage_id).unwrap();
    let original_rounds = stage.rounds().to_vec();
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(original_rounds.len(), 3);
    assert!(original_rounds.iter().all(|round| round.matches.len() == 2));
    assert!(original_rounds.iter().flat_map(|round| &round.matches).all(
        |scheduled| scheduled.status == MatchStatus::Pending
            && scheduled.participants.len() == 2
            && scheduled.result.is_none()
            && scheduled.cancellation_reason.is_none()
            && !scheduled.started
    ));
    let original_ids: BTreeSet<_> = original_rounds
        .iter()
        .flat_map(|round| &round.matches)
        .map(|scheduled| scheduled.match_id.clone())
        .collect();
    assert_eq!(original_ids.len(), 6);
    let double_loss_match = original_rounds[0].matches[0].clone();
    let other_first_match = original_rounds[0].matches[1].clone();
    let original_future_matches: Vec<_> = original_rounds
        .iter()
        .skip(1)
        .flat_map(|round| &round.matches)
        .cloned()
        .collect();
    assert_eq!(original_future_matches.len(), 4);
    assert!(original_future_matches.iter().all(|scheduled| {
        scheduled
            .participants
            .iter()
            .any(|team_id| double_loss_match.participants.contains(team_id))
    }));

    assert_eq!(
        tournament.start_match(&stage_id, &double_loss_match.match_id),
        Ok(())
    );
    assert_eq!(
        tournament
            .submit_result(
                &stage_id,
                &double_loss_match.match_id,
                MatchResult::DoubleLoss,
            )
            .unwrap(),
        UpdateOutcome::Applied
    );
    let stage = tournament.stage(&stage_id).unwrap();
    let played_double_loss = public_match(stage, &double_loss_match.match_id);
    assert_eq!(played_double_loss.status, MatchStatus::Completed);
    assert_eq!(played_double_loss.result, Some(MatchResult::DoubleLoss));
    assert!(played_double_loss.started);
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(
        public_match(stage, &other_first_match.match_id),
        other_first_match
    );
    assert!(stage.corrections().is_empty());
    for original in &original_future_matches {
        let cancelled = public_match(stage, &original.match_id);
        assert_eq!(cancelled.status, MatchStatus::Cancelled);
        assert!(cancelled.cancellation_reason.is_some());
        assert_eq!(cancelled.result, None);
        assert!(!cancelled.started);
    }

    // 另一首輪交手仍未玩，這是在 Active 階段改判，不是已結束賽程重新開賽。
    let winner_id = double_loss_match.participants[0].clone();
    let loser_id = double_loss_match.participants[1].clone();
    let replacement = MatchResult::Win {
        winner_id: winner_id.clone(),
        loser_id: loser_id.clone(),
        winner_games: 1,
        loser_games: 0,
    };
    let reason = "裁判核對原交手紀錄，將首輪雙方判負更正為一比零".to_string();
    assert_eq!(
        tournament.correct_result(
            &stage_id,
            &double_loss_match.match_id,
            replacement.clone(),
            reason.clone(),
        ),
        Ok(())
    );
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(
        public_match(stage, &other_first_match.match_id),
        other_first_match
    );

    // 第一政策 target：撤銷 DL 的退出效果，恢復原未玩配對，不生成替代交手。
    for original in &original_future_matches {
        let restored = public_match(stage, &original.match_id);
        assert_eq!(
            restored.status,
            MatchStatus::Pending,
            "Active RR 改判後原未玩交手仍被關閉：{}",
            restored.match_id
        );
        assert_eq!(restored.cancellation_reason, None);
        assert_eq!(restored.result, None);
        assert!(!restored.started);
        assert_eq!(&restored, original);
    }
    let corrected_match = public_match(stage, &double_loss_match.match_id);
    assert_eq!(corrected_match.status, MatchStatus::Completed);
    assert_eq!(corrected_match.result, Some(replacement.clone()));
    assert!(corrected_match.started);
    assert_eq!(corrected_match.participants, double_loss_match.participants);
    assert_eq!(
        stage.corrections(),
        &[ResultCorrection {
            match_id: double_loss_match.match_id.clone(),
            previous_result: MatchResult::DoubleLoss,
            replacement_result: replacement,
            reason,
        }]
    );
    assert_eq!(stage.standings().len(), 4);
    for team_id in double_loss_match
        .participants
        .iter()
        .chain(&other_first_match.participants)
    {
        assert!(stage.standings().iter().any(|row| &row.team_id == team_id));
    }
    // 一比零的獨立 literal；不能留下原 DL 負場或取消交手的假勝／棄賽分。
    for row in stage.standings() {
        let expected = if row.team_id == winner_id {
            (1, 0, 0, 1, 0, 1, 3)
        } else if row.team_id == loser_id {
            (0, 0, 1, 0, 1, -1, 0)
        } else {
            (0, 0, 0, 0, 0, 0, 0)
        };
        assert_eq!(
            (
                row.wins,
                row.draws,
                row.losses,
                row.game_wins,
                row.game_losses,
                row.net_game_wins,
                row.match_points,
            ),
            expected
        );
    }
    let corrected_standings = stage.standings().to_vec();
    let correction_audit = stage.corrections().to_vec();
    assert_eq!(stage.last_schedule_error(), None);
    assert!(stage.superseded_rounds().is_empty());

    // 只驗純 core 的公開 start seam，不把預生成配對當作 DO 房間已可開賽。
    let next_match = &original_future_matches[0];
    assert_eq!(
        tournament.start_match(&stage_id, &next_match.match_id),
        Ok(())
    );
    let stage = tournament.stage(&stage_id).unwrap();
    let started = public_match(stage, &next_match.match_id);
    assert_eq!(started.status, MatchStatus::InProgress);
    assert!(started.started);
    assert_eq!(started.result, None);
    assert_eq!(started.cancellation_reason, None);
    assert_eq!(started.participants, next_match.participants);
    for original in original_future_matches.iter().skip(1) {
        assert_eq!(&public_match(stage, &original.match_id), original);
    }
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(
        public_match(stage, &other_first_match.match_id),
        other_first_match
    );
    assert_eq!(
        public_match(stage, &double_loss_match.match_id),
        corrected_match
    );
    assert_eq!(stage.standings(), corrected_standings.as_slice());
    assert_eq!(stage.corrections(), correction_audit.as_slice());
    assert_eq!(stage.rounds().len(), 3);
    let final_ids: BTreeSet<_> = stage
        .rounds()
        .iter()
        .flat_map(|round| &round.matches)
        .map(|scheduled| scheduled.match_id.clone())
        .collect();
    assert_eq!(final_ids, original_ids);
    assert!(stage.superseded_rounds().is_empty());
}
