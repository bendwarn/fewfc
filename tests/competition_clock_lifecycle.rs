use fewfc::competition_clock::{
    ByoyomiEntryOrder, ClockConfig, ClockError, ClockUpdate, CompetitionClock,
    CompetitionClockCheckpoint, CompetitionRuling, GameResultMode, NewGameTime,
};
use fewfc::domain::TeamId;
use fewfc::domain::{GameConclusion, GameEndCause, GameOutcome};

fn team(id: &str) -> TeamId {
    TeamId::new(id)
}

fn snapshot(clock: &CompetitionClock, id: &str) -> fewfc::competition_clock::TeamClockSnapshot {
    clock
        .snapshot()
        .into_iter()
        .find(|entry| entry.team == team(id))
        .unwrap()
}

fn display_config() -> ClockConfig {
    ClockConfig {
        main_time_ms: Some(100),
        action_time_ms: Some(30),
        out_of_turn_choice_time_ms: Some(20),
    }
}

#[test]
fn e6e1_display_positive_main_zero_action_waits_at_exact_authoritative_deadline() {
    let mut config = display_config();
    config.action_time_ms = Some(0);
    let mut clock = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    assert_eq!(clock.next_timeout_deadline_ms().unwrap(), Some(100));
    let original = clock.checkpoint();
    for (time, remaining, active, waiting) in [
        (99, 1, false, false),
        (100, 0, true, true),
        (101, 0, true, true),
    ] {
        for _ in 0..2 {
            let display = clock.display(time).unwrap();
            assert_eq!(display.sampled_at_ms, time);
            assert_eq!(display.teams[0].main_time_remaining_ms, Some(remaining));
            assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(0));
            assert_eq!(display.teams[0].byoyomi_active, active);
            assert_eq!(display.teams[1].main_time_remaining_ms, Some(100));
            assert_eq!(display.awaiting_authority, waiting);
            assert_eq!(clock.checkpoint(), original);
            assert!(clock.timeout_ruling().is_none());
            assert_eq!(clock.recorded_byoyomi_order(), ByoyomiEntryOrder::None);
            let restored = CompetitionClock::from_checkpoint(original.clone()).unwrap();
            assert_eq!(restored.display(time).unwrap(), display);
            assert_eq!(restored.checkpoint(), original);
        }
    }
}

#[test]
fn e6e1_display_zero_action_budget_without_running_deadline_does_not_wait() {
    let mut config = display_config();
    config.action_time_ms = Some(0);
    let idle = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
    let mut paused = idle.clone();
    paused.begin_action(&team("a"), 1, 0).unwrap();
    paused.pause(99).unwrap();
    let mut stopped = idle.clone();
    stopped.begin_action(&team("a"), 1, 0).unwrap();
    stopped.end_window(99).unwrap();
    for clock in [idle, paused, stopped] {
        assert_eq!(clock.next_timeout_deadline_ms().unwrap(), None);
        let original = clock.checkpoint();
        for time in [100, 101, 10_000] {
            let display = clock.display(time).unwrap();
            assert!(!display.awaiting_authority);
            assert_eq!(
                display.teams[0].main_time_remaining_ms,
                original.teams[0].main_time_remaining_ms
            );
            assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(0));
            assert_eq!(clock.checkpoint(), original);
        }
    }
}

#[test]
fn e6e1_display_action_uses_exact_milliseconds_without_recording_virtual_rulings() {
    let mut clock = CompetitionClock::new([team("a"), team("b")], display_config(), 500).unwrap();
    clock.begin_action(&team("a"), 1, 500).unwrap();
    let original = clock.checkpoint();
    for (time, main, byoyomi, active, waiting) in [
        (550, 50, 30, false, false),
        (599, 1, 30, false, false),
        (600, 0, 30, true, false),
        (610, 0, 20, true, false),
        (630, 0, 0, true, true),
        (10_000, 0, 0, true, true),
    ] {
        let display = clock.display(time).unwrap();
        assert_eq!(display.sampled_at_ms, time);
        assert_eq!(display.teams[0].main_time_remaining_ms, Some(main));
        assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(byoyomi));
        assert_eq!(display.teams[0].byoyomi_active, active);
        assert_eq!(display.teams[1].main_time_remaining_ms, Some(100));
        assert_eq!(display.awaiting_authority, waiting);
        assert_eq!(clock.checkpoint(), original);
        assert!(clock.timeout_ruling().is_none());
        assert_eq!(clock.recorded_byoyomi_order(), ByoyomiEntryOrder::None);
        let restored = CompetitionClock::from_checkpoint(original.clone()).unwrap();
        assert_eq!(restored.display(time).unwrap(), display);
        assert_eq!(restored.checkpoint(), original);
    }
}

#[test]
fn e6e1_display_parallel_preparation_charges_each_waiting_team_once() {
    let mut clock = CompetitionClock::new([team("a"), team("b")], display_config(), 100).unwrap();
    clock.begin_preparation(100).unwrap();
    let before = clock.checkpoint();
    let display = clock.display(145).unwrap();
    assert_eq!(display.teams[0].main_time_remaining_ms, Some(55));
    assert_eq!(display.teams[1].main_time_remaining_ms, Some(55));
    assert_eq!(clock.checkpoint(), before);
    clock.finish_preparation(&team("a"), 125).unwrap();
    let before = clock.checkpoint();
    for (time, remaining, byoyomi, waiting) in
        [(160, 40, 30, false), (220, 0, 10, false), (500, 0, 0, true)]
    {
        let display = clock.display(time).unwrap();
        assert_eq!(display.teams[0].main_time_remaining_ms, Some(75));
        assert_eq!(display.teams[1].main_time_remaining_ms, Some(remaining));
        assert_eq!(display.teams[1].byoyomi_time_remaining_ms, Some(byoyomi));
        assert_eq!(display.awaiting_authority, waiting);
        assert_eq!(clock.checkpoint(), before);
    }
}

