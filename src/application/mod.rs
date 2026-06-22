//! Application services: command handling, automatic advancement, and replay.

use crate::domain::{
    ActionModification, AttackPointBreakdown, AutomaticReason, CardInstanceId, CardMoveDelta,
    CardZone, Command, CommandContext, CommandId, CommandKind, DamageTransform, DeckPlacement,
    ElementInteraction, EngineInvariantError, EventMetadata, EventSource, GameError, GameEvent,
    GameOutcome, GameResult, GameSetup, GameState, GameStatus, HpChangeDelta, LastElementalAttack,
    LastElementalAttackUpdate, LastFormationUse, PassActionReason, PassiveFlipOutcome,
    PassiveNoEffectReason, Phase, PlayerId, PublicGameEvent, PublicGameState, RecordedEvent,
    RulesetId, ShieldChangeDelta, TargetDecl, TeamId, TurnDrawSkipReason, ValidationError, Viewer,
    targeting::{RulePlayerTarget, RuleTeamTarget, TurnOrderTargets},
    validate_setup,
};
use crate::ports::DeckPreparation as DeckPreparationPort;
use crate::rules::{
    AttackCategory, AttackPlanDef, DamageTarget, EffectPlan, FormationCategory, PointFormula,
    base_formation_matcher, base_formation_registry,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventBatch {
    events: Vec<GameEvent>,
}

impl EventBatch {
    pub fn new(events: Vec<GameEvent>) -> Self {
        Self { events }
    }

    pub fn events(&self) -> &[GameEvent] {
        &self.events
    }

    pub fn into_events(self) -> Vec<GameEvent> {
        self.events
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartGame {
    pub setup: GameSetup,
    pub deck_order: Vec<CardInstanceId>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StartGameWithDeckPreparationError<E> {
    DeckPreparation(E),
    Game(GameError),
}

impl<E> From<GameError> for StartGameWithDeckPreparationError<E> {
    fn from(error: GameError) -> Self {
        Self::Game(error)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BaseRuleset;

impl BaseRuleset {
    pub fn new() -> Self {
        Self
    }

    pub fn id(&self) -> RulesetId {
        RulesetId::base()
    }

    pub fn start_game(
        &self,
        setup: &GameSetup,
        deck_order: Vec<CardInstanceId>,
    ) -> GameResult<Vec<GameEvent>> {
        validate_setup(setup)?;
        validate_card_instances(setup, &deck_order)?;
        initial_events(setup, deck_order)
    }

    pub fn decide_command(
        &self,
        state: &GameState,
        command: Command,
    ) -> GameResult<Vec<GameEvent>> {
        decide_command_with_base_ruleset(state, command)
    }

    pub fn advance_automatic(&self, state: &GameState) -> GameResult<Vec<GameEvent>> {
        advance_automatic(state)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FormationUseRequest {
    player: PlayerId,
    formation_id: String,
    cards: Vec<CardInstanceId>,
    declared_targets: Vec<TargetDecl>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct FormationUsePlan {
    player: PlayerId,
    formation_id: String,
    cards: Vec<CardInstanceId>,
    declared_targets: Vec<TargetDecl>,
    category: FormationCategory,
    effect_plan: EffectPlan,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BaseFormationPlanner;

impl BaseFormationPlanner {
    fn new() -> Self {
        Self
    }

    fn plan_use(
        &self,
        state: &GameState,
        request: FormationUseRequest,
    ) -> GameResult<FormationUsePlan> {
        let registry = base_formation_registry();
        let formation = registry.formation(&request.formation_id).ok_or_else(|| {
            GameError::Validation(ValidationError::UnknownFormation(
                request.formation_id.clone(),
            ))
        })?;

        let hand = state.hand(&request.player).ok_or_else(|| {
            GameError::Validation(ValidationError::UnknownPlayer(request.player.clone()))
        })?;
        let mut seen = HashSet::new();
        let mut submitted_elements = Vec::new();

        for card in &request.cards {
            if !seen.insert(*card) {
                return Err(GameError::Validation(
                    ValidationError::DuplicateSubmittedCard(*card),
                ));
            }

            if !hand.contains(card) {
                return Err(GameError::Validation(ValidationError::CardNotInHand(*card)));
            }

            submitted_elements.push(state.card_element(*card).ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))?);
        }

        if !base_formation_matcher().matches(&formation.pattern, &submitted_elements) {
            return Err(GameError::Validation(
                ValidationError::FormationPatternMismatch {
                    formation_id: request.formation_id,
                },
            ));
        }
        if request.formation_id == "five-streams-unite"
            && !submitted_cards_have_same_level(state, &request.cards)?
        {
            return Err(GameError::Validation(
                ValidationError::FormationPatternMismatch {
                    formation_id: request.formation_id,
                },
            ));
        }

        let effect = registry
            .effect_for(formation)
            .expect("base formation registry must link every formation to an effect");

        Ok(FormationUsePlan {
            player: request.player,
            formation_id: request.formation_id,
            cards: request.cards,
            declared_targets: request.declared_targets,
            category: formation.category.clone(),
            effect_plan: effect.plan.clone(),
        })
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct BaseEffectResolver;

impl BaseEffectResolver {
    fn new() -> Self {
        Self
    }

    fn resolve(&self, state: &GameState, plan: FormationUsePlan) -> GameResult<Vec<GameEvent>> {
        match &plan.effect_plan {
            EffectPlan::Attack(attack_plan) => {
                if !plan.declared_targets.is_empty() {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }
                let passive_resolutions =
                    passive_resolutions(state, &plan.player, IncomingActionKind::Attack);
                let action_modifications = action_modifications(&passive_resolutions);
                let mut events = passive_events(passive_resolutions);
                let target = attack_target(state, &plan.player, attack_plan)?;
                let target_team = player_team(state, &target)?;
                let points =
                    compute_attack_points(state, &attack_plan.point_formula, &plan.cards, &target)?;
                let has_target_shield = state.shield(&target).is_some_and(|value| value > 0);
                let point_breakdown = attack_point_breakdown(
                    state,
                    &plan.category,
                    &target,
                    points,
                    has_target_shield,
                );
                let final_amount = point_breakdown.final_amount;
                let damage_prevented =
                    action_modifications.contains(&ActionModification::PreventDamage);
                let shield_change = if damage_prevented {
                    None
                } else {
                    shield_absorption(state, &target, final_amount)
                };
                let has_shield_change = shield_change.is_some();
                let split_attack_damage = action_modifications
                    .contains(&ActionModification::SplitAttackDamage)
                    && !matches!(
                        point_breakdown.damage_transform,
                        DamageTransform::HealTarget
                    );
                let hp_change = if damage_prevented || has_shield_change {
                    no_hp_change(state, &target_team)?
                } else if split_attack_damage {
                    apply_attack_amount(
                        state,
                        &target_team,
                        (final_amount + 1) / 2,
                        DamageTransform::NormalDamage,
                    )?
                } else {
                    apply_attack_amount(
                        state,
                        &target_team,
                        final_amount,
                        point_breakdown.damage_transform,
                    )?
                };
                let card_moves = plan
                    .cards
                    .iter()
                    .copied()
                    .map(|card| CardMoveDelta {
                        card,
                        from: CardZone::Hand(plan.player.clone()),
                        to: CardZone::Discard,
                    })
                    .collect::<Vec<_>>();
                let elemental_context_update =
                    elemental_context_update(&plan.category, state.turn_number).map(|attack| {
                        LastElementalAttackUpdate {
                            player: plan.player.clone(),
                            attack,
                        }
                    });

                events.push(GameEvent::AttackResolved {
                    attacker: plan.player.clone(),
                    target,
                    formation_id: plan.formation_id.clone(),
                    used_cards: plan.cards.clone(),
                    point_breakdown,
                    hp_change,
                    shield_change,
                    card_moves,
                    elemental_context_update,
                });

                if split_attack_damage && !damage_prevented && !has_shield_change {
                    let attacker_team = player_team(state, &plan.player)?;
                    let attacker_damage = final_amount / 2;
                    if attacker_damage > 0 {
                        events.push(GameEvent::HpChanged {
                            change: apply_attack_amount(
                                state,
                                &attacker_team,
                                attacker_damage,
                                DamageTransform::NormalDamage,
                            )?,
                        });
                    }
                }

                if plan.formation_id == "five-streams-unite" {
                    let old_value = state
                        .turn_draw_bonus_by_player
                        .get(&plan.player)
                        .copied()
                        .unwrap_or(0);
                    events.push(GameEvent::TurnDrawBonusChanged {
                        player: plan.player,
                        old_value,
                        delta: 1,
                        new_value: old_value + 1,
                    });
                }

                Ok(events)
            }
            EffectPlan::PassiveSpell(_) => {
                if !plan.declared_targets.is_empty() {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }

                if state
                    .covered_passives
                    .iter()
                    .any(|passive| passive.owner == plan.player)
                {
                    return Err(GameError::Validation(
                        ValidationError::PendingPassiveAlreadyCovered {
                            player: plan.player,
                        },
                    ));
                }

                let passive_resolutions =
                    passive_resolutions(state, &plan.player, IncomingActionKind::PassiveSpell);
                let action_modifications = action_modifications(&passive_resolutions);
                let mut events = passive_events(passive_resolutions);
                events.push(GameEvent::PassiveCovered {
                    player: plan.player,
                    formation_id: plan.formation_id,
                    cards: plan.cards,
                    sealed: action_modifications.contains(&ActionModification::SealCoveredPassive),
                });
                Ok(events)
            }
            EffectPlan::ActiveSpell(spell) => {
                if !plan.declared_targets.is_empty() {
                    return Err(GameError::Validation(
                        ValidationError::UnexpectedDeclaredTargets {
                            formation_id: plan.formation_id,
                        },
                    ));
                }

                let passive_resolutions =
                    passive_resolutions(state, &plan.player, IncomingActionKind::ActiveSpell);
                let action_modifications = action_modifications(&passive_resolutions);
                let mut events = passive_events(passive_resolutions);
                events.push(GameEvent::FormationPerformed {
                    player: plan.player.clone(),
                    formation_id: plan.formation_id,
                    used_cards: plan.cards.clone(),
                    declared_targets: plan.declared_targets,
                });
                if !action_modifications.contains(&ActionModification::CancelSpell) {
                    let intents =
                        active_spell_intents(state, &plan.player, &spell.resolver_id, &plan.cards)?;
                    events.extend(effect_intent_events(state, intents)?);
                }
                Ok(events)
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    setup: GameSetup,
    events: Vec<GameEvent>,
    event_metadata: Vec<EventMetadata>,
    latest_snapshot: Option<GameState>,
}

impl GameRecord {
    pub fn start(setup: GameSetup, deck_order: Vec<CardInstanceId>) -> GameResult<Self> {
        let ruleset = BaseRuleset::new();
        let events = ruleset.start_game(&setup, deck_order)?;
        let event_metadata = metadata_for_events(events.len(), EventSource::Setup);

        let record = Self {
            setup,
            events,
            event_metadata,
            latest_snapshot: None,
        };

        record.replay()?;
        Ok(record)
    }

    pub fn start_game(input: StartGame) -> GameResult<Self> {
        Self::start(input.setup, input.deck_order)
    }

    pub fn start_with_deck_preparation<P>(
        setup: GameSetup,
        deck_preparation: &mut P,
    ) -> Result<Self, StartGameWithDeckPreparationError<P::Error>>
    where
        P: DeckPreparationPort,
    {
        let deck_order = deck_preparation
            .prepare_deck(&setup)
            .map_err(StartGameWithDeckPreparationError::DeckPreparation)?;
        Self::start(setup, deck_order).map_err(StartGameWithDeckPreparationError::Game)
    }

    pub fn events(&self) -> &[GameEvent] {
        &self.events
    }

    pub fn setup(&self) -> &GameSetup {
        &self.setup
    }

    pub fn recorded_events(&self) -> Vec<RecordedEvent> {
        self.events
            .iter()
            .zip(self.event_metadata.iter())
            .map(|(event, metadata)| RecordedEvent {
                metadata: metadata.clone(),
                event: event.clone(),
            })
            .collect()
    }

    pub fn public_events_for(&self, viewer: Viewer) -> Vec<PublicGameEvent> {
        crate::public_view::events_for(&self.events, viewer)
    }

    pub fn public_view(&self, viewer: Viewer) -> GameResult<PublicGameState> {
        Ok(crate::public_view::state_for(&self.state()?, viewer))
    }

    pub fn state(&self) -> GameResult<GameState> {
        self.replay()
    }

    pub fn replay(&self) -> GameResult<GameState> {
        let mut state = GameState::from_setup(&self.setup);
        for event in &self.events {
            apply_event(&mut state, event);
        }
        Ok(state)
    }

    pub fn latest_snapshot(&self) -> Option<&GameState> {
        self.latest_snapshot.as_ref()
    }

    pub fn apply(&mut self, command: Command) -> GameResult<EventBatch> {
        let state = self.state()?;
        let source = EventSource::Command {
            command_id: self.next_command_id(),
            context: command_context(&command),
        };
        let events = BaseRuleset::new().decide_command(&state, command)?;
        self.extend_events(events.clone(), |event| {
            source_for_command_event(event, &source)
        });
        Ok(EventBatch::new(events))
    }

    pub fn handle(&mut self, command: Command) -> GameResult<Vec<GameEvent>> {
        self.apply(command).map(EventBatch::into_events)
    }

    pub fn advance_until_decision(&mut self) -> GameResult<EventBatch> {
        let state = self.state()?;
        let events = BaseRuleset::new().advance_automatic(&state)?;
        self.extend_events(events.clone(), source_for_automatic_event);
        Ok(EventBatch::new(events))
    }

    pub fn advance_automatic(&mut self) -> GameResult<Vec<GameEvent>> {
        self.advance_until_decision().map(EventBatch::into_events)
    }

    pub fn verify_replay(&self) -> Result<GameState, ReplayVerificationError> {
        verify_recorded_events(&self.setup, &self.recorded_events())
    }

    fn extend_events(
        &mut self,
        events: Vec<GameEvent>,
        source_for_event: impl Fn(&GameEvent) -> EventSource,
    ) {
        let first_sequence = self.events.len() as u64 + 1;
        self.event_metadata.extend(
            events
                .iter()
                .enumerate()
                .map(|(index, event)| EventMetadata {
                    sequence: first_sequence + index as u64,
                    source: source_for_event(event),
                }),
        );
        self.events.extend(events.clone());
    }

    fn next_command_id(&self) -> CommandId {
        let next_id = self
            .event_metadata
            .iter()
            .filter_map(|metadata| match &metadata.source {
                EventSource::Command { command_id, .. } => Some(command_id.as_u64()),
                _ => None,
            })
            .max()
            .unwrap_or(0)
            + 1;
        CommandId::new(next_id)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ReplayVerificationError {
    DecisionFailed {
        sequence: u64,
        source: EventSource,
        error: Box<GameError>,
    },
    EventMismatch {
        sequence: u64,
        expected: Vec<GameEvent>,
        actual: Vec<GameEvent>,
    },
    MissingSetupDeck {
        sequence: u64,
    },
    CommandContextUnavailable {
        sequence: u64,
        source: EventSource,
    },
}

fn metadata_for_events(count: usize, source: EventSource) -> Vec<EventMetadata> {
    (0..count)
        .map(|index| EventMetadata {
            sequence: index as u64 + 1,
            source: source.clone(),
        })
        .collect()
}

fn source_for_command_event(_event: &GameEvent, source: &EventSource) -> EventSource {
    source.clone()
}

fn source_for_automatic_event(event: &GameEvent) -> EventSource {
    EventSource::Automatic {
        reason: automatic_reason(event)
            .expect("automatic advancement must only emit automatic events"),
    }
}

fn automatic_reason(event: &GameEvent) -> Option<AutomaticReason> {
    match event {
        GameEvent::TurnStarted { .. } => Some(AutomaticReason::TurnStart),
        GameEvent::CardsDrawnForTurnDiscardChoice { .. } => Some(AutomaticReason::TurnDraw),
        GameEvent::TurnDrawSkipped { .. } => Some(AutomaticReason::TurnDrawSkipped),
        GameEvent::DiscardRecycledIntoDeck { .. } => Some(AutomaticReason::DiscardRecycle),
        GameEvent::StatusExpired { .. } => Some(AutomaticReason::StatusExpired),
        GameEvent::TurnEnded { .. } => Some(AutomaticReason::TurnEnd),
        GameEvent::DeckPrepared { .. }
        | GameEvent::CardsDealt { .. }
        | GameEvent::ActionPassed { .. }
        | GameEvent::AttackResolved { .. }
        | GameEvent::CardsMoved { .. }
        | GameEvent::EffectChoiceAnswered { .. }
        | GameEvent::EffectChoiceRequested { .. }
        | GameEvent::FormationPerformed { .. }
        | GameEvent::HpChanged { .. }
        | GameEvent::PassiveCovered { .. }
        | GameEvent::PassiveFlipped { .. }
        | GameEvent::ShieldChanged { .. }
        | GameEvent::StatusAdded { .. }
        | GameEvent::StatusRemoved { .. }
        | GameEvent::TurnDrawBonusChanged { .. }
        | GameEvent::TurnDiscardChosen { .. } => None,
    }
}

fn command_context(command: &Command) -> CommandContext {
    match command {
        Command::PassAction { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::PassAction,
        },
        Command::PerformFormation {
            player,
            formation_id,
            ..
        } => CommandContext {
            player: player.clone(),
            kind: CommandKind::PerformFormation {
                formation_id: formation_id.clone(),
            },
        },
        Command::ChooseTurnDiscard { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::ChooseTurnDiscard,
        },
        Command::AnswerEffectChoice { player, .. } => CommandContext {
            player: player.clone(),
            kind: CommandKind::AnswerEffectChoice,
        },
    }
}

fn validate_card_instances(setup: &GameSetup, deck_order: &[CardInstanceId]) -> GameResult<()> {
    let mut seen = HashSet::new();
    let known_instances = setup
        .card_instances
        .iter()
        .map(|card| card.instance)
        .collect::<HashSet<_>>();

    for card in deck_order {
        if !seen.insert(*card) {
            return Err(GameError::Validation(ValidationError::DuplicateCard(*card)));
        }

        if !known_instances.contains(card) {
            return Err(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ));
        }
    }

    Ok(())
}

fn initial_events(
    setup: &GameSetup,
    deck_order: Vec<CardInstanceId>,
) -> GameResult<Vec<GameEvent>> {
    let needed = initial_deal_count(setup);
    if deck_order.len() < needed {
        return Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed,
                available: deck_order.len(),
            },
        ));
    }

    let mut events = vec![GameEvent::DeckPrepared {
        deck_order: deck_order.clone(),
    }];
    let mut next_card_index = 0;

    for (turn_index, player) in setup.turn_order.iter().enumerate() {
        let card_count = if turn_index == 0 { 4 } else { 5 };
        let cards = deck_order[next_card_index..next_card_index + card_count].to_vec();
        next_card_index += card_count;
        events.push(GameEvent::CardsDealt {
            player: player.clone(),
            cards,
        });
    }

    Ok(events)
}

fn initial_deal_count(setup: &GameSetup) -> usize {
    setup
        .turn_order
        .iter()
        .enumerate()
        .map(|(index, _)| if index == 0 { 4 } else { 5 })
        .sum()
}

pub fn advance_automatic(state: &GameState) -> GameResult<Vec<GameEvent>> {
    ensure_engine_invariants(state)?;

    if matches!(state.status, GameStatus::Finished { .. }) {
        return Ok(Vec::new());
    }
    if state.pending_choice.is_some() {
        return Ok(Vec::new());
    }

    let mut projected = state.clone();
    let mut events = Vec::new();

    loop {
        let next_event = match projected.phase {
            Phase::TurnStart => status_expiry_event(
                &projected,
                crate::domain::StatusExpiryTiming::TurnStart {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                        .clone(),
                },
            )
            .or_else(|| {
                Some(GameEvent::TurnStarted {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))
                        .ok()?
                        .clone(),
                    turn_number: projected.turn_number,
                })
            }),
            Phase::TurnDraw => next_turn_draw_event(&projected)?,
            Phase::TurnEnd => status_expiry_event(
                &projected,
                crate::domain::StatusExpiryTiming::TurnEnd {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                        .clone(),
                },
            )
            .or_else(|| {
                Some(GameEvent::TurnEnded {
                    player: projected
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))
                        .ok()?
                        .clone(),
                })
            }),
            Phase::Main | Phase::TurnDrawDiscardChoice => None,
        };

        let Some(event) = next_event else {
            break;
        };

        apply_event(&mut projected, &event);
        events.push(event);
    }

    Ok(events)
}

fn status_expiry_event(
    state: &GameState,
    timing: crate::domain::StatusExpiryTiming,
) -> Option<GameEvent> {
    let status = state
        .statuses
        .iter()
        .find(|status| status_expires_at(&status.duration, &timing, state.turn_number))?;

    Some(GameEvent::StatusExpired {
        status_id: status.id.clone(),
        owner: status.owner.clone(),
        expired_at: timing,
    })
}

fn status_expires_at(
    duration: &crate::domain::StatusDuration,
    timing: &crate::domain::StatusExpiryTiming,
    current_turn_number: u64,
) -> bool {
    match (duration, timing) {
        (
            crate::domain::StatusDuration::UntilTurnStart {
                player: duration_player,
            },
            crate::domain::StatusExpiryTiming::TurnStart {
                player: timing_player,
            },
        )
        | (
            crate::domain::StatusDuration::UntilTurnEnd {
                player: duration_player,
            },
            crate::domain::StatusExpiryTiming::TurnEnd {
                player: timing_player,
            },
        ) => duration_player == timing_player,
        (
            crate::domain::StatusDuration::UntilTurnEndNumber {
                player: duration_player,
                turn_number,
            },
            crate::domain::StatusExpiryTiming::TurnEnd {
                player: timing_player,
            },
        ) => duration_player == timing_player && *turn_number == current_turn_number,
        (crate::domain::StatusDuration::Permanent, _) => false,
        _ => false,
    }
}

fn next_turn_draw_event(state: &GameState) -> GameResult<Option<GameEvent>> {
    let player = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
        .clone();
    let hand = state
        .hand(&player)
        .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
    if player_has_status(state, &player, "CannotDraw") {
        return Ok(Some(GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::CannotDrawByStatus,
        }));
    }
    let available_space = state.hand_limit.saturating_sub(hand.len());

    if available_space == 0 {
        return Ok(Some(GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::HandLimitReached,
        }));
    }

    let draw_bonus = state
        .turn_draw_bonus_by_player
        .get(&player)
        .copied()
        .unwrap_or(0);
    let draw_count = (state.base_draw + draw_bonus).min(available_space) + 1;
    if state.deck.len() < draw_count {
        if state.deck.len() + state.discard.len() >= draw_count && !state.discard.is_empty() {
            return Ok(Some(GameEvent::DiscardRecycledIntoDeck {
                shuffled_order: state.discard.clone(),
                placement: DeckPlacement::Bottom,
            }));
        }

        return Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed: draw_count,
                available: state.deck.len(),
            },
        ));
    }

    let drawn_cards = state
        .deck
        .iter()
        .take(draw_count)
        .copied()
        .collect::<Vec<_>>();

    Ok(Some(GameEvent::CardsDrawnForTurnDiscardChoice {
        player,
        allowed_discards: drawn_cards.clone(),
        drawn_cards,
    }))
}

pub fn handle_command(state: &GameState, command: Command) -> GameResult<Vec<GameEvent>> {
    BaseRuleset::new().decide_command(state, command)
}

fn decide_command_with_base_ruleset(
    state: &GameState,
    command: Command,
) -> GameResult<Vec<GameEvent>> {
    ensure_engine_invariants(state)?;

    if matches!(state.status, GameStatus::Finished { .. }) {
        return Err(GameError::Validation(ValidationError::GameFinished));
    }
    if let Some(choice) = &state.pending_choice {
        let is_choice_answer = matches!(
            (&choice.kind, &command),
            (
                crate::domain::PendingChoiceKind::TurnDrawDiscard { .. },
                Command::ChooseTurnDiscard { .. }
            ) | (
                crate::domain::PendingChoiceKind::EffectGenerated { .. },
                Command::AnswerEffectChoice { .. }
            )
        );

        if !is_choice_answer {
            return Err(GameError::Validation(
                ValidationError::PendingChoiceInProgress {
                    player: choice.player.clone(),
                },
            ));
        }
    }

    match command {
        Command::PassAction { player, reason } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;

            match reason {
                PassActionReason::NoCardsInHand => {
                    let hand = state.hand(&player).ok_or_else(|| {
                        GameError::Validation(ValidationError::UnknownPlayer(player.clone()))
                    })?;
                    if !hand.is_empty() {
                        return Err(GameError::Validation(ValidationError::CannotPassAction {
                            reason,
                        }));
                    }
                }
                PassActionReason::CannotActByStatus => {
                    if !player_has_status(state, &player, "CannotAct") {
                        return Err(GameError::Validation(ValidationError::CannotPassAction {
                            reason,
                        }));
                    }
                }
            }

            Ok(vec![GameEvent::ActionPassed { player, reason }])
        }
        Command::PerformFormation {
            player,
            formation_id,
            cards,
            declared_targets,
        } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;

            let plan = BaseFormationPlanner::new().plan_use(
                state,
                FormationUseRequest {
                    player,
                    formation_id,
                    cards,
                    declared_targets,
                },
            )?;
            BaseEffectResolver::new().resolve(state, plan)
        }
        Command::ChooseTurnDiscard { player, discard } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::TurnDrawDiscardChoice)?;

            let allowed_discards = match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::TurnDrawDiscard {
                            allowed_discards, ..
                        },
                }) if choice_player == &player => allowed_discards,
                _ => return Err(GameError::Validation(ValidationError::MissingPendingChoice)),
            };

            if !allowed_discards.contains(&discard) {
                return Err(GameError::Validation(ValidationError::IllegalDiscard(
                    discard,
                )));
            }

            Ok(vec![GameEvent::TurnDiscardChosen { player, discard }])
        }
        Command::AnswerEffectChoice {
            player,
            selected_cards,
        } => {
            let (effect_id, continuation_id, allowed_cards) = match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::EffectGenerated {
                            effect_id,
                            continuation_id,
                            allowed_cards,
                        },
                }) if choice_player == &player => {
                    (effect_id.clone(), continuation_id.clone(), allowed_cards)
                }
                _ => return Err(GameError::Validation(ValidationError::MissingPendingChoice)),
            };

            let mut seen = HashSet::new();
            for selected_card in &selected_cards {
                if !seen.insert(*selected_card) {
                    return Err(GameError::Validation(ValidationError::DuplicateChoiceCard(
                        *selected_card,
                    )));
                }

                if !allowed_cards.contains(selected_card) {
                    return Err(GameError::Validation(ValidationError::IllegalChoiceCard(
                        *selected_card,
                    )));
                }
            }

            let mut events = vec![GameEvent::EffectChoiceAnswered {
                player: player.clone(),
                effect_id: effect_id.clone(),
                continuation_id: continuation_id.clone(),
                selected_cards: selected_cards.clone(),
            }];
            let intents = resume_effect_choice_intents(
                state,
                &player,
                &effect_id,
                &continuation_id,
                &selected_cards,
            )?;
            events.extend(effect_intent_events(state, intents)?);
            Ok(events)
        }
    }
}

