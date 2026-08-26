// 單一事件預期維持為 Vec<GameEvent>，以符合事件日誌斷言輔助工具。
#![allow(clippy::useless_vec)]

use fewfc::application::{
    AutomaticReason, CommandContext, CommandKind, EventSource, GameRecord, StartGame,
    advance_automatic as advance_state_automatic, apply_event, handle_command,
};
use fewfc::domain::{
    ActionModification, AttackOutcome, AttackPointBreakdown, AttackResolutionEffects,
    CannotPerformFormationReason, CardDef, CardDefId, CardInstanceDef, CardInstanceId,
    CardMoveDelta, CardZone, ChoiceAnswer, ChoiceId, Command, CommandId, DamageTransform,
    ElementInteraction, EngineInvariantError, EnvironmentAttackEffect, FormationAreaState,
    FormationInArea, GameConclusion, GameEndCause, GameError, GameEvent, GameOutcome, GameSetup,
    GameState, GameStatus, HpChangeDelta, LastElementalAttack, LastElementalAttackUpdate,
    LastFormationUse, PassActionReason, PassiveFlipOutcome, PassiveNoEffectGround, PendingChoice,
    PendingChoiceKind, PendingResolution, Phase, Player, PlayerFormationArea, PlayerHand, PlayerId,
    PlayerShield, RuleModuleId, RulesetId, ShieldChangeDelta, StatusDuration, StatusEffect,
    StatusExpiryTiming, StatusOwner, TeamHp, TeamId, TurnDrawBonusDelta, TurnDrawSkipReason,
    ValidationError,
};
use fewfc::public_view::{
    self, PublicCardRefs, PublicCoveredPassive, PublicGameEvent, PublicPendingChoice,
    PublicPendingChoicePresentation, PublicPlayerHand, PublicPreviousTurnFormation, Viewer,
};
use fewfc::rules::Element;

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

macro_rules! atomic_context {
    ($update:expr) => {
        Some(AttackResolutionEffects::with_elemental_context($update))
    };
}

/// 舊行為斷言描述命令的語意效果，而不是新的區域/終止生命週期事實。兩側都
/// 比較該不變量視圖，生命週期本身則由下方的專用測試斷言。
fn semantic_events(events: &[GameEvent]) -> Vec<GameEvent> {
    let mut semantic = Vec::new();
    for event in events {
        match event {
            GameEvent::FormationCommitted { .. }
            | GameEvent::FormationCardsDiscarded { .. }
            | GameEvent::FormationPerformed { .. }
            | GameEvent::ActionStarted { .. }
            | GameEvent::GameEnded { .. } => continue,
            GameEvent::TurnDrawResolved {
                player, discard, ..
            } => semantic.push(GameEvent::TurnDiscardChosen {
                player: player.clone(),
                discard: *discard,
            }),
            GameEvent::AttackResolved { .. } => {
                let mut attack = event.clone();
                let mut environment_transfers = Vec::new();
                if let GameEvent::AttackResolved {
                    card_moves,
                    elemental_context_update,
                    ..
                } = &mut attack
                {
                    // 陣形卡牌現在會經過陣形區，而不是使用攻擊舊有的手牌到棄牌堆
                    // 負載。
                    card_moves.clear();
                    if let Some(effects) = elemental_context_update {
                        effects.outcome = Default::default();
                        environment_transfers = std::mem::take(&mut effects.environment_transfers)
                            .into_iter()
                            .map(|transfer| GameEvent::EnvironmentTransferred {
                                player: transfer.player,
                                formation_id: transfer.formation_id,
                                from: transfer.from,
                                to: transfer.to,
                            })
                            .collect();
                        if effects.is_empty() {
                            *elemental_context_update = None;
                        }
                    }
                }
                semantic.push(attack);
                semantic.extend(environment_transfers);
            }
            _ => semantic.push(event.clone()),
        }
    }
    semantic
}

macro_rules! assert_event_semantics_eq {
    ($actual:expr, $expected:expr $(,)?) => {
        assert_eq!(semantic_events(&$actual), semantic_events(&$expected));
    };
}

fn cover(
    state: &mut GameState,
    owner: &str,
    formation_id: &str,
    cards: Vec<CardInstanceId>,
    sealed: bool,
) {
    state
        .formation_area_mut(&PlayerId::new(owner))
        .unwrap()
        .formation = Some(FormationInArea {
        formation_id: formation_id.to_string(),
        cards,
        star_substitution: None,
        state: FormationAreaState::FaceDownWaiting {
            sealed,
            revealed: false,
            neutralized: false,
            trigger_timing: fewfc::domain::PassiveTriggerTiming::NextPlayerActionStart,
        },
    });
}

fn no_covered(state: &GameState) -> bool {
    state
        .formation_areas
        .iter()
        .all(|area| area.formation.is_none())
}

fn card_def(id: &str, element: Element) -> CardDef {
    CardDef {
        id: CardDefId::new(id),
        name: id.to_string(),
        element,
        level: fewfc::domain::PrintedCardLevel::new(match id {
            "metal" => 3,
            "wood" => 2,
            "water" => 1,
            "fire" => 4,
            "earth" => 5,
            _ => 1,
        }),
    }
}

fn card_instance(instance: u64, def_id: &str) -> CardInstanceDef {
    CardInstanceDef {
        instance: card(instance),
        definition: CardDefId::new(def_id),
        origin: Default::default(),
    }
}

fn two_player_setup() -> GameSetup {
    two_player_setup_with_hp(30)
}

fn two_player_setup_with_hp(starting_hp: i32) -> GameSetup {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    GameSetup::two_player(p1, p2, starting_hp).with_cards(
        vec![
            card_def("metal", Element::Metal),
            card_def("wood", Element::Wood),
            card_def("water", Element::Water),
            card_def("fire", Element::Fire),
            card_def("earth", Element::Earth),
        ],
        (1..=20)
            .map(|id| {
                let def_id = match id % 5 {
                    1 => "metal",
                    2 => "wood",
                    3 => "water",
                    4 => "fire",
                    _ => "earth",
                };
                card_instance(id, def_id)
            })
            .collect(),
    )
}

fn bare_team_setup(players_by_team: &[(&str, &str)]) -> GameSetup {
    GameSetup {
        ruleset: RulesetId::base(),
        enabled_rule_modules: Vec::new(),
        players: players_by_team
            .iter()
            .map(|(player, team)| Player {
                id: PlayerId::new(*player),
                team: TeamId::new(*team),
            })
            .collect(),
        turn_order: players_by_team
            .iter()
            .map(|(player, _)| PlayerId::new(*player))
            .collect(),
        hp: players_by_team
            .iter()
            .map(|(_, team)| TeamId::new(*team))
            .fold(Vec::<TeamId>::new(), |mut teams, team| {
                if !teams.contains(&team) {
                    teams.push(team);
                }
                teams
            })
            .into_iter()
            .map(|team| TeamHp { team, hp: 30 })
            .collect(),
        card_defs: Vec::new(),
        card_instances: Vec::new(),
        deck_lists: Vec::new(),
        hand_limit: 5,
        base_draw: 2,
    }
}

fn official_deck() -> Vec<CardInstanceId> {
    (1..=20).map(card).collect()
}

fn deck_starting_with(first_cards: &[u64]) -> Vec<CardInstanceId> {
    let mut deck = first_cards.iter().copied().map(card).collect::<Vec<_>>();

    for id in 1..=20 {
        let candidate = card(id);
        if !deck.contains(&candidate) {
            deck.push(candidate);
        }
    }

    deck
}

fn cannot_act_status(player: PlayerId) -> StatusEffect {
    StatusEffect {
        id: format!("cannot-act-{}", player.as_str()),
        owner: StatusOwner::Player(player),
        kind: "CannotAct".to_string(),
        value: None,
        duration: StatusDuration::Permanent,
    }
}

fn add_status(state: &mut GameState, status: StatusEffect) {
    apply_event(state, &GameEvent::StatusAdded { status });
}

fn state_after_cannot_act_pass(record: &GameRecord, player: PlayerId) -> GameState {
    let mut state = record.state().clone();
    add_status(&mut state, cannot_act_status(player.clone()));

    let events = handle_command(
        &state,
        Command::PassAction {
            player,
            reason: PassActionReason::CannotActByStatus,
        },
    )
    .unwrap();

    for event in events {
        apply_event(&mut state, &event);
    }

    state
}

fn answer_choice(
    state: &GameState,
    player: PlayerId,
    answer: ChoiceAnswer,
) -> Result<Vec<GameEvent>, GameError> {
    handle_command(
        state,
        Command::AnswerChoice {
            player,
            choice_id: state
                .pending_choice
                .as_ref()
                .expect("pending choice")
                .choice_id,
            answer,
        },
    )
}

fn answer_record_choice(
    record: &mut GameRecord,
    player: PlayerId,
    answer: ChoiceAnswer,
) -> Result<Vec<GameEvent>, GameError> {
    let choice_id = record
        .state()
        .pending_choice
        .as_ref()
        .expect("pending choice")
        .choice_id;
    record.handle(Command::AnswerChoice {
        player,
        choice_id,
        answer,
    })
}

fn advance_record_to_next_main_after_turn_draw(
    record: &mut GameRecord,
    discard: CardInstanceId,
) -> Vec<GameEvent> {
    let mut events = record.advance_automatic().unwrap();
    events.extend(
        answer_record_choice(
            record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap(),
    );
    events.extend(record.advance_automatic().unwrap());
    events
}

/// 以當前 Turn Draw choice 的第一張可選 Card 走完完整回合。這只封裝合法的
/// automatic advancement 與 AnswerChoice，不安排任何受測規則狀態或結果。
fn advance_record_to_next_main_discarding_first_turn_draw_card(
    record: &mut GameRecord,
    player: &PlayerId,
) -> Vec<GameEvent> {
    let mut events = record.advance_automatic().unwrap();
    let discard = match &record
        .state()
        .pending_choice
        .as_ref()
        .expect("completed action must reach Turn Draw choice")
        .kind
    {
        PendingChoiceKind::Card { cards, .. } => cards[0],
        other => panic!("expected Turn Draw Card choice, got {other:?}"),
    };
    events.extend(
        answer_record_choice(
            record,
            player.clone(),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap(),
    );
    events.extend(record.advance_automatic().unwrap());
    events
}

fn record_after_p1_metal_attack_on_turn_1() -> GameRecord {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(10));
    record
}

#[test]
fn elemental_history_matrix_uses_only_the_immediate_previous_formation() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
    ]);
    let deck = vec![
        // P1：首個 Weapon、保留 Earth，之後可再用一個 Weapon。
        1, 6, 5, 2, // P2：Metal Strike 或 Weapon 的共同起手。
        11, 16, 4, 7, 8,
        // P1/P2 首輪 Turn Draw；31 只在 P2 第二次 Weapon 前進手。
        21, 26, 3, 31, 9, 10, // P1/P2 第二輪 Turn Draw。
        13, 14, 15, 12, 17, 18,
    ]
    .into_iter()
    .map(card)
    .collect::<Vec<_>>();

    fn finish_turn(record: &mut GameRecord, player: PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player,
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    // 共用開始：P1 的實體 Weapon 是合法 but non-elemental bridge，讓 P2 的下一個
    // Command 成為唯一可能的 immediate element source。
    let record_at_p2_turn_two = || {
        let mut record = GameRecord::start(setup.clone(), deck.clone()).unwrap();
        record.advance_automatic().unwrap();
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: vec![card(1), card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p1.clone(), card(3));
        assert_eq!(record.state().current_player(), Some(&p2));
        record
    };

    // 無修飾基準：P2 的實體 Weapon 不建立元素記錄，故 Earth 維持正常傷害。
    let mut baseline = record_at_p2_turn_two();
    baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(11), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, p2.clone(), card(10));
    let ordinary_earth = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        ordinary_earth.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 9,
                },
                hp_change: HpChangeDelta { team, old_hp: 188, delta: -9, effective_delta: -9, new_hp: 179, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "earth-strike"
            && cards == &vec![card(5)]
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(5)]
    ));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾本身：P2 的合法 Metal Strike 是 P1 Earth 的 immediate predecessor，
    // 因此 Earth 生成 Metal，解析為九點回復。
    let mut immediate = record_at_p2_turn_two();
    let metal = immediate
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        metal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Metal, resolved_turn: 2 },
                    }),
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(11)]
            && context_player == &p2
            && discarded_by == &p2
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(11)]
    ));
    finish_turn(&mut immediate, p2.clone(), card(9));
    let generating_earth = immediate
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        generating_earth.as_slice(),
        [
            GameEvent::FormationCommitted { .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::Generating,
                    damage_transform: DamageTransform::HealTarget,
                    final_amount: 9,
                },
                hp_change: HpChangeDelta { team, old_hp: 188, delta: 9, effective_delta: 9, new_hp: 197, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { .. },
        ] if team == &TeamId::new("team:p2")
    ));
    assert_eq!(immediate.replay().unwrap(), immediate.state().clone());
    assert_eq!(
        immediate.verify_replay().unwrap(),
        immediate.state().clone()
    );

    // 互動：P2 的 Metal record 仍保存為 turn 2，但 P1 Weapon（turn 3）與 P2
    // Weapon（turn 4）已使它過期。P1 turn 5 的 Earth 不能從該舊 record 取得效果。
    let mut stale = record_at_p2_turn_two();
    stale
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut stale, p2.clone(), card(9));
    stale
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(21), card(26)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut stale, p1.clone(), card(14));
    stale
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(16), card(31)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut stale, p2.clone(), card(17));
    assert_eq!(
        stale.state().last_elemental_attack_by_player.get(&p2),
        Some(&LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 2,
        })
    );
    assert_eq!(
        stale
            .public_view(Viewer::Observer)
            .unwrap()
            .previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: p2.clone(),
            formation_id: Some("weapon".to_string()),
            cards: PublicCardRefs::Known(vec![card(16), card(31)]),
        })
    );
    let stale_earth = stale
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        stale_earth.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 9,
                },
                hp_change: HpChangeDelta { team, old_hp: 176, delta: -9, effective_delta: -9, new_hp: 167, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "earth-strike"
            && cards == &vec![card(5)]
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(5)]
    ));
    assert_eq!(stale.replay().unwrap(), stale.state().clone());
    assert_eq!(stale.verify_replay().unwrap(), stale.state().clone());
}

#[test]
fn metal_environment_overcoming_matrix_stacks_matching_and_immediate_wood_context() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(500)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
        card_instance(36, "metal"),
        card_instance(41, "metal"),
        card_instance(46, "metal"),
        card_instance(51, "metal"),
    ]);

    fn finish_turn(record: &mut GameRecord, player: PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player,
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    // 無修飾基準：P2 的實體 Weapon 是真正的前一個 Formation，但不寫入元素脈絡；
    // 因此 P1 同一張 Metal Strike 只造成正常七點傷害。
    let mut baseline = GameRecord::start(
        setup.clone(),
        deck_starting_with(&[2, 3, 1, 4, 31, 36, 5, 12, 13, 6, 7, 8, 9, 10, 11]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(2), card(3)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, p1.clone(), card(8));
    let physical_weapon = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(31), card(36)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        physical_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::PassiveFlipped { outcome: PassiveFlipOutcome::NoEffect { .. }, .. },
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects { elemental_context_update: None, .. }),
                ..
            },
            GameEvent::FormationCardsDiscarded { .. },
        ] if player == &p2
            && formation_id == "weapon"
            && cards == &vec![card(31), card(36)]
    ));
    finish_turn(&mut baseline, p2.clone(), card(11));
    let ordinary_metal = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        ordinary_metal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta { team, old_hp: 500, delta: -7, effective_delta: -7, new_hp: 493, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "metal-strike"
            && cards == &vec![card(1)]
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(1)]
    ));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    let modifier_deck = {
        let opening = vec![
            1, 6, 11, 16, // P1：先用 Metal Strike，回合抽牌後湊齊 Sacred Beast。
            2, 3, 31, 36, 5, // P2：先用 Empty City，再選擇 Weapon 或 Wood Strike。
            21, 26, 4, // P1 第一次 Turn Draw
            41, 46, 7, // P2 第一次 Turn Draw
            8, 9, 51, // P1 的 Sacred Beast 後 Turn Draw，51 留給 Metal Strike。
            12, 13, 14, // P2 第二次 Turn Draw
        ]
        .into_iter()
        .map(card)
        .collect::<Vec<_>>();
        opening
    };

    // 修飾本身：Metal Environment 必須來自真實的 West White Tiger 使用。P2 的
    // Empty City 只作合法回合橋接，不會抑制 Sacred Beast 的 Attack 或轉移。
    let record_with_metal_environment = || {
        let mut record = GameRecord::start(setup.clone(), modifier_deck.clone()).unwrap();
        record.advance_automatic().unwrap();
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p1.clone(), card(4));
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "empty-city".to_string(),
                cards: vec![card(2), card(3)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p2.clone(), card(46));
        let tiger = record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "west-white-tiger".to_string(),
                cards: vec![card(6), card(11), card(16), card(21), card(26)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(matches!(
            tiger.as_slice(),
            [
                GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
                GameEvent::PassiveFlipped { outcome: PassiveFlipOutcome::NoEffect { .. }, .. },
                GameEvent::AttackResolved {
                    elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                    ..
                },
                GameEvent::FormationCardsDiscarded { .. },
            ] if player == &p1
                && formation_id == "west-white-tiger"
                && cards == &vec![card(6), card(11), card(16), card(21), card(26)]
                && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                    player: p1.clone(),
                    formation_id: "west-white-tiger".to_string(),
                    from: None,
                    to: Element::Metal,
                }]
        ));
        assert_eq!(record.state().environment, Some(Element::Metal));
        finish_turn(&mut record, p1.clone(), card(8));
        assert_eq!(record.state().current_player(), Some(&p2));
        record
    };

    // 僅修飾：P2 的實體 Weapon 仍不會建立元素脈絡；Metal Strike 因此只吃到
    // matching Environment 的一次加倍。
    let mut environment_only = record_with_metal_environment();
    let environment_weapon = environment_only
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(31), card(36)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        environment_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { .. },
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: None,
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { .. },
        ]
    ));
    finish_turn(&mut environment_only, p2.clone(), card(14));
    let environment_metal = environment_only
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(51)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        environment_metal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::MatchingElementDamageDoubled { environment: Element::Metal },
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 14,
                },
                hp_change: HpChangeDelta { old_hp: 412, delta: -14, effective_delta: -14, new_hp: 398, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "metal-strike"
            && cards == &vec![card(51)]
            && discarded_by == &p1
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(51)]
    ));
    assert_eq!(environment_only.state().environment, Some(Element::Metal));
    assert_eq!(
        environment_only.replay().unwrap(),
        environment_only.state().clone()
    );
    assert_eq!(
        environment_only.verify_replay().unwrap(),
        environment_only.state().clone()
    );

    // 互動：P2 的 Wood Strike 是立刻前一個元素攻擊；P1 的 Metal Strike 同時取得
    // matching Environment 與 overcoming Wood 的兩次獨立加倍。
    let mut interaction = record_with_metal_environment();
    interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "wood-strike".to_string(),
            cards: vec![card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut interaction, p2.clone(), card(13));
    let stacked_metal = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(51)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        stacked_metal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::MatchingElementDamageDoubled { environment: Element::Metal },
                    interaction: ElementInteraction::Overcoming,
                    damage_transform: DamageTransform::DoubleDamage,
                    final_amount: 28,
                },
                hp_change: HpChangeDelta { team, old_hp: 412, delta: -28, effective_delta: -28, new_hp: 384, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "metal-strike"
            && cards == &vec![card(51)]
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(51)]
    ));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert_eq!(
        interaction
            .public_view(Viewer::Observer)
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    assert!(interaction.state().discard.contains(&card(51)));
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn metal_environment_recovery_matrix_distinguishes_overcoming_and_same_contexts() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(22, "metal"),
        card_instance(23, "earth"),
        card_instance(24, "metal"),
        card_instance(25, "metal"),
        card_instance(30, "metal"),
    ]);

    fn finish_turn(record: &mut GameRecord, player: PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player,
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    // 無修飾基準：P2 的 Weapon 不建立元素脈絡，Earth Strike 對同一個目標只會
    // 正常造成九點傷害。
    let mut baseline = GameRecord::start(
        setup.clone(),
        deck_starting_with(&[2, 3, 5, 4, 24, 25, 1, 6, 11, 7, 8, 9, 10, 12, 13]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(2), card(3)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, p1.clone(), card(9));
    baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(24), card(25)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, p2.clone(), card(13));
    let ordinary_earth = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        ordinary_earth.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 9,
                },
                hp_change: HpChangeDelta { team, old_hp: 200, delta: -9, effective_delta: -9, new_hp: 191, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "earth-strike"
            && cards == &vec![card(5)]
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(5)]
    ));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    let modifier_deck = vec![
        // P1：Wood bridge 後補齊 White Tiger，並保留 Earth Strike。
        1, 6, 11, 2,
        // P2：physical bridge；第一個 Turn Draw 帶來 Water context Card。
        22, 24, 25, 30, 5, 16, 21, 4, // P1 第一個 Turn Draw
        3, 9, 10, // P2 第一個 Turn Draw
        23, 8, 12, // P1 Sacred Beast 後 Turn Draw
        13, 14, 15, // P2 第二個 Turn Draw
    ]
    .into_iter()
    .map(card)
    .collect::<Vec<_>>();

    // Metal Environment 只由合法 White Tiger 建立。P2 的第一個 Weapon 只是
    // 無元素 bridge，讓白虎和後續 Earth 使用分屬正確回合。
    let record_with_metal_environment = || {
        let mut record = GameRecord::start(setup.clone(), modifier_deck.clone()).unwrap();
        record.advance_automatic().unwrap();
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(2)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p1.clone(), card(4));
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "weapon".to_string(),
                cards: vec![card(24), card(25)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p2.clone(), card(10));
        let tiger = record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "west-white-tiger".to_string(),
                cards: vec![card(1), card(6), card(11), card(16), card(21)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(matches!(
            tiger.as_slice(),
            [
                GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
                GameEvent::AttackResolved {
                    hp_change: HpChangeDelta { team, old_hp: 194, delta: -81, effective_delta: -81, new_hp: 113, .. },
                    elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                    ..
                },
                GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
            ] if player == &p1
                && formation_id == "west-white-tiger"
                && cards == &vec![card(1), card(6), card(11), card(16), card(21)]
                && team == &TeamId::new("team:p2")
                && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                    player: p1.clone(),
                    formation_id: "west-white-tiger".to_string(),
                    from: None,
                    to: Element::Metal,
                }]
                && discarded_by == &p1
                && discarded == "west-white-tiger"
                && discarded_cards == &vec![card(1), card(6), card(11), card(16), card(21)]
        ));
        assert_eq!(record.state().environment, Some(Element::Metal));
        finish_turn(&mut record, p1.clone(), card(8));
        assert_eq!(record.state().current_player(), Some(&p2));
        record
    };

    // 僅修飾：第二個 Weapon 仍為實體 Formation，故 Earth 只取得 Metal
    // Environment 的 generating recovery，沒有前一元素 interaction。
    let mut environment_only = record_with_metal_environment();
    environment_only
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(22), card(30)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut environment_only, p2.clone(), card(15));
    let environment_earth = environment_only
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(23)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        environment_earth.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::GeneratingElementDamageConvertedToHealing { environment: Element::Metal },
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::HealTarget,
                    final_amount: 9,
                },
                hp_change: HpChangeDelta { team, old_hp: 113, delta: 9, effective_delta: 9, new_hp: 122, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "earth-strike"
            && cards == &vec![card(23)]
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(23)]
    ));
    assert_eq!(environment_only.state().environment, Some(Element::Metal));
    assert_eq!(
        environment_only
            .public_view(Viewer::Observer)
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    assert_eq!(
        environment_only.replay().unwrap(),
        environment_only.state().clone()
    );
    assert_eq!(
        environment_only.verify_replay().unwrap(),
        environment_only.state().clone()
    );

    // 互動：P2 的 Water Strike 是真正的 immediate elemental context；Earth 同時
    // 取得 Metal-generating recovery 與 overcoming Water，回復量精確為十八。
    let mut interaction = record_with_metal_environment();
    let water = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "water-strike".to_string(),
            cards: vec![card(3)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(water.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(AttackResolutionEffects {
                elemental_context_update: Some(LastElementalAttackUpdate { player: context_player, attack: LastElementalAttack { element: Element::Water, resolved_turn: 4 } }),
                ..
            }),
            ..
        } if context_player == &p2
    )));
    finish_turn(&mut interaction, p2.clone(), card(14));
    let combined_earth = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(23)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        combined_earth.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::GeneratingElementDamageConvertedToHealing { environment: Element::Metal },
                    interaction: ElementInteraction::Overcoming,
                    damage_transform: DamageTransform::HealTarget,
                    final_amount: 18,
                },
                hp_change: HpChangeDelta { team, old_hp: 113, delta: 18, effective_delta: 18, new_hp: 131, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "earth-strike"
            && cards == &vec![card(23)]
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(23)]
    ));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(131)
    );
    for used in [
        card(1),
        card(6),
        card(11),
        card(16),
        card(21),
        card(3),
        card(23),
    ] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );

    // 同元素 sibling：同一個合法 Metal Environment 中，P2 的 Earth Strike 成為
    // immediate context。P1 Earth 的 Environment recovery 仍適用，但同元素關係
    // 會把九點回復向上取整為五點。
    let mut same_element = record_with_metal_environment();
    let earth_context = same_element
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        earth_context.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Earth, resolved_turn: 4 },
                    }),
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "earth-strike"
            && cards == &vec![card(5)]
            && context_player == &p2
            && discarded_by == &p2
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(5)]
    ));
    finish_turn(&mut same_element, p2.clone(), card(14));
    let rounded_recovery = same_element
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(23)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        rounded_recovery.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::GeneratingElementDamageConvertedToHealing { environment: Element::Metal },
                    interaction: ElementInteraction::Same,
                    damage_transform: DamageTransform::HealTarget,
                    final_amount: 5,
                },
                hp_change: HpChangeDelta { team, old_hp: 113, delta: 5, effective_delta: 5, new_hp: 118, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "earth-strike"
            && cards == &vec![card(23)]
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(23)]
    ));
    assert_eq!(same_element.state().environment, Some(Element::Metal));
    assert_eq!(
        same_element
            .public_view(Viewer::Observer)
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    assert_eq!(
        same_element
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(118)
    );
    assert!(same_element.state().discard.contains(&card(5)));
    assert!(same_element.state().discard.contains(&card(23)));
    assert_eq!(same_element.replay().unwrap(), same_element.state().clone());
    assert_eq!(
        same_element.verify_replay().unwrap(),
        same_element.state().clone()
    );
}

#[test]
fn public_state_view_exposes_the_shared_environment_to_every_viewer() {
    let mut state = GameState::from_setup(
        &two_player_setup_with_hp(100)
            .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]),
    );
    state.environment = Some(Element::Water);

    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p1"))).environment,
        Some(Element::Water)
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Observer).environment,
        Some(Element::Water)
    );
}

