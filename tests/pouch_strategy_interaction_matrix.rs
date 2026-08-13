use fewfc::application::GameRecord;
use fewfc::domain::{
    CardInstanceId, CardZone, ChoiceAnswer, Command, EffectiveCardLevel, Element,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameEvent, HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID,
    POUCH_MODULE_ID, Player, PlayerId, ProfessionId, RuleModuleId, SPIRIT_MODULE_ID,
    STAR_MODULE_ID, SecretStrategy, SpiritKind, StarBreakReason, StarKind, TeamId,
};
use fewfc::public_view::{PublicGameEvent, Viewer};
use fewfc::rules::{OfficialRules, PlayableAction};

const POUCH_STRATEGY_MODULES: [&str; 6] = [
    PERSONAL_DECK_MODULE_ID,
    STAR_MODULE_ID,
    SPIRIT_MODULE_ID,
    HERO_SCHOOLS_MODULE_ID,
    POUCH_MODULE_ID,
    FIVE_DIRECTIONS_LEGEND_MODULE_ID,
];

struct PouchStrategyScenario {
    record: GameRecord,
    player: PlayerId,
    opponent: PlayerId,
    sheep_source: CardInstanceId,
}

impl PouchStrategyScenario {
    fn new() -> Self {
        let player = PlayerId::new("p1");
        let opponent = PlayerId::new("p2");
        let mut setup = OfficialRules::new()
            .configure_game(
                vec![
                    Player {
                        id: player.clone(),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: opponent.clone(),
                        team: TeamId::new("team:p2"),
                    },
                ],
                vec![player.clone(), opponent.clone()],
                POUCH_STRATEGY_MODULES
                    .into_iter()
                    .map(RuleModuleId::new)
                    .collect(),
            )
            .unwrap();
        for team_hp in &mut setup.hp {
            team_hp.hp = 10_000;
        }
        let mut record = GameRecord::start(setup, Vec::new()).unwrap();

        let sheep_source = card_in_deck(&record, &player, Element::Fire, 2);
        let opponent_pouch = card_in_deck(&record, &opponent, Element::Metal, 5);
        record
            .handle(Command::ChooseInitialPouch {
                player: player.clone(),
                card: sheep_source,
            })
            .unwrap();
        record
            .handle(Command::ChooseInitialPouch {
                player: opponent.clone(),
                card: opponent_pouch,
            })
            .unwrap();

        while let Some(request) = record.state().pending_randomness.clone() {
            let fewfc::domain::RandomnessOperation::DeckShuffle { .. } = request.operation else {
                unreachable!("Pouch preparation only requests Personal Deck shuffles");
            };
            record
                .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order: request.current_order,
                })
                .unwrap();
        }
        record.advance_automatic().unwrap();
        assert_eq!(record.state().current_player(), Some(&player));

        Self {
            record,
            player,
            opponent,
            sheep_source,
        }
    }

    fn trigger_sheep_stealing(&mut self) -> Vec<GameEvent> {
        self.record
            .handle(Command::TriggerSecretStrategy {
                player: self.player.clone(),
                strategy: SecretStrategy::SheepStealing,
                target_player: None,
                star: None,
                break_star: false,
                discard_card: None,
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
            })
            .unwrap()
    }

    fn pending_sheep_choice(
        &self,
    ) -> (
        fewfc::domain::ChoiceId,
        Vec<CardInstanceId>,
        Vec<CardInstanceId>,
    ) {
        let choice = self
            .record
            .state()
            .pending_choice
            .as_ref()
            .expect("legal Sheep Stealing must create a typed choice");
        let fewfc::domain::PendingChoiceKind::SheepStealing {
            source_card,
            owner,
            deck_cards,
            discard_cards,
        } = &choice.kind
        else {
            panic!("pending choice must be Sheep Stealing");
        };
        assert_eq!(*source_card, self.sheep_source);
        assert_eq!(owner.as_ref(), Some(&self.player));
        (choice.choice_id, deck_cards.clone(), discard_cards.clone())
    }

    fn answer_sheep_with_same_projected_cards(
        &mut self,
        cards: Vec<CardInstanceId>,
    ) -> Vec<GameEvent> {
        let (choice_id, _, _) = self.pending_sheep_choice();
        self.record
            .handle(Command::AnswerChoice {
                player: self.player.clone(),
                choice_id,
                answer: ChoiceAnswer::SheepStealing {
                    deck_cards: cards.clone(),
                    discard_cards: cards,
                },
            })
            .unwrap()
    }

    fn resolve_pending_randomness_reversed(&mut self) -> Vec<GameEvent> {
        let request = self
            .record
            .state()
            .pending_randomness
            .clone()
            .expect("matrix action must request trusted randomness");
        let mut shuffled_order = request.current_order;
        shuffled_order.reverse();
        self.record
            .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order,
            })
            .unwrap()
            .into_events()
    }

    fn exhaust_player_deck_below_two_with_legal_turns(&mut self) {
        while self.record.state().deck_for(&self.player).unwrap().len() >= 2 {
            assert_eq!(self.record.state().current_player(), Some(&self.player));
            perform_first_elemental_attack(&mut self.record, &self.player);
            finish_turn(&mut self.record, &self.player);

            assert_eq!(self.record.state().current_player(), Some(&self.opponent));
            perform_first_elemental_attack(&mut self.record, &self.opponent);
            finish_turn(&mut self.record, &self.opponent);
        }
        assert_eq!(self.record.state().current_player(), Some(&self.player));
        assert!(self.record.state().deck_for(&self.player).unwrap().len() < 2);
        assert!(self.record.state().discard_for(&self.player).unwrap().len() >= 2);
    }

    fn assert_replay(&self) {
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

struct ChainSheepScenario {
    record: GameRecord,
    player: PlayerId,
    opponent: PlayerId,
    chain_cards: Vec<CardInstanceId>,
    sheep_trigger: CardInstanceId,
}

impl ChainSheepScenario {
    fn new() -> Self {
        let player = PlayerId::new("p1");
        let opponent = PlayerId::new("p2");
        let mut setup = OfficialRules::new()
            .configure_game(
                vec![
                    Player {
                        id: player.clone(),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: opponent.clone(),
                        team: TeamId::new("team:p2"),
                    },
                ],
                vec![player.clone(), opponent.clone()],
                POUCH_STRATEGY_MODULES
                    .into_iter()
                    .map(RuleModuleId::new)
                    .collect(),
            )
            .unwrap();
        // Background only: several legal attack turns must not end the game before Chain.
        for team_hp in &mut setup.hp {
            team_hp.hp = 10_000;
        }
        let mut record = GameRecord::start(setup, Vec::new()).unwrap();

        let initial_pouch = card_in_deck(&record, &player, Element::Earth, 5);
        let opponent_pouch = card_in_deck(&record, &opponent, Element::Metal, 5);
        record
            .handle(Command::ChooseInitialPouch {
                player: player.clone(),
                card: initial_pouch,
            })
            .unwrap();
        record
            .handle(Command::ChooseInitialPouch {
                player: opponent.clone(),
                card: opponent_pouch,
            })
            .unwrap();

        let chain_cards = vec![
            card_in_deck(&record, &player, Element::Metal, 1),
            card_in_deck(&record, &player, Element::Wood, 2),
            card_in_deck(&record, &player, Element::Water, 3),
        ];
        let first_legal_action = card_in_deck(&record, &player, Element::Earth, 4);
        let sheep_trigger = card_in_deck(&record, &player, Element::Fire, 2);
        let mut planned_initial_order = chain_cards.clone();
        planned_initial_order.push(first_legal_action);
        planned_initial_order.push(sheep_trigger);

        while let Some(request) = record.state().pending_randomness.clone() {
            let fewfc::domain::RandomnessOperation::DeckShuffle { deck } = &request.operation
            else {
                unreachable!("Pouch preparation only requests Personal Deck shuffles");
            };
            let shuffled_order = if matches!(deck, fewfc::domain::RandomnessDeck::Player(owner) if owner == &player)
            {
                append_remaining_cards(planned_initial_order.clone(), &request.current_order)
            } else {
                request.current_order.clone()
            };
            record
                .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order,
                })
                .unwrap();
        }
        record.advance_automatic().unwrap();
        assert_eq!(record.state().current_player(), Some(&player));
        assert_eq!(
            &record.state().hand(&player).unwrap()[..chain_cards.len()],
            chain_cards.as_slice()
        );
        assert!(
            record
                .state()
                .hand(&player)
                .unwrap()
                .contains(&first_legal_action)
        );

        Self {
            record,
            player,
            opponent,
            chain_cards,
            sheep_trigger,
        }
    }

    fn exhaust_deck_below_two_preserving_chain_cards(&mut self) {
        while self.record.state().deck_for(&self.player).unwrap().len() >= 2 {
            assert_eq!(self.record.state().current_player(), Some(&self.player));
            perform_first_elemental_attack_excluding(
                &mut self.record,
                &self.player,
                &self.chain_cards,
            );
            finish_turn(&mut self.record, &self.player);

            assert_eq!(self.record.state().current_player(), Some(&self.opponent));
            perform_first_elemental_attack(&mut self.record, &self.opponent);
            finish_turn(&mut self.record, &self.opponent);
        }
        assert_eq!(self.record.state().current_player(), Some(&self.player));
        assert!(self.record.state().deck_for(&self.player).unwrap().len() < 2);
        assert!(self.record.state().discard_for(&self.player).unwrap().len() >= 2);
        assert!(self.chain_cards.iter().all(|card| {
            self.record
                .state()
                .hand(&self.player)
                .unwrap()
                .contains(card)
        }));
        assert!(
            self.record
                .state()
                .discard_for(&self.player)
                .unwrap()
                .contains(&self.sheep_trigger)
        );
    }

    fn perform_chain_and_recycle(&mut self) -> Vec<GameEvent> {
        let events = self
            .record
            .handle(Command::PerformFormation {
                player: self.player.clone(),
                formation_id: "pouch:chain".to_string(),
                cards: self.chain_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::FormationCommitted {
                    player,
                    formation_id,
                    cards,
                    state: fewfc::domain::FormationAreaState::FaceUpResolving,
                    ..
                },
                GameEvent::RandomnessRequested { request },
            ] if player == &self.player
                && formation_id == "pouch:chain"
                && cards == &self.chain_cards
                && matches!(
                    request.operation,
                    fewfc::domain::RandomnessOperation::DiscardShuffle {
                        pile: fewfc::domain::RandomnessDeck::Player(ref player),
                        placement: fewfc::domain::DeckPlacement::Bottom,
                    } if player == &self.player
                )
                && matches!(
                    request.continuation,
                    fewfc::domain::RandomnessContinuation::Pouch(
                        fewfc::domain::PouchRandomnessContinuation::ChainRecycle
                    )
                )
        ));
        assert_eq!(
            self.record
                .state()
                .formation_area(&self.player)
                .and_then(|area| area.formation.as_ref())
                .expect("Chain commitment must occupy the Formation Area")
                .cards,
            self.chain_cards
        );
        events
    }

    fn resolve_pending_randomness_reversed(&mut self) -> Vec<GameEvent> {
        let request = self
            .record
            .state()
            .pending_randomness
            .clone()
            .expect("matrix action must request trusted randomness");
        let mut shuffled_order = request.current_order;
        shuffled_order.reverse();
        self.record
            .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order,
            })
            .unwrap()
            .into_events()
    }

    fn pending_chain_choice(&self) -> (fewfc::domain::ChoiceId, Vec<CardInstanceId>) {
        let choice = self
            .record
            .state()
            .pending_choice
            .as_ref()
            .expect("Chain recycle must resume its typed choice");
        let fewfc::domain::PendingChoiceKind::Chain {
            pouch_owners,
            deck_cards,
        } = &choice.kind
        else {
            panic!("pending choice must be Chain");
        };
        assert_eq!(pouch_owners, &vec![self.player.clone()]);
        (choice.choice_id, deck_cards.clone())
    }

    fn answer_chain_with_sheep_stealing(&mut self, pouch_card: CardInstanceId) -> Vec<GameEvent> {
        let (choice_id, deck_cards) = self.pending_chain_choice();
        assert!(deck_cards.contains(&pouch_card));
        assert!(deck_cards.contains(&self.sheep_trigger));
        let pouch = self.record.state().card_def(pouch_card).unwrap();
        let trigger = self.record.state().card_def(self.sheep_trigger).unwrap();
        assert_ne!(pouch.element, trigger.element);
        assert_ne!(pouch.level, trigger.level);
        self.record
            .handle(Command::AnswerChoice {
                player: self.player.clone(),
                choice_id,
                answer: ChoiceAnswer::Chain {
                    pouch_owner: self.player.clone(),
                    pouch_card,
                    trigger_card: Some(self.sheep_trigger),
                    strategy: Some(SecretStrategy::SheepStealing),
                    target_player: None,
                    star: None,
                    break_star: false,
                    discard_card: None,
                },
            })
            .unwrap()
    }

    fn pending_sheep_choice(&self) -> (fewfc::domain::ChoiceId, Vec<CardInstanceId>) {
        let choice = self
            .record
            .state()
            .pending_choice
            .as_ref()
            .expect("Chain-triggered Sheep Stealing must create a typed choice");
        let fewfc::domain::PendingChoiceKind::SheepStealing {
            source_card,
            owner,
            deck_cards,
            discard_cards: _,
        } = &choice.kind
        else {
            panic!("pending choice must be Sheep Stealing");
        };
        assert_eq!(*source_card, self.sheep_trigger);
        assert_eq!(*owner, None);
        assert!(!deck_cards.contains(&self.sheep_trigger));
        (choice.choice_id, deck_cards.clone())
    }

    fn answer_sheep_with_same_projected_cards(
        &mut self,
        cards: Vec<CardInstanceId>,
    ) -> Vec<GameEvent> {
        let (choice_id, deck_cards) = self.pending_sheep_choice();
        assert_eq!(cards.len(), 2);
        assert!(cards.iter().all(|card| deck_cards.contains(card)));
        self.record
            .handle(Command::AnswerChoice {
                player: self.player.clone(),
                choice_id,
                answer: ChoiceAnswer::SheepStealing {
                    deck_cards: cards.clone(),
                    discard_cards: cards,
                },
            })
            .unwrap()
    }

    fn assert_replay(&self) {
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

struct StealTheBeamMageGuideScenario {
    record: GameRecord,
    player: PlayerId,
    opponent: PlayerId,
    pouch_source: CardInstanceId,
    mage_cards: Vec<CardInstanceId>,
    mage_guide_cards: Vec<CardInstanceId>,
    immortal_cards: Vec<CardInstanceId>,
}

impl StealTheBeamMageGuideScenario {
    fn new() -> Self {
        let player = PlayerId::new("p1");
        let opponent = PlayerId::new("p2");
        let mut setup = OfficialRules::new()
            .configure_game(
                vec![
                    Player {
                        id: player.clone(),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: opponent.clone(),
                        team: TeamId::new("team:p2"),
                    },
                ],
                vec![player.clone(), opponent.clone()],
                POUCH_STRATEGY_MODULES
                    .into_iter()
                    .map(RuleModuleId::new)
                    .collect(),
            )
            .unwrap();
        // Background only: the two legal profession turns and their opponent turns must not end
        // the game before the interaction under test.
        for team_hp in &mut setup.hp {
            team_hp.hp = 10_000;
        }
        let mut record = GameRecord::start(setup, Vec::new()).unwrap();

        let pouch_source = card_in_deck(&record, &player, Element::Wood, 1);
        let opponent_pouch = card_in_deck(&record, &opponent, Element::Metal, 5);
        record
            .handle(Command::ChooseInitialPouch {
                player: player.clone(),
                card: pouch_source,
            })
            .unwrap();
        record
            .handle(Command::ChooseInitialPouch {
                player: opponent.clone(),
                card: opponent_pouch,
            })
            .unwrap();

        let mage_cards = cards_in_deck(&record, &player, &[(Element::Fire, 1), (Element::Fire, 2)]);
        let mage_guide_cards =
            cards_in_deck(&record, &player, &[(Element::Fire, 3), (Element::Fire, 3)]);
        let turn_one_discard = card_in_deck(&record, &player, Element::Fire, 4);
        let immortal_cards = cards_in_deck(
            &record,
            &player,
            &[(Element::Wood, 5), (Element::Wood, 5), (Element::Wood, 4)],
        );
        let turn_two_discard = card_in_deck(&record, &player, Element::Metal, 1);
        let turn_two_kept_sibling = card_in_deck(&record, &player, Element::Earth, 1);
        let mut ordered_player_deck = mage_cards.clone();
        ordered_player_deck.extend(immortal_cards[..2].iter().copied());
        ordered_player_deck.extend(mage_guide_cards.iter().copied());
        ordered_player_deck.push(turn_one_discard);
        ordered_player_deck.push(immortal_cards[2]);
        ordered_player_deck.push(turn_two_discard);
        ordered_player_deck.push(turn_two_kept_sibling);

        while let Some(request) = record.state().pending_randomness.clone() {
            let fewfc::domain::RandomnessOperation::DeckShuffle { deck } = &request.operation
            else {
                unreachable!("Pouch preparation only requests Personal Deck shuffles");
            };
            let shuffled_order = if matches!(deck, fewfc::domain::RandomnessDeck::Player(owner) if owner == &player)
            {
                append_remaining_cards(ordered_player_deck.clone(), &request.current_order)
            } else {
                request.current_order.clone()
            };
            record
                .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order,
                })
                .unwrap();
        }
        record.advance_automatic().unwrap();
        assert_eq!(record.state().current_player(), Some(&player));
        assert!(
            mage_cards
                .iter()
                .chain(immortal_cards[..2].iter())
                .all(|card| record.state().hand(&player).unwrap().contains(card))
        );

        Self {
            record,
            player,
            opponent,
            pouch_source,
            mage_cards,
            mage_guide_cards,
            immortal_cards,
        }
    }

    fn change_to_mage_guide_through_legal_turns(&mut self) {
        let mage_events = self
            .record
            .handle(Command::ChangeProfession {
                player: self.player.clone(),
                profession: ProfessionId::new("mage"),
                cards: self.mage_cards.clone(),
            })
            .unwrap();
        assert!(matches!(
            mage_events.as_slice(),
            [
                GameEvent::ActionStarted { player },
                GameEvent::ProfessionChanged {
                    player: changed,
                    previous: None,
                    profession,
                    card_moves,
                },
            ] if player == &self.player
                && changed == &self.player
                && profession == &ProfessionId::new("mage")
                && has_hand_to_discard_moves(card_moves, &self.player, &self.mage_cards)
        ));
        finish_turn_discarding_fact(&mut self.record, &self.player, Element::Fire, 4);

        self.finish_opponent_turn();
        assert_eq!(self.record.state().current_player(), Some(&self.player));
        let mage_guide_events = self
            .record
            .handle(Command::ChangeProfession {
                player: self.player.clone(),
                profession: ProfessionId::new("mage-guide"),
                cards: self.mage_guide_cards.clone(),
            })
            .unwrap();
        assert!(matches!(
            mage_guide_events.as_slice(),
            [
                GameEvent::ActionStarted { player },
                GameEvent::ProfessionChanged {
                    player: changed,
                    previous: Some(previous),
                    profession,
                    card_moves,
                },
            ] if player == &self.player
                && changed == &self.player
                && previous == &ProfessionId::new("mage")
                && profession == &ProfessionId::new("mage-guide")
                && has_hand_to_discard_moves(card_moves, &self.player, &self.mage_guide_cards)
        ));
        finish_turn_discarding_fact(&mut self.record, &self.player, Element::Metal, 1);

        self.finish_opponent_turn();
        assert_eq!(self.record.state().current_player(), Some(&self.player));
        assert_eq!(
            self.record.state().profession_for(&self.player),
            Some(&ProfessionId::new("mage-guide"))
        );
        assert!(self.immortal_cards.iter().all(|card| {
            self.record
                .state()
                .hand(&self.player)
                .unwrap()
                .contains(card)
        }));
    }

    fn assert_printed_five_five_four_cannot_change_to_immortal(&self) {
        assert_eq!(
            self.immortal_cards
                .iter()
                .map(|card| self.record.state().card_def(*card).unwrap().level.value())
                .collect::<Vec<_>>(),
            vec![5, 5, 4]
        );
        assert_eq!(
            self.immortal_cards
                .iter()
                .map(|card| {
                    self.record
                        .state()
                        .effective_card_facts(&self.player, *card)
                        .unwrap()
                        .level
                })
                .collect::<Vec<_>>(),
            vec![
                EffectiveCardLevel::new(5),
                EffectiveCardLevel::new(5),
                EffectiveCardLevel::new(4),
            ]
        );
        assert!(!offers_immortal(
            &self
                .record
                .playable_actions(&self.player, &self.immortal_cards)
                .unwrap(),
            &self.immortal_cards,
        ));
    }

    fn trigger_steal_the_beam(&mut self) -> Vec<GameEvent> {
        self.record
            .handle(Command::TriggerSecretStrategy {
                player: self.player.clone(),
                strategy: SecretStrategy::StealTheBeam,
                target_player: None,
                star: None,
                break_star: false,
                discard_card: None,
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
            })
            .unwrap()
    }

    fn assert_steal_the_beam_bonus_snapshot(&self, events: &[GameEvent]) {
        let expected_snapshot = self.record.state().hand(&self.player).unwrap().to_vec();
        assert!(matches!(
            events,
            [
                GameEvent::PouchRevealed {
                    player,
                    owner: Some(owner),
                    card,
                    strategy: SecretStrategy::StealTheBeam,
                },
                GameEvent::PouchLevelBonusGranted { bonus },
                GameEvent::PouchConsumed {
                    owner: Some(consumed_owner),
                    card: consumed_card,
                },
            ] if player == &self.player
                && owner == &self.player
                && *card == self.pouch_source
                && bonus.player == self.player
                && bonus.cards == expected_snapshot
                && bonus.applied_on_turn == self.record.state().turn_number
                && consumed_owner == &self.player
                && *consumed_card == self.pouch_source
        ));
        assert!(self.record.state().pouch_for(&self.player).is_none());
        assert!(
            self.record
                .state()
                .discard_for(&self.player)
                .unwrap()
                .contains(&self.pouch_source)
        );
        assert!(self.immortal_cards.iter().all(|card| {
            self.record
                .state()
                .pouch_level_bonuses
                .iter()
                .any(|bonus| bonus.player == self.player && bonus.cards.contains(card))
        }));
    }

    fn assert_effective_five_five_five_offers_immortal(&self) {
        assert_eq!(
            self.immortal_cards
                .iter()
                .map(|card| {
                    self.record
                        .state()
                        .effective_card_facts(&self.player, *card)
                        .unwrap()
                        .level
                })
                .collect::<Vec<_>>(),
            vec![
                EffectiveCardLevel::new(5),
                EffectiveCardLevel::new(5),
                EffectiveCardLevel::new(5),
            ]
        );
        assert!(offers_immortal(
            &self
                .record
                .playable_actions(&self.player, &self.immortal_cards)
                .unwrap(),
            &self.immortal_cards,
        ));
    }

    fn change_to_immortal_and_assert_canonical_commitment(&mut self) {
        let events = self
            .record
            .handle(Command::ChangeProfession {
                player: self.player.clone(),
                profession: ProfessionId::new("immortal"),
                cards: self.immortal_cards.clone(),
            })
            .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::ActionStarted { player },
                GameEvent::ProfessionChanged {
                    player: changed,
                    previous: Some(previous),
                    profession,
                    card_moves,
                },
            ] if player == &self.player
                && changed == &self.player
                && previous == &ProfessionId::new("mage-guide")
                && profession == &ProfessionId::new("immortal")
                && has_hand_to_discard_moves(card_moves, &self.player, &self.immortal_cards)
        ));
        assert_eq!(
            self.record.state().profession_for(&self.player),
            Some(&ProfessionId::new("immortal"))
        );
        assert!(self.immortal_cards.iter().all(|card| {
            self.record
                .state()
                .discard_for(&self.player)
                .unwrap()
                .contains(card)
        }));
        assert!(
            self.record
                .state()
                .formation_area(&self.player)
                .and_then(|area| area.formation.as_ref())
                .is_none()
        );
    }

    fn finish_opponent_turn(&mut self) {
        assert_eq!(self.record.state().current_player(), Some(&self.opponent));
        perform_first_elemental_attack(&mut self.record, &self.opponent);
        finish_turn(&mut self.record, &self.opponent);
    }

    fn assert_replay(&self) {
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

struct LureReturnSoulScenario {
    record: GameRecord,
    lurer: PlayerId,
    target: PlayerId,
    lure_source: CardInstanceId,
    return_soul_source: CardInstanceId,
    mage_cards: Vec<CardInstanceId>,
    water_summon_cards: Vec<CardInstanceId>,
    mage_proficiency_cards: Vec<CardInstanceId>,
}

impl LureReturnSoulScenario {
    fn new() -> Self {
        let lurer = PlayerId::new("p1");
        let target = PlayerId::new("p2");
        let mut setup = OfficialRules::new()
            .configure_game(
                vec![
                    Player {
                        id: lurer.clone(),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: target.clone(),
                        team: TeamId::new("team:p2"),
                    },
                ],
                vec![lurer.clone(), target.clone()],
                POUCH_STRATEGY_MODULES
                    .into_iter()
                    .map(RuleModuleId::new)
                    .collect(),
            )
            .unwrap();
        // Background only: the legal action turns used to establish the two scopes must not end
        // before P2 resolves Return Soul.
        for team_hp in &mut setup.hp {
            team_hp.hp = 10_000;
        }
        let mut record = GameRecord::start(setup, Vec::new()).unwrap();

        let lure_source = card_in_deck(&record, &lurer, Element::Earth, 2);
        let return_soul_source = card_in_deck(&record, &target, Element::Metal, 1);
        record
            .handle(Command::ChooseInitialPouch {
                player: lurer.clone(),
                card: lure_source,
            })
            .unwrap();
        record
            .handle(Command::ChooseInitialPouch {
                player: target.clone(),
                card: return_soul_source,
            })
            .unwrap();

        let mage_cards = cards_in_deck(&record, &target, &[(Element::Fire, 1), (Element::Fire, 2)]);
        let water_summon_cards = cards_in_deck(
            &record,
            &target,
            &[(Element::Water, 1), (Element::Water, 2)],
        );
        let triple_fire_first = card_in_deck(&record, &target, Element::Fire, 3);
        let triple_fire_second = card_in_deck(&record, &target, Element::Fire, 4);
        let triple_fire_other = card_in_deck(&record, &target, Element::Wood, 1);
        let turn_one_discard = card_in_deck(&record, &target, Element::Earth, 1);
        let mage_proficiency_cards = vec![triple_fire_first, triple_fire_second, triple_fire_other];
        let mut ordered_target_deck = mage_cards.clone();
        ordered_target_deck.extend(water_summon_cards.iter().copied());
        ordered_target_deck.push(triple_fire_first);
        ordered_target_deck.push(triple_fire_second);
        ordered_target_deck.push(triple_fire_other);
        ordered_target_deck.push(turn_one_discard);

        while let Some(request) = record.state().pending_randomness.clone() {
            let fewfc::domain::RandomnessOperation::DeckShuffle { deck } = &request.operation
            else {
                unreachable!("Pouch preparation only requests Personal Deck shuffles");
            };
            let shuffled_order = if matches!(deck, fewfc::domain::RandomnessDeck::Player(owner) if owner == &target)
            {
                append_remaining_cards(ordered_target_deck.clone(), &request.current_order)
            } else {
                request.current_order.clone()
            };
            record
                .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order,
                })
                .unwrap();
        }
        record.advance_automatic().unwrap();
        assert_eq!(record.state().current_player(), Some(&lurer));

        Self {
            record,
            lurer,
            target,
            lure_source,
            return_soul_source,
            mage_cards,
            water_summon_cards,
            mage_proficiency_cards,
        }
    }

    fn change_to_mage_through_legal_turns(&mut self) {
        self.finish_lurers_elemental_turn();
        assert_eq!(self.record.state().current_player(), Some(&self.target));
        let events = self
            .record
            .handle(Command::ChangeProfession {
                player: self.target.clone(),
                profession: ProfessionId::new("mage"),
                cards: self.mage_cards.clone(),
            })
            .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::ActionStarted { player },
                GameEvent::ProfessionChanged {
                    player: changed,
                    previous: None,
                    profession,
                    card_moves,
                },
            ] if player == &self.target
                && changed == &self.target
                && profession == &ProfessionId::new("mage")
                && has_hand_to_discard_moves(card_moves, &self.target, &self.mage_cards)
        ));
        finish_turn(&mut self.record, &self.target);
        self.finish_lurers_elemental_turn();
        assert_eq!(self.record.state().current_player(), Some(&self.target));
        assert_eq!(
            self.record.state().profession_for(&self.target),
            Some(&ProfessionId::new("mage"))
        );
        assert!(
            self.water_summon_cards
                .iter()
                .chain(self.mage_proficiency_cards.iter())
                .all(|card| self
                    .record
                    .state()
                    .hand(&self.target)
                    .unwrap()
                    .contains(card))
        );
    }

    fn assert_mage_proficiency_baseline(&self) {
        assert!(offers_formation(
            &self
                .record
                .playable_actions(&self.target, &self.mage_proficiency_cards)
                .unwrap(),
            "triple-fire",
            &self.mage_proficiency_cards,
        ));
    }

    fn summon_water_spirit_through_legal_command(&mut self) {
        let events = self
            .record
            .handle(Command::PerformFormation {
                player: self.target.clone(),
                formation_id: "water-spirit-summoning".to_string(),
                cards: self.water_summon_cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(events.iter().any(|event| {
            matches!(
                event,
                GameEvent::FormationCommitted { player, formation_id, cards, .. }
                    if player == &self.target
                        && formation_id == "water-spirit-summoning"
                        && cards == &self.water_summon_cards
            )
        }));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                GameEvent::SpiritSummoned { player, previous: None, spirit: SpiritKind::Water }
                    if player == &self.target
            )
        }));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                GameEvent::FormationCardsDiscarded { player, formation_id, cards }
                    if player == &self.target
                        && formation_id == "water-spirit-summoning"
                        && cards == &self.water_summon_cards
            )
        }));
        assert_eq!(
            self.record.state().spirit_for(&self.target),
            Some(&fewfc::domain::PlayerSpirit {
                player: self.target.clone(),
                spirit: SpiritKind::Water,
                power: 2,
            })
        );
        assert!(self.water_summon_cards.iter().all(|card| {
            self.record
                .state()
                .discard_for(&self.target)
                .unwrap()
                .contains(card)
        }));
    }

    fn advance_to_the_luring_players_turn(&mut self) {
        finish_turn(&mut self.record, &self.target);
        assert_eq!(self.record.state().current_player(), Some(&self.lurer));
    }

    fn trigger_lure_the_tiger_away(&mut self) -> Vec<GameEvent> {
        self.record
            .handle(Command::TriggerSecretStrategy {
                player: self.lurer.clone(),
                strategy: SecretStrategy::LureTheTigerAway,
                target_player: Some(self.target.clone()),
                star: None,
                break_star: false,
                discard_card: None,
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
            })
            .unwrap()
    }

    fn assert_lure_scope_outcomes(&self, events: &[GameEvent]) {
        assert!(matches!(
            events,
            [
                GameEvent::PouchRevealed {
                    player,
                    owner: Some(owner),
                    card,
                    strategy: SecretStrategy::LureTheTigerAway,
                },
                GameEvent::StatusAdded { status: profession_scope },
                GameEvent::StatusAdded { status: spirit_scope },
                GameEvent::PouchConsumed { owner: Some(consumed_owner), card: consumed_card },
            ] if player == &self.lurer
                && owner == &self.lurer
                && *card == self.lure_source
                && matches!(
                    profession_scope,
                    fewfc::domain::StatusEffect {
                        owner: fewfc::domain::StatusOwner::Player(target),
                        kind,
                        duration: fewfc::domain::StatusDuration::UntilTurnEnd { player: expires },
                        ..
                    } if target == &self.target && kind == "PouchLurePlayer" && expires == &self.target
                )
                && matches!(
                    spirit_scope,
                    fewfc::domain::StatusEffect {
                        owner: fewfc::domain::StatusOwner::Player(target),
                        kind,
                        duration: fewfc::domain::StatusDuration::UntilTurnEnd { player: expires },
                        ..
                    } if target == &self.target && kind == "PouchLureSpirit" && expires == &self.target
                )
                && consumed_owner == &self.lurer
                && *consumed_card == self.lure_source
        ));
        assert!(self.record.state().pouch_for(&self.lurer).is_none());
        assert!(
            self.record
                .state()
                .discard_for(&self.lurer)
                .unwrap()
                .contains(&self.lure_source)
        );
    }

    fn advance_to_the_lured_players_turn(&mut self) {
        self.finish_lurers_elemental_turn();
        assert_eq!(self.record.state().current_player(), Some(&self.target));
    }

    fn assert_lure_suppresses_mage_proficiency(&self) {
        assert!(has_player_status(
            self.record.state(),
            &self.target,
            "PouchLurePlayer"
        ));
        assert!(has_player_status(
            self.record.state(),
            &self.target,
            "PouchLureSpirit"
        ));
        assert!(!offers_formation(
            &self
                .record
                .playable_actions(&self.target, &self.mage_proficiency_cards)
                .unwrap(),
            "triple-fire",
            &self.mage_proficiency_cards,
        ));
    }

    fn trigger_return_soul(&mut self) -> Vec<GameEvent> {
        self.record
            .handle(Command::TriggerSecretStrategy {
                player: self.target.clone(),
                strategy: SecretStrategy::ReturnSoul,
                target_player: None,
                star: None,
                break_star: false,
                discard_card: None,
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
            })
            .unwrap()
    }

    fn assert_return_soul_replaces_water_with_metal_at_one_power(&self, events: &[GameEvent]) {
        assert!(matches!(
            events,
            [
                GameEvent::PouchRevealed {
                    player,
                    owner: Some(owner),
                    card,
                    strategy: SecretStrategy::ReturnSoul,
                },
                GameEvent::SpiritRevived { player: revived, previous: Some(SpiritKind::Water), spirit: SpiritKind::Metal, power: 1 },
                GameEvent::PouchConsumed { owner: Some(consumed_owner), card: consumed_card },
            ] if player == &self.target
                && owner == &self.target
                && *card == self.return_soul_source
                && revived == &self.target
                && consumed_owner == &self.target
                && *consumed_card == self.return_soul_source
        ));
        assert_eq!(
            self.record.state().spirit_for(&self.target),
            Some(&fewfc::domain::PlayerSpirit {
                player: self.target.clone(),
                spirit: SpiritKind::Metal,
                power: 1,
            })
        );
        assert!(self.record.state().pouch_for(&self.target).is_none());
        assert!(
            self.record
                .state()
                .discard_for(&self.target)
                .unwrap()
                .contains(&self.return_soul_source)
        );
        assert_eq!(
            self.record.state().profession_for(&self.target),
            Some(&ProfessionId::new("mage"))
        );
    }

    fn finish_lurers_elemental_turn(&mut self) {
        assert_eq!(self.record.state().current_player(), Some(&self.lurer));
        perform_first_elemental_attack(&mut self.record, &self.lurer);
        finish_turn(&mut self.record, &self.lurer);
    }

    fn assert_replay(&self) {
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

struct DeceiveHeavenScenario {
    record: GameRecord,
    deceiver: PlayerId,
    target: PlayerId,
    pouch_source: CardInstanceId,
    water_star_cards: Option<Vec<CardInstanceId>>,
    metal_star_cards: Option<Vec<CardInstanceId>>,
}

impl DeceiveHeavenScenario {
    fn temporary_star_grant() -> Self {
        Self::new(None, None)
    }

    fn direct_break() -> Self {
        Some((
            vec![
                (Element::Water, 3),
                (Element::Water, 4),
                (Element::Water, 5),
                (Element::Earth, 1),
            ],
            vec![
                (Element::Metal, 3),
                (Element::Metal, 4),
                (Element::Metal, 5),
                (Element::Wood, 1),
                (Element::Wood, 2),
            ],
        ))
        .map_or_else(
            || unreachable!(),
            |(deceiver_order, target_order)| Self::new(Some(deceiver_order), Some(target_order)),
        )
    }

    fn new(
        deceiver_order: Option<Vec<(Element, u32)>>,
        target_order: Option<Vec<(Element, u32)>>,
    ) -> Self {
        let deceiver = PlayerId::new("p1");
        let target = PlayerId::new("p2");
        let mut setup = OfficialRules::new()
            .configure_game(
                vec![
                    Player {
                        id: deceiver.clone(),
                        team: TeamId::new("team:p1"),
                    },
                    Player {
                        id: target.clone(),
                        team: TeamId::new("team:p2"),
                    },
                ],
                vec![deceiver.clone(), target.clone()],
                POUCH_STRATEGY_MODULES
                    .into_iter()
                    .map(RuleModuleId::new)
                    .collect(),
            )
            .unwrap();
        // Background only: qualifying attacks must not end the game before the direct break.
        for team_hp in &mut setup.hp {
            team_hp.hp = 10_000;
        }
        let mut record = GameRecord::start(setup, Vec::new()).unwrap();
        let pouch_source = card_in_deck(&record, &deceiver, Element::Fire, 4);
        let target_pouch = card_in_deck(&record, &target, Element::Earth, 5);
        record
            .handle(Command::ChooseInitialPouch {
                player: deceiver.clone(),
                card: pouch_source,
            })
            .unwrap();
        record
            .handle(Command::ChooseInitialPouch {
                player: target.clone(),
                card: target_pouch,
            })
            .unwrap();

        let water_star_cards = deceiver_order
            .as_ref()
            .map(|facts| cards_in_deck(&record, &deceiver, &facts[..3]));
        let metal_star_cards = target_order
            .as_ref()
            .map(|facts| cards_in_deck(&record, &target, &facts[..3]));
        let planned_deceiver_order = deceiver_order
            .as_ref()
            .map(|facts| cards_in_deck(&record, &deceiver, facts));
        let planned_target_order = target_order
            .as_ref()
            .map(|facts| cards_in_deck(&record, &target, facts));

        while let Some(request) = record.state().pending_randomness.clone() {
            let fewfc::domain::RandomnessOperation::DeckShuffle { deck } = &request.operation
            else {
                unreachable!("Pouch preparation only requests Personal Deck shuffles");
            };
            let shuffled_order = match deck {
                fewfc::domain::RandomnessDeck::Player(player) if player == &deceiver => {
                    planned_deceiver_order
                        .as_ref()
                        .map(|order| append_remaining_cards(order.clone(), &request.current_order))
                        .unwrap_or(request.current_order.clone())
                }
                fewfc::domain::RandomnessDeck::Player(player) if player == &target => {
                    planned_target_order
                        .as_ref()
                        .map(|order| append_remaining_cards(order.clone(), &request.current_order))
                        .unwrap_or(request.current_order.clone())
                }
                _ => unreachable!("Pouch preparation has one personal deck per Player"),
            };
            record
                .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                    request_id: request.request_id,
                    shuffled_order,
                })
                .unwrap();
        }
        record.advance_automatic().unwrap();
        assert_eq!(record.state().current_player(), Some(&deceiver));

        Self {
            record,
            deceiver,
            target,
            pouch_source,
            water_star_cards,
            metal_star_cards,
        }
    }

    fn trigger_deceive_heaven(&mut self, star: StarKind, break_star: bool) -> Vec<GameEvent> {
        self.record
            .handle(Command::TriggerSecretStrategy {
                player: self.deceiver.clone(),
                strategy: SecretStrategy::DeceiveHeaven,
                target_player: None,
                star: Some(star),
                break_star,
                discard_card: None,
                deck_cards: Vec::new(),
                discard_cards: Vec::new(),
            })
            .unwrap()
    }

    fn assert_temporary_star_grant(&self, events: &[GameEvent], star: StarKind) {
        assert!(matches!(
            events,
            [
                GameEvent::PouchRevealed {
                    player,
                    owner: Some(owner),
                    card,
                    strategy: SecretStrategy::DeceiveHeaven,
                },
                GameEvent::TemporaryStarEffectGranted { effect },
                GameEvent::PouchConsumed { owner: Some(consumed_owner), card: consumed_card },
            ] if player == &self.deceiver
                && owner == &self.deceiver
                && *card == self.pouch_source
                && effect.player == self.deceiver
                && effect.star == star
                && effect.applied_on_turn == self.record.state().turn_number
                && consumed_owner == &self.deceiver
                && *consumed_card == self.pouch_source
        ));
        assert!(self.record.state().team_stars.is_empty());
        assert!(
            self.record
                .state()
                .temporary_star_effects
                .iter()
                .any(|effect| {
                    effect.player == self.deceiver
                        && effect.star == star
                        && effect.applied_on_turn == self.record.state().turn_number
                })
        );
        assert!(self.record.state().pouch_for(&self.deceiver).is_none());
        assert!(
            self.record
                .state()
                .discard_for(&self.deceiver)
                .unwrap()
                .contains(&self.pouch_source)
        );
    }

    fn finish_deceivers_turn(&mut self) {
        assert_eq!(self.record.state().current_player(), Some(&self.deceiver));
        perform_first_elemental_attack(&mut self.record, &self.deceiver);
        finish_turn(&mut self.record, &self.deceiver);
    }

    fn assert_temporary_star_expired(&self) {
        assert_eq!(self.record.state().current_player(), Some(&self.target));
        assert!(self.record.state().temporary_star_effects.is_empty());
        assert!(self.record.state().team_stars.is_empty());
    }

    fn summon_water_star_for_deceiver(&mut self) {
        let cards = self
            .water_star_cards
            .as_ref()
            .expect("direct-break fixture must plan Water Star cards")
            .clone();
        let events = self.perform_qualifying_triple(&self.deceiver.clone(), "triple-water", cards);
        assert!(events.iter().any(|event| {
            matches!(
                event,
                GameEvent::StarSummoned { player, team, star: StarKind::Water }
                    if player == &self.deceiver && team == &TeamId::new("team:p1")
            )
        }));
        assert_eq!(
            self.record.state().star_for_team(&TeamId::new("team:p1")),
            Some(StarKind::Water)
        );
        finish_turn(&mut self.record, &self.deceiver);
        assert_eq!(self.record.state().current_player(), Some(&self.target));
    }

    fn summon_metal_star_for_target(&mut self) {
        assert_eq!(self.record.state().current_player(), Some(&self.target));
        let cards = self
            .metal_star_cards
            .as_ref()
            .expect("direct-break fixture must plan Metal Star cards")
            .clone();
        let events = self.perform_qualifying_triple(&self.target.clone(), "triple-metal", cards);
        assert!(events.iter().any(|event| {
            matches!(
                event,
                GameEvent::StarSummoned { player, team, star: StarKind::Metal }
                    if player == &self.target && team == &TeamId::new("team:p2")
            )
        }));
        assert_eq!(
            self.record.state().star_for_team(&TeamId::new("team:p2")),
            Some(StarKind::Metal)
        );
        self.finish_target_turn();
    }

    fn perform_qualifying_triple(
        &mut self,
        player: &PlayerId,
        formation_id: &str,
        cards: Vec<CardInstanceId>,
    ) -> Vec<GameEvent> {
        let events = self
            .record
            .handle(Command::PerformFormation {
                player: player.clone(),
                formation_id: formation_id.to_string(),
                cards: cards.clone(),
                declared_targets: Vec::new(),
            })
            .unwrap();
        assert!(events.iter().any(|event| {
            matches!(
                event,
                GameEvent::FormationCommitted { player: committed, formation_id: committed_id, cards: committed_cards, .. }
                    if committed == player && committed_id == formation_id && committed_cards == &cards
            )
        }));
        assert!(events.iter().any(|event| {
            matches!(
                event,
                GameEvent::FormationCardsDiscarded { player: discarded, formation_id: discarded_id, cards: discarded_cards }
                    if discarded == player && discarded_id == formation_id && discarded_cards == &cards
            )
        }));
        assert!(cards.iter().all(|card| {
            self.record
                .state()
                .discard_for(player)
                .unwrap()
                .contains(card)
        }));
        events
    }

    fn finish_target_turn(&mut self) {
        assert_eq!(self.record.state().current_player(), Some(&self.target));
        finish_turn(&mut self.record, &self.target);
        assert_eq!(self.record.state().current_player(), Some(&self.deceiver));
    }

    fn assert_direct_break(&self, events: &[GameEvent]) {
        assert!(matches!(
            events,
            [
                GameEvent::PouchRevealed {
                    player,
                    owner: Some(owner),
                    card,
                    strategy: SecretStrategy::DeceiveHeaven,
                },
                GameEvent::StarBroken {
                    team,
                    star: StarKind::Metal,
                    reason: StarBreakReason::SecretStrategy,
                    hp_change: None,
                },
                GameEvent::PouchConsumed { owner: Some(consumed_owner), card: consumed_card },
            ] if player == &self.deceiver
                && owner == &self.deceiver
                && *card == self.pouch_source
                && team == &TeamId::new("team:p2")
                && consumed_owner == &self.deceiver
                && *consumed_card == self.pouch_source
        ));
        assert_eq!(
            self.record.state().star_for_team(&TeamId::new("team:p2")),
            None
        );
        assert_eq!(
            self.record.state().star_for_team(&TeamId::new("team:p1")),
            Some(StarKind::Water)
        );
        assert!(self.record.state().temporary_star_effects.is_empty());
        assert!(self.record.state().pouch_for(&self.deceiver).is_none());
        assert!(
            self.record
                .state()
                .discard_for(&self.deceiver)
                .unwrap()
                .contains(&self.pouch_source)
        );
    }

    fn assert_replay(&self) {
        assert_eq!(self.record.replay().unwrap(), self.record.state().clone());
        assert_eq!(
            self.record.verify_replay().unwrap(),
            self.record.state().clone()
        );
    }
}

