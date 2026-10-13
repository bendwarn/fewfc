//! 競賽對局的純 Rust 隊伍計時與競賽判定。
//!
//! 本模組只保存計時事實與競賽層判定。它不讀取 wall clock、不寫入遊戲狀態，
//! 也不修改既有的 `GameConclusion`；網站或權威服務可在外層把這些純函式結果
//! 轉成自己的持久化記錄。

use crate::domain::{GameConclusion, GameOutcome, TeamId};
use serde::{Deserialize, Serialize};

const DEFAULT_MAIN_TIME_MS: u64 = 5 * 60 * 1_000;
const DEFAULT_ACTION_TIME_MS: u64 = 30 * 1_000;
const DEFAULT_OUT_OF_TURN_CHOICE_TIME_MS: u64 = 30 * 1_000;

/// 一段時間額度。`None` 表示不限時；`Some(0)` 表示零額度。
/// 例外：主時間設定的 `Some(0)` 是不限時哨兵；有限主時間耗盡後的剩餘
/// `Some(0)` 仍表示已耗盡。
pub type TimeLimit = Option<u64>;

/// 競賽時鐘設定。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClockConfig {
    /// 每隊每局的主時間，預設五分鐘。設定 `Some(0)` 表示不限時；有限主時間耗盡
    /// 後的剩餘 `Some(0)` 仍表示已耗盡。
    pub main_time_ms: TimeLimit,
    /// 每個行動回合的讀秒，預設三十秒。
    pub action_time_ms: TimeLimit,
    /// 回合外選擇的獨立額度，預設三十秒。
    pub out_of_turn_choice_time_ms: TimeLimit,
}

impl Default for ClockConfig {
    fn default() -> Self {
        Self {
            main_time_ms: Some(DEFAULT_MAIN_TIME_MS),
            action_time_ms: Some(DEFAULT_ACTION_TIME_MS),
            out_of_turn_choice_time_ms: Some(DEFAULT_OUT_OF_TURN_CHOICE_TIME_MS),
        }
    }
}

/// 一隊在時鐘上的公開快照。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamClockSnapshot {
    pub team: TeamId,
    pub main_time_remaining_ms: TimeLimit,
    pub action_turn: Option<u64>,
    pub action_time_remaining_ms: TimeLimit,
    /// 目前 action turn 是否已開始讀秒；`entered_byoyomi` 是整局歷史事實。
    pub action_byoyomi_started: bool,
    pub entered_byoyomi: bool,
    pub byoyomi_entry_sequence: Option<u64>,
}

/// 唯讀的短期顯示取樣，不是可恢復的 checkpoint 或競賽裁判結果。
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockDisplay {
    pub sampled_at_ms: u64,
    pub teams: Vec<ClockDisplayTeam>,
    pub running: Option<CheckpointWindow>,
    pub paused: Option<CheckpointWindow>,
    /// 額度耗盡或既有時計已停止裁判流程時，顯示層須等待權威確認。
    pub awaiting_authority: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ClockDisplayTeam {
    pub team: TeamId,
    pub main_time_remaining_ms: TimeLimit,
    /// 尚未進入讀秒時顯示設定額度；不限時維持 None，不能誤作零。
    pub byoyomi_time_remaining_ms: TimeLimit,
    pub byoyomi_active: bool,
}

/// 時鐘目前是否正在扣除某種額度。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ActiveWindow<'a> {
    Preparation {
        teams: &'a [TeamId],
    },
    Action {
        team: &'a TeamId,
        turn: u64,
    },
    OutOfTurnChoice {
        team: &'a TeamId,
        choice_id: &'a str,
    },
}

/// 時鐘更新後的狀態。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClockUpdate {
    Unchanged,
    Advanced,
    Paused,
    Resumed,
    EnteredByoyomi { team: TeamId },
    TimedOut { team: TeamId },
    DoubleTimedOut { teams: [TeamId; 2] },
    NeedsDecision { reason: NeedsDecisionReason },
}

/// 仍不能由時計自動決定的情況。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NeedsDecisionReason {
    ZeroTimeControl,
    SimultaneousByoyomiEntry,
    UnlimitedEliminationDraw,
    ZeroTimeEliminationDraw,
}

