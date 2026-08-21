use fewfc::application::GameRecord;
use fewfc::domain::{
    ActionModification, AttackOutcome, AttackPointBreakdown, AttackResolutionEffects,
    CardInstanceId, ChoiceAnswer, Command, DamageTransform, Element, ElementInteraction,
    EnvironmentAttackEffect, FormationAreaState, GameEvent, HpChangeDelta, LastElementalAttack,
    LastElementalAttackUpdate, PassiveFlipOutcome, Player, PlayerId, RuleModuleId, STAR_MODULE_ID,
    StarBreakReason, StarElementSubstitution, StarKind, TargetDecl, TeamId, TeamStar,
};
use fewfc::public_view::{PublicCardRefs, PublicCoveredPassive, Viewer};
use fewfc::rules::{OfficialRules, PlayableAction};

struct StarDefenseScenario {
    record: GameRecord,
    owner: PlayerId,
    attacker: PlayerId,
    summoning_cards: Vec<CardInstanceId>,
    defense_cards: Vec<CardInstanceId>,
    substituted_card: CardInstanceId,
    first_attack_card: CardInstanceId,
    defended_attack_card: CardInstanceId,
}

impl StarDefenseScenario {
    fn ordinary_defense() -> Self {
        let mut scenario = Self::new();
        let ordinary_wood = scenario.card(Element::Wood, 2);
        scenario.defense_cards = vec![scenario.defense_cards[0], ordinary_wood];
        scenario.record = scenario.record_with_opening_cards(vec![
            scenario.defense_cards[0],
            scenario.defense_cards[1],
            scenario.card(Element::Metal, 2),
            scenario.card(Element::Fire, 2),
            scenario.first_attack_card,
            scenario.defended_attack_card,
            scenario.card(Element::Water, 1),
            scenario.card(Element::Fire, 1),
            scenario.card(Element::Wood, 3),
        ]);
        scenario
    }

    fn with_wood_star_substitution() -> Self {
        Self::new()
    }

    fn new() -> Self {
        let owner = PlayerId::new("p1");
        let attacker = PlayerId::new("p2");
        let setup = OfficialRules::new()
            .configure_game(
                vec![
                    Player {
                        id: owner.clone(),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: attacker.clone(),
                        team: TeamId::new("team:p2"),
                    },
                ],
                vec![owner.clone(), attacker.clone()],
                vec![RuleModuleId::new(STAR_MODULE_ID)],
            )
            .unwrap();

        let summoning_cards = vec![
            card_from_setup(&setup, Element::Wood, 3),
            card_from_setup(&setup, Element::Wood, 4),
            card_from_setup(&setup, Element::Wood, 5),
        ];
        let wood = card_from_setup(&setup, Element::Wood, 1);
        let substituted_card = card_from_setup(&setup, Element::Water, 2);
        let defense_cards = vec![wood, substituted_card];
        let first_attack_card = card_from_setup(&setup, Element::Metal, 1);
        let defended_attack_card = card_from_setup(&setup, Element::Earth, 1);

        // P1 的前四張卡牌允許符合資格的召喚；P2 的五張卡牌與後續水 2 只提供
        // 合法回合行動及後續選取的替代。狀態中沒有注入星、被動或結果。
        let opening = vec![
            summoning_cards[0],
            summoning_cards[1],
            summoning_cards[2],
            wood,
            first_attack_card,
            defended_attack_card,
            card_from_setup(&setup, Element::Water, 1),
            card_from_setup(&setup, Element::Fire, 1),
            card_from_setup(&setup, Element::Wood, 2),
            substituted_card,
        ];
        let record =
            GameRecord::start(setup.clone(), deck_starting_with(&setup, &opening)).unwrap();
        Self {
            record,
            owner,
            attacker,
            summoning_cards,
            defense_cards,
            substituted_card,
            first_attack_card,
            defended_attack_card,
        }
    }

    fn record_with_opening_cards(&self, opening: Vec<CardInstanceId>) -> GameRecord {
        GameRecord::start(
            self.record.setup().clone(),
            deck_starting_with(self.record.setup(), &opening),
        )
        .unwrap()
    }