#[test]
fn void_meridian_environment_matrix_clears_both_teams_then_finishes_as_a_draw() {
    let p0 = PlayerId::new("p0");
    let p3 = PlayerId::new("p3");
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let team_a = TeamId::new("team:a");
    let team_b = TeamId::new("team:b");
    let mut setup = two_player_setup_with_hp(101)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.players = vec![
        Player {
            id: p0.clone(),
            team: team_a.clone(),
        },
        Player {
            id: p3.clone(),
            team: team_b.clone(),
        },
        Player {
            id: p1.clone(),
            team: team_a.clone(),
        },
        Player {
            id: p2.clone(),
            team: team_b.clone(),
        },
    ];
    setup.turn_order = vec![p0.clone(), p3.clone(), p1.clone(), p2.clone()];
    // HP 是終局邊界的非受測背景。兩隊分別在 Sacred Beast 後與 Void Meridian 前
    // 到達 20，讓同一個 canonical EnvironmentCleared 一次耗盡雙方。
    setup.hp = vec![
        TeamHp {
            team: team_a.clone(),
            hp: 101,
        },
        TeamHp {
            team: team_b.clone(),
            hp: 20,
        },
    ];
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(36, "metal"),
        card_instance(41, "metal"),
        card_instance(46, "metal"),
        card_instance(47, "wood"),
        card_instance(48, "wood"),
        card_instance(49, "metal"),
        card_instance(50, "fire"),
        card_instance(51, "earth"),
        card_instance(52, "earth"),
    ]);
    let deck = vec![
        // 初始 4P 發牌：P0、P3、P1、P2。
        1, 2, 3, 4, 6, 11, 16, 21, 26, 47, 48, 49, 50, 5, 36, 41, 46, 12, 13,
        // 三個合法橋接回合的 Turn Draw Cards。
        7, 8, 9, 10, 14, 15, 17, 18, 19, 20, 51, 52,
    ]
    .into_iter()
    .map(card)
    .collect::<Vec<_>>();

    fn finish_turn(record: &mut GameRecord, player: PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player,
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();

    // P0 的 Empty City 是無傷害的合法橋接；P3 的下一個 Sacred Beast 仍正常解析。
    record
        .handle(Command::PerformFormation {
            player: p0.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(2), card(3)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut record, p0.clone(), card(9));

    let sacred_beast = record
        .handle(Command::PerformFormation {
            player: p3.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: vec![card(6), card(11), card(16), card(21), card(26)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        sacred_beast.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::PassiveFlipped { outcome: PassiveFlipOutcome::NoEffect { .. }, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown { final_amount: 81, .. },
                hp_change: HpChangeDelta { team, old_hp: 101, effective_delta: -81, new_hp: 20, .. },
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            },
            GameEvent::FormationCardsDiscarded { .. },
        ] if player == &p3
            && formation_id == "west-white-tiger"
            && cards == &vec![card(6), card(11), card(16), card(21), card(26)]
            && team == &team_a
            && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p3.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
    ));
    assert_eq!(record.state().environment, Some(Element::Metal));
    finish_turn(&mut record, p3.clone(), card(15));

    // P1 的 Barrier 只負責推進合法回合，既不修改 Environment，也不改變 Team HP。
    record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: vec![card(47), card(48), card(49), card(50)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut record, p1.clone(), card(19));
    assert_eq!(record.state().current_player(), Some(&p2));
    assert_eq!(record.state().environment, Some(Element::Metal));
    assert_eq!(
        record.state().hp,
        vec![
            TeamHp {
                team: team_a.clone(),
                hp: 20,
            },
            TeamHp {
                team: team_b.clone(),
                hp: 20,
            },
        ]
    );

    let void_events = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(36), card(41), card(46)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        void_events.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::EnvironmentCleared { player: clearer, formation_id: cleared_by, environment: Element::Metal, hp_changes },
            GameEvent::GameEnded { conclusion },
        ] if player == &p2
            && formation_id == "void-meridian-severing"
            && cards == &vec![card(36), card(41), card(46)]
            && clearer == &p2
            && cleared_by == "void-meridian-severing"
            && hp_changes == &vec![
                HpChangeDelta { team: team_a.clone(), old_hp: 20, delta: -20, effective_delta: -20, new_hp: 0 },
                HpChangeDelta { team: team_b.clone(), old_hp: 20, delta: -20, effective_delta: -20, new_hp: 0 },
            ]
            && conclusion.outcome == GameOutcome::Draw
            && conclusion.causes == vec![GameEndCause::TeamHpDepleted {
                teams: vec![team_a.clone(), team_b.clone()],
            }]
    ));
    assert_eq!(record.state().environment, None);
    assert!(matches!(
        record.state().status,
        GameStatus::Finished { ref conclusion } if conclusion.outcome == GameOutcome::Draw
    ));
    assert!(record.state().hp.iter().all(|entry| entry.hp == 0));
    assert!(matches!(
        record.state().formation_area(&p2),
        Some(PlayerFormationArea { formation: Some(FormationInArea { formation_id, cards, .. }), .. })
            if formation_id == "void-meridian-severing" && cards == &vec![card(36), card(41), card(46)]
    ));
    assert!(
        record
            .state()
            .discard
            .iter()
            .all(|discarded| ![card(36), card(41), card(46)].contains(discarded))
    );
    assert_eq!(
        record.public_view(Viewer::Observer).unwrap().environment,
        None
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn sacred_beast_same_environment_matrix_records_the_idempotent_metal_transfer() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(1_000)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
        card_instance(36, "metal"),
        card_instance(41, "metal"),
        card_instance(46, "metal"),
        card_instance(51, "metal"),
        card_instance(52, "metal"),
        card_instance(56, "metal"),
        card_instance(61, "metal"),
    ]);
    let opening = vec![
        1, 6, 11, 16, // P1：先以 Metal Strike 過渡，保留四張 Metal
        31, 36, 41, 46, 51, // P2：先以 physical Weapon 過渡，保留三張 Metal
        21, 26, 2, // P1 Turn Draw：補足第一個 West White Tiger
        52, 56, 61, // P2 Turn Draw：補足第二個 West White Tiger
        3, 4, 5, // P1 第一隻 Sacred Beast 後的 Turn Draw
    ];
    let opening = opening.iter().copied().map(card).collect::<Vec<_>>();
    let mut deck = opening.clone();
    deck.extend(
        (1..=20)
            .map(card)
            .filter(|candidate| !opening.contains(candidate)),
    );

    fn finish_turn(record: &mut GameRecord, player: &PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player.clone(),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();

    // 背景行動都由合法 Formation 建立：兩位玩家各自先完成一個正常回合，讓兩隻
    // Sacred Beast 的五張 Metal Cards 均經由真實 Turn Draw 取得。
    record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut record, &p1, card(2));
    record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(31), card(36)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut record, &p2, card(61));

    let first_beast_cards = vec![card(6), card(11), card(16), card(21), card(26)];
    assert!(
        first_beast_cards
            .iter()
            .all(|card| record.state().hand(&p1).unwrap().contains(card))
    );
    let first_beast = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: first_beast_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            first_beast.as_slice(),
            [
                GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
                GameEvent::AttackResolved {
                    attacker,
                    target,
                    point_breakdown: AttackPointBreakdown {
                        base_points: 81,
                        environment_effect: EnvironmentAttackEffect::None,
                        interaction: ElementInteraction::None,
                        damage_transform: DamageTransform::NormalDamage,
                        final_amount: 81,
                    },
                    hp_change: HpChangeDelta { team, effective_delta: -81, .. },
                    elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                    ..
                },
                GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
            ] if player == &p1
                && formation_id == "west-white-tiger"
                && cards == &first_beast_cards
                && attacker == &p1
                && target == &p2
                && team == &TeamId::new("team:p2")
                && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                    player: p1.clone(),
                    formation_id: "west-white-tiger".to_string(),
                    from: None,
                    to: Element::Metal,
                }]
                && discarded_by == &p1
                && discarded == "west-white-tiger"
                && discarded_cards == &first_beast_cards
        ),
        "unexpected first Sacred Beast baseline: {first_beast:#?}"
    );
    assert_eq!(record.state().environment, Some(Element::Metal));
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .find(|team| team.team == TeamId::new("team:p2"))
            .map(|team| team.hp),
        Some(912)
    );
    assert!(
        first_beast_cards
            .iter()
            .all(|card| record.state().discard.contains(card))
    );
    assert_eq!(
        record
            .public_view(Viewer::Player(p1.clone()))
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    assert_eq!(
        record
            .public_view(Viewer::Player(p2.clone()))
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    finish_turn(&mut record, &p1, card(4));

    // interaction：P2 的第二隻合法 Sacred Beast 在既有 Metal Environment 下仍
    // 必須記錄一次 Metal → Metal 的 canonical transfer，而不是只留下同一個最終值。
    let second_beast_cards = vec![card(41), card(46), card(51), card(52), card(56)];
    assert!(
        second_beast_cards
            .iter()
            .all(|card| record.state().hand(&p2).unwrap().contains(card))
    );
    let second_beast = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: second_beast_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            second_beast.as_slice(),
            [
                GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
                GameEvent::AttackResolved {
                    attacker,
                    target,
                    point_breakdown: AttackPointBreakdown {
                        base_points: 81,
                        environment_effect: EnvironmentAttackEffect::MatchingElementDamageDoubled {
                            environment: Element::Metal,
                        },
                        interaction: ElementInteraction::Same,
                        damage_transform: DamageTransform::HalfDamageRoundUp,
                        final_amount: 81,
                    },
                    hp_change: HpChangeDelta {
                        team,
                    old_hp: 988,
                    delta: -81,
                    new_hp: 907,
                        effective_delta: -81,
                    },
                    shield_change: None,
                    card_moves,
                    elemental_context_update: Some(AttackResolutionEffects {
                        outcome: AttackOutcome::Resolved,
                        elemental_context_update: Some(LastElementalAttackUpdate {
                            player: context_player,
                            attack: LastElementalAttack {
                                element: Element::Metal,
                                resolved_turn: 4,
                            },
                        }),
                        environment_transfers,
                        ..
                    }),
                    ..
                },
                GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
            ] if player == &p2
                && formation_id == "west-white-tiger"
                && cards == &second_beast_cards
                && attacker == &p2
                && target == &p1
                && team == &TeamId::new("team:p1")
                && card_moves.is_empty()
                && context_player == &p2
                && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                    player: p2.clone(),
                    formation_id: "west-white-tiger".to_string(),
                    from: Some(Element::Metal),
                    to: Element::Metal,
                }]
                && discarded_by == &p2
                && discarded == "west-white-tiger"
                && discarded_cards == &second_beast_cards
        ),
        "unexpected idempotent Sacred Beast interaction: {second_beast:#?}"
    );
    assert_eq!(record.state().environment, Some(Element::Metal));
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .find(|team| team.team == TeamId::new("team:p1"))
            .map(|team| team.hp),
        Some(907)
    );
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .find(|team| team.team == TeamId::new("team:p2"))
            .map(|team| team.hp),
        Some(912)
    );
    for card in first_beast_cards.iter().chain(second_beast_cards.iter()) {
        assert!(record.state().discard.contains(card));
    }
    assert_eq!(
        record
            .public_view(Viewer::Player(p1.clone()))
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    assert_eq!(
        record
            .public_view(Viewer::Player(p2.clone()))
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn new_game_preserves_card_instance_definitions_for_lookup() {
    let record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let state = record.state().clone();

    assert_eq!(
        state.card_def(card(1)),
        Some(&CardDef {
            id: CardDefId::new("metal"),
            name: "metal".to_string(),
            element: Element::Metal,
            level: fewfc::domain::PrintedCardLevel::new(3),
        })
    );
    assert_eq!(state.card_def(card(99)), None);
}

#[test]
fn game_state_resolves_card_instance_elements_for_formation_matching() {
    let record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let state = record.state().clone();

    assert_eq!(state.card_element(card(1)), Some(Element::Metal));
    assert_eq!(state.card_element(card(2)), Some(Element::Wood));
    assert_eq!(state.card_element(card(99)), None);
}

#[test]
fn new_game_rejects_deck_that_cannot_satisfy_initial_deal() {
    assert_eq!(
        GameRecord::start(two_player_setup(), vec![card(1), card(2), card(3)]),
        Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed: 9,
                available: 3,
            }
        ))
    );
}

#[test]
fn new_game_persists_deck_order_and_replay_matches_current_state() {
    let deck = official_deck();
    let record = GameRecord::start(two_player_setup(), deck.clone()).unwrap();

    assert_eq!(
        record.events().first(),
        Some(&GameEvent::DeckPrepared {
            deck_order: deck.clone()
        })
    );

    let state = record.state().clone();
    assert_eq!(state.phase, Phase::TurnStart);
    assert_eq!(record.setup().ruleset, RulesetId::base());
    assert_eq!(state.current_player(), Some(&PlayerId::new("p1")));
    assert_eq!(state.deck, (10..=20).map(card).collect::<Vec<_>>());
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn game_record_facade_applies_commands_and_verifies_replay() {
    let mut record = GameRecord::start_game(StartGame {
        setup: two_player_setup(),
        deck_order: official_deck(),
    })
    .unwrap();

    let automatic = record.advance_until_decision().unwrap();
    assert_eq!(
        automatic.events(),
        &[GameEvent::TurnStarted {
            player: PlayerId::new("p1"),
            turn_number: 1,
        }]
    );

    let command_events = record
        .apply(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        semantic_events(command_events.events()).as_slice(),
        [GameEvent::AttackResolved { .. }]
    ));

    let view = record.public_view(Viewer::Observer).unwrap();
    assert_eq!(view.phase, Phase::TurnDraw);
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn base_start_turn_lifecycle_matrix_commits_an_action_then_resolves_draw_choice_and_next_turn() {
    // 這是基礎規則開始邊界的命令層級生命週期矩陣。固定牌堆順序僅是背景：Start
    // 之後的每個轉換都是實際命令或自動標準轉換。
    let deck_order = official_deck();
    let mut record = GameRecord::start(two_player_setup(), deck_order.clone()).unwrap();
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    assert_eq!(
        record.events(),
        [
            GameEvent::DeckPrepared {
                deck_order: deck_order.clone(),
            },
            GameEvent::CardsDealt {
                player: p1.clone(),
                cards: vec![card(1), card(2), card(3), card(4)],
            },
            GameEvent::CardsDealt {
                player: p2.clone(),
                cards: vec![card(5), card(6), card(7), card(8), card(9)],
            },
        ]
    );
    assert_eq!(
        record.state().hand(&p1),
        Some(vec![card(1), card(2), card(3), card(4)].as_slice())
    );
    assert_eq!(
        record.state().hand(&p2),
        Some(vec![card(5), card(6), card(7), card(8), card(9)].as_slice())
    );
    assert_eq!(record.state().deck, (10..=20).map(card).collect::<Vec<_>>());
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());

    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![GameEvent::TurnStarted {
            player: p1.clone(),
            turn_number: 1,
        }]
    );
    let action = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        action.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                state: FormationAreaState::FaceUpResolving,
                ..
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                formation_id: attack_formation,
                point_breakdown: AttackPointBreakdown { final_amount: 7, .. },
                hp_change: HpChangeDelta { old_hp: 30, delta: -7, new_hp: 23, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded,
                cards: discarded_cards,
            },
        ] if player == &p1
            && formation_id == "metal-strike"
            && cards == &vec![card(1)]
            && attacker == &p1
            && target == &p2
            && attack_formation == "metal-strike"
            && discarded_by == &p1
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(1)]
    ));
    assert_eq!(record.state().phase, Phase::TurnDraw);
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(23)
    );
    assert!(record.state().discard.contains(&card(1)));

    let draw_events = record.advance_automatic().unwrap();
    let (choice_id, drawn_cards) = match draw_events.as_slice() {
        [
            GameEvent::CardsDrawnForTurnDiscardChoice {
                player,
                drawn_cards,
                allowed_discards,
            },
            GameEvent::ChoiceRequested { choice, .. },
        ] if player == &p1 && drawn_cards == allowed_discards && choice.player == p1 => {
            (choice.choice_id, drawn_cards.clone())
        }
        _ => panic!("expected canonical turn-draw Card choice, got {draw_events:?}"),
    };
    assert_eq!(drawn_cards, vec![card(10), card(11), card(12)]);

    let chosen_discard = card(12);
    assert_eq!(
        record
            .handle(Command::AnswerChoice {
                player: p1.clone(),
                choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![chosen_discard],
                },
            })
            .unwrap(),
        vec![
            GameEvent::ChoiceMade {
                player: p1.clone(),
                choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![chosen_discard],
                },
            },
            GameEvent::TurnDrawResolved {
                player: p1.clone(),
                discard: chosen_discard,
                kept_cards: vec![card(10), card(11)],
            },
        ]
    );
    assert_eq!(
        record.advance_automatic().unwrap(),
        vec![
            GameEvent::TurnEnded { player: p1.clone() },
            GameEvent::TurnStarted {
                player: p2.clone(),
                turn_number: 2,
            },
        ]
    );
    assert_eq!(record.state().current_player(), Some(&p2));
    assert_eq!(record.state().phase, Phase::ActiveEffects);
    assert_eq!(
        record.state().hand(&p1),
        Some(vec![card(2), card(3), card(4), card(10), card(11)].as_slice())
    );
    assert!(record.state().discard.contains(&chosen_discard));
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn game_record_exposes_recorded_events_with_sequence_metadata() {
    let deck = official_deck();
    let mut record = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let recorded_events = record.recorded_events();

    assert_eq!(
        recorded_events
            .iter()
            .map(|recorded| recorded.metadata.sequence)
            .collect::<Vec<_>>(),
        (1..=recorded_events.len() as u64).collect::<Vec<_>>()
    );
    assert_eq!(recorded_events[0].metadata.source, EventSource::Setup);
    assert_eq!(
        recorded_events[0].event,
        GameEvent::DeckPrepared { deck_order: deck }
    );
    assert_eq!(
        recorded_events[3].metadata.source,
        EventSource::Automatic {
            reason: AutomaticReason::TurnStart,
        }
    );
    assert_eq!(
        recorded_events.last().unwrap().metadata.source,
        EventSource::Command {
            command_id: CommandId::new(1),
            context: CommandContext {
                player: PlayerId::new("p1"),
                kind: CommandKind::PerformFormation {
                    formation_id: "metal-strike".to_string(),
                },
            },
        }
    );
}

#[test]
fn apply_event_projects_canonical_events_without_returning_validation_errors() {
    let setup = two_player_setup();
    let mut state = fewfc::domain::GameState::from_setup(&setup);

    let projected: () = apply_event(
        &mut state,
        &GameEvent::DeckPrepared {
            deck_order: vec![card(10), card(11)],
        },
    );

    assert_eq!(projected, ());
    assert_eq!(state.deck, vec![card(10), card(11)]);
}

#[test]
fn setup_validation_rejects_duplicate_card_instances_across_hands_and_deck() {
    let mut deck = official_deck();
    deck[1] = card(1);

    assert_eq!(
        GameRecord::start(two_player_setup(), deck),
        Err(GameError::Validation(ValidationError::DuplicateCard(card(
            1
        ))))
    );
}

#[test]
fn setup_validation_rejects_deck_card_without_instance_definition() {
    let mut setup = two_player_setup();
    setup
        .card_instances
        .retain(|instance_def| instance_def.instance != card(20));

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::MissingCardInstanceDefinition(card(20))
        ))
    );
}

#[test]
fn setup_validation_rejects_card_instance_with_unknown_definition() {
    let mut setup = two_player_setup();
    setup.card_instances[0].definition = CardDefId::new("missing");

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::MissingCardDefinition(CardDefId::new("missing"))
        ))
    );
}

#[test]
fn setup_validation_rejects_duplicate_card_instance_definitions() {
    let mut setup = two_player_setup();
    setup.card_instances.push(card_instance(1, "metal"));

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(ValidationError::DuplicateCard(card(
            1
        ))))
    );
}

#[test]
fn setup_validation_requires_hp_for_every_team() {
    let setup = GameSetup {
        ruleset: RulesetId::base(),
        enabled_rule_modules: Vec::new(),
        players: vec![Player {
            id: PlayerId::new("p1"),
            team: TeamId::new("A"),
        }],
        turn_order: vec![PlayerId::new("p1")],
        hp: Vec::new(),
        card_defs: Vec::new(),
        card_instances: Vec::new(),
        deck_lists: Vec::new(),
        hand_limit: 5,
        base_draw: 2,
    };

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(ValidationError::MissingTeamHp(
            TeamId::new("A")
        )))
    );
}

#[test]
fn setup_validation_rejects_team_mode_turn_order_that_is_not_alternating() {
    let setup = GameSetup {
        ruleset: RulesetId::base(),
        enabled_rule_modules: Vec::new(),
        players: vec![
            Player {
                id: PlayerId::new("p1"),
                team: TeamId::new("A"),
            },
            Player {
                id: PlayerId::new("p2"),
                team: TeamId::new("A"),
            },
            Player {
                id: PlayerId::new("p3"),
                team: TeamId::new("B"),
            },
            Player {
                id: PlayerId::new("p4"),
                team: TeamId::new("B"),
            },
        ],
        turn_order: vec![
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            PlayerId::new("p3"),
            PlayerId::new("p4"),
        ],
        hp: vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 30,
            },
        ],
        card_defs: Vec::new(),
        card_instances: Vec::new(),
        deck_lists: Vec::new(),
        hand_limit: 5,
        base_draw: 2,
    };

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::TeamSeatingNotAlternating {
                previous_player: PlayerId::new("p1"),
                player: PlayerId::new("p2"),
                team: TeamId::new("A"),
            }
        ))
    );
}

#[test]
fn setup_validation_requires_turn_order_to_contain_every_player_once() {
    let mut setup = two_player_setup();
    setup.turn_order = vec![PlayerId::new("p1"), PlayerId::new("p1")];

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::DuplicateTurnOrderPlayer(PlayerId::new("p1"))
        ))
    );

    let mut setup = two_player_setup();
    setup.turn_order = vec![PlayerId::new("p1")];

    assert_eq!(
        GameRecord::start(setup, official_deck()),
        Err(GameError::Validation(
            ValidationError::MissingTurnOrderPlayer(PlayerId::new("p2"))
        ))
    );
}

#[test]
fn setup_validation_rejects_invalid_team_mode_shapes() {
    assert_eq!(
        GameRecord::start(
            bare_team_setup(&[("p1", "A"), ("p2", "A"), ("p3", "B")]),
            official_deck()
        ),
        Err(GameError::Validation(
            ValidationError::TeamModeRequiresAtLeastFourPlayers { player_count: 3 }
        ))
    );

    assert_eq!(
        GameRecord::start(
            bare_team_setup(&[
                ("p1", "A"),
                ("p2", "B"),
                ("p3", "C"),
                ("p4", "A"),
                ("p5", "B"),
                ("p6", "C"),
            ]),
            deck_starting_with(&(1..=30).collect::<Vec<_>>()),
        ),
        Err(GameError::Validation(
            ValidationError::TeamModeRequiresExactlyTwoTeams { team_count: 3 }
        ))
    );

    assert_eq!(
        GameRecord::start(
            bare_team_setup(&[
                ("p1", "A"),
                ("p2", "B"),
                ("p3", "A"),
                ("p4", "B"),
                ("p5", "A"),
            ]),
            deck_starting_with(&(1..=30).collect::<Vec<_>>()),
        ),
        Err(GameError::Validation(
            ValidationError::TeamModeRequiresEqualTeamSizes {
                first_team: TeamId::new("A"),
                first_count: 3,
                second_team: TeamId::new("B"),
                second_count: 2,
            }
        ))
    );
}

#[test]
fn team_mode_builder_produces_valid_alternating_setup() {
    let setup = GameSetup::team_mode(
        TeamId::new("A"),
        vec![PlayerId::new("p1"), PlayerId::new("p3")],
        TeamId::new("B"),
        vec![PlayerId::new("p2"), PlayerId::new("p4")],
        30,
    );

    assert_eq!(
        setup.turn_order,
        vec![
            PlayerId::new("p1"),
            PlayerId::new("p2"),
            PlayerId::new("p3"),
            PlayerId::new("p4"),
        ]
    );
    assert_eq!(setup.ruleset, RulesetId::base());
    let card_setup = two_player_setup();
    GameRecord::start(
        setup.with_cards(card_setup.card_defs, card_setup.card_instances),
        official_deck(),
    )
    .unwrap();
}

#[test]
fn four_player_metal_strike_matrix_targets_the_cyclic_previous_opposing_team() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let p3 = PlayerId::new("p3");
    let p4 = PlayerId::new("p4");
    let card_setup = two_player_setup();
    let mut setup = GameSetup::team_mode(
        TeamId::new("A"),
        vec![p1.clone(), p3.clone()],
        TeamId::new("B"),
        vec![p2.clone(), p4.clone()],
        30,
    )
    .with_cards(card_setup.card_defs, card_setup.card_instances);
    setup.card_instances.extend((21..=30).map(|id| {
        card_instance(
            id,
            match id % 5 {
                1 => "metal",
                2 => "wood",
                3 => "water",
                4 => "fire",
                _ => "earth",
            },
        )
    }));
    let mut record = GameRecord::start(setup, (1..=30).map(card).collect()).unwrap();
    record.advance_automatic().unwrap();

    // 在 A/B 交錯座次 P1、P2、P3、P4 中，不宣告 target 的 Metal Strike 必須以
    // 環狀上一位 P4 為 target，並只扣除其 Team B 的 HP。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p4.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: vec![card(1)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("B"),
                    old_hp: 30,
                    delta: -7,
                    new_hp: 23,
                    effective_delta: -7,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p1.clone(),
                    attack: LastElementalAttack {
                        element: Element::Metal,
                        resolved_turn: 1,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
            },
        ]
    );
    assert_eq!(record.state().phase, Phase::TurnDraw);
    assert_eq!(record.state().current_player(), Some(&p1));
    assert_eq!(
        record.state().hp,
        vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 23,
            },
        ]
    );
    assert_eq!(record.state().hand(&p2).unwrap().len(), 5);
    assert_eq!(record.state().hand(&p3).unwrap().len(), 5);
    assert_eq!(record.state().hand(&p4).unwrap().len(), 5);
    assert_eq!(record.state().discard, vec![card(1)]);
    let observer_before_turn_end = record.public_view(Viewer::Observer).unwrap();
    assert_eq!(observer_before_turn_end.phase, Phase::TurnDraw);
    assert_eq!(
        observer_before_turn_end.hp,
        vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 23,
            },
        ]
    );
    assert_eq!(
        observer_before_turn_end.hands,
        vec![
            PublicPlayerHand {
                player: p1.clone(),
                cards: PublicCardRefs::Hidden { count: 3 },
            },
            PublicPlayerHand {
                player: p2.clone(),
                cards: PublicCardRefs::Hidden { count: 5 },
            },
            PublicPlayerHand {
                player: p3.clone(),
                cards: PublicCardRefs::Hidden { count: 5 },
            },
            PublicPlayerHand {
                player: p4.clone(),
                cards: PublicCardRefs::Hidden { count: 5 },
            },
        ]
    );

    // 以合法 Turn Draw choice 結束 P1 回合，讓 Public View 也保留此四人座次下的
    // 前一回合 Formation；P2/P3/P4 的手牌仍沒有被這次攻擊碰觸。
    advance_record_to_next_main_after_turn_draw(&mut record, card(20));
    assert_eq!(record.state().phase, Phase::ActiveEffects);
    assert_eq!(record.state().current_player(), Some(&p2));
    assert_eq!(
        record
            .public_view(Viewer::Observer)
            .unwrap()
            .previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: p1.clone(),
            formation_id: Some("metal-strike".to_string()),
            cards: PublicCardRefs::Known(vec![card(1)]),
        })
    );
    assert_eq!(record.state().hand(&p2).unwrap().len(), 5);
    assert_eq!(record.state().hand(&p3).unwrap().len(), 5);
    assert_eq!(record.state().hand(&p4).unwrap().len(), 5);
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn four_player_metal_strike_matrix_consumes_only_the_cyclic_previous_players_shield() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let p3 = PlayerId::new("p3");
    let p4 = PlayerId::new("p4");

    let setup = || {
        let card_setup = two_player_setup();
        let mut setup = GameSetup::team_mode(
            TeamId::new("A"),
            vec![p1.clone(), p3.clone()],
            TeamId::new("B"),
            vec![p2.clone(), p4.clone()],
            100,
        )
        .with_cards(card_setup.card_defs, card_setup.card_instances);
        setup.card_instances.extend((21..=60).map(|id| {
            card_instance(
                id,
                match id % 5 {
                    1 => "metal",
                    2 => "wood",
                    3 => "water",
                    4 => "fire",
                    _ => "earth",
                },
            )
        }));
        setup
    };
    let complete_deck = |prefix: &[u64]| {
        let mut deck = prefix.iter().copied().map(card).collect::<Vec<_>>();
        for id in 1..=60 {
            let candidate = card(id);
            if !deck.contains(&candidate) {
                deck.push(candidate);
            }
        }
        deck
    };
    let finish_turn = |record: &mut GameRecord, player: &PlayerId| {
        record.advance_automatic().unwrap();
        let discard = match record.state().pending_choice.as_ref() {
            Some(PendingChoice {
                kind: PendingChoiceKind::Card { cards, .. },
                ..
            }) => cards[0],
            other => panic!("expected Turn Draw card choice, got {other:?}"),
        };
        answer_record_choice(
            record,
            player.clone(),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    };

    let initial_generating = vec![card(1), card(3), card(2)];

    // 無 modifier 基準：同一個完整 P1→P2→P3→P4 cycle 中，P4 以 physical
    // Weapon 收尾，不會留下元素脈絡或 Shield；P1 第二回合的 Metal Strike 因而
    // 正常傷害 cyclic previous P4 所在的 Team B。
    let mut baseline = GameRecord::start(
        setup(),
        complete_deck(&[
            1, 3, 2, 6, // P1: Generating 後保留 Metal。
            21, 5, 7, 8, 9, // P2: Metal Strike bridge。
            15, 11, 13, 14, 20, // P3: Generating bridge。
            26, 16, 17, 18, 19, // P4: physical Weapon bridge。
        ]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "generating-formation".to_string(),
            cards: initial_generating.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, &p1);
    baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(21)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, &p2);
    baseline
        .handle(Command::PerformFormation {
            player: p3.clone(),
            formation_id: "generating-formation".to_string(),
            cards: vec![card(15), card(11), card(13)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, &p3);
    baseline
        .handle(Command::PerformFormation {
            player: p4.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(26), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, &p4);
    assert_eq!(baseline.state().current_player(), Some(&p1));
    assert_eq!(
        baseline
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p4.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: vec![card(6)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("B"),
                    old_hp: 100,
                    delta: -7,
                    new_hp: 93,
                    effective_delta: -7,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p1.clone(),
                    attack: LastElementalAttack {
                        element: Element::Metal,
                        resolved_turn: 5,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
            },
        ]
    );
    assert_eq!(baseline.state().shield(&p2), Some(0));
    assert_eq!(baseline.state().shield(&p4), Some(0));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    let p2_barrier = vec![card(7), card(22), card(21), card(24)];
    let p4_barrier = vec![card(17), card(27), card(26), card(29)];
    // 修飾本身：P2 與 P4 都以合法 Barrier 各自建立 44 Shield；P3 的非攻擊
    // bridge 不會翻開或消耗 P2 的 Shield。
    let mut interaction = GameRecord::start(
        setup(),
        complete_deck(&[
            1, 3, 2, 6, // P1
            7, 22, 21, 24, 5, // P2 Barrier
            15, 11, 13, 14, 20, // P3 Generating
            17, 27, 26, 29, 16, // P4 Barrier
        ]),
    )
    .unwrap();
    interaction.advance_automatic().unwrap();
    interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "generating-formation".to_string(),
            cards: initial_generating,
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut interaction, &p1);
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "barrier".to_string(),
                cards: p2_barrier.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "barrier".to_string(),
                cards: p2_barrier.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::ShieldChanged {
                player: p2.clone(),
                old_value: 0,
                delta: 44,
                new_value: 44,
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "barrier".to_string(),
                cards: p2_barrier.clone(),
            },
        ]
    );
    finish_turn(&mut interaction, &p2);
    interaction
        .handle(Command::PerformFormation {
            player: p3.clone(),
            formation_id: "generating-formation".to_string(),
            cards: vec![card(15), card(11), card(13)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut interaction, &p3);
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p4.clone(),
                formation_id: "barrier".to_string(),
                cards: p4_barrier.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p4.clone(),
                formation_id: "barrier".to_string(),
                cards: p4_barrier.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::ShieldChanged {
                player: p4.clone(),
                old_value: 0,
                delta: 44,
                new_value: 44,
            },
            GameEvent::FormationCardsDiscarded {
                player: p4.clone(),
                formation_id: "barrier".to_string(),
                cards: p4_barrier.clone(),
            },
        ]
    );
    finish_turn(&mut interaction, &p4);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Player(p3.clone()),
        Viewer::Player(p4.clone()),
        Viewer::Observer,
    ] {
        assert_eq!(
            interaction.public_view(viewer).unwrap().shields,
            vec![
                PlayerShield {
                    player: p1.clone(),
                    value: 0,
                },
                PlayerShield {
                    player: p2.clone(),
                    value: 44,
                },
                PlayerShield {
                    player: p3.clone(),
                    value: 0,
                },
                PlayerShield {
                    player: p4.clone(),
                    value: 44,
                },
            ]
        );
    }

    // 互動：P1 的第二回合 Metal Strike 只命中 cyclic previous P4 的 Shield。P2
    // 是同隊 sibling，仍完整保留 44；Team B 亦因 Shield absorption 維持 100 HP。
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p4.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: vec![card(6)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("B"),
                    old_hp: 100,
                    delta: 0,
                    new_hp: 100,
                    effective_delta: 0,
                },
                shield_change: Some(ShieldChangeDelta {
                    player: p4.clone(),
                    old_value: 44,
                    delta: -7,
                    new_value: 37,
                }),
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects {
                    outcome: AttackOutcome::AbsorbedByShield,
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: p1.clone(),
                        attack: LastElementalAttack {
                            element: Element::Metal,
                            resolved_turn: 5,
                        },
                    }),
                    ..Default::default()
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
            },
        ]
    );
    assert_eq!(interaction.state().phase, Phase::TurnDraw);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(interaction.state().shield(&p2), Some(44));
    assert_eq!(interaction.state().shield(&p4), Some(37));
    assert_eq!(
        interaction.state().hp,
        vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 100,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 100,
            },
        ]
    );
    for used in p2_barrier
        .iter()
        .chain(p4_barrier.iter())
        .chain([card(6)].iter())
    {
        assert!(interaction.state().discard.contains(used));
    }
    assert_eq!(
        interaction.public_view(Viewer::Observer).unwrap().shields,
        vec![
            PlayerShield {
                player: p1.clone(),
                value: 0,
            },
            PlayerShield {
                player: p2.clone(),
                value: 44,
            },
            PlayerShield {
                player: p3.clone(),
                value: 0,
            },
            PlayerShield {
                player: p4.clone(),
                value: 37,
            },
        ]
    );
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn rule_derived_attack_targets_reject_declared_targets_without_events() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(
        record.handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: vec![fewfc::domain::TargetDecl::Player(PlayerId::new("p2"))],
        }),
        Err(GameError::Validation(
            ValidationError::UnexpectedDeclaredTargets {
                formation_id: "metal-strike".to_string(),
            }
        ))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn rule_derived_active_spell_targets_reject_declared_targets_without_events() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[2, 7, 1, 4])).unwrap();
    record.advance_automatic().unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(
        record.handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "barrier".to_string(),
            cards: vec![card(2), card(7), card(1), card(4)],
            declared_targets: vec![fewfc::domain::TargetDecl::Player(PlayerId::new("p1"))],
        }),
        Err(GameError::Validation(
            ValidationError::UnexpectedDeclaredTargets {
                formation_id: "barrier".to_string(),
            }
        ))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn new_game_state_exposes_core_status_shields_and_passive_zones() {
    let record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let state = record.state().clone();

    assert_eq!(state.status, GameStatus::InProgress);
    assert_eq!(state.shield(&PlayerId::new("p1")), Some(0));
    assert_eq!(state.shield(&PlayerId::new("p2")), Some(0));
    assert!(no_covered(&state));
    assert!(state.statuses.is_empty());
}

#[test]
fn pending_choices_store_typed_answers_and_resolutions() {
    let choice = PendingChoice {
        choice_id: ChoiceId::new(7),
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::Card {
            cards: vec![card(1), card(2)],
            minimum: 1,
            maximum: 1,
            can_decline: false,
        },
    };

    assert_eq!(choice.clone(), choice);
    assert_eq!(
        PendingResolution::ChaosReturnTwo,
        PendingResolution::ChaosReturnTwo
    );
}

#[test]
fn automatic_draw_invariant_error_emits_no_events_and_leaves_state_unchanged() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::TurnDraw;
    let state_before = state.clone();

    assert_eq!(
        advance_state_automatic(&state),
        Err(GameError::EngineInvariant(
            EngineInvariantError::NotEnoughCards {
                needed: 3,
                available: 0,
            }
        ))
    );
    assert_eq!(state, state_before);
}

#[test]
fn duplicate_covered_passive_is_engine_invariant_before_command_validation() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    cover(&mut state, "p1", "defense", vec![card(2), card(7)], false);
    cover(&mut state, "p1", "seal", vec![card(3), card(8)], false);
    state.formation_areas.push(PlayerFormationArea {
        player: PlayerId::new("p1"),
        formation: state
            .formation_area(&PlayerId::new("p1"))
            .unwrap()
            .formation
            .clone(),
    });
    let state_before = state.clone();

    assert_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::NoCardsInHand,
            },
        ),
        Err(GameError::EngineInvariant(
            EngineInvariantError::DuplicateFormationArea {
                player: PlayerId::new("p1"),
            }
        ))
    );
    assert_eq!(state, state_before);
}

#[test]
fn turn_start_status_expiration_is_event_logged_before_turn_starts() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    apply_event(
        &mut state,
        &GameEvent::StatusAdded {
            status: StatusEffect {
                id: "cannot-act-p1".to_string(),
                owner: StatusOwner::Player(PlayerId::new("p1")),
                kind: "CannotAct".to_string(),
                value: None,
                duration: StatusDuration::UntilTurnStart {
                    player: PlayerId::new("p1"),
                },
            },
        },
    );

    let events = advance_state_automatic(&state).unwrap();
    assert_eq!(
        events,
        vec![
            GameEvent::StatusExpired {
                status_id: "cannot-act-p1".to_string(),
                owner: StatusOwner::Player(PlayerId::new("p1")),
                expired_at: StatusExpiryTiming::TurnStart {
                    player: PlayerId::new("p1"),
                },
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p1"),
                turn_number: 1,
            },
        ]
    );

    let mut replayed = GameState::from_setup(&setup);
    add_status(
        &mut replayed,
        StatusEffect {
            id: "cannot-act-p1".to_string(),
            owner: StatusOwner::Player(PlayerId::new("p1")),
            kind: "CannotAct".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnStart {
                player: PlayerId::new("p1"),
            },
        },
    );

    for event in &events {
        apply_event(&mut state, event);
        apply_event(&mut replayed, event);
    }
    assert!(state.statuses.is_empty());
    assert_eq!(state.phase, Phase::ActiveEffects);
    assert_eq!(replayed, state);
}

