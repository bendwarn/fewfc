use fewfc::application::{BaseRuleset, GameRecord, RecordedDecision};
use fewfc::domain::{
    CardDef, CardDefId, CardInstanceDef, CardInstanceId, Command, GameError, GameSetup,
    PassActionReason, PendingChoiceKind, PlayerId, TargetDecl,
};
use fewfc::public_view::{
    PublicCardRefs, PublicGameEvent, PublicGameState, PublicPendingChoiceKind, Viewer,
};
use fewfc::rules::Element;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{self, Read};

fn main() {
    if let Err(error) = run() {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let mut input = String::new();
    io::stdin()
        .read_to_string(&mut input)
        .map_err(|error| error.to_string())?;
    let request: ApiRequest = serde_json::from_str(&input).map_err(|error| error.to_string())?;
    let response = handle(request).map_err(|error| serde_json::to_string(&error).unwrap())?;
    println!(
        "{}",
        serde_json::to_string(&response).map_err(|error| error.to_string())?
    );
    Ok(())
}

fn handle(request: ApiRequest) -> Result<ApiResponse, ApiError> {
    let setup = sample_setup();
    let card_labels = card_labels(&setup);
    let viewer = viewer_from_request(request.viewer.as_deref());
    let mut record = record_from_request(&setup, request.record)?;

    match request.action {
        ApiAction::Start => {
            record = GameRecord::start(sample_setup(), sample_deck()).map_err(ApiError::Game)?;
            let _ = record.advance_until_decision().map_err(ApiError::Game)?;
        }
        ApiAction::Refresh => {}
        ApiAction::AdvanceAutomatic => {
            let _ = record.advance_until_decision().map_err(ApiError::Game)?;
        }
        ApiAction::PassAction => {
            let current_player = record
                .state()
                .map_err(ApiError::Game)?
                .current_player()
                .cloned()
                .ok_or_else(|| ApiError::Message("missing current player".to_string()))?;
            let _ = record
                .handle(Command::PassAction {
                    player: current_player,
                    reason: PassActionReason::NoCardsInHand,
                })
                .map_err(ApiError::Game)?;
            advance_after_command(&mut record)?;
        }
        ApiAction::PlayableFormations { player, cards } => {
            let state = record.state().map_err(ApiError::Game)?;
            let candidates = BaseRuleset::new()
                .playable_formations(&state, &PlayerId::new(player), &cards)
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
                        category: formation_category(&candidate.cards),
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
    let _ = record.advance_until_decision().map_err(ApiError::Game)?;
    Ok(())
}

fn record_from_request(
    setup: &GameSetup,
    record: Option<Vec<RecordedDecision>>,
) -> Result<GameRecord, ApiError> {
    match record {
        Some(recorded_decisions) if !recorded_decisions.is_empty() => {
            GameRecord::from_recorded_decisions(setup.clone(), recorded_decisions, None)
                .map_err(|error| ApiError::Message(format!("{error:?}")))
        }
        _ => GameRecord::start(setup.clone(), sample_deck()).map_err(ApiError::Game),
    }
}

fn response_for(
    record: &GameRecord,
    viewer: Viewer,
    card_labels: &HashMap<CardInstanceId, String>,
    playable_formations: Vec<WebPlayableFormation>,
) -> Result<ApiResponse, ApiError> {
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
    })
}

fn sample_setup() -> GameSetup {
    let defs = vec![
        card_def("metal", "金", Element::Metal, 1),
        card_def("wood", "木", Element::Wood, 1),
        card_def("water", "水", Element::Water, 1),
        card_def("fire", "火", Element::Fire, 1),
        card_def("earth", "土", Element::Earth, 1),
    ];
    let instances = (1..=20)
        .map(|id| {
            let def = match id % 5 {
                1 => "metal",
                2 => "wood",
                3 => "water",
                4 => "fire",
                _ => "earth",
            };
            CardInstanceDef {
                instance: CardInstanceId::new(id),
                definition: CardDefId::new(def),
            }
        })
        .collect();

    GameSetup::two_player(PlayerId::new("alice"), PlayerId::new("bob"), 20)
        .with_cards(defs, instances)
}

fn card_def(id: &str, name: &str, element: Element, level: u32) -> CardDef {
    CardDef {
        id: CardDefId::new(id),
        name: name.to_string(),
        element,
        level,
    }
}

fn sample_deck() -> Vec<CardInstanceId> {
    (1..=20).map(CardInstanceId::new).collect()
}

fn card_labels(setup: &GameSetup) -> HashMap<CardInstanceId, String> {
    setup
        .card_instances
        .iter()
        .filter_map(|instance| {
            let card_def = setup
                .card_defs
                .iter()
                .find(|card_def| card_def.id == instance.definition)?;
            Some((
                instance.instance,
                format!("{} {}", card_def.name, card_def.level),
            ))
        })
        .collect()
}

fn formation_category(cards: &[CardInstanceId]) -> WebFormationCategory {
    if cards.len() == 1 {
        WebFormationCategory::Attack
    } else {
        WebFormationCategory::Spell
    }
}

#[derive(Serialize, Deserialize)]
struct ApiRequest {
    action: ApiAction,
    viewer: Option<String>,
    record: Option<Vec<RecordedDecision>>,
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

fn viewer_from_request(viewer: Option<&str>) -> Viewer {
    match viewer {
        Some("alice") | Some("bob") => Viewer::Player(PlayerId::new(viewer.unwrap())),
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
        choice: fewfc::public_view::PublicPendingChoice,
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