#[test]
fn e6e1_display_parallel_deadline_is_only_a_waiting_indicator() {
    let mut config = display_config();
    config.main_time_ms = Some(120);
    let mut previous = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
    previous.begin_action(&team("a"), 1, 0).unwrap();
    previous.end_window(20).unwrap();
    let mut clock = previous.new_game(NewGameTime::CarryForward, 20).unwrap();
    clock.begin_preparation(20).unwrap();
    let before = clock.checkpoint();
    let display = clock.display(220).unwrap();
    assert_eq!(display.teams[0].main_time_remaining_ms, Some(0));
    assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(0));
    assert_eq!(display.teams[1].main_time_remaining_ms, Some(0));
    assert_eq!(display.teams[1].byoyomi_time_remaining_ms, Some(20));
    assert!(display.awaiting_authority);
    assert_eq!(clock.checkpoint(), before);
    assert!(clock.timeout_ruling().is_none());
    let json = serde_json::to_value(display).unwrap();
    assert!(json.get("winner").is_none());
    assert!(json.get("timeoutRuling").is_none());
    assert!(json.get("needsDecision").is_none());

    let mut simultaneous =
        CompetitionClock::new([team("a"), team("b")], display_config(), 0).unwrap();
    simultaneous.begin_preparation(0).unwrap();
    let before = simultaneous.checkpoint();
    let display = simultaneous.display(200).unwrap();
    assert!(display.awaiting_authority);
    assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(0));
    assert_eq!(display.teams[1].byoyomi_time_remaining_ms, Some(0));
    assert_eq!(simultaneous.checkpoint(), before);
    assert!(simultaneous.timeout_ruling().is_none());
}

#[test]
fn e6e1_display_choice_is_independent_and_paused_or_idle_never_charge() {
    let mut clock = CompetitionClock::new([team("a"), team("b")], display_config(), 0).unwrap();
    assert_eq!(
        clock.display(10_000).unwrap().teams[0].main_time_remaining_ms,
        Some(100)
    );
    clock.begin_action(&team("a"), 1, 0).unwrap();
    clock.end_window(110).unwrap();
    let original_action = snapshot(&clock, "a");
    clock
        .begin_out_of_turn_choice(&team("b"), "choice", 110)
        .unwrap();
    let before = clock.checkpoint();
    let display = clock.display(117).unwrap();
    assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(20));
    assert_eq!(display.teams[1].main_time_remaining_ms, Some(93));
    assert_eq!(
        serde_json::to_value(display.running).unwrap(),
        serde_json::json!({
            "type":"outOfTurnChoice", "team":"b", "choiceId":"choice", "remainingMs":20,
        })
    );
    assert_eq!(clock.checkpoint(), before);
    clock.pause(115).unwrap();
    assert_eq!(snapshot(&clock, "a"), original_action);
    assert_eq!(snapshot(&clock, "b").main_time_remaining_ms, Some(95));
    let before = clock.checkpoint();
    let display = clock.display(10_000).unwrap();
    assert!(display.running.is_none());
    assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(20));
    assert_eq!(display.teams[1].main_time_remaining_ms, Some(95));
    assert_eq!(
        serde_json::to_value(display.paused).unwrap()["remainingMs"],
        20
    );
    assert_eq!(clock.checkpoint(), before);
    clock.resume(200).unwrap();
    let before = clock.checkpoint();
    let display = clock.display(203).unwrap();
    assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(20));
    assert_eq!(display.teams[1].main_time_remaining_ms, Some(92));
    assert_eq!(
        serde_json::to_value(display.running).unwrap()["remainingMs"],
        20
    );
    assert_eq!(snapshot(&clock, "a"), original_action);
    assert_eq!(clock.checkpoint(), before);
}

#[test]
fn e6e1_display_zero_unlimited_and_terminal_do_not_guess_pending_rules() {
    for config in [
        ClockConfig {
            main_time_ms: None,
            action_time_ms: None,
            out_of_turn_choice_time_ms: None,
        },
        ClockConfig {
            main_time_ms: Some(0),
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(0),
        },
    ] {
        let mut clock = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
        clock.begin_preparation(0).unwrap();
        let before = clock.checkpoint();
        let display = clock.display(10_000).unwrap();
        assert_eq!(display.teams[0].main_time_remaining_ms, config.main_time_ms);
        assert_eq!(display.teams[1].main_time_remaining_ms, config.main_time_ms);
        assert!(!display.awaiting_authority);
        assert_eq!(clock.needs_decision(), None);
        assert_eq!(clock.checkpoint(), before);
    }
    let mut clock = CompetitionClock::new([team("a"), team("b")], display_config(), 0).unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    clock.observe(130).unwrap();
    let before = clock.checkpoint();
    let display = clock.display(10_000).unwrap();
    assert!(display.awaiting_authority);
    assert_eq!(display.teams[0].byoyomi_time_remaining_ms, Some(0));
    assert_eq!(clock.checkpoint(), before);
    assert_eq!(
        clock.display(129),
        Err(ClockError::MonotonicTimeWentBackwards {
            previous_ms: 130,
            received_ms: 129
        })
    );
}