    fn card(&self, element: Element, level: u32) -> CardInstanceId {
        card_from_setup(self.record.setup(), element, level)
    }

    fn advance_to_main(&mut self, expected: &PlayerId) {
        self.record.advance_automatic().unwrap();
        assert_eq!(self.record.state().current_player(), Some(expected));
    }

    fn finish_turn(&mut self, player: &PlayerId) {
        self.record.advance_automatic().unwrap();
        let choice = self
            .record
            .state()
            .pending_choice
            .as_ref()
            .expect("legal Formation use must reach its Turn Draw discard choice");
        let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
            panic!("Turn Draw must create its canonical Card choice");
        };
        self.record
            .handle(Command::AnswerChoice {
                player: player.clone(),
                choice_id: choice.choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![*cards.last().unwrap()],
                },
            })
            .unwrap();
        self.record.advance_automatic().unwrap();
    }

    fn perform(
        &mut self,
        player: &PlayerId,
        formation_id: &str,
        cards: Vec<CardInstanceId>,
        declared_targets: Vec<TargetDecl>,
    ) -> Vec<GameEvent> {
        self.record
            .handle(Command::PerformFormation {
                player: player.clone(),
                formation_id: formation_id.to_string(),
                cards,
                declared_targets,
            })
            .unwrap()
    }

    fn assert_replay(&self) {
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

#[test]
fn owned_star_defense_matrix_records_selected_substitution_and_preserves_star_lifecycle() {
    // 基準：普通兩張木防禦在沒有星時合法，且不記錄替代。其既有的被動生命週期
    // 與擁有星的修飾是分開的證據。
    let mut baseline = StarDefenseScenario::ordinary_defense();
    let owner = baseline.owner.clone();
    let attacker = baseline.attacker.clone();
    baseline.advance_to_main(&owner);
    let baseline_actions = OfficialRules::new()
        .playable_actions(baseline.record.state(), &owner, &baseline.defense_cards)
        .unwrap();
    assert!(baseline_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "defense" && candidate.star_substitution.is_none()
    )));
    let ordinary_cover = baseline.perform(
        &owner,
        "defense",
        baseline.defense_cards.clone(),
        Vec::new(),
    );
    assert_cover_commitment(&ordinary_cover, &owner, &baseline.defense_cards, None);
    assert!(baseline.record.state().team_stars.is_empty());
    baseline.finish_turn(&owner);
    let ordinary_attack = baseline.perform(
        &attacker,
        "metal-strike",
        vec![baseline.first_attack_card],
        Vec::new(),
    );
    assert_defense_flip_and_attack(
        &ordinary_attack,
        &owner,
        &attacker,
        &baseline.defense_cards,
        baseline.first_attack_card,
        Element::Metal,
        2,
        200,
    );
    assert!(baseline.record.state().covered_passive(&owner).is_none());
    baseline.assert_replay();

    // 修飾：P1 先透過符合資格的三木陣形合法召喚木星，之後星才能讓水 2 在防禦
    // 中算作木。
    let mut interaction = StarDefenseScenario::with_wood_star_substitution();
    let owner = interaction.owner.clone();
    let attacker = interaction.attacker.clone();
    interaction.advance_to_main(&owner);
    let summon_events = interaction.perform(
        &owner,
        "triple-wood",
        interaction.summoning_cards.clone(),
        Vec::new(),
    );
    assert!(matches!(
        summon_events.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, star_substitution: None, .. },
            GameEvent::AttackResolved { attacker, formation_id: attack_formation, point_breakdown, .. },
            GameEvent::StarSummoned { player: summoner, team, star: StarKind::Wood },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &owner
            && formation_id == "triple-wood"
            && cards == &interaction.summoning_cards
            && attacker == &owner
            && attack_formation == "triple-wood"
            && point_breakdown.base_points == 36
            && summoner == &owner
            && team == &TeamId::new("team:p1")
            && discarded_by == &owner
            && discarded == "triple-wood"
            && discarded_cards == &interaction.summoning_cards
    ));
    assert_eq!(
        interaction
            .record
            .state()
            .star_for_team(&TeamId::new("team:p1")),
        Some(StarKind::Wood)
    );
    assert_eq!(
        interaction
            .record
            .public_view(Viewer::Observer)
            .unwrap()
            .team_stars,
        vec![TeamStar {
            team: TeamId::new("team:p1"),
            star: StarKind::Wood,
        }]
    );
    interaction.finish_turn(&owner);
    interaction.advance_to_main(&attacker);

    // P2 的合法 Metal Strike 推進正常回合歷史；它不會改變 P1 的星，也不會捏造
    // 後續被動。
    let first_attack = interaction.perform(
        &attacker,
        "metal-strike",
        vec![interaction.first_attack_card],
        Vec::new(),
    );
    assert!(first_attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { point_breakdown, hp_change, .. }
            if point_breakdown == &AttackPointBreakdown {
                base_points: 5,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::Overcoming,
                damage_transform: DamageTransform::DoubleDamage,
                final_amount: 10,
            }
            && hp_change == &HpChangeDelta {
                team: TeamId::new("team:p1"),
                old_hp: 200,
                delta: -10,
                new_hp: 190,
                effective_delta: -10,
            }
    )));
    interaction.finish_turn(&attacker);
    interaction.advance_to_main(&owner);

    let substitution = StarElementSubstitution {
        card: interaction.substituted_card,
        printed_element: Element::Water,
        interpreted_element: Element::Wood,
    };
    let star_actions = OfficialRules::new()
        .playable_actions(
            interaction.record.state(),
            &owner,
            &interaction.defense_cards,
        )
        .unwrap();
    assert!(star_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "defense"
                && candidate.star_substitution.as_ref() == Some(&substitution)
    )));
    assert!(!star_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "defense" && candidate.star_substitution.is_none()
    )));

    let cover_events = interaction.perform(
        &owner,
        "defense",
        interaction.defense_cards.clone(),
        vec![TargetDecl::Card(interaction.substituted_card)],
    );
    assert_cover_commitment(
        &cover_events,
        &owner,
        &interaction.defense_cards,
        Some(&substitution),
    );
    assert_eq!(
        interaction
            .record
            .state()
            .covered_passive(&owner)
            .unwrap()
            .star_substitution,
        Some(substitution.clone())
    );
    assert!(matches!(
        interaction
            .record
            .public_view(Viewer::Player(owner.clone()))
            .unwrap()
            .covered_passives
            .as_slice(),
        [PublicCoveredPassive {
            owner: passive_owner,
            formation_id: Some(formation_id),
            cards: PublicCardRefs::Known(cards),
            star_substitution: Some(actual),
        }] if passive_owner == &owner
            && formation_id == "defense"
            && cards == &interaction.defense_cards
            && actual == &substitution
    ));
    assert!(matches!(
        interaction
            .record
            .public_view(Viewer::Player(attacker.clone()))
            .unwrap()
            .covered_passives
            .as_slice(),
        [PublicCoveredPassive {
            owner: passive_owner,
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }] if passive_owner == &owner
    ));
    interaction.finish_turn(&owner);
    interaction.advance_to_main(&attacker);

    // 互動：正常合法攻擊會翻開並消耗防禦，但基礎規則集的替代永遠不會消耗授權
    // 它的木星。
    let defended_attack = interaction.perform(
        &attacker,
        "earth-strike",
        vec![interaction.defended_attack_card],
        Vec::new(),
    );
    assert_defense_flip_and_attack(
        &defended_attack,
        &owner,
        &attacker,
        &interaction.defense_cards,
        interaction.defended_attack_card,
        Element::Earth,
        4,
        190,
    );
    assert!(
        !defended_attack
            .iter()
            .any(|event| matches!(event, GameEvent::StarBroken { .. }))
    );
    assert_eq!(
        interaction
            .record
            .state()
            .star_for_team(&TeamId::new("team:p1")),
        Some(StarKind::Wood)
    );
    assert!(interaction.record.state().covered_passive(&owner).is_none());
    for card in interaction
        .summoning_cards
        .iter()
        .copied()
        .chain(interaction.defense_cards.iter().copied())
        .chain([
            interaction.first_attack_card,
            interaction.defended_attack_card,
        ])
    {
        assert!(interaction.record.state().discard.contains(&card));
    }
    interaction.assert_replay();
}

