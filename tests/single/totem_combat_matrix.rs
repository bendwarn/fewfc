use fewfc::application::{advance_automatic, apply_event, handle_command};
use fewfc::domain::*;
use fewfc::rules::OfficialRules;

fn player(index: usize) -> PlayerId {
    PlayerId::new(format!("p{}", index + 1))
}

struct Combat {
    baseline: GameState,
    state: GameState,
    events: Vec<GameEvent>,
}
impl Combat {
    fn new(hands: [Vec<(Element, u32)>; 2]) -> Self {
        let base = GameSetup::two_player(player(0), player(1), 200);
        let setup = OfficialRules::new()
            .configure_versioned_game_with_decks(
                RuleVersion::V5_17,
                base.players,
                base.turn_order,
                [
                    STAR_MODULE_ID,
                    FIVE_DIRECTIONS_LEGEND_MODULE_ID,
                    HERO_SCHOOLS_MODULE_ID,
                    TOTEM_FORMATION_MODULE_ID,
                ]
                .into_iter()
                .map(RuleModuleId::new)
                .collect(),
                vec![],
            )
            .unwrap();
        let mut state = GameState::from_setup(&setup);
        state.phase = Phase::ActiveEffects;
        state.hand_limit = 0;
        let mut used = vec![];
        for (index, hand) in hands.into_iter().enumerate() {
            let cards = hand
                .into_iter()
                .map(|(element, level)| {
                    let card = setup
                        .card_instances
                        .iter()
                        .find(|instance| {
                            !used.contains(&instance.instance)
                                && setup.card_defs.iter().any(|definition| {
                                    definition.id == instance.definition
                                        && definition.element == element
                                        && definition.level.value() == level
                                })
                        })
                        .unwrap()
                        .instance;
                    used.push(card);
                    card
                })
                .collect();
            state.hands[index].cards = cards;
        }
        state.deck.retain(|card| !used.contains(card));
        Self {
            baseline: state.clone(),
            state,
            events: vec![],
        }
    }
    fn apply(&mut self, events: Vec<GameEvent>) -> Vec<GameEvent> {
        for event in &events {
            apply_event(&mut self.state, event);
        }
        self.events.extend(events.clone());
        events
    }
    fn perform(&mut self, index: usize, id: &str, requested: &[(Element, u32)]) -> Vec<GameEvent> {
        let mut cards: Vec<CardInstanceId> = vec![];
        for (element, level) in requested {
            let card = *self
                .state
                .hand(&player(index))
                .unwrap()
                .iter()
                .find(|card| {
                    !cards.contains(card)
                        && self.state.card_def(**card).is_some_and(|definition| {
                            definition.element == *element && definition.level.value() == *level
                        })
                })
                .unwrap();
            cards.push(card);
        }
        let events = handle_command(
            &self.state,
            Command::PerformFormation {
                player: player(index),
                formation_id: id.into(),
                cards,
                declared_targets: vec![],
            },
        )
        .unwrap();
        self.apply(events)
    }
    fn choose(&mut self, element: Element) -> Vec<GameEvent> {
        let pending = self.state.pending_choice.as_ref().unwrap();
        self.apply(
            handle_command(
                &self.state,
                Command::AnswerChoice {
                    player: pending.player.clone(),
                    choice_id: pending.choice_id,
                    answer: ChoiceAnswer::Environment {
                        environment: element,
                    },
                },
            )
            .unwrap(),
        )
    }
    fn next(&mut self, index: usize) {
        for _ in 0..12 {
            if self.state.current_player() == Some(&player(index))
                && self.state.phase == Phase::ActiveEffects
            {
                return;
            }
            let events = advance_automatic(&self.state).unwrap();
            assert!(
                !events.is_empty(),
                "turn cannot advance: {:?}",
                self.state.phase
            );
            self.apply(events);
        }
        panic!("turn did not reach the expected player");
    }
    fn hp(&self, index: usize) -> i32 {
        self.state.hp[index].hp
    }
    fn verify(&self) {
        let mut replayed = self.baseline.clone();
        for event in &self.events {
            apply_event(&mut replayed, event);
        }
        assert_eq!(replayed, self.state);
    }
}
fn array(element: Element) -> Vec<(Element, u32)> {
    [1, 2, 5]
        .into_iter()
        .map(|level| (element, level))
        .collect()
}
fn with(mut cards: Vec<(Element, u32)>, rest: &[(Element, u32)]) -> Vec<(Element, u32)> {
    cards.extend_from_slice(rest);
    cards
}
fn attack(events: &[GameEvent]) -> &GameEvent {
    events
        .iter()
        .find(|event| matches!(event, GameEvent::AttackResolved { .. }))
        .unwrap()
}
fn consumed(events: &[GameEvent]) -> usize {
    events
        .iter()
        .map(|event| match event {
            GameEvent::TotemChanged {
                reason: TotemChangeReason::EnvironmentRecoveryPrevented,
                ..
            } => 1,
            GameEvent::AttackResolved {
                elemental_context_update: Some(effects),
                ..
            } => effects
                .totem_changes
                .iter()
                .filter(|change| change.reason == TotemChangeReason::EnvironmentRecoveryPrevented)
                .count(),
            _ => 0,
        })
        .sum()
}

