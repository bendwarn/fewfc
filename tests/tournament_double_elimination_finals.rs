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
