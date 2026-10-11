use fewfc::competition_clock::{ClockConfig, CompetitionClock};
use fewfc::domain::TeamId;

#[test]
fn completed_preparation_checkpoint_round_trips_after_byoyomi_entry() {
    let a = TeamId::new("a");
    let b = TeamId::new("b");
    let mut clock = CompetitionClock::new(
        [a.clone(), b.clone()],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(30),
        },
        0,
    )
    .unwrap();
    clock.begin_preparation(0).unwrap();
    clock.finish_preparation(&a, 110).unwrap();
    clock.finish_preparation(&b, 120).unwrap();
    let checkpoint = clock.checkpoint();
    let restored = CompetitionClock::from_checkpoint(checkpoint.clone())
        .expect("合法完成準備後的 checkpoint 必須能恢复");
    assert_eq!(restored.checkpoint(), checkpoint);
}

#[test]
fn version_one_clock_checkpoint_upgrades_without_paused_state() {
    let teams = [TeamId::new("a"), TeamId::new("b")];
    let clock = CompetitionClock::new(teams, ClockConfig::default(), 0).unwrap();
    let mut legacy = serde_json::to_value(clock.checkpoint()).unwrap();
    legacy["version"] = serde_json::json!(1);
    legacy.as_object_mut().unwrap().remove("paused");

    let restored = CompetitionClock::from_checkpoint(serde_json::from_value(legacy).unwrap())
        .expect("既有 v1 checkpoint 應可升級恢復");
    assert_eq!(restored.checkpoint().version, 2);
    assert_eq!(restored.checkpoint().paused, None);
}

#[test]
fn version_one_checkpoint_with_paused_state_fails_closed() {
    let clock = CompetitionClock::new(
        [TeamId::new("a"), TeamId::new("b")],
        ClockConfig::default(),
        0,
    )
    .unwrap();
    let mut legacy = serde_json::to_value(clock.checkpoint()).unwrap();
    legacy["version"] = serde_json::json!(1);
    legacy["paused"] = serde_json::json!({ "type": "action", "team": "a", "turn": 1 });

    assert!(CompetitionClock::from_checkpoint(serde_json::from_value(legacy).unwrap()).is_err());
}
