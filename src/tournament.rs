//! 純賽程領域模組。
//!
//! 這個模組只管理隊伍、階段、輪次、交手與排名，不讀取遊戲規則狀態，
//! 也不寫入 GameOutcome 或 canonical events。

use serde::{Deserialize, Serialize};
use std::cmp::Ordering;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt::{Display, Formatter};

use crate::competition_clock::{self, ClockConfig};
use crate::competition_match::MatchFormat;

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TeamId(pub String);

impl From<&str> for TeamId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for TeamId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Display for TeamId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct StageId(pub String);

impl From<&str> for StageId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for StageId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Display for StageId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MatchId(pub String);

impl From<&str> for MatchId {
    fn from(value: &str) -> Self {
        Self(value.to_string())
    }
}

impl From<String> for MatchId {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl Display for MatchId {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SeededTeam {
    pub team_id: TeamId,
    pub seed: u32,
}

/// 指定一個已定來源名額及使用者明確給定的目的 seed。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageQualificationSlot {
    pub source_stage_id: StageId,
    pub source: StageQualificationSource,
    pub destination_seed: u32,
}

/// 跨階段解析只接受既有已定勝者或循環／瑞士階段的唯一實值名次；不推導 top-N。
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type",
    deny_unknown_fields
)]
pub enum StageQualificationSource {
    FinishedStageWinner,
    UniqueStandingRank { rank: u32 },
}

/// 記錄解析後的來源關係；其目的 seed 與隊伍亦包含於 SeededTeam 映射。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StageQualificationMapping {
    pub source_stage_id: StageId,
    pub source: StageQualificationSource,
    pub team_id: TeamId,
    pub destination_seed: u32,
}

/// 只讀解析成功後的固定名單；私有欄位避免呼叫端自行建造已解析權威。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedStageQualification {
    seeded_teams: Vec<SeededTeam>,
    source_mappings: Vec<StageQualificationMapping>,
}

impl ResolvedStageQualification {
    pub fn seeded_teams(&self) -> &[SeededTeam] {
        &self.seeded_teams
    }

    pub fn source_mappings(&self) -> &[StageQualificationMapping] {
        &self.source_mappings
    }
}

