//! Composition of player-facing action facts.
//!
//! Rule modules keep their reusable rule-specific clauses beside execution
//! (`echo`, `pouch`, and `spirit` currently contribute here).  This module is
//! the one place that joins those clauses to a legal offered action, orders
//! them, applies snapshot facts, and refuses incomplete offers.

use crate::domain::{CardInstanceId, GameState, PlayerId, SecretStrategy, SpiritSkill};

use super::{
    ActionAttackCategory, ActionCost, ActionTarget, ConsequenceCertainty, DeclaredInput,
    EffectAmount, EffectFormula, FormationCandidate, ImmediateEffect, PlayableAction,
    PlayerFacingActionDetail, ProfessionAbilityCandidate, ProfessionAbilityEffect,
    ProfessionChangeCandidate, RuleConsequence, RuleException, SecretStrategyEffect,
    SpiritSkillCandidate, TrustedRandomness,
};

pub(crate) fn attach_to_actions(
    state: &GameState,
    player: &PlayerId,
    actions: Vec<PlayableAction>,
) -> Vec<PlayableAction> {
    actions
        .into_iter()
        .map(|action| attach_to_action(state, player, action))
        .collect()
}

fn profession_ability_effect(ability_id: &str) -> ProfessionAbilityEffect {
    [
        crate::rules::hero::player_facing_ability_effect(ability_id),
        crate::rules::jianghu::player_facing_ability_effect(ability_id),
        crate::rules::confluence::player_facing_ability_effect(ability_id),
        crate::rules::dark::player_facing_ability_effect(ability_id),
    ]
    .into_iter()
    .flatten()
    .next()
    .unwrap_or_else(|| panic!("offered profession ability `{ability_id}` has no detail effect"))
}

fn attach_to_action(
    state: &GameState,
    player: &PlayerId,
    mut action: PlayableAction,
) -> PlayableAction {
    let detail = match &action {
        PlayableAction::PerformFormation(candidate) => formation_detail(state, player, candidate),
        PlayableAction::ChangeProfession(candidate) => profession_change_detail(candidate),
        PlayableAction::ActivateProfessionAbility(candidate) => {
            profession_ability_detail(state, player, candidate)
        }
        PlayableAction::UseSpiritSkill(candidate) => spirit_skill_detail(candidate),
    };
    debug_assert!(detail.is_complete());
    match &mut action {
        PlayableAction::PerformFormation(candidate) => candidate.detail = detail,
        PlayableAction::ChangeProfession(candidate) => candidate.detail = detail,
        PlayableAction::ActivateProfessionAbility(candidate) => candidate.detail = detail,
        PlayableAction::UseSpiritSkill(candidate) => candidate.detail = detail,
    }
    action
}

