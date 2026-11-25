use fewfc::competition_clock::ClockConfig;
use fewfc::competition_match::MatchFormat;
use fewfc::tournament::*;

fn tournament(format: TournamentFormat, count: u32) -> Tournament {
    Tournament::new(TournamentConfig {
        tournament_id: "assessment-event".into(),
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
            match_format: MatchFormat::FirstToWins(1),
            clock_config: ClockConfig::default(),
            swiss_rounds: None,
            swiss_policy: None,
        },
    })
    .unwrap()
}

fn finish(
    mut tournament: Tournament,
    double_loss_at: Option<RoundKind>,
    reset: bool,
) -> Tournament {
    let stage_id = StageId("stage".into());
    let mut used_double_loss = false;
    while tournament.stage(&stage_id).unwrap().status() != StageStatus::Finished {
        let pending = tournament
            .stage(&stage_id)
            .unwrap()
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .find(|item| item.status == MatchStatus::Pending)
            .expect("進行中的測試賽程必須有待完成交手")
            .clone();
        let result = if double_loss_at == Some(pending.kind)
            && !used_double_loss
            && (pending.kind != RoundKind::Main
                || tournament
                    .stage(&stage_id)
                    .unwrap()
                    .rounds()
                    .last()
                    .unwrap()
                    .round_number
                    == tournament
                        .stage(&stage_id)
                        .unwrap()
                        .config()
                        .teams
                        .len()
                        .next_power_of_two()
                        .ilog2())
        {
            used_double_loss = true;
            MatchResult::DoubleLoss
        } else {
            let winner = usize::from(reset && pending.kind == RoundKind::GrandFinal);
            MatchResult::win(
                pending.participants[winner].clone(),
                pending.participants[1 - winner].clone(),
            )
        };
        tournament
            .submit_result(&stage_id, &pending.match_id, result)
            .unwrap();
    }
    tournament
}

#[test]
fn assessment_replays_complete_elimination_schedules_and_preserves_shared_places() {
    for format in [
        TournamentFormat::SingleElimination,
        TournamentFormat::DoubleElimination,
    ] {
        for count in 2..=12 {
            for double_loss_at in [
                None,
                Some(RoundKind::Winners),
                Some(RoundKind::Losers),
                Some(RoundKind::Main),
                Some(RoundKind::GrandFinal),
                Some(RoundKind::GrandFinalReset),
            ] {
                let tournament = finish(tournament(format, count), double_loss_at, true);
                let assessment = tournament.assess_completion().unwrap_or_else(|error| {
                    panic!("{format:?} {count} {double_loss_at:?}: {error:?}")
                });
                assert!(assessment.ready, "{format:?} {count} {double_loss_at:?}");
                assert!(assessment.stages[0].settled);
                assert_eq!(assessment.stages[0].placements.len(), count as usize);
                if double_loss_at.is_none() {
                    assert!(
                        assessment.stages[0].vacancies.is_empty(),
                        "{format:?} {count}: {:?}",
                        assessment.stages[0].vacancies
                    );
                }
            }
        }
    }
}