fn ensure_engine_invariants(state: &GameState) -> GameResult<()> {
    let mut covered_passive_owners = HashSet::new();
    for passive in &state.covered_passives {
        if !covered_passive_owners.insert(passive.owner.clone()) {
            return Err(GameError::EngineInvariant(
                EngineInvariantError::DuplicateCoveredPassive {
                    player: passive.owner.clone(),
                },
            ));
        }
    }

    Ok(())
}

fn ensure_current_player(state: &GameState, actual: &crate::domain::PlayerId) -> GameResult<()> {
    let expected = state
        .current_player()
        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?;

    if expected == actual {
        Ok(())
    } else {
        Err(GameError::Validation(ValidationError::WrongPlayer {
            expected: expected.clone(),
            actual: actual.clone(),
        }))
    }
}

fn ensure_phase(state: &GameState, expected: Phase) -> GameResult<()> {
    if state.phase == expected {
        Ok(())
    } else {
        Err(GameError::Validation(ValidationError::WrongPhase {
            expected,
            actual: state.phase,
        }))
    }
}

fn player_has_status(state: &GameState, player: &crate::domain::PlayerId, kind: &str) -> bool {
    state.statuses.iter().any(|status| {
        matches!(&status.owner, crate::domain::StatusOwner::Player(owner) if owner == player)
            && status.kind == kind
    })
}