fn formation_detail(
    state: &GameState,
    _player: &PlayerId,
    candidate: &FormationCandidate,
) -> PlayerFacingActionDetail {
    let mut consequences = vec![RuleConsequence::Cost {
        certainty: ConsequenceCertainty::Guaranteed,
        cost: ActionCost::UseCards {
            cards: candidate.cards.clone(),
        },
    }];

    if let Some(substitution) = &candidate.star_substitution {
        consequences.push(RuleConsequence::Substitution {
            certainty: ConsequenceCertainty::Guaranteed,
            card: substitution.card,
            printed_element: substitution.printed_element,
            interpreted_element: substitution.interpreted_element,
        });
    }

    for target in &candidate.declared_targets {
        match target {
            crate::domain::TargetDecl::FormationRole { card, .. }
            | crate::domain::TargetDecl::CardMultiplicity { card, .. } => {
                consequences.push(RuleConsequence::DeclaredInput {
                    certainty: ConsequenceCertainty::Guaranteed,
                    input: DeclaredInput::TargetCard { card: *card },
                });
            }
            _ => {}
        }
    }

    let registry = crate::rules::official_formation_registry(&state.enabled_rule_modules);
    let formation = registry
        .formation(&candidate.formation_id)
        .expect("a selected formation candidate must remain in its registry");
    let effect = registry
        .effect(&formation.effect_id)
        .expect("a selected formation candidate must have an effect definition");
    match &effect.plan {
        super::EffectPlan::Attack(plan) => consequences.push(RuleConsequence::ImmediateEffect {
            certainty: ConsequenceCertainty::Guaranteed,
            effect: ImmediateEffect::Attack {
                target: match plan.damage_target {
                    super::DamageTarget::PreviousPlayer => ActionTarget::PreviousPlayer,
                },
                category: match plan.category {
                    super::AttackCategory::Elemental(_) => ActionAttackCategory::Elemental,
                    super::AttackCategory::Physical => ActionAttackCategory::Physical,
                    super::AttackCategory::Special => ActionAttackCategory::Special,
                },
                points: point_amount(&plan.point_formula),
            },
        }),
        super::EffectPlan::ActiveSpell(plan) => {
            consequences.push(RuleConsequence::ImmediateEffect {
                certainty: ConsequenceCertainty::Guaranteed,
                effect: ImmediateEffect::ResolveFormationEffect {
                    effect: plan.player_facing_effect,
                },
            })
        }
        super::EffectPlan::PassiveSpell(plan) => {
            consequences.push(RuleConsequence::ImmediateEffect {
                certainty: ConsequenceCertainty::Guaranteed,
                effect: ImmediateEffect::ResolveFormationEffect {
                    effect: plan.player_facing_effect,
                },
            })
        }
    }

    append_formation_specific_consequences(
        &mut consequences,
        state,
        _player,
        &candidate.formation_id,
    );
    PlayerFacingActionDetail::complete(consequences)
}

fn point_amount(formula: &super::PointFormula) -> EffectAmount {
    match formula {
        super::PointFormula::Fixed(value) => EffectAmount::Fixed { value: *value },
        super::PointFormula::LevelPlus(amount) => EffectAmount::Formula {
            formula: EffectFormula::LevelPlus { amount: *amount },
        },
        super::PointFormula::LevelSumTimes(multiplier) => EffectAmount::Formula {
            formula: EffectFormula::LevelSumTimes {
                multiplier: *multiplier,
            },
        },
        super::PointFormula::TargetHandCountTimes(multiplier) => EffectAmount::Formula {
            formula: EffectFormula::TargetHandCountTimes {
                multiplier: *multiplier,
            },
        },
        super::PointFormula::ElementProductTimes {
            element,
            multiplier,
        } => EffectAmount::Formula {
            formula: EffectFormula::ElementProductTimes {
                element: *element,
                multiplier: *multiplier,
            },
        },
    }
}

fn append_formation_specific_consequences(
    consequences: &mut Vec<RuleConsequence>,
    state: &GameState,
    player: &PlayerId,
    formation_id: &str,
) {
    if let Some(mut echo) = crate::rules::echo::action_detail_consequences(formation_id) {
        consequences.append(&mut echo);
    }
    if let Some(mut pouch) =
        crate::rules::pouch::formation_action_detail_consequences(state, player, formation_id)
    {
        consequences.append(&mut pouch);
    }
    for specific in [
        crate::rules::base::formation_action_detail_consequences(formation_id),
        crate::rules::dark::formation_action_detail_consequences(formation_id),
        crate::rules::tribulation::formation_action_detail_consequences(formation_id),
        crate::rules::confluence::formation_action_detail_consequences(formation_id),
    ] {
        if let Some(mut clauses) = specific {
            consequences.append(&mut clauses);
        }
    }
}

