use crate::domain::{GameState, PlayerId, StatusDuration, StatusOwner, TimedEffectReduction};

pub(crate) fn status_reductions(
    state: &GameState,
    target: &PlayerId,
    is_eligible: impl Fn(&str) -> bool,
) -> Vec<TimedEffectReduction> {
    state
        .statuses
        .iter()
        .filter(|status| {
            status.owner == StatusOwner::Player(target.clone()) && is_eligible(&status.id)
        })
        .filter_map(|status| {
            reduced_duration(state, &status.duration).map(|new_duration| {
                TimedEffectReduction::Status {
                    status_id: status.id.clone(),
                    owner: status.owner.clone(),
                    old_duration: status.duration.clone(),
                    new_duration,
                }
            })
        })
        .collect()
}

fn reduced_duration(
    state: &GameState,
    duration: &StatusDuration,
) -> Option<Option<StatusDuration>> {
    match duration {
        StatusDuration::UntilTurnStart { .. } | StatusDuration::UntilTurnEnd { .. } => Some(None),
        StatusDuration::UntilTurnEndNumber {
            player,
            turn_number,
        } => {
            let previous_turn = turn_number.saturating_sub(state.turn_order.len() as u64);
            Some(
                (previous_turn > state.turn_number).then(|| StatusDuration::UntilTurnEndNumber {
                    player: player.clone(),
                    turn_number: previous_turn,
                }),
            )
        }
        StatusDuration::Permanent => None,
    }
}
