use crate::domain::{CardInstanceId, GameError, GameResult, GameState, PlayerId, ValidationError};
use crate::rules::{
    EffectPlan, FormationCandidate, SubmittedCardFacts, base_formation_matcher,
    base_formation_registry,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FormationSelection {
    cards: Vec<CardInstanceId>,
    facts: Vec<SubmittedCardFacts>,
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

        Ok(Self {
            cards: selected_cards,
            facts,
        })
    }

    pub(super) fn candidates(&self) -> Vec<FormationCandidate> {
        let registry = base_formation_registry();
        let matcher = base_formation_matcher();

        registry
            .formations()
            .into_iter()
            .filter(|formation| matcher.matches(&formation.pattern, &self.facts))
            .map(|formation| FormationCandidate {
                formation_id: formation.id.clone(),
                formation_name: formation.name.clone(),
                category: formation.category.clone(),
                cards: self.cards.clone(),
            })
            .collect()
    }

    pub(super) fn require(self, formation_id: &str) -> GameResult<SelectedFormation> {
        let registry = base_formation_registry();
        let formation = registry.formation(formation_id).ok_or_else(|| {
            GameError::Validation(ValidationError::UnknownFormation(formation_id.to_string()))
        })?;

        if !base_formation_matcher().matches(&formation.pattern, &self.facts) {
            return Err(GameError::Validation(
                ValidationError::FormationPatternMismatch {
                    formation_id: formation_id.to_string(),
                },
            ));
        }

        let effect = registry
            .effect_for(formation)
            .expect("base formation registry must link every formation to an effect");

        Ok(SelectedFormation {
            formation_id: formation.id.clone(),
            cards: self.cards,
            effect_plan: effect.plan.clone(),
        })
    }
}
