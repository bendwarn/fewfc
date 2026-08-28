//! 已接受規則流程在等待選擇或受信任隨機性後的唯一續行接縫。

use crate::domain::{
    ChoiceAnswer, EngineInvariantError, GameError, GameEvent, GameResult, GameState, GameStatus,
    PendingResolution, Phase, PlayerId, RandomnessDeck,
};

/// 兩種外部輸入都必須先由各自的擁有者驗證。這個 closed enum 讓呼叫端只有
/// 一個 hand off 介面，而不把續行機制分類成兩套公開流程。
pub(crate) enum ValidatedPendingInput {
    Choice(crate::rules::pending_choice::ValidatedPendingChoiceInput),
    Randomness(crate::rules::randomness::ValidatedPendingRandomnessInput),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaitingMedium {
    Choice,
    Randomness,
}

/// 將已驗證輸入交給唯一的 Pending Resolution 接縫。
///
/// 此 deep module 擁有 canonical input event 的投影、Discard Shuffle 的後果排序、
/// exhaustive typed delegation，以及完成後等待狀態的 invariant。它刻意停在
/// Pending Resolution 完成處，不跨越 application 的 Player Decision seam。
pub(crate) fn resume(
    state: &GameState,
    input: ValidatedPendingInput,
) -> GameResult<Vec<GameEvent>> {
    let mut events = match input {
        ValidatedPendingInput::Choice(input) => resume_choice(state, input)?,
        ValidatedPendingInput::Randomness(input) => resume_randomness(state, input)?,
    };
    append_completed_formation_events(state, &mut events)?;
    ensure_waiting_invariant(state, &events)?;
    Ok(events)
}

fn resume_choice(
    state: &GameState,
    input: crate::rules::pending_choice::ValidatedPendingChoiceInput,
) -> GameResult<Vec<GameEvent>> {
    let (choice, resolution, player, choice_id, answer) = input.into_parts();
    if waiting_medium(&resolution) != WaitingMedium::Choice {
        return Err(invalid_pending_resolution());
    }
    let mut events = vec![GameEvent::ChoiceMade {
        player: player.clone(),
        choice_id,
        answer: answer.clone(),
    }];
    // ChoiceMade 會清除等待狀態；續行必須從此標準投影狀態規劃，才能安全地
    // 建立下一個 Choice 或 Randomness，而不取代目前的 canonical input。
    let mut projected = state.clone();
    crate::rules::projection::apply_event(&mut projected, &events[0]);

    match &resolution {
        PendingResolution::TurnDrawDiscard => {
            resume_turn_draw(&projected, &player, answer, &mut events)?;
        }
        PendingResolution::EchoPureFireTarget
        | PendingResolution::EchoSplitEarthFormation
        | PendingResolution::EchoRingingMetalDeckCard
        | PendingResolution::EchoPlantEarthMelody
        | PendingResolution::EchoCost { .. } => {
            if let Some(resumed) =
                crate::rules::echo::answer_choice(&projected, &player, &resolution, &answer)?
            {
                events.extend(resumed);
            }
        }
        PendingResolution::TribulationEarthRendingEnvironment
        | PendingResolution::TribulationEarthRendingCard => {
            if let Some(resumed) =
                crate::rules::tribulation::answer_choice(&projected, &resolution, &answer)?
            {
                events.extend(resumed);
            }
        }
        PendingResolution::PouchChain => {
            if let Some(resumed) =
                crate::rules::pouch::answer_chain_choice(&projected, &player, &answer)?
            {
                events.extend(resumed);
            }
        }
        PendingResolution::PouchSheepStealingChoice => {
            if let Some(resumed) =
                crate::rules::pouch::answer_sheep_choice(&projected, &choice, &player, &answer)?
            {
                events.extend(resumed);
            }
        }
        PendingResolution::HolyWindTakeHighest
        | PendingResolution::ChaosReturnTwo
        | PendingResolution::HeroRevelationKeepOne
        | PendingResolution::JianghuAzureCloudStepReturnOne
        | PendingResolution::ConfluenceDiscardInspectedCard { .. }
        | PendingResolution::ConfluenceClearWindKeepCards
        | PendingResolution::ConfluenceClearWindDiscardTop => {
            let ChoiceAnswer::Cards { cards } = &answer else {
                unreachable!("validated card resolution has a Card answer")
            };
            events.extend(crate::rules::base::formation_use::answer_choice(
                &projected,
                &choice,
                &player,
                &resolution,
                cards,
            )?);
        }
        PendingResolution::TurnDraw
        | PendingResolution::SpiritDeathOmen { .. }
        | PendingResolution::PouchInitialShuffle
        | PendingResolution::PouchChainRecycle
        | PendingResolution::PouchChainPostSearch { .. }
        | PendingResolution::PouchSheepStealingRecycle { .. }
        | PendingResolution::PouchSheepStealing { .. }
        | PendingResolution::TribulationRustedForestDiscardShuffle
        | PendingResolution::TribulationRustedForestShuffle
        | PendingResolution::EchoRingingMetalRecycleDiscard
        | PendingResolution::EchoRingingMetalPostSearch
        | PendingResolution::HeroRevelation
        | PendingResolution::ConfluenceClearWindTenThousandMiles
        | PendingResolution::ConfluenceClearWindRevealTop => {
            return Err(invalid_pending_resolution());
        }
    }
    Ok(events)
}

fn resume_turn_draw(
    state: &GameState,
    player: &PlayerId,
    answer: ChoiceAnswer,
    events: &mut Vec<GameEvent>,
) -> GameResult<()> {
    ensure_current_player(state, player)?;
    if state.phase != Phase::TurnDraw {
        return Err(GameError::Validation(
            crate::domain::ValidationError::WrongPhase {
                expected: Phase::TurnDraw,
                actual: state.phase,
            },
        ));
    }
    let ChoiceAnswer::Cards { cards } = answer else {
        unreachable!("validated Turn Draw answer is a Card answer")
    };
    let discard = cards[0];
    let kept_cards = state
        .turn_draw_pool
        .iter()
        .copied()
        .filter(|card| *card != discard)
        .collect();
    events.push(GameEvent::TurnDrawResolved {
        player: player.clone(),
        discard,
        kept_cards,
    });
    if let Some(owned) = state.spirit_for(player)
        && owned.power < 6
        && !crate::rules::pouch::spirit_is_suppressed(state, player)
        && crate::rules::spirit::turn_discard_charges(state, owned.spirit, discard)
    {
        events.push(GameEvent::SpiritPowerChanged {
            player: player.clone(),
            spirit: owned.spirit,
            old_power: owned.power,
            delta: 1,
            new_power: owned.power + 1,
            reason: crate::domain::SpiritPowerChangeReason::TurnDrawDiscard { card: discard },
        });
    }
    Ok(())
}

fn resume_randomness(
    state: &GameState,
    input: crate::rules::randomness::ValidatedPendingRandomnessInput,
) -> GameResult<Vec<GameEvent>> {
    let (resolution, request_id, operation, shuffled_order) = input.into_parts();
    if waiting_medium(&resolution) != WaitingMedium::Randomness {
        return Err(invalid_pending_resolution());
    }
    let mut events = vec![GameEvent::RandomnessResolved {
        request_id,
        operation: operation.clone(),
        shuffled_order,
    }];
    let mut projected = state.clone();
    crate::rules::projection::apply_event(&mut projected, &events[0]);
    if operation.is_discard_shuffle() {
        let recovery = crate::rules::confluence::tailwind_recovery_events(
            &projected,
            operation.destination_deck(),
        );
        for event in &recovery {
            crate::rules::projection::apply_event(&mut projected, event);
        }
        events.extend(recovery);
    }
    events.extend(after_randomness_events(
        &projected,
        &resolution,
        operation.destination_deck(),
    )?);
    Ok(events)
}

fn after_randomness_events(
    state: &GameState,
    resolution: &PendingResolution,
    resolved_deck: &RandomnessDeck,
) -> GameResult<Vec<GameEvent>> {
    match resolution {
        PendingResolution::TurnDraw => Ok(Vec::new()),
        PendingResolution::SpiritDeathOmen { player } => {
            crate::rules::spirit::after_death_omen_randomness_events(state, player)
        }
        PendingResolution::EchoRingingMetalRecycleDiscard
        | PendingResolution::EchoRingingMetalPostSearch => {
            crate::rules::echo::after_randomness_events(state, resolution)
        }
        PendingResolution::HeroRevelation => {
            crate::rules::hero::after_revelation_randomness_events(state)
        }
        PendingResolution::ConfluenceClearWindTenThousandMiles => {
            crate::rules::confluence::after_clear_wind_randomness_events(state)
        }
        PendingResolution::ConfluenceClearWindRevealTop => {
            crate::rules::confluence::after_clear_wind_supply_events(state)
        }
        PendingResolution::PouchInitialShuffle => {
            crate::rules::pouch::after_initial_shuffle_randomness_events(state, resolved_deck)
        }
        PendingResolution::PouchChainRecycle => {
            crate::rules::pouch::after_chain_recycle_randomness_events(state)
        }
        PendingResolution::PouchChainPostSearch { player } => {
            crate::rules::pouch::after_chain_post_search_randomness_events(state, player)
        }
        PendingResolution::PouchSheepStealingRecycle { source_card } => {
            crate::rules::pouch::after_sheep_recycle_randomness_events(state, *source_card)
        }
        PendingResolution::PouchSheepStealing { source_card, owner } => {
            Ok(vec![GameEvent::PouchConsumed {
                owner: owner.clone(),
                card: *source_card,
            }])
        }
        PendingResolution::TribulationRustedForestDiscardShuffle => {
            crate::rules::tribulation::after_rusted_forest_discard_shuffle_events(state)
        }
        PendingResolution::TribulationRustedForestShuffle => {
            crate::rules::tribulation::after_rusted_forest_randomness_events(state)
        }
        PendingResolution::TurnDrawDiscard
        | PendingResolution::HolyWindTakeHighest
        | PendingResolution::ChaosReturnTwo
        | PendingResolution::EchoPureFireTarget
        | PendingResolution::EchoSplitEarthFormation
        | PendingResolution::EchoRingingMetalDeckCard
        | PendingResolution::EchoPlantEarthMelody
        | PendingResolution::EchoCost { .. }
        | PendingResolution::HeroRevelationKeepOne
        | PendingResolution::JianghuAzureCloudStepReturnOne
        | PendingResolution::ConfluenceDiscardInspectedCard { .. }
        | PendingResolution::ConfluenceClearWindKeepCards
        | PendingResolution::ConfluenceClearWindDiscardTop
        | PendingResolution::PouchChain
        | PendingResolution::PouchSheepStealingChoice
        | PendingResolution::TribulationEarthRendingEnvironment
        | PendingResolution::TribulationEarthRendingCard => Err(invalid_pending_resolution()),
    }
}

fn ensure_waiting_invariant(state: &GameState, events: &[GameEvent]) -> GameResult<()> {
    let mut projected = state.clone();
    for event in events {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    match (
        projected.pending_choice.as_ref(),
        projected.pending_randomness.as_ref(),
        projected.pending_resolution.as_ref(),
    ) {
        (None, None, None) => Ok(()),
        (Some(_), None, Some(resolution))
            if waiting_medium(resolution) == WaitingMedium::Choice =>
        {
            Ok(())
        }
        (None, Some(_), Some(resolution))
            if waiting_medium(resolution) == WaitingMedium::Randomness =>
        {
            Ok(())
        }
        _ => Err(invalid_pending_resolution()),
    }
}

/// Pending Resolution 已無後續等待輸入時，委派 Formation Use 的 typed completion
/// hook 並保持 canonical terminal 與實體卡牌收尾的既有順序。
fn append_completed_formation_events(
    state: &GameState,
    events: &mut Vec<GameEvent>,
) -> GameResult<()> {
    crate::rules::base::formation_use::append_completed_active_spell_post_formation_events(
        state, events,
    )?;
    crate::rules::base::append_terminal_game_end(state, events);
    if events
        .iter()
        .any(|event| matches!(event, GameEvent::GameEnded { .. }))
    {
        return Ok(());
    }

    let mut projected = state.clone();
    for event in events.iter() {
        crate::rules::projection::apply_event(&mut projected, event);
    }
    if projected.pending_choice.is_some()
        || projected.pending_randomness.is_some()
        || !matches!(projected.status, GameStatus::InProgress)
        || projected.phase != Phase::Action
    {
        return Ok(());
    }
    let Some(player) = projected.current_player().cloned() else {
        return Ok(());
    };
    let Some(formation) = projected
        .formation_area(&player)
        .and_then(|area| area.formation.as_ref())
    else {
        return Ok(());
    };
    if matches!(
        formation.state,
        crate::domain::FormationAreaState::FaceUpResolving
    ) {
        events.push(GameEvent::FormationCardsDiscarded {
            player,
            formation_id: formation.formation_id.clone(),
            cards: formation.cards.clone(),
        });
    }
    Ok(())
}

fn waiting_medium(resolution: &PendingResolution) -> WaitingMedium {
    match resolution {
        PendingResolution::TurnDrawDiscard
        | PendingResolution::HolyWindTakeHighest
        | PendingResolution::ChaosReturnTwo
        | PendingResolution::EchoPureFireTarget
        | PendingResolution::EchoSplitEarthFormation
        | PendingResolution::EchoRingingMetalDeckCard
        | PendingResolution::EchoPlantEarthMelody
        | PendingResolution::EchoCost { .. }
        | PendingResolution::HeroRevelationKeepOne
        | PendingResolution::JianghuAzureCloudStepReturnOne
        | PendingResolution::ConfluenceDiscardInspectedCard { .. }
        | PendingResolution::ConfluenceClearWindKeepCards
        | PendingResolution::ConfluenceClearWindDiscardTop
        | PendingResolution::PouchChain
        | PendingResolution::PouchSheepStealingChoice
        | PendingResolution::TribulationEarthRendingEnvironment
        | PendingResolution::TribulationEarthRendingCard => WaitingMedium::Choice,
        PendingResolution::TurnDraw
        | PendingResolution::SpiritDeathOmen { .. }
        | PendingResolution::PouchInitialShuffle
        | PendingResolution::PouchChainRecycle
        | PendingResolution::PouchChainPostSearch { .. }
        | PendingResolution::PouchSheepStealingRecycle { .. }
        | PendingResolution::PouchSheepStealing { .. }
        | PendingResolution::TribulationRustedForestDiscardShuffle
        | PendingResolution::TribulationRustedForestShuffle
        | PendingResolution::EchoRingingMetalRecycleDiscard
        | PendingResolution::EchoRingingMetalPostSearch
        | PendingResolution::HeroRevelation
        | PendingResolution::ConfluenceClearWindTenThousandMiles
        | PendingResolution::ConfluenceClearWindRevealTop => WaitingMedium::Randomness,
    }
}

fn invalid_pending_resolution() -> GameError {
    GameError::EngineInvariant(EngineInvariantError::InvalidPendingResolution)
}

fn ensure_current_player(state: &GameState, actual: &PlayerId) -> GameResult<()> {
    let expected = state.current_player().ok_or(GameError::Validation(
        crate::domain::ValidationError::EmptyTurnOrder,
    ))?;
    if expected == actual {
        Ok(())
    } else {
        Err(GameError::Validation(
            crate::domain::ValidationError::WrongPlayer {
                expected: expected.clone(),
                actual: actual.clone(),
            },
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::{WaitingMedium, waiting_medium};
    use crate::domain::{ConfluenceResonance, PendingResolution, PlayerId};

    #[test]
    fn every_pending_resolution_waiting_medium_is_exhaustively_classified() {
        let choice_resolutions = vec![
            PendingResolution::TurnDrawDiscard,
            PendingResolution::HolyWindTakeHighest,
            PendingResolution::ChaosReturnTwo,
            PendingResolution::EchoPureFireTarget,
            PendingResolution::EchoSplitEarthFormation,
            PendingResolution::EchoRingingMetalDeckCard,
            PendingResolution::EchoPlantEarthMelody,
            PendingResolution::EchoCost {
                melody_id: "melody".to_string(),
            },
            PendingResolution::HeroRevelationKeepOne,
            PendingResolution::JianghuAzureCloudStepReturnOne,
            PendingResolution::ConfluenceDiscardInspectedCard {
                resonance: ConfluenceResonance::Mirror,
                after: None,
            },
            PendingResolution::ConfluenceClearWindKeepCards,
            PendingResolution::ConfluenceClearWindDiscardTop,
            PendingResolution::PouchChain,
            PendingResolution::PouchSheepStealingChoice,
            PendingResolution::TribulationEarthRendingEnvironment,
            PendingResolution::TribulationEarthRendingCard,
        ];
        let randomness_resolutions = vec![
            PendingResolution::TurnDraw,
            PendingResolution::SpiritDeathOmen {
                player: PlayerId::new("p1"),
            },
            PendingResolution::PouchInitialShuffle,
            PendingResolution::PouchChainRecycle,
            PendingResolution::PouchChainPostSearch {
                player: PlayerId::new("p1"),
            },
            PendingResolution::PouchSheepStealingRecycle {
                source_card: crate::domain::CardInstanceId::new(1),
            },
            PendingResolution::PouchSheepStealing {
                source_card: crate::domain::CardInstanceId::new(1),
                owner: None,
            },
            PendingResolution::TribulationRustedForestDiscardShuffle,
            PendingResolution::TribulationRustedForestShuffle,
            PendingResolution::EchoRingingMetalRecycleDiscard,
            PendingResolution::EchoRingingMetalPostSearch,
            PendingResolution::HeroRevelation,
            PendingResolution::ConfluenceClearWindTenThousandMiles,
            PendingResolution::ConfluenceClearWindRevealTop,
        ];

        for resolution in choice_resolutions {
            assert_eq!(waiting_medium(&resolution), WaitingMedium::Choice);
        }
        for resolution in randomness_resolutions {
            assert_eq!(waiting_medium(&resolution), WaitingMedium::Randomness);
        }
    }
}
