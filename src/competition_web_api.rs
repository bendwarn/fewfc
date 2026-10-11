//! 競賽核心的伺服器專用 JSON/WASM 橋接。
//!
//! 這個模組只負責把既有的報名、計時與賽程核心接到可信伺服器邊界。
//! 它不授權玩家、不讀取 wall clock，也不重寫任何競賽規則。`serverState`
//! 只能由受控的伺服器保存；對玩家或觀眾應只轉送 `public`／`private`。

use crate::competition_clock::{
    ByoyomiEntryOrder, ClockConfig, ClockError, ClockUpdate, CompetitionClock,
    CompetitionClockCheckpoint, CompetitionRuling, EliminationContinuation,
    EliminationDrawLossReason, GameResultMode, NeedsDecisionReason, NewGameTime, RulingOutcome,
    TeamMainTime,
};
use crate::competition_match::{
    CompetitionMatch, CompetitionMatchConfig, CompetitionMatchId, CompetitionRulingRecord,
    CorrectionId, CorrectionOutcome, FirstPlayerIntent, GameRecord, GameRecordId, GameStartKind,
    GameTerminalEvidence, MatchError, MatchOutcome, MatchStatus, RefereeId, ResultSubmissionId,
    RulingCorrection, RulingRecordId, SubmissionOutcome, TimeoutExpiry, TrustedGameResult,
};
use crate::competition_registration::{
    CompetitionAnnouncement, CompetitionRulesSettings, RegistrationAction, RegistrationActor,
    RegistrationCommand, RegistrationError, RegistrationState, TrustedTimestamp,
    resolve_competition_settings,
};
use crate::domain::{GameConclusion, GameEndCause, GameOutcome, TeamId as DomainTeamId};
use crate::tournament::{
    MatchId, MatchResult, QualifiedStagePlanError, QualifiedStageTemplate, ScheduleError, StageId,
    TeamId as ScheduleTeamId, Tournament, TournamentConfig, UpdateOutcome,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

/// 由伺服器驗證後注入的操作者；request 不接受玩家自行宣稱的授權結果。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CompetitionServerActor {
    Organizer { player_id: String },
    Player { player_id: String },
    Referee { player_id: String },
}

impl CompetitionServerActor {
    fn registration_actor(&self) -> Result<RegistrationActor, BridgeError> {
        match self {
            Self::Organizer { player_id } => Ok(RegistrationActor::Organizer(
                crate::competition_registration::RegistrationPlayerId::new(player_id),
            )),
            Self::Player { player_id } => Ok(RegistrationActor::Player(
                crate::competition_registration::RegistrationPlayerId::new(player_id),
            )),
            Self::Referee { .. } => Err(BridgeError::new(
                "unauthorizedActor",
                "authorization",
                "裁判不能代替報名主辦方或隊員執行報名命令",
            )),
        }
    }

    fn player_id(&self) -> Option<&str> {
        match self {
            Self::Player { player_id } => Some(player_id),
            Self::Organizer { .. } | Self::Referee { .. } => None,
        }
    }

    fn require_official(&self) -> Result<(), BridgeError> {
        match self {
            Self::Organizer { .. } | Self::Referee { .. } => Ok(()),
            Self::Player { .. } => Err(BridgeError::new(
                "unauthorizedActor",
                "authorization",
                "玩家 actor 不得直接操作競賽時鐘或賽程",
            )),
        }
    }
}

/// 報名狀態的可驗證 checkpoint。核心刻意不公開內部狀態，因此以命令歷史重播。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrationCheckpoint {
    pub announcement: CompetitionAnnouncement,
    pub commands: Vec<RegistrationCheckpointCommand>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RegistrationCheckpointCommand {
    pub command_id: String,
    pub expected_revision: u64,
    pub server_actor: CompetitionServerActor,
    pub action: RegistrationAction,
    pub trusted_time_ms: u64,
}

/// 三個既有核心的伺服器 checkpoint。不可把整個欄位直接送給玩家。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompetitionServerState {
    pub registration: Option<RegistrationCheckpoint>,
    /// E1 起以 match id 分開保存交手；`clock` 仍是 E2 前的單一過渡欄位。
    pub clock: Option<CompetitionClockCheckpoint>,
    pub tournament: Option<Tournament>,
    #[serde(default)]
    pub matches: BTreeMap<String, CompetitionMatchCheckpoint>,
}

/// 交手 checkpoint 只保存原始設定與已接受事件，不把 mutable `CompetitionMatch`
/// snapshot 當成可信來源；恢復時必須從 config 重建並重播 history。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompetitionMatchCheckpoint {
    pub config: CompetitionMatchConfig,
    pub history: Vec<CompetitionMatchCheckpointEvent>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum CompetitionMatchCheckpointEvent {
    AssignFirstPlayer {
        team: DomainTeamId,
        reason: String,
    },
    SubmitGame {
        submission: WebTrustedGameResult,
    },
    CorrectRuling {
        correction_id: CorrectionId,
        referee_id: RefereeId,
        game_record_id: GameRecordId,
        replacement_ruling_record_id: RulingRecordId,
        replacement_ruling: WebCompetitionRuling,
        reason: String,
    },
}

/// 明確的競賽 JSON request；所有時間都由可信伺服器注入，單位為 Unix 或 monotonic 毫秒，
/// 依 operation 的既有核心語意使用。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompetitionRequest {
    pub server_actor: CompetitionServerActor,
    pub trusted_time_ms: u64,
    pub operation: CompetitionOperation,
    #[serde(default)]
    pub server_state: CompetitionServerState,
}

/// 僅新查詢使用 exact 空 DTO；不改既有 operation 的未知欄位處理契約。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TournamentViewInput {}

/// 只接受既有 template；解析來源與組隊權限始終留在 Tournament 核心。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct QualifiedStageBridgeInput {
    pub template: QualifiedStageTemplate,
}

/// 純顯示 query 的時間只能來自可信 envelope，不接受 caller 提供設定或 checkpoint。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClockViewInput {}

/// 僅讀已驗證 checkpoint 的權威期限，不接受 caller 覆寫設定／時間或提交觀測。
#[derive(Clone, Debug, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ClockCheckpointMetadataInput {}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum CompetitionOperation {
    RegistrationResolveSettings(CompetitionRulesSettings),
    RegistrationCreate {
        announcement: CompetitionAnnouncement,
    },
    RegistrationExecute {
        command_id: String,
        expected_revision: u64,
        action: RegistrationAction,
    },
    RegistrationView,
    RegistrationExportLockedRoster,

    ClockCreate {
        teams: [DomainTeamId; 2],
        config: ClockConfig,
    },
    ClockBeginPreparation,
    ClockFinishPreparation {
        team: DomainTeamId,
    },
    ClockBeginAction {
        team: DomainTeamId,
        turn: u64,
    },
    ClockBeginOutOfTurnChoice {
        team: DomainTeamId,
        choice_id: String,
    },
    ClockObserve,
    ClockView(ClockViewInput),
    ClockCheckpointMetadata(ClockCheckpointMetadataInput),
    ClockEndWindow,
    ClockPause {
        reason: String,
    },
    ClockResume {
        reason: String,
    },
    ClockNewGame {
        time: NewGameTimeInput,
    },
    ClockRuleGame {
        conclusion: WebGameConclusion,
        mode: GameResultModeInput,
        #[serde(default)]
        byoyomi_entry_order: Option<ByoyomiEntryOrderInput>,
    },

    TournamentCreate {
        config: TournamentConfig,
    },
    TournamentPreviewQualifiedStage(QualifiedStageBridgeInput),
    TournamentCreateQualifiedStage(QualifiedStageBridgeInput),
    TournamentView(TournamentViewInput),
    TournamentStartMatch {
        stage_id: String,
        match_id: String,
    },
    TournamentSubmitResult {
        stage_id: String,
        match_id: String,
        result: MatchResult,
    },
    TournamentCorrectResult {
        stage_id: String,
        match_id: String,
        replacement: MatchResult,
        reason: String,
    },
    TournamentDropTeam {
        stage_id: String,
        team_id: String,
    },

    MatchCreate {
        config: CompetitionMatchConfig,
    },
    MatchView {
        match_id: CompetitionMatchId,
    },
    MatchAssignFirstPlayer {
        match_id: CompetitionMatchId,
        team: DomainTeamId,
        reason: String,
    },
    MatchSubmitGame {
        match_id: CompetitionMatchId,
        submission: WebTrustedGameResult,
    },
    MatchCorrectRuling {
        match_id: CompetitionMatchId,
        correction_id: CorrectionId,
        referee_id: RefereeId,
        game_record_id: GameRecordId,
        replacement_ruling_record_id: RulingRecordId,
        replacement_ruling: WebCompetitionRuling,
        reason: String,
    },
}

/// 核心 GameConclusion 使用領域預設 enum JSON；web bridge 以此 DTO
/// 固定 camelCase，再在可信邊界轉回核心型別。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebTrustedGameResult {
    pub submission_id: ResultSubmissionId,
    pub game_record_id: GameRecordId,
    pub ruling_record_id: RulingRecordId,
    pub start_kind: GameStartKind,
    pub first_player: DomainTeamId,
    pub terminal_evidence: WebGameTerminalEvidence,
    pub ruling: WebCompetitionRuling,
}

impl TryFrom<WebTrustedGameResult> for TrustedGameResult {
    type Error = BridgeError;

