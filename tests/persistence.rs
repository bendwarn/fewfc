use fewfc::application::{GameRecord, ReplayVerificationError, verify_recorded_events};
use fewfc::domain::{
    CardDef, CardDefId, CardInstanceDef, CardInstanceId, Command, EventSource, GameError,
    GameEvent, GameSetup, PendingChoice, PendingChoiceKind, PlayerId, RulesetId, ValidationError,
};
use fewfc::infrastructure::{
    FileSystemPersistence, FixedDeckPreparation, InMemoryPersistence, PersistedGameRecord,
    PersistedSnapshot, PersistenceMetadata, SeededDeckPreparation,
};
use fewfc::ports::{DeckPreparation, EventLogStorage, SnapshotStorage};
use fewfc::rules::Element;
use std::fs;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

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

fn temp_persistence_dir(test_name: &str) -> PathBuf {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    std::env::temp_dir().join(format!("fewfc-{test_name}-{}-{nonce}", std::process::id()))
}

#[test]
fn seeded_deck_preparation_produces_repeatable_orders_from_setup() {
    let setup = two_player_setup();
    let mut first = SeededDeckPreparation::new(42);
    let mut second = SeededDeckPreparation::new(42);
    let mut different = SeededDeckPreparation::new(7);

    let first_order = first.prepare_deck(&setup).unwrap();

    assert_eq!(second.prepare_deck(&setup).unwrap(), first_order);
    assert_ne!(different.prepare_deck(&setup).unwrap(), first_order);
    assert_eq!(first_order.len(), setup.card_instances.len());
}

#[test]
fn fixed_deck_preparation_order_is_recorded_in_deck_prepared_event() {
    let setup = two_player_setup();
    let prepared_order = deck_starting_with(&[5, 10, 1, 2]);
    let mut preparation = FixedDeckPreparation::new(prepared_order.clone());
    let prepared_deck = preparation.prepare_deck(&setup).unwrap();
    let record = GameRecord::start(setup, prepared_deck).unwrap();

    assert_eq!(
        record.events().first(),
        Some(&GameEvent::DeckPrepared {
            deck_order: prepared_order,
        })
    );
}

#[test]
fn replay_uses_recorded_deck_order_not_later_deck_preparation() {
    let setup = two_player_setup();
    let mut initial_preparation = SeededDeckPreparation::new(11);
    let mut later_preparation = SeededDeckPreparation::new(99);
    let initial_order = initial_preparation.prepare_deck(&setup).unwrap();
    let later_order = later_preparation.prepare_deck(&setup).unwrap();
    let record = GameRecord::start(setup.clone(), initial_order.clone()).unwrap();

    assert_ne!(initial_order, later_order);
    assert_eq!(record.replay().unwrap(), record.state().unwrap());
    assert_eq!(
        record.events().first(),
        Some(&GameEvent::DeckPrepared {
            deck_order: initial_order,
        })
    );
}