fn submitted_cards_have_same_level(
    state: &GameState,
    cards: &[CardInstanceId],
) -> GameResult<bool> {
    let Some(first_card) = cards.first() else {
        return Ok(false);
    };
    let first_level = state
        .card_def(*first_card)
        .ok_or(GameError::Validation(
            ValidationError::MissingCardInstanceDefinition(*first_card),
        ))?
        .level;

    cards.iter().try_fold(true, |same_level, card| {
        let level = state
            .card_def(*card)
            .ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))?
            .level;
        Ok(same_level && level == first_level)
    })
}

#[derive(Clone, Copy)]
enum IncomingActionKind {
    Attack,
    ActiveSpell,
    PassiveSpell,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PassiveResolution {
    event: GameEvent,
    modifications: Vec<ActionModification>,
}

fn passive_resolutions(
    state: &GameState,
    incoming_player: &PlayerId,
    incoming_kind: IncomingActionKind,
) -> Vec<PassiveResolution> {
    let Some(previous_player) = previous_player(state, incoming_player).ok() else {
        return Vec::new();
    };

    state
        .covered_passives
        .iter()
        .filter(|passive| passive.owner == previous_player)
        .map(|passive| {
            let modifications =
                passive_spell_intents(&passive.formation_id, incoming_kind, passive.sealed)
                    .into_iter()
                    .filter_map(|intent| match intent {
                        EffectIntent::ModifyAction { modification } => Some(modification),
                        _ => None,
                    })
                    .collect::<Vec<_>>();

            PassiveResolution {
                event: GameEvent::PassiveFlipped {
                    owner: passive.owner.clone(),
                    incoming_player: incoming_player.clone(),
                    passive_id: passive.formation_id.clone(),
                    cards: passive.cards.clone(),
                    outcome: passive_outcome(
                        &passive.formation_id,
                        incoming_kind,
                        passive.sealed,
                        &modifications,
                    ),
                },
                modifications,
            }
        })
        .collect()
}

fn passive_events(resolutions: Vec<PassiveResolution>) -> Vec<GameEvent> {
    resolutions
        .into_iter()
        .map(|resolution| resolution.event)
        .collect()
}

fn action_modifications(resolutions: &[PassiveResolution]) -> Vec<ActionModification> {
    resolutions
        .iter()
        .flat_map(|resolution| resolution.modifications.iter().cloned())
        .collect()
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
        ("defense", IncomingActionKind::ActiveSpell | IncomingActionKind::PassiveSpell) => {
            PassiveFlipOutcome::NoEffect {
                reason: PassiveNoEffectReason::NotAnAttack,
            }
        }
        ("countershock", IncomingActionKind::ActiveSpell | IncomingActionKind::PassiveSpell) => {
            PassiveFlipOutcome::NoEffect {
                reason: PassiveNoEffectReason::NotAnAttack,
            }
        }
        ("seal", IncomingActionKind::Attack) => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotASpell,
        },
        _ => PassiveFlipOutcome::NoEffect {
            reason: PassiveNoEffectReason::NotASpell,
        },
    }
}

