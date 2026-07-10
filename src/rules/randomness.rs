use crate::domain::{
    EchoRandomnessContinuation, GameError, GameEvent, GameResult, GameState,
    PouchRandomnessContinuation, RandomnessContinuation, RandomnessDeck,
    TribulationRandomnessContinuation, TrustedRandomnessAnswer, ValidationError,
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

    let current_order = current_order_for_request(state, &request.deck, &request.continuation)?;
    if !continuation_matches_current_order(
        &request.continuation,
        &request.current_order,
        current_order,
    ) {
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
        deck: request.deck.clone(),
        shuffled_order: answer.shuffled_order.clone(),
    }];
    let mut projected = state.clone();
    crate::rules::projection::apply_event(&mut projected, &events[0]);
    events.extend(after_randomness_events(
        &projected,
        &request.continuation,
        &request.deck,
    )?);
    Ok(events)
}

fn current_order_for_request<'a>(
    state: &'a GameState,
    deck: &'a RandomnessDeck,
    continuation: &RandomnessContinuation,
) -> GameResult<&'a [crate::domain::CardInstanceId]> {
    match (deck, continuation_validates_discard(continuation)) {
        (RandomnessDeck::Shared, false) => Ok(&state.deck),
        (RandomnessDeck::Shared, true) => Ok(&state.discard),
        (RandomnessDeck::Player(player), false) => state
            .deck_for(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone()))),
        (RandomnessDeck::Player(player), true) => state
            .discard_for(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone()))),
    }
}

fn continuation_validates_discard(continuation: &RandomnessContinuation) -> bool {
    matches!(
        continuation,
        RandomnessContinuation::Echo(EchoRandomnessContinuation::RingingMetalRecycleDiscard)
    )
}

fn continuation_matches_current_order(
    continuation: &RandomnessContinuation,
    requested_order: &[crate::domain::CardInstanceId],
    current_order: &[crate::domain::CardInstanceId],
) -> bool {
    if continuation_validates_discard(continuation) {
        let mut available = current_order.to_vec();
        requested_order.iter().all(|card| {
            available
                .iter()
                .position(|candidate| candidate == card)
                .map(|position| available.remove(position))
                .is_some()
        })
    } else {
        current_order == requested_order
    }
}

fn after_randomness_events(
    state: &GameState,
    continuation: &RandomnessContinuation,
    resolved_deck: &RandomnessDeck,
) -> GameResult<Vec<GameEvent>> {
    match continuation {
        RandomnessContinuation::Echo(continuation) => {
            crate::rules::echo::after_randomness_events(state, continuation)
        }
        RandomnessContinuation::Pouch(PouchRandomnessContinuation::InitialShuffle) => {
            crate::rules::pouch::after_initial_shuffle_randomness_events(state, resolved_deck)
        }
        RandomnessContinuation::Pouch(PouchRandomnessContinuation::SheepStealing) => Ok(Vec::new()),
        RandomnessContinuation::Tribulation(
            TribulationRandomnessContinuation::RustedForestShuffle,
        ) => crate::rules::tribulation::after_rusted_forest_randomness_events(state),
    }
}