#[test]
fn e6e1_display_reads_reset_or_carry_forward_without_starting_another_game() {
    let mut previous = CompetitionClock::new([team("a"), team("b")], display_config(), 0).unwrap();
    previous.begin_action(&team("a"), 1, 0).unwrap();
    previous.end_window(23).unwrap();
    for (policy, expected) in [(NewGameTime::Reset, 100), (NewGameTime::CarryForward, 77)] {
        let next = previous.new_game(policy, 50).unwrap();
        let before = next.checkpoint();
        let display = next.display(10_000).unwrap();
        assert_eq!(display.teams[0].main_time_remaining_ms, Some(expected));
        assert_eq!(display.teams[1].main_time_remaining_ms, Some(100));
        assert!(display.running.is_none());
        assert!(display.paused.is_none());
        assert!(!display.awaiting_authority);
        assert_eq!(next.checkpoint(), before);
    }
}

#[test]
fn e6e1_display_preserves_unlimited_byoyomi_and_zero_or_unlimited_choice() {
    let mut config = display_config();
    config.action_time_ms = None;
    let mut clock = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    let before = clock.checkpoint();
    let display = clock.display(10_000).unwrap();
    assert_eq!(display.teams[0].main_time_remaining_ms, Some(0));
    assert_eq!(display.teams[0].byoyomi_time_remaining_ms, None);
    assert!(display.teams[0].byoyomi_active);
    assert!(!display.awaiting_authority);
    assert_eq!(clock.checkpoint(), before);
    for allowance in [None, Some(0)] {
        let mut config = display_config();
        config.out_of_turn_choice_time_ms = allowance;
        let mut choice = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
        choice
            .begin_out_of_turn_choice(&team("b"), "choice", 0)
            .unwrap();
        let before = choice.checkpoint();
        let display = choice.display(10_000).unwrap();
        assert_eq!(display.teams[1].main_time_remaining_ms, Some(100));
        assert_eq!(display.awaiting_authority, allowance == Some(0));
        assert_eq!(
            serde_json::to_value(display.running).unwrap()["remainingMs"],
            serde_json::to_value(allowance).unwrap()
        );
        assert_eq!(choice.checkpoint(), before);
    }
}

#[test]
fn starting_after_idle_does_not_charge_the_idle_interval() {
    let mut clock =
        CompetitionClock::new([team("a"), team("b")], ClockConfig::default(), 0).unwrap();
    clock.begin_action(&team("a"), 1, 100_000).unwrap();
    clock.end_window(110_000).unwrap();
    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, Some(290_000));
    clock.begin_action(&team("a"), 1, 200_000).unwrap();
    clock.end_window(210_000).unwrap();
    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, Some(280_000));
}

#[test]
fn carry_forward_requires_a_stopped_clock_without_byoyomi_entry() {
    let mut clock =
        CompetitionClock::new([team("a"), team("b")], ClockConfig::default(), 0).unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    assert!(clock.new_game(NewGameTime::CarryForward, 1).is_err());
    clock.end_window(300_000).unwrap();
    assert!(clock.new_game(NewGameTime::CarryForward, 300_000).is_err());
}

#[test]
fn checkpoint_restores_an_in_progress_choice_without_resetting_time() {
    let mut clock =
        CompetitionClock::new([team("a"), team("b")], ClockConfig::default(), 0).unwrap();
    clock
        .begin_out_of_turn_choice(&team("b"), "choice", 100)
        .unwrap();
    clock.observe(320_100).unwrap();
    let checkpoint = clock.checkpoint();
    assert_eq!(checkpoint.teams[0].main_time_remaining_ms, Some(300_000));
    assert_eq!(checkpoint.teams[1].main_time_remaining_ms, Some(0));
    assert_eq!(
        serde_json::to_value(&checkpoint).unwrap()["running"],
        serde_json::json!({
            "type":"outOfTurnChoice", "team":"b", "choiceId":"choice", "remainingMs":10_000,
        })
    );
    let wire = serde_json::to_string(&checkpoint).unwrap();
    let mut restored =
        CompetitionClock::from_checkpoint(serde_json::from_str(&wire).unwrap()).unwrap();
    assert_eq!(restored.checkpoint(), checkpoint);
    assert_eq!(restored.expired_team(), None);
    restored.observe(330_100).unwrap();
    assert_eq!(restored.expired_team(), Some(&team("b")));
    assert_eq!(
        snapshot(&restored, "a").main_time_remaining_ms,
        Some(300_000)
    );
}

#[test]
fn system_idle_does_not_charge_and_same_action_turn_keeps_byoyomi() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(20),
        },
        0,
    )
    .unwrap();

    clock.begin_action(&team("a"), 1, 0).unwrap();
    clock.observe(110).unwrap();
    assert!(snapshot(&clock, "a").entered_byoyomi);
    clock.observe(120).unwrap();
    clock.end_window(120).unwrap();

    // 系統解析期間沒有 active window，不扣除原讀秒；斷線本身不應停止視窗。
    clock.observe(1_000).unwrap();
    clock.begin_action(&team("a"), 1, 1_000).unwrap();
    clock.observe(1_005).unwrap();

    assert_eq!(snapshot(&clock, "a").action_time_remaining_ms, Some(5));
    assert!(snapshot(&clock, "a").entered_byoyomi);
    assert_eq!(snapshot(&clock, "a").byoyomi_entry_sequence, Some(0));
}

