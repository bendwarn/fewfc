//! Domain model: game state, ids, events, commands, and rule invariants.

pub mod targeting;

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

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub const BASE_RULESET_ID: &str = "base";
pub const DISCARD_RETRIEVAL_MODULE_ID: &str = "discard-retrieval";
pub const FIVE_DIRECTIONS_LEGEND_MODULE_ID: &str = "five-directions-legend";
pub const PERSONAL_DECK_MODULE_ID: &str = "personal-deck";
pub const STAR_MODULE_ID: &str = "star";

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

impl Default for RulesetId {
    fn default() -> Self {
        Self::base()
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RuleModuleId(String);

impl RuleModuleId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum StarKind {
    Metal,
    Wood,
    Water,
    Fire,
    Earth,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TeamStar {
    pub team: TeamId,
    pub star: StarKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerStarHistory {
    pub player: PlayerId,
    pub stars: Vec<StarKind>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FiveStarAlignment {
    pub player: PlayerId,
    pub team: TeamId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum StarBreakReason {
    Replaced,
    OpposedBy(StarKind),
    StarFormationUsed { formation_id: String },
    VoidStarBreaking,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardDefId(String);

impl CardDefId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
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
    #[serde(default)]
    pub origin: CardOrigin,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub enum CardOrigin {
    #[default]
    Shared,
    Player(PlayerId),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerDeckList {
    pub player: PlayerId,
    pub name: String,
    pub cards: Vec<CardDefId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerCardPile {
    pub player: PlayerId,
    pub cards: Vec<CardInstanceId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LastTurnDiscard {
    pub card: CardInstanceId,
    pub turn_number: u64,
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
pub struct CounterEffect {
    pub owner: PlayerId,
    pub effect_id: String,
    pub established_on_turn: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LastFormationUse {
    pub formation_id: String,
    #[serde(default)]
    pub resolved_effect_id: String,
    pub used_cards: Vec<CardInstanceId>,
    pub resolved_turn: u64,
}

impl LastFormationUse {
    pub fn effective_effect_id(&self) -> &str {
        if self.resolved_effect_id.is_empty() {
            &self.formation_id
        } else {
            &self.resolved_effect_id
        }
    }
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
    #[serde(default)]
    pub enabled_rule_modules: Vec<RuleModuleId>,
    pub players: Vec<Player>,
    pub turn_order: Vec<PlayerId>,
    pub hp: Vec<TeamHp>,
    pub card_defs: Vec<CardDef>,
    pub card_instances: Vec<CardInstanceDef>,
    #[serde(default)]
    pub deck_lists: Vec<PlayerDeckList>,
    pub hand_limit: usize,
    pub base_draw: usize,
}

impl GameSetup {
    pub fn two_player(first_player: PlayerId, second_player: PlayerId, starting_hp: i32) -> Self {
        let first_team = TeamId::new(format!("team:{}", first_player.as_str()));
        let second_team = TeamId::new(format!("team:{}", second_player.as_str()));

        Self {
            ruleset: RulesetId::base(),
            enabled_rule_modules: Vec::new(),
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
            deck_lists: Vec::new(),
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
            enabled_rule_modules: Vec::new(),
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
            deck_lists: Vec::new(),
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

    pub fn with_rule_modules(mut self, modules: Vec<RuleModuleId>) -> Self {
        self.enabled_rule_modules = modules;
        self
    }

    pub fn with_deck_lists(mut self, deck_lists: Vec<PlayerDeckList>) -> Self {
        self.deck_lists = deck_lists;
        self
    }

    pub fn has_rule_module(&self, module_id: &str) -> bool {
        self.enabled_rule_modules
            .iter()
            .any(|module| module.as_str() == module_id)
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
    #[serde(default)]
    pub ruleset: RulesetId,
    #[serde(default)]
    pub enabled_rule_modules: Vec<RuleModuleId>,
    pub status: GameStatus,
    pub turn_number: u64,
    pub phase: Phase,
    pub current_turn_index: usize,
    pub players: Vec<Player>,
    pub turn_order: Vec<PlayerId>,
    pub hp: Vec<TeamHp>,
    #[serde(default)]
    pub initial_hp: Vec<TeamHp>,
    pub card_defs: Vec<CardDef>,
    pub card_instances: Vec<CardInstanceDef>,
    pub deck: Vec<CardInstanceId>,
    #[serde(default)]
    pub player_decks: Vec<PlayerCardPile>,
    pub hands: Vec<PlayerHand>,
    pub discard: Vec<CardInstanceId>,
    #[serde(default)]
    pub player_discards: Vec<PlayerCardPile>,
    #[serde(default)]
    pub exposed_foreign_cards: Vec<CardInstanceId>,
    #[serde(default)]
    pub last_turn_discard_by_player: HashMap<PlayerId, LastTurnDiscard>,
    pub pending_choice: Option<PendingChoice>,
    pub shields: Vec<PlayerShield>,
    pub covered_passives: Vec<CoveredPassive>,
    #[serde(default)]
    pub counter_effects: Vec<CounterEffect>,
    pub statuses: Vec<StatusEffect>,
    #[serde(default)]
    pub environment: Option<Element>,
    #[serde(default)]
    pub team_stars: Vec<TeamStar>,
    #[serde(default)]
    pub star_histories: Vec<PlayerStarHistory>,
    #[serde(default)]
    pub five_star_alignment: Option<FiveStarAlignment>,
    pub last_elemental_attack_by_player: HashMap<PlayerId, LastElementalAttack>,
    pub last_formation_by_player: HashMap<PlayerId, LastFormationUse>,
    pub turn_draw_bonus_by_player: HashMap<PlayerId, usize>,
    pub hand_limit: usize,
    pub base_draw: usize,
}

impl GameState {
    pub fn from_setup(setup: &GameSetup) -> Self {
        Self {
            ruleset: setup.ruleset.clone(),
            enabled_rule_modules: setup.enabled_rule_modules.clone(),
            status: GameStatus::InProgress,
            turn_number: 1,
            phase: Phase::TurnStart,
            current_turn_index: 0,
            players: setup.players.clone(),
            turn_order: setup.turn_order.clone(),
            hp: setup.hp.clone(),
            initial_hp: setup.hp.clone(),
            card_defs: setup.card_defs.clone(),
            card_instances: setup.card_instances.clone(),
            deck: Vec::new(),
            player_decks: setup
                .players
                .iter()
                .map(|player| PlayerCardPile {
                    player: player.id.clone(),
                    cards: Vec::new(),
                })
                .collect(),
            hands: setup
                .players
                .iter()
                .map(|player| PlayerHand::new(player.id.clone(), Vec::new()))
                .collect(),
            discard: Vec::new(),
            player_discards: setup
                .players
                .iter()
                .map(|player| PlayerCardPile {
                    player: player.id.clone(),
                    cards: Vec::new(),
                })
                .collect(),
            exposed_foreign_cards: Vec::new(),
            last_turn_discard_by_player: HashMap::new(),
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
            counter_effects: Vec::new(),
            statuses: Vec::new(),
            environment: None,
            team_stars: Vec::new(),
            star_histories: setup
                .players
                .iter()
                .map(|player| PlayerStarHistory {
                    player: player.id.clone(),
                    stars: Vec::new(),
                })
                .collect(),
            five_star_alignment: None,
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

    pub fn card_origin(&self, instance: CardInstanceId) -> Option<&CardOrigin> {
        self.card_instances
            .iter()
            .find(|card| card.instance == instance)
            .map(|card| &card.origin)
    }

    pub fn has_rule_module(&self, module_id: &str) -> bool {
        self.enabled_rule_modules
            .iter()
            .any(|module| module.as_str() == module_id)
    }

    pub fn uses_personal_decks(&self) -> bool {
        self.has_rule_module(PERSONAL_DECK_MODULE_ID)
    }

    pub fn deck_for(&self, player: &PlayerId) -> Option<&[CardInstanceId]> {
        if self.uses_personal_decks() {
            self.player_decks
                .iter()
                .find(|pile| &pile.player == player)
                .map(|pile| pile.cards.as_slice())
        } else {
            Some(self.deck.as_slice())
        }
    }

    pub fn deck_for_mut(&mut self, player: &PlayerId) -> Option<&mut Vec<CardInstanceId>> {
        if self.uses_personal_decks() {
            self.player_decks
                .iter_mut()
                .find(|pile| &pile.player == player)
                .map(|pile| &mut pile.cards)
        } else {
            Some(&mut self.deck)
        }
    }

    pub fn discard_for(&self, player: &PlayerId) -> Option<&[CardInstanceId]> {
        if self.uses_personal_decks() {
            self.player_discards
                .iter()
                .find(|pile| &pile.player == player)
                .map(|pile| pile.cards.as_slice())
        } else {
            Some(self.discard.as_slice())
        }
    }

    pub fn discard_for_mut(&mut self, player: &PlayerId) -> Option<&mut Vec<CardInstanceId>> {
        if self.uses_personal_decks() {
            self.player_discards
                .iter_mut()
                .find(|pile| &pile.player == player)
                .map(|pile| &mut pile.cards)
        } else {
            Some(&mut self.discard)
        }
    }

    pub fn discard_owner(&self, card: CardInstanceId) -> Option<PlayerId> {
        match self.card_origin(card)? {
            CardOrigin::Shared => None,
            CardOrigin::Player(player) => Some(player.clone()),
        }
    }

    pub fn initial_hp(&self, team: &TeamId) -> Option<i32> {
        self.initial_hp
            .iter()
            .find(|team_hp| &team_hp.team == team)
            .map(|team_hp| team_hp.hp)
    }

    pub fn star_for_team(&self, team: &TeamId) -> Option<StarKind> {
        self.team_stars
            .iter()
            .find(|owned| &owned.team == team)
            .map(|owned| owned.star)
    }

    pub fn summoned_stars_for(&self, player: &PlayerId) -> Option<&[StarKind]> {
        self.star_histories
            .iter()
            .find(|history| &history.player == player)
            .map(|history| history.stars.as_slice())
    }
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

impl PendingChoiceKind {
    pub fn required_count(&self) -> usize {
        match self {
            Self::TurnDrawDiscard { .. } => 1,
            Self::EffectGenerated {
                continuation_id,
                allowed_cards,
                ..
            } => {
                if continuation_id == "chaos:return-two" {
                    allowed_cards.len().min(2)
                } else {
                    allowed_cards.len().min(1)
                }
            }
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    DeckPrepared {
        deck_order: Vec<CardInstanceId>,
    },
    PlayerDeckPrepared {
        player: PlayerId,
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
    FormationEffectCopied {
        player: PlayerId,
        effect_id: String,
    },
    FormationEffectIgnored {
        player: PlayerId,
        formation_id: String,
        reason: FormationNoEffectReason,
    },
    CounterEffectEstablished {
        owner: PlayerId,
        effect_id: String,
    },
    CounterEffectResolved {
        owner: PlayerId,
        incoming_player: PlayerId,
        effect_id: String,
        outcome: PassiveFlipOutcome,
    },
    HandInspected {
        viewer: PlayerId,
        target: PlayerId,
        cards: Vec<CardInstanceId>,
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
    EnvironmentTransferred {
        player: PlayerId,
        formation_id: String,
        from: Option<Element>,
        to: Element,
    },
    EnvironmentCleared {
        player: PlayerId,
        formation_id: String,
        environment: Element,
        hp_changes: Vec<HpChangeDelta>,
    },
    StarBroken {
        team: TeamId,
        star: StarKind,
        reason: StarBreakReason,
        hp_change: Option<HpChangeDelta>,
    },
    StarSummoned {
        player: PlayerId,
        team: TeamId,
        star: StarKind,
    },
    VoidStarBreakingCompleted {
        player: PlayerId,
    },
    FiveStarAlignmentAchieved {
        player: PlayerId,
        team: TeamId,
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
    PlayerDiscardRecycledIntoDeck {
        player: PlayerId,
        shuffled_order: Vec<CardInstanceId>,
        placement: DeckPlacement,
    },
    DiscardRetrieved {
        player: PlayerId,
        previous_player: PlayerId,
        card: CardInstanceId,
        hp_change: HpChangeDelta,
        card_move: CardMoveDelta,
    },
    TurnEnded {
        player: PlayerId,
    },
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
    RetrievePreviousTurnDiscard {
        player: PlayerId,
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
    EmptyCity,
    IgnoredBySacredBeast,
    IneffectiveInEnvironment { environment: Element },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum FormationNoEffectReason {
    IneffectiveInEnvironment { environment: Element },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AttackPointBreakdown {
    pub base_points: i32,
    #[serde(default)]
    pub environment_effect: EnvironmentAttackEffect,
    pub interaction: ElementInteraction,
    pub damage_transform: DamageTransform,
    pub final_amount: i32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum EnvironmentAttackEffect {
    #[default]
    None,
    MatchingElementDamageDoubled {
        environment: Element,
    },
    GeneratingElementDamageConvertedToHealing {
        environment: Element,
    },
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
    PlayerDeckTop(PlayerId),
    PlayerDiscard(PlayerId),
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
    CannotPerformFormation {
        reason: CannotPerformFormationReason,
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
    UnsupportedRuleset(RulesetId),
    DuplicateRuleModule(RuleModuleId),
    UnknownRuleModule(RuleModuleId),
    PersonalDeckListsRequired,
    DuplicatePlayerDeckList(PlayerId),
    MissingPlayerDeckList(PlayerId),
    UnknownDeckListPlayer(PlayerId),
    InvalidDeckCardCount {
        player: PlayerId,
        expected: usize,
        actual: usize,
    },
    DeckLevelLimitExceeded {
        player: PlayerId,
        maximum: u32,
        actual: u32,
    },
    DeckCopyLimitExceeded {
        player: PlayerId,
        card: CardDefId,
        maximum: usize,
        actual: usize,
    },
    DeckInstancesMismatch {
        player: PlayerId,
    },
    DiscardRetrievalDisabled,
    NoRetrievableDiscard {
        previous_player: PlayerId,
    },
    CannotPassAction {
        reason: PassActionReason,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum CannotPerformFormationReason {
    WrongPhase {
        expected: Phase,
        actual: Phase,
    },
    WrongPlayer {
        expected: PlayerId,
        actual: PlayerId,
    },
    PendingChoiceInProgress {
        player: PlayerId,
    },
    CannotActByStatus {
        player: PlayerId,
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

    let mut rule_modules = HashSet::new();
    for module in &setup.enabled_rule_modules {
        if !rule_modules.insert(module.clone()) {
            return Err(GameError::Validation(ValidationError::DuplicateRuleModule(
                module.clone(),
            )));
        }
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
    validate_deck_lists(setup)?;

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

        if let CardOrigin::Player(player) = &card_instance.origin
            && !setup
                .players
                .iter()
                .any(|candidate| &candidate.id == player)
        {
            return Err(GameError::Validation(ValidationError::UnknownPlayer(
                player.clone(),
            )));
        }
    }

    Ok(())
}

fn validate_deck_lists(setup: &GameSetup) -> GameResult<()> {
    if !setup.has_rule_module(PERSONAL_DECK_MODULE_ID) {
        return Ok(());
    }

    if setup.deck_lists.is_empty() {
        return Err(GameError::Validation(
            ValidationError::PersonalDeckListsRequired,
        ));
    }

    let definitions = setup
        .card_defs
        .iter()
        .map(|card| (card.id.clone(), card))
        .collect::<HashMap<_, _>>();
    let mut players = HashSet::new();

    for deck in &setup.deck_lists {
        if !setup.players.iter().any(|player| player.id == deck.player) {
            return Err(GameError::Validation(
                ValidationError::UnknownDeckListPlayer(deck.player.clone()),
            ));
        }
        if !players.insert(deck.player.clone()) {
            return Err(GameError::Validation(
                ValidationError::DuplicatePlayerDeckList(deck.player.clone()),
            ));
        }
        if deck.cards.len() != 60 {
            return Err(GameError::Validation(
                ValidationError::InvalidDeckCardCount {
                    player: deck.player.clone(),
                    expected: 60,
                    actual: deck.cards.len(),
                },
            ));
        }

        let mut level_total = 0;
        let mut copies = HashMap::<CardDefId, usize>::new();
        for card_id in &deck.cards {
            let definition = definitions.get(card_id).ok_or_else(|| {
                GameError::Validation(ValidationError::MissingCardDefinition(card_id.clone()))
            })?;
            level_total += definition.level;
            *copies.entry(card_id.clone()).or_default() += 1;
        }
        if level_total > 170 {
            return Err(GameError::Validation(
                ValidationError::DeckLevelLimitExceeded {
                    player: deck.player.clone(),
                    maximum: 170,
                    actual: level_total,
                },
            ));
        }
        for (card, actual) in copies {
            let definition = definitions
                .get(&card)
                .expect("deck card definitions were validated");
            let maximum = match definition.level {
                1..=3 => 4,
                4..=5 => 3,
                _ => 0,
            };
            if actual > maximum {
                return Err(GameError::Validation(
                    ValidationError::DeckCopyLimitExceeded {
                        player: deck.player.clone(),
                        card,
                        maximum,
                        actual,
                    },
                ));
            }
        }
    }

    for player in &setup.players {
        if !players.contains(&player.id) {
            return Err(GameError::Validation(
                ValidationError::MissingPlayerDeckList(player.id.clone()),
            ));
        }
    }

    for deck in &setup.deck_lists {
        let expected =
            deck.cards
                .iter()
                .fold(HashMap::<CardDefId, usize>::new(), |mut counts, card| {
                    *counts.entry(card.clone()).or_default() += 1;
                    counts
                });
        let actual = setup
            .card_instances
            .iter()
            .filter(
                |instance| matches!(&instance.origin, CardOrigin::Player(player) if player == &deck.player),
            )
            .fold(HashMap::<CardDefId, usize>::new(), |mut counts, instance| {
                *counts.entry(instance.definition.clone()).or_default() += 1;
                counts
            });
        if expected != actual {
            return Err(GameError::Validation(
                ValidationError::DeckInstancesMismatch {
                    player: deck.player.clone(),
                },
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
    team_sizes.sort_by_key(|(team, _)| (*team).clone());
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
