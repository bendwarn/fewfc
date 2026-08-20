//! 玩家可見行動詳細資料補充內容的組合。
//!
//! 規則模組將可重用的規則專屬條款放在執行邏輯旁邊（目前由 `echo`、`pouch`
//! 與 `spirit` 提供）。此模組是唯一將這些條款接到合法已提供行動並排序的
//! 位置。行動識別仍保留在已提供行動上，空的補充內容也是合法的。

use crate::domain::{GameState, PlayerId, SecretStrategy, SpiritSkill};

use super::{
    ActionAttackCategory, ActionCost, ActionTarget, ConsequenceCertainty, EffectAmount,
    EffectFormula, FormationCandidate, ImmediateEffect, PlayableAction, PlayerFacingActionDetail,
    ProfessionAbilityCandidate, ProfessionAbilityEffect, RuleConsequence, RuleException,
    SecretStrategyEffect, SpiritSkillCandidate,
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
    match &mut action {
        PlayableAction::PerformFormation(candidate) => {
            candidate.detail = formation_detail(state, candidate)
        }
        PlayableAction::ChangeProfession(candidate) => {
            candidate.detail = profession_change_detail()
        }
        PlayableAction::ActivateProfessionAbility(candidate) => {
            candidate.detail = profession_ability_detail(state, player, candidate)
        }
        PlayableAction::UseSpiritSkill(candidate) => {
            candidate.detail = spirit_skill_detail(candidate)
        }
        PlayableAction::TriggerSecretStrategy(_)
        | PlayableAction::RetrievePreviousTurnDiscard(_)
        | PlayableAction::Pass { .. } => {}
    }
    action
}

fn formation_detail(state: &GameState, candidate: &FormationCandidate) -> PlayerFacingActionDetail {
    let mut consequences = Vec::new();

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

    append_formation_specific_consequences(&mut consequences, &candidate.formation_id);
    PlayerFacingActionDetail::composed(consequences)
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
    formation_id: &str,
) {
    if let Some(mut echo) = crate::rules::echo::action_detail_consequences(formation_id) {
        consequences.append(&mut echo);
    }
    if let Some(mut pouch) = crate::rules::pouch::formation_action_detail_consequences(formation_id)
    {
        consequences.append(&mut pouch);
    }
    for mut clauses in [
        crate::rules::base::formation_action_detail_consequences(formation_id),
        crate::rules::dark::formation_action_detail_consequences(formation_id),
        crate::rules::tribulation::formation_action_detail_consequences(formation_id),
        crate::rules::confluence::formation_action_detail_consequences(formation_id),
    ]
    .into_iter()
    .flatten()
    {
        consequences.append(&mut clauses);
    }
}

fn profession_change_detail() -> PlayerFacingActionDetail {
    PlayerFacingActionDetail::composed(Vec::new())
}

fn profession_ability_detail(
    state: &GameState,
    player: &PlayerId,
    candidate: &ProfessionAbilityCandidate,
) -> PlayerFacingActionDetail {
    let mut consequences = vec![RuleConsequence::ImmediateEffect {
        certainty: ConsequenceCertainty::Guaranteed,
        effect: ImmediateEffect::ActivateProfessionAbility {
            effect: profession_ability_effect(&candidate.ability_id),
        },
    }];
    if !candidate.cards.is_empty()
        && (crate::rules::hero::player_facing_ability_discards_selected_cards(
            &candidate.ability_id,
        ) || crate::rules::jianghu::player_facing_ability_discards_selected_cards(
            &candidate.ability_id,
        ) || crate::rules::confluence::player_facing_ability_discards_selected_cards(
            &candidate.ability_id,
        ))
    {
        consequences.push(RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::DiscardSelectedCards,
        });
    }
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
    PlayerFacingActionDetail::composed(consequences)
}

fn spirit_skill_detail(candidate: &SpiritSkillCandidate) -> PlayerFacingActionDetail {
    let mut consequences = vec![
        RuleConsequence::ImmediateEffect {
            certainty: ConsequenceCertainty::Guaranteed,
            effect: ImmediateEffect::UseSpiritSkill {
                effect: crate::rules::spirit::player_facing_effect(candidate.skill),
            },
        },
        RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::SpendSpiritPower {
                amount: crate::rules::spirit::skill_cost(candidate.skill),
            },
        },
    ];
    if candidate.selected_card.is_some() && candidate.skill == SpiritSkill::Flow {
        consequences.push(RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::DiscardSelectedCards,
        });
    }
    PlayerFacingActionDetail::composed(consequences)
}

