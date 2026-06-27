use crate::application::{BaseRuleset, GameRecord, RecordedDecision};
use crate::domain::{
    CardInstanceId, Command, GameError, GameSetup, PassActionReason, PendingChoiceKind, Phase,
    Player, PlayerId, StatusOwner, TargetDecl, TeamHp, TeamId,
};
use crate::public_view::{
    PublicCardRefs, PublicGameEvent, PublicGameState, PublicPendingChoiceKind, Viewer,
};
use crate::rules::FormationCategory;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub fn handle_request_json(input: &str) -> Result<String, String> {
    let request: ApiRequest = serde_json::from_str(input).map_err(|error| error.to_string())?;
    let response = handle(request).map_err(|error| serde_json::to_string(&error).unwrap())?;

    serde_json::to_string(&response).map_err(|error| error.to_string())
}

fn handle(request: ApiRequest) -> Result<ApiResponse, ApiError> {
    let ruleset = BaseRuleset::new();
    let setup = setup_for_request(&ruleset, request.setup, request.first_player.as_deref())?;
    let card_labels = ruleset.card_labels(&setup);
    let viewer = viewer_from_request(request.viewer.as_deref());
    let deck_seed = request.deck_seed.clone();
    let mut record = record_from_request(&ruleset, &setup, request.record, deck_seed.as_deref())?;

    match request.action {
        ApiAction::Start => {
            record = GameRecord::start(
                setup.clone(),
                deck_order_for_start(&ruleset, &setup, deck_seed.as_deref()),
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
            let candidates = ruleset
                .playable_formations(record.state(), &PlayerId::new(player), &cards)
                .map_err(ApiError::Game)?;
            return Ok(response_for(
                &record,
                viewer,
                &card_labels,
                candidates
                    .into_iter()
                    .map(|candidate| WebPlayableFormation {
                        id: candidate.formation_id,
                        name: candidate.formation_name,
                        category: WebFormationCategory::from(candidate.category),
                        summary: format!("使用 {} 張牌發動。", candidate.cards.len()),
                    })
                    .collect(),
            )?);
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
    }

    response_for(&record, viewer, &card_labels, Vec::new())
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
    ruleset: &BaseRuleset,
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
            deck_order_for_start(ruleset, setup, deck_seed),
        )
        .map_err(ApiError::Game),
    }
}

