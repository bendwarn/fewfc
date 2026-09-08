use crate::domain::{
    CONFLUENCE_GENERATION_MODULE_ID, CardInstanceId, Command, DARK_GLIMMER_MODULE_ID,
    DISCARD_RETRIEVAL_MODULE_ID, ECHO_MODULE_ID, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError,
    GameEvent, GameResult, GameSetup, GameState, HERO_SCHOOLS_MODULE_ID, JIANGHU_MODULE_ID,
    PERSONAL_DECK_MODULE_ID, POUCH_MODULE_ID, Player, PlayerDeckList, PlayerId, RuleModuleId,
    RulesetId, SPIRIT_MODULE_ID, STAR_MODULE_ID, TRIBULATION_MODULE_ID, ValidationError,
    validate_setup,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

use super::{
    DeckCompositionCatalog, PlayableAction, ResolvedPersonalDeck, base::BaseRuleset,
    official_formation_registry,
};

const ADVANCED_RULE_MODULE_IDS: &[&str] = &[
    STAR_MODULE_ID,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID,
    HERO_SCHOOLS_MODULE_ID,
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct RuleModuleSpec {
    id: &'static str,
    category: OfficialRuleModuleCategory,
    default_enabled: bool,
    dependencies: &'static [&'static str],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum OfficialRuleModuleCategory {
    Advanced,
    Optional,
    Theme,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct OfficialRuleModuleSpec {
    pub id: RuleModuleId,
    pub category: OfficialRuleModuleCategory,
    pub default_enabled: bool,
    pub dependencies: Vec<RuleModuleId>,
}

const OFFICIAL_RULE_MODULES: &[RuleModuleSpec] = &[
    RuleModuleSpec {
        id: crate::domain::TOTEM_FORMATION_MODULE_ID,
        category: OfficialRuleModuleCategory::Theme,
        default_enabled: true,
        dependencies: ADVANCED_RULE_MODULE_IDS,
    },
    RuleModuleSpec {
        id: DISCARD_RETRIEVAL_MODULE_ID,
        category: OfficialRuleModuleCategory::Optional,
        default_enabled: true,
        dependencies: &[],
    },
    RuleModuleSpec {
        id: PERSONAL_DECK_MODULE_ID,
        category: OfficialRuleModuleCategory::Optional,
        default_enabled: true,
        dependencies: &[],
    },
    RuleModuleSpec {
        id: FIVE_DIRECTIONS_LEGEND_MODULE_ID,
        category: OfficialRuleModuleCategory::Advanced,
        default_enabled: true,
        dependencies: &[],
    },
    RuleModuleSpec {
        id: STAR_MODULE_ID,
        category: OfficialRuleModuleCategory::Advanced,
        default_enabled: true,
        dependencies: &[],
    },
    RuleModuleSpec {
        id: HERO_SCHOOLS_MODULE_ID,
        category: OfficialRuleModuleCategory::Advanced,
        default_enabled: true,
        dependencies: &[],
    },
    RuleModuleSpec {
        id: SPIRIT_MODULE_ID,
        category: OfficialRuleModuleCategory::Theme,
        default_enabled: true,
        dependencies: ADVANCED_RULE_MODULE_IDS,
    },
    RuleModuleSpec {
        id: JIANGHU_MODULE_ID,
        category: OfficialRuleModuleCategory::Theme,
        default_enabled: true,
        dependencies: ADVANCED_RULE_MODULE_IDS,
    },
    RuleModuleSpec {
        id: CONFLUENCE_GENERATION_MODULE_ID,
        category: OfficialRuleModuleCategory::Theme,
        default_enabled: true,
        dependencies: ADVANCED_RULE_MODULE_IDS,
    },
    RuleModuleSpec {
        id: DARK_GLIMMER_MODULE_ID,
        category: OfficialRuleModuleCategory::Theme,
        default_enabled: true,
        dependencies: &[SPIRIT_MODULE_ID],
    },
    RuleModuleSpec {
        id: ECHO_MODULE_ID,
        category: OfficialRuleModuleCategory::Theme,
        default_enabled: true,
        dependencies: ADVANCED_RULE_MODULE_IDS,
    },
    RuleModuleSpec {
        id: TRIBULATION_MODULE_ID,
        category: OfficialRuleModuleCategory::Theme,
        default_enabled: true,
        dependencies: ADVANCED_RULE_MODULE_IDS,
    },
    RuleModuleSpec {
        id: POUCH_MODULE_ID,
        category: OfficialRuleModuleCategory::Theme,
        default_enabled: true,
        dependencies: &[PERSONAL_DECK_MODULE_ID, SPIRIT_MODULE_ID],
    },
];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct OfficialRules;

impl OfficialRules {
    pub fn new() -> Self {
        Self
    }

    pub fn configure_game(
        &self,
        players: Vec<Player>,
        turn_order: Vec<PlayerId>,
        enabled_rule_modules: Vec<RuleModuleId>,
    ) -> GameResult<GameSetup> {
        self.configure_game_with_decks(players, turn_order, enabled_rule_modules, Vec::new())
    }

    pub fn configure_game_with_decks(
        &self,
        players: Vec<Player>,
        turn_order: Vec<PlayerId>,
        enabled_rule_modules: Vec<RuleModuleId>,
        deck_lists: Vec<PlayerDeckList>,
    ) -> GameResult<GameSetup> {
        self.configure_versioned_game_with_decks(
            crate::domain::RuleVersion::default(),
            players,
            turn_order,
            enabled_rule_modules,
            deck_lists,
        )
    }

    pub fn configure_versioned_game_with_decks(
        &self,
        rule_version: crate::domain::RuleVersion,
        players: Vec<Player>,
        turn_order: Vec<PlayerId>,
        enabled_rule_modules: Vec<RuleModuleId>,
        deck_lists: Vec<PlayerDeckList>,
    ) -> GameResult<GameSetup> {
        self.resolve_version_modules(rule_version, Some(enabled_rule_modules.clone()))?;
        let mut setup = BaseRuleset::new().official_game_setup(players, turn_order);
        setup.rule_version = rule_version;
        setup.enabled_rule_modules = enabled_rule_modules;
        let uses_advanced_rules = setup.has_rule_module(FIVE_DIRECTIONS_LEGEND_MODULE_ID)
            || setup.has_rule_module(STAR_MODULE_ID)
            || setup.has_rule_module(HERO_SCHOOLS_MODULE_ID);
        let official_hp = match (setup.players.len(), uses_advanced_rules) {
            (2, false) => 100,
            (2, true) => 200,
            (_, false) => 150,
            (_, true) => 250,
        };
        for team_hp in &mut setup.hp {
            team_hp.hp = official_hp;
        }
        if setup.has_rule_module(PERSONAL_DECK_MODULE_ID) {
            BaseRuleset::new().configure_personal_decks(&mut setup, deck_lists);
        }
        self.validate_setup(&setup)?;
        Ok(setup)
    }

    pub fn default_rule_modules(&self) -> Vec<RuleModuleId> {
        OFFICIAL_RULE_MODULES
            .iter()
            .filter(|module| module.default_enabled)
            .map(|module| RuleModuleId::new(module.id))
            .filter(|module| crate::domain::RuleVersion::default().allows_module(module))
            .collect()
    }

    pub fn rule_module_catalog(&self) -> Vec<OfficialRuleModuleSpec> {
        OFFICIAL_RULE_MODULES
            .iter()
            .map(|module| OfficialRuleModuleSpec {
                id: RuleModuleId::new(module.id),
                category: module.category,
                default_enabled: module.default_enabled,
                dependencies: module
                    .dependencies
                    .iter()
                    .map(|dependency| RuleModuleId::new(*dependency))
                    .collect(),
            })
            .collect()
    }

    pub fn resolve_rule_modules(
        &self,
        candidate: Option<Vec<RuleModuleId>>,
    ) -> GameResult<Vec<RuleModuleId>> {
        let modules = candidate.unwrap_or_else(|| self.default_rule_modules());
        self.validate_modules(&modules)?;
        Ok(modules)
    }

    pub fn resolve_version_modules(
        &self,
        version: crate::domain::RuleVersion,
        candidate: Option<Vec<RuleModuleId>>,
    ) -> GameResult<Vec<RuleModuleId>> {
        let modules = candidate.unwrap_or_else(|| {
            OFFICIAL_RULE_MODULES
                .iter()
                .filter(|module| module.default_enabled)
                .map(|module| RuleModuleId::new(module.id))
                .filter(|module| version.allows_module(module))
                .collect()
        });
        for module in &modules {
            if !version.allows_module(module) {
                return Err(GameError::Validation(ValidationError::UnknownRuleModule(
                    module.clone(),
                )));
            }
        }
        self.validate_modules(&modules)?;
        Ok(modules)
    }

    pub fn preconstructed_deck(&self, player: PlayerId) -> PlayerDeckList {
        BaseRuleset::new().preconstructed_deck(player)
    }

    pub fn deck_composition_catalog(&self) -> DeckCompositionCatalog {
        super::base::deck_composition::DeckComposition.catalog()
    }

    pub fn resolve_personal_deck(
        &self,
        player: PlayerId,
        candidate: Option<PlayerDeckList>,
    ) -> ResolvedPersonalDeck {
        super::base::deck_composition::DeckComposition.resolve(player, candidate)
    }

    pub fn start_game(
        &self,
        setup: &GameSetup,
        deck_order: Vec<CardInstanceId>,
    ) -> GameResult<Vec<GameEvent>> {
        self.validate_setup(setup)?;
        BaseRuleset::new().start_game(setup, deck_order)
    }

    pub fn decide_command(
        &self,
        state: &GameState,
        command: Command,
    ) -> GameResult<Vec<GameEvent>> {
        self.validate_state(state)?;
        BaseRuleset::new().decide_command(state, command)
    }

    pub fn advance_automatic(&self, state: &GameState) -> GameResult<Vec<GameEvent>> {
        self.validate_state(state)?;
        BaseRuleset::new().advance_automatic(state)
    }

    pub fn playable_actions(
        &self,
        state: &GameState,
        player: &PlayerId,
        selected_cards: &[CardInstanceId],
    ) -> GameResult<Vec<PlayableAction>> {
        self.validate_state(state)?;
        BaseRuleset::new().playable_actions(state, player, selected_cards)
    }

    pub fn official_deck_order(&self, setup: &GameSetup) -> GameResult<Vec<CardInstanceId>> {
        self.validate_setup(setup)?;
        Ok(BaseRuleset::new().official_deck_order(setup))
    }

    pub fn card_labels(&self, setup: &GameSetup) -> GameResult<HashMap<CardInstanceId, String>> {
        self.validate_setup(setup)?;
        Ok(BaseRuleset::new().card_labels(setup))
    }

    pub fn formation_names(&self, setup: &GameSetup) -> GameResult<HashMap<String, String>> {
        self.validate_setup(setup)?;
        Ok(official_formation_registry(&setup.enabled_rule_modules)
            .formations()
            .into_iter()
            .map(|formation| (formation.id.clone(), formation.name.clone()))
            .collect())
    }

    pub fn validate_setup(&self, setup: &GameSetup) -> GameResult<()> {
        validate_setup(setup)?;
        self.validate_ruleset(&setup.ruleset)?;
        self.resolve_version_modules(setup.rule_version, Some(setup.enabled_rule_modules.clone()))
            .map(|_| ())
    }

    fn validate_state(&self, state: &GameState) -> GameResult<()> {
        self.validate_ruleset(&state.ruleset)?;
        self.resolve_version_modules(state.rule_version, Some(state.enabled_rule_modules.clone()))
            .map(|_| ())
    }

    fn validate_ruleset(&self, ruleset: &RulesetId) -> GameResult<()> {
        if ruleset == &RulesetId::base() {
            Ok(())
        } else {
            Err(GameError::Validation(ValidationError::UnsupportedRuleset(
                ruleset.clone(),
            )))
        }
    }

    fn validate_modules(&self, modules: &[RuleModuleId]) -> GameResult<()> {
        let mut seen = HashSet::new();
        for module in modules {
            if !seen.insert(module) {
                return Err(GameError::Validation(ValidationError::DuplicateRuleModule(
                    module.clone(),
                )));
            }

            if !OFFICIAL_RULE_MODULES
                .iter()
                .any(|known| known.id == module.as_str())
            {
                return Err(GameError::Validation(ValidationError::UnknownRuleModule(
                    module.clone(),
                )));
            }
        }

        for enabled in modules {
            let spec = OFFICIAL_RULE_MODULES
                .iter()
                .find(|known| known.id == enabled.as_str())
                .expect("unknown Rule Modules returned above");
            let required = spec
                .dependencies
                .iter()
                .filter(|required| !modules.iter().any(|module| module.as_str() == **required))
                .map(|required| RuleModuleId::new(*required))
                .collect::<Vec<_>>();
            if !required.is_empty() {
                return Err(GameError::Validation(
                    ValidationError::MissingRuleModuleDependencies {
                        module: enabled.clone(),
                        required,
                    },
                ));
            }
        }

        Ok(())
    }
}
