use crate::application::{GameRecord, RecordedDecision, replay_frame};
use crate::domain::targeting::{RulePlayerTarget, TurnOrderTargets};
use crate::domain::{
    CardDefId, CardInstanceId, CardOrigin, Command, DISCARD_RETRIEVAL_MODULE_ID,
    EffectChoiceAnswer, Element, GameError, GameEvent, GamePreparationStage, GameSetup, GameState,
    GameStatus, PassActionReason, PendingChoiceKind, PendingRandomness, Phase, Player,
    PlayerDeckList, PlayerId, ProfessionId, RuleModuleId, SecretStrategy, SpiritKind, SpiritSkill,
    StarKind, StatusDuration, StatusOwner, TargetDecl, TeamHp, TeamId, TrustedRandomnessAnswer,
    TurnDrawSkipReason,
};
use crate::public_view::{
    PublicCardInterpretation, PublicCardRefs, PublicGameEvent, PublicGameState,
    PublicPendingChoiceKind, PublicPendingChoicePresentation, Viewer,
};
use crate::rules::{
    DeckCompositionCatalog, FormationCategory, OfficialRuleModuleSpec, OfficialRules,
    PlayableAction,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn handle_request_json(input: &str) -> Result<String, String> {
    let request: ApiRequest = serde_json::from_str(input).map_err(|error| error.to_string())?;
    let response = handle(request).map_err(|error| serde_json::to_string(&error).unwrap())?;

    serde_json::to_string(&response).map_err(|error| error.to_string())
}

pub fn rules_catalog_json() -> Result<String, String> {
    let rules = OfficialRules::new();
    serde_json::to_string(&WebRulesCatalog {
        version: 1,
        rule_modules: rules.rule_module_catalog(),
        deck_composition: rules.deck_composition_catalog(),
    })
    .map_err(|error| error.to_string())
}

pub fn resolve_personal_deck_json(input: &str) -> Result<String, String> {
    let request: WebPersonalDeckRequest =
        serde_json::from_str(input).map_err(|error| error.to_string())?;
    let candidate = request.candidate.map(|candidate| PlayerDeckList {
        player: PlayerId::new(&request.player),
        name: candidate.name,
        cards: candidate.cards.into_iter().map(CardDefId::new).collect(),
    });
    let resolved =
        OfficialRules::new().resolve_personal_deck(PlayerId::new(request.player), candidate);

    serde_json::to_string(&resolved).map_err(|error| error.to_string())
}

pub fn resolve_rule_modules_json(input: &str) -> Result<String, String> {
    let request: WebRuleModulesRequest =
        serde_json::from_str(input).map_err(|error| error.to_string())?;
    let candidate = request.candidate.map(|modules| {
        modules
            .into_iter()
            .map(RuleModuleId::new)
            .collect::<Vec<_>>()
    });
    let modules = OfficialRules::new()
        .resolve_rule_modules(candidate)
        .map_err(|error| format!("{error:?}"))?;
    serde_json::to_string(&WebResolvedRuleModules { modules }).map_err(|error| error.to_string())
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebRuleModulesRequest {
    candidate: Option<Vec<String>>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebResolvedRuleModules {
    modules: Vec<RuleModuleId>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebPersonalDeckRequest {
    player: String,
    candidate: Option<WebPersonalDeckCandidate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebPersonalDeckCandidate {
    name: String,
    cards: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebRulesCatalog {
    version: u32,
    rule_modules: Vec<OfficialRuleModuleSpec>,
    deck_composition: DeckCompositionCatalog,
}

#[derive(Clone)]
struct WebCardFact {
    element: Element,
    level: u32,
    secret_strategies: Vec<WebSecretStrategyCardOption>,
}

fn card_facts_for_setup(setup: &GameSetup) -> HashMap<CardInstanceId, WebCardFact> {
    let definitions = setup
        .card_defs
        .iter()
        .map(|definition| (definition.id.clone(), definition))
        .collect::<HashMap<_, _>>();

    setup
        .card_instances
        .iter()
        .filter_map(|instance| {
            definitions.get(&instance.definition).map(|definition| {
                (
                    instance.instance,
                    WebCardFact {
                        element: definition.element,
                        level: definition.level,
                        secret_strategies: crate::rules::pouch::strategy_options_for_card(
                            definition.element,
                            definition.level,
                        )
                        .into_iter()
                        .map(WebSecretStrategyCardOption::from)
                        .collect(),
                    },
                )
            })
        })
        .collect()
}

fn handle(request: ApiRequest) -> Result<ApiResult, ApiError> {
    let rules = OfficialRules::new();
    let setup = setup_for_request(&rules, request.setup, request.first_player.as_deref())?;
    let card_labels = rules.card_labels(&setup).map_err(ApiError::Game)?;
    let card_facts = card_facts_for_setup(&setup);
    let formation_names = rules.formation_names(&setup).map_err(ApiError::Game)?;
    let viewer = viewer_from_request(request.viewer.as_deref());
    let deck_seed = request.deck_seed.clone();
    if let ApiAction::ReplayFrame { step } = request.action {
        let frame = replay_frame(&setup, &request.record.unwrap_or_default(), step)
            .map_err(ApiError::Game)?;
        return replay_response_for(frame, &card_labels, &card_facts, &formation_names);
    }
    let mut record = record_from_request(&rules, &setup, request.record, deck_seed.as_deref())?;

    match request.action {
        ApiAction::ReplayFrame { .. } => unreachable!("replay frames return before record loading"),
        ApiAction::Start => {
            record = GameRecord::start(
                setup.clone(),
                deck_order_for_start(&rules, &setup, deck_seed.as_deref())?,
            )
            .map_err(ApiError::Game)?;
            advance_to_interactive_decision(&mut record)?;
        }
        ApiAction::StartDevelopmentScenario { player, scenario } => {
            let player = PlayerId::new(player);
            record = GameRecord::start(
                setup.clone(),
                development_scenario_deck_order(&rules, &setup, &player, &scenario)?,
            )
            .map_err(ApiError::Game)?;
            if scenario == "pouch-chain-sheep" {
                complete_pouch_chain_development_preparation(&mut record, &player)?;
            } else {
                advance_to_interactive_decision(&mut record)?;
            }
        }
        ApiAction::ChooseInitialPouch { player, card } => {
            let _ = record
                .handle(Command::ChooseInitialPouch {
                    player: PlayerId::new(player),
                    card,
                })
                .map_err(ApiError::Game)?;
        }
        ApiAction::TriggerSecretStrategy {
            player,
            strategy,
            target_player,
            star,
            break_star,
            discard_card,
            deck_cards,
            discard_cards,
        } => {
            let _ = record
                .handle(Command::TriggerSecretStrategy {
                    player: PlayerId::new(player),
                    strategy,
                    target_player: target_player.map(PlayerId::new),
                    star,
                    break_star,
                    discard_card,
                    deck_cards,
                    discard_cards,
                })
                .map_err(ApiError::Game)?;
        }
        ApiAction::Refresh => {}
        ApiAction::AdvanceAutomatic => {
            advance_to_interactive_decision(&mut record)?;
        }
        ApiAction::PassAction => {
            let (current_player, reason) = pass_action_for_state(record.state())
                .ok_or_else(|| ApiError::Message("action pass is not legal".to_string()))?;
            let _ = record
                .handle(Command::PassAction {
                    player: current_player,
                    reason,
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
        ApiAction::PlayableActions { player, cards } => {
            let candidates = record
                .playable_actions(&PlayerId::new(player), &cards)
                .map_err(ApiError::Game)?;
            return response_for(
                &record,
                viewer,
                &card_labels,
                &card_facts,
                &formation_names,
                candidates.into_iter().map(web_playable_action).collect(),
            )
            .map(|response| ApiResult::Ready { response });
        }
        ApiAction::DevelopmentScenarioAction { player, scenario } => {
            let player = PlayerId::new(player);
            let candidate = development_scenario_action(&record, &player, &scenario)?;
            let playable_actions = candidate.into_iter().map(web_playable_action).collect();
            return response_for(
                &record,
                viewer,
                &card_labels,
                &card_facts,
                &formation_names,
                playable_actions,
            )
            .map(|response| ApiResult::Ready { response });
        }
        ApiAction::PrepareDevelopmentScenario { player, scenario } => {
            prepare_development_scenario(&mut record, &PlayerId::new(player), &scenario)?;
        }
        ApiAction::TrustedRandomHandCandidates {
            player,
            candidate_action,
        } => {
            let Some(selection_count) = trusted_random_hand_selection_count(&candidate_action)
            else {
                return response_for(
                    &record,
                    viewer,
                    &card_labels,
                    &card_facts,
                    &formation_names,
                    Vec::new(),
                )
                .map(|response| ApiResult::Ready { response });
            };
            let player = PlayerId::new(player);
            if record.state().current_player() != Some(&player) {
                return Err(ApiError::Game(GameError::Validation(
                    crate::domain::ValidationError::WrongPlayer {
                        expected: record
                            .state()
                            .current_player()
                            .cloned()
                            .unwrap_or_else(|| player.clone()),
                        actual: player,
                    },
                )));
            }
            let target = TurnOrderTargets::new(record.state())
                .player_target(&player, RulePlayerTarget::NextPlayer)
                .map_err(ApiError::Game)?;
            let candidates = record
                .state()
                .hand(&target)
                .ok_or_else(|| {
                    ApiError::Game(GameError::Validation(
                        crate::domain::ValidationError::UnknownPlayer(target.clone()),
                    ))
                })?
                .to_vec();
            let mut response = response_for(
                &record,
                viewer,
                &card_labels,
                &card_facts,
                &formation_names,
                Vec::new(),
            )?;
            response.trusted_random_candidates = Some(candidates);
            response.trusted_random_candidate_count = Some(selection_count);
            return Ok(ApiResult::Ready { response });
        }
        ApiAction::PerformFormation {
            player,
            formation_id,
            cards,
            star_substitution_card,
            match_option_role,
            match_option_card,
            match_option_slots,
            trusted_random_cards,
        } => {
            let mut declared_targets = star_substitution_card
                .map(TargetDecl::Card)
                .into_iter()
                .collect::<Vec<_>>();
            if let (Some(role), Some(card)) = (match_option_role, match_option_card) {
                if role == "card-multiplicity" {
                    declared_targets.push(TargetDecl::CardMultiplicity {
                        card,
                        slots: match_option_slots.unwrap_or(2),
                    });
                } else {
                    declared_targets.push(TargetDecl::FormationRole { role, card });
                }
            }
            let command = if let Some(random_cards) = trusted_random_cards {
                Command::PerformFormationWithTrustedRandomness {
                    player: PlayerId::new(player),
                    formation_id,
                    cards,
                    declared_targets,
                    random_cards,
                }
            } else {
                Command::PerformFormation {
                    player: PlayerId::new(player),
                    formation_id,
                    cards,
                    declared_targets,
                }
            };
            let _ = record.handle(command).map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
        ApiAction::ChangeProfession {
            player,
            profession_id,
            cards,
        } => {
            let _ = record
                .handle(Command::ChangeProfession {
                    player: PlayerId::new(player),
                    profession: ProfessionId::new(profession_id),
                    cards,
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
        ApiAction::ActivateProfessionAbility {
            player,
            ability_id,
            cards,
            target_card,
            declared_element,
            declared_level,
        } => {
            let _ = record
                .handle(Command::ActivateProfessionAbility {
                    player: PlayerId::new(player),
                    ability_id,
                    cards,
                    target_card,
                    declared_element,
                    declared_level,
                })
                .map_err(ApiError::Game)?;
        }
        ApiAction::UseSpiritSkill {
            player,
            skill,
            selected_card,
            declared_level,
            trusted_random_cards,
        } => {
            let command = if let Some(random_cards) = trusted_random_cards {
                Command::UseSpiritSkillWithTrustedRandomness {
                    player: PlayerId::new(player),
                    skill,
                    selected_card,
                    declared_level,
                    random_cards,
                }
            } else {
                Command::UseSpiritSkill {
                    player: PlayerId::new(player),
                    skill,
                    selected_card,
                    declared_level,
                }
            };
            let _ = record.handle(command).map_err(ApiError::Game)?;
        }
        ApiAction::ChooseTurnDiscard { player, card } => {
            let _ = record
                .handle(Command::ChooseTurnDiscard {
                    player: PlayerId::new(player),
                    discard: card,
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
        ApiAction::AnswerEffectChoice { player, cards } => {
            let _ = record
                .handle(Command::AnswerEffectChoice {
                    player: PlayerId::new(player),
                    selected_cards: cards,
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
        ApiAction::AnswerEffectChoiceTyped { player, answer } => {
            let _ = record
                .handle(Command::AnswerEffectChoiceTyped {
                    player: PlayerId::new(player),
                    answer,
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
        ApiAction::ResolveRandomness {
            request_id,
            shuffled_order,
        } => {
            let _ = record
                .resolve_randomness(TrustedRandomnessAnswer {
                    request_id,
                    shuffled_order,
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
        ApiAction::RetrievePreviousTurnDiscard { player } => {
            let _ = record
                .handle(Command::RetrievePreviousTurnDiscard {
                    player: PlayerId::new(player),
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
    }

    if let Some(request) = record.state().pending_randomness.clone() {
        return Ok(ApiResult::NeedsRandomness {
            record: record.recorded_decisions(),
            request,
        });
    }

    response_for(
        &record,
        viewer,
        &card_labels,
        &card_facts,
        &formation_names,
        Vec::new(),
    )
    .map(|response| ApiResult::Ready { response })
}

fn advance_after_command(record: &mut GameRecord) -> Result<(), ApiError> {
    advance_to_interactive_decision(record)?;
    Ok(())
}

fn advance_to_interactive_decision(record: &mut GameRecord) -> Result<(), ApiError> {
    loop {
        let _ = record.advance_until_decision().map_err(ApiError::Game)?;

        let Some((player, reason)) = pass_action_for_state(record.state()) else {
            return Ok(());
        };
        if can_retrieve_discard(record.state()) {
            return Ok(());
        }

        let _ = record
            .handle(Command::PassAction { player, reason })
            .map_err(ApiError::Game)?;
    }
}

fn pass_action_for_state(state: &crate::domain::GameState) -> Option<(PlayerId, PassActionReason)> {
    if state.phase != Phase::Main || state.pending_choice.is_some() {
        return None;
    }

    let player = state.current_player()?.clone();
    let hand = state.hand(&player)?;

    if hand.is_empty() {
        return Some((player, PassActionReason::NoCardsInHand));
    }

    let cannot_act = state.statuses.iter().any(|status| {
        status.kind == "CannotAct"
            && matches!(&status.owner, StatusOwner::Player(owner) if owner == &player)
    });

    cannot_act.then_some((player, PassActionReason::CannotActByStatus))
}

fn record_from_request(
    rules: &OfficialRules,
    setup: &GameSetup,
    record: Option<Vec<RecordedDecision>>,
    deck_seed: Option<&str>,
) -> Result<GameRecord, ApiError> {
    match record {
        Some(recorded_decisions) if !recorded_decisions.is_empty() => {
            GameRecord::from_recorded_decisions(setup.clone(), recorded_decisions)
                .map_err(|error| ApiError::Message(format!("{error:?}")))
        }
        _ => GameRecord::start(
            setup.clone(),
            deck_order_for_start(rules, setup, deck_seed)?,
        )
        .map_err(ApiError::Game),
    }
}

fn response_for(
    record: &GameRecord,
    viewer: Viewer,
    card_labels: &HashMap<CardInstanceId, String>,
    card_facts: &HashMap<CardInstanceId, WebCardFact>,
    formation_names: &HashMap<String, String>,
    mut playable_actions: Vec<WebPlayableAction>,
) -> Result<ApiResponse, ApiError> {
    let vocabulary = PlayerVocabulary::for_modules(&record.state().enabled_rule_modules);
    let viewer_player = match &viewer {
        Viewer::Player(player) => Some(player.clone()),
        Viewer::Observer | Viewer::Replay => None,
    };
    let can_pass = pass_action_for_state(record.state()).is_some();
    let can_retrieve_discard = can_retrieve_discard(record.state());
    let discard_retrieval_action =
        discard_retrieval_action(record.state(), card_labels, card_facts);
    if let Viewer::Player(player) = &viewer
        && record.state().current_player() == Some(player)
        && record.state().phase == Phase::Main
        && record.state().pending_choice.is_none()
    {
        for action in record
            .playable_actions(player, &[])
            .map_err(ApiError::Game)?
        {
            if matches!(
                action,
                PlayableAction::ActivateProfessionAbility(_) | PlayableAction::UseSpiritSkill(_)
            ) {
                let action = web_playable_action(action);
                if !playable_actions.contains(&action) {
                    playable_actions.push(action);
                }
            }
        }
    }
    let secret_strategy_actions = viewer_player
        .as_ref()
        .map(|player| secret_strategy_actions_for(record.state(), player))
        .unwrap_or_default();
    Ok(ApiResponse {
        record: record.recorded_decisions(),
        state: WebPublicGameState::from_public(
            record.public_view(viewer.clone()).map_err(ApiError::Game)?,
            card_labels,
            card_facts,
            formation_names,
        ),
        events: record
            .public_events_for(viewer)
            .into_iter()
            .enumerate()
            .filter(|(_, event)| {
                !matches!(
                    event,
                    PublicGameEvent::DeckPrepared { .. }
                        | PublicGameEvent::PlayerDeckPrepared { .. }
                        | PublicGameEvent::CardsDealt { .. }
                )
            })
            .rev()
            .map(|(index, event)| {
                WebPublicGameEvent::from_public(
                    index + 1,
                    event,
                    card_labels,
                    formation_names,
                    &vocabulary,
                )
            })
            .collect(),
        playable_actions,
        interaction: WebInteraction {
            can_pass,
            has_optional_effect: can_retrieve_discard,
            can_retrieve_discard,
            discard_retrieval_action,
            can_choose_initial_pouch: viewer_player.as_ref().is_some_and(|player| {
                matches!(
                    &record.state().status,
                    crate::domain::GameStatus::Preparing {
                        stage:
                            crate::domain::GamePreparationStage::InitialPouchSelection {
                                player: expected,
                            },
                    } if expected == player
                )
            }),
            can_trigger_pouch: viewer_player.as_ref().is_some_and(|player| {
                matches!(record.state().status, crate::domain::GameStatus::InProgress)
                    && record.state().current_player() == Some(player)
                    && record.state().phase == Phase::Main
                    && record.state().pouch_for(player).is_some()
            }),
            secret_strategy_actions,
        },
        trusted_random_candidates: None,
        trusted_random_candidate_count: None,
    })
}

fn secret_strategy_actions_for(
    state: &GameState,
    player: &PlayerId,
) -> Vec<WebSecretStrategyActionOption> {
    let mut source_cards = match state.pending_choice.as_ref() {
        Some(choice)
            if &choice.player == player
                && matches!(
                    &choice.kind,
                    PendingChoiceKind::TypedEffect { effect_id, .. }
                        if effect_id == crate::rules::pouch::CHAIN_ID
                ) =>
        {
            match &choice.kind {
                PendingChoiceKind::TypedEffect { options, .. } => options
                    .cards
                    .as_ref()
                    .map(|cards| cards.allowed_cards.clone())
                    .unwrap_or_default(),
                _ => unreachable!("matched typed Chain choice"),
            }
        }
        _ if state.current_player() == Some(player) && state.phase == Phase::Main => {
            let mut cards = state.deck_for(player).unwrap_or_default().to_vec();
            if let Some(pouch) = state.pouch_for(player) {
                cards.push(pouch.card);
            }
            cards
        }
        _ => Vec::new(),
    };
    source_cards.sort();
    source_cards.dedup();
    source_cards
        .into_iter()
        .flat_map(|card| crate::rules::pouch::strategy_action_options(state, player, card))
        .map(WebSecretStrategyActionOption::from)
        .collect()
}

fn replay_response_for(
    frame: crate::application::ReplayFrame,
    card_labels: &HashMap<CardInstanceId, String>,
    card_facts: &HashMap<CardInstanceId, WebCardFact>,
    formation_names: &HashMap<String, String>,
) -> Result<ApiResult, ApiError> {
    let vocabulary = PlayerVocabulary::for_modules(&frame.state.enabled_rule_modules);
    let state = WebPublicGameState::from_public(
        crate::public_view::state_for(&frame.state, Viewer::Replay),
        card_labels,
        card_facts,
        formation_names,
    );
    let events = crate::public_view::events_for(&frame.events, Viewer::Replay)
        .into_iter()
        .enumerate()
        .filter(|(_, event)| {
            !matches!(
                event,
                PublicGameEvent::DeckPrepared { .. } | PublicGameEvent::PlayerDeckPrepared { .. }
            )
        })
        .map(|(index, event)| {
            WebPublicGameEvent::from_public(
                index + 1,
                event,
                card_labels,
                formation_names,
                &vocabulary,
            )
        })
        .collect();

    Ok(ApiResult::ReplayFrame {
        current_step: frame.step,
        total_steps: frame.total_steps,
        state,
        events,
        interaction: WebInteraction::disabled(),
    })
}

#[derive(Serialize, Deserialize)]
struct ApiRequest {
    action: ApiAction,
    viewer: Option<String>,
    record: Option<Vec<RecordedDecision>>,
    #[serde(default)]
    setup: Option<WebGameSetup>,
    #[serde(default, rename = "firstPlayer")]
    first_player: Option<String>,
    #[serde(default, rename = "deckSeed")]
    deck_seed: Option<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebGameSetup {
    players: Vec<WebSetupPlayer>,
    turn_order: Vec<String>,
    #[serde(default)]
    enabled_rule_modules: Vec<String>,
    #[serde(default)]
    deck_lists: Vec<WebSetupDeckList>,
    #[serde(default)]
    initial_hp: Vec<WebSetupTeamHp>,
}

#[derive(Serialize, Deserialize)]
struct WebSetupPlayer {
    id: String,
    team: String,
}

#[derive(Serialize, Deserialize)]
struct WebSetupTeamHp {
    team: String,
    hp: i32,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct WebSetupDeckList {
    player: String,
    name: String,
    cards: Vec<String>,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
#[serde(deny_unknown_fields)]
enum ApiAction {
    ReplayFrame {
        step: usize,
    },
    Start,
    StartDevelopmentScenario {
        player: String,
        scenario: String,
    },
    Refresh,
    AdvanceAutomatic,
    ChooseInitialPouch {
        player: String,
        card: CardInstanceId,
    },
    TriggerSecretStrategy {
        player: String,
        strategy: SecretStrategy,
        #[serde(default, rename = "targetPlayer")]
        target_player: Option<String>,
        #[serde(default)]
        star: Option<StarKind>,
        #[serde(default, rename = "breakStar")]
        break_star: bool,
        #[serde(default, rename = "discardCard")]
        discard_card: Option<CardInstanceId>,
        #[serde(default, rename = "deckCards")]
        deck_cards: Vec<CardInstanceId>,
        #[serde(default, rename = "discardCards")]
        discard_cards: Vec<CardInstanceId>,
    },
    PassAction,
    PlayableActions {
        player: String,
        cards: Vec<CardInstanceId>,
    },
    DevelopmentScenarioAction {
        player: String,
        scenario: String,
    },
    PrepareDevelopmentScenario {
        player: String,
        scenario: String,
    },
    TrustedRandomHandCandidates {
        player: String,
        #[serde(rename = "candidateAction")]
        candidate_action: Box<ApiAction>,
    },
    PerformFormation {
        player: String,
        #[serde(rename = "formationId")]
        formation_id: String,
        cards: Vec<CardInstanceId>,
        #[serde(default, rename = "starSubstitutionCard")]
        star_substitution_card: Option<CardInstanceId>,
        #[serde(default, rename = "matchOptionRole")]
        match_option_role: Option<String>,
        #[serde(default, rename = "matchOptionCard")]
        match_option_card: Option<CardInstanceId>,
        #[serde(default, rename = "matchOptionSlots")]
        match_option_slots: Option<usize>,
        #[serde(default, rename = "trustedRandomCards")]
        trusted_random_cards: Option<Vec<CardInstanceId>>,
    },
    ChangeProfession {
        player: String,
        #[serde(rename = "professionId")]
        profession_id: String,
        cards: Vec<CardInstanceId>,
    },
    ActivateProfessionAbility {
        player: String,
        #[serde(rename = "abilityId")]
        ability_id: String,
        cards: Vec<CardInstanceId>,
        #[serde(default, rename = "targetCard")]
        target_card: Option<CardInstanceId>,
        #[serde(default, rename = "declaredElement")]
        declared_element: Option<crate::domain::Element>,
        #[serde(default, rename = "declaredLevel")]
        declared_level: Option<u32>,
    },
    UseSpiritSkill {
        player: String,
        skill: crate::domain::SpiritSkill,
        #[serde(default, rename = "selectedCard")]
        selected_card: Option<CardInstanceId>,
        #[serde(default, rename = "declaredLevel")]
        declared_level: Option<u32>,
        #[serde(default, rename = "trustedRandomCards")]
        trusted_random_cards: Option<Vec<CardInstanceId>>,
    },
    ChooseTurnDiscard {
        player: String,
        card: CardInstanceId,
    },
    AnswerEffectChoice {
        player: String,
        cards: Vec<CardInstanceId>,
    },
    AnswerEffectChoiceTyped {
        player: String,
        answer: EffectChoiceAnswer,
    },
    ResolveRandomness {
        #[serde(rename = "requestId")]
        request_id: String,
        #[serde(rename = "shuffledOrder")]
        shuffled_order: Vec<CardInstanceId>,
    },
    RetrievePreviousTurnDiscard {
        player: String,
    },
}

fn development_scenario_action(
    record: &GameRecord,
    player: &PlayerId,
    scenario: &str,
) -> Result<Option<PlayableAction>, ApiError> {
    let hand = record
        .state()
        .hand(player)
        .ok_or_else(|| ApiError::Message("development scenario player has no hand".to_string()))?;
    if matches!(
        scenario,
        "tribulation-earth-rending" | "tribulation-rusted-forest"
    ) {
        let (first_element, second_element, formation_id) = match scenario {
            "tribulation-earth-rending" => (
                Element::Earth,
                Element::Wood,
                crate::rules::tribulation::EARTH_RENDING,
            ),
            "tribulation-rusted-forest" => (
                Element::Wood,
                Element::Metal,
                crate::rules::tribulation::RUSTED_FOREST,
            ),
            _ => unreachable!(),
        };
        let cards = [4, 5]
            .into_iter()
            .flat_map(|size| card_combinations(hand, size))
            .find(|cards| {
                let facts = cards
                    .iter()
                    .filter_map(|card| {
                        Some(crate::rules::SubmittedCardFacts {
                            element: record.state().card_def(*card)?.element,
                            level: record.state().card_level_for(player, *card)?,
                        })
                    })
                    .collect::<Vec<_>>();
                facts.len() == cards.len()
                    && crate::rules::tribulation::matches_elements(
                        &facts,
                        first_element,
                        second_element,
                    )
            });
        let Some(cards) = cards else {
            return Ok(None);
        };
        return record
            .playable_actions(player, &cards)
            .map_err(ApiError::Game)
            .map(|actions| {
                actions.into_iter().find(|action| {
                    matches!(
                        action,
                        PlayableAction::PerformFormation(candidate)
                            if candidate.formation_id == formation_id
                    )
                })
            });
    }
    let candidate_sizes: &[usize] = match scenario {
        "hero-schools-transition" => &[1],
        "spirit-metal" | "spirit-fire" | "echo-pure-fire" | "echo-split-earth" => &[2],
        "pouch-chain-sheep" => &[3],
        _ => {
            return Err(ApiError::Message(
                "unknown development scenario".to_string(),
            ));
        }
    };
    for &size in candidate_sizes {
        for cards in card_combinations(hand, size) {
            let actions = record
                .playable_actions(player, &cards)
                .map_err(ApiError::Game)?;
            if let Some(action) = actions.into_iter().find(|action| match (scenario, action) {
                ("hero-schools-transition", PlayableAction::ChangeProfession(candidate)) => {
                    candidate.profession_id.as_str() == "mesmer"
                }
                ("spirit-metal", PlayableAction::PerformFormation(candidate)) => {
                    candidate.formation_id == "metal-spirit-summoning"
                }
                ("spirit-fire", PlayableAction::PerformFormation(candidate)) => {
                    candidate.formation_id == "fire-spirit-summoning"
                }
                ("echo-pure-fire", PlayableAction::PerformFormation(candidate)) => {
                    candidate.formation_id == crate::rules::echo::PURE_FIRE
                }
                ("echo-split-earth", PlayableAction::PerformFormation(candidate)) => {
                    candidate.formation_id == crate::rules::echo::SPLIT_EARTH
                }
                ("pouch-chain-sheep", PlayableAction::PerformFormation(candidate)) => {
                    candidate.formation_id == crate::rules::pouch::CHAIN_ID
                }
                _ => false,
            }) {
                return Ok(Some(action));
            }
        }
    }
    Ok(None)
}

fn prepare_development_scenario(
    record: &mut GameRecord,
    player: &PlayerId,
    scenario: &str,
) -> Result<(), ApiError> {
    let skill = match scenario {
        "spirit-metal" => SpiritSkill::FlyingBlade,
        "spirit-fire" => SpiritSkill::Splendor,
        _ => {
            return Err(ApiError::Message(
                "development scenario does not need preparation".to_string(),
            ));
        }
    };
    let desired_power = crate::rules::spirit::skill_cost(skill);

    for _ in 0..80 {
        if record.state().current_player() == Some(player)
            && record.state().phase == Phase::Main
            && development_skill_is_playable(record, player, skill)?
        {
            return Ok(());
        }

        if let Some(choice) = record.state().pending_choice.clone() {
            let PendingChoiceKind::TurnDrawDiscard {
                allowed_discards, ..
            } = choice.kind
            else {
                return Err(ApiError::Message(
                    "development scenario cannot answer pending choice".to_string(),
                ));
            };
            let discard = allowed_discards
                .iter()
                .min_by_key(|discard| {
                    let mut candidate = record.clone();
                    if candidate
                        .handle(Command::ChooseTurnDiscard {
                            player: choice.player.clone(),
                            discard: **discard,
                        })
                        .is_err()
                    {
                        return u32::MAX;
                    }
                    candidate
                        .state()
                        .spirit_for(player)
                        .map_or(u32::MAX, |owned| owned.power.abs_diff(desired_power))
                })
                .copied()
                .ok_or_else(|| {
                    ApiError::Message(
                        "development scenario discard choice has no Cards".to_string(),
                    )
                })?;
            record
                .handle(Command::ChooseTurnDiscard {
                    player: choice.player,
                    discard,
                })
                .map_err(ApiError::Game)?;
            advance_after_command(record)?;
            continue;
        }

        if record.state().phase != Phase::Main || record.state().current_player().is_none() {
            advance_to_interactive_decision(record)?;
            continue;
        }

        let current =
            record.state().current_player().cloned().ok_or_else(|| {
                ApiError::Message("development scenario has no player".to_string())
            })?;
        let hand = record
            .state()
            .hand(&current)
            .ok_or_else(|| {
                ApiError::Message("development scenario player has no hand".to_string())
            })?
            .to_vec();
        let formation = hand.iter().find_map(|card| {
            record
                .playable_actions(&current, &[*card])
                .ok()?
                .into_iter()
                .find_map(|action| match action {
                    PlayableAction::PerformFormation(candidate) => Some(candidate),
                    _ => None,
                })
        });
        let Some(formation) = formation else {
            return Err(ApiError::Message(
                "development scenario player has no one-Card Formation".to_string(),
            ));
        };
        record
            .handle(Command::PerformFormation {
                player: current,
                formation_id: formation.formation_id,
                cards: formation.cards,
                declared_targets: formation.declared_targets,
            })
            .map_err(ApiError::Game)?;
        advance_after_command(record)?;
    }

    Err(ApiError::Message(
        "development scenario could not reach the requested Spirit Skill".to_string(),
    ))
}

fn development_skill_is_playable(
    record: &GameRecord,
    player: &PlayerId,
    skill: SpiritSkill,
) -> Result<bool, ApiError> {
    let mut selections = vec![Vec::new()];
    selections.extend(
        record
            .state()
            .hand(player)
            .into_iter()
            .flatten()
            .map(|card| vec![*card]),
    );
    for cards in selections {
        if record
            .playable_actions(player, &cards)
            .map_err(ApiError::Game)?
            .into_iter()
            .any(|action| {
                matches!(action, PlayableAction::UseSpiritSkill(candidate) if candidate.skill == skill)
            })
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn development_scenario_deck_order(
    rules: &OfficialRules,
    setup: &GameSetup,
    player: &PlayerId,
    scenario: &str,
) -> Result<Vec<CardInstanceId>, ApiError> {
    let deck_order = rules.official_deck_order(setup).map_err(ApiError::Game)?;
    let uses_personal_decks = setup.has_rule_module(crate::domain::PERSONAL_DECK_MODULE_ID);
    let pool = deck_order
        .iter()
        .copied()
        .filter(|card| {
            if !uses_personal_decks {
                return true;
            }
            setup
                .card_instances
                .iter()
                .find(|instance| instance.instance == *card)
                .is_some_and(
                    |instance| matches!(&instance.origin, CardOrigin::Player(owner) if owner == player),
                )
        })
        .filter(|card| {
            let element = setup.card_instances.iter().find_map(|instance| {
                (instance.instance == *card).then(|| {
                    setup
                        .card_defs
                        .iter()
                        .find(|definition| definition.id == instance.definition)
                        .map(|definition| definition.element)
                })?
            });
            match (scenario, element) {
                ("hero-schools-transition", Some(Element::Water)) => true,
                ("spirit-metal", Some(element)) => {
                    element == crate::rules::spirit::element(SpiritKind::Metal)
                }
                ("spirit-fire", Some(element)) => {
                    element == crate::rules::spirit::element(SpiritKind::Fire)
                }
                ("echo-pure-fire", Some(Element::Fire | Element::Water)) => true,
                ("echo-split-earth", Some(Element::Earth)) => true,
                ("tribulation-earth-rending", Some(Element::Earth | Element::Wood)) => true,
                ("tribulation-rusted-forest", Some(Element::Wood | Element::Metal)) => true,
                ("pouch-chain-sheep", Some(_)) => true,
                _ => false,
            }
        })
        .collect::<Vec<_>>();
    let candidate_sizes: &[usize] = match scenario {
        "hero-schools-transition" => &[1],
        "spirit-metal" | "spirit-fire" | "echo-pure-fire" | "echo-split-earth" => &[2],
        "tribulation-earth-rending" | "tribulation-rusted-forest" => &[4, 5],
        "pouch-chain-sheep" => &[3],
        _ => {
            return Err(ApiError::Message(
                "unknown development scenario".to_string(),
            ));
        }
    };
    let selected = candidate_sizes
        .iter()
        .flat_map(|size| card_combinations(&pool, *size))
        .find(|cards| {
            let facts = cards
                .iter()
                .filter_map(|card| {
                    let instance = setup
                        .card_instances
                        .iter()
                        .find(|instance| instance.instance == *card)?;
                    let definition = setup
                        .card_defs
                        .iter()
                        .find(|definition| definition.id == instance.definition)?;
                    Some(crate::rules::SubmittedCardFacts {
                        element: definition.element,
                        level: definition.level,
                    })
                })
                .collect::<Vec<_>>();
            if facts.len() != cards.len() {
                return false;
            }
            match scenario {
                "hero-schools-transition" => crate::rules::hero::matches_initial_profession(
                    crate::rules::hero::MESMER_ID,
                    &facts,
                ),
                "spirit-metal" => facts
                    .iter()
                    .all(|card| card.element == crate::rules::spirit::element(SpiritKind::Metal)),
                "spirit-fire" => facts
                    .iter()
                    .all(|card| card.element == crate::rules::spirit::element(SpiritKind::Fire)),
                "echo-pure-fire" => crate::rules::echo::matches_pure_fire(&facts),
                "echo-split-earth" => {
                    facts.len() == 2 && facts.iter().all(|card| card.element == Element::Earth)
                }
                "tribulation-earth-rending" => crate::rules::tribulation::matches_elements(
                    &facts,
                    Element::Earth,
                    Element::Wood,
                ),
                "tribulation-rusted-forest" => crate::rules::tribulation::matches_elements(
                    &facts,
                    Element::Wood,
                    Element::Metal,
                ),
                "pouch-chain-sheep" => crate::rules::pouch::matches_chain(&facts),
                _ => false,
            }
        })
        .ok_or_else(|| {
            ApiError::Message(format!(
                "development scenario has no deterministic starting hand: {scenario}"
            ))
        })?;

    if scenario == "pouch-chain-sheep" {
        let initial_pouch = deck_order
            .iter()
            .copied()
            .find(|card| {
                !selected.contains(card)
                    && setup.card_instances.iter().any(|instance| {
                        instance.instance == *card
                            && matches!(
                                &instance.origin,
                                CardOrigin::Player(owner) if owner == player
                            )
                    })
            })
            .ok_or_else(|| {
                ApiError::Message("Pouch scenario needs an initial Pouch Card".to_string())
            })?;
        let head = std::iter::once(initial_pouch)
            .chain(selected.iter().copied())
            .collect::<Vec<_>>();
        return Ok(head
            .iter()
            .copied()
            .chain(deck_order.into_iter().filter(|card| !head.contains(card)))
            .collect());
    }

    let mut remaining = deck_order
        .into_iter()
        .filter(|card| !selected.contains(card))
        .collect::<Vec<_>>();
    match scenario {
        "spirit-metal" => remaining.sort_by_key(|card| {
            setup_card_element(setup, *card)
                == Some(crate::rules::spirit::element(SpiritKind::Metal))
        }),
        "spirit-fire" => remaining.sort_by_key(|card| {
            setup_card_element(setup, *card)
                != Some(crate::rules::spirit::element(SpiritKind::Fire))
        }),
        _ => {}
    }

    Ok(selected.iter().copied().chain(remaining).collect())
}

fn complete_pouch_chain_development_preparation(
    record: &mut GameRecord,
    scenario_player: &PlayerId,
) -> Result<(), ApiError> {
    while let GameStatus::Preparing {
        stage: GamePreparationStage::InitialPouchSelection { player },
    } = record.state().status.clone()
    {
        let card = record
            .state()
            .deck_for(&player)
            .and_then(|deck| deck.first().copied())
            .ok_or_else(|| {
                ApiError::Message("Pouch scenario has no initial Pouch Card".to_string())
            })?;
        record
            .handle(Command::ChooseInitialPouch { player, card })
            .map_err(ApiError::Game)?;
    }
    while let Some(request) = record.state().pending_randomness.clone() {
        let shuffled_order = match &request.operation {
            crate::domain::RandomnessOperation::DeckShuffle {
                deck: crate::domain::RandomnessDeck::Player(player),
            } if player == scenario_player => {
                let chain_cards = card_combinations(&request.current_order, 3)
                    .into_iter()
                    .find(|cards| {
                        let facts = cards
                            .iter()
                            .filter_map(|card| {
                                record.state().card_def(*card).map(|definition| {
                                    crate::rules::SubmittedCardFacts {
                                        element: definition.element,
                                        level: definition.level,
                                    }
                                })
                            })
                            .collect::<Vec<_>>();
                        facts.len() == cards.len() && crate::rules::pouch::matches_chain(&facts)
                    })
                    .ok_or_else(|| {
                        ApiError::Message("Pouch scenario has no Chain Cards".to_string())
                    })?;
                let remaining = request
                    .current_order
                    .iter()
                    .copied()
                    .filter(|card| !chain_cards.contains(card))
                    .collect::<Vec<_>>();
                let trigger = remaining
                    .iter()
                    .copied()
                    .find(|card| {
                        record
                            .state()
                            .card_def(*card)
                            .is_some_and(|definition| definition.level == 2)
                    })
                    .ok_or_else(|| {
                        ApiError::Message("Pouch scenario has no Sheep trigger Card".to_string())
                    })?;
                let filler = remaining
                    .iter()
                    .copied()
                    .find(|card| *card != trigger)
                    .expect("a Personal Deck has more than one non-Chain Card");
                chain_cards
                    .iter()
                    .copied()
                    .chain(std::iter::once(filler))
                    .chain(std::iter::once(trigger))
                    .chain(
                        remaining
                            .into_iter()
                            .filter(|card| *card != filler && *card != trigger),
                    )
                    .collect()
            }
            _ => request.current_order.clone(),
        };
        record
            .resolve_randomness(TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order,
            })
            .map_err(ApiError::Game)?;
    }
    advance_to_interactive_decision(record)?;
    if let Some(crate::domain::PendingChoice {
        player,
        kind: PendingChoiceKind::TurnDrawDiscard {
            allowed_discards, ..
        },
    }) = record.state().pending_choice.clone()
    {
        let discard = allowed_discards.first().copied().ok_or_else(|| {
            ApiError::Message("Pouch scenario has no Turn Draw discard".to_string())
        })?;
        record
            .handle(Command::ChooseTurnDiscard { player, discard })
            .map_err(ApiError::Game)?;
        advance_to_interactive_decision(record)?;
    }
    Ok(())
}

fn setup_card_element(setup: &GameSetup, card: CardInstanceId) -> Option<Element> {
    let instance = setup
        .card_instances
        .iter()
        .find(|instance| instance.instance == card)?;
    setup
        .card_defs
        .iter()
        .find(|definition| definition.id == instance.definition)
        .map(|definition| definition.element)
}

fn card_combinations(cards: &[CardInstanceId], size: usize) -> Vec<Vec<CardInstanceId>> {
    if size == 0 {
        return vec![Vec::new()];
    }
    cards
        .iter()
        .enumerate()
        .flat_map(|(index, card)| {
            card_combinations(&cards[index + 1..], size - 1)
                .into_iter()
                .map(move |rest| std::iter::once(*card).chain(rest).collect())
        })
        .collect()
}

fn web_playable_action(candidate: PlayableAction) -> WebPlayableAction {
    match candidate {
        PlayableAction::PerformFormation(candidate) => WebPlayableAction::PerformFormation {
            id: candidate.formation_id.clone(),
            name: candidate.formation_name,
            category: WebFormationCategory::from(candidate.category),
            policy: WebFormationActionPolicy::from_id(&candidate.formation_id),
            summary: candidate.summary,
            cards: candidate.cards,
            star_substitution: candidate
                .star_substitution
                .map(WebStarElementSubstitution::from),
            match_option: candidate
                .declared_targets
                .iter()
                .find_map(|target| match target {
                    TargetDecl::FormationRole { role, card } => Some(WebFormationMatchOption {
                        role: role.clone(),
                        card: *card,
                        slots: 1,
                        preview: candidate.preview.clone(),
                    }),
                    TargetDecl::CardMultiplicity { card, slots } => Some(WebFormationMatchOption {
                        role: "card-multiplicity".to_string(),
                        card: *card,
                        slots: *slots,
                        preview: candidate.preview.clone(),
                    }),
                    _ => None,
                }),
        },
        PlayableAction::ChangeProfession(candidate) => WebPlayableAction::ChangeProfession {
            id: candidate.profession_id.as_str().to_string(),
            name: candidate.profession_name,
            summary: candidate.rule_text,
            cards: candidate.cards,
        },
        PlayableAction::ActivateProfessionAbility(candidate) => {
            WebPlayableAction::ActivateProfessionAbility {
                id: candidate.ability_id,
                name: candidate.ability_name,
                summary: candidate.rule_text,
                cards: candidate.cards,
                target_card: candidate.target_card,
                declared_element: candidate
                    .declared_element
                    .map(|element| format!("{element:?}")),
                declared_level: candidate.declared_level,
            }
        }
        PlayableAction::UseSpiritSkill(candidate) => WebPlayableAction::UseSpiritSkill {
            id: format!("{:?}", candidate.skill),
            name: candidate.skill_name,
            summary: candidate.rule_text,
            cards: candidate.selected_card.into_iter().collect(),
            selected_card: candidate.selected_card,
            declared_level: candidate.declared_level,
        },
    }
}

fn trusted_random_hand_selection_count(action: &ApiAction) -> Option<usize> {
    match action {
        ApiAction::PerformFormation { formation_id, .. } => {
            crate::rules::randomness::trusted_random_hand_count_for_formation(formation_id)
        }
        ApiAction::UseSpiritSkill { skill, .. } => {
            crate::rules::randomness::trusted_random_hand_count_for_spirit_skill(*skill)
        }
        _ => None,
    }
}

fn can_retrieve_discard(state: &crate::domain::GameState) -> bool {
    if state.phase != Phase::Main
        || state.pending_choice.is_some()
        || !state.has_rule_module(DISCARD_RETRIEVAL_MODULE_ID)
    {
        return false;
    }

    let Some(player) = state.current_player() else {
        return false;
    };
    let Some(index) = state
        .turn_order
        .iter()
        .position(|candidate| candidate == player)
    else {
        return false;
    };
    let previous_index = if index == 0 {
        state.turn_order.len().saturating_sub(1)
    } else {
        index - 1
    };
    let Some(previous_player) = state.turn_order.get(previous_index) else {
        return false;
    };
    state
        .last_turn_discard_by_player
        .get(previous_player)
        .filter(|discard| discard.turn_number + 1 == state.turn_number)
        .is_some_and(|turn_discard| {
            state
                .discard_for(previous_player)
                .is_some_and(|discard| discard.contains(&turn_discard.card))
        })
}

fn discard_retrieval_action(
    state: &crate::domain::GameState,
    labels: &HashMap<CardInstanceId, String>,
    card_facts: &HashMap<CardInstanceId, WebCardFact>,
) -> Option<WebDiscardRetrievalActionDetail> {
    if !can_retrieve_discard(state) {
        return None;
    }
    let player = state.current_player()?;
    let player_index = state
        .turn_order
        .iter()
        .position(|candidate| candidate == player)?;
    let previous_index = if player_index == 0 {
        state.turn_order.len().checked_sub(1)?
    } else {
        player_index - 1
    };
    let previous = state.turn_order.get(previous_index)?;
    let card = state.last_turn_discard_by_player.get(previous)?.card;
    let level = state.card_def(card)?.level as i32;
    Some(WebDiscardRetrievalActionDetail {
        card: WebCard::from_id(card, labels, card_facts),
        previous_player: previous.clone(),
        hp_cost: crate::rules::hero::discard_retrieval_cost(state, player, level * 2),
    })
}

fn fixture_setup(rules: &OfficialRules, first_player: Option<&str>) -> Result<GameSetup, ApiError> {
    let (first, second) = if first_player == Some("bob") {
        ("bob", "alice")
    } else {
        ("alice", "bob")
    };
    let setup = GameSetup::two_player(PlayerId::new(first), PlayerId::new(second), 20);

    rules
        .configure_game(setup.players, setup.turn_order, Vec::new())
        .map_err(ApiError::Game)
}

fn setup_for_request(
    rules: &OfficialRules,
    requested: Option<WebGameSetup>,
    first_player: Option<&str>,
) -> Result<GameSetup, ApiError> {
    let Some(requested) = requested else {
        return fixture_setup(rules, first_player);
    };

    if requested.players.is_empty() {
        return Err(ApiError::Message(
            "game setup must contain players".to_string(),
        ));
    }

    let players = requested
        .players
        .into_iter()
        .map(|player| Player {
            id: PlayerId::new(player.id),
            team: TeamId::new(player.team),
        })
        .collect::<Vec<_>>();
    let turn_order = requested
        .turn_order
        .into_iter()
        .map(PlayerId::new)
        .collect::<Vec<_>>();
    let modules = requested
        .enabled_rule_modules
        .into_iter()
        .map(RuleModuleId::new)
        .collect();
    let deck_lists = requested
        .deck_lists
        .into_iter()
        .map(|deck| PlayerDeckList {
            player: PlayerId::new(deck.player),
            name: deck.name,
            cards: deck.cards.into_iter().map(CardDefId::new).collect(),
        })
        .collect();
    let mut setup = rules
        .configure_game_with_decks(players, turn_order, modules, deck_lists)
        .map_err(ApiError::Game)?;
    if !requested.initial_hp.is_empty() {
        setup.hp = requested
            .initial_hp
            .into_iter()
            .map(|team_hp| TeamHp {
                team: TeamId::new(team_hp.team),
                hp: team_hp.hp,
            })
            .collect();
        rules.validate_setup(&setup).map_err(ApiError::Game)?;
    }
    Ok(setup)
}

fn deck_order_for_start(
    rules: &OfficialRules,
    setup: &GameSetup,
    deck_seed: Option<&str>,
) -> Result<Vec<CardInstanceId>, ApiError> {
    let mut deck_order = rules.official_deck_order(setup).map_err(ApiError::Game)?;
    shuffle_deck(
        &mut deck_order,
        deck_seed.unwrap_or("fewfc-default-shuffle"),
    );
    Ok(deck_order)
}

fn shuffle_deck(deck_order: &mut [CardInstanceId], seed: &str) {
    if deck_order.len() < 2 {
        return;
    }

    let mut state = seed_to_u64(seed);

    for index in (1..deck_order.len()).rev() {
        state = next_shuffle_state(state);
        let swap_index = (state as usize) % (index + 1);
        deck_order.swap(index, swap_index);
    }
}

fn seed_to_u64(seed: &str) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;

    for byte in seed.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }

    if hash == 0 { 0x9e3779b97f4a7c15 } else { hash }
}

fn next_shuffle_state(mut state: u64) -> u64 {
    state ^= state << 13;
    state ^= state >> 7;
    state ^= state << 17;

    if state == 0 {
        0x9e3779b97f4a7c15
    } else {
        state
    }
}

fn viewer_from_request(viewer: Option<&str>) -> Viewer {
    match viewer {
        Some(viewer) if viewer != "observer" => Viewer::Player(PlayerId::new(viewer)),
        _ => Viewer::Observer,
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum ApiResult {
    NeedsRandomness {
        record: Vec<RecordedDecision>,
        request: PendingRandomness,
    },
    Ready {
        #[serde(flatten)]
        response: ApiResponse,
    },
    ReplayFrame {
        #[serde(rename = "currentStep")]
        current_step: usize,
        #[serde(rename = "totalSteps")]
        total_steps: usize,
        state: WebPublicGameState,
        events: Vec<WebPublicGameEvent>,
        interaction: WebInteraction,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApiResponse {
    record: Vec<RecordedDecision>,
    state: WebPublicGameState,
    events: Vec<WebPublicGameEvent>,
    playable_actions: Vec<WebPlayableAction>,
    interaction: WebInteraction,
    #[serde(skip_serializing_if = "Option::is_none")]
    trusted_random_candidates: Option<Vec<CardInstanceId>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trusted_random_candidate_count: Option<usize>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebInteraction {
    can_pass: bool,
    has_optional_effect: bool,
    can_retrieve_discard: bool,
    discard_retrieval_action: Option<WebDiscardRetrievalActionDetail>,
    can_choose_initial_pouch: bool,
    can_trigger_pouch: bool,
    secret_strategy_actions: Vec<WebSecretStrategyActionOption>,
}

impl WebInteraction {
    fn disabled() -> Self {
        Self {
            can_pass: false,
            has_optional_effect: false,
            can_retrieve_discard: false,
            discard_retrieval_action: None,
            can_choose_initial_pouch: false,
            can_trigger_pouch: false,
            secret_strategy_actions: Vec::new(),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebDiscardRetrievalActionDetail {
    card: WebCard,
    previous_player: PlayerId,
    hp_cost: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPublicGameState {
    enabled_rule_modules: Vec<String>,
    status: String,
    turn_number: u64,
    phase: String,
    current_player: Option<String>,
    players: Vec<WebPlayer>,
    turn_order: Vec<String>,
    hp: Vec<WebTeamHp>,
    hands: Vec<WebPlayerHand>,
    deck_count: usize,
    discard: Vec<WebCard>,
    player_decks: Vec<WebPlayerDeck>,
    player_discards: Vec<WebPlayerDiscard>,
    pouches: Vec<WebPouch>,
    preparation_player: Option<String>,
    covered_passives: Vec<WebCoveredPassive>,
    counter_effects: Vec<WebCounterEffect>,
    pending_choice: Option<WebPendingChoice>,
    pending_randomness: Option<WebPendingRandomness>,
    shields: Vec<WebShield>,
    statuses: Vec<WebStatus>,
    jianghu_states: Vec<WebJianghuState>,
    limited_uses: Vec<WebLimitedUse>,
    confluence_card_obligations: Vec<WebConfluenceCardObligation>,
    scheduled_echoes: Vec<WebScheduledEcho>,
    flow_states: Vec<WebFlowState>,
    formation_suppressions: Vec<WebFormationSuppression>,
    scheduled_plant_earth: Vec<WebScheduledPlantEarth>,
    environment: Option<String>,
    team_stars: Vec<WebTeamStar>,
    star_histories: Vec<WebPlayerStarHistory>,
    five_star_alignment: Option<WebFiveStarAlignment>,
    professions: Vec<WebPlayerProfession>,
    profession_catalog: Vec<WebProfessionCatalogEntry>,
    card_interpretations: Vec<WebCardInterpretationPresentation>,
    spirits: Vec<WebPlayerSpirit>,
    previous_turn_formation: Option<WebPreviousTurnFormation>,
}

impl WebPublicGameState {
    fn from_public(
        state: PublicGameState,
        labels: &HashMap<CardInstanceId, String>,
        card_facts: &HashMap<CardInstanceId, WebCardFact>,
        formation_names: &HashMap<String, String>,
    ) -> Self {
        let enabled_rule_modules = state.enabled_rule_modules.clone();
        let preparation_player = match &state.status {
            crate::domain::GameStatus::Preparing {
                stage: crate::domain::GamePreparationStage::InitialPouchSelection { player },
            } => Some(player.as_str().to_string()),
            _ => None,
        };
        Self {
            enabled_rule_modules: enabled_rule_modules
                .iter()
                .map(|module| module.as_str().to_string())
                .collect(),
            status: match &state.status {
                crate::domain::GameStatus::Preparing { .. } => "Preparing".to_string(),
                crate::domain::GameStatus::InProgress => "InProgress".to_string(),
                crate::domain::GameStatus::Finished { .. } => "Finished".to_string(),
            },
            turn_number: state.turn_number,
            phase: format!("{:?}", state.phase),
            current_player: state
                .current_player
                .map(|player| player.as_str().to_string()),
            players: state
                .players
                .into_iter()
                .map(|player| WebPlayer {
                    id: player.id.as_str().to_string(),
                    team: serde_json::to_value(player.team)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string(),
                })
                .collect(),
            turn_order: state
                .turn_order
                .into_iter()
                .map(|player| player.as_str().to_string())
                .collect(),
            hp: state
                .hp
                .into_iter()
                .map(|hp| WebTeamHp {
                    team: serde_json::to_value(hp.team)
                        .unwrap()
                        .as_str()
                        .unwrap()
                        .to_string(),
                    hp: hp.hp,
                })
                .collect(),
            hands: state
                .hands
                .into_iter()
                .map(|hand| WebPlayerHand {
                    player: hand.player.as_str().to_string(),
                    cards: WebCardRefs::from_public(hand.cards, labels, card_facts),
                })
                .collect(),
            deck_count: state.deck_count,
            discard: state
                .discard
                .into_iter()
                .map(|card| WebCard::from_id(card, labels, card_facts))
                .collect(),
            player_decks: state
                .player_decks
                .into_iter()
                .map(|pile| WebPlayerDeck {
                    player: pile.player.as_str().to_string(),
                    cards: WebCardRefs::from_public(pile.cards, labels, card_facts),
                })
                .collect(),
            player_discards: state
                .player_discards
                .into_iter()
                .map(|pile| WebPlayerDiscard {
                    player: pile.player.as_str().to_string(),
                    cards: pile
                        .cards
                        .into_iter()
                        .map(|card| WebCard::from_id(card, labels, card_facts))
                        .collect(),
                })
                .collect(),
            pouches: state
                .pouches
                .into_iter()
                .map(|pouch| WebPouch {
                    owner: pouch.owner.as_str().to_string(),
                    card: pouch
                        .card
                        .map(|card| WebCard::from_id(card, labels, card_facts)),
                })
                .collect(),
            preparation_player,
            covered_passives: state
                .covered_passives
                .into_iter()
                .map(|passive| WebCoveredPassive {
                    owner: passive.owner.as_str().to_string(),
                    formation_id: passive.formation_id,
                    cards: WebCardRefs::from_public(passive.cards, labels, card_facts),
                    star_substitution: passive
                        .star_substitution
                        .map(WebStarElementSubstitution::from),
                })
                .collect(),
            counter_effects: state
                .counter_effects
                .into_iter()
                .map(|counter| WebCounterEffect {
                    owner: counter.owner.as_str().to_string(),
                    effect_id: counter.effect_id.clone(),
                    effect_name: formation_name(formation_names, &counter.effect_id),
                })
                .collect(),
            pending_choice: state.pending_choice.map(|choice| {
                WebPendingChoice::from_public(choice, labels, card_facts, &enabled_rule_modules)
            }),
            pending_randomness: state
                .pending_randomness
                .map(|request| WebPendingRandomness {
                    request_id: request.request_id,
                    deck: match request.deck {
                        crate::domain::RandomnessDeck::Shared => "shared".to_string(),
                        crate::domain::RandomnessDeck::Player(player) => {
                            format!("player:{}", player.as_str())
                        }
                    },
                    operation: match request.operation {
                        crate::public_view::PublicRandomnessOperation::DeckShuffle => "deckShuffle",
                        crate::public_view::PublicRandomnessOperation::DiscardShuffle => {
                            "discardShuffle"
                        }
                    }
                    .to_string(),
                    card_count: request.card_count,
                }),
            shields: state
                .shields
                .into_iter()
                .map(|shield| WebShield {
                    player: shield.player.as_str().to_string(),
                    value: shield.value,
                })
                .collect(),
            statuses: state
                .statuses
                .into_iter()
                .map(|status| {
                    let presentation = WebStatusPresentation::from_kind(&status.kind);
                    let owner = match status.owner {
                        StatusOwner::Player(player) => WebStatusOwner::Player {
                            id: player.as_str().to_string(),
                        },
                        StatusOwner::Team(team) => WebStatusOwner::Team {
                            id: serde_json::to_value(team)
                                .expect("team id should serialize")
                                .as_str()
                                .expect("team id should serialize as a string")
                                .to_string(),
                        },
                    };
                    WebStatus {
                        id: status.id,
                        owner,
                        kind: status.kind,
                        presentation,
                        duration: WebStatusDuration::from(status.duration),
                    }
                })
                .collect(),
            jianghu_states: state
                .jianghu_states
                .into_iter()
                .map(|active| WebJianghuState {
                    owner: active.owner.as_str().to_string(),
                    kind: format!("{:?}", active.kind),
                    remaining_turns: active.remaining_turns,
                    expires_on_turn: active.expires_on_turn,
                })
                .collect(),
            limited_uses: state
                .limited_uses
                .into_iter()
                .map(|use_count| {
                    let presentation = WebLimitedUsePresentation::from_key(&use_count.key);
                    WebLimitedUse {
                        owner: use_count.owner.as_str().to_string(),
                        key: use_count.key,
                        presentation,
                        remaining: use_count.remaining,
                        maximum: use_count.maximum,
                    }
                })
                .collect(),
            confluence_card_obligations: state
                .confluence_card_obligations
                .into_iter()
                .map(|obligation| WebConfluenceCardObligation {
                    owner: obligation.owner.as_str().to_string(),
                    card: obligation.card,
                    allow_profession_formation: obligation.allow_profession_formation,
                })
                .collect(),
            scheduled_echoes: state
                .scheduled_echoes
                .into_iter()
                .map(|schedule| WebScheduledEcho {
                    player: schedule.player.as_str().to_string(),
                    melody: WebEchoSchedulePresentation::from_id(&schedule.melody_id),
                    due_turn_number: schedule.due_turn_number,
                })
                .collect(),
            flow_states: state
                .flow_states
                .into_iter()
                .map(|flow| WebFlowState {
                    player: flow.player.as_str().to_string(),
                    layers: flow.layers,
                })
                .collect(),
            formation_suppressions: state
                .formation_suppressions
                .into_iter()
                .map(|suppression| WebFormationSuppression {
                    source: suppression.source.as_str().to_string(),
                    target: suppression.target.as_str().to_string(),
                    formation_name: formation_name(formation_names, &suppression.formation_id),
                    expires_on_turn_number: suppression.expires_on_turn_number,
                })
                .collect(),
            scheduled_plant_earth: state
                .scheduled_plant_earth
                .into_iter()
                .map(|schedule| WebScheduledPlantEarth {
                    player: schedule.player.as_str().to_string(),
                    due_turn_number: schedule.due_turn_number,
                })
                .collect(),
            environment: state
                .environment
                .map(|environment| format!("{environment:?}")),
            team_stars: state
                .team_stars
                .into_iter()
                .map(|owned| WebTeamStar {
                    team: owned.team.as_str().to_string(),
                    star: format!("{:?}", owned.star),
                })
                .collect(),
            star_histories: state
                .star_histories
                .into_iter()
                .map(|history| WebPlayerStarHistory {
                    player: history.player.as_str().to_string(),
                    stars: history
                        .stars
                        .into_iter()
                        .map(|star| format!("{star:?}"))
                        .collect(),
                })
                .collect(),
            five_star_alignment: state
                .five_star_alignment
                .map(|alignment| WebFiveStarAlignment {
                    player: alignment.player.as_str().to_string(),
                    team: alignment.team.as_str().to_string(),
                }),
            professions: state
                .professions
                .into_iter()
                .filter_map(|owned| {
                    let profession = crate::rules::profession::definition(
                        &enabled_rule_modules,
                        &owned.profession,
                    )?;
                    Some(WebPlayerProfession {
                        player: owned.player.as_str().to_string(),
                        id: owned.profession.as_str().to_string(),
                        name: profession.name.to_string(),
                        abilities: crate::rules::profession::effective_ability_summaries(
                            &enabled_rule_modules,
                            &owned.profession,
                        )
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                    })
                })
                .collect(),
            profession_catalog: crate::rules::profession::catalog(&enabled_rule_modules)
                .into_iter()
                .map(|profession| {
                    let parent_name = profession.parents.first().and_then(|parent| {
                        crate::rules::profession::definition(&enabled_rule_modules, parent)
                            .map(|definition| definition.name.to_string())
                    });
                    let inheritance = if let Some(parent) = &parent_name {
                        format!("升階後保留 {parent} 的能力")
                    } else if matches!(profession.id.as_str(), "immortal" | "saint") {
                        "轉職後不保留原學派能力".to_string()
                    } else {
                        "不繼承其他職業能力".to_string()
                    };
                    WebProfessionCatalogEntry {
                        id: profession.id.as_str().to_string(),
                        name: profession.name.to_string(),
                        requirement: profession.rule_text.to_string(),
                        parent_name,
                        inheritance,
                        abilities: crate::rules::profession::effective_ability_summaries(
                            &enabled_rule_modules,
                            &profession.id,
                        )
                        .into_iter()
                        .map(str::to_string)
                        .collect(),
                        formations: crate::rules::profession::profession_formation_summaries(
                            &enabled_rule_modules,
                            &profession.id,
                        )
                        .into_iter()
                        .map(|(name, summary)| WebProfessionFormationSummary { name, summary })
                        .collect(),
                    }
                })
                .collect(),
            card_interpretations: state
                .card_interpretations
                .into_iter()
                .map(|interpretation| match interpretation {
                    PublicCardInterpretation::ProfessionAbility {
                        player,
                        ability_id,
                        card,
                        element,
                        level,
                    } => WebCardInterpretationPresentation::ProfessionAbility {
                        player: player.as_str().to_string(),
                        ability: match ability_id.as_str() {
                            "illusion" => WebProfessionInterpretationAbility::Illusion,
                            "phantasm" => WebProfessionInterpretationAbility::Phantasm,
                            "jianghu:blazing-yang-art" => {
                                WebProfessionInterpretationAbility::BlazingYangArt
                            }
                            "dark:dark-spirit" => WebProfessionInterpretationAbility::DarkSpirit,
                            _ => WebProfessionInterpretationAbility::Unclassified,
                        },
                        card: card.map(|card| WebCard::from_id(card, labels, card_facts)),
                        element,
                        level,
                    },
                    PublicCardInterpretation::SpiritSkill {
                        player,
                        skill,
                        card,
                        level,
                    } => WebCardInterpretationPresentation::SpiritSkill {
                        player: player.as_str().to_string(),
                        skill: match skill {
                            Some(crate::domain::SpiritSkill::Glimmer) => {
                                WebSpiritInterpretationSkill::Glimmer
                            }
                            Some(crate::domain::SpiritSkill::Splendor) => {
                                WebSpiritInterpretationSkill::Splendor
                            }
                            None => WebSpiritInterpretationSkill::LegacyFireLevel,
                            Some(_) => WebSpiritInterpretationSkill::Unclassified,
                        },
                        card: card.map(|card| WebCard::from_id(card, labels, card_facts)),
                        level,
                    },
                })
                .collect(),
            spirits: state
                .spirits
                .into_iter()
                .map(|owned| WebPlayerSpirit {
                    player: owned.player.as_str().to_string(),
                    spirit: format!("{:?}", owned.spirit),
                    power: owned.power,
                })
                .collect(),
            previous_turn_formation: state.previous_turn_formation.map(|formation| {
                WebPreviousTurnFormation {
                    player: formation.player.as_str().to_string(),
                    formation_id: formation.formation_id.clone(),
                    formation_name: formation
                        .formation_id
                        .as_deref()
                        .map(|id| formation_name(formation_names, id)),
                    cards: WebCardRefs::from_public(formation.cards, labels, card_facts),
                }
            }),
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPlayer {
    id: String,
    team: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebTeamHp {
    team: String,
    hp: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebTeamStar {
    team: String,
    star: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPlayerStarHistory {
    player: String,
    stars: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPlayerSpirit {
    player: String,
    spirit: String,
    power: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebJianghuState {
    owner: String,
    kind: String,
    remaining_turns: u32,
    expires_on_turn: Option<u64>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebLimitedUse {
    owner: String,
    key: String,
    presentation: WebLimitedUsePresentation,
    remaining: u32,
    maximum: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum WebLimitedUsePresentation {
    HeavenlyResonance,
    ImprisoningArray,
    Tailwind,
    VoidRealm,
    Unclassified,
}

impl WebLimitedUsePresentation {
    fn from_key(key: &str) -> Self {
        match key {
            "confluence:heavenly-resonance" => Self::HeavenlyResonance,
            "confluence:imprisoning-array" => Self::ImprisoningArray,
            "confluence:tailwind" => Self::Tailwind,
            "confluence:void-realm" => Self::VoidRealm,
            _ => Self::Unclassified,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebConfluenceCardObligation {
    owner: String,
    card: Option<CardInstanceId>,
    allow_profession_formation: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebFiveStarAlignment {
    player: String,
    team: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPlayerProfession {
    player: String,
    id: String,
    name: String,
    abilities: Vec<String>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebProfessionCatalogEntry {
    id: String,
    name: String,
    requirement: String,
    parent_name: Option<String>,
    inheritance: String,
    abilities: Vec<String>,
    formations: Vec<WebProfessionFormationSummary>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebProfessionFormationSummary {
    name: String,
    summary: String,
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WebCardInterpretationPresentation {
    ProfessionAbility {
        player: String,
        ability: WebProfessionInterpretationAbility,
        card: Option<WebCard>,
        element: Element,
        level: u32,
    },
    SpiritSkill {
        player: String,
        skill: WebSpiritInterpretationSkill,
        card: Option<WebCard>,
        level: u32,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum WebProfessionInterpretationAbility {
    Illusion,
    Phantasm,
    BlazingYangArt,
    DarkSpirit,
    Unclassified,
}

#[derive(Serialize)]
enum WebSpiritInterpretationSkill {
    Glimmer,
    Splendor,
    LegacyFireLevel,
    Unclassified,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPlayerHand {
    player: String,
    cards: WebCardRefs,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPlayerDeck {
    player: String,
    cards: WebCardRefs,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPlayerDiscard {
    player: String,
    cards: Vec<WebCard>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPouch {
    owner: String,
    card: Option<WebCard>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebCoveredPassive {
    owner: String,
    formation_id: Option<String>,
    cards: WebCardRefs,
    star_substitution: Option<WebStarElementSubstitution>,
}

#[derive(Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WebStarElementSubstitution {
    card: CardInstanceId,
    printed_element: crate::domain::Element,
    interpreted_element: crate::domain::Element,
}

#[derive(Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct WebFormationMatchOption {
    role: String,
    card: CardInstanceId,
    slots: usize,
    preview: Option<String>,
}

impl From<crate::domain::StarElementSubstitution> for WebStarElementSubstitution {
    fn from(substitution: crate::domain::StarElementSubstitution) -> Self {
        Self {
            card: substitution.card,
            printed_element: substitution.printed_element,
            interpreted_element: substitution.interpreted_element,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebCounterEffect {
    owner: String,
    effect_id: String,
    effect_name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPreviousTurnFormation {
    player: String,
    formation_id: Option<String>,
    formation_name: Option<String>,
    cards: WebCardRefs,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPendingChoice {
    player: String,
    purpose: String,
    presentation: PublicPendingChoicePresentation,
    kind: String,
    cards: Vec<WebCard>,
    deck_cards: Vec<WebCard>,
    discard_cards: Vec<WebCard>,
    required_count: usize,
    minimum_count: usize,
    maximum_count: usize,
    players: Vec<String>,
    formations: Vec<String>,
    formation_groups: Vec<WebFormationChoiceGroup>,
    environments: Vec<String>,
    can_decline: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebFormationChoiceGroup {
    rule_module_id: Option<String>,
    formations: Vec<WebFormationChoice>,
}

#[derive(Serialize)]
struct WebFormationChoice {
    id: String,
    name: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPendingRandomness {
    request_id: String,
    deck: String,
    operation: String,
    card_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebScheduledEcho {
    player: String,
    melody: WebEchoSchedulePresentation,
    due_turn_number: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum WebEchoSchedulePresentation {
    RingingMetal,
    FallingWood,
    FlowingWater,
    WarFire,
    SplitEarth,
    PureFire,
    Unclassified,
}

impl WebEchoSchedulePresentation {
    fn from_id(id: &str) -> Self {
        match id {
            "echo:ringing-metal" => Self::RingingMetal,
            "echo:falling-wood" => Self::FallingWood,
            "echo:flowing-water" => Self::FlowingWater,
            "echo:war-fire" => Self::WarFire,
            "echo:split-earth" => Self::SplitEarth,
            "echo:pure-fire" => Self::PureFire,
            _ => Self::Unclassified,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebFlowState {
    player: String,
    layers: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebFormationSuppression {
    source: String,
    target: String,
    formation_name: String,
    expires_on_turn_number: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebScheduledPlantEarth {
    player: String,
    due_turn_number: u64,
}

impl WebPendingChoice {
    fn from_public(
        choice: crate::public_view::PublicPendingChoice,
        labels: &HashMap<CardInstanceId, String>,
        card_facts: &HashMap<CardInstanceId, WebCardFact>,
        enabled_rule_modules: &[RuleModuleId],
    ) -> Self {
        let (minimum_count, maximum_count) = match &choice.kind {
            PublicPendingChoiceKind::Known(kind) => kind.selection_bounds(),
            PublicPendingChoiceKind::Hidden => (0, 0),
        };
        let required_count = minimum_count;

        let presentation = choice.presentation;
        let is_split_earth_formation_choice = matches!(
            presentation,
            PublicPendingChoicePresentation::EchoSplitEarthFormation
        );
        match choice.kind {
            PublicPendingChoiceKind::Known(PendingChoiceKind::TurnDrawDiscard {
                allowed_discards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                presentation,
                kind: "TurnDrawDiscard".to_string(),
                required_count,
                minimum_count,
                maximum_count,
                cards: allowed_discards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels, card_facts))
                    .collect(),
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
                players: Vec::new(),
                formations: Vec::new(),
                formation_groups: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
            PublicPendingChoiceKind::Known(PendingChoiceKind::EffectGenerated {
                allowed_cards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                presentation,
                kind: "EffectGenerated".to_string(),
                required_count,
                minimum_count,
                maximum_count,
                cards: allowed_cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels, card_facts))
                    .collect(),
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
                players: Vec::new(),
                formations: Vec::new(),
                formation_groups: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
            PublicPendingChoiceKind::Known(PendingChoiceKind::CardSetChoice {
                allowed_cards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                presentation,
                kind: "EffectGenerated".to_string(),
                required_count,
                minimum_count,
                maximum_count,
                cards: allowed_cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels, card_facts))
                    .collect(),
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
                players: Vec::new(),
                formations: Vec::new(),
                formation_groups: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
            PublicPendingChoiceKind::Known(PendingChoiceKind::TypedEffect { options, .. }) => {
                Self {
                    player: choice.player.as_str().to_string(),
                    purpose: choice.purpose,
                    presentation,
                    kind: "TypedEffect".to_string(),
                    required_count,
                    minimum_count,
                    maximum_count,
                    cards: options
                        .cards
                        .map(|cards| {
                            cards
                                .allowed_cards
                                .into_iter()
                                .map(|card| WebCard::from_id(card, labels, card_facts))
                                .collect()
                        })
                        .unwrap_or_default(),
                    deck_cards: Vec::new(),
                    discard_cards: Vec::new(),
                    players: options
                        .players
                        .into_iter()
                        .map(|player| player.as_str().to_string())
                        .collect(),
                    formation_groups: if is_split_earth_formation_choice {
                        web_formation_choice_groups(&options.formations, enabled_rule_modules)
                    } else {
                        Vec::new()
                    },
                    formations: options.formations,
                    environments: options
                        .environments
                        .into_iter()
                        .map(|environment| format!("{environment:?}"))
                        .collect(),
                    can_decline: options.can_decline,
                }
            }
            PublicPendingChoiceKind::Known(PendingChoiceKind::SheepStealing {
                deck_cards,
                discard_cards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                presentation,
                kind: "SheepStealing".to_string(),
                required_count,
                minimum_count,
                maximum_count,
                cards: Vec::new(),
                deck_cards: deck_cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels, card_facts))
                    .collect(),
                discard_cards: discard_cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels, card_facts))
                    .collect(),
                players: Vec::new(),
                formations: Vec::new(),
                formation_groups: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
            PublicPendingChoiceKind::Hidden => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                presentation,
                kind: "Hidden".to_string(),
                cards: Vec::new(),
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
                required_count: 0,
                minimum_count: 0,
                maximum_count: 0,
                players: Vec::new(),
                formations: Vec::new(),
                formation_groups: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
        }
    }
}

fn web_formation_choice_groups(
    allowed_formations: &[String],
    enabled_rule_modules: &[RuleModuleId],
) -> Vec<WebFormationChoiceGroup> {
    let allowed = allowed_formations
        .iter()
        .map(String::as_str)
        .collect::<std::collections::HashSet<_>>();

    crate::rules::official_formation_groups(enabled_rule_modules)
        .into_iter()
        .filter_map(|group| {
            let formations = group
                .formations
                .into_iter()
                .filter(|formation| allowed.contains(formation.id.as_str()))
                .map(|formation| WebFormationChoice {
                    id: formation.id,
                    name: formation.name,
                })
                .collect::<Vec<_>>();
            (!formations.is_empty()).then(|| WebFormationChoiceGroup {
                rule_module_id: group
                    .rule_module_id
                    .map(|module| module.as_str().to_string()),
                formations,
            })
        })
        .collect()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebShield {
    player: String,
    value: i32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebStatus {
    id: String,
    owner: WebStatusOwner,
    kind: String,
    presentation: WebStatusPresentation,
    duration: WebStatusDuration,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
enum WebStatusPresentation {
    CannotAct,
    CannotDraw,
    DivineCalculation,
    GaleRain,
    GoldenCicada,
    WatchFire,
    LurePlayer,
    LureSpirit,
    SpiritStoneShield,
    JianghuFanBeyondHeaven,
    JianghuYangAura,
    JianghuDancingYang,
    JianghuMeteor,
    Unclassified,
}

impl WebStatusPresentation {
    fn from_kind(kind: &str) -> Self {
        match kind {
            "CannotAct" => Self::CannotAct,
            "CannotDraw" => Self::CannotDraw,
            "DivineCalculation" => Self::DivineCalculation,
            "GaleRain" => Self::GaleRain,
            "PouchGoldenCicada" => Self::GoldenCicada,
            "PouchWatchFire" => Self::WatchFire,
            "PouchLurePlayer" => Self::LurePlayer,
            "PouchLureSpirit" => Self::LureSpirit,
            "SpiritStoneShield" => Self::SpiritStoneShield,
            "JianghuFanBeyondHeaven" => Self::JianghuFanBeyondHeaven,
            "JianghuYangAura" => Self::JianghuYangAura,
            "JianghuDancingYang" => Self::JianghuDancingYang,
            "JianghuMeteor" => Self::JianghuMeteor,
            _ => Self::Unclassified,
        }
    }
}

#[derive(Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum WebStatusDuration {
    UntilTurnStart { player: PlayerId },
    UntilTurnEnd { player: PlayerId },
    UntilTurnEndNumber { player: PlayerId, turn_number: u64 },
    Permanent,
}

impl From<StatusDuration> for WebStatusDuration {
    fn from(duration: StatusDuration) -> Self {
        match duration {
            StatusDuration::UntilTurnStart { player } => Self::UntilTurnStart { player },
            StatusDuration::UntilTurnEnd { player } => Self::UntilTurnEnd { player },
            StatusDuration::UntilTurnEndNumber {
                player,
                turn_number,
            } => Self::UntilTurnEndNumber {
                player,
                turn_number,
            },
            StatusDuration::Permanent => Self::Permanent,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum WebStatusOwner {
    Player { id: String },
    Team { id: String },
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum WebCardRefs {
    Known { cards: Vec<WebCard> },
    Hidden { count: usize },
    PartiallyKnown { cards: Vec<Option<WebCard>> },
}

impl WebCardRefs {
    fn from_public(
        cards: PublicCardRefs,
        labels: &HashMap<CardInstanceId, String>,
        card_facts: &HashMap<CardInstanceId, WebCardFact>,
    ) -> Self {
        match cards {
            PublicCardRefs::Known(cards) => Self::Known {
                cards: cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels, card_facts))
                    .collect(),
            },
            PublicCardRefs::Hidden { count } => Self::Hidden { count },
            PublicCardRefs::PartiallyKnown { cards } => Self::PartiallyKnown {
                cards: cards
                    .into_iter()
                    .map(|card| card.map(|card| WebCard::from_id(card, labels, card_facts)))
                    .collect(),
            },
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebCard {
    id: CardInstanceId,
    label: String,
    element: Option<Element>,
    level: Option<u32>,
    secret_strategies: Vec<WebSecretStrategyCardOption>,
}

impl WebCard {
    fn from_id(
        id: CardInstanceId,
        labels: &HashMap<CardInstanceId, String>,
        card_facts: &HashMap<CardInstanceId, WebCardFact>,
    ) -> Self {
        let facts = card_facts.get(&id);
        Self {
            id,
            label: labels
                .get(&id)
                .cloned()
                .unwrap_or_else(|| "一張牌".to_string()),
            element: facts.map(|facts| facts.element),
            level: facts.map(|facts| facts.level),
            secret_strategies: facts
                .map(|facts| facts.secret_strategies.clone())
                .unwrap_or_default(),
        }
    }
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebSecretStrategyCardOption {
    strategy: SecretStrategy,
    input: WebSecretStrategyInputRequirement,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebSecretStrategyActionOption {
    source_card: CardInstanceId,
    strategy: SecretStrategy,
    input: WebSecretStrategyInputRequirement,
    target_players: Vec<PlayerId>,
    stars: Vec<StarKind>,
    break_stars: Vec<StarKind>,
    deck_cards: Vec<CardInstanceId>,
    discard_cards: Vec<CardInstanceId>,
    hand_cards: Vec<CardInstanceId>,
    required_card_count: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
enum WebSecretStrategyInputRequirement {
    None,
    TargetPlayer,
    DeckDiscardSwap,
    Star,
    Retreat,
}

impl From<crate::rules::pouch::SecretStrategyCardOption> for WebSecretStrategyCardOption {
    fn from(option: crate::rules::pouch::SecretStrategyCardOption) -> Self {
        use crate::rules::pouch::SecretStrategyInputRequirement as Input;
        Self {
            strategy: option.strategy,
            input: match option.input {
                Input::None => WebSecretStrategyInputRequirement::None,
                Input::TargetPlayer => WebSecretStrategyInputRequirement::TargetPlayer,
                Input::DeckDiscardSwap => WebSecretStrategyInputRequirement::DeckDiscardSwap,
                Input::Star => WebSecretStrategyInputRequirement::Star,
                Input::Retreat => WebSecretStrategyInputRequirement::Retreat,
            },
        }
    }
}

impl From<crate::rules::pouch::SecretStrategyActionOption> for WebSecretStrategyActionOption {
    fn from(option: crate::rules::pouch::SecretStrategyActionOption) -> Self {
        use crate::rules::pouch::SecretStrategyInputRequirement as Input;
        Self {
            source_card: option.source_card,
            strategy: option.strategy,
            input: match option.input {
                Input::None => WebSecretStrategyInputRequirement::None,
                Input::TargetPlayer => WebSecretStrategyInputRequirement::TargetPlayer,
                Input::DeckDiscardSwap => WebSecretStrategyInputRequirement::DeckDiscardSwap,
                Input::Star => WebSecretStrategyInputRequirement::Star,
                Input::Retreat => WebSecretStrategyInputRequirement::Retreat,
            },
            target_players: option.target_players,
            stars: option.stars,
            break_stars: option.break_stars,
            deck_cards: option.deck_cards,
            discard_cards: option.discard_cards,
            hand_cards: option.hand_cards,
            required_card_count: option.required_card_count,
        }
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPublicGameEvent {
    id: String,
    event_type: String,
    title: String,
    summary: String,
}

impl WebPublicGameEvent {
    fn from_public(
        sequence: usize,
        event: PublicGameEvent,
        labels: &HashMap<CardInstanceId, String>,
        formation_names: &HashMap<String, String>,
        vocabulary: &PlayerVocabulary,
    ) -> Self {
        let (title, summary) =
            event_presentation_with_vocabulary(&event, labels, formation_names, vocabulary);
        Self {
            id: format!("event-{sequence}"),
            event_type: event_type(&event),
            title,
            summary,
        }
    }
}

/// The only boundary where stable rule identities become player-facing language.
/// Canonical events deliberately retain IDs for deterministic replay.
struct PlayerVocabulary {
    professions: HashMap<ProfessionId, String>,
}

impl PlayerVocabulary {
    fn for_modules(enabled_modules: &[RuleModuleId]) -> Self {
        Self {
            professions: crate::rules::profession::catalog(enabled_modules)
                .into_iter()
                .map(|profession| (profession.id, profession.name.to_string()))
                .collect(),
        }
    }

    #[cfg(test)]
    fn all_official() -> Self {
        Self::for_modules(
            &[
                crate::domain::HERO_SCHOOLS_MODULE_ID,
                crate::domain::JIANGHU_MODULE_ID,
                crate::domain::CONFLUENCE_GENERATION_MODULE_ID,
                crate::domain::DARK_GLIMMER_MODULE_ID,
            ]
            .into_iter()
            .map(RuleModuleId::new)
            .collect::<Vec<_>>(),
        )
    }

    fn profession(&self, profession: &ProfessionId) -> String {
        self.professions
            .get(profession)
            .cloned()
            .unwrap_or_else(|| "未知職業".to_string())
    }

    fn previous_profession(&self, profession: Option<&ProfessionId>) -> String {
        profession.map_or_else(
            || "無職業".to_string(),
            |profession| self.profession(profession),
        )
    }

    fn ability(&self, id: &str) -> &'static str {
        match id {
            "shadow-cut" => "影切",
            "meditation" => "冥思",
            "revelation" => "啟示",
            "illusion" => "幻術",
            "phantasm" => "幻朧",
            "hero:physical-damage-resistance" => "卸勁",
            "hero:weapon-proficiency" => "武器專精",
            "hero:defense-proficiency" => "防禦專精",
            "hero:countershock-proficiency" => "反震專精",
            "hero:weapon-mastery" => "武器精研",
            "hero:shock-burst-proficiency" => "震暴專精",
            "hero:metal-resistance" => "金行抗性",
            "hero:battle-soul" => "戰魄",
            "hero:seeker-discount" => "尋道術",
            "hero:generating-formation-proficiency" => "生陣專精",
            "hero:overcoming-formation-proficiency" => "剋陣專精",
            "hero:return-to-origin-proficiency" => "歸元專精",
            "hero:five-elements-cycle-proficiency" => "五行輪迴專精",
            "hero:wood-resistance" => "木行抗性",
            "hero:spell-protection" => "道源",
            "hero:illusion" => "幻術",
            "hero:seal-proficiency" => "封印專精",
            "hero:illusion-refinement" => "幻術精研",
            "hero:barrier-proficiency" => "氣壁專精",
            "hero:phantasm" => "幻朧",
            "hero:water-resistance" => "水行抗性",
            "hero:triple-element-proficiency" => "五行三張攻擊專精",
            "hero:radiance-proficiency" => "光芒專精",
            "hero:five-streams-unite-proficiency" => "五流歸一專精",
            "hero:fire-resistance" => "火行抗性",
            "hero:arcane-essence" => "法粹",
            "hero:windwalking" => "風行術",
            "hero:metamorphosis-proficiency" => "幻化專精",
            "hero:shadow-cut" => "影切",
            "hero:chaos-proficiency" => "混沌專精",
            "hero:earth-resistance" => "土行抗性",
            "hero:shadow-escape" => "影遁",
            "hero:choice" => "抉擇",
            "hero:breakthrough" => "突破",
            "hero:immortal-draw-bonus" => "仙術",
            "hero:meditation" => "冥思",
            "hero:sacred-art" => "聖術",
            "hero:revelation" => "啟示",
            "jianghu:heavenly-yang-aura" => "天陽罡",
            "jianghu:blazing-yang-art" => "烈陽訣",
            "jianghu:pure-yang-force" => "純陽勁",
            "jianghu:divine-yang-aura" => "神陽罡",
            "jianghu:dancing-yang-art" => "舞陽訣",
            "jianghu:extreme-yang-force" => "極陽勁",
            "jianghu:azure-cloud-step" => "青雲步",
            "jianghu:righteous-spirit" => "浩然正氣",
            "jianghu:meteor-step" => "流星步",
            "jianghu:poison-mastery" => "毒絕",
            "confluence:tuning" => "調律",
            "confluence:string-changing" => "易弦",
            "confluence:heavenly-resonance" => "天響",
            "confluence:living-dao" => "道法心生",
            "confluence:clear-wind" => "晴風",
            "confluence:tailwind" => "順風",
            "confluence:void-seeking" => "虛空追尋",
            "confluence:void-destruction" => "虛空破滅",
            "confluence:void-realm" => "虛空境界",
            "dark:dark-walking" => "暗行",
            "dark:dark-spirit" => "暗靈",
            "dark:dark-realm" => "暗境",
            "dark:berserk-shadow" => "狂影",
            "dark:demon-spirit-possession" => "魔靈附體",
            "dark:demon-spirit-revival" => "魔靈復甦",
            _ => "未知能力",
        }
    }

    fn limited_use(&self, key: &str) -> &'static str {
        match key {
            "confluence:heavenly-resonance" => "天響",
            "confluence:imprisoning-array" => "禁錮法陣",
            "confluence:tailwind" => "順風",
            "confluence:void-realm" => "虛空境界",
            _ => "未知能力",
        }
    }

    fn reason(&self, id: &str) -> &'static str {
        match id {
            "pouch:dark-crossing" => "暗渡陳倉",
            "confluence:void-seeking" => "虛空追尋",
            "dark:dark-walking" => "暗行",
            _ => "未知原因",
        }
    }

    fn choice_purpose(&self, purpose: &str, formation_names: &HashMap<String, String>) -> String {
        if purpose == "turn-draw-discard" {
            return "回合抽牌".to_string();
        }
        if let Some(name) = formation_names.get(purpose) {
            return name.clone();
        }
        match purpose {
            "choose-card-to-seal" => "封印".to_string(),
            _ => {
                let ability = self.ability(purpose);
                if ability != "未知能力" {
                    ability.to_string()
                } else {
                    "未知效果".to_string()
                }
            }
        }
    }
}

#[derive(Serialize, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WebPlayableAction {
    PerformFormation {
        id: String,
        name: String,
        category: WebFormationCategory,
        policy: WebFormationActionPolicy,
        summary: String,
        cards: Vec<CardInstanceId>,
        #[serde(rename = "starSubstitution")]
        star_substitution: Option<WebStarElementSubstitution>,
        #[serde(rename = "matchOption")]
        match_option: Option<WebFormationMatchOption>,
    },
    ChangeProfession {
        id: String,
        name: String,
        summary: String,
        cards: Vec<CardInstanceId>,
    },
    ActivateProfessionAbility {
        id: String,
        name: String,
        summary: String,
        cards: Vec<CardInstanceId>,
        #[serde(rename = "targetCard")]
        target_card: Option<CardInstanceId>,
        #[serde(rename = "declaredElement")]
        declared_element: Option<String>,
        #[serde(rename = "declaredLevel")]
        declared_level: Option<u32>,
    },
    UseSpiritSkill {
        id: String,
        name: String,
        summary: String,
        cards: Vec<CardInstanceId>,
        #[serde(rename = "selectedCard")]
        selected_card: Option<CardInstanceId>,
        #[serde(rename = "declaredLevel")]
        declared_level: Option<u32>,
    },
}

#[derive(Serialize, PartialEq, Eq)]
enum WebFormationCategory {
    Attack,
    Spell,
}

#[derive(Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
enum WebFormationActionPolicy {
    Standard,
    PouchChain,
    EchoRingingMetal,
    EchoFallingWood,
    EchoFlowingWater,
    EchoWarFire,
    EchoSplitEarth,
    EchoPureFire,
    EchoPlantEarth,
}

impl WebFormationActionPolicy {
    fn from_id(id: &str) -> Self {
        match id {
            "pouch:chain" => Self::PouchChain,
            "echo:ringing-metal" => Self::EchoRingingMetal,
            "echo:falling-wood" => Self::EchoFallingWood,
            "echo:flowing-water" => Self::EchoFlowingWater,
            "echo:war-fire" => Self::EchoWarFire,
            "echo:split-earth" => Self::EchoSplitEarth,
            "echo:pure-fire" => Self::EchoPureFire,
            "echo:plant-earth" => Self::EchoPlantEarth,
            _ => Self::Standard,
        }
    }
}

impl From<FormationCategory> for WebFormationCategory {
    fn from(category: FormationCategory) -> Self {
        match category {
            FormationCategory::Attack => Self::Attack,
            FormationCategory::Spell => Self::Spell,
        }
    }
}

fn event_type(event: &PublicGameEvent) -> String {
    match event {
        PublicGameEvent::GamePreparationStarted => "GamePreparationStarted".to_string(),
        PublicGameEvent::InitialPouchChosen { .. } => "InitialPouchChosen".to_string(),
        PublicGameEvent::PouchPlaced { .. } => "PouchPlaced".to_string(),
        PublicGameEvent::Public(event) => format!("{event:?}")
            .split_whitespace()
            .next()
            .unwrap_or("Event")
            .trim_end_matches('{')
            .to_string(),
        PublicGameEvent::DeckPrepared { .. } => "DeckPrepared".to_string(),
        PublicGameEvent::PlayerDeckPrepared { .. } => "PlayerDeckPrepared".to_string(),
        PublicGameEvent::CardsDealt { .. } => "CardsDealt".to_string(),
        PublicGameEvent::PassiveCovered { .. } => "PassiveCovered".to_string(),
        PublicGameEvent::CardsDrawnForTurnDiscardChoice { .. } => {
            "CardsDrawnForTurnDiscardChoice".to_string()
        }
        PublicGameEvent::CardsDrawnForProfessionChoice { .. } => {
            "CardsDrawnForProfessionChoice".to_string()
        }
        PublicGameEvent::EffectChoiceRequested { .. } => "EffectChoiceRequested".to_string(),
        PublicGameEvent::RandomnessRequested { .. } => "RandomnessRequested".to_string(),
        PublicGameEvent::RandomnessResolved { .. } => "RandomnessResolved".to_string(),
        PublicGameEvent::HandInspected { .. } => "HandInspected".to_string(),
        PublicGameEvent::SpiritSkillUsed { .. } => "SpiritSkillUsed".to_string(),
        PublicGameEvent::SpiritLevelInterpreted { .. } => "SpiritLevelInterpreted".to_string(),
        PublicGameEvent::CardsMoved { .. } => "CardsMoved".to_string(),
        PublicGameEvent::FormationRequirementSet { .. } => "FormationRequirementSet".to_string(),
        PublicGameEvent::FormationRequirementFulfilled { .. } => {
            "FormationRequirementFulfilled".to_string()
        }
    }
}

#[cfg(test)]
fn event_presentation(
    event: &PublicGameEvent,
    labels: &HashMap<CardInstanceId, String>,
    formation_names: &HashMap<String, String>,
) -> (String, String) {
    event_presentation_with_vocabulary(
        event,
        labels,
        formation_names,
        &PlayerVocabulary::all_official(),
    )
}

fn event_presentation_with_vocabulary(
    event: &PublicGameEvent,
    labels: &HashMap<CardInstanceId, String>,
    formation_names: &HashMap<String, String>,
    vocabulary: &PlayerVocabulary,
) -> (String, String) {
    match event {
        PublicGameEvent::GamePreparationStarted => {
            ("錦囊準備".to_string(), "玩家開始選擇初始錦囊。".to_string())
        }
        PublicGameEvent::InitialPouchChosen { player } => (
            "選擇錦囊".to_string(),
            format!("{} 已完成錦囊選擇。", player.as_str()),
        ),
        PublicGameEvent::PouchPlaced { owner, card } => (
            "覆蓋錦囊".to_string(),
            card.map_or_else(
                || format!("{} 獲得一個覆蓋錦囊。", owner.as_str()),
                |card| {
                    format!(
                        "{} 的錦囊為 {}。",
                        owner.as_str(),
                        card_summary(&card, labels)
                    )
                },
            ),
        ),
        PublicGameEvent::CardsDealt { player, cards } => (
            "初始發牌".to_string(),
            format!(
                "{} 收到 {}。",
                player.as_str(),
                card_refs_summary(cards, labels)
            ),
        ),
        PublicGameEvent::CardsDrawnForProfessionChoice {
            player,
            ability_id: _,
            cards,
        } => (
            "職業能力抽牌".to_string(),
            format!(
                "{} 因職業能力抽取 {}。",
                player.as_str(),
                card_refs_summary(cards, labels)
            ),
        ),
        PublicGameEvent::FormationRequirementSet {
            player,
            virtual_card,
        } => (
            "陣法義務".to_string(),
            virtual_card.as_ref().map_or_else(
                || format!("{} 必須在本回合完成指定陣法。", player.as_str()),
                |card| {
                    format!(
                        "{} 建立了 {} {} 級虛擬牌。",
                        player.as_str(),
                        element_short_name(card.element),
                        card.level
                    )
                },
            ),
        ),
        PublicGameEvent::FormationRequirementFulfilled {
            player,
            formation_id,
            virtual_card,
        } => (
            "陣法義務完成".to_string(),
            virtual_card.as_ref().map_or_else(
                || format!("{} 已以 {} 完成陣法義務。", player.as_str(), formation_id),
                |card| {
                    format!(
                        "{} 以 {} {} 級虛擬牌完成 {}。",
                        player.as_str(),
                        element_short_name(card.element),
                        card.level,
                        formation_id
                    )
                },
            ),
        ),
        PublicGameEvent::DeckPrepared { deck } => (
            "準備牌庫".to_string(),
            format!("已準備 {}。", card_refs_summary(deck, labels)),
        ),
        PublicGameEvent::PlayerDeckPrepared { player, deck } => (
            "準備個人牌庫".to_string(),
            format!(
                "{} 已準備 {}。",
                player.as_str(),
                card_refs_summary(deck, labels)
            ),
        ),
        PublicGameEvent::Public(event) => {
            game_event_presentation_with_vocabulary(event, labels, formation_names, vocabulary)
        }
        PublicGameEvent::PassiveCovered {
            player,
            formation_id,
            cards,
            star_substitution: _,
        } => (
            "蓋牌".to_string(),
            formation_id.as_deref().map_or_else(
                || {
                    format!(
                        "{} 蓋下了 {}。",
                        player.as_str(),
                        card_refs_summary(cards, labels)
                    )
                },
                |formation_id| {
                    format!(
                        "{} 蓋下「{}」，使用 {}。",
                        player.as_str(),
                        formation_name(formation_names, formation_id),
                        card_refs_summary(cards, labels)
                    )
                },
            ),
        ),
        PublicGameEvent::CardsDrawnForTurnDiscardChoice {
            player,
            drawn_cards,
            ..
        } => (
            "回合抽牌".to_string(),
            format!(
                "{} 進行回合抽牌，抽取 {}，需選擇一張捨棄。",
                player.as_str(),
                card_refs_summary(drawn_cards, labels)
            ),
        ),
        PublicGameEvent::EffectChoiceRequested {
            player, purpose, ..
        } => (
            "效果選擇".to_string(),
            format!(
                "{} 需要為「{}」作出選擇。",
                player.as_str(),
                vocabulary.choice_purpose(purpose, formation_names)
            ),
        ),
        PublicGameEvent::RandomnessRequested {
            card_count,
            operation,
            ..
        } => (
            "等待洗牌".to_string(),
            match operation {
                crate::public_view::PublicRandomnessOperation::DeckShuffle => {
                    format!("正在洗牌組中的 {card_count} 張牌。")
                }
                crate::public_view::PublicRandomnessOperation::DiscardShuffle => {
                    format!("正在洗棄牌堆的 {card_count} 張牌並放回牌組。")
                }
            },
        ),
        PublicGameEvent::RandomnessResolved {
            card_count,
            operation,
            ..
        } => (
            "完成洗牌".to_string(),
            match operation {
                crate::public_view::PublicRandomnessOperation::DeckShuffle => {
                    format!("已洗牌組中的 {card_count} 張牌。")
                }
                crate::public_view::PublicRandomnessOperation::DiscardShuffle => {
                    format!("已洗棄牌堆的 {card_count} 張牌並放回牌組。")
                }
            },
        ),
        PublicGameEvent::HandInspected {
            viewer,
            target,
            cards,
        } => (
            "檢視手牌".to_string(),
            match cards {
                PublicCardRefs::Known(_) => format!(
                    "{} 檢視 {} 的手牌：{}。",
                    viewer.as_str(),
                    target.as_str(),
                    card_refs_summary(cards, labels)
                ),
                PublicCardRefs::Hidden { count } => format!(
                    "{} 檢視了 {} 的 {} 張手牌。",
                    viewer.as_str(),
                    target.as_str(),
                    count
                ),
                PublicCardRefs::PartiallyKnown { .. } => format!(
                    "{} 檢視 {} 的手牌：{}。",
                    viewer.as_str(),
                    target.as_str(),
                    card_refs_summary(cards, labels)
                ),
            },
        ),
        PublicGameEvent::SpiritSkillUsed {
            player,
            skill,
            old_power,
            new_power,
            selected_card,
            declared_level,
            ..
        } => (
            "使用精靈技能".to_string(),
            format!(
                "{} 使用「{}」，靈力由 {} 變為 {}{}{}。",
                player.as_str(),
                spirit_skill_name(*skill),
                old_power,
                new_power,
                selected_card
                    .map(|card| format!("，指定牌 {}", card_summary(&card, labels)))
                    .unwrap_or_default(),
                declared_level
                    .map(|level| format!("，宣告 {level} 級"))
                    .unwrap_or_default(),
            ),
        ),
        PublicGameEvent::SpiritLevelInterpreted {
            player,
            card,
            level,
            ..
        } => (
            "精靈改變等級".to_string(),
            card.map_or_else(
                || format!("{} 指定一張手牌本回合視為 {} 級。", player.as_str(), level),
                |card| {
                    format!(
                        "{} 指定牌 {} 本回合視為 {} 級。",
                        player.as_str(),
                        card_summary(&card, labels),
                        level
                    )
                },
            ),
        ),
        PublicGameEvent::CardsMoved { cards } => {
            ("卡牌移動".to_string(), card_movement_summary(cards, labels))
        }
    }
}

#[cfg(test)]
fn game_event_presentation(
    event: &GameEvent,
    labels: &HashMap<CardInstanceId, String>,
    formation_names: &HashMap<String, String>,
) -> (String, String) {
    game_event_presentation_with_vocabulary(
        event,
        labels,
        formation_names,
        &PlayerVocabulary::all_official(),
    )
}

fn game_event_presentation_with_vocabulary(
    event: &GameEvent,
    labels: &HashMap<CardInstanceId, String>,
    formation_names: &HashMap<String, String>,
    vocabulary: &PlayerVocabulary,
) -> (String, String) {
    match event {
        GameEvent::GamePreparationStarted { .. } => {
            ("錦囊準備".to_string(), "玩家開始選擇初始錦囊。".to_string())
        }
        GameEvent::InitialPouchChosen { player, .. } => (
            "選擇錦囊".to_string(),
            format!("{} 已選擇初始錦囊。", player.as_str()),
        ),
        GameEvent::GamePreparationCompleted => (
            "準備完成".to_string(),
            "錦囊、洗牌與初始發牌已完成。".to_string(),
        ),
        GameEvent::PouchPlaced { owner, .. } => (
            "覆蓋錦囊".to_string(),
            format!("{} 獲得一個錦囊。", owner.as_str()),
        ),
        GameEvent::PouchRevealed {
            player, strategy, ..
        } => (
            "觸發秘計".to_string(),
            format!(
                "{} 觸發「{}」。",
                player.as_str(),
                secret_strategy_name(*strategy)
            ),
        ),
        GameEvent::PouchConsumed { .. } => {
            ("錦囊捨棄".to_string(), "秘計來源牌已捨棄。".to_string())
        }
        GameEvent::PouchLevelBonusGranted { bonus } => (
            "秘計‧偷梁".to_string(),
            format!("{} 的既有手牌等級提升。", bonus.player.as_str()),
        ),
        GameEvent::TemporaryStarEffectGranted { effect } => (
            "秘計‧瞞天".to_string(),
            format!(
                "{} 暫時獲得{}效果。",
                effect.player.as_str(),
                star_name(effect.star)
            ),
        ),
        GameEvent::SpiritRevived { player, spirit, .. } => (
            "秘計‧還魂".to_string(),
            format!("{} 召喚{}精靈。", player.as_str(), spirit_name(*spirit)),
        ),
        GameEvent::DeckPrepared { deck_order } => (
            "準備牌庫".to_string(),
            format!("已準備 {} 張牌。", deck_order.len()),
        ),
        GameEvent::PlayerDeckPrepared { player, deck_order } => (
            "準備個人牌庫".to_string(),
            format!("{} 已準備 {} 張牌。", player.as_str(), deck_order.len()),
        ),
        GameEvent::CardsDealt { player, cards } => (
            "初始發牌".to_string(),
            format!("{} 收到 {} 張牌。", player.as_str(), cards.len()),
        ),
        GameEvent::TurnStarted {
            player,
            turn_number,
        } if *turn_number == 1 => (
            "對局開始".to_string(),
            format!("已完成洗牌與發牌，{} 先手。", player.as_str()),
        ),
        GameEvent::TurnStarted {
            player,
            turn_number,
        } => (
            "回合開始".to_string(),
            format!("第 {turn_number} 回合由 {} 行動。", player.as_str()),
        ),
        GameEvent::ActionPassed { player, reason } => (
            "跳過行動".to_string(),
            format!(
                "{} 因{}而跳過行動。",
                player.as_str(),
                match reason {
                    PassActionReason::NoCardsInHand => "手中沒有牌",
                    PassActionReason::CannotActByStatus => "目前狀態無法行動",
                }
            ),
        ),
        GameEvent::ProfessionChanged {
            player,
            previous,
            profession,
            ..
        } => (
            "轉職".to_string(),
            format!(
                "{} 由{}轉職為{}。",
                player.as_str(),
                vocabulary.previous_profession(previous.as_ref()),
                vocabulary.profession(profession)
            ),
        ),
        GameEvent::ProfessionTransformed {
            player,
            previous,
            profession,
            reason,
            ..
        } => (
            "職業轉化".to_string(),
            format!(
                "{} 因「{}」由{}轉化為{}。",
                player.as_str(),
                vocabulary.reason(reason),
                vocabulary.previous_profession(previous.as_ref()),
                vocabulary.profession(profession)
            ),
        ),
        GameEvent::ProfessionBroken { player, profession } => (
            "職業破除".to_string(),
            format!(
                "{} 的{}已被破除。",
                player.as_str(),
                vocabulary.profession(profession)
            ),
        ),
        GameEvent::ProfessionAbilityActivated {
            player,
            ability_id,
            prepared,
        } => (
            "發動職業能力".to_string(),
            prepared.as_ref().map_or_else(
                || {
                    format!(
                        "{} 發動了「{}」。",
                        player.as_str(),
                        vocabulary.ability(ability_id)
                    )
                },
                |prepared| {
                    format!(
                        "{} 發動「{}」，將牌 {} 準備為 {} {} 級。",
                        player.as_str(),
                        vocabulary.ability(ability_id),
                        card_summary(&prepared.card, labels),
                        element_short_name(prepared.element),
                        prepared.level
                    )
                },
            ),
        ),
        GameEvent::FormationRequirementSet { requirement } => (
            "陣法義務".to_string(),
            format!(
                "{} 必須在本回合完成已準備的陣法。",
                requirement.player.as_str()
            ),
        ),
        GameEvent::FormationRequirementFulfilled {
            player,
            formation_id,
            ..
        } => (
            "陣法義務完成".to_string(),
            format!("{} 已以 {} 完成陣法義務。", player.as_str(), formation_id),
        ),
        GameEvent::SpiritSummoned {
            player,
            previous,
            spirit,
        } => (
            "召喚精靈".to_string(),
            if let Some(previous) = previous {
                format!(
                    "{} 的{}精靈被{}精靈取代，靈力為 2。",
                    player.as_str(),
                    spirit_name(*previous),
                    spirit_name(*spirit)
                )
            } else {
                format!(
                    "{} 召喚{}精靈，靈力為 2。",
                    player.as_str(),
                    spirit_name(*spirit)
                )
            },
        ),
        GameEvent::SpiritTransformed {
            player,
            previous,
            spirit,
            power,
        } => (
            "魔靈附體".to_string(),
            format!(
                "{} 的{}精靈轉化為{}精靈，保留 {power} 點靈力。",
                player.as_str(),
                spirit_name(*previous),
                spirit_name(*spirit)
            ),
        ),
        GameEvent::SpiritPowerChanged {
            player,
            spirit,
            old_power,
            new_power,
            ..
        } => (
            "精靈靈力增加".to_string(),
            format!(
                "{} 的{}精靈靈力由 {} 增加為 {}。",
                player.as_str(),
                spirit_name(*spirit),
                old_power,
                new_power
            ),
        ),
        GameEvent::SpiritSkillUsed {
            player,
            skill,
            old_power,
            new_power,
            ..
        } => (
            "使用精靈技能".to_string(),
            format!(
                "{} 使用「{}」，靈力由 {} 變為 {}。",
                player.as_str(),
                spirit_skill_name(*skill),
                old_power,
                new_power
            ),
        ),
        GameEvent::SpiritLevelInterpreted {
            player,
            card,
            level,
            ..
        } => (
            "精靈改變等級".to_string(),
            format!(
                "{} 指定牌 {} 本回合視為 {} 級。",
                player.as_str(),
                card_summary(card, labels),
                level
            ),
        ),
        GameEvent::SpiritBroken { player, spirit, .. } => (
            "精靈破除".to_string(),
            format!("{} 的{}精靈已破除。", player.as_str(), spirit_name(*spirit)),
        ),
        GameEvent::AutomaticBloomsResolved { resolutions } => (
            "自動綻放".to_string(),
            format!(
                "{}。",
                resolutions
                    .iter()
                    .map(|resolution| format!(
                        "{} 有 {} 個木精靈綻放，生命值 {} → {}",
                        resolution.team.as_str(),
                        resolution.spirit_changes.len(),
                        resolution.hp_change.old_hp,
                        resolution.hp_change.new_hp
                    ))
                    .collect::<Vec<_>>()
                    .join("；")
            ),
        ),
        GameEvent::CardsDrawnForProfessionChoice { .. } => (
            "職業能力抽牌".to_string(),
            "已抽取職業能力指定的牌。".to_string(),
        ),
        GameEvent::CardsDrawnForTurnDiscardChoice {
            player,
            drawn_cards,
            ..
        } => (
            "回合抽牌".to_string(),
            format!(
                "{} 進行回合抽牌，抽取 {}，需選擇一張捨棄。",
                player.as_str(),
                cards_summary(drawn_cards, labels)
            ),
        ),
        GameEvent::TurnDiscardChosen { player, discard } => (
            "捨棄".to_string(),
            format!(
                "{} 捨棄了 {}。",
                player.as_str(),
                card_summary(discard, labels)
            ),
        ),
        GameEvent::TurnDrawSkipped { player, reason } => (
            "略過抽牌".to_string(),
            format!(
                "{} 因{}而未抽牌。",
                player.as_str(),
                match reason {
                    TurnDrawSkipReason::HandLimitReached => "手牌已達上限",
                    TurnDrawSkipReason::CannotDrawByStatus => "目前狀態無法抽牌",
                }
            ),
        ),
        GameEvent::FormationPerformed {
            player,
            formation_id,
            used_cards,
            ..
        } => (
            "發動陣法".to_string(),
            format!(
                "{} 發動「{}」，使用 {}。",
                player.as_str(),
                formation_name(formation_names, formation_id),
                cards_summary(used_cards, labels)
            ),
        ),
        GameEvent::FormationMatchOptionDeclared {
            player,
            formation_id,
            ..
        } => (
            "陣法解釋".to_string(),
            format!(
                "{} 已指定「{}」的組成方式。",
                player.as_str(),
                formation_name(formation_names, formation_id)
            ),
        ),
        GameEvent::FormationEffectCopied { player, effect_id } => (
            "幻化".to_string(),
            format!(
                "{} 的幻化複製了「{}」。",
                player.as_str(),
                formation_name(formation_names, effect_id)
            ),
        ),
        GameEvent::FormationEffectIgnored {
            player,
            formation_id,
            reason: crate::domain::FormationNoEffectReason::IneffectiveInEnvironment { environment },
        } => (
            "陣法無效".to_string(),
            format!(
                "{} 的「{}」因{}而無效。",
                player.as_str(),
                formation_name(formation_names, formation_id),
                element_name(*environment),
            ),
        ),
        GameEvent::FormationEffectIgnored {
            player,
            formation_id,
            reason: crate::domain::FormationNoEffectReason::SuppressedBySplitEarth { .. },
        } => (
            "陣法無效".to_string(),
            format!(
                "{} 的「{}」受裂土影響而無效。",
                player.as_str(),
                formation_name(formation_names, formation_id),
            ),
        ),
        GameEvent::CounterEffectEstablished { owner, effect_id } => (
            "建立反制".to_string(),
            format!(
                "{} 建立了公開的「{}」效果。",
                owner.as_str(),
                formation_name(formation_names, effect_id)
            ),
        ),
        GameEvent::CounterEffectResolved {
            owner, effect_id, ..
        } => (
            "反制發動".to_string(),
            format!(
                "{} 的「{}」已發動。",
                owner.as_str(),
                formation_name(formation_names, effect_id)
            ),
        ),
        GameEvent::HandInspected { viewer, target, .. } => (
            "檢視手牌".to_string(),
            format!("{} 檢視了 {} 的手牌。", viewer.as_str(), target.as_str()),
        ),
        GameEvent::DeckTopRevealed { player, card } => (
            "晴風".to_string(),
            format!(
                "{} 展示牌堆最上方的 {}。",
                player.as_str(),
                cards_summary(&[*card], labels)
            ),
        ),
        GameEvent::AttackResolved {
            attacker,
            target,
            formation_id,
            hp_change,
            shield_change,
            ..
        } => {
            let result = shield_change.as_ref().map_or_else(
                || format!("生命值由 {} 變為 {}", hp_change.old_hp, hp_change.new_hp),
                |change| {
                    format!(
                        "{} 的防護罩由 {} 變為 {}",
                        target.as_str(),
                        change.old_value,
                        change.new_value
                    )
                },
            );

            (
                "攻擊結算".to_string(),
                format!(
                    "{} 以「{}」攻擊 {}，{}。",
                    attacker.as_str(),
                    formation_name(formation_names, formation_id),
                    target.as_str(),
                    result
                ),
            )
        }
        GameEvent::TurnDrawBonusChanged {
            player,
            old_value,
            new_value,
            ..
        } => (
            "抽牌調整".to_string(),
            format!(
                "{} 的額外抽牌數由 {old_value} 變為 {new_value}。",
                player.as_str()
            ),
        ),
        GameEvent::ShieldChanged {
            player,
            old_value,
            new_value,
            ..
        } => (
            "防護罩變化".to_string(),
            format!(
                "{} 的防護罩由 {old_value} 變為 {new_value}。",
                player.as_str()
            ),
        ),
        GameEvent::HpChanged { change } => (
            "生命變化".to_string(),
            format!("隊伍生命值由 {} 變為 {}。", change.old_hp, change.new_hp),
        ),
        GameEvent::CardsMoved { card_moves } => (
            "卡牌移動".to_string(),
            format!("有 {} 張牌移動到新的區域。", card_moves.len()),
        ),
        GameEvent::StatusAdded { .. } => {
            ("狀態生效".to_string(), "新的狀態效果已生效。".to_string())
        }
        GameEvent::StatusExpired { .. } => {
            ("狀態結束".to_string(), "一個狀態效果已到期。".to_string())
        }
        GameEvent::StatusRemoved { .. } => {
            ("狀態解除".to_string(), "一個狀態效果已解除。".to_string())
        }
        GameEvent::JianghuStateApplied { state } => (
            "江湖狀態生效".to_string(),
            format!(
                "{} 進入{}狀態。",
                state.owner.as_str(),
                jianghu_state_name(state.kind)
            ),
        ),
        GameEvent::JianghuStateExpired { owner, kind } => (
            "江湖狀態結束".to_string(),
            format!(
                "{} 的{}狀態已結束。",
                owner.as_str(),
                jianghu_state_name(*kind)
            ),
        ),
        GameEvent::JianghuPoisonTicked {
            owner,
            damage,
            remaining_turns,
            ..
        } => (
            "中毒".to_string(),
            format!(
                "{} 因中毒扣除 {damage} 點生命，剩餘 {remaining_turns} 回合。",
                owner.as_str()
            ),
        ),
        GameEvent::JianghuDelayedDamageResolved {
            owner, hp_change, ..
        } => (
            "天外飛扇".to_string(),
            format!(
                "{} 行動後扣除 {} 點生命。",
                owner.as_str(),
                -hp_change.effective_delta
            ),
        ),
        GameEvent::LimitedUseChanged {
            owner,
            key,
            old_remaining,
            new_remaining,
            maximum,
            ..
        } if key == crate::rules::confluence::TAILWIND_USE
            && *old_remaining == 0
            && *new_remaining == *maximum =>
        {
            (
                "順風回復".to_string(),
                format!(
                    "洗棄牌完成，{} 的順風回復為 {new_remaining}/{maximum} 次。",
                    owner.as_str()
                ),
            )
        }
        GameEvent::LimitedUseChanged {
            owner,
            key,
            new_remaining,
            maximum,
            ..
        } => (
            "次數限制".to_string(),
            format!(
                "{} 的{}剩餘 {new_remaining}/{maximum} 次。",
                owner.as_str(),
                vocabulary.limited_use(key)
            ),
        ),
        GameEvent::ConfluenceCardObligationSet { obligation } => (
            "調律".to_string(),
            format!(
                "{} 取得的牌須於本回合依調律限制使用。",
                obligation.owner.as_str()
            ),
        ),
        GameEvent::ConfluenceCardObligationCleared { owner, .. } => (
            "調律完成".to_string(),
            format!("{} 已完成調律牌義務。", owner.as_str()),
        ),
        GameEvent::EffectChoiceRequested { player, .. } => (
            "效果選擇".to_string(),
            format!("{} 需要選擇效果。", player.as_str()),
        ),
        GameEvent::EffectChoiceAnswered {
            player,
            selected_cards,
            ..
        } => (
            "完成選擇".to_string(),
            format!(
                "{} 已選擇 {}。",
                player.as_str(),
                cards_summary(selected_cards, labels)
            ),
        ),
        GameEvent::TypedEffectChoiceAnswered { player, answer, .. } => {
            let selection = match answer {
                crate::domain::EffectChoiceAnswer::Cards { cards } => cards_summary(cards, labels),
                crate::domain::EffectChoiceAnswer::Player { player } => {
                    format!("玩家 {}", player.as_str())
                }
                crate::domain::EffectChoiceAnswer::Formation { formation_id } => {
                    format!("陣法 {}", formation_name(formation_names, formation_id))
                }
                crate::domain::EffectChoiceAnswer::Environment { environment } => {
                    element_name(*environment).to_string()
                }
                crate::domain::EffectChoiceAnswer::Chain { .. } => "連環選擇".to_string(),
                crate::domain::EffectChoiceAnswer::SheepStealing { .. } => "牽羊交換".to_string(),
                crate::domain::EffectChoiceAnswer::Decline => "放棄".to_string(),
            };
            (
                "完成選擇".to_string(),
                format!("{} 選擇了 {}。", player.as_str(), selection),
            )
        }
        GameEvent::RandomnessRequested { request } => (
            "等待洗牌".to_string(),
            format!("正在重新排列 {} 張牌。", request.current_order.len()),
        ),
        GameEvent::RandomnessResolved { shuffled_order, .. } => (
            "完成洗牌".to_string(),
            format!("已重新排列 {} 張牌。", shuffled_order.len()),
        ),
        GameEvent::PassiveCovered { player, cards, .. } => (
            "蓋牌".to_string(),
            format!("{} 蓋下了 {} 張牌。", player.as_str(), cards.len()),
        ),
        GameEvent::PassiveCoverRevealed { owner } => (
            "蓋牌公開".to_string(),
            format!("{} 的蓋牌改為正面展示。", owner.as_str()),
        ),
        GameEvent::PassiveFlipped {
            owner, passive_id, ..
        } => {
            let detail = if passive_id == "empty-city" {
                format!("{} 的「空城」翻開。", owner.as_str())
            } else {
                format!(
                    "{} 的「{}」已翻開並完成結算。",
                    owner.as_str(),
                    formation_name(formation_names, passive_id)
                )
            };
            ("蓋牌翻開".to_string(), detail)
        }
        GameEvent::DiscardRetrieved {
            player,
            previous_player,
            card,
            hp_change,
            ..
        } => (
            "棄牌回收".to_string(),
            format!(
                "{} 支付生命值（{} → {}），回收 {} 的 {}。",
                player.as_str(),
                hp_change.old_hp,
                hp_change.new_hp,
                previous_player.as_str(),
                card_summary(card, labels)
            ),
        ),
        GameEvent::EnvironmentTransferred {
            player,
            formation_id,
            from,
            to,
        } => (
            "環境轉移".to_string(),
            format!(
                "{} 的「{}」將環境由{}轉移為{}。",
                player.as_str(),
                formation_name(formation_names, formation_id),
                from.map(element_name).unwrap_or("無環境"),
                element_name(*to),
            ),
        ),
        GameEvent::EnvironmentCleared {
            player,
            formation_id,
            environment,
            hp_changes,
        } => (
            "環境破除".to_string(),
            format!(
                "{} 的「{}」破除{}，{}。",
                player.as_str(),
                formation_name(formation_names, formation_id),
                element_name(*environment),
                hp_changes
                    .iter()
                    .map(|change| format!(
                        "{} 生命值 {} → {}",
                        change.team.as_str(),
                        change.old_hp,
                        change.new_hp
                    ))
                    .collect::<Vec<_>>()
                    .join("、"),
            ),
        ),
        GameEvent::StarBroken {
            team,
            star,
            hp_change,
            ..
        } => (
            "星辰破除".to_string(),
            hp_change.as_ref().map_or_else(
                || format!("{} 的{}已被破除。", team.as_str(), star_name(*star)),
                |change| {
                    format!(
                        "{} 的{}已被破除，生命值 {} → {}。",
                        team.as_str(),
                        star_name(*star),
                        change.old_hp,
                        change.new_hp
                    )
                },
            ),
        ),
        GameEvent::StarSummoned { player, star, .. } => (
            "召喚星辰".to_string(),
            format!("{} 召喚了{}。", player.as_str(), star_name(*star)),
        ),
        GameEvent::VoidStarBreakingCompleted { player } => (
            "破星結算".to_string(),
            format!("{} 的虛空破星術已完成結算。", player.as_str()),
        ),
        GameEvent::VoidReversionResolved {
            player,
            broken_professions,
            retained_legendary_professions,
            ..
        } => (
            "虛空返璞".to_string(),
            format!(
                "{} 破除 {} 個職業，保留 {} 個低等級保護的傳說職業。",
                player.as_str(),
                broken_professions.len(),
                retained_legendary_professions.len()
            ),
        ),
        GameEvent::VoidSpiritShatteringResolved {
            player,
            spirit_changes,
            broken_spirits,
            hp_changes,
            ..
        } => (
            "虛空碎靈".to_string(),
            format!(
                "{} 使 {} 個精靈靈力下降、破除 {} 個精靈；{}。",
                player.as_str(),
                spirit_changes.len(),
                broken_spirits.len(),
                hp_changes
                    .iter()
                    .map(|change| format!(
                        "{} 生命值 {} → {}",
                        change.team.as_str(),
                        change.old_hp,
                        change.new_hp
                    ))
                    .collect::<Vec<_>>()
                    .join("、")
            ),
        ),
        GameEvent::FiveStarAlignmentAchieved { player, .. } => (
            "五星連珠".to_string(),
            format!("{} 完成五星連珠，所屬隊伍獲勝。", player.as_str()),
        ),
        GameEvent::EchoCostPaid { player, .. } => (
            "支付迴響代價".to_string(),
            format!("{} 捨棄一張牌並排定迴響。", player.as_str()),
        ),
        GameEvent::EchoDeclined { player, .. } => (
            "放棄迴響".to_string(),
            format!("{} 選擇不觸發迴響。", player.as_str()),
        ),
        GameEvent::EchoScheduled { schedule } => (
            "排定迴響".to_string(),
            format!(
                "{} 的曲調將於第 {} 回合開始時迴響。",
                schedule.player.as_str(),
                schedule.due_turn_number
            ),
        ),
        GameEvent::EchoResolutionStarted { schedule } => (
            "迴響開始".to_string(),
            format!("{} 開始執行曲調迴響。", schedule.player.as_str()),
        ),
        GameEvent::EchoResolutionCompleted { player, .. } => (
            "迴響完成".to_string(),
            format!("{} 已完成曲調迴響。", player.as_str()),
        ),
        GameEvent::TimedEffectsReduced {
            source,
            target,
            reductions,
        } => (
            "淨火".to_string(),
            format!(
                "{} 使 {} 的 {} 個合格時效效果減少一回合或一層。",
                source.as_str(),
                target.as_str(),
                reductions.len()
            ),
        ),
        GameEvent::FlowStateChanged {
            player, new_layers, ..
        } => (
            "流水狀態".to_string(),
            format!("{} 的流水狀態為 {} 層。", player.as_str(), new_layers),
        ),
        GameEvent::FlowStateTriggered { player, .. } => (
            "流水觸發".to_string(),
            format!("{} 消耗一層流水，使本回合抽牌＋１。", player.as_str()),
        ),
        GameEvent::FormationSuppressionSet { suppression } => (
            "裂土指定".to_string(),
            format!(
                "{} 指定 {} 的陣法「{}」於下回合無效。",
                suppression.source.as_str(),
                suppression.target.as_str(),
                formation_name(formation_names, &suppression.formation_id)
            ),
        ),
        GameEvent::FormationSuppressionExpired {
            target,
            formation_id,
            ..
        } => (
            "裂土結束".to_string(),
            format!(
                "{} 的陣法「{}」不再受裂土影響。",
                target.as_str(),
                formation_name(formation_names, formation_id)
            ),
        ),
        GameEvent::RingingMetalCardRevealed { selection } => (
            "鳴金檢索".to_string(),
            format!(
                "{} 展示了牌 {}。",
                selection.player.as_str(),
                card_summary(&selection.card, labels)
            ),
        ),
        GameEvent::RingingMetalCompleted { selection } => (
            "鳴金完成".to_string(),
            format!("{} 將展示牌放到牌組最上方。", selection.player.as_str()),
        ),
        GameEvent::PlantEarthScheduled { schedule } => (
            "植土排定".to_string(),
            format!(
                "{} 將於第 {} 回合開始選擇曲調主效果。",
                schedule.player.as_str(),
                schedule.due_turn_number
            ),
        ),
        GameEvent::PlantEarthResolutionStarted { schedule } => (
            "植土開始".to_string(),
            format!("{} 開始結算植土。", schedule.player.as_str()),
        ),
        GameEvent::PlantEarthResolutionCompleted {
            player, melody_id, ..
        } => (
            "植土完成".to_string(),
            format!(
                "{} 已執行曲調「{}」的主效果。",
                player.as_str(),
                formation_name(formation_names, melody_id)
            ),
        ),
        GameEvent::EarthRendingStarted { resolution } => (
            "裂地崩山".to_string(),
            format!("{} 開始選擇裂地崩山的環境。", resolution.attacker.as_str()),
        ),
        GameEvent::EarthRendingEnvironmentChosen { environment } => (
            "選擇環境".to_string(),
            format!("裂地崩山選擇了 {}。", element_name(*environment)),
        ),
        GameEvent::EarthRendingPlayerAnswered { answer } => (
            "裂地崩山選擇".to_string(),
            if answer.protected {
                format!("{} 受神算保護。", answer.player.as_str())
            } else if answer.card.is_some() {
                format!("{} 選擇捨棄一張環行牌。", answer.player.as_str())
            } else {
                format!("{} 沒有環行牌並展示手牌。", answer.player.as_str())
            },
        ),
        GameEvent::HandRevealed { player, cards } => (
            "展示手牌".to_string(),
            format!(
                "{} 展示了 {}。",
                player.as_str(),
                cards_summary(cards, labels)
            ),
        ),
        GameEvent::EarthRendingCompleted { player } => (
            "裂地崩山完成".to_string(),
            format!("{} 完成裂地崩山。", player.as_str()),
        ),
        GameEvent::RustedForestStarted { resolution } => (
            "鏽鐵枯林".to_string(),
            format!("{} 開始處理鏽鐵枯林。", resolution.attacker.as_str()),
        ),
        GameEvent::RustedForestCardsRevealed { cards, .. } => (
            "鏽鐵枯林展示".to_string(),
            format!("展示了 {}。", cards_summary(cards, labels)),
        ),
        GameEvent::RustedForestDeckProcessed { .. } => (
            "鏽鐵枯林洗牌".to_string(),
            "已完成一個牌組的處理。".to_string(),
        ),
        GameEvent::RustedForestCompleted { player } => (
            "鏽鐵枯林完成".to_string(),
            format!("{} 完成鏽鐵枯林。", player.as_str()),
        ),
        GameEvent::TurnEnded { player } => (
            "回合結束".to_string(),
            format!("{} 的回合結束。", player.as_str()),
        ),
    }
}

fn star_name(star: crate::domain::StarKind) -> &'static str {
    crate::rules::star::star_name(star)
}

fn element_name(element: crate::domain::Element) -> &'static str {
    match element {
        crate::domain::Element::Metal => "金行環境",
        crate::domain::Element::Wood => "木行環境",
        crate::domain::Element::Water => "水行環境",
        crate::domain::Element::Fire => "火行環境",
        crate::domain::Element::Earth => "土行環境",
    }
}

fn element_short_name(element: crate::domain::Element) -> &'static str {
    match element {
        crate::domain::Element::Metal => "金",
        crate::domain::Element::Wood => "木",
        crate::domain::Element::Water => "水",
        crate::domain::Element::Fire => "火",
        crate::domain::Element::Earth => "土",
    }
}

fn spirit_name(spirit: crate::domain::SpiritKind) -> &'static str {
    match spirit {
        crate::domain::SpiritKind::Metal => "金",
        crate::domain::SpiritKind::Wood => "木",
        crate::domain::SpiritKind::Water => "水",
        crate::domain::SpiritKind::Fire => "火",
        crate::domain::SpiritKind::Earth => "土",
        crate::domain::SpiritKind::Evil => "惡",
        crate::domain::SpiritKind::Death => "死",
    }
}

fn spirit_skill_name(skill: crate::domain::SpiritSkill) -> &'static str {
    match skill {
        crate::domain::SpiritSkill::FlyingBlade => "飛刃",
        crate::domain::SpiritSkill::SwordRain => "劍雨",
        crate::domain::SpiritSkill::Fragrance => "芬芳",
        crate::domain::SpiritSkill::Bloom => "綻放",
        crate::domain::SpiritSkill::Flow => "川流",
        crate::domain::SpiritSkill::Vastness => "浩瀚",
        crate::domain::SpiritSkill::Glimmer => "螢光",
        crate::domain::SpiritSkill::Splendor => "絢爛",
        crate::domain::SpiritSkill::StoneShield => "石盾",
        crate::domain::SpiritSkill::RockWall => "岩壁",
        crate::domain::SpiritSkill::EvilGaze => "惡視",
        crate::domain::SpiritSkill::DeathOmen => "死兆",
    }
}

fn secret_strategy_name(strategy: SecretStrategy) -> &'static str {
    match strategy {
        SecretStrategy::GoldenCicada => "金蟬脫殼",
        SecretStrategy::StealTheBeam => "偷梁換柱",
        SecretStrategy::MuddyWaters => "混水摸魚",
        SecretStrategy::WatchTheFire => "隔岸觀火",
        SecretStrategy::LureTheTigerAway => "調虎離山",
        SecretStrategy::ReturnSoul => "借屍還魂",
        SecretStrategy::SheepStealing => "順手牽羊",
        SecretStrategy::DarkCrossing => "暗渡陳倉",
        SecretStrategy::DeceiveHeaven => "瞞天過海",
        SecretStrategy::Retreat => "急流勇退",
    }
}

fn jianghu_state_name(kind: crate::domain::JianghuStateKind) -> &'static str {
    match kind {
        crate::domain::JianghuStateKind::ThousandBlades => "千鋒",
        crate::domain::JianghuStateKind::SnowTreading => "踏雪",
        crate::domain::JianghuStateKind::Poison => "中毒",
    }
}

fn formation_name(formation_names: &HashMap<String, String>, formation_id: &str) -> String {
    formation_names
        .get(formation_id)
        .cloned()
        .unwrap_or_else(|| "未知陣法".to_string())
}

fn card_summary(card: &CardInstanceId, labels: &HashMap<CardInstanceId, String>) -> String {
    labels
        .get(card)
        .cloned()
        .unwrap_or_else(|| "一張牌".to_string())
}

fn cards_summary(cards: &[CardInstanceId], labels: &HashMap<CardInstanceId, String>) -> String {
    if cards.is_empty() {
        return "0 張牌".to_string();
    }

    cards
        .iter()
        .map(|card| card_summary(card, labels))
        .collect::<Vec<_>>()
        .join("、")
}

fn card_refs_summary(cards: &PublicCardRefs, labels: &HashMap<CardInstanceId, String>) -> String {
    match cards {
        PublicCardRefs::Known(cards) => cards
            .iter()
            .map(|card| card_summary(card, labels))
            .collect::<Vec<_>>()
            .join("、"),
        PublicCardRefs::Hidden { count } => format!("{count} 張牌"),
        PublicCardRefs::PartiallyKnown { cards } => {
            let known = cards.iter().flatten().count();
            format!("{} 張牌（其中 {known} 張公開）", cards.len())
        }
    }
}

fn card_movement_summary(
    cards: &PublicCardRefs,
    labels: &HashMap<CardInstanceId, String>,
) -> String {
    match cards {
        PublicCardRefs::Known(cards) => {
            format!("{} 移動到新的區域。", cards_summary(cards, labels))
        }
        PublicCardRefs::Hidden { count } => {
            format!("有 {count} 張牌移動到新的區域。")
        }
        PublicCardRefs::PartiallyKnown { cards } => {
            let known = cards.iter().flatten().copied().collect::<Vec<_>>();
            let hidden = cards.len() - known.len();
            format!(
                "{} 與 {hidden} 張未公開牌移動到新的區域。",
                cards_summary(&known, labels)
            )
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
enum ApiError {
    Game(GameError),
    Message(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expect_ready(result: ApiResult) -> ApiResponse {
        match result {
            ApiResult::Ready { response } => response,
            ApiResult::NeedsRandomness { request, .. } => {
                panic!("expected ready response, got randomness request {request:?}")
            }
            ApiResult::ReplayFrame { .. } => panic!("expected ready response, got replay frame"),
        }
    }

    #[test]
    fn start_request_returns_default_game_state() {
        let response = handle_request_json(r#"{"action":{"type":"start"},"viewer":"alice"}"#)
            .expect("start request should succeed");
        let json: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(json["type"], "ready");
        assert_eq!(json["state"]["turnNumber"], 1);
        assert!(json["record"].is_array());
        assert!(json.get("request").is_none());
    }

    #[test]
    fn replay_frame_dto_uses_camel_case_contract_fields() {
        let response = handle_request_json(
            r#"{"action":{"type":"replayFrame","step":0},"viewer":"replay","record":[]}"#,
        )
        .expect("replay frame should serialize");
        let json: serde_json::Value = serde_json::from_str(&response).unwrap();
        assert_eq!(json["type"], "replayFrame");
        assert!(json.get("currentStep").is_some());
        assert!(json.get("totalSteps").is_some());
        assert!(json.get("current_step").is_none());
        assert!(json.get("total_steps").is_none());
    }

    #[test]
    fn retrieving_the_only_discard_auto_passes_a_cannot_act_player() {
        let rules = OfficialRules::new();
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let enabled_rule_modules = [DISCARD_RETRIEVAL_MODULE_ID];
        let setup = rules
            .configure_game(
                vec![
                    Player {
                        id: alice.clone(),
                        team: TeamId::new("team:alice"),
                    },
                    Player {
                        id: bob.clone(),
                        team: TeamId::new("team:bob"),
                    },
                ],
                vec![alice.clone(), bob.clone()],
                enabled_rule_modules
                    .iter()
                    .map(|module| RuleModuleId::new(*module))
                    .collect(),
            )
            .unwrap();
        let web_setup = || WebGameSetup {
            players: vec![
                WebSetupPlayer {
                    id: "alice".to_string(),
                    team: "team:alice".to_string(),
                },
                WebSetupPlayer {
                    id: "bob".to_string(),
                    team: "team:bob".to_string(),
                },
            ],
            turn_order: vec!["alice".to_string(), "bob".to_string()],
            enabled_rule_modules: enabled_rule_modules
                .iter()
                .map(|module| (*module).to_string())
                .collect(),
            deck_lists: Vec::new(),
            initial_hp: Vec::new(),
        };
        let mut used = Vec::new();
        let radiance_cards = [
            (Element::Metal, 1),
            (Element::Metal, 2),
            (Element::Fire, 1),
            (Element::Water, 1),
        ]
        .into_iter()
        .map(|(element, level)| {
            let card = setup
                .card_instances
                .iter()
                .map(|instance| instance.instance)
                .find(|card| {
                    !used.contains(card)
                        && setup
                            .card_instances
                            .iter()
                            .find(|instance| instance.instance == *card)
                            .and_then(|instance| {
                                setup
                                    .card_defs
                                    .iter()
                                    .find(|definition| definition.id == instance.definition)
                            })
                            .is_some_and(|definition| {
                                definition.element == element && definition.level == level
                            })
                })
                .expect("official deck must contain the Radiance cards");
            used.push(card);
            card
        })
        .collect::<Vec<_>>();
        let mut deck_order = radiance_cards.clone();
        deck_order.extend(
            rules
                .official_deck_order(&setup)
                .unwrap()
                .into_iter()
                .filter(|card| !radiance_cards.contains(card)),
        );
        let mut record = GameRecord::start(setup, deck_order).unwrap();
        advance_to_interactive_decision(&mut record).unwrap();
        assert_eq!(record.state().hand(&alice), Some(radiance_cards.as_slice()));

        let after_formation = expect_ready(
            handle(ApiRequest {
                action: ApiAction::PerformFormation {
                    player: "alice".to_string(),
                    formation_id: "radiance".to_string(),
                    cards: radiance_cards,
                    star_substitution_card: None,
                    match_option_role: None,
                    match_option_card: None,
                    match_option_slots: None,
                    trusted_random_cards: None,
                },
                viewer: Some("alice".to_string()),
                record: Some(record.recorded_decisions()),
                setup: Some(web_setup()),
                first_player: None,
                deck_seed: None,
            })
            .unwrap(),
        );
        let discarded_card = after_formation
            .state
            .pending_choice
            .as_ref()
            .expect("turn draw should await a discard choice")
            .cards[0]
            .id;

        let before_retrieval = expect_ready(
            handle(ApiRequest {
                action: ApiAction::ChooseTurnDiscard {
                    player: "alice".to_string(),
                    card: discarded_card,
                },
                viewer: Some("bob".to_string()),
                record: Some(after_formation.record),
                setup: Some(web_setup()),
                first_player: None,
                deck_seed: None,
            })
            .unwrap(),
        );

        assert_eq!(before_retrieval.state.phase, "Main");
        assert_eq!(
            before_retrieval.state.current_player.as_deref(),
            Some("bob")
        );
        assert!(
            before_retrieval
                .state
                .statuses
                .iter()
                .any(|status| status.kind == "CannotAct")
        );
        assert!(
            before_retrieval
                .state
                .statuses
                .iter()
                .any(|status| status.kind == "CannotDraw")
        );
        assert!(before_retrieval.interaction.can_pass);
        assert!(before_retrieval.interaction.can_retrieve_discard);

        let after_retrieval = expect_ready(
            handle(ApiRequest {
                action: ApiAction::RetrievePreviousTurnDiscard {
                    player: "bob".to_string(),
                },
                viewer: Some("bob".to_string()),
                record: Some(before_retrieval.record),
                setup: Some(web_setup()),
                first_player: None,
                deck_seed: None,
            })
            .unwrap(),
        );
        let events = after_retrieval
            .record
            .iter()
            .flat_map(|decision| &decision.events)
            .collect::<Vec<_>>();

        assert!(after_retrieval.record.iter().any(|decision| matches!(
            &decision.source,
            crate::application::RecordedDecisionSource::Command {
                command: Command::PassAction { player, .. },
                ..
            } if player == &bob
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::DiscardRetrieved { player, .. } if player == &bob
        )));
        assert!(events.iter().any(|event| matches!(
            event,
            GameEvent::ActionPassed {
                player,
                reason: PassActionReason::CannotActByStatus,
            } if player == &bob
        )));
        assert_ne!(after_retrieval.state.current_player.as_deref(), Some("bob"));
        assert_ne!(
            (
                after_retrieval.state.current_player.as_deref(),
                after_retrieval.state.phase.as_str(),
            ),
            (Some("bob"), "Main")
        );
    }

    #[test]
    fn tribulation_development_scenario_uses_a_deterministic_starting_hand() {
        let rules = OfficialRules::new();
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let setup = rules
            .configure_game(
                vec![
                    Player {
                        id: alice.clone(),
                        team: TeamId::new("team:alice"),
                    },
                    Player {
                        id: bob.clone(),
                        team: TeamId::new("team:bob"),
                    },
                ],
                vec![alice.clone(), bob],
                rules
                    .default_rule_modules()
                    .into_iter()
                    .filter(|module| module.as_str() != crate::domain::POUCH_MODULE_ID)
                    .collect(),
            )
            .unwrap();

        let deck_order =
            development_scenario_deck_order(&rules, &setup, &alice, "tribulation-earth-rending")
                .unwrap();
        let mut record = GameRecord::start(setup, deck_order).unwrap();
        advance_to_interactive_decision(&mut record).unwrap();

        let action =
            development_scenario_action(&record, &alice, "tribulation-earth-rending").unwrap();

        assert!(matches!(
            action,
            Some(PlayableAction::PerformFormation(candidate))
                if candidate.formation_id == crate::rules::tribulation::EARTH_RENDING
        ));
    }

    #[test]
    fn pouch_chain_sheep_development_scenario_reaches_chain() {
        let rules = OfficialRules::new();
        let base = GameSetup::two_player(PlayerId::new("alice"), PlayerId::new("bob"), 20);
        let modules = rules.default_rule_modules();
        let setup = rules
            .configure_game(base.players, base.turn_order, modules)
            .unwrap();
        let alice = PlayerId::new("alice");
        assert!(setup.has_rule_module(crate::domain::POUCH_MODULE_ID));
        let deck_order =
            development_scenario_deck_order(&rules, &setup, &alice, "pouch-chain-sheep").unwrap();
        let mut record = GameRecord::start(setup, deck_order).unwrap();
        complete_pouch_chain_development_preparation(&mut record, &alice).unwrap();
        assert!(record.state().deck_for(&alice).unwrap().iter().any(|card| {
            record
                .state()
                .card_def(*card)
                .is_some_and(|definition| definition.level == 2)
        }));

        let action = development_scenario_action(&record, &alice, "pouch-chain-sheep").unwrap();
        let Some(PlayableAction::PerformFormation(candidate)) = action else {
            panic!("Pouch development scenario should offer Chain")
        };
        assert_eq!(candidate.formation_id, crate::rules::pouch::CHAIN_ID);
        record
            .handle(Command::PerformFormation {
                player: alice.clone(),
                formation_id: candidate.formation_id,
                cards: candidate.cards,
                declared_targets: candidate.declared_targets,
            })
            .unwrap();

        let chain_actions = secret_strategy_actions_for(record.state(), &alice);
        assert!(chain_actions.iter().any(|action| {
            action.strategy == SecretStrategy::SheepStealing
                && action.input == WebSecretStrategyInputRequirement::DeckDiscardSwap
        }));
        assert!(
            secret_strategy_actions_for(record.state(), &PlayerId::new("bob")).is_empty(),
            "a private Chain choice must not disclose its strategy options to another player"
        );
    }

    #[test]
    fn legal_rusted_forest_returns_only_a_camel_case_randomness_continuation() {
        let rules = OfficialRules::new();
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let setup = rules
            .configure_game(
                vec![
                    Player {
                        id: alice.clone(),
                        team: TeamId::new("team:alice"),
                    },
                    Player {
                        id: bob.clone(),
                        team: TeamId::new("team:bob"),
                    },
                ],
                vec![alice.clone(), bob],
                rules
                    .default_rule_modules()
                    .into_iter()
                    .filter(|module| {
                        !matches!(
                            module.as_str(),
                            crate::domain::POUCH_MODULE_ID | crate::domain::PERSONAL_DECK_MODULE_ID
                        )
                    })
                    .collect(),
            )
            .unwrap();
        let mut used = Vec::new();
        let formation_cards = [
            (Element::Wood, 3),
            (Element::Wood, 4),
            (Element::Metal, 3),
            (Element::Metal, 4),
        ]
        .into_iter()
        .map(|(element, level)| {
            let card = setup
                .card_instances
                .iter()
                .find_map(|instance| {
                    let definition = setup
                        .card_defs
                        .iter()
                        .find(|definition| definition.id == instance.definition)?;
                    (!used.contains(&instance.instance)
                        && definition.element == element
                        && definition.level == level)
                        .then_some(instance.instance)
                })
                .expect("official Deck must contain the Rusted Forest Cards");
            used.push(card);
            card
        })
        .collect::<Vec<_>>();
        let mut deck_order = formation_cards.clone();
        deck_order.extend(
            rules
                .official_deck_order(&setup)
                .unwrap()
                .into_iter()
                .filter(|card| !formation_cards.contains(card)),
        );
        let mut record = GameRecord::start(setup.clone(), deck_order).unwrap();
        advance_to_interactive_decision(&mut record).unwrap();
        assert_eq!(
            record.state().hand(&alice),
            Some(formation_cards.as_slice())
        );

        let request = serde_json::json!({
            "action": {
                "type": "performFormation",
                "player": "alice",
                "formationId": crate::rules::tribulation::RUSTED_FOREST,
                "cards": formation_cards,
            },
            "viewer": "alice",
            "record": record.recorded_decisions(),
            "setup": {
                "players": [
                    { "id": "alice", "team": "team:alice" },
                    { "id": "bob", "team": "team:bob" },
                ],
                "turnOrder": ["alice", "bob"],
                "enabledRuleModules": setup.enabled_rule_modules,
            },
        });
        let response = handle_request_json(&request.to_string())
            .expect("a legal Rusted Forest should return a randomness continuation");
        let json: serde_json::Value = serde_json::from_str(&response).unwrap();

        let root_fields = json
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            root_fields,
            ["record", "request", "type"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
        assert_eq!(json["type"], "needsRandomness");
        assert!(
            json["record"]
                .as_array()
                .is_some_and(|record| !record.is_empty())
        );
        let request_fields = json["request"]
            .as_object()
            .unwrap()
            .keys()
            .cloned()
            .collect::<std::collections::BTreeSet<_>>();
        assert_eq!(
            request_fields,
            ["continuation", "currentOrder", "operation", "requestId"]
                .into_iter()
                .map(str::to_string)
                .collect(),
        );
        assert_eq!(
            json["request"]["requestId"],
            "tribulation:rusted-forest:1:shared"
        );
        assert_eq!(
            json["request"]["operation"],
            serde_json::json!({"type": "deckShuffle", "deck": "Shared"})
        );
        assert_eq!(
            json["request"]["continuation"],
            serde_json::json!({
                "type": "tribulation",
                "kind": "rustedForestShuffle",
            })
        );
        assert!(json["request"]["currentOrder"].as_array().is_some());
        for player_ready_field in ["state", "events", "playableActions", "interaction"] {
            assert!(json.get(player_ready_field).is_none());
        }
    }

    #[test]
    fn every_named_action_scenario_has_a_repeatable_official_start() {
        let rules = OfficialRules::new();
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let setup = rules
            .configure_game(
                vec![
                    Player {
                        id: alice.clone(),
                        team: TeamId::new("team:alice"),
                    },
                    Player {
                        id: bob.clone(),
                        team: TeamId::new("team:bob"),
                    },
                ],
                vec![alice.clone(), bob],
                rules
                    .default_rule_modules()
                    .into_iter()
                    .filter(|module| module.as_str() != crate::domain::POUCH_MODULE_ID)
                    .collect(),
            )
            .unwrap();

        for scenario in [
            "hero-schools-transition",
            "spirit-metal",
            "spirit-fire",
            "echo-pure-fire",
            "echo-split-earth",
            "tribulation-earth-rending",
            "tribulation-rusted-forest",
        ] {
            let first = development_scenario_deck_order(&rules, &setup, &alice, scenario).unwrap();
            let second = development_scenario_deck_order(&rules, &setup, &alice, scenario).unwrap();
            assert_eq!(first, second, "{scenario} must have a fixed starting order");

            let mut record = GameRecord::start(setup.clone(), first).unwrap();
            advance_to_interactive_decision(&mut record).unwrap();
            assert!(
                development_scenario_action(&record, &alice, scenario)
                    .unwrap()
                    .is_some(),
                "{scenario} must reach its official action"
            );
        }
    }

    #[test]
    fn trusted_randomness_requirement_is_owned_by_the_rules_adapter() {
        assert_eq!(
            trusted_random_hand_selection_count(&ApiAction::PerformFormation {
                player: "alice".to_string(),
                formation_id: crate::rules::dark::DARK_CHAOS.to_string(),
                cards: Vec::new(),
                star_substitution_card: None,
                match_option_role: None,
                match_option_card: None,
                match_option_slots: None,
                trusted_random_cards: None,
            }),
            Some(2)
        );
        assert_eq!(
            trusted_random_hand_selection_count(&ApiAction::UseSpiritSkill {
                player: "alice".to_string(),
                skill: crate::domain::SpiritSkill::EvilGaze,
                selected_card: None,
                declared_level: None,
                trusted_random_cards: None,
            }),
            Some(2)
        );
        assert_eq!(
            trusted_random_hand_selection_count(&ApiAction::PassAction),
            None
        );
    }

    #[test]
    fn public_cards_serialize_structured_rule_facts() {
        let response = handle_request_json(
            r#"{
                "action":{"type":"start"},
                "viewer":"alice",
                "setup":{
                    "players":[
                        {"id":"alice","team":"team:alice"},
                        {"id":"bob","team":"team:bob"}
                    ],
                    "turnOrder":["alice","bob"]
                }
            }"#,
        )
        .expect("start request should succeed");
        let json: serde_json::Value = serde_json::from_str(&response).unwrap();
        let card = &json["state"]["hands"][0]["cards"]["cards"][0];

        assert!(card["element"].as_str().is_some());
        assert!(card["level"].as_u64().is_some());
        assert_eq!(card["secretStrategies"].as_array().unwrap().len(), 2);
    }

    #[test]
    fn start_request_supports_default_on_optional_rules_and_personal_decks() {
        let response = handle_request_json(
            r#"{
                "action":{"type":"start"},
                "viewer":"alice",
                "setup":{
                    "players":[
                        {"id":"alice","team":"team:alice"},
                        {"id":"bob","team":"team:bob"}
                    ],
                    "turnOrder":["alice","bob"],
                    "enabledRuleModules":["discard-retrieval","personal-deck"],
                    "deckLists":[]
                }
            }"#,
        )
        .expect("personal deck start should succeed");
        let json: serde_json::Value = serde_json::from_str(&response).unwrap();

        assert_eq!(
            json["state"]["enabledRuleModules"],
            serde_json::json!(["discard-retrieval", "personal-deck"])
        );
        assert_eq!(json["state"]["playerDecks"][0]["cards"]["count"], 56);
        assert_eq!(json["state"]["playerDecks"][1]["cards"]["count"], 55);
    }

    #[test]
    fn star_state_and_events_are_projected_for_web_clients() {
        let rules = OfficialRules::new();
        let setup = rules
            .configure_game(
                vec![
                    crate::domain::Player {
                        id: PlayerId::new("alice"),
                        team: TeamId::new("team:alice"),
                    },
                    crate::domain::Player {
                        id: PlayerId::new("bob"),
                        team: TeamId::new("team:bob"),
                    },
                ],
                vec![PlayerId::new("alice"), PlayerId::new("bob")],
                vec![RuleModuleId::new(crate::domain::STAR_MODULE_ID)],
            )
            .unwrap();
        let mut state = crate::domain::GameState::from_setup(&setup);
        state.team_stars.push(crate::domain::TeamStar {
            team: TeamId::new("team:alice"),
            star: crate::domain::StarKind::Metal,
        });
        state.star_histories[0].stars = vec![crate::domain::StarKind::Metal];
        let public = crate::public_view::state_for(&state, Viewer::Observer);
        let web = WebPublicGameState::from_public(
            public,
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web).unwrap();

        assert_eq!(json["teamStars"][0]["star"], "Metal");
        assert_eq!(json["starHistories"][0]["stars"][0], "Metal");
        assert!(json["fiveStarAlignment"].is_null());

        let event = GameEvent::StarSummoned {
            player: PlayerId::new("alice"),
            team: TeamId::new("team:alice"),
            star: crate::domain::StarKind::Metal,
        };
        assert_eq!(
            game_event_presentation(&event, &HashMap::new(), &HashMap::new()),
            (
                "召喚星辰".to_string(),
                "alice 召喚了金星‧太白。".to_string()
            )
        );
    }

    #[test]
    fn spirit_state_and_events_are_projected_for_web_clients() {
        let rules = OfficialRules::new();
        let setup = rules
            .configure_game(
                vec![
                    crate::domain::Player {
                        id: PlayerId::new("alice"),
                        team: TeamId::new("team:alice"),
                    },
                    crate::domain::Player {
                        id: PlayerId::new("bob"),
                        team: TeamId::new("team:bob"),
                    },
                ],
                vec![PlayerId::new("alice"), PlayerId::new("bob")],
                [
                    crate::domain::STAR_MODULE_ID,
                    crate::domain::FIVE_DIRECTIONS_LEGEND_MODULE_ID,
                    crate::domain::HERO_SCHOOLS_MODULE_ID,
                    crate::domain::SPIRIT_MODULE_ID,
                ]
                .into_iter()
                .map(RuleModuleId::new)
                .collect(),
            )
            .unwrap();
        let mut state = crate::domain::GameState::from_setup(&setup);
        state.spirits.push(crate::domain::PlayerSpirit {
            player: PlayerId::new("alice"),
            spirit: crate::domain::SpiritKind::Fire,
            power: 4,
        });
        let interpreted_card = setup.card_instances[0].instance;
        state
            .spirit_level_interpretations
            .push(crate::domain::SpiritLevelInterpretation {
                player: PlayerId::new("alice"),
                skill: Some(crate::domain::SpiritSkill::Glimmer),
                card: interpreted_card,
                level: 3,
                applied_on_turn: state.turn_number,
                interpretation_revision: 1,
            });
        let public = crate::public_view::state_for(&state, Viewer::Observer);
        let web = WebPublicGameState::from_public(
            public,
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web).unwrap();

        assert_eq!(json["spirits"][0]["player"], "alice");
        assert_eq!(json["spirits"][0]["spirit"], "Fire");
        assert_eq!(json["spirits"][0]["power"], 4);
        assert_eq!(json["cardInterpretations"][0]["type"], "spiritSkill");
        assert_eq!(json["cardInterpretations"][0]["skill"], "Glimmer");
        assert!(json["cardInterpretations"][0]["card"].is_null());
        assert_eq!(json["cardInterpretations"][0]["level"], 3);

        let owner_web = WebPublicGameState::from_public(
            crate::public_view::state_for(&state, Viewer::Player(PlayerId::new("alice"))),
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
        );
        let owner_json = serde_json::to_value(owner_web).unwrap();
        assert_eq!(
            owner_json["cardInterpretations"][0]["card"]["id"],
            serde_json::to_value(interpreted_card).unwrap()
        );

        let event = GameEvent::SpiritSummoned {
            player: PlayerId::new("alice"),
            previous: None,
            spirit: crate::domain::SpiritKind::Fire,
        };
        assert_eq!(
            game_event_presentation(&event, &HashMap::new(), &HashMap::new()),
            (
                "召喚精靈".to_string(),
                "alice 召喚火精靈，靈力為 2。".to_string()
            )
        );
    }

    #[test]
    fn star_substitution_is_exposed_and_accepted_by_the_web_formation_flow() {
        let substitution = WebStarElementSubstitution {
            card: CardInstanceId::new(42),
            printed_element: crate::domain::Element::Water,
            interpreted_element: crate::domain::Element::Wood,
        };
        let candidate = WebPlayableAction::PerformFormation {
            id: "defense".to_string(),
            name: "防禦".to_string(),
            category: WebFormationCategory::Spell,
            policy: WebFormationActionPolicy::Standard,
            summary: "星辰替代".to_string(),
            cards: vec![CardInstanceId::new(7), CardInstanceId::new(42)],
            star_substitution: Some(substitution),
            match_option: None,
        };
        let json = serde_json::to_value(candidate).unwrap();

        assert_eq!(json["starSubstitution"]["card"], 42);
        assert_eq!(json["starSubstitution"]["printedElement"], "Water");
        assert_eq!(json["starSubstitution"]["interpretedElement"], "Wood");

        let action: ApiAction = serde_json::from_value(serde_json::json!({
            "type": "performFormation",
            "player": "alice",
            "formationId": "defense",
            "cards": [7, 42],
            "starSubstitutionCard": 42
        }))
        .unwrap();
        assert!(matches!(
            action,
            ApiAction::PerformFormation {
                star_substitution_card: Some(card),
                ..
            } if card == CardInstanceId::new(42)
        ));
    }

    #[test]
    fn start_request_consolidates_setup_events_into_plain_language() {
        let response = handle_request_json(r#"{"action":{"type":"start"},"viewer":"alice"}"#)
            .expect("start request should succeed");
        let json: serde_json::Value =
            serde_json::from_str(&response).expect("response should be valid JSON");
        let events = json["events"]
            .as_array()
            .expect("response should contain events");

        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["title"], "對局開始");
        assert!(
            events[0]["summary"]
                .as_str()
                .is_some_and(|summary| summary.contains("已完成洗牌與發牌"))
        );
        let visible_events =
            serde_json::to_string(events).expect("visible events should serialize");
        assert!(!visible_events.contains("DeckPrepared"));
        assert!(!visible_events.contains("CardsDealt"));
    }

    #[test]
    fn finished_game_status_uses_the_stable_web_value() {
        let rules = OfficialRules::new();
        let setup = fixture_setup(&rules, None).unwrap();
        let mut state = crate::domain::GameState::from_setup(&setup);
        state.status = crate::domain::GameStatus::Finished {
            outcome: crate::domain::GameOutcome::Team(setup.players[0].team.clone()),
        };
        let web_state = WebPublicGameState::from_public(
            crate::public_view::state_for(&state, Viewer::Player(setup.players[0].id.clone())),
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web_state).expect("web state should serialize");

        assert_eq!(json["status"], "Finished");
    }

    #[test]
    fn shared_environment_is_projected_as_public_web_state() {
        let rules = OfficialRules::new();
        let setup = fixture_setup(&rules, None).unwrap();
        let mut state = crate::domain::GameState::from_setup(&setup);
        state.environment = Some(crate::domain::Element::Water);
        let web_state = WebPublicGameState::from_public(
            crate::public_view::state_for(&state, Viewer::Observer),
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web_state).expect("web state should serialize");

        assert_eq!(json["environment"], "Water");
    }

    #[test]
    fn status_owner_and_numbered_duration_use_the_web_contract() {
        let rules = OfficialRules::new();
        let setup = fixture_setup(&rules, None).unwrap();
        let mut state = crate::domain::GameState::from_setup(&setup);
        state.statuses.push(crate::domain::StatusEffect {
            id: "cannot-act-alice".to_string(),
            owner: StatusOwner::Player(PlayerId::new("alice")),
            kind: "CannotAct".to_string(),
            value: None,
            duration: crate::domain::StatusDuration::Permanent,
        });
        state.statuses.push(crate::domain::StatusEffect {
            id: "cannot-act-bob".to_string(),
            owner: StatusOwner::Player(PlayerId::new("bob")),
            kind: "CannotAct".to_string(),
            value: None,
            duration: crate::domain::StatusDuration::UntilTurnEndNumber {
                player: PlayerId::new("bob"),
                turn_number: 9,
            },
        });
        let web_state = WebPublicGameState::from_public(
            crate::public_view::state_for(&state, Viewer::Player(PlayerId::new("alice"))),
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web_state).expect("web state should serialize");

        assert_eq!(json["statuses"][0]["owner"]["kind"], "player");
        assert_eq!(json["statuses"][0]["owner"]["id"], "alice");
        assert_eq!(json["statuses"][0]["presentation"], "cannotAct");
        assert_eq!(json["statuses"][0]["duration"]["type"], "permanent");
        assert_eq!(
            json["statuses"][1]["duration"]["type"],
            "untilTurnEndNumber"
        );
        assert_eq!(json["statuses"][1]["duration"]["player"], "bob");
        assert_eq!(json["statuses"][1]["duration"]["turnNumber"], 9);
        assert!(json["statuses"][1]["duration"].get("turn_number").is_none());
    }

    #[test]
    fn profession_is_projected_with_inherited_ability_summary() {
        let rules = OfficialRules::new();
        let setup = rules
            .configure_game(
                vec![
                    Player {
                        id: PlayerId::new("alice"),
                        team: TeamId::new("team:alice"),
                    },
                    Player {
                        id: PlayerId::new("bob"),
                        team: TeamId::new("team:bob"),
                    },
                ],
                vec![PlayerId::new("alice"), PlayerId::new("bob")],
                vec![RuleModuleId::new(crate::domain::HERO_SCHOOLS_MODULE_ID)],
            )
            .unwrap();
        let mut state = crate::domain::GameState::from_setup(&setup);
        state.professions.push(crate::domain::PlayerProfession {
            player: PlayerId::new("alice"),
            profession: ProfessionId::new("hero"),
        });
        let web_state = WebPublicGameState::from_public(
            crate::public_view::state_for(&state, Viewer::Observer),
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web_state).expect("web state should serialize");

        assert_eq!(json["professions"][0]["name"], "勇者");
        assert_eq!(
            json["professions"][0]["abilities"]
                .as_array()
                .unwrap()
                .len(),
            8
        );
        assert_eq!(json["professionCatalog"].as_array().unwrap().len(), 18);
        assert_eq!(json["professionCatalog"][2]["name"], "勇者");
        assert_eq!(json["professionCatalog"][2]["parentName"], "戰神");
        assert!(
            json["professionCatalog"][2]["formations"]
                .as_array()
                .is_some_and(|formations| formations
                    .iter()
                    .any(|formation| { formation["name"] == "落光斬" }))
        );
    }

    #[test]
    fn profession_catalog_is_absent_when_hero_schools_is_disabled() {
        let rules = OfficialRules::new();
        let setup = fixture_setup(&rules, None).unwrap();
        let web_state = WebPublicGameState::from_public(
            crate::public_view::state_for(
                &crate::domain::GameState::from_setup(&setup),
                Viewer::Observer,
            ),
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web_state).expect("web state should serialize");

        assert_eq!(json["professionCatalog"], serde_json::json!([]));
    }

    #[test]
    fn profession_ability_action_uses_the_web_camel_case_contract() {
        let action = WebPlayableAction::ActivateProfessionAbility {
            id: "illusion".to_string(),
            name: "幻術".to_string(),
            summary: "prepare".to_string(),
            cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
            target_card: Some(CardInstanceId::new(3)),
            declared_element: Some("Water".to_string()),
            declared_level: Some(4),
        };
        let json = serde_json::to_value(action).expect("action should serialize");

        assert_eq!(json["targetCard"], 3);
        assert_eq!(json["declaredElement"], "Water");
        assert_eq!(json["declaredLevel"], 4);
        assert!(json.get("target_card").is_none());

        let illusion = WebPlayableAction::ActivateProfessionAbility {
            id: "illusion".to_string(),
            name: "幻術".to_string(),
            summary: "virtual".to_string(),
            cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
            target_card: None,
            declared_element: Some("Fire".to_string()),
            declared_level: Some(3),
        };
        assert!(serde_json::to_value(illusion).unwrap()["targetCard"].is_null());
    }

    #[test]
    fn card_movement_summary_names_only_cards_visible_to_the_viewer() {
        let labels = HashMap::from([
            (CardInstanceId::new(7), "火 5".to_string()),
            (CardInstanceId::new(8), "水 3".to_string()),
        ]);
        let known = PublicGameEvent::CardsMoved {
            cards: PublicCardRefs::Known(vec![CardInstanceId::new(7), CardInstanceId::new(8)]),
        };
        assert_eq!(
            event_presentation(&known, &labels, &HashMap::new()).1,
            "火 5、水 3 移動到新的區域。"
        );

        let partially_known = PublicGameEvent::CardsMoved {
            cards: PublicCardRefs::PartiallyKnown {
                cards: vec![Some(CardInstanceId::new(7)), None],
            },
        };
        assert_eq!(
            event_presentation(&partially_known, &labels, &HashMap::new()).1,
            "火 5 與 1 張未公開牌移動到新的區域。"
        );

        let hidden = PublicGameEvent::CardsMoved {
            cards: PublicCardRefs::Hidden { count: 2 },
        };
        let hidden_summary = event_presentation(&hidden, &labels, &HashMap::new()).1;
        assert_eq!(hidden_summary, "有 2 張牌移動到新的區域。");
        assert!(!hidden_summary.contains("火 5"));
        assert!(!hidden_summary.contains("水 3"));
    }

    #[test]
    fn response_includes_zero_card_profession_abilities() {
        let rules = OfficialRules::new();
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let setup = rules
            .configure_game(
                vec![
                    Player {
                        id: alice.clone(),
                        team: TeamId::new("team:alice"),
                    },
                    Player {
                        id: bob.clone(),
                        team: TeamId::new("team:bob"),
                    },
                ],
                vec![alice.clone(), bob],
                rules.default_rule_modules(),
            )
            .unwrap();
        let deck_order = rules.official_deck_order(&setup).unwrap();
        let mut record = GameRecord::start(setup.clone(), deck_order).unwrap();
        record.fixture_state_mut().phase = Phase::Main;
        record
            .fixture_state_mut()
            .professions
            .push(crate::domain::PlayerProfession {
                player: alice.clone(),
                profession: ProfessionId::new(crate::rules::jianghu::QI_GRANDMASTER_ID),
            });

        let response = response_for(
            &record,
            Viewer::Player(alice),
            &rules.card_labels(&setup).unwrap(),
            &card_facts_for_setup(&setup),
            &rules.formation_names(&setup).unwrap(),
            Vec::new(),
        )
        .unwrap();
        let json = serde_json::to_value(response).unwrap();

        assert!(
            json["playableActions"]
                .as_array()
                .is_some_and(|actions| actions.iter().any(|action| {
                    action["type"] == "activateProfessionAbility"
                        && action["id"] == "jianghu:dancing-yang-art"
                        && action["cards"] == serde_json::json!([])
                }))
        );
    }

    #[test]
    fn typed_choice_and_randomness_actions_use_the_web_camel_case_contract() {
        let choice: ApiAction = serde_json::from_value(serde_json::json!({
            "type": "answerEffectChoiceTyped",
            "player": "alice",
            "answer": {
                "type": "formation",
                "formationId": "echo:melody"
            }
        }))
        .unwrap();
        assert!(matches!(
            choice,
            ApiAction::AnswerEffectChoiceTyped {
                answer: EffectChoiceAnswer::Formation { formation_id },
                ..
            } if formation_id == "echo:melody"
        ));

        let randomness: ApiAction = serde_json::from_value(serde_json::json!({
            "type": "resolveRandomness",
            "requestId": "shuffle-1",
            "shuffledOrder": [3, 1, 2]
        }))
        .unwrap();
        assert!(matches!(
            randomness,
            ApiAction::ResolveRandomness {
                request_id,
                shuffled_order,
            } if request_id == "shuffle-1"
                && shuffled_order == vec![
                    CardInstanceId::new(3),
                    CardInstanceId::new(1),
                    CardInstanceId::new(2),
                ]
        ));
    }

    #[test]
    fn start_request_accepts_bob_as_first_player() {
        let response = handle_request_json(
            r#"{"action":{"type":"start"},"viewer":"bob","firstPlayer":"bob"}"#,
        )
        .expect("start request should succeed");

        assert!(response.contains(r#""currentPlayer":"bob""#));
        assert!(response.contains(r#""turnOrder":["bob","alice"]"#));
    }

    #[test]
    fn start_request_shuffles_deck_by_seed_before_dealing() {
        let rules = OfficialRules::new();
        let setup = fixture_setup(&rules, None).unwrap();
        let sorted_deck = rules.official_deck_order(&setup).unwrap();
        let first_shuffle = deck_order_for_start(&rules, &setup, Some("seed-a")).unwrap();
        let same_shuffle = deck_order_for_start(&rules, &setup, Some("seed-a")).unwrap();
        let different_shuffle = deck_order_for_start(&rules, &setup, Some("seed-b")).unwrap();

        assert_ne!(first_shuffle, sorted_deck);
        assert_eq!(first_shuffle, same_shuffle);
        assert_ne!(first_shuffle, different_shuffle);
    }

    #[test]
    fn start_request_accepts_four_player_team_setup() {
        let response = handle_request_json(
            r#"{
                "action":{"type":"start"},
                "viewer":"p3",
                "deckSeed":"team-seed",
                "setup":{
                    "players":[
                        {"id":"p1","team":"team-a"},
                        {"id":"p2","team":"team-b"},
                        {"id":"p3","team":"team-a"},
                        {"id":"p4","team":"team-b"}
                    ],
                    "turnOrder":["p1","p2","p3","p4"]
                }
            }"#,
        )
        .expect("four-player start request should succeed");

        assert!(response.contains(r#""currentPlayer":"p1""#));
        assert!(response.contains(r#""turnOrder":["p1","p2","p3","p4"]"#));
        assert!(response.contains(r#""id":"p3","team":"team-a""#));
    }

    #[test]
    fn playable_action_preserves_discriminant_rule_text_and_cards() {
        let start = handle_request_json(r#"{"action":{"type":"start"},"viewer":"alice"}"#)
            .expect("start request should succeed");
        let start: serde_json::Value =
            serde_json::from_str(&start).expect("start response should be valid JSON");
        let first_card = start["state"]["hands"]
            .as_array()
            .and_then(|hands| {
                hands
                    .iter()
                    .find(|hand| hand["player"] == "alice")
                    .and_then(|hand| hand["cards"]["cards"].as_array())
                    .and_then(|cards| cards.first())
                    .and_then(|card| card["id"].as_u64())
            })
            .expect("alice should have a visible card");
        let request = serde_json::json!({
            "action": {
                "type": "playableActions",
                "player": "alice",
                "cards": [first_card],
            },
            "viewer": "alice",
            "record": start["record"].clone(),
        });

        let response = handle_request_json(&request.to_string())
            .expect("playable formations request should succeed");
        let response: serde_json::Value =
            serde_json::from_str(&response).expect("response should be valid JSON");
        let candidate = &response["playableActions"][0];
        let summary = candidate["summary"]
            .as_str()
            .expect("candidate should have rule text");

        assert_eq!(candidate["type"], "performFormation");
        assert_eq!(candidate["cards"], serde_json::json!([first_card]));
        assert!(summary.contains("攻擊，點數＝等級＋４"));
        assert!(!summary.contains("張牌發動"));
    }

    #[test]
    fn pass_reason_is_derived_from_the_current_state() {
        let setup = fixture_setup(&OfficialRules::new(), None).unwrap();
        let mut state = crate::domain::GameState::from_setup(&setup);
        state.phase = Phase::Main;
        let current = state.current_player().cloned().unwrap();
        state.hand_mut(&current).unwrap().clear();

        assert_eq!(
            pass_action_for_state(&state),
            Some((current, PassActionReason::NoCardsInHand))
        );
    }

    #[test]
    fn effect_choice_exposes_required_card_count() {
        let choice = crate::public_view::PublicPendingChoice {
            player: PlayerId::new("alice"),
            purpose: "chaos".to_string(),
            presentation: PublicPendingChoicePresentation::Chaos,
            kind: PublicPendingChoiceKind::Known(PendingChoiceKind::EffectGenerated {
                effect_id: "chaos".to_string(),
                continuation_id: "chaos:return-two".to_string(),
                allowed_cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
            }),
        };
        let web_choice =
            WebPendingChoice::from_public(choice, &HashMap::new(), &HashMap::new(), &[]);
        let json = serde_json::to_value(web_choice).expect("choice should serialize");

        assert_eq!(json["requiredCount"], 2);
        assert_eq!(json["purpose"], "chaos");
        assert_eq!(json["presentation"]["type"], "chaos");
        assert_eq!(json["formationGroups"], serde_json::json!([]));
        assert_eq!(json["deckCards"], serde_json::json!([]));
        assert_eq!(json["discardCards"], serde_json::json!([]));
    }

    #[test]
    fn sheep_choice_serializes_separate_camel_case_card_piles() {
        let choice = crate::public_view::PublicPendingChoice {
            player: PlayerId::new("alice"),
            purpose: "pouch:sheep-stealing".to_string(),
            presentation: PublicPendingChoicePresentation::Unclassified,
            kind: PublicPendingChoiceKind::Known(PendingChoiceKind::SheepStealing {
                source_card: CardInstanceId::new(9),
                owner: Some(PlayerId::new("alice")),
                deck_cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
                discard_cards: vec![CardInstanceId::new(3), CardInstanceId::new(4)],
            }),
        };
        let web_choice =
            WebPendingChoice::from_public(choice, &HashMap::new(), &HashMap::new(), &[]);
        let json = serde_json::to_value(web_choice).expect("choice should serialize");

        assert_eq!(json["kind"], "SheepStealing");
        assert_eq!(json["cards"], serde_json::json!([]));
        assert_eq!(json["deckCards"].as_array().unwrap().len(), 2);
        assert_eq!(json["discardCards"].as_array().unwrap().len(), 2);
        assert!(json.get("deck_cards").is_none());
        assert!(json.get("discard_cards").is_none());
    }

    #[test]
    fn split_earth_choice_serializes_camel_case_formation_groups() {
        let choice = crate::public_view::PublicPendingChoice {
            player: PlayerId::new("alice"),
            purpose: "裂土指定".to_string(),
            presentation: PublicPendingChoicePresentation::EchoSplitEarthFormation,
            kind: PublicPendingChoiceKind::Known(PendingChoiceKind::TypedEffect {
                effect_id: "echo:split-earth".to_string(),
                continuation_id: "echo:split-earth:formation".to_string(),
                options: crate::domain::EffectChoiceOptions {
                    formations: vec!["weapon".to_string(), "echo:split-earth".to_string()],
                    ..Default::default()
                },
            }),
        };
        let web_choice = WebPendingChoice::from_public(
            choice,
            &HashMap::new(),
            &HashMap::new(),
            &[RuleModuleId::new(crate::domain::ECHO_MODULE_ID)],
        );
        let json = serde_json::to_value(web_choice).expect("choice should serialize");

        assert_eq!(
            json["formationGroups"],
            serde_json::json!([
                {
                    "ruleModuleId": null,
                    "formations": [{ "id": "weapon", "name": "武器" }]
                },
                {
                    "ruleModuleId": "echo",
                    "formations": [{ "id": "echo:split-earth", "name": "宮調‧裂土" }]
                }
            ])
        );
        assert!(json.get("formation_groups").is_none());
        assert!(json["formationGroups"][0].get("rule_module_id").is_none());
    }

    #[test]
    fn hidden_card_refs_use_plain_player_facing_copy() {
        assert_eq!(
            card_refs_summary(&PublicCardRefs::Hidden { count: 5 }, &HashMap::new()),
            "5 張牌"
        );
    }

    #[test]
    fn pending_randomness_web_contract_uses_camel_case_and_exposes_only_the_operation_kind() {
        let json = serde_json::to_value(WebPendingRandomness {
            request_id: "shuffle-1".to_string(),
            deck: "player:alice".to_string(),
            operation: "discardShuffle".to_string(),
            card_count: 3,
        })
        .unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "requestId": "shuffle-1",
                "deck": "player:alice",
                "operation": "discardShuffle",
                "cardCount": 3
            })
        );
        assert!(json.get("currentOrder").is_none());
    }

    #[test]
    fn turn_draw_records_use_rulebook_terms() {
        let labels = HashMap::from([(CardInstanceId::new(1), "金 1".to_string())]);
        let draw = GameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("alice"),
            drawn_cards: vec![CardInstanceId::new(1)],
            allowed_discards: vec![CardInstanceId::new(1)],
        };
        let discard = GameEvent::TurnDiscardChosen {
            player: PlayerId::new("alice"),
            discard: CardInstanceId::new(1),
        };

        assert_eq!(
            game_event_presentation(&draw, &labels, &HashMap::new()),
            (
                "回合抽牌".to_string(),
                "alice 進行回合抽牌，抽取 金 1，需選擇一張捨棄。".to_string()
            )
        );
        assert_eq!(
            game_event_presentation(&discard, &labels, &HashMap::new()),
            ("捨棄".to_string(), "alice 捨棄了 金 1。".to_string())
        );
    }

    #[test]
    fn attack_record_reports_damage_to_the_players_shield() {
        let formation_names = HashMap::from([("weapon".to_string(), "武器".to_string())]);
        let event = GameEvent::AttackResolved {
            attacker: PlayerId::new("alice"),
            target: PlayerId::new("bob"),
            formation_id: "weapon".to_string(),
            used_cards: Vec::new(),
            point_breakdown: crate::domain::AttackPointBreakdown {
                base_points: 12,
                environment_effect: crate::domain::EnvironmentAttackEffect::None,
                interaction: crate::domain::ElementInteraction::None,
                damage_transform: crate::domain::DamageTransform::NormalDamage,
                final_amount: 12,
            },
            hp_change: crate::domain::HpChangeDelta {
                team: TeamId::new("team-b"),
                old_hp: 20,
                delta: 0,
                new_hp: 20,
                effective_delta: 0,
            },
            shield_change: Some(crate::domain::ShieldChangeDelta {
                player: PlayerId::new("bob"),
                old_value: 30,
                delta: -24,
                new_value: 6,
            }),
            card_moves: Vec::new(),
            elemental_context_update: None,
        };

        assert_eq!(
            game_event_presentation(&event, &HashMap::new(), &formation_names),
            (
                "攻擊結算".to_string(),
                "alice 以「武器」攻擊 bob，bob 的防護罩由 30 變為 6。".to_string()
            )
        );
    }

    #[test]
    fn empty_city_record_only_states_that_it_flipped() {
        let event = GameEvent::PassiveFlipped {
            owner: PlayerId::new("alice"),
            incoming_player: PlayerId::new("bob"),
            passive_id: "empty-city".to_string(),
            cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
            outcome: crate::domain::PassiveFlipOutcome::NoEffect {
                reason: crate::domain::PassiveNoEffectReason::EmptyCity,
            },
        };

        assert_eq!(
            game_event_presentation(&event, &HashMap::new(), &HashMap::new()),
            ("蓋牌翻開".to_string(), "alice 的「空城」翻開。".to_string())
        );
    }

    #[test]
    fn inspected_hand_presentation_only_lists_cards_for_the_authorized_view() {
        let labels = HashMap::from([
            (CardInstanceId::new(1), "金 1".to_string()),
            (CardInstanceId::new(2), "木 2".to_string()),
        ]);
        let known = PublicGameEvent::HandInspected {
            viewer: PlayerId::new("alice"),
            target: PlayerId::new("bob"),
            cards: PublicCardRefs::Known(vec![CardInstanceId::new(1), CardInstanceId::new(2)]),
        };
        let hidden = PublicGameEvent::HandInspected {
            viewer: PlayerId::new("alice"),
            target: PlayerId::new("bob"),
            cards: PublicCardRefs::Hidden { count: 2 },
        };

        assert!(
            event_presentation(&known, &labels, &HashMap::new())
                .1
                .contains("金 1、木 2")
        );
        let hidden_summary = event_presentation(&hidden, &labels, &HashMap::new()).1;
        assert!(hidden_summary.contains("2 張手牌"));
        assert!(!hidden_summary.contains("金 1"));
        assert!(!hidden_summary.contains("木 2"));
    }

    #[test]
    fn chaos_choice_requires_two_cards_without_changing_record_schema() {
        let choice = PendingChoiceKind::EffectGenerated {
            effect_id: "chaos".to_string(),
            continuation_id: "chaos:return-two".to_string(),
            allowed_cards: vec![
                CardInstanceId::new(1),
                CardInstanceId::new(2),
                CardInstanceId::new(3),
            ],
        };

        assert_eq!(choice.required_count(), 2);
        assert!(
            !serde_json::to_string(&choice)
                .expect("choice should serialize")
                .contains("required_count")
        );
    }

    #[test]
    fn echo_public_state_uses_the_web_camel_case_contract() {
        let setup = fixture_setup(&OfficialRules::new(), None).unwrap();
        let mut state = crate::domain::GameState::from_setup(&setup);
        state.scheduled_echoes.push(crate::domain::ScheduledEcho {
            player: PlayerId::new("alice"),
            melody_id: "echo:falling-wood".to_string(),
            due_turn_number: 3,
        });
        state
            .flow_layers_by_player
            .insert(PlayerId::new("alice"), 2);
        state
            .formation_suppressions
            .push(crate::domain::FormationSuppression {
                source: PlayerId::new("alice"),
                target: PlayerId::new("bob"),
                formation_id: "weapon".to_string(),
                expires_on_turn_number: 2,
            });
        state
            .scheduled_plant_earth
            .push(crate::domain::ScheduledPlantEarth {
                player: PlayerId::new("alice"),
                due_turn_number: 3,
            });

        let public = crate::public_view::state_for(&state, Viewer::Observer);
        let web = WebPublicGameState::from_public(
            public,
            &HashMap::new(),
            &HashMap::new(),
            &HashMap::from([("weapon".to_string(), "兵器".to_string())]),
        );
        let json = serde_json::to_value(web).expect("Echo state should serialize");

        assert_eq!(json["scheduledEchoes"][0]["melody"], "fallingWood");
        assert_eq!(json["scheduledEchoes"][0]["dueTurnNumber"], 3);
        assert_eq!(json["flowStates"][0]["layers"], 2);
        assert_eq!(json["formationSuppressions"][0]["expiresOnTurnNumber"], 2);
        assert_eq!(json["formationSuppressions"][0]["formationName"], "兵器");
        assert_eq!(json["scheduledPlantEarth"][0]["dueTurnNumber"], 3);
        assert!(json.get("scheduled_echoes").is_none());
    }

    #[test]
    fn typed_echo_answers_are_explicit_in_event_history() {
        let target = GameEvent::TypedEffectChoiceAnswered {
            player: PlayerId::new("alice"),
            effect_id: "echo:pure-fire".to_string(),
            continuation_id: "echo:pure-fire:target".to_string(),
            answer: EffectChoiceAnswer::Player {
                player: PlayerId::new("bob"),
            },
        };
        assert!(
            game_event_presentation(&target, &HashMap::new(), &HashMap::new())
                .1
                .contains("玩家 bob")
        );

        let formation = GameEvent::TypedEffectChoiceAnswered {
            player: PlayerId::new("alice"),
            effect_id: "echo:plant-earth".to_string(),
            continuation_id: "echo:plant-earth:melody".to_string(),
            answer: EffectChoiceAnswer::Formation {
                formation_id: "echo:ringing-metal".to_string(),
            },
        };
        let names = HashMap::from([("echo:ringing-metal".to_string(), "商調‧鳴金".to_string())]);
        assert!(
            game_event_presentation(&formation, &HashMap::new(), &names)
                .1
                .contains("商調‧鳴金")
        );
    }

    #[test]
    fn player_event_summaries_never_expose_canonical_ids_or_debug_names() {
        let labels = HashMap::from([(CardInstanceId::new(42), "火 3".to_string())]);
        let formations = HashMap::from([
            ("echo:ringing-metal".to_string(), "商調‧鳴金".to_string()),
            ("echo:plant-earth".to_string(), "變宮‧植土".to_string()),
        ]);
        let vocabulary = PlayerVocabulary::for_modules(
            &[
                crate::domain::JIANGHU_MODULE_ID,
                crate::domain::CONFLUENCE_GENERATION_MODULE_ID,
                crate::domain::DARK_GLIMMER_MODULE_ID,
            ]
            .into_iter()
            .map(RuleModuleId::new)
            .collect::<Vec<_>>(),
        );
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let events = vec![
            GameEvent::ProfessionChanged {
                player: alice.clone(),
                previous: Some(ProfessionId::new(crate::rules::confluence::DAO_MAGE_ID)),
                profession: ProfessionId::new(crate::rules::confluence::DAO_SAINT_ID),
                card_moves: Vec::new(),
            },
            GameEvent::JianghuStateApplied {
                state: crate::domain::JianghuState {
                    owner: alice.clone(),
                    kind: crate::domain::JianghuStateKind::ThousandBlades,
                    remaining_turns: 1,
                    expires_on_turn: None,
                    last_resolved_turn: None,
                },
            },
            GameEvent::LimitedUseChanged {
                owner: alice.clone(),
                key: crate::rules::confluence::IMPRISONING_ARRAY_USE.to_string(),
                old_remaining: 1,
                new_remaining: 0,
                maximum: 1,
            },
            GameEvent::ProfessionChanged {
                player: alice.clone(),
                previous: Some(ProfessionId::new(crate::rules::dark::DARK_WALKER_ID)),
                profession: ProfessionId::new(crate::rules::dark::DARK_SPIRIT_ENVOY_ID),
                card_moves: Vec::new(),
            },
            GameEvent::SpiritTransformed {
                player: alice.clone(),
                previous: SpiritKind::Metal,
                spirit: SpiritKind::Evil,
                power: 2,
            },
            GameEvent::SpiritTransformed {
                player: alice.clone(),
                previous: SpiritKind::Evil,
                spirit: SpiritKind::Death,
                power: 2,
            },
            GameEvent::PouchRevealed {
                player: alice.clone(),
                owner: None,
                card: CardInstanceId::new(42),
                strategy: SecretStrategy::DarkCrossing,
            },
            GameEvent::ProfessionTransformed {
                player: alice.clone(),
                previous: Some(ProfessionId::new(crate::rules::confluence::DAO_MAGE_ID)),
                profession: ProfessionId::new(crate::rules::jianghu::LONE_WANDERER_ID),
                reason: "pouch:dark-crossing".to_string(),
            },
            GameEvent::FormationSuppressionSet {
                suppression: crate::domain::FormationSuppression {
                    source: alice.clone(),
                    target: bob,
                    formation_id: "echo:ringing-metal".to_string(),
                    expires_on_turn_number: 2,
                },
            },
            GameEvent::PlantEarthResolutionCompleted {
                player: alice.clone(),
                due_turn_number: 2,
                melody_id: "echo:plant-earth".to_string(),
            },
            GameEvent::RingingMetalCardRevealed {
                selection: crate::domain::RingingMetalSelection {
                    player: alice,
                    card: CardInstanceId::new(42),
                    deck: crate::domain::RandomnessDeck::Shared,
                },
            },
        ];

        let summaries = events
            .iter()
            .map(|event| {
                game_event_presentation_with_vocabulary(event, &labels, &formations, &vocabulary).1
            })
            .collect::<Vec<_>>();
        let joined = summaries.join("\n");
        for forbidden in [
            "jianghu:",
            "confluence:",
            "dark:",
            "echo:",
            "pouch:",
            "ThousandBlades",
            "Metal",
            "Evil",
            "Death",
            "CardInstanceId",
            " 42",
        ] {
            assert!(!joined.contains(forbidden), "leaked {forbidden}: {joined}");
        }
        assert!(!summaries[0].contains("無職業"));
        assert!(summaries[0].contains("道法師") && summaries[0].contains("道法聖"));
        assert!(summaries[1].contains("千鋒"));
        assert!(summaries[2].contains("禁錮法陣"));
        assert!(summaries[4].contains("金") && summaries[4].contains("惡"));
        assert!(summaries[5].contains("惡") && summaries[5].contains("死"));
        assert!(summaries[6].contains("暗渡陳倉"));
        assert!(summaries[7].contains("暗渡陳倉"));
        assert!(summaries[8].contains("商調‧鳴金"));
        assert!(summaries[9].contains("變宮‧植土"));
        assert!(summaries[10].contains("火 3"));

        let public_event = WebPublicGameEvent::from_public(
            1,
            PublicGameEvent::SpiritLevelInterpreted {
                player: PlayerId::new("alice"),
                skill: Some(SpiritSkill::EvilGaze),
                card: Some(CardInstanceId::new(42)),
                level: 1,
                applied_on_turn: 1,
            },
            &labels,
            &formations,
            &vocabulary,
        );
        assert!(public_event.summary.contains("火 3"));
        assert!(!public_event.summary.contains("42"));

        let choice_event = WebPublicGameEvent::from_public(
            2,
            PublicGameEvent::EffectChoiceRequested {
                player: PlayerId::new("alice"),
                purpose: "echo:ringing-metal".to_string(),
                kind: PublicPendingChoiceKind::Hidden,
            },
            &labels,
            &formations,
            &vocabulary,
        );
        assert!(choice_event.summary.contains("商調‧鳴金"));
        assert!(!choice_event.summary.contains("echo:"));
    }

    #[test]
    fn player_event_unknown_name_fallbacks_are_safe_and_not_no_profession() {
        let vocabulary = PlayerVocabulary::for_modules(&[]);
        let summary = game_event_presentation_with_vocabulary(
            &GameEvent::ProfessionChanged {
                player: PlayerId::new("alice"),
                previous: Some(ProfessionId::new("future:previous")),
                profession: ProfessionId::new("future:profession"),
                card_moves: Vec::new(),
            },
            &HashMap::new(),
            &HashMap::new(),
            &vocabulary,
        )
        .1;

        assert!(summary.contains("未知職業"));
        assert!(!summary.contains("無職業"));
        assert!(!summary.contains("future:"));
    }

    #[test]
    fn player_vocabulary_covers_catalog_and_activated_profession_ability_ids() {
        let vocabulary = PlayerVocabulary::all_official();
        let catalog_ability_ids = crate::rules::profession::catalog(
            &[
                crate::domain::HERO_SCHOOLS_MODULE_ID,
                crate::domain::JIANGHU_MODULE_ID,
                crate::domain::CONFLUENCE_GENERATION_MODULE_ID,
                crate::domain::DARK_GLIMMER_MODULE_ID,
            ]
            .into_iter()
            .map(RuleModuleId::new)
            .collect::<Vec<_>>(),
        )
        .into_iter()
        .flat_map(|profession| profession.ability_ids)
        .chain([
            "shadow-cut",
            "meditation",
            "revelation",
            "illusion",
            "phantasm",
        ])
        .collect::<Vec<_>>();

        for id in catalog_ability_ids {
            assert_ne!(vocabulary.ability(id), "未知能力", "missing {id}");
        }
    }

    #[test]
    fn public_card_label_fallback_never_uses_the_card_instance_debug_value() {
        let card = WebCard::from_id(CardInstanceId::new(42), &HashMap::new(), &HashMap::new());

        assert_eq!(card.label, "一張牌");
        assert!(!card.label.contains("42"));
        assert!(!card.label.contains("CardInstanceId"));
    }

    #[test]
    fn pouch_actions_use_the_web_camel_case_contract() {
        let action: ApiAction = serde_json::from_value(serde_json::json!({
            "type": "triggerSecretStrategy",
            "player": "alice",
            "strategy": "DeceiveHeaven",
            "targetPlayer": "bob",
            "star": "Fire",
            "breakStar": true,
            "discardCard": 7,
            "deckCards": [8, 9],
            "discardCards": [10, 11]
        }))
        .unwrap();
        assert!(matches!(
            action,
            ApiAction::TriggerSecretStrategy {
                target_player: Some(player),
                star: Some(crate::domain::StarKind::Fire),
                break_star: true,
                discard_card: Some(_),
                ref deck_cards,
                ref discard_cards,
                ..
            } if player == "bob" && deck_cards.len() == 2 && discard_cards.len() == 2
        ));

        let stale_chain = serde_json::from_value::<ApiAction>(serde_json::json!({
            "type": "performFormation",
            "player": "alice",
            "formationId": "pouch:chain",
            "cards": [1, 2, 3],
            "pouchOwner": "bob",
            "pouchCard": 7,
            "triggerCard": 8,
            "secretStrategy": "DeceiveHeaven",
            "secretStrategyStar": "Fire",
            "secretStrategyBreakStar": true,
            "secretStrategyDeckCards": [9, 10],
            "secretStrategyDiscardCards": [11, 12]
        }));
        assert!(stale_chain.is_err(), "legacy chain fields must be rejected");

        let chain: ApiAction = serde_json::from_value(serde_json::json!({
            "type": "performFormation",
            "player": "alice",
            "formationId": "pouch:chain",
            "cards": [1, 2, 3]
        }))
        .unwrap();
        assert!(matches!(
            chain,
            ApiAction::PerformFormation {
                ref formation_id,
                ref cards,
                ..
            } if formation_id == "pouch:chain" && cards == &vec![CardInstanceId::new(1), CardInstanceId::new(2), CardInstanceId::new(3)]
        ));
    }

    #[test]
    fn closed_web_presentation_mappings_cover_every_current_domain_value() {
        let statuses = [
            "CannotAct",
            "CannotDraw",
            "DivineCalculation",
            "GaleRain",
            "PouchGoldenCicada",
            "PouchWatchFire",
            "PouchLurePlayer",
            "PouchLureSpirit",
            "SpiritStoneShield",
            "JianghuFanBeyondHeaven",
            "JianghuYangAura",
            "JianghuDancingYang",
            "JianghuMeteor",
        ];
        let limited_uses = [
            "confluence:heavenly-resonance",
            "confluence:imprisoning-array",
            "confluence:tailwind",
            "confluence:void-realm",
        ];
        let echo_schedules = [
            "echo:ringing-metal",
            "echo:falling-wood",
            "echo:flowing-water",
            "echo:war-fire",
            "echo:split-earth",
            "echo:pure-fire",
        ];
        let formation_policies = [
            "base:weapon",
            "pouch:chain",
            "echo:ringing-metal",
            "echo:falling-wood",
            "echo:flowing-water",
            "echo:war-fire",
            "echo:split-earth",
            "echo:pure-fire",
            "echo:plant-earth",
        ];

        for status in statuses {
            assert_ne!(
                serde_json::to_value(WebStatusPresentation::from_kind(status)).unwrap(),
                serde_json::json!("unclassified")
            );
        }
        for key in limited_uses {
            assert_ne!(
                serde_json::to_value(WebLimitedUsePresentation::from_key(key)).unwrap(),
                serde_json::json!("unclassified")
            );
        }
        for melody in echo_schedules {
            assert_ne!(
                serde_json::to_value(WebEchoSchedulePresentation::from_id(melody)).unwrap(),
                serde_json::json!("unclassified")
            );
        }
        for formation in formation_policies {
            serde_json::to_value(WebFormationActionPolicy::from_id(formation)).unwrap();
        }
    }
}
