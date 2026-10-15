use fewfc::competition_clock::ClockConfig;
use fewfc::competition_match::MatchFormat;
use fewfc::tournament::{
    MatchPurpose, MatchResult, MatchStatus, RoundKind, SeededTeam, StageConfig, StageId,
    StageStatus, TeamId, Tournament, TournamentConfig, TournamentFormat, UpdateOutcome,
};

#[test]
fn single_elimination_two_team_final_double_loss_finishes_without_champion() {
    let stage_id = StageId::from("single-final");
    let team_a = TeamId::from("team-a");
    let team_b = TeamId::from("team-b");
    let config = StageConfig {
        stage_id: stage_id.clone(),
        format: TournamentFormat::SingleElimination,
        teams: vec![
            SeededTeam {
                team_id: team_a.clone(),
                seed: 1,
            },
            SeededTeam {
                team_id: team_b.clone(),
                seed: 2,
            },
        ],
        random_seed: 7,
        match_format: MatchFormat::FirstToWins(1),
        clock_config: ClockConfig::default(),
        swiss_rounds: None,
        swiss_policy: None,
    };
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "single-final-double-loss".into(),
        stage: config.clone(),
    })
    .expect("兩隊單敗先勝一局設定必須能建立真正決賽");

    // 先鎖住真實公開配對與開賽前提，不能把設定失敗當成政策 red。
    assert_eq!(tournament.stages().len(), 1);
    let initial_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(initial_stage.config(), &config);
    assert_eq!(initial_stage.status(), StageStatus::Active);
    assert_eq!(initial_stage.last_schedule_error(), None);
    assert_eq!(initial_stage.rounds().len(), 1);
    let initial_round = &initial_stage.rounds()[0];
    assert_eq!(initial_round.round_number, 1);
    assert_eq!(initial_round.kind, RoundKind::Main);
    assert_eq!(initial_round.matches.len(), 1);
    let original_final = initial_round.matches[0].clone();
    assert_eq!(original_final.stage_id, stage_id);
    assert_eq!(original_final.round_number, 1);
    assert_eq!(original_final.kind, RoundKind::Main);
    assert_eq!(original_final.participants.len(), 2);
    assert!(original_final.participants.contains(&team_a));
    assert!(original_final.participants.contains(&team_b));
    assert_eq!(original_final.status, MatchStatus::Pending);
    assert_eq!(original_final.result, None);
    assert!(!original_final.started);
    assert_eq!(original_final.cancellation_reason, None);

    tournament
        .start_match(&stage_id, &original_final.match_id)
        .expect("公開開賽必須先成功，決賽不能由手造 Stage 或 snapshot 代替");
    let started_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(started_stage.status(), StageStatus::Active);
    assert_eq!(started_stage.rounds().len(), 1);
    assert_eq!(started_stage.rounds()[0].matches.len(), 1);
    let started_final = &started_stage.rounds()[0].matches[0];
    assert_eq!(started_final.match_id, original_final.match_id);
    assert_eq!(started_final.participants, original_final.participants);
    assert_eq!(started_final.status, MatchStatus::InProgress);
    assert!(started_final.started);
    assert_eq!(started_final.result, None);
    assert_eq!(started_stage.last_schedule_error(), None);

    let outcome = tournament
        .submit_result(&stage_id, &original_final.match_id, MatchResult::DoubleLoss)
        .expect("合法已開賽決賽必須保留雙負結果");
    // 新政策第一個 oracle：決賽雙負正常收尾，不留成需要裁判的排程錯誤。
    assert_eq!(outcome, UpdateOutcome::Applied);

    assert_eq!(tournament.stages().len(), 1);
    assert_eq!(tournament.tournament_id(), "single-final-double-loss");
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.config(), &config);
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), None);
    assert_eq!(stage.last_schedule_error(), None);
    assert!(stage.current_round().is_none());
    assert!(stage.corrections().is_empty());
    assert!(stage.superseded_rounds().is_empty());
    assert_eq!(stage.rounds().len(), 1, "不新增決勝交手或假輪次");
    let final_round = &stage.rounds()[0];
    assert_eq!(final_round.round_number, original_final.round_number);
    assert_eq!(final_round.kind, RoundKind::Main);
    assert_eq!(final_round.matches.len(), 1);
    let final_match = &final_round.matches[0];
    assert_eq!(final_match.match_id, original_final.match_id);
    assert_eq!(final_match.stage_id, original_final.stage_id);
    assert_eq!(final_match.round_number, original_final.round_number);
    assert_eq!(final_match.sequence, original_final.sequence);
    assert_eq!(final_match.kind, original_final.kind);
    assert_eq!(final_match.participants, original_final.participants);
    assert_eq!(final_match.status, MatchStatus::Completed);
    assert!(final_match.started);
    assert_eq!(final_match.cancellation_reason, None);
    assert_eq!(final_match.result, Some(MatchResult::DoubleLoss));

    let serialized_stage = serde_json::to_value(stage).unwrap();
    assert_eq!(serialized_stage["status"], serde_json::json!("finished"));
    assert_eq!(
        serialized_stage["lastScheduleError"],
        serde_json::Value::Null
    );
    assert_eq!(
        serialized_stage["rounds"][0]["matches"][0]["result"],
        serde_json::json!({ "type": "doubleLoss" }),
        "原結果保持雙負，不改寫成勝者、和局或附加假勝局"
    );

    // 只核對雙負本身能確定的統計，不預設獎牌名次或一般退賽旗標。
    assert_eq!(stage.standings().len(), 2);
    for team_id in [&team_a, &team_b] {
        let standing = stage
            .standings()
            .iter()
            .find(|standing| &standing.team_id == team_id)
            .expect("原參賽隊伍的雙負統計必須保留");
        assert_eq!(standing.losses, 1);
        assert_eq!(standing.wins, 0);
        assert_eq!(standing.draws, 0);
        assert_eq!(standing.game_wins, 0);
    }
}