#[test]
fn defender_totem_prevents_environment_doubling_for_hp_but_not_shield() {
    for (element, id, shield) in [
        (Element::Metal, "west-spirit-array", false),
        (Element::Wood, "east-spirit-array", true),
    ] {
        let mut game = Combat::new([array(element), vec![(element, 4)]]);
        game.perform(0, id, &array(element));
        game.next(1);
        let hp = game.hp(0);
        let events = game.perform(
            1,
            if shield {
                "wood-strike"
            } else {
                "metal-strike"
            },
            &[(element, 4)],
        );
        if shield {
            assert_eq!(game.state.shield(&player(0)), Some(0));
            assert_eq!(game.hp(0), hp);
        } else {
            assert_eq!(game.hp(0), hp - 8);
        }
        assert_eq!(consumed(&events), 0);
        assert_eq!(game.state.totems.len(), 1);
        game.verify();
    }
}

#[test]
fn attacker_totem_prevents_environment_recovery_and_is_consumed_once() {
    let mut game = Combat::new([
        with(array(Element::Wood), &[(Element::Wood, 4)]),
        array(Element::Fire),
    ]);
    game.perform(0, "east-spirit-array", &array(Element::Wood));
    game.next(1);
    game.perform(1, "south-spirit-array", &array(Element::Fire));
    game.choose(Element::Metal);
    game.next(0);
    let hp = game.hp(1);
    let events = game.perform(0, "wood-strike", &[(Element::Wood, 4)]);
    assert_eq!(game.hp(1), hp - 8);
    assert_eq!(consumed(&events), 1);
    assert!(
        !game
            .state
            .totems
            .iter()
            .any(|totem| totem.player == player(0))
    );
    game.verify();
}

#[test]
fn sacred_beast_ignores_defender_doubling_exception() {
    let beast = (1..=5)
        .map(|level| (Element::Fire, level))
        .collect::<Vec<_>>();
    let mut game = Combat::new([array(Element::Fire), beast.clone()]);
    game.perform(0, "south-spirit-array", &array(Element::Fire));
    game.choose(Element::Water);
    game.next(1);
    let hp = game.hp(0);
    let events = game.perform(1, "south-vermilion-bird", &beast);
    assert_eq!(game.hp(0), hp - 162);
    assert_eq!(consumed(&events), 0);
    game.verify();
}

#[test]
fn sacred_beast_keeps_environment_recovery_and_does_not_consume_attacker_totem() {
    let beast = (1..=5)
        .map(|level| (Element::Wood, level))
        .collect::<Vec<_>>();
    let mut game = Combat::new([with(array(Element::Wood), &beast), array(Element::Fire)]);
    game.perform(0, "east-spirit-array", &array(Element::Wood));
    game.next(1);
    game.perform(1, "south-spirit-array", &array(Element::Fire));
    game.choose(Element::Metal);
    game.next(0);
    let events = game.perform(0, "east-azure-dragon", &beast);
    assert!(
        matches!(attack(&events),GameEvent::AttackResolved {point_breakdown,..} if point_breakdown.damage_transform == DamageTransform::HealTarget)
    );
    assert_eq!(consumed(&events), 0);
    assert!(
        game.state
            .totems
            .iter()
            .any(|totem| totem.player == player(0) && totem.totem == TotemKind::AzureHorn)
    );
    game.verify();
}

