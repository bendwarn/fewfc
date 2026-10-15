//! 純 Rust 的賽事交手生命週期與裁判更正核心。
//!
//! 本模組只編排已由可信遊戲／計時來源提交的局結果，不讀取網路、資料庫或真實房間。
//! 原始 `GameConclusion`、終止證據、競賽層 `CompetitionRuling` 與各自的紀錄 ID 永遠分開保存。

use crate::competition_clock::{
    CompetitionRuling, EliminationContinuation, EliminationDrawLossReason, GameResultMode,
    NeedsDecisionReason, NewGameTime, RulingOutcome,
};
use crate::domain::{GameConclusion, GameOutcome, TeamId};
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

macro_rules! string_id {
    ($name:ident) => {
        #[derive(Clone, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);

        impl $name {
            pub fn new(value: impl Into<String>) -> Self {
                Self(value.into())
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl From<&str> for $name {
            fn from(value: &str) -> Self {
                Self::new(value)
            }
        }

        impl From<String> for $name {
            fn from(value: String) -> Self {
                Self::new(value)
            }
        }

        impl Display for $name {
            fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
                self.0.fmt(formatter)
            }
        }
    };
}

string_id!(CompetitionMatchId);
string_id!(GameRecordId);
string_id!(RulingRecordId);
string_id!(ResultSubmissionId);
string_id!(CorrectionId);
string_id!(RefereeId);

/// 固定局數或先取得指定勝局數的交手格式。
#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    content = "value",
    deny_unknown_fields
)]
pub enum MatchFormat {
    FixedGames(u16),
    FirstToWins(u16),
}

impl MatchFormat {
    pub(crate) fn validate(self) -> Result<(), MatchError> {
        match self {
            Self::FixedGames(games) if games > 0 => Ok(()),
            Self::FirstToWins(wins) if wins > 0 => Ok(()),
            Self::FixedGames(_) => Err(MatchError::InvalidFormat("固定局數必須大於零".to_string())),
            Self::FirstToWins(_) => {
                Err(MatchError::InvalidFormat("勝局門檻必須大於零".to_string()))
            }
        }
    }

    /// 提供給既有 `CompetitionClock::rule_game` 的和局政策，不重寫時鐘判定。
    pub fn game_result_mode(self) -> GameResultMode {
        match self {
            Self::FixedGames(_) => GameResultMode::RetainDraw,
            Self::FirstToWins(_) => GameResultMode::Elimination,
        }
    }
}

/// 一局開始時所需的編排意圖。未知先手不可由本模組猜測。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename_all_fields = "camelCase"
)]
pub enum FirstPlayerIntent {
    Assigned { team: TeamId },
    Pending { reason: String },
}

/// 普通新局與淘汰賽和局加賽使用不同意圖，避免誤把加賽當成重置時間的新局。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename_all_fields = "camelCase"
)]
pub enum GameStartKind {
    OrdinaryNewGame,
    EliminationDrawContinuation {
        continuation: EliminationContinuation,
    },
}