pub(crate) fn secret_strategy_detail(strategy: SecretStrategy) -> PlayerFacingActionDetail {
    let consequences = vec![
        RuleConsequence::ImmediateEffect {
            certainty: ConsequenceCertainty::Guaranteed,
            effect: ImmediateEffect::TriggerSecretStrategy {
                effect: secret_strategy_effect(strategy),
            },
        },
        RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::ConsumePouch,
        },
    ];
    PlayerFacingActionDetail::composed(consequences)
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

pub(crate) fn discard_retrieval_detail(hp_cost: i32) -> PlayerFacingActionDetail {
    PlayerFacingActionDetail::composed(vec![
        RuleConsequence::ImmediateEffect {
            certainty: ConsequenceCertainty::Guaranteed,
            effect: ImmediateEffect::MovePreviousTurnDiscardToDeckTop,
        },
        RuleConsequence::Cost {
            certainty: ConsequenceCertainty::Guaranteed,
            cost: ActionCost::LoseHp { amount: hp_cost },
        },
    ])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{CardInstanceId, Element, GameSetup, Phase};
    use crate::rules::{FormationEffect, ProfessionChangeCandidate, TrustedRandomness};

    #[test]
    fn consequence_contract_uses_tagged_variants_and_camel_case_fields() {
        let detail = PlayerFacingActionDetail::composed(vec![
            RuleConsequence::Cost {
                certainty: ConsequenceCertainty::Guaranteed,
                cost: ActionCost::OptionalDiscardByPrintedElement {
                    allowed_printed_elements: vec![Element::Metal, Element::Earth],
                },
            },
            RuleConsequence::ImmediateEffect {
                certainty: ConsequenceCertainty::Guaranteed,
                effect: ImmediateEffect::MovePreviousTurnDiscardToDeckTop,
            },
            RuleConsequence::Cost {
                certainty: ConsequenceCertainty::Guaranteed,
                cost: ActionCost::ConsumePouch,
            },
            RuleConsequence::ImmediateEffect {
                certainty: ConsequenceCertainty::Guaranteed,
                effect: ImmediateEffect::ActivateProfessionAbility {
                    effect: ProfessionAbilityEffect::IncreaseTurnDraw { amount: 1 },
                },
            },
            RuleConsequence::FollowUpChoice {
                certainty: ConsequenceCertainty::FollowUp,
                choice: super::super::FollowUpChoice::SelectCards {
                    minimum: 1,
                    maximum: 2,
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
            RuleConsequence::Cost {
                certainty: ConsequenceCertainty::Guaranteed,
                cost: ActionCost::DiscardSelectedCards,
            },
        ]);
        let json = serde_json::to_value(detail).expect("detail must serialize");

        assert_eq!(json["consequences"][0]["type"], "cost");
        assert_eq!(
            json["consequences"][0]["cost"]["allowedPrintedElements"],
            serde_json::json!(["Metal", "Earth"])
        );
        assert_eq!(
            json["consequences"][1]["effect"],
            serde_json::json!({ "type": "movePreviousTurnDiscardToDeckTop" })
        );
        assert_eq!(
            json["consequences"][2]["cost"],
            serde_json::json!({ "type": "consumePouch" })
        );
        assert_eq!(
            json["consequences"][3]["effect"],
            serde_json::json!({
                "type": "activateProfessionAbility",
                "effect": { "type": "increaseTurnDraw", "amount": 1 }
            })
        );
        assert_eq!(json["consequences"][4]["type"], "followUpChoice");
        assert_eq!(
            json["consequences"][5]["operation"]["type"],
            "selectHiddenHandCards"
        );
        assert_eq!(json["consequences"][5]["operation"]["count"], 2);
        assert_eq!(json["consequences"][6]["timing"], "nextTurnStart");
        assert_eq!(json["consequences"][7]["exception"]["remaining"], 0);
        assert_eq!(
            json["consequences"][8]["cost"],
            serde_json::json!({ "type": "discardSelectedCards" })
        );
        assert_eq!(
            serde_json::to_value(TrustedRandomness::ShuffleDeck)
                .expect("randomness operation must serialize"),
            serde_json::json!({ "type": "shuffleDeck" })
        );
        assert_eq!(
            serde_json::to_value(TrustedRandomness::ShuffleDiscardIntoDeck)
                .expect("randomness operation must serialize"),
            serde_json::json!({ "type": "shuffleDiscardIntoDeck" })
        );
    }

    #[test]
    fn formation_effect_contract_uses_tagged_variants_and_camel_case_fields() {
        assert_eq!(
            serde_json::to_value(FormationEffect::PreventOtherPlayersFromActingOrDrawing {
                duration_turns: 1,
            })
            .expect("formation effect must serialize"),
            serde_json::json!({
                "type": "preventOtherPlayersFromActingOrDrawing",
                "durationTurns": 1,
            })
        );
        assert_eq!(
            serde_json::to_value(FormationEffect::InspectHand)
                .expect("unit formation effect must serialize"),
            serde_json::json!({ "type": "inspectHand" })
        );
    }

    #[test]
    fn composes_only_contextual_supplements_for_each_offer_family() {
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
        state.phase = Phase::ActiveEffects;
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
                    input_requirement: None,
                    detail: pending.clone(),
                }),
                PlayableAction::UseSpiritSkill(SpiritSkillCandidate {
                    skill: SpiritSkill::Splendor,
                    skill_name: "絢爛".to_string(),
                    selected_card: Some(CardInstanceId::new(6)),
                    declared_level: Some(4),
                    detail: pending,
                }),
                PlayableAction::PerformFormation(FormationCandidate {
                    formation_id: "east-azure-dragon".to_string(),
                    formation_name: "東‧青龍".to_string(),
                    category: super::super::FormationCategory::Attack,
                    cards: vec![CardInstanceId::new(11)],
                    star_substitution: None,
                    declared_targets: Vec::new(),
                    preview: None,
                    detail: PlayerFacingActionDetail::pending_composition(),
                }),
            ],
        );

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
        let PlayableAction::ChangeProfession(change) = &actions[1] else {
            panic!("second action should be Profession Change")
        };
        assert!(change.detail.consequences.is_empty());

        let PlayableAction::ActivateProfessionAbility(ability) = &actions[2] else {
            panic!("third action should be Profession Ability")
        };
        assert!(matches!(
            ability.detail.consequences.as_slice(),
            [
                RuleConsequence::ImmediateEffect {
                    effect: ImmediateEffect::ActivateProfessionAbility { .. },
                    ..
                },
                RuleConsequence::Cost {
                    cost: ActionCost::DiscardSelectedCards,
                    ..
                }
            ]
        ));

        let pouch = secret_strategy_detail(SecretStrategy::GoldenCicada);
        assert!(pouch.consequences.iter().any(|consequence| matches!(
            consequence,
            RuleConsequence::ImmediateEffect {
                effect: ImmediateEffect::TriggerSecretStrategy {
                    effect: SecretStrategyEffect::ProtectTriggeringPlayer,
                },
                ..
            }
        )));
        assert!(matches!(
            pouch.consequences.as_slice(),
            [
                RuleConsequence::ImmediateEffect { .. },
                RuleConsequence::Cost {
                    cost: ActionCost::ConsumePouch,
                    ..
                }
            ]
        ));

        let sheep = secret_strategy_detail(SecretStrategy::SheepStealing);
        assert_eq!(sheep.consequences.len(), 2);
        let PlayableAction::PerformFormation(sacred_beast) = &actions[4] else {
            panic!("fifth action should be the supplied sacred beast")
        };
        assert!(
            sacred_beast
                .detail
                .consequences
                .iter()
                .any(|consequence| matches!(
                    consequence,
                    RuleConsequence::RuleException {
                        exception: RuleException::IgnoresOtherFormationEffects,
                        ..
                    }
                ))
        );
        let retrieval = discard_retrieval_detail(6);
        assert!(matches!(
            retrieval.consequences.as_slice(),
            [
                RuleConsequence::ImmediateEffect {
                    effect: ImmediateEffect::MovePreviousTurnDiscardToDeckTop,
                    ..
                },
                RuleConsequence::Cost {
                    cost: ActionCost::LoseHp { amount: 6 },
                    ..
                }
            ]
        ));
    }
}
