use fewfc::competition_web_api::handle_request_json;
use fewfc::domain::{Player, PlayerId, RuleModuleId, RuleVersion, TeamId};
use fewfc::rules::OfficialRules;
use serde_json::{Value, json};

fn request(operation: Value, actor: Value, time: u64, state: Value) -> Value {
    let value = json!({
        "serverActor": actor,
        "trustedTimeMs": time,
        "operation": operation,
        "serverState": state,
    });
    serde_json::from_str(&handle_request_json(&value.to_string()).unwrap()).unwrap()
}

fn organizer() -> Value {
    json!({ "type": "organizer", "playerId": "organizer" })
}

fn player(id: &str) -> Value {
    json!({ "type": "player", "playerId": id })
}

fn empty_state() -> Value {
    json!({ "registration": null, "clock": null, "tournament": null, "matches": {} })
}

#[test]
fn e6a3a_registration_resolve_settings_matches_official_setup_for_all_legal_shapes() {
    let module_sets: &[&[&str]] = &[
        &[],
        &["star"],
        &["personal-deck"],
        &["star", "five-directions-legend", "hero-schools", "spirit"],
        &[
            "star",
            "five-directions-legend",
            "hero-schools",
            "spirit",
            "personal-deck",
            "pouch",
        ],
    ];
    for version in [RuleVersion::V5_16, RuleVersion::V5_17] {
        for team_size in [1_u16, 2, 3] {
            for modules in module_sets {
                let mut players = Vec::new();
                let mut turn_order = Vec::new();
                for index in 0..team_size {
                    for team in ["settings-a", "settings-b"] {
                        let id = PlayerId::new(format!("{team}-{index}"));
                        players.push(Player {
                            id: id.clone(),
                            team: TeamId::new(team),
                        });
                        turn_order.push(id);
                    }
                }
                // 期望值來自正式規則的實際人數與合法交替座次，不複製生命對照表。
                let setup = OfficialRules::new()
                    .configure_versioned_game_with_decks(
                        version,
                        players,
                        turn_order,
                        modules.iter().map(|id| RuleModuleId::new(*id)).collect(),
                        Vec::new(),
                    )
                    .unwrap();
                assert_eq!(setup.hp.len(), 2);
                let expected_hp = setup.hp[0].hp;
                assert!(expected_hp > 0);
                assert!(setup.hp.iter().all(|hp| hp.hp == expected_hp));
                let operation = json!({
                    "type": "registrationResolveSettings",
                    "ruleVersion": version,
                    "ruleset": "base",
                    "enabledRuleModules": modules,
                    "teamSize": team_size,
                });
                for actor in [
                    organizer(),
                    json!({"type": "referee", "playerId": "referee"}),
                ] {
                    for time in [0, 1_000] {
                        let queried =
                            request(operation.clone(), actor.clone(), time, empty_state());
                        assert_eq!(queried["ok"], true, "{queried}");
                        assert_eq!(queried["operation"], "registrationResolveSettings");
                        assert_eq!(queried["public"], json!({"initialHp": expected_hp}));
                        assert_eq!(queried["serverState"], empty_state());
                        assert!(queried.get("private").is_none());
                        assert_eq!(queried.as_object().unwrap().len(), 4);
                    }
                }
            }
        }
    }
}

fn e6a3a_settings_operation() -> Value {
    json!({ "type": "registrationResolveSettings", "ruleVersion": "5.17",
        "ruleset": "base", "enabledRuleModules": [], "teamSize": 2 })
}

#[test]
fn e6a3a_registration_resolve_settings_rejects_invalid_rules_and_resource_inputs() {
    let base = e6a3a_settings_operation();
    for (field, value, code) in [
        ("ruleVersion", json!("unknown"), "invalidRequest"),
        ("ruleset", json!("unknown"), "registrationError"),
        ("ruleset", json!("fewfc-base"), "registrationError"),
        ("teamSize", json!(0), "registrationError"),
        ("teamSize", json!(65536_u64), "invalidRequest"),
        ("teamSize", json!(u64::MAX), "invalidRequest"),
        ("teamSize", json!(-1), "invalidRequest"),
        ("teamSize", json!(1.5), "invalidRequest"),
        ("teamSize", json!("2"), "invalidRequest"),
        (
            "enabledRuleModules",
            json!(["unknown"]),
            "registrationError",
        ),
        (
            "enabledRuleModules",
            json!(["star", "star"]),
            "registrationError",
        ),
        ("enabledRuleModules", json!(["spirit"]), "registrationError"),
        ("enabledRuleModules", json!(["pouch"]), "registrationError"),
    ] {
        let mut invalid = base.clone();
        invalid[field] = value;
        let response = request(invalid, organizer(), 0, empty_state());
        assert_eq!(response["ok"], false, "{response}");
        assert_eq!(response["error"]["code"], code, "{response}");
        assert_eq!(response.as_object().unwrap().len(), 2);
        for field in ["public", "private", "serverState"] {
            assert!(response.get(field).is_none());
        }
    }
    for (version, module) in [("5.16", "totem-formation"), ("5.17", "echo")] {
        let mut invalid = base.clone();
        invalid["ruleVersion"] = json!(version);
        invalid["enabledRuleModules"] =
            json!(["star", "five-directions-legend", "hero-schools", module]);
        assert_eq!(
            request(invalid, organizer(), 0, empty_state())["error"]["code"],
            "registrationError"
        );
    }
    // 最大合法 u16 與非法模組組合，應先被既有模組驗證拒絕，不進人數配置。
    let mut invalid = base;
    invalid["teamSize"] = json!(u16::MAX);
    invalid["enabledRuleModules"] = json!(["unknown"]);
    assert_eq!(
        request(invalid, organizer(), 0, empty_state())["error"]["code"],
        "registrationError"
    );
}

#[test]
fn e6a3a_registration_resolve_settings_operation_is_exact_and_official_only() {
    use fewfc::competition_web_api::CompetitionOperation;
    let base = e6a3a_settings_operation();
    let operation: CompetitionOperation = serde_json::from_value(base.clone()).unwrap();
    assert_eq!(serde_json::to_value(operation).unwrap(), base);
    assert_eq!(base.as_object().unwrap().len(), 5);
    for field in [
        "initialHp",
        "initial_hp",
        "private",
        "announcement",
        "serverState",
        "players",
        "deckSnapshot",
        "trustedTimeMs",
        "expectedRevision",
        "rule_version",
        "enabled_rule_modules",
        "team_size",
    ] {
        let mut invalid = base.clone();
        invalid[field] = json!("forged");
        let response = request(invalid, organizer(), 0, empty_state());
        assert_eq!(response["ok"], false, "{response}");
        assert_eq!(response["error"]["code"], "invalidRequest");
        assert_eq!(response.as_object().unwrap().len(), 2);
    }
    for field in ["ruleVersion", "ruleset", "enabledRuleModules", "teamSize"] {
        let mut missing = base.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert_eq!(
            request(missing, organizer(), 0, empty_state())["error"]["code"],
            "invalidRequest"
        );
    }
    let unauthorized = request(base.clone(), player("captain"), 0, empty_state());
    assert_eq!(unauthorized["ok"], false);
    assert_eq!(unauthorized["error"]["code"], "unauthorizedActor");
    assert_eq!(unauthorized.as_object().unwrap().len(), 2);
    for actor in [organizer(), json!({"type":"referee", "playerId":"referee"})] {
        assert_eq!(request(base.clone(), actor, 0, empty_state())["ok"], true);
    }
}

