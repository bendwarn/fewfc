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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub enum MatchPurpose {
    #[default]
    Standard,
    Bronze,
}

impl MatchPurpose {
    fn is_standard(&self) -> bool {
        *self == Self::Standard
    }
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
    #[serde(default, skip_serializing_if = "MatchPurpose::is_standard")]
    pub purpose: MatchPurpose,
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
pub fn validate_competition_announcement_plan(
    plan: &CompetitionAnnouncementPlan,
) -> Result<(), String> {
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
        stage
            .match_format
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
                    if slot.destination_seed == 0
                        || !destination_seeds.insert(slot.destination_seed)
                    {
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
                            if matches!(
                                source_stage.format,
                                TournamentFormat::SingleElimination
                                    | TournamentFormat::DoubleElimination
                            ) => {}
                        StageQualificationSource::UniqueStandingRank { rank }
                            if *rank > 0
                                && matches!(
                                    source_stage.format,
                                    TournamentFormat::RoundRobin | TournamentFormat::Swiss
                                ) => {}
                        _ => {
                            return Err(
                                "晉級來源必須符合來源階段可提供的勝者或唯一名次".to_string()
                            );
                        }
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
    if !config
        .action_time_ms
        .is_some_and(|value| BYOYOMI_OPTIONS_MS.contains(&value))
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
    /// 累計分數以字串傳輸，避免 JavaScript 整數精度截斷；還原快照仍接受舊數值。
    #[serde(with = "u64_decimal_string")]
    pub match_points: u64,
    pub wins: u32,
    pub draws: u32,
    pub losses: u32,
    pub game_wins: u32,
    pub game_losses: u32,
    pub net_game_wins: i32,
    pub opponent_win_rate_basis_points: Option<u32>,
    /// 本階段不同對手的最終交手勝場總和，不是自身勝場或局勝場。
    #[serde(default, with = "u64_decimal_string")]
    pub opponent_win_score: u64,
    /// 本階段不同對手的最終交手敗場總和，不是自身敗場或扣分。
    #[serde(default, with = "u64_decimal_string")]
    pub opponent_loss_score: u64,
    pub dropped: bool,
    pub rank: Option<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "camelCase",
    rename_all_fields = "camelCase",
    tag = "type"
)]
pub enum RankingDecisionActor {
    Organizer { user_id: String },
    Referee { user_id: String },
}

/// 全項相同的同分群組由合格裁決者指定順序；保存比較值以避免更正後重用舊裁決。
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RankingTieDecision {
    pub tied_team_ids: Vec<TeamId>,
    pub ordered_team_ids: Vec<TeamId>,
    #[serde(with = "u64_decimal_string")]
    pub match_points: u64,
    #[serde(with = "u64_decimal_string")]
    pub opponent_win_score: u64,
    #[serde(with = "u64_decimal_string")]
    pub opponent_loss_score: u64,
    pub decided_by: RankingDecisionActor,
    pub reason: String,
}

mod u64_decimal_string {
    use serde::de::{self, Visitor};
    use serde::{Deserializer, Serializer};
    use std::fmt;

    pub fn serialize<S>(value: &u64, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<u64, D::Error>
    where
        D: Deserializer<'de>,
    {
        struct U64Visitor;

        impl Visitor<'_> for U64Visitor {
            type Value = u64;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a non-negative integer or canonical decimal string")
            }

            fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                Ok(value)
            }

            fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                u64::try_from(value).map_err(E::custom)
            }

            fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
            where
                E: de::Error,
            {
                if value.is_empty()
                    || (value.len() > 1 && value.starts_with('0'))
                    || !value.bytes().all(|byte| byte.is_ascii_digit())
                {
                    return Err(E::custom("matchPoints must be a canonical decimal string"));
                }
                value.parse().map_err(E::custom)
            }
        }

        deserializer.deserialize_any(U64Visitor)
    }
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
    #[serde(default)]
    ranking_tie_decisions: Vec<RankingTieDecision>,
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
    RankingTieStageNotFinished(StageId),
    RankingTieUnsupportedFormat(StageId),
    RankingTieNotFound(StageId),
    RankingTieOrderMismatch(StageId),
    RankingTieAlreadyDecided(StageId),
    EmptyRankingDecisionReason,
    InvalidRankingDecisionActor,
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
            Self::RankingTieStageNotFinished(stage) => {
                write!(f, "階段尚未完成，不可裁決同分名次：{stage}")
            }
            Self::RankingTieUnsupportedFormat(stage) => {
                write!(f, "此階段不使用積分同分裁決：{stage}")
            }
            Self::RankingTieNotFound(stage) => {
                write!(f, "指定隊伍目前不構成全項相同的同分群組：{stage}")
            }
            Self::RankingTieOrderMismatch(stage) => {
                write!(f, "裁決順序必須恰好列出同分群組的每一隊：{stage}")
            }
            Self::RankingTieAlreadyDecided(stage) => {
                write!(f, "此同分群組已有有效裁決：{stage}")
            }
            Self::EmptyRankingDecisionReason => write!(f, "同分裁決必須填寫公開理由"),
            Self::InvalidRankingDecisionActor => write!(f, "同分裁決者身分無效"),
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

    /// 只可由主辦或指定裁判處理已完成循環／瑞士階段的全同群組。
    pub fn adjudicate_ranking_tie(
        &mut self,
        stage_id: &StageId,
        ordered_team_ids: Vec<TeamId>,
        decided_by: RankingDecisionActor,
        reason: String,
    ) -> Result<(), ScheduleError> {
        let mut candidate = self.clone();
        candidate.refresh_rankings();
        let stage = candidate
            .stages
            .iter_mut()
            .find(|stage| &stage.config.stage_id == stage_id)
            .ok_or_else(|| ScheduleError::NoSuchStage(stage_id.clone()))?;
        stage.adjudicate_ranking_tie(ordered_team_ids, decided_by, reason)?;
        *self = candidate;
        Ok(())
    }

    /// 整場只有在所有階段收尾且沒有未決正式同分時才具備排名完成條件。
    pub fn rankings_resolved(&self) -> bool {
        let mut projection = self.clone();
        projection.refresh_rankings();
        !projection.stages.is_empty()
            && projection.stages.iter().all(|stage| {
                stage.status == StageStatus::Finished
                    && stage.unresolved_ranking_tie_groups().is_empty()
            })
    }

    /// 由階段賽果重算排名供公開投影使用，不改動權威快照。
    pub fn ranking_projection(&self) -> Self {
        let mut projection = self.clone();
        projection.refresh_rankings();
        projection
    }

    fn refresh_rankings(&mut self) {
        for stage in &mut self.stages {
            stage.refresh_standings();
        }
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
        let projection = self.ranking_projection();
        projection.resolve_stage_qualification_current(slots)
    }

    fn resolve_stage_qualification_current(
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
                    let ranked_teams: Vec<_> = stage
                        .standings
                        .iter()
                        .filter(|standing| standing.rank == Some(*rank))
                        .collect();
                    let standing = match ranked_teams.as_slice() {
                        [] => {
                            if let Some(team_id) = stage.unresolved_ranking_tie_at_rank(*rank) {
                                return Err(StageQualificationBlocker::StandingRankUnavailable {
                                    source_stage_id: slot.source_stage_id.clone(),
                                    team_id,
                                });
                            }
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
        let projection = self.ranking_projection();
        if projection
            .stages
            .iter()
            .any(|stage| stage.config.stage_id == template.stage_id)
        {
            return Err(QualifiedStagePlanError::DuplicateStageId(template.stage_id));
        }

        let qualification = projection
            .resolve_stage_qualification_current(&template.source_slots)
            .map_err(QualifiedStagePlanError::QualificationBlocked)?;
        let planned_stage = Stage::new(template.stage_config(qualification.seeded_teams.clone()))
            .map_err(QualifiedStagePlanError::InvalidTemplate)?;

        let mut seen_source_ids = BTreeSet::new();
        let mut source_stages = Vec::new();
        for slot in &template.source_slots {
            if !seen_source_ids.insert(slot.source_stage_id.clone()) {
                continue;
            }
            let source_stage = projection
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
            tournament_id: projection.tournament_id.clone(),
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
        let mut candidate = self.ranking_projection();
        if candidate.tournament_id != plan.tournament_id {
            return Err(QualifiedStagePlanError::ForeignTournament {
                planned_tournament_id: plan.tournament_id,
                current_tournament_id: candidate.tournament_id.clone(),
            });
        }
        if candidate
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
            let current_source = candidate
                .stages
                .iter()
                .find(|stage| &stage.config.stage_id == source_stage_id);
            if current_source != Some(source_snapshot) {
                return Err(QualifiedStagePlanError::SourceChanged {
                    source_stage_id: source_stage_id.clone(),
                });
            }
        }

        let current_qualification = candidate
            .resolve_stage_qualification_current(&plan.template.source_slots)
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
        candidate.stages.push(plan.planned_stage);
        *self = candidate;
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
        .find(|match_| {
            match_
                .result
                .as_ref()
                .is_some_and(MatchResult::is_double_loss)
        })
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
    #[serde(default)]
    ranking_tie_decisions: Vec<RankingTieDecision>,
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
            ranking_tie_decisions: snapshot.ranking_tie_decisions,
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
            ranking_tie_decisions: vec![],
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

    pub fn ranking_tie_decisions(&self) -> &[RankingTieDecision] {
        &self.ranking_tie_decisions
    }

    /// 傳回尚未由有效公開裁決解開的正式全同群組。
    pub fn unresolved_ranking_tie_groups(&self) -> Vec<Vec<TeamId>> {
        if !matches!(
            self.config.format,
            TournamentFormat::RoundRobin | TournamentFormat::Swiss
        ) {
            return Vec::new();
        }
        let mut groups: BTreeMap<(u64, u64, u64), Vec<TeamId>> = BTreeMap::new();
        for standing in &self.standings {
            groups
                .entry((
                    standing.match_points,
                    standing.opponent_win_score,
                    standing.opponent_loss_score,
                ))
                .or_default()
                .push(standing.team_id.clone());
        }
        groups
            .into_iter()
            .filter_map(
                |((match_points, opponent_win_score, opponent_loss_score), mut team_ids)| {
                    if team_ids.len() < 2 {
                        return None;
                    }
                    team_ids.sort();
                    let active = self.ranking_tie_decisions.iter().any(|decision| {
                        decision.tied_team_ids == team_ids
                            && decision.match_points == match_points
                            && decision.opponent_win_score == opponent_win_score
                            && decision.opponent_loss_score == opponent_loss_score
                    });
                    (!active).then_some(team_ids)
                },
            )
            .collect()
    }

    fn unresolved_ranking_tie_at_rank(&self, rank: u32) -> Option<TeamId> {
        if !matches!(
            self.config.format,
            TournamentFormat::RoundRobin | TournamentFormat::Swiss
        ) {
            return None;
        }
        let requested_rank = usize::try_from(rank).ok()?;
        if requested_rank == 0 {
            return None;
        }

        let ordered = self.standing_order_for_rank(&self.standings);
        let mut start = 0;
        while start < ordered.len() {
            let mut end = start + 1;
            while end < ordered.len()
                && self.compare_standing_rank_keys(&ordered[start], &ordered[end], &self.standings)
                    == Ordering::Equal
            {
                end += 1;
            }
            if end - start > 1 && requested_rank > start && requested_rank <= end {
                return ordered[start..end].iter().min().cloned();
            }
            start = end;
        }
        None
    }

    fn adjudicate_ranking_tie(
        &mut self,
        ordered_team_ids: Vec<TeamId>,
        decided_by: RankingDecisionActor,
        reason: String,
    ) -> Result<(), ScheduleError> {
        let stage_id = self.config.stage_id.clone();
        if self.status != StageStatus::Finished {
            return Err(ScheduleError::RankingTieStageNotFinished(stage_id));
        }
        if !matches!(
            self.config.format,
            TournamentFormat::RoundRobin | TournamentFormat::Swiss
        ) {
            return Err(ScheduleError::RankingTieUnsupportedFormat(stage_id));
        }
        let reason = reason.trim().to_string();
        if reason.is_empty() {
            return Err(ScheduleError::EmptyRankingDecisionReason);
        }
        let adjudicator_id = match &decided_by {
            RankingDecisionActor::Organizer { user_id }
            | RankingDecisionActor::Referee { user_id } => user_id,
        };
        if adjudicator_id.trim().is_empty() || adjudicator_id.trim() != adjudicator_id {
            return Err(ScheduleError::InvalidRankingDecisionActor);
        }
        if ordered_team_ids.len() < 2
            || ordered_team_ids.iter().collect::<BTreeSet<_>>().len() != ordered_team_ids.len()
        {
            return Err(ScheduleError::RankingTieOrderMismatch(stage_id));
        }
        let ordered_set = ordered_team_ids.iter().cloned().collect::<BTreeSet<_>>();
        let first = ordered_team_ids.first().unwrap();
        let Some(first_standing) = self
            .standings
            .iter()
            .find(|standing| &standing.team_id == first)
        else {
            return Err(ScheduleError::RankingTieNotFound(stage_id));
        };
        let mut tied_team_ids = self
            .standings
            .iter()
            .filter(|standing| {
                standing.match_points == first_standing.match_points
                    && standing.opponent_win_score == first_standing.opponent_win_score
                    && standing.opponent_loss_score == first_standing.opponent_loss_score
            })
            .map(|standing| standing.team_id.clone())
            .collect::<Vec<_>>();
        tied_team_ids.sort();
        if tied_team_ids.len() < 2 {
            return Err(ScheduleError::RankingTieNotFound(stage_id));
        }
        let tied_set = tied_team_ids.iter().cloned().collect::<BTreeSet<_>>();
        if tied_set != ordered_set {
            return Err(ScheduleError::RankingTieOrderMismatch(stage_id));
        }
        if self.ranking_tie_decisions.iter().any(|decision| {
            decision.tied_team_ids == tied_team_ids
                && decision.match_points == first_standing.match_points
                && decision.opponent_win_score == first_standing.opponent_win_score
                && decision.opponent_loss_score == first_standing.opponent_loss_score
        }) {
            return Err(ScheduleError::RankingTieAlreadyDecided(stage_id));
        }
        self.ranking_tie_decisions.push(RankingTieDecision {
            tied_team_ids,
            ordered_team_ids,
            match_points: first_standing.match_points,
            opponent_win_score: first_standing.opponent_win_score,
            opponent_loss_score: first_standing.opponent_loss_score,
            decided_by,
            reason,
        });
        self.refresh_standings();
        Ok(())
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
                if match_.purpose == MatchPurpose::Bronze
                    && (self.config.format != TournamentFormat::SingleElimination
                        || match_.kind != RoundKind::Main
                        || match_.participants.len() != 2)
                {
                    return Err(ScheduleError::InvalidSnapshot(
                        "季軍賽只能是單敗 Main round 的兩隊交手".to_string(),
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
            if round
                .matches
                .iter()
                .any(|match_| match_.purpose == MatchPurpose::Bronze)
            {
                let bronze_matches = round
                    .matches
                    .iter()
                    .filter(|match_| match_.purpose == MatchPurpose::Bronze)
                    .count();
                let standard_matches: Vec<_> = round
                    .matches
                    .iter()
                    .filter(|match_| match_.purpose == MatchPurpose::Standard)
                    .collect();
                if bronze_matches != 1
                    || round.matches.len() != 2
                    || standard_matches.len() != 1
                    || standard_matches[0].participants.len() != 2
                    || !self
                        .rounds
                        .last()
                        .is_some_and(|latest| latest.round_number == round.round_number)
                {
                    return Err(ScheduleError::InvalidSnapshot(
                        "季軍賽必須與唯一兩隊冠軍賽同列於最新輪次".to_string(),
                    ));
                }
            }
        }
        let mut decision_keys = BTreeSet::new();
        for decision in &self.ranking_tie_decisions {
            let adjudicator_id = match &decision.decided_by {
                RankingDecisionActor::Organizer { user_id }
                | RankingDecisionActor::Referee { user_id } => user_id,
            };
            let tied_set = decision
                .tied_team_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let ordered_set = decision
                .ordered_team_ids
                .iter()
                .cloned()
                .collect::<BTreeSet<_>>();
            let mut sorted_tied_ids = decision.tied_team_ids.clone();
            sorted_tied_ids.sort();
            if decision.tied_team_ids.len() < 2
                || tied_set.len() != decision.tied_team_ids.len()
                || decision.ordered_team_ids.len() != decision.tied_team_ids.len()
                || ordered_set != tied_set
                || decision.tied_team_ids != sorted_tied_ids
                || !tied_set
                    .iter()
                    .all(|team_id| config_teams.contains(team_id))
                || adjudicator_id.trim().is_empty()
                || adjudicator_id.trim() != adjudicator_id
                || decision.reason.trim().is_empty()
                || !decision_keys.insert((
                    decision.tied_team_ids.clone(),
                    decision.match_points,
                    decision.opponent_win_score,
                    decision.opponent_loss_score,
                ))
            {
                return Err(ScheduleError::InvalidSnapshot(
                    "同分裁決紀錄格式無效".to_string(),
                ));
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
                    && match_
                        .result
                        .as_ref()
                        .is_some_and(MatchResult::is_double_loss)
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
                        double_losses
                            .iter()
                            .find(|(round_number, participants, _)| {
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
        if self.config.format != TournamentFormat::RoundRobin || self.status != StageStatus::Active
        {
            return;
        }
        let expected_reason = Self::round_robin_double_loss_cancellation_reason(source_match_id);
        let still_exited: BTreeSet<_> = self
            .rounds
            .iter()
            .flat_map(|round| &round.matches)
            .filter(|match_| {
                match_.status == MatchStatus::Completed
                    && match_
                        .result
                        .as_ref()
                        .is_some_and(MatchResult::is_double_loss)
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
        let standard_matches: Vec<_> = final_round
            .matches
            .iter()
            .filter(|match_| match_.purpose == MatchPurpose::Standard)
            .collect();
        let [final_match] = standard_matches.as_slice() else {
            return false;
        };
        self.config.format == TournamentFormat::SingleElimination
            && final_round.kind == RoundKind::Main
            && final_match.kind == RoundKind::Main
            && final_match.participants.len() == 2
            && final_match.status == MatchStatus::Completed
            && final_match
                .result
                .as_ref()
                .is_some_and(MatchResult::is_double_loss)
            && final_round.matches.iter().all(is_closed)
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
            if let Some(match_) =
                self.rounds
                    .iter()
                    .flat_map(|round| &round.matches)
                    .find(|match_| {
                        match_
                            .result
                            .as_ref()
                            .is_some_and(MatchResult::is_double_loss)
                    })
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
        if let Some(double_loss) = round.matches.iter().find(|match_| {
            match_.purpose == MatchPurpose::Standard
                && match_
                    .result
                    .as_ref()
                    .is_some_and(MatchResult::is_double_loss)
        }) {
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
        let can_schedule_bronze = winners.len() == 2
            && round
                .matches
                .iter()
                .filter(|match_| match_.purpose == MatchPurpose::Standard)
                .count()
                == 2;
        let semifinal_losers = if can_schedule_bronze {
            completed_losers(round)?
        } else {
            Vec::new()
        };
        let can_schedule_bronze = can_schedule_bronze
            && semifinal_losers.len() == 2
            && semifinal_losers
                .iter()
                .all(|team_id| self.teams.get(team_id).is_some_and(|entry| !entry.dropped));
        self.append_elimination_round(RoundKind::Main, pair_adjacent_with_byes(&winners), true);
        if can_schedule_bronze {
            self.append_bronze_match(semifinal_losers);
        }
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
                    purpose: MatchPurpose::Standard,
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

    fn append_bronze_match(&mut self, participants: Vec<TeamId>) {
        let stage_id = self.config.stage_id.clone();
        let round = self.rounds.last_mut().expect("季軍賽須接在冠軍賽後");
        let sequence = round.matches.len() as u32 + 1;
        let match_id = MatchId(format!(
            "{}-g{}-r{}-m{}",
            stage_id.0, self.schedule_generation, round.round_number, sequence
        ));
        round.matches.push(ScheduledMatch {
            match_id,
            stage_id,
            round_number: round.round_number,
            sequence,
            kind: RoundKind::Main,
            purpose: MatchPurpose::Bronze,
            participants,
            status: MatchStatus::Pending,
            result: None,
            cancellation_reason: None,
            started: false,
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
        let mut active: Vec<TeamId> = ordered
            .iter()
            .filter(|team_id| self.is_swiss_team_eligible(team_id, &double_loss_exits))
            .cloned()
            .collect();
        if active.is_empty() {
            return Err(ScheduleError::NoLegalSwissPairing);
        }
        let round_seed = split_mix64(self.config.random_seed ^ (self.rounds.len() as u64 + 1));
        self.shuffle_swiss_pairing_ties(
            &mut active,
            split_mix64(round_seed ^ SWISS_TIE_ORDER_SEED_DOMAIN),
        );
        let opponents = self.opponents();
        let match_points: BTreeMap<TeamId, u64> = self
            .compute_standings()
            .into_iter()
            .map(|standing| (standing.team_id.clone(), standing.match_points))
            .collect();
        let mut pairing_seed = split_mix64(round_seed ^ SWISS_MATCHING_SEED_DOMAIN);
        if active.len() % 2 == 0 {
            let mut remaining = active;
            let (_, pairs) =
                pair_backtracking(&mut remaining, &opponents, &match_points, &mut pairing_seed)?;
            return Ok(pairs);
        }
        let mut bye_candidates: Vec<TeamId> = active
            .iter()
            .rev()
            .filter(|team_id| self.teams.get(*team_id).is_some_and(|entry| !entry.had_bye))
            .cloned()
            .collect();
        if !self.rounds.is_empty() {
            shuffle_team_ids(
                &mut bye_candidates,
                split_mix64(round_seed ^ SWISS_BYE_ORDER_SEED_DOMAIN),
            );
        }
        let mut best: Option<(usize, Vec<Vec<TeamId>>)> = None;
        for bye_team in bye_candidates {
            let mut remaining = active.clone();
            remaining.retain(|team_id| team_id != &bye_team);
            if let Ok((cross_group_pairs, next_pairs)) =
                pair_backtracking(&mut remaining, &opponents, &match_points, &mut pairing_seed)
            {
                if let Some((best_cross_group_pairs, _)) = &best {
                    if cross_group_pairs >= *best_cross_group_pairs {
                        if *best_cross_group_pairs == 0 {
                            break;
                        }
                        continue;
                    }
                }
                let mut pairs = vec![vec![bye_team]];
                pairs.extend(next_pairs);
                best = Some((cross_group_pairs, pairs));
                if cross_group_pairs == 0 {
                    break;
                }
            }
        }
        best.map(|(_, pairs)| pairs)
            .ok_or(ScheduleError::NoLegalSwissPairing)
    }

    fn shuffle_swiss_pairing_ties(&self, ordered: &mut [TeamId], seed: u64) {
        if self.rounds.is_empty() || ordered.len() < 2 {
            return;
        }
        let standings = self.compute_standings();
        let mut range_start = 0;
        let mut group_index = 0u64;
        while range_start < ordered.len() {
            let mut range_end = range_start + 1;
            while range_end < ordered.len()
                && self.compare_rank_keys(&ordered[range_start], &ordered[range_end], &standings)
                    == Ordering::Equal
            {
                range_end += 1;
            }
            if range_end - range_start > 1 {
                shuffle_team_ids(
                    &mut ordered[range_start..range_end],
                    split_mix64(seed ^ group_index),
                );
            }
            group_index += 1;
            range_start = range_end;
        }
    }

    fn is_swiss_team_eligible(
        &self,
        team_id: &TeamId,
        double_loss_exits: &BTreeSet<TeamId>,
    ) -> bool {
        self.teams.get(team_id).is_some_and(|entry| !entry.dropped)
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
                    && match_
                        .result
                        .as_ref()
                        .is_some_and(MatchResult::is_double_loss)
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

    /// 正式勝分／負分只看真正交手過的不同對手；賽前未開打缺席不建立對手關係。
    fn ranking_opponents(&self) -> BTreeMap<TeamId, BTreeSet<TeamId>> {
        let mut result: BTreeMap<TeamId, BTreeSet<TeamId>> = self
            .teams
            .keys()
            .cloned()
            .map(|team_id| (team_id, BTreeSet::new()))
            .collect();
        for match_ in self.rounds.iter().flat_map(|round| &round.matches) {
            if match_.participants.len() != 2
                || !matches!(match_.status, MatchStatus::Completed)
                || match_
                    .result
                    .as_ref()
                    .is_none_or(|result| matches!(result, MatchResult::NoShow { .. }))
            {
                continue;
            }
            let first = match_.participants[0].clone();
            let second = match_.participants[1].clone();
            result
                .entry(first.clone())
                .or_default()
                .insert(second.clone());
            result.entry(second).or_default().insert(first);
        }
        result
    }

    fn refresh_standings(&mut self) {
        self.standings = self.compute_standings();
        if self.has_single_elimination_final_double_loss() {
            // 冠軍空缺階段的其餘名次也未確認；保留真實統計，不填不可靠名次。
            return;
        }
        let ordered = self.standing_order_for_rank(&self.standings);
        let mut start = 0;
        while start < ordered.len() {
            let mut end = start + 1;
            while end < ordered.len()
                && self.compare_standing_rank_keys(&ordered[start], &ordered[end], &self.standings)
                    == Ordering::Equal
            {
                end += 1;
            }
            let unresolved_formal_tie = end - start > 1
                && matches!(
                    self.config.format,
                    TournamentFormat::RoundRobin | TournamentFormat::Swiss
                );
            for team_id in &ordered[start..end] {
                if let Some(row) = self
                    .standings
                    .iter_mut()
                    .find(|row| &row.team_id == team_id)
                {
                    // 全項相同時名次未決；不能自動並列或讓 seed／ID 決定排序。
                    row.rank = if unresolved_formal_tie {
                        None
                    } else {
                        Some(start as u32 + 1)
                    };
                }
            }
            start = end;
        }
        self.standings
            .sort_by_key(|row| row.rank.unwrap_or(u32::MAX));
    }

    fn standing_order_for_rank(&self, standings: &[Standing]) -> Vec<TeamId> {
        let mut order: Vec<TeamId> = standings.iter().map(|row| row.team_id.clone()).collect();
        order.sort_by(|left, right| self.compare_standing_rank_keys(left, right, standings));
        order
    }

    /// 正式循環／瑞士名次固定使用公告積分、不同對手勝分、不同對手負分。
    /// Swiss 配對另外沿用其公告 tie-breakers，不會因正式排名政策而改變。
    fn compare_standing_rank_keys(
        &self,
        left: &TeamId,
        right: &TeamId,
        standings: &[Standing],
    ) -> Ordering {
        if !matches!(
            self.config.format,
            TournamentFormat::RoundRobin | TournamentFormat::Swiss
        ) {
            return self.compare_rank_keys(left, right, standings);
        }
        let left_row = standings.iter().find(|row| &row.team_id == left).unwrap();
        let right_row = standings.iter().find(|row| &row.team_id == right).unwrap();
        let ordering = right_row
            .match_points
            .cmp(&left_row.match_points)
            .then_with(|| {
                right_row
                    .opponent_win_score
                    .cmp(&left_row.opponent_win_score)
            })
            .then_with(|| {
                left_row
                    .opponent_loss_score
                    .cmp(&right_row.opponent_loss_score)
            });
        if ordering != Ordering::Equal {
            return ordering;
        }
        let mut tied_team_ids = standings
            .iter()
            .filter(|row| {
                row.match_points == left_row.match_points
                    && row.opponent_win_score == left_row.opponent_win_score
                    && row.opponent_loss_score == left_row.opponent_loss_score
            })
            .map(|row| row.team_id.clone())
            .collect::<Vec<_>>();
        tied_team_ids.sort();
        let Some(decision) = self.ranking_tie_decisions.iter().find(|decision| {
            decision.tied_team_ids == tied_team_ids
                && decision.match_points == left_row.match_points
                && decision.opponent_win_score == left_row.opponent_win_score
                && decision.opponent_loss_score == left_row.opponent_loss_score
        }) else {
            return Ordering::Equal;
        };
        let left_order = decision
            .ordered_team_ids
            .iter()
            .position(|team_id| team_id == left)
            .unwrap();
        let right_order = decision
            .ordered_team_ids
            .iter()
            .position(|team_id| team_id == right)
            .unwrap();
        left_order.cmp(&right_order)
    }

    fn compare_rank_keys(&self, left: &TeamId, right: &TeamId, standings: &[Standing]) -> Ordering {
        let left_row = standings.iter().find(|row| &row.team_id == left).unwrap();
        let right_row = standings.iter().find(|row| &row.team_id == right).unwrap();
        if self.config.format == TournamentFormat::SingleElimination
            && self.status == StageStatus::Finished
        {
            match (
                self.single_elimination_placement(left),
                self.single_elimination_placement(right),
            ) {
                (Some(left_rank), Some(right_rank)) => return left_rank.cmp(&right_rank),
                (Some(_), None) => return Ordering::Less,
                (None, Some(_)) => return Ordering::Greater,
                (None, None) => {}
            }
        }
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

    fn single_elimination_placement(&self, team_id: &TeamId) -> Option<u32> {
        let round = self.rounds.last()?;
        let final_match = round
            .matches
            .iter()
            .find(|match_| match_.purpose == MatchPurpose::Standard)?;
        let bronze_match = round
            .matches
            .iter()
            .find(|match_| match_.purpose == MatchPurpose::Bronze)?;
        let final_result = final_match.result.as_ref()?;
        let bronze_result = bronze_match.result.as_ref()?;
        let champion = final_result.winner()?;
        let finalist = final_result.loser()?;
        let bronze_winner = bronze_result.winner()?;
        let bronze_loser = bronze_result.loser()?;
        if team_id == champion {
            Some(1)
        } else if team_id == finalist {
            Some(2)
        } else if team_id == bronze_winner {
            Some(3)
        } else if team_id == bronze_loser {
            Some(4)
        } else {
            None
        }
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

    fn head_to_head_points(&self, team_id: &TeamId, standings: &[Standing]) -> u64 {
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
        let mut points = 0_u64;
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
                    points += u64::from(match_result_points(result, team_id, &self.config));
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
                        opponent_win_score: 0,
                        opponent_loss_score: 0,
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
                for team_id in &match_.participants {
                    rows.get_mut(team_id).unwrap().match_points +=
                        u64::from(match_result_points(result, team_id, &self.config));
                }
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
                        for team_id in absent_teams {
                            let standing = rows.get_mut(team_id).unwrap();
                            standing.losses += 1;
                        }
                    }
                    MatchResult::Win {
                        winner_id,
                        loser_id,
                        ..
                    } => {
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
                        rows.get_mut(winner_id).unwrap().wins += 1;
                        rows.get_mut(loser_id).unwrap().losses += 1;
                    }
                    MatchResult::NoShow {
                        absent_teams,
                        winner_id: Some(winner_id),
                    } => {
                        rows.get_mut(winner_id).unwrap().wins += 1;
                        for team_id in absent_teams {
                            let standing = rows.get_mut(team_id).unwrap();
                            standing.losses += 1;
                        }
                    }
                    MatchResult::Draw { .. } => {
                        for team_id in &match_.participants {
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
                    MatchResult::Bye { .. } => {}
                }
            }
        }
        for row in rows.values_mut() {
            row.net_game_wins = row.game_wins as i32 - row.game_losses as i32;
        }
        let match_records: BTreeMap<TeamId, (u64, u64)> = rows
            .iter()
            .map(|(team_id, row)| {
                (
                    team_id.clone(),
                    (u64::from(row.wins), u64::from(row.losses)),
                )
            })
            .collect();
        let ranking_opponents = self.ranking_opponents();
        for row in rows.values_mut() {
            for opponent in ranking_opponents.get(&row.team_id).into_iter().flatten() {
                if let Some((wins, losses)) = match_records.get(opponent) {
                    row.opponent_win_score += wins;
                    row.opponent_loss_score += losses;
                }
            }
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
    shuffle_team_ids(&mut order, seed);
    order
}

const SWISS_TIE_ORDER_SEED_DOMAIN: u64 = 0x5449_455f_4f52_4445;
const SWISS_BYE_ORDER_SEED_DOMAIN: u64 = 0x4259_455f_4f52_4445;
const SWISS_MATCHING_SEED_DOMAIN: u64 = 0x5041_4952_494e_4753;

fn shuffle_team_ids(order: &mut [TeamId], seed: u64) {
    let mut state = seed;
    for index in (1..order.len()).rev() {
        state = split_mix64(state);
        let swap_index = (state % u64::from(index as u32 + 1)) as usize;
        order.swap(index, swap_index);
    }
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
        .filter(|match_| match_.purpose == MatchPurpose::Standard)
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
        .filter(|match_| match_.purpose == MatchPurpose::Standard)
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
                return Err(ScheduleError::ResultDoesNotMatchParticipants(
                    match_id.clone(),
                ));
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
    match_points: &BTreeMap<TeamId, u64>,
    seed: &mut u64,
) -> Result<(usize, Vec<Vec<TeamId>>), ScheduleError> {
    if remaining.is_empty() {
        return Ok((0, Vec::new()));
    }
    let first = remaining.remove(0);
    let mut same_points_candidates = Vec::new();
    let mut cross_group_candidates = Vec::new();
    for candidate in remaining.iter().filter(|candidate| {
        !opponents
            .get(&first)
            .is_some_and(|played| played.contains(*candidate))
    }) {
        if match_points[&first] == match_points[candidate] {
            same_points_candidates.push(candidate.clone());
        } else {
            cross_group_candidates.push(candidate.clone());
        }
    }
    *seed = split_mix64(*seed);
    shuffle_team_ids(&mut same_points_candidates, *seed);
    *seed = split_mix64(*seed);
    shuffle_team_ids(&mut cross_group_candidates, *seed);
    let candidates = same_points_candidates
        .into_iter()
        .map(|team_id| (team_id, false))
        .chain(
            cross_group_candidates
                .into_iter()
                .map(|team_id| (team_id, true)),
        );

    let mut best: Option<(usize, Vec<Vec<TeamId>>)> = None;
    for (candidate, crosses_group) in candidates {
        let index = remaining
            .iter()
            .position(|team_id| team_id == &candidate)
            .expect("瑞士配對候選仍在待配名單");
        let second = remaining.remove(index);
        let cross_cost = usize::from(crosses_group);
        let lower_bound =
            cross_cost + minimum_required_cross_group_pairs(remaining, opponents, match_points);
        if best
            .as_ref()
            .is_none_or(|(best_cost, _)| lower_bound < *best_cost)
        {
            if let Ok((suffix_cost, suffix_pairs)) =
                pair_backtracking(remaining, opponents, match_points, seed)
            {
                let total_cost = cross_cost + suffix_cost;
                if best
                    .as_ref()
                    .is_none_or(|(best_cost, _)| total_cost < *best_cost)
                {
                    let mut pairs = vec![vec![first.clone(), second.clone()]];
                    pairs.extend(suffix_pairs);
                    best = Some((total_cost, pairs));
                }
            }
        }
        remaining.insert(index, second);
    }
    remaining.insert(0, first);
    best.ok_or(ScheduleError::NoLegalSwissPairing)
}

fn minimum_required_cross_group_pairs(
    remaining: &[TeamId],
    opponents: &BTreeMap<TeamId, BTreeSet<TeamId>>,
    match_points: &BTreeMap<TeamId, u64>,
) -> usize {
    let mut group_sizes = BTreeMap::new();
    let mut isolated_by_group = BTreeMap::new();
    for team_id in remaining {
        *group_sizes.entry(match_points[team_id]).or_insert(0usize) += 1;
        let has_same_points_opponent = remaining.iter().any(|candidate| {
            team_id != candidate
                && match_points[team_id] == match_points[candidate]
                && !opponents
                    .get(team_id)
                    .is_some_and(|played| played.contains(candidate))
        });
        if !has_same_points_opponent {
            *isolated_by_group
                .entry(match_points[team_id])
                .or_insert(0usize) += 1;
        }
    }
    let parity_bound = group_sizes.values().filter(|size| *size % 2 == 1).count() / 2;
    let isolated_count: usize = isolated_by_group.values().sum();
    let isolated_bound = (isolated_count + 1) / 2;
    let group_isolated_bound = isolated_by_group.values().copied().max().unwrap_or(0);
    parity_bound.max(isolated_bound).max(group_isolated_bound)
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
        MatchResult::DoubleLoss => points.loss,
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

    fn ranking_fixture_match(
        stage: &mut Stage,
        match_id: &str,
        participants: &[&str],
        result: Option<MatchResult>,
        started: bool,
    ) {
        let round_number = stage.rounds.len() as u32 + 1;
        let participants = participants
            .iter()
            .map(|team| TeamId::from(*team))
            .collect();
        stage.rounds.push(Round {
            round_number,
            kind: RoundKind::Main,
            matches: vec![ScheduledMatch {
                match_id: MatchId::from(match_id),
                stage_id: stage.config.stage_id.clone(),
                round_number,
                sequence: 1,
                kind: RoundKind::Main,
                purpose: MatchPurpose::Standard,
                participants,
                status: if result.is_some() {
                    MatchStatus::Completed
                } else {
                    MatchStatus::Cancelled
                },
                result,
                cancellation_reason: None,
                started,
            }],
        });
    }

    fn ranking_standing(
        team: &str,
        match_points: u64,
        opponent_wins: u64,
        opponent_losses: u64,
    ) -> Standing {
        Standing {
            team_id: TeamId::from(team),
            match_points,
            wins: 0,
            draws: 0,
            losses: 0,
            game_wins: 100,
            game_losses: 0,
            net_game_wins: 100,
            opponent_win_rate_basis_points: Some(10_000),
            opponent_win_score: opponent_wins,
            opponent_loss_score: opponent_losses,
            dropped: false,
            rank: None,
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
            exact_ranking_tie_policy:
                ExactRankingTiePolicy::OrganizerOrUnconflictedRefereeWithPublicReason,
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
                    match_points: MatchPoints {
                        win: 3,
                        draw: 1,
                        loss: 0,
                        bye: 3,
                    },
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
        assert!(
            validate_competition_announcement_plan(&CompetitionAnnouncementPlan {
                stages: vec![invalid_clock],
                ..plan.clone()
            })
            .is_err()
        );

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
        assert_eq!(
            standings
                .iter()
                .find(|row| row.team_id.0 == "t1")
                .unwrap()
                .losses,
            1
        );
        assert_eq!(
            standings
                .iter()
                .find(|row| row.team_id.0 == "t2")
                .unwrap()
                .wins,
            1
        );
    }

    #[test]
    fn formal_standing_order_uses_only_the_three_published_comparison_keys() {
        let stage = Stage::new(config(TournamentFormat::RoundRobin, 4)).unwrap();
        let mut rows = vec![
            ranking_standing("t1", 8, 0, 99),
            ranking_standing("t2", 7, 99, 0),
            ranking_standing("t3", 7, 3, 9),
            ranking_standing("t4", 7, 2, 8),
        ];

        assert_eq!(
            stage.compare_standing_rank_keys(&TeamId::from("t1"), &TeamId::from("t2"), &rows),
            Ordering::Less,
            "自身積分優先於勝分／負分"
        );
        assert_eq!(
            stage.compare_standing_rank_keys(&TeamId::from("t3"), &TeamId::from("t4"), &rows),
            Ordering::Less,
            "勝分較多優先"
        );
        rows[2].opponent_win_score = rows[3].opponent_win_score;
        assert_eq!(
            stage.compare_standing_rank_keys(&TeamId::from("t3"), &TeamId::from("t4"), &rows),
            Ordering::Greater,
            "同勝分時負分較少優先"
        );
        rows[2].opponent_loss_score = rows[3].opponent_loss_score;
        assert_eq!(
            stage.compare_standing_rank_keys(&TeamId::from("t3"), &TeamId::from("t4"), &rows),
            Ordering::Equal,
            "局數、對手勝率、種子及隊伍 ID 不得破除全同"
        );
    }

    #[test]
    fn opponent_scores_use_unique_played_opponents_and_their_final_match_records() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 5)).unwrap();
        stage.rounds.clear();
        stage.status = StageStatus::Finished;
        ranking_fixture_match(
            &mut stage,
            "ab-first",
            &["t1", "t2"],
            Some(MatchResult::win("t1", "t2")),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "ab-rematch",
            &["t1", "t2"],
            Some(MatchResult::win("t2", "t1")),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "bc",
            &["t2", "t3"],
            Some(MatchResult::win("t2", "t3")),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "ac-no-show",
            &["t1", "t3"],
            Some(MatchResult::NoShow {
                absent_teams: vec![TeamId::from("t1")],
                winner_id: Some(TeamId::from("t3")),
            }),
            false,
        );
        ranking_fixture_match(
            &mut stage,
            "bd-double-loss",
            &["t2", "t4"],
            Some(MatchResult::DoubleLoss),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "be-draw",
            &["t2", "t5"],
            Some(MatchResult::draw()),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "ec",
            &["t5", "t3"],
            Some(MatchResult::win("t5", "t3")),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "td-bye",
            &["t4"],
            Some(MatchResult::Bye {
                team_id: TeamId::from("t4"),
            }),
            false,
        );
        ranking_fixture_match(&mut stage, "ad-cancelled", &["t1", "t4"], None, false);
        stage.refresh_standings();

        let standing = |team: &str| {
            stage
                .standings
                .iter()
                .find(|row| row.team_id.0 == team)
                .unwrap()
        };
        assert_eq!(
            (
                standing("t1").opponent_win_score,
                standing("t1").opponent_loss_score
            ),
            (2, 2)
        );
        assert_eq!(
            (
                standing("t2").opponent_win_score,
                standing("t2").opponent_loss_score
            ),
            (3, 5)
        );
        assert_eq!(
            (
                standing("t3").opponent_win_score,
                standing("t3").opponent_loss_score
            ),
            (3, 2)
        );
        assert_eq!((standing("t4").wins, standing("t4").losses), (0, 1));
        assert_eq!(standing("t1").losses, 2, "合法缺席結果仍計入自身交手敗場");

        let json = serde_json::to_value(standing("t1")).unwrap();
        assert_eq!(json["opponentWinScore"], "2");
        assert_eq!(json["opponentLossScore"], "2");
        assert!(json.get("opponent_win_score").is_none());
    }

    #[test]
    fn exact_ranking_ties_block_qualification_and_completion_until_public_decision() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 3)).unwrap();
        stage.rounds.clear();
        stage.status = StageStatus::Finished;
        ranking_fixture_match(
            &mut stage,
            "ac",
            &["t1", "t3"],
            Some(MatchResult::win("t1", "t3")),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "bc",
            &["t2", "t3"],
            Some(MatchResult::win("t2", "t3")),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "ab",
            &["t1", "t2"],
            Some(MatchResult::draw()),
            true,
        );
        stage.refresh_standings();
        assert_eq!(
            stage.unresolved_ranking_tie_groups(),
            vec![vec![TeamId::from("t1"), TeamId::from("t2")]]
        );

        let mut tournament = Tournament {
            tournament_id: "event".into(),
            stages: vec![stage],
        };
        assert!(!tournament.rankings_resolved());
        assert_eq!(
            tournament.resolve_stage_qualification(&[
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 1 },
                    1
                ),
                stage_qualification_slot(
                    "stage",
                    StageQualificationSource::UniqueStandingRank { rank: 3 },
                    3
                ),
            ]),
            Err(StageQualificationBlocker::StandingRankUnavailable {
                source_stage_id: StageId::from("stage"),
                team_id: TeamId::from("t1"),
            })
        );
        assert_eq!(
            tournament.adjudicate_ranking_tie(
                &StageId::from("stage"),
                vec![TeamId::from("t2"), TeamId::from("t1")],
                RankingDecisionActor::Referee {
                    user_id: "independent-referee".into()
                },
                "公開理由：已核對同分依據".into(),
            ),
            Ok(())
        );
        assert!(tournament.rankings_resolved());
        let stage = tournament.stage(&StageId::from("stage")).unwrap();
        assert_eq!(
            stage
                .standings()
                .iter()
                .find(|row| row.team_id.0 == "t2")
                .unwrap()
                .rank,
            Some(1)
        );
        assert_eq!(
            stage
                .standings()
                .iter()
                .find(|row| row.team_id.0 == "t1")
                .unwrap()
                .rank,
            Some(2)
        );
        assert_eq!(
            stage
                .standings()
                .iter()
                .find(|row| row.team_id.0 == "t3")
                .unwrap()
                .rank,
            Some(3)
        );
        assert!(stage.unresolved_ranking_tie_groups().is_empty());
        let decision = serde_json::to_value(&stage.ranking_tie_decisions()[0]).unwrap();
        assert_eq!(decision["orderedTeamIds"], serde_json::json!(["t2", "t1"]));
        assert_eq!(
            decision["decidedBy"],
            serde_json::json!({"type": "referee", "userId": "independent-referee"})
        );
        assert_eq!(decision["reason"], "公開理由：已核對同分依據");
        assert_eq!(decision["matchPoints"], "4");
        assert_eq!(
            tournament.adjudicate_ranking_tie(
                &StageId::from("stage"),
                vec![TeamId::from("t1"), TeamId::from("t2")],
                RankingDecisionActor::Organizer {
                    user_id: "organizer".into()
                },
                "再次覆寫".into(),
            ),
            Err(ScheduleError::RankingTieAlreadyDecided(StageId::from(
                "stage"
            )))
        );
    }

    #[test]
    fn legacy_tournament_snapshot_recomputes_formal_ranks_from_match_records() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 2)).unwrap();
        let match_id = stage.rounds()[0].matches[0].match_id.clone();
        stage
            .submit_result(&match_id, MatchResult::win("t1", "t2"))
            .unwrap();
        let tournament = Tournament {
            tournament_id: "event".into(),
            stages: vec![stage],
        };
        let mut legacy = serde_json::to_value(tournament).unwrap();
        let stage = legacy["stages"][0].as_object_mut().unwrap();
        stage.remove("rankingTieDecisions");
        for standing in stage["standings"].as_array_mut().unwrap() {
            let standing = standing.as_object_mut().unwrap();
            standing.remove("opponentWinScore");
            standing.remove("opponentLossScore");
        }

        let restored: Tournament = serde_json::from_value(legacy).unwrap();
        let stage = restored.stage(&StageId::from("stage")).unwrap();
        assert!(stage.ranking_tie_decisions().is_empty());
        let stored_standing = |team: &str| {
            stage
                .standings()
                .iter()
                .find(|row| row.team_id.0 == team)
                .unwrap()
        };
        assert_eq!(
            (
                stored_standing("t1").opponent_win_score,
                stored_standing("t1").opponent_loss_score
            ),
            (0, 0)
        );
        assert_eq!(
            (
                stored_standing("t2").opponent_win_score,
                stored_standing("t2").opponent_loss_score
            ),
            (0, 0)
        );

        let projection = restored.ranking_projection();
        let stage = projection.stage(&StageId::from("stage")).unwrap();
        let standing = |team: &str| {
            stage
                .standings()
                .iter()
                .find(|row| row.team_id.0 == team)
                .unwrap()
        };
        assert_eq!(
            (
                standing("t1").opponent_win_score,
                standing("t1").opponent_loss_score
            ),
            (0, 1)
        );
        assert_eq!(
            (
                standing("t2").opponent_win_score,
                standing("t2").opponent_loss_score
            ),
            (1, 0)
        );
        assert_eq!(
            (standing("t1").rank, standing("t2").rank),
            (Some(1), Some(2))
        );
        assert_eq!(
            restored,
            serde_json::from_value(serde_json::to_value(&restored).unwrap()).unwrap()
        );
    }

    #[test]
    fn ranking_tie_decision_rejects_partial_stale_and_unfinished_inputs() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 3)).unwrap();
        let mut tournament = Tournament {
            tournament_id: "event".into(),
            stages: vec![stage.clone()],
        };
        assert_eq!(
            tournament.adjudicate_ranking_tie(
                &StageId::from("stage"),
                vec![TeamId::from("t1"), TeamId::from("t2")],
                RankingDecisionActor::Organizer {
                    user_id: "organizer".into()
                },
                "理由".into(),
            ),
            Err(ScheduleError::RankingTieStageNotFinished(StageId::from(
                "stage"
            )))
        );

        stage.rounds.clear();
        stage.status = StageStatus::Finished;
        ranking_fixture_match(
            &mut stage,
            "cycle-a",
            &["t1", "t2"],
            Some(MatchResult::win("t1", "t2")),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "cycle-b",
            &["t2", "t3"],
            Some(MatchResult::win("t2", "t3")),
            true,
        );
        ranking_fixture_match(
            &mut stage,
            "cycle-c",
            &["t3", "t1"],
            Some(MatchResult::win("t3", "t1")),
            true,
        );
        stage.refresh_standings();
        tournament.stages[0] = stage;
        assert_eq!(
            tournament.adjudicate_ranking_tie(
                &StageId::from("stage"),
                vec![TeamId::from("t1"), TeamId::from("t2")],
                RankingDecisionActor::Organizer {
                    user_id: "organizer".into()
                },
                "理由".into(),
            ),
            Err(ScheduleError::RankingTieOrderMismatch(StageId::from(
                "stage"
            ))),
            "不可裁決三隊同分中的任意兩隊"
        );
        assert_eq!(
            tournament.adjudicate_ranking_tie(
                &StageId::from("stage"),
                vec![TeamId::from("t1"), TeamId::from("t2"), TeamId::from("t3")],
                RankingDecisionActor::Organizer {
                    user_id: "organizer".into()
                },
                "   ".into(),
            ),
            Err(ScheduleError::EmptyRankingDecisionReason)
        );
        assert_eq!(
            tournament.adjudicate_ranking_tie(
                &StageId::from("stage"),
                vec![TeamId::from("unknown"), TeamId::from("t1")],
                RankingDecisionActor::Organizer {
                    user_id: "organizer".into()
                },
                "理由".into(),
            ),
            Err(ScheduleError::RankingTieNotFound(StageId::from("stage")))
        );
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
        assert_eq!(
            standings
                .iter()
                .find(|row| row.team_id.0 == "t1")
                .unwrap()
                .losses,
            1
        );
        assert_eq!(
            standings
                .iter()
                .find(|row| row.team_id.0 == "t2")
                .unwrap()
                .losses,
            1
        );
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
            standings
                .iter()
                .find(|row| row.team_id.0 == "t1")
                .unwrap()
                .match_points,
            2
        );
        assert_eq!(
            standings
                .iter()
                .find(|row| row.team_id.0 == "t2")
                .unwrap()
                .match_points,
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
            standings
                .iter()
                .find(|row| row.team_id.0 == "t1")
                .unwrap()
                .match_points,
            2
        );
        assert_eq!(
            standings
                .iter()
                .find(|row| row.team_id.0 == "t2")
                .unwrap()
                .match_points,
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

    #[test]
    fn swiss_pairing_ties_use_the_persisted_seed_for_each_round() {
        let policy = swiss_policy(DrawCredit::new(1, 2).unwrap());
        let stage_for_seed = |seed| {
            let mut config = swiss_config(4, 2, policy.clone());
            config.random_seed = seed;
            Stage::new(config).unwrap()
        };
        let first = stage_for_seed(7);
        let same_seed = stage_for_seed(7);
        let different_seed = stage_for_seed(8);

        let pairing_for = |stage: &Stage| stage.swiss_pairing(&stage.swiss_order()).unwrap();
        assert_eq!(pairing_for(&first), pairing_for(&same_seed));
        assert_ne!(pairing_for(&first), pairing_for(&different_seed));
    }

    #[test]
    fn swiss_pairing_seed_selects_between_multiple_legal_matchings() {
        let teams = ["a", "b", "c", "d"].map(TeamId::from).to_vec();
        let opponents: BTreeMap<TeamId, BTreeSet<TeamId>> = teams
            .iter()
            .cloned()
            .map(|team_id| (team_id, BTreeSet::new()))
            .collect();
        let pairing_for_seed = |seed| {
            let mut remaining = teams.clone();
            let match_points = teams.iter().cloned().map(|team_id| (team_id, 0)).collect();
            let mut seed = seed;
            let (cross_group_pairs, pairs) =
                pair_backtracking(&mut remaining, &opponents, &match_points, &mut seed).unwrap();
            assert_eq!(cross_group_pairs, 0);
            pairs
        };

        assert_eq!(pairing_for_seed(7), pairing_for_seed(7));
        assert_ne!(pairing_for_seed(7), pairing_for_seed(9));
    }

    #[test]
    fn swiss_pairing_crosses_score_groups_when_rematches_block_same_points() {
        let teams = ["a", "b", "c", "d"].map(TeamId::from).to_vec();
        let match_points: BTreeMap<TeamId, u64> = [
            (TeamId::from("a"), 3),
            (TeamId::from("b"), 3),
            (TeamId::from("c"), 0),
            (TeamId::from("d"), 0),
        ]
        .into_iter()
        .collect();
        let opponents = BTreeMap::from([
            (TeamId::from("a"), BTreeSet::from([TeamId::from("b")])),
            (TeamId::from("b"), BTreeSet::from([TeamId::from("a")])),
            (TeamId::from("c"), BTreeSet::from([TeamId::from("d")])),
            (TeamId::from("d"), BTreeSet::from([TeamId::from("c")])),
        ]);
        let mut remaining = teams;
        let mut seed = 7;
        let (cross_group_pairs, pairs) =
            pair_backtracking(&mut remaining, &opponents, &match_points, &mut seed).unwrap();

        assert_eq!(cross_group_pairs, 2);
        assert_eq!(pairs.len(), 2);
        assert!(
            pairs
                .iter()
                .all(|pair| !opponents[&pair[0]].contains(&pair[1]))
        );
        assert!(
            pairs
                .iter()
                .all(|pair| match_points[&pair[0]] != match_points[&pair[1]])
        );
    }

    #[test]
    fn swiss_pairing_selects_bye_to_avoid_unnecessary_score_group_crossing() {
        let mut stage = Stage::new(swiss_config(
            5,
            2,
            swiss_policy(DrawCredit::new(1, 2).unwrap()),
        ))
        .unwrap();
        let first_round = stage.rounds()[0].clone();
        let previous_bye = first_round
            .matches
            .iter()
            .find_map(
                |match_| match (match_.participants.as_slice(), match_.result.as_ref()) {
                    ([team_id], Some(MatchResult::Bye { .. })) => Some(team_id.clone()),
                    _ => None,
                },
            )
            .unwrap();
        for match_ in first_round.matches {
            if match_.participants.len() == 2 {
                let winner = match_.participants[0].clone();
                let loser = match_.participants[1].clone();
                stage
                    .submit_result(&match_.match_id, MatchResult::win(winner, loser))
                    .unwrap();
            }
        }

        let points: BTreeMap<TeamId, u64> = stage
            .standings()
            .into_iter()
            .map(|standing| (standing.team_id.clone(), standing.match_points))
            .collect();
        let second_round = &stage.rounds()[1];
        let second_bye = second_round
            .matches
            .iter()
            .find_map(
                |match_| match (match_.participants.as_slice(), match_.result.as_ref()) {
                    ([team_id], Some(MatchResult::Bye { .. })) => Some(team_id),
                    _ => None,
                },
            )
            .unwrap();

        assert_ne!(second_bye, &previous_bye);
        assert!(
            second_round
                .matches
                .iter()
                .filter(|match_| match_.participants.len() == 2)
                .all(|match_| points[&match_.participants[0]] == points[&match_.participants[1]])
        );
    }

    #[test]
    fn swiss_pairing_uses_persisted_seed_to_break_equal_cost_bye_ties() {
        let complete_second_round = |seed| {
            let mut policy = swiss_policy(DrawCredit::new(1, 2).unwrap());
            policy.match_points.bye = 1;
            policy.tie_breakers = vec![SwissTieBreaker::MatchPoints];
            let mut config = swiss_config(5, 2, policy);
            config.random_seed = seed;
            config.teams = ["a", "b", "c", "d", "e"]
                .into_iter()
                .enumerate()
                .map(|(index, team_id)| SeededTeam {
                    team_id: TeamId::from(team_id),
                    seed: index as u32 + 1,
                })
                .collect();
            let mut stage = Stage::new(config).unwrap();
            let first_round = stage.rounds()[0].clone();
            let previous_bye = first_round
                .matches
                .iter()
                .find(|match_| match_.participants.len() == 1)
                .unwrap()
                .participants[0]
                .clone();
            let played: Vec<_> = first_round
                .matches
                .iter()
                .filter(|match_| match_.participants.len() == 2)
                .cloned()
                .collect();
            assert_eq!(played.len(), 2);

            let winner = played[1].participants[0].clone();
            let loser = played[1].participants[1].clone();
            stage
                .submit_result(&played[0].match_id, MatchResult::DoubleLoss)
                .unwrap();
            stage
                .submit_result(
                    &played[1].match_id,
                    MatchResult::win(winner.clone(), loser.clone()),
                )
                .unwrap();

            (
                stage.rounds()[1].clone(),
                first_round,
                winner,
                loser,
                previous_bye,
            )
        };
        let assert_equal_cost_bye_alternatives =
            |first_round: &Round, winner: &TeamId, loser: &TeamId, previous_bye: &TeamId| {
                let match_points = BTreeMap::from([
                    (winner.clone(), 3),
                    (loser.clone(), 0),
                    (previous_bye.clone(), 1),
                ]);
                let alternatives = [
                    (winner, [loser, previous_bye]),
                    (loser, [winner, previous_bye]),
                ];
                for (bye_team, pair) in alternatives {
                    assert_ne!(bye_team, previous_bye);
                    assert_ne!(match_points[pair[0]], match_points[pair[1]]);
                    assert!(first_round.matches.iter().all(|match_| {
                        !pair
                            .iter()
                            .all(|team_id| match_.participants.contains(team_id))
                    }));
                }
            };

        let (seed7_round, seed7_first, seed7_winner, seed7_loser, seed7_previous_bye) =
            complete_second_round(7);
        let (seed7_repeat, _, _, _, _) = complete_second_round(7);
        assert_eq!(seed7_round, seed7_repeat);
        assert_eq!(seed7_winner, TeamId::from("a"));
        assert_eq!(seed7_loser, TeamId::from("b"));
        assert_eq!(seed7_previous_bye, TeamId::from("c"));
        assert_equal_cost_bye_alternatives(
            &seed7_first,
            &seed7_winner,
            &seed7_loser,
            &seed7_previous_bye,
        );

        let (seed8_round, seed8_first, seed8_winner, seed8_loser, seed8_previous_bye) =
            complete_second_round(8);
        let (seed8_repeat, _, _, _, _) = complete_second_round(8);
        assert_eq!(seed8_round, seed8_repeat);
        assert_equal_cost_bye_alternatives(
            &seed8_first,
            &seed8_winner,
            &seed8_loser,
            &seed8_previous_bye,
        );

        let seed8_bye = seed8_round
            .matches
            .iter()
            .find(|match_| match_.participants.len() == 1)
            .unwrap()
            .participants[0]
            .clone();
        assert_eq!(
            seed8_bye, seed8_winner,
            "固定種子 8 應抽中等成本候選中的勝方輪空"
        );
        assert_ne!(seed8_bye, seed8_previous_bye);

        let seed8_cross_group_match = seed8_round
            .matches
            .iter()
            .find(|match_| match_.participants.len() == 2)
            .unwrap();
        assert!(seed8_cross_group_match.participants.contains(&seed8_loser));
        assert!(
            seed8_cross_group_match
                .participants
                .contains(&seed8_previous_bye)
        );

        let seed7_bye = seed7_round
            .matches
            .iter()
            .find(|match_| match_.participants.len() == 1)
            .unwrap()
            .participants[0]
            .clone();
        assert_ne!(seed7_bye, seed7_previous_bye);
        let seed7_cross_group_match = seed7_round
            .matches
            .iter()
            .find(|match_| match_.participants.len() == 2)
            .unwrap();
        assert!(seed7_bye == seed7_winner || seed7_bye == seed7_loser);
        let seed7_other_candidate = if seed7_bye == seed7_winner {
            &seed7_loser
        } else {
            &seed7_winner
        };
        assert!(
            seed7_cross_group_match
                .participants
                .contains(seed7_other_candidate)
        );
        assert!(
            seed7_cross_group_match
                .participants
                .contains(&seed7_previous_bye)
        );
    }

    #[test]
    fn swiss_pairing_prefers_complete_same_points_solution_before_crossing_groups() {
        let mut policy = swiss_policy(DrawCredit::new(1, 2).unwrap());
        policy.tie_breakers = vec![SwissTieBreaker::MatchPoints];
        let mut config = swiss_config(8, 2, policy);
        config.teams = ["a", "b", "c", "d", "e", "f", "g", "h"]
            .into_iter()
            .enumerate()
            .map(|(index, team_id)| SeededTeam {
                team_id: TeamId::from(team_id),
                seed: index as u32 + 1,
            })
            .collect();
        config.random_seed = 7;
        let mut stage = Stage::new(config).unwrap();
        let first_round = stage.rounds()[0].clone();
        for match_ in first_round.matches {
            let winner = match_.participants[0].clone();
            let loser = match_.participants[1].clone();
            stage
                .submit_result(&match_.match_id, MatchResult::win(winner, loser))
                .unwrap();
        }

        let points: BTreeMap<TeamId, u64> = stage
            .standings()
            .into_iter()
            .map(|standing| (standing.team_id.clone(), standing.match_points))
            .collect();
        let second_round = &stage.rounds()[1];
        for match_ in &second_round.matches {
            assert_eq!(
                points[&match_.participants[0]], points[&match_.participants[1]],
                "有完整不重賽同積分方案時，不應產生不必要的跨組配對"
            );
        }
    }

    #[test]
    fn swiss_progresses_all_announced_rounds_and_scores_custom_match_points_once() {
        let match_points = MatchPoints {
            win: 4,
            draw: 2,
            loss: 1,
            bye: 6,
        };
        let mut policy = swiss_policy(DrawCredit::new(1, 2).unwrap());
        policy.match_points = match_points.clone();
        let mut stage = Stage::new(swiss_config(5, 3, policy)).unwrap();
        let mut expected: BTreeMap<TeamId, (u64, u32, u32, u32)> = stage
            .config
            .teams
            .iter()
            .map(|team| (team.team_id.clone(), (0, 0, 0, 0)))
            .collect();
        let mut bye_teams = BTreeSet::new();
        let mut opponents: BTreeMap<TeamId, BTreeSet<TeamId>> = stage
            .config
            .teams
            .iter()
            .map(|team| (team.team_id.clone(), BTreeSet::new()))
            .collect();

        for round_number in 1..=3 {
            assert_eq!(stage.rounds().len(), round_number as usize);
            let round = stage.rounds().last().unwrap().clone();
            assert_eq!(round.round_number, round_number);
            let mut completion_retry_checked = false;
            for match_ in round.matches {
                assert_eq!(
                    match_.match_id.0,
                    format!("stage-g0-r{round_number}-m{}", match_.sequence)
                );
                if match_.participants.len() == 1 {
                    let team_id = match_.participants[0].clone();
                    assert!(matches!(match_.result, Some(MatchResult::Bye { .. })));
                    assert!(bye_teams.insert(team_id.clone()));
                    expected.get_mut(&team_id).unwrap().0 += u64::from(match_points.bye);
                    continue;
                }

                let first = match_.participants[0].clone();
                let second = match_.participants[1].clone();
                assert!(opponents.get_mut(&first).unwrap().insert(second.clone()));
                assert!(opponents.get_mut(&second).unwrap().insert(first.clone()));
                let (result, score_changes) = match round_number {
                    1 => {
                        let changes = [
                            (first.clone(), (u64::from(match_points.win), 1, 0, 0)),
                            (second.clone(), (u64::from(match_points.loss), 0, 0, 1)),
                        ];
                        (MatchResult::win(first, second), changes)
                    }
                    2 => {
                        let changes = [
                            (first.clone(), (u64::from(match_points.draw), 0, 1, 0)),
                            (second.clone(), (u64::from(match_points.draw), 0, 1, 0)),
                        ];
                        (MatchResult::draw(), changes)
                    }
                    3 if match_.sequence == 1 => {
                        let changes = [
                            (first.clone(), (u64::from(match_points.loss), 0, 0, 1)),
                            (second.clone(), (u64::from(match_points.loss), 0, 0, 1)),
                        ];
                        (MatchResult::DoubleLoss, changes)
                    }
                    _ => {
                        let changes = [
                            (first.clone(), (u64::from(match_points.win), 1, 0, 0)),
                            (second.clone(), (u64::from(match_points.loss), 0, 0, 1)),
                        ];
                        (
                            MatchResult::Forfeit {
                                winner_id: first,
                                loser_id: second,
                            },
                            changes,
                        )
                    }
                };
                for (team_id, change) in score_changes {
                    let actual = expected.get_mut(&team_id).unwrap();
                    actual.0 += change.0;
                    actual.1 += change.1;
                    actual.2 += change.2;
                    actual.3 += change.3;
                }
                assert_eq!(
                    stage
                        .submit_result(&match_.match_id, result.clone())
                        .unwrap(),
                    UpdateOutcome::Applied
                );
                if round_number == 1 && stage.rounds().len() == 2 {
                    let generated_round = stage.rounds()[1].clone();
                    assert_eq!(
                        stage.submit_result(&match_.match_id, result).unwrap(),
                        UpdateOutcome::AlreadyRecorded
                    );
                    assert_eq!(stage.rounds().len(), 2);
                    assert_eq!(stage.rounds()[1], generated_round);
                    completion_retry_checked = true;
                }
            }

            if round_number == 1 {
                assert!(completion_retry_checked);
                assert_eq!(stage.rounds().len(), 2);
                let snapshot = serde_json::to_value(&stage).unwrap();
                let restored: Stage = serde_json::from_value(snapshot).unwrap();
                assert_eq!(restored, stage);
                stage = restored;
            }
        }

        assert_eq!(stage.status(), StageStatus::Finished);
        assert_eq!(stage.rounds().len(), 3);
        assert_eq!(stage.schedule_generation, 0);
        assert_eq!(bye_teams.len(), 3);
        for row in stage.standings() {
            let expected = expected.get(&row.team_id).unwrap();
            assert_eq!(row.match_points, expected.0, "{} points", row.team_id);
            assert_eq!(row.wins, expected.1, "{} wins", row.team_id);
            assert_eq!(row.draws, expected.2, "{} draws", row.team_id);
            assert_eq!(row.losses, expected.3, "{} losses", row.team_id);
        }
        for (team_id, played) in opponents {
            let expected_matches = 3 - u32::from(bye_teams.contains(&team_id));
            assert_eq!(
                played.len(),
                expected_matches as usize,
                "{team_id} opponents"
            );
        }
    }

    #[test]
    fn swiss_accumulates_announced_u32_points_exactly_across_rounds_and_snapshots() {
        let mut policy = swiss_policy(DrawCredit::new(1, 2).unwrap());
        policy.match_points = MatchPoints {
            win: 0,
            draw: 0,
            loss: u32::MAX,
            bye: 0,
        };
        policy.tie_breakers = vec![SwissTieBreaker::MatchPoints];
        let mut config = swiss_config(4, 2, policy);
        config.random_seed = 7;
        config.teams = ["a", "b", "c", "d"]
            .into_iter()
            .enumerate()
            .map(|(index, team_id)| SeededTeam {
                team_id: TeamId::from(team_id),
                seed: index as u32 + 1,
            })
            .collect();
        let stage_id = config.stage_id.clone();
        let mut tournament = Tournament::new(TournamentConfig {
            tournament_id: "large-points".to_string(),
            stage: config,
        })
        .unwrap();

        let first_round = tournament.stage(&stage_id).unwrap().rounds()[0].clone();
        assert_eq!(
            first_round
                .matches
                .iter()
                .map(|match_| match_.participants.clone())
                .collect::<Vec<_>>(),
            vec![
                vec![TeamId::from("b"), TeamId::from("a")],
                vec![TeamId::from("c"), TeamId::from("d")],
            ]
        );
        for match_ in &first_round.matches {
            let winner = match_.participants[0].clone();
            let loser = match_.participants[1].clone();
            assert_eq!(
                tournament
                    .submit_result(&stage_id, &match_.match_id, MatchResult::win(winner, loser),)
                    .unwrap(),
                UpdateOutcome::Applied
            );
        }

        let second_round = tournament.stage(&stage_id).unwrap().rounds()[1].clone();
        let pairings: BTreeSet<BTreeSet<TeamId>> = second_round
            .matches
            .iter()
            .map(|match_| match_.participants.iter().cloned().collect())
            .collect();
        assert_eq!(
            pairings,
            BTreeSet::from([
                BTreeSet::from([TeamId::from("a"), TeamId::from("d")]),
                BTreeSet::from([TeamId::from("b"), TeamId::from("c")]),
            ])
        );
        assert!(second_round.matches.iter().all(|match_| {
            first_round.matches.iter().all(|previous| {
                previous.participants != match_.participants
                    && previous
                        .participants
                        .iter()
                        .rev()
                        .ne(match_.participants.iter())
            })
        }));

        let b_c_match = second_round
            .matches
            .iter()
            .find(|match_| match_.participants.contains(&TeamId::from("b")))
            .unwrap();
        assert_eq!(
            tournament
                .submit_result(&stage_id, &b_c_match.match_id, MatchResult::draw())
                .unwrap(),
            UpdateOutcome::Applied
        );

        let a_loss_points = tournament
            .stage(&stage_id)
            .unwrap()
            .standings()
            .iter()
            .find(|standing| standing.team_id == TeamId::from("a"))
            .unwrap()
            .match_points;
        let d_loss_points = tournament
            .stage(&stage_id)
            .unwrap()
            .standings()
            .iter()
            .find(|standing| standing.team_id == TeamId::from("d"))
            .unwrap()
            .match_points;
        assert_eq!(a_loss_points, u64::from(u32::MAX));
        assert_eq!(d_loss_points, u64::from(u32::MAX));

        let snapshot = serde_json::to_value(&tournament).unwrap();
        assert_eq!(
            snapshot["stages"][0]["standings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|standing| standing["teamId"] == "d")
                .unwrap()["matchPoints"],
            serde_json::json!("4294967295")
        );
        let mut legacy_snapshot = snapshot.clone();
        for standing in legacy_snapshot["stages"][0]["standings"]
            .as_array_mut()
            .unwrap()
        {
            standing["matchPoints"] = serde_json::json!(
                standing["matchPoints"]
                    .as_str()
                    .unwrap()
                    .parse::<u64>()
                    .unwrap()
            );
        }
        let restored: Tournament = serde_json::from_value(snapshot).unwrap();
        let restored_from_legacy_snapshot: Tournament =
            serde_json::from_value(legacy_snapshot).unwrap();
        assert_eq!(restored, tournament);
        assert_eq!(restored_from_legacy_snapshot, tournament);
        tournament = restored;

        let a_d_match = tournament.stage(&stage_id).unwrap().rounds()[1]
            .matches
            .iter()
            .find(|match_| match_.participants.contains(&TeamId::from("a")))
            .unwrap()
            .clone();
        let result = MatchResult::win(TeamId::from("a"), TeamId::from("d"));
        assert_eq!(
            tournament
                .submit_result(&stage_id, &a_d_match.match_id, result.clone())
                .unwrap(),
            UpdateOutcome::Applied
        );
        assert_eq!(
            tournament
                .submit_result(&stage_id, &a_d_match.match_id, result)
                .unwrap(),
            UpdateOutcome::AlreadyRecorded
        );
        assert_eq!(tournament.stage(&stage_id).unwrap().rounds().len(), 2);
        assert_eq!(
            tournament
                .stage(&stage_id)
                .unwrap()
                .standings()
                .iter()
                .find(|standing| standing.team_id == TeamId::from("d"))
                .unwrap()
                .match_points,
            8_589_934_590
        );

        let public_dto = serde_json::to_value(&tournament).unwrap();
        assert_eq!(
            public_dto["stages"][0]["standings"]
                .as_array()
                .unwrap()
                .iter()
                .find(|standing| standing["teamId"] == "d")
                .unwrap()["matchPoints"],
            serde_json::json!("8589934590")
        );
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
            Err(StageQualificationBlocker::StandingRankUnavailable {
                source_stage_id: StageId::from("stage"),
                team_id: TeamId::from("t1"),
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
            Err(StageQualificationBlocker::StandingRankUnavailable {
                source_stage_id: StageId::from("stage"),
                team_id: TeamId::from("t1"),
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
    fn stage_qualification_allows_unique_ranks_above_an_unresolved_tie() {
        let mut stage = Stage::new(config(TournamentFormat::RoundRobin, 4)).unwrap();
        let pending_match_ids: Vec<_> = stage
            .rounds()
            .iter()
            .flat_map(|round| &round.matches)
            .filter(|match_| match_.status == MatchStatus::Pending)
            .map(|match_| match_.match_id.clone())
            .collect();
        for match_id in pending_match_ids {
            let match_ = stage.find_match(&match_id).unwrap().clone();
            let [first, second] = match_.participants.as_slice() else {
                panic!("循環賽場次應有兩名參賽隊伍");
            };
            let result = if first == &TeamId::from("t1") || second == &TeamId::from("t1") {
                let loser = if first == &TeamId::from("t1") {
                    second
                } else {
                    first
                };
                MatchResult::win(TeamId::from("t1"), loser.clone())
            } else if first == &TeamId::from("t2") || second == &TeamId::from("t2") {
                let loser = if first == &TeamId::from("t2") {
                    second
                } else {
                    first
                };
                MatchResult::win(TeamId::from("t2"), loser.clone())
            } else {
                MatchResult::draw()
            };
            stage.submit_result(&match_id, result).unwrap();
        }

        assert_eq!(stage.status(), StageStatus::Finished);
        assert_eq!(
            stage
                .standings()
                .iter()
                .map(|standing| (&standing.team_id, standing.rank))
                .collect::<BTreeMap<_, _>>(),
            BTreeMap::from([
                (&TeamId::from("t1"), Some(1)),
                (&TeamId::from("t2"), Some(2)),
                (&TeamId::from("t3"), None),
                (&TeamId::from("t4"), None),
            ])
        );

        let tournament = stage_qualification_tournament(stage);
        let top_two_slots = [
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
            tournament
                .resolve_stage_qualification(&top_two_slots)
                .unwrap()
                .seeded_teams()
                .iter()
                .map(|team| team.team_id.clone())
                .collect::<Vec<_>>(),
            vec![TeamId::from("t1"), TeamId::from("t2")]
        );
        assert_eq!(
            tournament
                .plan_qualified_stage(qualified_stage_plan_template(
                    "next-stage",
                    TournamentFormat::SingleElimination,
                    top_two_slots.to_vec(),
                ))
                .unwrap()
                .qualification()
                .seeded_teams()
                .iter()
                .map(|team| team.team_id.clone())
                .collect::<Vec<_>>(),
            vec![TeamId::from("t1"), TeamId::from("t2")]
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
                    StageQualificationSource::UniqueStandingRank { rank: 3 },
                    2,
                ),
            ]),
            Err(StageQualificationBlocker::StandingRankUnavailable {
                source_stage_id: StageId::from("stage"),
                team_id: TeamId::from("t3"),
            })
        );
    }

    #[test]
    fn stage_qualification_blocks_dropped_team_even_when_ranked() {
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
        let unresolved_match_tournament = stage_qualification_tournament(unresolved_match_stage);
        assert!(matches!(
            unresolved_match_tournament.resolve_stage_qualification(&rank_slots),
            Err(StageQualificationBlocker::SourceMatchUnresolved {
                status: MatchStatus::InProgress,
                ..
            })
        ));

        let mut double_loss_stage =
            Stage::new(config(TournamentFormat::DoubleElimination, 2)).unwrap();
        let double_loss_match = double_loss_stage.rounds()[0].matches[0].match_id.clone();
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
            tournament.resolve_stage_qualification(&[stage_qualification_slot(
                "stage",
                StageQualificationSource::UniqueStandingRank { rank: 1 },
                1,
            ),]),
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
        assert!(
            serde_json::from_value::<StageQualificationSlot>(serde_json::json!({
                "sourceStageId": "qualifier",
                "source": {"type": "placement", "rank": 2},
                "destinationSeed": 7
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<StageQualificationSlot>(serde_json::json!({
                "sourceStageId": "qualifier",
                "source": {"type": "uniqueStandingRank", "rank": 2, "fallback": true},
                "destinationSeed": 7
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<StageQualificationSlot>(serde_json::json!({
                "sourceStageId": "qualifier",
                "source": {"type": "finishedStageWinner"},
                "destinationSeed": 7,
                "defaultRank": 1
            }))
            .is_err()
        );
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
            (Some(1), Some(swiss_policy(DrawCredit::new(1, 2).unwrap())))
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
            stage_qualification_slot("stage-a", StageQualificationSource::FinishedStageWinner, 7),
            stage_qualification_slot("stage-b", StageQualificationSource::FinishedStageWinner, 2),
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
            let (winner, loser) =
                if priority(&match_.participants[0]) < priority(&match_.participants[1]) {
                    (
                        match_.participants[0].clone(),
                        match_.participants[1].clone(),
                    )
                } else {
                    (
                        match_.participants[1].clone(),
                        match_.participants[0].clone(),
                    )
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
        assert_eq!(
            plan.planned_stage().config().stage_id,
            StageId::from("ranked-next")
        );
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
            let mut tournament =
                stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);
            let template =
                qualified_stage_plan_template("next", format, qualified_stage_plan_winner_slots());
            let plan = tournament.plan_qualified_stage(template).unwrap();
            let applied = tournament.apply_qualified_stage(plan).unwrap();
            assert_eq!(
                serde_json::to_value(&applied).unwrap(),
                expected_qualification
            );

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
            assert_eq!(
                stage.config().swiss_policy.is_some(),
                format == TournamentFormat::Swiss
            );
            assert_eq!(stage.status(), StageStatus::Active);
            let matches: Vec<_> = stage
                .rounds()
                .iter()
                .flat_map(|round| &round.matches)
                .collect();
            assert!(!matches.is_empty());
            assert!(
                matches
                    .iter()
                    .all(|match_| { match_.status == MatchStatus::Pending && !match_.started })
            );
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
            stage_qualification_slot("stage-a", StageQualificationSource::FinishedStageWinner, 1),
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
                StageQualificationBlocker::StandingRankUnavailable {
                    source_stage_id: StageId::from("stage"),
                    team_id: TeamId::from("t1"),
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
            QualifiedStagePlanError::QualificationBlocked(StageQualificationBlocker::DroppedTeam {
                source_stage_id: StageId::from("stage"),
                team_id: TeamId::from("t1"),
            }),
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

        let source_tournament = stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);
        let foreign_plan = source_tournament.plan_qualified_stage(template).unwrap();
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
                MatchResult::win_with_games(original_winner.clone(), original_loser.clone(), 2, 1),
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
        let mut tournament = stage_qualification_two_stage_winners(TournamentFormat::RoundRobin);
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