#[test]
fn countershock_applies_each_recipients_own_environment_exception() {
    let mut game = Combat::new([
        with(
            array(Element::Fire),
            &[(Element::Fire, 3), (Element::Fire, 4)],
        ),
        vec![(Element::Metal, 1), (Element::Fire, 3)],
    ]);
    game.perform(0, "south-spirit-array", &array(Element::Fire));
    game.choose(Element::Earth);
    game.next(1);
    game.perform(1, "metal-strike", &[(Element::Metal, 1)]);
    game.next(0);
    game.perform(0, "countershock", &[(Element::Fire, 3), (Element::Fire, 4)]);
    game.next(1);
    let hp = [game.hp(0), game.hp(1)];
    game.perform(1, "fire-strike", &[(Element::Fire, 3)]);
    assert_eq!(game.hp(0), hp[0] - 4);
    assert_eq!(game.hp(1), hp[1] - 8);
    game.verify();
}

#[test]
fn countershock_consumes_attacker_totem_once_and_keeps_both_shares_as_damage() {
    let mut game = Combat::new([
        with(
            array(Element::Wood),
            &[(Element::Metal, 1), (Element::Wood, 4)],
        ),
        with(
            array(Element::Fire),
            &[(Element::Fire, 3), (Element::Fire, 4)],
        ),
    ]);
    game.perform(0, "east-spirit-array", &array(Element::Wood));
    game.next(1);
    game.perform(1, "south-spirit-array", &array(Element::Fire));
    game.choose(Element::Metal);
    game.next(0);
    game.perform(0, "metal-strike", &[(Element::Metal, 1)]);
    game.next(1);
    game.perform(1, "countershock", &[(Element::Fire, 3), (Element::Fire, 4)]);
    game.next(0);
    let hp = [game.hp(0), game.hp(1)];
    let events = game.perform(0, "wood-strike", &[(Element::Wood, 4)]);
    assert_eq!(game.hp(0), hp[0] - 4);
    assert_eq!(game.hp(1), hp[1] - 4);
    assert_eq!(consumed(&events), 1);
    game.verify();
}

#[test]
fn totem_cancels_doubling_before_yang_aura_rounds_odd_damage_up() {
    let mut game = Combat::new([array(Element::Fire), vec![(Element::Fire, 3)]]);
    // 陽罡是既有減傷條件；待測圖騰仍由南靈陣正式取得。
    game.state
        .enabled_rule_modules
        .push(RuleModuleId::new(JIANGHU_MODULE_ID));
    game.state.statuses.push(StatusEffect {
        id: "existing-yang-aura".into(),
        owner: StatusOwner::Player(player(0)),
        kind: "JianghuYangAura".into(),
        value: None,
        duration: StatusDuration::Permanent,
    });
    game.baseline = game.state.clone();
    game.perform(0, "south-spirit-array", &array(Element::Fire));
    game.choose(Element::Water);
    game.next(1);
    let hp = game.hp(0);
    game.perform(1, "fire-strike", &[(Element::Fire, 3)]);
    assert_eq!(
        game.hp(0),
        hp - 4,
        "seven points skip doubling then halve with rounding up"
    );
    game.verify();
}

#[test]
fn countershock_without_totems_preserves_legacy_odd_damage_distribution() {
    let mut game = Combat::new([
        vec![(Element::Fire, 1), (Element::Fire, 2)],
        vec![(Element::Fire, 3)],
    ]);
    game.state.rule_version = RuleVersion::V5_16;
    game.state
        .enabled_rule_modules
        .retain(|module| module.as_str() != TOTEM_FORMATION_MODULE_ID);
    game.state.environment = Some(Element::Fire);
    game.baseline = game.state.clone();
    game.perform(0, "countershock", &[(Element::Fire, 1), (Element::Fire, 2)]);
    game.next(1);
    let hp = [game.hp(0), game.hp(1)];
    game.perform(1, "fire-strike", &[(Element::Fire, 3)]);
    assert_eq!(game.hp(0), hp[0] - 7);
    assert_eq!(game.hp(1), hp[1] - 7);
    game.verify();
}