#[test]
fn e6a3a_registration_resolve_settings_preserves_private_and_other_domains_raw() {
    let mut personal = announcement();
    personal["enabledRuleModules"] = json!(["personal-deck"]);
    personal["personalDecks"] = json!("required");
    let created = request(
        json!({"type":"registrationCreate", "announcement":personal}),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(created["ok"], true);
    let mut state = created["serverState"].clone();
    for (revision, (id, actor, action)) in [
        ("hp-open", organizer(), json!({"type":"open"})),
        ("hp-register", player("private-member"), json!({"type":"registerTeam", "teamId":"private-team", "captain":"private-member", "members":["private-member"]})),
        ("hp-confirm", player("private-member"), json!({"type":"confirmMembership", "teamId":"private-team", "rosterVersion":1})),
    ].into_iter().enumerate() {
        let response = request(json!({"type":"registrationExecute", "commandId":id, "expectedRevision":revision, "action":action}), actor, revision as u64 + 1, state);
        assert_eq!(response["ok"], true, "{response}");
        state = response["serverState"].clone();
    }
    let members = [
        PlayerId::new("private-member"),
        PlayerId::new("other-member"),
    ];
    let setup = OfficialRules::new()
        .configure_versioned_game_with_decks(
            RuleVersion::default(),
            members
                .iter()
                .map(|id| Player {
                    id: id.clone(),
                    team: TeamId::new(id.as_str()),
                })
                .collect(),
            members.to_vec(),
            vec![RuleModuleId::new("personal-deck")],
            Vec::new(),
        )
        .unwrap();
    let cards: Vec<_> = setup.deck_lists[0]
        .cards
        .iter()
        .map(|id| id.as_str())
        .collect();
    let submitted = request(
        json!({"type":"registrationExecute", "commandId":"hp-private-deck", "expectedRevision":3,
        "action":{"type":"submitDeck", "teamId":"private-team", "rosterVersion":1,
            "name":"private-deck-name", "cards":cards}}),
        player("private-member"),
        4,
        state,
    );
    assert_eq!(submitted["ok"], true, "{submitted}");
    assert!(
        submitted["serverState"]["registration"]
            .to_string()
            .contains("private-deck-name")
    );
    let clock = request(
        json!({"type":"clockCreate", "teams":["a","b"],
        "config":{"mainTimeMs":1000,"actionTimeMs":100,"outOfTurnChoiceTimeMs":20}}),
        organizer(),
        1000,
        submitted["serverState"].clone(),
    );
    assert_eq!(clock["ok"], true);
    let running = request(
        json!({"type":"clockBeginAction", "team":"a", "turn":1}),
        organizer(),
        1000,
        clock["serverState"].clone(),
    );
    assert_eq!(running["ok"], true);
    let created_match = request(
        json!({"type":"matchCreate", "config":assigned_match_config("hp-private-match")}),
        organizer(),
        1000,
        running["serverState"].clone(),
    );
    assert_eq!(created_match["ok"], true);
    let stage = tournament_view_create("roundRobin", 2)["public"]["stages"][0]["config"].clone();
    let tournament = request(
        json!({"type":"tournamentCreate", "config":{"tournamentId":"hp-other-event", "stage":stage}}),
        organizer(),
        1000,
        created_match["serverState"].clone(),
    );
    assert_eq!(tournament["ok"], true);
    let original = tournament["serverState"].clone();
    for time in [0, 2000, u64::MAX] {
        for actor in [organizer(), json!({"type":"referee", "playerId":"referee"})] {
            for _ in 0..2 {
                let queried = request(
                    e6a3a_settings_operation(),
                    actor.clone(),
                    time,
                    original.clone(),
                );
                assert_eq!(queried["ok"], true, "{queried}");
                assert_eq!(queried["serverState"], original);
                assert_eq!(queried["public"].as_object().unwrap().len(), 1);
                assert!(queried["public"]["initialHp"].as_i64().unwrap() > 0);
                assert!(queried.get("private").is_none());
                assert_eq!(queried.as_object().unwrap().len(), 4);
                for secret in [
                    "private-member",
                    "private-deck-name",
                    "hp-private-match",
                    "cards",
                    "clock",
                    "registration",
                ] {
                    assert!(!queried["public"].to_string().contains(secret));
                }
            }
        }
    }
    // 設定 query 不把合法舊 checkpoint 補預設欄位，也不觀測／重設其他 domain。
    let mut legacy = original;
    for field in ["paused", "preparationStarted", "doubleExpired"] {
        legacy["clock"].as_object_mut().unwrap().remove(field);
    }
    let queried = request(
        e6a3a_settings_operation(),
        organizer(),
        9999,
        legacy.clone(),
    );
    assert_eq!(queried["ok"], true, "{queried}");
    assert_eq!(queried["serverState"], legacy);
    let mut empty_legacy = empty_state();
    empty_legacy.as_object_mut().unwrap().remove("matches");
    assert_eq!(
        request(
            e6a3a_settings_operation(),
            organizer(),
            0,
            empty_legacy.clone()
        )["serverState"],
        empty_legacy
    );
    let mut malformed = empty_state();
    malformed["registration"] = json!("corrupt");
    let rejected = request(e6a3a_settings_operation(), organizer(), 0, malformed);
    assert_eq!(rejected["error"]["code"], "invalidRequest");
    assert!(rejected.get("public").is_none());
    assert!(rejected.get("private").is_none());
    assert!(rejected.get("serverState").is_none());
}

#[test]
fn e6e2a_clock_checkpoint_metadata_reads_authoritative_deadline_without_mutating_state() {
    let registration = request(
        json!({ "type": "registrationCreate", "announcement": announcement() }),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(registration["ok"], true, "{registration}");
    let opened = request(
        json!({ "type": "registrationExecute", "commandId": "metadata-open",
            "expectedRevision": 0, "action": { "type": "open" } }),
        organizer(),
        1,
        registration["serverState"].clone(),
    );
    assert_eq!(opened["ok"], true, "{opened}");
    let clock = request(
        json!({ "type": "clockCreate", "teams": ["a", "b"],
            "config": { "mainTimeMs": 1000, "actionTimeMs": 100, "outOfTurnChoiceTimeMs": 20 } }),
        organizer(),
        3000,
        opened["serverState"].clone(),
    );
    assert_eq!(clock["ok"], true, "{clock}");
    let running = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        3000,
        clock["serverState"].clone(),
    );
    assert_eq!(running["ok"], true, "{running}");
    assert_eq!(running["public"]["clock"]["nextTimeoutDeadlineMs"], 4100);
    let match_created = request(
        json!({ "type": "matchCreate", "config": assigned_match_config("metadata-other-match") }),
        organizer(),
        3000,
        running["serverState"].clone(),
    );
    assert_eq!(match_created["ok"], true, "{match_created}");
    let config = tournament_view_create("roundRobin", 2)["public"]["stages"][0]["config"].clone();
    let tournament = request(
        json!({ "type": "tournamentCreate",
            "config": { "tournamentId": "metadata-event", "stage": config } }),
        organizer(),
        3000,
        match_created["serverState"].clone(),
    );
    assert_eq!(tournament["ok"], true, "{tournament}");
    let state = tournament["serverState"].clone();
    for actor in [
        organizer(),
        player("reader"),
        json!({ "type": "referee", "playerId": "referee" }),
    ] {
        for time in [3000, 4000, 10000] {
            for _ in 0..2 {
                // metadata 讀 checkpoint 的既有期限，不依 query 時刻結算／進讀秒／判負。
                let queried = request(
                    json!({ "type": "clockCheckpointMetadata" }),
                    actor.clone(),
                    time,
                    state.clone(),
                );
                assert_eq!(queried["ok"], true, "{queried}");
                assert_eq!(queried["operation"], "clockCheckpointMetadata");
                assert_eq!(queried["public"], json!({ "nextTimeoutDeadlineMs": 4100 }));
                assert_eq!(queried["serverState"], state);
                assert!(queried.get("private").is_none());
                assert_eq!(queried.as_object().unwrap().len(), 4);
            }
        }
    }
}

fn e6e2a_metadata_query(state: &Value, deadline: Option<u64>, time: u64) {
    let queried = request(
        json!({ "type": "clockCheckpointMetadata" }),
        player("metadata-reader"),
        time,
        state.clone(),
    );
    assert_eq!(queried["ok"], true, "{queried}");
    assert_eq!(
        queried["public"],
        json!({ "nextTimeoutDeadlineMs": deadline })
    );
    assert_eq!(queried["serverState"], *state);
    assert!(queried.get("private").is_none());
    assert_eq!(queried.as_object().unwrap().len(), 4);
}

#[test]
fn e6e2a_clock_checkpoint_metadata_handles_windows_pause_terminal_and_new_game() {
    let idle = e6e1_clock_state(
        json!({ "mainTimeMs": 1000, "actionTimeMs": 100, "outOfTurnChoiceTimeMs": 20 }),
        3000,
        empty_state(),
    );
    e6e2a_metadata_query(&idle, None, 10000);
    let preparation = request(
        json!({ "type": "clockBeginPreparation" }),
        organizer(),
        3000,
        idle.clone(),
    );
    assert_eq!(preparation["ok"], true);
    e6e2a_metadata_query(&preparation["serverState"], Some(4100), 10000);
    let partial = request(
        json!({ "type": "clockFinishPreparation", "team": "a" }),
        organizer(),
        3100,
        preparation["serverState"].clone(),
    );
    assert_eq!(partial["ok"], true);
    e6e2a_metadata_query(&partial["serverState"], Some(4100), 10000);
    let paused = request(
        json!({ "type": "clockPause", "reason": "metadata 準備暫停" }),
        organizer(),
        3200,
        partial["serverState"].clone(),
    );
    assert_eq!(paused["ok"], true);
    e6e2a_metadata_query(&paused["serverState"], None, 10000);
    let resumed = request(
        json!({ "type": "clockResume", "reason": "metadata 準備恢復" }),
        organizer(),
        4000,
        paused["serverState"].clone(),
    );
    assert_eq!(resumed["ok"], true);
    e6e2a_metadata_query(&resumed["serverState"], Some(4900), 10000);
    let terminal = request(
        json!({ "type": "clockObserve" }),
        organizer(),
        4100,
        preparation["serverState"].clone(),
    );
    assert_eq!(terminal["ok"], true);
    e6e2a_metadata_query(&terminal["serverState"], None, 10000);
    let action = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        3000,
        idle.clone(),
    );
    assert_eq!(action["ok"], true);
    e6e2a_metadata_query(&action["serverState"], Some(4100), 10000);
    let choice = request(
        json!({ "type": "clockBeginOutOfTurnChoice", "team": "b", "choiceId": "metadata-choice" }),
        organizer(),
        3000,
        idle,
    );
    assert_eq!(choice["ok"], true);
    // B 先用主時間 1000，再用 choice 讀秒 20；晚查 metadata 不推進權威。
    e6e2a_metadata_query(&choice["serverState"], Some(4020), 10000);
    let ended = request(
        json!({ "type": "clockEndWindow" }),
        organizer(),
        3123,
        action["serverState"].clone(),
    );
    assert_eq!(ended["ok"], true);
    e6e2a_metadata_query(&ended["serverState"], None, 10000);
    for time in ["reset", "carryForward"] {
        let next = request(
            json!({ "type": "clockNewGame", "time": time }),
            organizer(),
            3130,
            ended["serverState"].clone(),
        );
        assert_eq!(next["ok"], true, "{next}");
        e6e2a_metadata_query(&next["serverState"], None, 10000);
    }
    for (config, deadline) in [
        (
            json!({ "mainTimeMs": 1000, "actionTimeMs": 0, "outOfTurnChoiceTimeMs": 20 }),
            Some(4000),
        ),
        (
            json!({ "mainTimeMs": null, "actionTimeMs": null, "outOfTurnChoiceTimeMs": null }),
            None,
        ),
    ] {
        let created = e6e1_clock_state(config, 3000, empty_state());
        let started = request(
            json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
            organizer(),
            3000,
            created,
        );
        assert_eq!(started["ok"], true);
        e6e2a_metadata_query(&started["serverState"], deadline, 10000);
    }
}

#[test]
fn e6e2a_clock_checkpoint_metadata_preserves_legacy_raw_state_shape() {
    let mut legacy = e6e1_clock_state(
        json!({ "mainTimeMs": 100, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 }),
        0,
        empty_state(),
    );
    legacy.as_object_mut().unwrap().remove("matches");
    let checkpoint = legacy["clock"].as_object_mut().unwrap();
    checkpoint.insert("version".into(), json!(1));
    for key in ["paused", "preparationStarted", "doubleExpired"] {
        checkpoint.remove(key);
    }
    for _ in 0..2 {
        e6e2a_metadata_query(&legacy, None, 10000);
    }
}

#[test]
fn e6e2a_clock_checkpoint_metadata_rejects_missing_and_malformed_checkpoints() {
    let missing = request(
        json!({ "type": "clockCheckpointMetadata" }),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(missing["ok"], false);
    assert_eq!(missing["error"]["code"], "missingState");
    let state = e6e1_clock_state(
        json!({ "mainTimeMs": 100, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 }),
        50,
        empty_state(),
    );
    let mutations: [fn(&mut Value); 8] = [
        |state| state["clock"] = json!("broken"),
        |state| state["clock"]["version"] = json!(999),
        |state| state["clock"]["teams"] = json!([]),
        |state| state["clock"]["teams"][1]["team"] = json!("a"),
        |state| state["clock"]["teams"][0]["mainTimeRemainingMs"] = json!(101),
        |state| state["clock"]["running"] = json!({ "type": "action", "team": "a", "turn": 1 }),
        |state| state["clock"]["expiredTeam"] = json!("a"),
        |state| state["clock"]["private"] = json!({ "cards": ["secret"] }),
    ];
    for mutate in mutations {
        let mut malformed = state.clone();
        mutate(&mut malformed);
        let failed = request(
            json!({ "type": "clockCheckpointMetadata" }),
            player("reader"),
            50,
            malformed,
        );
        assert_eq!(failed["ok"], false, "{failed}");
        assert!(matches!(
            failed["error"]["code"].as_str(),
            Some("invalidRequest" | "clockError")
        ));
        for field in ["public", "private", "serverState"] {
            assert!(failed.get(field).is_none());
        }
    }
}

#[test]
fn e6e2a_clock_checkpoint_metadata_wire_is_fieldless_exact_and_not_mutation_authority() {
    use fewfc::competition_web_api::{ClockCheckpointMetadataInput, CompetitionOperation};
    assert_eq!(
        serde_json::to_value(CompetitionOperation::ClockCheckpointMetadata(
            ClockCheckpointMetadataInput {}
        ))
        .unwrap(),
        json!({ "type": "clockCheckpointMetadata" })
    );
    let state = e6e1_clock_state(
        json!({ "mainTimeMs": 100, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 }),
        0,
        empty_state(),
    );
    for field in [
        "config",
        "checkpoint",
        "private",
        "serverState",
        "trustedTimeMs",
        "nextTimeoutDeadlineMs",
        "next_timeout_deadline_ms",
    ] {
        let mut extra = json!({ "type": "clockCheckpointMetadata" });
        extra[field] = json!(1);
        let failed = request(extra, player("reader"), 0, state.clone());
        assert_eq!(failed["ok"], false, "{field}: {failed}");
        assert_eq!(failed["error"]["code"], "invalidRequest");
    }
    for name in [
        "clock_checkpoint_metadata",
        "clockcheckpointmetadata",
        "clockCheckpointMetadata ",
    ] {
        let failed = request(json!({ "type": name }), player("reader"), 0, state.clone());
        assert_eq!(failed["error"]["code"], "invalidRequest");
    }
    let mutation = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        player("reader"),
        0,
        state.clone(),
    );
    assert_eq!(mutation["ok"], false);
    assert_eq!(mutation["error"]["code"], "unauthorizedActor");
    e6e2a_metadata_query(&state, None, 0);
}