#[test]
fn single_elimination_final_double_loss_leaves_both_finalists_unranked() {
    let stage_id = StageId::from("vacant-final");
    let team_a = TeamId::from("finalist-a");
    let team_b = TeamId::from("finalist-b");
    let config = StageConfig {
        stage_id: stage_id.clone(),
        format: TournamentFormat::SingleElimination,
        teams: vec![
            SeededTeam {
                team_id: team_a.clone(),
                seed: 1,
            },
            SeededTeam {
                team_id: team_b.clone(),
                seed: 2,
            },
        ],
        random_seed: 7,
        match_format: MatchFormat::FirstToWins(1),
        clock_config: ClockConfig::default(),
        swiss_rounds: None,
        swiss_policy: None,
    };
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "final-vacancy-ranks".into(),
        stage: config.clone(),
    })
    .expect("合法兩隊單敗必須先建立真正決賽");

    // 先經公開配對與開賽，不能手造 snapshot 或把前置失敗當成 rank 政策 red。
    assert_eq!(tournament.stages().len(), 1);
    let initial_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(initial_stage.config(), &config);
    assert_eq!(initial_stage.status(), StageStatus::Active);
    assert_eq!(initial_stage.last_schedule_error(), None);
    assert_eq!(initial_stage.rounds().len(), 1);
    assert_eq!(initial_stage.rounds()[0].kind, RoundKind::Main);
    assert_eq!(initial_stage.rounds()[0].matches.len(), 1);
    let original_final = initial_stage.rounds()[0].matches[0].clone();
    assert_eq!(original_final.stage_id, stage_id);
    assert_eq!(original_final.kind, RoundKind::Main);
    assert_eq!(original_final.participants.len(), 2);
    assert!(original_final.participants.contains(&team_a));
    assert!(original_final.participants.contains(&team_b));
    assert_eq!(original_final.status, MatchStatus::Pending);
    assert_eq!(original_final.result, None);
    assert!(!original_final.started);
    assert_eq!(original_final.cancellation_reason, None);

    tournament
        .start_match(&stage_id, &original_final.match_id)
        .expect("決賽必須透過公開 start_match 開賽");
    let started_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(started_stage.status(), StageStatus::Active);
    assert_eq!(started_stage.last_schedule_error(), None);
    assert_eq!(started_stage.rounds().len(), 1);
    assert_eq!(started_stage.rounds()[0].matches.len(), 1);
    let started_final = &started_stage.rounds()[0].matches[0];
    assert_eq!(started_final.match_id, original_final.match_id);
    assert_eq!(started_final.participants, original_final.participants);
    assert_eq!(started_final.status, MatchStatus::InProgress);
    assert!(started_final.started);
    assert_eq!(started_final.result, None);

    let outcome = tournament
        .submit_result(&stage_id, &original_final.match_id, MatchResult::DoubleLoss)
        .expect("合法決賽雙負必須正常收尾並保留原結果");
    assert_eq!(outcome, UpdateOutcome::Applied);
    assert_eq!(tournament.stages().len(), 1);
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.config(), &config);
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), None);
    assert_eq!(stage.last_schedule_error(), None);
    assert!(stage.current_round().is_none());
    assert!(stage.corrections().is_empty());
    assert!(stage.superseded_rounds().is_empty());
    assert_eq!(stage.rounds().len(), 1);
    assert_eq!(stage.rounds()[0].matches.len(), 1);
    let final_match = &stage.rounds()[0].matches[0];
    assert_eq!(final_match.match_id, original_final.match_id);
    assert_eq!(final_match.stage_id, original_final.stage_id);
    assert_eq!(final_match.round_number, original_final.round_number);
    assert_eq!(final_match.sequence, original_final.sequence);
    assert_eq!(final_match.kind, original_final.kind);
    assert_eq!(final_match.participants, original_final.participants);
    assert_eq!(final_match.status, MatchStatus::Completed);
    assert!(final_match.started);
    assert_eq!(final_match.result, Some(MatchResult::DoubleLoss));
    assert_eq!(final_match.cancellation_reason, None);

    // 兩隊都必須先保留一負及零勝／零和／零勝局，才能抵達新的 rank oracle。
    assert_eq!(stage.standings().len(), 2);
    for team_id in [&team_a, &team_b] {
        let standing = stage
            .standings()
            .iter()
            .find(|standing| &standing.team_id == team_id)
            .expect("兩名原 finalist 的統計必須保留");
        assert_eq!(standing.losses, 1);
        assert_eq!(standing.wins, 0);
        assert_eq!(standing.draws, 0);
        assert_eq!(standing.match_points, 0);
        assert_eq!(standing.game_wins, 0);
        assert_eq!(standing.game_losses, 0);
    }
    let serialized_stage = serde_json::to_value(stage).unwrap();
    assert_eq!(serialized_stage["status"], serde_json::json!("finished"));
    assert_eq!(
        serialized_stage["lastScheduleError"],
        serde_json::Value::Null
    );
    assert_eq!(
        serialized_stage["rounds"][0]["matches"][0]["result"],
        serde_json::json!({ "type": "doubleLoss" })
    );
    let serialized_standings = serialized_stage["standings"].as_array().unwrap();
    assert_eq!(serialized_standings.len(), 2);

    // F1 第一個政策斷言：冠軍空缺不能被自動排名填成冠亞軍或並列第一。
    for team_id in [&team_a, &team_b] {
        let standing = stage
            .standings()
            .iter()
            .find(|standing| &standing.team_id == team_id)
            .unwrap();
        assert_eq!(
            standing.rank, None,
            "決賽雙負 finalist {team_id} 必須無名次"
        );
    }
    for team_id in [&team_a, &team_b] {
        let serialized_standing = serialized_standings
            .iter()
            .find(|row| {
                row.get("teamId").and_then(serde_json::Value::as_str) == Some(team_id.0.as_str())
            })
            .expect("公開 serde standings 必須保留兩名原 finalist");
        assert_eq!(
            serialized_standing.get("rank"),
            Some(&serde_json::Value::Null),
            "finalist {team_id} 的公開 rank 必須是 null，不能缺欄位或造假名次"
        );
    }
}

