use fewfc::application::GameRecord;
use fewfc::domain::{CardInstanceId, GameResult, Player, PlayerId, RuleModuleId, TeamId};
use fewfc::rules::{OfficialRules, PlayableAction};

pub struct OfficialScenario {
    record: GameRecord,
}

impl OfficialScenario {
    pub fn two_player(module_ids: &[&str]) -> GameResult<Self> {
        let rules = OfficialRules::new();
        let players = vec![
            Player {
                id: PlayerId::new("p1"),
                team: TeamId::new("team:p1"),
            },
            Player {
                id: PlayerId::new("p2"),
                team: TeamId::new("team:p2"),
            },
        ];
        let setup = rules.configure_game(
            players,
            vec![PlayerId::new("p1"), PlayerId::new("p2")],
            module_ids.iter().copied().map(RuleModuleId::new).collect(),
        )?;
        let deck_order = rules.official_deck_order(&setup)?;
        Ok(Self {
            record: GameRecord::start(setup, deck_order)?,
        })
    }

    pub fn advance_to_decision(&mut self) -> GameResult<()> {
        self.record.advance_until_decision()?;
        Ok(())
    }

    pub fn current_player(&self) -> PlayerId {
        self.record
            .state()
            .current_player()
            .expect("scenario has a current player")
            .clone()
    }

    pub fn state(&self) -> &fewfc::domain::GameState {
        self.record.state()
    }

    pub fn first_hand_card(&self, player: &PlayerId) -> CardInstanceId {
        self.record
            .state()
            .hand(player)
            .and_then(|hand| hand.first())
            .copied()
            .expect("scenario player has a hand card")
    }

    pub fn playable_actions(
        &self,
        player: &PlayerId,
        cards: &[CardInstanceId],
    ) -> GameResult<Vec<PlayableAction>> {
        self.record.playable_actions(player, cards)
    }

    pub fn assert_replay_matches(&self) {
        assert_eq!(
            self.record.replay().expect("scenario record should replay"),
            self.record.state().clone(),
        );
    }
}
