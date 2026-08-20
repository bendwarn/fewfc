use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, CardMoveDelta, CardZone, ChoiceAnswer, Command, Element, GameEvent,
    HERO_SCHOOLS_MODULE_ID, Player, PlayerId, ProfessionId, RuleModuleId, TeamId,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};

struct WarriorDefenseScenario {
    record: GameRecord,
    p1: PlayerId,
    p2: PlayerId,
    warrior_cards: Vec<CardInstanceId>,
    defense_cards: Vec<CardInstanceId>,
}

impl WarriorDefenseScenario {
    fn new() -> Self {
        let p1 = PlayerId::new("p1");
        let p2 = PlayerId::new("p2");
        let setup = OfficialRules::new()
            .configure_game(
                vec![
                    Player {
                        id: p1.clone(),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: p2.clone(),
                        team: TeamId::new("team:p2"),
                    },
                ],
                vec![p1.clone(), p2.clone()],
                vec![RuleModuleId::new(HERO_SCHOOLS_MODULE_ID)],
            )
            .unwrap();
        let mut remaining = OfficialRules::new().official_deck_order(&setup).unwrap();
        let warrior_cards = vec![
            take_card(&setup, &mut remaining, Element::Metal, 1),
            take_card(&setup, &mut remaining, Element::Metal, 2),
        ];
        let defense_cards = vec![
            take_card(&setup, &mut remaining, Element::Wood, 1),
            take_card(&setup, &mut remaining, Element::Metal, 3),
        ];
        // P2 的五張起手卡是無關的固定背景；後續元素行動透過合法命令建立觸發時機。
        let mut deck = warrior_cards
            .iter()
            .chain(defense_cards.iter())
            .copied()
            .collect::<Vec<_>>();
        for element in [
            Element::Fire,
            Element::Water,
            Element::Earth,
            Element::Metal,
            Element::Wood,
        ] {
            deck.push(take_card(&setup, &mut remaining, element, 1));
        }
        deck.extend(remaining);
        let record = GameRecord::start(setup, deck).unwrap();
        Self {
            record,
            p1,
            p2,
            warrior_cards,
            defense_cards,
        }
    }

    fn advance_to_first_action(&mut self) {
        self.record.advance_automatic().unwrap();
        assert_eq!(self.record.state().current_player(), Some(&self.p1));
    }

    fn change_to_warrior(&mut self) -> Vec<GameEvent> {
        self.record
            .handle(Command::ChangeProfession {
                player: self.p1.clone(),
                profession: ProfessionId::new("warrior"),
                cards: self.warrior_cards.clone(),
            })
            .unwrap()
    }