    fn try_from(value: WebTrustedGameResult) -> Result<Self, Self::Error> {
        Ok(Self {
            submission_id: value.submission_id,
            game_record_id: value.game_record_id,
            ruling_record_id: value.ruling_record_id,
            start_kind: value.start_kind,
            first_player: value.first_player,
            terminal_evidence: value.terminal_evidence.try_into()?,
            ruling: value.ruling.try_into()?,
        })
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum NewGameTimeInput {
    Reset,
    CarryForward,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum GameResultModeInput {
    Elimination,
    RetainDraw,
}

impl From<GameResultModeInput> for GameResultMode {
    fn from(value: GameResultModeInput) -> Self {
        match value {
            GameResultModeInput::Elimination => Self::Elimination,
            GameResultModeInput::RetainDraw => Self::RetainDraw,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ByoyomiEntryOrderInput {
    None,
    First { team: DomainTeamId },
    Simultaneous,
}

impl From<ByoyomiEntryOrderInput> for ByoyomiEntryOrder {
    fn from(value: ByoyomiEntryOrderInput) -> Self {
        match value {
            ByoyomiEntryOrderInput::None => Self::None,
            ByoyomiEntryOrderInput::First { team } => Self::First(team),
            ByoyomiEntryOrderInput::Simultaneous => Self::Simultaneous,
        }
    }
}

/// Web 層使用的 Game Conclusion；不用領域 enum 的預設 variant 輸出，確保契約維持 camelCase。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebGameConclusion {
    pub outcome: WebGameOutcome,
    pub causes: Vec<WebGameEndCause>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WebGameTerminalEvidence {
    CanonicalGameEnded {
        conclusion: WebGameConclusion,
    },
    Timeout {
        timeout_id: String,
        expiries: Vec<WebTimeoutExpiry>,
        observed_at_monotonic_ms: u64,
        clock_checkpoint_digest: String,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct WebTimeoutExpiry {
    pub team: DomainTeamId,
    pub deadline_monotonic_ms: u64,
}

impl TryFrom<WebGameTerminalEvidence> for GameTerminalEvidence {
    type Error = BridgeError;

    fn try_from(value: WebGameTerminalEvidence) -> Result<Self, Self::Error> {
        Ok(match value {
            WebGameTerminalEvidence::CanonicalGameEnded { conclusion } => {
                Self::CanonicalGameEnded {
                    conclusion: conclusion.try_into()?,
                }
            }
            WebGameTerminalEvidence::Timeout {
                timeout_id,
                expiries,
                observed_at_monotonic_ms,
                clock_checkpoint_digest,
            } => Self::Timeout {
                timeout_id,
                expiries: expiries
                    .into_iter()
                    .map(|expiry| TimeoutExpiry {
                        team: expiry.team,
                        deadline_monotonic_ms: expiry.deadline_monotonic_ms,
                    })
                    .collect(),
                observed_at_monotonic_ms,
                clock_checkpoint_digest,
            },
        })
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WebGameOutcome {
    Winner { team: DomainTeamId },
    Draw,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum WebGameEndCause {
    TeamHpDepleted { teams: Vec<DomainTeamId> },
    DirectVictory { rule: String, team: DomainTeamId },
}

/// Web bridge 的 ruling DTO；尤其固定 `RulingOutcome` 的外部 enum JSON，
/// 不把核心型別的序列化細節直接暴露到 TypeScript。
#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WebRulingOutcome {
    Winner(DomainTeamId),
    Draw,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WebEliminationDrawLossReason {
    EnteredByoyomiFirst,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebTeamMainTime {
    pub team: DomainTeamId,
    pub main_time_remaining_ms: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WebEliminationContinuation {
    pub teams: Vec<WebTeamMainTime>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum WebCompetitionRuling {
    TimeoutLoss {
        loser: DomainTeamId,
    },
    DoubleTimeoutLoss {
        teams: [DomainTeamId; 2],
    },
    GameAccepted {
        outcome: WebRulingOutcome,
    },
    EliminationDrawLoss {
        loser: DomainTeamId,
        reason: WebEliminationDrawLossReason,
    },
    NeedsTiebreak {
        continuation: WebEliminationContinuation,
    },
    NeedsDecision {
        reason: NeedsDecisionReason,
    },
}

impl TryFrom<WebCompetitionRuling> for CompetitionRuling {
    type Error = BridgeError;

    fn try_from(value: WebCompetitionRuling) -> Result<Self, Self::Error> {
        Ok(match value {
            WebCompetitionRuling::TimeoutLoss { loser } => Self::TimeoutLoss { loser },
            WebCompetitionRuling::DoubleTimeoutLoss { teams } => Self::DoubleTimeoutLoss { teams },
            WebCompetitionRuling::GameAccepted { outcome } => Self::GameAccepted {
                outcome: match outcome {
                    WebRulingOutcome::Winner(team) => RulingOutcome::Winner(team),
                    WebRulingOutcome::Draw => RulingOutcome::Draw,
                },
            },
            WebCompetitionRuling::EliminationDrawLoss { loser, reason } => {
                Self::EliminationDrawLoss {
                    loser,
                    reason: match reason {
                        WebEliminationDrawLossReason::EnteredByoyomiFirst => {
                            EliminationDrawLossReason::EnteredByoyomiFirst
                        }
                    },
                }
            }
            WebCompetitionRuling::NeedsTiebreak { continuation } => Self::NeedsTiebreak {
                continuation: EliminationContinuation {
                    teams: continuation
                        .teams
                        .into_iter()
                        .map(|team| TeamMainTime {
                            team: team.team,
                            main_time_remaining_ms: team.main_time_remaining_ms,
                        })
                        .collect(),
                },
            },
            WebCompetitionRuling::NeedsDecision { reason } => Self::NeedsDecision { reason },
        })
    }
}

impl From<&CompetitionRuling> for WebCompetitionRuling {
    fn from(value: &CompetitionRuling) -> Self {
        match value {
            CompetitionRuling::TimeoutLoss { loser } => Self::TimeoutLoss {
                loser: loser.clone(),
            },
            CompetitionRuling::DoubleTimeoutLoss { teams } => Self::DoubleTimeoutLoss {
                teams: teams.clone(),
            },
            CompetitionRuling::GameAccepted { outcome } => Self::GameAccepted {
                outcome: match outcome {
                    RulingOutcome::Winner(team) => WebRulingOutcome::Winner(team.clone()),
                    RulingOutcome::Draw => WebRulingOutcome::Draw,
                },
            },
            CompetitionRuling::EliminationDrawLoss { loser, reason } => Self::EliminationDrawLoss {
                loser: loser.clone(),
                reason: match reason {
                    EliminationDrawLossReason::EnteredByoyomiFirst => {
                        WebEliminationDrawLossReason::EnteredByoyomiFirst
                    }
                },
            },
            CompetitionRuling::NeedsTiebreak { continuation } => Self::NeedsTiebreak {
                continuation: WebEliminationContinuation {
                    teams: continuation
                        .teams
                        .iter()
                        .map(|team| WebTeamMainTime {
                            team: team.team.clone(),
                            main_time_remaining_ms: team.main_time_remaining_ms,
                        })
                        .collect(),
                },
            },
            CompetitionRuling::NeedsDecision { reason } => Self::NeedsDecision { reason: *reason },
        }
    }
}

impl TryFrom<WebGameConclusion> for GameConclusion {
    type Error = BridgeError;

    fn try_from(value: WebGameConclusion) -> Result<Self, Self::Error> {
        if value.causes.is_empty() {
            return Err(BridgeError::new(
                "invalidConclusion",
                "validation",
                "Game Conclusion 至少需要一個結束原因",
            ));
        }
        let outcome = match value.outcome {
            WebGameOutcome::Winner { team } => GameOutcome::Winner(team),
            WebGameOutcome::Draw => GameOutcome::Draw,
        };
        let causes = value
            .causes
            .into_iter()
            .map(|cause| match cause {
                WebGameEndCause::TeamHpDepleted { teams } => GameEndCause::TeamHpDepleted { teams },
                WebGameEndCause::DirectVictory { rule, team } => {
                    GameEndCause::DirectVictory { rule, team }
                }
            })
            .collect();
        Ok(GameConclusion::new(outcome, causes))
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClockPublicView {
    config: ClockConfig,
    teams: Vec<crate::competition_clock::TeamClockSnapshot>,
    active_window: Option<ClockPublicWindow>,
    paused_window: Option<ClockPublicWindow>,
    expired_team: Option<DomainTeamId>,
    needs_decision: Option<crate::competition_clock::NeedsDecisionReason>,
    timeout_ruling: Option<CompetitionRuling>,
    byoyomi_entry_order: ByoyomiEntryOrder,
    last_observed_ms: u64,
    next_timeout_deadline_ms: Option<u64>,
}

/// 內部一致性檢查使用的最小 metadata；不擴充顯示 DTO 或曝光任何裁判結果。
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClockCheckpointMetadata {
    next_timeout_deadline_ms: Option<u64>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
enum ClockPublicWindow {
    Preparation {
        teams: Vec<DomainTeamId>,
    },
    Action {
        team: DomainTeamId,
        turn: u64,
    },
    OutOfTurnChoice {
        team: DomainTeamId,
        choice_id: String,
    },
}

fn clock_public_view(clock: &CompetitionClock) -> Result<ClockPublicView, ClockError> {
    let active_window = match clock.active_window() {
        Some(crate::competition_clock::ActiveWindow::Preparation { teams }) => {
            Some(ClockPublicWindow::Preparation {
                teams: teams.to_vec(),
            })
        }
        Some(crate::competition_clock::ActiveWindow::Action { team, turn }) => {
            Some(ClockPublicWindow::Action {
                team: team.clone(),
                turn,
            })
        }
        Some(crate::competition_clock::ActiveWindow::OutOfTurnChoice { team, choice_id }) => {
            Some(ClockPublicWindow::OutOfTurnChoice {
                team: team.clone(),
                choice_id: choice_id.to_string(),
            })
        }
        None => None,
    };
    let paused_window = match clock.paused_window() {
        Some(crate::competition_clock::ActiveWindow::Preparation { teams }) => {
            Some(ClockPublicWindow::Preparation {
                teams: teams.to_vec(),
            })
        }
        Some(crate::competition_clock::ActiveWindow::Action { team, turn }) => {
            Some(ClockPublicWindow::Action {
                team: team.clone(),
                turn,
            })
        }
        Some(crate::competition_clock::ActiveWindow::OutOfTurnChoice { team, choice_id }) => {
            Some(ClockPublicWindow::OutOfTurnChoice {
                team: team.clone(),
                choice_id: choice_id.to_string(),
            })
        }
        None => None,
    };
    Ok(ClockPublicView {
        config: clock.config(),
        teams: clock.snapshot(),
        active_window,
        paused_window,
        expired_team: clock.expired_team().cloned(),
        needs_decision: clock.needs_decision(),
        timeout_ruling: clock.timeout_ruling(),
        byoyomi_entry_order: clock.recorded_byoyomi_order(),
        last_observed_ms: clock.last_observed_ms(),
        next_timeout_deadline_ms: clock.next_timeout_deadline_ms()?,
    })
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CompetitionSuccess {
    ok: bool,
    operation: String,
    public: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    private: Option<Value>,
    server_state: CompetitionServerState,
}

#[derive(Clone, Debug, Serialize)]
struct CompetitionFailure {
    ok: bool,
    error: BridgeError,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BridgeError {
    pub code: String,
    pub layer: String,
    pub message: String,
}

impl BridgeError {
    fn new(code: &str, layer: &str, message: &str) -> Self {
        Self {
            code: code.to_string(),
            layer: layer.to_string(),
            message: message.to_string(),
        }
    }
}

/// 供既有 WASM request 入口使用的競賽 dispatch。
pub fn handle_request_json(input: &str) -> Result<String, String> {
    let request: CompetitionRequest = match serde_json::from_str(input) {
        Ok(request) => request,
        Err(error) => {
            return serde_json::to_string(&CompetitionFailure {
                ok: false,
                error: BridgeError::new("invalidRequest", "request", &error.to_string()),
            })
            .map_err(|error| error.to_string());
        }
    };
    let operation = operation_name(&request.operation);
    // 保留合法 query 的原始保存形狀，包含舊版省略的預設欄位；不能因顯示而做 migration。
    let original_query_state = if matches!(
        &request.operation,
        CompetitionOperation::ClockView(_)
            | CompetitionOperation::ClockCheckpointMetadata(_)
            | CompetitionOperation::RegistrationResolveSettings(_)
            | CompetitionOperation::TournamentPreviewQualifiedStage(_)
    ) {
        serde_json::from_str::<Value>(input)
            .map_err(|error| error.to_string())?
            .get("serverState")
            .cloned()
    } else {
        None
    };
    let response = match dispatch(request) {
        Ok((public, private, server_state)) => CompetitionSuccess {
            ok: true,
            operation,
            public,
            private,
            server_state,
        },
        Err(error) => {
            return serde_json::to_string(&CompetitionFailure { ok: false, error })
                .map_err(|error| error.to_string());
        }
    };
    if let Some(original) = original_query_state {
        let mut value = serde_json::to_value(&response).map_err(|error| error.to_string())?;
        value["serverState"] = original;
        serde_json::to_string(&value).map_err(|error| error.to_string())
    } else {
        serde_json::to_string(&response).map_err(|error| error.to_string())
    }
}

fn dispatch(
    request: CompetitionRequest,
) -> Result<(Value, Option<Value>, CompetitionServerState), BridgeError> {
    let mut state = request.server_state;
    let now = request.trusted_time_ms;
    let actor = request.server_actor;
    match request.operation {
        CompetitionOperation::RegistrationResolveSettings(settings) => {
            actor.require_official()?;
            let resolved = resolve_competition_settings(&settings).map_err(registration_error)?;
            let public = serde_json::to_value(resolved).map_err(serialize_error)?;
            Ok((public, None, state))
        }
        CompetitionOperation::RegistrationCreate { announcement } => {
            let organizer = match &actor {
                CompetitionServerActor::Organizer { player_id } => player_id,
                _ => {
                    return Err(BridgeError::new(
                        "unauthorizedActor",
                        "authorization",
                        "只有公告主辦方 actor 可以建立報名狀態",
                    ));
                }
            };
            if organizer != announcement.organizer.as_str() {
                return Err(BridgeError::new(
                    "unauthorizedActor",
                    "authorization",
                    "server actor 不是公告主辦方",
                ));
            }
            if state.registration.is_some() {
                return Err(BridgeError::new(
                    "stateAlreadyExists",
                    "validation",
                    "registration checkpoint 已存在",
                ));
            }
            let registration =
                RegistrationState::new(announcement.clone()).map_err(registration_error)?;
            let public =
                serde_json::to_value(registration.public_view()).map_err(serialize_error)?;
            state.registration = Some(RegistrationCheckpoint {
                announcement,
                commands: Vec::new(),
            });
            Ok((public, None, state))
        }
        CompetitionOperation::RegistrationExecute {
            command_id,
            expected_revision,
            action,
        } => {
            let checkpoint = state.registration.take().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 registration checkpoint")
            })?;
            let (mut registration, mut checkpoint) = restore_registration(checkpoint)?;
            let registration_actor = actor.registration_actor()?;
            let before_revision = registration.revision();
            let result = registration
                .execute(
                    RegistrationCommand {
                        command_id: command_id.clone(),
                        expected_revision,
                        actor: registration_actor,
                        action: action.clone(),
                    },
                    TrustedTimestamp::from_millis(now),
                )
                .map_err(registration_error)?;
            if result.revision != before_revision {
                checkpoint.commands.push(RegistrationCheckpointCommand {
                    command_id,
                    expected_revision,
                    server_actor: actor.clone(),
                    action,
                    trusted_time_ms: now,
                });
            }
            let public = serde_json::to_value(result.public_view).map_err(serialize_error)?;
            let private = actor
                .player_id()
                .map(|player_id| {
                    serde_json::to_value(registration.private_view(
                        &crate::competition_registration::RegistrationPlayerId::new(player_id),
                    ))
                    .map_err(serialize_error)
                })
                .transpose()?;
            state.registration = Some(checkpoint);
            Ok((public, private, state))
        }
        CompetitionOperation::RegistrationView => {
            let checkpoint = state.registration.take().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 registration checkpoint")
            })?;
            let (registration, checkpoint) = restore_registration(checkpoint)?;
            let public =
                serde_json::to_value(registration.public_view()).map_err(serialize_error)?;
            let private = actor
                .player_id()
                .map(|player_id| {
                    serde_json::to_value(registration.private_view(
                        &crate::competition_registration::RegistrationPlayerId::new(player_id),
                    ))
                    .map_err(serialize_error)
                })
                .transpose()?;
            state.registration = Some(checkpoint);
            Ok((public, private, state))
        }
        CompetitionOperation::RegistrationExportLockedRoster => {
            let checkpoint = state.registration.take().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 registration checkpoint")
            })?;
            let (registration, checkpoint) = restore_registration(checkpoint)?;
            let organizer = match actor {
                CompetitionServerActor::Organizer { player_id } => player_id,
                _ => {
                    return Err(BridgeError::new(
                        "unauthorizedActor",
                        "authorization",
                        "只有主辦方 actor 可以匯出鎖定名單",
                    ));
                }
            };
            if organizer != checkpoint.announcement.organizer.as_str() {
                return Err(BridgeError::new(
                    "unauthorizedActor",
                    "authorization",
                    "server actor 不是公告主辦方",
                ));
            }
            let locked = registration
                .export_locked_roster()
                .map_err(registration_error)?;
            let private = serde_json::to_value(locked).map_err(serialize_error)?;
            state.registration = Some(checkpoint);
            Ok((json!({ "exported": true }), Some(private), state))
        }
        CompetitionOperation::ClockCreate { teams, config } => {
            actor.require_official()?;
            if state.clock.is_some() {
                return Err(BridgeError::new(
                    "stateAlreadyExists",
                    "validation",
                    "clock checkpoint 已存在",
                ));
            }
            let clock = CompetitionClock::new(teams, config, now).map_err(clock_error)?;
            let public = serde_json::to_value(clock_public_view(&clock).map_err(clock_error)?)
                .map_err(serialize_error)?;
            state.clock = Some(clock.checkpoint());
            Ok((public, None, state))
        }
        CompetitionOperation::ClockView(_) => {
            let checkpoint = state.clock.as_ref().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 clock checkpoint")
            })?;
            let clock =
                CompetitionClock::from_checkpoint(checkpoint.clone()).map_err(clock_error)?;
            let public = serde_json::to_value(clock.display(now).map_err(clock_error)?)
                .map_err(serialize_error)?;
            Ok((public, None, state))
        }
        CompetitionOperation::ClockCheckpointMetadata(_) => {
            let checkpoint = state.clock.as_ref().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 clock checkpoint")
            })?;
            let clock =
                CompetitionClock::from_checkpoint(checkpoint.clone()).map_err(clock_error)?;
            let public = serde_json::to_value(ClockCheckpointMetadata {
                next_timeout_deadline_ms: clock.next_timeout_deadline_ms().map_err(clock_error)?,
            })
            .map_err(serialize_error)?;
            Ok((public, None, state))
        }
        operation @ (CompetitionOperation::ClockBeginPreparation
        | CompetitionOperation::ClockFinishPreparation { .. }
        | CompetitionOperation::ClockBeginAction { .. }
        | CompetitionOperation::ClockBeginOutOfTurnChoice { .. }
        | CompetitionOperation::ClockObserve
        | CompetitionOperation::ClockEndWindow
        | CompetitionOperation::ClockPause { .. }
        | CompetitionOperation::ClockResume { .. }
        | CompetitionOperation::ClockNewGame { .. }
        | CompetitionOperation::ClockRuleGame { .. }) => {
            actor.require_official()?;
            let checkpoint = state.clock.take().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 clock checkpoint")
            })?;
            let mut clock =
                CompetitionClock::from_checkpoint(checkpoint.clone()).map_err(clock_error)?;
            let public_result = if let CompetitionOperation::ClockRuleGame {
                conclusion,
                mode,
                byoyomi_entry_order,
            } = operation
            {
                let conclusion = GameConclusion::try_from(conclusion)?;
                let ruling = if let Some(order) = byoyomi_entry_order {
                    clock
                        .rule_game_with_byoyomi_order(&conclusion, mode.into(), order.into())
                        .map_err(clock_error)?
                } else {
                    clock
                        .rule_game(&conclusion, mode.into())
                        .map_err(clock_error)?
                };
                serde_json::to_value(ruling).map_err(serialize_error)?
            } else if let CompetitionOperation::ClockNewGame { time } = operation {
                let next = clock
                    .new_game(
                        match time {
                            NewGameTimeInput::Reset => NewGameTime::Reset,
                            NewGameTimeInput::CarryForward => NewGameTime::CarryForward,
                        },
                        now,
                    )
                    .map_err(clock_error)?;
                clock = next;
                json!({ "type": "newGame" })
            } else {
                let update = match operation {
                    CompetitionOperation::ClockBeginPreparation => clock.begin_preparation(now),
                    CompetitionOperation::ClockFinishPreparation { team } => {
                        clock.finish_preparation(&team, now)
                    }
                    CompetitionOperation::ClockBeginAction { team, turn } => {
                        clock.begin_action(&team, turn, now)
                    }
                    CompetitionOperation::ClockBeginOutOfTurnChoice { team, choice_id } => {
                        clock.begin_out_of_turn_choice(&team, &choice_id, now)
                    }
                    CompetitionOperation::ClockObserve => clock.observe(now),
                    CompetitionOperation::ClockEndWindow => clock.end_window(now),
                    CompetitionOperation::ClockPause { reason } => {
                        require_reason(&reason)?;
                        clock.pause(now)
                    }
                    CompetitionOperation::ClockResume { reason } => {
                        require_reason(&reason)?;
                        clock.resume(now)
                    }
                    CompetitionOperation::ClockNewGame { .. }
                    | CompetitionOperation::ClockRuleGame { .. } => unreachable!(),
                    _ => unreachable!(),
                }
                .map_err(clock_error)?;
                serde_json::to_value(clock_update(update)).map_err(serialize_error)?
            };
            let clock_view = clock_public_view(&clock).map_err(clock_error)?;
            let public = json!({
                "result": public_result,
                "clock": clock_view,
            });
            state.clock = Some(clock.checkpoint());
            Ok((public, None, state))
        }
        CompetitionOperation::TournamentCreate { config } => {
            actor.require_official()?;
            if state.tournament.is_some() {
                return Err(BridgeError::new(
                    "stateAlreadyExists",
                    "validation",
                    "tournament snapshot 已存在",
                ));
            }
            let tournament = Tournament::new(config).map_err(schedule_error)?;
            let public = serde_json::to_value(&tournament).map_err(serialize_error)?;
            state.tournament = Some(tournament);
            Ok((public, None, state))
        }
        CompetitionOperation::TournamentPreviewQualifiedStage(input) => {
            actor.require_official()?;
            let tournament = state.tournament.as_ref().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 tournament snapshot")
            })?;
            let plan = tournament
                .plan_qualified_stage(input.template)
                .map_err(qualified_stage_plan_error)?;
            let public = json!({
                "qualification": plan.qualification(),
                "plannedStage": plan.planned_stage(),
            });
            Ok((public, None, state))
        }
        CompetitionOperation::TournamentCreateQualifiedStage(input) => {
            actor.require_official()?;
            let mut tournament = state.tournament.take().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 tournament snapshot")
            })?;
            let plan = tournament
                .plan_qualified_stage(input.template)
                .map_err(qualified_stage_plan_error)?;
            let qualification = tournament
                .apply_qualified_stage(plan)
                .map_err(qualified_stage_plan_error)?;
            let current_tournament =
                serde_json::to_value(&tournament).map_err(serialize_error)?;
            let public = json!({
                "qualification": qualification,
                "currentTournament": current_tournament,
            });
            state.tournament = Some(tournament);
            Ok((public, None, state))
        }
        CompetitionOperation::TournamentView(_) => {
            let tournament = state.tournament.as_ref().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 tournament snapshot")
            })?;
            // request 的 Tournament Deserialize 已驗證快照；查詢不重建、配對或推進狀態。
            let public = serde_json::to_value(tournament).map_err(serialize_error)?;
            Ok((public, None, state))
        }
        operation @ (CompetitionOperation::TournamentStartMatch { .. }
        | CompetitionOperation::TournamentSubmitResult { .. }
        | CompetitionOperation::TournamentCorrectResult { .. }
        | CompetitionOperation::TournamentDropTeam { .. }) => {
            actor.require_official()?;
            let mut tournament = state.tournament.take().ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少 tournament snapshot")
            })?;
            let result = match operation {
                CompetitionOperation::TournamentStartMatch { stage_id, match_id } => tournament
                    .start_match(&StageId::from(stage_id), &MatchId::from(match_id))
                    .map(|()| json!({ "type": "started" })),
                CompetitionOperation::TournamentSubmitResult {
                    stage_id,
                    match_id,
                    result,
                } => tournament
                    .submit_result(&StageId::from(stage_id), &MatchId::from(match_id), result)
                    .map(|outcome| serde_json::to_value(update_outcome(outcome)).unwrap()),
                CompetitionOperation::TournamentCorrectResult {
                    stage_id,
                    match_id,
                    replacement,
                    reason,
                } => tournament
                    .correct_result(
                        &StageId::from(stage_id),
                        &MatchId::from(match_id),
                        replacement,
                        reason,
                    )
                    .map(|()| json!({ "type": "corrected" })),
                CompetitionOperation::TournamentDropTeam { stage_id, team_id } => tournament
                    .drop_team(&StageId::from(stage_id), &ScheduleTeamId::from(team_id))
                    .map(|()| json!({ "type": "dropped" })),
                _ => unreachable!(),
            }
            .map_err(schedule_error)?;
            let public = json!({
                "result": result,
                "tournament": serde_json::to_value(&tournament).map_err(serialize_error)?,
            });
            state.tournament = Some(tournament);
            Ok((public, None, state))
        }
        CompetitionOperation::MatchCreate { config } => {
            actor.require_official()?;
            validate_match_id(config.match_id.as_str())?;
            let match_id = config.match_id.as_str().to_string();
            if state.matches.contains_key(&match_id) {
                return Err(BridgeError::new(
                    "stateAlreadyExists",
                    "validation",
                    "match checkpoint 已存在",
                ));
            }
            let match_state = CompetitionMatch::new(config.clone()).map_err(match_error)?;
            let public = match_public_view(&match_state)?;
            state.matches.insert(
                match_id,
                CompetitionMatchCheckpoint {
                    config,
                    history: Vec::new(),
                },
            );
            Ok((public, None, state))
        }
        CompetitionOperation::MatchView { match_id } => {
            validate_match_id(match_id.as_str())?;
            let checkpoint = state.matches.get(match_id.as_str()).ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少指定 match checkpoint")
            })?;
            let match_state = restore_match_for_id(checkpoint, match_id.as_str())?;
            let public = match_public_view(&match_state)?;
            Ok((public, None, state))
        }
        CompetitionOperation::MatchAssignFirstPlayer {
            match_id,
            team,
            reason,
        } => {
            validate_match_id(match_id.as_str())?;
            actor.require_official()?;
            let key = match_id.as_str().to_string();
            let checkpoint = state.matches.get(&key).ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少指定 match checkpoint")
            })?;
            let mut match_state = restore_match_for_id(checkpoint, &key)?;
            let assignment_is_new = match_state
                .next_game()
                .is_some_and(|game| matches!(game.first_player, FirstPlayerIntent::Pending { .. }));
            match_state
                .assign_next_first_player(team.clone(), reason.clone())
                .map_err(match_error)?;
            let mut updated = checkpoint.clone();
            if assignment_is_new {
                updated
                    .history
                    .push(CompetitionMatchCheckpointEvent::AssignFirstPlayer { team, reason });
            }
            let public = match_public_view(&match_state)?;
            state.matches.insert(key, updated);
            Ok((public, None, state))
        }
        CompetitionOperation::MatchSubmitGame {
            match_id,
            submission,
        } => {
            validate_match_id(match_id.as_str())?;
            actor.require_official()?;
            let key = match_id.as_str().to_string();
            let checkpoint = state.matches.get(&key).ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少指定 match checkpoint")
            })?;
            let mut match_state = restore_match_for_id(checkpoint, &key)?;
            let submission_for_history = submission.clone();
            let submission = TrustedGameResult::try_from(submission)?;
            let outcome = match_state.submit_game(submission).map_err(match_error)?;
            let mut updated = checkpoint.clone();
            if matches!(outcome, SubmissionOutcome::Applied) {
                updated
                    .history
                    .push(CompetitionMatchCheckpointEvent::SubmitGame {
                        submission: submission_for_history,
                    });
            }
            let public = match_public_view(&match_state)?;
            state.matches.insert(key, updated);
            Ok((public, None, state))
        }
        CompetitionOperation::MatchCorrectRuling {
            match_id,
            correction_id,
            referee_id,
            game_record_id,
            replacement_ruling_record_id,
            replacement_ruling,
            reason,
        } => {
            validate_match_id(match_id.as_str())?;
            let actor_referee_id = match &actor {
                CompetitionServerActor::Referee { player_id } => player_id,
                _ => {
                    return Err(BridgeError::new(
                        "unauthorizedActor",
                        "authorization",
                        "只有 referee actor 可以更正交手判定",
                    ));
                }
            };
            if actor_referee_id != referee_id.as_str() {
                return Err(BridgeError::new(
                    "unauthorizedActor",
                    "authorization",
                    "actor refereeId 必須與更正 payload 相同",
                ));
            }
            let key = match_id.as_str().to_string();
            let checkpoint = state.matches.get(&key).ok_or_else(|| {
                BridgeError::new("missingState", "state", "缺少指定 match checkpoint")
            })?;
            let mut match_state = restore_match_for_id(checkpoint, &key)?;
            let correction_for_history = CompetitionMatchCheckpointEvent::CorrectRuling {
                correction_id: correction_id.clone(),
                referee_id: referee_id.clone(),
                game_record_id: game_record_id.clone(),
                replacement_ruling_record_id: replacement_ruling_record_id.clone(),
                replacement_ruling: replacement_ruling.clone(),
                reason: reason.clone(),
            };
            let replacement_ruling = replacement_ruling.try_into()?;
            let outcome = match_state
                .correct_ruling(
                    correction_id,
                    referee_id,
                    game_record_id,
                    replacement_ruling_record_id,
                    replacement_ruling,
                    reason,
                )
                .map_err(match_error)?;
            let mut updated = checkpoint.clone();
            if matches!(outcome, CorrectionOutcome::Applied) {
                updated.history.push(correction_for_history);
            }
            let public = match_public_view(&match_state)?;
            state.matches.insert(key, updated);
            Ok((public, None, state))
        }
    }
}

