use crate::domain::{
    GameError, GameResult, GameState, PendingResolution, RandomnessDeck, RandomnessOperation,
    TrustedRandomnessAnswer, ValidationError,
};

/// 已驗證的受信任隨機性輸入。只有本模組能建立它，解析模組只能消費它。
pub(crate) struct ValidatedPendingRandomnessInput {
    resolution: PendingResolution,
    request_id: crate::domain::RandomnessRequestId,
    operation: RandomnessOperation,
    shuffled_order: Vec<crate::domain::CardInstanceId>,
}

impl ValidatedPendingRandomnessInput {
    fn new(
        resolution: PendingResolution,
        request_id: crate::domain::RandomnessRequestId,
        operation: RandomnessOperation,
        shuffled_order: Vec<crate::domain::CardInstanceId>,
    ) -> Self {
        Self {
            resolution,
            request_id,
            operation,
            shuffled_order,
        }
    }

    pub(crate) fn into_parts(
        self,
    ) -> (
        PendingResolution,
        crate::domain::RandomnessRequestId,
        RandomnessOperation,
        Vec<crate::domain::CardInstanceId>,
    ) {
        (
            self.resolution,
            self.request_id,
            self.operation,
            self.shuffled_order,
        )
    }
}

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
) -> GameResult<Vec<crate::domain::GameEvent>> {
    let input = validate_pending_input(state, answer)?;
    crate::rules::pending_resolution::resume(
        state,
        crate::rules::pending_resolution::ValidatedPendingInput::Randomness(input),
    )
}

/// 驗證外部隨機性答案，並把它封裝成只能交給 Pending Resolution 的 token。
pub(crate) fn validate_pending_input(
    state: &GameState,
    answer: &TrustedRandomnessAnswer,
) -> GameResult<ValidatedPendingRandomnessInput> {
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
        .ok_or(GameError::EngineInvariant(
            crate::domain::EngineInvariantError::InvalidPendingResolution,
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

    Ok(ValidatedPendingRandomnessInput::new(
        resolution.clone(),
        request.request_id.clone(),
        request.operation.clone(),
        answer.shuffled_order.clone(),
    ))
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