#[test]
fn shield_and_defense_prevent_totem_consumption_when_recovery_never_applies() {
    for shield in [false, true] {
        let protection = if shield {
            vec![
                (Element::Wood, 3),
                (Element::Wood, 4),
                (Element::Metal, 2),
                (Element::Fire, 3),
            ]
        } else {
            vec![(Element::Wood, 3), (Element::Wood, 4)]
        };
        let mut game = Combat::new([
            with(
                array(Element::Wood),
                &[(Element::Metal, 1), (Element::Wood, 4)],
            ),
            with(array(Element::Fire), &protection),
        ]);
        game.perform(0, "east-spirit-array", &array(Element::Wood));
        game.next(1);
        game.perform(1, "south-spirit-array", &array(Element::Fire));
        game.choose(Element::Metal);
        game.next(0);
        game.perform(0, "metal-strike", &[(Element::Metal, 1)]);
        game.next(1);
        game.perform(1, if shield { "barrier" } else { "defense" }, &protection);
        game.next(0);
        let hp = game.hp(1);
        let events = game.perform(0, "wood-strike", &[(Element::Wood, 4)]);
        assert_eq!(game.hp(1), hp);
        assert_eq!(consumed(&events), 0);
        assert!(
            game.state
                .totems
                .iter()
                .any(|totem| totem.player == player(0) && totem.totem == TotemKind::AzureHorn)
        );
        game.verify();
    }
}

#[test]
fn independent_generation_still_heals_after_totem_consumes_environment_exception() {
    let mut game = Combat::new([
        with(array(Element::Wood), &[(Element::Wood, 4)]),
        array(Element::Fire),
    ]);
    game.perform(0, "east-spirit-array", &array(Element::Wood));
    game.next(1);
    game.perform(1, "south-spirit-array", &array(Element::Fire));
    game.choose(Element::Fire);
    game.next(0);
    let events = game.perform(0, "wood-strike", &[(Element::Wood, 4)]);
    assert_eq!(consumed(&events), 1);
    assert!(
        matches!(attack(&events),GameEvent::AttackResolved {point_breakdown,..} if point_breakdown.damage_transform==DamageTransform::HealTarget)
    );
    assert!(
        !game
            .state
            .totems
            .iter()
            .any(|totem| totem.player == player(0))
    );
    game.verify();
}

#[test]
fn south_seal_suppresses_all_effects_but_defense_only_prevents_damage() {
    for sealed in [false, true] {
        let element = if sealed {
            Element::Water
        } else {
            Element::Wood
        };
        let passive = vec![(element, 1), (element, 2)];
        let mut game = Combat::new([passive.clone(), array(Element::Fire)]);
        game.perform(0, if sealed { "seal" } else { "defense" }, &passive);
        game.next(1);
        let hp = game.hp(0);
        game.perform(1, "south-spirit-array", &array(Element::Fire));
        if sealed {
            assert!(game.state.pending_choice.is_none());
            assert!(game.state.totems.is_empty());
            assert_eq!(game.state.environment, None);
        } else {
            game.choose(Element::Fire);
            assert_eq!(game.state.environment, Some(Element::Fire));
            assert_eq!(
                game.state.totems,
                vec![PlayerTotem {
                    player: player(1),
                    totem: TotemKind::VermilionFeather
                }]
            );
        }
        assert_eq!(game.hp(0), hp);
        game.verify();
    }
}

#[test]
fn south_consumes_old_azure_before_granting_vermilion_in_fire_environment() {
    let mut game = Combat::new([
        with(array(Element::Wood), &array(Element::Fire)),
        array(Element::Fire),
    ]);
    game.perform(0, "east-spirit-array", &array(Element::Wood));
    game.next(1);
    game.perform(1, "south-spirit-array", &array(Element::Fire));
    game.choose(Element::Metal);
    game.next(0);
    game.perform(0, "south-spirit-array", &array(Element::Fire));
    let hp = game.hp(1);
    let events = game.choose(Element::Wood);
    assert_eq!(game.hp(1), hp - 20);
    assert_eq!(consumed(&events), 1);
    assert_eq!(game.state.environment, Some(Element::Fire));
    assert!(
        game.state
            .totems
            .iter()
            .any(|totem| totem.player == player(0) && totem.totem == TotemKind::VermilionFeather)
    );
    let GameEvent::AttackResolved {
        elemental_context_update: Some(effects),
        ..
    } = attack(&events)
    else {
        panic!("south attack effects")
    };
    let changes = effects
        .totem_changes
        .iter()
        .filter(|change| change.player == player(0))
        .collect::<Vec<_>>();
    assert_eq!(changes.len(), 2);
    assert_eq!(changes[0].previous, Some(TotemKind::AzureHorn));
    assert_eq!(changes[0].totem, None);
    assert_eq!(changes[1].totem, Some(TotemKind::VermilionFeather));
    game.verify();
}
