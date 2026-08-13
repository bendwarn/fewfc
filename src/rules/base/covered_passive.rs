use crate::domain::{
    ActionModification, Element, GameEvent, GameResult, GameState, PassiveFlipOutcome,
    PassiveNoEffectGround, PlayerId,
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
    pub(super) ignores_counter_effects_by_profession_ability: bool,
    pub(super) ignores_counter_effects_by_golden_cicada: bool,
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
                request.ignores_counter_effects_by_profession_ability,
                request.ignores_counter_effects_by_golden_cicada,
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
            outcome: passive_outcome(
                &passive.formation_id,
                request.incoming_kind,
                sealed,
                neutralized,
                request.ignores_formation_effects,
                request.ignores_counter_effects_by_profession_ability,
                request.ignores_counter_effects_by_golden_cicada,
                ineffective_environment,
                request.attack_points,
                &modifications,
            ),
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
            request.ignores_counter_effects_by_profession_ability,
            request.ignores_counter_effects_by_golden_cicada,
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
                false,
                request.ignores_formation_effects,
                request.ignores_counter_effects_by_profession_ability,
                request.ignores_counter_effects_by_golden_cicada,
                None,
                request.attack_points,
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
    ignores_counter_effects_by_profession_ability: bool,
    ignores_counter_effects_by_golden_cicada: bool,
    ineffective_environment: Option<Element>,
    attack_points: Option<i32>,
) -> Vec<ActionModification> {
    if sealed
        || ignores_formation_effects
        || ignores_counter_effects_by_profession_ability
        || ignores_counter_effects_by_golden_cicada
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
    neutralized: bool,
    ignores_formation_effects: bool,
    ignores_counter_effects_by_profession_ability: bool,
    ignores_counter_effects_by_golden_cicada: bool,
    ineffective_environment: Option<Element>,
    attack_points: Option<i32>,
    modifications: &[ActionModification],
) -> PassiveFlipOutcome {
    // A counter effect must first be applicable. An unrelated Defense facing a
    // Spell therefore has only NotAnAttack, even if an immunity is present.
    if let Some(ground) = inapplicability_ground(passive_id, incoming_kind, attack_points) {
        return PassiveFlipOutcome::NoEffect {
            grounds: vec![ground],
        };
    }

    let mut grounds = Vec::new();
    if neutralized {
        grounds.push(PassiveNoEffectGround::Neutralized);
    }
    if ignores_formation_effects {
        grounds.push(PassiveNoEffectGround::IgnoredBySacredBeast);
    }
    if ignores_counter_effects_by_profession_ability {
        grounds.push(PassiveNoEffectGround::IgnoredByProfessionAbility);
    }
    if ignores_counter_effects_by_golden_cicada {
        grounds.push(PassiveNoEffectGround::IgnoredByGoldenCicada);
    }
    if let Some(environment) = ineffective_environment {
        grounds.push(PassiveNoEffectGround::IneffectiveInEnvironment { environment });
    }
    if sealed {
        grounds.push(PassiveNoEffectGround::Sealed);
    }
    if passive_id == "empty-city" {
        grounds.push(PassiveNoEffectGround::EmptyCity);
    }
    if !grounds.is_empty() {
        return PassiveFlipOutcome::NoEffect { grounds };
    }

    if !modifications.is_empty() || passive_id == crate::rules::jianghu::POISON_SMOKE {
        return PassiveFlipOutcome::Applied {
            effect_id: passive_id.to_string(),
            modifications: modifications.to_vec(),
        };
    }

    unreachable!("an applicable passive either has a ground or a modification")
}

fn inapplicability_ground(
    passive_id: &str,
    incoming_kind: IncomingActionKind,
    attack_points: Option<i32>,
) -> Option<PassiveNoEffectGround> {
    match passive_id {
        "empty-city" => None,
        "defense" | "dao-defense" | "countershock" | "magic-shock"
            if incoming_kind != IncomingActionKind::Attack =>
        {
            Some(PassiveNoEffectGround::NotAnAttack)
        }
        crate::rules::jianghu::FLOWING_SHADOW_SWORD
            if incoming_kind != IncomingActionKind::Attack =>
        {
            Some(PassiveNoEffectGround::NotAnAttack)
        }
        crate::rules::jianghu::FLOWING_SHADOW_SWORD
            if attack_points.is_none_or(|points| points > 40) =>
        {
            Some(PassiveNoEffectGround::AttackPointsExceedLimit { maximum: 40 })
        }
        "seal"
            if !matches!(
                incoming_kind,
                IncomingActionKind::ActiveSpell | IncomingActionKind::PassiveSpell
            ) =>
        {
            Some(PassiveNoEffectGround::NotASpell)
        }
        "magic-seal" if incoming_kind != IncomingActionKind::ActiveSpell => {
            Some(PassiveNoEffectGround::NotASpell)
        }
        "holy-light-break" if incoming_kind != IncomingActionKind::PassiveSpell => {
            Some(PassiveNoEffectGround::NotASpell)
        }
        _ => None,
    }
}
