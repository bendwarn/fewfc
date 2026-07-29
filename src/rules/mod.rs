//! Rule registries: formations, effects, matchers, and formula resolvers.

pub(crate) mod action_detail;
pub(crate) mod base;
pub(crate) mod confluence;
pub(crate) mod dark;
pub(crate) mod echo;
pub(crate) mod hero;
pub(crate) mod jianghu;
mod official;
pub(crate) mod pending_choice;
pub(crate) mod pouch;
pub(crate) mod profession;
pub(crate) mod projection;
pub(crate) mod randomness;
pub(crate) mod spirit;
pub(crate) mod star;
pub(crate) mod timed_effect;
pub(crate) mod tribulation;

pub use base::deck_composition::{
    DeckCompositionCardDefinition, DeckCompositionCatalog, DeckListIssue, DeckListSource,
    DeckListTemplate, DeckListValidation, PersonalDeckComposition, ResolvedPersonalDeck,
    SharedDeckComposition,
};
pub use official::{OfficialRuleModuleCategory, OfficialRuleModuleSpec, OfficialRules};

use crate::domain::EffectiveCardLevel;
pub use crate::domain::Element;
use serde::Serialize;
use std::collections::HashMap;

/// Optional player-visible, state-specific facts supplementing an offered action.
///
/// This deliberately excludes action identity and models commitments, not the
/// canonical events that will eventually be emitted. In particular it never
/// contains a Choice ID, continuation, hidden card, or a prediction of trusted
/// randomness.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlayerFacingActionDetail {
    pub consequences: Vec<RuleConsequence>,
}

impl PlayerFacingActionDetail {
    pub(crate) fn composed(consequences: Vec<RuleConsequence>) -> Self {
        Self { consequences }
    }

    pub(crate) fn pending_composition() -> Self {
        // Candidates are first assembled by their rule modules and then
        // completed by the single action-detail composer before they leave
        // `BaseRuleset::playable_actions`.
        Self {
            consequences: Vec::new(),
        }
    }

    pub(crate) fn into_optional(self) -> Option<Self> {
        (!self.consequences.is_empty()).then_some(self)
    }
}