fn finish_turn(record: &mut GameRecord, player: &PlayerId) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("legal Formation use must create the turn-draw discard choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("turn draw must have a Card choice");
    };
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id: choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![*cards.last().unwrap()],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn finish_turn_discarding_fact(
    record: &mut GameRecord,
    player: &PlayerId,
    element: Element,
    level: u32,
) {
    record.advance_automatic().unwrap();
    let choice = record
        .state()
        .pending_choice
        .as_ref()
        .expect("legal action must create the turn-draw discard choice");
    let fewfc::domain::PendingChoiceKind::Card { cards, .. } = &choice.kind else {
        panic!("turn draw must have a Card choice");
    };
    let discard = cards
        .iter()
        .copied()
        .find(|card| {
            record.state().card_def(*card).is_some_and(|definition| {
                definition.element == element && definition.level.value() == level
            })
        })
        .expect("planned legal turn draw must offer the selected discard Card");
    record
        .handle(Command::AnswerChoice {
            player: player.clone(),
            choice_id: choice.choice_id,
            answer: ChoiceAnswer::Cards {
                cards: vec![discard],
            },
        })
        .unwrap();
    record.advance_automatic().unwrap();
}

fn perform_first_elemental_attack(record: &mut GameRecord, player: &PlayerId) {
    let card = record.state().hand(player).unwrap()[0];
    perform_elemental_attack_with_card(record, player, card);
}

