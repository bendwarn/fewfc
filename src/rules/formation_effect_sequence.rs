//! 陣法效果的逐 session 結算序列。
//!
//! 一個 outer resolution 可以有數個陣法效果。每個效果完成後必須立刻以同一份
//! HP ledger 規劃同命，否則下一個效果會讀到尚未扣除同命的舊生命。

use std::collections::{HashMap, HashSet};

use crate::domain::{
    GameError, GameEvent, GameResult, GameState, SpiritKind, TeamId,
    hp::{FormationHpEffectOutcome, HpChangePlan, HpChangeRequest},
    targeting::{RulePlayerTarget, TurnOrderTargets},
};

/// 已完成、但尚未附加同命的單一陣法效果。
pub(crate) struct ResolvedFormationEffect {
    pub(crate) events: Vec<GameEvent>,
    pub(crate) outcome: FormationHpEffectOutcome,
}

impl ResolvedFormationEffect {
    pub(crate) fn new(events: Vec<GameEvent>, outcome: FormationHpEffectOutcome) -> Self {
        Self { events, outcome }
    }
}

/// 將 primary event、其投影與同命的即時結算封裝在同一個深 module 內。
///
/// 呼叫端只需要先放入非陣法事件，再 append 一個已完成的 session；本 module
/// 保證下一個 session 看見前一個 primary effect 與同命都已投影的 canonical state。
pub(crate) struct FormationEffectSequence {
    projected: GameState,
    events: Vec<GameEvent>,
}

impl FormationEffectSequence {
    pub(crate) fn new(state: &GameState) -> Self {
        Self {
            projected: state.clone(),
            events: Vec::new(),
        }
    }

    pub(crate) fn state(&self) -> &GameState {
        &self.projected
    }

    pub(crate) fn extend(&mut self, events: impl IntoIterator<Item = GameEvent>) {
        for event in events {
            crate::rules::projection::apply_event(&mut self.projected, &event);
            self.events.push(event);
        }
    }

    pub(crate) fn append(
        &mut self,
        hp: &mut HpChangePlan,
        effect: ResolvedFormationEffect,
    ) -> GameResult<()> {
        let before = self.projected.clone();
        self.extend(effect.events);
        if effect.outcome.actual_hp_loss_teams().is_empty() {
            return Ok(());
        }

        let after = self.projected.clone();
        let affected = effect
            .outcome
            .actual_hp_loss_teams()
            .iter()
            .cloned()
            .collect::<HashSet<_>>();
        let mut counts = HashMap::<TeamId, i32>::new();
        for owned in before
            .spirits
            .iter()
            .filter(|owned| owned.spirit == SpiritKind::Death)
        {
            let owner_team = TurnOrderTargets::new(&before).team_of(&owned.player)?;
            if !affected.contains(&owner_team)
                || !after
                    .spirit_for(&owned.player)
                    .is_some_and(|after_owned| after_owned.spirit == SpiritKind::Death)
            {
                continue;
            }
            let target = TurnOrderTargets::new(&before)
                .player_target(&owned.player, RulePlayerTarget::NextPlayer)?;
            let team = TurnOrderTargets::new(&before).team_of(&target)?;
            let count = counts.entry(team).or_insert(0);
            *count = count.checked_add(1).ok_or_else(|| {
                GameError::EngineInvariant(crate::domain::EngineInvariantError::InvalidHpLedger {
                    reason: "shared fate owner count overflow".to_string(),
                })
            })?;
        }

        // 目標 Team 的 event 順序必須追隨 canonical HP 順序。
        for team in after.hp.iter().map(|entry| entry.team.clone()) {
            let Some(count) = counts.get(&team).copied() else {
                continue;
            };
            let delta = count.checked_mul(-10).ok_or_else(|| {
                GameError::EngineInvariant(crate::domain::EngineInvariantError::InvalidHpLedger {
                    reason: "shared fate HP delta overflow".to_string(),
                })
            })?;
            self.extend([GameEvent::HpChanged {
                change: hp.plan(&team, HpChangeRequest::By(delta))?,
            }]);
        }
        Ok(())
    }

    pub(crate) fn into_events(self) -> Vec<GameEvent> {
        self.events
    }
}
