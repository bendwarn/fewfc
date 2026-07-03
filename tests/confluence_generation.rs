use fewfc::application::{apply_event, handle_command};
use fewfc::domain::{
    CONFLUENCE_GENERATION_MODULE_ID, CardInstanceId, Command, DeckPlacement, Element,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent, GameState, HERO_SCHOOLS_MODULE_ID,
    LastTurnDiscard, LimitedUse, Phase, Player, PlayerId, PlayerProfession, ProfessionId,
    RuleModuleId, STAR_MODULE_ID, TeamId,
};
use fewfc::public_view::{Viewer, state_for};
use fewfc::rules::{OfficialRules, PlayableAction};

fn state(personal_deck: bool) -> GameState {
    let players = vec![
        Player {
            id: PlayerId::new("p1"),
            team: TeamId::new("team:a"),
        },
        Player {
            id: PlayerId::new("p2"),
            team: TeamId::new("team:b"),
        },
    ];
    let mut modules = vec![
        RuleModuleId::new(STAR_MODULE_ID),
        RuleModuleId::new(FIVE_DIRECTIONS_LEGEND_MODULE_ID),
        RuleModuleId::new(HERO_SCHOOLS_MODULE_ID),
        RuleModuleId::new(CONFLUENCE_GENERATION_MODULE_ID),
    ];
    if personal_deck {
        modules.push(RuleModuleId::new("personal-deck"));
    }
    let setup = OfficialRules::new()
        .configure_game(
            players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            modules,
        )
        .unwrap();
    let mut state = GameState::from_setup(&setup);
    state.current_turn_index = 1;
    state.turn_number = 2;
    state.phase = Phase::Main;
    state
}

fn cards(state: &GameState, requested: &[(Element, u32)]) -> Vec<CardInstanceId> {
    let mut used = Vec::new();
    requested
        .iter()
        .map(|(element, level)| {
            let card = state
                .card_instances
                .iter()
                .map(|instance| instance.instance)
                .find(|instance| {
                    !used.contains(instance)
                        && state.card_def(*instance).is_some_and(|definition| {
                            definition.element == *element && definition.level == *level
                        })
                })
                .unwrap();
            used.push(card);
            card
        })
        .collect()
}

fn set_hand(state: &mut GameState, cards: Vec<CardInstanceId>) {
    state
        .hands
        .iter_mut()
        .find(|hand| hand.player == PlayerId::new("p2"))
        .unwrap()
        .cards = cards;
}

fn set_residual(state: &mut GameState, card: CardInstanceId) {
    state
        .discard_for_mut(&PlayerId::new("p1"))
        .unwrap()
        .push(card);
    state.last_turn_discard_by_player.insert(
        PlayerId::new("p1"),
        LastTurnDiscard {
            card,
            turn_number: 1,
        },
    );
}

#[test]
fn residual_element_and_level_require_the_retrievable_printed_discard() {
    let mut game = state(false);
    let residual = cards(&game, &[(Element::Metal, 2)])[0];
    set_residual(&mut game, residual);
    let transition = cards(&game, &[(Element::Metal, 1), (Element::Metal, 2)]);
    set_hand(&mut game, transition.clone());

    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &transition)
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ChangeProfession(candidate)
            if candidate.profession_id == ProfessionId::new("confluence:tuner")
    )));

    game.discard_for_mut(&PlayerId::new("p1")).unwrap().clear();
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &transition)
        .unwrap();
    assert!(!actions.iter().any(|action| matches!(
        action,
        PlayableAction::ChangeProfession(candidate)
            if candidate.profession_id == ProfessionId::new("confluence:tuner")
    )));
}

#[test]
fn acquiring_dao_mage_persists_a_public_limited_use() {
    let mut game = state(false);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("seeker"),
    });
    let transition = cards(&game, &[(Element::Fire, 1), (Element::Fire, 5)]);
    set_hand(&mut game, transition.clone());
    let events = handle_command(
        &game,
        Command::ChangeProfession {
            player: PlayerId::new("p2"),
            profession: ProfessionId::new("confluence:dao-mage"),
            cards: transition,
        },
    )
    .unwrap();
    assert!(events.iter().any(|event| matches!(
        event,
        GameEvent::LimitedUseChanged {
            key,
            new_remaining: 1,
            maximum: 1,
            ..
        } if key == "confluence:imprisoning-array"
    )));
    for event in &events {
        apply_event(&mut game, event);
    }
    assert_eq!(state_for(&game, Viewer::Observer).limited_uses.len(), 1);
}

#[test]
fn tuning_is_offered_only_when_the_retrieved_card_can_complete_a_profession_change() {
    let mut game = state(false);
    game.professions.push(PlayerProfession {
        player: PlayerId::new("p2"),
        profession: ProfessionId::new("confluence:tuner"),
    });
    let residual = cards(&game, &[(Element::Metal, 2)])[0];
    set_residual(&mut game, residual);
    let hand = cards(&game, &[(Element::Fire, 3), (Element::Metal, 4)]);
    set_hand(&mut game, hand.clone());
    let actions = OfficialRules::new()
        .playable_actions(&game, &PlayerId::new("p2"), &[hand[0]])
        .unwrap();
    assert!(actions.iter().any(|action| matches!(
        action,
        PlayableAction::ActivateProfessionAbility(candidate)
            if candidate.ability_id == "confluence:tuning"
    )));
}

#[test]
fn tailwind_recovers_for_all_shared_deck_owners_but_only_personal_pile_owner() {
    let mut shared = state(false);
    shared.limited_uses = vec![limited("p1", 0), limited("p2", 0)];
    let recycled = cards(&shared, &[(Element::Earth, 1)])[0];
    shared.discard.push(recycled);
    apply_event(
        &mut shared,
        &GameEvent::DiscardRecycledIntoDeck {
            shuffled_order: vec![recycled],
            placement: DeckPlacement::Bottom,
        },
    );
    assert!(
        shared
            .limited_uses
            .iter()
            .all(|use_count| use_count.remaining == 1)
    );

    let mut personal = state(true);
    personal.limited_uses = vec![limited("p1", 0), limited("p2", 0)];
    let recycled = personal.player_discards[0]
        .cards
        .first()
        .copied()
        .unwrap_or_else(|| {
            let card = cards(&personal, &[(Element::Earth, 1)])[0];
            personal.player_discards[0].cards.push(card);
            card
        });
    apply_event(
        &mut personal,
        &GameEvent::PlayerDiscardRecycledIntoDeck {
            player: PlayerId::new("p1"),
            shuffled_order: vec![recycled],
            placement: DeckPlacement::Bottom,
        },
    );
    assert_eq!(personal.limited_uses[0].remaining, 1);
    assert_eq!(personal.limited_uses[1].remaining, 0);
}

fn limited(player: &str, remaining: u32) -> LimitedUse {
    LimitedUse {
        owner: PlayerId::new(player),
        key: "confluence:tailwind".to_string(),
        remaining,
        maximum: 1,
    }
}