fn profession_change_detail(candidate: &ProfessionChangeCandidate) -> PlayerFacingActionDetail {
    PlayerFacingActionDetail::complete(vec![
        RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::UseCards {
                cards: candidate.cards.clone(),
            },
        },
        RuleConsequence::ImmediateEffect {
            certainty: ConsequenceCertainty::Guaranteed,
            effect: ImmediateEffect::ChangeProfession {
                profession_id: candidate.profession_id.as_str().to_string(),
            },
        },
        RuleConsequence::RuleException {
            certainty: ConsequenceCertainty::Conditional,
            exception: RuleException::EffectMayBeIneffective,
        },
    ])
}

fn profession_ability_detail(
    state: &GameState,
    player: &PlayerId,
    candidate: &ProfessionAbilityCandidate,
) -> PlayerFacingActionDetail {
    let mut consequences = Vec::new();
    if !candidate.cards.is_empty() {
        consequences.push(RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::DiscardCards {
                cards: candidate.cards.clone(),
            },
        });
    }
    if let Some(card) = candidate.target_card {
        consequences.push(RuleConsequence::DeclaredInput {
            certainty: ConsequenceCertainty::Guaranteed,
            input: DeclaredInput::TargetCard { card },
        });
    }
    if let Some(element) = candidate.declared_element {
        consequences.push(RuleConsequence::DeclaredInput {
            certainty: ConsequenceCertainty::Guaranteed,
            input: DeclaredInput::Element { element },
        });
    }
    if let Some(level) = candidate.declared_level {
        consequences.push(RuleConsequence::DeclaredInput {
            certainty: ConsequenceCertainty::Guaranteed,
            input: DeclaredInput::Level { level },
        });
    }
    consequences.push(RuleConsequence::ImmediateEffect {
        certainty: ConsequenceCertainty::Guaranteed,
        effect: ImmediateEffect::ActivateProfessionAbility {
            ability_id: candidate.ability_id.clone(),
            effect: profession_ability_effect(&candidate.ability_id),
        },
    });
    if let Some(use_count) = state
        .limited_uses
        .iter()
        .find(|use_count| use_count.owner == *player && use_count.key == candidate.ability_id)
    {
        consequences.push(RuleConsequence::RuleException {
            certainty: ConsequenceCertainty::Guaranteed,
            exception: RuleException::LimitedUse {
                key: use_count.key.clone(),
                remaining: use_count.remaining,
                maximum: use_count.maximum,
            },
        });
    }
    consequences.push(RuleConsequence::RuleException {
        certainty: ConsequenceCertainty::Guaranteed,
        exception: RuleException::DoesNotEndAction,
    });
    PlayerFacingActionDetail::complete(consequences)
}

fn spirit_skill_detail(candidate: &SpiritSkillCandidate) -> PlayerFacingActionDetail {
    let mut consequences = vec![RuleConsequence::Cost {
        certainty: ConsequenceCertainty::Guaranteed,
        cost: ActionCost::SpendSpiritPower {
            amount: crate::rules::spirit::skill_cost(candidate.skill),
        },
    }];
    if let Some(card) = candidate.selected_card {
        if candidate.skill == SpiritSkill::Flow {
            consequences.push(RuleConsequence::Cost {
                certainty: ConsequenceCertainty::Guaranteed,
                cost: ActionCost::DiscardCards { cards: vec![card] },
            });
        } else {
            consequences.push(RuleConsequence::DeclaredInput {
                certainty: ConsequenceCertainty::Guaranteed,
                input: DeclaredInput::Card { card },
            });
        }
    }
    if let Some(level) = candidate.declared_level {
        consequences.push(RuleConsequence::DeclaredInput {
            certainty: ConsequenceCertainty::Guaranteed,
            input: DeclaredInput::Level { level },
        });
    }
    consequences.push(RuleConsequence::ImmediateEffect {
        certainty: ConsequenceCertainty::Guaranteed,
        effect: ImmediateEffect::UseSpiritSkill {
            effect: crate::rules::spirit::player_facing_effect(candidate.skill),
        },
    });
    if candidate.skill == SpiritSkill::EvilGaze {
        consequences.push(RuleConsequence::TrustedRandomness {
            certainty: ConsequenceCertainty::Random,
            operation: TrustedRandomness::SelectHiddenHandCards { count: 2 },
        });
    }
    consequences.push(RuleConsequence::RuleException {
        certainty: ConsequenceCertainty::Guaranteed,
        exception: RuleException::DoesNotEndAction,
    });
    PlayerFacingActionDetail::complete(consequences)
}