#[test]
fn choice_polling_accumulates_and_does_not_touch_action_turn() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(20),
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    clock.observe(110).unwrap();
    clock.observe(120).unwrap();
    clock.end_window(120).unwrap();
    let original_action = snapshot(&clock, "a");

    clock
        .begin_out_of_turn_choice(&team("b"), "choice-1", 120)
        .unwrap();
    clock.observe(125).unwrap();
    assert_eq!(snapshot(&clock, "a"), original_action);
    assert_eq!(snapshot(&clock, "b").main_time_remaining_ms, Some(95));
    clock.observe(132).unwrap();
    assert_eq!(snapshot(&clock, "a").action_time_remaining_ms, Some(10));
    assert_eq!(snapshot(&clock, "a"), original_action);
    assert_eq!(snapshot(&clock, "b").main_time_remaining_ms, Some(88));
    assert_eq!(
        clock.observe(220).unwrap(),
        ClockUpdate::EnteredByoyomi { team: team("b") }
    );
    assert_eq!(snapshot(&clock, "b").main_time_remaining_ms, Some(0));
    assert_eq!(snapshot(&clock, "a"), original_action);
    assert_eq!(clock.expired_team(), None);
    assert_eq!(
        serde_json::to_value(clock.checkpoint()).unwrap()["running"],
        serde_json::json!({
            "type":"outOfTurnChoice", "team":"b", "choiceId":"choice-1", "remainingMs":20,
        })
    );
    assert_eq!(
        clock.observe(240).unwrap(),
        ClockUpdate::TimedOut { team: team("b") }
    );
    assert_eq!(snapshot(&clock, "a"), original_action);
}

#[test]
fn choice_then_action_same_turn_does_not_reset_original_byoyomi() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(20),
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    clock.observe(110).unwrap();
    clock.observe(120).unwrap();
    clock.end_window(120).unwrap();

    clock
        .begin_out_of_turn_choice(&team("b"), "choice-1", 1_000)
        .unwrap();
    clock.observe(1_005).unwrap();
    clock.end_window(1_005).unwrap();
    clock.begin_action(&team("a"), 1, 2_000).unwrap();
    clock.observe(2_010).unwrap();

    assert_eq!(snapshot(&clock, "a").action_time_remaining_ms, Some(0));
    assert_eq!(clock.expired_team(), Some(&team("a")));
    assert_eq!(snapshot(&clock, "a").byoyomi_entry_sequence, Some(0));
}

#[test]
fn entry_order_survives_new_turns() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(5),
            action_time_ms: Some(10),
            out_of_turn_choice_time_ms: Some(10),
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    clock.observe(5).unwrap();
    clock.end_window(5).unwrap();
    clock.begin_action(&team("b"), 2, 5).unwrap();
    clock.observe(10).unwrap();
    clock.end_window(10).unwrap();
    clock.begin_action(&team("a"), 3, 10).unwrap();

    assert_eq!(
        clock.recorded_byoyomi_order(),
        ByoyomiEntryOrder::First(team("a"))
    );
    assert_eq!(snapshot(&clock, "a").byoyomi_entry_sequence, Some(0));
    assert_eq!(snapshot(&clock, "b").byoyomi_entry_sequence, Some(1));
}

#[test]
fn timeout_can_be_followed_by_an_ordinary_reset_game() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(10),
            action_time_ms: Some(5),
            out_of_turn_choice_time_ms: Some(5),
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    assert_eq!(
        clock.observe(15).unwrap(),
        ClockUpdate::TimedOut { team: team("a") }
    );
    let reset = clock.new_game(NewGameTime::Reset, 15).unwrap();
    assert_eq!(snapshot(&reset, "a").main_time_remaining_ms, Some(10));
    assert_eq!(reset.expired_team(), None);
}

#[test]
fn unlimited_main_time_ignores_elapsed_even_with_finite_byoyomi() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: None,
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(20),
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    assert_eq!(clock.observe(u64::MAX).unwrap(), ClockUpdate::Advanced);
    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, None);
    assert!(!snapshot(&clock, "a").entered_byoyomi);
}

#[test]
fn finite_main_with_unlimited_byoyomi_enters_once_but_never_times_out() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(10),
            action_time_ms: None,
            out_of_turn_choice_time_ms: Some(20),
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    assert_eq!(
        clock.observe(10).unwrap(),
        ClockUpdate::EnteredByoyomi { team: team("a") }
    );
    assert_eq!(clock.observe(u64::MAX).unwrap(), ClockUpdate::Advanced);
    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, Some(0));
    assert!(snapshot(&clock, "a").entered_byoyomi);
    assert_eq!(snapshot(&clock, "a").action_time_remaining_ms, None);
}

