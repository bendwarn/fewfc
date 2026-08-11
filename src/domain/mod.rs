//! Domain model: game state, ids, events, commands, and rule invariants.

pub mod targeting;

use serde::{Deserialize, Deserializer, Serialize, Serializer, de};
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
pub const HERO_SCHOOLS_MODULE_ID: &str = "hero-schools";
pub const SPIRIT_MODULE_ID: &str = "spirit";
pub const JIANGHU_MODULE_ID: &str = "jianghu";
pub const CONFLUENCE_GENERATION_MODULE_ID: &str = "confluence-generation";
pub const DARK_GLIMMER_MODULE_ID: &str = "dark-glimmer";
pub const ECHO_MODULE_ID: &str = "echo";
pub const TRIBULATION_MODULE_ID: &str = "tribulation";
pub const POUCH_MODULE_ID: &str = "pouch";

/// The Base Ruleset's immutable bounds for every consumable Card Level.
pub const MIN_CARD_LEVEL: u32 = 1;
pub const MAX_CARD_LEVEL: u32 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct CardLevelOutOfBounds {
    pub value: u32,
}

impl std::fmt::Display for CardLevelOutOfBounds {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "Card Level {} is outside the Base Ruleset range {}..={}",
            self.value, MIN_CARD_LEVEL, MAX_CARD_LEVEL
        )
    }
}

/// The immutable level printed on a Card Definition.
///
/// This intentionally is not interchangeable with [`EffectiveCardLevel`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct PrintedCardLevel(u32);

impl PrintedCardLevel {
    pub const fn new(value: u32) -> Self {
        assert!(value >= MIN_CARD_LEVEL && value <= MAX_CARD_LEVEL);
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }
}

impl std::fmt::Display for PrintedCardLevel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl TryFrom<u32> for PrintedCardLevel {
    type Error = CardLevelOutOfBounds;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if (MIN_CARD_LEVEL..=MAX_CARD_LEVEL).contains(&value) {
            Ok(Self(value))
        } else {
            Err(CardLevelOutOfBounds { value })
        }
    }
}

impl Serialize for PrintedCardLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for PrintedCardLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(u32::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

/// The bounded level exposed to ordinary rules after Card Interpretation Layers
/// have fully composed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[repr(transparent)]
pub struct EffectiveCardLevel(u32);

impl EffectiveCardLevel {
    pub const fn new(value: u32) -> Self {
        assert!(value >= MIN_CARD_LEVEL && value <= MAX_CARD_LEVEL);
        Self(value)
    }

    pub const fn value(self) -> u32 {
        self.0
    }

    pub(crate) fn clamp_composed(value: i32) -> Self {
        Self(value.clamp(MIN_CARD_LEVEL as i32, MAX_CARD_LEVEL as i32) as u32)
    }
}

impl PartialEq<u32> for EffectiveCardLevel {
    fn eq(&self, other: &u32) -> bool {
        self.0 == *other
    }
}

impl PartialOrd<u32> for EffectiveCardLevel {
    fn partial_cmp(&self, other: &u32) -> Option<std::cmp::Ordering> {
        self.0.partial_cmp(other)
    }
}

impl std::ops::Rem<u32> for EffectiveCardLevel {
    type Output = u32;

    fn rem(self, divisor: u32) -> Self::Output {
        self.0 % divisor
    }
}

impl std::iter::Sum<EffectiveCardLevel> for u32 {
    fn sum<I: Iterator<Item = EffectiveCardLevel>>(iter: I) -> Self {
        iter.map(EffectiveCardLevel::value).sum()
    }
}

impl From<PrintedCardLevel> for EffectiveCardLevel {
    fn from(level: PrintedCardLevel) -> Self {
        Self(level.0)
    }
}

impl TryFrom<u32> for EffectiveCardLevel {
    type Error = CardLevelOutOfBounds;

    fn try_from(value: u32) -> Result<Self, Self::Error> {
        if (MIN_CARD_LEVEL..=MAX_CARD_LEVEL).contains(&value) {
            Ok(Self(value))
        } else {
            Err(CardLevelOutOfBounds { value })
        }
    }
}

impl From<EffectiveCardLevel> for u32 {
    fn from(level: EffectiveCardLevel) -> Self {
        level.0
    }
}

impl std::fmt::Display for EffectiveCardLevel {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(formatter)
    }
}

impl Serialize for EffectiveCardLevel {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u32(self.0)
    }
}

