use fewfc::competition_clock::ClockConfig;
use fewfc::competition_match::MatchFormat;
use fewfc::tournament::*;
use std::collections::BTreeMap;

fn tournament(format: TournamentFormat, count: u32) -> Tournament {
    Tournament::new(TournamentConfig {
        tournament_id: "verification".into(),
        stage: StageConfig {
            stage_id: StageId("stage".into()),
            format,
            teams: (1..=count)
                .map(|seed| SeededTeam {
                    team_id: TeamId(format!("team-{seed}")),
                    seed,
                })
                .collect(),
            random_seed: 7,
            match_format: if matches!(
                format,
                TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination
            ) {
                MatchFormat::FirstToWins(1)
            } else {
                MatchFormat::FixedGames(1)
            },
            clock_config: ClockConfig::default(),
            swiss_rounds: None,
            swiss_policy: None,
        },
    })
    .unwrap()
}

fn stage_config(format: TournamentFormat, match_format: MatchFormat) -> StageConfig {
    StageConfig {
        stage_id: StageId("stage".into()),
        format,
        teams: vec![
            SeededTeam {
                team_id: TeamId("team-a".into()),
                seed: 1,
            },
            SeededTeam {
                team_id: TeamId("team-b".into()),
                seed: 2,
            },
        ],
        random_seed: 1,
        match_format,
        clock_config: ClockConfig::default(),
        swiss_rounds: None,
        swiss_policy: None,
    }
}

#[test]
fn elimination_finishes_with_expected_loss_counts_and_champion() {
    for format in [
        TournamentFormat::SingleElimination,
        TournamentFormat::DoubleElimination,
    ] {
        for count in 2..=32 {
            for choose_second in [false, true] {
                let mut tournament = tournament(format, count);
                let stage_id = StageId("stage".into());
                let mut losses = BTreeMap::<TeamId, u32>::new();
                let mut played = 0;
                while tournament.stage(&stage_id).unwrap().status() != StageStatus::Finished {
                    let pending = tournament
                        .stage(&stage_id)
                        .unwrap()
                        .rounds()
                        .iter()
                        .flat_map(|round| &round.matches)
                        .find(|m| m.status == MatchStatus::Pending)
                        .cloned()
                        .expect("未結束的淘汰賽必須有可進行的交手");
                    assert_eq!(pending.participants.len(), 2);
                    let winner = pending.participants[usize::from(choose_second)].clone();
                    let loser = pending.participants[usize::from(!choose_second)].clone();
                    *losses.entry(loser.clone()).or_default() += 1;
                    tournament
                        .submit_result(
                            &stage_id,
                            &pending.match_id,
                            MatchResult::win(winner.clone(), loser),
                        )
                        .unwrap_or_else(|e| panic!("{format:?}, {count}隊, {played}場: {e:?}"));
                    played += 1;
                    assert!(played <= 2 * count, "賽程不能無限延長");
                }
                let stage = tournament.stage(&stage_id).unwrap();
                let champion = stage.winner().expect("已結束淘汰賽必須有冠軍");
                let required_losses = if format == TournamentFormat::SingleElimination {
                    1
                } else {
                    2
                };
                let bronze_loser = stage
                    .rounds()
                    .last()
                    .and_then(|round| {
                        round
                            .matches
                            .iter()
                            .find(|match_| match_.purpose == MatchPurpose::Bronze)
                    })
                    .and_then(|match_| match_.result.as_ref())
                    .and_then(|result| match result {
                        MatchResult::Win { loser_id, .. }
                        | MatchResult::Forfeit { loser_id, .. } => Some(loser_id.clone()),
                        _ => None,
                    });
                for team in &tournament.stage(&stage_id).unwrap().config().teams {
                    let actual = losses.get(&team.team_id).copied().unwrap_or(0);
                    if team.team_id == champion {
                        assert!(actual < required_losses);
                    } else if Some(team.team_id.clone()) == bronze_loser {
                        assert_eq!(actual, 2, "季軍賽敗方必須有兩敗");
                    } else {
                        assert_eq!(
                            actual, required_losses,
                            "{format:?}, {count}隊: 非冠軍淘汰敗數"
                        );
                    }
                }
                let standing = tournament
                    .stage(&stage_id)
                    .unwrap()
                    .standings()
                    .iter()
                    .find(|standing| standing.team_id == champion)
                    .unwrap();
                assert_eq!(
                    standing.rank,
                    Some(1),
                    "{format:?}, {count}隊, choose_second={choose_second}: 最終決賽勝者必須排名第一"
                );
                let final_match = stage
                    .rounds()
                    .last()
                    .unwrap()
                    .matches
                    .iter()
                    .find(|match_| match_.purpose == MatchPurpose::Standard)
                    .unwrap();
                let runner_up = final_match
                    .participants
                    .iter()
                    .find(|id| *id != &champion)
                    .unwrap();
                assert_eq!(
                    stage
                        .standings()
                        .iter()
                        .find(|row| &row.team_id == runner_up)
                        .unwrap()
                        .rank,
                    Some(2),
                    "{format:?}, {count}隊：決賽敗方必須是亞軍"
                );
            }
        }
    }
}