#[test]
fn preparation_charges_each_team_once_until_that_team_finishes() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_preparation(0).unwrap();
    clock.observe(50).unwrap();
    clock.finish_preparation(&team("a"), 50).unwrap();
    clock.observe(80).unwrap();

    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, Some(50));
    assert_eq!(snapshot(&clock, "b").main_time_remaining_ms, Some(20));
    clock.finish_preparation(&team("b"), 80).unwrap();
    assert!(clock.active_window().is_none());
    clock.observe(1_000).unwrap();
    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, Some(50));
    assert_eq!(snapshot(&clock, "b").main_time_remaining_ms, Some(20));
}

#[test]
fn simultaneous_preparation_byoyomi_entry_is_not_a_loss() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_preparation(0).unwrap();
    assert_eq!(clock.observe(100).unwrap(), ClockUpdate::Advanced);
    assert_eq!(
        clock.recorded_byoyomi_order(),
        ByoyomiEntryOrder::Simultaneous
    );
    assert!(clock.timeout_ruling().is_none());
    assert_eq!(clock.observe(129).unwrap(), ClockUpdate::Advanced);
    assert!(clock.timeout_ruling().is_none());
}

#[test]
fn equal_preparation_deadlines_are_double_loss() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_preparation(0).unwrap();
    assert_eq!(
        clock.observe(200).unwrap(),
        ClockUpdate::DoubleTimedOut {
            teams: [team("a"), team("b")]
        }
    );
    assert_eq!(
        clock.timeout_ruling(),
        Some(CompetitionRuling::DoubleTimeoutLoss {
            teams: [team("a"), team("b")]
        })
    );
    assert_eq!(
        clock.recorded_byoyomi_order(),
        ByoyomiEntryOrder::Simultaneous
    );
}

#[test]
fn delayed_observation_crossing_different_deadlines_only_loses_first_team() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_preparation(0).unwrap();
    // 這個 checkpoint 表示 A 已先消耗 20 毫秒；兩隊 deadline 因而不同。
    let mut checkpoint = serde_json::to_value(clock.checkpoint()).unwrap();
    checkpoint["teams"][0]["mainTimeRemainingMs"] = serde_json::json!(80);
    clock = CompetitionClock::from_checkpoint(serde_json::from_value(checkpoint).unwrap()).unwrap();
    // B 的 deadline 是 130；延遲觀測到 200 仍只能判定 A 先在 110 耗盡。
    assert_eq!(
        clock.observe(200).unwrap(),
        ClockUpdate::TimedOut { team: team("a") }
    );
    assert_eq!(
        clock.timeout_ruling(),
        Some(CompetitionRuling::TimeoutLoss { loser: team("a") })
    );
    assert_eq!(snapshot(&clock, "b").main_time_remaining_ms, Some(0));
    assert_eq!(snapshot(&clock, "b").action_time_remaining_ms, Some(20));
}

#[test]
fn preparation_checkpoint_restores_waiting_teams_and_deadlines() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_preparation(0).unwrap();
    clock.observe(50).unwrap();
    clock.finish_preparation(&team("a"), 50).unwrap();
    let checkpoint = serde_json::to_value(clock.checkpoint()).unwrap();
    assert_eq!(checkpoint["preparationStarted"], true);
    assert_eq!(checkpoint["doubleExpired"], false);
    assert_eq!(checkpoint["running"]["type"], "preparation");
    assert!(checkpoint.get("preparation_started").is_none());
    let wire = serde_json::to_string(&checkpoint).unwrap();
    let mut restored =
        CompetitionClock::from_checkpoint(serde_json::from_str(&wire).unwrap()).unwrap();
    restored.finish_preparation(&team("b"), 50).unwrap();
    assert!(restored.active_window().is_none());
    assert_eq!(snapshot(&restored, "a").main_time_remaining_ms, Some(50));
    assert_eq!(snapshot(&restored, "b").main_time_remaining_ms, Some(50));
}

#[test]
fn completed_preparation_can_idle_then_start_action_and_restore_checkpoint() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_preparation(0).unwrap();
    clock.finish_preparation(&team("a"), 110).unwrap();
    clock.finish_preparation(&team("b"), 120).unwrap();

    // 完成準備後的 idle checkpoint 不應攜帶沒有 action turn 的暫時讀秒。
    let idle_checkpoint = clock.checkpoint();
    let mut restored = CompetitionClock::from_checkpoint(idle_checkpoint.clone()).unwrap();
    assert_eq!(restored.checkpoint(), idle_checkpoint);
    assert!(restored.active_window().is_none());
    assert_eq!(snapshot(&restored, "a").action_time_remaining_ms, None);
    assert_eq!(snapshot(&restored, "b").action_time_remaining_ms, None);
    assert!(snapshot(&restored, "a").entered_byoyomi);
    assert!(snapshot(&restored, "b").entered_byoyomi);

    // 下一個行動回合取得新的讀秒額度；這不是準備視窗的餘額延續。
    restored.begin_action(&team("a"), 1, 120).unwrap();
    restored.observe(130).unwrap();
    assert_eq!(snapshot(&restored, "a").action_time_remaining_ms, Some(20));
    let action_checkpoint = restored.checkpoint();
    let mut action_restored = CompetitionClock::from_checkpoint(action_checkpoint.clone()).unwrap();
    assert_eq!(action_restored.checkpoint(), action_checkpoint);
    action_restored.observe(140).unwrap();
    assert_eq!(
        snapshot(&action_restored, "a").action_time_remaining_ms,
        Some(10)
    );
}

