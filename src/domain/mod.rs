//! Domain model: game state, ids, events, commands, and rule invariants.

use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlayerId(String);

impl PlayerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TeamId(String);

impl TeamId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardInstanceId(u64);

impl CardInstanceId {
    pub fn new(id: u64) -> Self {
        Self(id)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    Metal,
    Wood,
    Water,
    Fire,
    Earth,
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardDefId(String);

impl CardDefId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardDef {
    pub id: CardDefId,
    pub name: String,
    pub element: Element,
    pub level: u32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardInstanceDef {
    pub instance: CardInstanceId,
    pub definition: CardDefId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Player {
    pub id: PlayerId,
    pub team: TeamId,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerHand {
    pub player: PlayerId,
    pub cards: Vec<CardInstanceId>,
}

impl PlayerHand {
    pub fn new(player: PlayerId, cards: Vec<CardInstanceId>) -> Self {
        Self { player, cards }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TeamHp {
    pub team: TeamId,
    pub hp: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameStatus {
    InProgress,
    Finished { winning_team: TeamId },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlayerShield {
    pub player: PlayerId,
    pub value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CoveredPassive {
    pub owner: PlayerId,
    pub cards: Vec<CardInstanceId>,
    pub covered_on_turn: u64,
    pub reveal_timing: PassiveTriggerTiming,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveTriggerTiming {
    NextPlayerActionStart,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StatusEffect {
    pub id: String,
    pub owner: StatusOwner,
    pub kind: String,
    pub value: Option<i32>,
    pub duration: StatusDuration,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StatusOwner {
    Player(PlayerId),
    Team(TeamId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusDuration {
    Turns { remaining: u32 },
    Rounds { remaining: u32 },
    UntilNextAction,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GameSetup {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    TurnStart,
    Main,
    TurnDraw,
    TurnDrawDiscardChoice,
    TurnEnd,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingChoice {
    pub player: PlayerId,
    pub kind: PendingChoiceKind,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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

#[derive(Clone, Debug, PartialEq, Eq)]
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
    ShieldChanged {
        player: PlayerId,
        old_value: i32,
        delta: i32,
        new_value: i32,
    },
    DiscardRecycledIntoDeck {
        shuffled_order: Vec<CardInstanceId>,
        placement: DeckPlacement,
    },
    TurnEnded {
        player: PlayerId,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedEvent {
    pub metadata: EventMetadata,
    pub event: GameEvent,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct EventMetadata {
    pub sequence: u64,
    pub source: EventSource,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventSource {
    System,
    Command,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TurnDrawSkipReason {
    HandLimitReached,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DeckPlacement {
    Bottom,
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum TargetDecl {
    Player(PlayerId),
    Team(TeamId),
    Card(CardInstanceId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttackPointBreakdown {
    pub base_points: i32,
    pub interaction: ElementInteraction,
    pub damage_transform: DamageTransform,
    pub final_amount: i32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementInteraction {
    Generating,
    Overcoming,
    Same,
    None,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DamageTransform {
    NormalDamage,
    HealTarget,
    DoubleDamage,
    HalfDamageRoundUp,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastElementalAttack {
    pub element: Element,
    pub resolved_turn: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LastElementalAttackUpdate {
    pub player: PlayerId,
    pub attack: LastElementalAttack,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HpChangeDelta {
    pub team: TeamId,
    pub old_hp: i32,
    pub delta: i32,
    pub new_hp: i32,
    pub effective_delta: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShieldChangeDelta {
    pub player: PlayerId,
    pub old_value: i32,
    pub delta: i32,
    pub new_value: i32,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardMoveDelta {
    pub card: CardInstanceId,
    pub from: CardZone,
    pub to: CardZone,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CardZone {
    Hand(PlayerId),
    Discard,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassActionReason {
    NoCardsInHand,
    CannotActByStatus,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GameError {
    EmptyTurnOrder,
    DuplicatePlayer(PlayerId),
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
    IllegalDiscard(CardInstanceId),
    UnknownFormation(String),
    DuplicateSubmittedCard(CardInstanceId),
    CardNotInHand(CardInstanceId),
    FormationPatternMismatch {
        formation_id: String,
    },
    DuplicateCard(CardInstanceId),
    MissingCardInstanceDefinition(CardInstanceId),
    MissingCardDefinition(CardDefId),
    CannotPassAction {
        reason: PassActionReason,
    },
    NotEnoughCards {
        needed: usize,
        available: usize,
    },
    RuleImplementation(RuleImplementationError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RuleImplementationError {
    EffectNotImplemented(String),
}

pub type GameResult<T> = Result<T, GameError>;

pub fn validate_setup(setup: &GameSetup) -> GameResult<()> {
    if setup.turn_order.is_empty() {
        return Err(GameError::EmptyTurnOrder);
    }

    let mut players = HashSet::new();
    for player in &setup.players {
        if !players.insert(player.id.clone()) {
            return Err(GameError::DuplicatePlayer(player.id.clone()));
        }
    }

    for player in &setup.turn_order {
        if !players.contains(player) {
            return Err(GameError::UnknownPlayer(player.clone()));
        }
    }

    let mut hp_teams = HashSet::new();
    for team_hp in &setup.hp {
        if !hp_teams.insert(team_hp.team.clone()) {
            return Err(GameError::DuplicateTeamHp(team_hp.team.clone()));
        }
    }

    for player in &setup.players {
        if !hp_teams.contains(&player.team) {
            return Err(GameError::MissingTeamHp(player.team.clone()));
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
            return Err(GameError::DuplicateCard(card_instance.instance));
        }

        if !card_defs.contains(&card_instance.definition) {
            return Err(GameError::MissingCardDefinition(
                card_instance.definition.clone(),
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

    let is_team_mode = players_by_team
        .values()
        .any(|player_count| *player_count > 1);
    if !is_team_mode || setup.turn_order.len() < 2 {
        return Ok(());
    }

    for index in 0..setup.turn_order.len() {
        let previous_player = &setup.turn_order[index];
        let player = &setup.turn_order[(index + 1) % setup.turn_order.len()];
        let previous_team = team_by_player
            .get(previous_player)
            .ok_or_else(|| GameError::UnknownPlayer(previous_player.clone()))?;
        let team = team_by_player
            .get(player)
            .ok_or_else(|| GameError::UnknownPlayer(player.clone()))?;

        if previous_team == team {
            return Err(GameError::TeamSeatingNotAlternating {
                previous_player: previous_player.clone(),
                player: player.clone(),
                team: team.clone(),
            });
        }
    }

    Ok(())
}