#[test]
fn stage_settings_are_serialized_as_the_single_source_of_truth() {
    let tournament = Tournament::new(TournamentConfig {
        tournament_id: "event".into(),
        stage: stage_config(
            TournamentFormat::SingleElimination,
            MatchFormat::FirstToWins(2),
        ),
    })
    .unwrap();
    let stage = serde_json::to_value(tournament).unwrap()["stages"][0]["config"].clone();
    assert_eq!(
        stage["matchFormat"],
        serde_json::json!({ "type": "firstToWins", "value": 2 })
    );
    assert_eq!(
        stage["clockConfig"]["mainTimeMs"],
        serde_json::json!(300_000)
    );
    assert!(stage.get("matchWinsRequired").is_none());
    assert!(stage.get("match_format").is_none());
    assert!(stage.get("clock_config").is_none());
}

#[test]
fn stage_rejects_match_format_that_does_not_fit_pairing_format() {
    for (format, match_format) in [
        (
            TournamentFormat::SingleElimination,
            MatchFormat::FixedGames(3),
        ),
        (
            TournamentFormat::DoubleElimination,
            MatchFormat::FixedGames(3),
        ),
        (TournamentFormat::RoundRobin, MatchFormat::FirstToWins(1)),
        (TournamentFormat::Swiss, MatchFormat::FixedGames(0)),
    ] {
        let result = Tournament::new(TournamentConfig {
            tournament_id: "event".into(),
            stage: stage_config(format, match_format),
        });
        assert!(result.is_err(), "{format:?} / {match_format:?} 應被拒絕");
    }
}

#[test]
fn stage_score_validation_uses_match_format_threshold_or_fixed_game_count() {
    let mut elimination = Tournament::new(TournamentConfig {
        tournament_id: "elimination".into(),
        stage: stage_config(
            TournamentFormat::SingleElimination,
            MatchFormat::FirstToWins(2),
        ),
    })
    .unwrap();
    let elimination_match = elimination
        .stage(&StageId("stage".into()))
        .unwrap()
        .rounds()[0]
        .matches[0]
        .match_id
        .clone();
    assert_eq!(
        elimination.submit_result(
            &StageId("stage".into()),
            &elimination_match,
            MatchResult::win_with_games("team-a", "team-b", 1, 0),
        ),
        Err(ScheduleError::InvalidScore(elimination_match.clone()))
    );
    elimination
        .submit_result(
            &StageId("stage".into()),
            &elimination_match,
            MatchResult::win_with_games("team-a", "team-b", 2, 1),
        )
        .unwrap();

    let mut round_robin = Tournament::new(TournamentConfig {
        tournament_id: "round-robin".into(),
        stage: stage_config(TournamentFormat::RoundRobin, MatchFormat::FixedGames(3)),
    })
    .unwrap();
    let round_robin_match = round_robin
        .stage(&StageId("stage".into()))
        .unwrap()
        .rounds()[0]
        .matches[0]
        .match_id
        .clone();
    round_robin
        .submit_result(
            &StageId("stage".into()),
            &round_robin_match,
            MatchResult::win_with_games("team-a", "team-b", 1, 0),
        )
        .unwrap();

    let mut drawn_round_robin = Tournament::new(TournamentConfig {
        tournament_id: "drawn-round-robin".into(),
        stage: stage_config(TournamentFormat::RoundRobin, MatchFormat::FixedGames(3)),
    })
    .unwrap();
    let drawn_match = drawn_round_robin
        .stage(&StageId("stage".into()))
        .unwrap()
        .rounds()[0]
        .matches[0]
        .match_id
        .clone();
    drawn_round_robin
        .submit_result(
            &StageId("stage".into()),
            &drawn_match,
            MatchResult::draw_with_games(1, 1),
        )
        .unwrap();
}

