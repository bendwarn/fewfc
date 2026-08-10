use fewfc::application::{
    AutomaticReason, EventSource, GameRecord, RecordedDecisionSource, ReplayVerificationError,
    replay, verify_recorded_decisions,
};
use fewfc::domain::{
    BaseChoiceContinuation, BaseRandomnessContinuation, CardDef, CardDefId, CardInstanceDef,
    CardInstanceId, ChoiceContinuation, ChoiceId, Command, GameError, GameEvent, GameSetup,
    GameState, HpChangeDelta, PendingChoice, PendingChoiceKind, PlayerId, RandomnessContinuation,
    RandomnessOperation, RuleModuleId, RulesetId, TeamId, ValidationError,
};
use fewfc::infrastructure::{
    FileSystemPersistence, FixedDeckPreparation, InMemoryPersistence, PersistedGameRecord,
    PersistedSnapshot, PersistenceMetadata, SeededDeckPreparation,
};
use fewfc::ports::{DeckPreparation, EventLogStorage, GameRecordRepository, SnapshotStorage};
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
fn game_record_start_can_use_deck_preparation_adapter() {
    let setup = two_player_setup();
    let prepared_order = deck_starting_with(&[5, 10, 1, 2]);
    let mut preparation = FixedDeckPreparation::new(prepared_order.clone());

    let record = GameRecord::start_with_deck_preparation(setup, &mut preparation).unwrap();

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
    assert_eq!(record.replay().unwrap(), record.state().clone());
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
        persisted.recorded_decisions.len(),
        record.recorded_decisions().len()
    );
    assert_eq!(persisted.replay().unwrap(), record.state().clone());
    assert_eq!(
        persisted.replay().unwrap().pending_choice,
        Some(PendingChoice {
            choice_id: ChoiceId::new(1),
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::Card {
                cards: vec![card(3), card(4), card(6), card(7), card(8)],
                minimum: 2,
                maximum: 2,
                can_decline: false,
            },
            continuation: ChoiceContinuation::Base(BaseChoiceContinuation::ChaosReturnTwo),
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

    assert_eq!(persisted.replay().unwrap(), record.state().clone());
    assert_eq!(
        persisted.replay().unwrap().pending_choice,
        Some(PendingChoice {
            choice_id: ChoiceId::new(1),
            player: PlayerId::new("p1"),
            kind: PendingChoiceKind::Card {
                cards: vec![card(10), card(11), card(12)],
                minimum: 1,
                maximum: 1,
                can_decline: false,
            },
            continuation: ChoiceContinuation::Base(BaseChoiceContinuation::TurnDrawDiscard),
        })
    );
}

#[test]
fn turn_draw_discard_shuffle_records_turn_draw_reason_and_replays() {
    let mut setup = two_player_setup();
    setup
        .card_instances
        .retain(|instance| instance.instance <= card(11));
    let mut record = GameRecord::start(setup, (1..=11).map(card).collect()).unwrap();

    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let events = record.advance_automatic().unwrap();
    assert!(matches!(
        events.as_slice(),
        [GameEvent::RandomnessRequested { request }]
            if request.request_id == "base:turn-draw:1:p1"
                && matches!(
                    request.operation,
                    RandomnessOperation::DiscardShuffle { .. }
                )
                && request.continuation
                    == RandomnessContinuation::Base(BaseRandomnessContinuation::TurnDraw)
    ));
    assert!(matches!(
        record.recorded_events().last(),
        Some(recorded)
            if matches!(recorded.event, GameEvent::RandomnessRequested { .. })
                && recorded.metadata.source
                    == EventSource::Automatic {
                        reason: AutomaticReason::TurnDraw,
                    }
    ));
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn persisted_event_log_json_round_trip_contains_decision_log_and_replays() {
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
    let expected_state = record.state().clone();
    let persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "base".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );

    let json = persisted.to_json().unwrap();
    assert!(json.contains("\"metadata\""));
    assert!(json.contains("\"setup\""));
    assert!(json.contains("\"recorded_decisions\""));
    assert!(!json.contains("\"latest_snapshot\""));

    let loaded = PersistedGameRecord::from_json(&json).unwrap();
    assert_eq!(loaded, persisted);
    assert_eq!(loaded.replay().unwrap(), expected_state);
}

#[test]
fn legacy_snapshot_phase_names_are_upgraded_before_deserialization() {
    let mut state = GameState::from_setup(&two_player_setup());
    state.phase = fewfc::domain::Phase::ActiveEffects;
    let snapshot = PersistedSnapshot::from_state(7, state);
    let legacy_json = snapshot
        .to_json()
        .unwrap()
        .replace("\"ActiveEffects\"", "\"Main\"");

    let loaded = PersistedSnapshot::from_json(&legacy_json).unwrap();
    assert_eq!(loaded.state.phase, fewfc::domain::Phase::ActiveEffects);
    assert_eq!(loaded.state.turn_draw_pool, Vec::new());
    assert_eq!(loaded.state.formation_areas.len(), 2);
}

#[test]
fn legacy_attack_context_is_wrapped_in_the_atomic_resolution_payload() {
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
    let persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "base".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );
    let mut wire = serde_json::to_value(&persisted).unwrap();
    assert!(unwrap_first_atomic_attack_context(&mut wire));
    let json = serde_json::to_string(&wire).unwrap();

    let loaded = PersistedGameRecord::from_json(&json).unwrap();
    assert_eq!(loaded.replay().unwrap(), record.state().clone());
}

fn unwrap_first_atomic_attack_context(value: &mut serde_json::Value) -> bool {
    match value {
        serde_json::Value::Array(values) => {
            values.iter_mut().any(unwrap_first_atomic_attack_context)
        }
        serde_json::Value::Object(values) => {
            if let Some(serde_json::Value::Object(attack)) = values.get_mut("AttackResolved")
                && let Some(serde_json::Value::Object(payload)) =
                    attack.get_mut("elemental_context_update")
                && let Some(old_context) = payload.remove("elemental_context_update")
            {
                attack.insert("elemental_context_update".to_string(), old_context);
                return true;
            }
            values.values_mut().any(unwrap_first_atomic_attack_context)
        }
        _ => false,
    }
}

#[test]
fn persisted_replay_uses_event_order_and_payloads_not_decision_source() {
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
    let expected_state = record.state().clone();
    let mut persisted = PersistedGameRecord::from_record(
        PersistenceMetadata {
            ruleset_id: "base".to_string(),
            engine_version: "test".to_string(),
        },
        &record,
    );

    for decision in &mut persisted.recorded_decisions {
        decision.source = RecordedDecisionSource::Setup;
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
        verify_recorded_decisions(record.setup(), &record.recorded_decisions()).unwrap(),
        record.state().clone()
    );
}

#[test]
fn replay_verification_reports_automatic_event_mismatch_sequence_and_details() {
    let mut record = GameRecord::start(two_player_setup(), (1..=20).map(card).collect()).unwrap();
    record.advance_automatic().unwrap();
    let mut recorded_decisions = record.recorded_decisions();
    let automatic_index = recorded_decisions
        .iter()
        .position(|decision| matches!(decision.source, RecordedDecisionSource::Automatic))
        .unwrap();
    recorded_decisions[automatic_index].events[0] = GameEvent::TurnStarted {
        player: PlayerId::new("p2"),
        turn_number: 1,
    };
    let automatic_sequence = recorded_decisions[..automatic_index]
        .iter()
        .map(|decision| decision.events.len() as u64)
        .sum::<u64>()
        + 1;

    match verify_recorded_decisions(record.setup(), &recorded_decisions) {
        Err(ReplayVerificationError::EventMismatch {
            sequence,
            expected,
            actual,
        }) => {
            assert_eq!(sequence, automatic_sequence);
            assert_eq!(
                expected,
                vec![GameEvent::TurnStarted {
                    player: PlayerId::new("p1"),
                    turn_number: 1,
                }]
            );
            assert_eq!(actual, recorded_decisions[automatic_index].events);
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
    let mut recorded_decisions = record.recorded_decisions();
    let command_index = recorded_decisions
        .iter()
        .position(|decision| {
            matches!(decision.source, RecordedDecisionSource::Command { .. })
                && decision
                    .events
                    .iter()
                    .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
        })
        .unwrap();
    if let Some(GameEvent::AttackResolved { hp_change, .. }) = recorded_decisions[command_index]
        .events
        .iter_mut()
        .find(|event| matches!(event, GameEvent::AttackResolved { .. }))
    {
        hp_change.delta = -99;
        hp_change.new_hp = 0;
    }
    let first_command_sequence = recorded_decisions[..command_index]
        .iter()
        .map(|decision| decision.events.len() as u64)
        .sum::<u64>()
        + 1;

    match verify_recorded_decisions(record.setup(), &recorded_decisions) {
        Err(ReplayVerificationError::EventMismatch {
            sequence,
            expected,
            actual,
        }) => {
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
        .recorded_decisions
        .iter()
        .position(|decision| {
            decision
                .events
                .iter()
                .any(|event| matches!(event, GameEvent::AttackResolved { .. }))
        })
        .unwrap();
    if let Some(GameEvent::AttackResolved { hp_change, .. }) = persisted.recorded_decisions
        [command_index]
        .events
        .iter_mut()
        .find(|event| matches!(event, GameEvent::AttackResolved { .. }))
    {
        hp_change.delta = -99;
        hp_change.new_hp = 0;
    }

    assert!(matches!(
        verify_recorded_decisions(&persisted.setup, &persisted.recorded_decisions),
        Err(ReplayVerificationError::EventMismatch { .. })
    ));
    let replayed = persisted.replay().unwrap();
    assert_ne!(replayed, record.state().clone());
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
    let snapshot =
        PersistedSnapshot::from_state(record.recorded_event_count() as u64, record.state().clone());

    adapter.save_event_log("game-1", &persisted).unwrap();
    assert_eq!(adapter.load_snapshot("game-1").unwrap(), None);
    adapter.save_snapshot("game-1", &snapshot).unwrap();

    let loaded_log = adapter.load_event_log("game-1").unwrap().unwrap();
    let loaded_snapshot = adapter.load_snapshot("game-1").unwrap().unwrap();

    assert_eq!(loaded_log, persisted);
    assert_eq!(loaded_log.replay().unwrap(), record.state().clone());
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
    let snapshot =
        PersistedSnapshot::from_state(record.recorded_event_count() as u64, record.state().clone());

    let mut storage = InMemoryPersistence::default();
    storage.save_event_log("game-1", &persisted).unwrap();
    storage.save_snapshot("game-1", &snapshot).unwrap();

    let loaded_log = storage.load_event_log("game-1").unwrap().unwrap();
    let loaded_snapshot = storage.load_snapshot("game-1").unwrap().unwrap();

    assert_eq!(loaded_log.replay().unwrap(), record.state().clone());
    assert_eq!(loaded_snapshot.state, record.state().clone());
    assert_eq!(
        loaded_snapshot
            .state
            .covered_passive(&PlayerId::new("p1"))
            .unwrap()
            .cards,
        vec![card(2), card(7)]
    );
}

#[test]
fn game_record_repository_round_trip_loads_game_record_without_persisted_dto_callers() {
    let mut record =
        GameRecord::start(two_player_setup(), deck_starting_with(&[1, 2, 3, 4])).unwrap();
    record.advance_automatic().unwrap();
    record
        .handle(Command::PerformFormation {
            player: PlayerId::new("p1"),
            formation_id: "metal-strike".to_string(),
            cards: vec![card(1)],
            declared_targets: Vec::new(),
        })
        .unwrap();

    let mut repository = InMemoryPersistence::default();
    repository.save_record("game-1", &record).unwrap();

    let loaded = repository.load_record("game-1").unwrap().unwrap();

    assert_eq!(loaded.setup(), record.setup());
    assert_eq!(loaded.events(), record.events());
    assert_eq!(loaded.state().clone(), record.state().clone());
    assert_eq!(repository.load_snapshot("game-1").unwrap(), None);
    assert_eq!(repository.load_record("missing").unwrap(), None);
}

#[test]
fn environment_events_round_trip_and_replay_without_recomputing_rules() {
    let setup =
        two_player_setup().with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]);
    let events = vec![
        GameEvent::EnvironmentTransferred {
            player: PlayerId::new("p1"),
            formation_id: "south-vermilion-bird".to_string(),
            from: None,
            to: Element::Fire,
        },
        GameEvent::EnvironmentCleared {
            player: PlayerId::new("p2"),
            formation_id: "void-meridian-severing".to_string(),
            environment: Element::Fire,
            hp_changes: vec![
                HpChangeDelta {
                    team: TeamId::new("team:p1"),
                    old_hp: 30,
                    delta: -20,
                    new_hp: 10,
                    effective_delta: -20,
                },
                HpChangeDelta {
                    team: TeamId::new("team:p2"),
                    old_hp: 30,
                    delta: -20,
                    new_hp: 10,
                    effective_delta: -20,
                },
            ],
        },
    ];

    let json = serde_json::to_string(&events).unwrap();
    let restored: Vec<GameEvent> = serde_json::from_str(&json).unwrap();
    let state = replay(&setup, &restored).unwrap();

    assert_eq!(restored, events);
    assert_eq!(state.environment, None);
    assert!(state.hp.iter().all(|team_hp| team_hp.hp == 10));
}