fn response_for(
    record: &GameRecord,
    viewer: Viewer,
    card_labels: &HashMap<CardInstanceId, String>,
    playable_formations: Vec<WebPlayableFormation>,
) -> Result<ApiResponse, ApiError> {
    let can_pass = pass_action_for_state(record.state()).is_some();

    Ok(ApiResponse {
        record: record.recorded_decisions(),
        state: WebPublicGameState::from_public(
            record.public_view(viewer.clone()).map_err(ApiError::Game)?,
            card_labels,
        ),
        events: record
            .public_events_for(viewer)
            .into_iter()
            .enumerate()
            .rev()
            .map(|(index, event)| WebPublicGameEvent::from_public(index + 1, event, card_labels))
            .collect(),
        playable_formations,
        interaction: WebInteraction {
            can_pass,
            // The current base ruleset does not yet model optional active-effect
            // Commands separately from the turn-closing Action Command.
            has_optional_effect: false,
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
}

#[derive(Serialize, Deserialize)]
struct WebSetupPlayer {
    id: String,
    team: String,
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
}

fn setup_for_first_player(ruleset: &BaseRuleset, first_player: Option<&str>) -> GameSetup {
    if first_player == Some("bob") {
        let mut setup = GameSetup::two_player(PlayerId::new("bob"), PlayerId::new("alice"), 20);
        let sample = ruleset.sample_game_setup();
        setup.card_defs = sample.card_defs;
        setup.card_instances = sample.card_instances;
        return setup;
    }

    ruleset.sample_game_setup()
}

fn setup_for_request(
    ruleset: &BaseRuleset,
    requested: Option<WebGameSetup>,
    first_player: Option<&str>,
) -> Result<GameSetup, ApiError> {
    let Some(requested) = requested else {
        return Ok(setup_for_first_player(ruleset, first_player));
    };

    if requested.players.is_empty() {
        return Err(ApiError::Message(
            "game setup must contain players".to_string(),
        ));
    }

    let mut setup = ruleset.sample_game_setup();
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
    let mut seen_teams = HashSet::new();
    let hp = players
        .iter()
        .filter_map(|player| {
            seen_teams.insert(player.team.clone()).then_some(TeamHp {
                team: player.team.clone(),
                hp: 20,
            })
        })
        .collect::<Vec<_>>();

    setup.players = players;
    setup.turn_order = turn_order;
    setup.hp = hp;

    Ok(setup)
}

fn deck_order_for_start(
    ruleset: &BaseRuleset,
    setup: &GameSetup,
    deck_seed: Option<&str>,
) -> Vec<CardInstanceId> {
    let mut deck_order = ruleset.sample_deck_order(setup);
    shuffle_deck(
        &mut deck_order,
        deck_seed.unwrap_or("fewfc-default-shuffle"),
    );
    deck_order
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
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPublicGameState {
    status: String,
    turn_number: u64,
    phase: String,
    current_player: Option<String>,
    players: Vec<WebPlayer>,
    turn_order: Vec<String>,
    hp: Vec<WebTeamHp>,
    hands: Vec<WebPlayerHand>,
    discard: Vec<WebCard>,
    covered_passives: Vec<WebCoveredPassive>,
    pending_choice: Option<WebPendingChoice>,
    shields: Vec<WebShield>,
    statuses: Vec<WebStatus>,
}

impl WebPublicGameState {
    fn from_public(state: PublicGameState, labels: &HashMap<CardInstanceId, String>) -> Self {
        Self {
            status: format!("{:?}", state.status),
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
            covered_passives: state
                .covered_passives
                .into_iter()
                .map(|passive| WebCoveredPassive {
                    owner: passive.owner.as_str().to_string(),
                    formation_id: passive.formation_id,
                    cards: WebCardRefs::from_public(passive.cards, labels),
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
                    owner: format!("{:?}", status.owner),
                    kind: status.kind,
                })
                .collect(),
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
struct WebCoveredPassive {
    owner: String,
    formation_id: String,
    cards: WebCardRefs,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct WebPendingChoice {
    player: String,
    kind: String,
    cards: Vec<WebCard>,
}

impl WebPendingChoice {
    fn from_public(
        choice: crate::public_view::PublicPendingChoice,
        labels: &HashMap<CardInstanceId, String>,
    ) -> Self {
        match choice.kind {
            PublicPendingChoiceKind::Known(PendingChoiceKind::TurnDrawDiscard {
                allowed_discards,
                ..
            }) => Self {
                player: choice.player.as_str().to_string(),
                kind: "TurnDrawDiscard".to_string(),
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
                cards: allowed_cards
                    .into_iter()
                    .map(|card| WebCard::from_id(card, labels))
                    .collect(),
            },
            PublicPendingChoiceKind::Hidden => Self {
                player: choice.player.as_str().to_string(),
                kind: "Hidden".to_string(),
                cards: Vec::new(),
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
    owner: String,
    kind: String,
}

#[derive(Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
enum WebCardRefs {
    Known { cards: Vec<WebCard> },
    Hidden { count: usize },
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
    summary: String,
}

impl WebPublicGameEvent {
    fn from_public(
        sequence: usize,
        event: PublicGameEvent,
        labels: &HashMap<CardInstanceId, String>,
    ) -> Self {
        let summary = event_summary(&event, labels);
        Self {
            id: format!("event-{sequence}"),
            event_type: event_type(&event),
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
        PublicGameEvent::CardsDealt { .. } => "CardsDealt".to_string(),
        PublicGameEvent::PassiveCovered { .. } => "PassiveCovered".to_string(),
        PublicGameEvent::CardsDrawnForTurnDiscardChoice { .. } => {
            "CardsDrawnForTurnDiscardChoice".to_string()
        }
        PublicGameEvent::EffectChoiceRequested { .. } => "EffectChoiceRequested".to_string(),
    }
}

fn event_summary(event: &PublicGameEvent, labels: &HashMap<CardInstanceId, String>) -> String {
    match event {
        PublicGameEvent::CardsDealt { player, cards } => {
            format!(
                "{} 收到 {}。",
                player.as_str(),
                card_refs_summary(cards, labels)
            )
        }
        PublicGameEvent::DeckPrepared { deck } => {
            format!("牌庫已準備：{}。", card_refs_summary(deck, labels))
        }
        PublicGameEvent::Public(event) => format!("{event:?}"),
        PublicGameEvent::PassiveCovered {
            player,
            formation_id,
            cards,
        } => format!(
            "{} 覆蓋 {}，使用 {}。",
            player.as_str(),
            formation_id,
            card_refs_summary(cards, labels)
        ),
        PublicGameEvent::CardsDrawnForTurnDiscardChoice {
            player,
            drawn_cards,
            ..
        } => format!(
            "{} 抽牌並需要棄置：{}。",
            player.as_str(),
            card_refs_summary(drawn_cards, labels)
        ),
        PublicGameEvent::EffectChoiceRequested { player, .. } => {
            format!("{} 需要選擇效果。", player.as_str())
        }
    }
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
        PublicCardRefs::Hidden { count } => format!("{count} 張隱藏牌"),
    }
}

#[derive(Serialize)]
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
        let ruleset = BaseRuleset::new();
        let setup = ruleset.sample_game_setup();
        let sorted_deck = ruleset.sample_deck_order(&setup);
        let first_shuffle = deck_order_for_start(&ruleset, &setup, Some("seed-a"));
        let same_shuffle = deck_order_for_start(&ruleset, &setup, Some("seed-a"));
        let different_shuffle = deck_order_for_start(&ruleset, &setup, Some("seed-b"));

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
    fn pass_reason_is_derived_from_the_current_state() {
        let mut state =
            crate::domain::GameState::from_setup(&BaseRuleset::new().sample_game_setup());
        state.phase = Phase::Main;
        let current = state.current_player().cloned().unwrap();
        state.hand_mut(&current).unwrap().clear();

        assert_eq!(
            pass_action_for_state(&state),
            Some((current, PassActionReason::NoCardsInHand))
        );
    }
}