    fn finish_turn(&mut self, player: &PlayerId) {
        self.record.advance_automatic().unwrap();
        let choice = self
            .record
            .state()
            .pending_choice
            .as_ref()
            .expect("each legal action must reach a Turn Draw discard choice");
        let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
            panic!("Turn Draw must expose its canonical Card choice");
        };
        self.record
            .handle(Command::AnswerChoice {
                player: player.clone(),
                choice_id: choice.choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![cards[0]],
                },
            })
            .unwrap();
        self.record.advance_automatic().unwrap();
    }

    fn perform_first_elemental_attack(&mut self, player: &PlayerId) -> Vec<GameEvent> {
        let card = self.record.state().hand(player).unwrap()[0];
        let formation_id = match self.record.state().card_element(card).unwrap() {
            Element::Metal => "metal-strike",
            Element::Wood => "wood-strike",
            Element::Water => "water-strike",
            Element::Fire => "fire-strike",
            Element::Earth => "earth-strike",
        };
        self.record
            .handle(Command::PerformFormation {
                player: player.clone(),
                formation_id: formation_id.to_string(),
                cards: vec![card],
                declared_targets: Vec::new(),
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

fn take_card(
    setup: &fewfc::domain::GameSetup,
    remaining: &mut Vec<CardInstanceId>,
    element: Element,
    level: u32,
) -> CardInstanceId {
    let position = remaining
        .iter()
        .position(|card| {
            let instance = setup
                .card_instances
                .iter()
                .find(|instance| instance.instance == *card)
                .unwrap();
            let definition = setup
                .card_defs
                .iter()
                .find(|definition| definition.id == instance.definition)
                .unwrap();
            definition.element == element && definition.level.value() == level
        })
        .unwrap();
    remaining.remove(position)
}

fn finish_turn_discarding(record: &mut GameRecord, player: &PlayerId, discard: CardInstanceId) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("a legal action must enter its canonical Turn Draw choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("Turn Draw must request a Card discard");
    };
    assert!(cards.contains(&discard));
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id: choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn elemental_formation_id(element: Element) -> &'static str {
    match element {
        Element::Metal => "metal-strike",
        Element::Wood => "wood-strike",
        Element::Water => "water-strike",
        Element::Fire => "fire-strike",
        Element::Earth => "earth-strike",
    }
}

#[test]
fn warrior_defense_proficiency_matrix_establishes_and_consumes_defense_without_empty_city_fallback()
{
    // 基準：確切的木加非木選擇不是基本防禦。在 Warrior 轉換前，它合法解析的
    // 只有 Empty City。
    let mut baseline = WarriorDefenseScenario::new();
    baseline.advance_to_first_action();
    let baseline_actions = baseline
        .record
        .playable_actions(&baseline.p1, &baseline.defense_cards)
        .unwrap();
    assert!(!baseline_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "defense"
    )));
    assert!(baseline_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "empty-city"
    )));
    let baseline_events = baseline
        .record
        .handle(Command::PerformFormation {
            player: baseline.p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: baseline.defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveCovered { formation_id, cards, .. }
            if formation_id == "empty-city" && cards == &baseline.defense_cards
    )));
    baseline.assert_replay();

    // 修飾：Warrior 透過自己的合法全金職業命令取得；後續木加非木組合現在會
    // 提供 Defense，不再退回 Empty City。
    let mut interaction = WarriorDefenseScenario::new();
    interaction.advance_to_first_action();
    let warrior_events = interaction.change_to_warrior();
    assert!(matches!(
        warrior_events.as_slice(),
        [
            GameEvent::ActionStarted { player: started },
            GameEvent::ProfessionChanged { player, previous: None, profession, card_moves },
        ] if started == &interaction.p1
            && player == &interaction.p1
                && profession == &ProfessionId::new("warrior")
                && card_moves.len() == interaction.warrior_cards.len()
    ));
    assert_eq!(
        interaction.record.state().profession_for(&interaction.p1),
        Some(&ProfessionId::new("warrior"))
    );
    let p1 = interaction.p1.clone();
    interaction.finish_turn(&p1);
    assert_eq!(
        interaction.record.state().current_player(),
        Some(&interaction.p2)
    );
    let p2 = interaction.p2.clone();
    interaction.perform_first_elemental_attack(&p2);
    interaction.finish_turn(&p2);
    assert_eq!(
        interaction.record.state().current_player(),
        Some(&interaction.p1)
    );

    assert!(interaction.defense_cards.iter().all(|card| {
        interaction
            .record
            .state()
            .hand(&interaction.p1)
            .unwrap()
            .contains(card)
    }));
    let warrior_actions = interaction
        .record
        .playable_actions(&interaction.p1, &interaction.defense_cards)
        .unwrap();
    assert!(warrior_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "defense"
    )));
    assert!(!warrior_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate) if candidate.formation_id == "empty-city"
    )));
    let defense_events = interaction
        .record
        .handle(Command::PerformFormation {
            player: interaction.p1.clone(),
            formation_id: "defense".to_string(),
            cards: interaction.defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense_events.as_slice(),
        [
            GameEvent::FormationCommitted { formation_id, cards, .. },
            GameEvent::PassiveCovered { player, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if formation_id == "defense"
            && cards == &interaction.defense_cards
            && player == &interaction.p1
            && covered == "defense"
            && covered_cards == &interaction.defense_cards
    ));
    interaction.finish_turn(&p1);
    assert_eq!(
        interaction.record.state().current_player(),
        Some(&interaction.p2)
    );

    // 互動：P2 下一次真正的攻擊會翻開並消耗熟練防禦，只防止生命值損失，仍會
    // 提交/棄置 P2 的卡牌。
    let hp_before = interaction
        .record
        .state()
        .hp
        .iter()
        .find(|entry| entry.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let attack_events = interaction.perform_first_elemental_attack(&p2);
    assert!(attack_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            passive_id,
            outcome: fewfc::domain::PassiveFlipOutcome::Applied { modifications, .. },
            ..
        } if owner == &interaction.p1
            && passive_id == "defense"
            && modifications == &vec![fewfc::domain::ActionModification::PreventDamage]
    )));
    assert!(attack_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change, .. }
            if hp_change.delta == 0 && hp_change.effective_delta == 0
    )));
    assert!(
        attack_events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCommitted { .. }))
    );
    assert!(
        attack_events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationCardsDiscarded { .. }))
    );
    assert_eq!(
        interaction
            .record
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        hp_before
    );
    assert!(
        interaction
            .record
            .state()
            .covered_passive(&interaction.p1)
            .is_none()
    );
    assert!(
        interaction.defense_cards.iter().all(|card| interaction
            .record
            .state()
            .discard
            .contains(card))
    );
    interaction.assert_replay();
}