impl<'de> Deserialize<'de> for EffectiveCardLevel {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Self::try_from(u32::deserialize(deserializer)?).map_err(de::Error::custom)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardLevelInterpretation {
    Set(EffectiveCardLevel),
    Adjust(i32),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardInterpretationSource {
    PouchLevelBonusGranted,
    SpiritLevelInterpreted,
    ProfessionAbilityActivated,
}

/// One ordered projection of a semantic canonical event onto a physical Card.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct CardInterpretationLayer {
    pub source: CardInterpretationSource,
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub applied_on_turn: u64,
    pub element: Option<Element>,
    pub level: Option<CardLevelInterpretation>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EffectiveCardFacts {
    pub element: Element,
    pub level: EffectiveCardLevel,
}

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
pub struct ProfessionId(String);

impl ProfessionId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerProfession {
    pub player: PlayerId,
    pub profession: ProfessionId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PreparedProfessionAbility {
    pub player: PlayerId,
    pub ability_id: String,
    pub card: CardInstanceId,
    pub element: Element,
    pub level: EffectiveCardLevel,
    pub allowed_formation_scope: Vec<String>,
    pub prepared_on_turn: u64,
    #[serde(default)]
    pub interpretation_revision: u64,
}

/// A non-physical component supplied by an activated profession ability.
/// It deliberately has neither an instance id nor an origin: only formation
/// resolution may consume it.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct VirtualFormationCard {
    pub source_ability_id: String,
    pub element: Element,
    pub level: EffectiveCardLevel,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FormationComposition {
    pub physical_cards: Vec<CardInstanceId>,
    #[serde(default)]
    pub virtual_card: Option<VirtualFormationCard>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FormationRequirement {
    pub player: PlayerId,
    pub physical_card: Option<CardInstanceId>,
    pub virtual_card: Option<VirtualFormationCard>,
    pub allowed_formation_scope: Vec<String>,
    pub applied_on_turn: u64,
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

    pub fn as_u64(self) -> u64 {
        self.0
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

impl From<Element> for StarKind {
    fn from(element: Element) -> Self {
        match element {
            Element::Metal => Self::Metal,
            Element::Wood => Self::Wood,
            Element::Water => Self::Water,
            Element::Fire => Self::Fire,
            Element::Earth => Self::Earth,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpiritKind {
    Metal,
    Wood,
    Water,
    Fire,
    Earth,
    Evil,
    Death,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerSpirit {
    pub player: PlayerId,
    pub spirit: SpiritKind,
    pub power: u32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum SpiritSkill {
    FlyingBlade,
    SwordRain,
    Fragrance,
    Bloom,
    Flow,
    Vastness,
    Glimmer,
    Splendor,
    StoneShield,
    RockWall,
    EvilGaze,
    DeathOmen,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpiritLevelInterpretation {
    pub player: PlayerId,
    #[serde(default)]
    pub skill: Option<SpiritSkill>,
    pub card: CardInstanceId,
    pub level: EffectiveCardLevel,
    pub applied_on_turn: u64,
    pub interpretation_revision: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SpiritBreakReason {
    PowerDepleted,
    VoidSpiritShattering,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct SpiritPowerDelta {
    pub player: PlayerId,
    pub spirit: SpiritKind,
    pub old_power: u32,
    pub delta: i32,
    pub new_power: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TeamBloomResolution {
    pub team: TeamId,
    pub spirit_changes: Vec<SpiritPowerDelta>,
    pub hp_change: HpChangeDelta,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum SpiritPowerChangeReason {
    TurnDrawDiscard { card: CardInstanceId },
    SkillEffect,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct StarElementSubstitution {
    pub card: CardInstanceId,
    pub printed_element: Element,
    pub interpreted_element: Element,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TeamStar {
    pub team: TeamId,
    pub star: StarKind,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum SecretStrategy {
    GoldenCicada,
    StealTheBeam,
    MuddyWaters,
    WatchTheFire,
    LureTheTigerAway,
    ReturnSoul,
    SheepStealing,
    DarkCrossing,
    DeceiveHeaven,
    Retreat,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerPouch {
    pub owner: PlayerId,
    pub card: CardInstanceId,
    pub known_by: Vec<PlayerId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PouchLevelBonus {
    pub player: PlayerId,
    pub cards: Vec<CardInstanceId>,
    pub applied_on_turn: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TemporaryStarEffect {
    pub player: PlayerId,
    pub star: StarKind,
    pub applied_on_turn: u64,
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
    SecretStrategy,
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
    pub level: PrintedCardLevel,
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
    Preparing { stage: GamePreparationStage },
    InProgress,
    Finished { conclusion: GameConclusion },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GamePreparationStage {
    InitialPouchSelection,
    PendingDeckShuffle,
    InitialDeal,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameOutcome {
    Winner(TeamId),
    Draw,
}

/// The immutable terminal fact for a Game.  This deliberately does not borrow
/// from a Formation Area or previous-turn query: those are mutable projections,
/// while a conclusion is part of the canonical record.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct GameConclusion {
    pub outcome: GameOutcome,
    pub causes: Vec<GameEndCause>,
    #[serde(default)]
    pub source_formation: Option<ConcludingFormationSnapshot>,
}

impl GameConclusion {
    pub fn new(
        outcome: GameOutcome,
        causes: Vec<GameEndCause>,
        source_formation: Option<ConcludingFormationSnapshot>,
    ) -> Self {
        assert!(
            !causes.is_empty(),
            "a Game Conclusion must have at least one Game End Cause"
        );
        Self {
            outcome,
            causes,
            source_formation,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameEndCause {
    TeamHpDepleted { teams: Vec<TeamId> },
    DirectVictory { rule: String, team: TeamId },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ConcludingFormationSnapshot {
    pub player: PlayerId,
    pub formation_id: String,
    pub cards: Vec<CardInstanceId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerShield {
    pub player: PlayerId,
    pub value: i32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct FormationInArea {
    pub formation_id: String,
    pub cards: Vec<CardInstanceId>,
    #[serde(default)]
    pub star_substitution: Option<StarElementSubstitution>,
    pub state: FormationAreaState,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FormationAreaState {
    FaceUpResolving,
    FaceDownResolving,
    FaceDownWaiting {
        sealed: bool,
        #[serde(default)]
        revealed: bool,
        #[serde(default)]
        neutralized: bool,
        trigger_timing: PassiveTriggerTiming,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PlayerFormationArea {
    pub player: PlayerId,
    pub formation: Option<FormationInArea>,
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

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum JianghuStateKind {
    ThousandBlades,
    SnowTreading,
    Poison,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct JianghuState {
    pub owner: PlayerId,
    pub kind: JianghuStateKind,
    #[serde(default)]
    pub remaining_turns: u32,
    pub expires_on_turn: Option<u64>,
    #[serde(default)]
    pub last_resolved_turn: Option<u64>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LimitedUse {
    pub owner: PlayerId,
    pub key: String,
    pub remaining: u32,
    pub maximum: u32,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ConfluenceCardObligation {
    pub owner: PlayerId,
    pub card: CardInstanceId,
    pub allow_profession_formation: bool,
    pub applied_on_turn: u64,
    pub residual_element: Option<Element>,
    pub residual_level: Option<PrintedCardLevel>,
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
pub enum TimedEffectReduction {
    Status {
        status_id: String,
        owner: StatusOwner,
        old_duration: StatusDuration,
        new_duration: Option<StatusDuration>,
    },
    CoveredPassive {
        owner: PlayerId,
    },
    CounterEffect {
        owner: PlayerId,
        effect_id: String,
    },
    JianghuState {
        owner: PlayerId,
        kind: JianghuStateKind,
        old_remaining_turns: u32,
        new_remaining_turns: u32,
        old_expires_on_turn: Option<u64>,
        new_expires_on_turn: Option<u64>,
    },
    FlowState {
        player: PlayerId,
        old_layers: u32,
        new_layers: u32,
    },
    FormationSuppression {
        target: PlayerId,
        formation_id: String,
    },
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
    ActiveEffects,
    Action,
    TurnDraw,
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
    pub pouches: Vec<PlayerPouch>,
    #[serde(default)]
    pub pouch_level_bonuses: Vec<PouchLevelBonus>,
    #[serde(default)]
    pub temporary_star_effects: Vec<TemporaryStarEffect>,
    #[serde(default)]
    pub exposed_foreign_cards: Vec<CardInstanceId>,
    #[serde(default)]
    pub last_turn_discard_by_player: HashMap<PlayerId, LastTurnDiscard>,
    /// The sole game-scoped holding zone for the N+1 cards waiting for the
    /// Turn Draw discard answer.  Cards here are in neither a hand nor a pile.
    #[serde(default)]
    pub turn_draw_pool: Vec<CardInstanceId>,
    pub pending_choice: Option<PendingChoice>,
    pub next_choice_id: ChoiceId,
    #[serde(default)]
    pub pending_randomness: Option<PendingRandomness>,
    pub shields: Vec<PlayerShield>,
    /// Every Player owns exactly one Formation Area.  Covered Passives are a
    /// face-down state of a Formation in this collection, never a second zone.
    #[serde(default)]
    pub formation_areas: Vec<PlayerFormationArea>,
    #[serde(default)]
    pub counter_effects: Vec<CounterEffect>,
    pub statuses: Vec<StatusEffect>,
    #[serde(default)]
    pub jianghu_states: Vec<JianghuState>,
    #[serde(default)]
    pub limited_uses: Vec<LimitedUse>,
    #[serde(default)]
    pub confluence_card_obligations: Vec<ConfluenceCardObligation>,
    #[serde(default)]
    pub scheduled_echoes: Vec<ScheduledEcho>,
    #[serde(default)]
    pub flow_layers_by_player: HashMap<PlayerId, u32>,
    #[serde(default)]
    pub flow_triggered_turn_by_player: HashMap<PlayerId, u64>,
    #[serde(default)]
    pub formation_suppressions: Vec<FormationSuppression>,
    #[serde(default)]
    pub active_echo_resolution: Option<ScheduledEcho>,
    #[serde(default)]
    pub ringing_metal_selection: Option<RingingMetalSelection>,
    #[serde(default)]
    pub scheduled_plant_earth: Vec<ScheduledPlantEarth>,
    #[serde(default)]
    pub active_plant_earth_resolution: Option<ScheduledPlantEarth>,
    #[serde(default)]
    pub active_earth_rending_resolution: Option<EarthRendingResolution>,
    #[serde(default)]
    pub active_rusted_forest_resolution: Option<RustedForestResolution>,
    #[serde(default)]
    pub environment: Option<Element>,
    #[serde(default)]
    pub team_stars: Vec<TeamStar>,
    #[serde(default)]
    pub star_histories: Vec<PlayerStarHistory>,
    #[serde(default)]
    pub five_star_alignment: Option<FiveStarAlignment>,
    #[serde(default)]
    pub professions: Vec<PlayerProfession>,
    #[serde(default)]
    pub spirits: Vec<PlayerSpirit>,
    #[serde(default)]
    pub spirit_skill_use_turns: HashMap<PlayerId, u64>,
    #[serde(default)]
    pub spirit_level_interpretations: Vec<SpiritLevelInterpretation>,
    /// Ordered Card Interpretation Layers projected from semantic canonical
    /// events. Ordinary rule consumers must resolve physical card facts through
    /// this collection instead of reading or recomposing event-specific state.
    #[serde(default)]
    pub card_interpretation_layers: Vec<CardInterpretationLayer>,
    #[serde(default)]
    pub card_interpretation_revision: u64,
    #[serde(default)]
    pub prepared_profession_abilities: Vec<PreparedProfessionAbility>,
    #[serde(default)]
    pub formation_requirements: Vec<FormationRequirement>,
    #[serde(default)]
    pub activated_profession_ability_turns: HashMap<PlayerId, u64>,
    pub last_elemental_attack_by_player: HashMap<PlayerId, LastElementalAttack>,
    pub last_formation_by_player: HashMap<PlayerId, LastFormationUse>,
    pub turn_draw_bonus_by_player: HashMap<PlayerId, usize>,
    pub hand_limit: usize,
    pub base_draw: usize,
}

impl GameState {
    pub fn from_setup(setup: &GameSetup) -> Self {
        let status = if setup.has_rule_module(POUCH_MODULE_ID) {
            GameStatus::Preparing {
                stage: GamePreparationStage::InitialPouchSelection,
            }
        } else {
            GameStatus::InProgress
        };
        Self {
            ruleset: setup.ruleset.clone(),
            enabled_rule_modules: setup.enabled_rule_modules.clone(),
            status,
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
            pouches: Vec::new(),
            pouch_level_bonuses: Vec::new(),
            temporary_star_effects: Vec::new(),
            exposed_foreign_cards: Vec::new(),
            last_turn_discard_by_player: HashMap::new(),
            turn_draw_pool: Vec::new(),
            pending_choice: None,
            next_choice_id: ChoiceId::new(1),
            pending_randomness: None,
            shields: setup
                .players
                .iter()
                .map(|player| PlayerShield {
                    player: player.id.clone(),
                    value: 0,
                })
                .collect(),
            formation_areas: setup
                .players
                .iter()
                .map(|player| PlayerFormationArea {
                    player: player.id.clone(),
                    formation: None,
                })
                .collect(),
            counter_effects: Vec::new(),
            statuses: Vec::new(),
            jianghu_states: Vec::new(),
            limited_uses: Vec::new(),
            confluence_card_obligations: Vec::new(),
            scheduled_echoes: Vec::new(),
            flow_layers_by_player: HashMap::new(),
            flow_triggered_turn_by_player: HashMap::new(),
            formation_suppressions: Vec::new(),
            active_echo_resolution: None,
            ringing_metal_selection: None,
            scheduled_plant_earth: Vec::new(),
            active_plant_earth_resolution: None,
            active_earth_rending_resolution: None,
            active_rusted_forest_resolution: None,
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
            professions: Vec::new(),
            spirits: Vec::new(),
            spirit_skill_use_turns: HashMap::new(),
            spirit_level_interpretations: Vec::new(),
            card_interpretation_layers: Vec::new(),
            card_interpretation_revision: 0,
            prepared_profession_abilities: Vec::new(),
            formation_requirements: Vec::new(),
            activated_profession_ability_turns: HashMap::new(),
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

    pub fn formation_area(&self, player: &PlayerId) -> Option<&PlayerFormationArea> {
        self.formation_areas
            .iter()
            .find(|area| &area.player == player)
    }

    pub fn formation_area_mut(&mut self, player: &PlayerId) -> Option<&mut PlayerFormationArea> {
        self.formation_areas
            .iter_mut()
            .find(|area| &area.player == player)
    }

    pub fn covered_passive(&self, player: &PlayerId) -> Option<&FormationInArea> {
        self.formation_area(player)?
            .formation
            .as_ref()
            .filter(|formation| {
                matches!(formation.state, FormationAreaState::FaceDownWaiting { .. })
            })
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

    pub fn profession_for(&self, player: &PlayerId) -> Option<&ProfessionId> {
        self.professions
            .iter()
            .find(|owned| &owned.player == player)
            .map(|owned| &owned.profession)
    }

    pub fn spirit_for(&self, player: &PlayerId) -> Option<&PlayerSpirit> {
        self.spirits.iter().find(|owned| &owned.player == player)
    }

    /// Resolves the one effective element and level exposed by a physical Card.
    ///
    /// Layer order is canonical event order. Relative adjustments may leave the
    /// Base Ruleset range while composing; the range is applied exactly once to
    /// the final composed level.
    pub fn effective_card_facts(
        &self,
        player: &PlayerId,
        card: CardInstanceId,
    ) -> Option<EffectiveCardFacts> {
        let definition = self.card_def(card)?;
        let mut element = definition.element;
        let mut composed_level = definition.level.value() as i32;

        for layer in self.card_interpretation_layers.iter().filter(|layer| {
            &layer.player == player
                && layer.card == card
                && layer.applied_on_turn == self.turn_number
        }) {
            if let Some(interpreted_element) = layer.element {
                element = interpreted_element;
            }
            match layer.level {
                Some(CardLevelInterpretation::Set(level)) => {
                    composed_level = level.value() as i32;
                }
                Some(CardLevelInterpretation::Adjust(delta)) => {
                    composed_level += delta;
                }
                None => {}
            }
        }

        Some(EffectiveCardFacts {
            element,
            level: EffectiveCardLevel::clamp_composed(composed_level),
        })
    }

    /// A compatibility convenience for callers that require only the bounded
    /// effective level. It delegates exclusively to `effective_card_facts`.
    pub fn card_level_for(
        &self,
        player: &PlayerId,
        card: CardInstanceId,
    ) -> Option<EffectiveCardLevel> {
        self.effective_card_facts(player, card)
            .map(|facts| facts.level)
    }

    pub fn pouch_for(&self, player: &PlayerId) -> Option<&PlayerPouch> {
        self.pouches.iter().find(|pouch| &pouch.owner == player)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(transparent)]
pub struct ChoiceId(u64);

impl ChoiceId {
    pub const fn new(value: u64) -> Self {
        Self(value)
    }

    pub const fn value(self) -> u64 {
        self.0
    }

    pub const fn next(self) -> Self {
        Self(self.0 + 1)
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PendingChoice {
    pub choice_id: ChoiceId,
    pub player: PlayerId,
    pub kind: PendingChoiceKind,
    pub continuation: ChoiceContinuation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ChoiceRequest {
    pub player: PlayerId,
    pub kind: PendingChoiceKind,
    pub continuation: ChoiceContinuation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ChoiceAnswer {
    Cards {
        cards: Vec<CardInstanceId>,
    },
    Player {
        player: PlayerId,
    },
    Formation {
        #[serde(rename = "formationId")]
        formation_id: String,
    },
    Environment {
        environment: Element,
    },
    Chain {
        #[serde(rename = "pouchOwner")]
        pouch_owner: PlayerId,
        #[serde(rename = "pouchCard")]
        pouch_card: CardInstanceId,
        #[serde(
            rename = "triggerCard",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        trigger_card: Option<CardInstanceId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        strategy: Option<SecretStrategy>,
        #[serde(
            rename = "targetPlayer",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        target_player: Option<PlayerId>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        star: Option<StarKind>,
        #[serde(rename = "breakStar", default)]
        break_star: bool,
        #[serde(
            rename = "discardCard",
            default,
            skip_serializing_if = "Option::is_none"
        )]
        discard_card: Option<CardInstanceId>,
    },
    SheepStealing {
        #[serde(rename = "deckCards")]
        deck_cards: Vec<CardInstanceId>,
        #[serde(rename = "discardCards")]
        discard_cards: Vec<CardInstanceId>,
    },
    Decline,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum PendingChoiceKind {
    Card {
        cards: Vec<CardInstanceId>,
        minimum: usize,
        maximum: usize,
        #[serde(default)]
        can_decline: bool,
    },
    Player {
        players: Vec<PlayerId>,
        #[serde(default)]
        can_decline: bool,
    },
    Formation {
        formations: Vec<String>,
        #[serde(default)]
        can_decline: bool,
    },
    Environment {
        environments: Vec<Element>,
        #[serde(default)]
        can_decline: bool,
    },
    Chain {
        #[serde(rename = "pouchOwners")]
        pouch_owners: Vec<PlayerId>,
        #[serde(rename = "deckCards")]
        deck_cards: Vec<CardInstanceId>,
    },
    SheepStealing {
        #[serde(rename = "sourceCard")]
        source_card: CardInstanceId,
        owner: Option<PlayerId>,
        #[serde(rename = "deckCards")]
        deck_cards: Vec<CardInstanceId>,
        #[serde(rename = "discardCards")]
        discard_cards: Vec<CardInstanceId>,
    },
}

impl PendingChoiceKind {
    pub fn card_bounds(&self) -> Option<(usize, usize)> {
        match self {
            Self::Card {
                minimum, maximum, ..
            } => Some((*minimum, *maximum)),
            _ => None,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", content = "kind", rename_all = "camelCase")]
pub enum ChoiceContinuation {
    Base(BaseChoiceContinuation),
    Echo(EchoChoiceContinuation),
    Hero(HeroChoiceContinuation),
    Jianghu(JianghuChoiceContinuation),
    Confluence(ConfluenceChoiceContinuation),
    Pouch(PouchChoiceContinuation),
    Tribulation(TribulationChoiceContinuation),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BaseChoiceContinuation {
    TurnDrawDiscard,
    HolyWindTakeHighest,
    ChaosReturnTwo,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EchoChoiceContinuation {
    PureFireTarget,
    SplitEarthFormation,
    RingingMetalDeckCard,
    PlantEarthMelody,
    Cost { melody_id: String },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HeroChoiceContinuation {
    RevelationKeepOne,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum JianghuChoiceContinuation {
    AzureCloudStepReturnOne,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConfluenceChoiceContinuation {
    DiscardInspectedCard {
        resonance: ConfluenceResonance,
        after: Option<Element>,
    },
    ClearWindKeepCards,
    ClearWindDiscardTop,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConfluenceResonance {
    Mirror,
    Myriad,
    Thousand,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PouchChoiceContinuation {
    Chain,
    SheepStealing,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TribulationChoiceContinuation {
    EarthRendingEnvironment,
    EarthRendingCard,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PendingRandomness {
    pub request_id: String,
    pub operation: RandomnessOperation,
    pub continuation: RandomnessContinuation,
    pub current_order: Vec<CardInstanceId>,
}

/// The pile mutation performed by a trusted randomness decision.
///
/// This deliberately lives beside, rather than inside, the continuation: a
/// continuation says which rule flow resumes, while this value says exactly
/// which pile supplied the permutation and where its result is placed.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum RandomnessOperation {
    DeckShuffle {
        deck: RandomnessDeck,
    },
    DiscardShuffle {
        pile: RandomnessDeck,
        placement: DeckPlacement,
    },
}

impl RandomnessOperation {
    pub fn source_pile(&self) -> &RandomnessDeck {
        match self {
            Self::DeckShuffle { deck } => deck,
            Self::DiscardShuffle { pile, .. } => pile,
        }
    }

    pub fn destination_deck(&self) -> &RandomnessDeck {
        match self {
            Self::DeckShuffle { deck } => deck,
            Self::DiscardShuffle { pile, .. } => pile,
        }
    }

    pub fn is_discard_shuffle(&self) -> bool {
        matches!(self, Self::DiscardShuffle { .. })
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(tag = "type", content = "kind", rename_all = "camelCase")]
pub enum RandomnessContinuation {
    Base(BaseRandomnessContinuation),
    Spirit(SpiritRandomnessContinuation),
    Echo(EchoRandomnessContinuation),
    Hero(HeroRandomnessContinuation),
    Confluence(ConfluenceRandomnessContinuation),
    Pouch(PouchRandomnessContinuation),
    Tribulation(TribulationRandomnessContinuation),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum BaseRandomnessContinuation {
    TurnDraw,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum SpiritRandomnessContinuation {
    DeathOmen { player: PlayerId },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum EchoRandomnessContinuation {
    RingingMetalRecycleDiscard,
    RingingMetalPostSearch,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum HeroRandomnessContinuation {
    Revelation,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum ConfluenceRandomnessContinuation {
    ClearWindTenThousandMiles,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum PouchRandomnessContinuation {
    InitialShuffle,
    ChainRecycle,
    SheepStealingRecycle {
        #[serde(rename = "sourceCard")]
        source_card: CardInstanceId,
    },
    SheepStealing {
        #[serde(rename = "sourceCard")]
        source_card: CardInstanceId,
        owner: Option<PlayerId>,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum TribulationRandomnessContinuation {
    RustedForestDiscardShuffle,
    RustedForestShuffle,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum RandomnessDeck {
    Shared,
    Player(PlayerId),
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct TrustedRandomnessAnswer {
    pub request_id: String,
    pub shuffled_order: Vec<CardInstanceId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledEcho {
    pub player: PlayerId,
    pub melody_id: String,
    pub due_turn_number: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct FormationSuppression {
    pub source: PlayerId,
    pub target: PlayerId,
    pub formation_id: String,
    pub expires_on_turn_number: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RingingMetalSelection {
    pub player: PlayerId,
    pub card: CardInstanceId,
    pub deck: RandomnessDeck,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledPlantEarth {
    pub player: PlayerId,
    pub due_turn_number: u64,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EarthRendingPlayerAnswer {
    pub player: PlayerId,
    pub card: Option<CardInstanceId>,
    pub revealed_hand: Vec<CardInstanceId>,
    pub protected: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EarthRendingResolution {
    pub attacker: PlayerId,
    pub used_cards: Vec<CardInstanceId>,
    pub environment: Option<Element>,
    pub remaining_players: Vec<PlayerId>,
    pub answers: Vec<EarthRendingPlayerAnswer>,
    pub damage_prevented: bool,
    pub split_attack_damage: bool,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct RustedForestResolution {
    pub attacker: PlayerId,
    pub used_cards: Vec<CardInstanceId>,
    pub remaining_decks: Vec<RandomnessDeck>,
    pub damage_prevented: bool,
    pub split_attack_damage: bool,
}

// Canonical events intentionally remain unboxed: their variants are projected, replayed, and
// serialized directly.  Indirection here would add allocation without changing the public
// record shape.
#[allow(clippy::large_enum_variant)]
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum GameEvent {
    GamePreparationStarted {
        player_decks: Vec<PlayerCardPile>,
    },
    InitialPouchChosen {
        player: PlayerId,
        card: CardInstanceId,
    },
    InitialPouchSelectionCompleted,
    GamePreparationCompleted,
    PouchPlaced {
        source: PlayerId,
        owner: PlayerId,
        card: CardInstanceId,
        known_by: Vec<PlayerId>,
        previous: Option<CardInstanceId>,
    },
    PouchRevealed {
        player: PlayerId,
        owner: Option<PlayerId>,
        card: CardInstanceId,
        strategy: SecretStrategy,
    },
    PouchConsumed {
        owner: Option<PlayerId>,
        card: CardInstanceId,
    },
    PouchLevelBonusGranted {
        bonus: PouchLevelBonus,
    },
    TemporaryStarEffectGranted {
        effect: TemporaryStarEffect,
    },
    SpiritRevived {
        player: PlayerId,
        previous: Option<SpiritKind>,
        spirit: SpiritKind,
        power: u32,
    },
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
    ProfessionChanged {
        player: PlayerId,
        previous: Option<ProfessionId>,
        profession: ProfessionId,
        card_moves: Vec<CardMoveDelta>,
    },
    ProfessionTransformed {
        player: PlayerId,
        previous: Option<ProfessionId>,
        profession: ProfessionId,
        reason: String,
    },
    ProfessionBroken {
        player: PlayerId,
        profession: ProfessionId,
    },
    ProfessionAbilityActivated {
        player: PlayerId,
        ability_id: String,
        prepared: Option<PreparedProfessionAbility>,
    },
    FormationRequirementSet {
        requirement: FormationRequirement,
    },
    FormationRequirementFulfilled {
        player: PlayerId,
        formation_id: String,
        composition: FormationComposition,
    },
    SpiritSummoned {
        player: PlayerId,
        previous: Option<SpiritKind>,
        spirit: SpiritKind,
    },
    SpiritTransformed {
        player: PlayerId,
        previous: SpiritKind,
        spirit: SpiritKind,
        power: u32,
    },
    SpiritPowerChanged {
        player: PlayerId,
        spirit: SpiritKind,
        old_power: u32,
        delta: i32,
        new_power: u32,
        reason: SpiritPowerChangeReason,
    },
    SpiritSkillUsed {
        player: PlayerId,
        spirit: SpiritKind,
        skill: SpiritSkill,
        old_power: u32,
        new_power: u32,
        selected_card: Option<CardInstanceId>,
        declared_level: Option<u32>,
    },
    SpiritLevelInterpreted {
        player: PlayerId,
        #[serde(default)]
        skill: Option<SpiritSkill>,
        card: CardInstanceId,
        level: EffectiveCardLevel,
        applied_on_turn: u64,
        interpretation_revision: u64,
    },
    SpiritBroken {
        player: PlayerId,
        spirit: SpiritKind,
        reason: SpiritBreakReason,
    },
    AutomaticBloomsResolved {
        resolutions: Vec<TeamBloomResolution>,
    },
    /// Commits physical Formation cards from the performer's hand into that
    /// Player's Formation Area after validation succeeds.  Subsequent
    /// prevention or ineffectiveness never reverses this fact.
    FormationCommitted {
        player: PlayerId,
        formation_id: String,
        cards: Vec<CardInstanceId>,
        #[serde(default)]
        star_substitution: Option<StarElementSubstitution>,
        state: FormationAreaState,
    },
    /// The normal face-up completion boundary for a Formation.  Replay moves
    /// every listed card from the owner's Formation Area to its origin discard.
    FormationCardsDiscarded {
        player: PlayerId,
        formation_id: String,
        cards: Vec<CardInstanceId>,
    },
    /// Marks the first accepted non-Formation Action Command. Formation
    /// actions enter Action through FormationCommitted.
    ActionStarted {
        player: PlayerId,
    },
    CardsDrawnForTurnDiscardChoice {
        player: PlayerId,
        drawn_cards: Vec<CardInstanceId>,
        allowed_discards: Vec<CardInstanceId>,
    },
    CardsDrawnForProfessionChoice {
        player: PlayerId,
        ability_id: String,
        cards: Vec<CardInstanceId>,
    },
    TurnDiscardChosen {
        player: PlayerId,
        discard: CardInstanceId,
    },
    /// Atomic Turn Draw completion.  `discard` is moved to its origin discard
    /// before `kept_cards` enter the hand, with no replay state between them.
    TurnDrawResolved {
        player: PlayerId,
        discard: CardInstanceId,
        kept_cards: Vec<CardInstanceId>,
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
    FormationMatchOptionDeclared {
        player: PlayerId,
        formation_id: String,
        targets: Vec<TargetDecl>,
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
    DeckTopRevealed {
        player: PlayerId,
        card: CardInstanceId,
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
        /// The complete simultaneous semantic result for this attack.  It is
        /// optional only for backward-compatible decoding of records written
        /// before attack resolutions became atomic.
        #[serde(default)]
        elemental_context_update: Option<AttackResolutionEffects>,
    },
    /// A terminal conclusion is always the final canonical event.  It is
    /// intentionally separate from card zones and previous-Formation state.
    GameEnded {
        conclusion: GameConclusion,
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
    VoidReversionResolved {
        player: PlayerId,
        hp_change: HpChangeDelta,
        card_moves: Vec<CardMoveDelta>,
        broken_professions: Vec<PlayerProfession>,
        retained_legendary_professions: Vec<PlayerProfession>,
    },
    VoidSpiritShatteringResolved {
        player: PlayerId,
        card_moves: Vec<CardMoveDelta>,
        spirit_changes: Vec<SpiritPowerDelta>,
        broken_spirits: Vec<PlayerSpirit>,
        hp_changes: Vec<HpChangeDelta>,
        #[serde(default)]
        broken_professions: Vec<PlayerProfession>,
        #[serde(default)]
        revived_spirits: Vec<PlayerSpirit>,
        #[serde(default)]
        shared_fate_hp_changes: Vec<HpChangeDelta>,
    },
    FiveStarAlignmentAchieved {
        player: PlayerId,
        team: TeamId,
    },
    KingYamaDecreeVictoryAchieved {
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
    JianghuStateApplied {
        state: JianghuState,
    },
    JianghuStateExpired {
        owner: PlayerId,
        kind: JianghuStateKind,
    },
    JianghuPoisonTicked {
        owner: PlayerId,
        damage: i32,
        remaining_turns: u32,
        hp_change: HpChangeDelta,
        #[serde(default)]
        shared_fate_hp_change: Option<HpChangeDelta>,
    },
    JianghuDelayedDamageResolved {
        owner: PlayerId,
        status_id: String,
        hp_change: HpChangeDelta,
        #[serde(default)]
        shared_fate_hp_change: Option<HpChangeDelta>,
    },
    LimitedUseChanged {
        owner: PlayerId,
        key: String,
        old_remaining: u32,
        new_remaining: u32,
        maximum: u32,
    },
    ConfluenceCardObligationSet {
        obligation: ConfluenceCardObligation,
    },
    ConfluenceCardObligationCleared {
        owner: PlayerId,
        card: CardInstanceId,
    },
    ChoiceRequested {
        choice: PendingChoice,
    },
    ChoiceMade {
        player: PlayerId,
        choice_id: ChoiceId,
        answer: ChoiceAnswer,
    },
    RandomnessRequested {
        request: PendingRandomness,
    },
    RandomnessResolved {
        request_id: String,
        operation: RandomnessOperation,
        shuffled_order: Vec<CardInstanceId>,
    },
    EchoCostPaid {
        player: PlayerId,
        melody_id: String,
        card_move: CardMoveDelta,
    },
    EchoDeclined {
        player: PlayerId,
        melody_id: String,
    },
    EchoScheduled {
        schedule: ScheduledEcho,
    },
    EchoResolutionStarted {
        schedule: ScheduledEcho,
    },
    EchoResolutionCompleted {
        player: PlayerId,
        melody_id: String,
        due_turn_number: u64,
    },
    TimedEffectsReduced {
        source: PlayerId,
        target: PlayerId,
        reductions: Vec<TimedEffectReduction>,
    },
    FlowStateChanged {
        player: PlayerId,
        old_layers: u32,
        new_layers: u32,
    },
    FlowStateTriggered {
        player: PlayerId,
        old_layers: u32,
        new_layers: u32,
        old_draw_bonus: usize,
        new_draw_bonus: usize,
    },
    FormationSuppressionSet {
        suppression: FormationSuppression,
    },
    FormationSuppressionExpired {
        target: PlayerId,
        formation_id: String,
        expired_on_turn_number: u64,
    },
    RingingMetalCardRevealed {
        selection: RingingMetalSelection,
    },
    RingingMetalCompleted {
        selection: RingingMetalSelection,
    },
    PlantEarthScheduled {
        schedule: ScheduledPlantEarth,
    },
    PlantEarthResolutionStarted {
        schedule: ScheduledPlantEarth,
    },
    PlantEarthResolutionCompleted {
        player: PlayerId,
        due_turn_number: u64,
        melody_id: String,
    },
    EarthRendingStarted {
        resolution: EarthRendingResolution,
    },
    EarthRendingEnvironmentChosen {
        environment: Element,
    },
    EarthRendingPlayerAnswered {
        answer: EarthRendingPlayerAnswer,
    },
    HandRevealed {
        player: PlayerId,
        cards: Vec<CardInstanceId>,
    },
    EarthRendingCompleted {
        player: PlayerId,
    },
    RustedForestStarted {
        resolution: RustedForestResolution,
    },
    RustedForestCardsRevealed {
        deck: RandomnessDeck,
        cards: Vec<CardInstanceId>,
    },
    RustedForestDeckProcessed {
        deck: RandomnessDeck,
    },
    RustedForestCompleted {
        player: PlayerId,
    },
    PassiveCovered {
        player: PlayerId,
        formation_id: String,
        cards: Vec<CardInstanceId>,
        #[serde(default)]
        star_substitution: Option<StarElementSubstitution>,
        sealed: bool,
    },
    PassiveCoverRevealed {
        owner: PlayerId,
    },
    PassiveFlipped {
        owner: PlayerId,
        incoming_player: PlayerId,
        passive_id: String,
        cards: Vec<CardInstanceId>,
        outcome: PassiveFlipOutcome,
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
    ChooseInitialPouch {
        player: PlayerId,
        card: CardInstanceId,
    },
    TriggerSecretStrategy {
        player: PlayerId,
        strategy: SecretStrategy,
        target_player: Option<PlayerId>,
        star: Option<StarKind>,
        break_star: bool,
        discard_card: Option<CardInstanceId>,
        deck_cards: Vec<CardInstanceId>,
        discard_cards: Vec<CardInstanceId>,
    },
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
    PerformFormationWithTrustedRandomness {
        player: PlayerId,
        formation_id: String,
        cards: Vec<CardInstanceId>,
        declared_targets: Vec<TargetDecl>,
        random_cards: Vec<CardInstanceId>,
    },
    ChangeProfession {
        player: PlayerId,
        profession: ProfessionId,
        cards: Vec<CardInstanceId>,
    },
    ActivateProfessionAbility {
        player: PlayerId,
        ability_id: String,
        cards: Vec<CardInstanceId>,
        target_card: Option<CardInstanceId>,
        declared_element: Option<Element>,
        declared_level: Option<u32>,
    },
    UseSpiritSkill {
        player: PlayerId,
        skill: SpiritSkill,
        selected_card: Option<CardInstanceId>,
        declared_level: Option<u32>,
    },
    UseSpiritSkillWithTrustedRandomness {
        player: PlayerId,
        skill: SpiritSkill,
        selected_card: Option<CardInstanceId>,
        declared_level: Option<u32>,
        random_cards: Vec<CardInstanceId>,
    },
    #[serde(rename = "answerChoice")]
    AnswerChoice {
        player: PlayerId,
        #[serde(rename = "choiceId")]
        choice_id: ChoiceId,
        answer: ChoiceAnswer,
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
    FormationRole {
        role: String,
        card: CardInstanceId,
    },
    CardMultiplicity {
        card: CardInstanceId,
        slots: usize,
    },
    SecretStrategy(SecretStrategy),
    SecretStrategyOptions {
        target_player: Option<PlayerId>,
        star: Option<StarKind>,
        break_star: bool,
        discard_card: Option<CardInstanceId>,
        deck_cards: Vec<CardInstanceId>,
        discard_cards: Vec<CardInstanceId>,
    },
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
    RevealCoveredPassive,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PassiveNoEffectReason {
    NotAnAttack,
    NotASpell,
    Sealed,
    Neutralized,
    EmptyCity,
    IgnoredBySacredBeast,
    IgnoredByProfessionAbility,
    IneffectiveInEnvironment { environment: Element },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum FormationNoEffectReason {
    IneffectiveInEnvironment { environment: Element },
    SuppressedBySplitEarth { source: PlayerId },
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

/// Canonical deltas that occur at the same resolution point as an attack.
/// The vectors have stable serialization order for replay, but their order is
/// not a rules-processing priority: every delta is computed from the state
/// before the enclosing `AttackResolved` event.
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct AttackResolutionEffects {
    #[serde(default)]
    pub outcome: AttackOutcome,
    #[serde(default)]
    pub elemental_context_update: Option<LastElementalAttackUpdate>,
    #[serde(default)]
    pub hp_changes: Vec<HpChangeDelta>,
    #[serde(default)]
    pub shield_changes: Vec<ShieldChangeDelta>,
    #[serde(default)]
    pub card_moves: Vec<CardMoveDelta>,
    #[serde(default)]
    pub statuses_added: Vec<StatusEffect>,
    #[serde(default)]
    pub statuses_removed: Vec<AttackStatusRemoval>,
    #[serde(default)]
    pub counter_effects_established: Vec<AttackCounterEffect>,
    #[serde(default)]
    pub turn_draw_bonus_changes: Vec<TurnDrawBonusDelta>,
    #[serde(default)]
    pub environment_transfers: Vec<EnvironmentTransferDelta>,
}

impl AttackResolutionEffects {
    pub fn with_elemental_context(update: LastElementalAttackUpdate) -> Self {
        Self {
            elemental_context_update: Some(update),
            ..Self::default()
        }
    }

    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AttackOutcome {
    #[default]
    Resolved,
    DamagePrevented,
    AbsorbedByShield,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AttackStatusRemoval {
    pub status_id: String,
    pub owner: StatusOwner,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct AttackCounterEffect {
    pub owner: PlayerId,
    pub effect_id: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TurnDrawBonusDelta {
    pub player: PlayerId,
    pub old_value: usize,
    pub delta: i32,
    pub new_value: usize,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct EnvironmentTransferDelta {
    pub player: PlayerId,
    pub formation_id: String,
    pub from: Option<Element>,
    pub to: Element,
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
    Pouch(PlayerId),
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
    ReplayStepOutOfRange {
        step: usize,
        total_steps: usize,
    },
    GameFinished,
    GamePreparationInProgress,
    InitialPouchSelectionUnavailable,
    InitialPouchAlreadyChosen {
        player: PlayerId,
    },
    InvalidInitialPouch(CardInstanceId),
    PouchRuleDisabled,
    NoPouch {
        player: PlayerId,
    },
    SecretStrategyConditionMismatch,
    SecretStrategyInputInvalid,
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
    InvalidChoiceAnswer,
    StaleChoiceId {
        expected: ChoiceId,
        actual: ChoiceId,
    },
    PendingChoiceInProgress {
        player: PlayerId,
    },
    PendingRandomnessInProgress {
        request_id: String,
    },
    MissingPendingRandomness,
    StalePendingRandomness,
    InvalidRandomnessPermutation,
    IllegalDiscard(CardInstanceId),
    IllegalChoiceCard(CardInstanceId),
    DuplicateChoiceCard(CardInstanceId),
    UnknownFormation(String),
    HeroSchoolsDisabled,
    UnknownProfession(ProfessionId),
    ProfessionPrerequisiteNotMet {
        profession: ProfessionId,
        required: ProfessionId,
        actual: Option<ProfessionId>,
    },
    ProfessionChangePatternMismatch {
        profession: ProfessionId,
    },
    UnknownProfessionAbility(String),
    ProfessionAbilityUnavailable(String),
    ProfessionAbilityAlreadyActivated {
        player: PlayerId,
        turn_number: u64,
    },
    ProfessionAbilityCannotResolve(String),
    SpiritRuleDisabled,
    NoSpirit {
        player: PlayerId,
    },
    SpiritSkillUnavailable {
        spirit: SpiritKind,
        skill: SpiritSkill,
    },
    SpiritSkillAlreadyUsed {
        player: PlayerId,
        turn_number: u64,
    },
    InsufficientSpiritPower {
        required: u32,
        actual: u32,
    },
    SpiritSkillInputInvalid {
        skill: SpiritSkill,
    },
    TrustedRandomSelectionRequired {
        effect_id: String,
    },
    UnexpectedDeclaredTargets {
        formation_id: String,
    },
    DuplicateSubmittedCard(CardInstanceId),
    CardNotInHand(CardInstanceId),
    FormationPatternMismatch {
        formation_id: String,
    },
    FormationMatchOptionRequired {
        formation_id: String,
    },
    CannotPerformFormation {
        reason: CannotPerformFormationReason,
    },
    CannotChangeProfession {
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
    MissingRuleModuleDependencies {
        module: RuleModuleId,
        required: Vec<RuleModuleId>,
    },
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
    InvalidPendingChoice,
    DuplicateFormationArea { player: PlayerId },
    FormationAreaMissing { player: PlayerId },
    DuplicateProfession { player: PlayerId },
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
            level_total += definition.level.value();
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
            let maximum = match definition.level.value() {
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
