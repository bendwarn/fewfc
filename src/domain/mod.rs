//! Domain model: game state, ids, events, commands, and rule invariants.

use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerId(String);

impl PlayerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TeamId(String);

impl TeamId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

pub const BASE_RULESET_ID: &str = "base";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RulesetId(String);

impl RulesetId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn base() -> Self {
        Self::new(BASE_RULESET_ID)
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardInstanceId(u64);

impl CardInstanceId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CommandId(u64);

impl CommandId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }

    pub fn as_u64(&self) -> u64 {
        self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    Metal,
    Wood,
    Water,
    Fire,
    Earth,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardDefId(String);

impl CardDefId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CardDef {
    pub id: CardDefId,
    pub name: String,
    pub element: Element,
    pub level: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CardInstanceDef {
    pub instance: CardInstanceId,
    pub definition: CardDefId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: PlayerId,
    pub team: TeamId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerHand {
    pub player: PlayerId,
    pub cards: Vec<CardInstanceId>,
}

impl PlayerHand {
    pub fn new(player: PlayerId, cards: Vec<CardInstanceId>) -> Self {
        Self { player, cards }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TeamHp {
    pub team: TeamId,
    pub hp: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameStatus {
    InProgress,
    Finished { outcome: GameOutcome },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameOutcome {
    Team(TeamId),
    Draw,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerShield {
    pub player: PlayerId,
    pub value: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CoveredPassive {
    pub owner: PlayerId,
    pub formation_id: String,
    pub cards: Vec<CardInstanceId>,
    pub sealed: bool,
    pub covered_on_turn: u64,
    pub reveal_timing: PassiveTriggerTiming,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LastFormationUse {
    pub formation_id: String,
    pub resolved_turn: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveTriggerTiming {
    NextPlayerActionStart,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StatusEffect {
    pub id: String,
    pub owner: StatusOwner,
    pub kind: String,
    pub value: Option<i32>,
    pub duration: StatusDuration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum StatusOwner {
    Player(PlayerId),
    Team(TeamId),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum StatusDuration {
    UntilTurnStart { player: PlayerId },
    UntilTurnEnd { player: PlayerId },
    UntilTurnEndNumber { player: PlayerId, turn_number: u64 },
    Permanent,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum StatusExpiryTiming {
    TurnStart { player: PlayerId },
    TurnEnd { player: PlayerId },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GameSetup {
    pub ruleset: RulesetId,
    pub players: Vec<Player>,
    pub turn_order: Vec<PlayerId>,
    pub hp: Vec<TeamHp>,
    pub card_defs: Vec<CardDef>,
    pub card_instances: Vec<CardInstanceDef>,
    pub hand_limit: usize,
    pub base_draw: usize,
}

impl GameSetup {
    pub fn two_player(first_player: PlayerId, second_player: PlayerId, starting_hp: i32) -> Self {
        let first_team = TeamId::new(format!("team:{}", first_player.as_str()));
        let second_team = TeamId::new(format!("team:{}", second_player.as_str()));

        Self {
            ruleset: RulesetId::base(),
            players: vec![
                Player {
                    id: first_player.clone(),
                    team: first_team.clone(),
                },
                Player {
                    id: second_player.clone(),
                    team: second_team.clone(),
                },
            ],
            turn_order: vec![first_player, second_player],
            hp: vec![
                TeamHp {
                    team: first_team,
                    hp: starting_hp,
                },
                TeamHp {
                    team: second_team,
                    hp: starting_hp,
                },
            ],
            card_defs: Vec::new(),
            card_instances: Vec::new(),
            hand_limit: 5,
            base_draw: 2,
        }
    }

    pub fn team_mode(
        first_team: TeamId,
        first_players: Vec<PlayerId>,
        second_team: TeamId,
        second_players: Vec<PlayerId>,
        starting_hp: i32,
    ) -> Self {
        let mut players = Vec::new();
        let mut turn_order = Vec::new();

        for index in 0..first_players.len().max(second_players.len()) {
            if let Some(player) = first_players.get(index) {
                players.push(Player {
                    id: player.clone(),
                    team: first_team.clone(),
                });
                turn_order.push(player.clone());
            }

            if let Some(player) = second_players.get(index) {
                players.push(Player {
                    id: player.clone(),
                    team: second_team.clone(),
                });
                turn_order.push(player.clone());
            }
        }

        Self {
            ruleset: RulesetId::base(),
            players,
            turn_order,
            hp: vec![
                TeamHp {
                    team: first_team,
                    hp: starting_hp,
                },
                TeamHp {
                    team: second_team,
                    hp: starting_hp,
                },
            ],
            card_defs: Vec::new(),
            card_instances: Vec::new(),
            hand_limit: 5,
            base_draw: 2,
        }
    }

    pub fn with_cards(
        mut self,
        card_defs: Vec<CardDef>,
        card_instances: Vec<CardInstanceDef>,
    ) -> Self {
        self.card_defs = card_defs;
        self.card_instances = card_instances;
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    TurnStart,
    Main,
    TurnDraw,
    TurnDrawDiscardChoice,
    TurnEnd,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GameState {
    pub status: GameStatus,
    pub turn_number: u64,
    pub phase: Phase,
    pub current_turn_index: usize,
    pub players: Vec<Player>,
    pub turn_order: Vec<PlayerId>,
    pub hp: Vec<TeamHp>,
    pub card_defs: Vec<CardDef>,
    pub card_instances: Vec<CardInstanceDef>,
    pub deck: Vec<CardInstanceId>,
    pub hands: Vec<PlayerHand>,
    pub discard: Vec<CardInstanceId>,
    pub pending_choice: Option<PendingChoice>,
    pub shields: Vec<PlayerShield>,
    pub covered_passives: Vec<CoveredPassive>,
    pub statuses: Vec<StatusEffect>,
    pub last_elemental_attack_by_player: HashMap<PlayerId, LastElementalAttack>,
    pub last_formation_by_player: HashMap<PlayerId, LastFormationUse>,
    pub turn_draw_bonus_by_player: HashMap<PlayerId, usize>,
    pub hand_limit: usize,
    pub base_draw: usize,
}

impl GameState {
    pub fn from_setup(setup: &GameSetup) -> Self {
        Self {
            status: GameStatus::InProgress,
            turn_number: 1,
            phase: Phase::TurnStart,
            current_turn_index: 0,
            players: setup.players.clone(),
            turn_order: setup.turn_order.clone(),
            hp: setup.hp.clone(),
            card_defs: setup.card_defs.clone(),
            card_instances: setup.card_instances.clone(),
            deck: Vec::new(),
            hands: setup
                .players
                .iter()
                .map(|player| PlayerHand::new(player.id.clone(), Vec::new()))
                .collect(),
            discard: Vec::new(),
            pending_choice: None,
            shields: setup
                .players
                .iter()
                .map(|player| PlayerShield {
                    player: player.id.clone(),
                    value: 0,
                })
                .collect(),
            covered_passives: Vec::new(),
            statuses: Vec::new(),
            last_elemental_attack_by_player: HashMap::new(),
            last_formation_by_player: HashMap::new(),
            turn_draw_bonus_by_player: HashMap::new(),
            hand_limit: setup.hand_limit,
            base_draw: setup.base_draw,
        }
    }

    pub fn current_player(&self) -> Option<&PlayerId> {
        self.turn_order.get(self.current_turn_index)
    }

    pub fn hand(&self, player: &PlayerId) -> Option<&[CardInstanceId]> {
        self.hands
            .iter()
            .find(|hand| &hand.player == player)
            .map(|hand| hand.cards.as_slice())
    }

    pub fn hand_mut(&mut self, player: &PlayerId) -> Option<&mut Vec<CardInstanceId>> {
        self.hands
            .iter_mut()
            .find(|hand| &hand.player == player)
            .map(|hand| &mut hand.cards)
    }

    pub fn shield(&self, player: &PlayerId) -> Option<i32> {
        self.shields
            .iter()
            .find(|shield| &shield.player == player)
            .map(|shield| shield.value)
    }

    pub fn card_def(&self, instance: CardInstanceId) -> Option<&CardDef> {
        let definition = &self
            .card_instances
            .iter()
            .find(|card| card.instance == instance)?
            .definition;

        self.card_defs
            .iter()
            .find(|card_def| &card_def.id == definition)
    }

    pub fn card_element(&self, instance: CardInstanceId) -> Option<Element> {
        self.card_def(instance).map(|card_def| card_def.element)
    }

    pub fn view_for(&self, viewer: Viewer) -> PublicGameState {
        PublicGameState {
            status: self.status.clone(),
            turn_number: self.turn_number,
            phase: self.phase,
            current_player: self.current_player().cloned(),
            players: self.players.clone(),
            turn_order: self.turn_order.clone(),
            hp: self.hp.clone(),
            hands: self
                .hands
                .iter()
                .map(|hand| PublicPlayerHand {
                    player: hand.player.clone(),
                    cards: if viewer.can_see_player_hidden_cards(&hand.player) {
                        PublicCardRefs::Known(hand.cards.clone())
                    } else {
                        PublicCardRefs::Hidden {
                            count: hand.cards.len(),
                        }
                    },
                })
                .collect(),
            discard: self.discard.clone(),
            covered_passives: self
                .covered_passives
                .iter()
                .map(|passive| PublicCoveredPassive {
                    owner: passive.owner.clone(),
                    formation_id: passive.formation_id.clone(),
                    cards: if viewer.can_see_player_hidden_cards(&passive.owner) {
                        PublicCardRefs::Known(passive.cards.clone())
                    } else {
                        PublicCardRefs::Hidden {
                            count: passive.cards.len(),
                        }
                    },
                })
                .collect(),
            pending_choice: self
                .pending_choice
                .as_ref()
                .map(|choice| PublicPendingChoice {
                    player: choice.player.clone(),
                    kind: if viewer.can_see_player_hidden_cards(&choice.player) {
                        PublicPendingChoiceKind::Known(choice.kind.clone())
                    } else {
                        PublicPendingChoiceKind::Hidden
                    },
                }),
            shields: self.shields.clone(),
            statuses: self.statuses.clone(),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Viewer {
    Player(PlayerId),
    Observer,
}

impl Viewer {
    fn can_see_player_hidden_cards(&self, player: &PlayerId) -> bool {
        matches!(self, Viewer::Player(viewer) if viewer == player)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicGameState {
    pub status: GameStatus,
    pub turn_number: u64,
    pub phase: Phase,
    pub current_player: Option<PlayerId>,
    pub players: Vec<Player>,
    pub turn_order: Vec<PlayerId>,
    pub hp: Vec<TeamHp>,
    pub hands: Vec<PublicPlayerHand>,
    pub discard: Vec<CardInstanceId>,
    pub covered_passives: Vec<PublicCoveredPassive>,
    pub pending_choice: Option<PublicPendingChoice>,
    pub shields: Vec<PlayerShield>,
    pub statuses: Vec<StatusEffect>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPlayerHand {
    pub player: PlayerId,
    pub cards: PublicCardRefs,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicCoveredPassive {
    pub owner: PlayerId,
    pub formation_id: String,
    pub cards: PublicCardRefs,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicCardRefs {
    Known(Vec<CardInstanceId>),
    Hidden { count: usize },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PublicPendingChoice {
    pub player: PlayerId,
    pub kind: PublicPendingChoiceKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicPendingChoiceKind {
    Known(PendingChoiceKind),
    Hidden,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PendingChoice {
    pub player: PlayerId,
    pub kind: PendingChoiceKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PendingChoiceKind {
    TurnDrawDiscard {
        drawn_cards: Vec<CardInstanceId>,
        allowed_discards: Vec<CardInstanceId>,
    },
    EffectGenerated {
        effect_id: String,
        continuation_id: String,
        allowed_cards: Vec<CardInstanceId>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    DeckPrepared {
        deck_order: Vec<CardInstanceId>,
    },
    CardsDealt {
        player: PlayerId,
        cards: Vec<CardInstanceId>,
    },
    TurnStarted {
        player: PlayerId,
        turn_number: u64,
    },
    ActionPassed {
        player: PlayerId,
        reason: PassActionReason,
    },
    CardsDrawnForTurnDiscardChoice {
        player: PlayerId,
        drawn_cards: Vec<CardInstanceId>,
        allowed_discards: Vec<CardInstanceId>,
    },
    TurnDiscardChosen {
        player: PlayerId,
        discard: CardInstanceId,
    },
    TurnDrawSkipped {
        player: PlayerId,
        reason: TurnDrawSkipReason,
    },
    FormationPerformed {
        player: PlayerId,
        formation_id: String,
        used_cards: Vec<CardInstanceId>,
        declared_targets: Vec<TargetDecl>,
    },
    AttackResolved {
        attacker: PlayerId,
        target: PlayerId,
        formation_id: String,
        used_cards: Vec<CardInstanceId>,
        point_breakdown: AttackPointBreakdown,
        hp_change: HpChangeDelta,
        shield_change: Option<ShieldChangeDelta>,
        card_moves: Vec<CardMoveDelta>,
        elemental_context_update: Option<LastElementalAttackUpdate>,
    },
    TurnDrawBonusChanged {
        player: PlayerId,
        old_value: usize,
        delta: i32,
        new_value: usize,
    },
    ShieldChanged {
        player: PlayerId,
        old_value: i32,
        delta: i32,
        new_value: i32,
    },
    HpChanged {
        change: HpChangeDelta,
    },
    CardsMoved {
        card_moves: Vec<CardMoveDelta>,
    },
    StatusAdded {
        status: StatusEffect,
    },
    StatusExpired {
        status_id: String,
        owner: StatusOwner,
        expired_at: StatusExpiryTiming,
    },
    StatusRemoved {
        status_id: String,
        owner: StatusOwner,
    },
    EffectChoiceRequested {
        player: PlayerId,
        kind: PendingChoiceKind,
    },
    EffectChoiceAnswered {
        player: PlayerId,
        effect_id: String,
        continuation_id: String,
        selected_cards: Vec<CardInstanceId>,
    },
    PassiveCovered {
        player: PlayerId,
        formation_id: String,
        cards: Vec<CardInstanceId>,
        sealed: bool,
    },
    PassiveFlipped {
        owner: PlayerId,
        incoming_player: PlayerId,
        passive_id: String,
        cards: Vec<CardInstanceId>,
        outcome: PassiveFlipOutcome,
    },
    DiscardRecycledIntoDeck {
        shuffled_order: Vec<CardInstanceId>,
        placement: DeckPlacement,
    },
    TurnEnded {
        player: PlayerId,
    },
}

impl GameEvent {
    pub fn view_for(&self, viewer: Viewer) -> PublicGameEvent {
        match self {
            GameEvent::DeckPrepared { deck_order } => PublicGameEvent::DeckPrepared {
                deck: PublicCardRefs::Hidden {
                    count: deck_order.len(),
                },
            },
            GameEvent::CardsDealt { player, cards } => PublicGameEvent::CardsDealt {
                player: player.clone(),
                cards: if viewer.can_see_player_hidden_cards(player) {
                    PublicCardRefs::Known(cards.clone())
                } else {
                    PublicCardRefs::Hidden { count: cards.len() }
                },
            },
            GameEvent::PassiveCovered {
                player,
                formation_id,
                cards,
                sealed: _,
            } => PublicGameEvent::PassiveCovered {
                player: player.clone(),
                formation_id: formation_id.clone(),
                cards: if viewer.can_see_player_hidden_cards(player) {
                    PublicCardRefs::Known(cards.clone())
                } else {
                    PublicCardRefs::Hidden { count: cards.len() }
                },
            },
            GameEvent::CardsDrawnForTurnDiscardChoice {
                player,
                drawn_cards,
                allowed_discards,
            } => PublicGameEvent::CardsDrawnForTurnDiscardChoice {
                player: player.clone(),
                drawn_cards: if viewer.can_see_player_hidden_cards(player) {
                    PublicCardRefs::Known(drawn_cards.clone())
                } else {
                    PublicCardRefs::Hidden {
                        count: drawn_cards.len(),
                    }
                },
                allowed_discards: if viewer.can_see_player_hidden_cards(player) {
                    PublicCardRefs::Known(allowed_discards.clone())
                } else {
                    PublicCardRefs::Hidden {
                        count: allowed_discards.len(),
                    }
                },
            },
            GameEvent::EffectChoiceRequested { player, kind } => {
                PublicGameEvent::EffectChoiceRequested {
                    player: player.clone(),
                    kind: if viewer.can_see_player_hidden_cards(player) {
                        PublicPendingChoiceKind::Known(kind.clone())
                    } else {
                        PublicPendingChoiceKind::Hidden
                    },
                }
            }
            event => PublicGameEvent::Public(event.clone()),
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PublicGameEvent {
    Public(GameEvent),
    DeckPrepared {
        deck: PublicCardRefs,
    },
    CardsDealt {
        player: PlayerId,
        cards: PublicCardRefs,
    },
    PassiveCovered {
        player: PlayerId,
        formation_id: String,
        cards: PublicCardRefs,
    },
    CardsDrawnForTurnDiscardChoice {
        player: PlayerId,
        drawn_cards: PublicCardRefs,
        allowed_discards: PublicCardRefs,
    },
    EffectChoiceRequested {
        player: PlayerId,
        kind: PublicPendingChoiceKind,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RecordedEvent {
    pub metadata: EventMetadata,
    pub event: GameEvent,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EventMetadata {
    pub sequence: u64,
    pub source: EventSource,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum EventSource {
    Setup,
    Automatic {
        reason: AutomaticReason,
    },
    Command {
        command_id: CommandId,
        context: CommandContext,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum AutomaticReason {
    TurnStart,
    TurnDraw,
    TurnDrawSkipped,
    DiscardRecycle,
    StatusExpired,
    TurnEnd,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CommandContext {
    pub player: PlayerId,
    pub kind: CommandKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CommandKind {
    PassAction,
    PerformFormation { formation_id: String },
    ChooseTurnDiscard,
    AnswerEffectChoice,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnDrawSkipReason {
    HandLimitReached,
    CannotDrawByStatus,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckPlacement {
    Bottom,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum Command {
    PassAction {
        player: PlayerId,
        reason: PassActionReason,
    },
    PerformFormation {
        player: PlayerId,
        formation_id: String,
        cards: Vec<CardInstanceId>,
        declared_targets: Vec<TargetDecl>,
    },
    ChooseTurnDiscard {
        player: PlayerId,
        discard: CardInstanceId,
    },
    AnswerEffectChoice {
        player: PlayerId,
        selected_cards: Vec<CardInstanceId>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum TargetDecl {
    Player(PlayerId),
    Team(TeamId),
    Card(CardInstanceId),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum PassiveFlipOutcome {
    Applied {
        effect_id: String,
        modifications: Vec<ActionModification>,
    },
    NoEffect {
        reason: PassiveNoEffectReason,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ActionModification {
    PreventDamage,
    SplitAttackDamage,
    CancelSpell,
    SealCoveredPassive,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveNoEffectReason {
    NotAnAttack,
    NotASpell,
    Sealed,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AttackPointBreakdown {
    pub base_points: i32,
    pub interaction: ElementInteraction,
    pub damage_transform: DamageTransform,
    pub final_amount: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementInteraction {
    Generating,
    Overcoming,
    Same,
    None,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageTransform {
    NormalDamage,
    HealTarget,
    DoubleDamage,
    HalfDamageRoundUp,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LastElementalAttack {
    pub element: Element,
    pub resolved_turn: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LastElementalAttackUpdate {
    pub player: PlayerId,
    pub attack: LastElementalAttack,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct HpChangeDelta {
    pub team: TeamId,
    pub old_hp: i32,
    pub delta: i32,
    pub new_hp: i32,
    pub effective_delta: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ShieldChangeDelta {
    pub player: PlayerId,
    pub old_value: i32,
    pub delta: i32,
    pub new_value: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CardMoveDelta {
    pub card: CardInstanceId,
    pub from: CardZone,
    pub to: CardZone,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CardZone {
    Hand(PlayerId),
    DeckTop,
    Discard,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassActionReason {
    NoCardsInHand,
    CannotActByStatus,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameError {
    Validation(ValidationError),
    RuleImplementation(RuleImplementationError),
    EngineInvariant(EngineInvariantError),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum ValidationError {
    GameFinished,
    EmptyTurnOrder,
    DuplicatePlayer(PlayerId),
    DuplicateTurnOrderPlayer(PlayerId),
    MissingTurnOrderPlayer(PlayerId),
    TeamModeRequiresAtLeastFourPlayers {
        player_count: usize,
    },
    TeamModeRequiresExactlyTwoTeams {
        team_count: usize,
    },
    TeamModeRequiresEqualTeamSizes {
        first_team: TeamId,
        first_count: usize,
        second_team: TeamId,
        second_count: usize,
    },
    DuplicateTeamHp(TeamId),
    MissingTeamHp(TeamId),
    UnknownPlayer(PlayerId),
    WrongPlayer {
        expected: PlayerId,
        actual: PlayerId,
    },
    WrongPhase {
        expected: Phase,
        actual: Phase,
    },
    TeamSeatingNotAlternating {
        previous_player: PlayerId,
        player: PlayerId,
        team: TeamId,
    },
    MissingPendingChoice,
    PendingChoiceInProgress {
        player: PlayerId,
    },
    IllegalDiscard(CardInstanceId),
    IllegalChoiceCard(CardInstanceId),
    DuplicateChoiceCard(CardInstanceId),
    UnknownFormation(String),
    UnexpectedDeclaredTargets {
        formation_id: String,
    },
    DuplicateSubmittedCard(CardInstanceId),
    CardNotInHand(CardInstanceId),
    FormationPatternMismatch {
        formation_id: String,
    },
    PendingPassiveAlreadyCovered {
        player: PlayerId,
    },
    DuplicateCard(CardInstanceId),
    MissingCardInstanceDefinition(CardInstanceId),
    MissingCardDefinition(CardDefId),
    RulesetMismatch {
        setup: RulesetId,
        metadata: RulesetId,
    },
    CannotPassAction {
        reason: PassActionReason,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum EngineInvariantError {
    NotEnoughCards { needed: usize, available: usize },
    DuplicatePendingChoice { player: PlayerId },
    DuplicateCoveredPassive { player: PlayerId },
    ZoneOwnershipInconsistency { card: CardInstanceId },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RuleImplementationError {
    EffectNotImplemented(String),
}

pub type GameResult<T> = Result<T, GameError>;

pub fn validate_setup(setup: &GameSetup) -> GameResult<()> {
    if setup.turn_order.is_empty() {
        return Err(GameError::Validation(ValidationError::EmptyTurnOrder));
    }

    let mut players = HashSet::new();
    for player in &setup.players {
        if !players.insert(player.id.clone()) {
            return Err(GameError::Validation(ValidationError::DuplicatePlayer(
                player.id.clone(),
            )));
        }
    }

    for player in &setup.turn_order {
        if setup
            .turn_order
            .iter()
            .filter(|candidate| *candidate == player)
            .count()
            > 1
        {
            return Err(GameError::Validation(
                ValidationError::DuplicateTurnOrderPlayer(player.clone()),
            ));
        }

        if !players.contains(player) {
            return Err(GameError::Validation(ValidationError::UnknownPlayer(
                player.clone(),
            )));
        }
    }

    for player in &setup.players {
        if !setup.turn_order.contains(&player.id) {
            return Err(GameError::Validation(
                ValidationError::MissingTurnOrderPlayer(player.id.clone()),
            ));
        }
    }

    let mut hp_teams = HashSet::new();
    for team_hp in &setup.hp {
        if !hp_teams.insert(team_hp.team.clone()) {
            return Err(GameError::Validation(ValidationError::DuplicateTeamHp(
                team_hp.team.clone(),
            )));
        }
    }

    for player in &setup.players {
        if !hp_teams.contains(&player.team) {
            return Err(GameError::Validation(ValidationError::MissingTeamHp(
                player.team.clone(),
            )));
        }
    }

    validate_team_seating(setup)?;
    validate_card_setup(setup)?;

    Ok(())
}

fn validate_card_setup(setup: &GameSetup) -> GameResult<()> {
    let card_defs = setup
        .card_defs
        .iter()
        .map(|card_def| card_def.id.clone())
        .collect::<HashSet<_>>();
    let mut card_instances = HashSet::new();

    for card_instance in &setup.card_instances {
        if !card_instances.insert(card_instance.instance) {
            return Err(GameError::Validation(ValidationError::DuplicateCard(
                card_instance.instance,
            )));
        }

        if !card_defs.contains(&card_instance.definition) {
            return Err(GameError::Validation(
                ValidationError::MissingCardDefinition(card_instance.definition.clone()),
            ));
        }
    }

    Ok(())
}

fn validate_team_seating(setup: &GameSetup) -> GameResult<()> {
    let mut players_by_team = HashMap::<TeamId, usize>::new();
    let team_by_player = setup
        .players
        .iter()
        .map(|player| {
            *players_by_team.entry(player.team.clone()).or_default() += 1;
            (player.id.clone(), player.team.clone())
        })
        .collect::<HashMap<_, _>>();

    let team_mode_shape = setup.players.len() != 2
        || players_by_team
            .values()
            .any(|player_count| *player_count > 1);
    if !team_mode_shape {
        return Ok(());
    }

    if setup.players.len() < 4 {
        return Err(GameError::Validation(
            ValidationError::TeamModeRequiresAtLeastFourPlayers {
                player_count: setup.players.len(),
            },
        ));
    }

    if players_by_team.len() != 2 {
        return Err(GameError::Validation(
            ValidationError::TeamModeRequiresExactlyTwoTeams {
                team_count: players_by_team.len(),
            },
        ));
    }

    let mut team_sizes = players_by_team.iter().collect::<Vec<_>>();
    team_sizes.sort_by(|(left_team, _), (right_team, _)| left_team.cmp(right_team));
    let (first_team, first_count) = team_sizes[0];
    let (second_team, second_count) = team_sizes[1];
    if first_count != second_count {
        return Err(GameError::Validation(
            ValidationError::TeamModeRequiresEqualTeamSizes {
                first_team: first_team.clone(),
                first_count: *first_count,
                second_team: second_team.clone(),
                second_count: *second_count,
            },
        ));
    }

    for index in 0..setup.turn_order.len() {
        let previous_player = &setup.turn_order[index];
        let player = &setup.turn_order[(index + 1) % setup.turn_order.len()];
        let previous_team = team_by_player.get(previous_player).ok_or_else(|| {
            GameError::Validation(ValidationError::UnknownPlayer(previous_player.clone()))
        })?;
        let team = team_by_player
            .get(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;

        if previous_team == team {
            return Err(GameError::Validation(
                ValidationError::TeamSeatingNotAlternating {
                    previous_player: previous_player.clone(),
                    player: player.clone(),
                    team: team.clone(),
                },
            ));
        }
    }

    Ok(())
}