fn restore_match(checkpoint: &CompetitionMatchCheckpoint) -> Result<CompetitionMatch, BridgeError> {
    let mut match_state = CompetitionMatch::new(checkpoint.config.clone()).map_err(|error| {
        BridgeError::new(
            "invalidMatchCheckpoint",
            "state",
            &format!("match config 無法重建：{error:?}"),
        )
    })?;
    for event in &checkpoint.history {
        let result = match event {
            CompetitionMatchCheckpointEvent::AssignFirstPlayer { team, reason } => {
                let pending = match_state.next_game().is_some_and(|game| {
                    matches!(game.first_player, FirstPlayerIntent::Pending { .. })
                });
                if !pending {
                    return Err(invalid_match_checkpoint(
                        "先手事件不是對尚未指定的下一局重播",
                    ));
                }
                match_state.assign_next_first_player(team.clone(), reason.clone())
            }
            CompetitionMatchCheckpointEvent::SubmitGame { submission } => match_state
                .submit_game(TrustedGameResult::try_from(submission.clone())?)
                .and_then(|outcome| match outcome {
                    SubmissionOutcome::Applied => Ok(()),
                    SubmissionOutcome::AlreadyRecorded => Err(MatchError::InvalidRuling(
                        "checkpoint history 不得重複記錄同一結果".to_string(),
                    )),
                }),
            CompetitionMatchCheckpointEvent::CorrectRuling {
                correction_id,
                referee_id,
                game_record_id,
                replacement_ruling_record_id,
                replacement_ruling,
                reason,
            } => match_state
                .correct_ruling(
                    correction_id.clone(),
                    referee_id.clone(),
                    game_record_id.clone(),
                    replacement_ruling_record_id.clone(),
                    replacement_ruling.clone().try_into()?,
                    reason.clone(),
                )
                .and_then(|outcome| match outcome {
                    CorrectionOutcome::Applied => Ok(()),
                    CorrectionOutcome::AlreadyRecorded => Err(MatchError::InvalidRuling(
                        "checkpoint history 不得重複記錄同一更正".to_string(),
                    )),
                }),
        };
        if let Err(error) = result {
            return Err(invalid_match_checkpoint(&format!(
                "match history 無法重播：{error:?}"
            )));
        }
    }
    Ok(match_state)
}