#[test]
fn checkpoint_rejects_inconsistent_remaining_and_terminal_facts() {
    let config = ClockConfig {
        main_time_ms: Some(100),
        action_time_ms: Some(30),
        out_of_turn_choice_time_ms: Some(20),
    };
    let clock = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
    let base = serde_json::to_value(clock.checkpoint()).unwrap();

    let mut main_over_limit = base.clone();
    main_over_limit["teams"][0]["mainTimeRemainingMs"] = serde_json::json!(101);
    assert!(
        CompetitionClock::from_checkpoint(serde_json::from_value(main_over_limit).unwrap())
            .is_err()
    );

    let mut finite_main_missing = base.clone();
    finite_main_missing["teams"][0]["mainTimeRemainingMs"] = serde_json::Value::Null;
    assert!(
        CompetitionClock::from_checkpoint(serde_json::from_value(finite_main_missing).unwrap())
            .is_err()
    );

    let mut main_and_byoyomi_conflict = base.clone();
    main_and_byoyomi_conflict["teams"][0]["enteredByoyomi"] = serde_json::json!(true);
    main_and_byoyomi_conflict["teams"][0]["byoyomiEntrySequence"] = serde_json::json!(0);
    main_and_byoyomi_conflict["nextByoyomiEntrySequence"] = serde_json::json!(1);
    assert!(
        CompetitionClock::from_checkpoint(
            serde_json::from_value(main_and_byoyomi_conflict).unwrap()
        )
        .is_err()
    );

    let mut active_action = clock.clone();
    active_action.begin_action(&team("a"), 1, 0).unwrap();
    active_action.observe(110).unwrap();
    let action_checkpoint = serde_json::to_value(active_action.checkpoint()).unwrap();
    let mut active_action_missing = action_checkpoint.clone();
    active_action_missing["teams"][0]["actionTimeRemainingMs"] = serde_json::Value::Null;
    assert!(
        CompetitionClock::from_checkpoint(serde_json::from_value(active_action_missing).unwrap())
            .is_err()
    );
    let mut active_action_over_limit = action_checkpoint;
    active_action_over_limit["teams"][0]["actionTimeRemainingMs"] = serde_json::json!(31);
    assert!(
        CompetitionClock::from_checkpoint(
            serde_json::from_value(active_action_over_limit).unwrap()
        )
        .is_err()
    );

    let mut active_choice = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
    active_choice
        .begin_out_of_turn_choice(&team("b"), "choice", 0)
        .unwrap();
    let mut choice_missing = serde_json::to_value(active_choice.checkpoint()).unwrap();
    choice_missing["running"]["remainingMs"] = serde_json::Value::Null;
    assert!(
        CompetitionClock::from_checkpoint(serde_json::from_value(choice_missing).unwrap()).is_err()
    );

    let mut expired = clock.clone();
    expired.begin_action(&team("a"), 1, 0).unwrap();
    expired.observe(130).unwrap();
    let mut expired_inconsistent = serde_json::to_value(expired.checkpoint()).unwrap();
    expired_inconsistent["teams"][0]["actionTimeRemainingMs"] = serde_json::json!(1);
    assert!(
        CompetitionClock::from_checkpoint(serde_json::from_value(expired_inconsistent).unwrap())
            .is_err()
    );

    let mut double_expired = clock;
    double_expired.begin_preparation(0).unwrap();
    double_expired.observe(130).unwrap();
    assert!(CompetitionClock::from_checkpoint(double_expired.checkpoint()).is_ok());
    let double_checkpoint = serde_json::to_value(double_expired.checkpoint()).unwrap();
    let mut double_with_decision = double_checkpoint.clone();
    double_with_decision["needsDecision"] = serde_json::json!("zeroTimeControl");
    assert!(
        CompetitionClock::from_checkpoint(serde_json::from_value(double_with_decision).unwrap())
            .is_err()
    );
    let mut double_with_partial_expiry = double_checkpoint;
    double_with_partial_expiry["teams"][0]["actionTimeRemainingMs"] = serde_json::json!(1);
    assert!(
        CompetitionClock::from_checkpoint(
            serde_json::from_value(double_with_partial_expiry).unwrap()
        )
        .is_err()
    );
}

#[test]
fn pause_resume_action_window_freezes_time_and_resets_the_baseline() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(20),
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    assert_eq!(clock.pause(40).unwrap(), ClockUpdate::Paused);
    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, Some(60));
    assert_eq!(clock.observe(1_000).unwrap(), ClockUpdate::Unchanged);
    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, Some(60));
    assert_eq!(clock.resume(2_000).unwrap(), ClockUpdate::Resumed);
    assert_eq!(clock.observe(2_010).unwrap(), ClockUpdate::Advanced);
    assert_eq!(snapshot(&clock, "a").main_time_remaining_ms, Some(50));
    assert_eq!(clock.pause(2_010).unwrap(), ClockUpdate::Paused);
    assert_eq!(clock.pause(2_010), Err(ClockError::ClockAlreadyPaused));
    assert_eq!(
        clock.begin_action(&team("a"), 1, 2_010),
        Err(ClockError::ClockIsPaused)
    );
    assert_eq!(clock.end_window(2_010), Err(ClockError::ClockIsPaused));
    assert_eq!(
        clock.new_game(NewGameTime::Reset, 2_010),
        Err(ClockError::ClockIsPaused)
    );
}