fn passive_spell_intents(
    passive_id: &str,
    incoming_kind: IncomingActionKind,
    sealed: bool,
) -> Vec<EffectIntent> {
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

    vec![EffectIntent::ModifyAction { modification }]
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum EffectIntent {
    SetShield {
        player: PlayerId,
        value: i32,
    },
    ChangeHp {
        team: TeamId,
        delta: i32,
    },
    MoveCards {
        card_moves: Vec<CardMoveDelta>,
    },
    AddStatus {
        status: crate::domain::StatusEffect,
    },
    ResolveCopiedAttack {
        formation_id: String,
        category: FormationCategory,
        point_formula: PointFormula,
        used_cards: Vec<CardInstanceId>,
    },
    ModifyAction {
        modification: ActionModification,
    },
    RequestChoice {
        player: PlayerId,
        kind: crate::domain::PendingChoiceKind,
    },
}

fn active_spell_intents(
    state: &GameState,
    player: &PlayerId,
    resolver_id: &str,
    used_cards: &[CardInstanceId],
) -> GameResult<Vec<EffectIntent>> {
    match resolver_id {
        "barrier" => Ok(vec![EffectIntent::SetShield {
            player: player.clone(),
            value: level_sum(state, used_cards)? * 4,
        }]),
        "metamorphosis" => metamorphosis_intents(state, player, used_cards),
        "generating-formation" => {
            let team = resolve_rule_team_target(state, player, RuleTeamTarget::OwnSide)?;
            Ok(vec![EffectIntent::ChangeHp {
                team,
                delta: level_sum(state, used_cards)? * 3,
            }])
        }
        "overcoming-formation" => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let old_value = state.shield(&target).unwrap_or(0);
            let new_value = (old_value - level_sum(state, used_cards)? * 3).max(0);
            Ok(vec![EffectIntent::SetShield {
                player: target,
                value: new_value,
            }])
        }
        "radiance" => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let expires_at = nth_future_turn_for_player(state, &target, 2)?;
            Ok(vec![
                EffectIntent::AddStatus {
                    status: crate::domain::StatusEffect {
                        id: format!(
                            "radiance-cannot-act-{}-turn-{}",
                            target.as_str(),
                            state.turn_number
                        ),
                        owner: crate::domain::StatusOwner::Player(target.clone()),
                        kind: "CannotAct".to_string(),
                        value: None,
                        duration: crate::domain::StatusDuration::UntilTurnEndNumber {
                            player: target.clone(),
                            turn_number: expires_at,
                        },
                    },
                },
                EffectIntent::AddStatus {
                    status: crate::domain::StatusEffect {
                        id: format!(
                            "radiance-cannot-draw-{}-turn-{}",
                            target.as_str(),
                            state.turn_number
                        ),
                        owner: crate::domain::StatusOwner::Player(target.clone()),
                        kind: "CannotDraw".to_string(),
                        value: None,
                        duration: crate::domain::StatusDuration::UntilTurnEndNumber {
                            player: target,
                            turn_number: expires_at,
                        },
                    },
                },
            ])
        }
        "chaos" => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let allowed_cards = state
                .hand(&target)
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
                })?
                .to_vec();
            if allowed_cards.is_empty() {
                return Ok(Vec::new());
            }
            Ok(vec![EffectIntent::RequestChoice {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::EffectGenerated {
                    effect_id: resolver_id.to_string(),
                    continuation_id: "chaos:return-two".to_string(),
                    allowed_cards,
                },
            }])
        }
        "return-to-origin" => {
            let team = player_team(state, player)?;
            Ok(vec![EffectIntent::ChangeHp {
                team,
                delta: level_sum(state, used_cards)? * 4,
            }])
        }
        "five-elements-cycle" => {
            let own_team = player_team(state, player)?;
            let opposing_team =
                resolve_rule_team_target(state, player, RuleTeamTarget::OpposingSide)?;
            let own_hp = team_hp(state, &own_team)?;
            let opposing_hp = team_hp(state, &opposing_team)?;
            Ok(vec![
                EffectIntent::ChangeHp {
                    team: own_team,
                    delta: opposing_hp - own_hp,
                },
                EffectIntent::ChangeHp {
                    team: opposing_team,
                    delta: own_hp - opposing_hp,
                },
            ])
        }
        _ => Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(resolver_id.to_string()),
        )),
    }
}