#[test]
fn turn_end_status_expiration_is_event_logged_before_turn_ends() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::TurnEnd;
    apply_event(
        &mut state,
        &GameEvent::StatusAdded {
            status: StatusEffect {
                id: "cannot-act-until-end".to_string(),
                owner: StatusOwner::Player(PlayerId::new("p1")),
                kind: "CannotAct".to_string(),
                value: None,
                duration: StatusDuration::UntilTurnEnd {
                    player: PlayerId::new("p1"),
                },
            },
        },
    );

    let events = advance_state_automatic(&state).unwrap();
    assert_eq!(
        events,
        vec![
            GameEvent::StatusExpired {
                status_id: "cannot-act-until-end".to_string(),
                owner: StatusOwner::Player(PlayerId::new("p1")),
                expired_at: StatusExpiryTiming::TurnEnd {
                    player: PlayerId::new("p1"),
                },
            },
            GameEvent::TurnEnded {
                player: PlayerId::new("p1"),
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p2"),
                turn_number: 2,
            },
        ]
    );

    let mut replayed = GameState::from_setup(&setup);
    replayed.phase = Phase::TurnEnd;
    add_status(
        &mut replayed,
        StatusEffect {
            id: "cannot-act-until-end".to_string(),
            owner: StatusOwner::Player(PlayerId::new("p1")),
            kind: "CannotAct".to_string(),
            value: None,
            duration: StatusDuration::UntilTurnEnd {
                player: PlayerId::new("p1"),
            },
        },
    );

    for event in &events {
        apply_event(&mut state, event);
        apply_event(&mut replayed, event);
    }
    assert!(state.statuses.is_empty());
    assert_eq!(state.phase, Phase::ActiveEffects);
    assert_eq!(state.current_player(), Some(&PlayerId::new("p2")));
    assert_eq!(replayed, state);
}

#[test]
fn permanent_statuses_do_not_expire_and_remain_visible_to_command_validation() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = record.state().clone();
    add_status(&mut state, cannot_act_status(PlayerId::new("p1")));

    assert_eq!(advance_state_automatic(&state).unwrap(), Vec::new());
    assert_event_semantics_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::CannotActByStatus,
            },
        )
        .unwrap(),
        vec![GameEvent::ActionPassed {
            player: PlayerId::new("p1"),
            reason: PassActionReason::CannotActByStatus,
        }]
    );
    assert_eq!(state.statuses, vec![cannot_act_status(PlayerId::new("p1"))]);
}

#[test]
fn invalid_command_returns_error_without_appending_events_or_changing_state() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    let result = record.handle(Command::AnswerChoice {
        player: PlayerId::new("p1"),
        choice_id: ChoiceId::new(1),
        answer: ChoiceAnswer::Cards {
            cards: vec![card(1)],
        },
    });

    assert_eq!(
        result,
        Err(GameError::Validation(ValidationError::MissingPendingChoice))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn pass_action_with_no_cards_consumes_the_turn_action_and_enters_turn_draw() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    apply_event(
        &mut state,
        &GameEvent::TurnStarted {
            player: PlayerId::new("p1"),
            turn_number: 1,
        },
    );

    assert_event_semantics_eq!(
        handle_command(
            &state,
            Command::PassAction {
                player: PlayerId::new("p1"),
                reason: PassActionReason::NoCardsInHand,
            },
        )
        .unwrap(),
        vec![GameEvent::ActionPassed {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        }]
    );
}

#[test]
fn pass_action_with_cards_is_rejected_without_changing_state() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    let result = record.handle(Command::PassAction {
        player: PlayerId::new("p1"),
        reason: PassActionReason::NoCardsInHand,
    });

    assert_eq!(
        result,
        Err(GameError::Validation(ValidationError::CannotPassAction {
            reason: PassActionReason::NoCardsInHand,
        }))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn choosing_turn_discard_rejects_cards_not_drawn_this_turn() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = state_after_cannot_act_pass(&record, PlayerId::new("p1"));
    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }
    let state_before = state.clone();

    assert_eq!(
        answer_choice(
            &state,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(2)]
            },
        ),
        Err(GameError::Validation(ValidationError::InvalidChoiceAnswer))
    );
    assert_eq!(state, state_before);
}

#[test]
fn turn_draw_is_skipped_when_hand_is_already_at_limit() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    let mut state = state_after_cannot_act_pass(&record, PlayerId::new("p1"));
    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }
    for event in answer_choice(
        &state,
        PlayerId::new("p1"),
        ChoiceAnswer::Cards {
            cards: vec![card(10)],
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }
    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    add_status(&mut state, cannot_act_status(PlayerId::new("p2")));
    for event in handle_command(
        &state,
        Command::PassAction {
            player: PlayerId::new("p2"),
            reason: PassActionReason::CannotActByStatus,
        },
    )
    .unwrap()
    {
        apply_event(&mut state, &event);
    }

    assert_eq!(
        advance_state_automatic(&state).unwrap(),
        vec![
            GameEvent::TurnDrawSkipped {
                player: PlayerId::new("p2"),
                reason: TurnDrawSkipReason::HandLimitReached,
            },
            GameEvent::TurnEnded {
                player: PlayerId::new("p2"),
            },
            GameEvent::TurnStarted {
                player: PlayerId::new("p1"),
                turn_number: 3,
            },
        ]
    );

    for event in advance_state_automatic(&state).unwrap() {
        apply_event(&mut state, &event);
    }

    assert_eq!(state.phase, Phase::ActiveEffects);
    assert_eq!(state.current_player(), Some(&PlayerId::new("p1")));
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(5), card(6), card(7), card(8), card(9)].as_slice())
    );
    assert_eq!(state.deck, (12..=20).map(card).collect::<Vec<_>>());
    assert!(state.pending_choice.is_none());
}

#[test]
fn turn_draw_recycles_discard_to_deck_bottom_when_deck_is_insufficient() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::TurnDraw;
    state.current_turn_index = 1;
    state.deck = vec![card(12)];
    state.discard = vec![card(2)];
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(
            PlayerId::new("p2"),
            vec![card(5), card(6), card(7), card(8)],
        ),
    ];

    let events = advance_state_automatic(&state).unwrap();
    assert!(matches!(
        semantic_events(&events).as_slice(),
        [GameEvent::RandomnessRequested { request, .. }]
            if request.operation.is_discard_shuffle()
                && request.current_order == vec![card(2)]
    ));
}

#[test]
fn triple_fire_level_sum_matrix_commits_resolves_and_replays() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = GameRecord::start(
        two_player_setup_with_hp(100),
        deck_starting_with(&[4, 9, 14, 1]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    // 三張 Fire 等級都是四；這個無 modifier 基準必須以完整合法 Command 產生
    // 等級總和十二乘三後的三十六點傷害。
    let events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "triple-fire".to_string(),
            cards: vec![card(4), card(9), card(14)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        events,
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(4), card(9), card(14)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "triple-fire".to_string(),
                used_cards: vec![card(4), card(9), card(14)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 36,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 36,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 100,
                    delta: -36,
                    new_hp: 64,
                    effective_delta: -36,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p1.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 1,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(4), card(9), card(14)],
            },
        ]
    );
    assert_eq!(record.state().phase, Phase::TurnDraw);
    assert_eq!(record.state().current_player(), Some(&p1));
    assert_eq!(record.state().discard, vec![card(4), card(9), card(14)]);
    assert_eq!(
        record.state().last_elemental_attack_by_player.get(&p1),
        Some(&LastElementalAttack {
            element: Element::Fire,
            resolved_turn: 1,
        })
    );
    let observer_view = record.public_view(Viewer::Observer).unwrap();
    assert_eq!(observer_view.phase, Phase::TurnDraw);
    assert_eq!(
        observer_view.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 100,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 64,
            },
        ]
    );
    assert_eq!(observer_view.discard, vec![card(4), card(9), card(14)]);
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn shock_burst_level_sum_matrix_has_no_elemental_context() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = GameRecord::start(
        two_player_setup_with_hp(100),
        deck_starting_with(&[4, 9, 3, 5]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    // Shock Burst 是非元素攻擊：四張已提交卡牌的等級總和十四乘四為五十六，但
    // 不能藉此寫入任何最後元素脈絡。
    let events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "shock-burst".to_string(),
            cards: vec![card(4), card(9), card(3), card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        events,
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "shock-burst".to_string(),
                cards: vec![card(4), card(9), card(3), card(5)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "shock-burst".to_string(),
                used_cards: vec![card(4), card(9), card(3), card(5)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 56,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 56,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 100,
                    delta: -56,
                    new_hp: 44,
                    effective_delta: -56,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects {
                    outcome: AttackOutcome::Resolved,
                    ..Default::default()
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "shock-burst".to_string(),
                cards: vec![card(4), card(9), card(3), card(5)],
            },
        ]
    );
    assert_eq!(record.state().phase, Phase::TurnDraw);
    assert_eq!(record.state().current_player(), Some(&p1));
    assert!(
        !record
            .state()
            .last_elemental_attack_by_player
            .contains_key(&p1)
    );
    assert_eq!(
        record.state().discard,
        vec![card(4), card(9), card(3), card(5)]
    );
    let observer_view = record.public_view(Viewer::Observer).unwrap();
    assert_eq!(observer_view.phase, Phase::TurnDraw);
    assert_eq!(
        observer_view.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 100,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 44,
            },
        ]
    );
    assert_eq!(
        observer_view.discard,
        vec![card(4), card(9), card(3), card(5)]
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn generating_formation_recovery_matrix_clamps_at_full_hp_and_heals_after_legal_damage() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let setup = two_player_setup_with_hp(100);
    let deck = deck_starting_with(&[1, 3, 2, 6, 4, 9, 14, 5, 11]);
    let initial_generating_cards = vec![card(1), card(3), card(2)];
    let recovery_generating_cards = vec![card(6), card(7), card(8)];

    // 無修飾基準：Generating 的完整合法流程仍會記錄請求的十八點回復；滿血的
    // effective delta 則正確為零，這與 HP clamp 不變量是分開的行為證據。
    let mut baseline = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    assert_eq!(
        baseline
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 100,
                    delta: 18,
                    new_hp: 100,
                    effective_delta: 0,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating_cards.clone(),
            },
        ]
    );
    assert_eq!(baseline.state().phase, Phase::TurnDraw);
    assert_eq!(baseline.state().current_player(), Some(&p1));
    assert_eq!(baseline.state().discard, initial_generating_cards);
    assert_eq!(
        baseline.public_view(Viewer::Observer).unwrap().hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 100,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 100,
            },
        ]
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    let advance_turn =
        |record: &mut GameRecord, player: &PlayerId, cards_to_keep: &[CardInstanceId]| {
            record.advance_automatic().unwrap();
            let discard = match record.state().pending_choice.as_ref() {
                Some(PendingChoice {
                    kind: PendingChoiceKind::Card { cards, .. },
                    ..
                }) => cards
                    .iter()
                    .copied()
                    .find(|card| !cards_to_keep.contains(card))
                    .expect("Turn Draw must offer a discard outside the next Formation"),
                other => panic!("expected Turn Draw card choice, got {other:?}"),
            };
            answer_record_choice(
                record,
                player.clone(),
                ChoiceAnswer::Cards {
                    cards: vec![discard],
                },
            )
            .unwrap();
            record.advance_automatic().unwrap();
        };

    let perform_injury_setup = |record: &mut GameRecord| {
        record.advance_automatic().unwrap();
        let initial_generating = record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap();
        advance_turn(record, &p1, &recovery_generating_cards);
        let triple_fire = record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(4), card(9), card(14)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        (initial_generating, triple_fire)
    };

    // 修飾本身：先以合法 Generating 作為非元素 bridge，再由 Triple Fire 建立 P1
    // 的受傷狀態；因 bridge 沒有元素攻擊脈絡，完整 attack outcome 是正常三十六點。
    let mut modifier = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    let (initial_generating, triple_fire) = perform_injury_setup(&mut modifier);
    assert_eq!(
        initial_generating,
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 100,
                    delta: 18,
                    new_hp: 100,
                    effective_delta: 0,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating_cards.clone(),
            },
        ]
    );
    assert_eq!(
        triple_fire,
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(4), card(9), card(14)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "triple-fire".to_string(),
                used_cards: vec![card(4), card(9), card(14)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 36,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 36,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 100,
                    delta: -36,
                    new_hp: 64,
                    effective_delta: -36,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 2,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(4), card(9), card(14)],
            },
        ]
    );
    assert_eq!(
        modifier
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(64)
    );
    assert_eq!(modifier.replay().unwrap(), modifier.state().clone());
    assert_eq!(modifier.verify_replay().unwrap(), modifier.state().clone());

    // 互動：以相同合法歷史抵達 P1 的下一個回合，第二組 Generating 現在完整回復
    // 十八點；P2 的既有 100 HP 及其他 side effect 都不受影響。
    let mut interaction = GameRecord::start(setup, deck).unwrap();
    perform_injury_setup(&mut interaction);
    advance_turn(&mut interaction, &p2, &[]);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    let recovery = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "generating-formation".to_string(),
            cards: recovery_generating_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        recovery,
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: recovery_generating_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 64,
                    delta: 18,
                    new_hp: 82,
                    effective_delta: 18,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: recovery_generating_cards.clone(),
            },
        ]
    );
    assert_eq!(interaction.state().phase, Phase::TurnDraw);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(
        interaction.state().hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 82,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 100,
            },
        ]
    );
    for used in [
        card(1),
        card(3),
        card(2),
        card(4),
        card(9),
        card(14),
        card(6),
        card(7),
        card(8),
    ] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(
        interaction.public_view(Viewer::Observer).unwrap().hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 82,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 100,
            },
        ]
    );
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn recovery_cannot_raise_team_hp_above_its_initial_value() {
    let mut record =
        GameRecord::start(two_player_setup_with_hp(20), deck_starting_with(&[1, 3, 2])).unwrap();
    record.advance_automatic().unwrap();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "generating-formation".to_string(),
            cards: vec![card(1), card(3), card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert_eq!(
        state
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .map(|team_hp| team_hp.hp),
        Some(20)
    );
    assert_eq!(state.discard, vec![card(1), card(3), card(2)]);
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn four_player_generating_matrix_recovers_only_its_own_team_after_previous_player_damage() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let p3 = PlayerId::new("p3");
    let p4 = PlayerId::new("p4");
    let card_setup = two_player_setup();
    let mut setup = GameSetup::team_mode(
        TeamId::new("A"),
        vec![p1.clone(), p3.clone()],
        TeamId::new("B"),
        vec![p2.clone(), p4.clone()],
        100,
    )
    .with_cards(card_setup.card_defs, card_setup.card_instances);
    setup.card_instances.extend((21..=40).map(|id| {
        card_instance(
            id,
            match id % 5 {
                1 => "metal",
                2 => "wood",
                3 => "water",
                4 => "fire",
                _ => "earth",
            },
        )
    }));
    let mut deck = vec![
        // P1: 初始 Generating，並保留第二組 Generating 的 Metal 牌。
        1, 3, 2, 26, // P2、P3：各自合法的非攻擊 Generating bridge。
        5, 6, 8, 9, 10, 15, 11, 13, 14, 20, // P4: 兩張 Metal Weapon；其餘是背景。
        16, 21, 17, 18, 19,
        // P1 的第一個 Turn Draw：第二組 Generating 與可捨棄背景牌。
        27, 28, 25, 22,
    ]
    .into_iter()
    .map(card)
    .collect::<Vec<_>>();
    for id in 1..=40 {
        let candidate = card(id);
        if !deck.contains(&candidate) {
            deck.push(candidate);
        }
    }
    let initial_generating = vec![card(1), card(3), card(2)];
    let recovery_generating = vec![card(26), card(27), card(28)];

    let finish_turn =
        |record: &mut GameRecord, player: &PlayerId, cards_to_keep: &[CardInstanceId]| {
            record.advance_automatic().unwrap();
            let discard = match record.state().pending_choice.as_ref() {
                Some(PendingChoice {
                    kind: PendingChoiceKind::Card { cards, .. },
                    ..
                }) => cards
                    .iter()
                    .copied()
                    .find(|card| !cards_to_keep.contains(card))
                    .expect("Turn Draw must offer a discard outside the next Formation"),
                other => panic!("expected Turn Draw card choice, got {other:?}"),
            };
            answer_record_choice(
                record,
                player.clone(),
                ChoiceAnswer::Cards {
                    cards: vec![discard],
                },
            )
            .unwrap();
            record.advance_automatic().unwrap();
        };

    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();

    // 無修飾基準：P1 的 Generating 只能作用於 own Team A。滿血時仍保留請求的
    // 十八點回復，但 effective delta 為零，Team B 完全不受影響。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("A"),
                    old_hp: 100,
                    delta: 18,
                    new_hp: 100,
                    effective_delta: 0,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: initial_generating.clone(),
            },
        ]
    );
    finish_turn(&mut record, &p1, &recovery_generating);

    // P2/P3 只用合法非攻擊 bridge 推進四人回合；它們不改變任何 Team HP。
    record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "generating-formation".to_string(),
            cards: vec![card(5), card(6), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut record, &p2, &[]);
    record
        .handle(Command::PerformFormation {
            player: p3.clone(),
            formation_id: "generating-formation".to_string(),
            cards: vec![card(15), card(11), card(13)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut record, &p3, &[]);

    // 修飾本身：在相同的交錯 Team 座次，P4 的 Weapon 必須傷害 cyclic previous
    // P3 所在 Team A，而不是 P2 或 Team B。
    assert_eq!(record.state().current_player(), Some(&p4));
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p4.clone(),
                formation_id: "weapon".to_string(),
                cards: vec![card(16), card(21)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p4.clone(),
                formation_id: "weapon".to_string(),
                cards: vec![card(16), card(21)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p4.clone(),
                target: p3.clone(),
                formation_id: "weapon".to_string(),
                used_cards: vec![card(16), card(21)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("A"),
                    old_hp: 100,
                    delta: -12,
                    new_hp: 88,
                    effective_delta: -12,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects {
                    outcome: AttackOutcome::Resolved,
                    ..Default::default()
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p4.clone(),
                formation_id: "weapon".to_string(),
                cards: vec![card(16), card(21)],
            },
        ]
    );
    finish_turn(&mut record, &p4, &[]);
    assert_eq!(record.state().current_player(), Some(&p1));
    assert_eq!(
        record
            .public_view(Viewer::Observer)
            .unwrap()
            .previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: p4.clone(),
            formation_id: Some("weapon".to_string()),
            cards: PublicCardRefs::Known(vec![card(16), card(21)]),
        })
    );

    // 互動：P1 的第二組合法 Generating 只回復受 P4 影響的 Team A。它請求十八點，
    // 但由 100 HP cap 將 effective delta 精確限制為十二；Team B 仍是 100。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: recovery_generating.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: recovery_generating.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("A"),
                    old_hp: 88,
                    delta: 18,
                    new_hp: 100,
                    effective_delta: 12,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "generating-formation".to_string(),
                cards: recovery_generating.clone(),
            },
        ]
    );
    assert_eq!(record.state().phase, Phase::TurnDraw);
    assert_eq!(record.state().current_player(), Some(&p1));
    assert_eq!(
        record.state().hp,
        vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 100,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 100,
            },
        ]
    );
    for used in initial_generating
        .iter()
        .chain([card(5), card(6), card(8)].iter())
        .chain([card(15), card(11), card(13)].iter())
        .chain([card(16), card(21)].iter())
        .chain(recovery_generating.iter())
    {
        assert!(record.state().discard.contains(used));
    }
    assert_eq!(
        record.public_view(Viewer::Observer).unwrap().hp,
        vec![
            TeamHp {
                team: TeamId::new("A"),
                hp: 100,
            },
            TeamHp {
                team: TeamId::new("B"),
                hp: 100,
            },
        ]
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn overcoming_formation_four_player_matrix_reduces_only_the_next_players_personal_shield() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let p3 = PlayerId::new("p3");
    let p4 = PlayerId::new("p4");
    let setup = || {
        let card_setup = two_player_setup();
        let mut setup = GameSetup::team_mode(
            TeamId::new("A"),
            vec![p1.clone(), p3.clone()],
            TeamId::new("B"),
            vec![p2.clone(), p4.clone()],
            100,
        )
        .with_cards(card_setup.card_defs, card_setup.card_instances);
        setup.card_instances.extend((21..=40).map(|id| {
            card_instance(
                id,
                match id % 5 {
                    1 => "metal",
                    2 => "wood",
                    3 => "water",
                    4 => "fire",
                    _ => "earth",
                },
            )
        }));
        setup
    };
    let complete_deck = |p3_barrier: bool| {
        let p3_cards = if p3_barrier {
            vec![12, 17, 11, 14, 13]
        } else {
            vec![11, 13, 12, 14, 17]
        };
        let mut deck = [
            vec![2, 7, 1, 4],         // P1 Barrier
            vec![10, 15, 26, 27, 30], // P2 Meta，之後保留剋陣
            p3_cards,
            vec![36, 37, 38, 19, 20], // P4 生陣 bridge
            vec![31, 6, 5],           // P1 第一個 Turn Draw，保留 Metal 31
        ]
        .concat()
        .into_iter()
        .map(card)
        .collect::<Vec<_>>();
        for id in 1..=40 {
            let candidate = card(id);
            if !deck.contains(&candidate) {
                deck.push(candidate);
            }
        }
        deck
    };
    let finish_turn =
        |record: &mut GameRecord, player: &PlayerId, discard: Option<CardInstanceId>| {
            record.advance_automatic().unwrap();
            let discard = discard.unwrap_or_else(|| match record.state().pending_choice.as_ref() {
                Some(PendingChoice {
                    kind: PendingChoiceKind::Card { cards, .. },
                    ..
                }) => cards[0],
                other => panic!("expected Turn Draw Card choice, got {other:?}"),
            });
            answer_record_choice(
                record,
                player.clone(),
                ChoiceAnswer::Cards {
                    cards: vec![discard],
                },
            )
            .unwrap();
            record.advance_automatic().unwrap();
        };
    let p1_barrier = vec![card(2), card(7), card(1), card(4)];
    let p2_meta = vec![card(10), card(15)];
    let p2_overcoming = vec![card(26), card(27), card(30)];
    let p3_barrier = vec![card(12), card(17), card(11), card(14)];

    let record_before_overcoming = |p3_uses_barrier: bool| {
        let mut record = GameRecord::start(setup(), complete_deck(p3_uses_barrier)).unwrap();
        record.advance_automatic().unwrap();
        assert_eq!(
            record
                .handle(Command::PerformFormation {
                    player: p1.clone(),
                    formation_id: "barrier".to_string(),
                    cards: p1_barrier.clone(),
                    declared_targets: Vec::new(),
                })
                .unwrap(),
            vec![
                GameEvent::FormationCommitted {
                    player: p1.clone(),
                    formation_id: "barrier".to_string(),
                    cards: p1_barrier.clone(),
                    star_substitution: None,
                    state: FormationAreaState::FaceUpResolving,
                },
                GameEvent::ShieldChanged {
                    player: p1.clone(),
                    old_value: 0,
                    delta: 44,
                    new_value: 44,
                },
                GameEvent::FormationCardsDiscarded {
                    player: p1.clone(),
                    formation_id: "barrier".to_string(),
                    cards: p1_barrier.clone(),
                },
            ]
        );
        finish_turn(&mut record, &p1, Some(card(6)));
        assert_eq!(record.state().current_player(), Some(&p2));
        assert_eq!(
            record
                .handle(Command::PerformFormation {
                    player: p2.clone(),
                    formation_id: "metamorphosis".to_string(),
                    cards: p2_meta.clone(),
                    declared_targets: Vec::new(),
                })
                .unwrap(),
            vec![
                GameEvent::FormationCommitted {
                    player: p2.clone(),
                    formation_id: "metamorphosis".to_string(),
                    cards: p2_meta.clone(),
                    star_substitution: None,
                    state: FormationAreaState::FaceUpResolving,
                },
                GameEvent::FormationEffectCopied {
                    player: p2.clone(),
                    effect_id: "barrier".to_string(),
                },
                GameEvent::ShieldChanged {
                    player: p2.clone(),
                    old_value: 0,
                    delta: 40,
                    new_value: 40,
                },
                GameEvent::FormationCardsDiscarded {
                    player: p2.clone(),
                    formation_id: "metamorphosis".to_string(),
                    cards: p2_meta.clone(),
                },
            ]
        );
        finish_turn(&mut record, &p2, None);

        if p3_uses_barrier {
            assert_eq!(
                record
                    .handle(Command::PerformFormation {
                        player: p3.clone(),
                        formation_id: "barrier".to_string(),
                        cards: p3_barrier.clone(),
                        declared_targets: Vec::new(),
                    })
                    .unwrap(),
                vec![
                    GameEvent::FormationCommitted {
                        player: p3.clone(),
                        formation_id: "barrier".to_string(),
                        cards: p3_barrier.clone(),
                        star_substitution: None,
                        state: FormationAreaState::FaceUpResolving,
                    },
                    GameEvent::ShieldChanged {
                        player: p3.clone(),
                        old_value: 0,
                        delta: 44,
                        new_value: 44,
                    },
                    GameEvent::FormationCardsDiscarded {
                        player: p3.clone(),
                        formation_id: "barrier".to_string(),
                        cards: p3_barrier.clone(),
                    },
                ]
            );
        } else {
            assert_eq!(
                record
                    .handle(Command::PerformFormation {
                        player: p3.clone(),
                        formation_id: "generating-formation".to_string(),
                        cards: vec![card(11), card(13), card(12)],
                        declared_targets: Vec::new(),
                    })
                    .unwrap(),
                vec![
                    GameEvent::FormationCommitted {
                        player: p3.clone(),
                        formation_id: "generating-formation".to_string(),
                        cards: vec![card(11), card(13), card(12)],
                        star_substitution: None,
                        state: FormationAreaState::FaceUpResolving,
                    },
                    GameEvent::HpChanged {
                        change: HpChangeDelta {
                            team: TeamId::new("A"),
                            old_hp: 100,
                            delta: 18,
                            new_hp: 100,
                            effective_delta: 0,
                        },
                    },
                    GameEvent::FormationCardsDiscarded {
                        player: p3.clone(),
                        formation_id: "generating-formation".to_string(),
                        cards: vec![card(11), card(13), card(12)],
                    },
                ]
            );
        }
        finish_turn(&mut record, &p3, None);
        record
            .handle(Command::PerformFormation {
                player: p4.clone(),
                formation_id: "generating-formation".to_string(),
                cards: vec![card(36), card(38), card(37)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, &p4, None);
        assert_eq!(record.state().current_player(), Some(&p1));
        assert!(record.state().hand(&p1).unwrap().contains(&card(31)));
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(31)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, &p1, None);
        assert_eq!(record.state().current_player(), Some(&p2));
        record
    };

    // 基準：P1/P2 都有自己合法建立的 Shield，但 P2 的下家 P3 沒有 Shield；剋陣
    // 仍完整提交，且不會誤傷任一其他 Player 的 Shield。
    let mut baseline = record_before_overcoming(false);
    assert_eq!(
        baseline
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "overcoming-formation".to_string(),
                cards: p2_overcoming.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "overcoming-formation".to_string(),
                cards: p2_overcoming.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "overcoming-formation".to_string(),
                cards: p2_overcoming.clone(),
            },
        ]
    );
    assert_eq!(baseline.state().shield(&p1), Some(44));
    assert_eq!(baseline.state().shield(&p2), Some(40));
    assert_eq!(baseline.state().shield(&p3), Some(0));
    assert_eq!(baseline.state().shield(&p4), Some(0));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 互動：P3 是 P2 在四人交替座次中的下家。P3 的 44 Shield 被三十點剋陣削為
    // 14；P1 的同隊 Shield 與 P2 自己的 Shield 都必須保持原值。
    let mut interaction = record_before_overcoming(true);
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "overcoming-formation".to_string(),
                cards: p2_overcoming.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "overcoming-formation".to_string(),
                cards: p2_overcoming.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::ShieldChanged {
                player: p3.clone(),
                old_value: 44,
                delta: -30,
                new_value: 14,
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "overcoming-formation".to_string(),
                cards: p2_overcoming.clone(),
            },
        ]
    );
    assert_eq!(interaction.state().phase, Phase::TurnDraw);
    assert_eq!(interaction.state().current_player(), Some(&p2));
    assert_eq!(interaction.state().shield(&p1), Some(44));
    assert_eq!(interaction.state().shield(&p2), Some(40));
    assert_eq!(interaction.state().shield(&p3), Some(14));
    assert_eq!(interaction.state().shield(&p4), Some(0));
    for used in p1_barrier
        .iter()
        .chain(p2_meta.iter())
        .chain(p3_barrier.iter())
        .chain([card(36), card(38), card(37), card(31)].iter())
        .chain(p2_overcoming.iter())
    {
        assert!(interaction.state().discard.contains(used));
    }
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Player(p3.clone()),
        Viewer::Player(p4.clone()),
        Viewer::Observer,
    ] {
        assert_eq!(
            interaction.public_view(viewer).unwrap().shields,
            vec![
                PlayerShield {
                    player: p1.clone(),
                    value: 44,
                },
                PlayerShield {
                    player: p2.clone(),
                    value: 40,
                },
                PlayerShield {
                    player: p3.clone(),
                    value: 14,
                },
                PlayerShield {
                    player: p4.clone(),
                    value: 0,
                },
            ]
        );
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn radiance_cannot_act_matrix_blocks_a_usable_formation_but_keeps_status_specific_pass_legal() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：P1 完成普通回合後，P2 擁有合法的 Wood Strike。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 4, 3, 2, 5, 7, 8, 9]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let baseline_events = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "wood-strike".to_string(),
            cards: vec![card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        baseline_events
            .iter()
            .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：Radiance 透過完整陣形命令建立；P2 仍持有相同可用的木卡，但只能
    // 執行狀態專屬的 Pass 行動。
    let mut radiance = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 4, 3, 2, 5, 7, 8, 9]),
    )
    .unwrap();
    radiance.advance_automatic().unwrap();
    let radiance_cards = vec![card(1), card(6), card(4), card(3)];
    let radiance_events = radiance
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "radiance".to_string(),
            cards: radiance_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    let inspected_cards = vec![card(2), card(5), card(7), card(8), card(9)];
    assert!(matches!(
        radiance_events.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::HandInspected { viewer, target, cards: snapshot },
            GameEvent::StatusAdded { status: cannot_act },
            GameEvent::StatusAdded { status: cannot_draw },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "radiance"
            && cards == &radiance_cards
            && viewer == &p1
            && target == &p2
            && snapshot == &inspected_cards
            && cannot_act.owner == StatusOwner::Player(p2.clone())
            && cannot_act.kind == "CannotAct"
            && cannot_draw.owner == StatusOwner::Player(p2.clone())
            && cannot_draw.kind == "CannotDraw"
            && discarded_by == &p1
            && discarded == "radiance"
            && discarded_cards == &radiance_cards
    ));
    let inspected = &radiance_events[1];
    assert_eq!(
        public_view::event_for(inspected, Viewer::Player(p1.clone())),
        PublicGameEvent::HandInspected {
            viewer: p1.clone(),
            target: p2.clone(),
            cards: PublicCardRefs::Known(inspected_cards.clone()),
        }
    );
    for viewer in [Viewer::Player(p2.clone()), Viewer::Observer] {
        assert_eq!(
            public_view::event_for(inspected, viewer),
            PublicGameEvent::HandInspected {
                viewer: p1.clone(),
                target: p2.clone(),
                cards: PublicCardRefs::Hidden { count: 5 },
            }
        );
    }
    let public_statuses = radiance.state().statuses.clone();
    assert_eq!(public_statuses.len(), 2);
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Observer,
    ] {
        assert_eq!(
            radiance.public_view(viewer).unwrap().statuses,
            public_statuses
        );
    }
    // 讓副本經過合法 Radiance 命令造成的標準事件。在 GameRecord 的便利迴圈提交
    // 強制 pass 前，它會抵達 P2 的 ActiveEffects，沒有手動製造狀態。
    let mut p2_turn = radiance.state().clone();
    while p2_turn.current_player() != Some(&p2) || p2_turn.phase != Phase::ActiveEffects {
        let automatic_events = advance_state_automatic(&p2_turn).unwrap();
        assert!(
            !automatic_events.is_empty(),
            "automatic progression must reach P2's command boundary"
        );
        for event in automatic_events {
            apply_event(&mut p2_turn, &event);
        }
        if let Some(choice) = p2_turn.pending_choice.clone() {
            let PendingChoiceKind::Card { cards, .. } = choice.kind else {
                panic!("P1's normal Turn Draw must use a Card choice");
            };
            for event in answer_choice(
                &p2_turn,
                choice.player,
                ChoiceAnswer::Cards {
                    cards: vec![cards[0]],
                },
            )
            .unwrap()
            {
                apply_event(&mut p2_turn, &event);
            }
        }
    }
    assert!(p2_turn.hand(&p2).unwrap().contains(&card(2)));
    assert_eq!(
        handle_command(
            &p2_turn,
            Command::PerformFormation {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(2)],
                declared_targets: Vec::new(),
            },
        ),
        Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::CannotActByStatus { player: p2.clone() },
            }
        ))
    );

    let progression_events = advance_record_to_next_main_after_turn_draw(&mut radiance, card(10));
    assert!(progression_events.iter().any(|event| matches!(
        event,
        GameEvent::ActionPassed { player, reason }
            if player == &p2 && reason == &PassActionReason::CannotActByStatus
    )));
    assert!(progression_events.iter().any(|event| matches!(
        event,
        GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::CannotDrawByStatus,
        } if player == &p2
    )));
    assert!(!progression_events.iter().any(|event| {
        matches!(
            event,
            GameEvent::CardsDrawnForTurnDiscardChoice { player, .. } if player == &p2
        ) || matches!(
            event,
            GameEvent::TurnDrawResolved { player, .. } if player == &p2
        )
    }));
    assert_eq!(radiance.state().hand(&p2), Some(inspected_cards.as_slice()));

    // P1 在兩個受影響的 P2 回合之間仍以合法 Command 行動；第二次 P2 Pass
    // 應再次跳過抽牌，並在 Turn End 一起讓兩個 Radiance status 自然到期。
    radiance
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "wood-strike".to_string(),
            cards: vec![card(12)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let second_pass_events = advance_record_to_next_main_after_turn_draw(&mut radiance, card(13));
    assert!(matches!(
        second_pass_events.as_slice(),
        [
            GameEvent::CardsDrawnForTurnDiscardChoice { player: draw_player, .. },
            GameEvent::ChoiceRequested { choice, .. },
            GameEvent::ChoiceMade { player: answered_by, .. },
            GameEvent::TurnDrawResolved { player: resolved_player, .. },
            GameEvent::TurnEnded { player: p1_ended },
            GameEvent::TurnStarted { player: p2_started, turn_number: 4 },
            GameEvent::ActionStarted { player: p2_action },
            GameEvent::ActionPassed { player: passed_by, reason: PassActionReason::CannotActByStatus },
            GameEvent::TurnDrawSkipped { player: skipped_by, reason: TurnDrawSkipReason::CannotDrawByStatus },
            GameEvent::StatusExpired { status_id: cannot_act_id, owner: StatusOwner::Player(cannot_act_owner), expired_at: StatusExpiryTiming::TurnEnd { player: cannot_act_expiry_player } },
            GameEvent::StatusExpired { status_id: cannot_draw_id, owner: StatusOwner::Player(cannot_draw_owner), expired_at: StatusExpiryTiming::TurnEnd { player: cannot_draw_expiry_player } },
            GameEvent::TurnEnded { player: p2_ended },
            GameEvent::TurnStarted { player: p1_started, turn_number: 5 },
        ] if draw_player == &p1
            && choice.player == p1
            && answered_by == &p1
            && resolved_player == &p1
            && p1_ended == &p1
            && p2_started == &p2
            && p2_action == &p2
            && passed_by == &p2
            && skipped_by == &p2
            && cannot_act_id == "radiance-cannot-act-p2-turn-1"
            && cannot_act_owner == &p2
            && cannot_act_expiry_player == &p2
            && cannot_draw_id == "radiance-cannot-draw-p2-turn-1"
            && cannot_draw_owner == &p2
            && cannot_draw_expiry_player == &p2
            && p2_ended == &p2
            && p1_started == &p1
    ));
    assert!(radiance.state().statuses.is_empty());
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Observer,
    ] {
        assert!(radiance.public_view(viewer).unwrap().statuses.is_empty());
    }
    assert_eq!(radiance.state().hand(&p2), Some(inspected_cards.as_slice()));

    // 修飾過期後，P1 的合法 bridge 讓 P2 回到正常 Action boundary；同一張先前
    // 被禁止的 Wood Card 現在必須完整解析，而非僅以可用 actions 查詢側面證明。
    radiance
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut radiance, card(16));
    assert_eq!(
        radiance
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(2)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(2)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "wood-strike".to_string(),
                used_cards: vec![card(2)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 6,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 6,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: -6,
                    new_hp: 24,
                    effective_delta: -6,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Wood,
                        resolved_turn: 6,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(2)],
            },
        ]
    );
    assert_eq!(radiance.replay().unwrap(), radiance.state().clone());
    assert_eq!(radiance.verify_replay().unwrap(), radiance.state().clone());
}