#[test]
fn e6e1_clock_view_samples_without_mutating_checkpoint_for_all_actors() {
    let created = request(
        json!({ "type": "clockCreate", "teams": ["a", "b"], "config": {
            "mainTimeMs": 100, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20,
        }}),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(created["ok"], true, "{created}");
    let started = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        0,
        created["serverState"].clone(),
    );
    assert_eq!(started["ok"], true, "{started}");
    let state = started["serverState"].clone();
    for actor in [
        organizer(),
        player("reader"),
        json!({"type":"referee", "playerId":"ref"}),
    ] {
        for _ in 0..2 {
            let queried = request(
                json!({ "type": "clockView" }),
                actor.clone(),
                110,
                state.clone(),
            );
            assert_eq!(queried["ok"], true, "{queried}");
            assert_eq!(queried["operation"], "clockView");
            assert_eq!(
                queried["public"],
                json!({
                    "sampledAtMs": 110,
                    "teams": [
                        {"team":"a", "mainTimeRemainingMs":0, "byoyomiTimeRemainingMs":20, "byoyomiActive":true},
                        {"team":"b", "mainTimeRemainingMs":100, "byoyomiTimeRemainingMs":30, "byoyomiActive":false},
                    ],
                    "running":{"type":"action", "team":"a", "turn":1},
                    "paused":null, "awaitingAuthority":false,
                })
            );
            assert_eq!(queried["serverState"], state);
            assert!(queried.get("private").is_none());
            assert_eq!(queried.as_object().unwrap().len(), 4);
        }
    }
}

fn e6e1_clock_state(config: Value, time: u64, state: Value) -> Value {
    let created = request(
        json!({"type":"clockCreate", "teams":["a","b"], "config":config}),
        organizer(),
        time,
        state,
    );
    assert_eq!(created["ok"], true, "{created}");
    created["serverState"].clone()
}

fn e6e1_query(state: &Value, time: u64) -> Value {
    let queried = request(
        json!({"type":"clockView"}),
        player("reader"),
        time,
        state.clone(),
    );
    assert_eq!(queried["ok"], true, "{queried}");
    assert_eq!(queried["operation"], "clockView");
    assert_eq!(queried["serverState"], *state);
    assert!(queried.get("private").is_none());
    let display = &queried["public"];
    assert_eq!(display.as_object().unwrap().len(), 5);
    for entry in display["teams"].as_array().unwrap() {
        assert_eq!(entry.as_object().unwrap().len(), 4);
    }
    for forbidden in [
        "result",
        "timeoutRuling",
        "winner",
        "needsDecision",
        "expiredTeam",
        "byoyomiEntryOrder",
        "serverState",
        "registration",
        "matches",
        "private",
    ] {
        assert!(display.get(forbidden).is_none(), "{forbidden}: {display}");
    }
    display.clone()
}

#[test]
fn e6e1_clock_view_positive_main_zero_action_exact_deadline_is_display_only() {
    let idle = e6e1_clock_state(
        json!({"mainTimeMs":100, "actionTimeMs":0, "outOfTurnChoiceTimeMs":20}),
        0,
        empty_state(),
    );
    let started = request(
        json!({"type":"clockBeginAction", "team":"a", "turn":1}),
        organizer(),
        0,
        idle.clone(),
    );
    assert_eq!(started["ok"], true, "{started}");
    assert_eq!(started["public"]["clock"]["nextTimeoutDeadlineMs"], 100);
    let state = started["serverState"].clone();
    for (time, remaining, active, waiting) in [
        (99, 1, false, false),
        (100, 0, true, true),
        (101, 0, true, true),
    ] {
        for _ in 0..2 {
            let display = e6e1_query(&state, time);
            assert_eq!(
                display,
                json!({
                    "sampledAtMs":time,
                    "teams":[
                        {"team":"a", "mainTimeRemainingMs":remaining, "byoyomiTimeRemainingMs":0, "byoyomiActive":active},
                        {"team":"b", "mainTimeRemainingMs":100, "byoyomiTimeRemainingMs":0, "byoyomiActive":false},
                    ],
                    "running":{"type":"action", "team":"a", "turn":1},
                    "paused":null, "awaitingAuthority":waiting,
                })
            );
        }
    }
    assert_eq!(state["clock"]["teams"][0]["mainTimeRemainingMs"], 100);
    assert_eq!(state["clock"]["nextByoyomiEntrySequence"], 0);
    assert_eq!(state["clock"]["expiredTeam"], Value::Null);
    let paused = request(
        json!({"type":"clockPause", "reason":"裁判暫停"}),
        organizer(),
        99,
        state.clone(),
    );
    assert_eq!(paused["ok"], true);
    let stopped = request(json!({"type":"clockEndWindow"}), organizer(), 99, state);
    assert_eq!(stopped["ok"], true);
    for checkpoint in [
        idle,
        paused["serverState"].clone(),
        stopped["serverState"].clone(),
    ] {
        for time in [100, 101, 10_000] {
            let display = e6e1_query(&checkpoint, time);
            assert_eq!(display["awaitingAuthority"], false);
            assert_eq!(
                display["teams"][0]["mainTimeRemainingMs"],
                checkpoint["clock"]["teams"][0]["mainTimeRemainingMs"]
            );
        }
    }
}

#[test]
fn e6e1_clock_view_reads_parallel_choice_pause_idle_and_terminal_without_mutation() {
    let config = json!({"mainTimeMs":100, "actionTimeMs":30, "outOfTurnChoiceTimeMs":20});
    let idle = e6e1_clock_state(config.clone(), 0, empty_state());
    assert_eq!(
        e6e1_query(&idle, 10_000)["teams"][0]["mainTimeRemainingMs"],
        100
    );
    let preparation = request(
        json!({"type":"clockBeginPreparation"}),
        organizer(),
        0,
        idle.clone(),
    );
    assert_eq!(preparation["ok"], true);
    let preparation = preparation["serverState"].clone();
    let display = e6e1_query(&preparation, 67);
    assert_eq!(display["teams"][0]["mainTimeRemainingMs"], 33);
    assert_eq!(display["teams"][1]["mainTimeRemainingMs"], 33);
    let display = e6e1_query(&preparation, 110);
    assert_eq!(display["teams"][0]["byoyomiTimeRemainingMs"], 20);
    assert_eq!(display["teams"][1]["byoyomiTimeRemainingMs"], 20);
    assert_eq!(display["awaitingAuthority"], false);
    let display = e6e1_query(&preparation, 200);
    assert_eq!(display["teams"][0]["byoyomiTimeRemainingMs"], 0);
    assert_eq!(display["teams"][1]["byoyomiTimeRemainingMs"], 0);
    assert_eq!(display["awaitingAuthority"], true);
    assert_eq!(preparation["clock"]["doubleExpired"], false);
    assert_eq!(preparation["clock"]["nextByoyomiEntrySequence"], 0);

    let choice = request(
        json!({"type":"clockBeginOutOfTurnChoice", "team":"b", "choiceId":"choice"}),
        organizer(),
        0,
        idle,
    );
    assert_eq!(choice["ok"], true);
    let choice_state = choice["serverState"].clone();
    // B 先耗主時間 100，107 時 choice 餘 13；保留原讀秒暫停與唯讀目的。
    let display = e6e1_query(&choice_state, 107);
    assert_eq!(display["running"]["remainingMs"], 13);
    assert_eq!(display["teams"][1]["mainTimeRemainingMs"], 0);
    let paused = request(
        json!({"type":"clockPause", "reason":"裁判暫停"}),
        organizer(),
        107,
        choice_state.clone(),
    );
    assert_eq!(paused["ok"], true);
    let display = e6e1_query(&paused["serverState"], 10_000);
    assert_eq!(display["running"], Value::Null);
    assert_eq!(display["paused"]["remainingMs"], 13);
    assert_eq!(display["awaitingAuthority"], false);
    let terminal = request(
        json!({"type":"clockObserve"}),
        organizer(),
        120,
        choice_state,
    );
    assert_eq!(terminal["ok"], true);
    let display = e6e1_query(&terminal["serverState"], 10_000);
    assert_eq!(display["running"]["remainingMs"], 0);
    assert_eq!(display["awaitingAuthority"], true);

    // 配置主時間 0 表示不限時；本例 null 組合也無截止時刻，不等待裁判。
    for main in [Value::Null, json!(0)] {
        let state = e6e1_clock_state(
            json!({"mainTimeMs":main, "actionTimeMs":null, "outOfTurnChoiceTimeMs":null}),
            0,
            empty_state(),
        );
        let prepared = request(
            json!({"type":"clockBeginPreparation"}),
            organizer(),
            0,
            state,
        );
        assert_eq!(prepared["ok"], true);
        let display = e6e1_query(&prepared["serverState"], 10_000);
        assert_eq!(display["teams"][0]["mainTimeRemainingMs"], main);
        assert_eq!(display["teams"][0]["byoyomiTimeRemainingMs"], Value::Null);
        assert_eq!(display["awaitingAuthority"], false);
    }
}