fn metamorphosis_intents(
    state: &GameState,
    player: &PlayerId,
    used_cards: &[CardInstanceId],
) -> GameResult<Vec<EffectIntent>> {
    let previous_player =
        resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)?;
    let Some(last_formation) = state.last_formation_by_player.get(&previous_player) else {
        return Ok(Vec::new());
    };

    let registry = base_formation_registry();
    let Some(formation) = registry.formation(&last_formation.formation_id) else {
        return Ok(Vec::new());
    };
    let effect = registry
        .effect_for(formation)
        .expect("base formation registry must link every formation to an effect");

    match &effect.plan {
        EffectPlan::Attack(plan) => Ok(vec![EffectIntent::ResolveCopiedAttack {
            formation_id: formation.id.clone(),
            category: formation.category.clone(),
            point_formula: plan.point_formula.clone(),
            used_cards: used_cards.to_vec(),
        }]),
        EffectPlan::ActiveSpell(spell) if spell.resolver_id != "metamorphosis" => {
            active_spell_intents(state, player, &spell.resolver_id, used_cards)
        }
        EffectPlan::ActiveSpell(_) | EffectPlan::PassiveSpell(_) => Ok(Vec::new()),
    }
}

fn level_sum(state: &GameState, cards: &[CardInstanceId]) -> GameResult<i32> {
    cards.iter().try_fold(0, |sum, card| {
        let level = state
            .card_def(*card)
            .ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(*card),
            ))?
            .level as i32;
        Ok(sum + level)
    })
}

fn team_hp(state: &GameState, team: &TeamId) -> GameResult<i32> {
    state
        .hp
        .iter()
        .find(|team_hp| &team_hp.team == team)
        .map(|team_hp| team_hp.hp)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))
}

fn nth_future_turn_for_player(
    state: &GameState,
    player: &PlayerId,
    occurrence: usize,
) -> GameResult<u64> {
    TurnOrderTargets::new(state).nth_future_turn_for_player(player, occurrence)
}

fn effect_intent_events(
    state: &GameState,
    intents: Vec<EffectIntent>,
) -> GameResult<Vec<GameEvent>> {
    let mut requested_choice_player = None;
    let mut events = Vec::new();

    for intent in intents {
        let event = match intent {
            EffectIntent::SetShield { player, value } => {
                let old_value = state.shield(&player).unwrap_or(0);
                if old_value == value {
                    continue;
                }
                GameEvent::ShieldChanged {
                    player,
                    old_value,
                    delta: value - old_value,
                    new_value: value,
                }
            }
            EffectIntent::ChangeHp { team, delta } => {
                if delta == 0 {
                    continue;
                }
                let old_hp = state
                    .hp
                    .iter()
                    .find(|team_hp| team_hp.team == team)
                    .ok_or_else(|| {
                        GameError::Validation(ValidationError::MissingTeamHp(team.clone()))
                    })?
                    .hp;
                GameEvent::HpChanged {
                    change: HpChangeDelta {
                        team,
                        old_hp,
                        delta,
                        new_hp: old_hp + delta,
                        effective_delta: delta,
                    },
                }
            }
            EffectIntent::MoveCards { card_moves } => GameEvent::CardsMoved { card_moves },
            EffectIntent::AddStatus { status } => GameEvent::StatusAdded { status },
            EffectIntent::ResolveCopiedAttack {
                formation_id,
                category,
                point_formula,
                used_cards,
            } => {
                let target = attack_target(
                    state,
                    &state
                        .current_player()
                        .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                        .clone(),
                    &AttackPlanDef {
                        point_formula: point_formula.clone(),
                        damage_target: DamageTarget::PreviousPlayer,
                    },
                )?;
                let target_team = player_team(state, &target)?;
                let player = state
                    .current_player()
                    .ok_or(GameError::Validation(ValidationError::EmptyTurnOrder))?
                    .clone();
                let points = compute_attack_points(state, &point_formula, &used_cards, &target)?;
                let has_target_shield = state.shield(&target).is_some_and(|value| value > 0);
                let point_breakdown =
                    attack_point_breakdown(state, &category, &target, points, has_target_shield);
                let shield_change = shield_absorption(state, &target, point_breakdown.final_amount);
                let hp_change = if shield_change.is_some() {
                    no_hp_change(state, &target_team)?
                } else {
                    apply_attack_amount(
                        state,
                        &target_team,
                        point_breakdown.final_amount,
                        point_breakdown.damage_transform,
                    )?
                };
                GameEvent::AttackResolved {
                    attacker: player.clone(),
                    target,
                    formation_id,
                    used_cards,
                    point_breakdown,
                    hp_change,
                    shield_change,
                    card_moves: Vec::new(),
                    elemental_context_update: elemental_context_update(
                        &category,
                        state.turn_number,
                    )
                    .map(|attack| LastElementalAttackUpdate { player, attack }),
                }
            }
            EffectIntent::ModifyAction { modification } => match modification {
                ActionModification::PreventDamage
                | ActionModification::SplitAttackDamage
                | ActionModification::CancelSpell
                | ActionModification::SealCoveredPassive => continue,
            },
            EffectIntent::RequestChoice { player, kind } => {
                if let Some(existing_player) = requested_choice_player {
                    return Err(GameError::EngineInvariant(
                        EngineInvariantError::DuplicatePendingChoice {
                            player: existing_player,
                        },
                    ));
                }

                requested_choice_player = Some(player.clone());
                GameEvent::EffectChoiceRequested { player, kind }
            }
        };

        events.push(event);
    }

    Ok(events)
}

fn resume_effect_choice_intents(
    state: &GameState,
    player: &PlayerId,
    effect_id: &str,
    continuation_id: &str,
    selected_cards: &[CardInstanceId],
) -> GameResult<Vec<EffectIntent>> {
    match (effect_id, continuation_id) {
        ("metamorphosis", "metamorphosis:choose-card") => {
            let selected_card = selected_cards
                .first()
                .ok_or(GameError::Validation(ValidationError::MissingPendingChoice))?;
            let value = state
                .card_def(*selected_card)
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(*selected_card),
                ))?
                .level as i32;

            Ok(vec![EffectIntent::SetShield {
                player: player.clone(),
                value,
            }])
        }
        ("chaos", "chaos:return-two") => {
            let target = resolve_rule_player_target(state, player, RulePlayerTarget::NextPlayer)?;
            let target_hand = state.hand(&target).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
            })?;
            let required_count = target_hand.len().min(2);
            if selected_cards.len() != required_count {
                return Err(GameError::Validation(ValidationError::MissingPendingChoice));
            }

            Ok(vec![EffectIntent::MoveCards {
                card_moves: selected_cards
                    .iter()
                    .rev()
                    .copied()
                    .map(|card| CardMoveDelta {
                        card,
                        from: CardZone::Hand(target.clone()),
                        to: CardZone::DeckTop,
                    })
                    .collect(),
            }])
        }
        _ => Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(
                continuation_id.to_string(),
            ),
        )),
    }
}

fn attack_target(
    state: &GameState,
    attacker: &PlayerId,
    plan: &AttackPlanDef,
) -> GameResult<PlayerId> {
    match plan.damage_target {
        DamageTarget::PreviousPlayer => {
            resolve_rule_player_target(state, attacker, RulePlayerTarget::PreviousPlayer)
        }
        DamageTarget::DeclaredPlayer | DamageTarget::TeamOfDeclaredPlayer => {
            Err(GameError::RuleImplementation(
                crate::domain::RuleImplementationError::EffectNotImplemented(
                    "declared-attack-target".to_string(),
                ),
            ))
        }
    }
}