#[test]
fn single_elimination_four_team_final_double_loss_preserves_stats_after_restore_and_retry() {
    let stage_id = StageId::from("four-team-vacant-final");
    let config = StageConfig {
        stage_id: stage_id.clone(),
        format: TournamentFormat::SingleElimination,
        teams: vec![
            SeededTeam {
                team_id: TeamId::from("prefix-a"),
                seed: 1,
            },
            SeededTeam {
                team_id: TeamId::from("prefix-b"),
                seed: 2,
            },
            SeededTeam {
                team_id: TeamId::from("prefix-c"),
                seed: 3,
            },
            SeededTeam {
                team_id: TeamId::from("prefix-d"),
                seed: 4,
            },
        ],
        random_seed: 7,
        match_format: MatchFormat::FirstToWins(1),
        clock_config: ClockConfig::default(),
        swiss_rounds: None,
        swiss_policy: None,
    };
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "four-team-final-vacancy-roundtrip".into(),
        stage: config.clone(),
    })
    .expect("合法四隊單敗必須透過公開 new 建立兩場半決賽");

    let initial_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(initial_stage.config(), &config);
    assert_eq!(initial_stage.status(), StageStatus::Active);
    assert_eq!(initial_stage.last_schedule_error(), None);
    assert_eq!(initial_stage.rounds().len(), 1);
    let original_semifinals = initial_stage.rounds()[0].clone();
    assert_eq!(original_semifinals.round_number, 1);
    assert_eq!(original_semifinals.kind, RoundKind::Main);
    assert_eq!(original_semifinals.matches.len(), 2);
    for semifinal in &original_semifinals.matches {
        assert_eq!(semifinal.stage_id, stage_id);
        assert_eq!(semifinal.round_number, 1);
        assert_eq!(semifinal.kind, RoundKind::Main);
        assert_eq!(semifinal.participants.len(), 2);
        assert_eq!(semifinal.status, MatchStatus::Pending);
        assert_eq!(semifinal.result, None);
        assert!(!semifinal.started);
        assert_eq!(semifinal.cancellation_reason, None);
    }
    for team in &config.teams {
        assert_eq!(
            original_semifinals
                .matches
                .iter()
                .flat_map(|semifinal| &semifinal.participants)
                .filter(|team_id| *team_id == &team.team_id)
                .count(),
            1,
            "四個原參賽隊伍都必須且只能出現一次"
        );
    }

    // 從真實公開配對選定結果；一場實際勝局與一場棄權讓前輪統計不同。
    let win_semifinal = &original_semifinals.matches[0];
    let forfeit_semifinal = &original_semifinals.matches[1];
    let win_finalist = win_semifinal.participants[0].clone();
    let win_loser = win_semifinal.participants[1].clone();
    let forfeit_finalist = forfeit_semifinal.participants[0].clone();
    let forfeit_loser = forfeit_semifinal.participants[1].clone();
    let win_result = MatchResult::win(win_finalist.clone(), win_loser.clone());
    let forfeit_result = MatchResult::forfeit(forfeit_finalist.clone(), forfeit_loser.clone());
    for (semifinal, result) in [
        (win_semifinal, &win_result),
        (forfeit_semifinal, &forfeit_result),
    ] {
        tournament
            .start_match(&stage_id, &semifinal.match_id)
            .expect("每場半決賽都必須先經公開 start_match");
        let started = tournament.stage(&stage_id).unwrap().rounds()[0]
            .matches
            .iter()
            .find(|match_| match_.match_id == semifinal.match_id)
            .unwrap();
        assert_eq!(started.participants, semifinal.participants);
        assert_eq!(started.status, MatchStatus::InProgress);
        assert!(started.started);
        assert_eq!(started.result, None);
        assert_eq!(
            tournament
                .submit_result(&stage_id, &semifinal.match_id, (*result).clone())
                .expect("合法半決賽勝負必須正常記錄"),
            UpdateOutcome::Applied
        );
    }

    let next_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(next_stage.status(), StageStatus::Active);
    assert_eq!(next_stage.last_schedule_error(), None);
    assert_eq!(next_stage.rounds().len(), 2);
    let completed_semifinals = next_stage.rounds()[0].clone();
    assert_eq!(
        completed_semifinals.round_number,
        original_semifinals.round_number
    );
    assert_eq!(completed_semifinals.kind, original_semifinals.kind);
    assert_eq!(completed_semifinals.matches.len(), 2);
    for ((completed, original), result) in completed_semifinals
        .matches
        .iter()
        .zip(&original_semifinals.matches)
        .zip([&win_result, &forfeit_result])
    {
        assert_eq!(completed.match_id, original.match_id);
        assert_eq!(completed.stage_id, original.stage_id);
        assert_eq!(completed.round_number, original.round_number);
        assert_eq!(completed.sequence, original.sequence);
        assert_eq!(completed.kind, original.kind);
        assert_eq!(completed.participants, original.participants);
        assert_eq!(completed.status, MatchStatus::Completed);
        assert!(completed.started);
        assert_eq!(completed.result.as_ref(), Some(result));
        assert_eq!(completed.cancellation_reason, None);
    }
    assert_eq!(next_stage.rounds()[1].round_number, 2);
    assert_eq!(next_stage.rounds()[1].kind, RoundKind::Main);
    assert_eq!(next_stage.rounds()[1].matches.len(), 2);
    let original_final = next_stage.rounds()[1]
        .matches
        .iter()
        .find(|match_| match_.purpose == MatchPurpose::Standard)
        .unwrap()
        .clone();
    let original_bronze = next_stage.rounds()[1]
        .matches
        .iter()
        .find(|match_| match_.purpose == MatchPurpose::Bronze)
        .unwrap()
        .clone();
    assert_eq!(original_final.stage_id, stage_id);
    assert_eq!(original_final.round_number, 2);
    assert_eq!(original_final.kind, RoundKind::Main);
    assert_ne!(original_final.match_id, win_semifinal.match_id);
    assert_ne!(original_final.match_id, forfeit_semifinal.match_id);
    assert_eq!(original_final.participants.len(), 2);
    assert!(original_final.participants.contains(&win_finalist));
    assert!(original_final.participants.contains(&forfeit_finalist));
    assert_eq!(original_final.status, MatchStatus::Pending);
    assert!(!original_final.started);
    assert_eq!(original_final.result, None);
    assert_eq!(original_final.cancellation_reason, None);

    tournament
        .start_match(&stage_id, &original_final.match_id)
        .expect("兩半決賽真實結清後，決賽必須透過公開 start_match");
    let started_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(started_stage.rounds().len(), 2);
    assert_eq!(&started_stage.rounds()[0], &completed_semifinals);
    let started_final = started_stage.rounds()[1]
        .matches
        .iter()
        .find(|match_| match_.purpose == MatchPurpose::Standard)
        .unwrap();
    assert_eq!(started_final.match_id, original_final.match_id);
    assert_eq!(started_final.participants, original_final.participants);
    assert_eq!(started_final.status, MatchStatus::InProgress);
    assert!(started_final.started);
    assert_eq!(started_final.result, None);
    assert_eq!(
        tournament
            .submit_result(&stage_id, &original_final.match_id, MatchResult::DoubleLoss)
            .expect("有合法四隊 prefix 的真正決賽雙負必須正常收尾"),
        UpdateOutcome::Applied
    );

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(tournament.stages().len(), 1);
    assert_eq!(stage.config(), &config);
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(stage.winner(), None);
    assert_eq!(stage.last_schedule_error(), None);
    assert!(stage.current_round().is_some());
    assert!(stage.corrections().is_empty());
    assert!(stage.superseded_rounds().is_empty());
    assert_eq!(stage.rounds().len(), 2, "不新增決勝交手或假輪次");
    assert_eq!(&stage.rounds()[0], &completed_semifinals);
    assert_eq!(stage.rounds()[1].matches.len(), 2);
    let final_match = stage.rounds()[1]
        .matches
        .iter()
        .find(|match_| match_.purpose == MatchPurpose::Standard)
        .unwrap();
    let bronze_match = stage.rounds()[1]
        .matches
        .iter()
        .find(|match_| match_.purpose == MatchPurpose::Bronze)
        .unwrap();
    assert_eq!(bronze_match.match_id, original_bronze.match_id);
    assert_eq!(bronze_match.participants, original_bronze.participants);
    assert_eq!(bronze_match.status, MatchStatus::Pending);
    assert_eq!(bronze_match.result, None);
    assert_eq!(final_match.match_id, original_final.match_id);
    assert_eq!(final_match.stage_id, original_final.stage_id);
    assert_eq!(final_match.round_number, original_final.round_number);
    assert_eq!(final_match.sequence, original_final.sequence);
    assert_eq!(final_match.kind, original_final.kind);
    assert_eq!(final_match.participants, original_final.participants);
    assert_eq!(final_match.status, MatchStatus::Completed);
    assert!(final_match.started);
    assert_eq!(final_match.result, Some(MatchResult::DoubleLoss));
    assert_eq!(final_match.cancellation_reason, None);

    // Literal 統計來自已提交的一局勝負、棄權與決賽各一負，不清除前輪事實。
    assert_eq!(stage.standings().len(), 4);
    let win_row = stage
        .standings()
        .iter()
        .find(|row| row.team_id == win_finalist)
        .unwrap();
    assert_eq!(win_row.wins, 1);
    assert_eq!(win_row.losses, 1);
    assert_eq!(win_row.draws, 0);
    assert_eq!(win_row.match_points, 3);
    assert_eq!(win_row.game_wins, 1);
    assert_eq!(win_row.game_losses, 0);
    assert_eq!(win_row.net_game_wins, 1);
    let forfeit_row = stage
        .standings()
        .iter()
        .find(|row| row.team_id == forfeit_finalist)
        .unwrap();
    assert_eq!(forfeit_row.wins, 1);
    assert_eq!(forfeit_row.losses, 1);
    assert_eq!(forfeit_row.draws, 0);
    assert_eq!(forfeit_row.match_points, 3);
    assert_eq!(forfeit_row.game_wins, 0);
    assert_eq!(forfeit_row.game_losses, 0);
    assert_eq!(forfeit_row.net_game_wins, 0);
    let win_loser_row = stage
        .standings()
        .iter()
        .find(|row| row.team_id == win_loser)
        .unwrap();
    assert_eq!(win_loser_row.wins, 0);
    assert_eq!(win_loser_row.losses, 1);
    assert_eq!(win_loser_row.draws, 0);
    assert_eq!(win_loser_row.match_points, 0);
    assert_eq!(win_loser_row.game_wins, 0);
    assert_eq!(win_loser_row.game_losses, 1);
    assert_eq!(win_loser_row.net_game_wins, -1);
    let forfeit_loser_row = stage
        .standings()
        .iter()
        .find(|row| row.team_id == forfeit_loser)
        .unwrap();
    assert_eq!(forfeit_loser_row.wins, 0);
    assert_eq!(forfeit_loser_row.losses, 1);
    assert_eq!(forfeit_loser_row.draws, 0);
    assert_eq!(forfeit_loser_row.match_points, 0);
    assert_eq!(forfeit_loser_row.game_wins, 0);
    assert_eq!(forfeit_loser_row.game_losses, 0);
    assert_eq!(forfeit_loser_row.net_game_wins, 0);
    // 只序列化剛由公共操作建立的現況，不手造 snapshot 或模擬舊資料遷移。
    let snapshot = serde_json::to_value(&tournament).unwrap();
    assert_eq!(snapshot["stages"][0]["status"], "active");
    assert_eq!(
        snapshot["stages"][0]["rounds"][1]["matches"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    let mut restored: Tournament = serde_json::from_value(snapshot.clone())
        .expect("真實四隊 prefix 與決賽雙負的 current Tournament 必須能 serde 還原");
    assert_eq!(serde_json::to_value(&restored).unwrap(), snapshot);
    let restored_stage = restored.stage(&stage_id).unwrap();
    assert_eq!(restored_stage.status(), StageStatus::Active);
    assert_eq!(restored_stage.winner(), None);
    assert_eq!(restored_stage.last_schedule_error(), None);
    let restored_round = restored_stage.current_round().unwrap();
    assert_eq!(restored_round.matches.len(), 2);
    assert_eq!(
        restored_round
            .matches
            .iter()
            .find(|match_| match_.purpose == MatchPurpose::Bronze)
            .unwrap()
            .status,
        MatchStatus::Pending
    );
    assert_eq!(
        restored
            .submit_result(&stage_id, &original_final.match_id, MatchResult::DoubleLoss)
            .expect("current serde 還原後重送原決賽雙負必須正常去重"),
        UpdateOutcome::AlreadyRecorded
    );
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        snapshot,
        "同結果 retry 不得更動任何原交手、開賽事實、統計或待賽季軍狀態"
    );
}

