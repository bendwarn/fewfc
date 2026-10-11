use fewfc::competition_clock::ClockConfig;
use fewfc::competition_match::MatchFormat;
use fewfc::tournament::{
    DrawCredit, MatchPoints, MatchResult, MatchStatus, RoundKind, SeededTeam, StageConfig, StageId,
    StageStatus, SwissStandingPolicy, SwissTieBreaker, TeamId, Tournament, TournamentConfig,
    TournamentFormat, UpdateOutcome,
};

#[test]
fn swiss_double_loss_exits_both_teams_before_second_round_and_preserves_history() {
    let stage_id = StageId::from("swiss-double-loss-exit");
    let config = StageConfig {
        stage_id: stage_id.clone(),
        format: TournamentFormat::Swiss,
        teams: (1..=6)
            .map(|seed| SeededTeam {
                team_id: TeamId::from(format!("swiss-exit-team-{seed}")),
                seed,
            })
            .collect(),
        random_seed: 7,
        match_format: MatchFormat::FixedGames(1),
        clock_config: ClockConfig::default(),
        swiss_rounds: Some(2),
        swiss_policy: Some(SwissStandingPolicy {
            match_points: MatchPoints {
                win: 3,
                draw: 1,
                loss: 0,
                bye: 3,
            },
            draw_credit: DrawCredit::new(1, 2).expect("半勝和局額度必須是合法明示政策"),
            opponent_win_rate_floor_percent: None,
            opponent_win_rate_cap_percent: None,
            exclude_byes_from_opponent_win_rate: true,
            tie_breakers: vec![SwissTieBreaker::SeedOrder],
        }),
    };
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "swiss-two-round-double-loss-exit".into(),
        stage: config.clone(),
    })
    .expect("六隊、兩輪、明示 Swiss policy 與 fixedGames(1) 必須能公開建立");

    assert_eq!(tournament.stages().len(), 1);
    let initial_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(initial_stage.config(), &config);
    assert_eq!(initial_stage.status(), StageStatus::Active);
    assert_eq!(initial_stage.last_schedule_error(), None);
    assert_eq!(initial_stage.rounds().len(), 1);
    let original_first_round = initial_stage.rounds()[0].clone();
    assert_eq!(original_first_round.round_number, 1);
    assert_eq!(original_first_round.kind, RoundKind::Main);
    assert_eq!(original_first_round.matches.len(), 3);
    for match_ in &original_first_round.matches {
        assert_eq!(match_.stage_id, stage_id);
        assert_eq!(match_.round_number, 1);
        assert_eq!(match_.kind, RoundKind::Main);
        assert_eq!(match_.participants.len(), 2);
        assert_eq!(match_.status, MatchStatus::Pending);
        assert_eq!(match_.result, None);
        assert!(!match_.started);
        assert_eq!(match_.cancellation_reason, None);
    }
    for team in &config.teams {
        assert_eq!(
            original_first_round
                .matches
                .iter()
                .flat_map(|match_| &match_.participants)
                .filter(|team_id| *team_id == &team.team_id)
                .count(),
            1,
            "六隊各在公開首輪出現一次，不自行重算 seed 配對"
        );
    }

    // 只從真公開三組配對取身分；首組雙負，另兩組各一局正常勝負。
    let original_double_loss = &original_first_round.matches[0];
    let exited_teams = original_double_loss.participants.clone();
    let remaining_teams: Vec<_> = original_first_round.matches[1..]
        .iter()
        .flat_map(|match_| match_.participants.iter().cloned())
        .collect();
    assert_eq!(remaining_teams.len(), 4);
    tournament
        .start_match(&stage_id, &original_double_loss.match_id)
        .expect("Swiss 雙負也必須先公開真開賽");
    let started = &tournament.stage(&stage_id).unwrap().rounds()[0].matches[0];
    assert_eq!(started.match_id, original_double_loss.match_id);
    assert_eq!(started.participants, exited_teams);
    assert_eq!(started.status, MatchStatus::InProgress);
    assert!(started.started);
    assert_eq!(started.result, None);
    assert_eq!(
        tournament
            .submit_result(
                &stage_id,
                &original_double_loss.match_id,
                MatchResult::DoubleLoss,
            )
            .expect("合法首輪雙負必須先正常記錄原結果"),
        UpdateOutcome::Applied
    );
    let after_double_loss = tournament.stage(&stage_id).unwrap();
    assert_eq!(after_double_loss.status(), StageStatus::Active);
    assert_eq!(after_double_loss.last_schedule_error(), None);
    assert_eq!(after_double_loss.rounds().len(), 1);
    assert_eq!(
        &after_double_loss.rounds()[0].matches[1..],
        &original_first_round.matches[1..],
        "未回報的另外兩組保持原 Pending／未 start／無結果事實"
    );
    let recorded_double_loss = after_double_loss.rounds()[0].matches[0].clone();
    assert_eq!(recorded_double_loss.match_id, original_double_loss.match_id);
    assert_eq!(recorded_double_loss.stage_id, original_double_loss.stage_id);
    assert_eq!(
        recorded_double_loss.round_number,
        original_double_loss.round_number
    );
    assert_eq!(recorded_double_loss.sequence, original_double_loss.sequence);
    assert_eq!(recorded_double_loss.kind, original_double_loss.kind);
    assert_eq!(recorded_double_loss.participants, exited_teams);
    assert_eq!(recorded_double_loss.status, MatchStatus::Completed);
    assert!(recorded_double_loss.started);
    assert_eq!(recorded_double_loss.result, Some(MatchResult::DoubleLoss));
    assert_eq!(recorded_double_loss.cancellation_reason, None);
    for team_id in &exited_teams {
        let row = after_double_loss
            .standings()
            .iter()
            .find(|row| &row.team_id == team_id)
            .unwrap();
        assert_eq!(row.losses, 1);
        assert_eq!(row.wins, 0);
        assert_eq!(row.draws, 0);
        assert_eq!(row.match_points, 0);
        assert_eq!(row.game_wins, 0);
        assert_eq!(row.game_losses, 0);
        assert_eq!(row.net_game_wins, 0);
    }

    for index in 1..3 {
        let original = &original_first_round.matches[index];
        tournament
            .start_match(&stage_id, &original.match_id)
            .expect("另兩組首輪都必須經公開真開賽");
        let started = &tournament.stage(&stage_id).unwrap().rounds()[0].matches[index];
        assert_eq!(started.match_id, original.match_id);
        assert_eq!(started.participants, original.participants);
        assert_eq!(started.status, MatchStatus::InProgress);
        assert!(started.started);
        assert_eq!(started.result, None);
        assert_eq!(
            tournament
                .submit_result(
                    &stage_id,
                    &original.match_id,
                    MatchResult::win(
                        original.participants[0].clone(),
                        original.participants[1].clone(),
                    ),
                )
                .expect("fixedGames(1) 正常一局勝負必須成功回報"),
            UpdateOutcome::Applied
        );
        let stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(stage.status(), StageStatus::Active);
        assert_eq!(stage.last_schedule_error(), None);
        assert_eq!(stage.rounds()[0].matches[0], recorded_double_loss);
        if index == 1 {
            assert_eq!(stage.rounds().len(), 1, "前輪未全閉合不得建第二輪");
            assert_eq!(stage.rounds()[0].matches[2], original_first_round.matches[2]);
        }
    }

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.rounds().len(), 2);
    let completed_first_round = stage.rounds()[0].clone();
    assert_eq!(completed_first_round.round_number, 1);
    assert_eq!(completed_first_round.kind, RoundKind::Main);
    assert_eq!(completed_first_round.matches.len(), 3);
    assert_eq!(completed_first_round.matches[0], recorded_double_loss);
    for (completed, original) in completed_first_round
        .matches
        .iter()
        .zip(&original_first_round.matches)
        .skip(1)
    {
        assert_eq!(completed.match_id, original.match_id);
        assert_eq!(completed.stage_id, original.stage_id);
        assert_eq!(completed.round_number, original.round_number);
        assert_eq!(completed.sequence, original.sequence);
        assert_eq!(completed.kind, original.kind);
        assert_eq!(completed.participants, original.participants);
        assert_eq!(completed.status, MatchStatus::Completed);
        assert!(completed.started);
        assert_eq!(
            completed.result,
            Some(MatchResult::Win {
                winner_id: original.participants[0].clone(),
                loser_id: original.participants[1].clone(),
                winner_games: 1,
                loser_games: 0,
            })
        );
        assert_eq!(completed.cancellation_reason, None);
        let winner_row = stage
            .standings()
            .iter()
            .find(|row| row.team_id == original.participants[0])
            .unwrap();
        assert_eq!(winner_row.wins, 1);
        assert_eq!(winner_row.losses, 0);
        assert_eq!(winner_row.draws, 0);
        assert_eq!(winner_row.match_points, 3);
        assert_eq!(winner_row.game_wins, 1);
        assert_eq!(winner_row.game_losses, 0);
        let loser_row = stage
            .standings()
            .iter()
            .find(|row| row.team_id == original.participants[1])
            .unwrap();
        assert_eq!(loser_row.wins, 0);
        assert_eq!(loser_row.losses, 1);
        assert_eq!(loser_row.draws, 0);
        assert_eq!(loser_row.match_points, 0);
        assert_eq!(loser_row.game_wins, 0);
        assert_eq!(loser_row.game_losses, 1);
    }
    let second_round = stage.rounds()[1].clone();
    assert_eq!(second_round.round_number, 2);
    assert_eq!(second_round.kind, RoundKind::Main);

    // 第一個新政策 target：雙負兩隊退出，不得再被配入真正第二輪。
    assert!(
        second_round.matches.iter().all(|match_| {
            exited_teams
                .iter()
                .all(|team_id| !match_.participants.contains(team_id))
        }),
        "Swiss 雙負兩隊必須退出本階段，第二輪不能重新配入"
    );
    assert_eq!(
        second_round.matches.len(), 2,
        "四個未雙負隊伍只配兩組真交手"
    );
    for team_id in &remaining_teams {
        assert_eq!(
            second_round
                .matches
                .iter()
                .flat_map(|match_| &match_.participants)
                .filter(|participant| *participant == team_id)
                .count(),
            1,
            "四個合法剩餘隊伍各且只出現一次"
        );
    }
    for match_ in &second_round.matches {
        assert_eq!(match_.stage_id, stage_id);
        assert_eq!(match_.round_number, 2);
        assert_eq!(match_.kind, RoundKind::Main);
        assert_eq!(match_.participants.len(), 2);
        assert_eq!(match_.status, MatchStatus::Pending);
        assert!(!match_.started);
        assert_eq!(match_.result, None);
        assert_eq!(match_.cancellation_reason, None);
        assert!(
            match_
                .participants
                .iter()
                .all(|team_id| remaining_teams.contains(team_id))
        );
        // 兩個原正常配對各取一隊：只要求合法跨對手，不固定配對演算法的選擇。
        for original in &original_first_round.matches[1..] {
            assert_eq!(
                match_
                    .participants
                    .iter()
                    .filter(|team_id| original.participants.contains(team_id))
                    .count(),
                1,
                "第二輪不能重複任一首輪對手，也不造 Bye 或 Forfeit"
            );
        }
    }

    // 只 roundtrip 真 public 操作所得的第二輪現況；退出的內部旗標不固定。
    let snapshot = serde_json::to_value(&tournament).unwrap();
    let mut restored: Tournament = serde_json::from_value(snapshot.clone())
        .expect("Swiss 雙負歷史與真第二輪 current Tournament 必須能還原");
    assert_eq!(serde_json::to_value(&restored).unwrap(), snapshot);
    let restored_stage = restored.stage(&stage_id).unwrap();
    assert_eq!(restored_stage.status(), StageStatus::Active);
    assert_eq!(restored_stage.last_schedule_error(), None);
    assert_eq!(&restored_stage.rounds()[0], &completed_first_round);
    assert_eq!(&restored_stage.rounds()[1], &second_round);
    assert_eq!(
        restored
            .submit_result(
                &stage_id,
                &original_double_loss.match_id,
                MatchResult::DoubleLoss,
            )
            .expect("還原後重送原雙負不得再加一負或改變第二輪"),
        UpdateOutcome::AlreadyRecorded
    );
    assert_eq!(serde_json::to_value(&restored).unwrap(), snapshot);
    tournament = restored;

    // 沿同一 trace 真開賽及完成第二輪，不能用一輪即結束的弱覆蓋代替。
    for (index, original) in second_round.matches.iter().enumerate() {
        tournament
            .start_match(&stage_id, &original.match_id)
            .expect("第二輪兩組真交手必須各經公開 start_match");
        let started_stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(started_stage.status(), StageStatus::Active);
        assert_eq!(started_stage.last_schedule_error(), None);
        assert_eq!(started_stage.rounds().len(), 2);
        assert_eq!(&started_stage.rounds()[0], &completed_first_round);
        let started = &started_stage.rounds()[1].matches[index];
        assert_eq!(started.match_id, original.match_id);
        assert_eq!(started.participants, original.participants);
        assert_eq!(started.status, MatchStatus::InProgress);
        assert!(started.started);
        assert_eq!(started.result, None);
        assert_eq!(
            tournament
                .submit_result(
                    &stage_id,
                    &original.match_id,
                    MatchResult::win(
                        original.participants[0].clone(),
                        original.participants[1].clone(),
                    ),
                )
                .expect("第二輪 fixedGames(1) 真勝負必須正常結清"),
            UpdateOutcome::Applied
        );
        let stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(stage.last_schedule_error(), None);
        assert_eq!(stage.rounds().len(), 2);
        assert_eq!(&stage.rounds()[0], &completed_first_round);
        if index == 0 {
            assert_eq!(stage.status(), StageStatus::Active);
            assert_eq!(stage.rounds()[1].matches[1], second_round.matches[1]);
        }
    }

    let finished = tournament.stage(&stage_id).unwrap();
    assert_eq!(finished.status(), StageStatus::Finished);
    assert_eq!(finished.last_schedule_error(), None);
    assert_eq!(finished.config(), &config);
    assert_eq!(
        finished.rounds().len(), 2,
        "只完成公告兩輪，不加假輪次或交手"
    );
    assert!(finished.current_round().is_none());
    assert!(finished.corrections().is_empty());
    assert!(finished.superseded_rounds().is_empty());
    assert_eq!(&finished.rounds()[0], &completed_first_round);
    assert_eq!(finished.rounds()[1].round_number, 2);
    assert_eq!(finished.rounds()[1].kind, RoundKind::Main);
    assert_eq!(finished.rounds()[1].matches.len(), 2);
    for (completed, original) in finished.rounds()[1]
        .matches
        .iter()
        .zip(&second_round.matches)
    {
        assert_eq!(completed.match_id, original.match_id);
        assert_eq!(completed.stage_id, original.stage_id);
        assert_eq!(completed.round_number, original.round_number);
        assert_eq!(completed.sequence, original.sequence);
        assert_eq!(completed.kind, original.kind);
        assert_eq!(completed.participants, original.participants);
        assert_eq!(completed.status, MatchStatus::Completed);
        assert!(completed.started);
        assert_eq!(
            completed.result,
            Some(MatchResult::Win {
                winner_id: original.participants[0].clone(),
                loser_id: original.participants[1].clone(),
                winner_games: 1,
                loser_games: 0,
            })
        );
        assert_eq!(completed.cancellation_reason, None);
    }
    assert_eq!(finished.standings().len(), 6);
    for team in &config.teams {
        assert_eq!(
            finished
                .standings()
                .iter()
                .filter(|row| row.team_id == team.team_id)
                .count(),
            1,
            "退出仍保留原參賽隊伍與真實統計"
        );
    }
    for team_id in &exited_teams {
        assert!(finished.rounds().iter().skip(1).all(|round| {
            round
                .matches
                .iter()
                .all(|match_| !match_.participants.contains(team_id))
        }));
        let row = finished
            .standings()
            .iter()
            .find(|row| &row.team_id == team_id)
            .unwrap();
        assert_eq!(row.losses, 1);
        assert_eq!(row.wins, 0);
        assert_eq!(row.draws, 0);
        assert_eq!(row.match_points, 0);
        assert_eq!(row.game_wins, 0);
        assert_eq!(row.game_losses, 0);
        assert_eq!(row.net_game_wins, 0);
    }
    // 四場真 Win(1–0) 與一場雙負的已知 literal 總帳，不猜 winner 或並列名次。
    let standings = finished.standings();
    assert_eq!(standings.iter().map(|row| row.wins).sum::<u32>(), 4);
    assert_eq!(standings.iter().map(|row| row.losses).sum::<u32>(), 6);
    assert_eq!(standings.iter().map(|row| row.draws).sum::<u32>(), 0);
    assert_eq!(standings.iter().map(|row| row.match_points).sum::<u32>(), 12);
    assert_eq!(standings.iter().map(|row| row.game_wins).sum::<u32>(), 4);
    assert_eq!(standings.iter().map(|row| row.game_losses).sum::<u32>(), 4);
}