pub(crate) fn secret_strategy_detail(
    source_card: CardInstanceId,
    strategy: SecretStrategy,
    input: super::SecretStrategyInput,
) -> PlayerFacingActionDetail {
    let mut consequences = vec![
        RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::ConsumePouch { source_card },
        },
        RuleConsequence::ImmediateEffect {
            certainty: ConsequenceCertainty::Guaranteed,
            effect: ImmediateEffect::TriggerSecretStrategy {
                effect: secret_strategy_effect(strategy),
            },
        },
    ];
    if input != super::SecretStrategyInput::None {
        consequences.push(RuleConsequence::FollowUpChoice {
            certainty: ConsequenceCertainty::FollowUp,
            choice: super::FollowUpChoice::SelectSecretStrategyInput { input },
        });
    }
    PlayerFacingActionDetail::complete(consequences)
}

fn secret_strategy_effect(strategy: SecretStrategy) -> SecretStrategyEffect {
    match strategy {
        SecretStrategy::GoldenCicada => SecretStrategyEffect::ProtectTriggeringPlayer,
        SecretStrategy::StealTheBeam => SecretStrategyEffect::IncreaseHandLevels,
        SecretStrategy::MuddyWaters => SecretStrategyEffect::IncreaseTurnDraw,
        SecretStrategy::WatchTheFire => SecretStrategyEffect::NegateNextPlayerFormationHpChanges,
        SecretStrategy::LureTheTigerAway => {
            SecretStrategyEffect::SuppressPlayerAbilitiesAndSpiritPower
        }
        SecretStrategy::ReturnSoul => SecretStrategyEffect::SummonSpiritFromPouch,
        SecretStrategy::SheepStealing => SecretStrategyEffect::SwapDeckAndDiscard,
        SecretStrategy::DarkCrossing => SecretStrategyEffect::DirectProfessionChange,
        SecretStrategy::DeceiveHeaven => SecretStrategyEffect::BreakOrGainStar,
        SecretStrategy::Retreat => SecretStrategyEffect::ClearOrChangeEnvironment,
    }
}