#[test]
fn single_elimination_nonfinal_double_loss_preserves_vacant_bracket_position() {
    let stage_id = StageId::from("single-vacant-branch");
    let config = StageConfig {
        stage_id: stage_id.clone(),
        format: TournamentFormat::SingleElimination,
        teams: (1..=8)
            .map(|seed| SeededTeam {
                team_id: TeamId::from(format!("vacancy-team-{seed}")),
                seed,
            })
            .collect(),
        random_seed: 7,
        match_format: MatchFormat::FirstToWins(1),
        clock_config: ClockConfig::default(),
        swiss_rounds: None,
        swiss_policy: None,
    };
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "single-vacancy-position".into(),
        stage: config.clone(),
    })
    .expect("合法八隊單敗必須透過公開 new 建立四場首輪交手");

    assert_eq!(tournament.stages().len(), 1);
    let initial_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(initial_stage.config(), &config);
    assert_eq!(initial_stage.status(), StageStatus::Active);
    assert_eq!(initial_stage.last_schedule_error(), None);
    assert_eq!(initial_stage.rounds().len(), 1);
    let original_first_round = initial_stage.rounds()[0].clone();
    assert_eq!(original_first_round.round_number, 1);
    assert_eq!(original_first_round.kind, RoundKind::Main);
    assert_eq!(original_first_round.matches.len(), 4);
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
            "八個原參賽隊伍必須各出現一次，不自行重算 seed 配對"
        );
    }

    // 位置來自原公開四場順序：第一場空缺，第二場勝者仍屬首分支。
    let original_double_loss = &original_first_round.matches[0];
    let exited_teams = original_double_loss.participants.clone();
    let bye_team = original_first_round.matches[1].participants[0].clone();
    let third_winner = original_first_round.matches[2].participants[0].clone();
    let fourth_winner = original_first_round.matches[3].participants[0].clone();
    tournament
        .start_match(&stage_id, &original_double_loss.match_id)
        .expect("非決賽雙負也必須先經公開 start_match 真開賽");
    let started_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(started_stage.status(), StageStatus::Active);
    assert_eq!(started_stage.last_schedule_error(), None);
    assert_eq!(started_stage.rounds().len(), 1);
    assert_eq!(started_stage.rounds()[0].matches.len(), 4);
    let started_double_loss = &started_stage.rounds()[0].matches[0];
    assert_eq!(started_double_loss.match_id, original_double_loss.match_id);
    assert_eq!(started_double_loss.participants, exited_teams);
    assert_eq!(started_double_loss.status, MatchStatus::InProgress);
    assert!(started_double_loss.started);
    assert_eq!(started_double_loss.result, None);

    let outcome = tournament
        .submit_result(
            &stage_id,
            &original_double_loss.match_id,
            MatchResult::DoubleLoss,
        )
        .expect("合法非決賽雙負必須先能提交原結果");
    // 第一個新政策 target：正常 Applied，而非需要裁判的排程錯誤。
    assert_eq!(outcome, UpdateOutcome::Applied);
    let after_double_loss = tournament.stage(&stage_id).unwrap();
    assert_eq!(after_double_loss.last_schedule_error(), None);
    assert_eq!(after_double_loss.status(), StageStatus::Active);
    assert_eq!(
        after_double_loss.rounds().len(),
        1,
        "其餘首輪未決不能建下一輪"
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
    assert_eq!(
        &after_double_loss.rounds()[0].matches[1..],
        &original_first_round.matches[1..]
    );
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
    }

    for index in 1..4 {
        let original = &original_first_round.matches[index];
        tournament
            .start_match(&stage_id, &original.match_id)
            .expect("另三場首輪都必須先公開開賽");
        let started_stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(started_stage.rounds().len(), 1);
        let started = &started_stage.rounds()[0].matches[index];
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
                .expect("首輪合法一局勝負必須正常記錄"),
            UpdateOutcome::Applied
        );
        let stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(stage.last_schedule_error(), None);
        assert_eq!(stage.status(), StageStatus::Active);
        if index < 3 {
            assert_eq!(stage.rounds().len(), 1, "仍有原首輪未決不能提早建下一輪");
            assert_eq!(
                &stage.rounds()[0].matches[index + 1..],
                &original_first_round.matches[index + 1..]
            );
        }
    }

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.rounds().len(), 2);
    let completed_first_round = stage.rounds()[0].clone();
    assert_eq!(completed_first_round.round_number, 1);
    assert_eq!(completed_first_round.kind, RoundKind::Main);
    assert_eq!(completed_first_round.matches.len(), 4);
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
            Some(MatchResult::win(
                original.participants[0].clone(),
                original.participants[1].clone(),
            ))
        );
        assert_eq!(completed.cancellation_reason, None);
    }

    // 不能 compact 三勝者：首位置只給原第二場勝者 Bye，後位置仍為第三對第四。
    let second_round = &stage.rounds()[1];
    assert_eq!(second_round.round_number, 2);
    assert_eq!(second_round.kind, RoundKind::Main);
    assert_eq!(second_round.matches.len(), 2);
    let bye = second_round.matches[0].clone();
    assert_eq!(bye.stage_id, stage_id);
    assert_eq!(bye.round_number, 2);
    assert_eq!(bye.kind, RoundKind::Main);
    assert_eq!(bye.participants, vec![bye_team.clone()]);
    assert_eq!(bye.status, MatchStatus::Completed);
    assert_eq!(
        bye.result,
        Some(MatchResult::Bye {
            team_id: bye_team.clone()
        })
    );
    assert!(!bye.started, "輪空不造真正開賽或對局事實");
    assert_eq!(bye.cancellation_reason, None);
    let original_second_match = second_round.matches[1].clone();
    assert_eq!(original_second_match.stage_id, stage_id);
    assert_eq!(original_second_match.round_number, 2);
    assert_eq!(original_second_match.kind, RoundKind::Main);
    assert_eq!(original_second_match.participants.len(), 2);
    assert!(original_second_match.participants.contains(&third_winner));
    assert!(original_second_match.participants.contains(&fourth_winner));
    assert!(!original_second_match.participants.contains(&bye_team));
    assert_eq!(original_second_match.status, MatchStatus::Pending);
    assert!(!original_second_match.started);
    assert_eq!(original_second_match.result, None);
    assert_eq!(original_second_match.cancellation_reason, None);
    for team_id in &exited_teams {
        assert!(
            second_round
                .matches
                .iter()
                .all(|match_| !match_.participants.contains(team_id))
        );
    }

    // 三個首輪 Win 各只算一真勝；Bye 不加勝場、勝局、積分或額外負場。
    assert_eq!(stage.standings().len(), 8);
    for original in &original_first_round.matches[1..] {
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

    // 沿同一因果 trace 完成剩餘真交手，再讓原 Bye 隊取得真正決賽的一勝。
    tournament
        .start_match(&stage_id, &original_second_match.match_id)
        .expect("另一分支的第二輪真交手必須公開開賽");
    let started_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(started_stage.rounds().len(), 2, "第二輪未決不能先建決賽");
    assert_eq!(started_stage.rounds()[1].matches[0], bye);
    let started_second_match = &started_stage.rounds()[1].matches[1];
    assert_eq!(
        started_second_match.match_id,
        original_second_match.match_id
    );
    assert_eq!(
        started_second_match.participants,
        original_second_match.participants
    );
    assert_eq!(started_second_match.status, MatchStatus::InProgress);
    assert!(started_second_match.started);
    assert_eq!(started_second_match.result, None);
    assert_eq!(
        tournament
            .submit_result(
                &stage_id,
                &original_second_match.match_id,
                MatchResult::win(third_winner.clone(), fourth_winner.clone()),
            )
            .expect("另一分支真勝負正常結清"),
        UpdateOutcome::Applied
    );
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(stage.rounds().len(), 3);
    assert_eq!(&stage.rounds()[0], &completed_first_round);
    assert_eq!(stage.rounds()[1].matches[0], bye);
    let completed_second_round = stage.rounds()[1].clone();
    assert_eq!(stage.rounds()[2].round_number, 3);
    assert_eq!(stage.rounds()[2].kind, RoundKind::Main);
    assert_eq!(stage.rounds()[2].matches.len(), 1);
    let original_final = stage.rounds()[2].matches[0].clone();
    assert_eq!(original_final.participants.len(), 2);
    assert!(original_final.participants.contains(&bye_team));
    assert!(original_final.participants.contains(&third_winner));
    assert_eq!(original_final.status, MatchStatus::Pending);
    assert!(!original_final.started);
    assert_eq!(original_final.result, None);
    tournament
        .start_match(&stage_id, &original_final.match_id)
        .expect("兩分支均結清後決賽必須公開開賽");
    let started_final = &tournament.stage(&stage_id).unwrap().rounds()[2].matches[0];
    assert_eq!(started_final.match_id, original_final.match_id);
    assert_eq!(started_final.participants, original_final.participants);
    assert_eq!(started_final.status, MatchStatus::InProgress);
    assert!(started_final.started);
    assert_eq!(started_final.result, None);
    assert_eq!(
        tournament
            .submit_result(
                &stage_id,
                &original_final.match_id,
                MatchResult::win(bye_team.clone(), third_winner.clone()),
            )
            .expect("原 Bye 隊只能由真決賽 Win 再增加一勝"),
        UpdateOutcome::Applied
    );

    let finished = tournament.stage(&stage_id).unwrap();
    assert_eq!(finished.status(), StageStatus::Finished);
    assert_eq!(finished.winner(), Some(bye_team.clone()));
    assert_eq!(finished.last_schedule_error(), None);
    assert!(finished.current_round().is_none());
    assert!(finished.corrections().is_empty());
    assert!(finished.superseded_rounds().is_empty());
    assert_eq!(finished.rounds().len(), 3);
    assert_eq!(&finished.rounds()[0], &completed_first_round);
    assert_eq!(&finished.rounds()[1], &completed_second_round);
    assert_eq!(finished.rounds()[2].matches.len(), 1);
    let final_match = &finished.rounds()[2].matches[0];
    assert_eq!(final_match.match_id, original_final.match_id);
    assert_eq!(final_match.stage_id, original_final.stage_id);
    assert_eq!(final_match.round_number, original_final.round_number);
    assert_eq!(final_match.sequence, original_final.sequence);
    assert_eq!(final_match.kind, original_final.kind);
    assert_eq!(final_match.participants, original_final.participants);
    assert_eq!(final_match.status, MatchStatus::Completed);
    assert!(final_match.started);
    assert_eq!(
        final_match.result,
        Some(MatchResult::win(bye_team.clone(), third_winner.clone()))
    );
    assert_eq!(final_match.cancellation_reason, None);
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
    }
    let bye_team_row = finished
        .standings()
        .iter()
        .find(|row| row.team_id == bye_team)
        .unwrap();
    assert_eq!(
        bye_team_row.wins, 2,
        "首輪與決賽兩場真勝，不把 Bye 算成第三勝"
    );
    assert_eq!(bye_team_row.losses, 0);
    assert_eq!(bye_team_row.draws, 0);
    assert_eq!(bye_team_row.match_points, 6);
    assert_eq!(bye_team_row.game_wins, 2);
    assert_eq!(bye_team_row.game_losses, 0);
}

