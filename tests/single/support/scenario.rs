#![allow(dead_code)]
//! 深度規則測試使用的情境工具。
//!
//! 情境只安排開局資料與提交正式命令；它不建立另一套規則語言，也不修改執行中的
//! 遊戲狀態。每個步驟都保留 `GameRecord` 的 accepted decision 邊界。

use fewfc::application::{GameRecord, RecordedDecision, ReplayVerificationError};
use fewfc::domain::{
    CardInstanceId, CardOrigin, ChoiceAnswer, Command, Element, GameError, GameResult, GameSetup,
    GameState, Player, PlayerId, RuleModuleId, TeamId, TrustedRandomnessAnswer,
};
use fewfc::rules::{OfficialRules, PlayableAction};

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScenarioPlan {
    players: Vec<Player>,
    turn_order: Vec<PlayerId>,
    modules: Vec<RuleModuleId>,
}

impl ScenarioPlan {
    pub fn two_player(module_ids: &[&str]) -> Self {
        let p1 = PlayerId::new("p1");
        let p2 = PlayerId::new("p2");
        Self {
            players: vec![
                Player {
                    id: p1.clone(),
                    team: TeamId::new("team:p1"),
                },
                Player {
                    id: p2.clone(),
                    team: TeamId::new("team:p2"),
                },
            ],
            turn_order: vec![p1, p2],
            modules: module_ids.iter().copied().map(RuleModuleId::new).collect(),
        }
    }

    pub fn setup(&self) -> Result<GameSetup, ScenarioError> {
        OfficialRules::new()
            .configure_game(
                self.players.clone(),
                self.turn_order.clone(),
                self.modules.clone(),
            )
            .map_err(ScenarioError::Game)
    }

    pub fn start(self) -> Result<Scenario, ScenarioError> {
        self.start_with_deck(|_, deck| Ok(deck))
    }