#[test]
fn defense_cannot_act_pass_matrix_flips_the_covered_passive_on_radiances_second_turn() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let matrix_deck = deck_starting_with(&[
        1, 6, 4, 3, // P1：Radiance
        5, 8, 9, 10, 11, // P2：保有可用的 Metal Strike
        2, 7, 12, // P1 的首次 Turn Draw：後續合法覆蓋 Defense
        13, 14, 15, // P1 覆蓋後的 Turn Draw
    ]);

    // baseline：沒有 CannotAct 時，P2 同一張 Metal Card 的正常 Formation 能完整
    // 解析；這不是因空手而得到的假性 Pass。
    let mut baseline = GameRecord::start(two_player_setup(), matrix_deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(12));
    let baseline_events = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            baseline_events.as_slice(),
            [
                GameEvent::FormationCommitted {
                    player,
                    formation_id,
                    cards,
                    state: FormationAreaState::FaceUpResolving,
                    ..
                },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team,
                    old_hp: 30,
                    delta: -7,
                    new_hp: 23,
                    effective_delta: -7,
                },
                ..
                },
                GameEvent::FormationCardsDiscarded {
                    player: discarded_by,
                    formation_id: discarded,
                    cards: discarded_cards,
                },
            ] if player == &p2
                && formation_id == "metal-strike"
            && cards == &vec![card(11)]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
                && discarded == "metal-strike"
                && discarded_cards == &vec![card(11)]
        ),
        "unexpected usable-formation baseline: {baseline_events:#?}"
    );
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|team| team.team == TeamId::new("team:p1"))
            .map(|team| team.hp),
        Some(23)
    );
    assert!(baseline.state().discard.contains(&card(11)));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // modifier：Radiance 是唯一建立 CannotAct 的來源。P2 有可用 Card，第一個
    // 受影響回合仍只能走狀態專屬 Pass，且此時尚未有可翻開的被動陣形。
    let mut interaction = GameRecord::start(two_player_setup(), matrix_deck).unwrap();
    interaction.advance_automatic().unwrap();
    let radiance_cards = vec![card(1), card(6), card(4), card(3)];
    let radiance_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "radiance".to_string(),
            cards: radiance_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            radiance_events.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::HandInspected { viewer, target, .. },
                GameEvent::StatusAdded { status: cannot_act },
                GameEvent::StatusAdded { status: cannot_draw },
                GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
            ] if player == &p1
            && formation_id == "radiance"
            && cards == &radiance_cards
            && viewer == &p1
                && target == &p2
                && cannot_act.owner == StatusOwner::Player(p2.clone())
                && cannot_act.kind == "CannotAct"
                && cannot_draw.owner == StatusOwner::Player(p2.clone())
                && cannot_draw.kind == "CannotDraw"
                && discarded_by == &p1
                && discarded == "radiance"
                && discarded_cards == &radiance_cards
        ),
        "unexpected legal Radiance lifecycle: {radiance_events:#?}"
    );
    let first_pass_events = advance_record_to_next_main_after_turn_draw(&mut interaction, card(12));
    assert!(first_pass_events.iter().any(|event| matches!(
        event,
        GameEvent::ActionPassed { player, reason }
            if player == &p2 && reason == &PassActionReason::CannotActByStatus
    )));
    assert!(first_pass_events.iter().any(|event| matches!(
        event,
        GameEvent::ActionStarted { player } if player == &p2
    )));
    assert!(first_pass_events.iter().any(|event| matches!(
        event,
        GameEvent::TurnDrawSkipped {
            player,
            reason: TurnDrawSkipReason::CannotDrawByStatus,
        } if player == &p2
    )));
    assert!(!first_pass_events.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped { .. }
            | GameEvent::AttackResolved { .. }
            | GameEvent::CardsMoved { .. }
    )));
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(interaction.state().phase, Phase::ActiveEffects);
    assert!(interaction.state().hand(&p2).unwrap().contains(&card(11)));
    assert!(interaction.state().statuses.iter().any(|status| {
        status.owner == StatusOwner::Player(p2.clone()) && status.kind == "CannotAct"
    }));

    // interaction：P1 在兩個 Radiance 回合之間透過合法 Command 覆蓋 Defense；P2
    // 第二次狀態專屬 Pass 是下一個 Action，故只產生一個 NotAnAttack ground。
    let defense_cards = vec![card(2), card(7)];
    let defense_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                state: FormationAreaState::FaceDownResolving,
                ..
            },
            GameEvent::PassiveCovered {
                player: owner,
                formation_id: covered,
                cards: covered_cards,
                sealed: false,
                ..
            },
        ] if player == &p1
            && formation_id == "defense"
            && cards == &defense_cards
            && owner == &p1
            && covered == "defense"
            && covered_cards == &defense_cards
    ));
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p1.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(defense_cards.clone()),
            star_substitution: None,
        }]
    );
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );

    let second_pass_events =
        advance_record_to_next_main_after_turn_draw(&mut interaction, card(13));
    assert!(
        matches!(
            second_pass_events.as_slice(),
            [
                GameEvent::CardsDrawnForTurnDiscardChoice {
                    player: draw_player,
                    drawn_cards,
                    allowed_discards,
                },
                GameEvent::ChoiceRequested {
                    choice: PendingChoice {
                        choice_id,
                        player: choice_player,
                        kind: PendingChoiceKind::Card {
                            cards,
                            minimum: 1,
                            maximum: 1,
                            can_decline: false,
                        },
                    },
                    resolution: PendingResolution::TurnDrawDiscard,
                },
                GameEvent::ChoiceMade {
                    player: answered_by,
                    choice_id: answered_choice,
                    answer: ChoiceAnswer::Cards { cards: chosen_cards },
                },
                GameEvent::TurnDrawResolved {
                    player: resolved_player,
                    discard,
                    kept_cards,
                },
                GameEvent::TurnEnded { player: p1_ended },
                GameEvent::TurnStarted {
                    player: p2_started,
                    turn_number: 4,
                },
                GameEvent::ActionStarted { player: p2_action },
                GameEvent::PassiveFlipped {
                    owner,
                    incoming_player,
                    passive_id,
                    cards: passive_cards,
                    outcome: PassiveFlipOutcome::NoEffect { .. },
                },
                GameEvent::ActionPassed {
                    player: passed_by,
                    reason: PassActionReason::CannotActByStatus,
                },
                GameEvent::TurnDrawSkipped {
                    player: skipped_by,
                    reason: TurnDrawSkipReason::CannotDrawByStatus,
                },
                GameEvent::StatusExpired {
                    status_id: cannot_act_id,
                    owner: StatusOwner::Player(cannot_act_owner),
                    expired_at: StatusExpiryTiming::TurnEnd { player: cannot_act_expiry_player },
                },
                GameEvent::StatusExpired {
                    status_id: cannot_draw_id,
                    owner: StatusOwner::Player(cannot_draw_owner),
                    expired_at: StatusExpiryTiming::TurnEnd { player: cannot_draw_expiry_player },
                },
                GameEvent::TurnEnded { player: p2_ended },
                GameEvent::TurnStarted {
                    player: p1_started,
                    turn_number: 5,
                },
            ] if draw_player == &p1
                && drawn_cards == &vec![card(13), card(14), card(15)]
                && allowed_discards == drawn_cards
                && choice_player == &p1
                && cards == drawn_cards
                && answered_by == &p1
                && answered_choice == choice_id
                && chosen_cards == &vec![card(13)]
                && resolved_player == &p1
                && discard == &card(13)
                && kept_cards == &vec![card(14), card(15)]
                && p1_ended == &p1
                && p2_started == &p2
                && p2_action == &p2
                && owner == &p1
                && incoming_player == &p2
                && passive_id == "defense"
                && passive_cards == &defense_cards
                && passed_by == &p2
                && skipped_by == &p2
                && cannot_act_id == "radiance-cannot-act-p2-turn-1"
                && cannot_act_owner == &p2
                && cannot_act_expiry_player == &p2
                && cannot_draw_id == "radiance-cannot-draw-p2-turn-1"
                && cannot_draw_owner == &p2
                && cannot_draw_expiry_player == &p2
                && p2_ended == &p2
                && p1_started == &p1
        ),
        "unexpected second Radiance pass lifecycle: {second_pass_events:#?}"
    );
    let grounds = match &second_pass_events[7] {
        GameEvent::PassiveFlipped {
            outcome: PassiveFlipOutcome::NoEffect { grounds },
            ..
        } => grounds,
        other => panic!("the exact sequence must flip Defense, got {other:?}"),
    };
    assert_eq!(grounds.len(), 1);
    assert_eq!(
        grounds
            .iter()
            .copied()
            .collect::<std::collections::HashSet<_>>(),
        std::collections::HashSet::from([PassiveNoEffectGround::NotAnAttack])
    );
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert!(
        interaction
            .state()
            .formation_area(&p1)
            .is_some_and(|area| area.formation.is_none())
    );
    assert!(interaction.state().statuses.is_empty());
    assert!(interaction.state().discard.contains(&card(2)));
    assert!(interaction.state().discard.contains(&card(7)));
    assert!(interaction.state().hand(&p2).unwrap().contains(&card(11)));
    assert!(!second_pass_events.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { .. } | GameEvent::CardsMoved { .. }
    )));
    assert!(
        public_view::state_for(interaction.state(), Viewer::Player(p1.clone()))
            .covered_passives
            .is_empty()
    );
    assert!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone()))
            .covered_passives
            .is_empty()
    );
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(interaction.state().phase, Phase::ActiveEffects);
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn metamorphosis_ignores_a_base_formation_from_an_older_turn() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.turn_number = 4;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5), card(10)]),
    ];
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "barrier".to_string(),
            resolved_effect_id: "barrier".to_string(),
            used_cards: vec![card(1), card(6)],
            resolved_turn: 2,
        },
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();

    assert!(
        !events
            .iter()
            .any(|event| matches!(event, GameEvent::FormationEffectCopied { .. }))
    );
    for event in &events {
        apply_event(&mut state, event);
    }
    assert_eq!(state.shield(&PlayerId::new("p2")), Some(0));
}

#[test]
fn metamorphosis_weapon_matrix_copies_a_legal_source_without_losing_its_own_identity() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 2, 3, 5, 10, 4, 7, 8]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    let weapon_cards = vec![card(1), card(6)];
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: weapon_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: weapon_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "weapon".to_string(),
                used_cards: weapon_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 30,
                    delta: -12,
                    new_hp: 18,
                    effective_delta: -12,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects::default()),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: weapon_cards.clone(),
            },
        ]
    );
    advance_record_to_next_main_after_turn_draw(&mut record, card(9));

    let metamorphosis_cards = vec![card(5), card(10)];
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::FormationEffectCopied {
                player: p2.clone(),
                effect_id: "weapon".to_string(),
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "weapon".to_string(),
                used_cards: metamorphosis_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 20,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 20,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: -20,
                    new_hp: 10,
                    effective_delta: -20,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects::default()),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
            },
        ]
    );

    let state = record.state().clone();
    assert!(state.pending_choice.is_none());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(state.shield(&p2), Some(0));
    assert_eq!(
        state.last_formation_by_player.get(&p2),
        Some(&LastFormationUse {
            formation_id: "metamorphosis".to_string(),
            resolved_effect_id: "weapon".to_string(),
            used_cards: metamorphosis_cards.clone(),
            resolved_turn: 2,
        })
    );
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 10,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 18,
            },
        ]
    );
    for used in weapon_cards.iter().chain(metamorphosis_cards.iter()) {
        assert!(state.discard.contains(used));
    }
    assert_eq!(
        record
            .public_view(Viewer::Observer)
            .unwrap()
            .previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: p1.clone(),
            formation_id: Some("weapon".to_string()),
            cards: PublicCardRefs::Known(weapon_cards),
        })
    );
    assert_eq!(record.replay().unwrap(), state);
    assert_eq!(record.verify_replay().unwrap(), state);
}

#[test]
fn metamorphosis_metal_strike_matrix_recomputes_the_copied_elemental_effect() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 2, 3, 4, 5, 10, 6, 7, 8]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    let source_cards = vec![card(1)];
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: source_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: source_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: source_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 30,
                    delta: -7,
                    new_hp: 23,
                    effective_delta: -7,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p1.clone(),
                    attack: LastElementalAttack {
                        element: Element::Metal,
                        resolved_turn: 1,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: source_cards.clone(),
            },
        ]
    );
    advance_record_to_next_main_after_turn_draw(&mut record, card(9));

    let metamorphosis_cards = vec![card(5), card(10)];
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::FormationEffectCopied {
                player: p2.clone(),
                effect_id: "metal-strike".to_string(),
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: metamorphosis_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 14,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::Same,
                    damage_transform: DamageTransform::HalfDamageRoundUp,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: -7,
                    new_hp: 23,
                    effective_delta: -7,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Metal,
                        resolved_turn: 2,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(
        state.last_formation_by_player.get(&p2),
        Some(&LastFormationUse {
            formation_id: "metamorphosis".to_string(),
            resolved_effect_id: "metal-strike".to_string(),
            used_cards: metamorphosis_cards.clone(),
            resolved_turn: 2,
        })
    );
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 23,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 23,
            },
        ]
    );
    for used in source_cards.iter().chain(metamorphosis_cards.iter()) {
        assert!(state.discard.contains(used));
    }
    assert_eq!(
        record
            .public_view(Viewer::Observer)
            .unwrap()
            .previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: p1.clone(),
            formation_id: Some("metal-strike".to_string()),
            cards: PublicCardRefs::Known(source_cards),
        })
    );
    assert_eq!(record.replay().unwrap(), state);
    assert_eq!(record.verify_replay().unwrap(), state);
}

#[test]
fn metamorphosis_weapon_chain_matrix_preserves_effect_identity_through_a_terminal_copy() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let weapon_cards = vec![card(1), card(6)];
    let p2_metamorphosis_cards = vec![card(15), card(20)];
    let p1_metamorphosis_cards = vec![card(5), card(10)];
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 5, 10, 15, 20, 2, 3, 4]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    // 基準來源：P1 的真實 Weapon 建立可被下一位複製的 resolved effect。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: weapon_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: weapon_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "weapon".to_string(),
                used_cards: weapon_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 30,
                    delta: -12,
                    new_hp: 18,
                    effective_delta: -12,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects::default()),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: weapon_cards.clone(),
            },
        ]
    );
    advance_record_to_next_main_after_turn_draw(&mut record, card(7));

    // 修飾本身：P2 的 Meta 以自己的 cards 重算 Weapon，卻保有 Meta 的 formation
    // identity；這也是下一個 Meta 必須看見的 immediate resolved effect。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: p2_metamorphosis_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: p2_metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::FormationEffectCopied {
                player: p2.clone(),
                effect_id: "weapon".to_string(),
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "weapon".to_string(),
                used_cards: p2_metamorphosis_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 20,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 20,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: -20,
                    new_hp: 10,
                    effective_delta: -20,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects::default()),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: p2_metamorphosis_cards.clone(),
            },
        ]
    );
    assert_eq!(
        record.state().last_formation_by_player.get(&p2),
        Some(&LastFormationUse {
            formation_id: "metamorphosis".to_string(),
            resolved_effect_id: "weapon".to_string(),
            used_cards: p2_metamorphosis_cards.clone(),
            resolved_turn: 2,
        })
    );
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut record, &p2);

    // 互動：P1 不可回溯到原始 Weapon；它合法複製 P2 剛解析的 Weapon effect。終局
    // GameEnded 直接接在攻擊後，因此 incoming Meta cards 仍留在 formation area。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: p1_metamorphosis_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: p1_metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::FormationEffectCopied {
                player: p1.clone(),
                effect_id: "weapon".to_string(),
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "weapon".to_string(),
                used_cards: p1_metamorphosis_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 20,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 20,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 18,
                    delta: -20,
                    new_hp: 0,
                    effective_delta: -18,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects::default()),
            },
            GameEvent::GameEnded {
                conclusion: GameConclusion::new(
                    GameOutcome::Winner(TeamId::new("team:p1")),
                    vec![GameEndCause::TeamHpDepleted {
                        teams: vec![TeamId::new("team:p2")],
                    }],
                    None,
                ),
            },
        ]
    );

    let state = record.state().clone();
    for (player, cards, turn) in [
        (p1.clone(), p1_metamorphosis_cards.clone(), 3),
        (p2.clone(), p2_metamorphosis_cards.clone(), 2),
    ] {
        assert_eq!(
            state.last_formation_by_player.get(&player),
            Some(&LastFormationUse {
                formation_id: "metamorphosis".to_string(),
                resolved_effect_id: "weapon".to_string(),
                used_cards: cards,
                resolved_turn: turn,
            })
        );
    }
    assert!(matches!(
        state.status,
        GameStatus::Finished { ref conclusion }
            if conclusion.outcome == GameOutcome::Winner(TeamId::new("team:p1"))
    ));
    assert_eq!(
        state.formation_area(&p1),
        Some(&PlayerFormationArea {
            player: p1.clone(),
            formation: Some(FormationInArea {
                formation_id: "metamorphosis".to_string(),
                cards: p1_metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            }),
        })
    );
    for used in weapon_cards.iter().chain(p2_metamorphosis_cards.iter()) {
        assert!(state.discard.contains(used));
    }
    for used in &p1_metamorphosis_cards {
        assert!(!state.discard.contains(used));
    }
    assert_eq!(
        record
            .public_view(Viewer::Observer)
            .unwrap()
            .previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: p2,
            formation_id: Some("metamorphosis".to_string()),
            cards: PublicCardRefs::Known(p2_metamorphosis_cards),
        })
    );
    assert_eq!(record.replay().unwrap(), state);
    assert_eq!(record.verify_replay().unwrap(), state);
}

#[test]
fn metamorphosis_five_streams_matrix_copies_target_hand_damage_and_independent_draw_bonus() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let bridge_cards = vec![card(11), card(16)];
    let source_cards = vec![card(1), card(6), card(21), card(26), card(31)];
    let metamorphosis_cards = vec![card(5), card(10)];
    let mut setup = two_player_setup_with_hp(100);
    // P2 先手用兩張 Weapon 作為不共享的合法 bridge；P1 隨後才拿到完整的
    // Five Streams。額外實體卡只固定合法手牌背景，不安排任何受測歷史或結果。
    setup.turn_order = vec![p2.clone(), p1.clone()];
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
    ]);
    let mut record = GameRecord::start(
        setup,
        deck_starting_with(&[11, 16, 5, 10, 1, 6, 21, 26, 31]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    // 背景 bridge 仍走完整命令：它保留 P2 的 Meta cards，並讓 P2 在第一次
    // Turn Draw 後恰有四張手牌，作為來源 Five Streams 的 target-hand baseline。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "weapon".to_string(),
                cards: bridge_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "weapon".to_string(),
                cards: bridge_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "weapon".to_string(),
                used_cards: bridge_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 100,
                    delta: -12,
                    new_hp: 88,
                    effective_delta: -12,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects::default()),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "weapon".to_string(),
                cards: bridge_cards.clone(),
            },
        ]
    );
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut record, &p2);
    assert_eq!(record.state().current_player(), Some(&p1));
    assert_eq!(record.state().hand(&p2).unwrap().len(), 4);

    // 基準來源：P1 以合法五張同等級卡片解析 Five Streams。P2 的四張手牌使
    // 傷害為 60；其獨立的 +1 draw bonus 必須進入同一個 AttackResolved outcome。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "five-streams-unite".to_string(),
                cards: source_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "five-streams-unite".to_string(),
                cards: source_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "five-streams-unite".to_string(),
                used_cards: source_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 60,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 60,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 100,
                    delta: -60,
                    new_hp: 40,
                    effective_delta: -60,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects {
                    turn_draw_bonus_changes: vec![TurnDrawBonusDelta {
                        player: p1.clone(),
                        old_value: 0,
                        delta: 1,
                        new_value: 1,
                    }],
                    ..Default::default()
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "five-streams-unite".to_string(),
                cards: source_cards.clone(),
            },
        ]
    );
    assert_eq!(record.state().turn_draw_bonus_by_player.get(&p1), Some(&1));

    let source_draw_events = record.advance_automatic().unwrap();
    let source_choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("Five Streams source must reach Turn Draw choice")
        .clone();
    let source_drawn_cards = match &source_choice.kind {
        PendingChoiceKind::Card { cards, .. } => cards.clone(),
        other => panic!("expected source turn-draw cards, got {other:?}"),
    };
    assert_eq!(source_drawn_cards.len(), 4);
    assert_eq!(
        source_draw_events,
        vec![
            GameEvent::CardsDrawnForTurnDiscardChoice {
                player: p1.clone(),
                drawn_cards: source_drawn_cards.clone(),
                allowed_discards: source_drawn_cards.clone(),
            },
            GameEvent::ChoiceRequested {
                choice: source_choice.clone(),
                resolution: PendingResolution::TurnDrawDiscard,
            },
        ]
    );
    let source_discard = source_drawn_cards[0];
    assert_eq!(
        record
            .handle(Command::AnswerChoice {
                player: p1.clone(),
                choice_id: source_choice.choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![source_discard],
                },
            })
            .unwrap(),
        vec![
            GameEvent::ChoiceMade {
                player: p1.clone(),
                choice_id: source_choice.choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![source_discard],
                },
            },
            GameEvent::TurnDrawResolved {
                player: p1.clone(),
                discard: source_discard,
                kept_cards: source_drawn_cards[1..].to_vec(),
            },
        ]
    );
    assert_eq!(record.state().hand(&p1).unwrap().len(), 3);
    assert_eq!(record.state().turn_draw_bonus_by_player.get(&p1), Some(&1));
    assert!(matches!(
        record.advance_automatic().unwrap().as_slice(),
        [
            GameEvent::TurnEnded { player: ended },
            GameEvent::TurnStarted { player: started, .. },
        ] if ended == &p1 && started == &p2
    ));
    assert!(record.state().turn_draw_bonus_by_player.get(&p1).is_none());
    assert_eq!(record.state().current_player(), Some(&p2));

    // 互動：P2 以自己合法的 Meta cards 複製 Five Streams。複製後重新讀取 P1
    // 剛保留的三張手牌，因此傷害是 45，且 P2 自己再取得一份 draw bonus。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::FormationEffectCopied {
                player: p2.clone(),
                effect_id: "five-streams-unite".to_string(),
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "five-streams-unite".to_string(),
                used_cards: metamorphosis_cards.clone(),
                point_breakdown: AttackPointBreakdown {
                    base_points: 45,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 45,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 88,
                    delta: -45,
                    new_hp: 43,
                    effective_delta: -45,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects {
                    turn_draw_bonus_changes: vec![TurnDrawBonusDelta {
                        player: p2.clone(),
                        old_value: 0,
                        delta: 1,
                        new_value: 1,
                    }],
                    ..Default::default()
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
            },
        ]
    );
    assert_eq!(
        record.state().last_formation_by_player.get(&p2),
        Some(&LastFormationUse {
            formation_id: "metamorphosis".to_string(),
            resolved_effect_id: "five-streams-unite".to_string(),
            used_cards: metamorphosis_cards.clone(),
            resolved_turn: 3,
        })
    );
    assert_eq!(record.state().turn_draw_bonus_by_player.get(&p2), Some(&1));

    let copied_draw_events = record.advance_automatic().unwrap();
    let copied_choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("copied Five Streams must reach Turn Draw choice")
        .clone();
    let copied_drawn_cards = match &copied_choice.kind {
        PendingChoiceKind::Card { cards, .. } => cards.clone(),
        other => panic!("expected copied turn-draw cards, got {other:?}"),
    };
    assert_eq!(copied_drawn_cards.len(), 4);
    assert_eq!(
        copied_draw_events,
        vec![
            GameEvent::CardsDrawnForTurnDiscardChoice {
                player: p2.clone(),
                drawn_cards: copied_drawn_cards.clone(),
                allowed_discards: copied_drawn_cards.clone(),
            },
            GameEvent::ChoiceRequested {
                choice: copied_choice.clone(),
                resolution: PendingResolution::TurnDrawDiscard,
            },
        ]
    );
    let copied_discard = copied_drawn_cards[0];
    record
        .handle(Command::AnswerChoice {
            player: p2.clone(),
            choice_id: copied_choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![copied_discard],
            },
        })
        .unwrap();
    assert_eq!(record.state().turn_draw_bonus_by_player.get(&p2), Some(&1));
    assert!(matches!(
        record.advance_automatic().unwrap().as_slice(),
        [
            GameEvent::TurnEnded { player: ended },
            GameEvent::TurnStarted { player: started, .. },
        ] if ended == &p2 && started == &p1
    ));
    assert!(record.state().turn_draw_bonus_by_player.get(&p2).is_none());

    let state = record.state().clone();
    assert_eq!(state.current_player(), Some(&p1));
    assert_eq!(state.phase, Phase::ActiveEffects);
    for used in bridge_cards
        .iter()
        .chain(source_cards.iter())
        .chain(metamorphosis_cards.iter())
        .chain([source_discard, copied_discard].iter())
    {
        assert!(state.discard.contains(used));
    }
    assert_eq!(
        record
            .public_view(Viewer::Observer)
            .unwrap()
            .previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: p2,
            formation_id: Some("metamorphosis".to_string()),
            cards: PublicCardRefs::Known(metamorphosis_cards),
        })
    );
    assert_eq!(record.replay().unwrap(), state);
    assert_eq!(record.verify_replay().unwrap(), state);
}

#[test]
fn metamorphosis_barrier_matrix_copies_the_spell_with_its_own_card_levels() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let source_cards = vec![card(2), card(7), card(1), card(4)];
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[2, 7, 1, 4, 5, 10, 6, 8, 9]),
    )
    .unwrap();
    record.advance_automatic().unwrap();
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "barrier".to_string(),
                cards: source_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "barrier".to_string(),
                cards: source_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::ShieldChanged {
                player: p1.clone(),
                old_value: 0,
                delta: 44,
                new_value: 44,
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "barrier".to_string(),
                cards: source_cards.clone(),
            },
        ]
    );
    advance_record_to_next_main_after_turn_draw(&mut record, card(3));

    let metamorphosis_cards = vec![card(5), card(10)];
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::FormationEffectCopied {
                player: p2.clone(),
                effect_id: "barrier".to_string(),
            },
            GameEvent::ShieldChanged {
                player: p2.clone(),
                old_value: 0,
                delta: 40,
                new_value: 40,
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(state.shield(&p1), Some(44));
    assert_eq!(state.shield(&p2), Some(40));
    assert_eq!(
        state.last_formation_by_player.get(&p2),
        Some(&LastFormationUse {
            formation_id: "metamorphosis".to_string(),
            resolved_effect_id: "barrier".to_string(),
            used_cards: metamorphosis_cards.clone(),
            resolved_turn: 2,
        })
    );
    for used in source_cards.iter().chain(metamorphosis_cards.iter()) {
        assert!(state.discard.contains(used));
    }
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Observer,
    ] {
        let view = record.public_view(viewer).unwrap();
        assert_eq!(
            view.shields,
            vec![
                PlayerShield {
                    player: p1.clone(),
                    value: 44,
                },
                PlayerShield {
                    player: p2.clone(),
                    value: 40,
                },
            ]
        );
        assert_eq!(
            view.previous_turn_formation,
            Some(PublicPreviousTurnFormation {
                player: p1.clone(),
                formation_id: Some("barrier".to_string()),
                cards: PublicCardRefs::Known(source_cards.clone()),
            })
        );
    }
    assert_eq!(record.replay().unwrap(), state);
    assert_eq!(record.verify_replay().unwrap(), state);
}