#[test]
fn persisted_event_log_round_trip_replays_mid_turn_effect_choice() {
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
                effect_id: "chaos".to_string(),
                continuation_id: "chaos:return-two".to_string(),
                allowed_cards: vec![card(3), card(4), card(6), card(7), card(8)],
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
fn persisted_event_log_json_round_trip_includes_snapshot_and_replays() {
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
    persisted.latest_snapshot = Some(PersistedSnapshot::from_state(
        persisted.recorded_events.len() as u64,
        expected_state.clone(),
    ));

    let json = persisted.to_json().unwrap();
    assert!(json.contains("\"metadata\""));
    assert!(json.contains("\"setup\""));
    assert!(json.contains("\"recorded_events\""));
    assert!(json.contains("\"latest_snapshot\""));

    let loaded = PersistedGameRecord::from_json(&json).unwrap();
    assert_eq!(loaded, persisted);
    assert_eq!(loaded.replay().unwrap(), expected_state);
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
fn replay_verification_succeeds_for_recorded_setup_automatic_and_command_events() {
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

    assert_eq!(
        verify_recorded_events(record.setup(), &record.recorded_events()).unwrap(),
        record.state().unwrap()
    );
}

#[test]
fn replay_verification_reports_automatic_event_mismatch_sequence_and_details() {
    let mut record = GameRecord::start(two_player_setup(), (1..=20).map(card).collect()).unwrap();
    record.advance_automatic().unwrap();
    let mut recorded_events = record.recorded_events();
    let automatic_index = recorded_events
        .iter()
        .position(|recorded| matches!(recorded.metadata.source, EventSource::Automatic { .. }))
        .unwrap();
    recorded_events[automatic_index].event = GameEvent::TurnStarted {
        player: PlayerId::new("p2"),
        turn_number: 1,
    };

    match verify_recorded_events(record.setup(), &recorded_events) {
        Err(ReplayVerificationError::EventMismatch {
            sequence,
            expected,
            actual,
        }) => {
            assert_eq!(sequence, recorded_events[automatic_index].metadata.sequence);
            assert_eq!(
                expected,
                vec![GameEvent::TurnStarted {
                    player: PlayerId::new("p1"),
                    turn_number: 1,
                }]
            );
            assert_eq!(actual, vec![recorded_events[automatic_index].event.clone()]);
        }
        other => panic!("expected automatic event mismatch, got {other:?}"),
    }
}

#[test]
fn replay_verification_reports_command_event_mismatch_sequence_and_details() {
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
    let mut recorded_events = record.recorded_events();
    let command_index = recorded_events
        .iter()
        .position(|recorded| matches!(recorded.event, GameEvent::AttackResolved { .. }))
        .unwrap();
    if let GameEvent::AttackResolved { hp_change, .. } = &mut recorded_events[command_index].event {
        hp_change.delta = -99;
        hp_change.new_hp = 0;
    }

    match verify_recorded_events(record.setup(), &recorded_events) {
        Err(ReplayVerificationError::EventMismatch {
            sequence,
            expected,
            actual,
        }) => {
            let first_command_sequence = recorded_events
                .iter()
                .find(|recorded| matches!(recorded.metadata.source, EventSource::Command { .. }))
                .unwrap()
                .metadata
                .sequence;
            assert_eq!(sequence, first_command_sequence);
            assert_ne!(expected, actual);
            assert!(matches!(
                actual
                    .iter()
                    .find(|event| matches!(event, GameEvent::AttackResolved { .. })),
                Some(GameEvent::AttackResolved { hp_change, .. }) if hp_change.delta == -99
            ));
        }
        other => panic!("expected command event mismatch, got {other:?}"),
    }
}

#[test]
fn pure_replay_applies_canonical_events_even_when_verification_would_fail() {
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
    let mut persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "base".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );
    let command_index = persisted
        .recorded_events
        .iter()
        .position(|recorded| matches!(recorded.event, GameEvent::AttackResolved { .. }))
        .unwrap();
    if let GameEvent::AttackResolved { hp_change, .. } =
        &mut persisted.recorded_events[command_index].event
    {
        hp_change.delta = -99;
        hp_change.new_hp = 0;
    }

    assert!(matches!(
        verify_recorded_events(&persisted.setup, &persisted.recorded_events),
        Err(ReplayVerificationError::EventMismatch { .. })
    ));
    let replayed = persisted.replay().unwrap();
    assert_ne!(replayed, record.state().unwrap());
    assert!(replayed.hp.iter().any(|team_hp| team_hp.hp == 0));
}

#[test]
fn filesystem_persistence_saves_loads_event_logs_and_optional_snapshots() {
    let root = temp_persistence_dir("filesystem-round-trip");
    let mut adapter = FileSystemPersistence::new(&root);
    let mut record = GameRecord::start(two_player_setup(), deck_starting_with(&[2, 7])).unwrap();
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

    adapter.save_event_log("game-1", &persisted).unwrap();
    assert_eq!(adapter.load_snapshot("game-1").unwrap(), None);
    adapter.save_snapshot("game-1", &snapshot).unwrap();

    let loaded_log = adapter.load_event_log("game-1").unwrap().unwrap();
    let loaded_snapshot = adapter.load_snapshot("game-1").unwrap().unwrap();

    assert_eq!(loaded_log, persisted);
    assert_eq!(loaded_log.replay().unwrap(), record.state().unwrap());
    assert_eq!(loaded_snapshot, snapshot);

    fs::remove_dir_all(root).unwrap();
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