fn restore_match_for_id(
    checkpoint: &CompetitionMatchCheckpoint,
    match_id: &str,
) -> Result<CompetitionMatch, BridgeError> {
    if checkpoint.config.match_id.as_str() != match_id {
        return Err(invalid_match_checkpoint(
            "checkpoint key 與 config.matchId 不一致",
        ));
    }
    restore_match(checkpoint)
}

fn invalid_match_checkpoint(message: &str) -> BridgeError {
    BridgeError::new("invalidMatchCheckpoint", "state", message)
}

fn validate_match_id(match_id: &str) -> Result<(), BridgeError> {
    if match_id.trim().is_empty() {
        return Err(BridgeError::new(
            "invalidMatchId",
            "validation",
            "matchId 不可為空",
        ));
    }
    Ok(())
}

fn match_public_view(match_state: &CompetitionMatch) -> Result<Value, BridgeError> {
    let games = match_state
        .games()
        .iter()
        .map(web_game_record)
        .collect::<Result<Vec<_>, _>>()?;
    let view = MatchPublicView {
        config: match_state.config().clone(),
        status: match_state.status(),
        score: match_state.score(),
        games,
        rulings: match_state
            .rulings()
            .iter()
            .map(web_ruling_record)
            .collect(),
        corrections: match_state
            .corrections()
            .iter()
            .map(web_ruling_correction)
            .collect(),
        outcome: match_state.outcome().cloned(),
        next_first_player: match_state
            .next_game()
            .and_then(|game| match &game.first_player {
                FirstPlayerIntent::Assigned { team } => Some(team.clone()),
                FirstPlayerIntent::Pending { .. } => None,
            }),
        next_game: match_state.next_game().cloned(),
    };
    serde_json::to_value(json!({ "match": view })).map_err(serialize_error)
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MatchPublicView {
    config: CompetitionMatchConfig,
    status: MatchStatus,
    score: [u16; 2],
    games: Vec<WebGameRecord>,
    rulings: Vec<WebCompetitionRulingRecord>,
    corrections: Vec<WebRulingCorrection>,
    outcome: Option<MatchOutcome>,
    next_first_player: Option<DomainTeamId>,
    next_game: Option<crate::competition_match::GameStartIntent>,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebCompetitionRulingRecord {
    ruling_record_id: RulingRecordId,
    game_record_id: GameRecordId,
    ruling: WebCompetitionRuling,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebRulingCorrection {
    correction_id: CorrectionId,
    referee_id: RefereeId,
    game_record_id: GameRecordId,
    previous_ruling_record_id: RulingRecordId,
    replacement_ruling_record_id: RulingRecordId,
    replacement_ruling: WebCompetitionRuling,
    reason: String,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WebGameRecord {
    game_number: u16,
    game_record_id: GameRecordId,
    start_kind: GameStartKind,
    first_player: DomainTeamId,
    terminal_evidence: WebGameTerminalEvidence,
    ruling_record_id: RulingRecordId,
}

fn web_game_record(game: &GameRecord) -> Result<WebGameRecord, BridgeError> {
    Ok(WebGameRecord {
        game_number: game.game_number,
        game_record_id: game.game_record_id.clone(),
        start_kind: game.start_kind.clone(),
        first_player: game.first_player.clone(),
        terminal_evidence: web_terminal_evidence(&game.terminal_evidence),
        ruling_record_id: game.ruling_record_id.clone(),
    })
}

fn web_ruling_record(record: &CompetitionRulingRecord) -> WebCompetitionRulingRecord {
    WebCompetitionRulingRecord {
        ruling_record_id: record.ruling_record_id.clone(),
        game_record_id: record.game_record_id.clone(),
        ruling: WebCompetitionRuling::from(&record.ruling),
    }
}

fn web_ruling_correction(correction: &RulingCorrection) -> WebRulingCorrection {
    WebRulingCorrection {
        correction_id: correction.correction_id.clone(),
        referee_id: correction.referee_id.clone(),
        game_record_id: correction.game_record_id.clone(),
        previous_ruling_record_id: correction.previous_ruling_record_id.clone(),
        replacement_ruling_record_id: correction.replacement_ruling_record_id.clone(),
        replacement_ruling: WebCompetitionRuling::from(&correction.replacement_ruling),
        reason: correction.reason.clone(),
    }
}

fn web_game_conclusion(conclusion: &GameConclusion) -> WebGameConclusion {
    let outcome = match &conclusion.outcome {
        GameOutcome::Winner(team) => WebGameOutcome::Winner { team: team.clone() },
        GameOutcome::Draw => WebGameOutcome::Draw,
    };
    let causes = conclusion
        .causes
        .iter()
        .map(|cause| match cause {
            GameEndCause::TeamHpDepleted { teams } => WebGameEndCause::TeamHpDepleted {
                teams: teams.clone(),
            },
            GameEndCause::DirectVictory { rule, team } => WebGameEndCause::DirectVictory {
                rule: rule.clone(),
                team: team.clone(),
            },
        })
        .collect();
    WebGameConclusion { outcome, causes }
}

fn web_terminal_evidence(evidence: &GameTerminalEvidence) -> WebGameTerminalEvidence {
    match evidence {
        GameTerminalEvidence::CanonicalGameEnded { conclusion } => {
            WebGameTerminalEvidence::CanonicalGameEnded {
                conclusion: web_game_conclusion(conclusion),
            }
        }
        GameTerminalEvidence::Timeout {
            timeout_id,
            expiries,
            observed_at_monotonic_ms,
            clock_checkpoint_digest,
        } => WebGameTerminalEvidence::Timeout {
            timeout_id: timeout_id.clone(),
            expiries: expiries
                .iter()
                .map(|expiry| WebTimeoutExpiry {
                    team: expiry.team.clone(),
                    deadline_monotonic_ms: expiry.deadline_monotonic_ms,
                })
                .collect(),
            observed_at_monotonic_ms: *observed_at_monotonic_ms,
            clock_checkpoint_digest: clock_checkpoint_digest.clone(),
        },
    }
}

fn restore_registration(
    checkpoint: RegistrationCheckpoint,
) -> Result<(RegistrationState, RegistrationCheckpoint), BridgeError> {
    let mut state =
        RegistrationState::new(checkpoint.announcement.clone()).map_err(registration_error)?;
    for command in &checkpoint.commands {
        if command.expected_revision != state.revision() {
            return Err(BridgeError::new(
                "invalidRegistrationCheckpoint",
                "state",
                "registration command history 的 expectedRevision 不連續",
            ));
        }
        let actor = command.server_actor.registration_actor()?;
        state
            .execute(
                RegistrationCommand {
                    command_id: command.command_id.clone(),
                    expected_revision: command.expected_revision,
                    actor,
                    action: command.action.clone(),
                },
                TrustedTimestamp::from_millis(command.trusted_time_ms),
            )
            .map_err(|error| {
                BridgeError::new(
                    "invalidRegistrationCheckpoint",
                    "state",
                    &format!("registration command history 無法重播：{error:?}"),
                )
            })?;
    }
    Ok((state, checkpoint))
}

fn clock_update(update: ClockUpdate) -> Value {
    match update {
        ClockUpdate::Unchanged => json!({ "type": "unchanged" }),
        ClockUpdate::Advanced => json!({ "type": "advanced" }),
        ClockUpdate::Paused => json!({ "type": "paused" }),
        ClockUpdate::Resumed => json!({ "type": "resumed" }),
        ClockUpdate::EnteredByoyomi { team } => json!({
            "type": "enteredByoyomi",
            "team": team,
        }),
        ClockUpdate::TimedOut { team } => json!({ "type": "timedOut", "team": team }),
        ClockUpdate::DoubleTimedOut { teams } => json!({
            "type": "doubleTimedOut",
            "teams": teams,
        }),
        ClockUpdate::NeedsDecision { reason } => json!({
            "type": "needsDecision",
            "reason": reason,
        }),
    }
}

fn require_reason(reason: &str) -> Result<(), BridgeError> {
    if reason.trim().is_empty() {
        Err(BridgeError::new(
            "emptyReason",
            "validation",
            "pause/resume reason 不可為空",
        ))
    } else {
        Ok(())
    }
}

fn update_outcome(outcome: UpdateOutcome) -> Value {
    match outcome {
        UpdateOutcome::Applied => json!({ "type": "applied" }),
        UpdateOutcome::AlreadyRecorded => json!({ "type": "alreadyRecorded" }),
        UpdateOutcome::AppliedWithScheduleError(message) => json!({
            "type": "appliedWithScheduleError",
            "scheduleError": message,
        }),
    }
}

fn operation_name(operation: &CompetitionOperation) -> String {
    match operation {
        CompetitionOperation::RegistrationResolveSettings(_) => "registrationResolveSettings",
        CompetitionOperation::RegistrationCreate { .. } => "registrationCreate",
        CompetitionOperation::RegistrationExecute { .. } => "registrationExecute",
        CompetitionOperation::RegistrationView => "registrationView",
        CompetitionOperation::RegistrationExportLockedRoster => "registrationExportLockedRoster",
        CompetitionOperation::ClockCreate { .. } => "clockCreate",
        CompetitionOperation::ClockBeginPreparation => "clockBeginPreparation",
        CompetitionOperation::ClockFinishPreparation { .. } => "clockFinishPreparation",
        CompetitionOperation::ClockBeginAction { .. } => "clockBeginAction",
        CompetitionOperation::ClockBeginOutOfTurnChoice { .. } => "clockBeginOutOfTurnChoice",
        CompetitionOperation::ClockObserve => "clockObserve",
        CompetitionOperation::ClockView(_) => "clockView",
        CompetitionOperation::ClockCheckpointMetadata(_) => "clockCheckpointMetadata",
        CompetitionOperation::ClockEndWindow => "clockEndWindow",
        CompetitionOperation::ClockPause { .. } => "clockPause",
        CompetitionOperation::ClockResume { .. } => "clockResume",
        CompetitionOperation::ClockNewGame { .. } => "clockNewGame",
        CompetitionOperation::ClockRuleGame { .. } => "clockRuleGame",
        CompetitionOperation::TournamentCreate { .. } => "tournamentCreate",
        CompetitionOperation::TournamentPreviewQualifiedStage(_) => {
            "tournamentPreviewQualifiedStage"
        }
        CompetitionOperation::TournamentCreateQualifiedStage(_) => {
            "tournamentCreateQualifiedStage"
        }
        CompetitionOperation::TournamentView(_) => "tournamentView",
        CompetitionOperation::TournamentStartMatch { .. } => "tournamentStartMatch",
        CompetitionOperation::TournamentSubmitResult { .. } => "tournamentSubmitResult",
        CompetitionOperation::TournamentCorrectResult { .. } => "tournamentCorrectResult",
        CompetitionOperation::TournamentDropTeam { .. } => "tournamentDropTeam",
        CompetitionOperation::MatchCreate { .. } => "matchCreate",
        CompetitionOperation::MatchView { .. } => "matchView",
        CompetitionOperation::MatchAssignFirstPlayer { .. } => "matchAssignFirstPlayer",
        CompetitionOperation::MatchSubmitGame { .. } => "matchSubmitGame",
        CompetitionOperation::MatchCorrectRuling { .. } => "matchCorrectRuling",
    }
    .to_string()
}

fn serialize_error(error: serde_json::Error) -> BridgeError {
    BridgeError::new("serializationFailed", "bridge", &error.to_string())
}

fn registration_error(error: RegistrationError) -> BridgeError {
    let code = match error {
        RegistrationError::InvalidAnnouncement { .. } => "invalidAnnouncement",
        RegistrationError::RulesValidation { .. } => "rulesValidation",
        RegistrationError::InvalidCommandId => "invalidCommandId",
        RegistrationError::CommandAlreadyUsed { .. } => "commandAlreadyUsed",
        RegistrationError::CommandIdConflict { .. } => "commandIdConflict",
        RegistrationError::RevisionConflict { .. } => "revisionConflict",
        RegistrationError::Unauthorized => "unauthorizedActor",
        RegistrationError::InvalidLifecycle { .. } => "invalidLifecycle",
        RegistrationError::InvalidTimestamp { .. } => "invalidTimestamp",
        RegistrationError::TimestampWentBackwards { .. } => "timestampWentBackwards",
        RegistrationError::InvalidTeam { .. } => "invalidTeam",
        RegistrationError::DuplicateTeam => "duplicateTeam",
        RegistrationError::UnknownTeam => "unknownTeam",
        RegistrationError::RosterVersionMismatch { .. } => "rosterVersionMismatch",
        RegistrationError::NotTeamMember => "notTeamMember",
        RegistrationError::NotCaptain => "notCaptain",
        RegistrationError::MissingConfirmations { .. } => "missingConfirmations",
        RegistrationError::MissingDecks { .. } => "missingDecks",
        RegistrationError::PersonalDecksDisabled => "personalDecksDisabled",
        RegistrationError::InvalidPersonalDeck { .. } => "invalidPersonalDeck",
        RegistrationError::TeamAlreadyCheckedIn => "teamAlreadyCheckedIn",
        RegistrationError::TeamWithdrawn => "teamWithdrawn",
        RegistrationError::NotInvited => "notInvited",
    };
    BridgeError::new(
        "registrationError",
        "validation",
        &format!("{code}: {error:?}"),
    )
}

fn clock_error(error: ClockError) -> BridgeError {
    let code = match error {
        ClockError::TeamNotRegistered(_) => "teamNotRegistered",
        ClockError::TeamsMustBeDistinct => "teamsMustBeDistinct",
        ClockError::ExactlyTwoTeamsRequired => "exactlyTwoTeamsRequired",
        ClockError::InvalidConfig(_) => "invalidClockConfig",
        ClockError::MonotonicTimeWentBackwards { .. } => "monotonicTimeWentBackwards",
        ClockError::TimeArithmeticOverflow => "timeArithmeticOverflow",
        ClockError::WindowAlreadyActive => "windowAlreadyActive",
        ClockError::NoActiveWindow => "noActiveWindow",
        ClockError::ActiveWindowMismatch => "activeWindowMismatch",
        ClockError::ActionTurnWentBackwards { .. } => "actionTurnWentBackwards",
        ClockError::ChoiceIdEmpty => "choiceIdEmpty",
        ClockError::ClockAlreadyExpired(_) => "clockAlreadyExpired",
        ClockError::ClockAlreadyDoubleExpired => "clockAlreadyDoubleExpired",
        ClockError::ClockAlreadyPaused => "clockAlreadyPaused",
        ClockError::ClockNotPaused => "clockNotPaused",
        ClockError::ClockIsPaused => "clockIsPaused",
        ClockError::PreparationNotComplete => "preparationNotComplete",
        ClockError::CannotCarryForwardAfterDecision => "cannotCarryForwardAfterDecision",
        ClockError::InvalidCheckpoint(_) => "invalidCheckpoint",
    };
    BridgeError::new("clockError", "validation", &format!("{code}: {error:?}"))
}

fn schedule_error(error: ScheduleError) -> BridgeError {
    let code = match error {
        ScheduleError::EmptyTeams => "emptyTeams",
        ScheduleError::DuplicateTeam(_) => "duplicateTeam",
        ScheduleError::InvalidSeed => "invalidSeed",
        ScheduleError::InvalidMatchFormat(_) => "invalidMatchFormat",
        ScheduleError::InvalidClockConfig(_) => "invalidClockConfig",
        ScheduleError::SwissRoundsRequired => "swissRoundsRequired",
        ScheduleError::SwissPolicyRequired => "swissPolicyRequired",
        ScheduleError::InvalidSwissPolicy(_) => "invalidSwissPolicy",
        ScheduleError::NoSuchStage(_) => "noSuchStage",
        ScheduleError::NoSuchMatch(_) => "noSuchMatch",
        ScheduleError::MatchAlreadyStarted(_) => "matchAlreadyStarted",
        ScheduleError::ResultAlreadyRecorded(_) => "resultAlreadyRecorded",
        ScheduleError::ResultDoesNotMatchParticipants(_) => "resultDoesNotMatchParticipants",
        ScheduleError::DrawNotAllowed(_) => "drawNotAllowed",
        ScheduleError::InvalidScore(_) => "invalidScore",
        ScheduleError::MatchNotCompleted(_) => "matchNotCompleted",
        ScheduleError::CorrectionWouldRescheduleStartedMatch(_) => {
            "correctionWouldRescheduleStartedMatch"
        }
        ScheduleError::CorrectionWouldChangeFutureSchedule(_) => {
            "correctionWouldChangeFutureSchedule"
        }
        ScheduleError::InvalidSnapshot(_) => "invalidSnapshot",
        ScheduleError::EmptyCorrectionReason => "emptyCorrectionReason",
        ScheduleError::NoLegalSwissPairing => "noLegalSwissPairing",
        ScheduleError::DoubleLossNeedsReferee(_) => "doubleLossNeedsReferee",
        ScheduleError::InvalidOperation(_) => "invalidOperation",
    };
    BridgeError::new("scheduleError", "validation", &format!("{code}: {error}"))
}

fn qualified_stage_plan_error(error: QualifiedStagePlanError) -> BridgeError {
    let code = match &error {
        QualifiedStagePlanError::QualificationBlocked(_) => "qualificationBlocked",
        QualifiedStagePlanError::InvalidTemplate(_) => "invalidTemplate",
        QualifiedStagePlanError::DuplicateStageId(_) => "duplicateStageId",
        QualifiedStagePlanError::ForeignTournament { .. } => "foreignTournament",
        QualifiedStagePlanError::SourceChanged { .. } => "sourceChanged",
        QualifiedStagePlanError::StalePlan => "stalePlan",
    };
    BridgeError::new(code, "validation", &error.to_string())
}

fn match_error(error: MatchError) -> BridgeError {
    let code = match &error {
        MatchError::InvalidFormat(_) => "invalidFormat",
        MatchError::TeamsMustBeDistinct => "teamsMustBeDistinct",
        MatchError::TeamNotInMatch(_) => "teamNotInMatch",
        MatchError::InvalidFirstPlayer(_) => "invalidFirstPlayer",
        MatchError::DuplicateReferee(_) => "duplicateReferee",
        MatchError::EmptyFirstPlayerReason => "emptyFirstPlayerReason",
        MatchError::FirstPlayerAlreadyAssigned { .. } => "firstPlayerAlreadyAssigned",
        MatchError::MatchClosed => "matchClosed",
        MatchError::MatchAwaitingReferee => "matchAwaitingReferee",
        MatchError::FirstPlayerPending { .. } => "firstPlayerPending",
        MatchError::FirstPlayerMismatch { .. } => "firstPlayerMismatch",
        MatchError::GameStartMismatch => "gameStartMismatch",
        MatchError::WrongGameNumber { .. } => "wrongGameNumber",
        MatchError::DuplicateSubmissionIdDifferentPayload(_) => "duplicateSubmissionIdConflict",
        MatchError::DuplicateGameRecordId(_) => "duplicateGameRecordId",
        MatchError::DuplicateRulingRecordId(_) => "duplicateRulingRecordId",
        MatchError::InvalidRuling(_) => "invalidRuling",
        MatchError::InvalidTerminalEvidence(_) => "invalidTerminalEvidence",
        MatchError::GameNotFound(_) => "gameNotFound",
        MatchError::UnknownReferee(_) => "unknownReferee",
        MatchError::SelfMatchRefereeConflict(_) => "selfMatchRefereeConflict",
        MatchError::EmptyCorrectionReason => "emptyCorrectionReason",
        MatchError::DuplicateCorrectionIdDifferentPayload(_) => "duplicateCorrectionIdConflict",
        MatchError::DuplicateCorrectionId(_) => "duplicateCorrectionId",
        MatchError::CorrectionWouldInvalidateLaterGames(_) => "correctionWouldInvalidateLaterGames",
    };
    BridgeError::new("matchError", "validation", &format!("{code}: {error}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tournament::QualifiedStageTemplate;

    #[test]
    fn web_dto_uses_camel_case_for_server_contract() {
        let request = serde_json::to_value(CompetitionRequest {
            server_actor: CompetitionServerActor::Referee {
                player_id: "ref".into(),
            },
            trusted_time_ms: 42,
            operation: CompetitionOperation::ClockBeginOutOfTurnChoice {
                team: DomainTeamId::new("b"),
                choice_id: "choice-1".into(),
            },
            server_state: CompetitionServerState::default(),
        })
        .unwrap();
        assert_eq!(request["trustedTimeMs"], 42);
        assert_eq!(request["serverActor"]["playerId"], "ref");
        assert_eq!(request["operation"]["choiceId"], "choice-1");
        assert!(request.get("trusted_time_ms").is_none());
    }

    #[test]
    fn qualified_stage_bridge_operations_use_exact_camel_case_contract() {
        let template: QualifiedStageTemplate = serde_json::from_value(
            qualified_stage_bridge_template(
                "next",
                "roundRobin",
                qualified_stage_bridge_final_winner_slots(),
            ),
        )
        .unwrap();
        let preview = serde_json::to_value(
            CompetitionOperation::TournamentPreviewQualifiedStage(QualifiedStageBridgeInput {
                template: template.clone(),
            }),
        )
        .unwrap();
        let create = serde_json::to_value(
            CompetitionOperation::TournamentCreateQualifiedStage(QualifiedStageBridgeInput {
                template,
            }),
        )
        .unwrap();

        for (wire, operation) in [
            (preview, "tournamentPreviewQualifiedStage"),
            (create, "tournamentCreateQualifiedStage"),
        ] {
            assert_eq!(wire["type"], operation);
            assert_eq!(wire.as_object().unwrap().len(), 2);
            assert_eq!(wire["template"]["stageId"], "next");
            assert_eq!(wire["template"]["randomSeed"], 19);
            assert_eq!(
                wire["template"]["sourceSlots"][0]["sourceStageId"],
                "source-a"
            );
            assert_eq!(
                wire["template"]["sourceSlots"][0]["destinationSeed"],
                7
            );
            assert!(wire["template"].get("random_seed").is_none());
            assert!(wire["template"].get("source_slots").is_none());
            assert!(wire["template"]["sourceSlots"][0]
                .get("source_stage_id").is_none());
            assert!(wire["template"]["sourceSlots"][0]
                .get("destination_seed").is_none());
        }
    }

    #[test]
    fn clock_pause_resume_operations_keep_explicit_camel_case_names_and_reason() {
        let request = serde_json::to_value(CompetitionRequest {
            server_actor: CompetitionServerActor::Referee {
                player_id: "ref".into(),
            },
            trusted_time_ms: 42,
            operation: CompetitionOperation::ClockPause {
                reason: "裁判處理".into(),
            },
            server_state: CompetitionServerState::default(),
        })
        .unwrap();
        assert_eq!(request["operation"]["type"], "clockPause");
        assert_eq!(request["operation"]["reason"], "裁判處理");
        assert!(request["operation"].get("clock_pause").is_none());
    }

    #[test]
    fn terminal_evidence_dto_uses_exact_camel_case_fields() {
        let evidence = WebGameTerminalEvidence::Timeout {
            timeout_id: "timeout-1".into(),
            expiries: vec![WebTimeoutExpiry {
                team: DomainTeamId::new("a"),
                deadline_monotonic_ms: 130,
            }],
            observed_at_monotonic_ms: 140,
            clock_checkpoint_digest: "digest-1".into(),
        };
        let wire = serde_json::to_value(evidence).unwrap();
        assert_eq!(wire["type"], "timeout");
        assert_eq!(wire["timeoutId"], "timeout-1");
        assert_eq!(wire["expiries"][0]["deadlineMonotonicMs"], 130);
        assert_eq!(wire["observedAtMonotonicMs"], 140);
        assert_eq!(wire["clockCheckpointDigest"], "digest-1");
        assert!(wire.get("timeout_id").is_none());
        assert!(wire.get("deadline_monotonic_ms").is_none());
        assert!(wire.get("observed_at_monotonic_ms").is_none());
        assert!(wire.get("clock_checkpoint_digest").is_none());
    }

    fn qualified_stage_bridge_request(
        server_state: Option<Value>,
        actor: Value,
        operation: Value,
    ) -> Value {
        let mut request = json!({
            "serverActor": actor,
            "trustedTimeMs": 123,
            "operation": operation,
        });
        if let Some(server_state) = server_state {
            request["serverState"] = server_state;
        }
        request
    }

    fn qualified_stage_bridge_call(request: Value) -> Value {
        let input = request.to_string();
        let output = handle_request_json(&input).expect("public JSON bridge returns a response");
        serde_json::from_str(&output).expect("public JSON bridge response is JSON")
    }

    fn qualified_stage_bridge_call_as(
        server_state: Option<Value>,
        actor: Value,
        operation: Value,
    ) -> Value {
        qualified_stage_bridge_call(qualified_stage_bridge_request(
            server_state,
            actor,
            operation,
        ))
    }

    fn qualified_stage_bridge_organizer() -> Value {
        json!({"type": "organizer", "playerId": "organizer"})
    }

    fn qualified_stage_bridge_call_official(
        server_state: Value,
        operation: Value,
    ) -> Value {
        qualified_stage_bridge_call_as(
            Some(server_state),
            qualified_stage_bridge_organizer(),
            operation,
        )
    }

    fn qualified_stage_bridge_assert_ok(response: &Value) {
        assert_eq!(response["ok"], json!(true), "unexpected bridge response: {response}");
    }

    fn qualified_stage_bridge_assert_error(response: &Value, code: &str) {
        assert_eq!(response["ok"], json!(false), "unexpected bridge response: {response}");
        assert_eq!(response["error"]["code"], code, "unexpected bridge error: {response}");
        assert!(response.get("serverState").is_none());
    }

    fn qualified_stage_bridge_next_state(response: &Value) -> Value {
        qualified_stage_bridge_assert_ok(response);
        response["serverState"].clone()
    }

    fn qualified_stage_bridge_empty_state() -> Value {
        json!({
            "registration": null,
            "clock": null,
            "tournament": null,
            "matches": {},
        })
    }

    fn qualified_stage_bridge_state_with_other_domains() -> Value {
        let mut state = qualified_stage_bridge_empty_state();
        let registration = qualified_stage_bridge_call_official(
            state,
            json!({
                "type": "registrationCreate",
                "announcement": {
                    "organizer": "organizer",
                    "ruleVersion": "5.16",
                    "ruleset": "base",
                    "enabledRuleModules": [],
                    "teamSize": 2,
                    "initialHp": 200,
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
                },
            }),
        );
        state = qualified_stage_bridge_next_state(&registration);

        let clock = qualified_stage_bridge_call_official(
            state,
            json!({
                "type": "clockCreate",
                "teams": ["clock-a", "clock-b"],
                "config": {
                    "mainTimeMs": 300000,
                    "actionTimeMs": 30000,
                    "outOfTurnChoiceTimeMs": 30000,
                },
            }),
        );
        state = qualified_stage_bridge_next_state(&clock);

        let match_creation = qualified_stage_bridge_call_official(
            state,
            json!({
                "type": "matchCreate",
                "config": {
                    "matchId": "side-match",
                    "teams": ["match-a", "match-b"],
                    "format": {"type": "firstToWins", "value": 1},
                    "firstGame": {"type": "pending", "reason": "bridge preservation fixture"},
                    "referees": [],
                },
            }),
        );
        qualified_stage_bridge_next_state(&match_creation)
    }

    fn qualified_stage_bridge_clock_config() -> Value {
        json!({
            "mainTimeMs": 300000,
            "actionTimeMs": 30000,
            "outOfTurnChoiceTimeMs": 30000,
        })
    }

    fn qualified_stage_bridge_create_tournament(
        mut server_state: Value,
        stage_id: &str,
        team_ids: &[&str],
    ) -> Value {
        let teams = team_ids
            .iter()
            .enumerate()
            .map(|(index, team_id)| json!({"teamId": team_id, "seed": index + 1}))
            .collect::<Vec<_>>();
        let response = qualified_stage_bridge_call_official(
            server_state,
            json!({
                "type": "tournamentCreate",
                "config": {
                    "tournamentId": "qualification-bridge",
                    "stage": {
                        "stageId": stage_id,
                        "format": "roundRobin",
                        "teams": teams,
                        "randomSeed": 11,
                        "matchFormat": {"type": "fixedGames", "value": 1},
                        "clockConfig": qualified_stage_bridge_clock_config(),
                        "swissRounds": null,
                        "swissPolicy": null,
                    },
                },
            }),
        );
        server_state = qualified_stage_bridge_next_state(&response);
        server_state
    }

    fn qualified_stage_bridge_template(
        stage_id: &str,
        format: &str,
        source_slots: Value,
    ) -> Value {
        let elimination = format == "singleElimination" || format == "doubleElimination";
        let swiss_policy = if format == "swiss" {
            json!({
                "matchPoints": {"win": 3, "draw": 1, "loss": 0, "bye": 3},
                "drawCredit": {"numerator": 1, "denominator": 2},
                "opponentWinRateFloorPercent": null,
                "opponentWinRateCapPercent": null,
                "excludeByesFromOpponentWinRate": false,
                "tieBreakers": ["opponentWinRate"],
            })
        } else {
            Value::Null
        };
        json!({
            "stageId": stage_id,
            "format": format,
            "randomSeed": 19,
            "matchFormat": {
                "type": if elimination {"firstToWins"} else {"fixedGames"},
                "value": 1,
            },
            "clockConfig": qualified_stage_bridge_clock_config(),
            "swissRounds": if format == "swiss" {json!(1)} else {Value::Null},
            "swissPolicy": swiss_policy,
            "sourceSlots": source_slots,
        })
    }

    fn qualified_stage_bridge_rank_slots(stage_id: &str, first_seed: u64, second_seed: u64) -> Value {
        json!([
            {
                "sourceStageId": stage_id,
                "source": {"type": "uniqueStandingRank", "rank": 1},
                "destinationSeed": first_seed,
            },
            {
                "sourceStageId": stage_id,
                "source": {"type": "uniqueStandingRank", "rank": 2},
                "destinationSeed": second_seed,
            },
        ])
    }

    fn qualified_stage_bridge_pending_match(server_state: &Value, stage_id: &str) -> Option<Value> {
        let stage = server_state["tournament"]["stages"]
            .as_array()?
            .iter()
            .find(|stage| stage["config"]["stageId"] == stage_id)?;
        for round in stage["rounds"].as_array()? {
            for match_ in round["matches"].as_array()? {
                if match_["status"] == "pending" {
                    return Some(match_.clone());
                }
            }
        }
        None
    }

    fn qualified_stage_bridge_finish_stage(
        mut server_state: Value,
        stage_id: &str,
        preferred_winner: Option<&str>,
        draw_all: bool,
    ) -> Value {
        while let Some(match_) = qualified_stage_bridge_pending_match(&server_state, stage_id) {
            let match_id = match_["matchId"].as_str().unwrap().to_string();
            let participants = match_["participants"].as_array().unwrap();
            assert_eq!(participants.len(), 2);
            let first = participants[0].as_str().unwrap();
            let second = participants[1].as_str().unwrap();
            let start = qualified_stage_bridge_call_official(
                server_state,
                json!({"type": "tournamentStartMatch", "stageId": stage_id, "matchId": match_id}),
            );
            server_state = qualified_stage_bridge_next_state(&start);

            let result = if draw_all {
                json!({"type": "draw", "firstGames": 0, "secondGames": 0})
            } else {
                let winner = preferred_winner
                    .filter(|preferred| *preferred == first || *preferred == second)
                    .unwrap_or(first);
                let loser = if winner == first {second} else {first};
                json!({"type": "win", "winnerId": winner, "loserId": loser,
                    "winnerGames": 1, "loserGames": 0})
            };
            let submitted = qualified_stage_bridge_call_official(
                server_state,
                json!({"type": "tournamentSubmitResult", "stageId": stage_id,
                    "matchId": match_id, "result": result}),
            );
            server_state = qualified_stage_bridge_next_state(&submitted);
        }
        server_state
    }

    fn qualified_stage_bridge_stage_winner(server_state: &Value, stage_id: &str) -> String {
        let stage = server_state["tournament"]["stages"]
            .as_array()
            .unwrap()
            .iter()
            .find(|stage| stage["config"]["stageId"] == stage_id)
            .unwrap();
        stage["rounds"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|round| round["matches"].as_array().unwrap())
            .find_map(|match_| match_["result"]["winnerId"].as_str())
            .unwrap()
            .to_string()
    }

    fn qualified_stage_bridge_add_source_b_with_tournament_api(
        mut server_state: Value,
    ) -> Value {
        let mut tournament: Tournament =
            serde_json::from_value(server_state["tournament"].clone())
                .expect("fixture tournament state deserializes through its public DTO");
        let template: QualifiedStageTemplate = serde_json::from_value(
            qualified_stage_bridge_template(
                "source-b",
                "roundRobin",
                qualified_stage_bridge_rank_slots("source-a", 1, 2),
            ),
        )
        .expect("source-b fixture template deserializes through its public DTO");
        let plan = tournament
            .plan_qualified_stage(template)
            .expect("public Tournament API plans source-b from completed source-a");
        tournament
            .apply_qualified_stage(plan)
            .expect("public Tournament API applies the source-b fixture plan");
        server_state["tournament"] = serde_json::to_value(tournament)
            .expect("public Tournament state serializes back into the fixture");
        server_state
    }

    fn qualified_stage_bridge_completed_two_sources() -> Value {
        let state = qualified_stage_bridge_create_tournament(
            qualified_stage_bridge_state_with_other_domains(),
            "source-a",
            &["source-a1", "source-a2"],
        );
        let state = qualified_stage_bridge_finish_stage(state, "source-a", Some("source-a1"), false);
        let first_winner = qualified_stage_bridge_stage_winner(&state, "source-a");
        let state = qualified_stage_bridge_add_source_b_with_tournament_api(state);
        let other_winner = if first_winner == "source-a1" {"source-a2"} else {"source-a1"};
        let state = qualified_stage_bridge_finish_stage(state, "source-b", Some(other_winner), false);
        assert_ne!(first_winner, qualified_stage_bridge_stage_winner(&state, "source-b"));
        state
    }

    fn qualified_stage_bridge_finished_single_source() -> Value {
        let state = qualified_stage_bridge_create_tournament(
            qualified_stage_bridge_state_with_other_domains(),
            "source-a",
            &["source-a1", "source-a2"],
        );
        qualified_stage_bridge_finish_stage(state, "source-a", Some("source-a1"), false)
    }

    fn qualified_stage_bridge_final_winner_slots() -> Value {
        json!([
            {
                "sourceStageId": "source-a",
                "source": {"type": "finishedStageWinner"},
                "destinationSeed": 7,
            },
            {
                "sourceStageId": "source-b",
                "source": {"type": "finishedStageWinner"},
                "destinationSeed": 2,
            },
        ])
    }

    #[test]
    fn qualified_stage_bridge_previews_and_creates_all_formats_without_mutating_other_domains() {
        let server_state = qualified_stage_bridge_completed_two_sources();
        let first_winner = qualified_stage_bridge_stage_winner(&server_state, "source-a");
        let second_winner = qualified_stage_bridge_stage_winner(&server_state, "source-b");
        let expected_qualification = json!({
            "seededTeams": [
                {"teamId": first_winner, "seed": 7},
                {"teamId": second_winner, "seed": 2},
            ],
            "sourceMappings": [
                {"sourceStageId": "source-a", "source": {"type": "finishedStageWinner"},
                    "teamId": first_winner, "destinationSeed": 7},
                {"sourceStageId": "source-b", "source": {"type": "finishedStageWinner"},
                    "teamId": second_winner, "destinationSeed": 2},
            ],
        });

        for format in ["singleElimination", "doubleElimination", "roundRobin", "swiss"] {
            let template = qualified_stage_bridge_template(
                &format.to_string(),
                format,
                qualified_stage_bridge_final_winner_slots(),
            );
            let preview = qualified_stage_bridge_call_official(
                server_state.clone(),
                json!({"type": "tournamentPreviewQualifiedStage", "template": template.clone()}),
            );
            qualified_stage_bridge_assert_ok(&preview);
            assert_eq!(preview["operation"], "tournamentPreviewQualifiedStage");
            assert_eq!(preview["public"]["qualification"], expected_qualification);
            assert_eq!(preview["serverState"], server_state);
            assert_eq!(preview["public"].as_object().unwrap().len(), 2);
            let planned_stage = &preview["public"]["plannedStage"];
            assert_eq!(planned_stage["config"]["stageId"], format);
            assert_eq!(planned_stage["config"]["randomSeed"], 19);
            assert_eq!(planned_stage["config"]["format"], format);
            assert!(planned_stage["config"].get("random_seed").is_none());
            assert!(preview["public"]["qualification"].get("seeded_teams").is_none());
            assert!(preview["public"]["qualification"]["sourceMappings"][0]
                .get("source_stage_id").is_none());

            let created = qualified_stage_bridge_call_official(
                server_state.clone(),
                json!({"type": "tournamentCreateQualifiedStage", "template": template}),
            );
            qualified_stage_bridge_assert_ok(&created);
            assert_eq!(created["operation"], "tournamentCreateQualifiedStage");
            assert_eq!(created["public"]["qualification"], expected_qualification);
            assert_eq!(created["public"].as_object().unwrap().len(), 2);
            assert_eq!(created["public"]["currentTournament"], created["serverState"]["tournament"]);
            assert_eq!(created["serverState"]["tournament"]["stages"].as_array().unwrap().len(), 3);
            assert_eq!(created["serverState"]["tournament"]["stages"][2]["config"]["stageId"], format);
            assert_eq!(created["serverState"]["registration"], server_state["registration"]);
            assert_eq!(created["serverState"]["clock"], server_state["clock"]);
            assert_eq!(created["serverState"]["matches"], server_state["matches"]);
        }
    }

    #[test]
    fn qualified_stage_bridge_requires_official_actor_and_tournament_state() {
        let template = qualified_stage_bridge_template(
            "next",
            "roundRobin",
            qualified_stage_bridge_rank_slots("source-a", 1, 2),
        );
        let missing = qualified_stage_bridge_call_as(
            None,
            qualified_stage_bridge_organizer(),
            json!({"type": "tournamentPreviewQualifiedStage", "template": template.clone()}),
        );
        qualified_stage_bridge_assert_error(&missing, "missingState");

        let server_state = qualified_stage_bridge_create_tournament(
            qualified_stage_bridge_state_with_other_domains(),
            "source-a",
            &["source-a1", "source-a2"],
        );
        let player = qualified_stage_bridge_call_as(
            Some(server_state.clone()),
            json!({"type": "player", "playerId": "player-a"}),
            json!({"type": "tournamentCreateQualifiedStage", "template": template}),
        );
        qualified_stage_bridge_assert_error(&player, "unauthorizedActor");
        assert!(server_state["tournament"].is_object());

        let completed_state = qualified_stage_bridge_completed_two_sources();
        for operation_type in [
            "tournamentPreviewQualifiedStage",
            "tournamentCreateQualifiedStage",
        ] {
            let referee = qualified_stage_bridge_call_as(
                Some(completed_state.clone()),
                json!({"type": "referee", "playerId": "referee-a"}),
                json!({
                    "type": operation_type,
                    "template": qualified_stage_bridge_template(
                        "next",
                        "roundRobin",
                        qualified_stage_bridge_final_winner_slots(),
                    ),
                }),
            );
            qualified_stage_bridge_assert_ok(&referee);
            assert_eq!(referee["operation"], operation_type);
        }
    }

    #[test]
    fn qualified_stage_bridge_rejects_forged_snake_case_and_preview_proof_fields() {
        let server_state = qualified_stage_bridge_completed_two_sources();
        let template = qualified_stage_bridge_template(
            "next",
            "roundRobin",
            qualified_stage_bridge_final_winner_slots(),
        );

        for operation_type in [
            "tournamentPreviewQualifiedStage",
            "tournamentCreateQualifiedStage",
        ] {
            let mut forged = json!({"type": operation_type, "template": template.clone()});
            forged["template"]["teams"] = json!([{"teamId": "forged", "seed": 1}]);
            let response = qualified_stage_bridge_call_official(server_state.clone(), forged);
            qualified_stage_bridge_assert_error(&response, "invalidRequest");

            let mut snake_case = json!({"type": operation_type, "template": template.clone()});
            let object = snake_case["template"].as_object_mut().unwrap();
            let random_seed = object.remove("randomSeed").unwrap();
            object.insert("random_seed".into(), random_seed);
            let response = qualified_stage_bridge_call_official(server_state.clone(), snake_case);
            qualified_stage_bridge_assert_error(&response, "invalidRequest");

            let mut operation = json!({"type": operation_type, "template": template.clone()});
            operation["previewProof"] =
                json!({"seededTeams": [{"teamId": "forged", "seed": 1}]});
            let response =
                qualified_stage_bridge_call_official(server_state.clone(), operation);
            qualified_stage_bridge_assert_error(&response, "invalidRequest");
        }
    }

    #[test]
    fn qualified_stage_bridge_fails_closed_for_invalid_template_seed_and_duplicate_destination() {
        let server_state = qualified_stage_bridge_finished_single_source();
        let valid_slots = qualified_stage_bridge_rank_slots("source-a", 1, 2);

        let mut invalid_template = qualified_stage_bridge_template(
            "next",
            "roundRobin",
            valid_slots.clone(),
        );
        invalid_template["matchFormat"] = json!({"type": "fixedGames", "value": 0});
        let response = qualified_stage_bridge_call_official(
            server_state.clone(),
            json!({"type": "tournamentPreviewQualifiedStage", "template": invalid_template}),
        );
        qualified_stage_bridge_assert_error(&response, "invalidTemplate");

        let mut invalid_seed_slots = valid_slots.clone();
        invalid_seed_slots[0]["destinationSeed"] = json!(0);
        let response = qualified_stage_bridge_call_official(
            server_state.clone(),
            json!({"type": "tournamentPreviewQualifiedStage", "template":
                qualified_stage_bridge_template("next", "roundRobin", invalid_seed_slots)}),
        );
        qualified_stage_bridge_assert_error(&response, "qualificationBlocked");

        let mut duplicate_seed_slots = valid_slots;
        duplicate_seed_slots[1]["destinationSeed"] = json!(1);
        let response = qualified_stage_bridge_call_official(
            server_state.clone(),
            json!({"type": "tournamentPreviewQualifiedStage", "template":
                qualified_stage_bridge_template("next", "roundRobin", duplicate_seed_slots)}),
        );
        qualified_stage_bridge_assert_error(&response, "qualificationBlocked");

        let duplicate_target = qualified_stage_bridge_call_official(
            server_state,
            json!({"type": "tournamentPreviewQualifiedStage", "template":
                qualified_stage_bridge_template("source-a", "roundRobin",
                    qualified_stage_bridge_rank_slots("source-a", 1, 2))}),
        );
        qualified_stage_bridge_assert_error(&duplicate_target, "duplicateStageId");
    }

    #[test]
    fn qualified_stage_bridge_blocks_unfinished_tied_and_dropped_sources() {
        let unfinished_state = qualified_stage_bridge_create_tournament(
            qualified_stage_bridge_state_with_other_domains(),
            "source-a",
            &["source-a1", "source-a2"],
        );
        let unfinished = qualified_stage_bridge_call_official(
            unfinished_state,
            json!({"type": "tournamentPreviewQualifiedStage", "template":
                qualified_stage_bridge_template("next", "roundRobin",
                    qualified_stage_bridge_rank_slots("source-a", 1, 2))}),
        );
        qualified_stage_bridge_assert_error(&unfinished, "qualificationBlocked");

        let tied_state = qualified_stage_bridge_create_tournament(
            qualified_stage_bridge_state_with_other_domains(),
            "source-a",
            &["t1", "t2", "t3"],
        );
        let tied_state = qualified_stage_bridge_finish_stage(tied_state, "source-a", None, true);
        let tied_slots = json!([
            {"sourceStageId": "source-a", "source": {"type": "uniqueStandingRank", "rank": 1},
                "destinationSeed": 1},
            {"sourceStageId": "source-a", "source": {"type": "uniqueStandingRank", "rank": 3},
                "destinationSeed": 2},
        ]);
        let tied = qualified_stage_bridge_call_official(
            tied_state,
            json!({"type": "tournamentPreviewQualifiedStage", "template":
                qualified_stage_bridge_template("next", "roundRobin", tied_slots)}),
        );
        qualified_stage_bridge_assert_error(&tied, "qualificationBlocked");

        let dropped_state = qualified_stage_bridge_create_tournament(
            qualified_stage_bridge_state_with_other_domains(),
            "source-a",
            &["source-a1", "source-a2"],
        );
        let dropped_state = qualified_stage_bridge_next_state(
            &qualified_stage_bridge_call_official(
                dropped_state,
                json!({"type": "tournamentDropTeam", "stageId": "source-a", "teamId": "source-a1"}),
            ),
        );
        let dropped = qualified_stage_bridge_call_official(
            dropped_state,
            json!({"type": "tournamentPreviewQualifiedStage", "template":
                qualified_stage_bridge_template("next", "roundRobin",
                    qualified_stage_bridge_rank_slots("source-a", 1, 2))}),
        );
        qualified_stage_bridge_assert_error(&dropped, "qualificationBlocked");
    }

    #[test]
    fn qualified_stage_bridge_create_reparses_current_source_after_preview() {
        let server_state = qualified_stage_bridge_completed_two_sources();
        let template = qualified_stage_bridge_template(
            "next",
            "roundRobin",
            qualified_stage_bridge_final_winner_slots(),
        );
        let preview = qualified_stage_bridge_call_official(
            server_state.clone(),
            json!({"type": "tournamentPreviewQualifiedStage", "template": template.clone()}),
        );
        qualified_stage_bridge_assert_ok(&preview);
        let original_first_winner = qualified_stage_bridge_stage_winner(&server_state, "source-a");
        let source_b_match = qualified_stage_bridge_pending_match(&server_state, "source-b");
        let source_b_match = source_b_match.unwrap_or_else(|| {
            server_state["tournament"]["stages"]
                .as_array().unwrap().iter()
                .find(|stage| stage["config"]["stageId"] == "source-b").unwrap()
                ["rounds"][0]["matches"][0].clone()
        });
        let source_b_match_id = source_b_match["matchId"].as_str().unwrap();
        let participants = source_b_match["participants"].as_array().unwrap();
        let replacement_loser = participants
            .iter()
            .map(|team| team.as_str().unwrap())
            .find(|team| *team != original_first_winner)
            .unwrap();
        let corrected = qualified_stage_bridge_call_official(
            server_state,
            json!({
                "type": "tournamentCorrectResult",
                "stageId": "source-b",
                "matchId": source_b_match_id,
                "replacement": {"type": "win", "winnerId": original_first_winner,
                    "loserId": replacement_loser, "winnerGames": 1, "loserGames": 0},
                "reason": "用目前來源驗證 create 重新解析",
            }),
        );
        let current_state = qualified_stage_bridge_next_state(&corrected);
        assert_eq!(
            preview["public"]["qualification"]["seededTeams"][1]["teamId"],
            "source-a2",
        );
        assert_eq!(qualified_stage_bridge_stage_winner(&current_state, "source-b"), original_first_winner);

        let create = qualified_stage_bridge_call_official(
            current_state,
            json!({"type": "tournamentCreateQualifiedStage", "template": template}),
        );
        qualified_stage_bridge_assert_error(&create, "qualificationBlocked");
    }
}