fn perform_first_elemental_attack_excluding(
    record: &mut GameRecord,
    player: &PlayerId,
    excluded: &[CardInstanceId],
) {
    let card = record
        .state()
        .hand(player)
        .unwrap()
        .iter()
        .copied()
        .find(|card| !excluded.contains(card))
        .expect("legal deck exhaustion must preserve Chain's committed Cards");
    perform_elemental_attack_with_card(record, player, card);
}

fn perform_elemental_attack_with_card(
    record: &mut GameRecord,
    player: &PlayerId,
    card: CardInstanceId,
) {
    let formation_id = match record.state().card_element(card).unwrap() {
        Element::Metal => "metal-strike",
        Element::Wood => "wood-strike",
        Element::Water => "water-strike",
        Element::Fire => "fire-strike",
        Element::Earth => "earth-strike",
    };
    record
        .handle(Command::PerformFormation {
            player: player.clone(),
            formation_id: formation_id.to_string(),
            cards: vec![card],
            declared_targets: Vec::new(),
        })
        .unwrap();
}

fn append_remaining_cards(
    mut planned_cards: Vec<CardInstanceId>,
    current_order: &[CardInstanceId],
) -> Vec<CardInstanceId> {
    let remaining = current_order
        .iter()
        .copied()
        .filter(|card| !planned_cards.contains(card))
        .collect::<Vec<_>>();
    planned_cards.extend(remaining);
    assert_eq!(planned_cards.len(), current_order.len());
    planned_cards
}