#[test]
fn pause_resume_preserves_preparation_and_out_of_turn_windows() {
    let mut preparation = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    preparation.begin_preparation(0).unwrap();
    preparation.pause(40).unwrap();
    assert_eq!(preparation.observe(1_000).unwrap(), ClockUpdate::Unchanged);
    preparation.resume(1_000).unwrap();
    preparation.finish_preparation(&team("a"), 1_000).unwrap();
    assert_eq!(snapshot(&preparation, "a").main_time_remaining_ms, Some(60));

    let mut choice = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            out_of_turn_choice_time_ms: Some(20),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    choice
        .begin_out_of_turn_choice(&team("b"), "choice", 0)
        .unwrap();
    choice.pause(5).unwrap();
    let checkpoint = choice.checkpoint();
    assert_eq!(snapshot(&choice, "a").main_time_remaining_ms, Some(300_000));
    assert_eq!(snapshot(&choice, "b").main_time_remaining_ms, Some(299_995));
    let mut restored = CompetitionClock::from_checkpoint(checkpoint.clone()).unwrap();
    assert_eq!(restored.checkpoint(), checkpoint);
    assert_eq!(restored.observe(50), Ok(ClockUpdate::Unchanged));
    assert_eq!(snapshot(&restored, "b").main_time_remaining_ms, Some(299_995));
    restored.resume(100).unwrap();
    assert_eq!(restored.next_timeout_deadline_ms(), Ok(Some(300_115)));
    assert_eq!(restored.observe(115), Ok(ClockUpdate::Advanced));
    assert_eq!(snapshot(&restored, "b").main_time_remaining_ms, Some(299_980));
    assert_eq!(restored.expired_team(), None);
    assert_eq!(
        restored.observe(300_095),
        Ok(ClockUpdate::EnteredByoyomi { team: team("b") })
    );
    assert_eq!(restored.observe(300_114), Ok(ClockUpdate::Advanced));
    assert_eq!(restored.expired_team(), None);
    assert_eq!(
        restored.observe(300_115),
        Ok(ClockUpdate::TimedOut { team: team("b") })
    );
    assert_eq!(snapshot(&restored, "a").main_time_remaining_ms, Some(300_000));
}

#[test]
fn next_timeout_deadline_uses_the_earliest_preparation_team_without_ruling() {
    let mut same = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    same.begin_preparation(0).unwrap();
    assert_eq!(same.next_timeout_deadline_ms(), Ok(Some(130)));

    let mut different_checkpoint = same.checkpoint();
    different_checkpoint.last_observed_ms = 20;
    different_checkpoint.teams[0].main_time_remaining_ms = Some(70);
    different_checkpoint.teams[1].main_time_remaining_ms = Some(80);
    let different = CompetitionClock::from_checkpoint(different_checkpoint).unwrap();
    assert_eq!(different.next_timeout_deadline_ms(), Ok(Some(120)));
    assert!(different.timeout_ruling().is_none());
}

#[test]
fn zero_main_time_preparation_is_unlimited_without_a_timeout_deadline() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(0),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();

    assert_eq!(clock.begin_preparation(0).unwrap(), ClockUpdate::Advanced);
    assert_eq!(clock.needs_decision(), None);
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(None));
    let checkpoint = clock.checkpoint();
    let encoded = serde_json::to_string(&checkpoint).unwrap();
    let decoded: CompetitionClockCheckpoint = serde_json::from_str(&encoded).unwrap();
    assert_eq!(decoded, checkpoint);
    assert_eq!(
        CompetitionClock::from_checkpoint(decoded)
            .unwrap()
            .checkpoint(),
        checkpoint
    );
}

#[test]
fn next_timeout_deadline_keeps_an_active_action_deadline_absolute() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 100).unwrap();
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(230)));
    clock.observe(200).unwrap();
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(230)));
    clock.observe(210).unwrap();
    assert_eq!(clock.next_timeout_deadline_ms(), Ok(Some(230)));
}

#[test]
fn next_timeout_deadline_covers_out_of_turn_and_non_timeout_boundaries() {
    let mut choice = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            out_of_turn_choice_time_ms: Some(20),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    choice
        .begin_out_of_turn_choice(&team("b"), "choice", 100)
        .unwrap();
    assert_eq!(choice.next_timeout_deadline_ms(), Ok(Some(300_120)));

    choice.pause(110).unwrap();
    assert_eq!(choice.next_timeout_deadline_ms(), Ok(None));

    let unlimited = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: None,
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    assert_eq!(unlimited.next_timeout_deadline_ms(), Ok(None));

    let mut decision = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(0),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    decision.begin_action(&team("a"), 1, 0).unwrap();
    assert_eq!(decision.next_timeout_deadline_ms(), Ok(None));
}

#[test]
fn next_timeout_deadline_is_none_after_terminal_state_and_overflow_fails_closed() {
    let mut terminal = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(10),
            action_time_ms: Some(5),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    terminal.begin_action(&team("a"), 1, 0).unwrap();
    terminal.observe(15).unwrap();
    assert_eq!(terminal.next_timeout_deadline_ms(), Ok(None));

    let mut overflow = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(1),
            action_time_ms: Some(1),
            ..ClockConfig::default()
        },
        u64::MAX - 1,
    )
    .unwrap();
    overflow.begin_action(&team("a"), 1, u64::MAX - 1).unwrap();
    assert_eq!(
        overflow.next_timeout_deadline_ms(),
        Err(ClockError::TimeArithmeticOverflow)
    );
}