#[test]
fn round_robin_has_every_pair_exactly_once() {
    for count in 2..=32 {
        let tournament = tournament(TournamentFormat::RoundRobin, count);
        let stage = tournament.stage(&StageId("stage".into())).unwrap();
        let mut pairs = std::collections::BTreeSet::new();
        for round in stage.rounds() {
            let mut participants = std::collections::BTreeSet::new();
            for match_ in &round.matches {
                for team in &match_.participants {
                    assert!(participants.insert(team), "同輪不得重複出賽");
                }
                if match_.participants.len() == 2 {
                    let mut pair = match_.participants.clone();
                    pair.sort();
                    assert!(pairs.insert(pair), "循環賽不得重複配對");
                }
            }
        }
        assert_eq!(pairs.len(), (count * (count - 1) / 2) as usize);
    }
}

#[test]
fn result_retries_and_invalid_reports_preserve_the_snapshot() {
    let mut tournament = tournament(TournamentFormat::RoundRobin, 4);
    let stage_id = StageId("stage".into());
    let scheduled = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0].clone();
    let before = serde_json::to_value(&tournament).unwrap();
    assert!(
        tournament
            .submit_result(
                &stage_id,
                &scheduled.match_id,
                MatchResult::win("not-registered", scheduled.participants[1].clone())
            )
            .is_err()
    );
    assert_eq!(serde_json::to_value(&tournament).unwrap(), before);
    let result = MatchResult::win(
        scheduled.participants[0].clone(),
        scheduled.participants[1].clone(),
    );
    tournament
        .submit_result(&stage_id, &scheduled.match_id, result.clone())
        .unwrap();
    let recorded = serde_json::to_value(&tournament).unwrap();
    tournament
        .submit_result(&stage_id, &scheduled.match_id, result)
        .unwrap();
    assert_eq!(serde_json::to_value(&tournament).unwrap(), recorded);
    let restored: Tournament = serde_json::from_value(recorded.clone()).unwrap();
    assert_eq!(serde_json::to_value(restored).unwrap(), recorded);
}

#[test]
fn an_unfinished_elimination_stage_has_no_winner() {
    let mut tournament = tournament(TournamentFormat::SingleElimination, 4);
    let stage_id = StageId("stage".into());
    assert_eq!(tournament.stage(&stage_id).unwrap().winner(), None);
    let match_ = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0].clone();
    tournament
        .submit_result(
            &stage_id,
            &match_.match_id,
            MatchResult::win(
                match_.participants[0].clone(),
                match_.participants[1].clone(),
            ),
        )
        .unwrap();
    assert_eq!(tournament.stage(&stage_id).unwrap().winner(), None);
}

#[test]
fn double_elimination_withdrawal_cannot_schedule_the_team_again() {
    let mut tournament = tournament(TournamentFormat::DoubleElimination, 2);
    let stage_id = StageId("stage".into());
    tournament
        .drop_team(&stage_id, &TeamId::from("team-1"))
        .unwrap();
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), Some(TeamId::from("team-2")));
    assert!(
        stage
            .rounds()
            .iter()
            .flat_map(|r| &r.matches)
            .all(|m| m.status == MatchStatus::Completed)
    );
    assert!(
        stage
            .standings()
            .iter()
            .all(|s| s.game_wins == 0 && s.game_losses == 0)
    );
}