#[test]
fn single_elimination_final_double_loss_after_prior_vacancy_preserves_decided_ranks() {
    let stage_id = StageId::from("prior-vacancy-final");
    let config = StageConfig {
        stage_id: stage_id.clone(),
        format: TournamentFormat::SingleElimination,
        teams: (1..=8)
            .map(|seed| SeededTeam {
                team_id: TeamId::from(format!("prior-vacancy-team-{seed}")),
                seed,
            })
            .collect(),
        random_seed: 7,
        match_format: MatchFormat::FirstToWins(1),
        clock_config: ClockConfig::default(),
        swiss_rounds: None,
        swiss_policy: None,
    };
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "prior-vacancy-final-double-loss".into(),
        stage: config.clone(),
    })
    .expect("合法八隊單敗必須先透過公開 new 建立首輪");

    assert_eq!(tournament.stages().len(), 1);
    let initial_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(initial_stage.config(), &config);
    assert_eq!(initial_stage.status(), StageStatus::Active);
    assert_eq!(initial_stage.last_schedule_error(), None);
    assert_eq!(initial_stage.rounds().len(), 1);
    let original_first_round = initial_stage.rounds()[0].clone();
    assert_eq!(original_first_round.round_number, 1);
    assert_eq!(original_first_round.kind, RoundKind::Main);
    assert_eq!(original_first_round.matches.len(), 4);
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
            "只讀原公開配對身分，不自行推算 seed 演算法"
        );
    }

    // 沿既有合法空位路徑：第一場雙負，另三場各有一局真勝。
    let original_double_loss = &original_first_round.matches[0];
    let exited_teams = original_double_loss.participants.clone();
    let bye_team = original_first_round.matches[1].participants[0].clone();
    let third_winner = original_first_round.matches[2].participants[0].clone();
    let fourth_winner = original_first_round.matches[3].participants[0].clone();
    tournament
        .start_match(&stage_id, &original_double_loss.match_id)
        .expect("首輪雙負必須先經公開 start_match 真開賽");
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
            .expect("既有非決賽雙負前置必須正常提交"),
        UpdateOutcome::Applied
    );
    let after_double_loss = tournament.stage(&stage_id).unwrap();
    assert_eq!(after_double_loss.status(), StageStatus::Active);
    assert_eq!(after_double_loss.last_schedule_error(), None);
    assert_eq!(after_double_loss.rounds().len(), 1);
    assert_eq!(
        &after_double_loss.rounds()[0].matches[1..],
        &original_first_round.matches[1..]
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

    for index in 1..4 {
        let original = &original_first_round.matches[index];
        tournament
            .start_match(&stage_id, &original.match_id)
            .expect("另三場首輪都必須公開真開賽");
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
                .expect("首輪真勝負前置必須正常結清"),
            UpdateOutcome::Applied
        );
        let stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(stage.status(), StageStatus::Active);
        assert_eq!(stage.last_schedule_error(), None);
        if index < 3 {
            assert_eq!(stage.rounds().len(), 1, "前輪未決不建立下一輪");
            assert_eq!(
                &stage.rounds()[0].matches[index + 1..],
                &original_first_round.matches[index + 1..]
            );
        }
    }

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.rounds().len(), 2);
    let completed_first_round = stage.rounds()[0].clone();
    assert_eq!(completed_first_round.round_number, 1);
    assert_eq!(completed_first_round.kind, RoundKind::Main);
    assert_eq!(completed_first_round.matches.len(), 4);
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
    }

    // 原首分支的空位只給第二場勝者輪空，不 compact 成另一場真交手。
    let second_round = &stage.rounds()[1];
    assert_eq!(second_round.round_number, 2);
    assert_eq!(second_round.kind, RoundKind::Main);
    assert_eq!(second_round.matches.len(), 2);
    let bye = second_round.matches[0].clone();
    assert_eq!(bye.stage_id, stage_id);
    assert_eq!(bye.round_number, 2);
    assert_eq!(bye.kind, RoundKind::Main);
    assert_eq!(bye.participants, vec![bye_team.clone()]);
    assert_eq!(bye.status, MatchStatus::Completed);
    assert_eq!(
        bye.result,
        Some(MatchResult::Bye {
            team_id: bye_team.clone()
        })
    );
    assert!(!bye.started, "輪空不得假造 start 或對局事實");
    assert_eq!(bye.cancellation_reason, None);
    let original_semifinal = second_round.matches[1].clone();
    assert_eq!(original_semifinal.stage_id, stage_id);
    assert_eq!(original_semifinal.round_number, 2);
    assert_eq!(original_semifinal.kind, RoundKind::Main);
    assert_eq!(original_semifinal.participants.len(), 2);
    assert!(original_semifinal.participants.contains(&third_winner));
    assert!(original_semifinal.participants.contains(&fourth_winner));
    assert_eq!(original_semifinal.status, MatchStatus::Pending);
    assert!(!original_semifinal.started);
    assert_eq!(original_semifinal.result, None);
    assert_eq!(original_semifinal.cancellation_reason, None);
    let bye_row = stage
        .standings()
        .iter()
        .find(|row| row.team_id == bye_team)
        .unwrap();
    assert_eq!(bye_row.wins, 1);
    assert_eq!(bye_row.losses, 0);
    assert_eq!(bye_row.draws, 0);
    assert_eq!(bye_row.match_points, 3);
    assert_eq!(bye_row.game_wins, 1);
    assert_eq!(bye_row.game_losses, 0);
    assert_eq!(bye_row.net_game_wins, 1);

    tournament
        .start_match(&stage_id, &original_semifinal.match_id)
        .expect("另一分支必須真開賽，輪空不呼叫 start_match");
    let started_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(started_stage.rounds().len(), 2);
    assert_eq!(started_stage.rounds()[1].matches[0], bye);
    let started_semifinal = &started_stage.rounds()[1].matches[1];
    assert_eq!(started_semifinal.match_id, original_semifinal.match_id);
    assert_eq!(
        started_semifinal.participants,
        original_semifinal.participants
    );
    assert_eq!(started_semifinal.status, MatchStatus::InProgress);
    assert!(started_semifinal.started);
    assert_eq!(started_semifinal.result, None);
    assert_eq!(
        tournament
            .submit_result(
                &stage_id,
                &original_semifinal.match_id,
                MatchResult::win(third_winner.clone(), fourth_winner.clone()),
            )
            .expect("另一分支真半決賽必須正常結清"),
        UpdateOutcome::Applied
    );
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Active);
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(stage.rounds().len(), 3);
    assert_eq!(&stage.rounds()[0], &completed_first_round);
    assert_eq!(stage.rounds()[1].matches[0], bye);
    let completed_second_round = stage.rounds()[1].clone();
    let completed_semifinal = &completed_second_round.matches[1];
    assert_eq!(completed_semifinal.match_id, original_semifinal.match_id);
    assert_eq!(completed_semifinal.stage_id, original_semifinal.stage_id);
    assert_eq!(
        completed_semifinal.round_number,
        original_semifinal.round_number
    );
    assert_eq!(completed_semifinal.sequence, original_semifinal.sequence);
    assert_eq!(completed_semifinal.kind, original_semifinal.kind);
    assert_eq!(
        completed_semifinal.participants,
        original_semifinal.participants
    );
    assert_eq!(completed_semifinal.status, MatchStatus::Completed);
    assert!(completed_semifinal.started);
    assert_eq!(
        completed_semifinal.result,
        Some(MatchResult::Win {
            winner_id: third_winner.clone(),
            loser_id: fourth_winner.clone(),
            winner_games: 1,
            loser_games: 0,
        })
    );
    assert_eq!(completed_semifinal.cancellation_reason, None);
    assert_eq!(stage.rounds()[2].round_number, 3);
    assert_eq!(stage.rounds()[2].kind, RoundKind::Main);
    assert_eq!(stage.rounds()[2].matches.len(), 1);
    let original_final = stage.rounds()[2].matches[0].clone();
    assert_eq!(original_final.stage_id, stage_id);
    assert_eq!(original_final.round_number, 3);
    assert_eq!(original_final.kind, RoundKind::Main);
    assert_eq!(original_final.participants.len(), 2);
    assert!(original_final.participants.contains(&bye_team));
    assert!(original_final.participants.contains(&third_winner));
    assert_eq!(original_final.status, MatchStatus::Pending);
    assert!(!original_final.started);
    assert_eq!(original_final.result, None);
    assert_eq!(original_final.cancellation_reason, None);
    tournament
        .start_match(&stage_id, &original_final.match_id)
        .expect("兩分支合法結清後，真正決賽必須公開 start_match");
    let started_stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(started_stage.status(), StageStatus::Active);
    assert_eq!(started_stage.last_schedule_error(), None);
    assert_eq!(started_stage.rounds().len(), 3);
    assert_eq!(&started_stage.rounds()[0], &completed_first_round);
    assert_eq!(&started_stage.rounds()[1], &completed_second_round);
    let started_final = &started_stage.rounds()[2].matches[0];
    assert_eq!(started_final.match_id, original_final.match_id);
    assert_eq!(started_final.participants, original_final.participants);
    assert_eq!(started_final.status, MatchStatus::InProgress);
    assert!(started_final.started);
    assert_eq!(started_final.result, None);

    let outcome = tournament
        .submit_result(&stage_id, &original_final.match_id, MatchResult::DoubleLoss)
        .expect("有合法前輪雙負與原位置輪空的真決賽雙負必須能提交");
    // P1 第一個新政策 target 是命令 receipt，不以 getter 或後置統計代替。
    assert_eq!(outcome, UpdateOutcome::Applied);

    let finished = tournament.stage(&stage_id).unwrap();
    assert_eq!(finished.last_schedule_error(), None);
    assert_eq!(finished.status(), StageStatus::Finished);
    assert_eq!(finished.winner(), None);
    assert!(finished.current_round().is_none());
    assert!(finished.corrections().is_empty());
    assert!(finished.superseded_rounds().is_empty());
    assert_eq!(tournament.stages().len(), 1);
    assert_eq!(finished.config(), &config);
    assert_eq!(finished.rounds().len(), 3, "冠軍空缺不加決勝交手或假輪次");
    assert_eq!(&finished.rounds()[0], &completed_first_round);
    assert_eq!(&finished.rounds()[1], &completed_second_round);
    assert_eq!(finished.rounds()[2].matches.len(), 1);
    let final_match = &finished.rounds()[2].matches[0];
    assert_eq!(final_match.match_id, original_final.match_id);
    assert_eq!(final_match.stage_id, original_final.stage_id);
    assert_eq!(final_match.round_number, original_final.round_number);
    assert_eq!(final_match.sequence, original_final.sequence);
    assert_eq!(final_match.kind, original_final.kind);
    assert_eq!(final_match.participants, original_final.participants);
    assert_eq!(final_match.status, MatchStatus::Completed);
    assert!(final_match.started);
    assert_eq!(final_match.result, Some(MatchResult::DoubleLoss));
    assert_eq!(final_match.cancellation_reason, None);
    for team_id in &exited_teams {
        assert!(finished.rounds().iter().skip(1).all(|round| {
            round
                .matches
                .iter()
                .all(|match_| !match_.participants.contains(team_id))
        }));
    }

    // 決賽雙負只讓冠亞空缺；已正常決出的季軍與早輪第五名仍保留。
    assert_eq!(finished.standings().len(), 8);
    let standing_rank = |team_id: &TeamId| {
        finished
            .standings()
            .iter()
            .find(|row| &row.team_id == team_id)
            .unwrap()
            .rank
    };
    assert_eq!(standing_rank(&bye_team), None, "決賽雙負不決出冠軍");
    assert_eq!(standing_rank(&third_winner), None, "決賽雙負不決出亞軍");
    assert_eq!(
        standing_rank(&fourth_winner),
        Some(3),
        "半決賽敗方沿空位取得季軍"
    );
    // 同一 trace 的 literal 統計：輪空隊一真勝，另一 finalist 兩真勝，各加決賽一負。
    for (team_id, wins, losses, points, game_wins, game_losses, net) in [
        (&bye_team, 1, 1, 3, 1, 0, 1),
        (&third_winner, 2, 1, 6, 2, 0, 2),
        (&fourth_winner, 1, 1, 3, 1, 1, 0),
    ] {
        let row = finished
            .standings()
            .iter()
            .find(|row| &row.team_id == team_id)
            .unwrap();
        assert_eq!(row.wins, wins);
        assert_eq!(row.losses, losses);
        assert_eq!(row.draws, 0);
        assert_eq!(row.match_points, points);
        assert_eq!(row.game_wins, game_wins);
        assert_eq!(row.game_losses, game_losses);
        assert_eq!(row.net_game_wins, net);
    }
    for original in &original_first_round.matches[1..] {
        assert_eq!(
            standing_rank(&original.participants[1]),
            Some(5),
            "早輪正常敗方保留第五名"
        );
        let row = finished
            .standings()
            .iter()
            .find(|row| row.team_id == original.participants[1])
            .unwrap();
        assert_eq!(row.wins, 0);
        assert_eq!(row.losses, 1);
        assert_eq!(row.draws, 0);
        assert_eq!(row.match_points, 0);
        assert_eq!(row.game_wins, 0);
        assert_eq!(row.game_losses, 1);
        assert_eq!(row.net_game_wins, -1);
    }
    for team_id in &exited_teams {
        let row = finished
            .standings()
            .iter()
            .find(|row| &row.team_id == team_id)
            .unwrap();
        assert_eq!(row.wins, 0);
        assert_eq!(row.losses, 1);
        assert_eq!(row.draws, 0);
        assert_eq!(row.match_points, 0);
        assert_eq!(row.game_wins, 0);
        assert_eq!(row.game_losses, 0);
        assert_eq!(row.net_game_wins, 0);
    }

    // 真 current serde roundtrip，不手造或修改 snapshot，不宣稱舊資料遷移。
    let snapshot = serde_json::to_value(&tournament).unwrap();
    let serialized_rows = snapshot["stages"][0]["standings"].as_array().unwrap();
    assert_eq!(serialized_rows.len(), 8);
    for (team_id, rank) in [
        (&bye_team, None::<u32>),
        (&third_winner, None),
        (&fourth_winner, Some(3)),
    ] {
        assert_eq!(
            serialized_rows
                .iter()
                .find(|row| {
                    row.get("teamId").and_then(serde_json::Value::as_str)
                        == Some(team_id.0.as_str())
                })
                .and_then(|row| row.get("rank"))
                .cloned(),
            Some(serde_json::to_value(rank).unwrap())
        );
    }
    for original in &original_first_round.matches[1..] {
        assert_eq!(
            serialized_rows
                .iter()
                .find(|row| {
                    row.get("teamId").and_then(serde_json::Value::as_str)
                        == Some(original.participants[1].0.as_str())
                })
                .and_then(|row| row.get("rank"))
                .cloned(),
            Some(serde_json::json!(5))
        );
    }
    let mut restored: Tournament = serde_json::from_value(snapshot.clone())
        .expect("合法前輪雙負、輪空及真決賽雙負的 current Tournament 必須能還原");
    assert_eq!(serde_json::to_value(&restored).unwrap(), snapshot);
    let restored_stage = restored.stage(&stage_id).unwrap();
    assert_eq!(restored_stage.status(), StageStatus::Finished);
    assert_eq!(restored_stage.winner(), None);
    assert_eq!(restored_stage.last_schedule_error(), None);
    assert_eq!(
        restored_stage
            .standings()
            .iter()
            .find(|row| row.team_id == fourth_winner)
            .unwrap()
            .rank,
        Some(3)
    );
    for original in &original_first_round.matches[1..] {
        assert_eq!(
            restored_stage
                .standings()
                .iter()
                .find(|row| row.team_id == original.participants[1])
                .unwrap()
                .rank,
            Some(5)
        );
    }
    assert_eq!(
        restored
            .submit_result(&stage_id, &original_final.match_id, MatchResult::DoubleLoss)
            .expect("還原後重送同一真決賽雙負必須正常去重"),
        UpdateOutcome::AlreadyRecorded
    );
    assert_eq!(
        serde_json::to_value(&restored).unwrap(),
        snapshot,
        "原雙負、輪空、身分、start、真統計與空缺名次投影不得因同結果 retry 改變"
    );
}

