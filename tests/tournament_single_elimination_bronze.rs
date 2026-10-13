use fewfc::competition_clock::ClockConfig;
use fewfc::competition_match::MatchFormat;
use fewfc::tournament::{
    MatchPurpose, MatchResult, MatchStatus, RoundKind, SeededTeam, StageConfig, StageId,
    StageStatus, TeamId, Tournament, TournamentConfig, TournamentFormat, UpdateOutcome,
};

fn four_team_single_elimination() -> Tournament {
    Tournament::new(TournamentConfig {
        tournament_id: "bronze-event".into(),
        stage: StageConfig {
            stage_id: StageId::from("single-stage"),
            format: TournamentFormat::SingleElimination,
            teams: (1..=4)
                .map(|seed| SeededTeam {
                    team_id: TeamId::from(format!("team-{seed}")),
                    seed,
                })
                .collect(),
            random_seed: 11,
            match_format: MatchFormat::FirstToWins(2),
            clock_config: ClockConfig {
                main_time_ms: Some(300_000),
                action_time_ms: Some(30_000),
                out_of_turn_choice_time_ms: Some(30_000),
            },
            swiss_rounds: None,
            swiss_policy: None,
        },
    })
    .unwrap()
}

#[test]
fn semifinal_losers_play_one_bronze_match_and_wait_for_both_results() {
    for bronze_finishes_first in [false, true] {
        let stage_id = StageId::from("single-stage");
        let mut tournament = four_team_single_elimination();
        let first_round = tournament.stage(&stage_id).unwrap().rounds()[0].clone();
        assert_eq!(first_round.matches.len(), 2);

        for semifinal in &first_round.matches {
            tournament
                .submit_result(
                    &stage_id,
                    &semifinal.match_id,
                    MatchResult::win_with_games(
                        semifinal.participants[0].clone(),
                        semifinal.participants[1].clone(),
                        2,
                        0,
                    ),
                )
                .unwrap();
        }

        let stage = tournament.stage(&stage_id).unwrap();
        let final_round = stage.rounds().last().unwrap();
        assert_eq!(stage.status(), StageStatus::Active);
        assert_eq!(final_round.round_number, 2);
        assert_eq!(final_round.kind, RoundKind::Main);
        assert_eq!(final_round.matches.len(), 2);
        let championship = final_round
            .matches
            .iter()
            .find(|match_| match_.purpose == MatchPurpose::Standard)
            .unwrap()
            .clone();
        let bronze = final_round
            .matches
            .iter()
            .find(|match_| match_.purpose == MatchPurpose::Bronze)
            .unwrap()
            .clone();
        assert_eq!(championship.round_number, bronze.round_number);
        assert_eq!(championship.status, MatchStatus::Pending);
        assert_eq!(bronze.status, MatchStatus::Pending);
        assert_eq!(championship.participants.len(), 2);
        assert_eq!(bronze.participants.len(), 2);
        assert!(
            championship
                .participants
                .iter()
                .all(|team| !bronze.participants.contains(team))
        );
        assert_eq!(stage.config().match_format, MatchFormat::FirstToWins(2));
        assert_eq!(stage.config().clock_config.main_time_ms, Some(300_000));
        assert_eq!(stage.config().clock_config.action_time_ms, Some(30_000));

        let bronze_result = MatchResult::win_with_games(
            bronze.participants[0].clone(),
            bronze.participants[1].clone(),
            2,
            0,
        );
        let championship_result = MatchResult::win_with_games(
            championship.participants[0].clone(),
            championship.participants[1].clone(),
            2,
            0,
        );
        let first_match = if bronze_finishes_first {
            (&bronze, bronze_result.clone())
        } else {
            (&championship, championship_result.clone())
        };
        assert_eq!(
            tournament
                .submit_result(&stage_id, &first_match.0.match_id, first_match.1.clone())
                .unwrap(),
            UpdateOutcome::Applied
        );
        assert_eq!(
            tournament.stage(&stage_id).unwrap().status(),
            StageStatus::Active
        );
        assert_eq!(tournament.stage(&stage_id).unwrap().rounds().len(), 2);

        let after_first = tournament.stage(&stage_id).unwrap().rounds().to_vec();
        assert_eq!(
            tournament
                .submit_result(&stage_id, &first_match.0.match_id, first_match.1)
                .unwrap(),
            UpdateOutcome::AlreadyRecorded
        );
        assert_eq!(
            tournament.stage(&stage_id).unwrap().rounds(),
            after_first.as_slice()
        );
        assert_eq!(
            tournament.stage(&stage_id).unwrap().status(),
            StageStatus::Active
        );

        let second_match = if bronze_finishes_first {
            (&championship, championship_result)
        } else {
            (&bronze, bronze_result)
        };
        tournament
            .submit_result(&stage_id, &second_match.0.match_id, second_match.1)
            .unwrap();

        let finished = tournament.stage(&stage_id).unwrap();
        assert_eq!(finished.status(), StageStatus::Finished);
        assert_eq!(
            finished.winner(),
            Some(championship.participants[0].clone())
        );
        assert_eq!(finished.rounds().len(), 2);
        assert_eq!(
            finished
                .standings()
                .iter()
                .find(|standing| standing.team_id == championship.participants[0])
                .unwrap()
                .rank,
            Some(1)
        );
        assert_eq!(
            finished
                .standings()
                .iter()
                .find(|standing| standing.team_id == championship.participants[1])
                .unwrap()
                .rank,
            Some(2)
        );
        assert_eq!(
            finished
                .standings()
                .iter()
                .find(|standing| standing.team_id == bronze.participants[0])
                .unwrap()
                .rank,
            Some(3)
        );
        assert_eq!(
            finished
                .standings()
                .iter()
                .find(|standing| standing.team_id == bronze.participants[1])
                .unwrap()
                .rank,
            Some(4)
        );

        let serialized = serde_json::to_value(finished).unwrap();
        let final_matches = serialized["rounds"][1]["matches"].as_array().unwrap();
        assert!(final_matches[0].get("purpose").is_none());
        assert_eq!(final_matches[1]["purpose"], "bronze");
    }
}

#[test]
fn dropped_semifinal_loser_is_not_reintroduced_for_a_bronze_match() {
    let stage_id = StageId::from("single-stage");
    let mut tournament = four_team_single_elimination();
    let first_round = tournament.stage(&stage_id).unwrap().rounds()[0].clone();
    let dropped_team = first_round.matches[0].participants[1].clone();

    tournament.drop_team(&stage_id, &dropped_team).unwrap();
    for semifinal in &first_round.matches {
        let current = tournament.stage(&stage_id).unwrap();
        if current.rounds()[0].matches[semifinal.sequence as usize - 1].status
            == MatchStatus::Completed
        {
            continue;
        }
        tournament
            .submit_result(
                &stage_id,
                &semifinal.match_id,
                MatchResult::win_with_games(
                    semifinal.participants[0].clone(),
                    semifinal.participants[1].clone(),
                    2,
                    0,
                ),
            )
            .unwrap();
    }

    let stage = tournament.stage(&stage_id).unwrap();
    let final_round = stage.rounds().last().unwrap();
    assert_eq!(final_round.matches.len(), 1);
    assert!(final_round.matches.iter().all(|match_| {
        match_.purpose != MatchPurpose::Bronze && !match_.participants.contains(&dropped_team)
    }));
}