#[test]
fn a_draw_cannot_have_unequal_game_wins() {
    let mut tournament = tournament(TournamentFormat::RoundRobin, 2);
    let stage_id = StageId("stage".into());
    let match_id = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0]
        .match_id
        .clone();
    let before = serde_json::to_value(&tournament).unwrap();
    assert!(
        tournament
            .submit_result(
                &stage_id,
                &match_id,
                MatchResult::Draw {
                    first_games: 2,
                    second_games: 0
                }
            )
            .is_err()
    );
    assert_eq!(serde_json::to_value(&tournament).unwrap(), before);
}

#[test]
fn round_robin_correction_does_not_rewrite_unrelated_pairings() {
    let mut tournament = tournament(TournamentFormat::RoundRobin, 4);
    let stage_id = StageId("stage".into());
    let scheduled = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0].clone();
    tournament
        .submit_result(
            &stage_id,
            &scheduled.match_id,
            MatchResult::win(
                scheduled.participants[0].clone(),
                scheduled.participants[1].clone(),
            ),
        )
        .unwrap();
    let future = tournament.stage(&stage_id).unwrap().rounds()[1..].to_vec();
    tournament
        .correct_result(
            &stage_id,
            &scheduled.match_id,
            MatchResult::win(
                scheduled.participants[1].clone(),
                scheduled.participants[0].clone(),
            ),
            "裁判核對原始紀錄後更正".into(),
        )
        .unwrap();
    assert_eq!(tournament.stage(&stage_id).unwrap().rounds()[1..], future);
    assert_eq!(tournament.stage(&stage_id).unwrap().corrections().len(), 1);
}

#[test]
fn elimination_correction_rebuilds_unstarted_pairings_with_new_ids_and_history() {
    for format in [
        TournamentFormat::SingleElimination,
        TournamentFormat::DoubleElimination,
    ] {
        let mut tournament = tournament(format, 4);
        let stage_id = StageId("stage".into());
        let first_round = tournament.stage(&stage_id).unwrap().rounds()[0].clone();
        for match_ in &first_round.matches {
            tournament
                .submit_result(
                    &stage_id,
                    &match_.match_id,
                    MatchResult::win(
                        match_.participants[0].clone(),
                        match_.participants[1].clone(),
                    ),
                )
                .unwrap();
        }
        let previous_future = tournament.stage(&stage_id).unwrap().rounds()[1].clone();
        let corrected = &first_round.matches[0];
        tournament
            .correct_result(
                &stage_id,
                &corrected.match_id,
                MatchResult::win(
                    corrected.participants[1].clone(),
                    corrected.participants[0].clone(),
                ),
                "裁判確認勝方輸入顛倒".into(),
            )
            .unwrap();
        let stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(stage.superseded_rounds(), &[previous_future.clone()]);
        let next = stage.rounds()[1].clone();
        assert_ne!(
            next.matches[0].match_id,
            previous_future.matches[0].match_id
        );
        let expected = if format == TournamentFormat::SingleElimination {
            &corrected.participants[1]
        } else {
            &corrected.participants[0]
        };
        assert!(
            next.matches
                .iter()
                .any(|m| m.participants.contains(expected))
        );
        assert!(
            tournament
                .start_match(&stage_id, &previous_future.matches[0].match_id)
                .is_err()
        );
        tournament
            .start_match(&stage_id, &next.matches[0].match_id)
            .unwrap();
        let before = serde_json::to_value(&tournament).unwrap();
        assert!(
            tournament
                .correct_result(
                    &stage_id,
                    &corrected.match_id,
                    MatchResult::win(
                        corrected.participants[0].clone(),
                        corrected.participants[1].clone()
                    ),
                    "不可直接重配已開打交手".into()
                )
                .is_err()
        );
        assert_eq!(serde_json::to_value(&tournament).unwrap(), before);
        let restored: Tournament = serde_json::from_value(before.clone()).unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), before);
    }
}