/// 計時器的錯誤。錯誤不會提交任何候選狀態。
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ClockError {
    TeamNotRegistered(TeamId),
    TeamsMustBeDistinct,
    ExactlyTwoTeamsRequired,
    InvalidConfig(&'static str),
    MonotonicTimeWentBackwards {
        previous_ms: u64,
        received_ms: u64,
    },
    TimeArithmeticOverflow,
    WindowAlreadyActive,
    NoActiveWindow,
    ActiveWindowMismatch,
    ActionTurnWentBackwards {
        previous_turn: u64,
        received_turn: u64,
    },
    ChoiceIdEmpty,
    ClockAlreadyExpired(TeamId),
    ClockAlreadyDoubleExpired,
    ClockAlreadyPaused,
    ClockNotPaused,
    ClockIsPaused,
    PreparationNotComplete,
    CannotCarryForwardAfterDecision,
    InvalidCheckpoint(&'static str),
}

/// 為下一局建立時鐘時，主時間採用的來源。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum NewGameTime {
    /// 普通新局：依設定重置每隊主時間。
    Reset,
    /// 淘汰賽和局加賽：沿用前一局結束時的主時間。
    CarryForward,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TeamClockState {
    team: TeamId,
    main_time_remaining_ms: TimeLimit,
    action_turn: Option<u64>,
    action_time_remaining_ms: TimeLimit,
    action_byoyomi_started: bool,
    entered_byoyomi: bool,
    byoyomi_entry_sequence: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum RunningWindow {
    Preparation {
        teams: Vec<TeamId>,
    },
    Action {
        team: TeamId,
        turn: u64,
    },
    OutOfTurnChoice {
        team: TeamId,
        choice_id: String,
        remaining_ms: TimeLimit,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct ClockState {
    preparation_started: bool,
    double_expired: bool,
    teams: Vec<TeamClockState>,
    running: Option<RunningWindow>,
    paused: Option<RunningWindow>,
    last_observed_ms: u64,
    next_entry_sequence: u64,
    expired_team: Option<TeamId>,
    needs_decision: Option<NeedsDecisionReason>,
}

/// 兩隊共用主時間、各自記錄讀秒進入順序的純時鐘。
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CompetitionClock {
    config: ClockConfig,
    state: ClockState,
}

impl CompetitionClock {
    /// 建立一局的新時鐘。建立時的毫秒值只是單調時間基準，不是 wall clock。
    pub fn new(
        teams: [TeamId; 2],
        config: ClockConfig,
        monotonic_ms: u64,
    ) -> Result<Self, ClockError> {
        validate_config(config)?;
        if teams[0] == teams[1] {
            return Err(ClockError::TeamsMustBeDistinct);
        }
        Ok(Self {
            config,
            state: ClockState {
                preparation_started: false,
                double_expired: false,
                teams: teams
                    .into_iter()
                    .map(|team| TeamClockState {
                        team,
                        main_time_remaining_ms: config.main_time_ms,
                        action_turn: None,
                        action_time_remaining_ms: None,
                        action_byoyomi_started: false,
                        entered_byoyomi: false,
                        byoyomi_entry_sequence: None,
                    })
                    .collect(),
                running: None,
                paused: None,
                last_observed_ms: monotonic_ms,
                next_entry_sequence: 0,
                expired_team: None,
                needs_decision: None,
            },
        })
    }

    pub fn config(&self) -> ClockConfig {
        self.config
    }

    pub fn last_observed_ms(&self) -> u64 {
        self.state.last_observed_ms
    }

    pub fn snapshot(&self) -> Vec<TeamClockSnapshot> {
        self.state
            .teams
            .iter()
            .map(|team| TeamClockSnapshot {
                team: team.team.clone(),
                main_time_remaining_ms: team.main_time_remaining_ms,
                action_turn: team.action_turn,
                action_time_remaining_ms: team.action_time_remaining_ms,
                action_byoyomi_started: team.action_byoyomi_started,
                entered_byoyomi: team.entered_byoyomi,
                byoyomi_entry_sequence: team.byoyomi_entry_sequence,
            })
            .collect()
    }

    /// 沿用核心扣時規則，但所有觀測、讀秒排序及到期標記只存在獨立 clone。
    /// 不輸出虛擬 timeout ruling 或未決原因，也不讓顯示查詢推進正式時計。
    pub fn display(&self, monotonic_ms: u64) -> Result<ClockDisplay, ClockError> {
        let mut sampled = self.clone();
        sampled.observe(monotonic_ms)?;
        // 精確到達權威 running deadline 就停止顯示倒數；不依賴 observe 是否已產生裁判旗標。
        // paused／idle 的 deadline 為 None，不能只因未使用的讀秒額度是零而等待。
        let deadline_reached = self
            .next_timeout_deadline_ms()?
            .is_some_and(|deadline| monotonic_ms >= deadline);
        Ok(ClockDisplay {
            sampled_at_ms: monotonic_ms,
            teams: sampled
                .state
                .teams
                .iter()
                .map(|team| {
                    let choice_byoyomi = match sampled
                        .state
                        .running
                        .as_ref()
                        .or(sampled.state.paused.as_ref())
                    {
                        Some(RunningWindow::OutOfTurnChoice {
                            team: deciding_team,
                            remaining_ms: Some(remaining),
                            ..
                        }) if deciding_team == &team.team
                            && !sampled.is_main_time_configuration_unlimited()
                            && team.main_time_remaining_ms == Some(0) => Some(*remaining),
                        _ => None,
                    };
                    ClockDisplayTeam {
                        team: team.team.clone(),
                        main_time_remaining_ms: team.main_time_remaining_ms,
                        byoyomi_time_remaining_ms: if choice_byoyomi.is_some() {
                            choice_byoyomi
                        } else if team.action_byoyomi_started {
                            team.action_time_remaining_ms
                        } else {
                            sampled.config.action_time_ms
                        },
                        byoyomi_active: choice_byoyomi.is_some() || team.action_byoyomi_started,
                    }
                })
                .collect(),
            running: sampled.state.running.as_ref().map(CheckpointWindow::from),
            paused: sampled.state.paused.as_ref().map(CheckpointWindow::from),
            awaiting_authority: deadline_reached
                || sampled.state.expired_team.is_some()
                || sampled.state.double_expired
                || sampled.state.needs_decision.is_some(),
        })
    }

    /// 產生可驗證的完整持久化資料。`snapshot()` 只適合顯示，不能用來恢復時鐘。
    pub fn checkpoint(&self) -> CompetitionClockCheckpoint {
        CompetitionClockCheckpoint {
            preparation_started: self.state.preparation_started,
            double_expired: self.state.double_expired,
            version: CHECKPOINT_VERSION,
            config: self.config,
            teams: self.snapshot(),
            running: self.state.running.as_ref().map(CheckpointWindow::from),
            paused: self.state.paused.as_ref().map(CheckpointWindow::from),
            last_observed_ms: self.state.last_observed_ms,
            next_byoyomi_entry_sequence: self.state.next_entry_sequence,
            expired_team: self.state.expired_team.clone(),
            needs_decision: self.state.needs_decision,
        }
    }

    /// 從獨立 checkpoint 驗證並恢復；驗證失敗時不會產生部分時鐘。
    pub fn from_checkpoint(checkpoint: CompetitionClockCheckpoint) -> Result<Self, ClockError> {
        checkpoint.into_clock()
    }

    pub fn active_window(&self) -> Option<ActiveWindow<'_>> {
        match self.state.running.as_ref()? {
            RunningWindow::Preparation { teams } => Some(ActiveWindow::Preparation { teams }),
            RunningWindow::Action { team, turn } => {
                Some(ActiveWindow::Action { team, turn: *turn })
            }
            RunningWindow::OutOfTurnChoice {
                team, choice_id, ..
            } => Some(ActiveWindow::OutOfTurnChoice { team, choice_id }),
        }
    }

    pub fn expired_team(&self) -> Option<&TeamId> {
        self.state.expired_team.as_ref()
    }

    /// 超時判定不需要虛構遊戲結論；雙方判負不提供勝者。
    pub fn timeout_ruling(&self) -> Option<CompetitionRuling> {
        if self.state.double_expired {
            Some(CompetitionRuling::DoubleTimeoutLoss {
                teams: [
                    self.state.teams[0].team.clone(),
                    self.state.teams[1].team.clone(),
                ],
            })
        } else {
            self.state
                .expired_team
                .clone()
                .map(|loser| CompetitionRuling::TimeoutLoss { loser })
        }
    }

    /// 由權威服務開始兩隊並行準備；同隊多人等待只扣一份時間。
    /// 主時間耗盡後，整段準備共用一次設定的行動讀秒額度。
    pub fn begin_preparation(&mut self, monotonic_ms: u64) -> Result<ClockUpdate, ClockError> {
        self.validate_observation(monotonic_ms)?;
        self.require_operable()?;
        self.require_not_paused()?;
        if matches!(&self.state.running, Some(RunningWindow::Preparation { .. })) {
            return self.observe(monotonic_ms);
        }
        if self.state.running.is_some()
            || self.state.preparation_started
            || self
                .state
                .teams
                .iter()
                .any(|team| team.action_turn.is_some())
        {
            return Err(ClockError::WindowAlreadyActive);
        }
        self.state.preparation_started = true;
        self.state.last_observed_ms = monotonic_ms;
        self.state.running = Some(RunningWindow::Preparation {
            teams: self
                .state
                .teams
                .iter()
                .map(|team| team.team.clone())
                .collect(),
        });
        Ok(ClockUpdate::Advanced)
    }

    /// 僅在該隊全員完成所有準備選擇後呼叫；玩家的個別選擇不停止隊伍時鐘。
    pub fn finish_preparation(
        &mut self,
        team: &TeamId,
        monotonic_ms: u64,
    ) -> Result<ClockUpdate, ClockError> {
        self.validate_observation(monotonic_ms)?;
        self.require_operable()?;
        self.require_not_paused()?;
        self.require_team(team)?;
        let mut candidate = self.clone();
        if !matches!(
            &candidate.state.running,
            Some(RunningWindow::Preparation { .. })
        ) {
            return Err(ClockError::ActiveWindowMismatch);
        }
        let update = candidate.observe_inner(monotonic_ms)?;
        if candidate.timeout_ruling().is_none() {
            candidate.clear_preparation_byoyomi(team)?;
            if let Some(RunningWindow::Preparation { teams }) = &mut candidate.state.running {
                teams.retain(|waiting| waiting != team);
                if teams.is_empty() {
                    candidate.state.running = None;
                }
            }
        }

        *self = candidate;
        Ok(update)
    }

    pub fn paused_window(&self) -> Option<ActiveWindow<'_>> {
        match self.state.paused.as_ref()? {
            RunningWindow::Preparation { teams } => Some(ActiveWindow::Preparation { teams }),
            RunningWindow::Action { team, turn } => {
                Some(ActiveWindow::Action { team, turn: *turn })
            }
            RunningWindow::OutOfTurnChoice {
                team, choice_id, ..
            } => Some(ActiveWindow::OutOfTurnChoice { team, choice_id }),
        }
    }

    /// 回傳目前視窗下一個可能耗盡的絕對單調時間；沒有可判定的期限時回傳 `None`。
    pub fn next_timeout_deadline_ms(&self) -> Result<Option<u64>, ClockError> {
        if self.state.running.is_none()
            || self.state.paused.is_some()
            || self.state.double_expired
            || self.state.expired_team.is_some()
            || self.state.needs_decision.is_some()
        {
            return Ok(None);
        }

        let remaining = match self.state.running.as_ref().expect("running 已檢查") {
            RunningWindow::Preparation { teams } => {
                let mut earliest: Option<u64> = None;
                for team in teams {
                    if let Some(candidate) = self.preparation_timeout_remaining(team)? {
                        earliest =
                            Some(earliest.map_or(candidate, |current| current.min(candidate)));
                    }
                }
                earliest
            }
            RunningWindow::Action { team, .. } => self.action_timeout_remaining(team)?,
            RunningWindow::OutOfTurnChoice {
                team, remaining_ms, ..
            } => match (self.team_state(team)?.main_time_remaining_ms, *remaining_ms) {
                (Some(main), Some(choice)) if !self.is_main_time_configuration_unlimited() => {
                    Some(main.checked_add(choice).ok_or(ClockError::TimeArithmeticOverflow)?)
                }
                _ => *remaining_ms,
            },
        };

        remaining
            .map(|remaining| {
                self.state
                    .last_observed_ms
                    .checked_add(remaining)
                    .ok_or(ClockError::TimeArithmeticOverflow)
            })
            .transpose()
    }

    fn clear_preparation_byoyomi(&mut self, team: &TeamId) -> Result<(), ClockError> {
        let state = self.team_state_mut(team)?;
        if state.action_turn.is_none() {
            // 準備讀秒只屬於準備視窗；完成準備後不能把它誤當成下一個行動回合。
            state.action_byoyomi_started = false;
            state.action_time_remaining_ms = None;
        }
        Ok(())
    }

    pub fn needs_decision(&self) -> Option<NeedsDecisionReason> {
        self.state.needs_decision
    }

    /// 開始或繼續一個行動回合。同隊同回合的重試不會重置讀秒。
    pub fn begin_action(
        &mut self,
        team: &TeamId,
        turn: u64,
        monotonic_ms: u64,
    ) -> Result<ClockUpdate, ClockError> {
        let mut candidate = self.clone();
        let update = candidate.begin_action_inner(team, turn, monotonic_ms)?;
        *self = candidate;
        Ok(update)
    }

    /// 開始回合外選擇；有限主時間與有限選擇額度依主時間、獨立讀秒順序扣除。
    /// 不覆蓋任何隊伍原有的行動回合額度。
    pub fn begin_out_of_turn_choice(
        &mut self,
        team: &TeamId,
        choice_id: &str,
        monotonic_ms: u64,
    ) -> Result<ClockUpdate, ClockError> {
        if choice_id.is_empty() {
            return Err(ClockError::ChoiceIdEmpty);
        }
        let mut candidate = self.clone();
        let update = candidate.begin_out_of_turn_choice_inner(team, choice_id, monotonic_ms)?;
        *self = candidate;
        Ok(update)
    }

    /// 以可信單調時間觀測前進。相同輸入是冪等；系統解析期間沒有 active window，
    /// 因而只更新基準而不扣任何隊伍時間。
    pub fn observe(&mut self, monotonic_ms: u64) -> Result<ClockUpdate, ClockError> {
        if monotonic_ms < self.state.last_observed_ms {
            return Err(ClockError::MonotonicTimeWentBackwards {
                previous_ms: self.state.last_observed_ms,
                received_ms: monotonic_ms,
            });
        }
        if monotonic_ms == self.state.last_observed_ms {
            return Ok(ClockUpdate::Unchanged);
        }
        if self.state.paused.is_some() {
            self.state.last_observed_ms = monotonic_ms;
            return Ok(ClockUpdate::Unchanged);
        }
        let mut candidate = self.clone();
        let update = candidate.observe_inner(monotonic_ms)?;
        *self = candidate;
        Ok(update)
    }

    /// 結束目前的扣時視窗。重複結束同一視窗是冪等操作。
    pub fn end_window(&mut self, monotonic_ms: u64) -> Result<ClockUpdate, ClockError> {
        let mut candidate = self.clone();
        let update = candidate.end_window_inner(monotonic_ms)?;
        *self = candidate;
        Ok(update)
    }

    /// 先以權威時間觀測目前視窗，再把尚未終端的視窗移入 paused 狀態。
    pub fn pause(&mut self, monotonic_ms: u64) -> Result<ClockUpdate, ClockError> {
        let mut candidate = self.clone();
        candidate.validate_observation(monotonic_ms)?;
        candidate.require_operable()?;
        if candidate.state.paused.is_some() {
            return Err(ClockError::ClockAlreadyPaused);
        }
        if candidate.state.running.is_none() {
            return Err(ClockError::NoActiveWindow);
        }
        let update = candidate.observe_inner(monotonic_ms)?;
        if candidate.state.expired_team.is_some()
            || candidate.state.double_expired
            || candidate.state.needs_decision.is_some()
        {
            *self = candidate;
            return Ok(update);
        }
        candidate.state.paused = candidate.state.running.take();
        *self = candidate;
        Ok(ClockUpdate::Paused)
    }

    /// 從 paused 視窗恢復，`monotonic_ms` 只建立新的可信計時基準。
    pub fn resume(&mut self, monotonic_ms: u64) -> Result<ClockUpdate, ClockError> {
        self.validate_observation(monotonic_ms)?;
        self.require_operable()?;
        if self.state.paused.is_none() {
            return Err(ClockError::ClockNotPaused);
        }
        self.state.last_observed_ms = monotonic_ms;
        self.state.running = self.state.paused.take();
        Ok(ClockUpdate::Resumed)
    }

    /// 普通新局重置所有主時間；淘汰賽和局加賽才可要求沿用主時間。
    pub fn new_game(&self, time: NewGameTime, monotonic_ms: u64) -> Result<Self, ClockError> {
        self.require_not_paused()?;
        let byoyomi_order = self.recorded_byoyomi_order();
        if time == NewGameTime::CarryForward
            && (self.state.needs_decision.is_some()
                || self.state.double_expired
                || self.state.expired_team.is_some()
                || self.state.running.is_some()
                || matches!(byoyomi_order, ByoyomiEntryOrder::First(_)))
        {
            return Err(ClockError::CannotCarryForwardAfterDecision);
        }
        if monotonic_ms < self.state.last_observed_ms {
            return Err(ClockError::MonotonicTimeWentBackwards {
                previous_ms: self.state.last_observed_ms,
                received_ms: monotonic_ms,
            });
        }
        let main_time_remaining_ms = match time {
            NewGameTime::Reset => vec![self.config.main_time_ms; 2],
            NewGameTime::CarryForward => self
                .state
                .teams
                .iter()
                .map(|team| team.main_time_remaining_ms)
                .collect::<Vec<_>>(),
        };
        let mut next = Self::new(
            [
                self.state.teams[0].team.clone(),
                self.state.teams[1].team.clone(),
            ],
            self.config,
            monotonic_ms,
        )?;
        for (team, remaining) in next.state.teams.iter_mut().zip(main_time_remaining_ms) {
            team.main_time_remaining_ms = remaining;
        }
        Ok(next)
    }

    /// 依原始 Game Conclusion 產生競賽層判定；此方法不會修改 conclusion。
    pub fn rule_game(
        &self,
        conclusion: &GameConclusion,
        mode: GameResultMode,
    ) -> Result<CompetitionRuling, ClockError> {
        self.rule_game_with_byoyomi_order(conclusion, mode, self.recorded_byoyomi_order())
    }

    /// 供權威來源明確標記無法排序的同時進入讀秒；不自行猜測勝負。
    pub fn rule_game_with_byoyomi_order(
        &self,
        conclusion: &GameConclusion,
        mode: GameResultMode,
        order: ByoyomiEntryOrder,
    ) -> Result<CompetitionRuling, ClockError> {
        if let Some(ruling) = self.timeout_ruling() {
            return Ok(ruling);
        }
        match &conclusion.outcome {
            GameOutcome::Winner(team) => {
                self.require_team(team)?;
                Ok(CompetitionRuling::GameAccepted {
                    outcome: RulingOutcome::Winner(team.clone()),
                })
            }
            GameOutcome::Draw if mode == GameResultMode::RetainDraw => {
                Ok(CompetitionRuling::GameAccepted {
                    outcome: RulingOutcome::Draw,
                })
            }
            GameOutcome::Draw => match order {
                ByoyomiEntryOrder::First(team) => {
                    self.require_team(&team)?;
                    Ok(CompetitionRuling::EliminationDrawLoss {
                        loser: team.clone(),
                        reason: EliminationDrawLossReason::EnteredByoyomiFirst,
                    })
                }
                ByoyomiEntryOrder::None | ByoyomiEntryOrder::Simultaneous => {
                    Ok(CompetitionRuling::NeedsTiebreak {
                        continuation: self.carry_forward_snapshot(),
                    })
                }
            },
        }
    }

    pub fn recorded_byoyomi_order(&self) -> ByoyomiEntryOrder {
        let mut entered = self
            .state
            .teams
            .iter()
            .filter_map(|team| {
                team.byoyomi_entry_sequence
                    .map(|sequence| (&team.team, sequence))
            })
            .collect::<Vec<_>>();
        entered.sort_by_key(|(_, sequence)| *sequence);
        if entered.len() == 2 && entered[0].1 == entered[1].1 {
            return ByoyomiEntryOrder::Simultaneous;
        }
        entered
            .first()
            .map_or(ByoyomiEntryOrder::None, |(team, _)| {
                ByoyomiEntryOrder::First((*team).clone())
            })
    }

    fn begin_action_inner(
        &mut self,
        team: &TeamId,
        turn: u64,
        monotonic_ms: u64,
    ) -> Result<ClockUpdate, ClockError> {
        self.validate_observation(monotonic_ms)?;
        self.require_operable()?;
        self.require_not_paused()?;
        self.require_team(team)?;
        if let Some(active) = &self.state.running {
            if matches!(active, RunningWindow::Action { team: active_team, turn: active_turn } if active_team == team && *active_turn == turn)
            {
                return self.observe_inner(monotonic_ms);
            }
            return Err(ClockError::WindowAlreadyActive);
        }
        // 開始時間建立新的可信基準；開始前 idle 或斷線等待不屬於任何窗口。
        self.state.last_observed_ms = monotonic_ms;
        let team_state = self.team_state_mut(team)?;
        if let Some(previous_turn) = team_state.action_turn {
            if turn < previous_turn {
                return Err(ClockError::ActionTurnWentBackwards {
                    previous_turn,
                    received_turn: turn,
                });
            }
        }
        if team_state.action_turn != Some(turn) {
            team_state.action_turn = Some(turn);
            team_state.action_time_remaining_ms = None;
            team_state.action_byoyomi_started = false;
        }
        self.state.running = Some(RunningWindow::Action {
            team: team.clone(),
            turn,
        });
        if self.action_has_no_time(team) {
            self.state.needs_decision = Some(NeedsDecisionReason::ZeroTimeControl);
            return Ok(ClockUpdate::NeedsDecision {
                reason: NeedsDecisionReason::ZeroTimeControl,
            });
        }
        if !self.is_main_time_configuration_unlimited()
            && self.team_state(team)?.main_time_remaining_ms == Some(0)
        {
            self.enter_byoyomi(team)?;
            Ok(ClockUpdate::EnteredByoyomi { team: team.clone() })
        } else {
            Ok(ClockUpdate::Advanced)
        }
    }

    fn begin_out_of_turn_choice_inner(
        &mut self,
        team: &TeamId,
        choice_id: &str,
        monotonic_ms: u64,
    ) -> Result<ClockUpdate, ClockError> {
        self.validate_observation(monotonic_ms)?;
        self.require_operable()?;
        self.require_not_paused()?;
        self.require_team(team)?;
        if let Some(active) = &self.state.running {
            if matches!(active, RunningWindow::OutOfTurnChoice { team: active_team, choice_id: active_id, .. } if active_team == team && active_id == choice_id)
            {
                return self.observe_inner(monotonic_ms);
            }
            return Err(ClockError::WindowAlreadyActive);
        }
        let effective_remaining_ms = effective_out_of_turn_choice_allowance(self.config);
        self.state.running = Some(RunningWindow::OutOfTurnChoice {
            team: team.clone(),
            choice_id: choice_id.to_owned(),
            remaining_ms: effective_remaining_ms,
        });
        // choice 自己從這一刻起算，不覆蓋原有 action turn 的餘額。
        self.state.last_observed_ms = monotonic_ms;
        if effective_remaining_ms == Some(0) {
            self.state.needs_decision = Some(NeedsDecisionReason::ZeroTimeControl);
            return Ok(ClockUpdate::NeedsDecision {
                reason: NeedsDecisionReason::ZeroTimeControl,
            });
        }
        Ok(ClockUpdate::Advanced)
    }

    fn observe_inner(&mut self, monotonic_ms: u64) -> Result<ClockUpdate, ClockError> {
        let elapsed = monotonic_ms
            .checked_sub(self.state.last_observed_ms)
            .ok_or(ClockError::TimeArithmeticOverflow)?;
        self.state.last_observed_ms = monotonic_ms;
        if self.state.double_expired
            || self.state.expired_team.is_some()
            || self.state.needs_decision.is_some()
        {
            return Ok(ClockUpdate::Unchanged);
        }
        let Some(running) = self.state.running.clone() else {
            return Ok(ClockUpdate::Advanced);
        };
        if elapsed == 0 {
            return Ok(ClockUpdate::Unchanged);
        }
        match running {
            RunningWindow::Preparation { teams } => {
                self.consume_preparation_elapsed(&teams, elapsed)
            }
            RunningWindow::OutOfTurnChoice {
                team,
                choice_id,
                remaining_ms,
            } => {
                let mut choice_elapsed = elapsed;
                let mut entered_byoyomi = false;
                if !self.is_main_time_configuration_unlimited() && remaining_ms.is_some() {
                    if let Some(main) = self.team_state(&team)?.main_time_remaining_ms {
                        if elapsed < main {
                            self.team_state_mut(&team)?.main_time_remaining_ms = Some(main - elapsed);
                            return Ok(ClockUpdate::Advanced);
                        }
                        self.team_state_mut(&team)?.main_time_remaining_ms = Some(0);
                        choice_elapsed = elapsed - main;
                        if !self.team_state(&team)?.entered_byoyomi {
                            // choice 只記整局首次讀秒歷史，不初始化或重置任何 action turn 額度。
                            let sequence = self.state.next_entry_sequence;
                            self.state.next_entry_sequence = sequence
                                .checked_add(1)
                                .ok_or(ClockError::TimeArithmeticOverflow)?;
                            let state = self.team_state_mut(&team)?;
                            state.entered_byoyomi = true;
                            state.byoyomi_entry_sequence = Some(sequence);
                            entered_byoyomi = true;
                        }
                    }
                }
                match remaining_ms {
                    None => Ok(ClockUpdate::Advanced),
                    Some(remaining) if choice_elapsed >= remaining => {
                        self.state.running = Some(RunningWindow::OutOfTurnChoice {
                            team: team.clone(),
                            choice_id,
                            remaining_ms: Some(0),
                        });
                        self.state.expired_team = Some(team.clone());
                        Ok(ClockUpdate::TimedOut { team })
                    }
                    Some(remaining) => {
                        self.state.running = Some(RunningWindow::OutOfTurnChoice {
                            team: team.clone(),
                            choice_id,
                            remaining_ms: Some(remaining - choice_elapsed),
                        });
                        if entered_byoyomi {
                            Ok(ClockUpdate::EnteredByoyomi { team })
                        } else {
                            Ok(ClockUpdate::Advanced)
                        }
                    }
                }
            },
            RunningWindow::Action { team, .. } => self.consume_action_elapsed(&team, elapsed),
        }
    }

    fn end_window_inner(&mut self, monotonic_ms: u64) -> Result<ClockUpdate, ClockError> {
        self.validate_observation(monotonic_ms)?;
        self.require_operable()?;
        self.require_not_paused()?;
        if self.state.running.is_none() {
            return Ok(ClockUpdate::Unchanged);
        }
        if matches!(&self.state.running, Some(RunningWindow::Preparation { teams }) if !teams.is_empty())
        {
            return Err(ClockError::PreparationNotComplete);
        }
        let update = self.observe_inner(monotonic_ms)?;
        if self.state.expired_team.is_some() || self.state.needs_decision.is_some() {
            return Ok(update);
        }
        self.state.running = None;
        Ok(update)
    }

    fn consume_action_elapsed(
        &mut self,
        team: &TeamId,
        elapsed: u64,
    ) -> Result<ClockUpdate, ClockError> {
        if self.is_main_time_configuration_unlimited() {
            return Ok(ClockUpdate::Advanced);
        }
        let main_remaining = self.team_state(team)?.main_time_remaining_ms;
        let Some(main) = main_remaining else {
            // 不限主時間永遠不進入讀秒，也不因長 elapsed 扣除任何額度。
            return Ok(ClockUpdate::Advanced);
        };
        if main > 0 {
            if elapsed < main {
                self.team_state_mut(team)?.main_time_remaining_ms = Some(main - elapsed);
                return Ok(ClockUpdate::Advanced);
            }
            let extra = elapsed - main;
            self.team_state_mut(team)?.main_time_remaining_ms = Some(0);
            self.enter_byoyomi(team)?;
            if extra == 0 {
                return Ok(ClockUpdate::EnteredByoyomi { team: team.clone() });
            }
            return match self.consume_byoyomi_elapsed(team, extra)? {
                ClockUpdate::Advanced => Ok(ClockUpdate::EnteredByoyomi { team: team.clone() }),
                update => Ok(update),
            };
        }
        self.enter_byoyomi(team)?;
        self.consume_byoyomi_elapsed(team, elapsed)
    }

    fn consume_preparation_elapsed(
        &mut self,
        teams: &[TeamId],
        elapsed: u64,
    ) -> Result<ClockUpdate, ClockError> {
        // 先找最早的實際耗盡時刻。延遲觀測只會處理到第一個 deadline；
        // 只有兩隊的 deadline 完全相同，才形成 DoubleLoss。
        let deadlines = teams
            .iter()
            .filter_map(|team| {
                self.preparation_deadline(team)
                    .transpose()
                    .map(|result| result.map(|deadline| (team.clone(), deadline)))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let earliest = deadlines.iter().map(|(_, deadline)| *deadline).min();
        let advance = earliest.map_or(elapsed, |deadline| elapsed.min(deadline));

        let entry_times = teams
            .iter()
            .filter_map(|team| {
                self.preparation_entry_time(team)
                    .map(|entry| (team.clone(), entry))
            })
            .filter(|(_, entry)| *entry <= advance)
            .collect::<Vec<_>>();
        self.enter_preparation_byoyomi(&entry_times)?;
        for team in teams {
            self.advance_preparation_team(team, advance)?;
        }

        let Some(deadline) = earliest.filter(|deadline| *deadline <= elapsed) else {
            return if entry_times.is_empty() {
                Ok(ClockUpdate::Advanced)
            } else if entry_times.len() == 1 {
                Ok(ClockUpdate::EnteredByoyomi {
                    team: entry_times[0].0.clone(),
                })
            } else {
                Ok(ClockUpdate::Advanced)
            };
        };
        let expired = deadlines
            .iter()
            .filter(|(_, candidate)| *candidate == deadline)
            .map(|(team, _)| team.clone())
            .collect::<Vec<_>>();
        if expired.len() == 2 {
            self.state.double_expired = true;
            return Ok(ClockUpdate::DoubleTimedOut {
                teams: [expired[0].clone(), expired[1].clone()],
            });
        }
        self.state.expired_team = expired.into_iter().next();
        Ok(ClockUpdate::TimedOut {
            team: self.state.expired_team.clone().expect("至少一隊耗盡"),
        })
    }

    fn preparation_entry_time(&self, team: &TeamId) -> Option<u64> {
        if self.is_main_time_configuration_unlimited() {
            return None;
        }
        let state = self.team_state(team).ok()?;
        if state.main_time_remaining_ms.is_none() || state.action_byoyomi_started {
            return None;
        }
        state.main_time_remaining_ms
    }

    fn preparation_deadline(&self, team: &TeamId) -> Result<Option<u64>, ClockError> {
        if self.is_main_time_configuration_unlimited() {
            return Ok(None);
        }
        let state = self.team_state(team)?;
        let Some(main) = state.main_time_remaining_ms else {
            return Ok(None);
        };
        let byoyomi = if state.action_byoyomi_started {
            state.action_time_remaining_ms
        } else {
            self.config.action_time_ms
        };
        match byoyomi {
            Some(byoyomi) => Ok(Some(if state.action_byoyomi_started {
                byoyomi
            } else {
                main.checked_add(byoyomi)
                    .ok_or(ClockError::TimeArithmeticOverflow)?
            })),
            None => Ok(None),
        }
    }

    fn preparation_timeout_remaining(&self, team: &TeamId) -> Result<Option<u64>, ClockError> {
        if self.is_main_time_configuration_unlimited() {
            return Ok(None);
        }
        let state = self.team_state(team)?;
        let Some(main) = state.main_time_remaining_ms else {
            return Ok(None);
        };
        let allowance = if state.action_byoyomi_started {
            state.action_time_remaining_ms
        } else {
            self.config.action_time_ms
        };
        match allowance {
            None => Ok(None),
            Some(allowance) if state.action_byoyomi_started => Ok(Some(allowance)),
            Some(allowance) => Ok(Some(
                main.checked_add(allowance)
                    .ok_or(ClockError::TimeArithmeticOverflow)?,
            )),
        }
    }

    fn action_timeout_remaining(&self, team: &TeamId) -> Result<Option<u64>, ClockError> {
        if self.is_main_time_configuration_unlimited() {
            return Ok(None);
        }
        let state = self.team_state(team)?;
        let Some(main) = state.main_time_remaining_ms else {
            return Ok(None);
        };
        if state.action_byoyomi_started {
            return Ok(state.action_time_remaining_ms);
        }
        self.config
            .action_time_ms
            .map(|allowance| {
                main.checked_add(allowance)
                    .ok_or(ClockError::TimeArithmeticOverflow)
            })
            .transpose()
    }

    fn enter_preparation_byoyomi(
        &mut self,
        entry_times: &[(TeamId, u64)],
    ) -> Result<(), ClockError> {
        let mut entry_times = entry_times.to_vec();
        entry_times.sort_by_key(|(_, entry)| *entry);
        let mut index = 0;
        while index < entry_times.len() {
            let entry = entry_times[index].1;
            let end = entry_times
                .iter()
                .skip(index)
                .position(|(_, candidate)| *candidate != entry)
                .map_or(entry_times.len(), |offset| index + offset);
            let sequence = self.state.next_entry_sequence;
            for (team, _) in &entry_times[index..end] {
                self.enter_byoyomi_at_sequence(team, sequence)?;
            }
            self.state.next_entry_sequence = sequence
                .checked_add(1)
                .ok_or(ClockError::TimeArithmeticOverflow)?;
            index = end;
        }
        Ok(())
    }

    fn enter_byoyomi_at_sequence(
        &mut self,
        team: &TeamId,
        sequence: u64,
    ) -> Result<(), ClockError> {
        let action_time = self.config.action_time_ms;
        let state = self.team_state_mut(team)?;
        if state.action_byoyomi_started {
            return Ok(());
        }
        state.action_byoyomi_started = true;
        state.action_time_remaining_ms = action_time;
        state.entered_byoyomi = true;
        state.byoyomi_entry_sequence = Some(sequence);
        Ok(())
    }

    fn advance_preparation_team(&mut self, team: &TeamId, elapsed: u64) -> Result<(), ClockError> {
        if self.is_main_time_configuration_unlimited() {
            return Ok(());
        }
        let main = self.team_state(team)?.main_time_remaining_ms;
        let Some(main) = main else {
            return Ok(());
        };
        if self.team_state(team)?.action_byoyomi_started {
            let byoyomi_elapsed = if main > 0 {
                elapsed.saturating_sub(main)
            } else {
                elapsed
            };
            self.team_state_mut(team)?.main_time_remaining_ms = Some(0);
            if let Some(remaining) = self.team_state(team)?.action_time_remaining_ms {
                self.team_state_mut(team)?.action_time_remaining_ms =
                    Some(remaining.saturating_sub(byoyomi_elapsed));
            }
            return Ok(());
        }
        if elapsed < main {
            self.team_state_mut(team)?.main_time_remaining_ms = Some(main - elapsed);
        } else {
            self.team_state_mut(team)?.main_time_remaining_ms = Some(0);
        }
        Ok(())
    }

    fn is_main_time_configuration_unlimited(&self) -> bool {
        self.config.main_time_ms == Some(0)
    }

    fn consume_byoyomi_elapsed(
        &mut self,
        team: &TeamId,
        elapsed: u64,
    ) -> Result<ClockUpdate, ClockError> {
        let Some(remaining) = self.team_state(team)?.action_time_remaining_ms else {
            // 不限讀秒表示主時間耗盡後仍不限時。
            return Ok(ClockUpdate::Advanced);
        };
        if elapsed >= remaining {
            self.team_state_mut(team)?.action_time_remaining_ms = Some(0);
            self.state.expired_team = Some(team.clone());
            return Ok(ClockUpdate::TimedOut { team: team.clone() });
        }
        self.team_state_mut(team)?.action_time_remaining_ms = Some(remaining - elapsed);
        Ok(ClockUpdate::Advanced)
    }

    fn enter_byoyomi(&mut self, team: &TeamId) -> Result<(), ClockError> {
        if self.team_state(team)?.action_byoyomi_started {
            return Ok(());
        }
        let sequence = self.state.next_entry_sequence;
        let action_time = self.config.action_time_ms;
        let history_exists = self.team_state(team)?.entered_byoyomi;
        let next_sequence = if history_exists {
            sequence
        } else {
            sequence
                .checked_add(1)
                .ok_or(ClockError::TimeArithmeticOverflow)?
        };
        if !history_exists {
            self.state.next_entry_sequence = next_sequence;
        }
        let state = self.team_state_mut(team)?;
        state.action_byoyomi_started = true;
        state.action_time_remaining_ms = action_time;
        if history_exists {
            return Ok(());
        }
        state.entered_byoyomi = true;
        state.byoyomi_entry_sequence = Some(sequence);
        Ok(())
    }

    fn carry_forward_snapshot(&self) -> EliminationContinuation {
        EliminationContinuation {
            teams: self
                .state
                .teams
                .iter()
                .map(|team| TeamMainTime {
                    team: team.team.clone(),
                    main_time_remaining_ms: team.main_time_remaining_ms,
                })
                .collect(),
        }
    }

    fn action_has_no_time(&self, team: &TeamId) -> bool {
        if self.is_main_time_configuration_unlimited() {
            return false;
        }
        self.team_state(team).ok().is_some_and(|state| {
            state.main_time_remaining_ms == Some(0) && self.config.action_time_ms == Some(0)
        })
    }

    fn validate_observation(&self, monotonic_ms: u64) -> Result<(), ClockError> {
        if monotonic_ms < self.state.last_observed_ms {
            return Err(ClockError::MonotonicTimeWentBackwards {
                previous_ms: self.state.last_observed_ms,
                received_ms: monotonic_ms,
            });
        }
        Ok(())
    }

    fn require_operable(&self) -> Result<(), ClockError> {
        if self.state.double_expired {
            return Err(ClockError::ClockAlreadyDoubleExpired);
        }
        if let Some(team) = &self.state.expired_team {
            return Err(ClockError::ClockAlreadyExpired(team.clone()));
        }
        if self.state.needs_decision.is_some() {
            return Err(ClockError::CannotCarryForwardAfterDecision);
        }
        Ok(())
    }

    fn require_not_paused(&self) -> Result<(), ClockError> {
        if self.state.paused.is_some() {
            Err(ClockError::ClockIsPaused)
        } else {
            Ok(())
        }
    }

    fn require_team(&self, team: &TeamId) -> Result<(), ClockError> {
        if self.state.teams.iter().any(|state| &state.team == team) {
            Ok(())
        } else {
            Err(ClockError::TeamNotRegistered(team.clone()))
        }
    }

    fn team_state(&self, team: &TeamId) -> Result<&TeamClockState, ClockError> {
        self.state
            .teams
            .iter()
            .find(|state| &state.team == team)
            .ok_or_else(|| ClockError::TeamNotRegistered(team.clone()))
    }

    fn team_state_mut(&mut self, team: &TeamId) -> Result<&mut TeamClockState, ClockError> {
        self.state
            .teams
            .iter_mut()
            .find(|state| &state.team == team)
            .ok_or_else(|| ClockError::TeamNotRegistered(team.clone()))
    }
}

/// 驗證賽事階段可採用的時鐘設定。
///
/// 階段設定與實際時鐘共用這個入口，避免各層複製不同的合法性規則。
pub fn validate_config(config: ClockConfig) -> Result<(), ClockError> {
    let _ = config;
    Ok(())
}

fn validate_configured_remaining(
    configured: TimeLimit,
    remaining: TimeLimit,
    reason: &'static str,
) -> Result<(), ClockError> {
    match (configured, remaining) {
        (Some(limit), Some(remaining)) if remaining <= limit => Ok(()),
        (None, None) => Ok(()),
        _ => Err(ClockError::InvalidCheckpoint(reason)),
    }
}

fn effective_out_of_turn_choice_allowance(config: ClockConfig) -> TimeLimit {
    if config.main_time_ms == Some(0) {
        None
    } else {
        config.out_of_turn_choice_time_ms
    }
}

/// 遊戲和局在競賽層的處理方式；必須由呼叫者明確指定。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameResultMode {
    Elimination,
    RetainDraw,
}

/// 讀秒進入先後。`Simultaneous` 僅能由權威外層明確提供，模組不自行推定。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ByoyomiEntryOrder {
    None,
    First(TeamId),
    Simultaneous,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum RulingOutcome {
    Winner(TeamId),
    Draw,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum EliminationDrawLossReason {
    EnteredByoyomiFirst,
}

/// 淘汰賽或普通遊戲的競賽層結果；它不取代原始 Game Conclusion。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CompetitionRuling {
    TimeoutLoss {
        loser: TeamId,
    },
    DoubleTimeoutLoss {
        teams: [TeamId; 2],
    },
    GameAccepted {
        outcome: RulingOutcome,
    },
    EliminationDrawLoss {
        loser: TeamId,
        reason: EliminationDrawLossReason,
    },
    NeedsTiebreak {
        continuation: EliminationContinuation,
    },
    NeedsDecision {
        reason: NeedsDecisionReason,
    },
}

/// 淘汰賽和局且讀秒進入先後無法決勝時，加賽沿用的主時間。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EliminationContinuation {
    pub teams: Vec<TeamMainTime>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TeamMainTime {
    pub team: TeamId,
    pub main_time_remaining_ms: TimeLimit,
}

const CHECKPOINT_VERSION: u16 = 2;

/// 時計唯一完整的可序列化 checkpoint；它刻意與面向 UI 的 `TeamClockSnapshot`
/// 分開，避免顯示資料被誤當成可恢復狀態。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompetitionClockCheckpoint {
    #[serde(default)]
    pub preparation_started: bool,
    #[serde(default)]
    pub double_expired: bool,
    pub version: u16,
    pub config: ClockConfig,
    pub teams: Vec<TeamClockSnapshot>,
    pub running: Option<CheckpointWindow>,
    #[serde(default)]
    pub paused: Option<CheckpointWindow>,
    pub last_observed_ms: u64,
    pub next_byoyomi_entry_sequence: u64,
    pub expired_team: Option<TeamId>,
    pub needs_decision: Option<NeedsDecisionReason>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CheckpointWindow {
    Preparation {
        teams: Vec<TeamId>,
    },
    Action {
        team: TeamId,
        turn: u64,
    },
    OutOfTurnChoice {
        team: TeamId,
        choice_id: String,
        remaining_ms: TimeLimit,
    },
}

impl From<&RunningWindow> for CheckpointWindow {
    fn from(window: &RunningWindow) -> Self {
        match window {
            RunningWindow::Preparation { teams } => Self::Preparation {
                teams: teams.clone(),
            },
            RunningWindow::Action { team, turn } => Self::Action {
                team: team.clone(),
                turn: *turn,
            },
            RunningWindow::OutOfTurnChoice {
                team,
                choice_id,
                remaining_ms,
            } => Self::OutOfTurnChoice {
                team: team.clone(),
                choice_id: choice_id.clone(),
                remaining_ms: *remaining_ms,
            },
        }
    }
}

impl CompetitionClockCheckpoint {
    fn restore_window(
        window: CheckpointWindow,
        states: &[TeamClockState],
        config: ClockConfig,
        preparation_started: bool,
    ) -> Result<RunningWindow, ClockError> {
        match window {
            CheckpointWindow::Preparation { teams } => {
                if teams.is_empty()
                    || teams.len() > 2
                    || teams
                        .iter()
                        .any(|team| !states.iter().any(|entry| &entry.team == team))
                    || teams.windows(2).any(|pair| pair[0] == pair[1])
                {
                    return Err(ClockError::InvalidCheckpoint(
                        "running preparation teams are invalid",
                    ));
                }
                if !preparation_started {
                    return Err(ClockError::InvalidCheckpoint(
                        "running preparation needs preparationStarted",
                    ));
                }
                Ok(RunningWindow::Preparation { teams })
            }
            CheckpointWindow::Action { team, turn } => {
                if !states.iter().any(|entry| entry.team == team) {
                    return Err(ClockError::TeamNotRegistered(team));
                }
                let state = states.iter().find(|entry| entry.team == team).unwrap();
                if state.action_turn != Some(turn) {
                    return Err(ClockError::InvalidCheckpoint(
                        "running action does not match the team's action turn",
                    ));
                }
                Ok(RunningWindow::Action { team, turn })
            }
            CheckpointWindow::OutOfTurnChoice {
                team,
                choice_id,
                remaining_ms,
            } => {
                if !states.iter().any(|entry| entry.team == team) {
                    return Err(ClockError::TeamNotRegistered(team));
                }
                if choice_id.is_empty() {
                    return Err(ClockError::ChoiceIdEmpty);
                }
                validate_configured_remaining(
                    effective_out_of_turn_choice_allowance(config),
                    remaining_ms,
                    "choice remaining time does not match configured allowance",
                )?;
                Ok(RunningWindow::OutOfTurnChoice {
                    team,
                    choice_id,
                    remaining_ms,
                })
            }
        }
    }

    fn into_clock(self) -> Result<CompetitionClock, ClockError> {
        if self.version != 1 && self.version != CHECKPOINT_VERSION {
            return Err(ClockError::InvalidCheckpoint(
                "unsupported checkpoint version",
            ));
        }
        if self.version == 1 && self.paused.is_some() {
            return Err(ClockError::InvalidCheckpoint(
                "version one checkpoint cannot contain paused state",
            ));
        }
        validate_config(self.config)?;
        if self.teams.len() != 2 {
            return Err(ClockError::ExactlyTwoTeamsRequired);
        }
        if self.teams[0].team == self.teams[1].team {
            return Err(ClockError::TeamsMustBeDistinct);
        }
        if self.expired_team.is_some() && self.needs_decision.is_some() {
            return Err(ClockError::InvalidCheckpoint(
                "expired and needsDecision cannot both be set",
            ));
        }
        if self.double_expired && self.expired_team.is_some() {
            return Err(ClockError::InvalidCheckpoint(
                "doubleExpired cannot have one expired team",
            ));
        }
        if self.double_expired && self.needs_decision.is_some() {
            return Err(ClockError::InvalidCheckpoint(
                "doubleExpired cannot require another decision",
            ));
        }
        if self.running.is_some() && self.paused.is_some() {
            return Err(ClockError::InvalidCheckpoint(
                "running and paused windows cannot both be set",
            ));
        }
        if self.paused.is_some()
            && (self.expired_team.is_some() || self.double_expired || self.needs_decision.is_some())
        {
            return Err(ClockError::InvalidCheckpoint(
                "paused clock cannot be terminal or require a decision",
            ));
        }
        if let Some(team) = self.expired_team.as_ref() {
            if !self.teams.iter().any(|entry| &entry.team == team) {
                return Err(ClockError::TeamNotRegistered(team.clone()));
            }
        }
        let active_window = self.running.as_ref().or(self.paused.as_ref());
        let is_preparation = matches!(active_window, Some(CheckpointWindow::Preparation { .. }));
        let preparation_teams = match active_window {
            Some(CheckpointWindow::Preparation { teams }) => Some(teams.as_slice()),
            _ => None,
        };
        let mut states = Vec::with_capacity(2);
        for entry in &self.teams {
            validate_configured_remaining(
                self.config.main_time_ms,
                entry.main_time_remaining_ms,
                "main time remaining does not match configured allowance",
            )?;
            if let Some(sequence) = entry.byoyomi_entry_sequence {
                if sequence >= self.next_byoyomi_entry_sequence {
                    return Err(ClockError::InvalidCheckpoint(
                        "byoyomi entry sequence is not strictly valid",
                    ));
                }
            }
            if entry.entered_byoyomi != entry.byoyomi_entry_sequence.is_some() {
                return Err(ClockError::InvalidCheckpoint(
                    "byoyomi entry flag and sequence disagree",
                ));
            }
            if self.config.main_time_ms == Some(0) && entry.entered_byoyomi {
                return Err(ClockError::InvalidCheckpoint(
                    "configured zero main time cannot have byoyomi history",
                ));
            }
            if entry
                .main_time_remaining_ms
                .is_some_and(|remaining| remaining > 0)
                && (entry.entered_byoyomi
                    || entry.action_byoyomi_started
                    || entry.action_time_remaining_ms.is_some())
            {
                return Err(ClockError::InvalidCheckpoint(
                    "main time remaining conflicts with byoyomi state",
                ));
            }
            if entry.action_time_remaining_ms.is_some() && !entry.action_byoyomi_started {
                return Err(ClockError::InvalidCheckpoint(
                    "action time cannot exist before an active byoyomi turn",
                ));
            }
            if entry.action_byoyomi_started {
                validate_configured_remaining(
                    self.config.action_time_ms,
                    entry.action_time_remaining_ms,
                    "active action time does not match configured allowance",
                )?;
                if entry.action_turn.is_none()
                    && !preparation_teams
                        .is_some_and(|teams| teams.iter().any(|team| team == &entry.team))
                {
                    return Err(ClockError::InvalidCheckpoint(
                        "a turnless byoyomi state must belong to preparation",
                    ));
                }
            }
            if entry.action_turn.is_none()
                && entry.action_time_remaining_ms.is_some()
                && !is_preparation
            {
                return Err(ClockError::InvalidCheckpoint(
                    "action time cannot exist without an action turn",
                ));
            }
            if !entry.entered_byoyomi && entry.action_time_remaining_ms.is_some() {
                return Err(ClockError::InvalidCheckpoint(
                    "action time cannot exist before byoyomi",
                ));
            }
            if entry.action_byoyomi_started && !entry.entered_byoyomi {
                return Err(ClockError::InvalidCheckpoint(
                    "an active byoyomi turn needs a recorded game entry",
                ));
            }
            states.push(TeamClockState {
                team: entry.team.clone(),
                main_time_remaining_ms: entry.main_time_remaining_ms,
                action_turn: entry.action_turn,
                action_time_remaining_ms: entry.action_time_remaining_ms,
                action_byoyomi_started: entry.action_byoyomi_started,
                entered_byoyomi: entry.entered_byoyomi,
                byoyomi_entry_sequence: entry.byoyomi_entry_sequence,
            });
        }
        if self.needs_decision.is_some() && self.running.is_none() {
            return Err(ClockError::InvalidCheckpoint(
                "needsDecision requires an active window",
            ));
        }
        if self.double_expired {
            let valid_double_window = matches!(
                &self.running,
                Some(CheckpointWindow::Preparation { teams })
                    if teams.len() == 2
                        && states.iter().all(|state| {
                            teams.iter().any(|team| team == &state.team)
                                && state.main_time_remaining_ms == Some(0)
                                && state.action_byoyomi_started
                                && state.action_time_remaining_ms == Some(0)
                        })
            );
            if !valid_double_window {
                return Err(ClockError::InvalidCheckpoint(
                    "doubleExpired does not match two exhausted preparation clocks",
                ));
            }
        }
        if let Some(expired_team) = self.expired_team.as_ref() {
            let state = states
                .iter()
                .find(|state| &state.team == expired_team)
                .expect("expired team was checked above");
            let action_exhausted = state.main_time_remaining_ms == Some(0)
                && state.action_byoyomi_started
                && state.action_time_remaining_ms == Some(0);
            let valid_expiry = match &self.running {
                Some(CheckpointWindow::Action { team, .. }) => {
                    team == expired_team && action_exhausted
                }
                Some(CheckpointWindow::Preparation { teams }) => {
                    teams.iter().any(|team| team == expired_team) && action_exhausted
                }
                Some(CheckpointWindow::OutOfTurnChoice {
                    team, remaining_ms, ..
                }) => team == expired_team && *remaining_ms == Some(0),
                None => false,
            };
            if !valid_expiry {
                return Err(ClockError::InvalidCheckpoint(
                    "expired team does not match an exhausted active clock",
                ));
            }
        }
        let running = self
            .running
            .map(|window| {
                Self::restore_window(window, &states, self.config, self.preparation_started)
            })
            .transpose()?;
        let paused = self
            .paused
            .map(|window| {
                Self::restore_window(window, &states, self.config, self.preparation_started)
            })
            .transpose()?;
        Ok(CompetitionClock {
            config: self.config,
            state: ClockState {
                preparation_started: self.preparation_started,
                double_expired: self.double_expired,
                teams: states,
                running,
                paused,
                last_observed_ms: self.last_observed_ms,
                next_entry_sequence: self.next_byoyomi_entry_sequence,
                expired_team: self.expired_team,
                needs_decision: self.needs_decision,
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{GameEndCause, GameOutcome};

    fn team(id: &str) -> TeamId {
        TeamId::new(id)
    }

    fn conclusion(outcome: GameOutcome) -> GameConclusion {
        GameConclusion::new(
            outcome,
            vec![GameEndCause::DirectVictory {
                rule: "test".to_owned(),
                team: team("a"),
            }],
        )
    }

    fn clock() -> CompetitionClock {
        CompetitionClock::new([team("a"), team("b")], ClockConfig::default(), 1_000).unwrap()
    }

    fn remaining(clock: &CompetitionClock, id: &str) -> TeamClockSnapshot {
        clock
            .snapshot()
            .into_iter()
            .find(|entry| entry.team == team(id))
            .unwrap()
    }

    #[test]
    fn shares_main_time_and_does_not_charge_system_resolution() {
        let mut clock = clock();
        clock.begin_action(&team("a"), 1, 1_000).unwrap();
        clock.observe(61_000).unwrap();
        assert_eq!(remaining(&clock, "a").main_time_remaining_ms, Some(240_000));
        clock.end_window(61_000).unwrap();
        clock.observe(3_600_000).unwrap();
        assert_eq!(remaining(&clock, "a").main_time_remaining_ms, Some(240_000));
        assert_eq!(remaining(&clock, "b").main_time_remaining_ms, Some(300_000));
    }

    #[test]
    fn main_time_expiry_enters_full_byoyomi_and_same_turn_does_not_reset_it() {
        let mut clock = clock();
        clock.begin_action(&team("a"), 1, 1_000).unwrap();
        assert_eq!(
            clock.observe(301_000).unwrap(),
            ClockUpdate::EnteredByoyomi { team: team("a") }
        );
        assert_eq!(
            remaining(&clock, "a").action_time_remaining_ms,
            Some(30_000)
        );
        clock.observe(311_000).unwrap();
        clock.begin_action(&team("a"), 1, 311_000).unwrap();
        assert_eq!(
            remaining(&clock, "a").action_time_remaining_ms,
            Some(20_000)
        );
    }

    #[test]
    fn a_new_turn_discards_unused_byoyomi_and_starts_a_fresh_allowance() {
        let mut clock = clock();
        clock.begin_action(&team("a"), 1, 1_000).unwrap();
        clock.observe(301_000).unwrap();
        clock.end_window(301_000).unwrap();
        clock.begin_action(&team("b"), 2, 301_000).unwrap();
        clock.end_window(301_000).unwrap();
        clock.begin_action(&team("a"), 3, 301_000).unwrap();
        assert_eq!(
            remaining(&clock, "a").action_time_remaining_ms,
            Some(30_000)
        );
    }

    #[test]
    fn timeout_is_only_declared_when_byoyomi_is_exhausted() {
        let mut clock = clock();
        clock.begin_action(&team("a"), 1, 1_000).unwrap();
        assert_eq!(clock.observe(300_999).unwrap(), ClockUpdate::Advanced);
        assert_eq!(
            clock.observe(330_999).unwrap(),
            ClockUpdate::EnteredByoyomi { team: team("a") }
        );
        assert_eq!(
            clock.observe(331_000).unwrap(),
            ClockUpdate::TimedOut { team: team("a") }
        );
        assert_eq!(clock.expired_team(), Some(&team("a")));
    }

    #[test]
    fn out_of_turn_choice_uses_independent_time() {
        let mut clock = clock();
        clock
            .begin_out_of_turn_choice(&team("b"), "choice-1", 1_000)
            .unwrap();
        clock.observe(30_999).unwrap();
        assert_eq!(remaining(&clock, "b").main_time_remaining_ms, Some(270_001));
        assert_eq!(remaining(&clock, "a").main_time_remaining_ms, Some(300_000));
        clock.observe(330_999).unwrap();
        assert_eq!(clock.expired_team(), None);
        assert_eq!(
            clock.checkpoint().running,
            Some(CheckpointWindow::OutOfTurnChoice {
                team: team("b"),
                choice_id: "choice-1".to_owned(),
                remaining_ms: Some(1),
            })
        );
        assert_eq!(
            clock.observe(331_000).unwrap(),
            ClockUpdate::TimedOut { team: team("b") }
        );
        assert_eq!(remaining(&clock, "a").main_time_remaining_ms, Some(300_000));
    }

    #[test]
    fn repeated_observations_and_repeated_begin_are_idempotent() {
        let mut clock = clock();
        clock.begin_action(&team("a"), 1, 1_000).unwrap();
        clock.observe(11_000).unwrap();
        let before = clock.snapshot();
        assert_eq!(clock.observe(11_000).unwrap(), ClockUpdate::Unchanged);
        assert_eq!(clock.snapshot(), before);
        clock.begin_action(&team("a"), 1, 11_000).unwrap();
        assert_eq!(clock.snapshot(), before);
    }

    #[test]
    fn rejects_time_rollback_and_keeps_state_unchanged() {
        let mut clock = clock();
        clock.begin_action(&team("a"), 1, 1_000).unwrap();
        let before = clock.clone();
        assert_eq!(
            clock.observe(999),
            Err(ClockError::MonotonicTimeWentBackwards {
                previous_ms: 1_000,
                received_ms: 999,
            })
        );
        assert_eq!(clock, before);
    }

    #[test]
    fn exact_boundary_times_out_and_large_time_does_not_overflow() {
        let mut clock = CompetitionClock::new(
            [team("a"), team("b")],
            ClockConfig {
                main_time_ms: Some(5),
                action_time_ms: Some(5),
                ..ClockConfig::default()
            },
            0,
        )
        .unwrap();
        clock.begin_action(&team("a"), 1, u64::MAX - 10).unwrap();
        assert_eq!(
            clock.observe(u64::MAX).unwrap(),
            ClockUpdate::TimedOut { team: team("a") }
        );
    }

    #[test]
    fn ordinary_game_resets_but_elimination_continuation_carries_main_time() {
        let mut clock = clock();
        clock.begin_action(&team("a"), 1, 1_000).unwrap();
        clock.observe(61_000).unwrap();
        clock.end_window(61_000).unwrap();
        let carried = clock
            .rule_game(&conclusion(GameOutcome::Draw), GameResultMode::Elimination)
            .unwrap();
        assert_eq!(
            carried,
            CompetitionRuling::NeedsTiebreak {
                continuation: EliminationContinuation {
                    teams: vec![
                        TeamMainTime {
                            team: team("a"),
                            main_time_remaining_ms: Some(240_000)
                        },
                        TeamMainTime {
                            team: team("b"),
                            main_time_remaining_ms: Some(300_000)
                        },
                    ],
                },
            }
        );
        let next = clock.new_game(NewGameTime::CarryForward, 61_000).unwrap();
        assert_eq!(remaining(&next, "a").main_time_remaining_ms, Some(240_000));
        let ordinary = clock.new_game(NewGameTime::Reset, 61_000).unwrap();
        assert_eq!(
            remaining(&ordinary, "a").main_time_remaining_ms,
            Some(300_000)
        );
    }

    #[test]
    fn byoyomi_first_loses_only_when_original_game_is_a_draw() {
        let mut clock = clock();
        clock.begin_action(&team("a"), 1, 1_000).unwrap();
        clock.observe(301_000).unwrap();
        let original = conclusion(GameOutcome::Winner(team("a")));
        assert_eq!(
            clock
                .rule_game(&original, GameResultMode::Elimination)
                .unwrap(),
            CompetitionRuling::GameAccepted {
                outcome: RulingOutcome::Winner(team("a"))
            }
        );
        let draw = conclusion(GameOutcome::Draw);
        assert_eq!(
            clock.rule_game(&draw, GameResultMode::Elimination).unwrap(),
            CompetitionRuling::EliminationDrawLoss {
                loser: team("a"),
                reason: EliminationDrawLossReason::EnteredByoyomiFirst,
            }
        );
    }

    #[test]
    fn unlimited_time_carries_draw_when_byoyomi_order_does_not_distinguish_a_loser() {
        let config = ClockConfig {
            main_time_ms: Some(0),
            ..ClockConfig::default()
        };
        let mut zero = CompetitionClock::new([team("a"), team("b")], config, 0).unwrap();
        assert_eq!(
            zero.begin_action(&team("a"), 1, 0).unwrap(),
            ClockUpdate::Advanced
        );
        let unlimited = CompetitionClock::new(
            [team("a"), team("b")],
            ClockConfig {
                main_time_ms: None,
                action_time_ms: None,
                ..ClockConfig::default()
            },
            0,
        )
        .unwrap();
        assert_eq!(
            unlimited.rule_game_with_byoyomi_order(
                &conclusion(GameOutcome::Draw),
                GameResultMode::Elimination,
                ByoyomiEntryOrder::None,
            ),
            Ok(CompetitionRuling::NeedsTiebreak {
                continuation: EliminationContinuation {
                    teams: vec![
                        TeamMainTime {
                            team: team("a"),
                            main_time_remaining_ms: None,
                        },
                        TeamMainTime {
                            team: team("b"),
                            main_time_remaining_ms: None,
                        },
                    ],
                },
            })
        );
        assert_eq!(
            unlimited.rule_game_with_byoyomi_order(
                &conclusion(GameOutcome::Draw),
                GameResultMode::Elimination,
                ByoyomiEntryOrder::Simultaneous,
            ),
            Ok(CompetitionRuling::NeedsTiebreak {
                continuation: EliminationContinuation {
                    teams: vec![
                        TeamMainTime {
                            team: team("a"),
                            main_time_remaining_ms: None,
                        },
                        TeamMainTime {
                            team: team("b"),
                            main_time_remaining_ms: None,
                        },
                    ],
                },
            })
        );
    }

    #[test]
    fn original_game_conclusion_is_unchanged_and_ruling_serializes_camel_case() {
        let original = conclusion(GameOutcome::Draw);
        let before = original.clone();
        let ruling = clock()
            .rule_game_with_byoyomi_order(
                &original,
                GameResultMode::Elimination,
                ByoyomiEntryOrder::Simultaneous,
            )
            .unwrap();
        assert_eq!(original, before);
        let serialized = serde_json::to_value(ClockConfig::default()).unwrap();
        assert_eq!(serialized["mainTimeMs"], 300_000);
        assert_eq!(serialized.get("main_time_ms"), None);
        let serialized = serde_json::to_value(EliminationContinuation {
            teams: vec![TeamMainTime {
                team: team("a"),
                main_time_remaining_ms: Some(1),
            }],
        })
        .unwrap();
        assert_eq!(serialized["teams"][0]["mainTimeRemainingMs"], 1);
        assert_eq!(serialized.get("main_time_remaining_ms"), None);
        assert!(matches!(ruling, CompetitionRuling::NeedsTiebreak { .. }));
    }
}