/// The certainty of a clause.  This is separate from the consequence kind so
/// presentation cannot accidentally turn a choice, random outcome, or delayed
/// result into a guaranteed final state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ConsequenceCertainty {
    Guaranteed,
    Conditional,
    Random,
    FollowUp,
    Scheduled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RuleConsequence {
    Cost {
        certainty: ConsequenceCertainty,
        cost: ActionCost,
    },
    ImmediateEffect {
        certainty: ConsequenceCertainty,
        effect: ImmediateEffect,
    },
    FollowUpChoice {
        certainty: ConsequenceCertainty,
        choice: FollowUpChoice,
    },
    TrustedRandomness {
        certainty: ConsequenceCertainty,
        operation: TrustedRandomness,
    },
    DelayedEffect {
        certainty: ConsequenceCertainty,
        timing: DelayedTiming,
        effect: DelayedEffect,
    },
    RuleException {
        certainty: ConsequenceCertainty,
        exception: RuleException,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActionCost {
    DiscardSelectedCards,
    SpendSpiritPower {
        amount: u32,
    },
    LoseHp {
        amount: i32,
    },
    OptionalDiscardByPrintedElement {
        allowed_printed_elements: Vec<Element>,
    },
    ConsumePouch,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ImmediateEffect {
    Attack {
        target: ActionTarget,
        category: ActionAttackCategory,
        points: EffectAmount,
    },
    ResolveFormationEffect {
        effect: FormationEffect,
    },
    ActivateProfessionAbility {
        effect: ProfessionAbilityEffect,
    },
    UseSpiritSkill {
        effect: SpiritSkillEffect,
    },
    TriggerSecretStrategy {
        effect: SecretStrategyEffect,
    },
    MovePreviousTurnDiscardToDeckTop,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionTarget {
    SelfPlayer,
    SelfTeam,
    PreviousPlayer,
    PreviousTeam,
    NextPlayer,
    NextTeam,
    SelectedPlayer,
    AllPlayers,
    OtherPlayers,
    EachTeam,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ActionAttackCategory {
    Elemental,
    Physical,
    Special,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EffectAmount {
    Fixed { value: u32 },
    Formula { formula: EffectFormula },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum EffectFormula {
    LevelPlus { amount: u32 },
    LevelSumTimes { multiplier: u32 },
    TargetHandCountTimes { multiplier: u32 },
    ElementProductTimes { element: Element, multiplier: u32 },
}

/// Semantic main-effect facts reused by every action family.  A spell plan
/// must choose one explicit fact; there is deliberately no generic
/// "resolve spell" fallback.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum FormationEffect {
    CoverCounter,
    CopyPreviousTurnFormation,
    RecoverHp,
    ReduceShield,
    InspectHand,
    CreateShield,
    ReturnTeamHp,
    DrawCards,
    SwapTeamHp,
    SummonSpirit,
    ClearEnvironment,
    ApplyStatus,
    ChangeEnvironment,
    BreakProfession,
    LimitedUseRecovery,
    ResolveMelodyMainEffect,
    BeginChainChoice,
    ShatterSpirits,
    BreakStars,
    DamageEachTeamBy15,
    ApplyGaleRain,
    ReduceEveryShieldBy20,
    AttackIncreasesTo80IfShieldReduced,
    ChooseEnvironmentAndRequireMatchingCardOrRevealHand,
    RevealTopEightDiscardLevelThreeOrHigherThenShuffle,
    TransferEnvironmentToUsedElement,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum SecretStrategyEffect {
    ProtectTriggeringPlayer,
    IncreaseHandLevels,
    IncreaseTurnDraw,
    NegateNextPlayerFormationHpChanges,
    SuppressPlayerAbilitiesAndSpiritPower,
    SummonSpiritFromPouch,
    SwapDeckAndDiscard,
    DirectProfessionChange,
    BreakOrGainStar,
    ClearOrChangeEnvironment,
}

/// The semantic commitment made by an activated Profession Ability.  The
/// ability id remains an action identity for command validation; presentation
/// must use this closed effect fact instead of a browser-side id-to-prose map.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ProfessionAbilityEffect {
    DamagePreviousTeamByCardLevelTimes { multiplier: u32 },
    IncreaseTurnDraw { amount: u32 },
    DrawThreeThenChooseOne,
    CreateVirtualFormationCard { scope: VirtualFormationScope },
    ApplyYangAura,
    PrepareFormationDrawBonus,
    PrepareCardWithLevelBonus { amount: u32, maximum: u32 },
    PrepareMeteorEffect,
    DrawTwoThenReturnOne,
    RetrievePreviousPlayerDiscardForProfessionUse,
    RetrievePreviousPlayerDiscard,
    RevealDeckTopAndChooseDiscard,
    ApplyShuffleRecovery,
    PrepareCardAtDeclaredLevel,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum VirtualFormationScope {
    ElementalStrike,
    BaseFormation,
    AnyFormation,
}

/// The semantic main effect of a Spirit Skill.  Inputs and costs are separate
/// consequences so the same fact can be reused without hiding a commitment.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum SpiritSkillEffect {
    DamagePreviousTeam { amount: u32 },
    RecoverOwnTeam { amount: u32 },
    DiscardSelectedCardAndIncreaseTurnDraw { amount: u32 },
    IncreaseTurnDraw { amount: u32 },
    InterpretSelectedCardLevel,
    ProtectNextPlayerFromAttack,
    SetOwnShield { amount: u32 },
    InspectRandomNextPlayerHandCards { count: usize },
    DiscardNextPlayerDeckAndDamageByHighestLevel { count: usize, multiplier: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum FollowUpChoice {
    SelectPlayer,
    SelectFormation,
    SelectDeckCard,
    SelectEnvironment,
    SelectPouchOwnerAndOptionalStrategy,
    SelectCards { minimum: usize, maximum: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum TrustedRandomness {
    ShuffleDeck,
    ShuffleDiscardIntoDeck,
    SelectHiddenHandCards { count: usize },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DelayedTiming {
    NextTurnStart,
    NextPlayerTurn,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum DelayedEffect {
    RepeatMelodyMainEffect,
    SelectAndPerformMelodyMainEffect,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RuleException {
    IgnoresOtherFormationEffects,
    LimitedUse {
        key: String,
        remaining: u32,
        maximum: u32,
    },
}

pub(crate) fn formation_resolved_on_previous_turn<'a>(
    state: &'a crate::domain::GameState,
    player: &crate::domain::PlayerId,
) -> Option<&'a crate::domain::LastFormationUse> {
    let previous_turn = state.turn_number.checked_sub(1)?;
    state
        .last_formation_by_player
        .get(player)
        .filter(|formation| formation.resolved_turn == previous_turn)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormationDef {
    pub id: String,
    pub name: String,
    pub rule_text: String,
    pub category: FormationCategory,
    pub pattern: FormationPattern,
    pub effect_id: String,
    pub point_formula: PointFormula,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FormationCategory {
    Attack,
    Spell,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AttackCategory {
    Elemental(Element),
    Physical,
    Special,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum FormationPattern {
    ExactElements(Vec<Element>),
    GeneratingSequence { length: usize },
    OvercomingSequence { length: usize },
    Custom(String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SubmittedCardFacts {
    pub element: Element,
    pub level: EffectiveCardLevel,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormationCandidate {
    pub formation_id: String,
    pub formation_name: String,
    pub category: FormationCategory,
    pub cards: Vec<crate::domain::CardInstanceId>,
    pub star_substitution: Option<crate::domain::StarElementSubstitution>,
    pub declared_targets: Vec<crate::domain::TargetDecl>,
    pub preview: Option<String>,
    pub detail: PlayerFacingActionDetail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlayableAction {
    PerformFormation(FormationCandidate),
    ChangeProfession(ProfessionChangeCandidate),
    ActivateProfessionAbility(ProfessionAbilityCandidate),
    UseSpiritSkill(SpiritSkillCandidate),
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ActionInputRequirement {
    VirtualFormationCard {
        elements: Vec<Element>,
        levels: Vec<u32>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfessionChangeCandidate {
    pub profession_id: crate::domain::ProfessionId,
    pub profession_name: String,
    pub cards: Vec<crate::domain::CardInstanceId>,
    pub detail: PlayerFacingActionDetail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfessionAbilityCandidate {
    pub ability_id: String,
    pub ability_name: String,
    pub cards: Vec<crate::domain::CardInstanceId>,
    pub target_card: Option<crate::domain::CardInstanceId>,
    pub declared_element: Option<Element>,
    pub declared_level: Option<u32>,
    pub input_requirement: Option<ActionInputRequirement>,
    pub detail: PlayerFacingActionDetail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpiritSkillCandidate {
    pub skill: crate::domain::SpiritSkill,
    pub skill_name: String,
    pub selected_card: Option<crate::domain::CardInstanceId>,
    pub declared_level: Option<u32>,
    pub detail: PlayerFacingActionDetail,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum PointFormula {
    Fixed(u32),
    LevelPlus(u32),
    LevelSumTimes(u32),
    TargetHandCountTimes(u32),
    ElementProductTimes { element: Element, multiplier: u32 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct EffectDef {
    pub id: String,
    pub plan: EffectPlan,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum EffectPlan {
    Attack(AttackPlanDef),
    ActiveSpell(SpellPlanDef),
    PassiveSpell(SpellPlanDef),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AttackPlanDef {
    pub category: AttackCategory,
    pub point_formula: PointFormula,
    pub damage_target: DamageTarget,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SpellPlanDef {
    pub resolver_id: String,
    /// Required semantic action-detail fact.  This sits with the rule plan,
    /// not in a generic Formation-ID presentation fallback.
    pub player_facing_effect: FormationEffect,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum DamageTarget {
    PreviousPlayer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct FormationRegistry {
    formations: HashMap<String, FormationDef>,
    effects: HashMap<String, EffectDef>,
}

impl FormationRegistry {
    pub(crate) fn new(
        formations: Vec<FormationDef>,
        effects: Vec<EffectDef>,
    ) -> Result<Self, RegistryError> {
        let effects = effects
            .into_iter()
            .map(|effect| (effect.id.clone(), effect))
            .collect::<HashMap<_, _>>();

        let mut formation_map = HashMap::new();
        for formation in formations {
            if !effects.contains_key(&formation.effect_id) {
                return Err(RegistryError::MissingEffect {
                    formation_id: formation.id,
                    effect_id: formation.effect_id,
                });
            }

            if formation_map
                .insert(formation.id.clone(), formation)
                .is_some()
            {
                return Err(RegistryError::DuplicateFormation);
            }
        }

        Ok(Self {
            formations: formation_map,
            effects,
        })
    }

    pub(crate) fn formation(&self, id: &str) -> Option<&FormationDef> {
        self.formations.get(id)
    }

    pub(crate) fn effect(&self, id: &str) -> Option<&EffectDef> {
        self.effects.get(id)
    }

    pub(crate) fn effect_for(&self, formation: &FormationDef) -> Option<&EffectDef> {
        self.effect(&formation.effect_id)
    }

    pub(crate) fn formations(&self) -> Vec<&FormationDef> {
        let mut formations = self.formations.values().collect::<Vec<_>>();
        formations.sort_by(|left, right| left.id.cmp(&right.id));
        formations
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum RegistryError {
    DuplicateFormation,
    MissingEffect {
        formation_id: String,
        effect_id: String,
    },
}

#[derive(Default)]
pub(crate) struct FormationMatcher<'a> {
    custom_matchers: HashMap<String, CustomMatcher<'a>>,
}

type CustomMatcher<'a> = Box<dyn Fn(&[SubmittedCardFacts]) -> bool + 'a>;

impl<'a> FormationMatcher<'a> {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn with_custom(
        mut self,
        id: impl Into<String>,
        matcher: impl Fn(&[SubmittedCardFacts]) -> bool + 'a,
    ) -> Self {
        self.custom_matchers.insert(id.into(), Box::new(matcher));
        self
    }

    pub(crate) fn matches(
        &self,
        pattern: &FormationPattern,
        submitted: &[SubmittedCardFacts],
    ) -> bool {
        let submitted_elements = submitted_elements(submitted);
        match pattern {
            FormationPattern::ExactElements(elements) => {
                element_counts(elements) == element_counts(&submitted_elements)
            }
            FormationPattern::GeneratingSequence { length } => {
                matches_unordered_sequence(&submitted_elements, *length, &GENERATING_CYCLE)
            }
            FormationPattern::OvercomingSequence { length } => {
                matches_unordered_sequence(&submitted_elements, *length, &OVERCOMING_CYCLE)
            }
            FormationPattern::Custom(id) => self
                .custom_matchers
                .get(id)
                .is_some_and(|matcher| matcher(submitted)),
        }
    }
}

fn submitted_elements(submitted: &[SubmittedCardFacts]) -> Vec<Element> {
    submitted.iter().map(|card| card.element).collect()
}

fn element_counts(elements: &[Element]) -> HashMap<Element, usize> {
    let mut counts = HashMap::new();
    for element in elements {
        *counts.entry(*element).or_default() += 1;
    }
    counts
}

const GENERATING_CYCLE: [Element; 5] = [
    Element::Wood,
    Element::Fire,
    Element::Earth,
    Element::Metal,
    Element::Water,
];

const OVERCOMING_CYCLE: [Element; 5] = [
    Element::Wood,
    Element::Earth,
    Element::Water,
    Element::Fire,
    Element::Metal,
];

fn matches_unordered_sequence(submitted: &[Element], length: usize, cycle: &[Element; 5]) -> bool {
    if submitted.len() != length || length == 0 || length > cycle.len() {
        return false;
    }

    let submitted_counts = element_counts(submitted);

    (0..cycle.len()).any(|start| {
        let sequence = (0..length)
            .map(|offset| cycle[(start + offset) % cycle.len()])
            .collect::<Vec<_>>();
        element_counts(&sequence) == submitted_counts
    })
}

pub(crate) fn base_formation_registry() -> FormationRegistry {
    FormationRegistry::new(base_formations(), base_effects())
        .expect("base formation registry must be internally consistent")
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OfficialFormationSummary {
    pub(crate) id: String,
    pub(crate) name: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct OfficialFormationGroup {
    pub(crate) rule_module_id: Option<crate::domain::RuleModuleId>,
    pub(crate) formations: Vec<OfficialFormationSummary>,
}

fn official_formation_spec_groups(
    modules: &[crate::domain::RuleModuleId],
) -> Vec<(Option<crate::domain::RuleModuleId>, Vec<BaseFormationSpec>)> {
    let mut groups = vec![(None, base_specs())];
    for module in modules {
        let specs = match module.as_str() {
            crate::domain::FIVE_DIRECTIONS_LEGEND_MODULE_ID => Some(five_directions_legend_specs()),
            crate::domain::STAR_MODULE_ID => Some(star::specs()),
            crate::domain::HERO_SCHOOLS_MODULE_ID => Some(hero::formation_specs()),
            crate::domain::SPIRIT_MODULE_ID => Some(spirit::specs()),
            crate::domain::JIANGHU_MODULE_ID => Some(jianghu::formation_specs()),
            crate::domain::CONFLUENCE_GENERATION_MODULE_ID => Some(confluence::formation_specs()),
            crate::domain::DARK_GLIMMER_MODULE_ID => Some(dark::formation_specs()),
            crate::domain::ECHO_MODULE_ID => Some(echo::formation_specs()),
            crate::domain::TRIBULATION_MODULE_ID => Some(tribulation::formation_specs()),
            crate::domain::POUCH_MODULE_ID => Some(pouch::formation_specs()),
            _ => None,
        };
        if let Some(specs) = specs {
            groups.push((Some(module.clone()), specs));
        }
    }
    groups
}

pub(crate) fn official_formation_groups(
    modules: &[crate::domain::RuleModuleId],
) -> Vec<OfficialFormationGroup> {
    official_formation_spec_groups(modules)
        .into_iter()
        .map(|(rule_module_id, specs)| OfficialFormationGroup {
            rule_module_id,
            formations: specs
                .into_iter()
                .map(|spec| OfficialFormationSummary {
                    id: spec.formation.id,
                    name: spec.formation.name,
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn official_formation_registry(
    modules: &[crate::domain::RuleModuleId],
) -> FormationRegistry {
    let specs = official_formation_spec_groups(modules)
        .into_iter()
        .flat_map(|(_, specs)| specs)
        .collect::<Vec<_>>();
    FormationRegistry::new(
        specs.iter().map(|spec| spec.formation.clone()).collect(),
        specs.into_iter().map(|spec| spec.effect).collect(),
    )
    .expect("official formation registry must be internally consistent")
}

pub(crate) fn sacred_beast_element(formation_id: &str) -> Option<Element> {
    match formation_id {
        "east-azure-dragon" => Some(Element::Wood),
        "west-white-tiger" => Some(Element::Metal),
        "south-vermilion-bird" => Some(Element::Fire),
        "north-black-tortoise" => Some(Element::Water),
        "center-yellow-serpent" => Some(Element::Earth),
        _ => None,
    }
}

pub(crate) fn environment_makes_formation_ineffective(
    state: &crate::domain::GameState,
    formation_id: &str,
) -> bool {
    if !state.has_rule_module(crate::domain::FIVE_DIRECTIONS_LEGEND_MODULE_ID) {
        return false;
    }

    matches!(
        (state.environment, formation_id),
        (Some(Element::Metal), "defense" | "barrier")
            | (Some(Element::Metal), dark::DARK_BARRIER)
            | (Some(Element::Wood), "metamorphosis" | "chaos")
            | (Some(Element::Wood), dark::DARK_CHAOS)
            | (Some(Element::Water), "countershock" | "shock-burst")
            | (Some(Element::Water), dark::DARK_SHOCK_BURST)
            | (Some(Element::Fire), "weapon" | "radiance")
            | (Some(Element::Fire), dark::DARK_RADIANCE)
            | (Some(Element::Earth), "seal" | "return-to-origin")
            | (Some(Element::Earth), dark::DARK_RETURN_TO_ORIGIN)
    )
}

pub(crate) fn base_formation_matcher<'a>() -> FormationMatcher<'a> {
    FormationMatcher::new()
        .with_custom("two-different-elements", |submitted| {
            submitted.len() == 2 && submitted[0].element != submitted[1].element
        })
        .with_custom("five-same-level", |submitted| {
            let Some(first_card) = submitted.first() else {
                return false;
            };
            submitted.len() == 5 && submitted.iter().all(|card| card.level == first_card.level)
        })
        .with_custom("three-same-level", |submitted| {
            let Some(first_card) = submitted.first() else {
                return false;
            };
            submitted.len() == 3 && submitted.iter().all(|card| card.level == first_card.level)
        })
        .with_custom("echo:pure-fire", echo::matches_pure_fire)
        .with_custom("echo:plant-earth", |submitted| {
            submitted.len() == 2
                && submitted.iter().any(|card| card.element == Element::Earth)
                && submitted.iter().any(|card| card.element == Element::Wood)
                && submitted.iter().map(|card| card.level).sum::<u32>() >= 7
        })
        .with_custom("tribulation:thunder-fire", |submitted| {
            tribulation::matches_elements(submitted, Element::Metal, Element::Fire)
        })
        .with_custom("tribulation:gale-rain", |submitted| {
            tribulation::matches_elements(submitted, Element::Fire, Element::Water)
        })
        .with_custom("tribulation:mudslide-torrent", |submitted| {
            tribulation::matches_elements(submitted, Element::Water, Element::Earth)
        })
        .with_custom("tribulation:earth-rending", |submitted| {
            tribulation::matches_elements(submitted, Element::Earth, Element::Wood)
        })
        .with_custom("tribulation:rusted-forest", |submitted| {
            tribulation::matches_elements(submitted, Element::Wood, Element::Metal)
        })
        .with_custom("tribulation:divine-calculation", |submitted| {
            submitted.len() == 1 && submitted[0].level >= 4
        })
        .with_custom(pouch::CHAIN_ID, pouch::matches_chain)
        .with_custom("metal-and-same-level", |submitted| {
            submitted.len() == 2
                && submitted.iter().any(|card| card.element == Element::Metal)
                && submitted[0].level == submitted[1].level
        })
        .with_custom("wood-and-same-level", |submitted| {
            submitted.len() == 2
                && submitted.iter().any(|card| card.element == Element::Wood)
                && submitted[0].level == submitted[1].level
        })
        .with_custom("water-and-same-level", |submitted| {
            submitted.len() == 2
                && submitted.iter().any(|card| card.element == Element::Water)
                && submitted[0].level == submitted[1].level
        })
        .with_custom("fire-and-same-level", |submitted| {
            submitted.len() == 2
                && submitted.iter().any(|card| card.element == Element::Fire)
                && submitted[0].level == submitted[1].level
        })
        .with_custom("earth-and-same-level", |submitted| {
            submitted.len() == 2
                && submitted.iter().any(|card| card.element == Element::Earth)
                && submitted[0].level == submitted[1].level
        })
        .with_custom("reincarnation", |submitted| {
            submitted.len() == 4
                && submitted.iter().enumerate().any(|(wood_index, wood)| {
                    wood.element == Element::Wood
                        && matches_unordered_sequence(
                            &submitted
                                .iter()
                                .enumerate()
                                .filter(|(index, _)| *index != wood_index)
                                .map(|(_, card)| card.element)
                                .collect::<Vec<_>>(),
                            3,
                            &GENERATING_CYCLE,
                        )
                        && submitted
                            .iter()
                            .enumerate()
                            .filter(|(index, _)| *index != wood_index)
                            .map(|(_, card)| card.level)
                            .sum::<u32>()
                            >= 10
                })
        })
        .with_custom("water-water-three-any", |submitted| {
            submitted.len() == 5
                && submitted
                    .iter()
                    .filter(|card| card.element == Element::Water)
                    .count()
                    >= 2
        })
        .with_custom("fire-fire-two-non-fire", |submitted| {
            submitted.len() == 4
                && submitted
                    .iter()
                    .filter(|card| card.element == Element::Fire)
                    .count()
                    == 2
        })
        .with_custom("earth-sum-ten", |submitted| {
            !submitted.is_empty()
                && submitted.iter().all(|card| card.element == Element::Earth)
                && submitted.iter().map(|card| card.level).sum::<u32>() >= 10
        })
        .with_custom("three-level-five", |submitted| {
            submitted.len() == 3 && submitted.iter().all(|card| card.level == 5)
        })
        .with_custom("single-even-level", |submitted| {
            submitted.len() == 1 && submitted[0].level % 2 == 0
        })
        .with_custom("three-level-four", |submitted| {
            submitted.len() == 3 && submitted.iter().all(|card| card.level == 4)
        })
        .with_custom("three-different-levels", |submitted| {
            submitted.len() == 3
                && submitted
                    .iter()
                    .map(|card| card.level)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == 3
        })
        .with_custom("metal-metal-and-level-five", |submitted| {
            submitted.len() == 3
                && submitted
                    .iter()
                    .filter(|card| card.element == Element::Metal)
                    .count()
                    >= 2
                && submitted.iter().any(|card| card.level == 5)
        })
        .with_custom("water-water-and-level-five", |submitted| {
            submitted.len() == 3
                && submitted
                    .iter()
                    .filter(|card| card.element == Element::Water)
                    .count()
                    >= 2
                && submitted.iter().any(|card| card.level == 5)
        })
        .with_custom("levels-one-through-five", |submitted| {
            submitted.len() == 5
                && submitted
                    .iter()
                    .map(|card| card.level)
                    .collect::<std::collections::HashSet<_>>()
                    == std::collections::HashSet::from([
                        EffectiveCardLevel::new(1),
                        EffectiveCardLevel::new(2),
                        EffectiveCardLevel::new(3),
                        EffectiveCardLevel::new(4),
                        EffectiveCardLevel::new(5),
                    ])
        })
        .with_custom("single-level-one", |submitted| {
            submitted.len() == 1 && submitted[0].level == 1
        })
        .with_custom("single-level-three", |submitted| {
            submitted.len() == 1 && submitted[0].level == 3
        })
        .with_custom("single-level-five", |submitted| {
            submitted.len() == 1 && submitted[0].level == 5
        })
        .with_custom("three-different-elements-same-level", |submitted| {
            submitted.len() == 3
                && submitted
                    .iter()
                    .all(|card| card.level == submitted[0].level)
                && submitted
                    .iter()
                    .map(|card| card.element)
                    .collect::<std::collections::HashSet<_>>()
                    .len()
                    == 3
        })
        .with_custom("three-water", |submitted| {
            submitted.len() == 3 && submitted.iter().all(|card| card.element == Element::Water)
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct BaseFormationSpec {
    pub(crate) formation: FormationDef,
    pub(crate) effect: EffectDef,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SpellTiming {
    Active,
    Passive,
}

fn base_formations() -> Vec<FormationDef> {
    base_specs()
        .into_iter()
        .map(|spec| spec.formation)
        .collect()
}

fn base_effects() -> Vec<EffectDef> {
    base_specs().into_iter().map(|spec| spec.effect).collect()
}

fn base_formation_rule_text(id: &str) -> &'static str {
    match id {
        "metal-strike" => "金行攻擊，點數＝等級＋４",
        "wood-strike" => "木行攻擊，點數＝等級＋４",
        "water-strike" => "水行攻擊，點數＝等級＋４",
        "fire-strike" => "火行攻擊，點數＝等級＋４",
        "earth-strike" => "土行攻擊，點數＝等級＋４",
        "weapon" => "物理攻擊，點數＝等級總和×２",
        "defense" => "被動術式，反制：下家下回合攻擊之傷害無效",
        "seal" => "被動術式，反制：下家下回合術式無效",
        "countershock" => "被動術式，反制：下家下回合攻擊之傷害由目標與施展者平分",
        "metamorphosis" => "主動術式，複製上家上回合施展之基礎規則陣法之類別與效果",
        "empty-city" => "被動術式，無效果",
        "triple-metal" => "金行攻擊，點數＝等級總和×３",
        "triple-wood" => "木行攻擊，點數＝等級總和×３",
        "triple-water" => "水行攻擊，點數＝等級總和×３",
        "triple-fire" => "火行攻擊，點數＝等級總和×３",
        "triple-earth" => "土行攻擊，點數＝等級總和×３",
        "generating-formation" => "主動術式，回復生命值，點數＝等級總和×３",
        "overcoming-formation" => "主動術式，扣除下家防護罩，點數＝等級總和×３",
        "radiance" => "主動術式，檢視下家手牌，下家無法行動及抽牌２回合",
        "barrier" => "主動術式，建構防護罩，點數＝等級總和×４",
        "return-to-origin" => "主動術式，回復生命值，點數＝等級總和×４",
        "shock-burst" => "物理攻擊，點數＝等級總和×４",
        "chaos" => "主動術式，檢視下家手牌，將其中兩張放回牌堆最上方",
        "five-elements-cycle" => "主動術式，雙方生命值交換",
        "five-streams-unite" => "特殊攻擊，點數＝目標手牌數×１５，本回合抽牌＋１",
        _ => panic!("missing base formation rule text for {id}"),
    }
}

fn base_specs() -> Vec<BaseFormationSpec> {
    vec![
        elemental_attack(
            "metal-strike",
            "金擊術",
            Element::Metal,
            1,
            PointFormula::LevelPlus(4),
        ),
        elemental_attack(
            "wood-strike",
            "木擊術",
            Element::Wood,
            1,
            PointFormula::LevelPlus(4),
        ),
        elemental_attack(
            "water-strike",
            "水擊術",
            Element::Water,
            1,
            PointFormula::LevelPlus(4),
        ),
        elemental_attack(
            "fire-strike",
            "火擊術",
            Element::Fire,
            1,
            PointFormula::LevelPlus(4),
        ),
        elemental_attack(
            "earth-strike",
            "土擊術",
            Element::Earth,
            1,
            PointFormula::LevelPlus(4),
        ),
        attack(
            "weapon",
            "武器",
            AttackCategory::Physical,
            FormationPattern::ExactElements(vec![Element::Metal, Element::Metal]),
            PointFormula::LevelSumTimes(2),
        ),
        spell(
            "defense",
            "防禦",
            SpellTiming::Passive,
            FormationPattern::ExactElements(vec![Element::Wood, Element::Wood]),
        ),
        spell(
            "seal",
            "封印",
            SpellTiming::Passive,
            FormationPattern::ExactElements(vec![Element::Water, Element::Water]),
        ),
        spell(
            "countershock",
            "反震",
            SpellTiming::Passive,
            FormationPattern::ExactElements(vec![Element::Fire, Element::Fire]),
        ),
        spell(
            "metamorphosis",
            "幻化",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![Element::Earth, Element::Earth]),
        ),
        spell(
            "empty-city",
            "空城",
            SpellTiming::Passive,
            FormationPattern::Custom("two-different-elements".to_string()),
        ),
        elemental_attack(
            "triple-metal",
            "鍠金",
            Element::Metal,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        elemental_attack(
            "triple-wood",
            "樸木",
            Element::Wood,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        elemental_attack(
            "triple-water",
            "洄水",
            Element::Water,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        elemental_attack(
            "triple-fire",
            "熾火",
            Element::Fire,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        elemental_attack(
            "triple-earth",
            "坱土",
            Element::Earth,
            3,
            PointFormula::LevelSumTimes(3),
        ),
        spell(
            "generating-formation",
            "生陣",
            SpellTiming::Active,
            FormationPattern::GeneratingSequence { length: 3 },
        ),
        spell(
            "overcoming-formation",
            "剋陣",
            SpellTiming::Active,
            FormationPattern::OvercomingSequence { length: 3 },
        ),
        spell(
            "radiance",
            "光芒",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Metal,
                Element::Metal,
                Element::Fire,
                Element::Water,
            ]),
        ),
        spell(
            "barrier",
            "氣壁",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Wood,
                Element::Wood,
                Element::Metal,
                Element::Fire,
            ]),
        ),
        spell(
            "return-to-origin",
            "歸元",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Water,
                Element::Water,
                Element::Earth,
                Element::Wood,
            ]),
        ),
        attack(
            "shock-burst",
            "震暴",
            AttackCategory::Physical,
            FormationPattern::ExactElements(vec![
                Element::Fire,
                Element::Fire,
                Element::Water,
                Element::Earth,
            ]),
            PointFormula::LevelSumTimes(4),
        ),
        spell(
            "chaos",
            "混沌",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Earth,
                Element::Earth,
                Element::Wood,
                Element::Metal,
            ]),
        ),
        spell(
            "five-elements-cycle",
            "五行輪迴",
            SpellTiming::Active,
            FormationPattern::ExactElements(vec![
                Element::Metal,
                Element::Wood,
                Element::Water,
                Element::Fire,
                Element::Earth,
            ]),
        ),
        attack(
            "five-streams-unite",
            "五流歸一",
            AttackCategory::Special,
            FormationPattern::Custom("five-same-level".to_string()),
            PointFormula::TargetHandCountTimes(15),
        ),
    ]
}

fn five_directions_legend_specs() -> Vec<BaseFormationSpec> {
    vec![
        sacred_beast("east-azure-dragon", "東‧青龍", Element::Wood),
        sacred_beast("west-white-tiger", "西‧白虎", Element::Metal),
        sacred_beast("south-vermilion-bird", "南‧朱雀", Element::Fire),
        sacred_beast("north-black-tortoise", "北‧玄武", Element::Water),
        sacred_beast("center-yellow-serpent", "中‧黃蛇", Element::Earth),
        BaseFormationSpec {
            formation: FormationDef {
                id: "void-meridian-severing".to_string(),
                name: "虛空斷脈術".to_string(),
                rule_text: "主動術式，破除環境，環境被破除時雙方扣除２０點生命".to_string(),
                category: FormationCategory::Spell,
                pattern: FormationPattern::Custom("three-same-level".to_string()),
                effect_id: "void-meridian-severing".to_string(),
                point_formula: PointFormula::Fixed(0),
            },
            effect: EffectDef {
                id: "void-meridian-severing".to_string(),
                plan: EffectPlan::ActiveSpell(SpellPlanDef {
                    resolver_id: "void-meridian-severing".to_string(),
                    player_facing_effect: FormationEffect::ClearEnvironment,
                }),
            },
        },
    ]
}

fn sacred_beast(id: &str, name: &str, element: Element) -> BaseFormationSpec {
    let formation = FormationDef {
        id: id.to_string(),
        name: name.to_string(),
        rule_text: format!(
            "{}行攻擊，點數＝８１，傷害後轉移環境，本陣法不受其他陣法效果影響",
            match element {
                Element::Metal => "金",
                Element::Wood => "木",
                Element::Water => "水",
                Element::Fire => "火",
                Element::Earth => "土",
            }
        ),
        category: FormationCategory::Attack,
        pattern: FormationPattern::ExactElements(vec![element; 5]),
        effect_id: id.to_string(),
        point_formula: PointFormula::Fixed(81),
    };
    let effect = EffectDef {
        id: id.to_string(),
        plan: EffectPlan::Attack(AttackPlanDef {
            category: AttackCategory::Elemental(element),
            point_formula: PointFormula::Fixed(81),
            damage_target: DamageTarget::PreviousPlayer,
        }),
    };

    BaseFormationSpec { formation, effect }
}

fn elemental_attack(
    id: &str,
    name: &str,
    element: Element,
    count: usize,
    point_formula: PointFormula,
) -> BaseFormationSpec {
    attack(
        id,
        name,
        AttackCategory::Elemental(element),
        FormationPattern::ExactElements(vec![element; count]),
        point_formula,
    )
}

fn attack(
    id: &str,
    name: &str,
    category: AttackCategory,
    pattern: FormationPattern,
    point_formula: PointFormula,
) -> BaseFormationSpec {
    let formation = FormationDef {
        id: id.to_string(),
        name: name.to_string(),
        rule_text: base_formation_rule_text(id).to_string(),
        category: FormationCategory::Attack,
        pattern,
        effect_id: id.to_string(),
        point_formula: point_formula.clone(),
    };
    let effect = EffectDef {
        id: id.to_string(),
        plan: EffectPlan::Attack(AttackPlanDef {
            category,
            point_formula,
            damage_target: DamageTarget::PreviousPlayer,
        }),
    };

    BaseFormationSpec { formation, effect }
}

fn spell(
    id: &str,
    name: &str,
    timing: SpellTiming,
    pattern: FormationPattern,
) -> BaseFormationSpec {
    let formation = FormationDef {
        id: id.to_string(),
        name: name.to_string(),
        rule_text: base_formation_rule_text(id).to_string(),
        category: FormationCategory::Spell,
        pattern,
        effect_id: id.to_string(),
        point_formula: PointFormula::Fixed(0),
    };
    let plan = match timing {
        SpellTiming::Active => EffectPlan::ActiveSpell(SpellPlanDef {
            resolver_id: id.to_string(),
            player_facing_effect: base_spell_player_facing_effect(id),
        }),
        SpellTiming::Passive => EffectPlan::PassiveSpell(SpellPlanDef {
            resolver_id: id.to_string(),
            player_facing_effect: base_spell_player_facing_effect(id),
        }),
    };
    let effect = EffectDef {
        id: id.to_string(),
        plan,
    };

    BaseFormationSpec { formation, effect }
}

fn base_spell_player_facing_effect(id: &str) -> FormationEffect {
    match id {
        "defense" | "seal" | "countershock" | "empty-city" => FormationEffect::CoverCounter,
        "metamorphosis" => FormationEffect::CopyPreviousTurnFormation,
        "generating-formation" | "return-to-origin" | "reincarnation" => FormationEffect::RecoverHp,
        "overcoming-formation" => FormationEffect::ReduceShield,
        "radiance" | "chaos" => FormationEffect::InspectHand,
        "barrier" | "purple-light-shield" => FormationEffect::CreateShield,
        "five-elements-cycle" => FormationEffect::ReturnTeamHp,
        "shadow-assault" | "instant-shadow-death" | "holy-wind" => FormationEffect::ApplyStatus,
        _ => panic!("base spell `{id}` is missing a player-facing effect fact"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn card(element: Element) -> SubmittedCardFacts {
        leveled_card(element, 1)
    }

    fn leveled_card(element: Element, level: u32) -> SubmittedCardFacts {
        SubmittedCardFacts {
            element,
            level: EffectiveCardLevel::new(level),
        }
    }

    #[test]
    fn official_formation_groups_include_only_rules_that_provide_formations() {
        let modules = OfficialRules::new().default_rule_modules();
        let groups = official_formation_groups(&modules);

        assert_eq!(
            groups
                .iter()
                .map(|group| group.rule_module_id.as_ref().map(|module| module.as_str()))
                .collect::<Vec<_>>(),
            vec![
                None,
                Some("five-directions-legend"),
                Some("star"),
                Some("hero-schools"),
                Some("spirit"),
                Some("jianghu"),
                Some("confluence-generation"),
                Some("dark-glimmer"),
                Some("echo"),
                Some("tribulation"),
                Some("pouch"),
            ]
        );
        assert_eq!(groups[0].formations[0].id, "metal-strike");
        assert_eq!(groups[0].formations[0].name, "金擊術");
        assert!(groups[8].formations.iter().any(|formation| {
            formation.id == "echo:split-earth" && formation.name == "宮調‧裂土"
        }));

        let grouped_ids = groups
            .iter()
            .flat_map(|group| {
                group
                    .formations
                    .iter()
                    .map(|formation| formation.id.clone())
            })
            .collect::<Vec<_>>();
        let mut registry_ids = official_formation_registry(&modules)
            .formations()
            .into_iter()
            .map(|formation| formation.id.clone())
            .collect::<Vec<_>>();
        registry_ids.sort();
        let mut sorted_grouped_ids = grouped_ids.clone();
        sorted_grouped_ids.sort();

        assert_eq!(sorted_grouped_ids, registry_ids);
        sorted_grouped_ids.dedup();
        assert_eq!(sorted_grouped_ids.len(), grouped_ids.len());
    }

    #[test]
    fn official_formation_groups_follow_the_enabled_rule_module_order() {
        let groups = official_formation_groups(&[
            crate::domain::RuleModuleId::new(crate::domain::ECHO_MODULE_ID),
            crate::domain::RuleModuleId::new(crate::domain::DISCARD_RETRIEVAL_MODULE_ID),
            crate::domain::RuleModuleId::new(crate::domain::FIVE_DIRECTIONS_LEGEND_MODULE_ID),
        ]);

        assert_eq!(
            groups
                .iter()
                .map(|group| group.rule_module_id.as_ref().map(|module| module.as_str()))
                .collect::<Vec<_>>(),
            vec![None, Some("echo"), Some("five-directions-legend")]
        );
    }

    #[test]
    fn matcher_handles_unordered_exact_counts_and_sequences() {
        let matcher = FormationMatcher::default();

        assert!(matcher.matches(
            &FormationPattern::ExactElements(vec![
                Element::Metal,
                Element::Metal,
                Element::Fire,
                Element::Water,
            ]),
            &[
                card(Element::Water),
                card(Element::Metal),
                card(Element::Fire),
                card(Element::Metal),
            ],
        ));
        assert!(matcher.matches(
            &FormationPattern::GeneratingSequence { length: 3 },
            &[
                card(Element::Earth),
                card(Element::Wood),
                card(Element::Fire),
            ],
        ));
        assert!(matcher.matches(
            &FormationPattern::OvercomingSequence { length: 3 },
            &[
                card(Element::Water),
                card(Element::Wood),
                card(Element::Earth),
            ],
        ));
    }

    #[test]
    fn matcher_rejects_incomplete_or_wrong_patterns() {
        let matcher = FormationMatcher::default();

        assert!(!matcher.matches(
            &FormationPattern::ExactElements(vec![Element::Metal, Element::Fire, Element::Water,]),
            &[card(Element::Metal), card(Element::Fire)],
        ));
        assert!(!matcher.matches(
            &FormationPattern::GeneratingSequence { length: 3 },
            &[
                card(Element::Metal),
                card(Element::Fire),
                card(Element::Water),
            ],
        ));
        assert!(!matcher.matches(
            &FormationPattern::OvercomingSequence { length: 3 },
            &[
                card(Element::Wood),
                card(Element::Fire),
                card(Element::Earth),
            ],
        ));
    }

    #[test]
    fn matcher_keeps_custom_hooks_internal() {
        let matcher = FormationMatcher::new().with_custom("all-water", |submitted| {
            submitted.len() == 3
                && submitted
                    .iter()
                    .all(|card| card.element == Element::Water && card.level > 1)
        });
        let pattern = FormationPattern::Custom("all-water".to_string());

        assert!(matcher.matches(
            &pattern,
            &[
                leveled_card(Element::Water, 2),
                leveled_card(Element::Water, 3),
                leveled_card(Element::Water, 4),
            ],
        ));
        assert!(!matcher.matches(
            &pattern,
            &[
                leveled_card(Element::Water, 2),
                leveled_card(Element::Water, 1),
                leveled_card(Element::Water, 4),
            ],
        ));
    }

    #[test]
    fn registry_links_formation_schema_to_effect_plan() {
        let registry = FormationRegistry::new(
            vec![FormationDef {
                id: "metal-strike".to_string(),
                name: "Metal Strike".to_string(),
                rule_text: "Elemental attack".to_string(),
                category: FormationCategory::Attack,
                pattern: FormationPattern::ExactElements(vec![Element::Metal]),
                effect_id: "deal-formation-damage".to_string(),
                point_formula: PointFormula::Fixed(3),
            }],
            vec![EffectDef {
                id: "deal-formation-damage".to_string(),
                plan: EffectPlan::Attack(AttackPlanDef {
                    category: AttackCategory::Elemental(Element::Metal),
                    point_formula: PointFormula::LevelSumTimes(1),
                    damage_target: DamageTarget::PreviousPlayer,
                }),
            }],
        )
        .unwrap();

        let formation = registry.formation("metal-strike").unwrap();
        let effect = registry.effect_for(formation).unwrap();

        assert_eq!(formation.effect_id, "deal-formation-damage");
        assert_eq!(effect.id, "deal-formation-damage");
        assert!(matches!(effect.plan, EffectPlan::Attack(_)));
    }

    #[test]
    fn base_registry_contains_every_documented_formation() {
        let registry = base_formation_registry();
        let expected = [
            ("metal-strike", "金擊術"),
            ("wood-strike", "木擊術"),
            ("water-strike", "水擊術"),
            ("fire-strike", "火擊術"),
            ("earth-strike", "土擊術"),
            ("weapon", "武器"),
            ("defense", "防禦"),
            ("seal", "封印"),
            ("countershock", "反震"),
            ("metamorphosis", "幻化"),
            ("empty-city", "空城"),
            ("triple-metal", "鍠金"),
            ("triple-wood", "樸木"),
            ("triple-water", "洄水"),
            ("triple-fire", "熾火"),
            ("triple-earth", "坱土"),
            ("generating-formation", "生陣"),
            ("overcoming-formation", "剋陣"),
            ("radiance", "光芒"),
            ("barrier", "氣壁"),
            ("return-to-origin", "歸元"),
            ("shock-burst", "震暴"),
            ("chaos", "混沌"),
            ("five-elements-cycle", "五行輪迴"),
            ("five-streams-unite", "五流歸一"),
        ];

        assert_eq!(registry.formations().len(), expected.len());
        assert_eq!(registry.effects.len(), expected.len());
        for (id, name) in expected {
            let formation = registry.formation(id).expect("base formation must exist");
            assert_eq!(formation.name, name);
            assert!(!formation.rule_text.is_empty());
            assert!(registry.effect_for(formation).is_some());
        }
    }

    #[test]
    fn base_registry_patterns_and_effect_plans_are_consistent() {
        let registry = base_formation_registry();
        let matcher = base_formation_matcher();

        assert!(matcher.matches(
            &registry.formation("empty-city").unwrap().pattern,
            &[card(Element::Metal), card(Element::Wood)],
        ));
        assert!(!matcher.matches(
            &registry.formation("empty-city").unwrap().pattern,
            &[card(Element::Metal), card(Element::Metal)],
        ));
        assert!(matcher.matches(
            &registry.formation("five-streams-unite").unwrap().pattern,
            &[
                leveled_card(Element::Metal, 3),
                leveled_card(Element::Wood, 3),
                leveled_card(Element::Water, 3),
                leveled_card(Element::Fire, 3),
                leveled_card(Element::Earth, 3),
            ],
        ));
        assert_eq!(
            registry.effect("triple-fire").unwrap().plan,
            EffectPlan::Attack(AttackPlanDef {
                category: AttackCategory::Elemental(Element::Fire),
                point_formula: PointFormula::LevelSumTimes(3),
                damage_target: DamageTarget::PreviousPlayer,
            }),
        );
        assert_eq!(
            registry.effect("barrier").unwrap().plan,
            EffectPlan::ActiveSpell(SpellPlanDef {
                resolver_id: "barrier".to_string(),
                player_facing_effect: FormationEffect::CreateShield,
            }),
        );
    }
}
