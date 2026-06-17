use fewfc::application::GameRecord;
use fewfc::domain::{
    CardDef, CardDefId, CardInstanceDef, CardInstanceId, Command, EventSource, GameError,
    GameSetup, PendingChoice, PendingChoiceKind, PlayerId, RulesetId, ValidationError,
};
use fewfc::infrastructure::{
    InMemoryPersistence, PersistedGameRecord, PersistedSnapshot, PersistenceMetadata,
};
use fewfc::ports::{EventLogStorage, SnapshotStorage};
use fewfc::rules::Element;

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

fn card_def(id: &str, element: Element) -> CardDef {
    CardDef {
        id: CardDefId::new(id),
        name: id.to_string(),
        element,
        level: match id {
            "metal" => 3,
            "wood" => 2,
            "water" => 1,
            "fire" => 4,
            "earth" => 5,
            _ => 1,
        },
    }
}

fn card_instance(instance: u64, def_id: &str) -> CardInstanceDef {
    CardInstanceDef {
        instance: card(instance),
        definition: CardDefId::new(def_id),
    }
}

fn two_player_setup() -> GameSetup {
    GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).with_cards(
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

#[test]
fn persisted_event_log_round_trip_replays_mid_turn_effect_choice() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[5, 10, 1, 2])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metamorphosis".to_string(),
            cards: vec![card(5), card(10)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "variant-from-caller".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );

    assert_eq!(persisted.metadata.ruleset_id, "base");
    assert_eq!(persisted.setup.ruleset, RulesetId::base());
    assert_eq!(
        persisted.recorded_events.len(),
        record.recorded_events().len()
    );
    assert_eq!(persisted.replay().unwrap(), record.state().unwrap());
    assert_eq!(
        persisted.replay().unwrap().pending_choice,
        Some(PendingChoice {
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::EffectGenerated {
                effect_id: "metamorphosis".to_string(),
                continuation_id: "metamorphosis:choose-card".to_string(),
                allowed_cards: vec![card(1), card(2)],
            },
        })
    );
}

#[test]
fn persisted_event_log_round_trip_replays_turn_draw_discard_choice() {
    let mut record = GameRecord::start(two_player_setup(), (1..=20).map(card).collect()).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    record.advance_automatic().unwrap();

    let persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "base".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );

    assert_eq!(persisted.replay().unwrap(), record.state().unwrap());
    assert_eq!(
        persisted.replay().unwrap().pending_choice,
        Some(PendingChoice {
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::TurnDrawDiscard {
                drawn_cards: vec![card(10), card(11), card(12)],
                allowed_discards: vec![card(10), card(11), card(12)],
            },
        })
    );
}

#[test]
fn persisted_replay_uses_event_order_and_payloads_not_recorded_metadata() {
    let mut record = GameRecord::start(two_player_setup(), deck_starting_with(&[1])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let expected_state = record.state().unwrap();
    let mut persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "base".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );

    for (index, recorded) in persisted.recorded_events.iter_mut().enumerate() {
        recorded.metadata.sequence = 10_000 - index as u64;
        recorded.metadata.source = EventSource::Setup;
    }

    assert_eq!(persisted.replay().unwrap(), expected_state);
}

#[test]
fn persisted_record_replay_rejects_ruleset_identity_mismatch() {
    let record = GameRecord::start(two_player_setup(), deck_starting_with(&[1])).unwrap();
    let mut persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "base".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );
    persisted.metadata.ruleset_id = "variant".to_string();

    assert_eq!(
        persisted.replay(),
        Err(GameError::Validation(ValidationError::RulesetMismatch {
            setup: RulesetId::base(),
            metadata: RulesetId::new("variant"),
        }))
    );
}

#[test]
fn persistence_ports_round_trip_event_log_and_optional_snapshot_checkpoint() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[2, 7, 1, 3])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "defense".to_string(),
            cards: vec![card(2), card(7)],
            declared_targets: Vec::new(),
        })
        .unwrap();
    let persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "base".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );
    let snapshot = PersistedSnapshot::from_state(
        persisted.recorded_events.len() as u64,
        record.state().unwrap(),
    );

    let mut storage = InMemoryPersistence::default();
    storage.save_event_log("game-1", &persisted).unwrap();
    storage.save_snapshot("game-1", &snapshot).unwrap();

    let loaded_log = storage.load_event_log("game-1").unwrap().unwrap();
    let loaded_snapshot = storage.load_snapshot("game-1").unwrap().unwrap();

    assert_eq!(loaded_log.replay().unwrap(), record.state().unwrap());
    assert_eq!(loaded_snapshot.state, record.state().unwrap());
    assert_eq!(
        loaded_snapshot.state.covered_passives[0].cards,
        vec![card(2), card(7)]
    );
}