#[test]
fn pause_at_deadline_commits_timeout_instead_of_rescuing_the_clock() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(10),
            action_time_ms: Some(5),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    assert_eq!(
        clock.pause(15).unwrap(),
        ClockUpdate::TimedOut { team: team("a") }
    );
    assert!(clock.paused_window().is_none());
    assert!(matches!(
        clock.resume(100),
        Err(ClockError::ClockAlreadyExpired(_))
    ));
    assert_eq!(
        clock.timeout_ruling(),
        Some(CompetitionRuling::TimeoutLoss { loser: team("a") })
    );
}

#[test]
fn paused_byoyomi_checkpoint_keeps_entry_sequence_and_rule_game_is_read_only() {
    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(10),
            action_time_ms: Some(30),
            ..ClockConfig::default()
        },
        0,
    )
    .unwrap();
    clock.begin_action(&team("a"), 1, 0).unwrap();
    clock.pause(10).unwrap();
    let before = clock.checkpoint();
    let restored = CompetitionClock::from_checkpoint(before.clone()).unwrap();
    assert_eq!(restored.checkpoint(), before);
    assert_eq!(snapshot(&restored, "a").byoyomi_entry_sequence, Some(0));

    let conclusion = GameConclusion::new(
        GameOutcome::Winner(team("a")),
        vec![GameEndCause::DirectVictory {
            rule: "test".into(),
            team: team("a"),
        }],
    );
    assert_eq!(
        restored.rule_game(&conclusion, GameResultMode::RetainDraw),
        Ok(CompetitionRuling::GameAccepted {
            outcome: fewfc::competition_clock::RulingOutcome::Winner(team("a")),
        })
    );
}

#[test]
fn paused_other_team_out_of_turn_choice_display_preserves_independent_byoyomi_after_restore() {
    use fewfc::competition_clock::CheckpointWindow;

    let mut clock = CompetitionClock::new(
        [team("a"), team("b")],
        ClockConfig {
            main_time_ms: Some(100),
            action_time_ms: Some(30),
            out_of_turn_choice_time_ms: Some(30),
        },
        1_000,
    )
    .unwrap();
    assert_eq!(
        clock.begin_action(&team("a"), 1, 1_000),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(clock.end_window(1_010), Ok(ClockUpdate::Advanced));
    let original_action = snapshot(&clock, "a");
    assert_eq!(original_action.main_time_remaining_ms, Some(90));
    assert_eq!(
        clock.begin_out_of_turn_choice(&team("b"), "paused-choice-1", 1_010),
        Ok(ClockUpdate::Advanced)
    );
    assert_eq!(
        clock.observe(1_110),
        Ok(ClockUpdate::EnteredByoyomi { team: team("b") })
    );
    assert_eq!(clock.observe(1_120), Ok(ClockUpdate::Advanced));
    let choice_window = CheckpointWindow::OutOfTurnChoice {
        team: team("b"),
        choice_id: "paused-choice-1".to_owned(),
        remaining_ms: Some(20),
    };
    assert_eq!(clock.checkpoint().running, Some(choice_window.clone()));
    assert_eq!(clock.pause(1_120), Ok(ClockUpdate::Paused));

    // 只序列化公開介面產生的真實 paused checkpoint，不自行改造 state 或 fixture。
    let checkpoint = clock.checkpoint();
    assert_eq!(checkpoint.running, None);
    assert_eq!(checkpoint.paused, Some(choice_window.clone()));
    let wire = serde_json::to_string(&checkpoint).unwrap();
    let decoded: CompetitionClockCheckpoint = serde_json::from_str(&wire).unwrap();
    assert_eq!(decoded, checkpoint);
    let restored = CompetitionClock::from_checkpoint(decoded).unwrap();
    assert_eq!(restored.checkpoint(), checkpoint);
    assert_eq!(snapshot(&restored, "a"), original_action);
    let restored_b = snapshot(&restored, "b");
    assert_eq!(restored_b.main_time_remaining_ms, Some(0));
    assert!(restored_b.entered_byoyomi);
    assert_eq!(restored_b.action_turn, None);
    assert_eq!(restored_b.action_time_remaining_ms, None);
    assert!(!restored_b.action_byoyomi_started);

    let before_display = restored.checkpoint();
    let display = restored.display(5_000).unwrap();
    assert_eq!(restored.checkpoint(), before_display);
    assert_eq!(snapshot(&restored, "a"), original_action);
    assert_eq!(snapshot(&restored, "b"), restored_b);
    assert_eq!(display.running, None);
    assert_eq!(display.paused, Some(choice_window));
    assert_eq!(display.teams[0].main_time_remaining_ms, Some(90));
    assert_eq!(display.teams[1].main_time_remaining_ms, Some(0));
    assert!(!display.awaiting_authority);

    // F1 第一個投影斷言：暫停仍保留 choice 的讀秒狀態，不能改取 action 設定 30。
    assert_eq!(
        display.teams[1].byoyomi_time_remaining_ms,
        Some(20),
        "paused 他隊 choice 應顯示獨立讀秒 20，不是 action 設定 30"
    );
    assert!(display.teams[1].byoyomi_active);
}