fn card_in_deck(
    record: &GameRecord,
    player: &PlayerId,
    element: Element,
    level: u32,
) -> CardInstanceId {
    record
        .state()
        .deck_for(player)
        .unwrap()
        .iter()
        .copied()
        .find(|card| {
            record.state().card_def(*card).is_some_and(|definition| {
                definition.element == element && definition.level.value() == level
            })
        })
        .unwrap()
}

fn cards_in_deck(
    record: &GameRecord,
    player: &PlayerId,
    facts: &[(Element, u32)],
) -> Vec<CardInstanceId> {
    let mut used = Vec::new();
    facts
        .iter()
        .map(|(element, level)| {
            let card = record
                .state()
                .deck_for(player)
                .unwrap()
                .iter()
                .copied()
                .find(|card| {
                    !used.contains(card)
                        && record.state().card_def(*card).is_some_and(|definition| {
                            definition.element == *element && definition.level.value() == *level
                        })
                })
                .expect("planned legal Pouch deck must contain each selected Card");
            used.push(card);
            card
        })
        .collect()
}

fn has_hand_to_discard_moves(
    moves: &[fewfc::domain::CardMoveDelta],
    player: &PlayerId,
    cards: &[CardInstanceId],
) -> bool {
    moves
        == cards
            .iter()
            .map(|card| fewfc::domain::CardMoveDelta {
                card: *card,
                from: CardZone::Hand(player.clone()),
                to: CardZone::PlayerDiscard(player.clone()),
            })
            .collect::<Vec<_>>()
}

