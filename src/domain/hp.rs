//! 每個規則結算都以一份已驗證的 Team HP 帳本規劃生命變更。

use std::collections::{HashMap, HashSet};

use super::{EngineInvariantError, GameError, GameResult, GameState, HpChangeDelta, TeamId};

impl HpChangeDelta {
    fn new(team: TeamId, old_hp: i32, delta: i32, new_hp: i32, effective_delta: i32) -> Self {
        Self {
            team,
            old_hp,
            delta,
            new_hp,
            effective_delta,
        }
    }
}

/// 在套用防止效果或合法生命範圍前所請求的 HP 變更。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum HpChangeRequest {
    By(i32),
    To(i32),
    Prevented(i32),
}

/// 一般帳本與作用域限定的陣法效果共用的私有接縫。
trait HpChangePlanner {
    fn current_hp(&self, team: &TeamId) -> GameResult<i32>;
    fn plan(&mut self, team: &TeamId, request: HpChangeRequest) -> GameResult<HpChangeDelta>;
}

/// 單一外層規則結算使用的已驗證、有順序 HP 帳本。
#[derive(Debug)]
pub(crate) struct HpChangePlan {
    current: HashMap<TeamId, i32>,
    initial: HashMap<TeamId, i32>,
    order: Vec<TeamId>,
}

impl HpChangePlan {
    pub(crate) fn new(state: &GameState) -> GameResult<Self> {
        let mut current = HashMap::new();
        let mut initial = HashMap::new();
        let mut order = Vec::with_capacity(state.hp.len());

        for entry in &state.hp {
            if !current.insert(entry.team.clone(), entry.hp).is_none() {
                return Err(invariant("duplicate current team HP"));
            }
            order.push(entry.team.clone());
        }
        for entry in &state.initial_hp {
            if !initial.insert(entry.team.clone(), entry.hp).is_none() {
                return Err(invariant("duplicate initial team HP"));
            }
        }
        if current.len() != initial.len() || current.keys().any(|team| !initial.contains_key(team))
        {
            return Err(invariant("current and initial team HP ledgers differ"));
        }
        for team in &order {
            let hp = *current.get(team).expect("validated current ledger");
            let maximum = *initial.get(team).expect("validated initial ledger");
            if maximum < 0 || hp < 0 || hp > maximum {
                return Err(invariant("team HP is outside its initial HP range"));
            }
        }
        Ok(Self {
            current,
            initial,
            order,
        })
    }

    pub(crate) fn begin_formation_effect(&mut self) -> FormationHpEffectPlan<'_> {
        FormationHpEffectPlan {
            plan: self,
            actual_hp_loss_teams: HashSet::new(),
        }
    }

    pub(crate) fn current_hp(&self, team: &TeamId) -> GameResult<i32> {
        HpChangePlanner::current_hp(self, team)
    }

    pub(crate) fn plan(
        &mut self,
        team: &TeamId,
        request: HpChangeRequest,
    ) -> GameResult<HpChangeDelta> {
        HpChangePlanner::plan(self, team, request)
    }

    pub(crate) fn canonical_ordered_teams(&self, teams: &HashSet<TeamId>) -> Vec<TeamId> {
        self.order
            .iter()
            .filter(|team| teams.contains(*team))
            .cloned()
            .collect()
    }
}

impl HpChangePlanner for HpChangePlan {
    fn current_hp(&self, team: &TeamId) -> GameResult<i32> {
        self.current.get(team).copied().ok_or_else(|| {
            GameError::EngineInvariant(EngineInvariantError::InvalidHpLedger {
                reason: format!("missing team HP: {}", team.as_str()),
            })
        })
    }

    fn plan(&mut self, team: &TeamId, request: HpChangeRequest) -> GameResult<HpChangeDelta> {
        let old_hp = self.current_hp(team)?;
        let maximum = *self.initial.get(team).ok_or_else(|| {
            GameError::EngineInvariant(EngineInvariantError::InvalidHpLedger {
                reason: format!("missing initial team HP: {}", team.as_str()),
            })
        })?;
        let (delta, candidate) = match request {
            HpChangeRequest::By(delta) => (delta, old_hp.checked_add(delta)),
            HpChangeRequest::To(target) => (
                target
                    .checked_sub(old_hp)
                    .ok_or_else(|| overflow("HP target delta"))?,
                Some(target),
            ),
            HpChangeRequest::Prevented(attempted) => (attempted, Some(old_hp)),
        };
        let candidate = candidate.ok_or_else(|| overflow("HP change"))?;
        let new_hp = match request {
            HpChangeRequest::Prevented(_) => old_hp,
            _ => candidate.clamp(0, maximum),
        };
        let effective_delta = new_hp
            .checked_sub(old_hp)
            .ok_or_else(|| overflow("effective HP delta"))?;
        self.current.insert(team.clone(), new_hp);
        Ok(HpChangeDelta::new(
            team.clone(),
            old_hp,
            delta,
            new_hp,
            effective_delta,
        ))
    }
}

/// 一個已結算的陣法效果；其設計上不能規劃同命。
pub(crate) struct FormationHpEffectPlan<'a> {
    plan: &'a mut HpChangePlan,
    actual_hp_loss_teams: HashSet<TeamId>,
}

impl FormationHpEffectPlan<'_> {
    pub(crate) fn plan(
        &mut self,
        team: &TeamId,
        request: HpChangeRequest,
    ) -> GameResult<HpChangeDelta> {
        HpChangePlanner::plan(self, team, request)
    }

    pub(crate) fn finish(self) -> FormationHpEffectOutcome {
        FormationHpEffectOutcome {
            actual_hp_loss_teams: self
                .plan
                .canonical_ordered_teams(&self.actual_hp_loss_teams),
        }
    }
}

impl HpChangePlanner for FormationHpEffectPlan<'_> {
    fn current_hp(&self, team: &TeamId) -> GameResult<i32> {
        self.plan.current_hp(team)
    }

    fn plan(&mut self, team: &TeamId, request: HpChangeRequest) -> GameResult<HpChangeDelta> {
        let change = self.plan.plan(team, request)?;
        if change.effective_delta() < 0 {
            self.actual_hp_loss_teams.insert(team.clone());
        }
        Ok(change)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormationHpEffectOutcome {
    actual_hp_loss_teams: Vec<TeamId>,
}

impl FormationHpEffectOutcome {
    pub(crate) fn actual_hp_loss_teams(&self) -> &[TeamId] {
        &self.actual_hp_loss_teams
    }
}

#[cfg(test)]
pub(crate) fn test_delta(
    team: TeamId,
    old_hp: i32,
    delta: i32,
    new_hp: i32,
    effective_delta: i32,
) -> HpChangeDelta {
    HpChangeDelta::new(team, old_hp, delta, new_hp, effective_delta)
}

fn invariant(message: &str) -> GameError {
    GameError::EngineInvariant(EngineInvariantError::InvalidHpLedger {
        reason: message.to_string(),
    })
}

fn overflow(operation: &str) -> GameError {
    GameError::EngineInvariant(EngineInvariantError::InvalidHpLedger {
        reason: format!("integer overflow while planning {operation}"),
    })
}
