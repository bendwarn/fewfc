use std::collections::BTreeMap;

use fewfc::competition_clock::ClockConfig;
use fewfc::competition_match::MatchFormat;
use fewfc::tournament::{
    MatchId, MatchResult, MatchStatus, Round, RoundKind, SeededTeam, StageConfig, StageId,
    StageStatus, TeamId, Tournament, TournamentConfig, TournamentFormat, UpdateOutcome,
};

fn four_team_double_elimination() -> Tournament {
    Tournament::new(TournamentConfig {
        tournament_id: "double-event".into(),
        stage: StageConfig {
            stage_id: StageId::from("double-stage"),
            format: TournamentFormat::DoubleElimination,
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

fn current_round(tournament: &Tournament, stage_id: &StageId, kind: RoundKind) -> Round {
    let round = tournament
        .stage(stage_id)
        .unwrap()
        .rounds()
        .last()
        .unwrap()
        .clone();
    assert_eq!(round.kind, kind);
    round
}

fn participants(round: &Round) -> Vec<TeamId> {
    let mut team_ids = round
        .matches
        .iter()
        .flat_map(|match_| match_.participants.iter().cloned())
        .collect::<Vec<_>>();
    team_ids.sort();
    team_ids
}

fn submit_winner(
    tournament: &mut Tournament,
    stage_id: &StageId,
    match_id: &MatchId,
    winner: &TeamId,
) {
    let match_ = tournament
        .stage(stage_id)
        .unwrap()
        .rounds()
        .iter()
        .flat_map(|round| &round.matches)
        .find(|match_| &match_.match_id == match_id)
        .unwrap()
        .clone();
    let loser = match_
        .participants
        .iter()
        .find(|team_id| *team_id != winner)
        .unwrap()
        .clone();
    tournament
        .submit_result(
            stage_id,
            match_id,
            MatchResult::win_with_games(winner.clone(), loser, 2, 0),
        )
        .unwrap();
}

fn complete_round_with_first_participant_wins(
    tournament: &mut Tournament,
    stage_id: &StageId,
    kind: RoundKind,
) -> Round {
    let round = current_round(tournament, stage_id, kind);
    for match_ in &round.matches {
        submit_winner(
            tournament,
            stage_id,
            &match_.match_id,
            &match_.participants[0],
        );
    }
    round
}

fn complete_double_elimination(
    force_reset: bool,
) -> (Tournament, StageId, TeamId, TeamId, TeamId, TeamId) {
    let stage_id = StageId::from("double-stage");
    let mut tournament = four_team_double_elimination();

    let winners_round =
        complete_round_with_first_participant_wins(&mut tournament, &stage_id, RoundKind::Winners);
    assert_eq!(winners_round.matches.len(), 2);
    let winners_after_first_round = winners_round
        .matches
        .iter()
        .map(|match_| match_.participants[0].clone())
        .collect::<Vec<_>>();
    let first_loss_teams = winners_round
        .matches
        .iter()
        .map(|match_| match_.participants[1].clone())
        .collect::<Vec<_>>();

    let losers_round =
        complete_round_with_first_participant_wins(&mut tournament, &stage_id, RoundKind::Losers);
    assert_eq!(participants(&losers_round), {
        let mut expected = first_loss_teams.clone();
        expected.sort();
        expected
    });
    let lower_bracket_winner = losers_round.matches[0].participants[0].clone();
    let first_lower_bracket_elimination = losers_round.matches[0].participants[1].clone();

    let winners_semifinal =
        complete_round_with_first_participant_wins(&mut tournament, &stage_id, RoundKind::Winners);
    assert_eq!(participants(&winners_semifinal), {
        let mut expected = winners_after_first_round;
        expected.sort();
        expected
    });
    let winners_champion = winners_semifinal.matches[0].participants[0].clone();
    let second_loss_team = winners_semifinal.matches[0].participants[1].clone();

    let losers_final = current_round(&tournament, &stage_id, RoundKind::Losers);
    assert_eq!(losers_final.matches.len(), 1);
    assert_eq!(participants(&losers_final), {
        let mut expected = vec![lower_bracket_winner.clone(), second_loss_team.clone()];
        expected.sort();
        expected
    });
    let losers_champion = losers_final.matches[0].participants[0].clone();
    let third_place_team = losers_final.matches[0].participants[1].clone();
    submit_winner(
        &mut tournament,
        &stage_id,
        &losers_final.matches[0].match_id,
        &losers_champion,
    );

    let grand_final = current_round(&tournament, &stage_id, RoundKind::GrandFinal);
    assert_eq!(grand_final.matches.len(), 1);
    assert_eq!(participants(&grand_final), {
        let mut expected = vec![winners_champion.clone(), losers_champion.clone()];
        expected.sort();
        expected
    });
    let result_winner = if force_reset {
        losers_champion.clone()
    } else {
        winners_champion.clone()
    };
    let grand_final_result = MatchResult::win_with_games(
        result_winner.clone(),
        if result_winner == winners_champion {
            losers_champion.clone()
        } else {
            winners_champion.clone()
        },
        2,
        0,
    );
    let grand_final_id = grand_final.matches[0].match_id.clone();
    tournament
        .submit_result(&stage_id, &grand_final_id, grand_final_result.clone())
        .unwrap();

    if force_reset {
        assert_eq!(
            tournament.stage(&stage_id).unwrap().status(),
            StageStatus::Active
        );
        let reset = current_round(&tournament, &stage_id, RoundKind::GrandFinalReset);
        assert_eq!(reset.matches.len(), 1);
        assert_ne!(reset.matches[0].match_id, grand_final_id);
        assert_eq!(participants(&reset), participants(&grand_final));
        let round_count = tournament.stage(&stage_id).unwrap().rounds().len();
        assert_eq!(
            tournament
                .submit_result(&stage_id, &grand_final_id, grand_final_result)
                .unwrap(),
            UpdateOutcome::AlreadyRecorded
        );
        let stage = tournament.stage(&stage_id).unwrap();
        assert_eq!(stage.rounds().len(), round_count);
        assert_eq!(
            stage.rounds().last().unwrap().kind,
            RoundKind::GrandFinalReset
        );
        assert!(
            stage
                .rounds()
                .last()
                .unwrap()
                .matches
                .iter()
                .all(|match_| { match_.status == MatchStatus::Pending && match_.result.is_none() })
        );

        submit_winner(
            &mut tournament,
            &stage_id,
            &reset.matches[0].match_id,
            &winners_champion,
        );
    }

    (
        tournament,
        stage_id,
        winners_champion,
        losers_champion,
        third_place_team,
        first_lower_bracket_elimination,
    )
}

fn double_elimination_before_lower_final() -> (Tournament, StageId, TeamId, Round) {
    let stage_id = StageId::from("double-stage");
    let mut tournament = four_team_double_elimination();
    complete_round_with_first_participant_wins(&mut tournament, &stage_id, RoundKind::Winners);
    complete_round_with_first_participant_wins(&mut tournament, &stage_id, RoundKind::Losers);
    let winners_semifinal =
        complete_round_with_first_participant_wins(&mut tournament, &stage_id, RoundKind::Winners);
    let winners_champion = winners_semifinal.matches[0].participants[0].clone();
    let lower_final = current_round(&tournament, &stage_id, RoundKind::Losers);
    assert_eq!(lower_final.matches.len(), 1);
    (tournament, stage_id, winners_champion, lower_final)
}

fn complete_double_elimination_with_vacant_upper_semifinals(
    team_count: usize,
    vacant_upper_semifinals: bool,
) -> Tournament {
    let stage_id = StageId::from(format!("ticket-20-double-{team_count}"));
    let tournament_id = format!("ticket-20-double-{team_count}");
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id,
        stage: StageConfig {
            stage_id: stage_id.clone(),
            format: TournamentFormat::DoubleElimination,
            teams: (1..=team_count)
                .map(|seed| SeededTeam {
                    team_id: TeamId::from(format!("team-{seed}")),
                    seed: seed as u32,
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

    for _ in 0..128 {
        let stage = tournament.stage(&stage_id).unwrap();
        if stage.status() == StageStatus::Finished {
            break;
        }
        let round = stage
            .rounds()
            .iter()
            .find(|round| {
                round
                    .matches
                    .iter()
                    .any(|match_| match_.status == MatchStatus::Pending)
            })
            .expect("未完成雙敗階段必須有待賽輪次")
            .clone();
        for match_ in round
            .matches
            .iter()
            .filter(|match_| match_.status == MatchStatus::Pending)
        {
            tournament.start_match(&stage_id, &match_.match_id).unwrap();
            let result = if vacant_upper_semifinals
                && round.kind == RoundKind::Winners
                && round.round_number == 5
            {
                MatchResult::DoubleLoss
            } else {
                MatchResult::win_with_games(
                    match_.participants[0].clone(),
                    match_.participants[1].clone(),
                    1,
                    0,
                )
            };
            tournament
                .submit_result(&stage_id, &match_.match_id, result)
                .unwrap();
        }
    }

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.last_schedule_error(), None);
    assert!(stage.rounds().iter().all(|round| {
        round
            .matches
            .iter()
            .all(|match_| match_.status == MatchStatus::Completed && match_.result.is_some())
    }));
    if vacant_upper_semifinals {
        assert!(stage.rounds().iter().all(|round| !matches!(
            round.kind,
            RoundKind::GrandFinal | RoundKind::GrandFinalReset
        )));
    }
    tournament
}

#[derive(Clone, Copy)]
enum DoubleEliminationRankScenario {
    NoVacancy,
    OpeningDoubleLoss,
    NoChampion,
    OriginalSlotVacancies,
    EmptyFirstTwoLowerLayers,
}

fn complete_double_elimination_rank_scenario(
    team_count: usize,
    scenario: DoubleEliminationRankScenario,
) -> Tournament {
    let stage_id = StageId::from("ticket-20-rank-retention");
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "ticket-20-rank-retention".into(),
        stage: StageConfig {
            stage_id: stage_id.clone(),
            format: TournamentFormat::DoubleElimination,
            teams: (1..=team_count)
                .map(|seed| SeededTeam {
                    team_id: TeamId::from(format!("team-{seed}")),
                    seed: u32::try_from(seed).unwrap(),
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

    for _ in 0..32 {
        let stage = tournament.stage(&stage_id).unwrap();
        if stage.status() == StageStatus::Finished {
            break;
        }
        let round = stage.current_round().unwrap().clone();
        for match_ in round
            .matches
            .iter()
            .filter(|match_| match_.status == MatchStatus::Pending)
        {
            tournament.start_match(&stage_id, &match_.match_id).unwrap();
            let double_loss = match scenario {
                DoubleEliminationRankScenario::NoVacancy => false,
                DoubleEliminationRankScenario::OpeningDoubleLoss => {
                    round.kind == RoundKind::Winners
                        && round.round_number == 1
                        && match_.sequence == 1
                }
                DoubleEliminationRankScenario::OriginalSlotVacancies => {
                    round.kind == RoundKind::Winners
                        && round.round_number == 1
                        && matches!(match_.sequence, 2 | 3 | 4 | 6)
                }
                DoubleEliminationRankScenario::EmptyFirstTwoLowerLayers => {
                    round.kind == RoundKind::Winners
                        && round.round_number == 1
                        && matches!(match_.sequence, 2 | 3 | 4 | 6 | 8)
                }
                DoubleEliminationRankScenario::NoChampion => {
                    (round.kind == RoundKind::Winners && round.round_number == 3)
                        || (round.kind == RoundKind::Losers && round.round_number == 4)
                }
            };
            let result = if double_loss {
                MatchResult::DoubleLoss
            } else {
                MatchResult::win_with_games(
                    match_.participants[0].clone(),
                    match_.participants[1].clone(),
                    1,
                    0,
                )
            };
            tournament
                .submit_result(&stage_id, &match_.match_id, result)
                .unwrap();
        }
    }
    assert_eq!(
        tournament.stage(&stage_id).unwrap().status(),
        StageStatus::Finished
    );
    tournament
}

fn rank_for(tournament: &Tournament, stage_id: &StageId, team_id: &TeamId) -> Option<u32> {
    tournament
        .stage(stage_id)
        .unwrap()
        .standings()
        .iter()
        .find(|standing| &standing.team_id == team_id)
        .unwrap()
        .rank
}

fn normal_loser(match_: &fewfc::tournament::ScheduledMatch) -> &TeamId {
    match match_.result.as_ref().unwrap() {
        MatchResult::Win { loser_id, .. } | MatchResult::Forfeit { loser_id, .. } => loser_id,
        result => panic!("一般敗部交手必須有正常敗方，收到 {result:?}"),
    }
}

#[test]
fn vacant_double_elimination_preserves_later_lower_bracket_depths_and_skipped_rounds() {
    for (team_count, expected_third_lower_rank) in [(8_usize, 4_u32), (16, 6)] {
        let normal = complete_double_elimination_with_vacant_upper_semifinals(team_count, false);
        let vacant = complete_double_elimination_with_vacant_upper_semifinals(team_count, true);
        let stage_id = StageId::from(format!("ticket-20-double-{team_count}"));
        let normal_stage = normal.stage(&stage_id).unwrap();
        let vacant_stage = vacant.stage(&stage_id).unwrap();
        let normal_lower_rounds = normal_stage
            .rounds()
            .iter()
            .filter(|round| round.kind == RoundKind::Losers)
            .collect::<Vec<_>>();
        let vacant_lower_rounds = vacant_stage
            .rounds()
            .iter()
            .filter(|round| round.kind == RoundKind::Losers)
            .collect::<Vec<_>>();
        assert!(normal_lower_rounds.len() >= 3);
        assert!(vacant_lower_rounds.len() >= 3);
        let normal_third_lower = normal_lower_rounds[2];
        let vacant_third_lower = vacant_lower_rounds[2];
        assert_eq!(normal_third_lower.round_number, 6);
        assert_eq!(vacant_third_lower.round_number, 6);
        assert_eq!(vacant_third_lower.matches.len(), team_count / 8);

        for vacant_match in &vacant_third_lower.matches {
            let normal_match = normal_third_lower
                .matches
                .iter()
                .find(|match_| match_.match_id == vacant_match.match_id)
                .expect("空缺流程保留相同敗部交手")
                .clone();
            assert_eq!(vacant_match, &normal_match);
            let loser = normal_loser(vacant_match);
            assert_eq!(
                rank_for(&normal, &stage_id, loser),
                Some(expected_third_lower_rank)
            );
            assert_eq!(
                rank_for(&vacant, &stage_id, loser),
                Some(expected_third_lower_rank),
                "第 {team_count} 隊第三個敗部深度沿用原名次"
            );
        }
    }

    let normal = complete_double_elimination_with_vacant_upper_semifinals(16, false);
    let vacant = complete_double_elimination_with_vacant_upper_semifinals(16, true);
    let stage_id = StageId::from("ticket-20-double-16");
    let normal_stage = normal.stage(&stage_id).unwrap();
    let vacant_stage = vacant.stage(&stage_id).unwrap();
    let normal_lower_rounds = normal_stage
        .rounds()
        .iter()
        .filter(|round| round.kind == RoundKind::Losers)
        .collect::<Vec<_>>();
    let vacant_lower_rounds = vacant_stage
        .rounds()
        .iter()
        .filter(|round| round.kind == RoundKind::Losers)
        .collect::<Vec<_>>();
    let normal_fourth_lower = normal_lower_rounds[3];
    let vacant_fourth_lower = vacant_lower_rounds[3];
    assert_eq!(normal_fourth_lower.round_number, 8);
    assert_eq!(vacant_fourth_lower.round_number, 7);
    assert_eq!(normal_fourth_lower.matches.len(), 2);
    assert_eq!(vacant_fourth_lower.matches.len(), 1);
    let normal_match = &normal_fourth_lower.matches[0];
    let vacant_match = &vacant_fourth_lower.matches[0];
    let mut normalized_vacant_match = vacant_match.clone();
    normalized_vacant_match.match_id = normal_match.match_id.clone();
    normalized_vacant_match.round_number = normal_match.round_number;
    assert_eq!(&normalized_vacant_match, normal_match);
    let loser = normal_loser(normal_match);
    assert_eq!(loser, &TeamId::from("team-8"));
    assert_eq!(rank_for(&normal, &stage_id, loser), Some(4));
    assert_eq!(rank_for(&vacant, &stage_id, loser), Some(4));
}

#[test]
fn double_elimination_preserves_early_lower_ranks_with_or_without_a_champion() {
    let stage_id = StageId::from("ticket-20-rank-retention");
    let normal =
        complete_double_elimination_rank_scenario(8, DoubleEliminationRankScenario::NoVacancy);
    let normal_stage = normal.stage(&stage_id).unwrap();
    let normal_match = normal_stage
        .rounds()
        .iter()
        .flat_map(|round| &round.matches)
        .find(|match_| {
            match_.kind == RoundKind::Losers && match_.round_number == 2 && match_.sequence == 2
        })
        .unwrap();
    let loser = TeamId::from("team-6");
    assert_eq!(normal_loser(normal_match), &loser);
    assert_eq!(rank_for(&normal, &stage_id, &loser), Some(7));

    for scenario in [
        DoubleEliminationRankScenario::OpeningDoubleLoss,
        DoubleEliminationRankScenario::NoChampion,
    ] {
        let vacant = complete_double_elimination_rank_scenario(8, scenario);
        let vacant_stage = vacant.stage(&stage_id).unwrap();
        assert_eq!(vacant_stage.last_schedule_error(), None);
        assert!(vacant_stage.rounds().iter().all(|round| {
            round
                .matches
                .iter()
                .all(|match_| match_.status == MatchStatus::Completed && match_.result.is_some())
        }));
        let vacant_match = vacant_stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .find(|match_| match_.match_id == normal_match.match_id)
            .unwrap();
        assert_eq!(vacant_match, normal_match);
        assert_eq!(rank_for(&vacant, &stage_id, &loser), Some(7));
        if matches!(scenario, DoubleEliminationRankScenario::NoChampion) {
            assert_eq!(vacant_stage.winner(), None);
            assert!(vacant_stage.rounds().iter().all(|round| !matches!(
                round.kind,
                RoundKind::GrandFinal | RoundKind::GrandFinalReset
            )));
        }
    }
}

#[test]
fn double_elimination_single_fresh_loser_advances_through_original_lower_slots() {
    let stage_id = StageId::from("ticket-20-rank-retention");
    let normal =
        complete_double_elimination_rank_scenario(13, DoubleEliminationRankScenario::NoVacancy);
    let vacant = complete_double_elimination_rank_scenario(
        13,
        DoubleEliminationRankScenario::OriginalSlotVacancies,
    );
    let normal_stage = normal.stage(&stage_id).unwrap();
    let vacant_stage = vacant.stage(&stage_id).unwrap();
    assert_eq!(vacant_stage.last_schedule_error(), None);
    assert!(vacant_stage.rounds().iter().all(|round| {
        round
            .matches
            .iter()
            .all(|match_| match_.status == MatchStatus::Completed && match_.result.is_some())
    }));

    let lower_rounds = vacant_stage
        .rounds()
        .iter()
        .filter(|round| round.kind == RoundKind::Losers)
        .collect::<Vec<_>>();
    assert_eq!(
        lower_rounds
            .iter()
            .map(|round| round.round_number)
            .collect::<Vec<_>>(),
        vec![2, 4, 6, 8, 9]
    );
    let teams = |round: &Round| {
        round
            .matches
            .iter()
            .map(|match_| match_.participants.clone())
            .collect::<Vec<_>>()
    };
    let sorted_teams = |round: &Round| {
        let mut teams = teams(round);
        teams.sort();
        teams
    };
    assert_eq!(teams(lower_rounds[0]), vec![vec![TeamId::from("team-11")]]);
    assert_eq!(sorted_teams(lower_rounds[1]), {
        let mut expected = vec![vec![TeamId::from("team-11")], vec![TeamId::from("team-6")]];
        expected.sort();
        expected
    });
    assert_eq!(sorted_teams(lower_rounds[2]), {
        let mut expected = vec![
            vec![TeamId::from("team-11")],
            vec![TeamId::from("team-6")],
            vec![TeamId::from("team-3")],
        ];
        expected.sort();
        expected
    });
    assert_eq!(sorted_teams(lower_rounds[3]), {
        let mut expected = vec![
            vec![TeamId::from("team-11"), TeamId::from("team-6")],
            vec![TeamId::from("team-3"), TeamId::from("team-2")],
        ];
        expected.sort();
        expected
    });
    assert_eq!(
        teams(lower_rounds[4]),
        vec![vec![TeamId::from("team-11"), TeamId::from("team-3")]]
    );
    assert!(
        lower_rounds[..3]
            .iter()
            .flat_map(|round| &round.matches)
            .all(|match_| matches!(match_.result.as_ref(), Some(MatchResult::Bye { .. })))
    );

    for (team_id, normal_rank, expected_rank) in
        [("team-6", 9, 4), ("team-2", 4, 4), ("team-3", 6, 3)]
    {
        let team_id = TeamId::from(team_id);
        assert_eq!(rank_for(&normal, &stage_id, &team_id), Some(normal_rank));
        assert_eq!(rank_for(&vacant, &stage_id, &team_id), Some(expected_rank));
    }
    assert_eq!(
        normal_stage
            .rounds()
            .iter()
            .filter(|round| round.kind == RoundKind::Losers)
            .count(),
        5
    );
}

#[test]
fn double_elimination_preserves_depth_after_two_empty_lower_layers() {
    let stage_id = StageId::from("ticket-20-rank-retention");
    let vacant = complete_double_elimination_rank_scenario(
        13,
        DoubleEliminationRankScenario::EmptyFirstTwoLowerLayers,
    );
    let stage = vacant.stage(&stage_id).unwrap();
    assert_eq!(stage.last_schedule_error(), None);
    assert!(stage.rounds().iter().all(|round| {
        round
            .matches
            .iter()
            .all(|match_| match_.status == MatchStatus::Completed && match_.result.is_some())
    }));

    let lower_rounds = stage
        .rounds()
        .iter()
        .filter(|round| round.kind == RoundKind::Losers)
        .collect::<Vec<_>>();
    assert_eq!(
        lower_rounds
            .iter()
            .map(|round| round.round_number)
            .collect::<Vec<_>>(),
        vec![4, 6]
    );
    assert_eq!(lower_rounds[0].matches.len(), 1);
    assert_eq!(lower_rounds[0].matches[0].sequence, 3);
    assert_eq!(
        lower_rounds[0].matches[0].participants,
        vec![TeamId::from("team-3")]
    );
    assert!(matches!(
        lower_rounds[0].matches[0].result.as_ref(),
        Some(MatchResult::Bye { .. })
    ));
    assert_eq!(lower_rounds[1].matches.len(), 1);
    assert_eq!(lower_rounds[1].matches[0].sequence, 2);
    assert_eq!(
        lower_rounds[1].matches[0].participants,
        vec![TeamId::from("team-3"), TeamId::from("team-2")]
    );
    assert_eq!(
        normal_loser(&lower_rounds[1].matches[0]),
        &TeamId::from("team-2")
    );
    assert_eq!(
        rank_for(&vacant, &stage_id, &TeamId::from("team-2")),
        Some(4)
    );
}

fn assert_same_result_is_idempotent(
    tournament: &mut Tournament,
    stage_id: &StageId,
    match_: &fewfc::tournament::ScheduledMatch,
    result: MatchResult,
) {
    let before = serde_json::to_value(&*tournament).unwrap();
    assert_eq!(
        tournament
            .submit_result(stage_id, &match_.match_id, result)
            .unwrap(),
        UpdateOutcome::AlreadyRecorded
    );
    assert_eq!(serde_json::to_value(&*tournament).unwrap(), before);
}

fn assert_final_placements(
    tournament: &Tournament,
    stage_id: &StageId,
    champion: &TeamId,
    runner_up: &TeamId,
    third: &TeamId,
    fourth: &TeamId,
) {
    let stage = tournament.stage(stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), Some(champion.clone()));
    let ranks = stage
        .standings()
        .iter()
        .map(|standing| (standing.team_id.clone(), standing.rank))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(ranks.get(champion), Some(&Some(1)));
    assert_eq!(ranks.get(runner_up), Some(&Some(2)));
    assert_eq!(ranks.get(third), Some(&Some(3)));
    assert_eq!(ranks.get(fourth), Some(&Some(4)));
    assert!(
        stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .all(|match_| match_.status == MatchStatus::Completed && match_.result.is_some())
    );
}

#[test]
fn double_elimination_follows_fixed_team_paths_and_finishes_without_a_reset_when_winners_champion_wins()
 {
    let (tournament, stage_id, champion, runner_up, third, fourth) =
        complete_double_elimination(false);
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(
        stage
            .rounds()
            .iter()
            .map(|round| round.kind)
            .collect::<Vec<_>>(),
        [
            RoundKind::Winners,
            RoundKind::Losers,
            RoundKind::Winners,
            RoundKind::Losers,
            RoundKind::GrandFinal,
        ]
    );
    assert_final_placements(
        &tournament,
        &stage_id,
        &champion,
        &runner_up,
        &third,
        &fourth,
    );
}

#[test]
fn double_elimination_creates_one_new_reset_match_and_finishes_after_its_result() {
    let (tournament, stage_id, champion, runner_up, third, fourth) =
        complete_double_elimination(true);
    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(
        stage
            .rounds()
            .iter()
            .map(|round| round.kind)
            .collect::<Vec<_>>(),
        [
            RoundKind::Winners,
            RoundKind::Losers,
            RoundKind::Winners,
            RoundKind::Losers,
            RoundKind::GrandFinal,
            RoundKind::GrandFinalReset,
        ]
    );
    assert_final_placements(
        &tournament,
        &stage_id,
        &champion,
        &runner_up,
        &third,
        &fourth,
    );
}

#[test]
fn double_elimination_lower_final_double_loss_advances_only_the_winners_champion() {
    let (mut tournament, stage_id, winners_champion, lower_final) =
        double_elimination_before_lower_final();
    let double_loss = lower_final.matches[0].clone();
    assert_eq!(
        tournament
            .submit_result(&stage_id, &double_loss.match_id, MatchResult::DoubleLoss)
            .unwrap(),
        UpdateOutcome::Applied
    );

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), Some(winners_champion.clone()));
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(stage.rounds().last().unwrap().kind, RoundKind::Losers);
    assert_eq!(stage.rounds().len(), 4, "空敗部決賽不得造總決賽");
    let recorded = &stage.rounds().last().unwrap().matches[0];
    assert_eq!(recorded.match_id, double_loss.match_id);
    assert_eq!(recorded.participants, double_loss.participants);
    assert_eq!(recorded.status, MatchStatus::Completed);
    assert_eq!(recorded.result, Some(MatchResult::DoubleLoss));
    let ranks = stage
        .standings()
        .iter()
        .map(|row| (row.team_id.clone(), row.rank))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(ranks.get(&winners_champion), Some(&Some(1)));
    assert_eq!(ranks.get(&double_loss.participants[0]), Some(&None));
    assert_eq!(ranks.get(&double_loss.participants[1]), Some(&None));

    let snapshot = serde_json::to_value(&tournament).unwrap();
    let mut restored: Tournament = serde_json::from_value(snapshot.clone()).unwrap();
    assert_eq!(
        restored
            .submit_result(&stage_id, &double_loss.match_id, MatchResult::DoubleLoss)
            .unwrap(),
        UpdateOutcome::AlreadyRecorded
    );
    assert_eq!(serde_json::to_value(&restored).unwrap(), snapshot);
}

#[test]
fn double_elimination_grand_final_double_loss_leaves_first_two_places_vacant() {
    let (mut tournament, stage_id, winners_champion, lower_final) =
        double_elimination_before_lower_final();
    let lower_winner = lower_final.matches[0].participants[0].clone();
    let third = lower_final.matches[0].participants[1].clone();
    submit_winner(
        &mut tournament,
        &stage_id,
        &lower_final.matches[0].match_id,
        &lower_winner,
    );
    let grand_final = current_round(&tournament, &stage_id, RoundKind::GrandFinal);
    let final_match = grand_final.matches[0].clone();
    assert_eq!(
        tournament
            .submit_result(&stage_id, &final_match.match_id, MatchResult::DoubleLoss)
            .unwrap(),
        UpdateOutcome::Applied
    );

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), None);
    assert_eq!(stage.rounds().last().unwrap().kind, RoundKind::GrandFinal);
    assert_eq!(stage.rounds().len(), 5, "總決賽雙負不得建立重置賽");
    let ranks = stage
        .standings()
        .iter()
        .map(|row| (row.team_id.clone(), row.rank))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(ranks.get(&winners_champion), Some(&None));
    assert_eq!(ranks.get(&lower_winner), Some(&None));
    assert_eq!(
        ranks.get(&third),
        Some(&Some(3)),
        "總決賽前已決出的季軍保留"
    );
    assert!(
        stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .all(|match_| match_.kind != RoundKind::GrandFinalReset)
    );
    assert_same_result_is_idempotent(
        &mut tournament,
        &stage_id,
        &final_match,
        MatchResult::DoubleLoss,
    );
}

#[test]
fn double_elimination_reset_double_loss_preserves_the_already_decided_third_place() {
    let (mut tournament, stage_id, winners_champion, lower_final) =
        double_elimination_before_lower_final();
    let lower_winner = lower_final.matches[0].participants[0].clone();
    let third = lower_final.matches[0].participants[1].clone();
    submit_winner(
        &mut tournament,
        &stage_id,
        &lower_final.matches[0].match_id,
        &lower_winner,
    );
    let grand_final = current_round(&tournament, &stage_id, RoundKind::GrandFinal);
    let grand_final_match = grand_final.matches[0].clone();
    tournament
        .submit_result(
            &stage_id,
            &grand_final_match.match_id,
            MatchResult::win_with_games(lower_winner.clone(), winners_champion.clone(), 2, 0),
        )
        .unwrap();
    let reset = current_round(&tournament, &stage_id, RoundKind::GrandFinalReset);
    let reset_match = reset.matches[0].clone();
    assert_eq!(
        tournament
            .submit_result(&stage_id, &reset_match.match_id, MatchResult::DoubleLoss)
            .unwrap(),
        UpdateOutcome::Applied
    );

    let stage = tournament.stage(&stage_id).unwrap();
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.winner(), None);
    assert_eq!(
        stage.rounds().last().unwrap().kind,
        RoundKind::GrandFinalReset
    );
    let ranks = stage
        .standings()
        .iter()
        .map(|row| (row.team_id.clone(), row.rank))
        .collect::<BTreeMap<_, _>>();
    assert_eq!(ranks.get(&winners_champion), Some(&None));
    assert_eq!(ranks.get(&lower_winner), Some(&None));
    assert_eq!(ranks.get(&third), Some(&Some(3)));
    assert_same_result_is_idempotent(
        &mut tournament,
        &stage_id,
        &reset_match,
        MatchResult::DoubleLoss,
    );
}

#[test]
fn double_elimination_surviving_lower_champion_wins_the_vacant_grand_final_position() {
    let stage_id = StageId::from("double-eight-vacancy");
    let mut tournament = Tournament::new(TournamentConfig {
        tournament_id: "double-eight-vacancy".into(),
        stage: StageConfig {
            stage_id: stage_id.clone(),
            format: TournamentFormat::DoubleElimination,
            teams: (1..=8)
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
    let mut last_lower_winner = None;
    let mut double_losses = 0;

    for _ in 0..16 {
        let stage = tournament.stage(&stage_id).unwrap();
        if stage.status() == StageStatus::Finished {
            break;
        }
        let round = stage.current_round().unwrap().clone();
        let clear_winners_semifinals = round.kind == RoundKind::Winners && round.round_number == 3;
        for match_ in round
            .matches
            .iter()
            .filter(|match_| match_.status == MatchStatus::Pending)
        {
            tournament.start_match(&stage_id, &match_.match_id).unwrap();
            let result = if clear_winners_semifinals {
                double_losses += 1;
                MatchResult::DoubleLoss
            } else {
                if round.kind == RoundKind::Losers {
                    last_lower_winner = Some(match_.participants[0].clone());
                }
                MatchResult::win_with_games(
                    match_.participants[0].clone(),
                    match_.participants[1].clone(),
                    1,
                    0,
                )
            };
            tournament
                .submit_result(&stage_id, &match_.match_id, result)
                .unwrap();
        }
    }

    let stage = tournament.stage(&stage_id).unwrap();
    let lower_champion = last_lower_winner.expect("敗部冠軍已正常決出");
    assert_eq!(double_losses, 2);
    assert_eq!(stage.status(), StageStatus::Finished);
    assert_eq!(stage.last_schedule_error(), None);
    assert_eq!(stage.winner(), Some(lower_champion.clone()));
    assert_eq!(
        stage
            .standings()
            .iter()
            .find(|standing| standing.team_id == lower_champion)
            .unwrap()
            .rank,
        Some(1)
    );
    assert_eq!(
        stage
            .standings()
            .iter()
            .find(|standing| standing.team_id == TeamId::from("team-7"))
            .unwrap()
            .rank,
        Some(5),
        "上部空位不得清除 team-7 已由敗部第四輪確定的第五名"
    );
    assert!(
        stage
            .standings()
            .iter()
            .all(|standing| standing.rank != Some(2)),
        "總決賽沒有敗方，不能把較早敗部淘汰者壓成亞軍"
    );
    assert!(stage.rounds().iter().all(|round| !matches!(
        round.kind,
        RoundKind::GrandFinal | RoundKind::GrandFinalReset
    )));
}