/// 名額無法由目前 Tournament 權威確定時回傳的封閉阻擋集合。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum StageQualificationBlocker {
    TooFewSlots {
        count: usize,
    },
    MissingSourceStage {
        source_stage_id: StageId,
    },
    SourceStageNotFinished {
        source_stage_id: StageId,
    },
    SourceScheduleUnresolved {
        source_stage_id: StageId,
    },
    SourceMatchUnresolved {
        source_stage_id: StageId,
        match_id: MatchId,
        status: MatchStatus,
    },
    DoubleLossNeedsReferee {
        source_stage_id: StageId,
        match_id: MatchId,
    },
    WinnerNotDetermined {
        source_stage_id: StageId,
    },
    StandingRanksUnsupported {
        source_stage_id: StageId,
        format: TournamentFormat,
    },
    InvalidStandingRank {
        source_stage_id: StageId,
        rank: u32,
    },
    StandingRankUnavailable {
        source_stage_id: StageId,
        team_id: TeamId,
    },
    SourceTeamMissing {
        source_stage_id: StageId,
        team_id: TeamId,
    },
    StandingRankNotFound {
        source_stage_id: StageId,
        rank: u32,
    },
    StandingRankNotUnique {
        source_stage_id: StageId,
        rank: u32,
        team_ids: Vec<TeamId>,
    },
    DroppedTeam {
        source_stage_id: StageId,
        team_id: TeamId,
    },
    InvalidDestinationSeed {
        seed: u32,
    },
    DuplicateDestinationSeed {
        seed: u32,
    },
    DuplicateSource {
        source_stage_id: StageId,
        source: StageQualificationSource,
    },
    DuplicateTeam {
        team_id: TeamId,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TournamentFormat {
    SingleElimination,
    RoundRobin,
    DoubleElimination,
    Swiss,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RoundKind {
    Main,
    Winners,
    Losers,
    GrandFinal,
    GrandFinalReset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum StageStatus {
    Active,
    Finished,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MatchStatus {
    Pending,
    InProgress,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum MatchResult {
    // 雙方皆負，不代表和局，也不推定任何單局比分。
    DoubleLoss,
    // 賽前報到缺席是賽事判定，不代表交手或 Game 已開始。
    NoShow {
        absent_teams: Vec<TeamId>,
        winner_id: Option<TeamId>,
    },
    Win {
        winner_id: TeamId,
        loser_id: TeamId,
        winner_games: u16,
        loser_games: u16,
    },
    Draw {
        first_games: u16,
        second_games: u16,
    },
    Bye {
        team_id: TeamId,
    },
    Forfeit {
        winner_id: TeamId,
        loser_id: TeamId,
    },
}

impl MatchResult {
    pub fn win(winner_id: impl Into<TeamId>, loser_id: impl Into<TeamId>) -> Self {
        Self::Win {
            winner_id: winner_id.into(),
            loser_id: loser_id.into(),
            winner_games: 1,
            loser_games: 0,
        }
    }

    pub fn win_with_games(
        winner_id: impl Into<TeamId>,
        loser_id: impl Into<TeamId>,
        winner_games: u16,
        loser_games: u16,
    ) -> Self {
        Self::Win {
            winner_id: winner_id.into(),
            loser_id: loser_id.into(),
            winner_games,
            loser_games,
        }
    }

    pub fn draw() -> Self {
        Self::Draw {
            first_games: 0,
            second_games: 0,
        }
    }

    pub fn draw_with_games(first_games: u16, second_games: u16) -> Self {
        Self::Draw {
            first_games,
            second_games,
        }
    }

    pub fn forfeit(winner_id: impl Into<TeamId>, loser_id: impl Into<TeamId>) -> Self {
        Self::Forfeit {
            winner_id: winner_id.into(),
            loser_id: loser_id.into(),
        }
    }

    fn winner(&self) -> Option<&TeamId> {
        match self {
            Self::Win { winner_id, .. } | Self::Forfeit { winner_id, .. } => Some(winner_id),
            Self::NoShow { winner_id, .. } => winner_id.as_ref(),
            Self::Bye { team_id } => Some(team_id),
            Self::Draw { .. } | Self::DoubleLoss => None,
        }
    }

    fn loser(&self) -> Option<&TeamId> {
        match self {
            Self::Win { loser_id, .. } | Self::Forfeit { loser_id, .. } => Some(loser_id),
            Self::NoShow { absent_teams, .. } if absent_teams.len() == 1 => absent_teams.first(),
            Self::Bye { .. } | Self::Draw { .. } | Self::DoubleLoss => None,
            Self::NoShow { .. } => None,
        }
    }

    fn is_double_loss(&self) -> bool {
        matches!(
            self,
            Self::DoubleLoss
                | Self::NoShow {
                    winner_id: None,
                    ..
                }
        )
    }

    fn game_score(&self, team_id: &TeamId) -> Option<(u32, u32)> {
        match self {
            Self::Win {
                winner_id,
                loser_id,
                winner_games,
                loser_games,
            } if winner_id == team_id => Some((*winner_games as u32, *loser_games as u32)),
            Self::Win {
                winner_id,
                loser_id,
                winner_games,
                loser_games,
            } if loser_id == team_id => Some((*loser_games as u32, *winner_games as u32)),
            Self::Forfeit { .. } | Self::NoShow { .. } | Self::Bye { .. } | Self::Draw { .. } => {
                None
            }
            Self::Win { .. } | Self::DoubleLoss => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledMatch {
    pub match_id: MatchId,
    pub stage_id: StageId,
    pub round_number: u32,
    pub sequence: u32,
    pub kind: RoundKind,
    pub participants: Vec<TeamId>,
    pub status: MatchStatus,
    pub result: Option<MatchResult>,
    pub cancellation_reason: Option<String>,
    pub started: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Round {
    pub round_number: u32,
    pub kind: RoundKind,
    pub matches: Vec<ScheduledMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatchPoints {
    pub win: u32,
    pub draw: u32,
    pub loss: u32,
    pub bye: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DrawCredit {
    pub numerator: u32,
    pub denominator: u32,
}

impl DrawCredit {
    pub fn new(numerator: u32, denominator: u32) -> Result<Self, ScheduleError> {
        if denominator == 0 || numerator > denominator {
            return Err(ScheduleError::InvalidSwissPolicy(
                "drawCredit 必須是 0 到 1 之間的有理數".to_string(),
            ));
        }
        Ok(Self {
            numerator,
            denominator,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SwissTieBreaker {
    MatchPoints,
    HeadToHeadMatchPoints,
    OpponentWinRate,
    NetGameWins,
    SeedOrder,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SwissStandingPolicy {
    pub match_points: MatchPoints,
    pub draw_credit: DrawCredit,
    pub opponent_win_rate_floor_percent: Option<u8>,
    pub opponent_win_rate_cap_percent: Option<u8>,
    pub exclude_byes_from_opponent_win_rate: bool,
    pub tie_breakers: Vec<SwissTieBreaker>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StageConfig {
    pub stage_id: StageId,
    pub format: TournamentFormat,
    pub teams: Vec<SeededTeam>,
    pub random_seed: u64,
    pub match_format: MatchFormat,
    pub clock_config: ClockConfig,
    pub swiss_rounds: Option<u32>,
    pub swiss_policy: Option<SwissStandingPolicy>,
}

/// 只提供下一階段設定與來源名額；隊伍必須由 Tournament 解析得出。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QualifiedStageTemplate {
    pub stage_id: StageId,
    pub format: TournamentFormat,
    pub random_seed: u64,
    pub match_format: MatchFormat,
    pub clock_config: ClockConfig,
    pub swiss_rounds: Option<u32>,
    pub swiss_policy: Option<SwissStandingPolicy>,
    pub source_slots: Vec<StageQualificationSlot>,
}

/// 公告內固定的首輪種子政策。抽籤在報名截止後進行，結果由賽程持久保存。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum InitialSeedPolicy {
    RandomDrawAfterRegistrationDeadline,
}

/// 瑞士制只在合法配對方案間使用已保存的亂數種子，不把抽籤用作正式名次。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum SwissPairingPolicy {
    PublishedRulesWithPersistedRandomSeed,
}

/// 完整名次比較順序及全同時的限定處置政策。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CompetitionRankingPolicy {
    MatchPointsThenOpponentWinsThenOpponentLosses,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ExactRankingTiePolicy {
    OrganizerOrUnconflictedRefereeWithPublicReason,
}

/// 公告階段的參賽來源；首階段使用全部合格隊伍，後續階段列出明確來源名額。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type",
    deny_unknown_fields
)]
pub enum AnnouncedStageQualification {
    AllEligibleTeams,
    SourceSlots { slots: Vec<StageQualificationSlot> },
}

/// 賽前公告的一個階段。隊伍與隨機種子在截止後才由自動化依此政策確定。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnnouncedTournamentStage {
    pub stage_id: StageId,
    pub format: TournamentFormat,
    pub match_format: MatchFormat,
    pub clock_config: ClockConfig,
    pub swiss_rounds: Option<u32>,
    pub swiss_policy: Option<SwissStandingPolicy>,
    pub qualification: AnnouncedStageQualification,
}

/// 與規則、參賽型態及報名期限同一版保存並公開的完整競賽方案。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompetitionAnnouncementPlan {
    pub initial_seed_policy: InitialSeedPolicy,
    pub swiss_pairing_policy: SwissPairingPolicy,
    pub ranking_policy: CompetitionRankingPolicy,
    pub exact_ranking_tie_policy: ExactRankingTiePolicy,
    pub stages: Vec<AnnouncedTournamentStage>,
}

/// 驗證公告的來源圖及每階段設定；不產生隊伍、種子或配對。
pub fn validate_competition_announcement_plan(plan: &CompetitionAnnouncementPlan) -> Result<(), String> {
    if plan.stages.is_empty() {
        return Err("公告至少要有一個賽事階段".to_string());
    }

    let mut stage_ids = BTreeSet::new();
    for (index, stage) in plan.stages.iter().enumerate() {
        let stage_id = stage.stage_id.to_string();
        if stage_id.trim().is_empty()
            || stage_id.trim() != stage_id
            || !stage_ids.insert(stage.stage_id.clone())
        {
            return Err("階段識別碼不可空白或重複".to_string());
        }
        stage.match_format
            .validate()
            .map_err(|error| error.to_string())?;
        validate_announced_clock(&stage.clock_config)?;
        match (stage.format, stage.swiss_rounds, &stage.swiss_policy) {
            (TournamentFormat::Swiss, Some(rounds), Some(policy)) if rounds > 0 => {
                validate_swiss_policy(policy).map_err(|error| error.to_string())?;
            }
            (TournamentFormat::Swiss, _, _) => {
                return Err("瑞士階段必須明示正整數輪數與完整瑞士政策".to_string());
            }
            (_, None, None) => {}
            _ => return Err("非瑞士階段不可設定瑞士輪數或政策".to_string()),
        }

        match (&stage.qualification, index) {
            (AnnouncedStageQualification::AllEligibleTeams, 0) => {}
            (AnnouncedStageQualification::SourceSlots { .. }, 0) => {
                return Err("首階段必須納入全部合格隊伍".to_string());
            }
            (AnnouncedStageQualification::AllEligibleTeams, _) => {
                return Err("後續階段必須明示晉級來源".to_string());
            }
            (AnnouncedStageQualification::SourceSlots { slots }, _) if slots.is_empty() => {
                return Err("後續階段至少要設定一個晉級來源名額".to_string());
            }
            (AnnouncedStageQualification::SourceSlots { slots }, _) => {
                let mut destination_seeds = BTreeSet::new();
                let mut sources = BTreeSet::new();
                for slot in slots {
                    if slot.destination_seed == 0 || !destination_seeds.insert(slot.destination_seed) {
                        return Err("晉級名額的目的種子必須是不同的正整數".to_string());
                    }
                    if !stage_ids.contains(&slot.source_stage_id) {
                        return Err("晉級來源必須指向本階段之前已公告的階段".to_string());
                    }
                    let source_index = plan.stages[..index]
                        .iter()
                        .position(|candidate| candidate.stage_id == slot.source_stage_id)
                        .ok_or_else(|| "晉級來源必須指向本階段之前已公告的階段".to_string())?;
                    let source_stage = &plan.stages[source_index];
                    match &slot.source {
                        StageQualificationSource::FinishedStageWinner
                            if matches!(source_stage.format, TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination) => {}
                        StageQualificationSource::UniqueStandingRank { rank }
                            if *rank > 0 && matches!(source_stage.format, TournamentFormat::RoundRobin | TournamentFormat::Swiss) => {}
                        _ => return Err("晉級來源必須符合來源階段可提供的勝者或唯一名次".to_string()),
                    }
                    if !sources.insert((slot.source_stage_id.clone(), slot.source.clone())) {
                        return Err("相同晉級來源不可重複設定".to_string());
                    }
                }
            }
        }
    }
    Ok(())
}

fn validate_announced_clock(config: &ClockConfig) -> Result<(), String> {
    const BYOYOMI_OPTIONS_MS: [u64; 3] = [30_000, 60_000, 180_000];
    match config.main_time_ms {
        Some(value) if value % 60_000 == 0 => {}
        _ => return Err("主時間必須以分鐘設定；0 表示不限時".to_string()),
    }
    if !config.action_time_ms.is_some_and(|value| BYOYOMI_OPTIONS_MS.contains(&value))
        || config.action_time_ms != config.out_of_turn_choice_time_ms
    {
        return Err("讀秒只可選 30 秒、1 分鐘或 3 分鐘，且共用於行動與回合外選擇".to_string());
    }
    Ok(())
}

fn validate_swiss_policy(policy: &SwissStandingPolicy) -> Result<(), ScheduleError> {
    if let (Some(floor), Some(cap)) = (
        policy.opponent_win_rate_floor_percent,
        policy.opponent_win_rate_cap_percent,
    ) && floor > cap
    {
        return Err(ScheduleError::InvalidSwissPolicy(
            "OMW floor 不可大於 cap".to_string(),
        ));
    }
    if policy
        .opponent_win_rate_floor_percent
        .is_some_and(|floor| floor > 100)
        || policy
            .opponent_win_rate_cap_percent
            .is_some_and(|cap| cap > 100)
    {
        return Err(ScheduleError::InvalidSwissPolicy(
            "OMW floor/cap 必須介於 0 到 100".to_string(),
        ));
    }
    if policy.draw_credit.denominator == 0
        || policy.draw_credit.numerator > policy.draw_credit.denominator
    {
        return Err(ScheduleError::InvalidSwissPolicy(
            "drawCredit 必須是 0 到 1 之間的有理數".to_string(),
        ));
    }
    let mut seen_tie_breakers: Vec<SwissTieBreaker> = Vec::new();
    for tie_breaker in &policy.tie_breakers {
        if seen_tie_breakers.contains(tie_breaker) {
            return Err(ScheduleError::InvalidSwissPolicy(
                "同分比較項目不可重複".to_string(),
            ));
        }
        seen_tie_breakers.push(tie_breaker.clone());
    }
    Ok(())
}

impl QualifiedStageTemplate {
    fn stage_config(&self, teams: Vec<SeededTeam>) -> StageConfig {
        StageConfig {
            stage_id: self.stage_id.clone(),
            format: self.format,
            teams,
            random_seed: self.random_seed,
            match_format: self.match_format.clone(),
            clock_config: self.clock_config.clone(),
            swiss_rounds: self.swiss_rounds,
            swiss_policy: self.swiss_policy.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TournamentConfig {
    pub tournament_id: String,
    pub stage: StageConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Standing {
    pub team_id: TeamId,
    pub match_points: u32,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub game_wins: u32,
    pub game_losses: u32,
    pub net_game_wins: i32,
    pub opponent_win_rate_basis_points: Option<u32>,
    pub dropped: bool,
    pub rank: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResultCorrection {
    pub match_id: MatchId,
    pub previous_result: MatchResult,
    pub replacement_result: MatchResult,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TeamEntry {
    seed: u32,
    dropped: bool,
    had_bye: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum DoublePhase {
    Winners,
    Losers,
    GrandFinal,
    GrandFinalReset,
    Finished,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct DoubleState {
    phase: DoublePhase,
    winners_pool: Vec<TeamId>,
    losers_waiting: Vec<TeamId>,
    lower_pool: Vec<TeamId>,
    winners_champion: Option<TeamId>,
    losers_champion: Option<TeamId>,
    grand_final_winner: Option<TeamId>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Stage {
    config: StageConfig,
    status: StageStatus,
    rounds: Vec<Round>,
    standings: Vec<Standing>,
    corrections: Vec<ResultCorrection>,
    last_schedule_error: Option<String>,
    schedule_generation: u64,
    superseded_rounds: Vec<Round>,
    teams: BTreeMap<TeamId, TeamEntry>,
    double_state: Option<DoubleState>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Tournament {
    tournament_id: String,
    stages: Vec<Stage>,
}

/// 短命 plan；來源 Stage 快照只作 apply 重核證據，不序列化或持久化。
pub struct QualifiedStagePlan {
    tournament_id: String,
    template: QualifiedStageTemplate,
    source_stages: Vec<Stage>,
    qualification: ResolvedStageQualification,
    planned_stage: Stage,
}

impl QualifiedStagePlan {
    /// 預覽解析後的來源關係與目的 seed。
    pub fn qualification(&self) -> &ResolvedStageQualification {
        &self.qualification
    }

    /// 預覽由既有 Stage 建構器產生、尚未開賽的核心階段。
    pub fn planned_stage(&self) -> &Stage {
        &self.planned_stage
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UpdateOutcome {
    Applied,
    AlreadyRecorded,
    AppliedWithScheduleError(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScheduleError {
    EmptyTeams,
    DuplicateTeam(TeamId),
    InvalidSeed,
    InvalidMatchFormat(String),
    InvalidClockConfig(String),
    SwissRoundsRequired,
    SwissPolicyRequired,
    InvalidSwissPolicy(String),
    NoSuchStage(StageId),
    NoSuchMatch(MatchId),
    MatchAlreadyStarted(MatchId),
    ResultAlreadyRecorded(MatchId),
    ResultDoesNotMatchParticipants(MatchId),
    DrawNotAllowed(MatchId),
    InvalidScore(MatchId),
    MatchNotCompleted(MatchId),
    CorrectionWouldRescheduleStartedMatch(MatchId),
    CorrectionWouldChangeFutureSchedule(MatchId),
    InvalidSnapshot(String),
    EmptyCorrectionReason,
    NoLegalSwissPairing,
    DoubleLossNeedsReferee(MatchId),
    InvalidOperation(String),
}

impl Display for ScheduleError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyTeams => write!(f, "至少需要兩隊"),
            Self::DuplicateTeam(team) => write!(f, "隊伍重複：{team}"),
            Self::InvalidSeed => write!(f, "seed 必須從 1 開始且不可重複"),
            Self::InvalidMatchFormat(reason) => write!(f, "交手格式無效：{reason}"),
            Self::InvalidClockConfig(reason) => write!(f, "時鐘設定無效：{reason}"),
            Self::SwissRoundsRequired => write!(f, "Swiss 必須明確設定 swissRounds"),
            Self::SwissPolicyRequired => write!(f, "Swiss 必須明確設定 swissPolicy"),
            Self::InvalidSwissPolicy(reason) => write!(f, "Swiss policy 無效：{reason}"),
            Self::NoSuchStage(stage) => write!(f, "找不到階段：{stage}"),
            Self::NoSuchMatch(match_id) => write!(f, "找不到交手：{match_id}"),
            Self::MatchAlreadyStarted(match_id) => write!(f, "交手已開始：{match_id}"),
            Self::ResultAlreadyRecorded(match_id) => write!(f, "交手已有不同結果：{match_id}"),
            Self::ResultDoesNotMatchParticipants(match_id) => {
                write!(f, "結果參賽者不符合交手：{match_id}")
            }
            Self::DrawNotAllowed(match_id) => write!(f, "淘汰交手不接受和局：{match_id}"),
            Self::InvalidScore(match_id) => write!(f, "交手比分無效：{match_id}"),
            Self::MatchNotCompleted(match_id) => write!(f, "交手尚未完成：{match_id}"),
            Self::CorrectionWouldRescheduleStartedMatch(match_id) => {
                write!(f, "更正會改動已開始的後續交手：{match_id}")
            }
            Self::CorrectionWouldChangeFutureSchedule(match_id) => {
                write!(f, "更正會改動已建立的後續賽程：{match_id}")
            }
            Self::InvalidSnapshot(reason) => write!(f, "賽程快照無效：{reason}"),
            Self::EmptyCorrectionReason => write!(f, "更正必須留下原因"),
            Self::NoLegalSwissPairing => write!(f, "找不到不重賽的合法 Swiss 配對"),
            Self::DoubleLossNeedsReferee(id) => {
                write!(f, "雙方判負已保留，淘汰賽後續需裁判處理：{id}")
            }
            Self::InvalidOperation(reason) => write!(f, "操作無效：{reason}"),
        }
    }
}

impl std::error::Error for ScheduleError {}

#[derive(Debug, Clone, PartialEq, Eq)]
/// 階段 plan 建立或 apply 被拒絕時的封閉錯誤集合。
pub enum QualifiedStagePlanError {
    QualificationBlocked(StageQualificationBlocker),
    InvalidTemplate(ScheduleError),
    DuplicateStageId(StageId),
    ForeignTournament {
        planned_tournament_id: String,
        current_tournament_id: String,
    },
    SourceChanged {
        source_stage_id: StageId,
    },
    StalePlan,
}

impl Display for QualifiedStagePlanError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::QualificationBlocked(blocker) => write!(f, "晉級名額無法確認：{blocker:?}"),
            Self::InvalidTemplate(error) => write!(f, "下一階段設定無效：{error}"),
            Self::DuplicateStageId(stage_id) => write!(f, "階段 ID 已存在：{stage_id}"),
            Self::ForeignTournament {
                planned_tournament_id,
                current_tournament_id,
            } => write!(
                f,
                "階段計畫屬於賽事 {planned_tournament_id}，目前賽事為 {current_tournament_id}"
            ),
            Self::SourceChanged { source_stage_id } => {
                write!(f, "資格來源已變更：{source_stage_id}")
            }
            Self::StalePlan => write!(f, "階段計畫與其解析結果不一致"),
        }
    }
}

impl std::error::Error for QualifiedStagePlanError {}

impl Tournament {
    pub fn tournament_id(&self) -> &str {
        &self.tournament_id
    }

    pub fn stages(&self) -> &[Stage] {
        &self.stages
    }

    pub fn new(config: TournamentConfig) -> Result<Self, ScheduleError> {
        let stage = Stage::new(config.stage)?;
        Ok(Self {
            tournament_id: config.tournament_id,
            stages: vec![stage],
        })
    }

    pub fn stage(&self, stage_id: &StageId) -> Result<&Stage, ScheduleError> {
        self.stages
            .iter()
            .find(|stage| &stage.config.stage_id == stage_id)
            .ok_or_else(|| ScheduleError::NoSuchStage(stage_id.clone()))
    }

    /// 依呼叫端明確指定的來源名額解析固定隊伍與目的 seed，不修改 Tournament。
    ///
    /// slots 至少兩筆；來源、隊伍與正 seed 均須唯一。名次比對 standings.rank 的實值，
    /// 結果保留 slots 順序與 seed 對應。來源未完成、仍有未決賽程、名次不唯一或隊伍
    /// 無法確定時，以封閉的 StageQualificationBlocker 阻擋整次解析。
    pub fn resolve_stage_qualification(
        &self,
        slots: &[StageQualificationSlot],
    ) -> Result<ResolvedStageQualification, StageQualificationBlocker> {
        let mut destination_seeds = BTreeSet::new();
        let mut sources = BTreeSet::new();
        for slot in slots {
            if slot.destination_seed == 0 {
                return Err(StageQualificationBlocker::InvalidDestinationSeed {
                    seed: slot.destination_seed,
                });
            }
            if !destination_seeds.insert(slot.destination_seed) {
                return Err(StageQualificationBlocker::DuplicateDestinationSeed {
                    seed: slot.destination_seed,
                });
            }
            if let StageQualificationSource::UniqueStandingRank { rank: 0 } = &slot.source {
                return Err(StageQualificationBlocker::InvalidStandingRank {
                    source_stage_id: slot.source_stage_id.clone(),
                    rank: 0,
                });
            }
            if !sources.insert((slot.source_stage_id.clone(), slot.source.clone())) {
                return Err(StageQualificationBlocker::DuplicateSource {
                    source_stage_id: slot.source_stage_id.clone(),
                    source: slot.source.clone(),
                });
            }
        }
        if slots.len() < 2 {
            return Err(StageQualificationBlocker::TooFewSlots { count: slots.len() });
        }

        let mut seen_teams = BTreeSet::new();
        let mut seeded_teams = Vec::with_capacity(slots.len());
        let mut source_mappings = Vec::with_capacity(slots.len());
        for slot in slots {
            let stage = self
                .stages
                .iter()
                .find(|stage| stage.config.stage_id == slot.source_stage_id)
                .ok_or_else(|| StageQualificationBlocker::MissingSourceStage {
                    source_stage_id: slot.source_stage_id.clone(),
                })?;
            ensure_stage_qualification_source_is_closed(stage)?;

            let team_id = match &slot.source {
                StageQualificationSource::FinishedStageWinner => {
                    let team_id = stage.winner().ok_or_else(|| {
                        StageQualificationBlocker::WinnerNotDetermined {
                            source_stage_id: slot.source_stage_id.clone(),
                        }
                    })?;
                    let Some(entry) = stage.teams.get(&team_id) else {
                        return Err(StageQualificationBlocker::SourceTeamMissing {
                            source_stage_id: slot.source_stage_id.clone(),
                            team_id,
                        });
                    };
                    let standing_is_dropped = stage
                        .standings
                        .iter()
                        .find(|standing| standing.team_id == team_id)
                        .is_some_and(|standing| standing.dropped);
                    if entry.dropped || standing_is_dropped {
                        return Err(StageQualificationBlocker::DroppedTeam {
                            source_stage_id: slot.source_stage_id.clone(),
                            team_id,
                        });
                    }
                    team_id
                }
                StageQualificationSource::UniqueStandingRank { rank } => {
                    if !matches!(
                        stage.config.format,
                        TournamentFormat::RoundRobin | TournamentFormat::Swiss
                    ) {
                        return Err(StageQualificationBlocker::StandingRanksUnsupported {
                            source_stage_id: slot.source_stage_id.clone(),
                            format: stage.config.format,
                        });
                    }
                    if let Some(standing) = stage
                        .standings
                        .iter()
                        .find(|standing| standing.rank.is_none())
                    {
                        return Err(StageQualificationBlocker::StandingRankUnavailable {
                            source_stage_id: slot.source_stage_id.clone(),
                            team_id: standing.team_id.clone(),
                        });
                    }
                    let ranked_teams: Vec<_> = stage
                        .standings
                        .iter()
                        .filter(|standing| standing.rank == Some(*rank))
                        .collect();
                    let standing = match ranked_teams.as_slice() {
                        [] => {
                            return Err(StageQualificationBlocker::StandingRankNotFound {
                                source_stage_id: slot.source_stage_id.clone(),
                                rank: *rank,
                            });
                        }
                        [standing] => *standing,
                        _ => {
                            let mut team_ids: Vec<_> = ranked_teams
                                .iter()
                                .map(|standing| standing.team_id.clone())
                                .collect();
                            team_ids.sort();
                            return Err(StageQualificationBlocker::StandingRankNotUnique {
                                source_stage_id: slot.source_stage_id.clone(),
                                rank: *rank,
                                team_ids,
                            });
                        }
                    };
                    let Some(entry) = stage.teams.get(&standing.team_id) else {
                        return Err(StageQualificationBlocker::SourceTeamMissing {
                            source_stage_id: slot.source_stage_id.clone(),
                            team_id: standing.team_id.clone(),
                        });
                    };
                    if entry.dropped || standing.dropped {
                        return Err(StageQualificationBlocker::DroppedTeam {
                            source_stage_id: slot.source_stage_id.clone(),
                            team_id: standing.team_id.clone(),
                        });
                    }
                    standing.team_id.clone()
                }
            };

            if !seen_teams.insert(team_id.clone()) {
                return Err(StageQualificationBlocker::DuplicateTeam { team_id });
            }
            seeded_teams.push(SeededTeam {
                team_id: team_id.clone(),
                seed: slot.destination_seed,
            });
            source_mappings.push(StageQualificationMapping {
                source_stage_id: slot.source_stage_id.clone(),
                source: slot.source.clone(),
                team_id,
                destination_seed: slot.destination_seed,
            });
        }

        Ok(ResolvedStageQualification {
            seeded_teams,
            source_mappings,
        })
    }

    /// 以目前已確定來源建立純預覽；plan 私有保存來源 Stage 全快照供 apply 重核。
    pub fn plan_qualified_stage(
        &self,
        template: QualifiedStageTemplate,
    ) -> Result<QualifiedStagePlan, QualifiedStagePlanError> {
        if self
            .stages
            .iter()
            .any(|stage| stage.config.stage_id == template.stage_id)
        {
            return Err(QualifiedStagePlanError::DuplicateStageId(
                template.stage_id,
            ));
        }

        let qualification = self
            .resolve_stage_qualification(&template.source_slots)
            .map_err(QualifiedStagePlanError::QualificationBlocked)?;
        let planned_stage = Stage::new(template.stage_config(qualification.seeded_teams.clone()))
            .map_err(QualifiedStagePlanError::InvalidTemplate)?;

        let mut seen_source_ids = BTreeSet::new();
        let mut source_stages = Vec::new();
        for slot in &template.source_slots {
            if !seen_source_ids.insert(slot.source_stage_id.clone()) {
                continue;
            }
            let source_stage = self
                .stages
                .iter()
                .find(|stage| stage.config.stage_id == slot.source_stage_id)
                .ok_or_else(|| {
                    QualifiedStagePlanError::QualificationBlocked(
                        StageQualificationBlocker::MissingSourceStage {
                            source_stage_id: slot.source_stage_id.clone(),
                        },
                    )
                })?;
            source_stages.push(source_stage.clone());
        }

        Ok(QualifiedStagePlan {
            tournament_id: self.tournament_id.clone(),
            template,
            source_stages,
            qualification,
            planned_stage,
        })
    }

    /// 只在同一賽事、來源事實與解析結果仍完全一致時追加預覽階段。
    pub fn apply_qualified_stage(
        &mut self,
        plan: QualifiedStagePlan,
    ) -> Result<ResolvedStageQualification, QualifiedStagePlanError> {
        if self.tournament_id != plan.tournament_id {
            return Err(QualifiedStagePlanError::ForeignTournament {
                planned_tournament_id: plan.tournament_id,
                current_tournament_id: self.tournament_id.clone(),
            });
        }
        if self
            .stages
            .iter()
            .any(|stage| stage.config.stage_id == plan.template.stage_id)
        {
            return Err(QualifiedStagePlanError::DuplicateStageId(
                plan.template.stage_id,
            ));
        }

        for source_snapshot in &plan.source_stages {
            let source_stage_id = &source_snapshot.config.stage_id;
            let current_source = self
                .stages
                .iter()
                .find(|stage| &stage.config.stage_id == source_stage_id);
            if current_source != Some(source_snapshot) {
                return Err(QualifiedStagePlanError::SourceChanged {
                    source_stage_id: source_stage_id.clone(),
                });
            }
        }

        let current_qualification = self
            .resolve_stage_qualification(&plan.template.source_slots)
            .map_err(|_| QualifiedStagePlanError::StalePlan)?;
        if current_qualification != plan.qualification
            || plan.planned_stage.config
                != plan
                    .template
                    .stage_config(current_qualification.seeded_teams.clone())
        {
            return Err(QualifiedStagePlanError::StalePlan);
        }

        let qualification = plan.qualification;
        self.stages.push(plan.planned_stage);
        Ok(qualification)
    }

    pub fn submit_result(
        &mut self,
        stage_id: &StageId,
        match_id: &MatchId,
        result: MatchResult,
    ) -> Result<UpdateOutcome, ScheduleError> {
        let mut candidate = self.clone();
        let stage = candidate
            .stages
            .iter_mut()
            .find(|stage| &stage.config.stage_id == stage_id)
            .ok_or_else(|| ScheduleError::NoSuchStage(stage_id.clone()))?;
        let outcome = stage.submit_result(match_id, result)?;
        *self = candidate;
        Ok(outcome)
    }

    pub fn start_match(
        &mut self,
        stage_id: &StageId,
        match_id: &MatchId,
    ) -> Result<(), ScheduleError> {
        let mut candidate = self.clone();
        let stage = candidate
            .stages
            .iter_mut()
            .find(|stage| &stage.config.stage_id == stage_id)
            .ok_or_else(|| ScheduleError::NoSuchStage(stage_id.clone()))?;
        stage.start_match(match_id)?;
        *self = candidate;
        Ok(())
    }

    pub fn correct_result(
        &mut self,
        stage_id: &StageId,
        match_id: &MatchId,
        replacement: MatchResult,
        reason: String,
    ) -> Result<(), ScheduleError> {
        let mut candidate = self.clone();
        let stage = candidate
            .stages
            .iter_mut()
            .find(|stage| &stage.config.stage_id == stage_id)
            .ok_or_else(|| ScheduleError::NoSuchStage(stage_id.clone()))?;
        stage.correct_result(match_id, replacement, reason)?;
        *self = candidate;
        Ok(())
    }

    pub fn drop_team(&mut self, stage_id: &StageId, team_id: &TeamId) -> Result<(), ScheduleError> {
        let mut candidate = self.clone();
        let stage = candidate
            .stages
            .iter_mut()
            .find(|stage| &stage.config.stage_id == stage_id)
            .ok_or_else(|| ScheduleError::NoSuchStage(stage_id.clone()))?;
        stage.drop_team(team_id)?;
        *self = candidate;
        Ok(())
    }
}

fn ensure_stage_qualification_source_is_closed(
    stage: &Stage,
) -> Result<(), StageQualificationBlocker> {
    if matches!(
        stage.config.format,
        TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination
    ) && let Some(match_) = stage
        .rounds
        .iter()
        .flat_map(|round| &round.matches)
        .find(|match_| match_.result.as_ref().is_some_and(MatchResult::is_double_loss))
    {
        return Err(StageQualificationBlocker::DoubleLossNeedsReferee {
            source_stage_id: stage.config.stage_id.clone(),
            match_id: match_.match_id.clone(),
        });
    }
    if stage.last_schedule_error.is_some() {
        return Err(StageQualificationBlocker::SourceScheduleUnresolved {
            source_stage_id: stage.config.stage_id.clone(),
        });
    }
    if stage.status != StageStatus::Finished {
        return Err(StageQualificationBlocker::SourceStageNotFinished {
            source_stage_id: stage.config.stage_id.clone(),
        });
    }
    if let Some(match_) = stage
        .rounds
        .iter()
        .flat_map(|round| &round.matches)
        .find(|match_| {
            matches!(
                match_.status,
                MatchStatus::Pending | MatchStatus::InProgress
            )
        })
    {
        return Err(StageQualificationBlocker::SourceMatchUnresolved {
            source_stage_id: stage.config.stage_id.clone(),
            match_id: match_.match_id.clone(),
            status: match_.status,
        });
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StageSnapshot {
    config: StageConfig,
    status: StageStatus,
    rounds: Vec<Round>,
    standings: Vec<Standing>,
    corrections: Vec<ResultCorrection>,
    #[serde(default)]
    last_schedule_error: Option<String>,
    #[serde(default)]
    schedule_generation: u64,
    #[serde(default)]
    superseded_rounds: Vec<Round>,
    teams: BTreeMap<TeamId, TeamEntry>,
    double_state: Option<DoubleState>,
}

impl<'de> Deserialize<'de> for Stage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let snapshot = StageSnapshot::deserialize(deserializer)?;
        let stage = Self {
            config: snapshot.config,
            status: snapshot.status,
            rounds: snapshot.rounds,
            standings: snapshot.standings,
            corrections: snapshot.corrections,
            last_schedule_error: snapshot.last_schedule_error,
            schedule_generation: snapshot.schedule_generation,
            superseded_rounds: snapshot.superseded_rounds,
            teams: snapshot.teams,
            double_state: snapshot.double_state,
        };
        stage
            .validate_snapshot()
            .map_err(serde::de::Error::custom)?;
        Ok(stage)
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TournamentSnapshot {
    tournament_id: String,
    stages: Vec<Stage>,
}

impl<'de> Deserialize<'de> for Tournament {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let snapshot = TournamentSnapshot::deserialize(deserializer)?;
        if snapshot.stages.is_empty() {
            return Err(serde::de::Error::custom("賽事至少需要一個階段"));
        }
        let mut stage_ids = BTreeSet::new();
        for stage in &snapshot.stages {
            if !stage_ids.insert(stage.config.stage_id.clone()) {
                return Err(serde::de::Error::custom("階段 ID 不可重複"));
            }
        }
        Ok(Self {
            tournament_id: snapshot.tournament_id,
            stages: snapshot.stages,
        })
    }
}

impl Stage {
    fn new(config: StageConfig) -> Result<Self, ScheduleError> {
        validate_config(&config)?;
        let teams = config
            .teams
            .iter()
            .map(|team| {
                (
                    team.team_id.clone(),
                    TeamEntry {
                        seed: team.seed,
                        dropped: false,
                        had_bye: false,
                    },
                )
            })
            .collect();
        let mut stage = Self {
            config,
            status: StageStatus::Active,
            rounds: vec![],
            standings: vec![],
            corrections: vec![],
            last_schedule_error: None,
            schedule_generation: 0,
            superseded_rounds: vec![],
            teams,
            double_state: None,
        };
        stage.initialize_schedule()?;
        stage.refresh_standings();
        stage.progress()?;
        stage.refresh_standings();
        Ok(stage)
    }

    pub fn current_round(&self) -> Option<&Round> {
        self.rounds.iter().rev().find(|round| {
            round.matches.iter().any(|match_| {
                match_.status == MatchStatus::Pending || match_.status == MatchStatus::InProgress
            })
        })
    }

    pub fn standings(&self) -> &[Standing] {
        &self.standings
    }

    pub fn config(&self) -> &StageConfig {
        &self.config
    }

    pub fn status(&self) -> StageStatus {
        self.status
    }

    pub fn rounds(&self) -> &[Round] {
        &self.rounds
    }

    pub fn corrections(&self) -> &[ResultCorrection] {
        &self.corrections
    }

    /// 被更正取代的配對保留供稽核，但不能再接受開賽或結果。
    pub fn superseded_rounds(&self) -> &[Round] {
        &self.superseded_rounds
    }

    pub fn last_schedule_error(&self) -> Option<&str> {
        self.last_schedule_error.as_deref()
    }

    pub fn winner(&self) -> Option<TeamId> {
        if self.status != StageStatus::Finished {
            return None;
        }
        match self.config.format {
            TournamentFormat::SingleElimination => self
                .rounds
                .last()
                .and_then(|round| completed_winners(round).ok())
                .and_then(|winners| winners.into_iter().next()),
            TournamentFormat::DoubleElimination => self
                .double_state
                .as_ref()
                .and_then(|state| state.grand_final_winner.clone()),
            TournamentFormat::RoundRobin | TournamentFormat::Swiss => {
                let mut winner = None;
                for row in self.standings.iter().filter(|row| row.rank == Some(1)) {
                    if winner.is_some() {
                        return None;
                    }
                    winner = Some(row.team_id.clone());
                }
                winner
            }
        }
    }

    fn validate_snapshot(&self) -> Result<(), ScheduleError> {
        validate_config(&self.config)?;
        let config_teams: BTreeSet<_> = self
            .config
            .teams
            .iter()
            .map(|team| team.team_id.clone())
            .collect();
        let snapshot_teams: BTreeSet<_> = self.teams.keys().cloned().collect();
        if config_teams != snapshot_teams {
            return Err(ScheduleError::InvalidSnapshot(
                "teams 與 config.teams 不一致".to_string(),
            ));
        }
        let mut match_ids = BTreeSet::new();
        for round in &self.rounds {
            let mut teams_in_round = BTreeSet::new();
            for match_ in &round.matches {
                if match_.stage_id != self.config.stage_id
                    || match_.round_number != round.round_number
                    || match_.kind != round.kind
                {
                    return Err(ScheduleError::InvalidSnapshot(
                        "交手的階段或輪次欄位不一致".to_string(),
                    ));
                }
                if !match_ids.insert(match_.match_id.clone()) {
                    return Err(ScheduleError::InvalidSnapshot(
                        "matchId 不可重複".to_string(),
                    ));
                }
                if !(1..=2).contains(&match_.participants.len()) {
                    return Err(ScheduleError::InvalidSnapshot(
                        "交手必須有一或兩隊".to_string(),
                    ));
                }
                for team_id in &match_.participants {
                    if !config_teams.contains(team_id) || !teams_in_round.insert(team_id.clone()) {
                        return Err(ScheduleError::InvalidSnapshot(
                            "同一輪不可重複出賽或使用未知隊伍".to_string(),
                        ));
                    }
                }
                match match_.status {
                    MatchStatus::Pending | MatchStatus::InProgress => {
                        if match_.result.is_some() {
                            return Err(ScheduleError::InvalidSnapshot(
                                "未完成交手不可有結果".to_string(),
                            ));
                        }
                    }
                    MatchStatus::Completed => {
                        let result = match_.result.as_ref().ok_or_else(|| {
                            ScheduleError::InvalidSnapshot("完成交手缺少結果".to_string())
                        })?;
                        validate_match_result(
                            &self.config.format,
                            match_.kind,
                            &match_.match_id,
                            &match_.participants,
                            result,
                            self.config.match_format,
                        )?;
                    }
                    MatchStatus::Cancelled => {
                        if match_.result.is_some() || match_.cancellation_reason.is_none() {
                            return Err(ScheduleError::InvalidSnapshot(
                                "取消交手不可有結果且必須有原因".to_string(),
                            ));
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn submit_result(
        &mut self,
        match_id: &MatchId,
        result: MatchResult,
    ) -> Result<UpdateOutcome, ScheduleError> {
        let mut candidate = self.clone();
        let outcome = candidate.apply_result(match_id, result)?;
        if outcome == UpdateOutcome::AlreadyRecorded {
            return Ok(outcome);
        }
        match candidate.progress() {
            Ok(()) => {
                candidate.refresh_standings();
                *self = candidate;
                Ok(outcome)
            }
            Err(ScheduleError::NoLegalSwissPairing) => {
                let message = "找不到不重賽的合法 Swiss 配對，已保留本輪結果".to_string();
                candidate.last_schedule_error = Some(message.clone());
                candidate.refresh_standings();
                *self = candidate;
                Ok(UpdateOutcome::AppliedWithScheduleError(message))
            }
            Err(error @ ScheduleError::DoubleLossNeedsReferee(_)) => {
                let message = error.to_string();
                candidate.last_schedule_error = Some(message.clone());
                candidate.refresh_standings();
                *self = candidate;
                Ok(UpdateOutcome::AppliedWithScheduleError(message))
            }
            Err(error) => Err(error),
        }
    }

    fn apply_result(
        &mut self,
        match_id: &MatchId,
        result: MatchResult,
    ) -> Result<UpdateOutcome, ScheduleError> {
        let (round_kind, participants, status, old_result) = {
            let match_ = self.find_match(match_id)?;
            (
                match_.kind,
                match_.participants.clone(),
                match_.status,
                match_.result.clone(),
            )
        };
        if let Some(existing) = old_result {
            return if existing == result {
                Ok(UpdateOutcome::AlreadyRecorded)
            } else {
                Err(ScheduleError::ResultAlreadyRecorded(match_id.clone()))
            };
        }
        if status == MatchStatus::Cancelled {
            return Err(ScheduleError::InvalidOperation(
                "取消的交手不能提交結果".to_string(),
            ));
        }
        validate_match_result(
            &self.config.format,
            round_kind,
            match_id,
            &participants,
            &result,
            self.config.match_format,
        )?;
        let match_ = self.find_match_mut(match_id)?;
        if !matches!(&result, MatchResult::NoShow { .. }) {
            match_.started = true;
        }
        match_.status = if matches!(result, MatchResult::Bye { .. }) {
            MatchStatus::Completed
        } else {
            MatchStatus::Completed
        };
        match_.result = Some(result);
        Ok(UpdateOutcome::Applied)
    }

    fn start_match(&mut self, match_id: &MatchId) -> Result<(), ScheduleError> {
        let match_ = self.find_match_mut(match_id)?;
        if match_.status != MatchStatus::Pending {
            return Err(ScheduleError::MatchAlreadyStarted(match_id.clone()));
        }
        if match_.participants.len() != 2 {
            return Err(ScheduleError::InvalidOperation(
                "沒有兩隊參賽者的交手不能開始".to_string(),
            ));
        }
        match_.started = true;
        match_.status = MatchStatus::InProgress;
        Ok(())
    }

    fn correct_result(
        &mut self,
        match_id: &MatchId,
        replacement: MatchResult,
        reason: String,
    ) -> Result<(), ScheduleError> {
        if reason.trim().is_empty() {
            return Err(ScheduleError::EmptyCorrectionReason);
        }
        let (round_number, old_result, participants, kind) = {
            let match_ = self.find_match(match_id)?;
            (
                match_.round_number,
                match_.result.clone(),
                match_.participants.clone(),
                match_.kind,
            )
        };
        let old_result =
            old_result.ok_or_else(|| ScheduleError::MatchNotCompleted(match_id.clone()))?;
        if old_result == replacement {
            return Ok(());
        }
        validate_match_result(
            &self.config.format,
            kind,
            match_id,
            &participants,
            &replacement,
            self.config.match_format,
        )?;
        if self.config.format != TournamentFormat::RoundRobin
            && self.rounds.iter().any(|round| {
                round.round_number > round_number
                    && round.matches.iter().any(|match_| match_.started)
            })
        {
            return Err(ScheduleError::CorrectionWouldRescheduleStartedMatch(
                match_id.clone(),
            ));
        }
        let restore_double_loss_future_matches = self.config.format == TournamentFormat::RoundRobin
            && self.status == StageStatus::Active
            && old_result.is_double_loss();
        let match_ = self.find_match_mut(match_id)?;
        match_.result = Some(replacement.clone());
        match_.status = MatchStatus::Completed;
        self.corrections.push(ResultCorrection {
            match_id: match_id.clone(),
            previous_result: old_result,
            replacement_result: replacement,
            reason,
        });
        if restore_double_loss_future_matches {
            self.restore_round_robin_double_loss_future_matches(
                match_id,
                round_number,
                &participants,
            );
        }
        self.rebuild_pending_schedule(round_number)?;
        self.refresh_standings();
        Ok(())
    }

    fn drop_team(&mut self, team_id: &TeamId) -> Result<(), ScheduleError> {
        let entry = self
            .teams
            .get_mut(team_id)
            .ok_or_else(|| ScheduleError::InvalidOperation(format!("找不到隊伍：{team_id}")))?;
        entry.dropped = true;
        match self.progress() {
            Ok(()) => {}
            Err(ScheduleError::NoLegalSwissPairing) => {
                self.last_schedule_error = Some("退賽已保留，找不到合法瑞士配對".into());
            }
            Err(error) => return Err(error),
        }
        self.refresh_standings();
        Ok(())
    }

    fn resolve_withdrawn_pairings(&mut self) -> Result<(), ScheduleError> {
        for round in &mut self.rounds {
            for match_ in &mut round.matches {
                if !matches!(
                    match_.status,
                    MatchStatus::Pending | MatchStatus::InProgress
                ) {
                    continue;
                }
                let dropped: Vec<_> = match_
                    .participants
                    .iter()
                    .filter(|id| self.teams.get(*id).is_some_and(|entry| entry.dropped))
                    .cloned()
                    .collect();
                if dropped.len() == 2 {
                    return Err(ScheduleError::InvalidOperation(
                        "雙方退賽的交手需要另行裁判處置".into(),
                    ));
                }
                if let Some(team_id) = dropped.first() {
                    if match_.participants.len() == 2 {
                        let winner_id = match_
                            .participants
                            .iter()
                            .find(|participant| *participant != team_id)
                            .cloned()
                            .expect("兩隊交手必有另一隊");
                        match_.result = Some(MatchResult::forfeit(winner_id, team_id.clone()));
                        match_.status = MatchStatus::Completed;
                    } else {
                        match_.status = MatchStatus::Cancelled;
                        match_.cancellation_reason = Some("退賽隊伍沒有對手".to_string());
                    }
                }
            }
        }
        Ok(())
    }

    fn cancel_round_robin_double_loss_future_matches(&mut self) {
        if self.config.format != TournamentFormat::RoundRobin {
            return;
        }
        let double_losses: Vec<_> = self
            .rounds
            .iter()
            .flat_map(|round| &round.matches)
            .filter(|match_| {
                match_.status == MatchStatus::Completed
                    && match_.result.as_ref().is_some_and(MatchResult::is_double_loss)
            })
            .map(|match_| {
                (
                    match_.round_number,
                    match_.participants.clone(),
                    match_.match_id.clone(),
                )
            })
            .collect();
        // 循環雙負退出只取消未開打的後續原配對，不套用自願退賽的判勝流程。
        for round in &mut self.rounds {
            for match_ in &mut round.matches {
                if match_.status == MatchStatus::Pending
                    && !match_.started
                    && match_.result.is_none()
                {
                    if let Some((_, _, source_match_id)) =
                        double_losses.iter().find(|(round_number, participants, _)| {
                            match_.round_number > *round_number
                                && participants
                                    .iter()
                                    .any(|team_id| match_.participants.contains(team_id))
                        })
                    {
                        match_.status = MatchStatus::Cancelled;
                        match_.cancellation_reason = Some(
                            Self::round_robin_double_loss_cancellation_reason(source_match_id),
                        );
                    }
                }
            }
        }
    }

    fn round_robin_double_loss_cancellation_reason(source_match_id: &MatchId) -> String {
        format!("參賽隊伍已因雙方判負退出本階段（來源交手：{source_match_id}）")
    }

    fn restore_round_robin_double_loss_future_matches(
        &mut self,
        source_match_id: &MatchId,
        source_round: u32,
        source_teams: &[TeamId],
    ) {
        if self.config.format != TournamentFormat::RoundRobin || self.status != StageStatus::Active {
            return;
        }
        let expected_reason = Self::round_robin_double_loss_cancellation_reason(source_match_id);
        let still_exited: BTreeSet<_> = self
            .rounds
            .iter()
            .flat_map(|round| &round.matches)
            .filter(|match_| {
                match_.status == MatchStatus::Completed
                    && match_.result.as_ref().is_some_and(MatchResult::is_double_loss)
            })
            .flat_map(|match_| match_.participants.iter().cloned())
            .collect();
        let teams = &self.teams;
        // 只解除來源可辨識的系統取消；舊通用原因、其他退出或已玩事實一律保留。
        for round in &mut self.rounds {
            for match_ in &mut round.matches {
                if match_.status == MatchStatus::Cancelled
                    && match_.round_number > source_round
                    && match_.kind == RoundKind::Main
                    && match_.participants.len() == 2
                    && !match_.started
                    && match_.result.is_none()
                    && match_.cancellation_reason.as_deref() == Some(expected_reason.as_str())
                    && match_
                        .participants
                        .iter()
                        .any(|team_id| source_teams.contains(team_id))
                    && match_.participants.iter().all(|team_id| {
                        !still_exited.contains(team_id)
                            && teams.get(team_id).is_some_and(|entry| !entry.dropped)
                    })
                {
                    match_.status = MatchStatus::Pending;
                    match_.cancellation_reason = None;
                }
            }
        }
    }

    fn initialize_schedule(&mut self) -> Result<(), ScheduleError> {
        match self.config.format {
            TournamentFormat::SingleElimination => self.initialize_single(),
            TournamentFormat::RoundRobin => self.initialize_round_robin(),
            TournamentFormat::DoubleElimination => self.initialize_double(),
            TournamentFormat::Swiss => self.initialize_swiss(),
        }
    }

    fn initialize_single(&mut self) -> Result<(), ScheduleError> {
        let order = seeded_order(&self.config.teams);
        self.append_elimination_round(RoundKind::Main, seeded_bracket_pairs(&order), true);
        Ok(())
    }

    fn initialize_round_robin(&mut self) -> Result<(), ScheduleError> {
        let mut order: Vec<Option<TeamId>> = seeded_order(&self.config.teams)
            .into_iter()
            .map(Some)
            .collect();
        if order.len() % 2 == 1 {
            order.push(None);
        }
        let rounds = order.len().saturating_sub(1);
        for round_number in 0..rounds {
            let mut pairs = vec![];
            for index in 0..(order.len() / 2) {
                let first = order[index].clone();
                let second = order[order.len() - index - 1].clone();
                match (first, second) {
                    (Some(first), Some(second)) => pairs.push(vec![first, second]),
                    (Some(team_id), None) | (None, Some(team_id)) => {
                        self.mark_bye(&team_id);
                        pairs.push(vec![team_id]);
                    }
                    (None, None) => {}
                }
            }
            self.append_round(RoundKind::Main, (round_number + 1) as u32, pairs, true);
            let last = order.pop().expect("固定長陣列");
            order.insert(1, last);
        }
        Ok(())
    }

    fn initialize_double(&mut self) -> Result<(), ScheduleError> {
        let order = seeded_order(&self.config.teams);
        self.double_state = Some(DoubleState {
            phase: DoublePhase::Winners,
            winners_pool: order.clone(),
            losers_waiting: vec![],
            lower_pool: vec![],
            winners_champion: None,
            losers_champion: None,
            grand_final_winner: None,
        });
        self.append_elimination_round(RoundKind::Winners, seeded_bracket_pairs(&order), true);
        Ok(())
    }

    fn initialize_swiss(&mut self) -> Result<(), ScheduleError> {
        let order = shuffled_order(&self.config.teams, self.config.random_seed);
        let pairs = self.swiss_pairing(&order)?;
        self.append_round(RoundKind::Main, 1, pairs, true);
        Ok(())
    }

    fn has_single_elimination_final_double_loss(&self) -> bool {
        let Some((final_round, previous_rounds)) = self.rounds.split_last() else {
            return false;
        };
        let [final_match] = final_round.matches.as_slice() else {
            return false;
        };
        self.config.format == TournamentFormat::SingleElimination
            && final_round.kind == RoundKind::Main
            && final_match.kind == RoundKind::Main
            && final_match.participants.len() == 2
            && final_match.status == MatchStatus::Completed
            && final_match.result.as_ref().is_some_and(MatchResult::is_double_loss)
            && previous_rounds
                .iter()
                .all(|round| round.matches.iter().all(is_closed))
    }

    fn progress(&mut self) -> Result<(), ScheduleError> {
        if self.has_single_elimination_final_double_loss() {
            // 真正單敗決賽雙負保留原交手，冠軍空缺；前輪須全結清，含已閉合雙負。
            self.status = StageStatus::Finished;
            return Ok(());
        }
        if self.config.format != TournamentFormat::SingleElimination {
            self.ensure_elimination_progression()?;
        }
        loop {
            if self.status == StageStatus::Finished {
                return Ok(());
            }
            self.cancel_round_robin_double_loss_future_matches();
            // 退賽效果也適用於剛生成的敗部／總決賽，不能讓退賽隊伍再次等待出賽。
            self.resolve_withdrawn_pairings()?;
            let Some(round) = self.rounds.last() else {
                return Ok(());
            };
            if round.matches.iter().any(|match_| {
                match_.status == MatchStatus::Pending || match_.status == MatchStatus::InProgress
            }) || (self.config.format == TournamentFormat::SingleElimination
                && self
                    .rounds
                    .iter()
                    .any(|round| round.matches.iter().any(|match_| !is_closed(match_))))
            {
                return Ok(());
            }
            match self.config.format {
                TournamentFormat::SingleElimination => self.progress_single()?,
                TournamentFormat::RoundRobin => {
                    if self
                        .rounds
                        .iter()
                        .all(|round| round.matches.iter().all(is_closed))
                    {
                        self.status = StageStatus::Finished;
                    }
                    return Ok(());
                }
                TournamentFormat::DoubleElimination => self.progress_double()?,
                TournamentFormat::Swiss => self.progress_swiss()?,
            }
        }
    }

    fn ensure_elimination_progression(&self) -> Result<(), ScheduleError> {
        if matches!(
            self.config.format,
            TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination
        ) {
            if let Some(match_) = self
                .rounds
                .iter()
                .flat_map(|round| &round.matches)
                .find(|match_| match_.result.as_ref().is_some_and(MatchResult::is_double_loss))
            {
                return Err(ScheduleError::DoubleLossNeedsReferee(
                    match_.match_id.clone(),
                ));
            }
        }
        Ok(())
    }

    fn progress_single(&mut self) -> Result<(), ScheduleError> {
        let round = self.rounds.last().expect("至少有一輪");
        if let Some(double_loss) = round
            .matches
            .iter()
            .find(|match_| match_.result.as_ref().is_some_and(MatchResult::is_double_loss))
        {
            // 先依原交手位置組分支，才移除無 winner 的空位，不能壓縮全部勝者。
            let pairs = round
                .matches
                .chunks(2)
                .map(|branch| {
                    let participants = branch
                        .iter()
                        .filter_map(|match_| match &match_.result {
                            Some(result) if result.is_double_loss() => None,
                            None => None,
                            Some(result) => Some(result.winner().cloned().ok_or_else(|| {
                                ScheduleError::InvalidOperation("和局沒有晉級隊伍".to_string())
                            })),
                        })
                        .collect::<Result<Vec<_>, _>>()?;
                    if participants.is_empty() {
                        // 雙空位傳播留待後續 slice，先保留雙負而不建立零隊交手。
                        return Err(ScheduleError::DoubleLossNeedsReferee(
                            double_loss.match_id.clone(),
                        ));
                    }
                    Ok(participants)
                })
                .collect::<Result<Vec<_>, _>>()?;
            self.append_elimination_round(RoundKind::Main, pairs, true);
            return Ok(());
        }
        let winners = completed_winners(round)?;
        if winners.len() <= 1 {
            self.status = StageStatus::Finished;
            return Ok(());
        }
        self.append_elimination_round(RoundKind::Main, pair_adjacent_with_byes(&winners), true);
        Ok(())
    }

    fn progress_double(&mut self) -> Result<(), ScheduleError> {
        let last_kind = self.rounds.last().expect("至少有一輪").kind;
        match last_kind {
            RoundKind::Winners => self.progress_double_winners(),
            RoundKind::Losers => self.progress_double_losers(),
            RoundKind::GrandFinal => self.progress_double_grand_final(false),
            RoundKind::GrandFinalReset => self.progress_double_grand_final(true),
            RoundKind::Main => Err(ScheduleError::InvalidOperation(
                "雙敗階段出現 Main round".to_string(),
            )),
        }
    }

    fn progress_double_winners(&mut self) -> Result<(), ScheduleError> {
        let round = self.rounds.last().expect("至少有一輪");
        let winners = completed_winners(round)?;
        let losers = completed_losers(round)?;
        let state = self.double_state.as_mut().expect("雙敗狀態存在");
        state.winners_pool = winners;
        state.losers_waiting.extend(losers);
        if state.winners_pool.len() > 1 {
            if state.losers_waiting.len() >= 2 {
                state.phase = DoublePhase::Losers;
                let lower = take_lower_pairing(state);
                self.append_elimination_round(RoundKind::Losers, lower, true);
            } else {
                state.phase = DoublePhase::Winners;
                let pairs = pair_adjacent_with_byes(&state.winners_pool);
                self.append_elimination_round(RoundKind::Winners, pairs, true);
            }
        } else {
            state.winners_champion = state.winners_pool.first().cloned();
            if state.losers_waiting.len() + state.lower_pool.len() > 1 {
                state.phase = DoublePhase::Losers;
                let lower = take_lower_pairing(state);
                self.append_elimination_round(RoundKind::Losers, lower, true);
            } else {
                state.losers_champion = state
                    .losers_waiting
                    .first()
                    .cloned()
                    .or_else(|| state.lower_pool.first().cloned());
                self.schedule_grand_final(false)?;
            }
        }
        Ok(())
    }

    fn progress_double_losers(&mut self) -> Result<(), ScheduleError> {
        let round = self.rounds.last().expect("至少有一輪");
        let winners = completed_winners(round)?;
        let state = self.double_state.as_mut().expect("雙敗狀態存在");
        state.lower_pool = winners;
        if state.winners_pool.len() > 1 {
            state.phase = DoublePhase::Winners;
            let pairs = pair_adjacent_with_byes(&state.winners_pool);
            self.append_elimination_round(RoundKind::Winners, pairs, true);
        } else if state.losers_waiting.len() + state.lower_pool.len() > 1 {
            state.phase = DoublePhase::Losers;
            let lower = take_lower_pairing(state);
            self.append_elimination_round(RoundKind::Losers, lower, true);
        } else {
            state.losers_champion = state.lower_pool.first().cloned();
            self.schedule_grand_final(false)?;
        }
        Ok(())
    }

    fn schedule_grand_final(&mut self, reset: bool) -> Result<(), ScheduleError> {
        let state = self.double_state.as_mut().expect("雙敗狀態存在");
        let first = state
            .winners_champion
            .clone()
            .ok_or_else(|| ScheduleError::InvalidOperation("缺少勝部冠軍".to_string()))?;
        let second = state
            .losers_champion
            .clone()
            .ok_or_else(|| ScheduleError::InvalidOperation("缺少敗部冠軍".to_string()))?;
        state.phase = if reset {
            DoublePhase::GrandFinalReset
        } else {
            DoublePhase::GrandFinal
        };
        self.append_elimination_round(
            if reset {
                RoundKind::GrandFinalReset
            } else {
                RoundKind::GrandFinal
            },
            vec![vec![first, second]],
            false,
        );
        Ok(())
    }

    fn progress_double_grand_final(&mut self, reset: bool) -> Result<(), ScheduleError> {
        let winner = completed_winners(self.rounds.last().expect("至少有一輪"))?
            .into_iter()
            .next()
            .ok_or_else(|| ScheduleError::InvalidOperation("總決賽沒有勝方".to_string()))?;
        let state = self.double_state.as_mut().expect("雙敗狀態存在");
        if !reset && Some(winner.clone()) == state.losers_champion {
            self.schedule_grand_final(true)?;
        } else {
            state.grand_final_winner = Some(winner);
            state.phase = DoublePhase::Finished;
            self.status = StageStatus::Finished;
        }
        Ok(())
    }

    fn progress_swiss(&mut self) -> Result<(), ScheduleError> {
        let rounds = self.config.swiss_rounds.expect("已驗證 Swiss rounds");
        if self.rounds.len() as u32 >= rounds {
            self.status = StageStatus::Finished;
            return Ok(());
        }
        let order = self.swiss_order();
        let pairs = self.swiss_pairing(&order)?;
        self.append_round(RoundKind::Main, self.rounds.len() as u32 + 1, pairs, true);
        Ok(())
    }

    fn append_elimination_round(
        &mut self,
        kind: RoundKind,
        pairs: Vec<Vec<TeamId>>,
        auto_byes: bool,
    ) {
        let round_number = self.rounds.len() as u32 + 1;
        self.append_round(kind, round_number, pairs, auto_byes);
    }

    fn append_round(
        &mut self,
        kind: RoundKind,
        round_number: u32,
        pairs: Vec<Vec<TeamId>>,
        auto_byes: bool,
    ) {
        let stage_id = self.config.stage_id.clone();
        let matches = pairs
            .into_iter()
            .enumerate()
            .map(|(index, participants)| {
                let match_id = MatchId(format!(
                    "{}-g{}-r{}-m{}",
                    stage_id.0,
                    self.schedule_generation,
                    round_number,
                    index + 1
                ));
                let mut match_ = ScheduledMatch {
                    match_id,
                    stage_id: stage_id.clone(),
                    round_number,
                    sequence: index as u32 + 1,
                    kind,
                    participants,
                    status: MatchStatus::Pending,
                    result: None,
                    cancellation_reason: None,
                    started: false,
                };
                if auto_byes && match_.participants.len() == 1 {
                    let team_id = match_.participants[0].clone();
                    self.mark_bye(&team_id);
                    match_.status = MatchStatus::Completed;
                    match_.result = Some(MatchResult::Bye { team_id });
                }
                match_
            })
            .collect();
        self.rounds.push(Round {
            round_number,
            kind,
            matches,
        });
    }

    fn mark_bye(&mut self, team_id: &TeamId) {
        if let Some(entry) = self.teams.get_mut(team_id) {
            entry.had_bye = true;
        }
    }

    fn find_match(&self, match_id: &MatchId) -> Result<&ScheduledMatch, ScheduleError> {
        self.rounds
            .iter()
            .flat_map(|round| round.matches.iter())
            .find(|match_| &match_.match_id == match_id)
            .ok_or_else(|| ScheduleError::NoSuchMatch(match_id.clone()))
    }

    fn find_match_mut(&mut self, match_id: &MatchId) -> Result<&mut ScheduledMatch, ScheduleError> {
        self.rounds
            .iter_mut()
            .flat_map(|round| round.matches.iter_mut())
            .find(|match_| &match_.match_id == match_id)
            .ok_or_else(|| ScheduleError::NoSuchMatch(match_id.clone()))
    }

    fn rebuild_pending_schedule(&mut self, corrected_round: u32) -> Result<(), ScheduleError> {
        if self.config.format == TournamentFormat::RoundRobin {
            return Ok(());
        }
        let retained: Vec<_> = self
            .rounds
            .iter()
            .filter(|round| round.round_number <= corrected_round)
            .cloned()
            .collect();
        let mut superseded = self.superseded_rounds.clone();
        superseded.extend(
            self.rounds
                .iter()
                .filter(|round| round.round_number > corrected_round)
                .cloned(),
        );
        let generation = self
            .schedule_generation
            .checked_add(1)
            .ok_or_else(|| ScheduleError::InvalidOperation("賽程版本溢位".into()))?;
        let mut rebuilt = self.clone();
        rebuilt.schedule_generation = generation;
        rebuilt.superseded_rounds = superseded;
        rebuilt.status = StageStatus::Active;
        rebuilt.last_schedule_error = None;
        for entry in rebuilt.teams.values_mut() {
            entry.had_bye = false;
        }
        for round in &retained {
            for match_ in &round.matches {
                if let Some(MatchResult::Bye { team_id }) = &match_.result {
                    rebuilt.mark_bye(team_id);
                }
            }
        }
        if self.config.format == TournamentFormat::DoubleElimination {
            // 用已記錄各輪的勝敗重建勝敗部流向，不重新抽籤或改動已進行的輪次。
            rebuilt.double_state = Self::new(self.config.clone())?.double_state;
            rebuilt.rounds.clear();
            for (index, round) in retained.iter().enumerate() {
                rebuilt.rounds.truncate(index);
                rebuilt.rounds.push(round.clone());
                if round.matches.iter().all(is_closed)
                    && rebuilt.ensure_elimination_progression().is_ok()
                {
                    rebuilt.progress_double()?;
                }
            }
        } else {
            rebuilt.rounds = retained;
        }
        match rebuilt.progress() {
            Ok(()) => {}
            Err(ScheduleError::NoLegalSwissPairing) => {
                rebuilt.last_schedule_error = Some("更正已保留，找不到合法瑞士配對".into());
            }
            Err(error @ ScheduleError::DoubleLossNeedsReferee(_)) => {
                rebuilt.last_schedule_error = Some(error.to_string());
            }
            Err(error) => return Err(error),
        }
        *self = rebuilt;
        Ok(())
    }

    fn swiss_order(&self) -> Vec<TeamId> {
        let double_loss_exits = self.swiss_double_loss_exits();
        let mut order: Vec<TeamId> = self
            .teams
            .iter()
            .filter_map(|(team_id, _)| {
                self.is_swiss_team_eligible(team_id, &double_loss_exits)
                    .then_some(team_id.clone())
            })
            .collect();
        let standings = self.compute_standings();
        order.sort_by(|left, right| {
            self.compare_rank_keys(left, right, &standings)
                .then_with(|| self.teams[left].seed.cmp(&self.teams[right].seed))
                .then_with(|| left.cmp(right))
        });
        order
    }

    fn swiss_pairing(&self, ordered: &[TeamId]) -> Result<Vec<Vec<TeamId>>, ScheduleError> {
        let double_loss_exits = self.swiss_double_loss_exits();
        let active: Vec<TeamId> = ordered
            .iter()
            .filter(|team_id| self.is_swiss_team_eligible(team_id, &double_loss_exits))
            .cloned()
            .collect();
        if active.is_empty() {
            return Err(ScheduleError::NoLegalSwissPairing);
        }
        let opponents = self.opponents();
        if active.len() % 2 == 0 {
            let mut remaining = active;
            let mut pairs = vec![];
            pair_backtracking(&mut remaining, &opponents, &mut pairs)?;
            return Ok(pairs);
        }
        let bye_candidates = active
            .iter()
            .rev()
            .filter(|team_id| self.teams.get(*team_id).is_some_and(|entry| !entry.had_bye));
        for bye_team in bye_candidates {
            let mut remaining = active.clone();
            remaining.retain(|team_id| team_id != bye_team);
            let mut pairs = vec![vec![bye_team.clone()]];
            if pair_backtracking(&mut remaining, &opponents, &mut pairs).is_ok() {
                return Ok(pairs);
            }
        }
        Err(ScheduleError::NoLegalSwissPairing)
    }

    fn is_swiss_team_eligible(
        &self,
        team_id: &TeamId,
        double_loss_exits: &BTreeSet<TeamId>,
    ) -> bool {
        self.teams
            .get(team_id)
            .is_some_and(|entry| !entry.dropped)
            && !double_loss_exits.contains(team_id)
    }

    fn swiss_double_loss_exits(&self) -> BTreeSet<TeamId> {
        if self.config.format != TournamentFormat::Swiss {
            return BTreeSet::new();
        }
        self.rounds
            .iter()
            .flat_map(|round| &round.matches)
            .filter(|match_| {
                match_.status == MatchStatus::Completed
                    && match_.result.as_ref().is_some_and(MatchResult::is_double_loss)
            })
            .flat_map(|match_| match_.participants.iter().cloned())
            .collect()
    }

    fn opponents(&self) -> BTreeMap<TeamId, BTreeSet<TeamId>> {
        let mut result: BTreeMap<TeamId, BTreeSet<TeamId>> = self
            .teams
            .keys()
            .cloned()
            .map(|team_id| (team_id, BTreeSet::new()))
            .collect();
        for round in &self.rounds {
            for match_ in &round.matches {
                if match_.participants.len() == 2 && match_.result.is_some() {
                    let first = match_.participants[0].clone();
                    let second = match_.participants[1].clone();
                    result
                        .entry(first.clone())
                        .or_default()
                        .insert(second.clone());
                    result.entry(second).or_default().insert(first);
                }
            }
        }
        result
    }

    fn refresh_standings(&mut self) {
        self.standings = self.compute_standings();
        if self.has_single_elimination_final_double_loss() {
            // 冠軍空缺階段的其餘名次也未確認；保留真實統計，不填不可靠名次。
            return;
        }
        let ordered = self.swiss_order_for_rank(&self.standings);
        let mut previous: Option<Standing> = None;
        let mut rank = 0;
        for (index, team_id) in ordered.into_iter().enumerate() {
            let current = self
                .standings
                .iter()
                .find(|row| row.team_id == team_id)
                .unwrap()
                .clone();
            if previous.as_ref().is_none_or(|previous| {
                self.compare_rank_keys(&previous.team_id, &current.team_id, &self.standings)
                    != Ordering::Equal
            }) {
                rank = index as u32 + 1;
            }
            if let Some(row) = self.standings.iter_mut().find(|row| row.team_id == team_id) {
                row.rank = Some(rank);
            }
            previous = Some(current);
        }
        self.standings
            .sort_by_key(|row| row.rank.unwrap_or(u32::MAX));
    }

    fn swiss_order_for_rank(&self, standings: &[Standing]) -> Vec<TeamId> {
        let mut order: Vec<TeamId> = standings.iter().map(|row| row.team_id.clone()).collect();
        order.sort_by(|left, right| self.compare_rank_keys(left, right, standings));
        order
    }

    fn compare_rank_keys(&self, left: &TeamId, right: &TeamId, standings: &[Standing]) -> Ordering {
        let left_row = standings.iter().find(|row| &row.team_id == left).unwrap();
        let right_row = standings.iter().find(|row| &row.team_id == right).unwrap();
        if let Some(champion) = self.elimination_champion() {
            if left == &champion && right != &champion {
                return Ordering::Less;
            }
            if right == &champion && left != &champion {
                return Ordering::Greater;
            }
            // 淘汰賽名次依淘汰輪次，不以累積勝場壓過決賽敗方；同輪淘汰並列。
            let exit_round = |team: &TeamId| {
                self.rounds.iter().rev().find_map(|round| {
                    round
                        .matches
                        .iter()
                        .any(|match_| match &match_.result {
                            Some(
                                MatchResult::Win { loser_id, .. }
                                | MatchResult::Forfeit { loser_id, .. },
                            ) => loser_id == team,
                            _ => false,
                        })
                        .then_some(round.round_number)
                })
            };
            return exit_round(right).cmp(&exit_round(left));
        }
        let mut ordering = right_row.match_points.cmp(&left_row.match_points);
        if ordering != Ordering::Equal {
            return ordering;
        }
        match self.config.format {
            TournamentFormat::RoundRobin => {
                ordering = self
                    .head_to_head_points(left, standings)
                    .cmp(&self.head_to_head_points(right, standings))
                    .reverse();
                if ordering == Ordering::Equal {
                    ordering = right_row.net_game_wins.cmp(&left_row.net_game_wins);
                }
            }
            TournamentFormat::Swiss => {
                let policy = self.config.swiss_policy.as_ref().unwrap();
                for tie_breaker in &policy.tie_breakers {
                    ordering = match tie_breaker {
                        SwissTieBreaker::MatchPoints => Ordering::Equal,
                        SwissTieBreaker::HeadToHeadMatchPoints => self
                            .head_to_head_points(left, standings)
                            .cmp(&self.head_to_head_points(right, standings))
                            .reverse(),
                        SwissTieBreaker::OpponentWinRate => right_row
                            .opponent_win_rate_basis_points
                            .cmp(&left_row.opponent_win_rate_basis_points),
                        SwissTieBreaker::NetGameWins => {
                            right_row.net_game_wins.cmp(&left_row.net_game_wins)
                        }
                        SwissTieBreaker::SeedOrder => {
                            self.teams[left].seed.cmp(&self.teams[right].seed)
                        }
                    };
                    if ordering != Ordering::Equal {
                        break;
                    }
                }
            }
            TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination => {
                ordering = right_row.wins.cmp(&left_row.wins);
                if ordering == Ordering::Equal {
                    ordering = right_row.net_game_wins.cmp(&left_row.net_game_wins);
                }
            }
        }
        ordering
    }

    fn elimination_champion(&self) -> Option<TeamId> {
        match self.config.format {
            TournamentFormat::SingleElimination if self.status == StageStatus::Finished => self
                .rounds
                .last()
                .and_then(|round| completed_winners(round).ok())
                .and_then(|winners| winners.into_iter().next()),
            TournamentFormat::DoubleElimination => self
                .double_state
                .as_ref()
                .and_then(|state| state.grand_final_winner.clone()),
            _ => None,
        }
    }

    fn head_to_head_points(&self, team_id: &TeamId, standings: &[Standing]) -> u32 {
        let team_points = standings
            .iter()
            .find(|row| &row.team_id == team_id)
            .map(|row| row.match_points)
            .unwrap();
        let tied_points = standings
            .iter()
            .filter(|row| row.match_points == team_points)
            .map(|row| row.team_id.clone())
            .collect::<BTreeSet<_>>();
        let mut points = 0;
        for round in &self.rounds {
            for match_ in &round.matches {
                if match_.participants.len() != 2
                    || !match_.participants.contains(team_id)
                    || !tied_points.contains(&match_.participants[0])
                    || !tied_points.contains(&match_.participants[1])
                {
                    continue;
                }
                if let Some(result) = &match_.result {
                    points += match_result_points(result, team_id, &self.config);
                }
            }
        }
        points
    }

    fn compute_standings(&self) -> Vec<Standing> {
        let double_loss_exits = self.swiss_double_loss_exits();
        let mut rows: BTreeMap<TeamId, Standing> = self
            .teams
            .iter()
            .map(|(team_id, entry)| {
                (
                    team_id.clone(),
                    Standing {
                        team_id: team_id.clone(),
                        match_points: 0,
                        wins: 0,
                        draws: 0,
                        losses: 0,
                        game_wins: 0,
                        game_losses: 0,
                        net_game_wins: 0,
                        opponent_win_rate_basis_points: None,
                        dropped: entry.dropped || double_loss_exits.contains(team_id),
                        rank: None,
                    },
                )
            })
            .collect();
        for round in &self.rounds {
            for match_ in &round.matches {
                let Some(result) = &match_.result else {
                    continue;
                };
                match result {
                    MatchResult::DoubleLoss => {
                        for team_id in &match_.participants {
                            rows.get_mut(team_id).unwrap().losses += 1;
                        }
                    }
                    MatchResult::NoShow {
                        absent_teams,
                        winner_id: None,
                    } => {
                        let loss_points = if self.config.format == TournamentFormat::Swiss {
                            self.config.swiss_policy.as_ref().unwrap().match_points.loss
                        } else {
                            0
                        };
                        for team_id in absent_teams {
                            let standing = rows.get_mut(team_id).unwrap();
                            standing.match_points += loss_points;
                            standing.losses += 1;
                        }
                    }
                    MatchResult::Win {
                        winner_id,
                        loser_id,
                        ..
                    } => {
                        let points = if self.config.format == TournamentFormat::Swiss {
                            self.config.swiss_policy.as_ref().unwrap().match_points.win
                        } else {
                            3
                        };
                        rows.get_mut(winner_id).unwrap().match_points += points;
                        rows.get_mut(winner_id).unwrap().wins += 1;
                        rows.get_mut(loser_id).unwrap().losses += 1;
                        let (winner_score, loser_score) = result.game_score(winner_id).unwrap();
                        rows.get_mut(winner_id).unwrap().game_wins += winner_score;
                        rows.get_mut(winner_id).unwrap().game_losses += loser_score;
                        rows.get_mut(loser_id).unwrap().game_wins += loser_score;
                        rows.get_mut(loser_id).unwrap().game_losses += winner_score;
                    }
                    MatchResult::Forfeit {
                        winner_id,
                        loser_id,
                    } => {
                        let points = if self.config.format == TournamentFormat::Swiss {
                            self.config.swiss_policy.as_ref().unwrap().match_points.win
                        } else {
                            3
                        };
                        rows.get_mut(winner_id).unwrap().match_points += points;
                        rows.get_mut(winner_id).unwrap().wins += 1;
                        rows.get_mut(loser_id).unwrap().losses += 1;
                    }
                    MatchResult::NoShow {
                        absent_teams,
                        winner_id: Some(winner_id),
                    } => {
                        let (win_points, loss_points) = if self.config.format
                            == TournamentFormat::Swiss
                        {
                            let points = &self.config.swiss_policy.as_ref().unwrap().match_points;
                            (points.win, points.loss)
                        } else {
                            (3, 0)
                        };
                        rows.get_mut(winner_id).unwrap().match_points += win_points;
                        rows.get_mut(winner_id).unwrap().wins += 1;
                        for team_id in absent_teams {
                            let standing = rows.get_mut(team_id).unwrap();
                            standing.match_points += loss_points;
                            standing.losses += 1;
                        }
                    }
                    MatchResult::Draw { .. } => {
                        let points = if self.config.format == TournamentFormat::Swiss {
                            self.config.swiss_policy.as_ref().unwrap().match_points.draw
                        } else {
                            1
                        };
                        for team_id in &match_.participants {
                            rows.get_mut(team_id).unwrap().match_points += points;
                            rows.get_mut(team_id).unwrap().draws += 1;
                        }
                        if match_.participants.len() == 2 {
                            let (first_games, second_games) = match result {
                                MatchResult::Draw {
                                    first_games,
                                    second_games,
                                } => (*first_games as u32, *second_games as u32),
                                _ => unreachable!(),
                            };
                            rows.get_mut(&match_.participants[0]).unwrap().game_wins += first_games;
                            rows.get_mut(&match_.participants[0]).unwrap().game_losses +=
                                second_games;
                            rows.get_mut(&match_.participants[1]).unwrap().game_wins +=
                                second_games;
                            rows.get_mut(&match_.participants[1]).unwrap().game_losses +=
                                first_games;
                        }
                    }
                    MatchResult::Bye { team_id } => {
                        let points = if self.config.format == TournamentFormat::Swiss {
                            self.config.swiss_policy.as_ref().unwrap().match_points.bye
                        } else {
                            0
                        };
                        rows.get_mut(team_id).unwrap().match_points += points;
                    }
                }
            }
        }
        for row in rows.values_mut() {
            row.net_game_wins = row.game_wins as i32 - row.game_losses as i32;
        }
        if self.config.format == TournamentFormat::Swiss {
            let policy = self.config.swiss_policy.as_ref().unwrap();
            for row in rows.values_mut() {
                let opponents = self
                    .opponents()
                    .get(&row.team_id)
                    .cloned()
                    .unwrap_or_default();
                let mut total_basis_points = 0u64;
                for opponent in &opponents {
                    total_basis_points +=
                        self.opponent_win_rate_basis_points(opponent, policy) as u64;
                }
                row.opponent_win_rate_basis_points = if opponents.is_empty() {
                    None
                } else {
                    Some((total_basis_points / opponents.len() as u64) as u32)
                };
            }
        }
        rows.into_values().collect()
    }

    fn opponent_win_rate_basis_points(
        &self,
        team_id: &TeamId,
        policy: &SwissStandingPolicy,
    ) -> u32 {
        let mut wins = 0u64;
        let mut draws = 0u64;
        let mut losses = 0u64;
        let mut byes = 0u64;
        for round in &self.rounds {
            for match_ in &round.matches {
                let Some(result) = &match_.result else {
                    continue;
                };
                if match_.participants.len() == 2 && match_.participants.contains(team_id) {
                    match result {
                        MatchResult::Win { winner_id, .. }
                        | MatchResult::Forfeit { winner_id, .. }
                            if winner_id == team_id =>
                        {
                            wins += 1
                        }
                        MatchResult::Win { loser_id, .. }
                        | MatchResult::Forfeit { loser_id, .. }
                            if loser_id == team_id =>
                        {
                            losses += 1
                        }
                        MatchResult::Draw { .. } => draws += 1,
                        MatchResult::DoubleLoss => losses += 1,
                        MatchResult::NoShow {
                            absent_teams,
                            winner_id,
                        } => {
                            if winner_id.as_ref() == Some(team_id) {
                                wins += 1;
                            } else if absent_teams.contains(team_id) {
                                losses += 1;
                            }
                        }
                        _ => {}
                    }
                } else if matches!(result, MatchResult::Bye { team_id: bye_team } if bye_team == team_id)
                {
                    byes += 1;
                }
            }
        }
        if policy.exclude_byes_from_opponent_win_rate {
            byes = 0;
        }
        let credit = &policy.draw_credit;
        let denominator = (wins + draws + losses + byes) * u64::from(credit.denominator);
        if denominator == 0 {
            return 0;
        }
        let numerator =
            (wins + byes) * u64::from(credit.denominator) + draws * u64::from(credit.numerator);
        let mut basis_points = ((numerator * 10_000) / denominator).min(10_000) as u32;
        if let Some(floor) = policy.opponent_win_rate_floor_percent {
            basis_points = basis_points.max(u32::from(floor) * 100);
        }
        if let Some(cap) = policy.opponent_win_rate_cap_percent {
            basis_points = basis_points.min(u32::from(cap) * 100);
        }
        basis_points
    }
}

fn validate_config(config: &StageConfig) -> Result<(), ScheduleError> {
    if config.teams.len() < 2 {
        return Err(ScheduleError::EmptyTeams);
    }
    competition_clock::validate_config(config.clock_config)
        .map_err(|error| ScheduleError::InvalidClockConfig(format!("{error:?}")))?;
    config
        .match_format
        .validate()
        .map_err(|error| ScheduleError::InvalidMatchFormat(error.to_string()))?;
    let is_elimination = matches!(
        config.format,
        TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination
    );
    match (is_elimination, config.match_format) {
        (true, MatchFormat::FirstToWins(_)) => {}
        (false, MatchFormat::FixedGames(_)) => {}
        (true, MatchFormat::FixedGames(_)) => {
            return Err(ScheduleError::InvalidMatchFormat(
                "淘汰賽只接受 firstToWins".to_string(),
            ));
        }
        (false, MatchFormat::FirstToWins(_)) => {
            return Err(ScheduleError::InvalidMatchFormat(
                "循環或 Swiss 只接受 fixedGames".to_string(),
            ));
        }
    }
    let mut ids = BTreeSet::new();
    let mut seeds = BTreeSet::new();
    for team in &config.teams {
        if !ids.insert(team.team_id.clone()) {
            return Err(ScheduleError::DuplicateTeam(team.team_id.clone()));
        }
        if team.seed == 0 || !seeds.insert(team.seed) {
            return Err(ScheduleError::InvalidSeed);
        }
    }
    if config.format == TournamentFormat::Swiss {
        if config.swiss_rounds.is_none() {
            return Err(ScheduleError::SwissRoundsRequired);
        }
        if config.swiss_rounds == Some(0) {
            return Err(ScheduleError::InvalidSwissPolicy(
                "swissRounds 必須大於零".to_string(),
            ));
        }
        let policy = config
            .swiss_policy
            .as_ref()
            .ok_or(ScheduleError::SwissPolicyRequired)?;
        validate_swiss_policy(policy)?;
    }
    Ok(())
}

fn seeded_order(teams: &[SeededTeam]) -> Vec<TeamId> {
    let mut teams = teams.to_vec();
    teams.sort_by_key(|team| team.seed);
    teams.into_iter().map(|team| team.team_id).collect()
}

fn shuffled_order(teams: &[SeededTeam], seed: u64) -> Vec<TeamId> {
    let mut order = seeded_order(teams);
    let mut state = seed;
    for index in (1..order.len()).rev() {
        state = split_mix64(state);
        let swap_index = (state % u64::from(index as u32 + 1)) as usize;
        order.swap(index, swap_index);
    }
    order
}

fn split_mix64(mut value: u64) -> u64 {
    value = value.wrapping_add(0x9e3779b97f4a7c15);
    let mut result = value;
    result = (result ^ (result >> 30)).wrapping_mul(0xbf58476d1ce4e5b9);
    result = (result ^ (result >> 27)).wrapping_mul(0x94d049bb133111eb);
    result ^ (result >> 31)
}

fn pair_adjacent_with_byes(order: &[TeamId]) -> Vec<Vec<TeamId>> {
    let mut pairs = vec![];
    let mut index = 0;
    while index < order.len() {
        if index + 1 < order.len() {
            pairs.push(vec![order[index].clone(), order[index + 1].clone()]);
        } else {
            pairs.push(vec![order[index].clone()]);
        }
        index += 2;
    }
    pairs
}

fn seeded_bracket_pairs(order: &[TeamId]) -> Vec<Vec<TeamId>> {
    let bracket_size = order.len().next_power_of_two();
    let mut seed_positions = vec![1usize];
    while seed_positions.len() < bracket_size {
        let next_seed = seed_positions.len() * 2 + 1;
        let mut next_positions = Vec::with_capacity(seed_positions.len() * 2);
        for seed in seed_positions {
            next_positions.push(seed);
            next_positions.push(next_seed - seed);
        }
        seed_positions = next_positions;
    }
    seed_positions
        .chunks(2)
        .map(|pair| {
            pair.iter()
                .filter_map(|seed| order.get(seed - 1).cloned())
                .collect::<Vec<_>>()
        })
        .collect()
}

fn take_lower_pairing(state: &mut DoubleState) -> Vec<Vec<TeamId>> {
    let mut pool = std::mem::take(&mut state.lower_pool);
    pool.append(&mut state.losers_waiting);
    pair_adjacent_with_byes(&pool)
}

fn completed_winners(round: &Round) -> Result<Vec<TeamId>, ScheduleError> {
    round
        .matches
        .iter()
        .filter_map(|match_| match_.result.as_ref())
        .map(|result| {
            result
                .winner()
                .cloned()
                .ok_or_else(|| ScheduleError::InvalidOperation("和局沒有晉級隊伍".to_string()))
        })
        .collect()
}

fn completed_losers(round: &Round) -> Result<Vec<TeamId>, ScheduleError> {
    round
        .matches
        .iter()
        .filter_map(|match_| match_.result.as_ref())
        .filter_map(|result| result.loser().cloned())
        .collect::<Vec<_>>()
        .pipe(Ok)
}

trait Pipe: Sized {
    fn pipe<T>(self, function: impl FnOnce(Self) -> T) -> T {
        function(self)
    }
}

impl<T> Pipe for T {}

fn is_closed(match_: &ScheduledMatch) -> bool {
    matches!(
        match_.status,
        MatchStatus::Completed | MatchStatus::Cancelled
    )
}

fn validate_match_result(
    format: &TournamentFormat,
    kind: RoundKind,
    match_id: &MatchId,
    participants: &[TeamId],
    result: &MatchResult,
    match_format: MatchFormat,
) -> Result<(), ScheduleError> {
    let is_elimination = matches!(
        format,
        TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination
    );
    if is_elimination && matches!(result, MatchResult::Draw { .. }) {
        return Err(ScheduleError::DrawNotAllowed(match_id.clone()));
    }
    match result {
        MatchResult::DoubleLoss => {
            if participants.len() != 2 || participants[0] == participants[1] {
                return Err(ScheduleError::ResultDoesNotMatchParticipants(
                    match_id.clone(),
                ));
            }
        }
        MatchResult::Win {
            winner_id,
            loser_id,
            winner_games,
            loser_games,
        } => {
            let score_is_valid = match match_format {
                MatchFormat::FirstToWins(wins) => {
                    u32::from(*winner_games) == u32::from(wins)
                        && u32::from(*loser_games) < u32::from(wins)
                }
                // fixedGames 的 MatchResult 只保存整場比分，無法表示單局和局。
                MatchFormat::FixedGames(_) => true,
            };
            if participants.len() != 2
                || !participants.contains(winner_id)
                || !participants.contains(loser_id)
                || winner_id == loser_id
                || *winner_games <= *loser_games
                || !score_is_valid
            {
                return Err(ScheduleError::InvalidScore(match_id.clone()));
            }
        }
        MatchResult::Forfeit {
            winner_id,
            loser_id,
        } => {
            if participants.len() != 2
                || !participants.contains(winner_id)
                || !participants.contains(loser_id)
                || winner_id == loser_id
            {
                return Err(ScheduleError::ResultDoesNotMatchParticipants(
                    match_id.clone(),
                ));
            }
        }
        MatchResult::NoShow {
            absent_teams,
            winner_id,
        } => {
            let absent: BTreeSet<_> = absent_teams.iter().collect();
            let valid = participants.len() == 2
                && !absent_teams.is_empty()
                && absent.len() == absent_teams.len()
                && absent_teams.iter().all(|team| participants.contains(team));
            let outcome_matches = match absent_teams.len() {
                1 => winner_id.as_ref().is_some_and(|winner| {
                    participants.contains(winner) && !absent.contains(winner)
                }),
                2 => winner_id.is_none(),
                _ => false,
            };
            if !valid || !outcome_matches {
                return Err(ScheduleError::ResultDoesNotMatchParticipants(match_id.clone()));
            }
        }
        MatchResult::Draw {
            first_games,
            second_games,
        } => {
            if participants.len() != 2 || kind != RoundKind::Main {
                return Err(ScheduleError::ResultDoesNotMatchParticipants(
                    match_id.clone(),
                ));
            }
            if first_games != second_games {
                return Err(ScheduleError::InvalidScore(match_id.clone()));
            }
        }
        MatchResult::Bye { team_id } => {
            if participants.len() != 1 || participants[0] != *team_id {
                return Err(ScheduleError::ResultDoesNotMatchParticipants(
                    match_id.clone(),
                ));
            }
        }
    }
    Ok(())
}

fn pair_backtracking(
    remaining: &mut Vec<TeamId>,
    opponents: &BTreeMap<TeamId, BTreeSet<TeamId>>,
    pairs: &mut Vec<Vec<TeamId>>,
) -> Result<(), ScheduleError> {
    if remaining.is_empty() {
        return Ok(());
    }
    let first = remaining.remove(0);
    for index in 0..remaining.len() {
        let candidate = remaining[index].clone();
        if opponents
            .get(&first)
            .is_some_and(|played| played.contains(&candidate))
        {
            continue;
        }
        let second = remaining.remove(index);
        pairs.push(vec![first.clone(), second.clone()]);
        if pair_backtracking(remaining, opponents, pairs).is_ok() {
            return Ok(());
        }
        pairs.pop();
        remaining.insert(index, second);
    }
    remaining.insert(0, first);
    Err(ScheduleError::NoLegalSwissPairing)
}

fn match_result_points(result: &MatchResult, team_id: &TeamId, config: &StageConfig) -> u32 {
    let points = if config.format == TournamentFormat::Swiss {
        &config.swiss_policy.as_ref().unwrap().match_points
    } else {
        static ROUND_ROBIN_POINTS: MatchPoints = MatchPoints {
            win: 3,
            draw: 1,
            loss: 0,
            bye: 0,
        };
        &ROUND_ROBIN_POINTS
    };
    match result {
        MatchResult::Win { winner_id, .. } | MatchResult::Forfeit { winner_id, .. }
            if winner_id == team_id =>
        {
            points.win
        }
        MatchResult::Win { loser_id, .. } | MatchResult::Forfeit { loser_id, .. }
            if loser_id == team_id =>
        {
            points.loss
        }
        MatchResult::NoShow {
            winner_id: Some(winner_id),
            ..
        } if winner_id == team_id => points.win,
        MatchResult::NoShow { absent_teams, .. } if absent_teams.contains(team_id) => points.loss,
        MatchResult::Draw { .. } if config.format == TournamentFormat::Swiss => points.draw,
        MatchResult::Draw { .. } => points.draw,
        MatchResult::Bye { team_id: bye_team } if bye_team == team_id => points.bye,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn teams(count: u32) -> Vec<SeededTeam> {
        (1..=count)
            .map(|seed| SeededTeam {
                team_id: TeamId(format!("t{seed}")),
                seed,
            })
            .collect()
    }

    fn config(format: TournamentFormat, count: u32) -> StageConfig {
        StageConfig {
            stage_id: StageId("stage".to_string()),
            format,
            teams: teams(count),
            random_seed: 7,
            match_format: if matches!(
                format,
                TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination
            ) {
                MatchFormat::FirstToWins(1)
            } else {
                MatchFormat::FixedGames(1)
            },
            clock_config: ClockConfig::default(),
            swiss_rounds: None,
            swiss_policy: None,
        }
    }

    #[test]
    fn serializes_public_multiword_fields_as_camel_case() {
        let json = serde_json::to_value(SeededTeam {
            team_id: TeamId("t1".to_string()),
            seed: 1,
        })
        .unwrap();
        assert_eq!(json, serde_json::json!({"teamId": "t1", "seed": 1}));
    }

    #[test]
    fn announcement_plan_uses_exact_camel_case_and_rejects_invalid_clock_or_nested_fields() {
        let plan = CompetitionAnnouncementPlan {
            initial_seed_policy: InitialSeedPolicy::RandomDrawAfterRegistrationDeadline,
            swiss_pairing_policy: SwissPairingPolicy::PublishedRulesWithPersistedRandomSeed,
            ranking_policy: CompetitionRankingPolicy::MatchPointsThenOpponentWinsThenOpponentLosses,
            exact_ranking_tie_policy: ExactRankingTiePolicy::OrganizerOrUnconflictedRefereeWithPublicReason,
            stages: vec![AnnouncedTournamentStage {
                stage_id: StageId::from("stage-1"),
                format: TournamentFormat::Swiss,
                match_format: MatchFormat::FixedGames(3),
                clock_config: ClockConfig {
                    main_time_ms: Some(0),
                    action_time_ms: Some(60_000),
                    out_of_turn_choice_time_ms: Some(60_000),
                },
                swiss_rounds: Some(5),
                swiss_policy: Some(SwissStandingPolicy {
                    match_points: MatchPoints { win: 3, draw: 1, loss: 0, bye: 3 },
                    draw_credit: DrawCredit::new(1, 2).unwrap(),
                    opponent_win_rate_floor_percent: None,
                    opponent_win_rate_cap_percent: None,
                    exclude_byes_from_opponent_win_rate: false,
                    tie_breakers: vec![SwissTieBreaker::OpponentWinRate],
                }),
                qualification: AnnouncedStageQualification::AllEligibleTeams,
            }],
        };
        validate_competition_announcement_plan(&plan).unwrap();
        let value = serde_json::to_value(&plan).unwrap();
        assert_eq!(
            value,
            serde_json::json!({
                "initialSeedPolicy": "randomDrawAfterRegistrationDeadline",
                "swissPairingPolicy": "publishedRulesWithPersistedRandomSeed",
                "rankingPolicy": "matchPointsThenOpponentWinsThenOpponentLosses",
                "exactRankingTiePolicy": "organizerOrUnconflictedRefereeWithPublicReason",
                "stages": [{
                    "stageId": "stage-1", "format": "swiss",
                    "matchFormat": { "type": "fixedGames", "value": 3 },
                    "clockConfig": { "mainTimeMs": 0, "actionTimeMs": 60000, "outOfTurnChoiceTimeMs": 60000 },
                    "swissRounds": 5, "swissPolicy": {
                        "matchPoints": { "win": 3, "draw": 1, "loss": 0, "bye": 3 },
                        "drawCredit": { "numerator": 1, "denominator": 2 },
                        "opponentWinRateFloorPercent": null, "opponentWinRateCapPercent": null,
                        "excludeByesFromOpponentWinRate": false, "tieBreakers": ["opponentWinRate"]
                    },
                    "qualification": { "type": "allEligibleTeams" }
                }]
            })
        );

        let invalid_clock = AnnouncedTournamentStage {
            clock_config: ClockConfig {
                main_time_ms: Some(30_000),
                action_time_ms: Some(90_000),
                out_of_turn_choice_time_ms: Some(90_000),
            },
            ..plan.stages[0].clone()
        };
        assert!(validate_competition_announcement_plan(&CompetitionAnnouncementPlan {
            stages: vec![invalid_clock], ..plan.clone()
        }).is_err());

        let mut unknown_nested = value.clone();
        unknown_nested["stages"][0]["clockConfig"]["unannounced"] = serde_json::json!(true);
        assert!(serde_json::from_value::<CompetitionAnnouncementPlan>(unknown_nested).is_err());
    }

    #[test]
    fn serializes_match_result_fields_as_camel_case() {
        let json = serde_json::to_value(MatchResult::win_with_games("t1", "t2", 2, 1)).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "type": "win",
                "winnerId": "t1",
                "loserId": "t2",
                "winnerGames": 2,
                "loserGames": 1
            })
        );
    }

    #[test]
    fn no_show_result_records_absence_without_marking_the_match_started() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap();
        let match_id = stage.rounds()[0].matches[0].match_id.clone();
        let result = MatchResult::NoShow {
            absent_teams: vec![TeamId::from("t1")],
            winner_id: Some(TeamId::from("t2")),
        };

        stage.submit_result(&match_id, result.clone()).unwrap();

        let match_ = &stage.rounds()[0].matches[0];
        assert_eq!(match_.status, MatchStatus::Completed);
        assert!(!match_.started);
        assert_eq!(match_.result, Some(result));
        let standings = stage.standings();
        assert_eq!(standings.iter().find(|row| row.team_id.0 == "t1").unwrap().losses, 1);
        assert_eq!(standings.iter().find(|row| row.team_id.0 == "t2").unwrap().wins, 1);
    }

    #[test]
    fn double_no_show_counts_two_losses_and_serializes_its_reason() {
        let result = MatchResult::NoShow {
            absent_teams: vec![TeamId::from("t1"), TeamId::from("t2")],
            winner_id: None,
        };
        assert_eq!(
            serde_json::to_value(&result).unwrap(),
            serde_json::json!({ "type": "noShow", "absentTeams": ["t1", "t2"], "winnerId": null }),
        );

        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap();
        let match_id = stage.rounds()[0].matches[0].match_id.clone();
        stage.submit_result(&match_id, result).unwrap();
        let standings = stage.standings();
        assert_eq!(standings.iter().find(|row| row.team_id.0 == "t1").unwrap().losses, 1);
        assert_eq!(standings.iter().find(|row| row.team_id.0 == "t2").unwrap().losses, 1);
    }

    #[test]
    fn swiss_no_show_losses_receive_the_published_loss_points() {
        let mut policy = swiss_policy(DrawCredit::new(1, 2).unwrap());
        policy.match_points.loss = 2;

        let mut single_absence = Stage::new(swiss_config(2, 1, policy.clone())).unwrap();
        let match_id = single_absence.rounds()[0].matches[0].match_id.clone();
        single_absence
            .submit_result(
                &match_id,
                MatchResult::NoShow {
                    absent_teams: vec![TeamId::from("t1")],
                    winner_id: Some(TeamId::from("t2")),
                },
            )
            .unwrap();
        let standings = single_absence.standings();
        assert_eq!(
            standings.iter().find(|row| row.team_id.0 == "t1").unwrap().match_points,
            2
        );
        assert_eq!(
            standings.iter().find(|row| row.team_id.0 == "t2").unwrap().match_points,
            3
        );

        let mut double_absence = Stage::new(swiss_config(2, 1, policy)).unwrap();
        let match_id = double_absence.rounds()[0].matches[0].match_id.clone();
        double_absence
            .submit_result(
                &match_id,
                MatchResult::NoShow {
                    absent_teams: vec![TeamId::from("t1"), TeamId::from("t2")],
                    winner_id: None,
                },
            )
            .unwrap();
        let standings = double_absence.standings();
        assert_eq!(
            standings.iter().find(|row| row.team_id.0 == "t1").unwrap().match_points,
            2
        );
        assert_eq!(
            standings.iter().find(|row| row.team_id.0 == "t2").unwrap().match_points,
            2
        );
    }

    #[test]
    fn upstream_shape_is_not_needed_for_non_power_of_two_single_elimination() {
        let stage = Stage::new(config(TournamentFormat::SingleElimination, 5)).unwrap();
        assert_eq!(stage.rounds[0].matches.len(), 4);
        assert!(
            stage.rounds[0]
                .matches
                .iter()
                .any(|match_| matches!(match_.result, Some(MatchResult::Bye { .. })))
        );
    }

    #[test]
    fn swiss_requires_explicit_policy() {
        let mut config = config(TournamentFormat::Swiss, 5);
        config.swiss_rounds = Some(2);
        assert_eq!(
            Stage::new(config).unwrap_err(),
            ScheduleError::SwissPolicyRequired
        );
    }

    fn swiss_policy(draw_credit: DrawCredit) -> SwissStandingPolicy {
        SwissStandingPolicy {
            match_points: MatchPoints {
                win: 3,
                draw: 1,
                loss: 0,
                bye: 3,
            },
            draw_credit,
            opponent_win_rate_floor_percent: None,
            opponent_win_rate_cap_percent: None,
            exclude_byes_from_opponent_win_rate: false,
            tie_breakers: vec![SwissTieBreaker::OpponentWinRate],
        }
    }

    fn swiss_config(count: u32, rounds: u32, policy: SwissStandingPolicy) -> StageConfig {
        let mut config = config(TournamentFormat::Swiss, count);
        config.swiss_rounds = Some(rounds);
        config.swiss_policy = Some(policy);
        config
    }

    fn stage_qualification_tournament(stage: Stage) -> Tournament {
        Tournament {
            tournament_id: "tour".to_string(),
            stages: vec![stage],
        }
    }

    fn stage_qualification_slot(
        source_stage_id: &str,
        source: StageQualificationSource,
        destination_seed: u32,
    ) -> StageQualificationSlot {
        StageQualificationSlot {
            source_stage_id: StageId::from(source_stage_id),
            source,
            destination_seed,
        }
    }

    fn stage_qualification_new_stage(
        format: TournamentFormat,
        stage_id: &str,
        team_prefix: &str,
    ) -> Stage {
        let mut config = if format == TournamentFormat::Swiss {
            swiss_config(2, 1, swiss_policy(DrawCredit::new(1, 2).unwrap()))
        } else {
            config(format, 2)
        };
        config.stage_id = StageId::from(stage_id);
        for (index, team) in config.teams.iter_mut().enumerate() {
            team.team_id = TeamId(format!("{team_prefix}{}", index + 1));
        }
        Stage::new(config).unwrap()
    }

    fn stage_qualification_finish_with_winner(
        mut stage: Stage,
        preferred_winner: &TeamId,
    ) -> Stage {
        while stage.status() != StageStatus::Finished {
            let Some(match_) = stage
                .rounds()
                .iter()
                .flat_map(|round| &round.matches)
                .find(|match_| match_.status == MatchStatus::Pending)
                .cloned()
            else {
                break;
            };
            let winner = match_
                .participants
                .iter()
                .find(|team_id| *team_id == preferred_winner)
                .cloned()
                .unwrap_or_else(|| match_.participants[0].clone());
            let loser = match_
                .participants
                .iter()
                .find(|team_id| *team_id != &winner)
                .cloned()
                .expect("淘汰或積分交手必有兩隊");
            stage
                .submit_result(&match_.match_id, MatchResult::win(winner, loser))
                .unwrap();
        }
        stage
    }

    fn stage_qualification_two_stage_winners(format: TournamentFormat) -> Tournament {
        let first = stage_qualification_finish_with_winner(
            stage_qualification_new_stage(format, "stage-a", "a"),
            &TeamId::from("a1"),
        );
        let second = stage_qualification_finish_with_winner(
            stage_qualification_new_stage(format, "stage-b", "b"),
            &TeamId::from("b2"),
        );
        Tournament {
            tournament_id: "tour".to_string(),
            stages: vec![first, second],
        }
    }

    fn stage_qualification_round_robin_tie() -> Stage {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 3)).unwrap();
        let match_ids: Vec<_> = stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .filter(|match_| match_.status == MatchStatus::Pending)
            .map(|match_| match_.match_id.clone())
            .collect();
        let t1 = TeamId::from("t1");
        let t2 = TeamId::from("t2");
        for match_id in match_ids {
            let match_ = stage.find_match(&match_id).unwrap().clone();
            let result = if match_.participants.contains(&t1) && match_.participants.contains(&t2) {
                MatchResult::draw()
            } else {
                let winner = if match_.participants.contains(&t1) {
                    t1.clone()
                } else {
                    t2.clone()
                };
                let loser = match_
                    .participants
                    .iter()
                    .find(|team_id| *team_id != &winner)
                    .cloned()
                    .unwrap();
                MatchResult::win(winner, loser)
            };
            stage.submit_result(&match_id, result).unwrap();
        }
        stage
    }

    #[test]
    fn unfinished_single_elimination_has_no_winner() {
        let stage = Stage::new(config(TournamentFormat::SingleElimination, 2)).unwrap();
        assert_eq!(stage.status(), StageStatus::Active);
        assert_eq!(stage.winner(), None);
    }

    #[test]
    fn draw_with_unequal_game_counts_is_rejected() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap();
        let match_id = stage.rounds()[0].matches[0].match_id.clone();
        let snapshot = stage.clone();
        assert_eq!(
            stage.submit_result(&match_id, MatchResult::draw_with_games(2, 0)),
            Err(ScheduleError::InvalidScore(match_id.clone()))
        );
        assert_eq!(stage, snapshot);
    }

    #[test]
    fn double_elimination_schedules_and_completes_grand_final_reset() {
        let mut stage = Stage::new(config(TournamentFormat::DoubleElimination, 2)).unwrap();
        let winners_match = stage.rounds()[0].matches[0].match_id.clone();
        stage
            .submit_result(&winners_match, MatchResult::win("t1", "t2"))
            .unwrap();

        let grand_final = stage.rounds().last().unwrap().matches[0].match_id.clone();
        assert_eq!(stage.rounds().last().unwrap().kind, RoundKind::GrandFinal);
        stage
            .submit_result(&grand_final, MatchResult::win("t2", "t1"))
            .unwrap();

        assert_eq!(
            stage.rounds().last().unwrap().kind,
            RoundKind::GrandFinalReset
        );
        let reset = stage.rounds().last().unwrap().matches[0].match_id.clone();
        stage
            .submit_result(&reset, MatchResult::win("t2", "t1"))
            .unwrap();
        assert_eq!(stage.status(), StageStatus::Finished);
        assert_eq!(stage.winner(), Some(TeamId::from("t2")));
    }

    #[test]
    fn swiss_omw_uses_explicit_draw_credit_and_precise_floor_or_cap() {
        let zero_credit = swiss_policy(DrawCredit::new(0, 1).unwrap());
        let half_credit = swiss_policy(DrawCredit::new(1, 2).unwrap());
        for (policy, expected) in [(zero_credit, 0), (half_credit, 5_000)] {
            let mut stage = Stage::new(swiss_config(4, 1, policy)).unwrap();
            let match_ids: Vec<_> = stage.rounds()[0]
                .matches
                .iter()
                .filter(|match_| match_.participants.len() == 2)
                .map(|match_| match_.match_id.clone())
                .collect();
            for match_id in match_ids {
                stage.submit_result(&match_id, MatchResult::draw()).unwrap();
            }
            let policy = stage.config().swiss_policy.as_ref().unwrap();
            let target = TeamId::from("t1");
            assert_eq!(
                stage.opponent_win_rate_basis_points(&target, policy),
                expected
            );
        }

        for (floor, cap, expected) in [(Some(60), None, 6_000), (None, Some(40), 4_000)] {
            let mut policy = swiss_policy(DrawCredit::new(1, 2).unwrap());
            policy.opponent_win_rate_floor_percent = floor;
            policy.opponent_win_rate_cap_percent = cap;
            let mut stage = Stage::new(swiss_config(4, 1, policy)).unwrap();
            let match_id = stage.rounds()[0]
                .matches
                .iter()
                .find(|match_| match_.participants.len() == 2)
                .unwrap()
                .match_id
                .clone();
            let participants = stage.rounds()[0]
                .matches
                .iter()
                .find(|match_| match_.match_id == match_id)
                .unwrap()
                .participants
                .clone();
            stage
                .submit_result(
                    &match_id,
                    MatchResult::win(participants[0].clone(), participants[1].clone()),
                )
                .unwrap();
            let policy = stage.config().swiss_policy.as_ref().unwrap();
            assert_eq!(
                stage.opponent_win_rate_basis_points(
                    &participants[usize::from(floor.is_some())],
                    policy
                ),
                expected
            );
        }
    }

    #[test]
    fn dropping_team_turns_pending_pairing_into_forfeit() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap();
        let match_id = stage.rounds()[0].matches[0].match_id.clone();
        stage.drop_team(&TeamId::from("t1")).unwrap();
        let match_ = &stage.rounds()[0].matches[0];
        assert_eq!(match_.match_id, match_id);
        assert_eq!(match_.status, MatchStatus::Completed);
        assert_eq!(match_.result, Some(MatchResult::forfeit("t2", "t1")));
        assert_eq!(stage.standings()[0].team_id, TeamId::from("t2"));
        assert_eq!(stage.standings()[0].wins, 1);
    }

    #[test]
    fn round_robin_correction_preserves_unrelated_started_future() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 3)).unwrap();
        let first = stage.rounds()[0]
            .matches
            .iter()
            .find(|match_| match_.participants.len() == 2)
            .unwrap()
            .clone();
        let original =
            MatchResult::win(first.participants[0].clone(), first.participants[1].clone());
        let replacement =
            MatchResult::win(first.participants[1].clone(), first.participants[0].clone());
        stage
            .submit_result(&first.match_id, original.clone())
            .unwrap();
        stage
            .correct_result(&first.match_id, replacement.clone(), "裁判更正".to_string())
            .unwrap();

        let future = stage.rounds()[1]
            .matches
            .iter()
            .find(|match_| match_.participants.len() == 2)
            .unwrap()
            .clone();
        stage.start_match(&future.match_id).unwrap();
        stage
            .correct_result(&first.match_id, original.clone(), "再次更正".to_string())
            .unwrap();
        assert_eq!(
            stage.find_match(&future.match_id).unwrap().status,
            MatchStatus::InProgress
        );
        assert_eq!(
            stage.find_match(&first.match_id).unwrap().result,
            Some(original)
        );
    }

    #[test]
    fn invalid_swiss_policy_and_duplicate_seed_are_rejected() {
        let mut zero_rounds = swiss_config(4, 1, swiss_policy(DrawCredit::new(1, 2).unwrap()));
        zero_rounds.swiss_rounds = Some(0);
        assert!(matches!(
            Stage::new(zero_rounds),
            Err(ScheduleError::InvalidSwissPolicy(_))
        ));

        for (floor, cap) in [(Some(101), None), (None, Some(101)), (Some(80), Some(60))] {
            let mut invalid = swiss_config(4, 1, swiss_policy(DrawCredit::new(1, 2).unwrap()));
            invalid
                .swiss_policy
                .as_mut()
                .unwrap()
                .opponent_win_rate_floor_percent = floor;
            invalid
                .swiss_policy
                .as_mut()
                .unwrap()
                .opponent_win_rate_cap_percent = cap;
            assert!(matches!(
                Stage::new(invalid),
                Err(ScheduleError::InvalidSwissPolicy(_))
            ));
        }
        assert!(matches!(
            DrawCredit::new(2, 1),
            Err(ScheduleError::InvalidSwissPolicy(_))
        ));

        let mut duplicate_seed = config(TournamentFormat::SingleElimination, 2);
        duplicate_seed.teams[1].seed = 1;
        assert_eq!(Stage::new(duplicate_seed), Err(ScheduleError::InvalidSeed));
    }

    #[test]
    fn non_power_of_two_seeded_bracket_keeps_seed_protection_without_sentinel_id() {
        let stage = Stage::new(config(TournamentFormat::SingleElimination, 5)).unwrap();
        assert_eq!(stage.rounds()[0].matches.len(), 4);
        assert!(stage.rounds()[0].matches.iter().all(|match_| {
            !(match_.participants.contains(&TeamId::from("t1"))
                && match_.participants.contains(&TeamId::from("t2")))
        }));

        let mut sentinel_named_team = config(TournamentFormat::SingleElimination, 3);
        sentinel_named_team.teams[0].team_id = TeamId::from("__bye__");
        let stage = Stage::new(sentinel_named_team).unwrap();
        assert!(stage.rounds()[0].matches.iter().any(|match_| {
            match_.participants == vec![TeamId::from("__bye__")]
                && matches!(match_.result, Some(MatchResult::Bye { .. }))
        }));
    }

    #[test]
    fn stage_qualification_resolves_round_robin_ranks_without_reordering_or_mutation() {
        let stage = stage_qualification_finish_with_winner(
            Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap(),
            &TeamId::from("t1"),
        );
        let tournament = stage_qualification_tournament(stage);
        let before = serde_json::to_value(&tournament).unwrap();
        let slots = [
            stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 2 },
                9,
            ),
            stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 1 },
                3,
            ),
        ];

        let resolved = tournament.resolve_stage_qualification(&slots).unwrap();
        assert_eq!(
            resolved.seeded_teams(),
            &[
                SeededTeam {
                    team_id: TeamId::from("t2"),
                    seed: 9,
                },
                SeededTeam {
                    team_id: TeamId::from("t1"),
                    seed: 3,
                },
            ]
        );
        assert_eq!(
            resolved.source_mappings(),
            &[
                StageQualificationMapping {
                    source_stage_id: StageId::from("stage"),
                    source: StageQualificationSource::UniqueStandingRank { rank: 2 },
                    team_id: TeamId::from("t2"),
                    destination_seed: 9,
                },
                StageQualificationMapping {
                    source_stage_id: StageId::from("stage"),
                    source: StageQualificationSource::UniqueStandingRank { rank: 1 },
                    team_id: TeamId::from("t1"),
                    destination_seed: 3,
                },
            ]
        );
        assert_eq!(
            serde_json::to_value(&resolved).unwrap(),
            serde_json::json!({
                "seededTeams": [
                    {"teamId": "t2", "seed": 9},
                    {"teamId": "t1", "seed": 3}
                ],
                "sourceMappings": [
                    {
                        "sourceStageId": "stage",
                        "source": {"type": "uniqueStandingRank", "rank": 2},
                        "teamId": "t2",
                        "destinationSeed": 9
                    },
                    {
                        "sourceStageId": "stage",
                        "source": {"type": "uniqueStandingRank", "rank": 1},
                        "teamId": "t1",
                        "destinationSeed": 3
                    }
                ]
            })
        );
        assert_eq!(
            tournament.resolve_stage_qualification(&slots).unwrap(),
            resolved
        );
        assert_eq!(serde_json::to_value(&tournament).unwrap(), before);
    }

    #[test]
    fn stage_qualification_resolves_unique_swiss_standing_ranks() {
        let stage = stage_qualification_finish_with_winner(
            stage_qualification_new_stage(TournamentFormat::Swiss, "stage", "s"),
            &TeamId::from("s2"),
        );
        let tournament = stage_qualification_tournament(stage);
        let slots = [
            stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 1 },
                6,
            ),
            stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 2 },
                4,
            ),
        ];

        let resolved = tournament.resolve_stage_qualification(&slots).unwrap();
        assert_eq!(
            resolved.seeded_teams(),
            &[
                SeededTeam {
                    team_id: TeamId::from("s2"),
                    seed: 6,
                },
                SeededTeam {
                    team_id: TeamId::from("s1"),
                    seed: 4,
                },
            ]
        );
    }

    #[test]
    fn stage_qualification_resolves_finished_winners_for_all_formats() {
        for format in [
            TournamentFormat::SingleElimination,
            TournamentFormat::DoubleElimination,
            TournamentFormat::RoundRobin,
            TournamentFormat::Swiss,
        ] {
            let tournament = stage_qualification_two_stage_winners(format);
            let slots = [
                stage_qualification_slot(
                    "stage-a",
                    StageQualificationSource::FinishedStageWinner,
                    8,
                ),
                stage_qualification_slot(
                    "stage-b",
                    StageQualificationSource::FinishedStageWinner,
                    2,
                ),
            ];

            let resolved = tournament.resolve_stage_qualification(&slots).unwrap();
            assert_eq!(
                resolved.seeded_teams(),
                &[
                    SeededTeam {
                        team_id: TeamId::from("a1"),
                        seed: 8,
                    },
                    SeededTeam {
                        team_id: TeamId::from("b2"),
                        seed: 2,
                    },
                ]
            );
        }
    }

    #[test]
    fn stage_qualification_uses_rank_values_and_blocks_ties_and_missing_ranks() {
        let stage = stage_qualification_round_robin_tie();
        assert_eq!(stage.status(), StageStatus::Finished);
        assert_eq!(stage.winner(), None);
        let tournament = stage_qualification_tournament(stage);

        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 3 },
                    3,
                ),
            ]),
            Err(StageQualificationBlocker::StandingRankNotUnique {
                source_stage_id: StageId::from("stage"),
                rank: 1,
                team_ids: vec![TeamId::from("t1"), TeamId::from("t2")],
            })
        );
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 2 },
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 3 },
                    3,
                ),
            ]),
            Err(StageQualificationBlocker::StandingRankNotFound {
                source_stage_id: StageId::from("stage"),
                rank: 2,
            })
        );
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::FinishedStageWinner,
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 3 },
                    3,
                ),
            ]),
            Err(StageQualificationBlocker::WinnerNotDetermined {
                source_stage_id: StageId::from("stage"),
            })
        );
    }

    #[test]
    fn stage_qualification_blocks_dropped_and_unassigned_standing_rows() {
        let mut dropped_stage = Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap();
        dropped_stage.drop_team(&TeamId::from("t1")).unwrap();
        assert_eq!(dropped_stage.status(), StageStatus::Finished);
        let dropped_tournament = stage_qualification_tournament(dropped_stage);
        assert_eq!(
            dropped_tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 2 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::DroppedTeam {
                source_stage_id: StageId::from("stage"),
                team_id: TeamId::from("t1"),
            })
        );

        let mut unassigned_stage = stage_qualification_finish_with_winner(
            Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap(),
            &TeamId::from("t1"),
        );
        // 此處僅注入負例，驗證完成階段若含未定 rank 不會被解析。
        unassigned_stage.standings[0].rank = None;
        let unassigned_tournament = stage_qualification_tournament(unassigned_stage);
        assert!(matches!(
            unassigned_tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 2 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::StandingRankUnavailable { .. })
        ));
    }

    #[test]
    fn stage_qualification_blocks_unfinished_and_missing_source_stages() {
        let unfinished = stage_qualification_tournament(
            Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap(),
        );
        let rank_slots = [
            stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 1 },
                1,
            ),
            stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 2 },
                2,
            ),
        ];
        assert_eq!(
            unfinished.resolve_stage_qualification(&rank_slots),
            Err(StageQualificationBlocker::SourceStageNotFinished {
                source_stage_id: StageId::from("stage"),
            })
        );

        let missing_slots = [
            stage_qualification_slot(
                "missing",
                StageQualificationSource::UniqueStandingRank { rank: 1 },
                1,
            ),
            stage_qualification_slot(
                "missing",
                StageQualificationSource::UniqueStandingRank { rank: 2 },
                2,
            ),
        ];
        assert_eq!(
            unfinished.resolve_stage_qualification(&missing_slots),
            Err(StageQualificationBlocker::MissingSourceStage {
                source_stage_id: StageId::from("missing"),
            })
        );
    }

    #[test]
    fn stage_qualification_blocks_schedule_match_and_double_loss_unresolved_facts() {
        let mut schedule_error_stage = stage_qualification_finish_with_winner(
            Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap(),
            &TeamId::from("t1"),
        );
        schedule_error_stage.last_schedule_error = Some("failure injection".to_string());
        let schedule_error_tournament = stage_qualification_tournament(schedule_error_stage);
        let rank_slots = [
            stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 1 },
                1,
            ),
            stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 2 },
                2,
            ),
        ];
        assert_eq!(
            schedule_error_tournament.resolve_stage_qualification(&rank_slots),
            Err(StageQualificationBlocker::SourceScheduleUnresolved {
                source_stage_id: StageId::from("stage"),
            })
        );

        let mut unresolved_match_stage = stage_qualification_finish_with_winner(
            Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap(),
            &TeamId::from("t1"),
        );
        unresolved_match_stage.rounds[0].matches[0].status = MatchStatus::InProgress;
        let unresolved_match_tournament =
            stage_qualification_tournament(unresolved_match_stage);
        assert!(matches!(
            unresolved_match_tournament.resolve_stage_qualification(&rank_slots),
            Err(StageQualificationBlocker::SourceMatchUnresolved {
                status: MatchStatus::InProgress,
                ..
            })
        ));

        let mut double_loss_stage =
            Stage::new(config(TournamentFormat::DoubleElimination, 2)).unwrap();
        let double_loss_match = double_loss_stage.rounds()[0].matches[0]
            .match_id
            .clone();
        assert!(matches!(
            double_loss_stage.submit_result(&double_loss_match, MatchResult::DoubleLoss),
            Ok(UpdateOutcome::AppliedWithScheduleError(_))
        ));
        let double_loss_tournament = stage_qualification_tournament(double_loss_stage);
        assert_eq!(
            double_loss_tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::FinishedStageWinner,
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::DoubleLossNeedsReferee {
                source_stage_id: StageId::from("stage"),
                match_id: double_loss_match,
            })
        );
    }

    #[test]
    fn stage_qualification_blocks_elimination_standing_rank_sources() {
        let stage = stage_qualification_finish_with_winner(
            Stage::new(config(TournamentFormat::SingleElimination, 2)).unwrap(),
            &TeamId::from("t1"),
        );
        let tournament = stage_qualification_tournament(stage);
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 2 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::StandingRanksUnsupported {
                source_stage_id: StageId::from("stage"),
                format: TournamentFormat::SingleElimination,
            })
        );
    }

    #[test]
    fn stage_qualification_validates_seed_source_and_team_uniqueness() {
        let stage = stage_qualification_finish_with_winner(
            Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap(),
            &TeamId::from("t1"),
        );
        let tournament = stage_qualification_tournament(stage);

        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    0,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 2 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::InvalidDestinationSeed { seed: 0 })
        );
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 2 },
                    1,
                ),
            ]),
            Err(StageQualificationBlocker::DuplicateDestinationSeed { seed: 1 })
        );
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::DuplicateSource {
                source_stage_id: StageId::from("stage"),
                source: StageQualificationSource::UniqueStandingRank { rank: 1 },
            })
        );
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::FinishedStageWinner,
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::DuplicateTeam {
                team_id: TeamId::from("t1"),
            })
        );
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 0 },
                    1,
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 2 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::InvalidStandingRank {
                source_stage_id: StageId::from("stage"),
                rank: 0,
            })
        );
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    1,
                ),
            ]),
            Err(StageQualificationBlocker::TooFewSlots { count: 1 })
        );
    }

    #[test]
    fn stage_qualification_serialization_is_camel_case_and_source_enum_is_closed() {
        let slot = stage_qualification_slot(
            "qualifier",
            StageQualificationSource::UniqueStandingRank { rank: 2 },
            7,
        );
        let expected = serde_json::json!({
            "sourceStageId": "qualifier",
            "source": {"type": "uniqueStandingRank", "rank": 2},
            "destinationSeed": 7
        });
        assert_eq!(serde_json::to_value(&slot).unwrap(), expected);
        assert_eq!(
            serde_json::from_value::<StageQualificationSlot>(expected.clone()).unwrap(),
            slot
        );
        assert_eq!(
            serde_json::to_value(StageQualificationSource::FinishedStageWinner).unwrap(),
            serde_json::json!({"type": "finishedStageWinner"})
        );
        assert!(serde_json::from_value::<StageQualificationSlot>(serde_json::json!({
            "sourceStageId": "qualifier",
            "source": {"type": "placement", "rank": 2},
            "destinationSeed": 7
        }))
        .is_err());
        assert!(serde_json::from_value::<StageQualificationSlot>(serde_json::json!({
            "sourceStageId": "qualifier",
            "source": {"type": "uniqueStandingRank", "rank": 2, "fallback": true},
            "destinationSeed": 7
        }))
        .is_err());
        assert!(serde_json::from_value::<StageQualificationSlot>(serde_json::json!({
            "sourceStageId": "qualifier",
            "source": {"type": "finishedStageWinner"},
            "destinationSeed": 7,
            "defaultRank": 1
        }))
        .is_err());
        assert_eq!(
            serde_json::to_value(StageQualificationBlocker::StandingRankNotFound {
                source_stage_id: StageId::from("qualifier"),
                rank: 2,
            })
            .unwrap(),
            serde_json::json!({
                "type": "standingRankNotFound",
                "sourceStageId": "qualifier",
                "rank": 2
            })
        );
    }

    fn qualified_stage_plan_template(
        stage_id: &str,
        format: TournamentFormat,
        source_slots: Vec<StageQualificationSlot>,
    ) -> QualifiedStageTemplate {
        let is_elimination = matches!(
            format,
            TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination
        );
        let (swiss_rounds, swiss_policy) = if format == TournamentFormat::Swiss {
            (
                Some(1),
                Some(swiss_policy(DrawCredit::new(1, 2).unwrap())),
            )
        } else {
            (None, None)
        };
        QualifiedStageTemplate {
            stage_id: StageId::from(stage_id),
            format,
            random_seed: 19,
            match_format: if is_elimination {
                MatchFormat::FirstToWins(1)
            } else {
                MatchFormat::FixedGames(1)
            },
            clock_config: ClockConfig::default(),
            swiss_rounds,
            swiss_policy,
            source_slots,
        }
    }

    fn qualified_stage_plan_winner_slots() -> Vec<StageQualificationSlot> {
        vec![
            stage_qualification_slot(
                "stage-a",
                StageQualificationSource::FinishedStageWinner,
                7,
            ),
            stage_qualification_slot(
                "stage-b",
                StageQualificationSource::FinishedStageWinner,
                2,
            ),
        ]
    }

    fn assert_qualified_stage_plan_error_unchanged(
        tournament: &Tournament,
        template: QualifiedStageTemplate,
        expected: QualifiedStagePlanError,
    ) {
        let before = serde_json::to_value(tournament).unwrap();
        assert_eq!(
            tournament.plan_qualified_stage(template).err(),
            Some(expected)
        );
        assert_eq!(serde_json::to_value(tournament).unwrap(), before);
    }

    fn assert_qualified_stage_apply_error_unchanged(
        tournament: &mut Tournament,
        plan: QualifiedStagePlan,
        expected: QualifiedStagePlanError,
    ) {
        let before = serde_json::to_value(&*tournament).unwrap();
        assert_eq!(tournament.apply_qualified_stage(plan), Err(expected));
        assert_eq!(serde_json::to_value(&*tournament).unwrap(), before);
    }

    #[test]
    fn qualified_stage_plan_preview_is_pure_and_resolves_rank_sources() {
        let mut source_config = config(TournamentFormat::RoundRobin, 3);
        source_config.stage_id = StageId::from("ranked-source");
        let mut source = Stage::new(source_config).unwrap();
        while source.status() != StageStatus::Finished {
            let Some(match_) = source
                .rounds()
                .iter()
                .flat_map(|round| &round.matches)
                .find(|match_| match_.status == MatchStatus::Pending)
                .cloned()
            else {
                break;
            };
            let priority = |team_id: &TeamId| match team_id.0.as_str() {
                "t1" => 0,
                "t2" => 1,
                _ => 2,
            };
            let (winner, loser) = if priority(&match_.participants[0])
                < priority(&match_.participants[1])
            {
                (match_.participants[0].clone(), match_.participants[1].clone())
            } else {
                (match_.participants[1].clone(), match_.participants[0].clone())
            };
            source
                .submit_result(&match_.match_id, MatchResult::win(winner, loser))
                .unwrap();
        }
        let tournament = Tournament {
            tournament_id: "tour".to_string(),
            stages: vec![source],
        };
        let template = qualified_stage_plan_template(
            "ranked-next",
            TournamentFormat::RoundRobin,
            vec![
                stage_qualification_slot(
                    "ranked-source",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    8,
                ),
                stage_qualification_slot(
                    "ranked-source",
                    StageQualificationSource::UniqueStandingRank { rank: 2 },
                    4,
                ),
            ],
        );
        let before = serde_json::to_value(&tournament).unwrap();
        let template_json = serde_json::to_value(&template).unwrap();
        assert!(template_json.get("sourceSlots").is_some());
        assert!(template_json.get("randomSeed").is_some());
        assert!(template_json.get("source_slots").is_none());
        assert!(template_json.get("teams").is_none());
        let mut forged_template = template_json;
        forged_template
            .as_object_mut()
            .unwrap()
            .insert("teams".to_string(), serde_json::json!([]));
        assert!(serde_json::from_value::<QualifiedStageTemplate>(forged_template).is_err());

        let plan = tournament.plan_qualified_stage(template).unwrap();
        assert_eq!(
            plan.qualification().seeded_teams(),
            &[
                SeededTeam {
                    team_id: TeamId::from("t1"),
                    seed: 8,
                },
                SeededTeam {
                    team_id: TeamId::from("t2"),
                    seed: 4,
                },
            ]
        );
        assert_eq!(plan.planned_stage().config().stage_id, StageId::from("ranked-next"));
        assert_eq!(plan.planned_stage().status(), StageStatus::Active);
        assert_eq!(tournament.stages().len(), 1);
        assert_eq!(serde_json::to_value(&tournament).unwrap(), before);
    }

    #[test]
    fn qualified_stage_plan_applies_resolved_seeds_with_existing_format_policies() {
        let expected_qualification = serde_json::json!({
            "seededTeams": [
                {"teamId": "a1", "seed": 7},
                {"teamId": "b2", "seed": 2}
            ],
            "sourceMappings": [
                {
                    "sourceStageId": "stage-a",
                    "source": {"type": "finishedStageWinner"},
                    "teamId": "a1",
                    "destinationSeed": 7
                },
                {
                    "sourceStageId": "stage-b",
                    "source": {"type": "finishedStageWinner"},
                    "teamId": "b2",
                    "destinationSeed": 2
                }
            ]
        });
        for format in [
            TournamentFormat::SingleElimination,
            TournamentFormat::DoubleElimination,
            TournamentFormat::RoundRobin,
            TournamentFormat::Swiss,
        ] {
            let mut tournament = stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);
            let template = qualified_stage_plan_template(
                "next",
                format,
                qualified_stage_plan_winner_slots(),
            );
            let plan = tournament.plan_qualified_stage(template).unwrap();
            let applied = tournament.apply_qualified_stage(plan).unwrap();
            assert_eq!(serde_json::to_value(&applied).unwrap(), expected_qualification);

            let stage = tournament.stage(&StageId::from("next")).unwrap();
            assert_eq!(stage.config().format, format);
            assert_eq!(stage.config().random_seed, 19);
            assert_eq!(stage.config().teams.as_slice(), applied.seeded_teams());
            assert_eq!(
                stage.config().match_format,
                if matches!(
                    format,
                    TournamentFormat::SingleElimination | TournamentFormat::DoubleElimination
                ) {
                    MatchFormat::FirstToWins(1)
                } else {
                    MatchFormat::FixedGames(1)
                }
            );
            assert_eq!(
                stage.config().swiss_rounds,
                (format == TournamentFormat::Swiss).then_some(1_u32)
            );
            assert_eq!(stage.config().swiss_policy.is_some(), format == TournamentFormat::Swiss);
            assert_eq!(stage.status(), StageStatus::Active);
            let matches: Vec<_> = stage
                .rounds()
                .iter()
                .flat_map(|round| &round.matches)
                .collect();
            assert!(!matches.is_empty());
            assert!(matches.iter().all(|match_| {
                match_.status == MatchStatus::Pending && !match_.started
            }));
            assert_eq!(tournament.stages().len(), 3);
        }
    }

    #[test]
    fn qualified_stage_plan_rejects_invalid_seed_and_template_without_mutation() {
        let tournament = stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);

        let mut zero_seed = qualified_stage_plan_winner_slots();
        zero_seed[0].destination_seed = 0;
        assert_qualified_stage_plan_error_unchanged(
            &tournament,
            qualified_stage_plan_template("next", TournamentFormat::RoundRobin, zero_seed),
            QualifiedStagePlanError::QualificationBlocked(
                StageQualificationBlocker::InvalidDestinationSeed { seed: 0 },
            ),
        );

        let mut duplicate_seed = qualified_stage_plan_winner_slots();
        duplicate_seed[1].destination_seed = duplicate_seed[0].destination_seed;
        assert_qualified_stage_plan_error_unchanged(
            &tournament,
            qualified_stage_plan_template("next", TournamentFormat::RoundRobin, duplicate_seed),
            QualifiedStagePlanError::QualificationBlocked(
                StageQualificationBlocker::DuplicateDestinationSeed { seed: 7 },
            ),
        );

        let duplicate_team = vec![
            stage_qualification_slot(
                "stage-a",
                StageQualificationSource::FinishedStageWinner,
                1,
            ),
            stage_qualification_slot(
                "stage-a",
                StageQualificationSource::UniqueStandingRank { rank: 1 },
                2,
            ),
        ];
        assert_qualified_stage_plan_error_unchanged(
            &tournament,
            qualified_stage_plan_template("next", TournamentFormat::RoundRobin, duplicate_team),
            QualifiedStagePlanError::QualificationBlocked(
                StageQualificationBlocker::DuplicateTeam {
                    team_id: TeamId::from("a1"),
                },
            ),
        );

        let mut invalid_template = qualified_stage_plan_template(
            "next",
            TournamentFormat::RoundRobin,
            qualified_stage_plan_winner_slots(),
        );
        invalid_template.match_format = MatchFormat::FirstToWins(1);
        assert_qualified_stage_plan_error_unchanged(
            &tournament,
            invalid_template,
            QualifiedStagePlanError::InvalidTemplate(ScheduleError::InvalidMatchFormat(
                "循環或 Swiss 只接受 fixedGames".to_string(),
            )),
        );
    }

    #[test]
    fn qualified_stage_plan_blocks_unfinished_tied_and_dropped_sources_without_mutation() {
        let mut unfinished = stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);
        let mut active_config = config(TournamentFormat::RoundRobin, 2);
        active_config.stage_id = StageId::from("stage-a");
        let active_source = Stage::new(active_config).unwrap();
        unfinished.stages[0] = active_source;
        assert_qualified_stage_plan_error_unchanged(
            &unfinished,
            qualified_stage_plan_template(
                "next",
                TournamentFormat::RoundRobin,
                qualified_stage_plan_winner_slots(),
            ),
            QualifiedStagePlanError::QualificationBlocked(
                StageQualificationBlocker::SourceStageNotFinished {
                    source_stage_id: StageId::from("stage-a"),
                },
            ),
        );

        let tied_stage = stage_qualification_round_robin_tie();
        let second_stage = stage_qualification_finish_with_winner(
            stage_qualification_new_stage(TournamentFormat::RoundRobin, "stage-b", "b"),
            &TeamId::from("b1"),
        );
        let tied_tournament = Tournament {
            tournament_id: "tour".to_string(),
            stages: vec![tied_stage, second_stage],
        };
        assert_qualified_stage_plan_error_unchanged(
            &tied_tournament,
            qualified_stage_plan_template(
                "next",
                TournamentFormat::RoundRobin,
                vec![
                    stage_qualification_slot(
                        "stage",
                        StageQualificationSource::UniqueStandingRank { rank: 1 },
                        1,
                    ),
                    stage_qualification_slot(
                        "stage-b",
                        StageQualificationSource::FinishedStageWinner,
                        2,
                    ),
                ],
            ),
            QualifiedStagePlanError::QualificationBlocked(
                StageQualificationBlocker::StandingRankNotUnique {
                    source_stage_id: StageId::from("stage"),
                    rank: 1,
                    team_ids: vec![TeamId::from("t1"), TeamId::from("t2")],
                },
            ),
        );

        let mut dropped_stage = stage_qualification_finish_with_winner(
            Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap(),
            &TeamId::from("t1"),
        );
        dropped_stage.drop_team(&TeamId::from("t1")).unwrap();
        let dropped_tournament = stage_qualification_tournament(dropped_stage);
        assert_qualified_stage_plan_error_unchanged(
            &dropped_tournament,
            qualified_stage_plan_template(
                "next",
                TournamentFormat::RoundRobin,
                vec![
                    stage_qualification_slot(
                        "stage",
                        StageQualificationSource::UniqueStandingRank { rank: 1 },
                        1,
                    ),
                    stage_qualification_slot(
                        "stage",
                        StageQualificationSource::UniqueStandingRank { rank: 2 },
                        2,
                    ),
                ],
            ),
            QualifiedStagePlanError::QualificationBlocked(
                StageQualificationBlocker::DroppedTeam {
                    source_stage_id: StageId::from("stage"),
                    team_id: TeamId::from("t1"),
                },
            ),
        );
    }

    #[test]
    fn qualified_stage_plan_rejects_duplicate_target_and_foreign_tournament_without_mutation() {
        let mut tournament = stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);
        let template = qualified_stage_plan_template(
            "next",
            TournamentFormat::RoundRobin,
            qualified_stage_plan_winner_slots(),
        );
        let plan = tournament.plan_qualified_stage(template.clone()).unwrap();
        let mut intervening_config = config(TournamentFormat::RoundRobin, 2);
        intervening_config.stage_id = StageId::from("next");
        let intervening_stage = Stage::new(intervening_config).unwrap();
        tournament.stages.push(intervening_stage);
        assert_qualified_stage_apply_error_unchanged(
            &mut tournament,
            plan,
            QualifiedStagePlanError::DuplicateStageId(StageId::from("next")),
        );

        let source_tournament =
            stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);
        let foreign_plan = source_tournament
            .plan_qualified_stage(template)
            .unwrap();
        let mut foreign_tournament = Tournament {
            tournament_id: "foreign".to_string(),
            stages: source_tournament.stages.clone(),
        };
        assert_qualified_stage_apply_error_unchanged(
            &mut foreign_tournament,
            foreign_plan,
            QualifiedStagePlanError::ForeignTournament {
                planned_tournament_id: "tour".to_string(),
                current_tournament_id: "foreign".to_string(),
            },
        );
    }

    #[test]
    fn qualified_stage_plan_rejects_changed_winner_without_mutation() {
        let mut tournament =
            stage_qualification_two_stage_winners(TournamentFormat::SingleElimination);
        let plan = tournament
            .plan_qualified_stage(qualified_stage_plan_template(
                "next",
                TournamentFormat::RoundRobin,
                qualified_stage_plan_winner_slots(),
            ))
            .unwrap();
        let source_match = tournament
            .stage(&StageId::from("stage-a"))
            .unwrap()
            .rounds()[0]
            .matches[0]
            .match_id
            .clone();
        tournament
            .correct_result(
                &StageId::from("stage-a"),
                &source_match,
                MatchResult::win("a2", "a1"),
                "更正勝者".to_string(),
            )
            .unwrap();
        assert_eq!(
            tournament
                .stage(&StageId::from("stage-a"))
                .unwrap()
                .winner(),
            Some(TeamId::from("a2"))
        );
        assert_qualified_stage_apply_error_unchanged(
            &mut tournament,
            plan,
            QualifiedStagePlanError::SourceChanged {
                source_stage_id: StageId::from("stage-a"),
            },
        );
    }

    #[test]
    fn qualified_stage_plan_rejects_same_winner_correction_without_mutation() {
        let mut source_config = config(TournamentFormat::RoundRobin, 2);
        source_config.stage_id = StageId::from("stage-a");
        source_config.teams[0].team_id = TeamId::from("a1");
        source_config.teams[1].team_id = TeamId::from("a2");
        source_config.match_format = MatchFormat::FixedGames(3);
        let mut corrected_source = Stage::new(source_config).unwrap();
        let source_match = corrected_source.rounds()[0].matches[0].clone();
        let original_winner = source_match.participants[0].clone();
        let original_loser = source_match.participants[1].clone();
        corrected_source
            .submit_result(
                &source_match.match_id,
                MatchResult::win_with_games(
                    original_winner.clone(),
                    original_loser.clone(),
                    2,
                    1,
                ),
            )
            .unwrap();
        let other_source = stage_qualification_finish_with_winner(
            stage_qualification_new_stage(TournamentFormat::RoundRobin, "stage-b", "b"),
            &TeamId::from("b2"),
        );
        let mut tournament = Tournament {
            tournament_id: "tour".to_string(),
            stages: vec![corrected_source, other_source],
        };
        let plan = tournament
            .plan_qualified_stage(qualified_stage_plan_template(
                "next",
                TournamentFormat::RoundRobin,
                qualified_stage_plan_winner_slots(),
            ))
            .unwrap();

        tournament
            .correct_result(
                &StageId::from("stage-a"),
                &source_match.match_id,
                MatchResult::forfeit(original_winner.clone(), original_loser),
                "更正為棄權勝".to_string(),
            )
            .unwrap();
        assert_eq!(
            tournament
                .stage(&StageId::from("stage-a"))
                .unwrap()
                .winner(),
            Some(original_winner)
        );
        assert_qualified_stage_apply_error_unchanged(
            &mut tournament,
            plan,
            QualifiedStagePlanError::SourceChanged {
                source_stage_id: StageId::from("stage-a"),
            },
        );
    }

    #[test]
    fn qualified_stage_plan_rejects_stale_internal_plan_without_mutation() {
        let mut tournament =
            stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);
        let mut plan = tournament
            .plan_qualified_stage(qualified_stage_plan_template(
                "next",
                TournamentFormat::RoundRobin,
                qualified_stage_plan_winner_slots(),
            ))
            .unwrap();
        plan.template.source_slots[0].destination_seed = 11;
        assert_qualified_stage_apply_error_unchanged(
            &mut tournament,
            plan,
            QualifiedStagePlanError::StalePlan,
        );
    }
}
