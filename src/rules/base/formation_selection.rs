use crate::domain::{
    CardInstanceId, FormationComposition, FormationRequirement, GameError, GameResult, GameState,
    PlayerId, StarElementSubstitution, TargetDecl, ValidationError, VirtualFormationCard,
};
use crate::rules::{
    EffectPlan, FormationCandidate, FormationDef, FormationRegistry, SubmittedCardFacts,
    base_formation_matcher, base_formation_registry, official_formation_registry, star,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct FormationSelection<'a> {
    state: &'a GameState,
    enabled_rule_modules: Vec<crate::domain::RuleModuleId>,
    profession: Option<crate::domain::ProfessionId>,
    profession_abilities_suppressed: bool,
    cards: Vec<CardInstanceId>,
    facts: Vec<SubmittedCardFacts>,
    virtual_card: Option<VirtualFormationCard>,
    formation_requirement: Option<FormationRequirement>,
    registry: FormationRegistry,
    team_star: Option<crate::domain::StarKind>,
    available_stars: Vec<crate::domain::StarKind>,
    prepared: Option<crate::domain::PreparedProfessionAbility>,
    spirit_level_interpretations: Vec<crate::domain::SpiritLevelInterpretation>,
    residual_card_facts: Option<(crate::domain::Element, u32)>,
    limited_uses: Vec<crate::domain::LimitedUse>,
    confluence_card_obligation: Option<crate::domain::ConfluenceCardObligation>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(super) struct SelectedFormation {
    pub(super) formation_id: String,
    pub(super) cards: Vec<CardInstanceId>,
    pub(super) composition: FormationComposition,
    pub(super) facts: Vec<SubmittedCardFacts>,
    pub(super) effect_plan: EffectPlan,
    pub(super) star_substitution: Option<StarElementSubstitution>,
    pub(super) declared_targets: Vec<TargetDecl>,
}

impl<'a> FormationSelection<'a> {
    pub(super) fn new(
        state: &'a GameState,
        player: &PlayerId,
        selected_cards: Vec<CardInstanceId>,
    ) -> GameResult<Self> {
        let hand = state
            .hand(player)
            .ok_or_else(|| GameError::Validation(ValidationError::UnknownPlayer(player.clone())))?;
        let requirement = state
            .formation_requirements
            .iter()
            .find(|requirement| {
                &requirement.player == player && requirement.applied_on_turn == state.turn_number
            })
            .cloned();
        let mut selected_cards = selected_cards;
        if let Some(card) = requirement
            .as_ref()
            .and_then(|requirement| requirement.physical_card)
            && !selected_cards.contains(&card)
        {
            selected_cards.push(card);
        }
        let mut seen = HashSet::new();
        let mut facts = selected_cards
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
                let prepared = state
                    .prepared_profession_abilities
                    .iter()
                    .rev()
                    .find(|prepared| {
                        &prepared.player == player
                            && prepared.card == *card
                            && prepared.prepared_on_turn == state.turn_number
                    });
                Ok(SubmittedCardFacts {
                    element: prepared
                        .map(|prepared| prepared.element)
                        .unwrap_or(card_def.element),
                    level: state
                        .card_level_for(player, *card)
                        .expect("known Card must have an effective level"),
                })
            })
            .collect::<GameResult<Vec<_>>>()?;
        let virtual_card = requirement
            .as_ref()
            .and_then(|requirement| requirement.virtual_card.clone());
        if let Some(card) = &virtual_card {
            facts.push(SubmittedCardFacts {
                element: card.element,
                level: card.level,
            });
        }

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
        let mut available_stars = team_star.into_iter().collect::<Vec<_>>();
        available_stars.extend(
            state
                .temporary_star_effects
                .iter()
                .filter(|effect| {
                    &effect.player == player && effect.applied_on_turn == state.turn_number
                })
                .map(|effect| effect.star),
        );

        Ok(Self {
            state,
            enabled_rule_modules: state.enabled_rule_modules.clone(),
            profession: state.profession_for(player).cloned(),
            profession_abilities_suppressed: crate::rules::pouch::profession_is_suppressed(
                state, player,
            ),
            cards: selected_cards,
            facts,
            virtual_card,
            formation_requirement: requirement,
            registry: official_formation_registry(&state.enabled_rule_modules),
            team_star,
            available_stars,
            prepared: (!crate::rules::pouch::profession_is_suppressed(state, player))
                .then(|| {
                    state
                        .prepared_profession_abilities
                        .iter()
                        .find(|prepared| {
                            &prepared.player == player
                                && prepared.prepared_on_turn == state.turn_number
                        })
                        .cloned()
                })
                .flatten(),
            spirit_level_interpretations: state
                .spirit_level_interpretations
                .iter()
                .filter(|interpretation| {
                    &interpretation.player == player
                        && interpretation.applied_on_turn == state.turn_number
                })
                .cloned()
                .collect(),
            residual_card_facts: crate::rules::confluence::residual_card_facts(state, player),
            limited_uses: state
                .limited_uses
                .iter()
                .filter(|use_count| &use_count.owner == player)
                .cloned()
                .collect(),
            confluence_card_obligation: state
                .confluence_card_obligations
                .iter()
                .find(|obligation| {
                    &obligation.owner == player && obligation.applied_on_turn == state.turn_number
                })
                .cloned(),
        })
    }

    pub(super) fn candidates(&self) -> Vec<FormationCandidate> {
        let matcher = base_formation_matcher();

        self.registry
            .formations()
            .into_iter()
            .flat_map(|formation| {
                let mut role_options = crate::rules::hero::formation_role_options(
                    &formation.id,
                    &self.cards,
                    &self.facts,
                );
                role_options.extend(crate::rules::confluence::formation_role_options(
                    &formation.id,
                    self.residual_card_facts,
                    &self.cards,
                ));
                let role_options = if role_options.is_empty() {
                    vec![(Vec::new(), None)]
                } else {
                    role_options
                        .into_iter()
                        .map(|(target, preview)| (vec![target], Some(preview)))
                        .collect()
                };
                let mut candidates =
                    self.match_options(formation, &matcher)
                        .into_iter()
                        .flat_map(move |star_substitution| {
                            role_options.clone().into_iter().map(
                                move |(declared_targets, preview)| FormationCandidate {
                                    formation_id: formation.id.clone(),
                                    formation_name: formation.name.clone(),
                                    rule_text: formation.rule_text.clone(),
                                    summary: formation.rule_text.clone(),
                                    category: formation.category.clone(),
                                    cards: self.cards.clone(),
                                    star_substitution: star_substitution.clone(),
                                    declared_targets,
                                    preview,
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                for card in self.sacred_art_options(formation, &matcher) {
                    candidates.push(FormationCandidate {
                        formation_id: formation.id.clone(),
                        formation_name: formation.name.clone(),
                        rule_text: formation.rule_text.clone(),
                        summary: formation.rule_text.clone(),
                        category: formation.category.clone(),
                        cards: self.cards.clone(),
                        star_substitution: None,
                        declared_targets: vec![TargetDecl::CardMultiplicity { card, slots: 2 }],
                        preview: Some(format!(
                            "牌 {} ×2；實際使用 {} 張牌",
                            card.as_u64(),
                            self.cards.len()
                        )),
                    });
                }
                candidates
            })
            .collect()
    }

    pub(super) fn require(
        self,
        formation_id: &str,
        declared_targets: Vec<TargetDecl>,
    ) -> GameResult<SelectedFormation> {
        let formation = self
            .registry
            .formation(formation_id)
            .cloned()
            .ok_or_else(|| {
                GameError::Validation(ValidationError::UnknownFormation(formation_id.to_string()))
            })?;

        if let Some(prepared_card) = declared_targets.iter().find_map(|target| match target {
            TargetDecl::FormationRole { role, card } if role == "prepared" => Some(*card),
            _ => None,
        }) {
            if self
                .prepared
                .as_ref()
                .is_some_and(|prepared| prepared.card == prepared_card)
                && self.prepared_matches(&formation, &base_formation_matcher())
            {
                return self.selected(&formation, None, declared_targets);
            }
            return Err(GameError::Validation(
                ValidationError::FormationPatternMismatch {
                    formation_id: formation_id.to_string(),
                },
            ));
        }
        if let Some((card, slots)) = declared_targets.iter().find_map(|target| match target {
            TargetDecl::CardMultiplicity { card, slots } => Some((*card, *slots)),
            _ => None,
        }) {
            if slots == 2
                && !declared_targets
                    .iter()
                    .any(|target| matches!(target, TargetDecl::Card(_)))
                && self
                    .sacred_art_options(&formation, &base_formation_matcher())
                    .contains(&card)
            {
                return self.selected(&formation, None, declared_targets);
            }
            return Err(GameError::Validation(
                ValidationError::FormationPatternMismatch {
                    formation_id: formation_id.to_string(),
                },
            ));
        }

        let options = self.match_options(&formation, &base_formation_matcher());
        if options.is_empty() {
            return Err(GameError::Validation(
                ValidationError::FormationPatternMismatch {
                    formation_id: formation_id.to_string(),
                },
            ));
        }
        let mut role_options =
            crate::rules::hero::formation_role_options(&formation.id, &self.cards, &self.facts);
        role_options.extend(crate::rules::confluence::formation_role_options(
            &formation.id,
            self.residual_card_facts,
            &self.cards,
        ));
        let mut declared_targets = declared_targets;
        if !role_options.is_empty() {
            let declared_role = declared_targets
                .iter()
                .find(|target| matches!(target, TargetDecl::FormationRole { .. }));
            match declared_role {
                Some(target) if role_options.iter().any(|(legal, _)| legal == target) => {}
                Some(_) => {
                    return Err(GameError::Validation(
                        ValidationError::FormationPatternMismatch {
                            formation_id: formation_id.to_string(),
                        },
                    ));
                }
                None if role_options.len() == 1 => {
                    declared_targets.push(role_options[0].0.clone());
                }
                None => {
                    return Err(GameError::Validation(
                        ValidationError::FormationMatchOptionRequired {
                            formation_id: formation_id.to_string(),
                        },
                    ));
                }
            }
        }
        let declared_substitution = declared_targets.iter().find_map(|target| match target {
            TargetDecl::Card(card) => Some(*card),
            TargetDecl::Player(_)
            | TargetDecl::Team(_)
            | TargetDecl::FormationRole { .. }
            | TargetDecl::CardMultiplicity { .. }
            | TargetDecl::SecretStrategy(_)
            | TargetDecl::SecretStrategyOptions { .. } => None,
        });
        let star_substitution = match declared_substitution {
            Some(card) => options
                .iter()
                .find_map(|option| {
                    option
                        .as_ref()
                        .filter(|substitution| substitution.card == card)
                        .cloned()
                })
                .ok_or_else(|| {
                    GameError::Validation(ValidationError::FormationPatternMismatch {
                        formation_id: formation_id.to_string(),
                    })
                })?,
            None if options.contains(&None) => {
                return self.selected(&formation, None, declared_targets);
            }
            None => {
                return Err(GameError::Validation(
                    ValidationError::FormationMatchOptionRequired {
                        formation_id: formation_id.to_string(),
                    },
                ));
            }
        };
        let remaining_targets = declared_targets
            .into_iter()
            .filter(|target| !matches!(target, TargetDecl::Card(card) if *card == star_substitution.card))
            .collect();

        self.selected(&formation, Some(star_substitution), remaining_targets)
    }

    fn selected(
        self,
        formation: &FormationDef,
        star_substitution: Option<StarElementSubstitution>,
        declared_targets: Vec<TargetDecl>,
    ) -> GameResult<SelectedFormation> {
        let effect = self
            .registry
            .effect_for(formation)
            .expect("base formation registry must link every formation to an effect");

        let composition = FormationComposition {
            physical_cards: self.cards.clone(),
            virtual_card: self.virtual_card,
        };
        Ok(SelectedFormation {
            formation_id: formation.id.clone(),
            cards: self.cards,
            composition,
            facts: self.facts,
            effect_plan: effect.plan.clone(),
            star_substitution,
            declared_targets,
        })
    }

    fn match_options(
        &self,
        formation: &FormationDef,
        matcher: &crate::rules::FormationMatcher<'_>,
    ) -> Vec<Option<StarElementSubstitution>> {
        if let Some(requirement) = &self.formation_requirement
            && (!requirement.allowed_formation_scope.iter().any(|scope| {
                scope == "all"
                    || scope == &formation.id
                    || (scope == "base"
                        && base_formation_registry().formation(&formation.id).is_some())
            }) || requirement
                .physical_card
                .is_some_and(|card| !self.cards.contains(&card)))
        {
            return Vec::new();
        }
        if let Some(prepared) = &self.prepared
            && self.cards.contains(&prepared.card)
            && !prepared.allowed_formation_scope.iter().any(|scope| {
                scope == "all"
                    || scope == &formation.id
                    || (scope == "base"
                        && base_formation_registry().formation(&formation.id).is_some())
            })
        {
            return Vec::new();
        }
        if !crate::rules::confluence::formation_selection_satisfies_obligation(
            self.confluence_card_obligation.as_ref(),
            &formation.id,
            &self.cards,
        ) {
            return Vec::new();
        }
        if formation.id == "empty-city" && self.matches_passive_proficiency() {
            return Vec::new();
        }

        if crate::rules::hero::is_profession_formation(&formation.id)
            && !crate::rules::hero::can_use_profession_formation(
                &self.enabled_rule_modules,
                self.profession.as_ref(),
                &formation.id,
            )
        {
            return Vec::new();
        }
        if crate::rules::jianghu::is_profession_formation(&formation.id)
            && !crate::rules::jianghu::can_use_profession_formation(
                &self.enabled_rule_modules,
                self.profession.as_ref(),
                &formation.id,
            )
        {
            return Vec::new();
        }
        if crate::rules::confluence::is_profession_formation(&formation.id)
            && !crate::rules::confluence::can_use_profession_formation(
                &self.enabled_rule_modules,
                self.profession.as_ref(),
                &formation.id,
            )
        {
            return Vec::new();
        }
        if crate::rules::dark::is_profession_formation(&formation.id)
            && !crate::rules::dark::can_use_profession_formation(
                &self.enabled_rule_modules,
                self.profession.as_ref(),
                &formation.id,
            )
        {
            return Vec::new();
        }
        if crate::rules::confluence::is_profession_formation(&formation.id)
            && !crate::rules::confluence::formation_available_for_selection(
                &self.limited_uses,
                &formation.id,
            )
        {
            return Vec::new();
        }

        if let Some(required_star) = star::required_star(&formation.id)
            && !self.available_stars.contains(&required_star)
        {
            return Vec::new();
        }

        let mut options = Vec::new();
        let formation_matches = if crate::rules::confluence::is_profession_formation(&formation.id)
        {
            crate::rules::confluence::formation_matches(
                &formation.id,
                self.residual_card_facts,
                &self.facts,
            )
        } else if crate::rules::dark::is_profession_formation(&formation.id) {
            crate::rules::dark::formation_matches(&formation.id, &self.facts)
        } else {
            matcher.matches(&formation.pattern, &self.facts)
        };
        if formation_matches {
            options.push(None);
        }

        if !self.profession_abilities_suppressed
            && crate::rules::hero::matches_proficiency(
                &self.enabled_rule_modules,
                self.profession.as_ref(),
                &formation.id,
                &self.facts,
            )
            && !options.contains(&None)
        {
            options.push(None);
        }

        if base_formation_registry().formation(&formation.id).is_none() {
            return options;
        }

        for available_star in &self.available_stars {
            options.extend(
                self.facts[..self.cards.len()]
                    .iter()
                    .enumerate()
                    .filter_map(|(index, card)| {
                        if card.element != star::companion_element(*available_star) {
                            return None;
                        }
                        let mut interpreted = self.facts.clone();
                        interpreted[index].element = star::element(*available_star);
                        matcher.matches(&formation.pattern, &interpreted).then(|| {
                            Some(StarElementSubstitution {
                                card: self.cards[index],
                                printed_element: card.element,
                                interpreted_element: star::element(*available_star),
                            })
                        })
                    }),
            );
        }
        options
    }

    fn matches_passive_proficiency(&self) -> bool {
        self.registry.formations().into_iter().any(|formation| {
            formation.id != "empty-city"
                && matches!(
                    self.registry
                        .effect_for(formation)
                        .map(|effect| &effect.plan),
                    Some(EffectPlan::PassiveSpell(_))
                )
                && !self.profession_abilities_suppressed
                && crate::rules::hero::matches_proficiency(
                    &self.enabled_rule_modules,
                    self.profession.as_ref(),
                    &formation.id,
                    &self.facts,
                )
        })
    }

    fn prepared_matches(
        &self,
        formation: &FormationDef,
        matcher: &crate::rules::FormationMatcher<'_>,
    ) -> bool {
        let Some(prepared) = &self.prepared else {
            return false;
        };
        if crate::rules::hero::is_profession_formation(&formation.id)
            && !crate::rules::hero::can_use_profession_formation(
                &self.enabled_rule_modules,
                self.profession.as_ref(),
                &formation.id,
            )
            || crate::rules::jianghu::is_profession_formation(&formation.id)
                && !crate::rules::jianghu::can_use_profession_formation(
                    &self.enabled_rule_modules,
                    self.profession.as_ref(),
                    &formation.id,
                )
            || crate::rules::confluence::is_profession_formation(&formation.id)
                && !crate::rules::confluence::can_use_profession_formation(
                    &self.enabled_rule_modules,
                    self.profession.as_ref(),
                    &formation.id,
                )
            || crate::rules::dark::is_profession_formation(&formation.id)
                && !crate::rules::dark::can_use_profession_formation(
                    &self.enabled_rule_modules,
                    self.profession.as_ref(),
                    &formation.id,
                )
        {
            return false;
        }
        if !self.cards.contains(&prepared.card)
            || !prepared.allowed_formation_scope.iter().any(|scope| {
                scope == &formation.id
                    || (scope == "base"
                        && base_formation_registry().formation(&formation.id).is_some())
                    || scope == "all"
            })
        {
            return false;
        }
        let mut interpreted = self.facts.clone();
        let Some(index) = self.cards.iter().position(|card| *card == prepared.card) else {
            return false;
        };
        let later_spirit_level = self
            .spirit_level_interpretations
            .iter()
            .rev()
            .find(|interpretation| {
                interpretation.card == prepared.card
                    && interpretation.interpretation_revision > prepared.interpretation_revision
            })
            .map(|interpretation| interpretation.level);
        interpreted[index] = SubmittedCardFacts {
            element: prepared.element,
            level: later_spirit_level.unwrap_or(prepared.level),
        };
        matcher.matches(&formation.pattern, &interpreted)
            || crate::rules::confluence::formation_matches(
                &formation.id,
                self.residual_card_facts,
                &interpreted,
            )
            || crate::rules::dark::formation_matches(&formation.id, &interpreted)
            || !self.profession_abilities_suppressed
                && crate::rules::hero::matches_proficiency(
                    &self.enabled_rule_modules,
                    self.profession.as_ref(),
                    &formation.id,
                    &interpreted,
                )
    }

    fn sacred_art_options(
        &self,
        formation: &FormationDef,
        matcher: &crate::rules::FormationMatcher<'_>,
    ) -> Vec<CardInstanceId> {
        if self.profession_abilities_suppressed
            || self.cards.len() != 3
            || base_formation_registry().formation(&formation.id).is_none()
            || !crate::rules::hero::profession_has_ability(
                &self.enabled_rule_modules,
                self.profession.as_ref(),
                crate::rules::hero::ProfessionAbility::SacredArt,
            )
        {
            return Vec::new();
        }
        self.facts[..self.cards.len()]
            .iter()
            .enumerate()
            .filter_map(|(index, fact)| {
                if fact.level < 4 {
                    return None;
                }
                let mut slots = self.facts.clone();
                slots.push(*fact);
                matcher
                    .matches(&formation.pattern, &slots)
                    .then_some(self.cards[index])
            })
            .collect()
    }
}
