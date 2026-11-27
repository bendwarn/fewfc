//! 房間操作限時的終局判定。期限由房間伺服器管理。

use crate::domain::{
    GameConclusion, GameEndCause, GameError, GameEvent, GameOutcome, GamePreparationStage,
    GameResult, GameState, GameStatus, Phase, PlayerId, ValidationError,
};

pub(super) fn room_time_expiry_event(
    state: &GameState,
    players: &[PlayerId],
) -> GameResult<GameEvent> {
    let invalid = GameError::Validation(ValidationError::InvalidRoomTimeExpiry);
    let awaiting: Vec<&PlayerId> = match &state.status {
        GameStatus::Preparing {
            stage: GamePreparationStage::InitialPouchSelection,
        } => state
            .turn_order
            .iter()
            .filter(|player| state.pouch_for(player).is_none())
            .collect(),
        GameStatus::InProgress if state.pending_randomness.is_none() => {
            if let Some(choice) = &state.pending_choice {
                vec![&choice.player]
            } else if state.phase == Phase::ActiveEffects {
                state.current_player().into_iter().collect()
            } else {
                Vec::new()
            }
        }
        _ => Vec::new(),
    };
    if players.is_empty()
        || players.len() != awaiting.len()
        || !awaiting.iter().all(|player| players.contains(player))
    {
        return Err(invalid);
    }

    let teams: Vec<_> = state
        .hp
        .iter()
        .map(|entry| &entry.team)
        .filter(|team| {
            state
                .players
                .iter()
                .any(|player| &player.team == *team && players.contains(&player.id))
        })
        .cloned()
        .collect();
    let surviving_teams: Vec<_> = state
        .hp
        .iter()
        .map(|entry| &entry.team)
        .filter(|team| !teams.contains(team))
        .collect();
    let outcome = match surviving_teams.as_slice() {
        [winner] => GameOutcome::Winner((*winner).clone()),
        _ => GameOutcome::Draw,
    };
    Ok(GameEvent::GameEnded {
        conclusion: GameConclusion::new(outcome, vec![GameEndCause::RoomTimeExpired { teams }]),
    })
}
