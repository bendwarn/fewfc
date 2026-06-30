use crate::application::{GameRecord, RecordedDecision};
use crate::domain::{
    CardDefId, CardInstanceId, Command, DISCARD_RETRIEVAL_MODULE_ID, GameError, GameEvent,
    GameSetup, PassActionReason, PendingChoiceKind, Phase, Player, PlayerDeckList, PlayerId,
    RuleModuleId, StatusOwner, TargetDecl, TeamId, TurnDrawSkipReason,
};
use crate::public_view::{
    PublicCardRefs, PublicGameEvent, PublicGameState, PublicPendingChoiceKind, Viewer,
};
use crate::rules::{FormationCategory, OfficialRules};
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
        ApiAction::PlayableFormations { player, cards } => {
            let candidates = record
                .playable_formations(&PlayerId::new(player), &cards)
                .map_err(ApiError::Game)?;
            return response_for(
                &record,
                viewer,
                &card_labels,
                &formation_names,
                candidates
                    .into_iter()
                    .map(|candidate| WebPlayableFormation {
                        id: candidate.formation_id,
                        name: candidate.formation_name,
                        category: WebFormationCategory::from(candidate.category),
                        summary: candidate.rule_text,
                    })
                    .collect(),
            );
        }
        ApiAction::PerformFormation {
            player,
            formation_id,
            cards,
        } => {
            let _ = record
                .handle(Command::PerformFormation {
                    player: PlayerId::new(player),
                    formation_id,
                    cards,
                    declared_targets: Vec::<TargetDecl>::new(),
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
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
    playable_formations: Vec<WebPlayableFormation>,
) -> Result<ApiResponse, ApiError> {
    let can_pass = pass_action_for_state(record.state()).is_some();
    let can_retrieve_discard = can_retrieve_discard(record.state());

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
        playable_formations,
        interaction: WebInteraction {
            can_pass,
            has_optional_effect: can_retrieve_discard,
            can_retrieve_discard,
        },
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
}

#[derive(Serialize, Deserialize)]
struct WebSetupPlayer {
    id: String,
    team: String,
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
    PlayableFormations {
        player: String,
        cards: Vec<CardInstanceId>,
    },
    PerformFormation {
        player: String,
        #[serde(rename = "formationId")]
        formation_id: String,
        cards: Vec<CardInstanceId>,
    },
    ChooseTurnDiscard {
        player: String,
        card: CardInstanceId,
    },
    AnswerEffectChoice {
        player: String,
        cards: Vec<CardInstanceId>,
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
    rules
        .configure_game_with_decks(players, turn_order, modules, deck_lists)
        .map_err(ApiError::Game)
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
    playable_formations: Vec<WebPlayableFormation>,
    interaction: WebInteraction,
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
    shields: Vec<WebShield>,
    statuses: Vec<WebStatus>,
    previous_turn_formation: Option<WebPreviousTurnFormation>,
}

impl WebPublicGameState {
    fn from_public(
        state: PublicGameState,
        labels: &HashMap<CardInstanceId, String>,
        formation_names: &HashMap<String, String>,
    ) -> Self {
        Self {
            enabled_rule_modules: state
                .enabled_rule_modules
                .into_iter()
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
    kind: String,
    cards: Vec<WebCard>,
    required_count: usize,
}

impl WebPendingChoice {
    fn from_public(
        choice: crate::public_view::PublicPendingChoice,
        labels: &HashMap<CardInstanceId, String>,
    ) -> Self {
        let required_count = match &choice.kind {
            PublicPendingChoiceKind::Known(kind) => kind.required_count(),
            PublicPendingChoiceKind::Hidden => 0,
        };

        match choice.kind {
            PublicPendingChoiceKind::Known(PendingChoiceKind::TurnDrawDiscard {
                allowed_discards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                kind: "TurnDrawDiscard".to_string(),
                required_count,
                cards: allowed_discards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels))
                    .collect(),
            },
            PublicPendingChoiceKind::Known(PendingChoiceKind::EffectGenerated {
                allowed_cards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                kind: "EffectGenerated".to_string(),
                required_count,
                cards: allowed_cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels))
                    .collect(),
            },
            PublicPendingChoiceKind::Hidden => Self {
                player: choice.player.as_str().to_string(),
                kind: "Hidden".to_string(),
                cards: Vec::new(),
                required_count: 0,
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
#[serde(rename_all = "camelCase")]
struct WebPlayableFormation {
    id: String,
    name: String,
    category: WebFormationCategory,
    summary: String,
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
        PublicGameEvent::EffectChoiceRequested { .. } => "EffectChoiceRequested".to_string(),
        PublicGameEvent::HandInspected { .. } => "HandInspected".to_string(),
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
        PublicGameEvent::EffectChoiceRequested { player, .. } => (
            "效果選擇".to_string(),
            format!("{} 需要選擇效果。", player.as_str()),
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
        GameEvent::FormationEffectCopied { player, effect_id } => (
            "幻化".to_string(),
            format!(
                "{} 的幻化複製了「{}」。",
                player.as_str(),
                formation_name(formation_names, effect_id)
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
        GameEvent::PassiveCovered { player, cards, .. } => (
            "蓋牌".to_string(),
            format!("{} 蓋下了 {} 張牌。", player.as_str(), cards.len()),
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
        GameEvent::TurnEnded { player } => (
            "回合結束".to_string(),
            format!("{} 的回合結束。", player.as_str()),
        ),
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
    fn playable_formation_uses_the_ruleset_rule_text() {
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
                "type": "playableFormations",
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
        let summary = response["playableFormations"][0]["summary"]
            .as_str()
            .expect("candidate should have rule text");

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
            kind: PublicPendingChoiceKind::Known(PendingChoiceKind::EffectGenerated {
                effect_id: "chaos".to_string(),
                continuation_id: "chaos:return-two".to_string(),
                allowed_cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
            }),
        };
        let web_choice = WebPendingChoice::from_public(choice, &HashMap::new());
        let json = serde_json::to_value(web_choice).expect("choice should serialize");

        assert_eq!(json["requiredCount"], 2);
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
}
