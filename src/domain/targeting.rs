use super::{GameError, GameResult, GameState, PlayerId, TeamId, ValidationError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RulePlayerTarget {
    SelfPlayer,
    PreviousPlayer,
    NextPlayer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuleTeamTarget {
    OwnSide,
    OpposingSide,
}

#[derive(Clone, Copy, Debug)]
pub struct TurnOrderTargets<'a> {
    state: &'a GameState,
}

impl<'a> TurnOrderTargets<'a> {
    pub fn new(state: &'a GameState) -> Self {
        Self { state }
    }

    pub fn player_target(
        &self,
        player: &PlayerId,
        target: RulePlayerTarget,
    ) -> GameResult<PlayerId> {
        match target {
            RulePlayerTarget::SelfPlayer => {
                if self
                    .state
                    .turn_order
                    .iter()
                    .any(|candidate| candidate == player)
                {
                    Ok(player.clone())
                } else {
                    Err(GameError::Validation(ValidationError::UnknownPlayer(
                        player.clone(),
                    )))
                }
            }
            RulePlayerTarget::PreviousPlayer => self.adjacent_player(player, -1),
            RulePlayerTarget::NextPlayer => self.adjacent_player(player, 1),
        }
    }

    pub fn team_target(&self, player: &PlayerId, target: RuleTeamTarget) -> GameResult<TeamId> {
        let own_team = self.team_of(player)?;

        match target {
            RuleTeamTarget::OwnSide => Ok(own_team),
            RuleTeamTarget::OpposingSide => self
                .state
                .hp
                .iter()
                .map(|team_hp| team_hp.team.clone())
                .find(|team| team != &own_team)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::MissingTeamHp(own_team.clone()))
                }),
        }
    }

    pub fn team_of(&self, player: &PlayerId) -> GameResult<TeamId> {
        self.state
            .players
            .iter()
            .find(|candidate| &candidate.id == player)
            .map(|player| player.team.clone())
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))
    }

    pub fn nth_future_turn_for_player(
        &self,
        player: &PlayerId,
        occurrence: usize,
    ) -> GameResult<u64> {
        let current_index = self
            .state
            .current_turn_index
            .min(self.state.turn_order.len().saturating_sub(1));
        let target_index = self
            .state
            .turn_order
            .iter()
            .position(|candidate| candidate == player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
        let player_count = self.state.turn_order.len();
        let first_distance = (target_index + player_count - current_index) % player_count;
        let first_distance = if first_distance == 0 {
            player_count
        } else {
            first_distance
        };

        Ok(self.state.turn_number
            + first_distance as u64
            + ((occurrence - 1) * player_count) as u64)
    }

    pub fn adjacent_player(&self, player: &PlayerId, offset: isize) -> GameResult<PlayerId> {
        let index = self
            .state
            .turn_order
            .iter()
            .position(|candidate| candidate == player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?
            as isize;
        let player_count = self.state.turn_order.len() as isize;
        let target_index = (index + offset).rem_euclid(player_count) as usize;

        Ok(self.state.turn_order[target_index].clone())
    }
}