#[test]
fn three_card_star_formation_defense_matrix_prevents_damage_but_keeps_draw_and_star_consumption() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let p3 = PlayerId::new("p3");
    let p4 = PlayerId::new("p4");
    let metal_team = TeamId::new("team:metal");
    let water_team = TeamId::new("team:water");
    let setup = OfficialRules::new()
        .configure_game(
            vec![
                Player {
                    id: p1.clone(),
                    team: metal_team.clone(),
                },
                Player {
                    id: p2.clone(),
                    team: water_team.clone(),
                },
                Player {
                    id: p3.clone(),
                    team: metal_team.clone(),
                },
                Player {
                    id: p4.clone(),
                    team: water_team.clone(),
                },
            ],
            vec![p1.clone(), p2.clone(), p3.clone(), p4.clone()],
            vec![RuleModuleId::new(STAR_MODULE_ID)],
        )
        .unwrap();
    let metal_one = card_from_setup(&setup, Element::Metal, 1);
    let metal_two = card_from_setup(&setup, Element::Metal, 2);
    let metal_three = card_from_setup(&setup, Element::Metal, 3);
    let metal_four = card_from_setup(&setup, Element::Metal, 4);
    let metal_five = card_from_setup(&setup, Element::Metal, 5);
    let earth_four = card_from_setup(&setup, Element::Earth, 4);
    let wood_one = card_from_setup(&setup, Element::Wood, 1);
    let wood_two = card_from_setup(&setup, Element::Wood, 2);

    // 基準：固定的合法輸入在目前玩家手中，但在召喚金星前該行動不存在。
    let baseline_opening = vec![
        metal_one,
        metal_two,
        earth_four,
        card_from_setup(&setup, Element::Fire, 2),
    ];
    let mut baseline =
        GameRecord::start(setup.clone(), deck_starting_with(&setup, &baseline_opening)).unwrap();
    baseline.advance_automatic().unwrap();
    let taibai_cards = vec![metal_one, metal_two, earth_four];
    let without_star = baseline.playable_actions(&p1, &taibai_cards).unwrap();
    assert!(!without_star.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "taibai-heaven-forging"
    )));

    let opening = vec![
        metal_three,
        metal_four,
        metal_five,
        card_from_setup(&setup, Element::Wood, 4),
        wood_one,
        wood_two,
        card_from_setup(&setup, Element::Water, 1),
        card_from_setup(&setup, Element::Fire, 1),
        card_from_setup(&setup, Element::Earth, 1),
        metal_two,
        earth_four,
        metal_one,
        card_from_setup(&setup, Element::Wood, 3),
        card_from_setup(&setup, Element::Fire, 2),
        card_from_setup(&setup, Element::Water, 2),
        card_from_setup(&setup, Element::Fire, 3),
        card_from_setup(&setup, Element::Earth, 3),
        card_from_setup(&setup, Element::Wood, 5),
        card_from_setup(&setup, Element::Earth, 5),
    ];
    let mut record =
        GameRecord::start(setup.clone(), deck_starting_with(&setup, &opening)).unwrap();
    record.advance_automatic().unwrap();

    // 修飾：P1 合法召喚隊伍擁有的金星，並提交/棄置其來源卡牌。P3 會消耗同一顆
    // 共用的星。
    let summon_cards = vec![metal_three, metal_four, metal_five];
    let summoned = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "triple-metal".to_string(),
            cards: summon_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        summoned.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::AttackResolved { attacker, formation_id: attack_formation, point_breakdown, .. },
            GameEvent::StarSummoned { player: summoner, team, star: StarKind::Metal },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "triple-metal"
            && cards == &summon_cards
            && attacker == &p1
            && attack_formation == "triple-metal"
            && point_breakdown.base_points == 36
            && summoner == &p1
            && team == &metal_team
            && discarded_by == &p1
            && discarded == "triple-metal"
            && discarded_cards == &summon_cards
    ));
    assert_eq!(
        record.state().star_for_team(&metal_team),
        Some(StarKind::Metal)
    );
    assert_eq!(
        record.public_view(Viewer::Observer).unwrap().team_stars,
        vec![TeamStar {
            team: metal_team.clone(),
            star: StarKind::Metal,
        }]
    );
    finish_turn_for_star_matrix(&mut record, &p1);

    // P2 透過合法防禦命令建立修飾，而不是直接設定覆蓋被動。
    let defense = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "defense".to_string(),
            cards: vec![wood_one, wood_two],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, .. },
        ] if player == &p2
            && formation_id == "defense"
            && cards == &vec![wood_one, wood_two]
            && covered_by == &p2
            && covered == "defense"
            && covered_cards == &vec![wood_one, wood_two]
    ));
    assert!(matches!(
        record.public_view(Viewer::Player(p1.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: None, cards: PublicCardRefs::Hidden { count: 2 }, star_substitution: None }]
            if owner == &p2
    ));
    finish_turn_for_star_matrix(&mut record, &p2);
    assert_eq!(record.state().current_player(), Some(&p3));

    // 互動：P3 自己合法使用三張卡牌時，可以消耗 P1 為共用隊伍召喚的金星。防禦
    // 只防止受影響的傷害；Taibai 獨立的抽牌獎勵與星的消耗仍然保留。
    let with_star = record.playable_actions(&p3, &taibai_cards).unwrap();
    assert!(with_star.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "taibai-heaven-forging" && candidate.cards == taibai_cards
    )));
    let forged = record
        .handle(Command::PerformFormation {
            player: p3.clone(),
            formation_id: "taibai-heaven-forging".to_string(),
            cards: taibai_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(forged.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            passive_id,
            outcome: PassiveFlipOutcome::Applied { modifications, .. },
            ..
        } if owner == &p2
            && passive_id == "defense"
            && modifications == &vec![ActionModification::PreventDamage]
    )));
    assert!(forged.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            attacker,
            formation_id,
            point_breakdown: AttackPointBreakdown { base_points: 21, final_amount: 21, .. },
            hp_change: HpChangeDelta { team, delta: 0, effective_delta: 0, .. },
            elemental_context_update: Some(AttackResolutionEffects { turn_draw_bonus_changes, .. }),
            ..
        } if attacker == &p3
            && formation_id == "taibai-heaven-forging"
            && team == &water_team
            && turn_draw_bonus_changes
                == &vec![fewfc::domain::TurnDrawBonusDelta {
                    player: p3.clone(),
                    old_value: 0,
                    delta: 1,
                    new_value: 1,
                }]
    )));
    assert!(forged.iter().any(|event| matches!(
        event,
        GameEvent::StarBroken {
            team,
            star: StarKind::Metal,
            reason: StarBreakReason::StarFormationUsed { formation_id },
            hp_change: None,
        } if team == &metal_team && formation_id == "taibai-heaven-forging"
    )));
    assert!(forged.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p3 && formation_id == "taibai-heaven-forging" && cards == &taibai_cards
    )));
    assert!(record.state().covered_passive(&p2).is_none());
    assert!(record.state().team_stars.is_empty());
    assert_eq!(record.state().turn_draw_bonus_by_player.get(&p3), Some(&1));
    for card in taibai_cards.iter().chain([wood_one, wood_two].iter()) {
        assert!(record.state().discard.contains(card));
    }
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

