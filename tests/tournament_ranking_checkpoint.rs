use fewfc::competition_clock::ClockConfig;
use fewfc::competition_match::MatchFormat;
use fewfc::tournament::*;

fn completed_tournament() -> Tournament {
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "event".into(),
        stage: StageConfig {
            stage_id: StageId("final".into()),
            format: TournamentFormat::SingleElimination,
            teams: (1..=2)
                .map(|seed| SeededTeam {
                    team_id: TeamId(format!("team-{seed}")),
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
    let stage_id = StageId("final".into());
    let match_id = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0]
        .match_id
        .clone();
    tournament
        .submit_result(&stage_id, &match_id, MatchResult::win("team-1", "team-2"))
        .unwrap();
    tournament
}

#[test]
fn assessment_preserves_source_and_matches_teams_not_standing_order() {
    let mut snapshot = serde_json::to_value(completed_tournament()).unwrap();
    snapshot["stages"][0]["standings"]
        .as_array_mut()
        .unwrap()
        .reverse();
    let tournament: Tournament = serde_json::from_value(snapshot.clone()).unwrap();
    let assessment = tournament.assess_completion().unwrap();
    assert_eq!(serde_json::to_value(&tournament).unwrap(), snapshot);
    assert_eq!(
        assessment.stages[0].placements[0].team_id,
        TeamId("team-2".into())
    );
    assert_eq!(
        assessment.stages[0].placements[1].team_id,
        TeamId("team-1".into())
    );
}

#[test]
fn assessment_rejects_rank_drift_without_breaking_historical_projection() {
    let mut snapshot = serde_json::to_value(completed_tournament()).unwrap();
    snapshot["stages"][0]["standings"][0]["rank"] = serde_json::json!(2);
    let tournament: Tournament = serde_json::from_value(snapshot.clone()).unwrap();
    assert_eq!(
        tournament.assess_completion(),
        Err(TournamentAssessmentError::RankingCheckpoint(
            RankingCheckpointError::RankMismatch {
                stage_id: StageId("final".into()),
                team_id: TeamId("team-1".into()),
            }
        ))
    );
    assert_eq!(serde_json::to_value(&tournament).unwrap(), snapshot);
    assert_eq!(
        tournament.ranking_projection().stages()[0].standings()[0].rank,
        Some(1)
    );
}

#[test]
fn assessment_rejects_missing_or_duplicate_standing_teams() {
    for duplicate in [false, true] {
        let mut snapshot = serde_json::to_value(completed_tournament()).unwrap();
        let standings = snapshot["stages"][0]["standings"].as_array_mut().unwrap();
        if duplicate {
            standings[1] = standings[0].clone();
        } else {
            standings.pop();
        }
        let tournament: Tournament = serde_json::from_value(snapshot).unwrap();
        assert_eq!(
            tournament.assess_completion(),
            Err(TournamentAssessmentError::RankingCheckpoint(
                RankingCheckpointError::StandingTeamsMismatch {
                    stage_id: StageId("final".into()),
                }
            ))
        );
    }
}

#[test]
fn assessment_accepts_legitimate_shared_elimination_ranks() {
    let mut snapshot = serde_json::to_value(completed_tournament()).unwrap();
    let stage = &mut snapshot["stages"][0];
    for seed in 3..=8 {
        stage["config"]["teams"]
            .as_array_mut()
            .unwrap()
            .push(serde_json::json!({"teamId": format!("team-{seed}"), "seed": seed}));
    }
    let config: StageConfig = serde_json::from_value(stage["config"].clone()).unwrap();
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "shared-ranks".into(),
        stage: config,
    })
    .unwrap();
    let stage_id = StageId("final".into());
    while tournament.stage(&stage_id).unwrap().status() != StageStatus::Finished {
        let pending = tournament
            .stage(&stage_id)
            .unwrap()
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .find(|item| item.status == MatchStatus::Pending)
            .unwrap()
            .clone();
        tournament
            .submit_result(
                &stage_id,
                &pending.match_id,
                MatchResult::win(
                    pending.participants[0].clone(),
                    pending.participants[1].clone(),
                ),
            )
            .unwrap();
    }
    let ranks: Vec<_> = tournament.stages()[0]
        .standings()
        .iter()
        .filter_map(|row| row.rank)
        .collect();
    assert!(ranks.iter().filter(|&&rank| rank == 5).count() > 1);
    assert!(tournament.assess_completion().unwrap().ready);
}