impl GameStartKind {
    /// 將編排意圖交給既有時鐘核心；本模組不複製或重算剩餘時間。
    pub fn required_clock_time(&self) -> NewGameTime {
        match self {
            Self::OrdinaryNewGame => NewGameTime::Reset,
            Self::EliminationDrawContinuation { .. } => NewGameTime::CarryForward,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameStartIntent {
    pub game_number: u16,
    pub kind: GameStartKind,
    pub first_player: FirstPlayerIntent,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RefereeAssignment {
    pub referee_id: RefereeId,
    /// 若裁判是參賽隊伍成員，填入其所屬隊伍；獨立裁判填 `None`。
    pub conflicted_team: Option<TeamId>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionMatchConfig {
    pub match_id: CompetitionMatchId,
    pub teams: [TeamId; 2],
    pub format: MatchFormat,
    pub first_game: FirstPlayerIntent,
    pub referees: Vec<RefereeAssignment>,
}

/// 建立交手時的完整原始設定。後續時鐘與真實房間由外層提供。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionMatch {
    config: CompetitionMatchConfig,
    status: MatchStatus,
    score: [u16; 2],
    games: Vec<GameRecord>,
    rulings: Vec<CompetitionRulingRecord>,
    submissions: Vec<TrustedGameResult>,
    corrections: Vec<RulingCorrection>,
    outcome: Option<MatchOutcome>,
    next_first_player: Option<TeamId>,
    next_game: Option<GameStartIntent>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchStatus {
    AwaitingFirstPlayer,
    Open,
    AwaitingReferee,
    Completed,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename_all_fields = "camelCase"
)]
pub enum MatchOutcome {
    Winner {
        winner_id: TeamId,
        loser_id: TeamId,
        score: [u16; 2],
    },
    Draw {
        score: [u16; 2],
    },
    /// DoubleLoss 沒有勝者；兩隊退出目前階段，由賽程依原位置處理空缺。
    DoubleLoss {
        teams: [TeamId; 2],
        progression: DoubleLossProgression,
    },
    PendingReferee {
        game_record_id: GameRecordId,
        reason: NeedsDecisionReason,
    },
}

/// 對局層只確認兩隊退出；各賽制的名次空缺由賽程處理。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    rename_all = "camelCase",
    tag = "type",
    rename_all_fields = "camelCase"
)]
pub enum DoubleLossProgression {
    StageExit,
    /// 舊 checkpoint 的讀取相容；新 DoubleLoss 不再等待裁判處理。
    PendingReferee {
        reason: String,
    },
}

/// 一份原始遊戲紀錄；競賽判定只以 `ruling_record_id` 間接引用。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GameRecord {
    pub game_number: u16,
    pub game_record_id: GameRecordId,
    pub start_kind: GameStartKind,
    pub first_player: TeamId,
    pub terminal_evidence: GameTerminalEvidence,
    pub ruling_record_id: RulingRecordId,
}

/// 一局的可信終止來源。超時不是遊戲結論，也不能藉由合成 Draw 來保存。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum GameTerminalEvidence {
    CanonicalGameEnded {
        conclusion: GameConclusion,
    },
    Timeout {
        timeout_id: String,
        expiries: Vec<TimeoutExpiry>,
        observed_at_monotonic_ms: u64,
        clock_checkpoint_digest: String,
    },
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimeoutExpiry {
    pub team: TeamId,
    pub deadline_monotonic_ms: u64,
}

/// 獨立的競賽層判定紀錄；不覆寫原始 `GameConclusion`。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompetitionRulingRecord {
    pub ruling_record_id: RulingRecordId,
    pub game_record_id: GameRecordId,
    pub ruling: CompetitionRuling,
}