#[test]
fn swiss_rounds_preserve_draws_and_never_repeat_opponents() {
    for count in [5, 8, 9, 16] {
        let mut config = tournament(TournamentFormat::RoundRobin, count)
            .stage(&StageId("stage".into()))
            .unwrap()
            .config()
            .clone();
        config.format = TournamentFormat::Swiss;
        config.swiss_rounds = Some(3);
        config.swiss_policy = Some(SwissStandingPolicy {
            match_points: MatchPoints {
                win: 3,
                draw: 1,
                loss: 0,
                bye: 3,
            },
            draw_credit: DrawCredit::new(1, 2).unwrap(),
            opponent_win_rate_floor_percent: None,
            opponent_win_rate_cap_percent: None,
            exclude_byes_from_opponent_win_rate: true,
            tie_breakers: vec![SwissTieBreaker::OpponentWinRate],
        });
        let mut tournament = Tournament::new(TournamentConfig {
            tournament_id: "swiss".into(),
            stage: config,
        })
        .unwrap();
        let stage_id = StageId("stage".into());
        let mut played = 0;
        while tournament.stage(&stage_id).unwrap().status() != StageStatus::Finished {
            let match_ = tournament
                .stage(&stage_id)
                .unwrap()
                .rounds()
                .iter()
                .flat_map(|r| &r.matches)
                .find(|m| m.status == MatchStatus::Pending)
                .cloned()
                .expect("三輪範例應有合法配對");
            tournament
                .submit_result(&stage_id, &match_.match_id, MatchResult::draw())
                .unwrap();
            played += 1;
            assert!(played <= 3 * count);
        }
        let stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(stage.rounds().len(), 3);
        let mut pairs = std::collections::BTreeSet::new();
        let mut byes = std::collections::BTreeSet::new();
        for round in stage.rounds() {
            let mut participants = std::collections::BTreeSet::new();
            for match_ in &round.matches {
                for team in &match_.participants {
                    assert!(participants.insert(team));
                }
                let mut pair = match_.participants.clone();
                pair.sort();
                if pair.len() == 2 {
                    assert!(pairs.insert(pair));
                } else {
                    assert!(byes.insert(pair));
                }
            }
        }
        assert!(stage.standings().iter().all(|r| r.losses == 0));
    }
}

#[test]
fn swiss_keeps_withdrawals_when_no_legal_pairing_remains() {
    let mut config = tournament(TournamentFormat::RoundRobin, 2)
        .stage(&StageId("stage".into()))
        .unwrap()
        .config()
        .clone();
    config.format = TournamentFormat::Swiss;
    config.swiss_rounds = Some(3);
    config.swiss_policy = Some(SwissStandingPolicy {
        match_points: MatchPoints {
            win: 3,
            draw: 1,
            loss: 0,
            bye: 3,
        },
        draw_credit: DrawCredit::new(1, 2).unwrap(),
        opponent_win_rate_floor_percent: None,
        opponent_win_rate_cap_percent: None,
        exclude_byes_from_opponent_win_rate: true,
        tie_breakers: vec![SwissTieBreaker::OpponentWinRate],
    });
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "swiss".into(),
        stage: config,
    })
    .unwrap();
    let stage_id = StageId("stage".into());
    for id in ["team-1", "team-2"] {
        tournament.drop_team(&stage_id, &TeamId::from(id)).unwrap();
    }
    let stage = tournament.stage(&stage_id).unwrap();
    assert!(stage.standings().iter().all(|s| s.dropped));
    assert!(stage.last_schedule_error().is_some());
    assert_eq!(stage.status(), StageStatus::Active);
}