fn resolve_rule_player_target(
    state: &GameState,
    player: &PlayerId,
    target: RulePlayerTarget,
) -> GameResult<PlayerId> {
    TurnOrderTargets::new(state).player_target(player, target)
}

fn resolve_rule_team_target(
    state: &GameState,
    player: &PlayerId,
    target: RuleTeamTarget,
) -> GameResult<TeamId> {
    TurnOrderTargets::new(state).team_target(player, target)
}

#[allow(dead_code)]
fn adjacent_player(state: &GameState, player: &PlayerId, offset: isize) -> GameResult<PlayerId> {
    TurnOrderTargets::new(state).adjacent_player(player, offset)
}

fn previous_player(state: &GameState, player: &PlayerId) -> GameResult<PlayerId> {
    resolve_rule_player_target(state, player, RulePlayerTarget::PreviousPlayer)
}

fn player_team(state: &GameState, player: &PlayerId) -> GameResult<TeamId> {
    TurnOrderTargets::new(state).team_of(player)
}

fn compute_attack_points(
    state: &GameState,
    formula: &PointFormula,
    cards: &[CardInstanceId],
    target: &PlayerId,
) -> GameResult<i32> {
    let level_sum = || -> GameResult<i32> {
        cards.iter().try_fold(0, |sum, card| {
            let level = state
                .card_def(*card)
                .ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(*card),
                ))?
                .level as i32;
            Ok(sum + level)
        })
    };

    match formula {
        PointFormula::Fixed(points) => Ok(*points as i32),
        PointFormula::CardCount => Ok(cards.len() as i32),
        PointFormula::FormationPoints => level_sum(),
        PointFormula::LevelPlus(bonus) => Ok(state
            .card_def(cards[0])
            .ok_or(GameError::Validation(
                ValidationError::MissingCardInstanceDefinition(cards[0]),
            ))?
            .level as i32
            + *bonus as i32),
        PointFormula::LevelSumTimes(multiplier) => Ok(level_sum()? * *multiplier as i32),
        PointFormula::TargetHandCountTimes(multiplier) => {
            let target_hand = state.hand(target).ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownPlayer(target.clone()))
            })?;
            Ok(target_hand.len() as i32 * *multiplier as i32)
        }
    }
}

fn attack_point_breakdown(
    state: &GameState,
    category: &FormationCategory,
    target: &PlayerId,
    base_points: i32,
    skip_interaction: bool,
) -> AttackPointBreakdown {
    let Some(current_element) = elemental_attack_element(category) else {
        return AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        };
    };
    if skip_interaction {
        return AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        };
    }
    let Some(previous_attack) = state.last_elemental_attack_by_player.get(target) else {
        return AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        };
    };

    if current_element == previous_attack.element {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::Same,
            damage_transform: DamageTransform::HalfDamageRoundUp,
            final_amount: (base_points + 1) / 2,
        }
    } else if generates(current_element, previous_attack.element) {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::Generating,
            damage_transform: DamageTransform::HealTarget,
            final_amount: base_points,
        }
    } else if overcomes(current_element, previous_attack.element) {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::Overcoming,
            damage_transform: DamageTransform::DoubleDamage,
            final_amount: base_points * 2,
        }
    } else {
        AttackPointBreakdown {
            base_points,
            interaction: ElementInteraction::None,
            damage_transform: DamageTransform::NormalDamage,
            final_amount: base_points,
        }
    }
}

fn elemental_attack_element(category: &FormationCategory) -> Option<crate::domain::Element> {
    match category {
        FormationCategory::Attack(AttackCategory::Elemental(element)) => Some(*element),
        FormationCategory::Attack(AttackCategory::Physical | AttackCategory::Special)
        | FormationCategory::Spell(_) => None,
    }
}

fn generates(current: crate::domain::Element, previous: crate::domain::Element) -> bool {
    matches!(
        (current, previous),
        (crate::domain::Element::Metal, crate::domain::Element::Water)
            | (crate::domain::Element::Water, crate::domain::Element::Wood)
            | (crate::domain::Element::Wood, crate::domain::Element::Fire)
            | (crate::domain::Element::Fire, crate::domain::Element::Earth)
            | (crate::domain::Element::Earth, crate::domain::Element::Metal)
    )
}

fn overcomes(current: crate::domain::Element, previous: crate::domain::Element) -> bool {
    matches!(
        (current, previous),
        (crate::domain::Element::Metal, crate::domain::Element::Wood)
            | (crate::domain::Element::Wood, crate::domain::Element::Earth)
            | (crate::domain::Element::Earth, crate::domain::Element::Water)
            | (crate::domain::Element::Water, crate::domain::Element::Fire)
            | (crate::domain::Element::Fire, crate::domain::Element::Metal)
    )
}

fn apply_attack_amount(
    state: &GameState,
    team: &TeamId,
    amount: i32,
    transform: DamageTransform,
) -> GameResult<HpChangeDelta> {
    let old_hp = state
        .hp
        .iter()
        .find(|team_hp| &team_hp.team == team)
        .map(|team_hp| team_hp.hp)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;
    let delta = match transform {
        DamageTransform::HealTarget => amount,
        DamageTransform::NormalDamage
        | DamageTransform::DoubleDamage
        | DamageTransform::HalfDamageRoundUp => -amount,
    };
    let new_hp = (old_hp + delta).max(0);

    Ok(HpChangeDelta {
        team: team.clone(),
        old_hp,
        delta,
        new_hp,
        effective_delta: new_hp - old_hp,
    })
}

fn no_hp_change(state: &GameState, team: &TeamId) -> GameResult<HpChangeDelta> {
    let old_hp = state
        .hp
        .iter()
        .find(|team_hp| &team_hp.team == team)
        .map(|team_hp| team_hp.hp)
        .ok_or_else(|| GameError::Validation(ValidationError::MissingTeamHp(team.clone())))?;

    Ok(HpChangeDelta {
        team: team.clone(),
        old_hp,
        delta: 0,
        new_hp: old_hp,
        effective_delta: 0,
    })
}

fn shield_absorption(
    state: &GameState,
    player: &PlayerId,
    incoming_damage: i32,
) -> Option<ShieldChangeDelta> {
    let old_value = state.shield(player)?;
    if old_value <= 0 {
        return None;
    }

    Some(ShieldChangeDelta {
        player: player.clone(),
        old_value,
        delta: -incoming_damage,
        new_value: (old_value - incoming_damage).max(0),
    })
}

fn apply_card_move(state: &mut GameState, card_move: &CardMoveDelta) {
    let removed = match &card_move.from {
        CardZone::Hand(player) => {
            let hand = state
                .hand_mut(player)
                .expect("canonical card move must move cards from a known hand");
            let position = hand
                .iter()
                .position(|card| card == &card_move.card)
                .expect("canonical card move must move an existing card");
            hand.remove(position)
        }
        CardZone::DeckTop => {
            assert!(
                !state.deck.is_empty(),
                "canonical card move must move from a non-empty deck"
            );
            let position = state
                .deck
                .iter()
                .position(|deck_card| deck_card == &card_move.card)
                .expect("canonical card move must move an existing deck card");
            state.deck.remove(position)
        }
        CardZone::Discard => {
            let position = state
                .discard
                .iter()
                .position(|card| card == &card_move.card)
                .expect("canonical card move must move an existing discarded card");
            state.discard.remove(position)
        }
    };

    match &card_move.to {
        CardZone::Hand(player) => {
            let hand = state
                .hand_mut(player)
                .expect("canonical card move must move cards to a known hand");
            hand.push(removed);
        }
        CardZone::DeckTop => state.deck.insert(0, removed),
        CardZone::Discard => state.discard.push(removed),
    }
}

fn apply_shield_change(state: &mut GameState, change: &ShieldChangeDelta) {
    let shield = state
        .shields
        .iter_mut()
        .find(|shield| shield.player == change.player)
        .expect("canonical shield event must target an existing player");
    shield.value = change.new_value;
}

fn elemental_context_update(
    category: &FormationCategory,
    resolved_turn: u64,
) -> Option<LastElementalAttack> {
    match category {
        FormationCategory::Attack(AttackCategory::Elemental(element)) => {
            Some(LastElementalAttack {
                element: *element,
                resolved_turn,
            })
        }
        FormationCategory::Attack(AttackCategory::Physical | AttackCategory::Special)
        | FormationCategory::Spell(_) => None,
    }
}