#[test]
fn final_double_loss_produces_closed_placements_vacancies_and_camel_case_facts() {
    let tournament = finish(
        tournament(TournamentFormat::SingleElimination, 4),
        Some(RoundKind::Main),
        false,
    );
    let assessment = tournament.assess_completion().unwrap();
    let stage = &assessment.stages[0];
    assert!(assessment.ready && stage.settled);
    assert_eq!(
        stage
            .placements
            .iter()
            .filter(|row| row.placement == TeamPlacement::NoPlacement)
            .count(),
        2
    );
    assert!(
        stage
            .placements
            .iter()
            .any(|row| row.placement == TeamPlacement::Ranked { rank: 3 })
    );
    assert_eq!(
        stage
            .vacancies
            .iter()
            .map(|fact| fact.rank)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(
        stage
            .explanations
            .contains(&PlacementExplanation::SingleFinalDoubleLoss)
    );
    let json = serde_json::to_value(&assessment).unwrap();
    assert_eq!(json["tournamentId"], "assessment-event");
    assert_eq!(json["stages"][0]["stageId"], "stage");
    assert!(json["stages"][0]["placements"][0].get("teamId").is_some());
    assert!(json["stages"][0]["placements"][0].get("team_id").is_none());
}

#[test]
fn active_stage_is_unresolved_and_assessment_is_read_only() {
    let tournament = tournament(TournamentFormat::SingleElimination, 2);
    let before = serde_json::to_value(&tournament).unwrap();
    let assessment = tournament.assess_completion().unwrap();
    assert!(!assessment.ready);
    assert!(!assessment.stages[0].settled);
    assert_eq!(assessment.stages[0].placements.len(), 2);
    assert_eq!(serde_json::to_value(&tournament).unwrap(), before);
}

#[test]
fn finished_stage_with_missing_round_is_not_proved_by_matching_recomputed_ranks() {
    let tournament = finish(
        tournament(TournamentFormat::SingleElimination, 4),
        None,
        false,
    );
    let mut snapshot = serde_json::to_value(tournament).unwrap();
    snapshot["stages"][0]["rounds"]
        .as_array_mut()
        .unwrap()
        .pop();
    let malformed: Tournament = serde_json::from_value(snapshot).unwrap();
    let malformed = malformed.ranking_projection();
    assert_eq!(malformed.ranking_projection(), malformed);
    assert_eq!(
        malformed.assess_completion(),
        Err(TournamentAssessmentError::InvalidFinishedStage {
            stage_id: StageId("stage".into())
        })
    );
}

#[test]
fn finished_stage_with_pending_match_or_schedule_error_is_rejected() {
    let tournament = finish(
        tournament(TournamentFormat::SingleElimination, 2),
        None,
        false,
    );
    for pending in [true, false] {
        let mut snapshot = serde_json::to_value(&tournament).unwrap();
        if pending {
            let match_ = &mut snapshot["stages"][0]["rounds"][0]["matches"][0];
            match_["status"] = serde_json::json!("pending");
            match_["result"] = serde_json::Value::Null;
            match_["started"] = serde_json::json!(false);
        } else {
            snapshot["stages"][0]["lastScheduleError"] = serde_json::json!("待裁判處理");
        }
        let malformed: Tournament = serde_json::from_value(snapshot).unwrap();
        let malformed = malformed.ranking_projection();
        assert_eq!(
            malformed.assess_completion(),
            Err(TournamentAssessmentError::InvalidFinishedStage {
                stage_id: StageId("stage".into())
            })
        );
    }
}

fn round_robin(count: u32) -> Tournament {
    let base = tournament(TournamentFormat::SingleElimination, count);
    let mut config = base.stages()[0].config().clone();
    config.format = TournamentFormat::RoundRobin;
    config.match_format = MatchFormat::FixedGames(1);
    Tournament::new(TournamentConfig {
        tournament_id: "assessment-event".into(),
        stage: config,
    })
    .unwrap()
}

#[test]
fn assessment_rejects_a_cancelled_round_robin_match_without_an_earlier_double_loss() {
    let tournament = finish(round_robin(2), None, false);
    let mut snapshot = serde_json::to_value(&tournament).unwrap();
    let scheduled = &mut snapshot["stages"][0]["rounds"][0]["matches"][0];
    scheduled["status"] = serde_json::json!("cancelled");
    scheduled["result"] = serde_json::Value::Null;
    scheduled["started"] = serde_json::json!(false);
    scheduled["cancellationReason"] = serde_json::json!("交手取消");
    let mut malformed: Tournament = serde_json::from_value(snapshot).unwrap();
    let stage_id = StageId("stage".into());
    malformed
        .adjudicate_ranking_tie(
            &stage_id,
            vec![TeamId("team-1".into()), TeamId("team-2".into())],
            RankingDecisionActor::Organizer {
                user_id: "organizer".into(),
            },
            "公開裁決".into(),
        )
        .unwrap();
    assert_eq!(
        malformed.assess_completion(),
        Err(TournamentAssessmentError::InvalidFinishedStage { stage_id })
    );
}

#[test]
fn assessment_accepts_round_robin_cancellations_preserved_after_a_post_completion_correction() {
    let mut tournament = round_robin(4);
    let stage_id = StageId("stage".into());
    let first = tournament.stages()[0].rounds()[0].matches[0].clone();
    tournament
        .submit_result(&stage_id, &first.match_id, MatchResult::DoubleLoss)
        .unwrap();
    let mut tournament = finish(tournament, None, false);
    let cancelled_count = |tournament: &Tournament| {
        tournament.stages()[0].rounds()[1..]
            .iter()
            .flat_map(|round| &round.matches)
            .filter(|scheduled| scheduled.status == MatchStatus::Cancelled)
            .count()
    };
    assert_eq!(cancelled_count(&tournament), 4);
    assert!(tournament.assess_completion().is_ok());

    tournament
        .correct_result(
            &stage_id,
            &first.match_id,
            MatchResult::win(first.participants[0].clone(), first.participants[1].clone()),
            "賽後更正交手結果".into(),
        )
        .unwrap();
    assert_eq!(cancelled_count(&tournament), 4);
    assert!(tournament.assess_completion().is_ok());
}

fn qualified_template(stage_id: &str, ranks: &[u32]) -> QualifiedStageTemplate {
    QualifiedStageTemplate {
        stage_id: StageId(stage_id.into()),
        format: TournamentFormat::SingleElimination,
        random_seed: 7,
        match_format: MatchFormat::FirstToWins(1),
        clock_config: ClockConfig::default(),
        swiss_rounds: None,
        swiss_policy: None,
        source_slots: ranks
            .iter()
            .enumerate()
            .map(|(index, rank)| StageQualificationSlot {
                source_stage_id: StageId("stage".into()),
                source: StageQualificationSource::UniqueStandingRank { rank: *rank },
                destination_seed: index as u32 + 1,
            })
            .collect(),
    }
}

#[test]
fn unresolved_formal_tie_is_not_a_corrupt_checkpoint_or_no_placement() {
    let mut tournament = round_robin(2);
    let stage_id = StageId("stage".into());
    let match_id = tournament.stages()[0].rounds()[0].matches[0]
        .match_id
        .clone();
    tournament
        .submit_result(&stage_id, &match_id, MatchResult::draw())
        .unwrap();
    let assessment = tournament.assess_completion().unwrap();
    assert!(!assessment.ready);
    assert!(!assessment.stages[0].settled);
    assert!(
        assessment.stages[0]
            .placements
            .iter()
            .all(|row| row.placement == TeamPlacement::Unresolved)
    );
    tournament
        .adjudicate_ranking_tie(
            &stage_id,
            vec![TeamId("team-1".into()), TeamId("team-2".into())],
            RankingDecisionActor::Organizer {
                user_id: "organizer".into(),
            },
            "公開裁決".into(),
        )
        .unwrap();
    assert!(tournament.assess_completion().unwrap().ready);
}

#[test]
fn settled_stage_facts_remain_available_while_a_later_stage_is_active() {
    let mut tournament = round_robin(4);
    let source_id = StageId("stage".into());
    let matches: Vec<_> = tournament.stages()[0]
        .rounds()
        .iter()
        .flat_map(|round| &round.matches)
        .filter(|match_| match_.status == MatchStatus::Pending)
        .cloned()
        .collect();
    for match_ in matches {
        let mut participants = match_.participants.clone();
        participants.sort();
        tournament
            .submit_result(
                &source_id,
                &match_.match_id,
                MatchResult::win(participants[0].clone(), participants[1].clone()),
            )
            .unwrap();
    }
    let plan = tournament
        .plan_qualified_stage(qualified_template("elimination", &[1, 2]))
        .unwrap();
    tournament.apply_qualified_stage(plan).unwrap();
    let final_id = tournament.stages()[1].rounds()[0].matches[0]
        .match_id
        .clone();
    tournament
        .submit_result(
            &StageId("elimination".into()),
            &final_id,
            MatchResult::DoubleLoss,
        )
        .unwrap();
    let plan = tournament
        .plan_qualified_stage(qualified_template("later", &[3, 4]))
        .unwrap();
    tournament.apply_qualified_stage(plan).unwrap();
    let assessment = tournament.assess_completion().unwrap();
    assert!(!assessment.ready);
    assert!(assessment.stages[1].settled);
    assert!(
        assessment.stages[1]
            .placements
            .iter()
            .all(|row| row.placement == TeamPlacement::NoPlacement)
    );
    assert_eq!(
        assessment.stages[1]
            .vacancies
            .iter()
            .map(|fact| fact.rank)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
    assert!(!assessment.stages[2].settled);
    let state = serde_json::json!({ "registration": null, "clock": null, "matches": {}, "tournament": tournament });
    let request = serde_json::json!({
        "serverActor": { "type": "player", "playerId": "reader" }, "trustedTimeMs": 0,
        "operation": { "type": "tournamentAssessCompletion" }, "serverState": state,
    });
    let response: serde_json::Value = serde_json::from_str(
        &fewfc::competition_web_api::handle_request_json(&request.to_string()).unwrap(),
    )
    .unwrap();
    assert_eq!(response["ok"], true);
    assert_eq!(response["serverState"], state);
    assert_eq!(response["public"]["assessment"]["ready"], false);
    assert_eq!(
        response["public"]["assessment"]["stages"][1]["settled"],
        true
    );
    assert_eq!(
        response["public"]["assessment"]["stages"][1]["placements"][0]["placement"]["type"],
        "noPlacement"
    );
    assert_eq!(
        response["public"]["assessment"]["stages"][2]["settled"],
        false
    );
}

#[test]
fn explicit_zero_and_one_team_resolutions_are_settled() {
    for double_loss in [false, true] {
        let mut tournament = if double_loss {
            finish(
                tournament(TournamentFormat::SingleElimination, 2),
                Some(RoundKind::Main),
                false,
            )
        } else {
            finish(
                tournament(TournamentFormat::SingleElimination, 2),
                None,
                false,
            )
        };
        let template = QualifiedStageTemplate {
            stage_id: StageId("qualified".into()),
            format: TournamentFormat::SingleElimination,
            random_seed: 1,
            match_format: MatchFormat::FirstToWins(1),
            clock_config: ClockConfig::default(),
            swiss_rounds: None,
            swiss_policy: None,
            source_slots: vec![StageQualificationSlot {
                source_stage_id: StageId("stage".into()),
                source: StageQualificationSource::FinishedStageWinner,
                destination_seed: 1,
            }],
        };
        let plan = tournament.plan_qualified_stage(template).unwrap();
        tournament.apply_qualified_stage(plan).unwrap();
        let assessment = tournament.assess_completion().unwrap();
        assert!(assessment.ready);
        let qualified = &assessment.stages[1];
        assert!(qualified.settled);
        if double_loss {
            assert!(qualified.placements.is_empty());
            assert_eq!(
                qualified.vacancies[0].reason,
                PlacementVacancyReason::NoQualifiedParticipants
            );
        } else {
            assert_eq!(
                qualified.placements[0].placement,
                TeamPlacement::Ranked { rank: 1 }
            );
        }
    }
}

#[test]
fn assessment_replays_withdrawal_and_preserves_recorded_placement() {
    for format in [
        TournamentFormat::SingleElimination,
        TournamentFormat::DoubleElimination,
    ] {
        let mut tournament = tournament(format, 4);
        let stage_id = StageId("stage".into());
        let withdrawn = tournament.stages()[0].rounds()[0].matches[0].participants[0].clone();
        tournament.drop_team(&stage_id, &withdrawn).unwrap();
        let tournament = finish(tournament, None, false);
        let assessment = tournament.assess_completion().unwrap();
        assert!(assessment.ready);
        let rank = tournament.stages()[0]
            .standings()
            .iter()
            .find(|row| row.team_id == withdrawn)
            .unwrap()
            .rank;
        let expected = rank.map_or(TeamPlacement::NoPlacement, |rank| TeamPlacement::Ranked {
            rank,
        });
        assert_eq!(
            assessment.stages[0]
                .placements
                .iter()
                .find(|row| row.team_id == withdrawn)
                .unwrap()
                .placement,
            expected
        );
    }
}

#[test]
fn assessment_allows_withdrawal_after_a_forfeit_and_completed_bronze_match() {
    let mut tournament = tournament(TournamentFormat::SingleElimination, 4);
    let stage_id = StageId("stage".into());
    let first = tournament.stages()[0].rounds()[0].matches[0].clone();
    let loser = first.participants[1].clone();
    tournament
        .submit_result(
            &stage_id,
            &first.match_id,
            MatchResult::forfeit(first.participants[0].clone(), loser.clone()),
        )
        .unwrap();
    let mut tournament = finish(tournament, None, false);
    assert!(
        tournament.stages()[0]
            .rounds()
            .last()
            .unwrap()
            .matches
            .iter()
            .any(|match_| match_.purpose == MatchPurpose::Bronze
                && match_.participants.contains(&loser))
    );
    tournament.drop_team(&stage_id, &loser).unwrap();
    assert!(tournament.assess_completion().unwrap().ready);
}

#[test]
fn reset_double_loss_explanation_uses_camel_case_fields() {
    let tournament = finish(
        tournament(TournamentFormat::DoubleElimination, 4),
        Some(RoundKind::GrandFinalReset),
        true,
    );
    let assessment = tournament.assess_completion().unwrap();
    let explanation = serde_json::to_value(&assessment.stages[0].explanations[0]).unwrap();
    assert_eq!(
        explanation,
        serde_json::json!({"type": "doubleResetDoubleLoss", "thirdPlaceDecided": true})
    );
}

#[test]
fn both_no_shows_close_the_final_vacancy_but_one_no_show_awards_the_winner() {
    for both_absent in [false, true] {
        let mut tournament = tournament(TournamentFormat::SingleElimination, 2);
        let stage_id = StageId("stage".into());
        let final_match = tournament.stages()[0].rounds()[0].matches[0].clone();
        let absent_teams = if both_absent {
            final_match.participants.clone()
        } else {
            vec![final_match.participants[1].clone()]
        };
        tournament
            .submit_result(
                &stage_id,
                &final_match.match_id,
                MatchResult::NoShow {
                    absent_teams,
                    winner_id: (!both_absent).then(|| final_match.participants[0].clone()),
                },
            )
            .unwrap();
        let assessment = tournament.assess_completion().unwrap();
        assert!(assessment.ready);
        let stage = &assessment.stages[0];
        if both_absent {
            assert_eq!(
                stage
                    .vacancies
                    .iter()
                    .map(|fact| fact.rank)
                    .collect::<Vec<_>>(),
                vec![1, 2]
            );
            assert!(
                stage
                    .explanations
                    .contains(&PlacementExplanation::SingleFinalDoubleLoss)
            );
            assert!(
                stage
                    .placements
                    .iter()
                    .all(|fact| fact.placement == TeamPlacement::NoPlacement)
            );
        } else {
            assert!(stage.vacancies.is_empty());
            assert!(stage.explanations.is_empty());
            assert!(
                stage
                    .placements
                    .iter()
                    .any(|fact| fact.placement == TeamPlacement::Ranked { rank: 1 })
            );
        }
    }
}

#[test]
fn bronze_double_loss_proves_third_and_fourth_vacancies_without_erasing_final_places() {
    for both_absent in [false, true] {
        let mut tournament = tournament(TournamentFormat::SingleElimination, 4);
        let stage_id = StageId("stage".into());
        while tournament.stages()[0].status() != StageStatus::Finished {
            let match_ = tournament.stages()[0]
                .rounds()
                .iter()
                .flat_map(|round| &round.matches)
                .find(|match_| match_.status == MatchStatus::Pending)
                .unwrap()
                .clone();
            let result = if match_.purpose == MatchPurpose::Bronze {
                if both_absent {
                    MatchResult::NoShow {
                        absent_teams: match_.participants.clone(),
                        winner_id: None,
                    }
                } else {
                    MatchResult::DoubleLoss
                }
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
        let assessment = tournament.assess_completion().unwrap();
        assert!(assessment.ready);
        assert_eq!(
            assessment.stages[0].vacancies,
            vec![
                PlacementVacancyFact {
                    rank: 3,
                    reason: PlacementVacancyReason::BronzeDoubleLoss
                },
                PlacementVacancyFact {
                    rank: 4,
                    reason: PlacementVacancyReason::BronzeDoubleLoss
                },
            ]
        );
        assert!(
            assessment.stages[0]
                .explanations
                .contains(&PlacementExplanation::SingleBronzeDoubleLoss)
        );
        assert!(
            assessment.stages[0]
                .placements
                .iter()
                .any(|row| row.placement == TeamPlacement::Ranked { rank: 1 })
        );
        assert!(
            assessment.stages[0]
                .placements
                .iter()
                .any(|row| row.placement == TeamPlacement::Ranked { rank: 2 })
        );
    }
    let ordinary = finish(
        tournament(TournamentFormat::SingleElimination, 4),
        None,
        false,
    );
    assert!(
        ordinary.assess_completion().unwrap().stages[0]
            .vacancies
            .is_empty()
    );
}

#[test]
fn elimination_exit_facts_do_not_treat_gaps_from_shared_ranks_as_vacancies() {
    for all_first_round_double_loss in [false, true] {
        let mut tournament = tournament(TournamentFormat::SingleElimination, 8);
        let stage_id = StageId("stage".into());
        while tournament.stages()[0].status() != StageStatus::Finished {
            let match_ = tournament.stages()[0]
                .rounds()
                .iter()
                .flat_map(|round| &round.matches)
                .find(|match_| match_.status == MatchStatus::Pending)
                .unwrap()
                .clone();
            let is_double_loss =
                match_.round_number == 1 && (all_first_round_double_loss || match_.sequence == 1);
            let result = if is_double_loss {
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
        let stage = &tournament.assess_completion().unwrap().stages[0];
        assert_eq!(
            stage.vacancies.iter().any(|fact| fact.rank == 5),
            all_first_round_double_loss
        );
        assert!(
            stage
                .vacancies
                .iter()
                .all(|fact| ![6, 7, 8].contains(&fact.rank))
        );
        if all_first_round_double_loss {
            assert_eq!(
                stage
                    .vacancies
                    .iter()
                    .map(|fact| fact.rank)
                    .collect::<Vec<_>>(),
                vec![1, 2, 3, 4, 5]
            );
            assert!(
                stage
                    .explanations
                    .contains(&PlacementExplanation::EliminationPlaceVacant { rank: 5 })
            );
        } else {
            assert!(
                stage
                    .placements
                    .iter()
                    .any(|row| row.placement == TeamPlacement::Ranked { rank: 5 })
            );
        }
    }
}

#[test]
fn double_elimination_lower_double_loss_proves_the_unawarded_exit_place() {
    let mut tournament = tournament(TournamentFormat::DoubleElimination, 4);
    let stage_id = StageId("stage".into());
    let mut lower_double_loss = false;
    while tournament.stages()[0].status() != StageStatus::Finished {
        let match_ = tournament.stages()[0]
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .find(|match_| match_.status == MatchStatus::Pending)
            .unwrap()
            .clone();
        let result = if match_.kind == RoundKind::Losers && !lower_double_loss {
            lower_double_loss = true;
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
    let assessment = tournament.assess_completion().unwrap();
    assert!(assessment.ready && lower_double_loss);
    assert!(
        assessment.stages[0]
            .vacancies
            .contains(&PlacementVacancyFact {
                rank: 4,
                reason: PlacementVacancyReason::EliminationDoubleLoss,
            })
    );
    assert!(
        assessment.stages[0]
            .explanations
            .contains(&PlacementExplanation::EliminationPlaceVacant { rank: 4 })
    );
}
