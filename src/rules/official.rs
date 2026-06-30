use crate::domain::{
    CardInstanceId, Command, DISCARD_RETRIEVAL_MODULE_ID, GameError, GameEvent, GameResult,
    GameSetup, GameState, PERSONAL_DECK_MODULE_ID, Player, PlayerDeckList, PlayerId, RuleModuleId,
    RulesetId, ValidationError, validate_setup,
};
use std::collections::{HashMap, HashSet};

use super::{FormationCandidate, base::BaseRuleset, base_formation_registry};

const OFFICIAL_RULE_MODULE_IDS: &[&str] = &[DISCARD_RETRIEVAL_MODULE_ID, PERSONAL_DECK_MODULE_ID];

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
        self.validate_modules(&enabled_rule_modules)?;
        let mut setup = BaseRuleset::new().official_game_setup(players, turn_order);
        setup.enabled_rule_modules = enabled_rule_modules;
        if setup.has_rule_module(PERSONAL_DECK_MODULE_ID) {
            BaseRuleset::new().configure_personal_decks(&mut setup, deck_lists);
        }
        self.validate_setup(&setup)?;
        Ok(setup)
    }

    pub fn default_rule_modules(&self) -> Vec<RuleModuleId> {
        OFFICIAL_RULE_MODULE_IDS
            .iter()
            .map(|id| RuleModuleId::new(*id))
            .collect()
    }

    pub fn preconstructed_deck(&self, player: PlayerId) -> PlayerDeckList {
        BaseRuleset::new().preconstructed_deck(player)
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

    pub fn playable_formations(
        &self,
        state: &GameState,
        player: &PlayerId,
        selected_cards: &[CardInstanceId],
    ) -> GameResult<Vec<FormationCandidate>> {
        self.validate_state(state)?;
        BaseRuleset::new().playable_formations(state, player, selected_cards)
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
        Ok(base_formation_registry()
            .formations()
            .into_iter()
            .map(|formation| (formation.id.clone(), formation.name.clone()))
            .collect())
    }

    pub fn validate_setup(&self, setup: &GameSetup) -> GameResult<()> {
        validate_setup(setup)?;
        self.validate_ruleset(&setup.ruleset)?;
        self.validate_modules(&setup.enabled_rule_modules)
    }

    fn validate_state(&self, state: &GameState) -> GameResult<()> {
        self.validate_ruleset(&state.ruleset)?;
        self.validate_modules(&state.enabled_rule_modules)
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

            if !OFFICIAL_RULE_MODULE_IDS.contains(&module.as_str()) {
                return Err(GameError::Validation(ValidationError::UnknownRuleModule(
                    module.clone(),
                )));
            }
        }

        Ok(())
    }
}