#[test]
fn e6e1_clock_view_retains_other_domains_and_legacy_checkpoint_shape_after_restart() {
    let registration = request(
        json!({"type":"registrationCreate", "announcement":announcement()}),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(registration["ok"], true);
    let opened = request(
        json!({"type":"registrationExecute", "commandId":"open", "expectedRevision":0, "action":{"type":"open"}}),
        organizer(),
        0,
        registration["serverState"].clone(),
    );
    assert_eq!(opened["ok"], true, "{opened}");
    let registered = request(
        json!({"type":"registrationExecute", "commandId":"register", "expectedRevision":1,
        "action":{"type":"registerTeam", "teamId":"private-team", "captain":"private-member", "members":["private-member"]}}),
        player("private-member"),
        1,
        opened["serverState"].clone(),
    );
    assert_eq!(registered["ok"], true, "{registered}");
    let mut state = registered["serverState"].clone();
    let tournament = tournament_view_create("roundRobin", 2);
    assert_eq!(tournament["ok"], true);
    state["tournament"] = tournament["serverState"]["tournament"].clone();
    let created_match = request(
        json!({"type":"matchCreate", "config":assigned_match_config("other-match")}),
        organizer(),
        1,
        state,
    );
    assert_eq!(created_match["ok"], true);
    let state = e6e1_clock_state(
        json!({"mainTimeMs":100, "actionTimeMs":30, "outOfTurnChoiceTimeMs":20}),
        1,
        created_match["serverState"].clone(),
    );
    let started = request(
        json!({"type":"clockBeginAction", "team":"a", "turn":1}),
        organizer(),
        1,
        state,
    );
    assert_eq!(started["ok"], true);
    let state = started["serverState"].clone();
    for _ in 0..2 {
        let restored: Value = serde_json::from_str(&state.to_string()).unwrap();
        let display = e6e1_query(&restored, 111);
        assert_eq!(display["teams"][0]["byoyomiTimeRemainingMs"], 20);
        assert!(!display.to_string().contains("private-member"));
        assert!(!display.to_string().contains("other-match"));
    }
    let mut legacy = state;
    legacy.as_object_mut().unwrap().remove("matches");
    let checkpoint = legacy["clock"].as_object_mut().unwrap();
    checkpoint.insert("version".into(), json!(1));
    for key in ["paused", "preparationStarted", "doubleExpired"] {
        checkpoint.remove(key);
    }
    e6e1_query(&legacy, 111);
}

#[test]
fn e6e1_clock_view_missing_malformed_and_backwards_time_fail_closed() {
    let missing = request(
        json!({"type":"clockView"}),
        player("reader"),
        0,
        empty_state(),
    );
    assert_eq!(missing["ok"], false);
    assert_eq!(missing["error"]["code"], "missingState");
    let state = e6e1_clock_state(
        json!({"mainTimeMs":100, "actionTimeMs":30, "outOfTurnChoiceTimeMs":20}),
        50,
        empty_state(),
    );
    let backward = request(
        json!({"type":"clockView"}),
        player("reader"),
        49,
        state.clone(),
    );
    assert_eq!(backward["ok"], false);
    assert_eq!(backward["error"]["code"], "clockError");
    assert!(
        backward["error"]["message"]
            .as_str()
            .unwrap()
            .starts_with("monotonicTimeWentBackwards:")
    );
    let mutations: [fn(&mut Value); 8] = [
        |state| state["clock"] = json!("broken"),
        |state| state["clock"]["version"] = json!(999),
        |state| state["clock"]["teams"] = json!([]),
        |state| state["clock"]["teams"][1]["team"] = json!("a"),
        |state| state["clock"]["teams"][0]["mainTimeRemainingMs"] = json!(101),
        |state| state["clock"]["running"] = json!({"type":"action", "team":"a", "turn":1}),
        |state| state["clock"]["expiredTeam"] = json!("a"),
        |state| state["clock"]["private"] = json!({"cards":["secret"]}),
    ];
    for mutate in mutations {
        let mut malformed = state.clone();
        mutate(&mut malformed);
        let failed = request(json!({"type":"clockView"}), player("reader"), 50, malformed);
        assert_eq!(failed["ok"], false, "{failed}");
        assert!(matches!(
            failed["error"]["code"].as_str(),
            Some("invalidRequest" | "clockError")
        ));
        for field in ["public", "private", "serverState"] {
            assert!(failed.get(field).is_none());
        }
    }
}

#[test]
fn e6e1_clock_view_wire_is_fieldless_and_does_not_grant_mutation_authority() {
    use fewfc::competition_web_api::{ClockViewInput, CompetitionOperation};
    assert_eq!(
        serde_json::to_value(CompetitionOperation::ClockView(ClockViewInput {})).unwrap(),
        json!({"type":"clockView"})
    );
    let state = e6e1_clock_state(
        json!({"mainTimeMs":100, "actionTimeMs":30, "outOfTurnChoiceTimeMs":20}),
        0,
        empty_state(),
    );
    for field in [
        "config",
        "teams",
        "trustedTimeMs",
        "serverState",
        "reason",
        "time",
        "conclusion",
    ] {
        let mut operation = json!({"type":"clockView"});
        operation[field] = json!({"spoofed":true});
        let failed = request(operation, player("reader"), 0, state.clone());
        assert_eq!(failed["ok"], false, "{failed}");
        assert_eq!(failed["error"]["code"], "invalidRequest");
    }
    let failed = request(
        json!({"type":"clockObserve"}),
        player("reader"),
        1,
        state.clone(),
    );
    assert_eq!(failed["ok"], false);
    assert_eq!(failed["error"]["code"], "unauthorizedActor");
    e6e1_query(&state, 1);
}

fn match_config(match_id: &str, first_game: Value) -> Value {
    json!({
        "matchId": match_id,
        "teams": ["a", "b"],
        "format": { "type": "fixedGames", "value": 1 },
        "firstGame": first_game,
        "referees": [
            { "refereeId": "ref", "conflictedTeam": null }
        ],
    })
}

fn pending_match_config(match_id: &str) -> Value {
    match_config(
        match_id,
        json!({ "type": "pending", "reason": "需要可信編排指定先手" }),
    )
}

fn assigned_match_config(match_id: &str) -> Value {
    match_config(match_id, json!({ "type": "assigned", "team": "a" }))
}

fn winner_submission(submission_id: &str, game_id: &str, ruling_id: &str) -> Value {
    json!({
        "submissionId": submission_id,
        "gameRecordId": game_id,
        "rulingRecordId": ruling_id,
        "startKind": { "type": "ordinaryNewGame" },
        "firstPlayer": "a",
        "terminalEvidence": {
            "type": "canonicalGameEnded",
            "conclusion": {
                "outcome": { "type": "winner", "team": "a" },
                "causes": [{ "type": "teamHpDepleted", "teams": ["b"] }]
            }
        },
        "ruling": {
            "type": "gameAccepted",
            "outcome": { "winner": "a" },
        },
    })
}

fn double_loss_submission() -> Value {
    json!({
        "submissionId": "double-loss-submission",
        "gameRecordId": "double-loss-game",
        "rulingRecordId": "double-loss-ruling",
        "startKind": { "type": "ordinaryNewGame" },
        "firstPlayer": "a",
        "terminalEvidence": {
            "type": "timeout",
            "timeoutId": "timeout-double",
            "expiries": [
                { "team": "a", "deadlineMonotonicMs": 15 },
                { "team": "b", "deadlineMonotonicMs": 15 }
            ],
            "observedAtMonotonicMs": 15,
            "clockCheckpointDigest": "digest-double"
        },
        "ruling": {
            "type": "doubleTimeoutLoss",
            "teams": ["a", "b"],
        },
    })
}

fn timeout_submission(submission_id: &str, game_id: &str, ruling_id: &str) -> Value {
    json!({
        "submissionId": submission_id,
        "gameRecordId": game_id,
        "rulingRecordId": ruling_id,
        "startKind": { "type": "ordinaryNewGame" },
        "firstPlayer": "a",
        "terminalEvidence": {
            "type": "timeout",
            "timeoutId": "timeout-a",
            "expiries": [{ "team": "a", "deadlineMonotonicMs": 15 }],
            "observedAtMonotonicMs": 15,
            "clockCheckpointDigest": "digest-a"
        },
        "ruling": {
            "type": "timeoutLoss",
            "loser": "a"
        }
    })
}

fn announcement() -> Value {
    json!({
        "organizer": "organizer",
        "ruleVersion": "5.16",
        "ruleset": "base",
        "enabledRuleModules": [],
        "teamSize": 1,
        "initialHp": 30,
        "registrationMode": "public",
        "personalDecks": "disabled",
        "registrationDeadline": 60000,
        "checkInDeadline": 120000,
        "plan": {
            "initialSeedPolicy": "randomDrawAfterRegistrationDeadline",
            "swissPairingPolicy": "publishedRulesWithPersistedRandomSeed",
            "rankingPolicy": "matchPointsThenOpponentWinsThenOpponentLosses",
            "exactRankingTiePolicy": "organizerOrUnconflictedRefereeWithPublicReason",
            "stages": [{
                "stageId": "stage-1",
                "format": "roundRobin",
                "matchFormat": { "type": "fixedGames", "value": 1 },
                "clockConfig": { "mainTimeMs": 300000, "actionTimeMs": 30000, "outOfTurnChoiceTimeMs": 30000 },
                "swissRounds": null,
                "swissPolicy": null,
                "qualification": { "type": "allEligibleTeams" }
            }]
        },
    })
}

#[test]
fn registration_dispatch_uses_server_actor_and_restores_checkpoint() {
    let created = request(
        json!({ "type": "registrationCreate", "announcement": announcement() }),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(created["ok"], true);
    assert_eq!(created["public"]["lifecycle"], "draft");
    let state = created["serverState"].clone();

    let opened = request(
        json!({
            "type": "registrationExecute",
            "commandId": "open-1",
            "expectedRevision": 0,
            "action": { "type": "open" },
        }),
        organizer(),
        1,
        state,
    );
    assert_eq!(opened["public"]["lifecycle"], "open");
    let restored = request(
        json!({ "type": "registrationView" }),
        player("captain"),
        1,
        opened["serverState"].clone(),
    );
    assert_eq!(restored["ok"], true);
    assert_eq!(restored["public"]["revision"], 1);
    assert_eq!(restored["private"]["publicView"]["revision"], 1);
    assert!(restored["public"].get("ownTeam").is_none());
}

#[test]
fn registration_rejects_player_as_clock_authority_and_errors_are_machine_readable() {
    let result = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": {
                "mainTimeMs": 100,
                "actionTimeMs": 30,
                "outOfTurnChoiceTimeMs": 30,
            },
        }),
        player("a"),
        0,
        empty_state(),
    );
    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "unauthorizedActor");
    assert_eq!(result["error"]["layer"], "authorization");
}

#[test]
fn clock_dispatch_restores_checkpoint_and_preserves_trusted_time_contract() {
    let created = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": {
                "mainTimeMs": 100,
                "actionTimeMs": 30,
                "outOfTurnChoiceTimeMs": 20,
            },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let started = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        0,
        created["serverState"].clone(),
    );
    let observed = request(
        json!({ "type": "clockObserve" }),
        organizer(),
        110,
        started["serverState"].clone(),
    );
    assert_eq!(observed["ok"], true);
    assert_eq!(observed["public"]["result"]["type"], "enteredByoyomi");
    assert_eq!(
        observed["public"]["clock"]["teams"][0]["mainTimeRemainingMs"],
        0
    );
    assert_eq!(observed["public"]["clock"]["nextTimeoutDeadlineMs"], 130);
    assert_eq!(observed["serverState"]["clock"]["version"], 2);
}

#[test]
fn clock_public_view_exposes_the_authoritative_nullable_deadline_in_camel_case() {
    let created = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": {
                "mainTimeMs": 100,
                "actionTimeMs": 30,
                "outOfTurnChoiceTimeMs": 20,
            },
        }),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(created["public"]["nextTimeoutDeadlineMs"], Value::Null);

    let started = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        100,
        created["serverState"].clone(),
    );
    assert_eq!(started["public"]["clock"]["nextTimeoutDeadlineMs"], 230);
    assert!(
        started["public"]["clock"]
            .get("next_timeout_deadline_ms")
            .is_none()
    );

    let observed = request(
        json!({ "type": "clockObserve" }),
        organizer(),
        200,
        started["serverState"].clone(),
    );
    assert_eq!(observed["public"]["clock"]["nextTimeoutDeadlineMs"], 230);
}