#[test]
fn metamorphosis_defense_matrix_establishes_and_consumes_a_public_delayed_counter() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：同一張合法 Metal Strike 沒有 counter 時正常傷害 P2。
    let mut baseline = GameRecord::start(
        two_player_setup_with_hp(100),
        deck_starting_with(&[1, 6, 4, 3, 2, 5, 7, 8, 9]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    assert_eq!(
        baseline
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: vec![card(1)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 100,
                    delta: -7,
                    new_hp: 93,
                    effective_delta: -7,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p1.clone(),
                    attack: LastElementalAttack {
                        element: Element::Metal,
                        resolved_turn: 1,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
            },
        ]
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    let defense_cards = vec![card(2), card(7)];
    let metamorphosis_cards = vec![card(5), card(10)];
    let mut interaction = GameRecord::start(
        two_player_setup_with_hp(100),
        deck_starting_with(&[2, 7, 3, 4, 5, 10, 6, 8, 9, 1, 11, 12]),
    )
    .unwrap();
    interaction.advance_automatic().unwrap();

    // 修飾本身：P1 只能用完整合法 Defense command 建立原始 covered passive。
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "defense".to_string(),
                cards: defense_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "defense".to_string(),
                cards: defense_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceDownResolving,
            },
            GameEvent::PassiveCovered {
                player: p1.clone(),
                formation_id: "defense".to_string(),
                cards: defense_cards.clone(),
                star_substitution: None,
                sealed: false,
            },
        ]
    );
    assert_eq!(
        interaction
            .public_view(Viewer::Player(p1.clone()))
            .unwrap()
            .covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(defense_cards.clone()),
            star_substitution: None,
        }]
    );
    for viewer in [Viewer::Player(p2.clone()), Viewer::Observer] {
        assert_eq!(
            interaction.public_view(viewer).unwrap().covered_passives,
            vec![PublicCoveredPassive {
                owner: p1.clone(),
                formation_id: None,
                cards: PublicCardRefs::Hidden { count: 2 },
                star_substitution: None,
            }]
        );
    }
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(11));

    // Meta 是 P2 的合法下一個 Action：P1 原 Defense 會以 NotAnAttack 正常翻開並
    // 消耗；P2 取得的是獨立、公開的 delayed counter，而非重用 P1 的 covered card。
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::PassiveFlipped {
                owner: p1.clone(),
                incoming_player: p2.clone(),
                passive_id: "defense".to_string(),
                cards: defense_cards.clone(),
                outcome: PassiveFlipOutcome::NoEffect {
                    grounds: vec![PassiveNoEffectGround::NotAnAttack],
                },
            },
            GameEvent::FormationEffectCopied {
                player: p2.clone(),
                effect_id: "defense".to_string(),
            },
            GameEvent::CounterEffectEstablished {
                owner: p2.clone(),
                effect_id: "defense".to_string(),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
            },
        ]
    );
    assert!(interaction.state().covered_passive(&p1).is_none());
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Observer,
    ] {
        assert!(
            interaction
                .public_view(viewer)
                .unwrap()
                .counter_effects
                .iter()
                .any(|counter| counter.owner == p2 && counter.effect_id == "defense")
        );
    }

    // 互動：P2 結束自己的合法 Turn Draw 後，P1 的 Metal Strike 觸發並消耗 P2
    // counter；攻擊依舊完整 commitment/discard，但只有傷害變為零。
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut interaction, &p2);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::CounterEffectResolved {
                owner: p2.clone(),
                incoming_player: p1.clone(),
                effect_id: "defense".to_string(),
                outcome: PassiveFlipOutcome::Applied {
                    effect_id: "defense".to_string(),
                    modifications: vec![ActionModification::PreventDamage],
                },
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: vec![card(1)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 100,
                    delta: 0,
                    new_hp: 100,
                    effective_delta: 0,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects {
                    outcome: AttackOutcome::DamagePrevented,
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: p1.clone(),
                        attack: LastElementalAttack {
                            element: Element::Metal,
                            resolved_turn: 3,
                        },
                    }),
                    ..Default::default()
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
            },
        ]
    );
    assert!(interaction.state().counter_effects.is_empty());
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Observer,
    ] {
        assert!(
            interaction
                .public_view(viewer)
                .unwrap()
                .counter_effects
                .is_empty()
        );
    }
    assert_eq!(interaction.state().phase, Phase::TurnDraw);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    for used in defense_cards
        .iter()
        .chain(metamorphosis_cards.iter())
        .chain([card(1)].iter())
    {
        assert!(interaction.state().discard.contains(used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn metamorphosis_empty_city_matrix_copies_without_establishing_a_delayed_counter() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let empty_city_cards = vec![card(1), card(2)];
    let metamorphosis_cards = vec![card(5), card(10)];
    let mut record = GameRecord::start(
        two_player_setup_with_hp(100),
        deck_starting_with(&[1, 2, 3, 4, 5, 10, 7, 8, 9, 6, 11, 12]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    // 修飾本身：空城必須由 P1 的合法 covered passive command 建立；P1 保留後續
    // Metal Strike 所需的 card 6，不能以 fixture 塞入 last formation。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "empty-city".to_string(),
                cards: empty_city_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "empty-city".to_string(),
                cards: empty_city_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceDownResolving,
            },
            GameEvent::PassiveCovered {
                player: p1.clone(),
                formation_id: "empty-city".to_string(),
                cards: empty_city_cards.clone(),
                star_substitution: None,
                sealed: false,
            },
        ]
    );
    advance_record_to_next_main_after_turn_draw(&mut record, card(11));

    // 互動：Meta 合法翻開並複製空城的 effect identity，但空城沒有可建立的 delayed
    // counter。Defense positive sibling 在 metamorphosis_defense_matrix_... 已證明同一
    // lifecycle 確實會建立及消耗 counter。
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::PassiveFlipped {
                owner: p1.clone(),
                incoming_player: p2.clone(),
                passive_id: "empty-city".to_string(),
                cards: empty_city_cards.clone(),
                outcome: PassiveFlipOutcome::NoEffect {
                    grounds: vec![PassiveNoEffectGround::EmptyCity],
                },
            },
            GameEvent::FormationEffectCopied {
                player: p2.clone(),
                effect_id: "empty-city".to_string(),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "metamorphosis".to_string(),
                cards: metamorphosis_cards.clone(),
            },
        ]
    );
    assert!(record.state().covered_passive(&p1).is_none());
    assert_eq!(
        record.state().last_formation_by_player.get(&p2),
        Some(&LastFormationUse {
            formation_id: "metamorphosis".to_string(),
            resolved_effect_id: "empty-city".to_string(),
            used_cards: metamorphosis_cards.clone(),
            resolved_turn: 2,
        })
    );
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Observer,
    ] {
        assert!(
            record
                .public_view(viewer)
                .unwrap()
                .counter_effects
                .is_empty()
        );
    }

    // 空城複製後 P1 的下一個合法 Metal Strike 仍完整正常傷害；此 exact outcome
    // 同時排除 hidden counter 被消耗、或只檢查 final state 而漏掉的 canonical event。
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut record, &p2);
    assert_eq!(record.state().current_player(), Some(&p1));
    assert_eq!(
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: vec![card(6)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 100,
                    delta: -7,
                    new_hp: 93,
                    effective_delta: -7,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p1.clone(),
                    attack: LastElementalAttack {
                        element: Element::Metal,
                        resolved_turn: 3,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
            },
        ]
    );
    assert!(record.state().counter_effects.is_empty());
    assert_eq!(record.state().phase, Phase::TurnDraw);
    for used in empty_city_cards
        .iter()
        .chain(metamorphosis_cards.iter())
        .chain([card(6)].iter())
    {
        assert!(record.state().discard.contains(used));
    }
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn chaos_requests_two_next_player_hand_cards_and_returns_them_to_deck_top() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 2, 1])).unwrap();
    record.advance_automatic().unwrap();

    assert_event_semantics_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "chaos".to_string(),
                cards: vec![card(5), card(10), card(2), card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationPerformed {
                player: PlayerId::new("p1"),
                formation_id: "chaos".to_string(),
                used_cards: vec![card(5), card(10), card(2), card(1)],
                declared_targets: Vec::new(),
            },
            GameEvent::HandInspected {
                viewer: PlayerId::new("p1"),
                target: PlayerId::new("p2"),
                cards: vec![card(3), card(4), card(6), card(7), card(8)],
            },
            GameEvent::ChoiceRequested {
                choice: PendingChoice {
                    choice_id: ChoiceId::new(1),
                    player: PlayerId::new("p1"),
                    kind: PendingChoiceKind::Card {
                        cards: vec![card(3), card(4), card(6), card(7), card(8)],
                        minimum: 2,
                        maximum: 2,
                        can_decline: false,
                    },
                },
                resolution: PendingResolution::ChaosReturnTwo,
            },
        ]
    );

    assert_eq!(
        answer_record_choice(
            &mut record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(3)]
            },
        ),
        Err(GameError::Validation(ValidationError::InvalidChoiceAnswer))
    );
    assert_eq!(
        answer_record_choice(
            &mut record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(3), card(3)]
            },
        ),
        Err(GameError::Validation(ValidationError::InvalidChoiceAnswer))
    );

    assert_event_semantics_eq!(
        answer_record_choice(
            &mut record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(3), card(4)]
            },
        )
        .unwrap(),
        vec![
            GameEvent::ChoiceMade {
                player: PlayerId::new("p1"),
                choice_id: ChoiceId::new(1),
                answer: ChoiceAnswer::Cards {
                    cards: vec![card(3), card(4)],
                },
            },
            GameEvent::CardsMoved {
                card_moves: vec![
                    CardMoveDelta {
                        card: card(4),
                        from: CardZone::Hand(PlayerId::new("p2")),
                        to: CardZone::DeckTop,
                    },
                    CardMoveDelta {
                        card: card(3),
                        from: CardZone::Hand(PlayerId::new("p2")),
                        to: CardZone::DeckTop,
                    },
                ],
            },
        ]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(6), card(7), card(8)].as_slice())
    );
    assert_eq!(
        state.deck.iter().take(2).copied().collect::<Vec<_>>(),
        vec![card(3), card(4)]
    );
    assert!(state.pending_choice.is_none());
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn return_to_origin_recovery_matrix_heals_after_a_legal_previous_player_attack() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let return_cards = vec![card(3), card(8), card(5), card(2)];

    // 無修飾基準：Return to Origin 的完整 active-spell lifecycle 請求三十六點
    // 回復；滿 HP 時可觀察到 requested/effective delta 的差異。
    let mut baseline = GameRecord::start(
        two_player_setup_with_hp(100),
        deck_starting_with(&[3, 8, 5, 2]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    assert_eq!(
        baseline
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "return-to-origin".to_string(),
                cards: return_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "return-to-origin".to_string(),
                cards: return_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 100,
                    delta: 36,
                    new_hp: 100,
                    effective_delta: 0,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "return-to-origin".to_string(),
                cards: return_cards.clone(),
            },
        ]
    );
    assert_eq!(baseline.state().phase, Phase::TurnDraw);
    assert_eq!(baseline.state().current_player(), Some(&p1));
    assert_eq!(baseline.state().discard, return_cards.clone());
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾本身：調整合法 turn order，讓 P2 先以 Triple Fire 完整傷害 P1；這不是
    // fixture 直接寫入 HP 或上一回合結果。
    let mut p2_first_setup = two_player_setup_with_hp(100);
    p2_first_setup.turn_order = vec![p2.clone(), p1.clone()];
    let mut interaction = GameRecord::start(
        p2_first_setup,
        deck_starting_with(&[4, 9, 14, 1, 3, 8, 5, 2, 6]),
    )
    .unwrap();
    interaction.advance_automatic().unwrap();
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(4), card(9), card(14)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(4), card(9), card(14)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "triple-fire".to_string(),
                used_cards: vec![card(4), card(9), card(14)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 36,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 36,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 100,
                    delta: -36,
                    new_hp: 64,
                    effective_delta: -36,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 1,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(4), card(9), card(14)],
            },
        ]
    );
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut interaction, &p2);
    assert_eq!(interaction.state().current_player(), Some(&p1));

    // 互動：P1 的合法 Return to Origin 回復自身 Team 的三十六點；P2 沒有受影響。
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "return-to-origin".to_string(),
                cards: return_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "return-to-origin".to_string(),
                cards: return_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 64,
                    delta: 36,
                    new_hp: 100,
                    effective_delta: 36,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "return-to-origin".to_string(),
                cards: return_cards.clone(),
            },
        ]
    );
    assert_eq!(interaction.state().phase, Phase::TurnDraw);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(
        interaction.state().hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 100,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 100,
            },
        ]
    );
    for used in [
        card(4),
        card(9),
        card(14),
        card(3),
        card(8),
        card(5),
        card(2),
    ] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(
        interaction.public_view(Viewer::Observer).unwrap().hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 100,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 100,
            },
        ]
    );
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn five_elements_cycle_matrix_swaps_hp_after_a_legal_low_point_triple_fire() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let p2_first_setup = || {
        let mut setup = two_player_setup_with_hp(27);
        setup.turn_order = vec![p2.clone(), p1.clone()];
        setup
    };
    let cycle_cards = vec![card(1), card(2), card(3), card(4), card(5)];

    // 無修飾基準：P2 合法 Generating bridge 後，兩個 Team 都是 27，因此 Cycle
    // 仍完整提交；沒有實際變化時 canonical record 不會製造零值 HP event。
    let mut baseline = GameRecord::start(
        p2_first_setup(),
        deck_starting_with(&[6, 8, 7, 9, 1, 2, 3, 4, 5]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "generating-formation".to_string(),
            cards: vec![card(6), card(8), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut baseline, &p2);
    assert_eq!(
        baseline
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "five-elements-cycle".to_string(),
                cards: cycle_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "five-elements-cycle".to_string(),
                cards: cycle_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "five-elements-cycle".to_string(),
                cards: cycle_cards.clone(),
            },
        ]
    );
    assert_eq!(
        baseline.state().hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 27,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 27,
            },
        ]
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾本身：三張自訂 printed-level Fire 卡仍透過真實 Command 組成 Triple
    // Fire；等級 1+1+3 的十五點傷害把 P1 從 27 合法降為 12。
    let mut interaction_setup = p2_first_setup();
    interaction_setup.card_defs.extend([
        CardDef {
            id: CardDefId::new("fire-one"),
            name: "fire-one".to_string(),
            element: Element::Fire,
            level: fewfc::domain::PrintedCardLevel::new(1),
        },
        CardDef {
            id: CardDefId::new("fire-three"),
            name: "fire-three".to_string(),
            element: Element::Fire,
            level: fewfc::domain::PrintedCardLevel::new(3),
        },
    ]);
    interaction_setup.card_instances.extend([
        CardInstanceDef {
            instance: card(21),
            definition: CardDefId::new("fire-one"),
            origin: Default::default(),
        },
        CardInstanceDef {
            instance: card(22),
            definition: CardDefId::new("fire-one"),
            origin: Default::default(),
        },
        CardInstanceDef {
            instance: card(23),
            definition: CardDefId::new("fire-three"),
            origin: Default::default(),
        },
    ]);
    let mut deck = vec![
        card(21),
        card(22),
        card(23),
        card(6),
        card(1),
        card(2),
        card(3),
        card(4),
        card(5),
    ];
    for id in 1..=23 {
        let candidate = card(id);
        if !deck.contains(&candidate) {
            deck.push(candidate);
        }
    }
    let mut interaction = GameRecord::start(interaction_setup, deck).unwrap();
    interaction.advance_automatic().unwrap();
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(21), card(22), card(23)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(21), card(22), card(23)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "triple-fire".to_string(),
                used_cards: vec![card(21), card(22), card(23)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 15,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 15,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 27,
                    delta: -15,
                    new_hp: 12,
                    effective_delta: -15,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 1,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "triple-fire".to_string(),
                cards: vec![card(21), card(22), card(23)],
            },
        ]
    );
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut interaction, &p2);

    // 互動：P1 Cycle 的兩個 typed HP changes 要同時交換目前的兩隊值，不只是
    // 以最終 state 側面證明結果。
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "five-elements-cycle".to_string(),
                cards: cycle_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "five-elements-cycle".to_string(),
                cards: cycle_cards.clone(),
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 12,
                    delta: 15,
                    new_hp: 27,
                    effective_delta: 15,
                },
            },
            GameEvent::HpChanged {
                change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 27,
                    delta: -15,
                    new_hp: 12,
                    effective_delta: -15,
                },
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "five-elements-cycle".to_string(),
                cards: cycle_cards.clone(),
            },
        ]
    );
    assert_eq!(interaction.state().phase, Phase::TurnDraw);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(
        interaction.state().hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 27,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 12,
            },
        ]
    );
    for used in [
        card(21),
        card(22),
        card(23),
        card(1),
        card(2),
        card(3),
        card(4),
        card(5),
    ] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(
        interaction.public_view(Viewer::Observer).unwrap().hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 27,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 12,
            },
        ]
    );
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn answering_effect_choice_resumes_resolution_deterministically() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 2, 1])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "chaos".to_string(),
            cards: vec![card(5), card(10), card(2), card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    assert_event_semantics_eq!(
        answer_record_choice(
            &mut record,
            PlayerId::new("p1"),
            ChoiceAnswer::Cards {
                cards: vec![card(3), card(4)]
            },
        )
        .unwrap(),
        vec![
            GameEvent::ChoiceMade {
                player: PlayerId::new("p1"),
                choice_id: ChoiceId::new(1),
                answer: ChoiceAnswer::Cards {
                    cards: vec![card(3), card(4)]
                },
            },
            GameEvent::CardsMoved {
                card_moves: vec![
                    CardMoveDelta {
                        card: card(4),
                        from: CardZone::Hand(PlayerId::new("p2")),
                        to: CardZone::DeckTop,
                    },
                    CardMoveDelta {
                        card: card(3),
                        from: CardZone::Hand(PlayerId::new("p2")),
                        to: CardZone::DeckTop,
                    },
                ],
            },
        ]
    );

    let state = record.state().clone();
    assert!(state.pending_choice.is_none());
    assert_eq!(state.phase, Phase::TurnDraw);
    assert_eq!(
        state.hand(&PlayerId::new("p2")),
        Some(vec![card(6), card(7), card(8)].as_slice())
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn commands_and_automatic_advance_wait_while_effect_choice_is_pending() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 2, 1])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "chaos".to_string(),
            cards: vec![card(5), card(10), card(2), card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(
        record.handle(Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::NoCardsInHand,
        }),
        Err(GameError::Validation(
            ValidationError::PendingChoiceInProgress {
                player: PlayerId::new("p1"),
            }
        ))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
    assert_eq!(record.advance_automatic().unwrap(), Vec::new());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn empty_city_next_action_matrix_consumes_its_intentional_no_effect_while_attack_resolves() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);

    // 基準：下一個合法 Fire Strike 沒有被動可觸發，P1 完成普通行動/回合抽牌後
    // 正常造成傷害。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let baseline_attack = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        !baseline_attack
            .iter()
            .any(|event| matches!(event, GameEvent::PassiveFlipped { .. }))
    );
    assert!(baseline_attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved { hp_change: HpChangeDelta { effective_delta, .. }, .. }
            if *effective_delta < 0
    )));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // Empty City 由合法被動命令建立。它刻意的無效果結果不能抑制下一個 incoming
    // Attack。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    let cover = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(1), card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        cover.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p1
            && formation_id == "empty-city"
            && cards == &vec![card(1), card(2)]
            && owner == &p1
            && covered == "empty-city"
            && covered_cards == &vec![card(1), card(2)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(10));
    let hp_before = interaction
        .state()
        .hp
        .iter()
        .find(|team| team.team == TeamId::new("team:p1"))
        .map(|team| team.hp)
        .unwrap();
    let attack = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: passive_cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            },
            GameEvent::AttackResolved { attacker, target, hp_change: HpChangeDelta { team, effective_delta, .. }, .. },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-strike"
            && cards == &vec![card(9)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "empty-city"
            && passive_cards == &vec![card(1), card(2)]
            && grounds == &vec![PassiveNoEffectGround::EmptyCity]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && *effective_delta < 0
            && discarded_by == &p2
            && discarded == "fire-strike"
            && discarded_cards == &vec![card(9)]
    ));
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert!(
        interaction
            .state()
            .hp
            .iter()
            .find(|team| team.team == TeamId::new("team:p1"))
            .is_some_and(|team| team.hp < hp_before)
    );
    for card in [card(1), card(2), card(9)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn player_cannot_cover_second_passive_while_one_is_pending() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), vec![card(2), card(7)]),
        PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];
    cover(&mut state, "p1", "seal", vec![card(3), card(8)], false);
    let state_before = state.clone();

    assert_eq!(
        handle_command(
            &state,
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "defense".to_string(),
                cards: vec![card(2), card(7)],
                declared_targets: Vec::new(),
            },
        ),
        Err(GameError::Validation(
            ValidationError::PendingPassiveAlreadyCovered {
                player: PlayerId::new("p1"),
            }
        ))
    );
    assert_eq!(state, state_before);
}

