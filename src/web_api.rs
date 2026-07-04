use crate::application::{GameRecord, RecordedDecision};
use crate::domain::targeting::{RulePlayerTarget, TurnOrderTargets};
use crate::domain::{
    CardDefId, CardInstanceId, Command, DISCARD_RETRIEVAL_MODULE_ID, EffectChoiceAnswer, GameError,
    GameEvent, GameSetup, PassActionReason, PendingChoiceKind, PendingRandomness, Phase, Player,
    PlayerDeckList, PlayerId, ProfessionId, RuleModuleId, StarElementSubstitution, StatusOwner,
    TargetDecl, TeamHp, TeamId, TrustedRandomnessAnswer, TurnDrawSkipReason,
};
use crate::public_view::{
    PublicCardRefs, PublicGameEvent, PublicGameState, PublicPendingChoiceKind, Viewer,
};
use crate::rules::{FormationCategory, OfficialRules, PlayableAction};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub fn handle_request_json(input: &str) -> Result<String, String> {
    let request: ApiRequest = serde_json::from_str(input).map_err(|error| error.to_string())?;
    let response = handle(request).map_err(|error| serde_json::to_string(&error).unwrap())?;

    serde_json::to_string(&response).map_err(|error| error.to_string())
}

fn handle(request: ApiRequest) -> Result<ApiResponse, ApiError> {
    let rules = OfficialRules::new();
    let setup = setup_for_request(&rules, request.setup, request.first_player.as_deref())?;
    let card_labels = rules.card_labels(&setup).map_err(ApiError::Game)?;
    let formation_names = rules.formation_names(&setup).map_err(ApiError::Game)?;
    let viewer = viewer_from_request(request.viewer.as_deref());
    let deck_seed = request.deck_seed.clone();
    let mut record = record_from_request(&rules, &setup, request.record, deck_seed.as_deref())?;

    match request.action {
        ApiAction::Start => {
            record = GameRecord::start(
                setup.clone(),
                deck_order_for_start(&rules, &setup, deck_seed.as_deref())?,
            )
            .map_err(ApiError::Game)?;
            advance_to_interactive_decision(&mut record)?;
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
                &formation_names,
                candidates
                    .into_iter()
                    .map(|candidate| match candidate {
                        PlayableAction::PerformFormation(candidate) => {
                            let summary = candidate.star_substitution.as_ref().map_or_else(
                                || candidate.rule_text.clone(),
                                |substitution| {
                                    format!(
                                        "{} 星辰效果：將{}（{}）視為{}。",
                                        candidate.rule_text,
                                        card_summary(&substitution.card, &card_labels),
                                        card_element_name(substitution.printed_element),
                                        card_element_name(substitution.interpreted_element),
                                    )
                                },
                            );
                            WebPlayableAction::PerformFormation {
                                id: candidate.formation_id,
                                name: candidate.formation_name,
                                category: WebFormationCategory::from(candidate.category),
                                summary,
                                cards: candidate.cards,
                                star_substitution: candidate
                                    .star_substitution
                                    .map(WebStarElementSubstitution::from),
                                match_option: candidate.declared_targets.iter().find_map(
                                    |target| match target {
                                        TargetDecl::FormationRole { role, card } => {
                                            Some(WebFormationMatchOption {
                                                role: role.clone(),
                                                card: *card,
                                                slots: 1,
                                                preview: candidate.preview.clone(),
                                            })
                                        }
                                        TargetDecl::CardMultiplicity { card, slots } => {
                                            Some(WebFormationMatchOption {
                                                role: "card-multiplicity".to_string(),
                                                card: *card,
                                                slots: *slots,
                                                preview: candidate.preview.clone(),
                                            })
                                        }
                                        _ => None,
                                    },
                                ),
                            }
                        }
                        PlayableAction::ChangeProfession(candidate) => {
                            WebPlayableAction::ChangeProfession {
                                id: candidate.profession_id.as_str().to_string(),
                                name: candidate.profession_name,
                                summary: candidate.rule_text,
                                cards: candidate.cards,
                            }
                        }
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
                        PlayableAction::UseSpiritSkill(candidate) => {
                            WebPlayableAction::UseSpiritSkill {
                                id: format!("{:?}", candidate.skill),
                                name: candidate.skill_name,
                                summary: candidate.rule_text,
                                cards: candidate.selected_card.into_iter().collect(),
                                selected_card: candidate.selected_card,
                                declared_level: candidate.declared_level,
                            }
                        }
                    })
                    .collect(),
            );
        }
        ApiAction::TrustedRandomHandCandidates { player } => {
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
            let mut response =
                response_for(&record, viewer, &card_labels, &formation_names, Vec::new())?;
            response.trusted_random_candidates = Some(candidates);
            return Ok(response);
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
        }
    }

    response_for(&record, viewer, &card_labels, &formation_names, Vec::new())
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
    formation_names: &HashMap<String, String>,
    mut playable_actions: Vec<WebPlayableAction>,
) -> Result<ApiResponse, ApiError> {
    let can_pass = pass_action_for_state(record.state()).is_some();
    let can_retrieve_discard = can_retrieve_discard(record.state());
    if let Viewer::Player(player) = &viewer
        && record.state().current_player() == Some(player)
        && record.state().phase == Phase::Main
        && record.state().pending_choice.is_none()
    {
        for candidate in record
            .playable_actions(player, &[])
            .map_err(ApiError::Game)?
            .into_iter()
            .filter_map(|action| match action {
                PlayableAction::UseSpiritSkill(candidate) => Some(candidate),
                _ => None,
            })
        {
            let id = format!("{:?}", candidate.skill);
            let duplicate = playable_actions.iter().any(|action| {
                matches!(
                    action,
                    WebPlayableAction::UseSpiritSkill {
                        id: existing,
                        selected_card: None,
                        declared_level: None,
                        ..
                    } if existing == &id
                )
            });
            if !duplicate {
                playable_actions.push(WebPlayableAction::UseSpiritSkill {
                    id,
                    name: candidate.skill_name,
                    summary: candidate.rule_text,
                    cards: Vec::new(),
                    selected_card: None,
                    declared_level: None,
                });
            }
        }
    }

    Ok(ApiResponse {
        record: record.recorded_decisions(),
        state: WebPublicGameState::from_public(
            record.public_view(viewer.clone()).map_err(ApiError::Game)?,
            card_labels,
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
                WebPublicGameEvent::from_public(index + 1, event, card_labels, formation_names)
            })
            .collect(),
        playable_actions,
        interaction: WebInteraction {
            can_pass,
            has_optional_effect: can_retrieve_discard,
            can_retrieve_discard,
        },
        trusted_random_candidates: None,
        pending_randomness_request: record.state().pending_randomness.clone(),
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
enum ApiAction {
    Start,
    Refresh,
    AdvanceAutomatic,
    PassAction,
    PlayableActions {
        player: String,
        cards: Vec<CardInstanceId>,
    },
    TrustedRandomHandCandidates {
        player: String,
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
    pending_randomness_request: Option<PendingRandomness>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebInteraction {
    can_pass: bool,
    has_optional_effect: bool,
    can_retrieve_discard: bool,
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
    discard: Vec<WebCard>,
    player_decks: Vec<WebPlayerDeck>,
    player_discards: Vec<WebPlayerDiscard>,
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
    prepared_profession_abilities: Vec<WebPreparedProfessionAbility>,
    spirits: Vec<WebPlayerSpirit>,
    previous_turn_formation: Option<WebPreviousTurnFormation>,
}

impl WebPublicGameState {
    fn from_public(
        state: PublicGameState,
        labels: &HashMap<CardInstanceId, String>,
        formation_names: &HashMap<String, String>,
    ) -> Self {
        let enabled_rule_modules = state.enabled_rule_modules.clone();
        Self {
            enabled_rule_modules: enabled_rule_modules
                .iter()
                .map(|module| module.as_str().to_string())
                .collect(),
            status: match &state.status {
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
                    cards: WebCardRefs::from_public(hand.cards, labels),
                })
                .collect(),
            discard: state
                .discard
                .into_iter()
                .map(|card| WebCard::from_id(card, labels))
                .collect(),
            player_decks: state
                .player_decks
                .into_iter()
                .map(|pile| WebPlayerDeck {
                    player: pile.player.as_str().to_string(),
                    cards: WebCardRefs::from_public(pile.cards, labels),
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
                        .map(|card| WebCard::from_id(card, labels))
                        .collect(),
                })
                .collect(),
            covered_passives: state
                .covered_passives
                .into_iter()
                .map(|passive| WebCoveredPassive {
                    owner: passive.owner.as_str().to_string(),
                    formation_id: passive.formation_id,
                    cards: WebCardRefs::from_public(passive.cards, labels),
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
            pending_choice: state
                .pending_choice
                .map(|choice| WebPendingChoice::from_public(choice, labels)),
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
                .map(|status| WebStatus {
                    id: status.id,
                    owner: match status.owner {
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
                    },
                    kind: status.kind,
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
                .map(|use_count| WebLimitedUse {
                    owner: use_count.owner.as_str().to_string(),
                    key: use_count.key,
                    remaining: use_count.remaining,
                    maximum: use_count.maximum,
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
                    melody_id: schedule.melody_id,
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
                    formation_id: suppression.formation_id,
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
            prepared_profession_abilities: state
                .prepared_profession_abilities
                .into_iter()
                .map(|prepared| WebPreparedProfessionAbility {
                    player: prepared.player.as_str().to_string(),
                    ability_id: prepared.ability_id,
                    card: prepared.card,
                    element: format!("{:?}", prepared.element),
                    level: prepared.level,
                    allowed_formation_scope: prepared.allowed_formation_scope,
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
                    cards: WebCardRefs::from_public(formation.cards, labels),
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
    remaining: u32,
    maximum: u32,
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
#[serde(rename_all = "camelCase")]
struct WebPreparedProfessionAbility {
    player: String,
    ability_id: String,
    card: CardInstanceId,
    element: String,
    level: u32,
    allowed_formation_scope: Vec<String>,
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
struct WebCoveredPassive {
    owner: String,
    formation_id: Option<String>,
    cards: WebCardRefs,
    star_substitution: Option<WebStarElementSubstitution>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebStarElementSubstitution {
    card: CardInstanceId,
    printed_element: crate::domain::Element,
    interpreted_element: crate::domain::Element,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebFormationMatchOption {
    role: String,
    card: CardInstanceId,
    slots: usize,
    preview: Option<String>,
}

impl From<StarElementSubstitution> for WebStarElementSubstitution {
    fn from(substitution: StarElementSubstitution) -> Self {
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
    kind: String,
    cards: Vec<WebCard>,
    required_count: usize,
    minimum_count: usize,
    maximum_count: usize,
    players: Vec<String>,
    formations: Vec<String>,
    environments: Vec<String>,
    can_decline: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPendingRandomness {
    request_id: String,
    deck: String,
    card_count: usize,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebScheduledEcho {
    player: String,
    melody_id: String,
    due_turn_number: u64,
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
    formation_id: String,
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
    ) -> Self {
        let (minimum_count, maximum_count) = match &choice.kind {
            PublicPendingChoiceKind::Known(kind) => kind.selection_bounds(),
            PublicPendingChoiceKind::Hidden => (0, 0),
        };
        let required_count = minimum_count;

        match choice.kind {
            PublicPendingChoiceKind::Known(PendingChoiceKind::TurnDrawDiscard {
                allowed_discards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                kind: "TurnDrawDiscard".to_string(),
                required_count,
                minimum_count,
                maximum_count,
                cards: allowed_discards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels))
                    .collect(),
                players: Vec::new(),
                formations: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
            PublicPendingChoiceKind::Known(PendingChoiceKind::EffectGenerated {
                allowed_cards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                kind: "EffectGenerated".to_string(),
                required_count,
                minimum_count,
                maximum_count,
                cards: allowed_cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels))
                    .collect(),
                players: Vec::new(),
                formations: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
            PublicPendingChoiceKind::Known(PendingChoiceKind::CardSetChoice {
                allowed_cards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                kind: "EffectGenerated".to_string(),
                required_count,
                minimum_count,
                maximum_count,
                cards: allowed_cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels))
                    .collect(),
                players: Vec::new(),
                formations: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
            PublicPendingChoiceKind::Known(PendingChoiceKind::TypedEffect { options, .. }) => {
                Self {
                    player: choice.player.as_str().to_string(),
                    purpose: choice.purpose,
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
                                .map(|card| WebCard::from_id(card, labels))
                                .collect()
                        })
                        .unwrap_or_default(),
                    players: options
                        .players
                        .into_iter()
                        .map(|player| player.as_str().to_string())
                        .collect(),
                    formations: options.formations,
                    environments: options
                        .environments
                        .into_iter()
                        .map(|environment| format!("{environment:?}"))
                        .collect(),
                    can_decline: options.can_decline,
                }
            }
            PublicPendingChoiceKind::Hidden => Self {
                player: choice.player.as_str().to_string(),
                purpose: choice.purpose,
                kind: "Hidden".to_string(),
                cards: Vec::new(),
                required_count: 0,
                minimum_count: 0,
                maximum_count: 0,
                players: Vec::new(),
                formations: Vec::new(),
                environments: Vec::new(),
                can_decline: false,
            },
        }
    }
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
    fn from_public(cards: PublicCardRefs, labels: &HashMap<CardInstanceId, String>) -> Self {
        match cards {
            PublicCardRefs::Known(cards) => Self::Known {
                cards: cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels))
                    .collect(),
            },
            PublicCardRefs::Hidden { count } => Self::Hidden { count },
            PublicCardRefs::PartiallyKnown { cards } => Self::PartiallyKnown {
                cards: cards
                    .into_iter()
                    .map(|card| card.map(|card| WebCard::from_id(card, labels)))
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
}

impl WebCard {
    fn from_id(id: CardInstanceId, labels: &HashMap<CardInstanceId, String>) -> Self {
        Self {
            id,
            label: labels
                .get(&id)
                .cloned()
                .unwrap_or_else(|| format!("{id:?}")),
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
    ) -> Self {
        let (title, summary) = event_presentation(&event, labels, formation_names);
        Self {
            id: format!("event-{sequence}"),
            event_type: event_type(&event),
            title,
            summary,
        }
    }
}

#[derive(Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
enum WebPlayableAction {
    PerformFormation {
        id: String,
        name: String,
        category: WebFormationCategory,
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

#[derive(Serialize)]
enum WebFormationCategory {
    Attack,
    Spell,
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
    }
}

fn event_presentation(
    event: &PublicGameEvent,
    labels: &HashMap<CardInstanceId, String>,
    formation_names: &HashMap<String, String>,
) -> (String, String) {
    match event {
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
        PublicGameEvent::Public(event) => game_event_presentation(event, labels, formation_names),
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
            format!("{} 需要為 {} 作出選擇。", player.as_str(), purpose),
        ),
        PublicGameEvent::RandomnessRequested { card_count, .. } => (
            "等待洗牌".to_string(),
            format!("正在重新排列 {card_count} 張牌。"),
        ),
        PublicGameEvent::RandomnessResolved { card_count, .. } => (
            "完成洗牌".to_string(),
            format!("已重新排列 {card_count} 張牌。"),
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
                    .map(|card| format!("，指定牌 {}", card.as_u64()))
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
                        card.as_u64(),
                        level
                    )
                },
            ),
        ),
    }
}

fn game_event_presentation(
    event: &GameEvent,
    labels: &HashMap<CardInstanceId, String>,
    formation_names: &HashMap<String, String>,
) -> (String, String) {
    match event {
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
                previous
                    .as_ref()
                    .and_then(crate::rules::hero::profession)
                    .map(|profession| profession.name)
                    .unwrap_or("無職業"),
                crate::rules::hero::profession(profession)
                    .map(|profession| profession.name)
                    .unwrap_or(profession.as_str())
            ),
        ),
        GameEvent::ProfessionTransformed {
            player,
            profession,
            reason,
            ..
        } => (
            "職業轉化".to_string(),
            format!(
                "{} 因 {} 轉化為 {}。",
                player.as_str(),
                reason,
                profession.as_str()
            ),
        ),
        GameEvent::ProfessionBroken { player, profession } => (
            "職業破除".to_string(),
            format!(
                "{} 的{}已被破除。",
                player.as_str(),
                crate::rules::hero::profession(profession)
                    .map(|profession| profession.name)
                    .unwrap_or(profession.as_str())
            ),
        ),
        GameEvent::ProfessionAbilityActivated {
            player,
            ability_id,
            prepared,
        } => (
            "發動職業能力".to_string(),
            prepared.as_ref().map_or_else(
                || format!("{} 發動了「{}」。", player.as_str(), ability_id),
                |prepared| {
                    format!(
                        "{} 發動「{}」，將牌 {} 準備為 {:?} {} 級。",
                        player.as_str(),
                        ability_id,
                        prepared.card.as_u64(),
                        prepared.element,
                        prepared.level
                    )
                },
            ),
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
                "{} 的 {:?} 轉化為 {:?}，保留 {power} 點靈力。",
                player.as_str(),
                previous,
                spirit
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
                card.as_u64(),
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
            format!("{} 進入 {:?}。", state.owner.as_str(), state.kind),
        ),
        GameEvent::JianghuStateExpired { owner, kind } => (
            "江湖狀態結束".to_string(),
            format!("{} 的 {:?} 已結束。", owner.as_str(), kind),
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
            new_remaining,
            maximum,
            ..
        } => (
            "次數限制".to_string(),
            format!(
                "{} 的 {key} 剩餘 {new_remaining}/{maximum} 次。",
                owner.as_str()
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
        GameEvent::DiscardRecycledIntoDeck { shuffled_order, .. } => (
            "重整牌庫".to_string(),
            format!("棄牌堆的 {} 張牌已重新放回牌庫。", shuffled_order.len()),
        ),
        GameEvent::PlayerDiscardRecycledIntoDeck {
            player,
            shuffled_order,
            ..
        } => (
            "重整個人牌庫".to_string(),
            format!(
                "{} 的棄牌堆有 {} 張牌重新放回牌庫。",
                player.as_str(),
                shuffled_order.len()
            ),
        ),
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
                "{} 指定 {} 的陣法 {} 於下回合無效。",
                suppression.source.as_str(),
                suppression.target.as_str(),
                suppression.formation_id
            ),
        ),
        GameEvent::FormationSuppressionExpired {
            target,
            formation_id,
            ..
        } => (
            "裂土結束".to_string(),
            format!(
                "{} 的陣法 {} 不再受裂土影響。",
                target.as_str(),
                formation_id
            ),
        ),
        GameEvent::RingingMetalCardRevealed { selection } => (
            "鳴金檢索".to_string(),
            format!(
                "{} 展示了牌 {}。",
                selection.player.as_str(),
                selection.card.as_u64()
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
            format!("{} 已執行曲調 {} 的主效果。", player.as_str(), melody_id),
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

fn card_element_name(element: crate::domain::Element) -> &'static str {
    match element {
        crate::domain::Element::Metal => "金行牌",
        crate::domain::Element::Wood => "木行牌",
        crate::domain::Element::Water => "水行牌",
        crate::domain::Element::Fire => "火行牌",
        crate::domain::Element::Earth => "土行牌",
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
            .map(|card| {
                labels
                    .get(card)
                    .cloned()
                    .unwrap_or_else(|| format!("{card:?}"))
            })
            .collect::<Vec<_>>()
            .join("、"),
        PublicCardRefs::Hidden { count } => format!("{count} 張牌"),
        PublicCardRefs::PartiallyKnown { cards } => {
            let known = cards.iter().flatten().count();
            format!("{} 張牌（其中 {known} 張公開）", cards.len())
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

    #[test]
    fn start_request_returns_default_game_state() {
        let response = handle_request_json(r#"{"action":{"type":"start"},"viewer":"alice"}"#)
            .expect("start request should succeed");

        assert!(response.contains(r#""turnNumber":1"#));
        assert!(response.contains(r#""record""#));
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
        let public = crate::public_view::state_for(&state, Viewer::Observer);
        let web = WebPublicGameState::from_public(
            public,
            &rules.card_labels(&setup).unwrap(),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web).unwrap();

        assert_eq!(json["spirits"][0]["player"], "alice");
        assert_eq!(json["spirits"][0]["spirit"], "Fire");
        assert_eq!(json["spirits"][0]["power"], 4);

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
        assert_eq!(card_element_name(crate::domain::Element::Water), "水行牌");
        let substitution = WebStarElementSubstitution {
            card: CardInstanceId::new(42),
            printed_element: crate::domain::Element::Water,
            interpreted_element: crate::domain::Element::Wood,
        };
        let candidate = WebPlayableAction::PerformFormation {
            id: "defense".to_string(),
            name: "防禦".to_string(),
            category: WebFormationCategory::Spell,
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
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web_state).expect("web state should serialize");

        assert_eq!(json["environment"], "Water");
    }

    #[test]
    fn status_owner_is_projected_as_structured_player_data() {
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
        let web_state = WebPublicGameState::from_public(
            crate::public_view::state_for(&state, Viewer::Player(PlayerId::new("alice"))),
            &rules.card_labels(&setup).unwrap(),
            &rules.formation_names(&setup).unwrap(),
        );
        let json = serde_json::to_value(web_state).expect("web state should serialize");

        assert_eq!(json["statuses"][0]["owner"]["kind"], "player");
        assert_eq!(json["statuses"][0]["owner"]["id"], "alice");
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
            kind: PublicPendingChoiceKind::Known(PendingChoiceKind::EffectGenerated {
                effect_id: "chaos".to_string(),
                continuation_id: "chaos:return-two".to_string(),
                allowed_cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
            }),
        };
        let web_choice = WebPendingChoice::from_public(choice, &HashMap::new());
        let json = serde_json::to_value(web_choice).expect("choice should serialize");

        assert_eq!(json["requiredCount"], 2);
        assert_eq!(json["purpose"], "chaos");
    }

    #[test]
    fn hidden_card_refs_use_plain_player_facing_copy() {
        assert_eq!(
            card_refs_summary(&PublicCardRefs::Hidden { count: 5 }, &HashMap::new()),
            "5 張牌"
        );
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
        let web = WebPublicGameState::from_public(public, &HashMap::new(), &HashMap::new());
        let json = serde_json::to_value(web).expect("Echo state should serialize");

        assert_eq!(json["scheduledEchoes"][0]["melodyId"], "echo:falling-wood");
        assert_eq!(json["scheduledEchoes"][0]["dueTurnNumber"], 3);
        assert_eq!(json["flowStates"][0]["layers"], 2);
        assert_eq!(json["formationSuppressions"][0]["expiresOnTurnNumber"], 2);
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
}
