//! Application services: command handling, automatic advancement, and replay.

use crate::domain::{
    ActionModification, AttackPointBreakdown, CardInstanceId, CardMoveDelta, CardZone, Command,
    DamageTransform, DeckPlacement, ElementInteraction, EventMetadata, EventSource, GameError,
    GameEvent, GameOutcome, GameResult, GameSetup, GameState, GameStatus, HpChangeDelta,
    LastElementalAttack, LastElementalAttackUpdate, PassActionReason, PassiveFlipOutcome,
    PassiveNoEffectReason, Phase, PlayerId, PublicGameEvent, RecordedEvent, ShieldChangeDelta,
    TeamId, TurnDrawSkipReason, Viewer, validate_setup,
};
use crate::rules::{
    AttackCategory, AttackPlanDef, DamageTarget, EffectPlan, FormationCategory, PointFormula,
    base_formation_matcher, base_formation_registry,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameRecord {
    setup: GameSetup,
    events: Vec<GameEvent>,
    latest_snapshot: Option<GameState>,
}

impl GameRecord {
    pub fn start(setup: GameSetup, deck_order: Vec<CardInstanceId>) -> GameResult<Self> {
        validate_setup(&setup)?;
        validate_card_instances(&setup, &deck_order)?;
        let events = initial_events(&setup, deck_order)?;

        let record = Self {
            setup,
            events,
            latest_snapshot: None,
        };

        record.replay()?;
        Ok(record)
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
            .enumerate()
            .map(|(index, event)| RecordedEvent {
                metadata: EventMetadata {
                    sequence: index as u64 + 1,
                    source: event_source(event),
                },
                event: event.clone(),
            })
            .collect()
    }

    pub fn public_events_for(&self, viewer: Viewer) -> Vec<PublicGameEvent> {
        self.events
            .iter()
            .map(|event| event.view_for(viewer.clone()))
            .collect()
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

    pub fn handle(&mut self, command: Command) -> GameResult<Vec<GameEvent>> {
        let state = self.state()?;
        let events = handle_command(&state, command)?;
        self.events.extend(events.clone());
        Ok(events)
    }

    pub fn advance_automatic(&mut self) -> GameResult<Vec<GameEvent>> {
        let state = self.state()?;
        let events = advance_automatic(&state)?;
        self.events.extend(events.clone());
        Ok(events)
    }
}

fn event_source(event: &GameEvent) -> EventSource {
    match event {
        GameEvent::DeckPrepared { .. }
        | GameEvent::CardsDealt { .. }
        | GameEvent::TurnStarted { .. }
        | GameEvent::CardsDrawnForTurnDiscardChoice { .. }
        | GameEvent::StatusExpired { .. }
        | GameEvent::TurnDrawSkipped { .. }
        | GameEvent::DiscardRecycledIntoDeck { .. }
        | GameEvent::TurnEnded { .. } => EventSource::System,
        GameEvent::ActionPassed { .. }
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
        | GameEvent::TurnDiscardChosen { .. } => EventSource::Command,
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
            return Err(GameError::DuplicateCard(*card));
        }

        if !known_instances.contains(card) {
            return Err(GameError::MissingCardInstanceDefinition(*card));
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
        return Err(GameError::NotEnoughCards {
            needed,
            available: deck_order.len(),
        });
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
                        .ok_or(GameError::EmptyTurnOrder)?
                        .clone(),
                },
            )
            .or_else(|| {
                Some(GameEvent::TurnStarted {
                    player: projected
                        .current_player()
                        .ok_or(GameError::EmptyTurnOrder)
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
                        .ok_or(GameError::EmptyTurnOrder)?
                        .clone(),
                },
            )
            .or_else(|| {
                Some(GameEvent::TurnEnded {
                    player: projected
                        .current_player()
                        .ok_or(GameError::EmptyTurnOrder)
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
        .find(|status| status_expires_at(&status.duration, &timing))?;

    Some(GameEvent::StatusExpired {
        status_id: status.id.clone(),
        owner: status.owner.clone(),
        expired_at: timing,
    })
}

fn status_expires_at(
    duration: &crate::domain::StatusDuration,
    timing: &crate::domain::StatusExpiryTiming,
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
        (crate::domain::StatusDuration::Permanent, _) => false,
        _ => false,
    }
}

fn next_turn_draw_event(state: &GameState) -> GameResult<Option<GameEvent>> {
    let player = state
        .current_player()
        .ok_or(GameError::EmptyTurnOrder)?
        .clone();
    let hand = state
        .hand(&player)
        .ok_or_else(|| GameError::UnknownPlayer(player.clone()))?;
    let available_space = state.hand_limit.saturating_sub(hand.len());

    if available_space == 0 {
        return Ok(Some(GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::HandLimitReached,
        }));
    }

    let draw_count = state.base_draw.min(available_space) + 1;
    if state.deck.len() < draw_count {
        if state.deck.len() + state.discard.len() >= draw_count && !state.discard.is_empty() {
            return Ok(Some(GameEvent::DiscardRecycledIntoDeck {
                shuffled_order: state.discard.clone(),
                placement: DeckPlacement::Bottom,
            }));
        }

        return Err(GameError::NotEnoughCards {
            needed: draw_count,
            available: state.deck.len(),
        });
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
    if matches!(state.status, GameStatus::Finished { .. }) {
        return Err(GameError::GameFinished);
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
            return Err(GameError::PendingChoiceInProgress {
                player: choice.player.clone(),
            });
        }
    }

    match command {
        Command::PassAction { player, reason } => {
            ensure_current_player(state, &player)?;
            ensure_phase(state, Phase::Main)?;

            match reason {
                PassActionReason::NoCardsInHand => {
                    let hand = state
                        .hand(&player)
                        .ok_or_else(|| GameError::UnknownPlayer(player.clone()))?;
                    if !hand.is_empty() {
                        return Err(GameError::CannotPassAction { reason });
                    }
                }
                PassActionReason::CannotActByStatus => {
                    if !player_has_status(state, &player, "CannotAct") {
                        return Err(GameError::CannotPassAction { reason });
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

            let registry = base_formation_registry();
            let formation = registry
                .formation(&formation_id)
                .ok_or_else(|| GameError::UnknownFormation(formation_id.clone()))?;

            let hand = state
                .hand(&player)
                .ok_or_else(|| GameError::UnknownPlayer(player.clone()))?;
            let mut seen = HashSet::new();
            let mut submitted_elements = Vec::new();

            for card in &cards {
                if !seen.insert(*card) {
                    return Err(GameError::DuplicateSubmittedCard(*card));
                }

                if !hand.contains(card) {
                    return Err(GameError::CardNotInHand(*card));
                }

                submitted_elements.push(
                    state
                        .card_element(*card)
                        .ok_or(GameError::MissingCardInstanceDefinition(*card))?,
                );
            }

            if !base_formation_matcher().matches(&formation.pattern, &submitted_elements) {
                return Err(GameError::FormationPatternMismatch { formation_id });
            }

            let effect = registry
                .effect_for(formation)
                .expect("base formation registry must link every formation to an effect");

            match &effect.plan {
                EffectPlan::Attack(plan) => {
                    if !declared_targets.is_empty() {
                        return Err(GameError::UnexpectedDeclaredTargets { formation_id });
                    }
                    let passive_resolutions =
                        passive_resolutions(state, &player, IncomingActionKind::Attack);
                    let action_modifications = action_modifications(&passive_resolutions);
                    let mut events = passive_events(passive_resolutions);
                    let target = attack_target(state, &player, &plan)?;
                    let target_team = player_team(state, &target)?;
                    let points =
                        compute_attack_points(state, &plan.point_formula, &cards, &target)?;
                    let has_target_shield = state.shield(&target).is_some_and(|value| value > 0);
                    let point_breakdown = attack_point_breakdown(
                        state,
                        &formation.category,
                        &target,
                        points,
                        has_target_shield,
                    );
                    let damage_prevented =
                        action_modifications.contains(&ActionModification::PreventDamage);
                    let shield_change = if damage_prevented {
                        None
                    } else {
                        shield_absorption(state, &target, point_breakdown.final_amount)
                    };
                    let hp_change = if damage_prevented || shield_change.is_some() {
                        no_hp_change(state, &target_team)?
                    } else {
                        apply_attack_amount(
                            state,
                            &target_team,
                            point_breakdown.final_amount,
                            point_breakdown.damage_transform,
                        )?
                    };
                    let card_moves = cards
                        .iter()
                        .copied()
                        .map(|card| CardMoveDelta {
                            card,
                            from: CardZone::Hand(player.clone()),
                            to: CardZone::Discard,
                        })
                        .collect::<Vec<_>>();
                    let elemental_context_update =
                        elemental_context_update(&formation.category, state.turn_number).map(
                            |attack| LastElementalAttackUpdate {
                                player: player.clone(),
                                attack,
                            },
                        );

                    events.push(GameEvent::AttackResolved {
                        attacker: player,
                        target,
                        formation_id,
                        used_cards: cards,
                        point_breakdown,
                        hp_change,
                        shield_change,
                        card_moves,
                        elemental_context_update,
                    });

                    Ok(events)
                }
                EffectPlan::PassiveSpell(_) => {
                    if state
                        .covered_passives
                        .iter()
                        .any(|passive| passive.owner == player)
                    {
                        return Err(GameError::PendingPassiveAlreadyCovered { player });
                    }

                    let passive_resolutions =
                        passive_resolutions(state, &player, IncomingActionKind::PassiveSpell);
                    let action_modifications = action_modifications(&passive_resolutions);
                    let mut events = passive_events(passive_resolutions);
                    events.push(GameEvent::PassiveCovered {
                        player,
                        formation_id,
                        cards,
                        sealed: action_modifications
                            .contains(&ActionModification::SealCoveredPassive),
                    });
                    Ok(events)
                }
                EffectPlan::ActiveSpell(spell) => {
                    let passive_resolutions =
                        passive_resolutions(state, &player, IncomingActionKind::ActiveSpell);
                    let action_modifications = action_modifications(&passive_resolutions);
                    let mut events = passive_events(passive_resolutions);
                    events.push(GameEvent::FormationPerformed {
                        player: player.clone(),
                        formation_id,
                        used_cards: cards.clone(),
                        declared_targets,
                    });
                    if !action_modifications.contains(&ActionModification::CancelSpell) {
                        let intents =
                            active_spell_intents(state, &player, &spell.resolver_id, &cards)?;
                        events.extend(effect_intent_events(state, intents)?);
                    }
                    Ok(events)
                }
            }
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
                _ => return Err(GameError::MissingPendingChoice),
            };

            if !allowed_discards.contains(&discard) {
                return Err(GameError::IllegalDiscard(discard));
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
                _ => return Err(GameError::MissingPendingChoice),
            };

            for selected_card in &selected_cards {
                if !allowed_cards.contains(selected_card) {
                    return Err(GameError::IllegalChoiceCard(*selected_card));
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

fn ensure_current_player(state: &GameState, actual: &crate::domain::PlayerId) -> GameResult<()> {
    let expected = state.current_player().ok_or(GameError::EmptyTurnOrder)?;

    if expected == actual {
        Ok(())
    } else {
        Err(GameError::WrongPlayer {
            expected: expected.clone(),
            actual: actual.clone(),
        })
    }
}

fn ensure_phase(state: &GameState, expected: Phase) -> GameResult<()> {
    if state.phase == expected {
        Ok(())
    } else {
        Err(GameError::WrongPhase {
            expected,
            actual: state.phase,
        })
    }
}

fn player_has_status(state: &GameState, player: &crate::domain::PlayerId, kind: &str) -> bool {
    state.statuses.iter().any(|status| {
        matches!(&status.owner, crate::domain::StatusOwner::Player(owner) if owner == player)
            && status.kind == kind
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
    RemoveStatus {
        status_id: String,
        owner: crate::domain::StatusOwner,
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
            value: 5,
        }]),
        "metamorphosis" => {
            let allowed_cards = state
                .hand(player)
                .ok_or_else(|| GameError::UnknownPlayer(player.clone()))?
                .iter()
                .copied()
                .filter(|card| !used_cards.contains(card))
                .collect::<Vec<_>>();

            Ok(vec![EffectIntent::RequestChoice {
                player: player.clone(),
                kind: crate::domain::PendingChoiceKind::EffectGenerated {
                    effect_id: resolver_id.to_string(),
                    continuation_id: "metamorphosis:choose-card".to_string(),
                    allowed_cards,
                },
            }])
        }
        "generating-formation" => {
            let team = player_team(state, player)?;
            Ok(vec![EffectIntent::ChangeHp { team, delta: 3 }])
        }
        "overcoming-formation" => {
            let target = previous_player(state, player)?;
            let team = player_team(state, &target)?;
            Ok(vec![EffectIntent::ChangeHp { team, delta: -3 }])
        }
        "radiance" => Ok(state
            .statuses
            .iter()
            .filter(|status| {
                matches!(&status.owner, crate::domain::StatusOwner::Player(owner) if owner == player)
            })
            .map(|status| EffectIntent::RemoveStatus {
                status_id: status.id.clone(),
                owner: status.owner.clone(),
            })
            .collect()),
        "chaos" => {
            let target = previous_player(state, player)?;
            Ok(vec![EffectIntent::AddStatus {
                status: crate::domain::StatusEffect {
                    id: format!(
                        "chaos-cannot-act-{}-turn-{}",
                        target.as_str(),
                        state.turn_number
                    ),
                    owner: crate::domain::StatusOwner::Player(target.clone()),
                    kind: "CannotAct".to_string(),
                    value: None,
                    duration: crate::domain::StatusDuration::UntilTurnEnd {
                        player: target.clone(),
                    },
                },
            }])
        }
        "return-to-origin" => {
            let team = player_team(state, player)?;
            Ok(vec![EffectIntent::ChangeHp { team, delta: 5 }])
        }
        "five-elements-cycle" => Ok(state
            .discard
            .first()
            .copied()
            .map(|card| {
                vec![EffectIntent::MoveCards {
                    card_moves: vec![CardMoveDelta {
                        card,
                        from: CardZone::Discard,
                        to: CardZone::Hand(player.clone()),
                    }],
                }]
            })
            .unwrap_or_default()),
        _ => Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(resolver_id.to_string()),
        )),
    }
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
                GameEvent::ShieldChanged {
                    player,
                    old_value,
                    delta: value - old_value,
                    new_value: value,
                }
            }
            EffectIntent::ChangeHp { team, delta } => {
                let old_hp = state
                    .hp
                    .iter()
                    .find(|team_hp| team_hp.team == team)
                    .ok_or_else(|| GameError::MissingTeamHp(team.clone()))?
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
            EffectIntent::RemoveStatus { status_id, owner } => {
                GameEvent::StatusRemoved { status_id, owner }
            }
            EffectIntent::ModifyAction { modification } => match modification {
                ActionModification::PreventDamage
                | ActionModification::CancelSpell
                | ActionModification::SealCoveredPassive => continue,
            },
            EffectIntent::RequestChoice { player, kind } => {
                if let Some(existing_player) = requested_choice_player {
                    return Err(GameError::PendingChoiceInProgress {
                        player: existing_player,
                    });
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
                .ok_or(GameError::MissingPendingChoice)?;
            let value = state
                .card_def(*selected_card)
                .ok_or(GameError::MissingCardInstanceDefinition(*selected_card))?
                .level as i32;

            Ok(vec![EffectIntent::SetShield {
                player: player.clone(),
                value,
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
        DamageTarget::PreviousPlayer => previous_player(state, attacker),
        DamageTarget::DeclaredPlayer | DamageTarget::TeamOfDeclaredPlayer => {
            Err(GameError::RuleImplementation(
                crate::domain::RuleImplementationError::EffectNotImplemented(
                    "declared-attack-target".to_string(),
                ),
            ))
        }
    }
}

fn previous_player(state: &GameState, player: &PlayerId) -> GameResult<PlayerId> {
    let index = state
        .turn_order
        .iter()
        .position(|candidate| candidate == player)
        .ok_or_else(|| GameError::UnknownPlayer(player.clone()))?;
    let previous_index = if index == 0 {
        state.turn_order.len() - 1
    } else {
        index - 1
    };

    Ok(state.turn_order[previous_index].clone())
}

fn player_team(state: &GameState, player: &PlayerId) -> GameResult<TeamId> {
    state
        .players
        .iter()
        .find(|candidate| &candidate.id == player)
        .map(|player| player.team.clone())
        .ok_or_else(|| GameError::UnknownPlayer(player.clone()))
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
                .ok_or(GameError::MissingCardInstanceDefinition(*card))?
                .level as i32;
            Ok(sum + level)
        })
    };

    match formula {
        PointFormula::Fixed(points) => Ok(*points as i32),
        PointFormula::CardCount => Ok(cards.len() as i32),
        PointFormula::FormationPoints => Err(GameError::RuleImplementation(
            crate::domain::RuleImplementationError::EffectNotImplemented(
                "formation-points".to_string(),
            ),
        )),
        PointFormula::LevelPlus(bonus) => Ok(state
            .card_def(cards[0])
            .ok_or(GameError::MissingCardInstanceDefinition(cards[0]))?
            .level as i32
            + *bonus as i32),
        PointFormula::LevelSumTimes(multiplier) => Ok(level_sum()? * *multiplier as i32),
        PointFormula::TargetHandCountTimes(multiplier) => {
            let target_hand = state
                .hand(target)
                .ok_or_else(|| GameError::UnknownPlayer(target.clone()))?;
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
        .ok_or_else(|| GameError::MissingTeamHp(team.clone()))?;
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
        .ok_or_else(|| GameError::MissingTeamHp(team.clone()))?;

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
            player, used_cards, ..
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
            hp_change,
            shield_change,
            card_moves,
            elemental_context_update,
            ..
        } => {
            debug_assert_eq!(state.current_player(), Some(attacker));
            debug_assert_eq!(state.phase, Phase::Main);

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

            state.phase = Phase::TurnDraw;
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

            state.current_turn_index = (state.current_turn_index + 1) % state.turn_order.len();
            state.turn_number += 1;
            state.phase = Phase::TurnStart;
        }
    }
}

fn finish_game_if_needed(state: &mut GameState) {
    if matches!(state.status, GameStatus::Finished { .. }) {
        return;
    }

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