fn offers_immortal(actions: &[PlayableAction], cards: &[CardInstanceId]) -> bool {
    actions.iter().any(|action| {
        matches!(
            action,
            PlayableAction::ChangeProfession(candidate)
                if candidate.profession_id == ProfessionId::new("immortal") && candidate.cards == cards
        )
    })
}

fn offers_formation(
    actions: &[PlayableAction],
    formation_id: &str,
    cards: &[CardInstanceId],
) -> bool {
    actions.iter().any(|action| {
        matches!(
            action,
            PlayableAction::PerformFormation(candidate)
                if candidate.formation_id == formation_id && candidate.cards == cards
        )
    })
}

fn has_player_status(state: &fewfc::domain::GameState, player: &PlayerId, kind: &str) -> bool {
    state.statuses.iter().any(|status| {
        status.kind == kind
            && matches!(
                &status.owner,
                fewfc::domain::StatusOwner::Player(owner) if owner == player
            )
    })
}

#[test]
fn pouch_sheep_stealing_matrix_returns_selected_deck_cards_from_the_projected_discard_pile() {
    let mut scenario = PouchStrategyScenario::new();
    let trigger_events = scenario.trigger_sheep_stealing();
    assert!(matches!(
        trigger_events.as_slice(),
        [
            GameEvent::PouchRevealed { player, owner: Some(owner), card, strategy: SecretStrategy::SheepStealing },
            GameEvent::ChoiceRequested { .. },
        ] if player == &scenario.player
            && owner == &scenario.player
            && *card == scenario.sheep_source
    ));

    let (_, deck_cards, discard_cards) = scenario.pending_sheep_choice();
    let exchanged = deck_cards[..2].to_vec();
    assert!(discard_cards.is_empty());
    assert!(exchanged.iter().all(|card| deck_cards.contains(card)));

    let answer_events = scenario.answer_sheep_with_same_projected_cards(exchanged.clone());
    let moves = answer_events
        .iter()
        .find_map(|event| match event {
            GameEvent::CardsMoved { card_moves } => Some(card_moves),
            _ => None,
        })
        .expect("Sheep Stealing must record its exchange before shuffle");
    assert_eq!(moves.len(), 4);
    for (index, card) in exchanged.iter().enumerate() {
        assert_eq!(moves[index].card, *card);
        assert_eq!(
            moves[index].from,
            CardZone::PlayerDeckTop(scenario.player.clone())
        );
        assert_eq!(
            moves[index].to,
            CardZone::PlayerDiscard(scenario.player.clone())
        );
        assert_eq!(moves[index + 2].card, *card);
        assert_eq!(
            moves[index + 2].from,
            CardZone::PlayerDiscard(scenario.player.clone())
        );
        assert_eq!(
            moves[index + 2].to,
            CardZone::PlayerDeckTop(scenario.player.clone())
        );
    }
    assert!(matches!(
        answer_events.last(),
        Some(GameEvent::RandomnessRequested { request })
            if matches!(
                request.operation,
                fewfc::domain::RandomnessOperation::DeckShuffle {
                    deck: fewfc::domain::RandomnessDeck::Player(ref player)
                } if player == &scenario.player
            )
    ));

    let shuffle_events = scenario.resolve_pending_randomness_reversed();
    assert!(matches!(
        shuffle_events.as_slice(),
        [
            GameEvent::RandomnessResolved { .. },
            GameEvent::PouchConsumed { owner: Some(owner), card },
        ] if owner == &scenario.player && *card == scenario.sheep_source
    ));
    assert!(
        scenario
            .record
            .state()
            .deck_for(&scenario.player)
            .unwrap()
            .iter()
            .all(|card| !scenario
                .record
                .state()
                .discard_for(&scenario.player)
                .unwrap()
                .contains(card))
    );
    assert!(exchanged.iter().all(|card| {
        scenario
            .record
            .state()
            .deck_for(&scenario.player)
            .unwrap()
            .contains(card)
            && !scenario
                .record
                .state()
                .discard_for(&scenario.player)
                .unwrap()
                .contains(card)
    }));
    assert!(
        scenario
            .record
            .state()
            .discard_for(&scenario.player)
            .unwrap()
            .contains(&scenario.sheep_source)
    );
    scenario.assert_replay();
}