#[test]
fn defense_attack_matrix_preserves_formation_lifecycle_while_preventing_damage() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let matrix_deck = deck_starting_with(&[1, 6, 2, 7]);

    // 基準：P1 使用正常合法行動並完成回合抽牌後，同一個 incoming Fire Strike
    // 會傷害 P1。
    let mut baseline = GameRecord::start(two_player_setup(), matrix_deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    let baseline_turn_events = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            baseline_turn_events.as_slice(),
            [
                GameEvent::FormationCommitted {
                    player,
                    formation_id,
                    cards,
                    state: FormationAreaState::FaceUpResolving,
                    ..
                },
                GameEvent::AttackResolved {
                    attacker,
                    target,
                    formation_id: resolved_formation,
                    used_cards,
                    point_breakdown: AttackPointBreakdown {
                        base_points: 12,
                        environment_effect: EnvironmentAttackEffect::None,
                        interaction: ElementInteraction::None,
                        damage_transform: DamageTransform::NormalDamage,
                        final_amount: 12,
                    },
                    hp_change: HpChangeDelta {
                        team,
                        old_hp: 30,
                        delta: -12,
                        new_hp: 18,
                        effective_delta: -12,
                    },
                    shield_change: None,
                    card_moves,
                    elemental_context_update: Some(AttackResolutionEffects {
                        elemental_context_update: None,
                        outcome: AttackOutcome::Resolved,
                        ..
                    }),
                },
                GameEvent::FormationCardsDiscarded {
                    player: discarded_by,
                    formation_id: discarded_formation,
                    cards: discarded_cards,
                },
            ] if player == &p1
                && formation_id == "weapon"
                && cards == &vec![card(1), card(6)]
                && attacker == &p1
                && target == &p2
                && resolved_formation == "weapon"
                && used_cards == cards
                && team == &TeamId::new("team:p2")
                && card_moves.is_empty()
                && discarded_by == &p1
                && discarded_formation == "weapon"
                && discarded_cards == cards
        ),
        "unexpected Defense baseline setup: {baseline_turn_events:#?}"
    );
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let baseline_hp = baseline
        .state()
        .hp
        .iter()
        .find(|entry| entry.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let baseline_events = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            baseline_events.as_slice(),
            [
                GameEvent::FormationCommitted {
                    player,
                    formation_id,
                    cards,
                    state: FormationAreaState::FaceUpResolving,
                    ..
                },
                GameEvent::AttackResolved {
                    attacker,
                    target,
                    formation_id: resolved_formation,
                    used_cards,
                    point_breakdown: AttackPointBreakdown {
                        base_points: 8,
                        environment_effect: EnvironmentAttackEffect::None,
                        interaction: ElementInteraction::None,
                        damage_transform: DamageTransform::NormalDamage,
                        final_amount: 8,
                    },
                    hp_change: HpChangeDelta {
                        team,
                        old_hp,
                        delta: -8,
                        new_hp,
                        effective_delta: -8,
                    },
                    shield_change: None,
                    card_moves,
                    elemental_context_update: Some(AttackResolutionEffects {
                        elemental_context_update: Some(LastElementalAttackUpdate {
                            player: context_player,
                            attack: LastElementalAttack {
                                element: Element::Fire,
                                resolved_turn: 2,
                            },
                        }),
                        outcome: AttackOutcome::Resolved,
                        ..
                    }),
                },
                GameEvent::FormationCardsDiscarded {
                    player: discarded_by,
                    formation_id: discarded_formation,
                    cards: discarded_cards,
                },
            ] if player == &p2
                && formation_id == "fire-strike"
                && cards == &vec![card(9)]
                && attacker == &p2
                && target == &p1
                && resolved_formation == "fire-strike"
                && used_cards == cards
                && team == &TeamId::new("team:p1")
                && *old_hp == baseline_hp
                && *new_hp == baseline_hp - 8
                && card_moves.is_empty()
                && context_player == &p2
                && discarded_by == &p2
                && discarded_formation == "fire-strike"
                && discarded_cards == cards
        ),
        "unexpected Defense baseline outcome: {baseline_events:#?}"
    );
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        baseline_hp - 8
    );
    assert!(baseline.state().covered_passive(&p1).is_none());
    for card in [card(1), card(6), card(9)] {
        assert!(baseline.state().discard.contains(&card));
    }
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾：Defense 只能由 P1 的合法被動陣形建立。
    let mut interaction = GameRecord::start(two_player_setup(), matrix_deck).unwrap();
    interaction.advance_automatic().unwrap();
    let defense_cards = vec![card(2), card(7)];
    let defense_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                state: FormationAreaState::FaceDownResolving,
                ..
            },
            GameEvent::PassiveCovered {
                player: owner,
                formation_id: covered,
                cards: covered_cards,
                star_substitution: None,
                sealed: false,
            },
        ] if player == &p1
            && formation_id == "defense"
            && cards == &defense_cards
            && owner == &p1
            && covered == "defense"
            && covered_cards == &defense_cards
    ));
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p1.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(defense_cards.clone()),
            star_substitution: None,
        }]
    );
    let hidden_passive = vec![PublicCoveredPassive {
        owner: p1.clone(),
        formation_id: None,
        cards: PublicCardRefs::Hidden { count: 2 },
        star_substitution: None,
    }];
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone())).covered_passives,
        hidden_passive
    );
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Observer).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    assert_eq!(
        interaction
            .public_events_for(Viewer::Player(p1.clone()))
            .last(),
        Some(&PublicGameEvent::PassiveCovered {
            player: p1.clone(),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(defense_cards.clone()),
            star_substitution: None,
        })
    );
    let hidden_passive_event = PublicGameEvent::PassiveCovered {
        player: p1.clone(),
        formation_id: None,
        cards: PublicCardRefs::Hidden { count: 2 },
        star_substitution: None,
    };
    assert_eq!(
        interaction
            .public_events_for(Viewer::Player(p2.clone()))
            .last(),
        Some(&hidden_passive_event)
    );
    assert_eq!(
        interaction.public_events_for(Viewer::Observer).last(),
        Some(&hidden_passive_event)
    );
    assert_eq!(
        interaction.events().last(),
        Some(&GameEvent::PassiveCovered {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: defense_cards.clone(),
            star_substitution: None,
            sealed: false,
        })
    );
    assert_eq!(
        interaction.state().covered_passive(&p1).unwrap().cards,
        defense_cards
    );
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(10));
    assert_eq!(interaction.state().current_player(), Some(&p2));

    // 互動：被動恰好翻開一次，只防止傷害，仍讓 incoming Formation 提交並移動其
    // 卡牌。
    let interaction_hp = interaction
        .state()
        .hp
        .iter()
        .find(|entry| entry.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let interaction_events = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            interaction_events.as_slice(),
            [
                GameEvent::FormationCommitted {
                    player,
                    formation_id,
                    cards,
                    state: FormationAreaState::FaceUpResolving,
                    ..
                },
                GameEvent::PassiveFlipped {
                    owner,
                    incoming_player,
                    passive_id,
                    cards: passive_cards,
                    outcome: PassiveFlipOutcome::Applied {
                        effect_id,
                        modifications,
                    },
                },
                GameEvent::AttackResolved {
                    attacker,
                    target,
                    formation_id: resolved_formation,
                    used_cards,
                    point_breakdown: AttackPointBreakdown {
                        base_points: 8,
                        environment_effect: EnvironmentAttackEffect::None,
                        interaction: ElementInteraction::None,
                        damage_transform: DamageTransform::NormalDamage,
                        final_amount: 8,
                    },
                    hp_change: HpChangeDelta {
                        team,
                        old_hp,
                        delta: 0,
                        new_hp,
                        effective_delta: 0,
                    },
                    shield_change: None,
                    card_moves,
                    elemental_context_update: Some(AttackResolutionEffects {
                        elemental_context_update: Some(LastElementalAttackUpdate {
                            player: context_player,
                            attack: LastElementalAttack {
                                element: Element::Fire,
                                resolved_turn: 2,
                            },
                        }),
                        outcome: AttackOutcome::DamagePrevented,
                        ..
                    }),
                },
                GameEvent::FormationCardsDiscarded {
                    player: discarded_by,
                    formation_id: discarded_formation,
                    cards: discarded_cards,
                },
            ] if player == &p2
                && formation_id == "fire-strike"
                && cards == &vec![card(9)]
                && owner == &p1
                && incoming_player == &p2
                && passive_id == "defense"
                && passive_cards == &defense_cards
                && effect_id == "defense"
                && modifications == &vec![ActionModification::PreventDamage]
                && attacker == &p2
                && target == &p1
                && resolved_formation == "fire-strike"
                && used_cards == cards
                && team == &TeamId::new("team:p1")
                && *old_hp == interaction_hp
                && *new_hp == interaction_hp
                && card_moves.is_empty()
                && context_player == &p2
                && discarded_by == &p2
                && discarded_formation == "fire-strike"
                && discarded_cards == cards
        ),
        "unexpected Defense interaction outcome: {interaction_events:#?}"
    );
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        interaction_hp
    );
    assert!(interaction.state().covered_passive(&p1).is_none());
    for card in defense_cards.iter().copied().chain([card(9)]) {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.state().current_player(), Some(&p2));
    assert_eq!(interaction.state().phase, Phase::TurnDraw);
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn defense_metal_environment_matrix_records_the_ground_but_keeps_attack_damage() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup
        .card_instances
        .extend([card_instance(21, "metal"), card_instance(26, "metal")]);
    let opening = vec![
        card(1),
        card(2),
        card(3),
        card(4), // P1 起手：合法的初始 Metal Strike
        card(6),
        card(11),
        card(16),
        card(21),
        card(26), // P2 起手：合法的 West White Tiger
        card(7),
        card(8), // P1 第一次回合抽牌；保留兩張木卡
        card(9), // P1 必須進行的第三次抽牌/棄牌候選
        card(14),
        card(10),
        card(12), // Sacred Beast 後 P2 抽牌；保留 Fire 14
        card(13),
        card(19), // P1 測試行動後的抽牌
    ];
    let mut deck = opening.clone();
    deck.extend(
        (1..=20)
            .map(card)
            .filter(|candidate| !opening.contains(candidate)),
    );

    fn finish_turn(record: &mut GameRecord, player: &PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player.clone(),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    fn establish_metal_environment(record: &mut GameRecord, p1: &PlayerId, p2: &PlayerId) {
        record.advance_automatic().unwrap();
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(record, p1, card(8));
        let sacred_beast = record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "west-white-tiger".to_string(),
                cards: vec![card(6), card(11), card(16), card(21), card(26)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(sacred_beast.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            } if environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p2.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
        )));
        assert_eq!(record.state().environment, Some(Element::Metal));
        finish_turn(record, p2, card(12));
        assert_eq!(record.state().current_player(), Some(p1));
    }

    // 僅修飾分支：Metal Environment 是合法 Sacred-Beast 結果。無關的 Empty City
    // 仍有正常且刻意的生命週期，而後續 Fire Attack 在 Metal Environment 下仍會
    // 正常傷害 P1。
    let mut environment_only = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    establish_metal_environment(&mut environment_only, &p1, &p2);
    environment_only
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(3), card(4)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut environment_only, &p1, card(19));
    let environment_attack = environment_only
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(14)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(environment_attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown { base_points: 8, final_amount: 8, .. },
            hp_change: HpChangeDelta { team, effective_delta: -8, .. },
            ..
        } if team == &TeamId::new("team:p1")
    )));

    // 互動：相同的合法環境現在與合法覆蓋的 Defense 共存。Defense 會隨環境地面
    // 恰好翻開一次；它不會防止 incoming Fire Attack，也不會抑制其生命週期。
    let mut interaction = GameRecord::start(setup, deck).unwrap();
    establish_metal_environment(&mut interaction, &p1, &p2);
    let defense = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, .. },
        ] if player == &p1
            && formation_id == "defense"
            && cards == &vec![card(2), card(7)]
            && owner == &p1
            && covered == "defense"
            && covered_cards == &vec![card(2), card(7)]
    ));
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p1.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(vec![card(2), card(7)]),
            star_substitution: None,
        }]
    );
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    finish_turn(&mut interaction, &p1, card(19));
    let attack = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(14)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: defense_cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 8, final_amount: 8, .. },
                hp_change: HpChangeDelta { team, effective_delta: -8, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-strike"
            && cards == &vec![card(14)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "defense"
            && defense_cards == &vec![card(2), card(7)]
            && grounds == &vec![PassiveNoEffectGround::IneffectiveInEnvironment { environment: Element::Metal }]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "fire-strike"
            && discarded_cards == &vec![card(14)]
    ));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert!(interaction.state().covered_passive(&p1).is_none());
    for card in [card(2), card(7), card(14)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn defense_sacred_beast_matrix_consumes_defense_but_keeps_the_beasts_damage_and_environment() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
    ]);
    let deck = {
        let opening = vec![
            card(2),
            card(7),
            card(1),
            card(31), // P1：合法 Defense 或基準實體 Weapon
            card(6),
            card(11),
            card(16),
            card(21),
            card(26), // P2：合法 West White Tiger
            card(3),
            card(4),
            card(5), // P1 回合抽牌
        ];
        let mut cards = opening.clone();
        cards.extend(
            (1..=20)
                .map(card)
                .filter(|candidate| !opening.contains(candidate)),
        );
        cards
    };
    let beast_cards = vec![card(6), card(11), card(16), card(21), card(26)];

    // 基準：沒有前一位玩家被動時，Sacred Beast 具有正常的 81 點攻擊與標準環境
    // 轉移。P1 的實體 Weapon 特意不留下元素前置行動脈絡。
    let mut baseline = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(31)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(4));
    let baseline_beast = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: beast_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_beast.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 81, final_amount: 81, .. },
                hp_change: HpChangeDelta { team, effective_delta: -81, .. },
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "west-white-tiger"
            && cards == &beast_cards
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p2.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
            && discarded_by == &p2
            && discarded == "west-white-tiger"
            && discarded_cards == &beast_cards
    ));
    assert_eq!(baseline.state().environment, Some(Element::Metal));
    for used in &beast_cards {
        assert!(baseline.state().discard.contains(used));
    }
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾本身：Defense 由真正的被動命令建立，接著對非擁有者保持隱藏，直到
    // P2 的下一個行動。
    let mut interaction = GameRecord::start(setup, deck).unwrap();
    interaction.advance_automatic().unwrap();
    let defense = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p1
            && formation_id == "defense"
            && cards == &vec![card(2), card(7)]
            && owner == &p1
            && covered == "defense"
            && covered_cards == &vec![card(2), card(7)]
    ));
    assert!(matches!(
        interaction.public_view(Viewer::Player(p2.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: None, cards: PublicCardRefs::Hidden { count: 2 }, star_substitution: None }]
            if owner == &p1
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(4));

    // 互動：Beast 的規則例外讓 Defense 產生單一無效果結果，但無關的 81 點傷害、
    // 轉移、提交與實體卡牌移動都保持完整。
    let beast = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: beast_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        beast.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: defense_cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 81, final_amount: 81, .. },
                hp_change: HpChangeDelta { team, effective_delta: -81, .. },
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "west-white-tiger"
            && cards == &beast_cards
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "defense"
            && defense_cards == &vec![card(2), card(7)]
            && grounds == &vec![PassiveNoEffectGround::IgnoredBySacredBeast]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p2.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
            && discarded_by == &p2
            && discarded == "west-white-tiger"
            && discarded_cards == &beast_cards
    ));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert!(interaction.state().covered_passive(&p1).is_none());
    for used in [
        card(2),
        card(7),
        card(6),
        card(11),
        card(16),
        card(21),
        card(26),
    ] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn metal_environment_barrier_matrix_keeps_commitment_and_cards_when_its_shield_is_ineffective() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
    ]);
    let barrier_cards = vec![card(2), card(7), card(31), card(4)];

    // 基準：Barrier 本身套用 44 點護盾，並在沒有 Environment 修飾時完成正常的
    // 陣形/卡牌生命週期。
    let mut baseline = GameRecord::start(setup.clone(), {
        let opening = vec![
            card(2),
            card(7),
            card(31),
            card(4), // P1：Barrier
            card(1),
            card(6),
            card(11),
            card(16),
            card(21),
            card(26),
        ];
        let mut cards = opening.clone();
        cards.extend(
            (1..=20)
                .map(card)
                .filter(|candidate| !opening.contains(candidate)),
        );
        cards
    })
    .unwrap();
    baseline.advance_automatic().unwrap();
    let baseline_barrier = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: barrier_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_barrier.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::ShieldChanged { player: shield_owner, old_value: 0, delta: 44, new_value: 44 },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "barrier"
            && cards == &barrier_cards
            && shield_owner == &p1
            && discarded_by == &p1
            && discarded == "barrier"
            && discarded_cards == &barrier_cards
    ));
    assert_eq!(baseline.state().shield(&p1), Some(44));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾：P2 的合法 Sacred Beast 是共用 Metal Environment 的唯一來源。P1 接著
    // 仍在下一次行動中使用相同且實體上合法的 Barrier 卡牌。
    let mut interaction = GameRecord::start(setup, {
        let opening = vec![
            card(1),
            card(2),
            card(7),
            card(4), // P1：第一次行動，接著在回合抽牌後使用 Barrier
            card(6),
            card(11),
            card(16),
            card(21),
            card(26), // P2：West White Tiger
            card(31),
            card(8),
            card(9), // P1 回合抽牌；保留 Metal 31
            card(10),
            card(12),
            card(13), // P2 回合抽牌
        ];
        let mut cards = opening.clone();
        cards.extend(
            (1..=20)
                .map(card)
                .filter(|candidate| !opening.contains(candidate)),
        );
        cards
    })
    .unwrap();
    interaction.advance_automatic().unwrap();
    interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(8));
    let sacred_beast = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: vec![card(6), card(11), card(16), card(21), card(26)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(sacred_beast.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
            ..
        } if environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
            player: p2.clone(),
            formation_id: "west-white-tiger".to_string(),
            from: None,
            to: Element::Metal,
        }]
    )));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    interaction.advance_automatic().unwrap();
    answer_record_choice(
        &mut interaction,
        p2.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(10)],
        },
    )
    .unwrap();
    interaction.advance_automatic().unwrap();

    // 互動：Metal Environment 讓 Barrier 的護盾效果不適用，但不能撤銷已接受的
    // 命令、陣形提交或棄牌。
    let ignored_barrier = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: barrier_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(ignored_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if player == &p1 && formation_id == "barrier" && cards == &barrier_cards
    )));
    assert!(ignored_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationEffectIgnored {
            player,
            formation_id,
            reason: fewfc::domain::FormationNoEffectReason::IneffectiveInEnvironment {
                environment: Element::Metal,
            },
        } if player == &p1 && formation_id == "barrier"
    )));
    assert!(ignored_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p1 && formation_id == "barrier" && cards == &barrier_cards
    )));
    assert!(!ignored_barrier.iter().any(|event| matches!(
        event,
        GameEvent::ShieldChanged { player, .. } if player == &p1
    )));
    assert_eq!(interaction.state().shield(&p1), Some(0));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert_eq!(
        interaction
            .public_view(Viewer::Player(p1.clone()))
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    assert_eq!(
        interaction
            .public_view(Viewer::Player(p2.clone()))
            .unwrap()
            .environment,
        Some(Element::Metal)
    );
    for used in barrier_cards {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn fire_environment_weapon_matrix_keeps_attack_commitment_when_damage_is_ineffective() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.push(card_instance(24, "fire"));
    let weapon_cards = vec![card(1), card(6)];

    // 基準：Weapon 自身在沒有環境修飾時，會對下家造成完整的物理傷害，並完成
    // 陣形提交與卡牌移動。
    let mut baseline = GameRecord::start(
        setup.clone(),
        deck_starting_with(&[1, 6, 2, 3, 4, 9, 14, 19, 24]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    let baseline_weapon = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: weapon_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta { team, effective_delta: -12, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "weapon"
            && cards == &weapon_cards
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "weapon"
            && discarded_cards == &weapon_cards
    ));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾：Fire Environment 僅由 P2 的合法 South Vermilion Bird 建立；P1
    // 透過真實回合抽牌保留 Weapon 所需的第二張 Metal 卡。
    let mut interaction = GameRecord::start(
        setup,
        deck_starting_with(&[
            1, 6, 2, 3, // P1：先使用 Empty City，保留兩張 Metal 給 Weapon
            4, 9, 14, 19, 24, // P2：South Vermilion Bird
            11, 7, 8, // P1 回合抽牌；保留 Metal 11
            5, 10, 12, // P2 回合抽牌
        ]),
    )
    .unwrap();
    interaction.advance_automatic().unwrap();
    interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "empty-city".to_string(),
            cards: vec![card(2), card(3)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(7));
    let south_vermilion_bird = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "south-vermilion-bird".to_string(),
            cards: vec![card(4), card(9), card(14), card(19), card(24)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(south_vermilion_bird.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown { base_points: 81, final_amount: 81, .. },
            elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
            ..
        } if environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
            player: p2.clone(),
            formation_id: "south-vermilion-bird".to_string(),
            from: None,
            to: Element::Fire,
        }]
    )));
    assert_eq!(interaction.state().environment, Some(Element::Fire));
    interaction.advance_automatic().unwrap();
    answer_record_choice(
        &mut interaction,
        p2.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(5)],
        },
    )
    .unwrap();
    interaction.advance_automatic().unwrap();

    // 互動：Fire Environment 僅讓 Weapon 的傷害無效；已接受的命令仍保有完整
    // canonical 提交、AttackResolved、棄牌與公共環境狀態。
    let ignored_weapon = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: weapon_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        ignored_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::FormationEffectIgnored {
                player: ignored_player,
                formation_id: ignored_formation,
                reason: fewfc::domain::FormationNoEffectReason::IneffectiveInEnvironment {
                    environment: Element::Fire,
                },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta { team, effective_delta: 0, .. },
                shield_change: None,
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "weapon"
            && cards == &weapon_cards
            && ignored_player == &p1
            && ignored_formation == "weapon"
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "weapon"
            && discarded_cards == &weapon_cards
    ));
    assert_eq!(interaction.state().environment, Some(Element::Fire));
    assert_eq!(
        interaction
            .public_view(Viewer::Player(p1.clone()))
            .unwrap()
            .environment,
        Some(Element::Fire)
    );
    assert_eq!(
        interaction
            .public_view(Viewer::Player(p2.clone()))
            .unwrap()
            .environment,
        Some(Element::Fire)
    );
    for used in weapon_cards {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn void_meridian_environment_matrix_commits_without_an_environment_effect() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
    ]);
    let void_cards = vec![card(21), card(26), card(31)];
    let mut record = GameRecord::start(
        setup,
        deck_starting_with(&[1, 2, 3, 4, 21, 26, 31, 6, 7, 5, 8, 9]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    // P1 的普通合法行動只用來抵達 P2 的 command boundary；沒有直接安排環境或
    // Void 的結果。P2 的三張同級 Metal 均來自初始合法發牌。
    record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(5));
    assert_eq!(record.state().current_player(), Some(&p2));
    assert_eq!(record.state().environment, None);

    let hp_before = record.state().hp.clone();
    let void = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "void-meridian-severing".to_string(),
            cards: void_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        void.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "void-meridian-severing"
            && cards == &void_cards
            && discarded_by == &p2
            && discarded == "void-meridian-severing"
            && discarded_cards == &void_cards
    ));
    assert!(!void.iter().any(|event| matches!(
        event,
        GameEvent::EnvironmentCleared { .. } | GameEvent::HpChanged { .. }
    )));
    assert_eq!(record.state().environment, None);
    assert_eq!(record.state().hp, hp_before);
    for used in void_cards {
        assert!(record.state().discard.contains(&used));
    }
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn shield_water_environment_metal_attack_matrix_absorbs_before_the_healing_interaction() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup
        .card_instances
        .extend([card_instance(23, "water"), card_instance(28, "water")]);
    let deck = deck_starting_with(&[
        1, 3, 8, 13, // P1：首回合 Metal Strike，保留三張 Water
        4, 5, 7, 12, 11, // P2：首回合 Empty City，保留 Barrier 的三張卡
        18, 23, 28, // P1 首次回合抽牌，湊成 North Black Tortoise
        9, 10, 14, // P2 首次回合抽牌，補足 Barrier 的 Fire
        6, 16, 2, // P1 聖獸後的回合抽牌，保留 Metal Strike
        15, 17, 19, // P2 第二次回合抽牌
    ]);

    fn finish_turn(record: &mut GameRecord, player: PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player,
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    fn record_before_p2_second_action(
        setup: &GameSetup,
        deck: &[CardInstanceId],
        p1: &PlayerId,
        p2: &PlayerId,
    ) -> GameRecord {
        let mut record = GameRecord::start(setup.clone(), deck.to_vec()).unwrap();
        record.advance_automatic().unwrap();

        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p1.clone(), card(28));

        // P2 的第一次合法行動只用來跨過回合並保留 Barrier 的必要卡牌；下一位
        // P1 的聖獸會自然翻開並消耗這個被動效果。
        record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "empty-city".to_string(),
                cards: vec![card(4), card(5)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p2.clone(), card(10));

        // Water Environment 必須來自合法的 North Black Tortoise，而不是直接
        // 寫入狀態。聖獸的傷害與環境轉移都屬同一個完整 canonical outcome。
        let north_black_tortoise = record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "north-black-tortoise".to_string(),
                cards: vec![card(3), card(8), card(13), card(18), card(23)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(north_black_tortoise.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown { base_points: 81, final_amount: 81, .. },
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            } if environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p1.clone(),
                formation_id: "north-black-tortoise".to_string(),
                from: None,
                to: Element::Water,
            }]
        )));
        assert_eq!(record.state().environment, Some(Element::Water));
        finish_turn(&mut record, p1.clone(), card(2));
        assert_eq!(record.state().current_player(), Some(p2));
        record
    }

    // 基準：Water Environment 讓 P1 的 Metal Strike 轉為回復；P2 沒有護盾時，
    // canonical HP 變化確實是正值。
    let mut baseline = record_before_p2_second_action(&setup, &deck, &p1, &p2);
    baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, p2.clone(), card(15));
    let baseline_metal = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_metal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::GeneratingElementDamageConvertedToHealing { environment: Element::Water },
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::HealTarget,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta { team, old_hp: 112, new_hp: 119, effective_delta: 7, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && discarded_by == &p1
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(6)]
    ));
    assert_eq!(baseline.state().shield(&p2), Some(0));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾：P2 用合法 Barrier 建立 44 點護盾。這個步驟本身必須獨立證明，不能
    // 只在後續 Attack 的最終狀態中推論護盾曾存在。
    let mut interaction = record_before_p2_second_action(&setup, &deck, &p1, &p2);
    let barrier = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "barrier".to_string(),
            cards: vec![card(7), card(12), card(11), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        barrier.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::ShieldChanged { player: shield_owner, old_value: 0, delta: 44, new_value: 44 },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "barrier"
            && cards == &vec![card(7), card(12), card(11), card(9)]
            && shield_owner == &p2
            && discarded_by == &p2
            && discarded == "barrier"
            && discarded_cards == &vec![card(7), card(12), card(11), card(9)]
    ));
    assert_eq!(interaction.state().shield(&p2), Some(44));
    finish_turn(&mut interaction, p2.clone(), card(15));

    // 交互：護盾先吸收 Metal Strike。因為攻擊在護盾階段結束，Water 的「轉為
    // 回復」與元素交互都不套用到 HP；但新的 Metal 元素上下文仍被 canonical
    // outcome 記錄，供下一次合法行動使用。
    let shielded_metal = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        shielded_metal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta { team, old_hp: 112, new_hp: 112, effective_delta: 0, .. },
                shield_change: Some(ShieldChangeDelta { player: shield_owner, old_value: 44, delta: -7, new_value: 37 }),
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Metal, .. },
                    }),
                    outcome: AttackOutcome::AbsorbedByShield,
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && shield_owner == &p2
            && context_player == &p1
            && discarded_by == &p1
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(6)]
    ));
    assert_eq!(interaction.state().shield(&p2), Some(37));
    assert_eq!(interaction.state().environment, Some(Element::Water));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|team| team.team == TeamId::new("team:p2"))
            .map(|team| team.hp),
        Some(112)
    );
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn five_streams_defense_matrix_keeps_the_turn_draw_bonus_when_damage_is_prevented() {
    // 牌堆順序只提供每位玩家合法命令所需的卡牌。P1 透過 Defense 建立修飾；此
    // 矩陣沒有注入覆蓋被動或行動結果。
    let mut setup = two_player_setup_with_hp(100);
    setup.card_instances.push(card_instance(21, "metal"));
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：五張同等級卡牌組成 Five Streams 並造成傷害，同時提交陣形並記錄其
    // 抽牌獎勵。
    let mut baseline = GameRecord::start(
        setup.clone(),
        deck_starting_with(&[5, 10, 2, 3, 1, 6, 11, 16, 21, 7, 8]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    let baseline_turn_events = baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_turn_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                ..
            },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded_formation,
                cards: discarded_cards,
            },
        ] if player == &p1
            && formation_id == "metamorphosis"
            && cards == &vec![card(5), card(10)]
            && discarded_by == &p1
            && discarded_formation == "metamorphosis"
            && discarded_cards == &vec![card(5), card(10)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(7));
    let baseline_target_hand_count = baseline.state().hand(&p1).unwrap().len();
    assert_eq!(baseline_target_hand_count, 4);
    assert_eq!(baseline_target_hand_count * 15, 60);
    let baseline_hp = baseline
        .state()
        .hp
        .iter()
        .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let baseline_events = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "five-streams-unite".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            baseline_events.as_slice(),
            [
                GameEvent::FormationCommitted {
                    player,
                    formation_id,
                    cards,
                    state: FormationAreaState::FaceUpResolving,
                    ..
                },
                GameEvent::AttackResolved {
                    attacker,
                    target,
                    formation_id: resolved_formation,
                    used_cards,
                    point_breakdown,
                    hp_change,
                    shield_change: None,
                    card_moves,
                    elemental_context_update: Some(effects),
                },
                GameEvent::FormationCardsDiscarded {
                    player: discarded_by,
                    formation_id: discarded_formation,
                    cards: discarded_cards,
                },
            ] if player == &p2
                && formation_id == "five-streams-unite"
                && cards == &vec![card(1), card(6), card(11), card(16), card(21)]
                && attacker == &p2
                && target == &p1
                && resolved_formation == "five-streams-unite"
                && used_cards == cards
                && point_breakdown.base_points == 60
                && point_breakdown.environment_effect == EnvironmentAttackEffect::None
                && point_breakdown.interaction == ElementInteraction::None
                && point_breakdown.damage_transform == DamageTransform::NormalDamage
                && point_breakdown.final_amount == 60
                && hp_change == &HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: baseline_hp,
                    delta: -60,
                    new_hp: baseline_hp - 60,
                    effective_delta: -60,
                }
                && card_moves.is_empty()
                && effects.outcome == AttackOutcome::Resolved
                && effects.turn_draw_bonus_changes.len() == 1
                && effects.turn_draw_bonus_changes[0].player == p2
                && effects.turn_draw_bonus_changes[0].old_value == 0
                && effects.turn_draw_bonus_changes[0].delta == 1
                && effects.turn_draw_bonus_changes[0].new_value == 1
                && discarded_by == &p2
                && discarded_formation == "five-streams-unite"
                && discarded_cards == cards
        ),
        "unexpected Five Streams baseline outcome: {baseline_events:#?}"
    );
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        baseline_hp - 60
    );
    assert_eq!(
        baseline.state().turn_draw_bonus_by_player.get(&p2),
        Some(&1)
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾與互動：P1 合法覆蓋 Defense。下一次攻擊會恰好翻開該被動，防止受影響
    // 的生命值損失，並保留 Five Streams 獨立的回合抽牌獎勵。
    let mut interaction = GameRecord::start(
        setup,
        deck_starting_with(&[2, 7, 4, 5, 1, 6, 11, 16, 21, 3, 8]),
    )
    .unwrap();
    interaction.advance_automatic().unwrap();
    let defense_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense_events.as_slice(),
        [
            GameEvent::FormationCommitted { formation_id, cards, .. },
            GameEvent::PassiveCovered { player, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if formation_id == "defense"
            && cards == &vec![card(2), card(7)]
            && player == &p1
            && covered == "defense"
            && covered_cards == &vec![card(2), card(7)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(3));
    assert_eq!(interaction.state().current_player(), Some(&p2));
    let interaction_target_hand_count = interaction.state().hand(&p1).unwrap().len();
    assert_eq!(interaction_target_hand_count, 4);
    assert_eq!(interaction_target_hand_count * 15, 60);
    let interaction_hp = interaction
        .state()
        .hp
        .iter()
        .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
        .unwrap()
        .hp;
    let interaction_events = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "five-streams-unite".to_string(),
            cards: vec![card(1), card(6), card(11), card(16), card(21)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(
        matches!(
            interaction_events.as_slice(),
            [
                GameEvent::FormationCommitted {
                    player,
                    formation_id,
                    cards,
                    state: FormationAreaState::FaceUpResolving,
                    ..
                },
                GameEvent::PassiveFlipped {
                    owner,
                    incoming_player,
                    passive_id,
                    cards: defense_cards,
                    outcome:
                        PassiveFlipOutcome::Applied {
                            effect_id,
                            modifications,
                        },
                },
                GameEvent::AttackResolved {
                    attacker,
                    target,
                    formation_id: resolved_formation,
                    used_cards,
                    point_breakdown,
                    hp_change,
                    shield_change: None,
                    card_moves,
                    elemental_context_update: Some(effects),
                },
                GameEvent::FormationCardsDiscarded {
                    player: discarded_by,
                    formation_id: discarded_formation,
                    cards: discarded_cards,
                },
            ] if player == &p2
                && formation_id == "five-streams-unite"
                && cards == &vec![card(1), card(6), card(11), card(16), card(21)]
                && owner == &p1
                && incoming_player == &p2
                && passive_id == "defense"
                && defense_cards == &vec![card(2), card(7)]
                && effect_id == "defense"
                && modifications == &vec![ActionModification::PreventDamage]
                && attacker == &p2
                && target == &p1
                && resolved_formation == "five-streams-unite"
                && used_cards == cards
                && point_breakdown.base_points == 60
                && point_breakdown.environment_effect == EnvironmentAttackEffect::None
                && point_breakdown.interaction == ElementInteraction::None
                && point_breakdown.damage_transform == DamageTransform::NormalDamage
                && point_breakdown.final_amount == 60
                && hp_change == &HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: interaction_hp,
                    delta: 0,
                    new_hp: interaction_hp,
                    effective_delta: 0,
                }
                && card_moves.is_empty()
                && effects.outcome == AttackOutcome::DamagePrevented
                && effects.turn_draw_bonus_changes.len() == 1
                && effects.turn_draw_bonus_changes[0].player == p2
                && effects.turn_draw_bonus_changes[0].old_value == 0
                && effects.turn_draw_bonus_changes[0].delta == 1
                && effects.turn_draw_bonus_changes[0].new_value == 1
                && discarded_by == &p2
                && discarded_formation == "five-streams-unite"
                && discarded_cards == cards
        ),
        "unexpected Defense × Five Streams outcome: {interaction_events:#?}"
    );
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|team_hp| team_hp.team == TeamId::new("team:p1"))
            .unwrap()
            .hp,
        interaction_hp
    );
    assert_eq!(
        interaction.state().turn_draw_bonus_by_player.get(&p2),
        Some(&1)
    );
    let turn_draw_events = interaction.advance_automatic().unwrap();
    let (choice_id, allowed_discards) = match interaction.state().pending_choice.as_ref() {
        Some(PendingChoice {
            choice_id,
            player,
            kind: PendingChoiceKind::Card { cards, .. },
            ..
        }) if player == &p2 && cards.len() == 4 => (*choice_id, cards.clone()),
        other => panic!("Five Streams draw must expose four legal discards, got {other:?}"),
    };
    assert!(
        matches!(
            turn_draw_events.as_slice(),
            [
                GameEvent::CardsDrawnForTurnDiscardChoice {
                    player,
                    drawn_cards,
                    allowed_discards,
                },
                GameEvent::ChoiceRequested { choice, resolution },
            ] if player == &p2
                && drawn_cards == allowed_discards
                && allowed_discards.len() == 4
                && choice.player == p2
                && matches!(&choice.kind, PendingChoiceKind::Card { cards, minimum: 1, maximum: 1, can_decline: false } if cards == allowed_discards)
                && resolution == &PendingResolution::TurnDrawDiscard
        ),
        "unexpected Five Streams draw resolution: {turn_draw_events:#?}"
    );
    for card_id in [
        card(2),
        card(7),
        card(1),
        card(6),
        card(11),
        card(16),
        card(21),
    ] {
        assert!(interaction.state().discard.contains(&card_id));
    }
    assert!(
        interaction
            .state()
            .formation_area(&p2)
            .is_some_and(|area| area.formation.is_none())
    );
    assert!(
        public_view::state_for(interaction.state(), Viewer::Player(p1.clone()))
            .covered_passives
            .is_empty()
    );
    assert!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone()))
            .covered_passives
            .is_empty()
    );

    let chosen_discard = allowed_discards[0];
    let choice_events = interaction
        .handle(Command::AnswerChoice {
            player: p2.clone(),
            choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![chosen_discard],
            },
        })
        .unwrap();
    assert!(
        matches!(
            choice_events.as_slice(),
            [
                GameEvent::ChoiceMade {
                    player: choice_player,
                    choice_id: made_choice,
                    answer: ChoiceAnswer::Cards { cards },
                },
                GameEvent::TurnDrawResolved {
                    player,
                    discard,
                    kept_cards,
                },
            ] if choice_player == &p2
                && made_choice == &choice_id
                && cards == &vec![chosen_discard]
                && player == &p2
                && discard == &chosen_discard
                && kept_cards.len() == 3
        ),
        "unexpected Five Streams draw answer: {choice_events:#?}"
    );
    assert!(interaction.state().discard.contains(&chosen_discard));
    let next_turn_events = interaction.advance_automatic().unwrap();
    assert!(matches!(
        next_turn_events.as_slice(),
        [
            GameEvent::TurnEnded { player: ended },
            GameEvent::TurnStarted { player: started, .. },
        ] if ended == &p2 && started == &p1
    ));
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn countershock_attack_matrix_splits_damage_after_legal_cover_and_preserves_attack_lifecycle() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[4, 9, 1, 2, 5, 10, 3, 6, 7]);

    // 基準：沒有修飾時，相同的普通 Metal Strike 只會傷害 P1。P1 的 Fire Strike
    // 是合法的先前行動，不是注入的回合歷史。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(4)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(8));
    let baseline_attack = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 7, final_amount: 7, .. },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -7, new_hp: 23, effective_delta: -7 },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(6)]
    ));
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(23)
    );
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(22)
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：Countershock 由 P1 的合法陣形命令建立。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    let cover_events = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        cover_events.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, star_substitution: None, sealed: false },
        ] if player == &p1
            && formation_id == "countershock"
            && cards == &vec![card(4), card(9)]
            && covered_by == &p1
            && covered == "countershock"
            && covered_cards == &vec![card(4), card(9)]
    ));
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p1.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: Some("countershock".to_string()),
            cards: PublicCardRefs::Known(vec![card(4), card(9)]),
            star_substitution: None,
        }]
    );
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(8));

    // 互動：相同的 Attack 會翻開 Countershock。它將七點分成兩個向上取整的四點
    // 損失，同時保留攻擊提交與雙方標準卡牌移動。
    let attack = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: passive_cards,
                outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                formation_id: attack_formation,
                used_cards,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -4, new_hp: 26, effective_delta: -4 },
                shield_change: None,
                card_moves,
                elemental_context_update: Some(AttackResolutionEffects {
                    outcome: fewfc::domain::AttackOutcome::Resolved,
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Metal, resolved_turn: 2 },
                    }),
                    hp_changes,
                    shield_changes,
                    card_moves: effect_card_moves,
                    statuses_added,
                    statuses_removed,
                    counter_effects_established,
                    turn_draw_bonus_changes,
                    environment_transfers,
                }),
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "countershock"
            && passive_cards == &vec![card(4), card(9)]
            && effect_id == "countershock"
            && modifications == &vec![ActionModification::SplitAttackDamage]
            && attacker == &p2
            && target == &p1
            && attack_formation == "metal-strike"
            && used_cards == &vec![card(6)]
            && team == &TeamId::new("team:p1")
            && card_moves.is_empty()
            && context_player == &p2
            && hp_changes == &vec![HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 30,
                delta: -4,
                new_hp: 26,
                effective_delta: -4,
            }]
            && shield_changes.is_empty()
            && effect_card_moves.is_empty()
            && statuses_added.is_empty()
            && statuses_removed.is_empty()
            && counter_effects_established.is_empty()
            && turn_draw_bonus_changes.is_empty()
            && environment_transfers.is_empty()
            && discarded_by == &p2
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(6)]
    ));
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(26)
    );
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(26)
    );
    for card in [card(4), card(9), card(6)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn countershock_shield_matrix_splits_before_physical_shield_absorption_through_legal_turns() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[
        2, 7, 1, 4, // P1 Barrier
        6, 11, 16, 5, 10, // P2 起手；先 Metal Strike 再 Weapon
        9, 14, 3, // P1 收到 Fire/Fire，另有一張回合抽牌棄牌
        8, 12, 13, // P2 第一次回合抽牌
        15, 17, 18, // P1 第二次回合抽牌
    ]);

    fn advance_p2_turn(record: &mut GameRecord, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            PlayerId::new("p2"),
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    fn establish_barrier_and_take_opening_attack(
        record: &mut GameRecord,
        p1: &PlayerId,
        p2: &PlayerId,
    ) {
        record.advance_automatic().unwrap();
        record
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "barrier".to_string(),
                cards: vec![card(2), card(7), card(1), card(4)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert_eq!(record.state().shield(p1), Some(44));
        advance_record_to_next_main_after_turn_draw(record, card(3));
        let opening_attack = record
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(opening_attack.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                hp_change: HpChangeDelta { delta: 0, effective_delta: 0, .. },
                shield_change: Some(ShieldChangeDelta { player, old_value: 44, delta: -7, new_value: 37 }),
                ..
            } if player == p1
        )));
        advance_p2_turn(record, card(8));
        assert_eq!(record.state().current_player(), Some(p1));
        assert_eq!(record.state().shield(p1), Some(37));
    }

    // 基準：只有 Barrier 時，最終實體 Weapon 會讓護盾消耗兩倍的十二點傷害，
    // 並讓兩隊生命值保持不變；P1 中間的普通 Fire Strike 除外。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    establish_barrier_and_take_opening_attack(&mut baseline, &p1, &p2);
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(15));
    let baseline_weapon = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(11), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown { base_points: 12, final_amount: 12, .. },
                hp_change: HpChangeDelta { team, delta: 0, effective_delta: 0, .. },
                shield_change: Some(ShieldChangeDelta { player: shield_owner, old_value: 37, delta: -24, new_value: 13 }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "weapon"
            && cards == &vec![card(11), card(16)]
            && team == &TeamId::new("team:p1")
            && shield_owner == &p1
            && discarded_by == &p2
            && discarded == "weapon"
            && discarded_cards == &vec![card(11), card(16)]
    ));
    assert_eq!(baseline.state().shield(&p1), Some(13));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：相同 Barrier 與中間的 P2 回合後，P1 透過合法被動陣形覆蓋 Countershock；
    // 沒有注入護盾或覆蓋狀態。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    establish_barrier_and_take_opening_attack(&mut interaction, &p1, &p2);
    let countershock = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(9), card(14)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        countershock.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, .. },
        ] if player == &p1
            && formation_id == "countershock"
            && cards == &vec![card(9), card(14)]
            && covered_by == &p1
            && covered == "countershock"
            && covered_cards == &vec![card(9), card(14)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(15));

    // 互動：Countershock 先將 incoming 的十二點減半。目標方的一半接著是對護盾
    // 的實體傷害，因此加倍為十二點；另一半傷害 P2 的生命值。兩個陣形生命週期
    // 都保持不變。
    let weapon = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(11), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(weapon.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            passive_id,
            outcome: PassiveFlipOutcome::Applied { modifications, .. },
            ..
        } if owner == &p1
            && passive_id == "countershock"
            && modifications == &vec![ActionModification::SplitAttackDamage]
    )));
    assert!(weapon.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown { base_points: 12, final_amount: 12, .. },
            hp_change: HpChangeDelta { team, delta: 0, effective_delta: 0, .. },
            shield_change: Some(ShieldChangeDelta { player, old_value: 37, delta: -12, new_value: 25 }),
            elemental_context_update: Some(effects),
            ..
        } if team == &TeamId::new("team:p1")
            && effects.hp_changes == vec![HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 30,
                delta: -6,
                new_hp: 24,
                effective_delta: -6,
            }]
    )));
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert_eq!(interaction.state().shield(&p1), Some(25));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(30)
    );
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(24)
    );
    for card in [card(9), card(14), card(11), card(16)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn countershock_lethal_matrix_resolves_both_split_losses_before_declaring_a_draw() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[
        4, 9, 1, 2, // P1：合法 Countershock
        6, 11, 16, 5, 10, // P2：合法 Metal Strike
        3, 7, // P1 回合抽牌，以下棄置卡牌 7
    ]);
    let mut record = GameRecord::start(two_player_setup_with_hp(4), deck).unwrap();
    record.advance_automatic().unwrap();

    // 修飾：覆蓋的 Countershock 透過實際回合抽牌生命週期抵達下一位玩家的行動。
    let covered = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        covered.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered_id, cards: covered_cards, .. },
        ] if player == &p1
            && formation_id == "countershock"
            && cards == &vec![card(4), card(9)]
            && covered_by == &p1
            && covered_id == "countershock"
            && covered_cards == &vec![card(4), card(9)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut record, card(7));
    assert_eq!(record.state().current_player(), Some(&p2));

    // 互動：七點 Attack 分成兩個向上取整的四點損失。兩個差異都必須在終止抽牌
    // 前記錄，同時被動與 incoming Formation 卡牌仍會抵達棄牌堆。
    let attack = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                outcome: PassiveFlipOutcome::Applied { modifications, .. },
                ..
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 7, final_amount: 7, .. },
                hp_change: HpChangeDelta { team, old_hp: 4, delta: -4, new_hp: 0, effective_delta: -4 },
                elemental_context_update: Some(AttackResolutionEffects { hp_changes, .. }),
                ..
            },
            GameEvent::GameEnded { conclusion },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "countershock"
            && modifications == &vec![ActionModification::SplitAttackDamage]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && hp_changes == &vec![HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 4,
                delta: -4,
                new_hp: 0,
                effective_delta: -4,
            }]
            && conclusion.outcome == GameOutcome::Draw
            && conclusion.causes == vec![GameEndCause::TeamHpDepleted {
                teams: vec![TeamId::new("team:p1"), TeamId::new("team:p2")],
            }]
    ));
    assert!(matches!(
        record.state().status,
        GameStatus::Finished { ref conclusion } if conclusion.outcome == GameOutcome::Draw
    ));
    assert!(record.state().hp.iter().all(|team| team.hp == 0));
    assert!(record.state().covered_passive(&p1).is_none());
    for card in [card(4), card(9)] {
        assert!(record.state().discard.contains(&card));
    }
    assert!(matches!(
        record.state().formation_area(&p2),
        Some(PlayerFormationArea {
            formation: Some(FormationInArea { formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }),
            ..
        }) if formation_id == "metal-strike" && cards == &vec![card(6)]
    ));
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn countershock_splits_a_generating_attack_and_both_sides_recover_hp() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = Phase::ActiveEffects;
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.hp = vec![
        TeamHp {
            team: TeamId::new("team:p1"),
            hp: 20,
        },
        TeamHp {
            team: TeamId::new("team:p2"),
            hp: 20,
        },
    ];
    state.hands = vec![
        PlayerHand::new(PlayerId::new("p1"), Vec::new()),
        PlayerHand::new(PlayerId::new("p2"), vec![card(5)]),
    ];
    state.last_elemental_attack_by_player.insert(
        PlayerId::new("p1"),
        LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        },
    );
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "metal-strike".to_string(),
            resolved_effect_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            resolved_turn: 1,
        },
    );
    cover(
        &mut state,
        "p1",
        "countershock",
        vec![card(4), card(9)],
        false,
    );

    let events = handle_command(
        &state,
        Command::PerformFormation {
            player: PlayerId::new("p2"),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        },
    )
    .unwrap();
    for event in &events {
        apply_event(&mut state, event);
    }

    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 25,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 25,
            },
        ]
    );
}