#[test]
fn single_elimination_propagates_two_empty_positions_without_creating_an_empty_match() {
    let stage_id = StageId::from("single-double-empty");
    let teams = (1..=8)
        .map(|seed| SeededTeam {
            team_id: TeamId::from(format!("double-empty-team-{seed}")),
            seed,
        })
        .collect::<Vec<_>>();
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "single-double-empty".into(),
        stage: StageConfig {
            stage_id: stage_id.clone(),
            format: TournamentFormat::SingleElimination,
            teams: teams.clone(),
            random_seed: 7,
            match_format: MatchFormat::FirstToWins(1),
            clock_config: ClockConfig::default(),
            swiss_rounds: None,
            swiss_policy: None,
        },
    })
    .unwrap();

    let first_round = tournament.stage(&stage_id).unwrap().rounds()[0].clone();
    assert_eq!(first_round.matches.len(), 4);
    let first_double_loss = first_round.matches[0].clone();
    let second_double_loss = first_round.matches[1].clone();
    for match_ in [&first_double_loss, &second_double_loss] {
        tournament.start_match(&stage_id, &match_.match_id).unwrap();
        assert_eq!(
            tournament
                .submit_result(&stage_id, &match_.match_id, MatchResult::DoubleLoss)
                .unwrap(),
            UpdateOutcome::Applied
        );
    }
    assert_eq!(tournament.stage(&stage_id).unwrap().rounds().len(), 1);
    for match_ in &first_round.matches[2..] {
        let winner = match_.participants[0].clone();
        tournament.start_match(&stage_id, &match_.match_id).unwrap();
        tournament
            .submit_result(
                &stage_id,
                &match_.match_id,
                MatchResult::win(winner, match_.participants[1].clone()),
            )
            .unwrap();
    }

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.rounds().len(), 2);
    let second_round = &stage.rounds()[1];
    assert_eq!(second_round.matches.len(), 1);
    assert_eq!(
        second_round.matches[0].sequence, 2,
        "首個雙空位只保留結構序號，不造空交手"
    );
    assert_eq!(second_round.matches[0].participants.len(), 2);
    assert!(
        !second_round.matches[0]
            .participants
            .iter()
            .any(|team_id| first_double_loss.participants.contains(team_id)
                || second_double_loss.participants.contains(team_id))
    );
    let final_survivor = second_round.matches[0].participants[0].clone();
    let final_match = second_round.matches[0].clone();
    let other = final_match.participants[1].clone();
    tournament
        .start_match(&stage_id, &final_match.match_id)
        .unwrap();
    tournament
        .submit_result(
            &stage_id,
            &final_match.match_id,
            MatchResult::win(final_survivor.clone(), other.clone()),
        )
        .unwrap();

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), Some(final_survivor.clone()));
    assert_eq!(
        stage.rounds().len(),
        2,
        "只剩唯一勝者時直接收尾，不造空位交手"
    );
    assert_eq!(stage.rounds()[1].matches[0].participants.len(), 2);
    assert_eq!(
        stage.rounds()[1].matches[0].result,
        Some(MatchResult::win(final_survivor.clone(), other))
    );
    assert!(
        stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .all(|match_| !match_.participants.is_empty())
    );
    let before_retry = serde_json::to_value(&tournament).unwrap();
    assert_eq!(
        tournament
            .submit_result(
                &stage_id,
                &first_double_loss.match_id,
                MatchResult::DoubleLoss
            )
            .unwrap(),
        UpdateOutcome::AlreadyRecorded
    );
    assert_eq!(serde_json::to_value(&tournament).unwrap(), before_retry);
}