#[test]
fn pouch_sheep_stealing_matrix_recycles_personal_discard_before_exposing_choice_and_replays() {
    let mut scenario = PouchStrategyScenario::new();
    scenario.exhaust_player_deck_below_two_with_legal_turns();

    let trigger_events = scenario.trigger_sheep_stealing();
    assert!(matches!(
        trigger_events.as_slice(),
        [
            GameEvent::PouchRevealed { player, card, strategy: SecretStrategy::SheepStealing, .. },
            GameEvent::RandomnessRequested { request },
        ] if player == &scenario.player
            && *card == scenario.sheep_source
            && matches!(
                request.operation,
                fewfc::domain::RandomnessOperation::DiscardShuffle {
                    pile: fewfc::domain::RandomnessDeck::Player(ref player),
                    placement: fewfc::domain::DeckPlacement::Bottom,
                } if player == &scenario.player
            )
            && matches!(
                request.continuation,
                fewfc::domain::RandomnessContinuation::Pouch(
                    fewfc::domain::PouchRandomnessContinuation::SheepStealingRecycle { source_card }
                ) if source_card == scenario.sheep_source
            )
    ));
    assert!(scenario.record.state().pending_choice.is_none());

    let recycle_events = scenario.resolve_pending_randomness_reversed();
    assert!(matches!(
        recycle_events.as_slice(),
        [
            GameEvent::RandomnessResolved { .. },
            GameEvent::ChoiceRequested { choice },
        ] if matches!(choice.kind, fewfc::domain::PendingChoiceKind::SheepStealing { .. })
    ));
    let (_, deck_cards, _) = scenario.pending_sheep_choice();
    assert!(deck_cards.len() >= 2);
    scenario.assert_replay();
}