#[test]
fn seal_attack_matrix_records_not_a_spell_but_keeps_the_attack_outcome() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[3, 8, 1, 6, 9, 10, 4, 5, 7, 11, 12]);

    // 基準：合法 Fire Strike 在沒有覆蓋被動時正常解析，建立確切的未受影響攻擊
    // 結果。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(11));
    let baseline_attack = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            point_breakdown: AttackPointBreakdown { base_points: 8, final_amount: 8, .. },
            hp_change: HpChangeDelta { team, old_hp: 30, delta: -8, new_hp: 22, effective_delta: -8 },
            ..
        } if team == &TeamId::new("team:p1")
    )));

    // 修飾：P1 在相同的 incoming Attack 前合法覆蓋 Seal。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(11));
    let attack = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        attack.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: seal_cards,
                outcome: PassiveFlipOutcome::NoEffect { grounds },
            },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown { base_points: 8, final_amount: 8, .. },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -8, new_hp: 22, effective_delta: -8 },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-strike"
            && cards == &vec![card(9)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && seal_cards == &vec![card(3), card(8)]
            && grounds == &vec![PassiveNoEffectGround::NotASpell]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "fire-strike"
            && discarded_cards == &vec![card(9)]
    ));
    assert!(interaction.state().covered_passive(&p1).is_none());
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(22)
    );
    for card in [card(3), card(8), card(9)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn seal_barrier_matrix_cancels_the_spell_but_keeps_formation_commitment_and_card_movement() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[3, 8, 1, 2, 7, 12, 4, 5, 6, 10, 11, 13]);
    let barrier_cards = vec![card(7), card(12), card(4), card(6)];

    // 基準：P1 普通合法的 Metal Strike 後，Barrier 自身效果給 P2 44 點護盾。
    // 它建立互動中必須在取消後仍存活的確切行動與卡牌生命週期。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let baseline_barrier = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "barrier".to_string(),
            cards: barrier_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_barrier.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::ShieldChanged { player: shield_owner, old_value: 0, delta: 44, new_value: 44 },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "barrier"
            && cards == &barrier_cards
            && shield_owner == &p2
            && discarded_by == &p2
            && discarded == "barrier"
            && discarded_cards == &barrier_cards
    ));
    assert_eq!(baseline.state().shield(&p2), Some(44));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：P1 合法覆蓋 Seal。其擁有者看得到覆蓋的卡牌，而 P2 在觸發前只看得到
    // 數量。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    let seal = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        seal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, star_substitution: None, sealed: false },
        ] if player == &p1
            && formation_id == "seal"
            && cards == &vec![card(3), card(8)]
            && covered_by == &p1
            && covered == "seal"
            && covered_cards == &vec![card(3), card(8)]
    ));
    assert_eq!(
        public_view::state_for(interaction.state(), Viewer::Player(p2.clone())).covered_passives,
        vec![PublicCoveredPassive {
            owner: p1.clone(),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }]
    );
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(10));

    // 互動：Seal 翻開一次並取消 Barrier 的護盾結果。Barrier 仍會提交並棄置四張
    // 實體卡牌；Seal 本身被消耗，因此沒有潛在反制或捏造的護盾。
    let canceled_barrier = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "barrier".to_string(),
            cards: barrier_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(canceled_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if player == &p2 && formation_id == "barrier" && cards == &barrier_cards
    )));
    assert!(canceled_barrier.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            incoming_player,
            passive_id,
            cards,
            outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
        } if owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && cards == &vec![card(3), card(8)]
            && effect_id == "seal"
            && modifications == &vec![ActionModification::CancelSpell]
    )));
    assert!(canceled_barrier.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p2 && formation_id == "barrier" && cards == &barrier_cards
    )));
    assert!(!canceled_barrier.iter().any(|event| matches!(
        event,
        GameEvent::ShieldChanged { player, .. } if player == &p2
    )));
    assert_eq!(interaction.state().shield(&p2), Some(0));
    assert!(interaction.state().covered_passive(&p1).is_none());
    for card in [card(3), card(8), card(7), card(12), card(4), card(6)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn seal_chaos_matrix_cancels_the_choice_but_keeps_spell_commitment_and_cards() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let chaos_cards = vec![card(5), card(10), card(7), card(6)];

    // 基準：Chaos 正常只向 P2 公開其具型別延續；P2 會從檢視的 P1 卡牌中精確
    // 選擇兩張並送回 DeckTop。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 2, 3, 4, 5, 10, 7, 6, 9, 11, 12, 13]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(13));
    let baseline_chaos = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "chaos".to_string(),
            cards: chaos_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(baseline_chaos.iter().any(|event| matches!(
        event,
        GameEvent::ChoiceRequested { choice, resolution }
            if choice.player == p2
                && matches!(resolution, PendingResolution::ChaosReturnTwo)
    )));
    let baseline_choice = baseline
        .state()
        .pending_choice
        .as_ref()
        .expect("unsuppressed Chaos must create its canonical choice")
        .clone();
    let PendingChoiceKind::Card {
        cards: allowed,
        minimum,
        maximum,
        ..
    } = &baseline_choice.kind
    else {
        panic!("Chaos must request a typed Card choice");
    };
    assert_eq!((*minimum, *maximum), (2, 2));
    let chosen = vec![allowed[0], allowed[1]];
    let baseline_answer = baseline
        .handle(Command::AnswerChoice {
            player: p2.clone(),
            choice_id: baseline_choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: chosen.clone(),
            },
        })
        .unwrap();
    assert!(baseline_answer.iter().any(|event| matches!(
        event,
        GameEvent::CardsMoved { card_moves }
            if card_moves.len() == 2 && card_moves.iter().all(|movement| {
                movement.from == CardZone::Hand(p1.clone()) && movement.to == CardZone::DeckTop
            })
    )));
    assert!(baseline.state().pending_choice.is_none());
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 互動：P1 合法覆蓋 Seal。P2 相同的合法 Chaos 陣形會觸發它、消耗兩個陣形，
    // 且即使保留 Chaos 的提交/卡牌移動，也不能建立選擇或檢視 P1 手牌。
    let mut interaction = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[3, 8, 1, 2, 5, 10, 7, 6, 4, 9, 11, 12, 13]),
    )
    .unwrap();
    interaction.advance_automatic().unwrap();
    let seal = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        seal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, .. },
        ] if player == &p1
            && formation_id == "seal"
            && cards == &vec![card(3), card(8)]
            && owner == &p1
            && covered == "seal"
            && covered_cards == &vec![card(3), card(8)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(12));
    let p1_hand_before = interaction.state().hand(&p1).unwrap().to_vec();
    let canceled = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "chaos".to_string(),
            cards: chaos_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        canceled.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: seal_cards,
                outcome: PassiveFlipOutcome::Applied { modifications, .. },
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "chaos"
            && cards == &chaos_cards
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && seal_cards == &vec![card(3), card(8)]
            && modifications == &vec![ActionModification::CancelSpell]
            && discarded_by == &p2
            && discarded == "chaos"
            && discarded_cards == &chaos_cards
    ));
    assert!(!canceled.iter().any(|event| matches!(
        event,
        GameEvent::ChoiceRequested { .. } | GameEvent::HandInspected { .. }
    )));
    assert!(interaction.state().pending_choice.is_none());
    assert_eq!(
        interaction.state().hand(&p1),
        Some(p1_hand_before.as_slice())
    );
    assert!(interaction.state().covered_passive(&p1).is_none());
    for card in [card(3), card(8), card(5), card(10), card(7), card(6)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn seal_incoming_covered_passive_matrix_commits_sealed_counter_then_consumes_it_on_attack() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut record = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[3, 8, 1, 2, 4, 5, 6, 7, 9, 10, 11, 12, 13]),
    )
    .unwrap();
    record.advance_automatic().unwrap();

    // 修飾：P1 合法覆蓋 Seal，完成真正的回合抽牌，並讓 P2 的反制陣形成為下一個
    // 待測命令。
    let seal = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(3), card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        seal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p1
            && formation_id == "seal"
            && cards == &vec![card(3), card(8)]
            && owner == &p1
            && covered == "seal"
            && covered_cards == &vec![card(3), card(8)]
    ));
    advance_record_to_next_main_after_turn_draw(&mut record, card(10));
    assert_eq!(record.state().current_player(), Some(&p2));

    // 基準對照：沒有 incoming Seal 時，Countershock 正常覆蓋。這讓確切的合法
    // 使用可與下方的封印互動區分。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 2, 3, 11, 4, 9, 5, 6, 7, 8, 10]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(8));
    let unsealed = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        unsealed.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p2
            && formation_id == "countershock"
            && cards == &vec![card(4), card(9)]
            && owner == &p2
            && covered == "countershock"
            && covered_cards == &vec![card(4), card(9)]
    ));

    // 互動：Seal 被消耗，使合法覆蓋的 Countershock 被封印。其擁有者看得到識別
    // 與卡牌，而 P1 只看得到數量；兩個公開投影都不會洩漏封印標記。
    let countershock = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "countershock".to_string(),
            cards: vec![card(4), card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        countershock.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveFlipped {
                owner,
                incoming_player,
                passive_id,
                cards: seal_cards,
                outcome: PassiveFlipOutcome::Applied { modifications, .. },
            },
            GameEvent::PassiveCovered { player: covered_by, formation_id: covered, cards: covered_cards, sealed: true, .. },
        ] if player == &p2
            && formation_id == "countershock"
            && cards == &vec![card(4), card(9)]
            && owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && seal_cards == &vec![card(3), card(8)]
            && modifications == &vec![ActionModification::SealCoveredPassive]
            && covered_by == &p2
            && covered == "countershock"
            && covered_cards == &vec![card(4), card(9)]
    ));
    assert!(matches!(
        record.public_view(Viewer::Player(p2.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: Some(formation_id), cards: PublicCardRefs::Known(cards), star_substitution: None }]
            if owner == &p2 && formation_id == "countershock" && cards == &vec![card(4), card(9)]
    ));
    assert!(matches!(
        record.public_view(Viewer::Player(p1.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: None, cards: PublicCardRefs::Hidden { count: 2 }, star_substitution: None }]
            if owner == &p2
    ));

    // 下一次 P1 Attack 會恰好翻開被封印的被動。它沒有 Countershock 修飾，因此
    // 攻擊者不承受反射傷害，而 P2 仍承受正常的 Metal Strike 損失。
    record.advance_automatic().unwrap();
    answer_record_choice(
        &mut record,
        p2.clone(),
        ChoiceAnswer::Cards {
            cards: vec![card(13)],
        },
    )
    .unwrap();
    record.advance_automatic().unwrap();
    let attack = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if player == &p1 && formation_id == "metal-strike" && cards == &vec![card(1)]
    )));
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            incoming_player,
            passive_id,
            cards: passive_cards,
            outcome: PassiveFlipOutcome::NoEffect { grounds },
        } if owner == &p2
            && incoming_player == &p1
            && passive_id == "countershock"
            && passive_cards == &vec![card(4), card(9)]
            && grounds == &vec![PassiveNoEffectGround::Sealed]
    )));
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::AttackResolved {
            attacker,
            target,
            point_breakdown: AttackPointBreakdown { base_points: 7, final_amount: 7, .. },
            hp_change: HpChangeDelta { team, old_hp: 30, delta: -7, new_hp: 23, effective_delta: -7 },
            ..
        } if attacker == &p1 && target == &p2 && team == &TeamId::new("team:p2")
    )));
    assert!(attack.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p1 && formation_id == "metal-strike" && cards == &vec![card(1)]
    )));
    assert!(record.state().covered_passive(&p2).is_none());
    for card in [card(3), card(8), card(4), card(9), card(1)] {
        assert!(record.state().discard.contains(&card));
    }
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn seal_void_meridian_matrix_cancels_environment_clearing_but_keeps_spell_commitment_and_cards() {
    let p0 = PlayerId::new("p0");
    let p3 = PlayerId::new("p3");
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(300)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.players = vec![
        Player {
            id: p0.clone(),
            team: TeamId::new("team:a"),
        },
        Player {
            id: p3.clone(),
            team: TeamId::new("team:b"),
        },
        Player {
            id: p1.clone(),
            team: TeamId::new("team:a"),
        },
        Player {
            id: p2.clone(),
            team: TeamId::new("team:b"),
        },
    ];
    setup.turn_order = vec![p0.clone(), p3.clone(), p1.clone(), p2.clone()];
    setup.hp = vec![
        TeamHp {
            team: TeamId::new("team:a"),
            hp: 300,
        },
        TeamHp {
            team: TeamId::new("team:b"),
            hp: 300,
        },
    ];
    // 額外的三張金卡只是固定背景：它們共同的印製等級使後續合法的 Void Meridian
    // 使用有效。
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(26, "metal"),
        card_instance(31, "metal"),
        card_instance(36, "metal"),
        card_instance(41, "metal"),
        card_instance(46, "earth"),
        card_instance(47, "wood"),
        card_instance(48, "fire"),
    ]);
    let opening = vec![
        card(1),
        card(2),
        card(3),
        card(4), // P0 起手
        card(6),
        card(11),
        card(16),
        card(21),
        card(26), // P3 起手：West White Tiger
        card(8),
        card(13),
        card(5),
        card(7),
        card(9), // P1 起手：Seal
        card(31),
        card(36),
        card(41),
        card(14),
        card(15), // P2 起手：Void Meridian
        card(10),
        card(12),
        card(17),
        card(18),
        card(19),
        card(20), // P0/P3 回合抽牌
        card(46),
        card(47),
        card(48), // P1 互動回合抽牌
    ];
    let mut deck = opening.clone();
    deck.extend(
        (1..=20)
            .map(card)
            .filter(|candidate| !opening.contains(candidate)),
    );

    fn finish_turn(record: &mut GameRecord, player: PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player,
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    fn record_before_modifier(
        setup: &GameSetup,
        deck: &[CardInstanceId],
        p0: &PlayerId,
        p3: &PlayerId,
        p1: &PlayerId,
        p2: &PlayerId,
    ) -> GameRecord {
        let mut record = GameRecord::start(setup.clone(), deck.to_vec()).unwrap();
        record.advance_automatic().unwrap();

        // P0 只推進回合。接著 P3 建立 Environment，P1 取得獨立的合法行動，而 P2
        // 以真正已持有 Void Meridian 的起手牌抵達。
        record
            .handle(Command::PerformFormation {
                player: p0.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        finish_turn(&mut record, p0.clone(), card(10));

        // 共用 Metal Environment 只能由合法 Sacred Beast 命令建立，而不是由固定
        // 資料變更注入。
        let west_white_tiger = record
            .handle(Command::PerformFormation {
                player: p3.clone(),
                formation_id: "west-white-tiger".to_string(),
                cards: vec![card(6), card(11), card(16), card(21), card(26)],
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(west_white_tiger.iter().any(|event| matches!(
            event,
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            } if environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p3.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
        )));
        assert_eq!(record.state().environment, Some(Element::Metal));
        finish_turn(&mut record, p3.clone(), card(18));

        assert_eq!(record.state().current_player(), Some(p1));
        assert_eq!(record.state().environment, Some(Element::Metal));
        assert!(record.state().hand(p2).unwrap().contains(&card(31)));
        assert!(record.state().hand(p2).unwrap().contains(&card(36)));
        assert!(record.state().hand(p2).unwrap().contains(&card(41)));
        record
    }

    // 基準：適用的 Void Meridian 法術會清除共用 Environment，並各改變兩隊生命值
    // 一次。其提交與卡牌移動是取消分支的參考生命週期。
    let mut baseline = record_before_modifier(&setup, &deck, &p0, &p3, &p1, &p2);
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "water-strike".to_string(),
            cards: vec![card(8)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, p1.clone(), card(46));
    let baseline_before_void = baseline.state().hp.clone();
    let void_baseline = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(31), card(36), card(41)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        void_baseline.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::EnvironmentCleared { player: clearer, formation_id: cleared_by, environment: Element::Metal, hp_changes },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "void-meridian-severing"
            && cards == &vec![card(31), card(36), card(41)]
            && clearer == &p2
            && cleared_by == "void-meridian-severing"
            && hp_changes.len() == 2
            && hp_changes.iter().zip(&baseline_before_void).all(|(change, before)| {
                change.team == before.team
                    && change.old_hp == before.hp
                    && change.new_hp == before.hp - 20
                    && change.effective_delta == -20
            })
            && discarded_by == &p2
            && discarded == "void-meridian-severing"
            && discarded_cards == &vec![card(31), card(36), card(41)]
    ));
    assert_eq!(baseline.state().environment, None);
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾本身：Seal 透過完整合法命令建立，並維持私有識別，直到 P2 執行下一個
    // 行動。
    let mut interaction = record_before_modifier(&setup, &deck, &p0, &p3, &p1, &p2);
    let seal = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "seal".to_string(),
            cards: vec![card(8), card(13)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        seal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceDownResolving, .. },
            GameEvent::PassiveCovered { player: owner, formation_id: covered, cards: covered_cards, sealed: false, .. },
        ] if player == &p1
            && formation_id == "seal"
            && cards == &vec![card(8), card(13)]
            && owner == &p1
            && covered == "seal"
            && covered_cards == &vec![card(8), card(13)]
    ));
    assert!(matches!(
        interaction.public_view(Viewer::Player(p1.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: Some(formation_id), cards: PublicCardRefs::Known(cards), star_substitution: None }]
            if owner == &p1 && formation_id == "seal" && cards == &vec![card(8), card(13)]
    ));
    assert!(matches!(
        interaction.public_view(Viewer::Player(p2.clone())).unwrap().covered_passives.as_slice(),
        [PublicCoveredPassive { owner, formation_id: None, cards: PublicCardRefs::Hidden { count: 2 }, star_substitution: None }]
            if owner == &p1
    ));
    finish_turn(&mut interaction, p1.clone(), card(46));
    let interaction_before_void = interaction.state().hp.clone();

    // 互動：Seal 取消適用的法術效果，但不取消其標準提交或實體卡牌移動。因此
    // Environment 與兩隊生命值都維持在 Void Meridian 之前的狀態。
    let void_canceled = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "void-meridian-severing".to_string(),
            cards: vec![card(31), card(36), card(41)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(void_canceled.iter().any(|event| matches!(
        event,
        GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. }
            if player == &p2
                && formation_id == "void-meridian-severing"
                && cards == &vec![card(31), card(36), card(41)]
    )));
    assert!(void_canceled.iter().any(|event| matches!(
        event,
        GameEvent::PassiveFlipped {
            owner,
            incoming_player,
            passive_id,
            cards,
            outcome: PassiveFlipOutcome::Applied { effect_id, modifications },
        } if owner == &p1
            && incoming_player == &p2
            && passive_id == "seal"
            && cards == &vec![card(8), card(13)]
            && effect_id == "seal"
            && modifications == &vec![ActionModification::CancelSpell]
    )));
    assert!(
        !void_canceled
            .iter()
            .any(|event| matches!(event, GameEvent::EnvironmentCleared { .. }))
    );
    assert!(void_canceled.iter().any(|event| matches!(
        event,
        GameEvent::FormationCardsDiscarded { player, formation_id, cards }
            if player == &p2
                && formation_id == "void-meridian-severing"
                && cards == &vec![card(31), card(36), card(41)]
    )));
    assert_eq!(interaction.state().environment, Some(Element::Metal));
    assert_eq!(interaction.state().hp, interaction_before_void);
    assert!(interaction.state().covered_passive(&p1).is_none());
    for used in [card(8), card(13), card(31), card(36), card(41)] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn public_state_view_includes_client_state_and_filters_hands_by_viewer() {
    let record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let state = record.state().clone();

    let p1_view = public_view::state_for(&state, Viewer::Player(PlayerId::new("p1")));
    assert_eq!(p1_view.status, GameStatus::InProgress);
    assert_eq!(p1_view.turn_number, 1);
    assert_eq!(p1_view.phase, Phase::TurnStart);
    assert_eq!(p1_view.current_player, Some(PlayerId::new("p1")));
    assert_eq!(p1_view.players, two_player_setup().players);
    assert_eq!(
        p1_view.turn_order,
        vec![PlayerId::new("p1"), PlayerId::new("p2")]
    );
    assert_eq!(
        p1_view.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 30,
            },
        ]
    );
    assert_eq!(
        p1_view.hands,
        vec![
            PublicPlayerHand {
                player: PlayerId::new("p1"),
                cards: PublicCardRefs::Known(vec![card(1), card(2), card(3), card(4)]),
            },
            PublicPlayerHand {
                player: PlayerId::new("p2"),
                cards: PublicCardRefs::Hidden { count: 5 },
            },
        ]
    );
    assert_eq!(p1_view.discard, Vec::<CardInstanceId>::new());
    assert_eq!(p1_view.covered_passives, Vec::new());
    assert_eq!(p1_view.pending_choice, None);
    assert_eq!(
        p1_view.shields,
        vec![
            PlayerShield {
                player: PlayerId::new("p1"),
                value: 0,
            },
            PlayerShield {
                player: PlayerId::new("p2"),
                value: 0,
            },
        ]
    );
    assert_eq!(p1_view.statuses, Vec::<StatusEffect>::new());

    let p2_view = public_view::state_for(&state, Viewer::Player(PlayerId::new("p2")));
    assert_eq!(
        p2_view.hands,
        vec![
            PublicPlayerHand {
                player: PlayerId::new("p1"),
                cards: PublicCardRefs::Hidden { count: 4 },
            },
            PublicPlayerHand {
                player: PlayerId::new("p2"),
                cards: PublicCardRefs::Known(vec![card(5), card(6), card(7), card(8), card(9)]),
            },
        ]
    );

    let observer_view = public_view::state_for(&state, Viewer::Observer);
    assert_eq!(
        observer_view.hands,
        vec![
            PublicPlayerHand {
                player: PlayerId::new("p1"),
                cards: PublicCardRefs::Hidden { count: 4 },
            },
            PublicPlayerHand {
                player: PlayerId::new("p2"),
                cards: PublicCardRefs::Hidden { count: 5 },
            },
        ]
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn public_state_view_exposes_only_the_previous_turns_formation() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.turn_number = 3;
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "metal-strike".to_string(),
            resolved_effect_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            resolved_turn: 1,
        },
    );
    state.last_formation_by_player.insert(
        PlayerId::new("p2"),
        LastFormationUse {
            formation_id: "wood-strike".to_string(),
            resolved_effect_id: "wood-strike".to_string(),
            used_cards: vec![card(2)],
            resolved_turn: 2,
        },
    );

    assert_eq!(
        public_view::state_for(&state, Viewer::Observer).previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: PlayerId::new("p2"),
            formation_id: Some("wood-strike".to_string()),
            cards: PublicCardRefs::Known(vec![card(2)]),
        })
    );
}

#[test]
fn previous_turn_covered_formation_hides_details_from_other_players() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.turn_number = 2;
    state.last_formation_by_player.insert(
        PlayerId::new("p1"),
        LastFormationUse {
            formation_id: "defense".to_string(),
            resolved_effect_id: "defense".to_string(),
            used_cards: vec![card(2), card(7)],
            resolved_turn: 1,
        },
    );
    cover(&mut state, "p1", "defense", vec![card(2), card(7)], false);

    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p1"))).previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: PlayerId::new("p1"),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(vec![card(2), card(7)]),
        })
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p2"))).previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
        })
    );
}

#[test]
fn initial_deal_event_view_filters_cards_to_dealt_player() {
    let event = GameEvent::CardsDealt {
        player: PlayerId::new("p1"),
        cards: vec![card(1), card(2), card(3), card(4)],
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: PublicCardRefs::Known(vec![card(1), card(2), card(3), card(4)]),
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: PublicCardRefs::Hidden { count: 4 },
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: PublicCardRefs::Hidden { count: 4 },
        }
    );
    assert_eq!(
        event,
        GameEvent::CardsDealt {
            player: PlayerId::new("p1"),
            cards: vec![card(1), card(2), card(3), card(4)],
        }
    );
}

#[test]
fn deck_prepared_event_view_hides_deck_order_for_every_viewer() {
    let event = GameEvent::DeckPrepared {
        deck_order: vec![card(1), card(2), card(3)],
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::DeckPrepared {
            deck: PublicCardRefs::Hidden { count: 3 },
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::DeckPrepared {
            deck: PublicCardRefs::Hidden { count: 3 },
        }
    );
    assert_eq!(
        event,
        GameEvent::DeckPrepared {
            deck_order: vec![card(1), card(2), card(3)],
        }
    );
}

#[test]
fn pending_effect_choice_state_view_shows_options_only_to_choice_player() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 2, 1])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "chaos".to_string(),
            cards: vec![card(5), card(10), card(2), card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let state = record.state().clone();

    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p1"))).pending_choice,
        Some(PublicPendingChoice::Visible {
            choice_id: ChoiceId::new(1),
            player: PlayerId::new("p1"),
            reason: PublicPendingChoicePresentation::Chaos,
            choice: PendingChoiceKind::Card {
                cards: vec![card(3), card(4), card(6), card(7), card(8)],
                minimum: 2,
                maximum: 2,
                can_decline: false,
            },
        })
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Player(PlayerId::new("p2"))).pending_choice,
        Some(PublicPendingChoice::Hidden {
            player: PlayerId::new("p1"),
            reason: PublicPendingChoicePresentation::Chaos,
        })
    );
    assert_eq!(
        public_view::state_for(&state, Viewer::Observer).pending_choice,
        Some(PublicPendingChoice::Hidden {
            player: PlayerId::new("p1"),
            reason: PublicPendingChoicePresentation::Chaos,
        })
    );
    assert_eq!(
        state.pending_choice,
        Some(PendingChoice {
            choice_id: ChoiceId::new(1),
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::Card {
                cards: vec![card(3), card(4), card(6), card(7), card(8)],
                minimum: 2,
                maximum: 2,
                can_decline: false,
            },
        })
    );
    assert_eq!(record.replay().unwrap(), state);
}

#[test]
fn passive_cover_event_view_filters_hidden_card_ids_without_changing_canonical_event() {
    let event = GameEvent::PassiveCovered {
        player: PlayerId::new("p1"),
        formation_id: "defense".to_string(),
        cards: vec![card(2), card(7)],
        star_substitution: None,
        sealed: true,
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: Some("defense".to_string()),
            cards: PublicCardRefs::Known(vec![card(2), card(7)]),
            star_substitution: None,
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: None,
            cards: PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }
    );
    assert_eq!(
        event,
        GameEvent::PassiveCovered {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            star_substitution: None,
            sealed: true,
        }
    );
}

#[test]
fn choice_requested_event_view_filters_options_and_resolutions() {
    let event = GameEvent::ChoiceRequested {
        choice: PendingChoice {
            choice_id: ChoiceId::new(9),
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::Card {
                cards: vec![card(1), card(2)],
                minimum: 2,
                maximum: 2,
                can_decline: false,
            },
        },
        resolution: PendingResolution::ChaosReturnTwo,
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::ChoiceRequested {
            choice: PublicPendingChoice::Visible {
                choice_id: ChoiceId::new(9),
                player: PlayerId::new("p1"),
                reason: PublicPendingChoicePresentation::Chaos,
                choice: PendingChoiceKind::Card {
                    cards: vec![card(1), card(2)],
                    minimum: 2,
                    maximum: 2,
                    can_decline: false,
                },
            },
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::ChoiceRequested {
            choice: PublicPendingChoice::Hidden {
                player: PlayerId::new("p1"),
                reason: PublicPendingChoicePresentation::Chaos,
            },
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Observer),
        PublicGameEvent::ChoiceRequested {
            choice: PublicPendingChoice::Hidden {
                player: PlayerId::new("p1"),
                reason: PublicPendingChoicePresentation::Chaos,
            },
        }
    );
    assert_eq!(
        event,
        GameEvent::ChoiceRequested {
            choice: PendingChoice {
                choice_id: ChoiceId::new(9),
                player: PlayerId::new("p1"),
                kind: PendingChoiceKind::Card {
                    cards: vec![card(1), card(2)],
                    minimum: 2,
                    maximum: 2,
                    can_decline: false,
                },
            },
            resolution: PendingResolution::ChaosReturnTwo,
        }
    );
}

#[test]
fn turn_draw_choice_event_view_filters_choice_options_to_choice_player() {
    let event = GameEvent::CardsDrawnForTurnDiscardChoice {
        player: PlayerId::new("p1"),
        drawn_cards: vec![card(10), card(11), card(12)],
        allowed_discards: vec![card(10), card(11), card(12)],
    };

    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p1"))),
        PublicGameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("p1"),
            drawn_cards: PublicCardRefs::Known(vec![card(10), card(11), card(12)]),
            allowed_discards: PublicCardRefs::Known(vec![card(10), card(11), card(12)]),
        }
    );
    assert_eq!(
        public_view::event_for(&event, Viewer::Player(PlayerId::new("p2"))),
        PublicGameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("p1"),
            drawn_cards: PublicCardRefs::Hidden { count: 3 },
            allowed_discards: PublicCardRefs::Hidden { count: 3 },
        }
    );
    assert_eq!(
        event,
        GameEvent::CardsDrawnForTurnDiscardChoice {
            player: PlayerId::new("p1"),
            drawn_cards: vec![card(10), card(11), card(12)],
            allowed_discards: vec![card(10), card(11), card(12)],
        }
    );
}

#[test]
fn invalid_perform_formation_commands_leave_events_and_state_unchanged() {
    let mut record = GameRecord::start(two_player_setup(), official_deck()).unwrap();
    let turn_start_events = record.events().to_vec();
    let turn_start_state = record.state().clone();

    assert_eq!(
        record.handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        }),
        Err(GameError::Validation(ValidationError::WrongPhase {
            expected: Phase::ActiveEffects,
            actual: Phase::TurnStart,
        }))
    );
    assert_eq!(record.events(), turn_start_events.as_slice());
    assert_eq!(record.state().clone(), turn_start_state);

    record.advance_automatic().unwrap();

    let cases = [
        (
            Command::PerformFormation {
                player: PlayerId::new("p2"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(5)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::WrongPlayer {
                expected: PlayerId::new("p1"),
                actual: PlayerId::new("p2"),
            }),
        ),
        (
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "missing".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::UnknownFormation("missing".to_string())),
        ),
        (
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "weapon".to_string(),
                cards: vec![card(1), card(1)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::DuplicateSubmittedCard(card(1))),
        ),
        (
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(5)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::CardNotInHand(card(5))),
        ),
        (
            Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1), card(2)],
                declared_targets: Vec::new(),
            },
            GameError::Validation(ValidationError::FormationPatternMismatch {
                formation_id: "metal-strike".to_string(),
            }),
        ),
    ];

    for (command, expected_error) in cases {
        let events_before = record.events().to_vec();
        let state_before = record.state().clone();

        assert_eq!(record.handle(command), Err(expected_error));
        assert_eq!(record.events(), events_before.as_slice());
        assert_eq!(record.state().clone(), state_before);
    }
}