fn single_eight_team_tournament(id: &str) -> (StageId, Tournament) {
    let stage_id = StageId::from(id);
    let tournament = Tournament::new(TournamentConfig {
        tournament_id: id.into(),
        stage: StageConfig {
            stage_id: stage_id.clone(),
            format: TournamentFormat::SingleElimination,
            teams: (1..=8)
                .map(|seed| SeededTeam {
                    team_id: TeamId::from(format!("{id}-team-{seed}")),
                    seed,
                })
                .collect(),
            random_seed: 7,
            match_format: MatchFormat::FirstToWins(1),
            clock_config: ClockConfig::default(),
            swiss_rounds: None,
            swiss_policy: None,
        },
    })
    .unwrap();
    (stage_id, tournament)
}

#[test]
fn single_elimination_retains_the_only_semifinal_loser_after_an_earlier_bye_and_final_double_loss()
{
    let (stage_id, mut tournament) = single_eight_team_tournament("single-prior-vacancy-final");
    let first_round = tournament.stage(&stage_id).unwrap().rounds()[0].clone();
    let mut bronze_candidate = None;

    for _ in 0..16 {
        let stage = tournament.stage(&stage_id).unwrap();
        if stage.status() == StageStatus::Finished {
            break;
        }
        let (round_number, match_) = stage
            .rounds()
            .iter()
            .find_map(|round| {
                round
                    .matches
                    .iter()
                    .find(|match_| match_.status == MatchStatus::Pending)
                    .map(|match_| (round.round_number, match_.clone()))
            })
            .expect("未完成階段必須有待賽交手");
        if round_number == 2 && match_.sequence == 2 {
            bronze_candidate = Some(match_.participants[1].clone());
        }
        tournament.start_match(&stage_id, &match_.match_id).unwrap();
        let result = if round_number == 1 && match_.sequence == 1
            || round_number == 3 && match_.purpose == MatchPurpose::Standard
        {
            MatchResult::DoubleLoss
        } else {
            MatchResult::win(
                match_.participants[0].clone(),
                match_.participants[1].clone(),
            )
        };
        tournament
            .submit_result(&stage_id, &match_.match_id, result)
            .unwrap();
    }

    let stage = tournament.stage(&stage_id).unwrap();
    let bronze_candidate = bronze_candidate.expect("原半決賽敗方必須可辨識");
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(stage.winner(), None);
    assert_eq!(stage.rounds().len(), 3);
    assert_eq!(
        stage.rounds()[1].matches[0].result,
        Some(MatchResult::Bye {
            team_id: stage.rounds()[1].matches[0].participants[0].clone(),
        })
    );
    assert_eq!(stage.rounds()[1].matches.len(), 2);
    assert_eq!(stage.rounds()[2].matches.len(), 1, "不虛構季軍交手");
    assert_eq!(
        stage
            .standings()
            .iter()
            .find(|row| row.team_id == bronze_candidate)
            .unwrap()
            .rank,
        Some(3)
    );
    for match_ in &first_round.matches[1..] {
        let earlier_loser = &match_.participants[1];
        assert_eq!(
            stage
                .standings()
                .iter()
                .find(|row| &row.team_id == earlier_loser)
                .unwrap()
                .rank,
            Some(5),
            "較早輪次名次沿原 bracket 深度保留"
        );
    }
    assert!(
        stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .all(|match_| match_.status == MatchStatus::Completed)
    );
}