#[test]
fn pouch_chain_sheep_stealing_matrix_recycles_then_sets_aside_the_trigger_from_its_shuffle() {
    let mut scenario = ChainSheepScenario::new();
    scenario.exhaust_deck_below_two_preserving_chain_cards();

    let committed = scenario.perform_chain_and_recycle();
    assert_eq!(committed.len(), 2);
    let recycle_events = scenario.resolve_pending_randomness_reversed();
    assert!(matches!(
        recycle_events.as_slice(),
        [
            GameEvent::RandomnessResolved { operation, .. },
            GameEvent::ChoiceRequested { choice },
        ] if matches!(
            operation,
            fewfc::domain::RandomnessOperation::DiscardShuffle {
                pile: fewfc::domain::RandomnessDeck::Player(player),
                placement: fewfc::domain::DeckPlacement::Bottom,
            } if player == &scenario.player
        ) && matches!(choice.kind, fewfc::domain::PendingChoiceKind::Chain { .. })
    ));

    let (_, chain_deck_cards) = scenario.pending_chain_choice();
    let pouch_card = chain_deck_cards
        .iter()
        .copied()
        .find(|card| {
            let definition = scenario.record.state().card_def(*card).unwrap();
            definition.element != Element::Fire && definition.level.value() != 2
        })
        .expect("the legal recycled deck has a Pouch Card distinct from the Sheep trigger");
    let chain_events = scenario.answer_chain_with_sheep_stealing(pouch_card);
    assert!(matches!(
        chain_events.as_slice(),
        [
            GameEvent::ChoiceMade { answer: ChoiceAnswer::Chain { pouch_owner, pouch_card: placed, trigger_card: Some(trigger), strategy: Some(SecretStrategy::SheepStealing), .. }, .. },
            GameEvent::PouchPlaced { source, owner, card, previous: Some(_), .. },
            GameEvent::PouchRevealed { player, owner: None, card: revealed, strategy: SecretStrategy::SheepStealing },
            GameEvent::ChoiceRequested { choice },
        ] if pouch_owner == &scenario.player
            && *placed == pouch_card
            && *trigger == scenario.sheep_trigger
            && source == &scenario.player
            && owner == &scenario.player
            && *card == pouch_card
            && player == &scenario.player
            && *revealed == scenario.sheep_trigger
            && matches!(choice.kind, fewfc::domain::PendingChoiceKind::SheepStealing { .. })
    ));
    assert_eq!(
        scenario
            .record
            .state()
            .pouch_for(&scenario.player)
            .expect("Chain must replace the Player's Pouch before triggering Sheep")
            .card,
        pouch_card
    );
    assert!(
        !scenario
            .record
            .state()
            .deck_for(&scenario.player)
            .unwrap()
            .contains(&scenario.sheep_trigger)
    );
    assert!(
        scenario
            .record
            .public_events_for(Viewer::Player(scenario.player.clone()))
            .iter()
            .any(|event| {
                matches!(
                    event,
                    PublicGameEvent::PouchPlaced { owner, card: Some(card) }
                        if owner == &scenario.player && *card == pouch_card
                )
            })
    );
    assert!(
        scenario
            .record
            .public_events_for(Viewer::Player(scenario.opponent.clone()))
            .iter()
            .any(|event| {
                matches!(
                    event,
                    PublicGameEvent::PouchPlaced { owner, card: None }
                        if owner == &scenario.player
                )
            })
    );

    let (_, sheep_deck_cards) = scenario.pending_sheep_choice();
    let exchanged = sheep_deck_cards[..2].to_vec();
    let sheep_events = scenario.answer_sheep_with_same_projected_cards(exchanged.clone());
    let moves = sheep_events
        .iter()
        .find_map(|event| match event {
            GameEvent::CardsMoved { card_moves } => Some(card_moves),
            _ => None,
        })
        .expect("Chain-triggered Sheep must record the exchange before shuffle");
    assert_eq!(moves.len(), 4);
    for (index, card) in exchanged.iter().enumerate() {
        assert_eq!(moves[index].card, *card);
        assert_eq!(
            moves[index].from,
            CardZone::PlayerDeckTop(scenario.player.clone())
        );
        assert_eq!(
            moves[index].to,
            CardZone::PlayerDiscard(scenario.player.clone())
        );
        assert_eq!(moves[index + 2].card, *card);
        assert_eq!(
            moves[index + 2].from,
            CardZone::PlayerDiscard(scenario.player.clone())
        );
        assert_eq!(
            moves[index + 2].to,
            CardZone::PlayerDeckTop(scenario.player.clone())
        );
    }
    let request = scenario
        .record
        .state()
        .pending_randomness
        .as_ref()
        .expect("Sheep exchange must request its deck shuffle");
    assert!(matches!(
        request.operation,
        fewfc::domain::RandomnessOperation::DeckShuffle {
            deck: fewfc::domain::RandomnessDeck::Player(ref player)
        } if player == &scenario.player
    ));
    assert!(matches!(
        request.continuation,
        fewfc::domain::RandomnessContinuation::Pouch(
            fewfc::domain::PouchRandomnessContinuation::SheepStealing { source_card, owner: None }
        ) if source_card == scenario.sheep_trigger
    ));
    assert!(!request.current_order.contains(&scenario.sheep_trigger));

    let shuffled = scenario.resolve_pending_randomness_reversed();
    assert!(matches!(
        shuffled.as_slice(),
        [
            GameEvent::RandomnessResolved { .. },
            GameEvent::PouchConsumed { owner: None, card },
        ] if *card == scenario.sheep_trigger
    ));
    assert!(
        scenario
            .record
            .state()
            .discard_for(&scenario.player)
            .unwrap()
            .contains(&scenario.sheep_trigger)
    );
    assert!(
        !scenario
            .record
            .state()
            .deck_for(&scenario.player)
            .unwrap()
            .contains(&scenario.sheep_trigger)
    );
    assert!(exchanged.iter().all(|card| {
        scenario
            .record
            .state()
            .deck_for(&scenario.player)
            .unwrap()
            .contains(card)
            && !scenario
                .record
                .state()
                .discard_for(&scenario.player)
                .unwrap()
                .contains(card)
    }));
    scenario.assert_replay();
}