fn finish_turn_for_star_matrix(record: &mut GameRecord, player: &PlayerId) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("a legal Formation must reach the canonical Turn Draw choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("Turn Draw must request a Card discard");
    };
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id: choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![*cards.last().unwrap()],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn assert_cover_commitment(
    events: &[GameEvent],
    owner: &PlayerId,
    cards: &[CardInstanceId],
    substitution: Option<&StarElementSubstitution>,
) {
    assert!(matches!(
        events,
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards: committed_cards,
                star_substitution: committed_substitution,
                state: FormationAreaState::FaceDownResolving,
            },
            GameEvent::PassiveCovered {
                player: covered_by,
                formation_id: covered,
                cards: covered_cards,
                star_substitution: covered_substitution,
                sealed: false,
            },
        ] if player == owner
            && formation_id == "defense"
            && committed_cards == cards
            && committed_substitution.as_ref() == substitution
            && covered_by == owner
            && covered == "defense"
            && covered_cards == cards
            && covered_substitution.as_ref() == substitution
    ));
}

fn assert_defense_flip_and_attack(
    events: &[GameEvent],
    owner: &PlayerId,
    attacker: &PlayerId,
    defense_cards: &[CardInstanceId],
    attack_card: CardInstanceId,
    attack_element: Element,
    resolved_turn: u64,
    old_hp: i32,
) {
    assert!(matches!(
        events,
        [
            GameEvent::FormationCommitted {
                player: committed_by,
                formation_id,
                cards,
                state: FormationAreaState::FaceUpResolving,
                ..
            },
            GameEvent::PassiveFlipped {
                owner: passive_owner,
                incoming_player,
                passive_id,
                cards: flipped_cards,
                outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
            },
            GameEvent::AttackResolved {
                attacker: resolved_by,
                target,
                formation_id: attack_formation,
                used_cards,
                point_breakdown,
                hp_change,
                shield_change: None,
                card_moves,
                elemental_context_update,
            },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded,
                cards: discarded_cards,
            },
        ] if committed_by == attacker
            && formation_id == match attack_element {
                Element::Metal => "metal-strike",
                Element::Wood => "wood-strike",
                Element::Water => "water-strike",
                Element::Fire => "fire-strike",
                Element::Earth => "earth-strike",
            }
            && cards == &vec![attack_card]
            && passive_owner == owner
            && incoming_player == attacker
            && passive_id == "defense"
            && flipped_cards == defense_cards
            && effect_id == "defense"
            && modifications == &vec![ActionModification::PreventDamage]
            && resolved_by == attacker
            && target == owner
            && attack_formation == formation_id
            && used_cards == &vec![attack_card]
            && point_breakdown == &AttackPointBreakdown {
                base_points: 5,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 5,
            }
            && hp_change == &HpChangeDelta {
                team: TeamId::new("team:p1"),
                old_hp,
                delta: 0,
                new_hp: old_hp,
                effective_delta: 0,
            }
            && card_moves.is_empty()
            && elemental_context_update == &Some(AttackResolutionEffects {
                outcome: AttackOutcome::DamagePrevented,
                elemental_context_update: Some(LastElementalAttackUpdate {
                    player: attacker.clone(),
                    attack: LastElementalAttack {
                        element: attack_element,
                        resolved_turn,
                    },
                }),
                ..AttackResolutionEffects::default()
            })
            && discarded_by == attacker
            && discarded == attack_formation
            && discarded_cards == &vec![attack_card]
    ));
}

fn card_from_setup(
    setup: &fewfc::domain::GameSetup,
    element: Element,
    level: u32,
) -> CardInstanceId {
    setup
        .card_instances
        .iter()
        .find_map(|instance| {
            setup
                .card_defs
                .iter()
                .find(|definition| definition.id == instance.definition)
                .filter(|definition| {
                    definition.element == element && definition.level.value() == level
                })
                .map(|_| instance.instance)
        })
        .expect("official deck contains the requested fixture Card")
}

fn deck_starting_with(
    setup: &fewfc::domain::GameSetup,
    first: &[CardInstanceId],
) -> Vec<CardInstanceId> {
    let mut deck = first.to_vec();
    deck.extend(
        OfficialRules::new()
            .official_deck_order(setup)
            .unwrap()
            .into_iter()
            .filter(|card| !first.contains(card)),
    );
    deck
}