#[test]
fn clock_public_view_deadline_boundaries_follow_the_core_window_state() {
    let created = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": { "mainTimeMs": 100, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let same_preparation = request(
        json!({ "type": "clockBeginPreparation" }),
        organizer(),
        0,
        created["serverState"].clone(),
    );
    assert_eq!(
        same_preparation["public"]["clock"]["nextTimeoutDeadlineMs"],
        130
    );

    let mut different_state = created["serverState"].clone();
    different_state["clock"]["lastObservedMs"] = json!(20);
    different_state["clock"]["teams"][0]["mainTimeRemainingMs"] = json!(70);
    different_state["clock"]["teams"][1]["mainTimeRemainingMs"] = json!(80);
    let different_preparation = request(
        json!({ "type": "clockBeginPreparation" }),
        organizer(),
        20,
        different_state,
    );
    assert_eq!(
        different_preparation["public"]["clock"]["nextTimeoutDeadlineMs"],
        120
    );

    let out_of_turn = request(
        json!({ "type": "clockBeginOutOfTurnChoice", "team": "b", "choiceId": "choice" }),
        organizer(),
        100,
        created["serverState"].clone(),
    );
    // 100 開始等待 B：主時間 100 加 choice 讀秒 20，真正到期為 220。
    assert_eq!(out_of_turn["public"]["clock"]["nextTimeoutDeadlineMs"], 220);

    let unlimited = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": { "mainTimeMs": null, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let unlimited_action = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        0,
        unlimited["serverState"].clone(),
    );
    assert_eq!(
        unlimited_action["public"]["clock"]["nextTimeoutDeadlineMs"],
        Value::Null
    );

    // 配置主時間 0 是不限時，不是有限主時間耗盡後的剩餘 0。
    let zero = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": { "mainTimeMs": 0, "actionTimeMs": 0, "outOfTurnChoiceTimeMs": 20 },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let zero_action = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        0,
        zero["serverState"].clone(),
    );
    assert_eq!(
        zero_action["public"]["clock"]["nextTimeoutDeadlineMs"],
        Value::Null
    );

    let zero_preparation = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": { "mainTimeMs": 0, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let prepared_zero = request(
        json!({ "type": "clockBeginPreparation" }),
        organizer(),
        0,
        zero_preparation["serverState"].clone(),
    );
    assert_eq!(
        prepared_zero["public"]["result"],
        json!({ "type": "advanced" })
    );
    assert_eq!(
        prepared_zero["public"]["clock"]["needsDecision"],
        Value::Null
    );
    assert_eq!(
        prepared_zero["public"]["clock"]["nextTimeoutDeadlineMs"],
        Value::Null
    );

    let terminal_start = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": { "mainTimeMs": 1, "actionTimeMs": 1, "outOfTurnChoiceTimeMs": 20 },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let begun = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        0,
        terminal_start["serverState"].clone(),
    );
    let terminal = request(
        json!({ "type": "clockObserve" }),
        organizer(),
        2,
        begun["serverState"].clone(),
    );
    assert_eq!(
        terminal["public"]["clock"]["nextTimeoutDeadlineMs"],
        Value::Null
    );

    let overflow = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": { "mainTimeMs": 1, "actionTimeMs": 1, "outOfTurnChoiceTimeMs": 20 },
        }),
        organizer(),
        u64::MAX,
        empty_state(),
    );
    let overflow_action = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        u64::MAX,
        overflow["serverState"].clone(),
    );
    assert_eq!(overflow_action["ok"], false);
    assert_eq!(overflow_action["error"]["code"], "clockError");
    assert!(
        overflow_action["error"]["message"]
            .as_str()
            .unwrap()
            .contains("timeArithmeticOverflow")
    );
}

#[test]
fn clock_pause_resume_bridge_preserves_paused_checkpoint_and_reason_contract() {
    let created = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": { "mainTimeMs": 100, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let started = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        0,
        created["serverState"].clone(),
    );
    let paused = request(
        json!({ "type": "clockPause", "reason": "裁判處理斷線" }),
        organizer(),
        40,
        started["serverState"].clone(),
    );
    assert_eq!(paused["ok"], true);
    assert_eq!(paused["operation"], "clockPause");
    assert_eq!(paused["public"]["result"]["type"], "paused");
    assert_eq!(paused["public"]["clock"]["pausedWindow"]["type"], "action");
    assert_eq!(
        paused["public"]["clock"]["nextTimeoutDeadlineMs"],
        Value::Null
    );
    assert!(paused["serverState"]["clock"]["running"].is_null());
    assert_eq!(paused["serverState"]["clock"]["paused"]["type"], "action");
    assert_eq!(
        paused["serverState"]["clock"]["teams"][0]["mainTimeRemainingMs"],
        60
    );

    let observed = request(
        json!({ "type": "clockObserve" }),
        organizer(),
        1_000,
        paused["serverState"].clone(),
    );
    assert_eq!(observed["public"]["result"]["type"], "unchanged");
    assert_eq!(
        observed["public"]["clock"]["teams"][0]["mainTimeRemainingMs"],
        60
    );

    let resumed = request(
        json!({ "type": "clockResume", "reason": "裁判處理完成" }),
        organizer(),
        2_000,
        observed["serverState"].clone(),
    );
    assert_eq!(resumed["operation"], "clockResume");
    assert_eq!(resumed["public"]["result"]["type"], "resumed");
    assert!(resumed["serverState"]["clock"]["paused"].is_null());
    assert_eq!(resumed["serverState"]["clock"]["running"]["type"], "action");
}