    /// `arrange` 僅能重排完整的官方 Deck；`GameRecord::start` 仍驗證其不變量。
    pub fn start_with_deck<F>(self, arrange: F) -> Result<Scenario, ScenarioError>
    where
        F: FnOnce(&GameSetup, Vec<CardInstanceId>) -> Result<Vec<CardInstanceId>, ScenarioError>,
    {
        let setup = self.setup()?;
        let deck = OfficialRules::new()
            .official_deck_order(&setup)
            .map_err(ScenarioError::Game)?;
        Scenario::from_setup_and_deck(setup.clone(), arrange(&setup, deck)?)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScenarioZone {
    Hand(PlayerId),
    Deck(PlayerId),
    Discard(PlayerId),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CardSelector {
    pub zone: ScenarioZone,
    pub origin: Option<CardOrigin>,
    pub element: Element,
    pub printed_level: u32,
    pub occurrence: Option<usize>,
}

impl CardSelector {
    pub fn in_hand(player: PlayerId, element: Element, printed_level: u32) -> Self {
        Self {
            zone: ScenarioZone::Hand(player),
            origin: None,
            element,
            printed_level,
            occurrence: None,
        }
    }

    pub fn occurrence(mut self, occurrence: usize) -> Self {
        self.occurrence = Some(occurrence);
        self
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScenarioStep {
    Command(Command),
    Answer {
        player: PlayerId,
        answer: ChoiceAnswer,
    },
    Randomness(TrustedRandomnessAnswer),
    Advance,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StepTrace {
    pub decisions: Vec<RecordedDecision>,
}

impl StepTrace {
    pub fn events(&self) -> Vec<&fewfc::domain::GameEvent> {
        self.decisions
            .iter()
            .flat_map(|decision| decision.events.iter())
            .collect()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ScenarioError {
    Game(GameError),
    MissingPendingChoice,
    CardSelection {
        selector: CardSelector,
        matches: Vec<CardInstanceId>,
    },
    UnsafeCheckpoint,
    ReplayMismatch,
    Replay(ReplayVerificationError),
    AdvanceFailed {
        error: GameError,
        trace: StepTrace,
    },
}

pub struct Scenario {
    record: GameRecord,
}

impl Scenario {
    fn from_setup_and_deck(
        setup: GameSetup,
        deck: Vec<CardInstanceId>,
    ) -> Result<Self, ScenarioError> {
        GameRecord::start(setup, deck)
            .map(|record| Self { record })
            .map_err(ScenarioError::Game)
    }

    pub fn record(&self) -> &GameRecord {
        &self.record
    }
    pub fn record_mut(&mut self) -> &mut GameRecord {
        &mut self.record
    }
    pub fn state(&self) -> &GameState {
        self.record.state()
    }
    pub fn current_player(&self) -> Option<&PlayerId> {
        self.state().current_player()
    }

    pub fn playable_actions(
        &self,
        player: &PlayerId,
        cards: &[CardInstanceId],
    ) -> GameResult<Vec<PlayableAction>> {
        self.record.playable_actions(player, cards)
    }

    pub fn select_card(&self, selector: CardSelector) -> Result<CardInstanceId, ScenarioError> {
        let cards = match &selector.zone {
            ScenarioZone::Hand(player) => self
                .state()
                .hand(player)
                .map(|cards| cards.to_vec())
                .unwrap_or_default(),
            ScenarioZone::Deck(player) => self
                .state()
                .deck_for(player)
                .map(|cards| cards.to_vec())
                .unwrap_or_default(),
            ScenarioZone::Discard(player) => self
                .state()
                .discard_for(player)
                .map(|cards| cards.to_vec())
                .unwrap_or_default(),
        };
        let matches = cards
            .into_iter()
            .filter(|card| {
                let Some(definition) = self.state().card_def(*card) else {
                    return false;
                };
                let origin_matches = selector.origin.as_ref().is_none_or(|origin| {
                    self.state()
                        .card_instances
                        .iter()
                        .any(|instance| instance.instance == *card && &instance.origin == origin)
                });
                definition.element == selector.element
                    && definition.level.value() == selector.printed_level
                    && origin_matches
            })
            .collect::<Vec<_>>();
        match selector.occurrence {
            Some(index) if index < matches.len() => Ok(matches[index]),
            None if matches.len() == 1 => Ok(matches[0]),
            _ => Err(ScenarioError::CardSelection { selector, matches }),
        }
    }

    pub fn step(&mut self, step: ScenarioStep) -> Result<StepTrace, ScenarioError> {
        let before = self.record.recorded_decisions().len();
        let is_advance = matches!(&step, ScenarioStep::Advance);
        let result = match step {
            ScenarioStep::Command(command) => self.record.handle(command).map(|_| ()),
            ScenarioStep::Answer { player, answer } => {
                let choice_id = self
                    .state()
                    .pending_choice
                    .as_ref()
                    .ok_or(ScenarioError::MissingPendingChoice)?
                    .choice_id;
                self.record
                    .handle(Command::AnswerChoice {
                        player,
                        choice_id,
                        answer,
                    })
                    .map(|_| ())
            }
            ScenarioStep::Randomness(answer) => self.record.resolve_randomness(answer).map(|_| ()),
            ScenarioStep::Advance => self.record.advance_until_decision().map(|_| ()),
        };
        let trace = StepTrace {
            decisions: self
                .record
                .recorded_decisions()
                .into_iter()
                .skip(before)
                .collect(),
        };
        match result {
            Ok(()) => Ok(trace),
            Err(error) if is_advance => Err(ScenarioError::AdvanceFailed { error, trace }),
            Err(error) => Err(ScenarioError::Game(error)),
        }
    }

    pub fn checkpoint(&self) -> Result<Self, ScenarioError> {
        if self.state().pending_choice.is_some() || self.state().pending_randomness.is_some() {
            return Err(ScenarioError::UnsafeCheckpoint);
        }
        Ok(Self {
            record: self.record.clone(),
        })
    }

    pub fn assert_replay_evidence(&self) -> Result<(), ScenarioError> {
        if self.record.replay().map_err(ScenarioError::Game)? != *self.state() {
            return Err(ScenarioError::ReplayMismatch);
        }
        if self.record.verify_replay().map_err(ScenarioError::Replay)? != *self.state() {
            return Err(ScenarioError::ReplayMismatch);
        }
        Ok(())
    }
}