#[test]
fn single_elimination_semifinal_loser_keeps_bronze_slot_after_a_double_empty_branch() {
    let (stage_id, mut tournament) = single_eight_team_tournament("single-double-empty-bronze");
    let first_round = tournament.stage(&stage_id).unwrap().rounds()[0].clone();
    let mut bronze_candidate = None;

    for _ in 0..16 {
        let stage = tournament.stage(&stage_id).unwrap();
        if stage.status() == StageStatus::Finished {
            break;
        }
        let (round_number, match_) = stage
            .rounds()
            .iter()
            .find_map(|round| {
                round
                    .matches
                    .iter()
                    .find(|match_| match_.status == MatchStatus::Pending)
                    .map(|match_| (round.round_number, match_.clone()))
            })
            .expect("未完成階段必須有待賽交手");
        if round_number == 2 {
            bronze_candidate = Some(match_.participants[1].clone());
        }
        tournament.start_match(&stage_id, &match_.match_id).unwrap();
        let result = if round_number == 1 && match_.sequence <= 2 {
            MatchResult::DoubleLoss
        } else {
            MatchResult::win(
                match_.participants[0].clone(),
                match_.participants[1].clone(),
            )
        };
        tournament
            .submit_result(&stage_id, &match_.match_id, result)
            .unwrap();
    }

    let stage = tournament.stage(&stage_id).unwrap();
    let bronze_candidate = bronze_candidate.expect("準決賽敗方必須可辨識");
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.rounds().len(), 2, "空缺不得生成假的決賽");
    assert_eq!(stage.rounds()[1].matches.len(), 1);
    let final_winner = match stage.rounds()[1].matches[0].result.as_ref().unwrap() {
        MatchResult::Win { winner_id, .. } | MatchResult::Forfeit { winner_id, .. } => winner_id,
        result => panic!("真正敗者必須由一般勝負決定，收到 {result:?}"),
    };
    assert_eq!(stage.winner(), Some(final_winner.clone()));
    assert_eq!(
        stage
            .standings()
            .iter()
            .find(|row| row.team_id == bronze_candidate)
            .unwrap()
            .rank,
        Some(3)
    );
    for match_ in &first_round.matches[2..] {
        let earlier_loser = &match_.participants[1];
        assert_eq!(
            stage
                .standings()
                .iter()
                .find(|row| &row.team_id == earlier_loser)
                .unwrap()
                .rank,
            Some(5),
            "較早敗者不得因半決賽空位壓縮名次"
        );
    }
}

#[test]
fn single_elimination_semifinal_double_loss_awards_the_surviving_title_and_bronze_bye() {
    let stage_id = StageId::from("single-semi-double-loss");
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "single-semi-double-loss".into(),
        stage: StageConfig {
            stage_id: stage_id.clone(),
            format: TournamentFormat::SingleElimination,
            teams: (1..=4)
                .map(|seed| SeededTeam {
                    team_id: TeamId::from(format!("team-{seed}")),
                    seed,
                })
                .collect(),
            random_seed: 7,
            match_format: MatchFormat::FirstToWins(1),
            clock_config: ClockConfig::default(),
            swiss_rounds: None,
            swiss_policy: None,
        },
    })
    .unwrap();
    let semifinals = tournament.stage(&stage_id).unwrap().rounds()[0].clone();
    let vacant_branch = &semifinals.matches[0];
    let played_branch = &semifinals.matches[1];
    let champion = played_branch.participants[0].clone();
    let bronze_bye = played_branch.participants[1].clone();

    for semifinal in [vacant_branch, played_branch] {
        tournament
            .start_match(&stage_id, &semifinal.match_id)
            .unwrap();
        let result = if semifinal.match_id == vacant_branch.match_id {
            MatchResult::DoubleLoss
        } else {
            MatchResult::win_with_games(champion.clone(), bronze_bye.clone(), 1, 0)
        };
        tournament
            .submit_result(&stage_id, &semifinal.match_id, result)
            .unwrap();
    }

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), Some(champion.clone()));
    assert_eq!(stage.rounds().len(), 1, "不造決賽或季軍交手");
    assert_eq!(stage.rounds()[0].matches.len(), 2);
    assert_eq!(
        stage
            .standings()
            .iter()
            .find(|standing| standing.team_id == champion)
            .unwrap()
            .rank,
        Some(1)
    );
    assert_eq!(
        stage
            .standings()
            .iter()
            .find(|standing| standing.team_id == bronze_bye)
            .unwrap()
            .rank,
        Some(3)
    );
    assert!(
        vacant_branch
            .participants
            .iter()
            .all(|team_id| stage.standings().iter().any(|standing| {
                standing.team_id == *team_id && standing.dropped && standing.rank.is_none()
            }))
    );
}