#[test]
fn clock_pause_resume_requires_a_non_empty_reason() {
    let created = request(
        json!({
            "type": "clockCreate",
            "teams": ["a", "b"],
            "config": { "mainTimeMs": 100, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let started = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        0,
        created["serverState"].clone(),
    );
    let failed = request(
        json!({ "type": "clockPause", "reason": "   " }),
        organizer(),
        1,
        started["serverState"].clone(),
    );
    assert_eq!(failed["ok"], false);
    assert_eq!(failed["error"]["code"], "emptyReason");
}

#[test]
fn invalid_clock_checkpoint_is_rejected_without_partial_restore() {
    let result = request(
        json!({ "type": "clockObserve" }),
        organizer(),
        1,
        json!({
            "registration": null,
            "clock": {
                "version": 999,
                "config": {
                    "mainTimeMs": 100,
                    "actionTimeMs": 30,
                    "outOfTurnChoiceTimeMs": 20,
                },
                "teams": [],
                "running": null,
                "lastObservedMs": 0,
                "nextByoyomiEntrySequence": 0,
                "expiredTeam": null,
                "needsDecision": null,
            },
            "tournament": null,
        }),
    );
    assert_eq!(result["ok"], false);
    assert_eq!(result["error"]["code"], "clockError");
    assert!(
        result["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalidCheckpoint")
    );
}

#[test]
fn tournament_result_with_schedule_error_keeps_committed_state() {
    let created = request(
        json!({
            "type": "tournamentCreate",
            "config": {
                "tournamentId": "event",
                "stage": {
                    "stageId": "stage",
                    "format": "singleElimination",
                    "teams": [
                        { "teamId": "a", "seed": 1 },
                        { "teamId": "b", "seed": 2 },
                    ],
                    "randomSeed": 1,
                    "matchFormat": { "type": "firstToWins", "value": 1 },
                    "clockConfig": {
                        "mainTimeMs": 300000,
                        "actionTimeMs": 30000,
                        "outOfTurnChoiceTimeMs": 30000,
                    },
                    "swissRounds": null,
                    "swissPolicy": null,
                },
            },
        }),
        organizer(),
        0,
        empty_state(),
    );
    let match_id = created["public"]["stages"][0]["rounds"][0]["matches"][0]["matchId"]
        .as_str()
        .unwrap()
        .to_string();
    let submitted = request(
        json!({
            "type": "tournamentSubmitResult",
            "stageId": "stage",
            "matchId": match_id,
            "result": { "type": "doubleLoss" },
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    assert_eq!(submitted["ok"], true);
    assert_eq!(
        submitted["public"]["result"]["type"],
        "appliedWithScheduleError"
    );
    assert!(submitted["public"]["result"]["scheduleError"].is_string());
    assert_eq!(
        submitted["public"]["tournament"]["stages"][0]["status"],
        "active"
    );
    let state = &submitted["serverState"]["tournament"];
    assert_eq!(
        state["stages"][0]["rounds"][0]["matches"][0]["result"]["type"],
        "doubleLoss"
    );
    assert!(state["stages"][0]["lastScheduleError"].is_string());
}

fn tournament_view_create(format: &str, team_count: u32) -> Value {
    let elimination = matches!(format, "singleElimination" | "doubleElimination");
    request(
        json!({
            "type": "tournamentCreate",
            "config": {
                "tournamentId": "current-event",
                "stage": {
                    "stageId": "stage",
                    "format": format,
                    "teams": (1..=team_count).map(|seed| json!({
                        "teamId": format!("team-{seed}"), "seed": seed,
                    })).collect::<Vec<_>>(),
                    "randomSeed": 7,
                    "matchFormat": {
                        "type": if elimination { "firstToWins" } else { "fixedGames" },
                        "value": 1,
                    },
                    "clockConfig": {
                        "mainTimeMs": 300000, "actionTimeMs": 30000,
                        "outOfTurnChoiceTimeMs": 30000,
                    },
                    "swissRounds": if format == "swiss" { json!(2) } else { Value::Null },
                    "swissPolicy": if format == "swiss" { json!({
                        "matchPoints": { "win": 3, "draw": 1, "loss": 0, "bye": 3 },
                        "drawCredit": { "numerator": 1, "denominator": 2 },
                        "opponentWinRateFloorPercent": null,
                        "opponentWinRateCapPercent": null,
                        "excludeByesFromOpponentWinRate": true,
                        "tieBreakers": ["matchPoints", "opponentWinRate", "seedOrder"],
                    }) } else { Value::Null },
                },
            },
        }),
        organizer(),
        0,
        empty_state(),
    )
}

#[test]
fn e6d1a_tournament_view_returns_current_snapshot_for_all_formats_and_actors() {
    for format in [
        "singleElimination",
        "doubleElimination",
        "roundRobin",
        "swiss",
    ] {
        let created = tournament_view_create(format, 4);
        assert_eq!(created["ok"], true, "{format}: {created}");
        let state = created["serverState"].clone();
        for actor in [
            organizer(),
            json!({ "type": "referee", "playerId": "ref" }),
            player("team-1"),
        ] {
            let queried = request(
                json!({ "type": "tournamentView" }),
                actor,
                999,
                state.clone(),
            );
            assert_eq!(queried["ok"], true, "{format}: {queried}");
            assert_eq!(queried["operation"], "tournamentView");
            assert_eq!(queried["public"], created["public"]);
            assert_eq!(queried["serverState"], state);
            assert!(queried.get("private").is_none());
            assert_eq!(queried.as_object().unwrap().len(), 4);
        }
    }
}

fn assert_tournament_view_unchanged(state: &Value) {
    // 查詢時間不觀測任何時鐘，重複查詢也不得重排或推進其他 domain。
    for time in [0, u64::MAX] {
        let queried = request(
            json!({ "type": "tournamentView" }),
            player("reader"),
            time,
            state.clone(),
        );
        assert_eq!(queried["ok"], true, "{queried}");
        assert_eq!(queried["operation"], "tournamentView");
        assert_eq!(queried["public"], state["tournament"]);
        assert_eq!(queried["serverState"], *state);
        assert!(queried.get("private").is_none());
    }
}

fn tournament_view_pending_match(state: &Value) -> Value {
    state["tournament"]["stages"][0]["rounds"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|round| round["matches"].as_array().unwrap())
        .find(|scheduled| scheduled["status"] == "pending")
        .expect("active 賽程必須有下一場 pending 交手")
        .clone()
}

fn tournament_view_win(scheduled: &Value, reverse: bool) -> Value {
    let winner = usize::from(reverse);
    json!({
        "type": "win",
        "winnerId": scheduled["participants"][winner],
        "loserId": scheduled["participants"][1 - winner],
        "winnerGames": 1,
        "loserGames": 0,
    })
}

#[test]
fn e6d1a_tournament_view_preserves_started_corrected_and_finished_snapshots() {
    for format in [
        "singleElimination",
        "doubleElimination",
        "roundRobin",
        "swiss",
    ] {
        let created = tournament_view_create(format, 4);
        assert_eq!(created["ok"], true, "{format}: {created}");
        let mut state = created["serverState"].clone();
        let first = tournament_view_pending_match(&state);
        let started = request(
            json!({ "type": "tournamentStartMatch", "stageId": "stage", "matchId": first["matchId"] }),
            organizer(),
            1,
            state,
        );
        assert_eq!(started["ok"], true, "{format}: {started}");
        state = started["serverState"].clone();
        let started_match = state["tournament"]["stages"][0]["rounds"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|round| round["matches"].as_array().unwrap())
            .find(|scheduled| scheduled["matchId"] == first["matchId"])
            .unwrap();
        assert_eq!(started_match["status"], "inProgress");
        assert_eq!(started_match["started"], true);
        assert_tournament_view_unchanged(&state);

        let submitted = request(
            json!({ "type": "tournamentSubmitResult", "stageId": "stage",
                "matchId": first["matchId"], "result": tournament_view_win(&first, false) }),
            organizer(),
            2,
            state,
        );
        assert_eq!(submitted["ok"], true, "{format}: {submitted}");
        state = submitted["serverState"].clone();
        assert_tournament_view_unchanged(&state);

        // 後續交手尚未開始時正式更正，query 必須保留核心產生的新世代／corrections。
        let corrected = request(
            json!({ "type": "tournamentCorrectResult", "stageId": "stage",
                "matchId": first["matchId"], "replacement": tournament_view_win(&first, true),
                "reason": "裁判更正賽果" }),
            json!({ "type": "referee", "playerId": "ref" }),
            3,
            state,
        );
        assert_eq!(corrected["ok"], true, "{format}: {corrected}");
        state = corrected["serverState"].clone();
        assert_eq!(
            state["tournament"]["stages"][0]["corrections"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
        assert_eq!(
            state["tournament"]["stages"][0]["corrections"][0]["reason"],
            "裁判更正賽果"
        );
        assert_tournament_view_unchanged(&state);

        let mut completed = 1;
        while state["tournament"]["stages"][0]["status"] != "finished" {
            assert!(completed < 20, "{format} 不得因查詢而無限新增賽程");
            let scheduled = tournament_view_pending_match(&state);
            let started = request(
                json!({ "type": "tournamentStartMatch", "stageId": "stage", "matchId": scheduled["matchId"] }),
                organizer(),
                4,
                state,
            );
            assert_eq!(started["ok"], true, "{format}: {started}");
            state = started["serverState"].clone();
            assert_tournament_view_unchanged(&state);
            let submitted = request(
                json!({ "type": "tournamentSubmitResult", "stageId": "stage",
                    "matchId": scheduled["matchId"], "result": tournament_view_win(&scheduled, false) }),
                organizer(),
                5,
                state,
            );
            assert_eq!(submitted["ok"], true, "{format}: {submitted}");
            state = submitted["serverState"].clone();
            assert_tournament_view_unchanged(&state);
            completed += 1;
        }
        assert_eq!(state["tournament"]["stages"][0]["status"], "finished");
        assert_eq!(state["tournament"]["stages"][0]["config"]["randomSeed"], 7);
        assert!(
            state["tournament"]["stages"][0]["standings"]
                .as_array()
                .unwrap()
                .iter()
                .any(|standing| standing["rank"] == 1)
        );
        assert_tournament_view_unchanged(&state);
    }
}

#[test]
fn e6d1a_tournament_view_missing_state_and_malformed_snapshots_fail_closed() {
    for actor in [
        organizer(),
        json!({ "type": "referee", "playerId": "ref" }),
        player("reader"),
    ] {
        assert_eq!(
            request(json!({ "type": "tournamentView" }), actor, 0, empty_state()),
            json!({
                "ok": false,
                "error": { "code": "missingState", "layer": "state", "message": "缺少 tournament snapshot" },
            })
        );
    }
    let created = tournament_view_create("singleElimination", 4);
    assert_eq!(created["ok"], true);
    let valid = created["serverState"].clone();
    let mutations: [fn(&mut Value); 8] = [
        |state| state["tournament"] = json!("not-a-tournament"),
        |state| state["tournament"]["stages"] = json!([]),
        |state| state["tournament"]["stages"][0]["status"] = json!("unknown"),
        |state| state["tournament"]["stages"][0]["config"]["format"] = json!("unknown"),
        |state| state["tournament"]["stages"][0]["config"]["teams"][0]["seed"] = json!(0),
        |state| {
            state["tournament"]["stages"][0]["rounds"][0]["matches"][0]["participants"] =
                json!(["outsider", "team-2"])
        },
        |state| {
            state["tournament"]["stages"][0]["rounds"][0]["matches"][0]["status"] =
                json!("completed")
        },
        |state| {
            let stage = state["tournament"]["stages"][0].clone();
            state["tournament"]["stages"]
                .as_array_mut()
                .unwrap()
                .push(stage);
        },
    ];
    for mutate in mutations {
        let mut malformed = valid.clone();
        mutate(&mut malformed);
        let failed = request(
            json!({ "type": "tournamentView" }),
            player("reader"),
            0,
            malformed,
        );
        assert_eq!(failed["ok"], false, "{failed}");
        assert_eq!(failed["error"]["code"], "invalidRequest");
        assert_eq!(failed["error"]["layer"], "request");
        assert!(failed.get("public").is_none());
        assert!(failed.get("private").is_none());
        assert!(failed.get("serverState").is_none());
    }
}

#[test]
fn e6d1a_tournament_view_wire_is_exact_and_does_not_grant_mutation_authority() {
    use fewfc::competition_web_api::{CompetitionOperation, TournamentViewInput};
    let operation = CompetitionOperation::TournamentView(TournamentViewInput {});
    assert_eq!(
        serde_json::to_value(operation).unwrap(),
        json!({ "type": "tournamentView" })
    );
    let created = tournament_view_create("roundRobin", 2);
    assert_eq!(created["ok"], true);
    for field in [
        "config",
        "stageId",
        "randomSeed",
        "metadata",
        "firstPlayer",
        "serverState",
    ] {
        let mut extra = json!({ "type": "tournamentView" });
        extra[field] = json!({ "spoofed": true });
        let failed = request(extra, player("reader"), 0, created["serverState"].clone());
        assert_eq!(failed["ok"], false, "{field}: {failed}");
        assert_eq!(failed["error"]["code"], "invalidRequest");
        assert_eq!(failed["error"]["layer"], "request");
    }
    let failed = request(
        json!({ "type": "tournamentStartMatch", "stageId": "stage",
            "matchId": created["public"]["stages"][0]["rounds"][0]["matches"][0]["matchId"] }),
        player("reader"),
        0,
        created["serverState"].clone(),
    );
    assert_eq!(failed["ok"], false);
    assert_eq!(failed["error"]["code"], "unauthorizedActor");
    assert_eq!(failed["error"]["layer"], "authorization");
    assert_tournament_view_unchanged(&created["serverState"]);
}

#[test]
fn e6d1a_tournament_view_preserves_registration_clock_and_match_checkpoints_without_private_output()
{
    let registration = request(
        json!({ "type": "registrationCreate", "announcement": announcement() }),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(registration["ok"], true);
    let opened = request(
        json!({ "type": "registrationExecute", "commandId": "open-view-state",
            "expectedRevision": 0, "action": { "type": "open" } }),
        organizer(),
        1,
        registration["serverState"].clone(),
    );
    assert_eq!(opened["ok"], true);
    let clock = request(
        json!({ "type": "clockCreate", "teams": ["a", "b"],
            "config": { "mainTimeMs": 100, "actionTimeMs": 30, "outOfTurnChoiceTimeMs": 20 } }),
        organizer(),
        0,
        opened["serverState"].clone(),
    );
    assert_eq!(clock["ok"], true);
    let running = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        10,
        clock["serverState"].clone(),
    );
    assert_eq!(running["ok"], true);
    let match_created = request(
        json!({ "type": "matchCreate", "config": assigned_match_config("private-match") }),
        organizer(),
        10,
        running["serverState"].clone(),
    );
    assert_eq!(match_created["ok"], true);
    let config = tournament_view_create("roundRobin", 2)["public"]["stages"][0]["config"].clone();
    let created = request(
        json!({ "type": "tournamentCreate",
        "config": { "tournamentId": "current-event", "stage": config } }),
        organizer(),
        10,
        match_created["serverState"].clone(),
    );
    assert_eq!(created["ok"], true);
    assert_tournament_view_unchanged(&created["serverState"]);
    let queried = request(
        json!({ "type": "tournamentView" }),
        player("reader"),
        999,
        created["serverState"].clone(),
    );
    assert_eq!(queried["public"], created["public"]);
    assert_eq!(queried["public"].as_object().unwrap().len(), 2);
    assert_eq!(queried["serverState"], created["serverState"]);
    for field in [
        "private",
        "registration",
        "clock",
        "serverState",
        "deck",
        "cards",
    ] {
        assert!(queried["public"].get(field).is_none());
    }
    assert!(!queried["public"].to_string().contains("private-match"));
    assert!(!queried["public"].to_string().contains("open-view-state"));
    assert!(queried.get("private").is_none());
}

#[test]
fn tournament_stage_settings_use_camel_case_and_reject_mismatched_formats() {
    let created = request(
        json!({
            "type": "tournamentCreate",
            "config": {
                "tournamentId": "event",
                "stage": {
                    "stageId": "stage",
                    "format": "singleElimination",
                    "teams": [
                        { "teamId": "a", "seed": 1 },
                        { "teamId": "b", "seed": 2 },
                    ],
                    "randomSeed": 1,
                    "matchFormat": { "type": "firstToWins", "value": 2 },
                    "clockConfig": {
                        "mainTimeMs": 300000,
                        "actionTimeMs": 30000,
                        "outOfTurnChoiceTimeMs": 30000,
                    },
                    "swissRounds": null,
                    "swissPolicy": null,
                },
            },
        }),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(created["ok"], true);
    let config = &created["public"]["stages"][0]["config"];
    assert_eq!(
        config["matchFormat"],
        json!({ "type": "firstToWins", "value": 2 })
    );
    assert_eq!(config["clockConfig"]["actionTimeMs"], 30000);
    assert!(config.get("matchWinsRequired").is_none());
    assert!(config.get("match_format").is_none());
    assert!(config.get("clock_config").is_none());

    let invalid = request(
        json!({
            "type": "tournamentCreate",
            "config": {
                "tournamentId": "invalid-event",
                "stage": {
                    "stageId": "invalid-stage",
                    "format": "roundRobin",
                    "teams": [
                        { "teamId": "a", "seed": 1 },
                        { "teamId": "b", "seed": 2 },
                    ],
                    "randomSeed": 1,
                    "matchFormat": { "type": "firstToWins", "value": 1 },
                    "clockConfig": {
                        "mainTimeMs": null,
                        "actionTimeMs": null,
                        "outOfTurnChoiceTimeMs": null,
                    },
                    "swissRounds": null,
                    "swissPolicy": null,
                },
            },
        }),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(invalid["ok"], false);
    assert_eq!(invalid["error"]["code"], "scheduleError");
    assert!(
        invalid["error"]["message"]
            .as_str()
            .unwrap()
            .contains("invalidMatchFormat")
    );
}

#[test]
fn match_checkpoints_are_independent_and_restore_by_replay() {
    let first = request(
        json!({ "type": "matchCreate", "config": pending_match_config("m1") }),
        organizer(),
        0,
        empty_state(),
    );
    let second = request(
        json!({ "type": "matchCreate", "config": assigned_match_config("m2") }),
        organizer(),
        0,
        first["serverState"].clone(),
    );
    let before_m2 = second["serverState"]["matches"]["m2"].clone();
    let assigned = request(
        json!({
            "type": "matchAssignFirstPlayer",
            "matchId": "m1",
            "team": "a",
            "reason": "裁判指定",
        }),
        organizer(),
        1,
        second["serverState"].clone(),
    );
    assert_eq!(assigned["ok"], true);
    assert_eq!(
        assigned["serverState"]["matches"]["m1"]["history"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(assigned["serverState"]["matches"]["m2"], before_m2);
    let repeated_assignment = request(
        json!({
            "type": "matchAssignFirstPlayer",
            "matchId": "m1",
            "team": "a",
            "reason": "重試",
        }),
        organizer(),
        1,
        assigned["serverState"].clone(),
    );
    assert_eq!(repeated_assignment["ok"], true);
    assert_eq!(
        repeated_assignment["serverState"]["matches"]["m1"]["history"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let submitted = request(
        json!({
            "type": "matchSubmitGame",
            "matchId": "m1",
            "submission": winner_submission("s1", "g1", "r1"),
        }),
        player("not-an-official"),
        2,
        assigned["serverState"].clone(),
    );
    assert_eq!(submitted["ok"], false);
    assert_eq!(submitted["error"]["code"], "unauthorizedActor");

    let submitted = request(
        json!({
            "type": "matchSubmitGame",
            "matchId": "m1",
            "submission": winner_submission("s1", "g1", "r1"),
        }),
        organizer(),
        2,
        assigned["serverState"].clone(),
    );
    assert_eq!(submitted["ok"], true);
    let viewed = request(
        json!({ "type": "matchView", "matchId": "m1" }),
        player("a"),
        3,
        submitted["serverState"].clone(),
    );
    assert_eq!(viewed["ok"], true);
    assert_eq!(viewed["public"]["match"]["games"][0]["gameRecordId"], "g1");
    assert_eq!(viewed["public"]["match"]["status"], "completed");
}

#[test]
fn failed_match_mutation_does_not_change_checkpoint() {
    let created = request(
        json!({ "type": "matchCreate", "config": pending_match_config("m1") }),
        organizer(),
        0,
        empty_state(),
    );
    let failed = request(
        json!({
            "type": "matchAssignFirstPlayer",
            "matchId": "m1",
            "team": "not-in-match",
            "reason": "bad",
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    assert_eq!(failed["ok"], false);
    let assigned = request(
        json!({
            "type": "matchAssignFirstPlayer",
            "matchId": "m1",
            "team": "a",
            "reason": "裁判指定",
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    let conflicting = request(
        json!({
            "type": "matchAssignFirstPlayer",
            "matchId": "m1",
            "team": "b",
            "reason": "衝突指定",
        }),
        organizer(),
        2,
        assigned["serverState"].clone(),
    );
    assert_eq!(conflicting["ok"], false);
    assert_eq!(conflicting["error"]["code"], "matchError");
    assert!(
        conflicting["error"]["message"]
            .as_str()
            .unwrap()
            .contains("firstPlayerAlreadyAssigned")
    );
    let viewed = request(
        json!({ "type": "matchView", "matchId": "m1" }),
        player("viewer"),
        1,
        created["serverState"].clone(),
    );
    assert_eq!(viewed["ok"], true);
    assert_eq!(viewed["public"]["match"]["status"], "awaitingFirstPlayer");
    assert_eq!(viewed["serverState"]["matches"]["m1"]["history"], json!([]));
}

#[test]
fn match_submission_and_correction_are_idempotent() {
    let created = request(
        json!({ "type": "matchCreate", "config": assigned_match_config("m1") }),
        organizer(),
        0,
        empty_state(),
    );
    let submitted = request(
        json!({
            "type": "matchSubmitGame",
            "matchId": "m1",
            "submission": winner_submission("s1", "g1", "r1"),
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    let repeated = request(
        json!({
            "type": "matchSubmitGame",
            "matchId": "m1",
            "submission": winner_submission("s1", "g1", "r1"),
        }),
        organizer(),
        2,
        submitted["serverState"].clone(),
    );
    assert_eq!(repeated["ok"], true);
    assert_eq!(
        repeated["serverState"]["matches"]["m1"]["history"]
            .as_array()
            .unwrap()
            .len(),
        1
    );

    let correction = json!({
        "type": "matchCorrectRuling",
        "matchId": "m1",
        "correctionId": "c1",
        "refereeId": "ref",
        "gameRecordId": "g1",
        "replacementRulingRecordId": "r2",
        "replacementRuling": {
            "type": "gameAccepted",
            "outcome": { "winner": "a" },
        },
        "reason": "裁判複核",
    });
    let corrected = request(
        correction.clone(),
        json!({ "type": "referee", "playerId": "ref" }),
        3,
        repeated["serverState"].clone(),
    );
    assert_eq!(corrected["ok"], true);
    let repeated_correction = request(
        correction,
        json!({ "type": "referee", "playerId": "ref" }),
        4,
        corrected["serverState"].clone(),
    );
    assert_eq!(repeated_correction["ok"], true);
    assert_eq!(
        repeated_correction["serverState"]["matches"]["m1"]["history"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        repeated_correction["public"]["match"]["corrections"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        repeated_correction["public"]["match"]["games"][0]["terminalEvidence"]["type"],
        "canonicalGameEnded"
    );
    assert_eq!(
        repeated_correction["public"]["match"]["games"][0]["terminalEvidence"]["conclusion"]["outcome"]
            ["type"],
        "winner"
    );
    assert_eq!(
        repeated_correction["public"]["match"]["rulings"][0]["ruling"]["outcome"]["winner"],
        "a"
    );
    assert!(
        repeated_correction["serverState"]["matches"]["m1"]["history"][0]["submission"]
            ["terminalEvidence"]["conclusion"]["outcome"]
            .get("Winner")
            .is_none()
    );
}

#[test]
fn match_authorization_and_referee_conflict_are_enforced() {
    let mut config = assigned_match_config("m1");
    config["referees"][0]["conflictedTeam"] = json!("a");
    let created = request(
        json!({ "type": "matchCreate", "config": config }),
        organizer(),
        0,
        empty_state(),
    );
    let submitted = request(
        json!({
            "type": "matchSubmitGame",
            "matchId": "m1",
            "submission": winner_submission("s1", "g1", "r1"),
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    let mismatch = request(
        json!({
            "type": "matchCorrectRuling",
            "matchId": "m1",
            "correctionId": "c1",
            "refereeId": "body-ref",
            "gameRecordId": "g1",
            "replacementRulingRecordId": "r2",
            "replacementRuling": { "type": "gameAccepted", "outcome": { "winner": "a" } },
            "reason": "複核",
        }),
        json!({ "type": "referee", "playerId": "actual-ref" }),
        2,
        submitted["serverState"].clone(),
    );
    assert_eq!(mismatch["ok"], false);
    assert_eq!(mismatch["error"]["code"], "unauthorizedActor");

    let self_match = request(
        json!({
            "type": "matchCorrectRuling",
            "matchId": "m1",
            "correctionId": "c1",
            "refereeId": "ref",
            "gameRecordId": "g1",
            "replacementRulingRecordId": "r2",
            "replacementRuling": { "type": "gameAccepted", "outcome": { "winner": "a" } },
            "reason": "複核",
        }),
        json!({ "type": "referee", "playerId": "ref" }),
        2,
        submitted["serverState"].clone(),
    );
    assert_eq!(self_match["ok"], false);
    assert_eq!(self_match["error"]["code"], "matchError");
    assert!(
        self_match["error"]["message"]
            .as_str()
            .unwrap()
            .contains("selfMatchRefereeConflict")
    );
}

#[test]
fn double_loss_stays_pending_for_referee() {
    let config = json!({
        "matchId": "m1",
        "teams": ["a", "b"],
        "format": { "type": "firstToWins", "value": 1 },
        "firstGame": { "type": "assigned", "team": "a" },
        "referees": [{ "refereeId": "ref", "conflictedTeam": null }],
    });
    let created = request(
        json!({ "type": "matchCreate", "config": config }),
        organizer(),
        0,
        empty_state(),
    );
    let result = request(
        json!({
            "type": "matchSubmitGame",
            "matchId": "m1",
            "submission": double_loss_submission(),
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    assert_eq!(result["ok"], true);
    assert_eq!(result["public"]["match"]["status"], "awaitingReferee");
    assert_eq!(result["public"]["match"]["outcome"]["type"], "doubleLoss");
    assert_eq!(
        result["public"]["match"]["outcome"]["progression"]["type"],
        "pendingReferee"
    );
    assert!(
        result["public"]["match"]["outcome"]
            .get("winnerId")
            .is_none()
    );
}

#[test]
fn timeout_terminal_evidence_survives_public_view_and_checkpoint_replay() {
    let created = request(
        json!({ "type": "matchCreate", "config": assigned_match_config("m1") }),
        organizer(),
        0,
        empty_state(),
    );
    let submitted = request(
        json!({
            "type": "matchSubmitGame",
            "matchId": "m1",
            "submission": timeout_submission("s1", "g1", "r1"),
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    assert_eq!(submitted["ok"], true);
    let evidence = &submitted["public"]["match"]["games"][0]["terminalEvidence"];
    assert_eq!(evidence["type"], "timeout");
    assert_eq!(evidence["timeoutId"], "timeout-a");
    assert_eq!(evidence["expiries"][0]["deadlineMonotonicMs"], 15);
    assert_eq!(evidence["observedAtMonotonicMs"], 15);
    assert_eq!(evidence["clockCheckpointDigest"], "digest-a");
    assert!(evidence.get("conclusion").is_none());

    let viewed = request(
        json!({ "type": "matchView", "matchId": "m1" }),
        player("viewer"),
        2,
        submitted["serverState"].clone(),
    );
    assert_eq!(
        viewed["public"]["match"]["games"][0]["terminalEvidence"],
        evidence.clone()
    );
    assert_eq!(
        viewed["serverState"]["matches"]["m1"]["history"][0]["submission"]["terminalEvidence"],
        evidence.clone()
    );
}

#[test]
fn mismatched_match_map_identity_fails_closed_for_every_match_lookup() {
    let created = request(
        json!({ "type": "matchCreate", "config": assigned_match_config("m2") }),
        organizer(),
        0,
        empty_state(),
    );
    let submitted = request(
        json!({
            "type": "matchSubmitGame",
            "matchId": "m2",
            "submission": winner_submission("s1", "g1", "r1"),
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    assert_eq!(submitted["ok"], true);

    let mut mismatched = submitted["serverState"].clone();
    mismatched["matches"]["m1"] = mismatched["matches"]["m2"].clone();
    let operations = vec![
        (
            json!({ "type": "matchView", "matchId": "m1" }),
            player("viewer"),
        ),
        (
            json!({
                "type": "matchAssignFirstPlayer",
                "matchId": "m1",
                "team": "a",
                "reason": "重試",
            }),
            organizer(),
        ),
        (
            json!({
                "type": "matchSubmitGame",
                "matchId": "m1",
                "submission": winner_submission("s1", "g1", "r1"),
            }),
            organizer(),
        ),
        (
            json!({
                "type": "matchCorrectRuling",
                "matchId": "m1",
                "correctionId": "c1",
                "refereeId": "ref",
                "gameRecordId": "g1",
                "replacementRulingRecordId": "r2",
                "replacementRuling": {
                    "type": "gameAccepted",
                    "outcome": { "winner": "a" },
                },
                "reason": "複核",
            }),
            json!({ "type": "referee", "playerId": "ref" }),
        ),
    ];

    for (operation, actor) in operations {
        let failed = request(operation, actor, 2, mismatched.clone());
        assert_eq!(failed["ok"], false);
        assert_eq!(failed["error"]["code"], "invalidMatchCheckpoint");

        let healthy = request(
            json!({ "type": "matchView", "matchId": "m2" }),
            player("viewer"),
            2,
            mismatched.clone(),
        );
        assert_eq!(healthy["ok"], true);
        assert_eq!(healthy["public"]["match"]["config"]["matchId"], "m2");
        assert_eq!(
            healthy["serverState"]["matches"]["m2"],
            mismatched["matches"]["m2"]
        );
    }
}

#[test]
fn empty_match_ids_are_rejected_on_create_and_lookup() {
    let created = request(
        json!({ "type": "matchCreate", "config": assigned_match_config("") }),
        organizer(),
        0,
        empty_state(),
    );
    assert_eq!(created["ok"], false);
    assert_eq!(created["error"]["code"], "invalidMatchId");

    let viewed = request(
        json!({ "type": "matchView", "matchId": "" }),
        player("viewer"),
        0,
        empty_state(),
    );
    assert_eq!(viewed["ok"], false);
    assert_eq!(viewed["error"]["code"], "invalidMatchId");
}

#[test]
fn malformed_and_legacy_match_checkpoints_fail_closed_or_restore_empty() {
    let legacy = request(
        json!({ "type": "matchCreate", "config": assigned_match_config("m1") }),
        organizer(),
        0,
        json!({ "registration": null, "clock": null, "tournament": null }),
    );
    assert_eq!(legacy["ok"], true);
    assert_eq!(legacy["serverState"]["matches"]["m1"]["history"], json!([]));

    let malformed = request(
        json!({ "type": "matchView", "matchId": "m1" }),
        player("viewer"),
        1,
        json!({
            "registration": null,
            "clock": null,
            "tournament": null,
            "matches": {
                "m1": {
                    "config": assigned_match_config("m1"),
                    "history": [{
                        "type": "submitGame",
                        "submission": {
                            "submissionId": "s1",
                            "gameRecordId": "g1",
                            "rulingRecordId": "r1",
                            "startKind": { "type": "ordinaryNewGame" },
                            "firstPlayer": "b",
                            "terminalEvidence": {
                                "type": "canonicalGameEnded",
                                "conclusion": {
                                    "outcome": { "type": "winner", "team": "a" },
                                    "causes": [{ "type": "teamHpDepleted", "teams": ["b"] }]
                                }
                            },
                            "ruling": {
                                "type": "gameAccepted",
                                "outcome": { "winner": "a" }
                            }
                        }
                    }]
                }
            }
        }),
    );
    assert_eq!(malformed["ok"], false);
    assert_eq!(malformed["error"]["code"], "invalidMatchCheckpoint");
}

#[test]
fn match_checkpoint_wire_contract_is_camel_case() {
    let created = request(
        json!({ "type": "matchCreate", "config": pending_match_config("m1") }),
        organizer(),
        0,
        empty_state(),
    );
    let assigned = request(
        json!({
            "type": "matchAssignFirstPlayer",
            "matchId": "m1",
            "team": "a",
            "reason": "裁判指定",
        }),
        organizer(),
        1,
        created["serverState"].clone(),
    );
    let wire = assigned.to_string();
    assert!(wire.contains("matchId"));
    assert!(wire.contains("firstGame"));
    assert!(wire.contains("assignFirstPlayer"));
    assert!(!wire.contains("match_id"));
    assert!(!wire.contains("first_game"));
}

#[test]
fn finite_other_team_choice_json_contract_separates_authority_from_readonly_sample() {
    let config = json!({
        "mainTimeMs": 100,
        "actionTimeMs": 30,
        "outOfTurnChoiceTimeMs": 30,
    });
    let created = request(
        json!({ "type": "clockCreate", "teams": ["a", "b"], "config": config }),
        organizer(),
        1_000,
        empty_state(),
    );
    assert_eq!(created["ok"], true, "{created}");
    assert_eq!(created["operation"], "clockCreate");
    assert_eq!(created["public"]["config"], config);
    assert_eq!(created["public"]["lastObservedMs"], 1_000);

    // 每次請求只接回前一公開回應的 serverState，不注入或修改 checkpoint。
    let action = request(
        json!({ "type": "clockBeginAction", "team": "a", "turn": 1 }),
        organizer(),
        1_000,
        created["serverState"].clone(),
    );
    assert_eq!(action["ok"], true, "{action}");
    assert_eq!(action["public"]["result"], json!({ "type": "advanced" }));
    assert_eq!(
        action["public"]["clock"]["activeWindow"],
        json!({ "type": "action", "team": "a", "turn": 1 })
    );
    let ended = request(
        json!({ "type": "clockEndWindow" }),
        organizer(),
        1_010,
        action["serverState"].clone(),
    );
    assert_eq!(ended["ok"], true, "{ended}");
    assert_eq!(ended["public"]["result"], json!({ "type": "advanced" }));
    assert_eq!(ended["public"]["clock"]["activeWindow"], Value::Null);
    let team_a_after_action = json!({
        "team": "a",
        "mainTimeRemainingMs": 90,
        "actionTurn": 1,
        "actionTimeRemainingMs": null,
        "actionByoyomiStarted": false,
        "enteredByoyomi": false,
        "byoyomiEntrySequence": null,
    });
    assert_eq!(ended["public"]["clock"]["teams"][0], team_a_after_action);
    assert_eq!(ended["serverState"]["clock"]["teams"][0], team_a_after_action);

    let choice = request(
        json!({ "type": "clockBeginOutOfTurnChoice", "team": "b", "choiceId": "42" }),
        organizer(),
        1_010,
        ended["serverState"].clone(),
    );
    assert_eq!(choice["ok"], true, "{choice}");
    assert_eq!(choice["operation"], "clockBeginOutOfTurnChoice");
    // 字面政策：B 先用主時間 100，再用獨立 choice 讀秒 30，不是在 1040 到期。
    assert_eq!(choice["public"]["clock"]["nextTimeoutDeadlineMs"], 1_140);
    assert_eq!(
        choice["public"],
        json!({
            "result": { "type": "advanced" },
            "clock": {
                "config": config,
                "teams": [team_a_after_action, {
                    "team": "b",
                    "mainTimeRemainingMs": 100,
                    "actionTurn": null,
                    "actionTimeRemainingMs": null,
                    "actionByoyomiStarted": false,
                    "enteredByoyomi": false,
                    "byoyomiEntrySequence": null,
                }],
                "activeWindow": { "type": "outOfTurnChoice", "team": "b", "choiceId": "42" },
                "pausedWindow": null,
                "expiredTeam": null,
                "needsDecision": null,
                "timeoutRuling": null,
                "byoyomiEntryOrder": "none",
                "lastObservedMs": 1_010,
                "nextTimeoutDeadlineMs": 1_140,
            },
        })
    );
    assert_eq!(choice.as_object().unwrap().len(), 4);
    assert_eq!(choice["serverState"]["clock"]["lastObservedMs"], 1_010);
    assert_eq!(
        choice["serverState"]["clock"]["running"],
        json!({ "type": "outOfTurnChoice", "team": "b", "choiceId": "42", "remainingMs": 30 })
    );

    let entered = request(
        json!({ "type": "clockObserve" }),
        organizer(),
        1_110,
        choice["serverState"].clone(),
    );
    assert_eq!(entered["ok"], true, "{entered}");
    assert_eq!(entered["operation"], "clockObserve");
    let team_b_in_byoyomi = json!({
        "team": "b",
        "mainTimeRemainingMs": 0,
        "actionTurn": null,
        "actionTimeRemainingMs": null,
        "actionByoyomiStarted": false,
        "enteredByoyomi": true,
        "byoyomiEntrySequence": 0,
    });
    assert_eq!(
        entered["public"],
        json!({
            "result": { "type": "enteredByoyomi", "team": "b" },
            "clock": {
                "config": config,
                "teams": [team_a_after_action, team_b_in_byoyomi],
                "activeWindow": { "type": "outOfTurnChoice", "team": "b", "choiceId": "42" },
                "pausedWindow": null,
                "expiredTeam": null,
                "needsDecision": null,
                "timeoutRuling": null,
                "byoyomiEntryOrder": { "first": "b" },
                "lastObservedMs": 1_110,
                "nextTimeoutDeadlineMs": 1_140,
            },
        })
    );
    assert_eq!(entered.as_object().unwrap().len(), 4);
    assert_eq!(entered["serverState"]["clock"]["lastObservedMs"], 1_110);
    assert_eq!(
        entered["serverState"]["clock"]["teams"],
        json!([team_a_after_action, team_b_in_byoyomi])
    );
    assert_eq!(
        entered["serverState"]["clock"]["running"],
        json!({ "type": "outOfTurnChoice", "team": "b", "choiceId": "42", "remainingMs": 30 })
    );

    let observed = request(
        json!({ "type": "clockObserve" }),
        organizer(),
        1_120,
        entered["serverState"].clone(),
    );
    assert_eq!(observed["ok"], true, "{observed}");
    assert_eq!(observed["operation"], "clockObserve");
    assert_eq!(
        observed["public"],
        json!({
            "result": { "type": "advanced" },
            "clock": {
                "config": config,
                "teams": [team_a_after_action, team_b_in_byoyomi],
                "activeWindow": { "type": "outOfTurnChoice", "team": "b", "choiceId": "42" },
                "pausedWindow": null,
                "expiredTeam": null,
                "needsDecision": null,
                "timeoutRuling": null,
                "byoyomiEntryOrder": { "first": "b" },
                "lastObservedMs": 1_120,
                "nextTimeoutDeadlineMs": 1_140,
            },
        })
    );
    assert_eq!(observed.as_object().unwrap().len(), 4);
    let authority_state = observed["serverState"].clone();
    assert_eq!(authority_state["clock"]["lastObservedMs"], 1_120);
    assert_eq!(
        authority_state["clock"]["teams"],
        json!([team_a_after_action, team_b_in_byoyomi])
    );
    assert_eq!(
        authority_state["clock"]["running"],
        json!({ "type": "outOfTurnChoice", "team": "b", "choiceId": "42", "remainingMs": 20 })
    );

    // 較晚的唯讀 sample 可顯示 15，但不能把權威的 1120／20 改成 1125／15。
    let viewed = request(
        json!({ "type": "clockView" }),
        player("reader"),
        1_125,
        authority_state.clone(),
    );
    assert_eq!(viewed["ok"], true, "{viewed}");
    assert_eq!(viewed["operation"], "clockView");
    assert_eq!(
        viewed["public"],
        json!({
            "sampledAtMs": 1_125,
            "teams": [
                { "team": "a", "mainTimeRemainingMs": 90, "byoyomiTimeRemainingMs": 30, "byoyomiActive": false },
                { "team": "b", "mainTimeRemainingMs": 0, "byoyomiTimeRemainingMs": 15, "byoyomiActive": true },
            ],
            "running": { "type": "outOfTurnChoice", "team": "b", "choiceId": "42", "remainingMs": 15 },
            "paused": null,
            "awaitingAuthority": false,
        })
    );
    assert_eq!(viewed.as_object().unwrap().len(), 4);
    assert_eq!(viewed["serverState"], authority_state);
    assert_eq!(viewed["serverState"]["clock"]["lastObservedMs"], 1_120);
    assert_eq!(
        viewed["serverState"]["clock"]["running"],
        json!({ "type": "outOfTurnChoice", "team": "b", "choiceId": "42", "remainingMs": 20 })
    );
}