pub(crate) fn discard_retrieval_detail(
    card: CardInstanceId,
    previous_player: PlayerId,
    hp_cost: i32,
) -> PlayerFacingActionDetail {
    PlayerFacingActionDetail::complete(vec![
        RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::LoseHp { amount: hp_cost },
        },
        RuleConsequence::ImmediateEffect {
            certainty: ConsequenceCertainty::Guaranteed,
            effect: ImmediateEffect::MovePreviousTurnDiscardToDeckTop {
                card,
                previous_player,
            },
        },
        RuleConsequence::RuleException {
            certainty: ConsequenceCertainty::Guaranteed,
            exception: RuleException::DoesNotEndAction,
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Element, GameSetup, Phase};

    #[test]
    fn consequence_contract_uses_tagged_variants_and_camel_case_fields() {
        let detail = PlayerFacingActionDetail::complete(vec![
            RuleConsequence::Cost {
                certainty: ConsequenceCertainty::Guaranteed,
                cost: ActionCost::OptionalDiscardByPrintedElement {
                    allowed_printed_elements: vec![Element::Metal, Element::Earth],
                },
            },
            RuleConsequence::Substitution {
                certainty: ConsequenceCertainty::Guaranteed,
                card: CardInstanceId::new(7),
                printed_element: Element::Metal,
                interpreted_element: Element::Earth,
            },
            RuleConsequence::ImmediateEffect {
                certainty: ConsequenceCertainty::Guaranteed,
                effect: ImmediateEffect::MovePreviousTurnDiscardToDeckTop {
                    card: CardInstanceId::new(8),
                    previous_player: PlayerId::new("bob"),
                },
            },
            RuleConsequence::Cost {
                certainty: ConsequenceCertainty::Guaranteed,
                cost: ActionCost::ConsumePouch {
                    source_card: CardInstanceId::new(10),
                },
            },
            RuleConsequence::ImmediateEffect {
                certainty: ConsequenceCertainty::Guaranteed,
                effect: ImmediateEffect::ChangeProfession {
                    profession_id: "hero:warrior".to_string(),
                },
            },
            RuleConsequence::ImmediateEffect {
                certainty: ConsequenceCertainty::Guaranteed,
                effect: ImmediateEffect::ActivateProfessionAbility {
                    ability_id: "meditation".to_string(),
                    effect: ProfessionAbilityEffect::IncreaseTurnDraw { amount: 1 },
                },
            },
            RuleConsequence::FollowUpChoice {
                certainty: ConsequenceCertainty::FollowUp,
                choice: super::super::FollowUpChoice::SelectSecretStrategyInput {
                    input: super::super::SecretStrategyInput::TargetPlayer,
                },
            },
            RuleConsequence::TrustedRandomness {
                certainty: ConsequenceCertainty::Random,
                operation: TrustedRandomness::SelectHiddenHandCards { count: 2 },
            },
            RuleConsequence::DelayedEffect {
                certainty: ConsequenceCertainty::Scheduled,
                timing: super::super::DelayedTiming::NextTurnStart,
                effect: super::super::DelayedEffect::RepeatMelodyMainEffect,
            },
            RuleConsequence::RuleException {
                certainty: ConsequenceCertainty::Guaranteed,
                exception: RuleException::LimitedUse {
                    key: "confluence:tailwind".to_string(),
                    remaining: 0,
                    maximum: 1,
                },
            },
            RuleConsequence::DeclaredInput {
                certainty: ConsequenceCertainty::Guaranteed,
                input: DeclaredInput::TargetCard {
                    card: CardInstanceId::new(9),
                },
            },
        ]);
        let json = serde_json::to_value(detail).expect("detail must serialize");

        assert_eq!(json["consequences"][0]["type"], "cost");
        assert_eq!(
            json["consequences"][0]["cost"]["allowedPrintedElements"],
            serde_json::json!(["Metal", "Earth"])
        );
        assert_eq!(json["consequences"][1]["printedElement"], "Metal");
        assert_eq!(json["consequences"][1]["interpretedElement"], "Earth");
        assert!(json["consequences"][1].get("printed_element").is_none());
        assert_eq!(json["consequences"][2]["type"], "immediateEffect");
        assert_eq!(json["consequences"][2]["effect"]["previousPlayer"], "bob");
        assert_eq!(json["consequences"][6]["type"], "followUpChoice");
        assert_eq!(json["consequences"][7]["operation"]["count"], 2);
        assert_eq!(json["consequences"][8]["timing"], "nextTurnStart");
        assert_eq!(json["consequences"][9]["exception"]["remaining"], 0);
        assert_eq!(json["consequences"][10]["input"]["type"], "targetCard");
        assert_eq!(json["consequences"][10]["input"]["card"], 9);
        assert_eq!(json["consequences"][3]["cost"]["sourceCard"], 10);
        assert!(json["consequences"][3]["cost"].get("source_card").is_none());
        assert_eq!(
            json["consequences"][4]["effect"]["professionId"],
            "hero:warrior"
        );
        assert!(
            json["consequences"][4]["effect"]
                .get("profession_id")
                .is_none()
        );
        assert_eq!(json["consequences"][5]["effect"]["abilityId"], "meditation");
        assert!(
            json["consequences"][5]["effect"]
                .get("ability_id")
                .is_none()
        );
        assert!(
            json["consequences"][2]["effect"]
                .get("previous_player")
                .is_none()
        );
    }

    #[test]
    fn composes_complete_details_for_every_offer_family_without_catalog_text() {
        let rules = crate::rules::OfficialRules::new();
        let alice = PlayerId::new("alice");
        let bob = PlayerId::new("bob");
        let setup = rules
            .configure_game(
                GameSetup::two_player(alice.clone(), bob.clone(), 20).players,
                vec![alice.clone(), bob],
                rules.default_rule_modules(),
            )
            .expect("official default configuration must be valid");
        let mut state = GameState::from_setup(&setup);
        state.phase = Phase::Main;
        let pending = PlayerFacingActionDetail::pending_composition();
        let actions = attach_to_actions(
            &state,
            &alice,
            vec![
                PlayableAction::PerformFormation(FormationCandidate {
                    formation_id: crate::rules::echo::PURE_FIRE.to_string(),
                    formation_name: "變徵‧淨火".to_string(),
                    category: super::super::FormationCategory::Spell,
                    cards: vec![CardInstanceId::new(1), CardInstanceId::new(2)],
                    star_substitution: None,
                    declared_targets: Vec::new(),
                    preview: None,
                    detail: pending.clone(),
                }),
                PlayableAction::ChangeProfession(ProfessionChangeCandidate {
                    profession_id: crate::domain::ProfessionId::new("hero:warrior"),
                    profession_name: "戰士".to_string(),
                    cards: vec![CardInstanceId::new(3)],
                    detail: pending.clone(),
                }),
                PlayableAction::ActivateProfessionAbility(ProfessionAbilityCandidate {
                    ability_id: "illusion".to_string(),
                    ability_name: "幻術".to_string(),
                    cards: vec![CardInstanceId::new(4), CardInstanceId::new(5)],
                    target_card: None,
                    declared_element: Some(Element::Water),
                    declared_level: Some(3),
                    detail: pending.clone(),
                }),
                PlayableAction::UseSpiritSkill(SpiritSkillCandidate {
                    skill: SpiritSkill::Splendor,
                    skill_name: "絢爛".to_string(),
                    selected_card: Some(CardInstanceId::new(6)),
                    declared_level: Some(4),
                    detail: pending,
                }),
            ],
        );

        assert!(actions.iter().all(|action| match action {
            PlayableAction::PerformFormation(candidate) => candidate.detail.is_complete(),
            PlayableAction::ChangeProfession(candidate) => candidate.detail.is_complete(),
            PlayableAction::ActivateProfessionAbility(candidate) => candidate.detail.is_complete(),
            PlayableAction::UseSpiritSkill(candidate) => candidate.detail.is_complete(),
        }));
        let PlayableAction::PerformFormation(echo) = &actions[0] else {
            panic!("first action should be the supplied Echo formation")
        };
        assert!(echo.detail.consequences.iter().any(|consequence| matches!(
            consequence,
            RuleConsequence::DelayedEffect {
                certainty: ConsequenceCertainty::Scheduled,
                timing: super::super::DelayedTiming::NextTurnStart,
                ..
            }
        )));
        let pouch = secret_strategy_detail(
            CardInstanceId::new(7),
            SecretStrategy::GoldenCicada,
            super::super::SecretStrategyInput::None,
        );
        assert!(pouch.consequences.iter().any(|consequence| matches!(
            consequence,
            RuleConsequence::ImmediateEffect {
                effect: ImmediateEffect::TriggerSecretStrategy {
                    effect: SecretStrategyEffect::ProtectTriggeringPlayer,
                },
                ..
            }
        )));
        let retrieval = discard_retrieval_detail(CardInstanceId::new(8), PlayerId::new("bob"), 6);
        assert!(retrieval.consequences.iter().any(|consequence| matches!(
            consequence,
            RuleConsequence::RuleException {
                exception: RuleException::DoesNotEndAction,
                ..
            }
        )));
    }
}
