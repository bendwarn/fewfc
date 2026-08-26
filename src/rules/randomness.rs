use crate::domain::{
    GameError, GameEvent, GameResult, GameState, PendingResolution, RandomnessDeck,
    RandomnessOperation, TrustedRandomnessAnswer, ValidationError,
};

pub(crate) fn trusted_random_hand_count_for_formation(formation_id: &str) -> Option<usize> {
    (formation_id == crate::rules::dark::DARK_CHAOS).then_some(2)
}

pub(crate) fn trusted_random_hand_count_for_spirit_skill(
    skill: crate::domain::SpiritSkill,
) -> Option<usize> {
    (skill == crate::domain::SpiritSkill::EvilGaze).then_some(2)
}

pub(crate) fn resolve_trusted_randomness(
    state: &GameState,
    answer: &TrustedRandomnessAnswer,
) -> GameResult<Vec<GameEvent>> {
    let request = state
        .pending_randomness
        .as_ref()
        .ok_or(GameError::Validation(
            ValidationError::MissingPendingRandomness,
        ))?;
    if request.request_id != answer.request_id {
        return Err(GameError::Validation(
            ValidationError::MissingPendingRandomness,
        ));
    }
    let resolution = state
        .pending_resolution
        .as_ref()
        .ok_or(GameError::Validation(
            ValidationError::MissingPendingRandomness,
        ))?;

    let current_order = current_order_for_request(state, &request.operation)?;
    if request.current_order != current_order {
        return Err(GameError::Validation(
            ValidationError::StalePendingRandomness,
        ));
    }

    let mut expected = request.current_order.clone();
    let mut actual = answer.shuffled_order.clone();
    expected.sort();
    actual.sort();
    if expected != actual {
        return Err(GameError::Validation(
            ValidationError::InvalidRandomnessPermutation,
        ));
    }

    let mut events = vec![GameEvent::RandomnessResolved {
        request_id: request.request_id.clone(),
        operation: request.operation.clone(),
        shuffled_order: answer.shuffled_order.clone(),
    }];
    let mut projected = state.clone();
    crate::rules::projection::apply_event(&mut projected, &events[0]);
    if request.operation.is_discard_shuffle() {
        let recovery = crate::rules::confluence::tailwind_recovery_events(
            &projected,
            request.operation.destination_deck(),
        );
        for event in &recovery {
            crate::rules::projection::apply_event(&mut projected, event);
        }
        events.extend(recovery);
    }
    events.extend(after_randomness_events(
        &projected,
        resolution,
        request.operation.destination_deck(),
    )?);
    crate::rules::base::append_completed_formation_events(state, &mut events)?;
    Ok(events)
}

fn current_order_for_request<'a>(
    state: &'a GameState,
    operation: &'a RandomnessOperation,
) -> GameResult<&'a [crate::domain::CardInstanceId]> {
    match operation.source_pile() {
        RandomnessDeck::Shared if operation.is_discard_shuffle() => Ok(&state.discard),
        RandomnessDeck::Shared => Ok(&state.deck),
        RandomnessDeck::Player(player) if operation.is_discard_shuffle() => state
            .discard_for(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone()))),
        RandomnessDeck::Player(player) => state
            .deck_for(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone()))),
    }
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
        _ => Err(GameError::Validation(
            ValidationError::MissingPendingRandomness,
        )),
    }
}