#[test]
fn double_loss_is_a_distinct_result_and_gives_each_team_one_loss() {
    let mut tournament = tournament(TournamentFormat::RoundRobin, 2);
    let stage_id = StageId("stage".into());
    let match_id = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0]
        .match_id
        .clone();
    let json = serde_json::to_value(MatchResult::DoubleLoss).unwrap();
    assert_eq!(json, serde_json::json!({"type": "doubleLoss"}));
    assert_ne!(MatchResult::DoubleLoss, MatchResult::draw());

    tournament
        .submit_result(&stage_id, &match_id, MatchResult::DoubleLoss)
        .unwrap();
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), None);
    for standing in stage.standings() {
        assert_eq!(standing.losses, 1);
        assert_eq!(standing.wins, 0);
        assert_eq!(standing.draws, 0);
        assert_eq!(standing.match_points, 0);
    }
}

#[test]
fn swiss_double_loss_is_normal_negative_score_without_a_fabricated_winner() {
    let mut config = tournament(TournamentFormat::RoundRobin, 2)
        .stage(&StageId("stage".into()))
        .unwrap()
        .config()
        .clone();
    config.format = TournamentFormat::Swiss;
    config.swiss_rounds = Some(1);
    config.swiss_policy = Some(SwissStandingPolicy {
        match_points: MatchPoints {
            win: 3,
            draw: 1,
            loss: 0,
            bye: 3,
        },
        draw_credit: DrawCredit::new(1, 2).unwrap(),
        opponent_win_rate_floor_percent: None,
        opponent_win_rate_cap_percent: None,
        exclude_byes_from_opponent_win_rate: true,
        tie_breakers: vec![SwissTieBreaker::SeedOrder],
    });
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "double-loss-swiss".into(),
        stage: config,
    })
    .unwrap();
    let stage_id = StageId("stage".into());
    let match_id = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0]
        .match_id
        .clone();
    tournament
        .submit_result(&stage_id, &match_id, MatchResult::DoubleLoss)
        .unwrap();
    let stage = tournament.stage(&stage_id).unwrap();
    assert!(stage.standings().iter().all(|standing| {
        standing.losses == 1
            && standing.wins == 0
            && standing.draws == 0
            && standing.match_points == 0
    }));
}

#[test]
fn single_elimination_final_double_loss_finishes_and_correction_is_explicit() {
    let mut tournament = tournament(TournamentFormat::SingleElimination, 2);
    let stage_id = StageId("stage".into());
    let match_ = tournament.stage(&stage_id).unwrap().rounds()[0].matches[0].clone();
    let double_loss = MatchResult::DoubleLoss;
    assert_eq!(
        tournament
            .submit_result(&stage_id, &match_.match_id, double_loss.clone())
            .unwrap(),
        UpdateOutcome::Applied
    );
    let snapshot = serde_json::to_value(&tournament).unwrap();
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), None);
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(
        stage.rounds()[0].matches[0].result,
        Some(double_loss.clone())
    );

    assert_eq!(
        tournament
            .submit_result(&stage_id, &match_.match_id, double_loss)
            .unwrap(),
        UpdateOutcome::AlreadyRecorded
    );
    assert_eq!(serde_json::to_value(&tournament).unwrap(), snapshot);

    tournament
        .correct_result(
            &stage_id,
            &match_.match_id,
            MatchResult::win(
                match_.participants[0].clone(),
                match_.participants[1].clone(),
            ),
            "裁判確認先後與晉級結果".into(),
        )
        .unwrap();
    assert_eq!(
        tournament.stage(&stage_id).unwrap().winner(),
        Some(match_.participants[0].clone())
    );
    assert_eq!(tournament.stage(&stage_id).unwrap().corrections().len(), 1);
    let correction = &tournament.stage(&stage_id).unwrap().corrections()[0];
    assert_eq!(correction.match_id, match_.match_id);
    assert_eq!(correction.previous_result, MatchResult::DoubleLoss);
    assert_eq!(
        correction.replacement_result,
        MatchResult::Win {
            winner_id: match_.participants[0].clone(),
            loser_id: match_.participants[1].clone(),
            winner_games: 1,
            loser_games: 0,
        }
    );
    assert_eq!(correction.reason, "裁判確認先後與晉級結果");
}