#[test]
fn pouch_steal_the_beam_mage_guide_matrix_turns_wood_five_five_four_into_a_legal_immortal_change() {
    let mut scenario = StealTheBeamMageGuideScenario::new();
    scenario.change_to_mage_guide_through_legal_turns();

    scenario.assert_printed_five_five_four_cannot_change_to_immortal();
    let trigger_events = scenario.trigger_steal_the_beam();
    scenario.assert_steal_the_beam_bonus_snapshot(&trigger_events);
    scenario.assert_effective_five_five_five_offers_immortal();
    scenario.change_to_immortal_and_assert_canonical_commitment();
    scenario.assert_replay();
}

#[test]
fn pouch_lure_return_soul_matrix_suppresses_profession_and_spirit_scopes_but_revives_at_one_power()
{
    let mut scenario = LureReturnSoulScenario::new();
    scenario.change_to_mage_through_legal_turns();
    scenario.assert_mage_proficiency_baseline();
    scenario.summon_water_spirit_through_legal_command();
    scenario.advance_to_the_luring_players_turn();

    let lure_events = scenario.trigger_lure_the_tiger_away();
    scenario.assert_lure_scope_outcomes(&lure_events);
    scenario.advance_to_the_lured_players_turn();
    scenario.assert_lure_suppresses_mage_proficiency();

    let return_soul_events = scenario.trigger_return_soul();
    scenario.assert_return_soul_replaces_water_with_metal_at_one_power(&return_soul_events);
    scenario.assert_replay();
}

#[test]
fn pouch_deceive_heaven_matrix_grants_a_temporary_star_for_its_turn_then_expires() {
    let mut scenario = DeceiveHeavenScenario::temporary_star_grant();
    let events = scenario.trigger_deceive_heaven(StarKind::Fire, false);
    scenario.assert_temporary_star_grant(&events, StarKind::Fire);
    scenario.finish_deceivers_turn();
    scenario.assert_temporary_star_expired();
    scenario.assert_replay();
}

#[test]
fn pouch_deceive_heaven_matrix_breaks_the_selected_legal_star_but_keeps_the_unaffected_sibling() {
    let mut scenario = DeceiveHeavenScenario::direct_break();
    scenario.summon_water_star_for_deceiver();
    scenario.summon_metal_star_for_target();

    let events = scenario.trigger_deceive_heaven(StarKind::Metal, true);
    scenario.assert_direct_break(&events);
    scenario.assert_replay();
}
