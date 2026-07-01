use crate::domain::{CardInstanceId, GameError, GameResult, GameState, PlayerId, ValidationError};
use crate::rules::{
    EffectPlan, FormationCandidate, FormationDef, FormationRegistry, SubmittedCardFacts,
    base_formation_matcher, base_formation_registry, official_formation_registry, star,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FormationSelection {
    cards: Vec<CardInstanceId>,
    facts: Vec<SubmittedCardFacts>,
    registry: FormationRegistry,
    team_star: Option<crate::domain::StarKind>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SelectedFormation {
    pub(super) formation_id: String,
    pub(super) cards: Vec<CardInstanceId>,
    pub(super) effect_plan: EffectPlan,
}

impl FormationSelection {
    pub(super) fn new(
        state: &GameState,
        player: &PlayerId,
        selected_cards: Vec<CardInstanceId>,
    ) -> GameResult<Self> {
        let hand = state
            .hand(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
        let mut seen = HashSet::new();
        let facts = selected_cards
            .iter()
            .map(|card| {
                if !seen.insert(*card) {
                    return Err(GameError::Validation(
                        ValidationError::DuplicateSubmittedCard(*card),
                    ));
                }

                if !hand.contains(card) {
                    return Err(GameError::Validation(ValidationError::CardNotInHand(*card)));
                }

                let card_def = state.card_def(*card).ok_or(GameError::Validation(
                    ValidationError::MissingCardInstanceDefinition(*card),
                ))?;
                Ok(SubmittedCardFacts {
                    element: card_def.element,
                    level: card_def.level,
                })
            })
            .collect::<GameResult<Vec<_>>>()?;

        let team_star = state
            .has_rule_module(crate::domain::STAR_MODULE_ID)
            .then(|| {
                state
                    .players
                    .iter()
                    .find(|candidate| &candidate.id == player)
                    .and_then(|candidate| state.star_for_team(&candidate.team))
            })
            .flatten();

        Ok(Self {
            cards: selected_cards,
            facts,
            registry: official_formation_registry(&state.enabled_rule_modules),
            team_star,
        })
    }

    pub(super) fn candidates(&self) -> Vec<FormationCandidate> {
        let matcher = base_formation_matcher();

        self.registry
            .formations()
            .into_iter()
            .filter(|formation| self.matches_formation(formation, &matcher))
            .map(|formation| FormationCandidate {
                formation_id: formation.id.clone(),
                formation_name: formation.name.clone(),
                rule_text: formation.rule_text.clone(),
                category: formation.category.clone(),
                cards: self.cards.clone(),
            })
            .collect()
    }

    pub(super) fn require(self, formation_id: &str) -> GameResult<SelectedFormation> {
        let formation = self.registry.formation(formation_id).ok_or_else(|| {
            GameError::Validation(ValidationError::UnknownFormation(formation_id.to_string()))
        })?;

        if !self.matches_formation(formation, &base_formation_matcher()) {
            return Err(GameError::Validation(
                ValidationError::FormationPatternMismatch {
                    formation_id: formation_id.to_string(),
                },
            ));
        }

        let effect = self
            .registry
            .effect_for(formation)
            .expect("base formation registry must link every formation to an effect");

        Ok(SelectedFormation {
            formation_id: formation.id.clone(),
            cards: self.cards,
            effect_plan: effect.plan.clone(),
        })
    }

    fn matches_formation(
        &self,
        formation: &FormationDef,
        matcher: &crate::rules::FormationMatcher<'_>,
    ) -> bool {
        if let Some(required_star) = star::required_star(&formation.id)
            && self.team_star != Some(required_star)
        {
            return false;
        }

        if matcher.matches(&formation.pattern, &self.facts) {
            return true;
        }

        let Some(owned_star) = self.team_star else {
            return false;
        };
        if base_formation_registry().formation(&formation.id).is_none() {
            return false;
        }

        self.facts.iter().enumerate().any(|(index, card)| {
            if card.element != star::companion_element(owned_star) {
                return false;
            }
            let mut interpreted = self.facts.clone();
            interpreted[index].element = star::element(owned_star);
            matcher.matches(&formation.pattern, &interpreted)
        })
    }
}
