use crate::domain::{
    BaseRandomnessContinuation, ConfluenceRandomnessContinuation, GameError, GameEvent, GameResult,
    GameState, HeroRandomnessContinuation, PouchRandomnessContinuation, RandomnessContinuation,
    RandomnessDeck, RandomnessOperation, TribulationRandomnessContinuation,
    TrustedRandomnessAnswer, ValidationError,
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
        &request.continuation,
        request.operation.destination_deck(),
    )?);
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
    continuation: &RandomnessContinuation,
    resolved_deck: &RandomnessDeck,
) -> GameResult<Vec<GameEvent>> {
    match continuation {
        RandomnessContinuation::Base(BaseRandomnessContinuation::TurnDraw) => Ok(Vec::new()),
        RandomnessContinuation::Echo(continuation) => {
            crate::rules::echo::after_randomness_events(state, continuation)
        }
        RandomnessContinuation::Hero(HeroRandomnessContinuation::Revelation) => {
            crate::rules::hero::after_revelation_randomness_events(state)
        }
        RandomnessContinuation::Confluence(
            ConfluenceRandomnessContinuation::ClearWindTenThousandMiles,
        ) => crate::rules::confluence::after_clear_wind_randomness_events(state),
        RandomnessContinuation::Pouch(PouchRandomnessContinuation::InitialShuffle) => {
            crate::rules::pouch::after_initial_shuffle_randomness_events(state, resolved_deck)
        }
        RandomnessContinuation::Pouch(PouchRandomnessContinuation::SheepStealing) => Ok(Vec::new()),
        RandomnessContinuation::Tribulation(
            TribulationRandomnessContinuation::RustedForestShuffle,
        ) => crate::rules::tribulation::after_rusted_forest_randomness_events(state),
    }
}
