use crate::domain::{
    ActionModification, GameEvent, GameResult, GameState, PassiveFlipOutcome,
    PassiveNoEffectReason, PlayerId,
    targeting::{RulePlayerTarget, TurnOrderTargets},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum IncomingActionKind {
    Attack,
    ActiveSpell,
    PassiveSpell,
    Pass,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct TriggerRequest {
    pub(super) incoming_player: PlayerId,
    pub(super) incoming_kind: IncomingActionKind,
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

    for passive in state
        .covered_passives
        .iter()
        .filter(|passive| passive.owner == previous_player)
    {
        let modifications = passive_spell_modifications(
            &passive.formation_id,
            request.incoming_kind,
            passive.sealed,
        );
        all_modifications.extend(modifications.iter().cloned());
        events.push(GameEvent::PassiveFlipped {
            owner: passive.owner.clone(),
            incoming_player: request.incoming_player.clone(),
            passive_id: passive.formation_id.clone(),
            cards: passive.cards.clone(),
            outcome: passive_outcome(
                &passive.formation_id,
                request.incoming_kind,
                passive.sealed,
                &modifications,
            ),
        });
    }

    for counter in state
        .counter_effects
        .iter()
        .filter(|counter| counter.owner == previous_player)
    {
        let modifications =
            passive_spell_modifications(&counter.effect_id, request.incoming_kind, false);
        all_modifications.extend(modifications.iter().cloned());
        events.push(GameEvent::CounterEffectResolved {
            owner: counter.owner.clone(),
            incoming_player: request.incoming_player.clone(),
            effect_id: counter.effect_id.clone(),
            outcome: passive_outcome(
                &counter.effect_id,
                request.incoming_kind,
                false,
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
) -> Vec<ActionModification> {
    if sealed {
        return Vec::new();
    }

    let modification = match (passive_id, incoming_kind) {
        ("defense", IncomingActionKind::Attack) => ActionModification::PreventDamage,
        ("countershock", IncomingActionKind::Attack) => ActionModification::SplitAttackDamage,
        ("seal", IncomingActionKind::ActiveSpell) => ActionModification::CancelSpell,
        ("seal", IncomingActionKind::PassiveSpell) => ActionModification::SealCoveredPassive,
        _ => return Vec::new(),
    };

    vec![modification]
}

fn passive_outcome(
    passive_id: &str,
    incoming_kind: IncomingActionKind,
    sealed: bool,
    modifications: &[ActionModification],
) -> PassiveFlipOutcome {
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
            | IncomingActionKind::Pass,
        ) => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotAnAttack,
        },
        (
            "countershock",
            IncomingActionKind::ActiveSpell
            | IncomingActionKind::PassiveSpell
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