pub fn apply_event(state: &mut GameState, event: &GameEvent) {
    match event {
        GameEvent::DeckPrepared { deck_order } => {
            state.deck = deck_order.clone();
        }
        GameEvent::CardsDealt { player, cards } => {
            let hand = state
                .hand_mut(player)
                .expect("canonical deal event must target a known player");
            hand.extend(cards.iter().copied());

            for card in cards {
                let position = state
                    .deck
                    .iter()
                    .position(|deck_card| deck_card == card)
                    .expect("canonical deal event must contain cards from deck");
                state.deck.remove(position);
            }
        }
        GameEvent::TurnStarted { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnStart);
            state.phase = Phase::Main;
        }
        GameEvent::ActionPassed { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::Main);
            state.phase = Phase::TurnDraw;
        }
        GameEvent::FormationPerformed {
            player,
            formation_id,
            used_cards,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::Main);

            for used_card in used_cards {
                let hand = state
                    .hand_mut(player)
                    .expect("canonical formation event must target a known player");
                let position = hand
                    .iter()
                    .position(|card| card == used_card)
                    .expect("canonical formation event must remove cards from hand");
                let removed = hand.remove(position);
                state.discard.push(removed);
            }

            state.last_formation_by_player.insert(
                player.clone(),
                LastFormationUse {
                    formation_id: formation_id.clone(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = Phase::TurnDraw;
        }
        GameEvent::PassiveCovered {
            player,
            formation_id,
            cards,
            sealed,
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::Main);

            let hand = state
                .hand_mut(player)
                .expect("canonical passive cover event must target a known player");
            for card in cards {
                let position = hand
                    .iter()
                    .position(|hand_card| hand_card == card)
                    .expect("canonical passive cover event must remove cards from hand");
                hand.remove(position);
            }

            state.covered_passives.push(crate::domain::CoveredPassive {
                owner: player.clone(),
                formation_id: formation_id.clone(),
                cards: cards.clone(),
                sealed: *sealed,
                covered_on_turn: state.turn_number,
                reveal_timing: crate::domain::PassiveTriggerTiming::NextPlayerActionStart,
            });
            state.last_formation_by_player.insert(
                player.clone(),
                LastFormationUse {
                    formation_id: formation_id.clone(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = Phase::TurnDraw;
        }
        GameEvent::PassiveFlipped {
            owner,
            passive_id: _,
            cards,
            ..
        } => {
            let passive_position = state
                .covered_passives
                .iter()
                .position(|passive| &passive.owner == owner)
                .expect("canonical passive flip event must target a covered passive");
            state.covered_passives.remove(passive_position);
            state.discard.extend(cards.iter().copied());
        }
        GameEvent::AttackResolved {
            attacker,
            formation_id,
            hp_change,
            shield_change,
            card_moves,
            elemental_context_update,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(attacker));
            debug_assert!(matches!(state.phase, Phase::Main | Phase::TurnDraw));

            let team_hp = state
                .hp
                .iter_mut()
                .find(|team_hp| team_hp.team == hp_change.team)
                .expect("canonical attack event must target an existing team");
            team_hp.hp = hp_change.new_hp;
            finish_game_if_needed(state);

            if let Some(shield_change) = shield_change {
                apply_shield_change(state, shield_change);
            }

            for card_move in card_moves {
                apply_card_move(state, card_move);
            }

            if let Some(update) = elemental_context_update {
                state
                    .last_elemental_attack_by_player
                    .insert(update.player.clone(), update.attack.clone());
            }

            state.last_formation_by_player.insert(
                attacker.clone(),
                LastFormationUse {
                    formation_id: formation_id.clone(),
                    resolved_turn: state.turn_number,
                },
            );
            state.phase = Phase::TurnDraw;
        }
        GameEvent::TurnDrawBonusChanged {
            player, new_value, ..
        } => {
            state
                .turn_draw_bonus_by_player
                .insert(player.clone(), *new_value);
        }
        GameEvent::HpChanged { change } => {
            let team_hp = state
                .hp
                .iter_mut()
                .find(|team_hp| team_hp.team == change.team)
                .expect("canonical hp event must target an existing team");
            team_hp.hp = change.new_hp;
            finish_game_if_needed(state);
        }
        GameEvent::CardsMoved { card_moves } => {
            for card_move in card_moves {
                apply_card_move(state, card_move);
            }
        }
        GameEvent::ShieldChanged {
            player,
            old_value: _,
            delta: _,
            new_value,
        } => {
            apply_shield_change(
                state,
                &ShieldChangeDelta {
                    player: player.clone(),
                    old_value: state.shield(player).unwrap_or(0),
                    delta: new_value - state.shield(player).unwrap_or(0),
                    new_value: *new_value,
                },
            );
        }
        GameEvent::StatusAdded { status } => {
            state.statuses.push(status.clone());
        }
        GameEvent::StatusExpired {
            status_id, owner, ..
        }
        | GameEvent::StatusRemoved { status_id, owner } => {
            let position = state
                .statuses
                .iter()
                .position(|status| &status.id == status_id && &status.owner == owner)
                .expect("canonical status removal event must target an active status");
            state.statuses.remove(position);
        }
        GameEvent::EffectChoiceRequested { player, kind } => {
            state.pending_choice = Some(crate::domain::PendingChoice {
                player: player.clone(),
                kind: kind.clone(),
            });
        }
        GameEvent::EffectChoiceAnswered {
            player,
            selected_cards,
            ..
        } => {
            match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind: crate::domain::PendingChoiceKind::EffectGenerated { allowed_cards, .. },
                }) if choice_player == player => {
                    for selected_card in selected_cards {
                        if !allowed_cards.contains(selected_card) {
                            panic!("canonical effect choice answer must select allowed cards");
                        }
                    }
                }
                _ => panic!("canonical effect choice answer must have a matching pending choice"),
            }

            state.pending_choice = None;
        }
        GameEvent::CardsDrawnForTurnDiscardChoice {
            player,
            drawn_cards,
            allowed_discards,
        } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnDraw);

            let hand = state
                .hand_mut(player)
                .expect("canonical draw event must target a known player");
            hand.extend(drawn_cards.iter().copied());
            state.deck.drain(0..drawn_cards.len());
            state.pending_choice = Some(crate::domain::PendingChoice {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::TurnDrawDiscard {
                    drawn_cards: drawn_cards.clone(),
                    allowed_discards: allowed_discards.clone(),
                },
            });
            state.phase = Phase::TurnDrawDiscardChoice;
        }
        GameEvent::TurnDiscardChosen { player, discard } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnDrawDiscardChoice);

            let allowed_discards = match &state.pending_choice {
                Some(crate::domain::PendingChoice {
                    player: choice_player,
                    kind:
                        crate::domain::PendingChoiceKind::TurnDrawDiscard {
                            allowed_discards, ..
                        },
                }) if choice_player == player => allowed_discards,
                _ => panic!("canonical discard event must have a matching pending choice"),
            };

            if !allowed_discards.contains(discard) {
                panic!("canonical discard event must choose an allowed card");
            }

            let hand = state
                .hand_mut(player)
                .expect("canonical discard event must target a known player");
            let discard_position = hand
                .iter()
                .position(|card| card == discard)
                .expect("canonical discard event must remove a card from hand");
            let discarded = hand.remove(discard_position);

            state.discard.push(discarded);
            state.pending_choice = None;
            state.phase = Phase::TurnEnd;
        }
        GameEvent::TurnDrawSkipped { player, .. } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnDraw);
            state.phase = Phase::TurnEnd;
        }
        GameEvent::DiscardRecycledIntoDeck {
            shuffled_order,
            placement,
        } => {
            debug_assert_eq!(state.phase, Phase::TurnDraw);

            match placement {
                DeckPlacement::Bottom => state.deck.extend(shuffled_order.iter().copied()),
            }

            for card in shuffled_order {
                let position = state
                    .discard
                    .iter()
                    .position(|discarded| discarded == card)
                    .expect("canonical recycle event must contain cards from discard");
                state.discard.remove(position);
            }
        }
        GameEvent::TurnEnded { player } => {
            debug_assert_eq!(state.current_player(), Some(player));
            debug_assert_eq!(state.phase, Phase::TurnEnd);

            state.turn_draw_bonus_by_player.remove(player);
            state.current_turn_index = (state.current_turn_index + 1) % state.turn_order.len();
            state.turn_number += 1;
            state.phase = Phase::TurnStart;
        }
    }
}

fn finish_game_if_needed(state: &mut GameState) {
    let alive_teams = state
        .hp
        .iter()
        .filter(|team_hp| team_hp.hp > 0)
        .map(|team_hp| team_hp.team.clone())
        .collect::<Vec<_>>();
    let defeated_count = state.hp.iter().filter(|team_hp| team_hp.hp == 0).count();

    if defeated_count == 0 {
        return;
    }

    state.status = if alive_teams.len() == 1 {
        GameStatus::Finished {
            outcome: GameOutcome::Team(alive_teams[0].clone()),
        }
    } else {
        GameStatus::Finished {
            outcome: GameOutcome::Draw,
        }
    };
}