#[test]
fn metal_profession_ladder_matrix_changes_warrior_to_war_god_to_hero_through_legal_turns() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let setup = OfficialRules::new()
        .configure_game(
            vec![
                Player {
                    id: p1.clone(),
                    team: TeamId::new("team:p1"),
                },
                Player {
                    id: p2.clone(),
                    team: TeamId::new("team:p2"),
                },
            ],
            vec![p1.clone(), p2.clone()],
            vec![RuleModuleId::new(HERO_SCHOOLS_MODULE_ID)],
        )
        .unwrap();
    let mut remaining = OfficialRules::new().official_deck_order(&setup).unwrap();

    let warrior_cards = vec![
        take_card(&setup, &mut remaining, Element::Metal, 1),
        take_card(&setup, &mut remaining, Element::Metal, 2),
    ];
    let retained_metal_four = take_card(&setup, &mut remaining, Element::Metal, 4);
    let war_god_metal_five = take_card(&setup, &mut remaining, Element::Metal, 5);
    let p2_opening = [
        Element::Wood,
        Element::Water,
        Element::Fire,
        Element::Earth,
        Element::Metal,
    ]
    .into_iter()
    .map(|element| take_card(&setup, &mut remaining, element, 1))
    .collect::<Vec<_>>();
    let war_god_metal_one = take_card(&setup, &mut remaining, Element::Metal, 1);
    let first_turn_discard = take_card(&setup, &mut remaining, Element::Wood, 2);
    let retained_first_draw = take_card(&setup, &mut remaining, Element::Water, 2);
    let p2_first_draw = [Element::Fire, Element::Earth, Element::Wood]
        .into_iter()
        .map(|element| take_card(&setup, &mut remaining, element, 2))
        .collect::<Vec<_>>();
    let hero_metal_five = take_card(&setup, &mut remaining, Element::Metal, 5);
    let second_turn_discard = take_card(&setup, &mut remaining, Element::Fire, 3);
    let retained_second_draw = take_card(&setup, &mut remaining, Element::Earth, 3);
    let p2_second_draw = [Element::Wood, Element::Water, Element::Fire]
        .into_iter()
        .map(|element| take_card(&setup, &mut remaining, element, 3))
        .collect::<Vec<_>>();
    let mut deck = warrior_cards
        .iter()
        .copied()
        .chain([retained_metal_four, war_god_metal_five])
        .chain(p2_opening.iter().copied())
        .chain([war_god_metal_one, first_turn_discard, retained_first_draw])
        .chain(p2_first_draw.iter().copied())
        .chain([hero_metal_five, second_turn_discard, retained_second_draw])
        .chain(p2_second_draw.iter().copied())
        .collect::<Vec<_>>();
    deck.extend(remaining);
    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();

    // 基準：第一次有效轉換前不存在職業狀態。
    assert!(record.state().professions.is_empty());
    assert!(
        state_for(record.state(), Viewer::Observer)
            .professions
            .is_empty()
    );

    let warrior = record
        .handle(Command::ChangeProfession {
            player: p1.clone(),
            profession: ProfessionId::new("warrior"),
            cards: warrior_cards.clone(),
        })
        .unwrap();
    assert!(matches!(
        warrior.as_slice(),
        [
            GameEvent::ActionStarted { player: started },
            GameEvent::ProfessionChanged { player, previous: None, profession, card_moves },
        ] if started == &p1
            && player == &p1
            && profession == &ProfessionId::new("warrior")
            && card_moves == &warrior_cards.iter().copied().map(|card| CardMoveDelta {
                card,
                from: CardZone::Hand(p1.clone()),
                to: CardZone::Discard,
            }).collect::<Vec<_>>()
    ));
    assert_eq!(
        record.state().profession_for(&p1),
        Some(&ProfessionId::new("warrior"))
    );
    finish_turn_discarding(&mut record, &p1, first_turn_discard);

    let p2_first_action = p2_opening[0];
    record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: elemental_formation_id(
                record.state().card_element(p2_first_action).unwrap(),
            )
            .to_string(),
            cards: vec![p2_first_action],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn_discarding(&mut record, &p2, p2_first_draw[0]);

    // 修飾與互動：前置條件與卡牌總和由合法命令檢查。狀態中沒有直接安排職業、
    // 行動可用性或歷史。
    let war_god_cards = vec![war_god_metal_one, war_god_metal_five];
    assert!(
        record
            .state()
            .hand(&p1)
            .unwrap()
            .contains(&war_god_metal_one)
    );
    assert!(
        record
            .state()
            .hand(&p1)
            .unwrap()
            .contains(&war_god_metal_five)
    );
    let war_god = record
        .handle(Command::ChangeProfession {
            player: p1.clone(),
            profession: ProfessionId::new("war-god"),
            cards: war_god_cards.clone(),
        })
        .unwrap();
    assert!(matches!(
        war_god.as_slice(),
        [
            GameEvent::ActionStarted { player: started },
            GameEvent::ProfessionChanged { player, previous: Some(previous), profession, card_moves },
        ] if started == &p1
            && player == &p1
            && previous == &ProfessionId::new("warrior")
            && profession == &ProfessionId::new("war-god")
            && card_moves == &war_god_cards.iter().copied().map(|card| CardMoveDelta {
                card,
                from: CardZone::Hand(p1.clone()),
                to: CardZone::Discard,
            }).collect::<Vec<_>>()
    ));
    assert_eq!(
        record.state().profession_for(&p1),
        Some(&ProfessionId::new("war-god"))
    );
    finish_turn_discarding(&mut record, &p1, second_turn_discard);

    let p2_second_action = record.state().hand(&p2).unwrap()[0];
    record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: elemental_formation_id(
                record.state().card_element(p2_second_action).unwrap(),
            )
            .to_string(),
            cards: vec![p2_second_action],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn_discarding(&mut record, &p2, p2_second_draw[0]);

    let hero_cards = vec![retained_metal_four, hero_metal_five];
    assert!(
        record
            .state()
            .hand(&p1)
            .unwrap()
            .contains(&retained_metal_four)
    );
    assert!(record.state().hand(&p1).unwrap().contains(&hero_metal_five));
    let hero = record
        .handle(Command::ChangeProfession {
            player: p1.clone(),
            profession: ProfessionId::new("hero"),
            cards: hero_cards.clone(),
        })
        .unwrap();
    assert!(matches!(
        hero.as_slice(),
        [
            GameEvent::ActionStarted { player: started },
            GameEvent::ProfessionChanged { player, previous: Some(previous), profession, card_moves },
        ] if started == &p1
            && player == &p1
            && previous == &ProfessionId::new("war-god")
            && profession == &ProfessionId::new("hero")
            && card_moves == &hero_cards.iter().copied().map(|card| CardMoveDelta {
                card,
                from: CardZone::Hand(p1.clone()),
                to: CardZone::Discard,
            }).collect::<Vec<_>>()
    ));
    assert_eq!(
        record.state().profession_for(&p1),
        Some(&ProfessionId::new("hero"))
    );
    assert_eq!(
        state_for(record.state(), Viewer::Observer)
            .professions
            .iter()
            .find(|owned| owned.player == p1)
            .map(|owned| owned.profession.clone()),
        Some(ProfessionId::new("hero"))
    );
    for card in warrior_cards
        .iter()
        .chain(war_god_cards.iter())
        .chain(hero_cards.iter())
    {
        assert!(record.state().discard.contains(card));
    }
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn war_god_weapon_mastery_matrix_keeps_weapon_identity_and_adds_turn_draw_through_legal_ladder() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let setup = OfficialRules::new()
        .configure_game(
            vec![
                Player {
                    id: p1.clone(),
                    team: TeamId::new("team:p1"),
                },
                Player {
                    id: p2.clone(),
                    team: TeamId::new("team:p2"),
                },
            ],
            vec![p1.clone(), p2.clone()],
            vec![RuleModuleId::new(HERO_SCHOOLS_MODULE_ID)],
        )
        .unwrap();

    // 基準：普通 Weapon 保有正常識別，沒有 Hero 回合抽牌修飾。這些卡牌是固定
    // 背景，不是職業固定資料。
    let mut baseline_remaining = OfficialRules::new().official_deck_order(&setup).unwrap();
    let baseline_weapon_cards = vec![
        take_card(&setup, &mut baseline_remaining, Element::Metal, 1),
        take_card(&setup, &mut baseline_remaining, Element::Metal, 2),
    ];
    let mut baseline_opening = baseline_weapon_cards.clone();
    baseline_opening.extend([
        take_card(&setup, &mut baseline_remaining, Element::Wood, 1),
        take_card(&setup, &mut baseline_remaining, Element::Fire, 1),
    ]);
    for element in [
        Element::Wood,
        Element::Water,
        Element::Fire,
        Element::Earth,
        Element::Metal,
    ] {
        baseline_opening.push(take_card(&setup, &mut baseline_remaining, element, 1));
    }
    baseline_opening.extend(baseline_remaining);
    let mut baseline = GameRecord::start(setup.clone(), baseline_opening).unwrap();
    baseline.advance_automatic().unwrap();
    let baseline_actions = baseline
        .playable_actions(&p1, &baseline_weapon_cards)
        .unwrap();
    assert!(baseline_actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "weapon" && candidate.cards == baseline_weapon_cards
    )));
    let ordinary_weapon = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: baseline_weapon_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(ordinary_weapon.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            formation_id,
            elemental_context_update: Some(effects),
            ..
        } if formation_id == "weapon" && effects.turn_draw_bonus_changes.is_empty()
    )));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：透過實際的雙人回合生命週期建立 Warrior，再建立 War God。P1 的職業
    // 卡牌、P2 的介入行動與每次棄牌選擇全都是標準命令。
    let mut remaining = OfficialRules::new().official_deck_order(&setup).unwrap();
    let warrior_cards = vec![
        take_card(&setup, &mut remaining, Element::Metal, 1),
        take_card(&setup, &mut remaining, Element::Metal, 2),
    ];
    let weapon_metal = take_card(&setup, &mut remaining, Element::Metal, 4);
    let war_god_metal_five = take_card(&setup, &mut remaining, Element::Metal, 5);
    let p2_opening = [
        Element::Wood,
        Element::Water,
        Element::Fire,
        Element::Earth,
        Element::Metal,
    ]
    .into_iter()
    .map(|element| take_card(&setup, &mut remaining, element, 1))
    .collect::<Vec<_>>();
    let war_god_metal_one = take_card(&setup, &mut remaining, Element::Metal, 1);
    let p1_first_discard = take_card(&setup, &mut remaining, Element::Wood, 2);
    let p1_first_kept = take_card(&setup, &mut remaining, Element::Water, 2);
    let p2_first_draw = [Element::Fire, Element::Earth, Element::Metal]
        .into_iter()
        .map(|element| take_card(&setup, &mut remaining, element, 2))
        .collect::<Vec<_>>();
    let weapon_wood = take_card(&setup, &mut remaining, Element::Wood, 1);
    let p1_second_discard = take_card(&setup, &mut remaining, Element::Water, 2);
    let p1_second_kept = take_card(&setup, &mut remaining, Element::Fire, 3);
    let p2_second_draw = [Element::Wood, Element::Water, Element::Fire]
        .into_iter()
        .map(|element| take_card(&setup, &mut remaining, element, 3))
        .collect::<Vec<_>>();
    let mut deck = warrior_cards
        .iter()
        .copied()
        .chain([weapon_metal, war_god_metal_five])
        .chain(p2_opening.iter().copied())
        .chain([war_god_metal_one, p1_first_discard, p1_first_kept])
        .chain(p2_first_draw.iter().copied())
        .chain([weapon_wood, p1_second_discard, p1_second_kept])
        .chain(p2_second_draw.iter().copied())
        .collect::<Vec<_>>();
    deck.extend(remaining);
    let mut interaction = GameRecord::start(setup, deck).unwrap();
    interaction.advance_automatic().unwrap();
    interaction
        .handle(Command::ChangeProfession {
            player: p1.clone(),
            profession: ProfessionId::new("warrior"),
            cards: warrior_cards.clone(),
        })
        .unwrap();
    finish_turn_discarding(&mut interaction, &p1, p1_first_discard);
    interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: elemental_formation_id(
                interaction.state().card_element(p2_opening[0]).unwrap(),
            )
            .to_string(),
            cards: vec![p2_opening[0]],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn_discarding(&mut interaction, &p2, p2_first_draw[0]);
    let war_god_cards = vec![war_god_metal_one, war_god_metal_five];
    interaction
        .handle(Command::ChangeProfession {
            player: p1.clone(),
            profession: ProfessionId::new("war-god"),
            cards: war_god_cards.clone(),
        })
        .unwrap();
    assert_eq!(
        interaction.state().profession_for(&p1),
        Some(&ProfessionId::new("war-god"))
    );
    finish_turn_discarding(&mut interaction, &p1, p1_second_discard);
    let p2_second_action = interaction.state().hand(&p2).unwrap()[0];
    interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: elemental_formation_id(
                interaction.state().card_element(p2_second_action).unwrap(),
            )
            .to_string(),
            cards: vec![p2_second_action],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn_discarding(&mut interaction, &p2, p2_second_draw[0]);

    // 互動：Warrior 繼承的 Weapon 熟練允許金加木，但已提交的陣形仍是 `weapon`；
    // War God 的精通會對同一攻擊結果附加恰好一個具型別的回合抽牌獎勵。
    let weapon_cards = vec![weapon_metal, weapon_wood];
    let actions = interaction.playable_actions(&p1, &weapon_cards).unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::PerformFormation(candidate)
            if candidate.formation_id == "weapon" && candidate.cards == weapon_cards
    )));
    let mastered_weapon = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: weapon_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        mastered_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::AttackResolved {
                attacker,
                formation_id: attack_formation,
                used_cards,
                elemental_context_update: Some(effects),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "weapon"
            && cards == &weapon_cards
            && attacker == &p1
            && attack_formation == "weapon"
            && used_cards == &weapon_cards
            && effects.turn_draw_bonus_changes.len() == 1
            && effects.turn_draw_bonus_changes[0].player == p1
            && effects.turn_draw_bonus_changes[0].old_value == 0
            && effects.turn_draw_bonus_changes[0].delta == 1
            && effects.turn_draw_bonus_changes[0].new_value == 1
            && discarded_by == &p1
            && discarded == "weapon"
            && discarded_cards == &weapon_cards
    ));
    assert_eq!(
        interaction.state().turn_draw_bonus_by_player.get(&p1),
        Some(&1)
    );
    assert!(
        weapon_cards
            .iter()
            .all(|card| interaction.state().discard.contains(card))
    );
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}