#[test]
fn perform_formation_matches_cards_by_instance_definitions() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[1, 6, 2, 3])).unwrap();
    record.advance_automatic().unwrap();

    assert_event_semantics_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "weapon".to_string(),
                cards: vec![card(1), card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p1"),
            target: PlayerId::new("p2"),
            formation_id: "weapon".to_string(),
            used_cards: vec![card(1), card(6)],
            point_breakdown: AttackPointBreakdown {
                base_points: 12,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 12,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 30,
                delta: -12,
                new_hp: 18,
                effective_delta: -12,
            },
            shield_change: None,
            card_moves: vec![
                CardMoveDelta {
                    card: card(1),
                    from: CardZone::Hand(PlayerId::new("p1")),
                    to: CardZone::Discard,
                },
                CardMoveDelta {
                    card: card(6),
                    from: CardZone::Hand(PlayerId::new("p1")),
                    to: CardZone::Discard,
                },
            ],
            elemental_context_update: None,
        }]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hand(&PlayerId::new("p1")),
        Some(vec![card(2), card(3)].as_slice())
    );
    assert_eq!(state.discard, vec![card(1), card(6)]);
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 30,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 18,
            },
        ]
    );
    assert!(state.last_elemental_attack_by_player.is_empty());
}

#[test]
fn attack_hp_delta_records_clamped_damage() {
    let mut record = GameRecord::start(two_player_setup_with_hp(5), official_deck()).unwrap();
    record.advance_automatic().unwrap();

    assert_event_semantics_eq!(
        record
            .handle(Command::PerformFormation {
                player: PlayerId::new("p1"),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        vec![GameEvent::AttackResolved {
            attacker: PlayerId::new("p1"),
            target: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            used_cards: vec![card(1)],
            point_breakdown: AttackPointBreakdown {
                base_points: 7,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 7,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 5,
                delta: -7,
                new_hp: 0,
                effective_delta: -5,
            },
            shield_change: None,
            card_moves: vec![CardMoveDelta {
                card: card(1),
                from: CardZone::Hand(PlayerId::new("p1")),
                to: CardZone::Discard,
            }],
            elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                player: PlayerId::new("p1"),
                attack: LastElementalAttack {
                    element: Element::Metal,
                    resolved_turn: 1,
                },
            }),
        }]
    );

    let state = record.state().clone();
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 5,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 0,
            },
        ]
    );
}

#[test]
fn attack_that_reduces_a_team_to_zero_finishes_game_with_opposing_team_winner() {
    let mut record = GameRecord::start(two_player_setup_with_hp(5), official_deck()).unwrap();
    record.advance_automatic().unwrap();

    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let state = record.state().clone();
    assert!(matches!(
        state.status,
        GameStatus::Finished { ref conclusion }
            if conclusion.outcome == GameOutcome::Winner(TeamId::new("team:p1"))
    ));
    assert_eq!(
        state.hp,
        vec![
            TeamHp {
                team: TeamId::new("team:p1"),
                hp: 5,
            },
            TeamHp {
                team: TeamId::new("team:p2"),
                hp: 0,
            },
        ]
    );
}

#[test]
fn commands_after_game_over_are_rejected_without_events_or_state_changes() {
    let mut record = GameRecord::start(two_player_setup_with_hp(5), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(
        record.handle(Command::PassAction {
            player: PlayerId::new("p1"),
            reason: PassActionReason::CannotActByStatus,
        }),
        Err(GameError::Validation(ValidationError::GameFinished))
    );
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn automatic_advance_stops_after_game_over() {
    let mut record = GameRecord::start(two_player_setup_with_hp(5), official_deck()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let events_before = record.events().to_vec();
    let state_before = record.state().clone();

    assert_eq!(record.advance_automatic().unwrap(), Vec::<GameEvent>::new());
    assert_eq!(record.events(), events_before.as_slice());
    assert_eq!(record.state().clone(), state_before);
}

#[test]
fn hp_resolution_finishes_as_draw_when_no_team_remains_alive() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);
    state.phase = Phase::ActiveEffects;
    state.hp = vec![
        TeamHp {
            team: TeamId::new("team:p1"),
            hp: 0,
        },
        TeamHp {
            team: TeamId::new("team:p2"),
            hp: 1,
        },
    ];

    apply_event(
        &mut state,
        &GameEvent::AttackResolved {
            attacker: PlayerId::new("p1"),
            target: PlayerId::new("p2"),
            formation_id: "metal-strike".to_string(),
            used_cards: Vec::new(),
            point_breakdown: AttackPointBreakdown {
                base_points: 7,
                environment_effect: EnvironmentAttackEffect::None,
                interaction: ElementInteraction::None,
                damage_transform: DamageTransform::NormalDamage,
                final_amount: 7,
            },
            hp_change: HpChangeDelta {
                team: TeamId::new("team:p2"),
                old_hp: 1,
                delta: -7,
                new_hp: 0,
                effective_delta: -1,
            },
            shield_change: None,
            card_moves: Vec::new(),
            elemental_context_update: None,
        },
    );
    apply_event(
        &mut state,
        &GameEvent::GameEnded {
            conclusion: GameConclusion::new(
                GameOutcome::Draw,
                vec![GameEndCause::TeamHpDepleted {
                    teams: vec![TeamId::new("team:p1"), TeamId::new("team:p2")],
                }],
                None,
            ),
        },
    );

    assert!(matches!(
        state.status,
        GameStatus::Finished { ref conclusion } if conclusion.outcome == GameOutcome::Draw
    ));
}

#[test]
fn elemental_attack_previous_element_matrix_distinguishes_physical_baseline_from_overcoming() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：真正的實體 Weapon 使用是立即的前一個陣形，但不建立元素脈絡。因此
    // 下一個 Fire Strike 會解析正常的八點傷害。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 6, 2, 3, 9, 4, 5, 7, 8]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let ordinary_fire = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        ordinary_fire,
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "fire-strike".to_string(),
                used_cards: vec![card(9)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 8,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 8,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: -8,
                    new_hp: 22,
                    effective_delta: -8,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 2,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
            },
        ]
    );
    assert!(
        baseline
            .state()
            .last_elemental_attack_by_player
            .get(&p1)
            .is_none()
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：P1 使用相同的公開命令路徑建立立即的金元素脈絡。它不是手動寫入
    // 最後元素狀態。
    let mut interaction = record_after_p1_metal_attack_on_turn_1();
    assert_eq!(
        interaction.state().last_elemental_attack_by_player.get(&p1),
        Some(&LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        })
    );
    assert_eq!(interaction.state().current_player(), Some(&p2));

    // 互動：Fire 克制緊接之前的金攻擊，將相同的基礎八點精確加倍。陣形提交、
    // 卡牌移動、具型別脈絡、最終狀態與回放仍然是標準的。
    let overcoming_fire = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        overcoming_fire,
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "fire-strike".to_string(),
                used_cards: vec![card(9)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 8,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::Overcoming,
                    damage_transform: DamageTransform::DoubleDamage,
                    final_amount: 16,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: -16,
                    new_hp: 14,
                    effective_delta: -16,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 2,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
            },
        ]
    );
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(14)
    );
    assert!(interaction.state().discard.contains(&card(1)));
    assert!(interaction.state().discard.contains(&card(9)));
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn elemental_attack_same_element_matrix_uses_legal_previous_metal_and_rounds_up() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");

    // 基準：P1 的實體 Weapon 是一個完整合法的前一個行動，但不建立元素脈絡；
    // 因此 P2 的 Metal Strike 維持正常 7 點傷害。
    let mut baseline = GameRecord::start(
        two_player_setup(),
        deck_starting_with(&[1, 11, 2, 3, 6, 9, 5, 7, 8, 10, 12, 13]),
    )
    .unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    let ordinary_metal = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        ordinary_metal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -7, new_hp: 23, effective_delta: -7 },
                shield_change: None,
                card_moves,
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && card_moves.is_empty()
            && discarded_by == &p2
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(6)]
    ));
    assert!(
        baseline
            .state()
            .last_elemental_attack_by_player
            .get(&p1)
            .is_none()
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾與交互：P1 以公開合法 Metal Strike 建立唯一可用的立即元素脈絡。P2
    // 的同元素 Metal Strike 仍會提交與棄牌，但 7 點傷害以半數向上取整為 4。
    let mut interaction = record_after_p1_metal_attack_on_turn_1();
    assert_eq!(interaction.state().current_player(), Some(&p2));
    assert_eq!(
        interaction.state().last_elemental_attack_by_player.get(&p1),
        Some(&LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        })
    );
    let same_metal = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        same_metal.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::Same,
                    damage_transform: DamageTransform::HalfDamageRoundUp,
                    final_amount: 4,
                },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -4, new_hp: 26, effective_delta: -4 },
                shield_change: None,
                card_moves,
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Metal, resolved_turn: 2 },
                    }),
                    outcome: AttackOutcome::Resolved,
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(6)]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && card_moves.is_empty()
            && context_player == &p2
            && discarded_by == &p2
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(6)]
    ));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(26)
    );
    assert!(interaction.state().discard.contains(&card(1)));
    assert!(interaction.state().discard.contains(&card(6)));
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn elemental_attack_generating_matrix_uses_legal_previous_metal_to_heal_an_injured_target() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup();
    // 額外的 Metal 只固定第二個 P1 action 所需手牌；沒有直接安排元素歷史、
    // HP、或任何解析結果。
    setup
        .card_instances
        .extend([card_instance(21, "metal"), card_instance(22, "metal")]);
    let deck = {
        let opening = vec![
            card(1),
            card(6),
            card(21),
            card(22), // P1：先實體 Weapon，後續可選 Weapon 或 Metal Strike
            card(11),
            card(16),
            card(5),
            card(4),
            card(7), // P2：Weapon 先使 P1 受傷，保留 Earth Strike
            card(10),
            card(12),
            card(13),
            card(14),
            card(15),
            card(17),
            card(18),
            card(19),
            card(20),
        ];
        let mut cards = opening.clone();
        cards.extend(
            (1..=20)
                .map(card)
                .filter(|candidate| !opening.contains(candidate)),
        );
        cards
    };

    // 基準：P1 的第二個實體 Weapon 不建立元素前置，因此受傷 P1 面對 Earth
    // Strike 時仍受到正常 9 點傷害。
    let mut baseline = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut baseline, &p1);
    baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(11), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut baseline, &p2);
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(18)
    );
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(21), card(22)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut baseline, &p1);
    let ordinary_earth = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        ordinary_earth.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 9,
                },
                hp_change: HpChangeDelta { team, old_hp: 18, delta: -9, new_hp: 9, effective_delta: -9 },
                shield_change: None,
                card_moves,
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "earth-strike"
            && cards == &vec![card(5)]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && card_moves.is_empty()
            && discarded_by == &p2
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(5)]
    ));
    assert!(
        baseline
            .state()
            .last_elemental_attack_by_player
            .get(&p1)
            .is_none()
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾與互動：唯一差別是 P1 的第二個 action 改為合法 Metal Strike，建立
    // 立即 Metal 脈絡；P2 的 Earth Strike 因相生轉為對同一受傷目標回復 9。
    let mut interaction = GameRecord::start(setup, deck).unwrap();
    interaction.advance_automatic().unwrap();
    interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut interaction, &p1);
    interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(11), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut interaction, &p2);
    let metal_context = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(21)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        metal_context.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Metal, resolved_turn: 3 },
                    }),
                    outcome: AttackOutcome::Resolved,
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "metal-strike"
            && cards == &vec![card(21)]
            && context_player == &p1
            && discarded_by == &p1
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(21)]
    ));
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut interaction, &p1);
    let generating_earth = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(5)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        generating_earth.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::Generating,
                    damage_transform: DamageTransform::HealTarget,
                    final_amount: 9,
                },
                hp_change: HpChangeDelta { team, old_hp: 18, delta: 9, new_hp: 27, effective_delta: 9 },
                shield_change: None,
                card_moves,
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Earth, resolved_turn: 4 },
                    }),
                    outcome: AttackOutcome::Resolved,
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "earth-strike"
            && cards == &vec![card(5)]
            && attacker == &p2
            && target == &p1
            && team == &TeamId::new("team:p1")
            && card_moves.is_empty()
            && context_player == &p2
            && discarded_by == &p2
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(5)]
    ));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(27)
    );
    for used in [card(1), card(6), card(11), card(16), card(21), card(5)] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn metal_environment_generating_matrix_recovers_once_when_both_grounds_apply() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(200)
        .with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    setup.card_instances.extend([
        card_instance(21, "metal"),
        card_instance(22, "metal"),
        card_instance(23, "earth"),
        card_instance(24, "metal"),
        card_instance(25, "metal"),
    ]);
    let deck = {
        let opening = vec![
            card(1),
            card(6),
            card(11),
            card(2), // P1：首回合 Wood，保留三張 West White Tiger Metal
            card(22),
            card(24),
            card(25),
            card(4),
            card(5), // P2：實體 Weapon bridge，保留第二回合 Metal
            card(16),
            card(21),
            card(3), // P1：補足 Sacred Beast；第三張作 Turn Draw discard
            card(9),
            card(10),
            card(12), // P2 Turn Draw
            card(13),
            card(23),
            card(14), // P1 在 Beast 後取得 Earth Strike
        ];
        let mut cards = opening.clone();
        cards.extend(
            (1..=20)
                .map(card)
                .filter(|candidate| !opening.contains(candidate)),
        );
        cards
    };
    let beast_cards = vec![card(1), card(6), card(11), card(16), card(21)];

    let mut record = GameRecord::start(setup, deck).unwrap();
    record.advance_automatic().unwrap();

    // 以合法 turn commands 準備出 P1 的五張 Metal。這兩個 bridge 不建立受測
    // Environment 或最後元素脈絡；它們只抵達下一個合法 action boundary。
    record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "wood-strike".to_string(),
            cards: vec![card(2)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut record, card(3));
    record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(24), card(25)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut record, &p2);
    assert_eq!(record.state().current_player(), Some(&p1));

    // 修飾本身：West White Tiger 以完整合法 Use 建立 Metal Environment，並使
    // P2 先成為可觀測的回復目標。
    let beast = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "west-white-tiger".to_string(),
            cards: beast_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        beast.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown { base_points: 81, interaction: ElementInteraction::None, final_amount: 81, .. },
                hp_change: HpChangeDelta { team, old_hp: 194, delta: -81, new_hp: 113, effective_delta: -81 },
                elemental_context_update: Some(AttackResolutionEffects { environment_transfers, .. }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "west-white-tiger"
            && cards == &beast_cards
            && environment_transfers == &vec![fewfc::domain::EnvironmentTransferDelta {
                player: p1.clone(),
                formation_id: "west-white-tiger".to_string(),
                from: None,
                to: Element::Metal,
            }]
            && discarded_by == &p1
            && discarded == "west-white-tiger"
            && discarded_cards == &beast_cards
    ));
    assert_eq!(record.state().environment, Some(Element::Metal));
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut record, &p1);

    // P2 的合法 Metal Strike 同時是下一個 Player 的立即元素前置。Metal
    // Environment 的加倍和前一個 Beast Metal 的同元素折半都屬無關 sibling，
    // 但不能取代這個新的 P2 context。
    let metal_context = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(22)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        metal_context.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::MatchingElementDamageDoubled { environment: Element::Metal },
                    interaction: ElementInteraction::Same,
                    damage_transform: DamageTransform::HalfDamageRoundUp,
                    final_amount: 7,
                },
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Metal, .. },
                    }),
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "metal-strike"
            && cards == &vec![card(22)]
            && context_player == &p2
            && discarded_by == &p2
            && discarded == "metal-strike"
            && discarded_cards == &vec![card(22)]
    ));
    advance_record_to_next_main_discarding_first_turn_draw_card(&mut record, &p2);

    // 交互：Earth 同時生成 Metal Environment 與 P2 的最後 Metal Attack。兩個
    // grounds 都要求回復，但 canonical point breakdown 僅保留一次 9 點回復。
    assert!(record.state().hand(&p1).unwrap().contains(&card(23)));
    let earth = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "earth-strike".to_string(),
            cards: vec![card(23)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        earth.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                point_breakdown: AttackPointBreakdown {
                    base_points: 9,
                    environment_effect: EnvironmentAttackEffect::GeneratingElementDamageConvertedToHealing { environment: Element::Metal },
                    interaction: ElementInteraction::Generating,
                    damage_transform: DamageTransform::HealTarget,
                    final_amount: 9,
                },
                hp_change: HpChangeDelta { team, old_hp: 113, delta: 9, new_hp: 122, effective_delta: 9 },
                shield_change: None,
                card_moves,
                elemental_context_update: Some(AttackResolutionEffects {
                    elemental_context_update: Some(LastElementalAttackUpdate {
                        player: context_player,
                        attack: LastElementalAttack { element: Element::Earth, .. },
                    }),
                    outcome: AttackOutcome::Resolved,
                    ..
                }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "earth-strike"
            && cards == &vec![card(23)]
            && attacker == &p1
            && target == &p2
            && team == &TeamId::new("team:p2")
            && card_moves.is_empty()
            && context_player == &p1
            && discarded_by == &p1
            && discarded == "earth-strike"
            && discarded_cards == &vec![card(23)]
    ));
    assert_eq!(record.state().environment, Some(Element::Metal));
    assert_eq!(
        record
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p2"))
            .map(|entry| entry.hp),
        Some(122)
    );
    for used in beast_cards.iter().chain([card(22), card(23)].iter()) {
        assert!(record.state().discard.contains(used));
    }
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn elemental_attack_unrelated_previous_element_matrix_keeps_normal_damage() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let setup = two_player_setup_with_hp(100);
    let deck = deck_starting_with(&[1, 6, 2, 3, 9, 4, 5, 7, 8]);

    let weapon_events = || {
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: vec![card(1), card(6)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "weapon".to_string(),
                used_cards: vec![card(1), card(6)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 100,
                    delta: -12,
                    new_hp: 88,
                    effective_delta: -12,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: Some(AttackResolutionEffects {
                    outcome: AttackOutcome::Resolved,
                    ..Default::default()
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: vec![card(1), card(6)],
            },
        ]
    };
    let metal_events = || {
        vec![
            GameEvent::FormationCommitted {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p1.clone(),
                target: p2.clone(),
                formation_id: "metal-strike".to_string(),
                used_cards: vec![card(1)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 7,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 7,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 100,
                    delta: -7,
                    new_hp: 93,
                    effective_delta: -7,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p1.clone(),
                    attack: LastElementalAttack {
                        element: Element::Metal,
                        resolved_turn: 1,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
            },
        ]
    };
    let wood_events = || {
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(7)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "wood-strike".to_string(),
                used_cards: vec![card(7)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 6,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 6,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 100,
                    delta: -6,
                    new_hp: 94,
                    effective_delta: -6,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Wood,
                        resolved_turn: 2,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(7)],
            },
        ]
    };

    // 無修飾基準：實體 Weapon 是合法的立即前一個 Formation，但不建立元素脈絡；
    // Wood Strike 因此正常造成六點傷害。
    let mut baseline = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    assert_eq!(
        baseline
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "weapon".to_string(),
                cards: vec![card(1), card(6)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        weapon_events()
    );
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(10));
    assert_eq!(
        baseline
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(7)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        wood_events()
    );
    for used in [card(1), card(6), card(7)] {
        assert!(baseline.state().discard.contains(&used));
    }
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾本身：同一張 Fire Strike 面對 P1 的合法立即 Metal Strike 時，正確取得
    // Overcoming 並從八點加倍為十六點。
    let mut modifier = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    modifier.advance_automatic().unwrap();
    assert_eq!(
        modifier
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        metal_events()
    );
    advance_record_to_next_main_after_turn_draw(&mut modifier, card(10));
    let overcoming_fire = modifier
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert_eq!(
        overcoming_fire,
        vec![
            GameEvent::FormationCommitted {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
                star_substitution: None,
                state: FormationAreaState::FaceUpResolving,
            },
            GameEvent::AttackResolved {
                attacker: p2.clone(),
                target: p1.clone(),
                formation_id: "fire-strike".to_string(),
                used_cards: vec![card(9)],
                point_breakdown: AttackPointBreakdown {
                    base_points: 8,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::Overcoming,
                    damage_transform: DamageTransform::DoubleDamage,
                    final_amount: 16,
                },
                hp_change: HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 100,
                    delta: -16,
                    new_hp: 84,
                    effective_delta: -16,
                },
                shield_change: None,
                card_moves: Vec::new(),
                elemental_context_update: atomic_context!(LastElementalAttackUpdate {
                    player: p2.clone(),
                    attack: LastElementalAttack {
                        element: Element::Fire,
                        resolved_turn: 2,
                    },
                }),
            },
            GameEvent::FormationCardsDiscarded {
                player: p2.clone(),
                formation_id: "fire-strike".to_string(),
                cards: vec![card(9)],
            },
        ]
    );
    for used in [card(1), card(9)] {
        assert!(modifier.state().discard.contains(&used));
    }
    assert_eq!(modifier.replay().unwrap(), modifier.state().clone());
    assert_eq!(modifier.verify_replay().unwrap(), modifier.state().clone());

    // 互動：Metal 記錄確實存在且是立即前一個元素，但 Wood 與 Metal 無關；Wood
    // 仍維持六點正常傷害，而不會錯用任何元素交互規則。
    let mut interaction = GameRecord::start(setup, deck).unwrap();
    interaction.advance_automatic().unwrap();
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p1.clone(),
                formation_id: "metal-strike".to_string(),
                cards: vec![card(1)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        metal_events()
    );
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(10));
    assert_eq!(
        interaction
            .handle(Command::PerformFormation {
                player: p2.clone(),
                formation_id: "wood-strike".to_string(),
                cards: vec![card(7)],
                declared_targets: Vec::new(),
            })
            .unwrap(),
        wood_events()
    );
    assert_eq!(
        interaction.state().last_elemental_attack_by_player.get(&p1),
        Some(&LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        })
    );
    assert_eq!(
        interaction.state().last_elemental_attack_by_player.get(&p2),
        Some(&LastElementalAttack {
            element: Element::Wood,
            resolved_turn: 2,
        })
    );
    for used in [card(1), card(7)] {
        assert!(interaction.state().discard.contains(&used));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}

#[test]
fn elemental_history_non_elemental_matrix_blocks_stale_metal_context() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = two_player_setup_with_hp(100);
    setup
        .card_instances
        .extend([card_instance(21, "metal"), card_instance(26, "metal")]);
    let deck = vec![
        1, 6, 2, 3, // P1：Metal/Weapon 的共同開局
        9, 11, 16, 4, 5, // P2：Fire 或 Weapon
        21, 26, 7, // P1 第一輪 Turn Draw
        8, 10, 12, // P2 Weapon 後 Turn Draw
        13, 14, 15, // P1 第二輪 Turn Draw
    ]
    .into_iter()
    .map(card)
    .collect::<Vec<_>>();

    fn finish_turn(record: &mut GameRecord, player: PlayerId, discard: CardInstanceId) {
        record.advance_automatic().unwrap();
        answer_record_choice(
            record,
            player,
            ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        )
        .unwrap();
        record.advance_automatic().unwrap();
    }

    // 無修飾基準：P1 的 Weapon 並不留下元素脈絡，P2 Fire 保持八點正常傷害。
    let mut baseline = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(1), card(6)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut baseline, p1.clone(), card(7));
    let normal_fire = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        normal_fire.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 8,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 8,
                },
                hp_change: HpChangeDelta { team, old_hp: 100, delta: -8, effective_delta: -8, new_hp: 92, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-strike"
            && cards == &vec![card(9)]
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "fire-strike"
            && discarded_cards == &vec![card(9)]
    ));
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());
    assert_eq!(baseline.verify_replay().unwrap(), baseline.state().clone());

    // 修飾本身：P1 的合法 Metal Strike 是 Fire 的直接前置，因此同一張 Fire
    // 會克制 Metal 並造成十六點。
    let mut immediate = GameRecord::start(setup.clone(), deck.clone()).unwrap();
    immediate.advance_automatic().unwrap();
    immediate
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut immediate, p1.clone(), card(7));
    let overcoming_fire = immediate
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        overcoming_fire.as_slice(),
        [
            GameEvent::FormationCommitted { .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 8,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::Overcoming,
                    damage_transform: DamageTransform::DoubleDamage,
                    final_amount: 16,
                },
                hp_change: HpChangeDelta { team, old_hp: 100, delta: -16, effective_delta: -16, new_hp: 84, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { .. },
        ] if team == &TeamId::new("team:p1")
    ));
    assert_eq!(immediate.replay().unwrap(), immediate.state().clone());
    assert_eq!(
        immediate.verify_replay().unwrap(),
        immediate.state().clone()
    );

    // 互動：Metal 記錄仍留在 state 的 turn 1，但 P1 Weapon（turn 3）已成為 P2
    // Fire 的立即上一個 Formation；Fire 因此不能再使用這個舊的 Metal ground。
    let mut stale = GameRecord::start(setup, deck).unwrap();
    stale.advance_automatic().unwrap();
    stale
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut stale, p1.clone(), card(7));
    stale
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(11), card(16)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut stale, p2.clone(), card(12));
    stale
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(21), card(26)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    finish_turn(&mut stale, p1.clone(), card(15));
    assert_eq!(
        stale.state().last_elemental_attack_by_player.get(&p1),
        Some(&LastElementalAttack {
            element: Element::Metal,
            resolved_turn: 1,
        })
    );
    assert_eq!(
        stale
            .public_view(Viewer::Observer)
            .unwrap()
            .previous_turn_formation,
        Some(PublicPreviousTurnFormation {
            player: p1.clone(),
            formation_id: Some("weapon".to_string()),
            cards: PublicCardRefs::Known(vec![card(21), card(26)]),
        })
    );
    let stale_fire = stale
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "fire-strike".to_string(),
            cards: vec![card(9)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        stale_fire.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                point_breakdown: AttackPointBreakdown {
                    base_points: 8,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 8,
                },
                hp_change: HpChangeDelta { team, old_hp: 88, delta: -8, effective_delta: -8, new_hp: 80, .. },
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "fire-strike"
            && cards == &vec![card(9)]
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "fire-strike"
            && discarded_cards == &vec![card(9)]
    ));
    for used in [card(1), card(11), card(16), card(21), card(26), card(9)] {
        assert!(stale.state().discard.contains(&used));
    }
    assert_eq!(stale.replay().unwrap(), stale.state().clone());
    assert_eq!(stale.verify_replay().unwrap(), stale.state().clone());
}

#[test]
fn shield_change_replaces_existing_player_shield_amount() {
    let setup = two_player_setup();
    let mut state = GameState::from_setup(&setup);

    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p1"),
            old_value: 0,
            delta: 3,
            new_value: 3,
        },
    );
    apply_event(
        &mut state,
        &GameEvent::ShieldChanged {
            player: PlayerId::new("p1"),
            old_value: 3,
            delta: 2,
            new_value: 5,
        },
    );

    assert_eq!(state.shield(&PlayerId::new("p1")), Some(5));
}

#[test]
fn barrier_weapon_matrix_applies_physical_double_shield_damage_without_hp_loss() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let deck = deck_starting_with(&[2, 7, 1, 4, 6, 11, 3, 5, 8]);

    // 基準：P1 普通合法行動後，相同的實體 Weapon 命令沒有護盾吸收其傷害。
    let mut baseline = GameRecord::start(two_player_setup(), deck.clone()).unwrap();
    baseline.advance_automatic().unwrap();
    baseline
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    advance_record_to_next_main_after_turn_draw(&mut baseline, card(9));
    let baseline_weapon = baseline
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(6), card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        baseline_weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                formation_id: attack_formation,
                used_cards,
                point_breakdown: AttackPointBreakdown {
                    base_points: 12,
                    environment_effect: EnvironmentAttackEffect::None,
                    interaction: ElementInteraction::None,
                    damage_transform: DamageTransform::NormalDamage,
                    final_amount: 12,
                },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: -12, new_hp: 18, effective_delta: -12 },
                shield_change: None,
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "weapon"
            && cards == &vec![card(6), card(11)]
            && attacker == &p2
            && target == &p1
            && attack_formation == "weapon"
            && used_cards == &vec![card(6), card(11)]
            && team == &TeamId::new("team:p1")
            && discarded_by == &p2
            && discarded == "weapon"
            && discarded_cards == &vec![card(6), card(11)]
    ));
    assert_eq!(baseline.state().shield(&p1), Some(0));
    assert_eq!(
        baseline
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(18)
    );
    assert_eq!(baseline.replay().unwrap(), baseline.state().clone());

    // 修飾：P1 透過相同的合法行動位置建立 Barrier；四張卡牌的等級總和提供
    // 44 點護盾。
    let mut interaction = GameRecord::start(two_player_setup(), deck).unwrap();
    interaction.advance_automatic().unwrap();
    let barrier = interaction
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "barrier".to_string(),
            cards: vec![card(2), card(7), card(1), card(4)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        barrier.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::ShieldChanged { player: shield_owner, old_value: 0, delta: 44, new_value: 44 },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p1
            && formation_id == "barrier"
            && cards == &vec![card(2), card(7), card(1), card(4)]
            && shield_owner == &p1
            && discarded_by == &p1
            && discarded == "barrier"
            && discarded_cards == &vec![card(2), card(7), card(1), card(4)]
    ));
    assert_eq!(interaction.state().shield(&p1), Some(44));
    assert_eq!(interaction.state().phase, Phase::TurnDraw);
    assert_eq!(interaction.state().current_player(), Some(&p1));
    assert_eq!(interaction.state().hand(&p1), Some([].as_slice()));
    assert_eq!(
        interaction.state().discard,
        vec![card(2), card(7), card(1), card(4)]
    );
    let expected_public_shields = vec![
        PlayerShield {
            player: p1.clone(),
            value: 44,
        },
        PlayerShield {
            player: p2.clone(),
            value: 0,
        },
    ];
    for viewer in [
        Viewer::Player(p1.clone()),
        Viewer::Player(p2.clone()),
        Viewer::Observer,
    ] {
        let view = interaction.public_view(viewer).unwrap();
        assert_eq!(view.phase, Phase::TurnDraw);
        assert_eq!(view.current_player, Some(p1.clone()));
        assert_eq!(view.shields, expected_public_shields);
    }
    let barrier_state = interaction.state().clone();
    assert_eq!(interaction.replay().unwrap(), barrier_state);
    assert_eq!(interaction.verify_replay().unwrap(), barrier_state);
    advance_record_to_next_main_after_turn_draw(&mut interaction, card(9));

    // 互動：實體傷害只會對護盾加倍。Weapon 陣形仍正常提交/棄置，並在將 44 降至
    // 20 的同時讓生命值保持不變。
    let weapon = interaction
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "weapon".to_string(),
            cards: vec![card(6), card(11)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        weapon.as_slice(),
        [
            GameEvent::FormationCommitted { player, formation_id, cards, state: FormationAreaState::FaceUpResolving, .. },
            GameEvent::AttackResolved {
                attacker,
                target,
                formation_id: attack_formation,
                used_cards,
                point_breakdown: AttackPointBreakdown { base_points: 12, final_amount: 12, .. },
                hp_change: HpChangeDelta { team, old_hp: 30, delta: 0, new_hp: 30, effective_delta: 0 },
                shield_change: Some(ShieldChangeDelta { player: shield_owner, old_value: 44, delta: -24, new_value: 20 }),
                ..
            },
            GameEvent::FormationCardsDiscarded { player: discarded_by, formation_id: discarded, cards: discarded_cards },
        ] if player == &p2
            && formation_id == "weapon"
            && cards == &vec![card(6), card(11)]
            && attacker == &p2
            && target == &p1
            && attack_formation == "weapon"
            && used_cards == &vec![card(6), card(11)]
            && team == &TeamId::new("team:p1")
            && shield_owner == &p1
            && discarded_by == &p2
            && discarded == "weapon"
            && discarded_cards == &vec![card(6), card(11)]
    ));
    assert_eq!(interaction.state().shield(&p1), Some(20));
    assert_eq!(
        interaction
            .state()
            .hp
            .iter()
            .find(|entry| entry.team == TeamId::new("team:p1"))
            .map(|entry| entry.hp),
        Some(30)
    );
    for card in [card(2), card(7), card(1), card(4), card(6), card(11)] {
        assert!(interaction.state().discard.contains(&card));
    }
    assert_eq!(interaction.replay().unwrap(), interaction.state().clone());
    assert_eq!(
        interaction.verify_replay().unwrap(),
        interaction.state().clone()
    );
}