pub fn replay(setup: &GameSetup, events: &[GameEvent]) -> Result<GameState, GameError> {
    validate_setup(setup)?;
    let mut state = GameState::from_setup(setup);
    for event in events {
        apply_event(&mut state, event);
    }
    Ok(state)
}

pub fn verify_recorded_events(
    setup: &GameSetup,
    recorded_events: &[RecordedEvent],
) -> Result<GameState, ReplayVerificationError> {
    validate_setup(setup).map_err(|error| ReplayVerificationError::DecisionFailed {
        sequence: 0,
        source: EventSource::Setup,
        error: Box::new(error),
    })?;

    let mut state = GameState::from_setup(setup);
    let mut index = 0;

    while index < recorded_events.len() {
        let sequence = recorded_events[index].metadata.sequence;
        let source = recorded_events[index].metadata.source.clone();
        let end = verification_group_end(recorded_events, index);
        let actual = recorded_events[index..end]
            .iter()
            .map(|recorded| recorded.event.clone())
            .collect::<Vec<_>>();

        let expected = match &source {
            EventSource::Setup => {
                let deck_order = actual
                    .iter()
                    .find_map(|event| match event {
                        GameEvent::DeckPrepared { deck_order } => Some(deck_order.clone()),
                        _ => None,
                    })
                    .ok_or(ReplayVerificationError::MissingSetupDeck { sequence })?;
                initial_events(setup, deck_order).map_err(|error| {
                    ReplayVerificationError::DecisionFailed {
                        sequence,
                        source: source.clone(),
                        error: Box::new(error),
                    }
                })?
            }
            EventSource::Automatic { .. } => advance_automatic(&state).map_err(|error| {
                ReplayVerificationError::DecisionFailed {
                    sequence,
                    source: source.clone(),
                    error: Box::new(error),
                }
            })?,
            EventSource::Command { context, .. } => {
                let command = command_from_recorded_events(context, &actual).ok_or_else(|| {
                    ReplayVerificationError::CommandContextUnavailable {
                        sequence,
                        source: source.clone(),
                    }
                })?;
                handle_command(&state, command).map_err(|error| {
                    ReplayVerificationError::DecisionFailed {
                        sequence,
                        source: source.clone(),
                        error: Box::new(error),
                    }
                })?
            }
        };

        if expected != actual {
            return Err(ReplayVerificationError::EventMismatch {
                sequence,
                expected,
                actual,
            });
        }

        for event in &actual {
            apply_event(&mut state, event);
        }
        index = end;
    }

    Ok(state)
}

fn verification_group_end(recorded_events: &[RecordedEvent], start: usize) -> usize {
    let source = &recorded_events[start].metadata.source;
    let mut end = start + 1;

    while end < recorded_events.len()
        && verification_sources_share_group(source, &recorded_events[end].metadata.source)
    {
        end += 1;
    }

    end
}

fn verification_sources_share_group(first: &EventSource, next: &EventSource) -> bool {
    match (first, next) {
        (EventSource::Setup, EventSource::Setup) => true,
        (EventSource::Automatic { .. }, EventSource::Automatic { .. }) => true,
        (
            EventSource::Command {
                command_id: first_id,
                ..
            },
            EventSource::Command {
                command_id: next_id,
                ..
            },
        ) => first_id == next_id,
        _ => false,
    }
}

fn command_from_recorded_events(context: &CommandContext, events: &[GameEvent]) -> Option<Command> {
    match &context.kind {
        CommandKind::PassAction => events.iter().find_map(|event| match event {
            GameEvent::ActionPassed { reason, .. } => Some(Command::PassAction {
                player: context.player.clone(),
                reason: *reason,
            }),
            _ => None,
        }),
        CommandKind::PerformFormation { formation_id } => {
            events.iter().find_map(|event| match event {
                GameEvent::FormationPerformed {
                    used_cards,
                    declared_targets,
                    ..
                } => Some(Command::PerformFormation {
                    player: context.player.clone(),
                    formation_id: formation_id.clone(),
                    cards: used_cards.clone(),
                    declared_targets: declared_targets.clone(),
                }),
                GameEvent::AttackResolved { used_cards, .. } => Some(Command::PerformFormation {
                    player: context.player.clone(),
                    formation_id: formation_id.clone(),
                    cards: used_cards.clone(),
                    declared_targets: Vec::new(),
                }),
                _ => None,
            })
        }
        CommandKind::ChooseTurnDiscard => events.iter().find_map(|event| match event {
            GameEvent::TurnDiscardChosen { discard, .. } => Some(Command::ChooseTurnDiscard {
                player: context.player.clone(),
                discard: *discard,
            }),
            _ => None,
        }),
        CommandKind::AnswerEffectChoice => events.iter().find_map(|event| match event {
            GameEvent::EffectChoiceAnswered { selected_cards, .. } => {
                Some(Command::AnswerEffectChoice {
                    player: context.player.clone(),
                    selected_cards: selected_cards.clone(),
                })
            }
            _ => None,
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn two_player_state() -> GameState {
        GameState::from_setup(&GameSetup::two_player(
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            30,
        ))
    }

    fn team_mode_state() -> GameState {
        GameState::from_setup(&GameSetup::team_mode(
            TeamId::new("A"),
            vec![PlayerId::new("p1"), PlayerId::new("p3")],
            TeamId::new("B"),
            vec![PlayerId::new("p2"), PlayerId::new("p4")],
            30,
        ))
    }

    #[test]
    fn formation_points_sum_submitted_card_levels() {
        let setup = GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).with_cards(
            vec![
                crate::domain::CardDef {
                    id: crate::domain::CardDefId::new("metal"),
                    name: "metal".to_string(),
                    element: crate::rules::Element::Metal,
                    level: 3,
                },
                crate::domain::CardDef {
                    id: crate::domain::CardDefId::new("wood"),
                    name: "wood".to_string(),
                    element: crate::rules::Element::Wood,
                    level: 2,
                },
            ],
            vec![
                crate::domain::CardInstanceDef {
                    instance: CardInstanceId::new(1),
                    definition: crate::domain::CardDefId::new("metal"),
                },
                crate::domain::CardInstanceDef {
                    instance: CardInstanceId::new(2),
                    definition: crate::domain::CardDefId::new("wood"),
                },
            ],
        );
        let state = GameState::from_setup(&setup);

        assert_eq!(
            compute_attack_points(
                &state,
                &PointFormula::FormationPoints,
                &[CardInstanceId::new(1), CardInstanceId::new(2)],
                &PlayerId::new("p2"),
            ),
            Ok(5)
        );
    }

    #[test]
    fn rule_player_targets_resolve_in_two_player_games() {
        let state = two_player_state();

        assert_eq!(
            resolve_rule_player_target(&state, &PlayerId::new("p1"), RulePlayerTarget::SelfPlayer),
            Ok(PlayerId::new("p1"))
        );
        assert_eq!(
            resolve_rule_player_target(
                &state,
                &PlayerId::new("p1"),
                RulePlayerTarget::PreviousPlayer
            ),
            Ok(PlayerId::new("p2"))
        );
        assert_eq!(
            resolve_rule_player_target(&state, &PlayerId::new("p1"), RulePlayerTarget::NextPlayer),
            Ok(PlayerId::new("p2"))
        );
    }

    #[test]
    fn rule_team_targets_resolve_in_two_player_games() {
        let state = two_player_state();

        assert_eq!(
            resolve_rule_team_target(&state, &PlayerId::new("p1"), RuleTeamTarget::OwnSide),
            Ok(TeamId::new("team:p1"))
        );
        assert_eq!(
            resolve_rule_team_target(&state, &PlayerId::new("p1"), RuleTeamTarget::OpposingSide),
            Ok(TeamId::new("team:p2"))
        );
    }

    #[test]
    fn rule_player_targets_resolve_in_team_mode() {
        let state = team_mode_state();

        assert_eq!(
            resolve_rule_player_target(&state, &PlayerId::new("p1"), RulePlayerTarget::SelfPlayer),
            Ok(PlayerId::new("p1"))
        );
        assert_eq!(
            resolve_rule_player_target(
                &state,
                &PlayerId::new("p1"),
                RulePlayerTarget::PreviousPlayer
            ),
            Ok(PlayerId::new("p4"))
        );
        assert_eq!(
            resolve_rule_player_target(&state, &PlayerId::new("p1"), RulePlayerTarget::NextPlayer),
            Ok(PlayerId::new("p2"))
        );
    }

    #[test]
    fn rule_team_targets_resolve_in_team_mode() {
        let state = team_mode_state();

        assert_eq!(
            resolve_rule_team_target(&state, &PlayerId::new("p1"), RuleTeamTarget::OwnSide),
            Ok(TeamId::new("A"))
        );
        assert_eq!(
            resolve_rule_team_target(&state, &PlayerId::new("p1"), RuleTeamTarget::OpposingSide),
            Ok(TeamId::new("B"))
        );
    }
}
