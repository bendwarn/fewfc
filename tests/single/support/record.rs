#![allow(dead_code)]

use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, ChoiceAnswer, Command, Element, GameEvent, GameSetup, PendingChoiceKind,
    PendingResolution, Player, PlayerId, ProfessionId, RuleModuleId, TeamId,
};
use fewfc::rules::OfficialRules;

pub fn setup(player_count: usize, modules: &[&str]) -> GameSetup {
    let players = (1..=player_count)
        .map(|seat| Player {
            id: PlayerId::new(format!("p{seat}")),
            team: TeamId::new(if seat % 2 == 1 { "team:a" } else { "team:b" }),
        })
        .collect::<Vec<_>>();
    OfficialRules::new()
        .configure_game(
            players.clone(),
            players.into_iter().map(|player| player.id).collect(),
            modules.iter().copied().map(RuleModuleId::new).collect(),
        )
        .unwrap()
}

pub fn ids(cards: &[u64]) -> Vec<CardInstanceId> {
    cards.iter().copied().map(CardInstanceId::new).collect()
}

pub fn start(setup: GameSetup, prefix: &[u64]) -> GameRecord {
    start_with_cards(setup, &ids(prefix))
}

pub fn start_with_cards(setup: GameSetup, prefix: &[CardInstanceId]) -> GameRecord {
    let deck = OfficialRules::new().official_deck_order(&setup).unwrap();
    let ordered = prefix
        .iter()
        .copied()
        .chain(deck.into_iter().filter(|card| !prefix.contains(card)))
        .collect();
    let mut record = GameRecord::start(setup, ordered).unwrap();
    record.advance_automatic().unwrap();
    record
}

/// 為需要多回合前置條件的情境安排開局與首輪抽牌。額外的抽牌空間屬於開局設定，
/// 陣形、職業及任何有效果的狀態都由實際命令建立。首輪抽牌一律棄掉填充牌。
pub fn start_with_reserves(
    mut setup: GameSetup,
    opening: &[&[u64]],
    reserves: &[&[u64]],
) -> GameRecord {
    assert_eq!(opening.len(), setup.players.len());
    assert_eq!(reserves.len(), setup.players.len());
    let base_draw = reserves
        .iter()
        .map(|cards| cards.len())
        .max()
        .unwrap()
        .max(2);
    setup.base_draw = base_draw;
    setup.hand_limit = base_draw + 5;

    let fixed = opening
        .iter()
        .chain(reserves)
        .flat_map(|cards| ids(cards))
        .collect::<Vec<_>>();
    let mut filler = OfficialRules::new()
        .official_deck_order(&setup)
        .unwrap()
        .into_iter()
        .filter(|card| !fixed.contains(card));
    let mut prefix = Vec::new();
    for (seat, cards) in opening.iter().enumerate() {
        let size = if seat == 0 { 4 } else { 5 };
        assert!(cards.len() <= size);
        prefix.extend(ids(cards));
        prefix.extend(filler.by_ref().take(size - cards.len()));
    }
    for cards in reserves {
        prefix.push(filler.next().unwrap());
        prefix.extend(ids(cards));
        prefix.extend(filler.by_ref().take(base_draw - cards.len()));
    }
    start_with_cards(setup, &prefix)
}

pub fn perform(
    record: &mut GameRecord,
    player: &PlayerId,
    formation_id: &str,
    cards: &[u64],
) -> Vec<GameEvent> {
    record
        .handle(Command::PerformFormation {
            player: player.clone(),
            formation_id: formation_id.to_string(),
            cards: ids(cards),
            declared_targets: Vec::new(),
        })
        .unwrap_or_else(|error| panic!("{player:?} {formation_id} {cards:?}: {error:?}"))
}

pub fn change(
    record: &mut GameRecord,
    player: &PlayerId,
    profession: &str,
    cards: &[u64],
) -> Vec<GameEvent> {
    record
        .handle(Command::ChangeProfession {
            player: player.clone(),
            profession: ProfessionId::new(profession),
            cards: ids(cards),
        })
        .unwrap_or_else(|error| panic!("{player:?} {profession} {cards:?}: {error:?}"))
}

pub fn answer(record: &mut GameRecord, player: &PlayerId, answer: ChoiceAnswer) -> Vec<GameEvent> {
    let choice_id = record.state().pending_choice.as_ref().unwrap().choice_id;
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id,
            answer,
        })
        .unwrap()
}

