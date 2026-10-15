//! 純 Rust 的競賽報名與設定狀態核心。
//!
//! 這個模組只管理賽事公告、隊伍名單、資料完成確認與牌組鎖定。它不建立
//! 遊戲規則或牌組驗證的副本，而是把既有 `OfficialRules` 當成驗證邊界。
//! `RegistrationState` 的可變資料刻意不實作 `Serialize`；對外只能取得公開
//! 視圖或當事人的私人視圖，避免把牌組卡表意外送進公開 DTO。

use crate::domain::{
    GameError, GameSetup, PERSONAL_DECK_MODULE_ID, Player, PlayerDeckList, PlayerId, RuleModuleId,
    RuleVersion, RulesetId, TeamHp, TeamId,
};
use crate::rules::{DeckListSource, OfficialRules};
use crate::tournament::{CompetitionAnnouncementPlan, validate_competition_announcement_plan};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

/// 外部可信時間，固定以 Unix 毫秒表示；核心不讀取牆上時鐘。
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrustedTimestamp(u64);

impl TrustedTimestamp {
    pub const fn from_millis(value: u64) -> Self {
        Self(value)
    }

    pub const fn as_millis(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegistrationPlayerId(String);

impl RegistrationPlayerId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct RegistrationTeamId(String);

impl RegistrationTeamId {
    pub fn new(id: impl Into<String>) -> Self {
        Self(id.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompetitionAnnouncement {
    pub organizer: RegistrationPlayerId,
    pub rule_version: RuleVersion,
    pub ruleset: RulesetId,
    pub enabled_rule_modules: Vec<RuleModuleId>,
    pub team_size: u16,
    pub initial_hp: i32,
    pub registration_mode: RegistrationMode,
    pub personal_decks: PersonalDeckPolicy,
    pub registration_deadline: TrustedTimestamp,
    pub check_in_deadline: TrustedTimestamp,
    pub plan: CompetitionAnnouncementPlan,
}

/// 純設定解析只接受規則選擇，不接受生命、公告或任何持久化狀態。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompetitionRulesSettings {
    pub rule_version: RuleVersion,
    pub ruleset: RulesetId,
    pub enabled_rule_modules: Vec<RuleModuleId>,
    pub team_size: u16,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedCompetitionSettings {
    pub initial_hp: i32,
}

/// 用實際等大兩隊與交替座次配置正式規則；不修改既有公告／報名 validator。
pub fn resolve_competition_settings(
    settings: &CompetitionRulesSettings,
) -> Result<ResolvedCompetitionSettings, RegistrationError> {
    let rules = OfficialRules::new();
    rules
        .resolve_version_modules(
            settings.rule_version,
            Some(settings.enabled_rule_modules.clone()),
        )
        .map_err(|error| RegistrationError::RulesValidation { error })?;
    let unavailable = || RegistrationError::InvalidAnnouncement {
        reason: "規則設定座次資源無法配置".to_string(),
    };
    // 用 usize 的受檢運算，不把 u16 的雙倍溢位當作新的 domain 人數上限。
    let player_count = usize::from(settings.team_size)
        .checked_mul(2)
        .ok_or_else(unavailable)?;
    let teams = [TeamId::new("settings-a"), TeamId::new("settings-b")];
    let mut players = Vec::new();
    let mut turn_order = Vec::new();
    players
        .try_reserve_exact(player_count)
        .map_err(|_| unavailable())?;
    turn_order
        .try_reserve_exact(player_count)
        .map_err(|_| unavailable())?;
    for index in 0..settings.team_size {
        for team in &teams {
            let id = PlayerId::new(format!("{}-{index}", team.as_str()));
            players.push(Player {
                id: id.clone(),
                team: team.clone(),
            });
            turn_order.push(id);
        }
    }
    // 空牌組的 preflight 僅重用 seating／ruleset 驗證；這裡的佔位生命不作輸出。
    // personal-deck 與其依賴已在上方驗證，正式牌組由下方完整 Rules setup 配置。
    let seating = GameSetup {
        rule_version: settings.rule_version,
        ruleset: settings.ruleset.clone(),
        enabled_rule_modules: Vec::new(),
        players,
        turn_order,
        hp: teams
            .iter()
            .map(|team| TeamHp {
                team: team.clone(),
                hp: 1,
            })
            .collect(),
        card_defs: Vec::new(),
        card_instances: Vec::new(),
        deck_lists: Vec::new(),
        hand_limit: 5,
        base_draw: 2,
    };
    rules
        .validate_setup(&seating)
        .map_err(|error| RegistrationError::RulesValidation { error })?;
    let setup = rules
        .configure_versioned_game_with_decks(
            settings.rule_version,
            seating.players,
            seating.turn_order,
            settings.enabled_rule_modules.clone(),
            Vec::new(),
        )
        .map_err(|error| RegistrationError::RulesValidation { error })?;
    let initial_hp = setup.hp.first().map(|hp| hp.hp).filter(|hp| *hp > 0);
    if setup.ruleset != settings.ruleset
        || setup.hp.len() != teams.len()
        || initial_hp.is_none()
        || teams
            .iter()
            .any(|team| setup.hp.iter().filter(|hp| &hp.team == team).count() != 1)
        || setup.hp.iter().any(|hp| Some(hp.hp) != initial_hp)
    {
        return Err(RegistrationError::InvalidAnnouncement {
            reason: "正式規則生命配置不一致".to_string(),
        });
    }
    Ok(ResolvedCompetitionSettings {
        initial_hp: initial_hp.ok_or_else(|| RegistrationError::InvalidAnnouncement {
            reason: "正式規則生命配置不一致".to_string(),
        })?,
    })
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RegistrationMode {
    Public,
    InviteOnly {
        invited_captains: Vec<RegistrationPlayerId>,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PersonalDeckPolicy {
    Disabled,
    Required,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RegistrationLifecycle {
    Draft,
    Open,
    Locked,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TeamStatus {
    Registered,
    CheckedIn,
    Withdrawn,
    NotCheckedIn,
}

/// 隊伍在資料凍結後未納入賽程的公開原因；不帶缺件玩家或牌組內容。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RegistrationExclusionReason {
    MissingMemberConfirmations,
    MissingRequiredDecks,
    MissingCaptainConfirmation,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RegistrationActor {
    Organizer(RegistrationPlayerId),
    Player(RegistrationPlayerId),
}

/// 所有會改變狀態的操作都帶有命令識別、預期版本與顯式操作者。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationCommand {
    pub command_id: String,
    pub expected_revision: u64,
    pub actor: RegistrationActor,
    pub action: RegistrationAction,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum RegistrationAction {
    Open,
    RegisterTeam {
        team_id: RegistrationTeamId,
        captain: RegistrationPlayerId,
        members: Vec<RegistrationPlayerId>,
    },
    ReplaceRoster {
        team_id: RegistrationTeamId,
        expected_roster_version: u64,
        members: Vec<RegistrationPlayerId>,
    },
    ConfirmMembership {
        team_id: RegistrationTeamId,
        roster_version: u64,
    },
    SubmitDeck {
        team_id: RegistrationTeamId,
        roster_version: u64,
        name: String,
        cards: Vec<String>,
    },
    CheckInTeam {
        team_id: RegistrationTeamId,
        roster_version: u64,
    },
    WithdrawTeam {
        team_id: RegistrationTeamId,
    },
    Lock {},
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegistrationCommandResult {
    pub revision: u64,
    pub public_view: PublicRegistrationView,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicRegistrationView {
    pub announcement: PublicAnnouncement,
    pub lifecycle: RegistrationLifecycle,
    pub revision: u64,
    pub teams: Vec<PublicTeamView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicAnnouncement {
    pub organizer: RegistrationPlayerId,
    pub rule_version: RuleVersion,
    pub ruleset: RulesetId,
    pub enabled_rule_modules: Vec<RuleModuleId>,
    pub team_size: u16,
    pub initial_hp: i32,
    pub registration_mode: PublicRegistrationMode,
    pub personal_decks: PersonalDeckPolicy,
    pub registration_deadline: TrustedTimestamp,
    pub check_in_deadline: TrustedTimestamp,
    pub plan: CompetitionAnnouncementPlan,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum PublicRegistrationMode {
    Public,
    InviteOnly,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PublicTeamView {
    pub team_id: RegistrationTeamId,
    pub captain: RegistrationPlayerId,
    pub members: Vec<RegistrationPlayerId>,
    pub roster_version: u64,
    pub confirmed_members: Vec<RegistrationPlayerId>,
    pub status: TeamStatus,
    pub exclusion_reasons: Vec<RegistrationExclusionReason>,
}

/// 玩家只能取得自己隊伍的私人資料；其他隊伍的牌組永遠不會進入此視圖。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateRegistrationView {
    pub public_view: PublicRegistrationView,
    pub own_team: Option<PrivateTeamView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateTeamView {
    pub team_id: RegistrationTeamId,
    pub captain: RegistrationPlayerId,
    pub members: Vec<RegistrationPlayerId>,
    pub roster_version: u64,
    pub confirmed_members: Vec<RegistrationPlayerId>,
    pub status: TeamStatus,
    pub own_deck: Option<PrivateDeckView>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PrivateDeckView {
    pub player: RegistrationPlayerId,
    pub name: String,
    pub cards: Vec<String>,
}

/// Lock 後交給可信開房／賽程層的私有輸出。它不是公開視圖，呼叫端必須把
/// 回傳值放在受控的伺服器邊界，不可直接送給觀眾或其他參賽者。
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedRoster {
    pub announcement: CompetitionAnnouncement,
    pub teams: Vec<LockedTeam>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedTeam {
    pub team_id: RegistrationTeamId,
    pub captain: RegistrationPlayerId,
    pub members: Vec<RegistrationPlayerId>,
    pub roster_version: u64,
    pub decks: Vec<LockedPlayerDeck>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LockedPlayerDeck {
    pub player: RegistrationPlayerId,
    pub name: String,
    pub cards: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum RegistrationError {
    InvalidAnnouncement {
        reason: String,
    },
    RulesValidation {
        error: GameError,
    },
    InvalidCommandId,
    CommandAlreadyUsed {
        command_id: String,
    },
    CommandIdConflict {
        command_id: String,
    },
    RevisionConflict {
        expected: u64,
        actual: u64,
    },
    Unauthorized,
    InvalidLifecycle {
        expected: RegistrationLifecycle,
        actual: RegistrationLifecycle,
    },
    InvalidTimestamp {
        reason: String,
    },
    TimestampWentBackwards {
        previous: TrustedTimestamp,
        actual: TrustedTimestamp,
    },
    InvalidTeam {
        reason: String,
    },
    DuplicateTeam,
    UnknownTeam,
    RosterVersionMismatch {
        expected: u64,
        actual: u64,
    },
    NotTeamMember,
    NotCaptain,
    MissingConfirmations {
        players: Vec<RegistrationPlayerId>,
    },
    MissingDecks {
        players: Vec<RegistrationPlayerId>,
    },
    PersonalDecksDisabled,
    InvalidPersonalDeck {
        player: RegistrationPlayerId,
    },
    TeamAlreadyCheckedIn,
    TeamWithdrawn,
    NotInvited,
}

#[derive(Clone, Debug)]
struct TeamRecord {
    team_id: RegistrationTeamId,
    captain: RegistrationPlayerId,
    members: Vec<RegistrationPlayerId>,
    roster_version: u64,
    confirmed_members: BTreeSet<RegistrationPlayerId>,
    decks: BTreeMap<RegistrationPlayerId, PlayerDeckList>,
    status: TeamStatus,
}

#[derive(Clone, Debug)]
struct CommandReceipt {
    fingerprint: String,
    result: RegistrationCommandResult,
}

/// 報名狀態本身不公開序列化；持久化層應保存命令／收據或在受控邊界建立
/// 專用 DTO。這讓內部牌組欄位不可能因 `derive(Serialize)` 混入公開回應。
#[derive(Clone, Debug)]
pub struct RegistrationState {
    announcement: CompetitionAnnouncement,
    lifecycle: RegistrationLifecycle,
    revision: u64,
    last_observed_time: Option<TrustedTimestamp>,
    teams: BTreeMap<RegistrationTeamId, TeamRecord>,
    processed_commands: BTreeMap<String, CommandReceipt>,
}

impl RegistrationState {
    pub fn new(announcement: CompetitionAnnouncement) -> Result<Self, RegistrationError> {
        validate_announcement(&announcement)?;
        Ok(Self {
            announcement,
            lifecycle: RegistrationLifecycle::Draft,
            revision: 0,
            last_observed_time: None,
            teams: BTreeMap::new(),
            processed_commands: BTreeMap::new(),
        })
    }

    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn lifecycle(&self) -> RegistrationLifecycle {
        self.lifecycle
    }

    pub fn public_view(&self) -> PublicRegistrationView {
        PublicRegistrationView {
            announcement: public_announcement(&self.announcement),
            lifecycle: self.lifecycle,
            revision: self.revision,
            teams: self
                .teams
                .values()
                .map(|team| public_team(team, self.announcement.personal_decks))
                .collect(),
        }
    }

    pub fn private_view(&self, actor: &RegistrationPlayerId) -> PrivateRegistrationView {
        let own_team = self
            .teams
            .values()
            .filter(|team| team.status != TeamStatus::Withdrawn)
            .find(|team| team.members.iter().any(|member| member == actor))
            .or_else(|| {
                self.teams
                    .values()
                    .find(|team| team.members.iter().any(|member| member == actor))
            })
            .map(|team| PrivateTeamView {
                team_id: team.team_id.clone(),
                captain: team.captain.clone(),
                members: team.members.clone(),
                roster_version: team.roster_version,
                confirmed_members: team.confirmed_members.iter().cloned().collect(),
                status: team.status,
                own_deck: team.decks.get(actor).map(|deck| PrivateDeckView {
                    player: actor.clone(),
                    name: deck.name.clone(),
                    cards: deck
                        .cards
                        .iter()
                        .map(|card| card.as_str().to_string())
                        .collect(),
                }),
            });
        PrivateRegistrationView {
            public_view: self.public_view(),
            own_team,
        }
    }

    /// 先查收據，再在 clone 上執行命令；任何錯誤都不會改變本狀態。
    pub fn execute(
        &mut self,
        command: RegistrationCommand,
        trusted_now: TrustedTimestamp,
    ) -> Result<RegistrationCommandResult, RegistrationError> {
        if command.command_id.is_empty() {
            return Err(RegistrationError::InvalidCommandId);
        }
        let fingerprint = serde_json::to_string(&command).map_err(|error| {
            RegistrationError::InvalidAnnouncement {
                reason: format!("cannot fingerprint command: {error}"),
            }
        })?;
        if let Some(receipt) = self.processed_commands.get(&command.command_id) {
            if receipt.fingerprint == fingerprint {
                return Ok(receipt.result.clone());
            }
            return Err(RegistrationError::CommandIdConflict {
                command_id: command.command_id,
            });
        }
        if command.expected_revision != self.revision {
            return Err(RegistrationError::RevisionConflict {
                expected: command.expected_revision,
                actual: self.revision,
            });
        }
        if let Some(previous) = self.last_observed_time
            && trusted_now < previous
        {
            return Err(RegistrationError::TimestampWentBackwards {
                previous,
                actual: trusted_now,
            });
        }

        let mut candidate = self.clone();
        candidate.apply_once(&command, trusted_now)?;
        candidate.revision += 1;
        candidate.last_observed_time = Some(trusted_now);
        let result = RegistrationCommandResult {
            revision: candidate.revision,
            public_view: candidate.public_view(),
        };
        candidate.processed_commands.insert(
            command.command_id,
            CommandReceipt {
                fingerprint,
                result: result.clone(),
            },
        );
        *self = candidate;
        Ok(result)
    }

    pub fn export_locked_roster(&self) -> Result<LockedRoster, RegistrationError> {
        if self.lifecycle != RegistrationLifecycle::Locked {
            return Err(RegistrationError::InvalidLifecycle {
                expected: RegistrationLifecycle::Locked,
                actual: self.lifecycle,
            });
        }
        for team in self
            .teams
            .values()
            .filter(|team| team.status == TeamStatus::CheckedIn)
        {
            self.validate_locked_team(team)?;
        }
        Ok(LockedRoster {
            announcement: self.announcement.clone(),
            teams: self
                .teams
                .values()
                .filter(|team| team.status == TeamStatus::CheckedIn)
                .map(|team| LockedTeam {
                    team_id: team.team_id.clone(),
                    captain: team.captain.clone(),
                    members: team.members.clone(),
                    roster_version: team.roster_version,
                    decks: team
                        .members
                        .iter()
                        .filter_map(|player| team.decks.get(player))
                        .map(|deck| LockedPlayerDeck {
                            player: RegistrationPlayerId::new(deck.player.as_str()),
                            name: deck.name.clone(),
                            cards: deck
                                .cards
                                .iter()
                                .map(|card| card.as_str().to_string())
                                .collect(),
                        })
                        .collect(),
                })
                .collect(),
        })
    }

    fn apply_once(
        &mut self,
        command: &RegistrationCommand,
        trusted_now: TrustedTimestamp,
    ) -> Result<(), RegistrationError> {
        match &command.action {
            RegistrationAction::Open => {
                self.require_organizer(&command.actor)?;
                self.require_lifecycle(RegistrationLifecycle::Draft)?;
                self.lifecycle = RegistrationLifecycle::Open;
            }
            RegistrationAction::RegisterTeam {
                team_id,
                captain,
                members,
            } => {
                self.require_player(&command.actor, captain)?;
                self.require_lifecycle(RegistrationLifecycle::Open)?;
                self.require_before(
                    trusted_now,
                    self.announcement.registration_deadline,
                    "registration",
                )?;
                self.require_invitation(captain)?;
                if self.teams.contains_key(team_id) {
                    return Err(RegistrationError::DuplicateTeam);
                }
                self.validate_roster(team_id, captain, members, None)?;
                self.teams.insert(
                    team_id.clone(),
                    TeamRecord {
                        team_id: team_id.clone(),
                        captain: captain.clone(),
                        members: members.clone(),
                        roster_version: 1,
                        confirmed_members: BTreeSet::new(),
                        decks: BTreeMap::new(),
                        status: TeamStatus::Registered,
                    },
                );
            }
            RegistrationAction::ReplaceRoster {
                team_id,
                expected_roster_version,
                members,
            } => {
                self.require_lifecycle(RegistrationLifecycle::Open)?;
                self.require_before(
                    trusted_now,
                    self.announcement.registration_deadline,
                    "registration",
                )?;
                let team = self
                    .teams
                    .get(team_id)
                    .ok_or(RegistrationError::UnknownTeam)?;
                self.require_captain(&command.actor, &team.captain)?;
                if team.status == TeamStatus::Withdrawn {
                    return Err(RegistrationError::TeamWithdrawn);
                }
                if team.roster_version != *expected_roster_version {
                    return Err(RegistrationError::RosterVersionMismatch {
                        expected: *expected_roster_version,
                        actual: team.roster_version,
                    });
                }
                let captain = team.captain.clone();
                self.validate_roster(team_id, &captain, members, Some(team_id))?;
                let team = self.teams.get_mut(team_id).expect("team checked above");
                team.members = members.clone();
                team.roster_version += 1;
                team.confirmed_members.clear();
                team.decks.clear();
                team.status = TeamStatus::Registered;
            }
            RegistrationAction::ConfirmMembership {
                team_id,
                roster_version,
            } => {
                self.require_lifecycle(RegistrationLifecycle::Open)?;
                self.require_before(
                    trusted_now,
                    self.announcement.registration_deadline,
                    "registration",
                )?;
                let actor = self.player_actor(&command.actor)?;
                let team = self
                    .teams
                    .get_mut(team_id)
                    .ok_or(RegistrationError::UnknownTeam)?;
                if team.status == TeamStatus::Withdrawn {
                    return Err(RegistrationError::TeamWithdrawn);
                }
                if team.roster_version != *roster_version {
                    return Err(RegistrationError::RosterVersionMismatch {
                        expected: *roster_version,
                        actual: team.roster_version,
                    });
                }
                if !team.members.contains(actor) {
                    return Err(RegistrationError::NotTeamMember);
                }
                team.confirmed_members.insert(actor.clone());
            }
            RegistrationAction::SubmitDeck {
                team_id,
                roster_version,
                name,
                cards,
            } => {
                self.require_lifecycle(RegistrationLifecycle::Open)?;
                self.require_before(
                    trusted_now,
                    self.announcement.registration_deadline,
                    "registration",
                )?;
                if self.announcement.personal_decks == PersonalDeckPolicy::Disabled {
                    return Err(RegistrationError::PersonalDecksDisabled);
                }
                let actor = self.player_actor(&command.actor)?;
                let team = self
                    .teams
                    .get(team_id)
                    .ok_or(RegistrationError::UnknownTeam)?;
                if team.status == TeamStatus::Withdrawn {
                    return Err(RegistrationError::TeamWithdrawn);
                }
                if team.roster_version != *roster_version {
                    return Err(RegistrationError::RosterVersionMismatch {
                        expected: *roster_version,
                        actual: team.roster_version,
                    });
                }
                if !team.members.contains(actor) {
                    return Err(RegistrationError::NotTeamMember);
                }
                let deck = validate_personal_deck(actor, name, cards)?;
                let team = self.teams.get_mut(team_id).expect("team checked above");
                // 隊長確認後重交牌組，須在報名截止前重新確認資料完成；
                // 名單確認仍可沿用，但這不是房間到場或準備。
                if team.status == TeamStatus::CheckedIn {
                    team.status = TeamStatus::Registered;
                }
                team.decks.insert(actor.clone(), deck);
            }
            RegistrationAction::CheckInTeam {
                team_id,
                roster_version,
            } => {
                self.require_lifecycle(RegistrationLifecycle::Open)?;
                self.require_before(
                    trusted_now,
                    self.announcement.registration_deadline,
                    "registration",
                )?;
                let team = self
                    .teams
                    .get(team_id)
                    .ok_or(RegistrationError::UnknownTeam)?;
                self.require_captain(&command.actor, &team.captain)?;
                if team.status == TeamStatus::Withdrawn {
                    return Err(RegistrationError::TeamWithdrawn);
                }
                if team.status == TeamStatus::CheckedIn {
                    return Err(RegistrationError::TeamAlreadyCheckedIn);
                }
                if team.roster_version != *roster_version {
                    return Err(RegistrationError::RosterVersionMismatch {
                        expected: *roster_version,
                        actual: team.roster_version,
                    });
                }
                let missing_confirmations = team
                    .members
                    .iter()
                    .filter(|member| !team.confirmed_members.contains(*member))
                    .cloned()
                    .collect::<Vec<_>>();
                if !missing_confirmations.is_empty() {
                    return Err(RegistrationError::MissingConfirmations {
                        players: missing_confirmations,
                    });
                }
                if self.announcement.personal_decks == PersonalDeckPolicy::Required {
                    let missing_decks = team
                        .members
                        .iter()
                        .filter(|member| !team.decks.contains_key(*member))
                        .cloned()
                        .collect::<Vec<_>>();
                    if !missing_decks.is_empty() {
                        return Err(RegistrationError::MissingDecks {
                            players: missing_decks,
                        });
                    }
                }
                self.teams
                    .get_mut(team_id)
                    .expect("team checked above")
                    .status = TeamStatus::CheckedIn;
            }
            RegistrationAction::WithdrawTeam { team_id } => {
                self.require_lifecycle(RegistrationLifecycle::Open)?;
                self.require_before(
                    trusted_now,
                    self.announcement.registration_deadline,
                    "registration",
                )?;
                let team = self
                    .teams
                    .get(team_id)
                    .ok_or(RegistrationError::UnknownTeam)?;
                match &command.actor {
                    RegistrationActor::Organizer(_) => self.require_organizer(&command.actor)?,
                    RegistrationActor::Player(player) if player == &team.captain => {}
                    RegistrationActor::Player(_) => return Err(RegistrationError::NotCaptain),
                }
                if team.status == TeamStatus::CheckedIn {
                    return Err(RegistrationError::TeamAlreadyCheckedIn);
                }
                if team.status == TeamStatus::Withdrawn {
                    return Err(RegistrationError::TeamWithdrawn);
                }
                self.teams
                    .get_mut(team_id)
                    .expect("team checked above")
                    .status = TeamStatus::Withdrawn;
            }
            RegistrationAction::Lock {} => {
                self.require_organizer(&command.actor)?;
                self.require_lifecycle(RegistrationLifecycle::Open)?;
                // 截止起資料已不能修改；Locked 是後續觀測提交，不讀牆上時鐘。
                if trusted_now < self.announcement.registration_deadline {
                    return Err(RegistrationError::InvalidTimestamp {
                        reason: "registration deadline has not passed".to_string(),
                    });
                }
                for team in self
                    .teams
                    .values()
                    .filter(|team| team.status == TeamStatus::CheckedIn)
                {
                    self.validate_locked_team(team)?;
                }
                for team in self.teams.values_mut() {
                    if team.status == TeamStatus::Registered {
                        team.status = TeamStatus::NotCheckedIn;
                    }
                }
                self.lifecycle = RegistrationLifecycle::Locked;
            }
        }
        Ok(())
    }

    fn require_lifecycle(&self, expected: RegistrationLifecycle) -> Result<(), RegistrationError> {
        if self.lifecycle != expected {
            return Err(RegistrationError::InvalidLifecycle {
                expected,
                actual: self.lifecycle,
            });
        }
        Ok(())
    }

    fn require_organizer(&self, actor: &RegistrationActor) -> Result<(), RegistrationError> {
        if matches!(actor, RegistrationActor::Organizer(id) if id == &self.announcement.organizer) {
            Ok(())
        } else {
            Err(RegistrationError::Unauthorized)
        }
    }

    fn require_player<'a>(
        &self,
        actor: &'a RegistrationActor,
        expected: &RegistrationPlayerId,
    ) -> Result<(), RegistrationError> {
        match actor {
            RegistrationActor::Player(player) if player == expected => Ok(()),
            _ => Err(RegistrationError::Unauthorized),
        }
    }

    fn require_captain(
        &self,
        actor: &RegistrationActor,
        captain: &RegistrationPlayerId,
    ) -> Result<(), RegistrationError> {
        self.require_player(actor, captain)
            .map_err(|_| RegistrationError::NotCaptain)
    }

    fn player_actor<'a>(
        &self,
        actor: &'a RegistrationActor,
    ) -> Result<&'a RegistrationPlayerId, RegistrationError> {
        match actor {
            RegistrationActor::Player(player) => Ok(player),
            RegistrationActor::Organizer(_) => Err(RegistrationError::Unauthorized),
        }
    }

    fn require_before(
        &self,
        trusted_now: TrustedTimestamp,
        deadline: TrustedTimestamp,
        kind: &str,
    ) -> Result<(), RegistrationError> {
        if trusted_now >= deadline {
            return Err(RegistrationError::InvalidTimestamp {
                reason: format!("{kind} deadline has passed"),
            });
        }
        Ok(())
    }

    fn require_invitation(&self, captain: &RegistrationPlayerId) -> Result<(), RegistrationError> {
        match &self.announcement.registration_mode {
            RegistrationMode::Public => Ok(()),
            RegistrationMode::InviteOnly { invited_captains }
                if invited_captains.contains(captain) =>
            {
                Ok(())
            }
            RegistrationMode::InviteOnly { .. } => Err(RegistrationError::NotInvited),
        }
    }

    fn validate_roster(
        &self,
        team_id: &RegistrationTeamId,
        captain: &RegistrationPlayerId,
        members: &[RegistrationPlayerId],
        same_team: Option<&RegistrationTeamId>,
    ) -> Result<(), RegistrationError> {
        if team_id.as_str().is_empty() || captain.as_str().is_empty() {
            return Err(RegistrationError::InvalidTeam {
                reason: "team ID and captain must not be empty".to_string(),
            });
        }
        if members.len() != self.announcement.team_size as usize {
            return Err(RegistrationError::InvalidTeam {
                reason: format!(
                    "team must contain exactly {} members",
                    self.announcement.team_size
                ),
            });
        }
        let unique = members.iter().collect::<BTreeSet<_>>();
        if unique.len() != members.len() || !unique.contains(captain) {
            return Err(RegistrationError::InvalidTeam {
                reason: "members must be unique and include the captain".to_string(),
            });
        }
        if members.iter().any(|member| member.as_str().is_empty()) {
            return Err(RegistrationError::InvalidTeam {
                reason: "member ID must not be empty".to_string(),
            });
        }
        if self.teams.values().any(|team| {
            if same_team.is_some_and(|current| current == &team.team_id) {
                return false;
            }
            team.status != TeamStatus::Withdrawn
                && team.members.iter().any(|member| members.contains(member))
        }) {
            return Err(RegistrationError::InvalidTeam {
                reason: "a player cannot belong to more than one team".to_string(),
            });
        }
        Ok(())
    }

    fn validate_locked_team(&self, team: &TeamRecord) -> Result<(), RegistrationError> {
        let missing_confirmations = team
            .members
            .iter()
            .filter(|member| !team.confirmed_members.contains(*member))
            .cloned()
            .collect::<Vec<_>>();
        if !missing_confirmations.is_empty() {
            return Err(RegistrationError::MissingConfirmations {
                players: missing_confirmations,
            });
        }
        if self.announcement.personal_decks == PersonalDeckPolicy::Required {
            let missing_decks = team
                .members
                .iter()
                .filter(|member| !team.decks.contains_key(*member))
                .cloned()
                .collect::<Vec<_>>();
            if !missing_decks.is_empty() {
                return Err(RegistrationError::MissingDecks {
                    players: missing_decks,
                });
            }
            if team.members.iter().any(|member| {
                let Some(deck) = team.decks.get(member) else {
                    return true;
                };
                !stored_personal_deck_is_valid(member, deck)
            }) {
                return Err(RegistrationError::InvalidPersonalDeck {
                    player: team
                        .members
                        .iter()
                        .find(|member| {
                            team.decks
                                .get(*member)
                                .is_some_and(|deck| !stored_personal_deck_is_valid(member, deck))
                        })
                        .cloned()
                        .expect("invalid deck exists"),
                });
            }
        }
        Ok(())
    }
}

pub fn validate_announcement(
    announcement: &CompetitionAnnouncement,
) -> Result<(), RegistrationError> {
    if announcement.organizer.as_str().is_empty() {
        return Err(RegistrationError::InvalidAnnouncement {
            reason: "organizer must not be empty".to_string(),
        });
    }
    if announcement.team_size == 0 {
        return Err(RegistrationError::InvalidAnnouncement {
            reason: "team size must be positive".to_string(),
        });
    }
    if announcement.initial_hp <= 0 {
        return Err(RegistrationError::InvalidAnnouncement {
            reason: "initial HP must be positive".to_string(),
        });
    }
    if announcement.registration_deadline > announcement.check_in_deadline {
        return Err(RegistrationError::InvalidAnnouncement {
            reason: "registration deadline must not be after check-in deadline".to_string(),
        });
    }
    if announcement.registration_deadline.as_millis() % 60_000 != 0
        || announcement.check_in_deadline.as_millis() % 60_000 != 0
    {
        return Err(RegistrationError::InvalidAnnouncement {
            reason: "registration deadline and competition start time must use whole minutes"
                .to_string(),
        });
    }
    validate_competition_announcement_plan(&announcement.plan)
        .map_err(|reason| RegistrationError::InvalidAnnouncement { reason })?;
    match &announcement.registration_mode {
        RegistrationMode::Public => {}
        RegistrationMode::InviteOnly { invited_captains } => {
            if invited_captains.is_empty()
                || invited_captains
                    .iter()
                    .any(|captain| captain.as_str().is_empty())
                || invited_captains.iter().collect::<BTreeSet<_>>().len() != invited_captains.len()
            {
                return Err(RegistrationError::InvalidAnnouncement {
                    reason: "invited captains must be non-empty and unique".to_string(),
                });
            }
        }
    }
    let uses_personal_decks = announcement
        .enabled_rule_modules
        .iter()
        .any(|module| module.as_str() == PERSONAL_DECK_MODULE_ID);
    if uses_personal_decks != (announcement.personal_decks == PersonalDeckPolicy::Required) {
        return Err(RegistrationError::InvalidAnnouncement {
            reason: "personal deck policy must match the Personal Deck Rule Module".to_string(),
        });
    }

    // 用既有 OfficialRules 建立最小合法 setup，再讓同一驗證器檢查 Ruleset、版本、
    // 模組相容性與依賴。這裡不重寫規則表或牌組限制。
    let rules = OfficialRules::new();
    let first_player = Player {
        id: PlayerId::new("__registration_validation_first__"),
        team: TeamId::new("__registration_validation_first_team__"),
    };
    let second_player = Player {
        id: PlayerId::new("__registration_validation_second__"),
        team: TeamId::new("__registration_validation_second_team__"),
    };
    let mut setup = rules
        .configure_versioned_game_with_decks(
            announcement.rule_version,
            vec![first_player.clone(), second_player.clone()],
            vec![first_player.id.clone(), second_player.id.clone()],
            announcement.enabled_rule_modules.clone(),
            Vec::new(),
        )
        .map_err(|error| RegistrationError::RulesValidation { error })?;
    setup.ruleset = announcement.ruleset.clone();
    for hp in &mut setup.hp {
        hp.hp = announcement.initial_hp;
    }
    rules
        .validate_setup(&setup)
        .map_err(|error| RegistrationError::RulesValidation { error })
}

fn validate_personal_deck(
    player: &RegistrationPlayerId,
    name: &str,
    cards: &[String],
) -> Result<PlayerDeckList, RegistrationError> {
    let domain_player = PlayerId::new(player.as_str());
    let candidate = PlayerDeckList {
        player: domain_player.clone(),
        name: name.to_string(),
        cards: cards
            .iter()
            .map(|card| crate::domain::CardDefId::new(card.clone()))
            .collect(),
    };
    let resolved =
        OfficialRules::new().resolve_personal_deck(domain_player, Some(candidate.clone()));
    if resolved.source != DeckListSource::Custom
        || !resolved
            .candidate_validation
            .as_ref()
            .is_some_and(|validation| validation.valid)
    {
        return Err(RegistrationError::InvalidPersonalDeck {
            player: player.clone(),
        });
    }
    Ok(candidate)
}

fn stored_personal_deck_is_valid(player: &RegistrationPlayerId, deck: &PlayerDeckList) -> bool {
    let resolved = OfficialRules::new()
        .resolve_personal_deck(PlayerId::new(player.as_str()), Some(deck.clone()));
    resolved.source == DeckListSource::Custom
        && resolved
            .candidate_validation
            .as_ref()
            .is_some_and(|validation| validation.valid)
}

fn public_announcement(announcement: &CompetitionAnnouncement) -> PublicAnnouncement {
    PublicAnnouncement {
        organizer: announcement.organizer.clone(),
        rule_version: announcement.rule_version,
        ruleset: announcement.ruleset.clone(),
        enabled_rule_modules: announcement.enabled_rule_modules.clone(),
        team_size: announcement.team_size,
        initial_hp: announcement.initial_hp,
        registration_mode: match &announcement.registration_mode {
            RegistrationMode::Public => PublicRegistrationMode::Public,
            RegistrationMode::InviteOnly { .. } => PublicRegistrationMode::InviteOnly,
        },
        personal_decks: announcement.personal_decks,
        registration_deadline: announcement.registration_deadline,
        check_in_deadline: announcement.check_in_deadline,
        plan: announcement.plan.clone(),
    }
}

fn public_team(team: &TeamRecord, personal_decks: PersonalDeckPolicy) -> PublicTeamView {
    let mut exclusion_reasons = Vec::new();
    // NotCheckedIn 只由 Lock 分類；以同一份不可變資料推導，不另存資格副本。
    if team.status == TeamStatus::NotCheckedIn {
        if team
            .members
            .iter()
            .any(|member| !team.confirmed_members.contains(member))
        {
            exclusion_reasons.push(RegistrationExclusionReason::MissingMemberConfirmations);
        }
        if personal_decks == PersonalDeckPolicy::Required
            && team
                .members
                .iter()
                .any(|member| !team.decks.contains_key(member))
        {
            exclusion_reasons.push(RegistrationExclusionReason::MissingRequiredDecks);
        }
        // 即使其餘欄位全齊，未完成當前版本的 CheckInTeam 仍不能納入賽程。
        exclusion_reasons.push(RegistrationExclusionReason::MissingCaptainConfirmation);
    }
    PublicTeamView {
        team_id: team.team_id.clone(),
        captain: team.captain.clone(),
        members: team.members.clone(),
        roster_version: team.roster_version,
        confirmed_members: team.confirmed_members.iter().cloned().collect(),
        status: team.status,
        exclusion_reasons,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{PERSONAL_DECK_MODULE_ID, RuleModuleId, RuleVersion, RulesetId};

    fn player(id: &str) -> RegistrationPlayerId {
        RegistrationPlayerId::new(id)
    }

    fn team(id: &str) -> RegistrationTeamId {
        RegistrationTeamId::new(id)
    }

    fn now(milliseconds: u64) -> TrustedTimestamp {
        TrustedTimestamp::from_millis(milliseconds)
    }

    fn announcement() -> CompetitionAnnouncement {
        CompetitionAnnouncement {
            organizer: player("organizer"),
            rule_version: RuleVersion::V5_16,
            ruleset: RulesetId::base(),
            enabled_rule_modules: Vec::new(),
            team_size: 2,
            initial_hp: 200,
            registration_mode: RegistrationMode::Public,
            personal_decks: PersonalDeckPolicy::Disabled,
            registration_deadline: now(60_000),
            check_in_deadline: now(120_000),
            plan: test_plan(),
        }
    }

    fn test_plan() -> CompetitionAnnouncementPlan {
        use crate::competition_clock::ClockConfig;
        use crate::competition_match::MatchFormat;
        use crate::tournament::{
            AnnouncedStageQualification, AnnouncedTournamentStage, CompetitionRankingPolicy,
            ExactRankingTiePolicy, InitialSeedPolicy, StageId, SwissPairingPolicy,
            TournamentFormat,
        };

        CompetitionAnnouncementPlan {
            initial_seed_policy: InitialSeedPolicy::RandomDrawAfterRegistrationDeadline,
            swiss_pairing_policy: SwissPairingPolicy::PublishedRulesWithPersistedRandomSeed,
            ranking_policy: CompetitionRankingPolicy::MatchPointsThenOpponentWinsThenOpponentLosses,
            exact_ranking_tie_policy:
                ExactRankingTiePolicy::OrganizerOrUnconflictedRefereeWithPublicReason,
            stages: vec![AnnouncedTournamentStage {
                stage_id: StageId::from("stage-1"),
                format: TournamentFormat::RoundRobin,
                match_format: MatchFormat::FixedGames(1),
                clock_config: ClockConfig {
                    main_time_ms: Some(300_000),
                    action_time_ms: Some(30_000),
                    out_of_turn_choice_time_ms: Some(30_000),
                },
                swiss_rounds: None,
                swiss_policy: None,
                qualification: AnnouncedStageQualification::AllEligibleTeams,
            }],
        }
    }

    fn command(
        id: &str,
        revision: u64,
        actor: RegistrationActor,
        action: RegistrationAction,
    ) -> RegistrationCommand {
        RegistrationCommand {
            command_id: id.to_string(),
            expected_revision: revision,
            actor,
            action,
        }
    }

    fn open(state: &mut RegistrationState) {
        state
            .execute(
                command(
                    "open",
                    0,
                    RegistrationActor::Organizer(player("organizer")),
                    RegistrationAction::Open,
                ),
                now(0),
            )
            .unwrap();
    }

    fn register(state: &mut RegistrationState) {
        state
            .execute(
                command(
                    "register-a",
                    1,
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::RegisterTeam {
                        team_id: team("a"),
                        captain: player("captain-a"),
                        members: vec![player("captain-a"), player("member-a")],
                    },
                ),
                now(99),
            )
            .unwrap();
    }

    #[test]
    fn announcement_uses_existing_rules_validation() {
        let mut invalid = announcement();
        invalid.enabled_rule_modules = vec![RuleModuleId::new("unknown")];
        assert!(matches!(
            RegistrationState::new(invalid),
            Err(RegistrationError::RulesValidation { .. })
        ));

        let mut invalid_ruleset = announcement();
        invalid_ruleset.ruleset = RulesetId::new("not-supported");
        assert!(matches!(
            RegistrationState::new(invalid_ruleset),
            Err(RegistrationError::RulesValidation { .. })
        ));
    }

    #[test]
    fn registration_requires_explicit_actor_and_prevents_duplicate_players() {
        let mut state = RegistrationState::new(announcement()).unwrap();
        assert!(matches!(
            state.execute(
                command(
                    "open",
                    0,
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::Open
                ),
                now(0)
            ),
            Err(RegistrationError::Unauthorized)
        ));
        open(&mut state);
        register(&mut state);
        let duplicate = state.execute(
            command(
                "register-b",
                2,
                RegistrationActor::Player(player("captain-b")),
                RegistrationAction::RegisterTeam {
                    team_id: team("b"),
                    captain: player("captain-b"),
                    members: vec![player("captain-b"), player("member-a")],
                },
            ),
            now(99),
        );
        assert!(matches!(
            duplicate,
            Err(RegistrationError::InvalidTeam { .. })
        ));
        assert_eq!(state.revision(), 2);
        assert_eq!(state.public_view().teams.len(), 1);
    }

    #[test]
    fn stale_roster_confirmation_is_rejected_and_roster_change_clears_confirmations() {
        let mut state = RegistrationState::new(announcement()).unwrap();
        open(&mut state);
        register(&mut state);
        state
            .execute(
                command(
                    "confirm-a",
                    2,
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::ConfirmMembership {
                        team_id: team("a"),
                        roster_version: 1,
                    },
                ),
                now(99),
            )
            .unwrap();
        state
            .execute(
                command(
                    "replace-a",
                    3,
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::ReplaceRoster {
                        team_id: team("a"),
                        expected_roster_version: 1,
                        members: vec![player("captain-a"), player("member-new")],
                    },
                ),
                now(99),
            )
            .unwrap();
        let stale = state.execute(
            command(
                "confirm-stale",
                4,
                RegistrationActor::Player(player("captain-a")),
                RegistrationAction::ConfirmMembership {
                    team_id: team("a"),
                    roster_version: 1,
                },
            ),
            now(99),
        );
        assert!(matches!(
            stale,
            Err(RegistrationError::RosterVersionMismatch {
                expected: 1,
                actual: 2
            })
        ));
        assert!(state.public_view().teams[0].confirmed_members.is_empty());
    }

    #[test]
    fn failed_commands_are_atomic_and_successful_commands_are_idempotent() {
        let mut state = RegistrationState::new(announcement()).unwrap();
        open(&mut state);
        let invalid = command(
            "register-invalid",
            1,
            RegistrationActor::Player(player("captain-a")),
            RegistrationAction::RegisterTeam {
                team_id: team("a"),
                captain: player("captain-a"),
                members: vec![player("captain-a")],
            },
        );
        assert!(state.execute(invalid, now(99)).is_err());
        assert_eq!(state.revision(), 1);
        assert!(state.public_view().teams.is_empty());

        let valid = command(
            "register-a",
            1,
            RegistrationActor::Player(player("captain-a")),
            RegistrationAction::RegisterTeam {
                team_id: team("a"),
                captain: player("captain-a"),
                members: vec![player("captain-a"), player("member-a")],
            },
        );
        let first = state.execute(valid.clone(), now(99)).unwrap();
        let second = state.execute(valid, now(99)).unwrap();
        assert_eq!(first, second);
        assert_eq!(state.revision(), 2);
        let conflict = state.execute(
            command(
                "register-a",
                2,
                RegistrationActor::Player(player("captain-a")),
                RegistrationAction::Open,
            ),
            now(99),
        );
        assert!(matches!(
            conflict,
            Err(RegistrationError::CommandIdConflict { .. })
        ));
    }

    #[test]
    fn lock_uses_external_time_and_marks_unchecked_teams_without_substitution() {
        let mut state = RegistrationState::new(announcement()).unwrap();
        open(&mut state);
        register(&mut state);
        let too_early = state.execute(
            command(
                "lock-early",
                2,
                RegistrationActor::Organizer(player("organizer")),
                RegistrationAction::Lock {},
            ),
            now(99),
        );
        assert!(too_early.is_err());
        assert_eq!(state.lifecycle(), RegistrationLifecycle::Open);
        state
            .execute(
                command(
                    "lock",
                    2,
                    RegistrationActor::Organizer(player("organizer")),
                    RegistrationAction::Lock {},
                ),
                now(60_000),
            )
            .unwrap();
        assert_eq!(state.lifecycle(), RegistrationLifecycle::Locked);
        assert_eq!(
            state.public_view().teams[0].status,
            TeamStatus::NotCheckedIn
        );
    }

    #[test]
    fn deadlines_are_exclusive_and_trusted_time_cannot_go_backwards() {
        let mut state = RegistrationState::new(announcement()).unwrap();
        open(&mut state);
        let boundary_register = state.execute(
            command(
                "boundary-register",
                1,
                RegistrationActor::Player(player("captain-a")),
                RegistrationAction::RegisterTeam {
                    team_id: team("a"),
                    captain: player("captain-a"),
                    members: vec![player("captain-a"), player("member-a")],
                },
            ),
            now(60_000),
        );
        assert!(matches!(
            boundary_register,
            Err(RegistrationError::InvalidTimestamp { .. })
        ));
        register(&mut state);
        let backwards = state.execute(
            command(
                "backwards",
                2,
                RegistrationActor::Player(player("captain-a")),
                RegistrationAction::ConfirmMembership {
                    team_id: team("a"),
                    roster_version: 1,
                },
            ),
            now(98),
        );
        assert!(matches!(
            backwards,
            Err(RegistrationError::TimestampWentBackwards { .. })
        ));
        let boundary_confirm = state.execute(
            command(
                "boundary-confirm",
                2,
                RegistrationActor::Player(player("captain-a")),
                RegistrationAction::ConfirmMembership {
                    team_id: team("a"),
                    roster_version: 1,
                },
            ),
            now(60_000),
        );
        assert!(matches!(
            boundary_confirm,
            Err(RegistrationError::InvalidTimestamp { .. })
        ));
        assert_eq!(state.revision(), 2);
    }

    #[test]
    fn fake_organizer_cannot_withdraw_and_withdrawn_players_can_rejoin() {
        let mut state = RegistrationState::new(announcement()).unwrap();
        open(&mut state);
        register(&mut state);
        let fake = state.execute(
            command(
                "fake-withdraw",
                2,
                RegistrationActor::Organizer(player("not-organizer")),
                RegistrationAction::WithdrawTeam { team_id: team("a") },
            ),
            now(99),
        );
        assert!(matches!(fake, Err(RegistrationError::Unauthorized)));
        state
            .execute(
                command(
                    "withdraw-a",
                    2,
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::WithdrawTeam { team_id: team("a") },
                ),
                now(99),
            )
            .unwrap();
        state
            .execute(
                command(
                    "register-b",
                    3,
                    RegistrationActor::Player(player("captain-b")),
                    RegistrationAction::RegisterTeam {
                        team_id: team("b"),
                        captain: player("captain-b"),
                        members: vec![player("captain-b"), player("member-a")],
                    },
                ),
                now(99),
            )
            .unwrap();
        assert_eq!(state.public_view().teams.len(), 2);
        assert_eq!(
            state
                .private_view(&player("member-a"))
                .own_team
                .unwrap()
                .team_id,
            team("b")
        );
    }

    #[test]
    fn locked_export_contains_only_checked_in_teams_and_private_decks() {
        let mut state = RegistrationState::new(announcement()).unwrap();
        open(&mut state);
        register(&mut state);
        state
            .execute(
                command(
                    "confirm-captain",
                    2,
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::ConfirmMembership {
                        team_id: team("a"),
                        roster_version: 1,
                    },
                ),
                now(99),
            )
            .unwrap();
        state
            .execute(
                command(
                    "confirm-member",
                    3,
                    RegistrationActor::Player(player("member-a")),
                    RegistrationAction::ConfirmMembership {
                        team_id: team("a"),
                        roster_version: 1,
                    },
                ),
                now(99),
            )
            .unwrap();
        state
            .execute(
                command(
                    "check-in",
                    4,
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::CheckInTeam {
                        team_id: team("a"),
                        roster_version: 1,
                    },
                ),
                now(99),
            )
            .unwrap();
        state
            .execute(
                command(
                    "lock",
                    5,
                    RegistrationActor::Organizer(player("organizer")),
                    RegistrationAction::Lock {},
                ),
                now(60_000),
            )
            .unwrap();
        let locked = state.export_locked_roster().unwrap();
        assert_eq!(locked.teams.len(), 1);
        assert_eq!(locked.teams[0].members.len(), 2);
        assert!(locked.teams[0].decks.is_empty());
    }

    #[test]
    fn personal_deck_submission_reuses_official_validation_and_stays_private() {
        let mut config = announcement();
        config.enabled_rule_modules = vec![RuleModuleId::new(PERSONAL_DECK_MODULE_ID)];
        config.personal_decks = PersonalDeckPolicy::Required;
        let mut state = RegistrationState::new(config).unwrap();
        open(&mut state);
        register(&mut state);
        let deck = OfficialRules::new().preconstructed_deck(PlayerId::new("captain-a"));
        state
            .execute(
                command(
                    "deck-a",
                    2,
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::SubmitDeck {
                        team_id: team("a"),
                        roster_version: 1,
                        name: deck.name,
                        cards: deck
                            .cards
                            .iter()
                            .map(|card| card.as_str().to_string())
                            .collect(),
                    },
                ),
                now(99),
            )
            .unwrap();
        let invalid = state.execute(
            command(
                "deck-bad",
                3,
                RegistrationActor::Player(player("member-a")),
                RegistrationAction::SubmitDeck {
                    team_id: team("a"),
                    roster_version: 1,
                    name: "bad".to_string(),
                    cards: vec!["does-not-exist".to_string()],
                },
            ),
            now(99),
        );
        assert!(matches!(
            invalid,
            Err(RegistrationError::InvalidPersonalDeck { .. })
        ));
        let public_json = serde_json::to_string(&state.public_view()).unwrap();
        assert!(!public_json.contains("captain-a-deck"));
        assert!(!public_json.contains("deck-a"));
        let private = state.private_view(&player("captain-a"));
        assert_eq!(private.own_team.unwrap().own_deck.unwrap().cards.len(), 60);
    }

    fn opening_personal_registration() -> RegistrationState {
        let mut config = announcement();
        config.enabled_rule_modules = vec![RuleModuleId::new(PERSONAL_DECK_MODULE_ID)];
        config.personal_decks = PersonalDeckPolicy::Required;
        let mut state = RegistrationState::new(config).unwrap();
        open(&mut state);
        register(&mut state);
        state
    }

    fn opening_personal_deck_action(id: &str, roster_version: u64) -> RegistrationAction {
        let deck = OfficialRules::new().preconstructed_deck(PlayerId::new(id));
        RegistrationAction::SubmitDeck {
            team_id: team("a"),
            roster_version,
            name: deck.name,
            cards: deck
                .cards
                .iter()
                .map(|card| card.as_str().to_string())
                .collect(),
        }
    }

    fn opening_prepare_data(state: &mut RegistrationState, members: &[&str], roster_version: u64) {
        for id in members {
            state
                .execute(
                    command(
                        &format!("opening-confirm-{id}-{}", state.revision()),
                        state.revision(),
                        RegistrationActor::Player(player(id)),
                        RegistrationAction::ConfirmMembership {
                            team_id: team("a"),
                            roster_version,
                        },
                    ),
                    now(99),
                )
                .unwrap();
            state
                .execute(
                    command(
                        &format!("opening-deck-{id}-{}", state.revision()),
                        state.revision(),
                        RegistrationActor::Player(player(id)),
                        opening_personal_deck_action(id, roster_version),
                    ),
                    now(99),
                )
                .unwrap();
        }
    }

    fn opening_captain_complete(state: &mut RegistrationState, roster_version: u64) {
        state
            .execute(
                command(
                    &format!("opening-captain-{}", state.revision()),
                    state.revision(),
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::CheckInTeam {
                        team_id: team("a"),
                        roster_version,
                    },
                ),
                now(99),
            )
            .unwrap();
    }

    #[test]
    fn opening_freeze_data_cutoff_precedes_locked_observation() {
        let actions = [
            RegistrationAction::ConfirmMembership {
                team_id: team("a"),
                roster_version: 1,
            },
            opening_personal_deck_action("captain-a", 1),
            RegistrationAction::CheckInTeam {
                team_id: team("a"),
                roster_version: 1,
            },
        ];
        for action in actions {
            for time in [59_999, 60_000, 60_001] {
                let mut state = opening_personal_registration();
                if matches!(&action, RegistrationAction::CheckInTeam { .. }) {
                    opening_prepare_data(&mut state, &["captain-a", "member-a"], 1);
                }
                let before = state.public_view();
                let private = state.private_view(&player("captain-a"));
                let result = state.execute(
                    command(
                        "opening-cutoff",
                        state.revision(),
                        RegistrationActor::Player(player("captain-a")),
                        action.clone(),
                    ),
                    now(time),
                );
                if time == 59_999 {
                    assert_eq!(result.unwrap().revision, before.revision + 1);
                } else {
                    assert!(matches!(
                        result,
                        Err(RegistrationError::InvalidTimestamp { .. })
                    ));
                    assert_eq!(state.public_view(), before);
                    assert_eq!(state.private_view(&player("captain-a")), private);
                }
                assert_eq!(state.lifecycle(), RegistrationLifecycle::Open);
                assert!(state.public_view().teams[0].exclusion_reasons.is_empty());
            }
        }
    }

    #[test]
    fn opening_freeze_public_reasons_are_closed_camel_case_without_private_data() {
        let mut state = opening_personal_registration();
        state
            .execute(
                command(
                    "opening-partial-deck",
                    state.revision(),
                    RegistrationActor::Player(player("captain-a")),
                    opening_personal_deck_action("captain-a", 1),
                ),
                now(99),
            )
            .unwrap();
        state
            .execute(
                command(
                    "opening-lock",
                    state.revision(),
                    RegistrationActor::Organizer(player("organizer")),
                    RegistrationAction::Lock {},
                ),
                now(60_000),
            )
            .unwrap();
        let public = state.public_view();
        let wire = serde_json::to_value(&public.teams[0]).unwrap();
        assert_eq!(
            wire,
            serde_json::json!({
                "teamId": "a", "captain": "captain-a",
                "members": ["captain-a", "member-a"], "rosterVersion": 1,
                "confirmedMembers": [], "status": "notCheckedIn",
                "exclusionReasons": [
                    "missingMemberConfirmations", "missingRequiredDecks", "missingCaptainConfirmation"
                ]
            })
        );
        assert_eq!(
            serde_json::from_value::<PublicTeamView>(wire.clone()).unwrap(),
            public.teams[0]
        );
        let mut missing = wire;
        missing.as_object_mut().unwrap().remove("exclusionReasons");
        assert!(serde_json::from_value::<PublicTeamView>(missing).is_err());
        for unknown in [
            "unknown",
            "missing_required_decks",
            "MissingCaptainConfirmation",
        ] {
            assert!(
                serde_json::from_value::<RegistrationExclusionReason>(serde_json::json!(unknown))
                    .is_err()
            );
        }
        assert!(state.export_locked_roster().unwrap().teams.is_empty());
        let private = state.private_view(&player("captain-a"));
        assert_eq!(private.public_view, public);
        assert_eq!(private.own_team.unwrap().own_deck.unwrap().cards.len(), 60);
        assert!(state.private_view(&player("outsider")).own_team.is_none());
    }

    #[test]
    fn opening_freeze_lock_requires_organizer_and_cannot_preempt_registration() {
        let mut state = opening_personal_registration();
        opening_prepare_data(&mut state, &["captain-a", "member-a"], 1);
        opening_captain_complete(&mut state, 1);
        let before = state.public_view();
        let private = state.private_view(&player("captain-a"));
        let attempts = [
            (RegistrationActor::Organizer(player("organizer")), 59_999),
            (RegistrationActor::Organizer(player("impostor")), 60_000),
            (RegistrationActor::Player(player("organizer")), 60_000),
            (RegistrationActor::Player(player("captain-a")), 60_000),
        ];
        for (actor, time) in attempts {
            let error = state
                .execute(
                    command(
                        "opening-denied-lock",
                        state.revision(),
                        actor,
                        RegistrationAction::Lock {},
                    ),
                    now(time),
                )
                .unwrap_err();
            if time == 59_999 {
                assert!(matches!(error, RegistrationError::InvalidTimestamp { .. }));
            } else {
                assert_eq!(error, RegistrationError::Unauthorized);
            }
            assert_eq!(state.public_view(), before);
            assert_eq!(state.private_view(&player("captain-a")), private);
            assert_eq!(state.lifecycle(), RegistrationLifecycle::Open);
        }
        state
            .execute(
                command(
                    "opening-denied-lock",
                    state.revision(),
                    RegistrationActor::Organizer(player("organizer")),
                    RegistrationAction::Lock {},
                ),
                now(60_000),
            )
            .unwrap();
        assert_eq!(state.revision(), before.revision + 1);
        assert_eq!(state.export_locked_roster().unwrap().teams.len(), 1);
    }

    #[test]
    fn opening_freeze_late_receipt_does_not_reverse_locked_state() {
        let mut state = opening_personal_registration();
        let confirmation = command(
            "opening-late-receipt",
            state.revision(),
            RegistrationActor::Player(player("captain-a")),
            RegistrationAction::ConfirmMembership {
                team_id: team("a"),
                roster_version: 1,
            },
        );
        let receipt = state.execute(confirmation.clone(), now(99)).unwrap();
        state
            .execute(
                command(
                    "opening-lock-later",
                    state.revision(),
                    RegistrationActor::Organizer(player("organizer")),
                    RegistrationAction::Lock {},
                ),
                now(60_000),
            )
            .unwrap();
        let before = state.public_view();
        let private = state.private_view(&player("captain-a"));
        let export = state.export_locked_roster().unwrap();
        assert_eq!(
            state.execute(confirmation.clone(), now(500)).unwrap(),
            receipt
        );
        assert_eq!(state.public_view(), before);
        assert_eq!(state.private_view(&player("captain-a")), private);
        assert_eq!(state.export_locked_roster().unwrap(), export);
        let mut conflict = confirmation;
        conflict.action = RegistrationAction::WithdrawTeam { team_id: team("a") };
        assert!(matches!(
            state.execute(conflict, now(500)),
            Err(RegistrationError::CommandIdConflict { .. })
        ));
        assert_eq!(state.public_view(), before);
    }

    #[test]
    fn opening_freeze_reason_matrix_does_not_misclassify_disabled_or_withdrawn_teams() {
        use RegistrationExclusionReason::{
            MissingCaptainConfirmation, MissingMemberConfirmations, MissingRequiredDecks,
        };
        let cases = [
            (
                PersonalDeckPolicy::Required,
                false,
                false,
                false,
                false,
                vec![
                    MissingMemberConfirmations,
                    MissingRequiredDecks,
                    MissingCaptainConfirmation,
                ],
            ),
            (
                PersonalDeckPolicy::Required,
                false,
                true,
                false,
                false,
                vec![MissingMemberConfirmations, MissingCaptainConfirmation],
            ),
            (
                PersonalDeckPolicy::Required,
                true,
                false,
                false,
                false,
                vec![MissingRequiredDecks, MissingCaptainConfirmation],
            ),
            (
                PersonalDeckPolicy::Required,
                true,
                true,
                false,
                false,
                vec![MissingCaptainConfirmation],
            ),
            (
                PersonalDeckPolicy::Required,
                true,
                true,
                true,
                false,
                vec![],
            ),
            (
                PersonalDeckPolicy::Disabled,
                true,
                false,
                false,
                false,
                vec![MissingCaptainConfirmation],
            ),
            (
                PersonalDeckPolicy::Disabled,
                true,
                false,
                true,
                false,
                vec![],
            ),
            (
                PersonalDeckPolicy::Required,
                false,
                false,
                false,
                true,
                vec![],
            ),
            (
                PersonalDeckPolicy::Disabled,
                false,
                false,
                false,
                true,
                vec![],
            ),
        ];
        for (policy, confirmed, submitted, captain_complete, withdrawn, expected) in cases {
            let mut state = if policy == PersonalDeckPolicy::Required {
                opening_personal_registration()
            } else {
                let mut state = RegistrationState::new(announcement()).unwrap();
                open(&mut state);
                register(&mut state);
                state
            };
            for id in ["captain-a", "member-a"] {
                if confirmed {
                    state
                        .execute(
                            command(
                                &format!("opening-matrix-confirm-{id}"),
                                state.revision(),
                                RegistrationActor::Player(player(id)),
                                RegistrationAction::ConfirmMembership {
                                    team_id: team("a"),
                                    roster_version: 1,
                                },
                            ),
                            now(99),
                        )
                        .unwrap();
                }
                if submitted {
                    state
                        .execute(
                            command(
                                &format!("opening-matrix-deck-{id}"),
                                state.revision(),
                                RegistrationActor::Player(player(id)),
                                opening_personal_deck_action(id, 1),
                            ),
                            now(99),
                        )
                        .unwrap();
                }
            }
            if captain_complete {
                opening_captain_complete(&mut state, 1);
            }
            if withdrawn {
                state
                    .execute(
                        command(
                            "opening-matrix-withdraw",
                            state.revision(),
                            RegistrationActor::Player(player("captain-a")),
                            RegistrationAction::WithdrawTeam { team_id: team("a") },
                        ),
                        now(99),
                    )
                    .unwrap();
            }
            assert!(state.public_view().teams[0].exclusion_reasons.is_empty());
            state
                .execute(
                    command(
                        "opening-matrix-lock",
                        state.revision(),
                        RegistrationActor::Organizer(player("organizer")),
                        RegistrationAction::Lock {},
                    ),
                    now(60_000),
                )
                .unwrap();
            let public = state.public_view();
            assert_eq!(public.teams[0].exclusion_reasons, expected);
            let status = if withdrawn {
                TeamStatus::Withdrawn
            } else if captain_complete {
                TeamStatus::CheckedIn
            } else {
                TeamStatus::NotCheckedIn
            };
            assert_eq!(public.teams[0].status, status);
            let exported = state.export_locked_roster().unwrap();
            if captain_complete {
                assert_eq!(exported.teams.len(), 1);
            } else {
                assert!(exported.teams.is_empty());
            }
            assert_eq!(state.private_view(&player("captain-a")).public_view, public);
        }
    }

    #[test]
    fn opening_freeze_roster_and_deck_changes_require_new_captain_completion() {
        for replace_roster in [false, true] {
            let mut state = opening_personal_registration();
            opening_prepare_data(&mut state, &["captain-a", "member-a"], 1);
            opening_captain_complete(&mut state, 1);
            let action = if replace_roster {
                RegistrationAction::ReplaceRoster {
                    team_id: team("a"),
                    expected_roster_version: 1,
                    members: vec![player("captain-a"), player("member-new")],
                }
            } else {
                opening_personal_deck_action("captain-a", 1)
            };
            state
                .execute(
                    command(
                        "opening-change-after-captain",
                        state.revision(),
                        RegistrationActor::Player(player("captain-a")),
                        action,
                    ),
                    now(99),
                )
                .unwrap();
            let before = state.public_view();
            assert_eq!(before.teams[0].status, TeamStatus::Registered);
            let roster_version = if replace_roster { 2 } else { 1 };
            if replace_roster {
                assert_eq!(before.teams[0].roster_version, 2);
                assert!(before.teams[0].confirmed_members.is_empty());
                assert!(
                    state
                        .private_view(&player("captain-a"))
                        .own_team
                        .unwrap()
                        .own_deck
                        .is_none()
                );
                assert!(state.private_view(&player("member-a")).own_team.is_none());
            } else {
                assert_eq!(before.teams[0].confirmed_members.len(), 2);
                assert!(
                    state
                        .private_view(&player("captain-a"))
                        .own_team
                        .unwrap()
                        .own_deck
                        .is_some()
                );
            }
            // 同一資料變更若在截止前重新完成確認，仍可合法納入。
            let mut completed = state.clone();
            if replace_roster {
                opening_prepare_data(&mut completed, &["captain-a", "member-new"], 2);
            }
            opening_captain_complete(&mut completed, roster_version);
            let late_confirmation = state.execute(
                command(
                    "opening-captain-too-late",
                    state.revision(),
                    RegistrationActor::Player(player("captain-a")),
                    RegistrationAction::CheckInTeam {
                        team_id: team("a"),
                        roster_version,
                    },
                ),
                now(60_000),
            );
            assert!(matches!(
                late_confirmation,
                Err(RegistrationError::InvalidTimestamp { .. })
            ));
            assert_eq!(state.public_view(), before);
            for candidate in [&mut state, &mut completed] {
                candidate
                    .execute(
                        command(
                            "opening-change-lock",
                            candidate.revision(),
                            RegistrationActor::Organizer(player("organizer")),
                            RegistrationAction::Lock {},
                        ),
                        now(60_000),
                    )
                    .unwrap();
            }
            let expected = if replace_roster {
                vec![
                    RegistrationExclusionReason::MissingMemberConfirmations,
                    RegistrationExclusionReason::MissingRequiredDecks,
                    RegistrationExclusionReason::MissingCaptainConfirmation,
                ]
            } else {
                vec![RegistrationExclusionReason::MissingCaptainConfirmation]
            };
            assert_eq!(state.public_view().teams[0].exclusion_reasons, expected);
            assert!(state.export_locked_roster().unwrap().teams.is_empty());
            assert!(
                completed.public_view().teams[0]
                    .exclusion_reasons
                    .is_empty()
            );
            assert_eq!(completed.export_locked_roster().unwrap().teams.len(), 1);
        }
    }

    #[test]
    fn opening_freeze_corrupt_checked_in_data_rolls_back_without_a_receipt() {
        let mut valid = opening_personal_registration();
        opening_prepare_data(&mut valid, &["captain-a", "member-a"], 1);
        opening_captain_complete(&mut valid, 1);
        valid
            .execute(
                command(
                    "opening-incomplete-team",
                    valid.revision(),
                    RegistrationActor::Player(player("captain-b")),
                    RegistrationAction::RegisterTeam {
                        team_id: team("b"),
                        captain: player("captain-b"),
                        members: vec![player("captain-b"), player("member-b")],
                    },
                ),
                now(99),
            )
            .unwrap();
        let original = valid.teams.get(&team("a")).unwrap().clone();
        for corruption in ["confirmation", "missing-deck", "invalid-deck"] {
            let mut state = valid.clone();
            // 故障注入：這些損壞不是正式命令可建立的合法 checkpoint。
            let corrupted = state.teams.get_mut(&team("a")).unwrap();
            match corruption {
                "confirmation" => {
                    corrupted.confirmed_members.remove(&player("captain-a"));
                }
                "missing-deck" => {
                    corrupted.decks.remove(&player("captain-a"));
                }
                "invalid-deck" => {
                    corrupted.decks.get_mut(&player("captain-a")).unwrap().cards[0] =
                        crate::domain::CardDefId::new("invalid-opening-card");
                }
                _ => unreachable!(),
            }
            let before = state.public_view();
            let private = state.private_view(&player("captain-a"));
            let lock = command(
                "opening-corrupt-lock",
                state.revision(),
                RegistrationActor::Organizer(player("organizer")),
                RegistrationAction::Lock {},
            );
            let error = state.execute(lock.clone(), now(60_000)).unwrap_err();
            match corruption {
                "confirmation" => assert!(matches!(
                    error,
                    RegistrationError::MissingConfirmations { .. }
                )),
                "missing-deck" => assert!(matches!(error, RegistrationError::MissingDecks { .. })),
                "invalid-deck" => assert!(matches!(
                    error,
                    RegistrationError::InvalidPersonalDeck { .. }
                )),
                _ => unreachable!(),
            }
            assert_eq!(state.public_view(), before);
            assert_eq!(state.private_view(&player("captain-a")), private);
            assert_eq!(state.lifecycle(), RegistrationLifecycle::Open);
            assert_eq!(state.public_view().teams[1].status, TeamStatus::Registered);
            // 修復注入資料後，同一原命令能執行，證明失敗未留下成功收據。
            state.teams.insert(team("a"), original.clone());
            state.execute(lock, now(60_000)).unwrap();
            assert_eq!(state.lifecycle(), RegistrationLifecycle::Locked);
            let export = state.export_locked_roster().unwrap();
            assert_eq!(export.teams.len(), 1);
            assert_eq!(export.teams[0].team_id, team("a"));
            assert_eq!(export.teams[0].decks.len(), 2);
        }
    }

    #[test]
    fn enum_fields_follow_camel_case_contract() {
        let command = command(
            "x",
            0,
            RegistrationActor::Player(player("captain-a")),
            RegistrationAction::ConfirmMembership {
                team_id: team("team-a"),
                roster_version: 7,
            },
        );
        let json = serde_json::to_value(command).unwrap();
        assert_eq!(json["action"]["type"], "confirmMembership");
        assert!(json["action"].get("rosterVersion").is_some());
        assert!(json["action"].get("roster_version").is_none());
    }

    #[test]
    fn invite_registration_only_accepts_invited_captain() {
        let mut config = announcement();
        config.registration_mode = RegistrationMode::InviteOnly {
            invited_captains: vec![player("invited")],
        };
        let mut state = RegistrationState::new(config).unwrap();
        open(&mut state);
        let denied = state.execute(
            command(
                "denied",
                1,
                RegistrationActor::Player(player("captain-a")),
                RegistrationAction::RegisterTeam {
                    team_id: team("a"),
                    captain: player("captain-a"),
                    members: vec![player("captain-a"), player("member-a")],
                },
            ),
            now(99),
        );
        assert!(matches!(denied, Err(RegistrationError::NotInvited)));
    }
}
