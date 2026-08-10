use crate::domain::{
    ActionModification, Element, GameEvent, GameResult, GameState, PassiveFlipOutcome,
    PassiveNoEffectReason, PlayerId,
    targeting::{RulePlayerTarget, TurnOrderTargets},
};
use crate::rules::environment_makes_formation_ineffective;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IncomingActionKind {
    Attack,
    ActiveSpell,
    PassiveSpell,
    ProfessionChange,
    Pass,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TriggerRequest {
    pub(super) incoming_player: PlayerId,
    pub(super) incoming_kind: IncomingActionKind,
    pub(super) ignores_formation_effects: bool,
    pub(super) ignores_counter_effects: bool,
    pub(super) attack_points: Option<i32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct PassiveTriggerResult {
    events: Vec<GameEvent>,
    modifications: Vec<ActionModification>,
}

impl PassiveTriggerResult {
    pub(super) fn events(self) -> Vec<GameEvent> {
        self.events
    }

    pub(super) fn prevents_damage(&self) -> bool {
        self.modifications
            .contains(&ActionModification::PreventDamage)
    }

    pub(super) fn splits_attack_damage(&self) -> bool {
        self.modifications
            .contains(&ActionModification::SplitAttackDamage)
    }

    pub(super) fn cancels_spell(&self) -> bool {
        self.modifications
            .contains(&ActionModification::CancelSpell)
    }

    pub(super) fn seals_covered_passive(&self) -> bool {
        self.modifications
            .contains(&ActionModification::SealCoveredPassive)
    }

    pub(super) fn reveals_covered_passive(&self) -> bool {
        self.modifications
            .contains(&ActionModification::RevealCoveredPassive)
    }
}

pub(super) fn trigger(state: &GameState, request: TriggerRequest) -> PassiveTriggerResult {
    let Some(previous_player) = previous_player(state, &request.incoming_player).ok() else {
        return PassiveTriggerResult {
            events: Vec::new(),
            modifications: Vec::new(),
        };
    };

    let mut events = Vec::new();
    let mut all_modifications = Vec::new();

    if let Some(passive) = state.covered_passive(&previous_player) {
        let (sealed, neutralized) = match &passive.state {
            crate::domain::FormationAreaState::FaceDownWaiting {
                sealed,
                neutralized,
                ..
            } => (*sealed, *neutralized),
            _ => {
                return PassiveTriggerResult {
                    events,
                    modifications: all_modifications,
                };
            }
        };
        let ineffective_environment =
            environment_makes_formation_ineffective(state, &passive.formation_id)
                .then_some(state.environment)
                .flatten();
        let modifications = if neutralized {
            Vec::new()
        } else {
            passive_spell_modifications(
                &passive.formation_id,
                request.incoming_kind,
                sealed,
                request.ignores_formation_effects,
                request.ignores_counter_effects,
                ineffective_environment,
                request.attack_points,
            )
        };
        all_modifications.extend(modifications.iter().cloned());
        events.push(GameEvent::PassiveFlipped {
            owner: previous_player.clone(),
            incoming_player: request.incoming_player.clone(),
            passive_id: passive.formation_id.clone(),
            cards: passive.cards.clone(),
            outcome: if neutralized {
                PassiveFlipOutcome::NoEffect {
                    reason: crate::domain::PassiveNoEffectReason::Neutralized,
                }
            } else {
                passive_outcome(
                    &passive.formation_id,
                    request.incoming_kind,
                    sealed,
                    request.ignores_formation_effects,
                    request.ignores_counter_effects,
                    ineffective_environment,
                    &modifications,
                )
            },
        });
    }

    for counter in state
        .counter_effects
        .iter()
        .filter(|counter| counter.owner == previous_player)
    {
        let modifications = passive_spell_modifications(
            &counter.effect_id,
            request.incoming_kind,
            false,
            request.ignores_formation_effects,
            request.ignores_counter_effects,
            None,
            request.attack_points,
        );
        all_modifications.extend(modifications.iter().cloned());
        events.push(GameEvent::CounterEffectResolved {
            owner: counter.owner.clone(),
            incoming_player: request.incoming_player.clone(),
            effect_id: counter.effect_id.clone(),
            outcome: passive_outcome(
                &counter.effect_id,
                request.incoming_kind,
                false,
                request.ignores_formation_effects,
                request.ignores_counter_effects,
                None,
                &modifications,
            ),
        });
    }

    PassiveTriggerResult {
        events,
        modifications: all_modifications,
    }
}

fn previous_player(state: &GameState, player: &PlayerId) -> GameResult<PlayerId> {
    TurnOrderTargets::new(state).player_target(player, RulePlayerTarget::PreviousPlayer)
}

fn passive_spell_modifications(
    passive_id: &str,
    incoming_kind: IncomingActionKind,
    sealed: bool,
    ignores_formation_effects: bool,
    ignores_counter_effects: bool,
    ineffective_environment: Option<Element>,
    attack_points: Option<i32>,
) -> Vec<ActionModification> {
    if sealed
        || ignores_formation_effects
        || ignores_counter_effects
        || ineffective_environment.is_some()
    {
        return Vec::new();
    }

    let modification = match (passive_id, incoming_kind) {
        ("defense" | "dao-defense", IncomingActionKind::Attack) => {
            ActionModification::PreventDamage
        }
        (crate::rules::jianghu::FLOWING_SHADOW_SWORD, IncomingActionKind::Attack)
            if attack_points.is_some_and(|points| points <= 40) =>
        {
            ActionModification::PreventDamage
        }
        ("countershock" | "magic-shock", IncomingActionKind::Attack) => {
            ActionModification::SplitAttackDamage
        }
        ("seal" | "magic-seal", IncomingActionKind::ActiveSpell) => ActionModification::CancelSpell,
        ("seal", IncomingActionKind::PassiveSpell) => ActionModification::SealCoveredPassive,
        ("holy-light-break", IncomingActionKind::PassiveSpell) => {
            ActionModification::RevealCoveredPassive
        }
        _ => return Vec::new(),
    };

    vec![modification]
}

fn passive_outcome(
    passive_id: &str,
    incoming_kind: IncomingActionKind,
    sealed: bool,
    ignores_formation_effects: bool,
    ignores_counter_effects: bool,
    ineffective_environment: Option<Element>,
    modifications: &[ActionModification],
) -> PassiveFlipOutcome {
    if ignores_formation_effects {
        return PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::IgnoredBySacredBeast,
        };
    }
    if ignores_counter_effects {
        return PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::IgnoredByProfessionAbility,
        };
    }

    if let Some(environment) = ineffective_environment {
        return PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::IneffectiveInEnvironment { environment },
        };
    }

    if sealed {
        return PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::Sealed,
        };
    }

    if !modifications.is_empty() {
        return PassiveFlipOutcome::Applied {
            effect_id: passive_id.to_string(),
            modifications: modifications.to_vec(),
        };
    }

    match (passive_id, incoming_kind) {
        ("empty-city", _) => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::EmptyCity,
        },
        (
            "defense",
            IncomingActionKind::ActiveSpell
            | IncomingActionKind::PassiveSpell
            | IncomingActionKind::ProfessionChange
            | IncomingActionKind::Pass,
        ) => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotAnAttack,
        },
        (
            "countershock",
            IncomingActionKind::ActiveSpell
            | IncomingActionKind::PassiveSpell
            | IncomingActionKind::ProfessionChange
            | IncomingActionKind::Pass,
        ) => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotAnAttack,
        },
        ("seal", IncomingActionKind::Attack) => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotASpell,
        },
        _ => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotASpell,
        },
    }
}