pub fn finish(record: &mut GameRecord, player: &PlayerId) -> Vec<GameEvent> {
    finish_discarding(record, player, None)
}

pub fn finish_discarding(
    record: &mut GameRecord,
    player: &PlayerId,
    discard: Option<CardInstanceId>,
) -> Vec<GameEvent> {
    let mut events = record.advance_automatic().unwrap();
    if let Some(choice) = record.state().pending_choice.as_ref() {
        assert_eq!(&choice.player, player);
        assert_eq!(
            record.state().pending_resolution,
            Some(PendingResolution::TurnDrawDiscard)
        );
        let PendingChoiceKind::Card { cards, .. } = &choice.kind else {
            panic!("回合抽牌應要求選擇棄牌");
        };
        let card = discard.unwrap_or(cards[0]);
        events.extend(answer(
            record,
            player,
            ChoiceAnswer::Cards { cards: vec![card] },
        ));
        events.extend(record.advance_automatic().unwrap());
    } else {
        assert!(discard.is_none(), "本回合沒有棄牌選擇");
    }
    events
}

pub fn strike_with_spare(
    record: &mut GameRecord,
    player: &PlayerId,
    reserved: &[u64],
) -> Vec<GameEvent> {
    let reserved = ids(reserved);
    let state = record.state();
    let card = state
        .hand(player)
        .unwrap()
        .iter()
        .copied()
        .filter(|card| !reserved.contains(card))
        .min_by_key(|card| state.card_def(*card).unwrap().level.value())
        .expect("開局需要保留一張可打出的額外卡牌");
    let formation = match state.card_def(card).unwrap().element {
        Element::Metal => "metal-strike",
        Element::Wood => "wood-strike",
        Element::Water => "water-strike",
        Element::Fire => "fire-strike",
        Element::Earth => "earth-strike",
    };
    record
        .handle(Command::PerformFormation {
            player: player.clone(),
            formation_id: formation.to_string(),
            cards: vec![card],
            declared_targets: Vec::new(),
        })
        .unwrap()
}

pub fn assert_replay(record: &GameRecord) {
    assert_eq!(record.replay().unwrap(), *record.state());
    assert_eq!(record.verify_replay().unwrap(), *record.state());
}

pub fn personal_prefix(setup: &GameSetup, hands: &[&[(Element, u32)]]) -> Vec<CardInstanceId> {
    use fewfc::domain::CardOrigin;

    let mut available = setup.card_instances.iter().collect::<Vec<_>>();
    let mut ordered = Vec::new();
    for (player, cards) in setup.turn_order.iter().zip(hands) {
        for &(element, level) in *cards {
            let index = available
                .iter()
                .position(|instance| {
                    instance.origin == CardOrigin::Player(player.clone())
                        && setup.card_defs.iter().any(|def| {
                            def.id == instance.definition
                                && def.element == element
                                && def.level.value() == level
                        })
                })
                .expect("預組牌堆中必須有對應的牌");
            ordered.push(available.remove(index).instance);
        }
    }
    ordered
}

pub fn start_pouch(
    setup: GameSetup,
    pouches: &[CardInstanceId],
    prefix: &[CardInstanceId],
) -> GameRecord {
    use fewfc::domain::{RandomnessOperation, TrustedRandomnessAnswer};

    let players = setup.turn_order.clone();
    assert_eq!(players.len(), pouches.len());
    let mut record = GameRecord::start(setup, Vec::new()).unwrap();
    for (player, card) in players.into_iter().zip(pouches.iter().copied()) {
        record
            .handle(Command::ChooseInitialPouch { player, card })
            .unwrap();
    }
    while let Some(request) = record.state().pending_randomness.clone() {
        assert!(matches!(
            request.operation,
            RandomnessOperation::DeckShuffle { .. }
        ));
        let order = prefix
            .iter()
            .copied()
            .filter(|card| request.current_order.contains(card))
            .chain(
                request
                    .current_order
                    .iter()
                    .copied()
                    .filter(|card| !prefix.contains(card)),
            )
            .collect();
        record
            .resolve_randomness(TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order: order,
            })
            .unwrap();
    }
    record.advance_automatic().unwrap();
    record
}