/// 可信結果來源提交的完整 payload。`submission_id` 是重試冪等鍵。
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TrustedGameResult {
    pub submission_id: ResultSubmissionId,
    pub game_record_id: GameRecordId,
    pub ruling_record_id: RulingRecordId,
    pub start_kind: GameStartKind,
    pub first_player: TeamId,
    pub terminal_evidence: GameTerminalEvidence,
    pub ruling: CompetitionRuling,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RulingCorrection {
    pub correction_id: CorrectionId,
    pub referee_id: RefereeId,
    pub game_record_id: GameRecordId,
    pub previous_ruling_record_id: RulingRecordId,
    pub replacement_ruling_record_id: RulingRecordId,
    pub replacement_ruling: CompetitionRuling,
    pub reason: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum SubmissionOutcome {
    Applied,
    AlreadyRecorded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CorrectionOutcome {
    Applied,
    AlreadyRecorded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum MatchError {
    InvalidFormat(String),
    TeamsMustBeDistinct,
    TeamNotInMatch(TeamId),
    InvalidFirstPlayer(String),
    DuplicateReferee(RefereeId),
    EmptyFirstPlayerReason,
    FirstPlayerAlreadyAssigned { expected: TeamId, received: TeamId },
    MatchClosed,
    MatchAwaitingReferee,
    FirstPlayerPending { game_number: u16, reason: String },
    FirstPlayerMismatch { expected: TeamId, received: TeamId },
    GameStartMismatch,
    WrongGameNumber { expected: u16, received: u16 },
    DuplicateSubmissionIdDifferentPayload(ResultSubmissionId),
    DuplicateGameRecordId(GameRecordId),
    DuplicateRulingRecordId(RulingRecordId),
    InvalidRuling(String),
    InvalidTerminalEvidence(String),
    GameNotFound(GameRecordId),
    UnknownReferee(RefereeId),
    SelfMatchRefereeConflict(RefereeId),
    EmptyCorrectionReason,
    DuplicateCorrectionIdDifferentPayload(CorrectionId),
    DuplicateCorrectionId(CorrectionId),
    CorrectionWouldInvalidateLaterGames(GameRecordId),
}

impl Display for MatchError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormat(reason) => write!(formatter, "交手格式無效：{reason}"),
            Self::TeamsMustBeDistinct => write!(formatter, "交手隊伍必須不同"),
            Self::TeamNotInMatch(team) => {
                write!(formatter, "隊伍不在交手中：{}", team.as_str())
            }
            Self::InvalidFirstPlayer(reason) => write!(formatter, "先手設定無效：{reason}"),
            Self::DuplicateReferee(id) => write!(formatter, "裁判重複：{id}"),
            Self::EmptyFirstPlayerReason => write!(formatter, "先手待裁定必須有原因"),
            Self::FirstPlayerAlreadyAssigned { expected, received } => write!(
                formatter,
                "先手已指定：預期 {}，收到 {}",
                expected.as_str(),
                received.as_str()
            ),
            Self::MatchClosed => write!(formatter, "交手已完成"),
            Self::MatchAwaitingReferee => write!(formatter, "交手等待裁判處理"),
            Self::FirstPlayerPending {
                game_number,
                reason,
            } => {
                write!(formatter, "第 {game_number} 局先手待裁定：{reason}")
            }
            Self::FirstPlayerMismatch { expected, received } => write!(
                formatter,
                "先手不符：預期 {}，收到 {}",
                expected.as_str(),
                received.as_str()
            ),
            Self::GameStartMismatch => write!(formatter, "局開始意圖不符"),
            Self::WrongGameNumber { expected, received } => {
                write!(formatter, "局號不符：預期 {expected}，收到 {received}")
            }
            Self::DuplicateSubmissionIdDifferentPayload(id) => {
                write!(formatter, "同一提交 ID 的 payload 不同：{id}")
            }
            Self::DuplicateGameRecordId(id) => write!(formatter, "遊戲紀錄 ID 已存在：{id}"),
            Self::DuplicateRulingRecordId(id) => write!(formatter, "判定紀錄 ID 已存在：{id}"),
            Self::InvalidRuling(reason) => write!(formatter, "競賽判定無效：{reason}"),
            Self::InvalidTerminalEvidence(reason) => write!(formatter, "終止證據無效：{reason}"),
            Self::GameNotFound(id) => write!(formatter, "找不到遊戲紀錄：{id}"),
            Self::UnknownReferee(id) => write!(formatter, "裁判未被指派：{id}"),
            Self::SelfMatchRefereeConflict(id) => write!(formatter, "裁判不得裁決自己的交手：{id}"),
            Self::EmptyCorrectionReason => write!(formatter, "裁判更正必須留下原因"),
            Self::DuplicateCorrectionIdDifferentPayload(id) => {
                write!(formatter, "同一更正 ID 的 payload 不同：{id}")
            }
            Self::DuplicateCorrectionId(id) => write!(formatter, "更正 ID 已存在：{id}"),
            Self::CorrectionWouldInvalidateLaterGames(id) => {
                write!(formatter, "更正會使後續局的開始意圖失效：{id}")
            }
        }
    }
}

impl std::error::Error for MatchError {}

impl CompetitionMatch {
    pub fn new(config: CompetitionMatchConfig) -> Result<Self, MatchError> {
        config.format.validate()?;
        if config.teams[0] == config.teams[1] {
            return Err(MatchError::TeamsMustBeDistinct);
        }
        validate_first_player(&config.teams, &config.first_game)?;
        let mut referee_ids = Vec::new();
        for referee in &config.referees {
            if referee_ids.contains(&referee.referee_id) {
                return Err(MatchError::DuplicateReferee(referee.referee_id.clone()));
            }
            referee_ids.push(referee.referee_id.clone());
            if let Some(team) = &referee.conflicted_team {
                if !config.teams.contains(team) {
                    return Err(MatchError::TeamNotInMatch(team.clone()));
                }
            }
        }

        let next_first_player = match &config.first_game {
            FirstPlayerIntent::Assigned { team } => Some(team.clone()),
            FirstPlayerIntent::Pending { .. } => None,
        };
        let status = if next_first_player.is_some() {
            MatchStatus::Open
        } else {
            MatchStatus::AwaitingFirstPlayer
        };
        Ok(Self {
            next_game: Some(GameStartIntent {
                game_number: 1,
                kind: GameStartKind::OrdinaryNewGame,
                first_player: config.first_game.clone(),
            }),
            config,
            status,
            score: [0, 0],
            games: Vec::new(),
            rulings: Vec::new(),
            submissions: Vec::new(),
            corrections: Vec::new(),
            outcome: None,
            next_first_player,
        })
    }

    pub fn config(&self) -> &CompetitionMatchConfig {
        &self.config
    }

    pub fn status(&self) -> MatchStatus {
        self.status
    }

    pub fn score(&self) -> [u16; 2] {
        self.score
    }

    pub fn outcome(&self) -> Option<&MatchOutcome> {
        self.outcome.as_ref()
    }

    pub fn next_game(&self) -> Option<&GameStartIntent> {
        self.next_game.as_ref()
    }

    pub fn games(&self) -> &[GameRecord] {
        &self.games
    }

    pub fn rulings(&self) -> &[CompetitionRulingRecord] {
        &self.rulings
    }

    pub fn corrections(&self) -> &[RulingCorrection] {
        &self.corrections
    }

    pub fn ruling_for_game(&self, game_id: &GameRecordId) -> Option<&CompetitionRulingRecord> {
        let game = self
            .games
            .iter()
            .find(|game| &game.game_record_id == game_id)?;
        self.rulings
            .iter()
            .find(|ruling| ruling.ruling_record_id == game.ruling_record_id)
    }

    /// 由外層明確提供下一局先手；本模組不套用未公告的先手政策。
    pub fn assign_next_first_player(
        &mut self,
        team: TeamId,
        reason: impl Into<String>,
    ) -> Result<(), MatchError> {
        let reason = reason.into();
        self.ensure_team(&team)?;
        if matches!(self.status, MatchStatus::AwaitingReferee) {
            return Err(MatchError::MatchAwaitingReferee);
        }
        let Some(next_game) = self.next_game.as_mut() else {
            return Err(MatchError::MatchClosed);
        };
        if let FirstPlayerIntent::Assigned { team: expected } = &next_game.first_player {
            if expected == &team {
                return Ok(());
            }
            return Err(MatchError::FirstPlayerAlreadyAssigned {
                expected: expected.clone(),
                received: team,
            });
        }
        if reason.trim().is_empty() {
            return Err(MatchError::EmptyFirstPlayerReason);
        }
        self.next_first_player = Some(team.clone());
        next_game.first_player = FirstPlayerIntent::Assigned { team };
        self.status = MatchStatus::Open;
        Ok(())
    }

    /// 提交已由可信遊戲／計時層產生的結果；相同提交 ID 與相同 payload 會冪等成功。
    pub fn submit_game(
        &mut self,
        submission: TrustedGameResult,
    ) -> Result<SubmissionOutcome, MatchError> {
        if let Some(previous) = self
            .submissions
            .iter()
            .find(|previous| previous.submission_id == submission.submission_id)
        {
            if previous == &submission {
                return Ok(SubmissionOutcome::AlreadyRecorded);
            }
            return Err(MatchError::DuplicateSubmissionIdDifferentPayload(
                submission.submission_id,
            ));
        }
        if self
            .games
            .iter()
            .any(|game| game.game_record_id == submission.game_record_id)
        {
            return Err(MatchError::DuplicateGameRecordId(submission.game_record_id));
        }
        if self
            .rulings
            .iter()
            .any(|ruling| ruling.ruling_record_id == submission.ruling_record_id)
        {
            return Err(MatchError::DuplicateRulingRecordId(
                submission.ruling_record_id,
            ));
        }
        let Some(next_game) = self.next_game.as_ref() else {
            return Err(match self.status {
                MatchStatus::AwaitingReferee => MatchError::MatchAwaitingReferee,
                _ => MatchError::MatchClosed,
            });
        };
        if self.status == MatchStatus::AwaitingReferee {
            return Err(MatchError::MatchAwaitingReferee);
        }
        if submission.game_record_id.as_str().is_empty()
            || submission.ruling_record_id.as_str().is_empty()
            || submission.submission_id.as_str().is_empty()
        {
            return Err(MatchError::InvalidRuling(
                "結果紀錄 ID 不可為空".to_string(),
            ));
        }
        if submission.start_kind != next_game.kind {
            return Err(MatchError::GameStartMismatch);
        }
        if submission.first_player
            != self.next_first_player.clone().ok_or_else(|| {
                let reason = match &next_game.first_player {
                    FirstPlayerIntent::Pending { reason } => reason.clone(),
                    FirstPlayerIntent::Assigned { .. } => "先手尚未可用".to_string(),
                };
                MatchError::FirstPlayerPending {
                    game_number: next_game.game_number,
                    reason,
                }
            })?
        {
            return Err(MatchError::FirstPlayerMismatch {
                expected: self.next_first_player.clone().expect("已在上一個判斷取出"),
                received: submission.first_player,
            });
        }
        self.ensure_team(&submission.first_player)?;
        let expected_number = next_game.game_number;
        if self.games.len() + 1 != expected_number as usize {
            return Err(MatchError::WrongGameNumber {
                expected: self.games.len() as u16 + 1,
                received: expected_number,
            });
        }
        validate_ruling(
            &self.config.teams,
            self.config.format,
            &submission.terminal_evidence,
            &submission.ruling,
        )?;

        let mut candidate = self.clone();
        candidate.games.push(GameRecord {
            game_number: expected_number,
            game_record_id: submission.game_record_id.clone(),
            start_kind: submission.start_kind.clone(),
            first_player: submission.first_player.clone(),
            terminal_evidence: submission.terminal_evidence.clone(),
            ruling_record_id: submission.ruling_record_id.clone(),
        });
        candidate.rulings.push(CompetitionRulingRecord {
            ruling_record_id: submission.ruling_record_id.clone(),
            game_record_id: submission.game_record_id.clone(),
            ruling: submission.ruling.clone(),
        });
        candidate.submissions.push(submission);
        candidate.next_first_player = None;
        candidate.rebuild_progress()?;
        *self = candidate;
        Ok(SubmissionOutcome::Applied)
    }

    /// 裁判只更正競賽判定，保留原始遊戲結論及所有舊判定紀錄。
    pub fn correct_ruling(
        &mut self,
        correction_id: CorrectionId,
        referee_id: RefereeId,
        game_record_id: GameRecordId,
        replacement_ruling_record_id: RulingRecordId,
        replacement_ruling: CompetitionRuling,
        reason: impl Into<String>,
    ) -> Result<CorrectionOutcome, MatchError> {
        let reason = reason.into();
        if let Some(previous) = self
            .corrections
            .iter()
            .find(|correction| correction.correction_id == correction_id)
        {
            if previous.referee_id == referee_id
                && previous.game_record_id == game_record_id
                && previous.replacement_ruling_record_id == replacement_ruling_record_id
                && previous.replacement_ruling == replacement_ruling
                && previous.reason == reason
            {
                return Ok(CorrectionOutcome::AlreadyRecorded);
            }
            return Err(MatchError::DuplicateCorrectionIdDifferentPayload(
                correction_id,
            ));
        }
        if reason.trim().is_empty() {
            return Err(MatchError::EmptyCorrectionReason);
        }
        let referee = self
            .config
            .referees
            .iter()
            .find(|referee| referee.referee_id == referee_id)
            .ok_or_else(|| MatchError::UnknownReferee(referee_id.clone()))?;
        if referee
            .conflicted_team
            .as_ref()
            .is_some_and(|team| self.config.teams.contains(team))
        {
            return Err(MatchError::SelfMatchRefereeConflict(referee_id));
        }
        if self
            .rulings
            .iter()
            .any(|ruling| ruling.ruling_record_id == replacement_ruling_record_id)
        {
            return Err(MatchError::DuplicateRulingRecordId(
                replacement_ruling_record_id,
            ));
        }
        let game_index = self
            .games
            .iter()
            .position(|game| game.game_record_id == game_record_id)
            .ok_or_else(|| MatchError::GameNotFound(game_record_id.clone()))?;
        let game = &self.games[game_index];
        validate_ruling(
            &self.config.teams,
            self.config.format,
            &game.terminal_evidence,
            &replacement_ruling,
        )?;
        let previous_ruling_record_id = game.ruling_record_id.clone();
        let mut candidate = self.clone();
        candidate.rulings.push(CompetitionRulingRecord {
            ruling_record_id: replacement_ruling_record_id.clone(),
            game_record_id: game_record_id.clone(),
            ruling: replacement_ruling.clone(),
        });
        candidate.games[game_index].ruling_record_id = replacement_ruling_record_id.clone();
        candidate.corrections.push(RulingCorrection {
            correction_id,
            referee_id,
            game_record_id: game_record_id.clone(),
            previous_ruling_record_id,
            replacement_ruling_record_id,
            replacement_ruling,
            reason,
        });
        candidate.rebuild_progress().map_err(|error| match error {
            MatchError::GameStartMismatch => {
                MatchError::CorrectionWouldInvalidateLaterGames(game_record_id)
            }
            other => other,
        })?;
        *self = candidate;
        Ok(CorrectionOutcome::Applied)
    }

    fn ensure_team(&self, team: &TeamId) -> Result<(), MatchError> {
        if self.config.teams.contains(team) {
            Ok(())
        } else {
            Err(MatchError::TeamNotInMatch(team.clone()))
        }
    }

    fn rebuild_progress(&mut self) -> Result<(), MatchError> {
        self.score = [0, 0];
        self.outcome = None;
        let mut expected_kind = GameStartKind::OrdinaryNewGame;
        for (index, game) in self.games.clone().into_iter().enumerate() {
            if game.game_number != index as u16 + 1 || game.start_kind != expected_kind {
                return Err(MatchError::GameStartMismatch);
            }
            let ruling = self
                .rulings
                .iter()
                .find(|ruling| ruling.ruling_record_id == game.ruling_record_id)
                .map(|ruling| ruling.ruling.clone())
                .ok_or_else(|| MatchError::GameNotFound(game.game_record_id.clone()))?;
            match game_resolution(&self.config.teams, self.config.format, &game, &ruling)? {
                GameResolution::Winner(team) => {
                    self.increment_score(&team)?;
                    if self.series_complete(index as u16 + 1) {
                        self.outcome = Some(self.completed_outcome());
                        if index + 1 != self.games.len() {
                            return Err(MatchError::CorrectionWouldInvalidateLaterGames(
                                game.game_record_id.clone(),
                            ));
                        }
                        self.status = MatchStatus::Completed;
                        self.next_game = None;
                        self.next_first_player = None;
                        return Ok(());
                    }
                    self.next_first_player = None;
                    expected_kind = GameStartKind::OrdinaryNewGame;
                }
                GameResolution::Draw => {
                    self.next_first_player =
                        Some(other_team(&self.config.teams, &game.first_player));
                    expected_kind = GameStartKind::OrdinaryNewGame;
                    if let MatchFormat::FixedGames(total) = self.config.format {
                        if index as u16 + 1 == total {
                            self.outcome = Some(self.completed_outcome());
                            if index + 1 != self.games.len() {
                                return Err(MatchError::CorrectionWouldInvalidateLaterGames(
                                    game.game_record_id.clone(),
                                ));
                            }
                            self.status = MatchStatus::Completed;
                            self.next_game = None;
                            self.next_first_player = None;
                            return Ok(());
                        }
                    }
                }
                GameResolution::EliminationDrawContinuation { continuation } => {
                    self.next_first_player =
                        Some(other_team(&self.config.teams, &game.first_player));
                    expected_kind = GameStartKind::EliminationDrawContinuation { continuation };
                }
                GameResolution::DoubleLoss { teams } => {
                    self.outcome = Some(MatchOutcome::DoubleLoss {
                        teams,
                        progression: DoubleLossProgression::StageExit,
                    });
                    if index + 1 != self.games.len() {
                        return Err(MatchError::CorrectionWouldInvalidateLaterGames(
                            game.game_record_id.clone(),
                        ));
                    }
                    self.status = MatchStatus::Completed;
                    self.next_game = None;
                    return Ok(());
                }
                GameResolution::NeedsDecision { reason } => {
                    self.outcome = Some(MatchOutcome::PendingReferee {
                        game_record_id: game.game_record_id.clone(),
                        reason,
                    });
                    if index + 1 != self.games.len() {
                        return Err(MatchError::CorrectionWouldInvalidateLaterGames(
                            game.game_record_id.clone(),
                        ));
                    }
                    self.status = MatchStatus::AwaitingReferee;
                    self.next_game = None;
                    return Ok(());
                }
            }
        }

        let game_number = self.games.len() as u16 + 1;
        let first_player = self.next_first_player.clone().map_or_else(
            || FirstPlayerIntent::Pending {
                reason: "先手政策未在交手契約中指定，需由可信編排或裁判明確決定".to_string(),
            },
            |team| FirstPlayerIntent::Assigned { team },
        );
        self.next_game = Some(GameStartIntent {
            game_number,
            kind: expected_kind,
            first_player,
        });
        self.status = if self.next_first_player.is_some() {
            MatchStatus::Open
        } else {
            MatchStatus::AwaitingFirstPlayer
        };
        Ok(())
    }

    fn increment_score(&mut self, team: &TeamId) -> Result<(), MatchError> {
        let index = self
            .config
            .teams
            .iter()
            .position(|candidate| candidate == team)
            .ok_or_else(|| MatchError::TeamNotInMatch(team.clone()))?;
        self.score[index] = self.score[index]
            .checked_add(1)
            .ok_or_else(|| MatchError::InvalidFormat("比分溢位".to_string()))?;
        Ok(())
    }

    fn series_complete(&self, processed_games: u16) -> bool {
        match self.config.format {
            MatchFormat::FixedGames(total) => processed_games >= total,
            MatchFormat::FirstToWins(wins) => self.score.iter().any(|score| *score >= wins),
        }
    }

    fn completed_outcome(&self) -> MatchOutcome {
        if matches!(self.config.format, MatchFormat::FixedGames(_))
            && self.score[0] == self.score[1]
        {
            return MatchOutcome::Draw { score: self.score };
        }
        let winner_index = if self.score[0] > self.score[1] { 0 } else { 1 };
        MatchOutcome::Winner {
            winner_id: self.config.teams[winner_index].clone(),
            loser_id: self.config.teams[1 - winner_index].clone(),
            score: self.score,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum GameResolution {
    Winner(TeamId),
    Draw,
    EliminationDrawContinuation {
        continuation: EliminationContinuation,
    },
    DoubleLoss {
        teams: [TeamId; 2],
    },
    NeedsDecision {
        reason: NeedsDecisionReason,
    },
}

fn validate_first_player(
    teams: &[TeamId; 2],
    intent: &FirstPlayerIntent,
) -> Result<(), MatchError> {
    match intent {
        FirstPlayerIntent::Assigned { team } if teams.contains(team) => Ok(()),
        FirstPlayerIntent::Assigned { team } => Err(MatchError::TeamNotInMatch(team.clone())),
        FirstPlayerIntent::Pending { reason } if !reason.trim().is_empty() => Ok(()),
        FirstPlayerIntent::Pending { .. } => Err(MatchError::EmptyFirstPlayerReason),
    }
}

fn validate_ruling(
    teams: &[TeamId; 2],
    format: MatchFormat,
    terminal_evidence: &GameTerminalEvidence,
    ruling: &CompetitionRuling,
) -> Result<(), MatchError> {
    match (terminal_evidence, ruling) {
        (GameTerminalEvidence::Timeout { .. }, CompetitionRuling::TimeoutLoss { loser }) => {
            return validate_timeout_evidence(
                teams,
                terminal_evidence,
                ExpectedTimeoutTeams::One(loser.clone()),
            );
        }
        (
            GameTerminalEvidence::Timeout { .. },
            CompetitionRuling::DoubleTimeoutLoss { teams: lost },
        ) => {
            return validate_timeout_evidence(
                teams,
                terminal_evidence,
                ExpectedTimeoutTeams::Both(lost.clone()),
            );
        }
        (GameTerminalEvidence::Timeout { .. }, _) => {
            return Err(MatchError::InvalidRuling(
                "超時終止證據只能配對超時判定".to_string(),
            ));
        }
        (
            GameTerminalEvidence::CanonicalGameEnded { .. },
            CompetitionRuling::TimeoutLoss { .. } | CompetitionRuling::DoubleTimeoutLoss { .. },
        ) => {
            return Err(MatchError::InvalidRuling(
                "原始遊戲結論不可配對超時判定".to_string(),
            ));
        }
        (
            GameTerminalEvidence::CanonicalGameEnded { conclusion },
            CompetitionRuling::GameAccepted { outcome },
        ) => {
            match (&conclusion.outcome, outcome) {
                (GameOutcome::Winner(expected), RulingOutcome::Winner(received))
                    if expected == received =>
                {
                    require_team(teams, received)?;
                }
                (GameOutcome::Draw, RulingOutcome::Draw)
                    if matches!(format, MatchFormat::FixedGames(_)) => {}
                (GameOutcome::Winner(_), RulingOutcome::Winner(_)) => {
                    return Err(MatchError::InvalidRuling(
                        "GameAccepted 的勝者必須等於原始遊戲結論".to_string(),
                    ));
                }
                (GameOutcome::Draw, RulingOutcome::Draw) => {
                    return Err(MatchError::InvalidRuling(
                        "先取 N 勝交手的和局必須走淘汰賽和局政策".to_string(),
                    ));
                }
                _ => {
                    return Err(MatchError::InvalidRuling(
                        "GameAccepted 與原始遊戲結論不一致".to_string(),
                    ));
                }
            };
            return Ok(());
        }
        (
            GameTerminalEvidence::CanonicalGameEnded { conclusion },
            CompetitionRuling::EliminationDrawLoss { loser, reason },
        ) => {
            require_team(teams, loser)?;
            if !matches!(format, MatchFormat::FirstToWins(_))
                || !matches!(reason, EliminationDrawLossReason::EnteredByoyomiFirst)
                || conclusion.outcome != GameOutcome::Draw
            {
                return Err(MatchError::InvalidRuling(
                    "淘汰賽和局判負只能用於先取 N 勝的原始和局".to_string(),
                ));
            }
            return Ok(());
        }
        (
            GameTerminalEvidence::CanonicalGameEnded { conclusion },
            CompetitionRuling::NeedsTiebreak { continuation },
        ) => {
            validate_continuation(teams, continuation)?;
            if !matches!(format, MatchFormat::FirstToWins(_))
                || conclusion.outcome != GameOutcome::Draw
            {
                return Err(MatchError::InvalidRuling(
                    "和局加賽只能用於先取 N 勝的原始和局".to_string(),
                ));
            }
            return Ok(());
        }
        (
            GameTerminalEvidence::CanonicalGameEnded { conclusion },
            CompetitionRuling::NeedsDecision { reason },
        ) => {
            match reason {
                NeedsDecisionReason::ZeroTimeControl => {
                    // 零時間控制的具體政策尚未定案，保留待裁定，不從結論猜測勝負。
                }
                NeedsDecisionReason::SimultaneousByoyomiEntry
                | NeedsDecisionReason::UnlimitedEliminationDraw
                | NeedsDecisionReason::ZeroTimeEliminationDraw => {
                    if !matches!(format, MatchFormat::FirstToWins(_))
                        || conclusion.outcome != GameOutcome::Draw
                    {
                        return Err(MatchError::InvalidRuling(
                            "此待裁定原因只能對應先取 N 勝的原始和局".to_string(),
                        ));
                    }
                }
            }
            return Ok(());
        }
    }
}

fn validate_timeout_evidence(
    teams: &[TeamId; 2],
    terminal_evidence: &GameTerminalEvidence,
    expected_teams: ExpectedTimeoutTeams,
) -> Result<(), MatchError> {
    let GameTerminalEvidence::Timeout {
        timeout_id,
        expiries,
        observed_at_monotonic_ms,
        clock_checkpoint_digest,
    } = terminal_evidence
    else {
        return Err(MatchError::InvalidTerminalEvidence(
            "超時判定必須使用 timeout 終止證據".to_string(),
        ));
    };
    if timeout_id.trim().is_empty() {
        return Err(MatchError::InvalidTerminalEvidence(
            "timeoutId 不可為空".to_string(),
        ));
    }
    if clock_checkpoint_digest.trim().is_empty() {
        return Err(MatchError::InvalidTerminalEvidence(
            "clockCheckpointDigest 不可為空".to_string(),
        ));
    }
    if !(1..=2).contains(&expiries.len()) {
        return Err(MatchError::InvalidTerminalEvidence(
            "timeout 必須提供一或兩個 expiry".to_string(),
        ));
    }
    for expiry in expiries {
        require_team(teams, &expiry.team)?;
        if expiry.deadline_monotonic_ms > *observed_at_monotonic_ms {
            return Err(MatchError::InvalidTerminalEvidence(
                "expiry deadline 不可晚於 observedAtMonotonicMs".to_string(),
            ));
        }
    }
    if expiries.len() == 2
        && (expiries[0].team == expiries[1].team
            || expiries[0].deadline_monotonic_ms != expiries[1].deadline_monotonic_ms)
    {
        return Err(MatchError::InvalidTerminalEvidence(
            "兩個 expiry 必須是雙方且 deadline 完全相同".to_string(),
        ));
    }

    match expected_teams {
        ExpectedTimeoutTeams::One(loser) => {
            if expiries.len() != 1 || expiries[0].team != loser {
                return Err(MatchError::InvalidRuling(
                    "單一 expiry 必須配對完全相同的 TimeoutLoss loser".to_string(),
                ));
            }
        }
        ExpectedTimeoutTeams::Both(lost) => {
            if expiries.len() != 2
                || !expiries.iter().all(|expiry| lost.contains(&expiry.team))
                || lost[0] == lost[1]
                || !teams.contains(&lost[0])
                || !teams.contains(&lost[1])
            {
                return Err(MatchError::InvalidRuling(
                    "兩個 expiry 必須配對涵蓋交手雙方的 DoubleTimeoutLoss".to_string(),
                ));
            }
        }
    }
    Ok(())
}

enum ExpectedTimeoutTeams {
    One(TeamId),
    Both([TeamId; 2]),
}

fn game_resolution(
    teams: &[TeamId; 2],
    format: MatchFormat,
    game: &GameRecord,
    ruling: &CompetitionRuling,
) -> Result<GameResolution, MatchError> {
    validate_ruling(teams, format, &game.terminal_evidence, ruling)?;
    Ok(match ruling {
        CompetitionRuling::TimeoutLoss { loser } => {
            GameResolution::Winner(other_team(teams, loser))
        }
        CompetitionRuling::DoubleTimeoutLoss { teams: lost } => GameResolution::DoubleLoss {
            teams: lost.clone(),
        },
        CompetitionRuling::GameAccepted { outcome } => match outcome {
            RulingOutcome::Winner(team) => GameResolution::Winner(team.clone()),
            RulingOutcome::Draw => GameResolution::Draw,
        },
        CompetitionRuling::EliminationDrawLoss { loser, .. } => {
            GameResolution::Winner(other_team(teams, loser))
        }
        CompetitionRuling::NeedsTiebreak { continuation } => {
            GameResolution::EliminationDrawContinuation {
                continuation: continuation.clone(),
            }
        }
        CompetitionRuling::NeedsDecision { reason } => {
            GameResolution::NeedsDecision { reason: *reason }
        }
    })
}

fn require_team(teams: &[TeamId; 2], team: &TeamId) -> Result<(), MatchError> {
    if teams.contains(team) {
        Ok(())
    } else {
        Err(MatchError::TeamNotInMatch(team.clone()))
    }
}

fn other_team(teams: &[TeamId; 2], loser: &TeamId) -> TeamId {
    if &teams[0] == loser {
        teams[1].clone()
    } else {
        teams[0].clone()
    }
}

fn validate_continuation(
    teams: &[TeamId; 2],
    continuation: &EliminationContinuation,
) -> Result<(), MatchError> {
    if continuation.teams.len() != 2
        || continuation.teams[0].team == continuation.teams[1].team
        || !teams.contains(&continuation.teams[0].team)
        || !teams.contains(&continuation.teams[1].team)
    {
        return Err(MatchError::InvalidRuling(
            "和局加賽必須提供交手雙方各自的剩餘主時間".to_string(),
        ));
    }
    Ok(())
}
