use fewfc::application::{GameRecord, apply_event, handle_command};
use fewfc::domain::{
    CardInstanceId, CardOrigin, CardZone, ChainPouchDecision, ChoiceAnswer, ChoiceContinuation,
    ChoiceId, Command, EffectiveCardLevel, Element, FIVE_DIRECTIONS_LEGEND_MODULE_ID, GameError,
    GameEvent, GameState, GameStatus, HERO_SCHOOLS_MODULE_ID, PERSONAL_DECK_MODULE_ID,
    POUCH_MODULE_ID, PendingChoice, PendingChoiceKind, Phase, Player, PlayerId, PlayerPouch,
    PouchChoiceContinuation, ProfessionId, RuleModuleId, SPIRIT_MODULE_ID, STAR_MODULE_ID,
    SecretStrategy, SecretStrategyDecision, SecretStrategyEnvironmentOperation,
    SecretStrategyStarOperation, SpiritKind, SpiritSkill, StarBreakReason, StarKind, TeamId,
    ValidationError,
};
use fewfc::public_view::{PublicGameEvent, Viewer, state_for};
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
                decision: SecretStrategyDecision::SheepStealing {
                    source_card: self.sheep_source,
                },
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
        // 僅是背景：在 Chain 前，數個合法攻擊回合不能結束遊戲。
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
                    decision: ChainPouchDecision::PlaceAndTrigger {
                        pouch_owner: self.player.clone(),
                        pouch_card,
                        decision: SecretStrategyDecision::SheepStealing {
                            source_card: self.sheep_trigger,
                        },
                    },
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
    later_entrant: CardInstanceId,
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
        // 僅是背景：兩個合法職業回合及對手回合不能在測試互動前結束遊戲。
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
        // 這張牌刻意排在兩次職業變更後才會抽到的位置；它不能被偷梁換柱
        // 在觸發當下的手牌快照誤納入加成。
        let later_entrant = card_in_deck(&record, &player, Element::Earth, 2);
        let mut ordered_player_deck = mage_cards.clone();
        ordered_player_deck.extend(immortal_cards[..2].iter().copied());
        ordered_player_deck.extend(mage_guide_cards.iter().copied());
        ordered_player_deck.push(turn_one_discard);
        ordered_player_deck.push(immortal_cards[2]);
        ordered_player_deck.push(turn_two_discard);
        ordered_player_deck.push(turn_two_kept_sibling);
        ordered_player_deck.push(later_entrant);

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
            later_entrant,
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
                decision: SecretStrategyDecision::NoInput {
                    source_card: self.record.state().pouch_for(&self.player).unwrap().card,
                    strategy: SecretStrategy::StealTheBeam,
                },
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

    fn resolve_turn_draw_with_a_later_entrant_outside_the_bonus_snapshot(&mut self) {
        let turn_draw_events = self.record.advance_automatic().unwrap();
        let (choice_id, drawn_cards) = match turn_draw_events.as_slice() {
            [
                GameEvent::CardsDrawnForTurnDiscardChoice {
                    player,
                    drawn_cards,
                    allowed_discards,
                },
                GameEvent::ChoiceRequested { choice },
            ] if player == &self.player
                && drawn_cards == allowed_discards
                && drawn_cards.contains(&self.later_entrant)
                && choice.player == self.player
                && matches!(
                    &choice.kind,
                    fewfc::domain::PendingChoiceKind::Card {
                        cards,
                        minimum: 1,
                        maximum: 1,
                        can_decline: false,
                    } if cards == drawn_cards
                ) =>
            {
                (choice.choice_id, drawn_cards.clone())
            }
            _ => panic!(
                "expected canonical Turn Draw choice with the later entrant, got {turn_draw_events:?}"
            ),
        };
        let discarded = drawn_cards
            .iter()
            .copied()
            .find(|card| *card != self.later_entrant)
            .expect("the legal Turn Draw pool must retain the planned later entrant");
        let answer_events = self
            .record
            .handle(Command::AnswerChoice {
                player: self.player.clone(),
                choice_id,
                answer: ChoiceAnswer::Cards {
                    cards: vec![discarded],
                },
            })
            .unwrap();
        assert_eq!(
            answer_events,
            vec![
                GameEvent::ChoiceMade {
                    player: self.player.clone(),
                    choice_id,
                    answer: ChoiceAnswer::Cards {
                        cards: vec![discarded],
                    },
                },
                GameEvent::TurnDrawResolved {
                    player: self.player.clone(),
                    discard: discarded,
                    kept_cards: drawn_cards
                        .iter()
                        .copied()
                        .filter(|card| *card != discarded)
                        .collect(),
                },
            ]
        );

        // 合法 Turn Draw 解答把保留牌放入手牌，但仍在同一回合的 TurnEnd；
        // 因此可直接對比觸發時快照牌與後入牌的有效等級。
        assert!(
            self.record
                .state()
                .hand(&self.player)
                .unwrap()
                .contains(&self.later_entrant)
        );
        assert!(
            self.record
                .state()
                .discard_for(&self.player)
                .unwrap()
                .contains(&discarded)
        );
        assert!(
            !self
                .record
                .state()
                .deck_for(&self.player)
                .unwrap()
                .contains(&self.later_entrant)
        );
        let bonus = self
            .record
            .state()
            .pouch_level_bonuses
            .iter()
            .find(|bonus| bonus.player == self.player)
            .expect("Steal the Beam bonus must survive until the natural turn end");
        assert!(!bonus.cards.contains(&self.later_entrant));
        let snapshot_card = self.immortal_cards[2];
        assert!(bonus.cards.contains(&snapshot_card));
        let snapshot_printed = self.record.state().card_def(snapshot_card).unwrap().level;
        let later_printed = self
            .record
            .state()
            .card_def(self.later_entrant)
            .unwrap()
            .level;
        assert_eq!(
            self.record
                .state()
                .effective_card_facts(&self.player, snapshot_card)
                .unwrap()
                .level,
            EffectiveCardLevel::new(snapshot_printed.value() + 1)
        );
        assert_eq!(
            self.record
                .state()
                .effective_card_facts(&self.player, self.later_entrant)
                .unwrap()
                .level,
            EffectiveCardLevel::new(later_printed.value())
        );

        assert_eq!(
            self.record.advance_automatic().unwrap(),
            vec![
                GameEvent::TurnEnded {
                    player: self.player.clone(),
                },
                GameEvent::TurnStarted {
                    player: self.opponent.clone(),
                    turn_number: 6,
                },
            ]
        );
        assert!(self.record.state().pouch_level_bonuses.is_empty());
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
        // 僅是背景：用來建立兩個範圍的合法行動回合不能在 P2 解析 Return Soul 前
        // 結束。
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
                decision: SecretStrategyDecision::TargetPlayer {
                    source_card: self.record.state().pouch_for(&self.lurer).unwrap().card,
                    target_player: self.target.clone(),
                },
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
                decision: SecretStrategyDecision::NoInput {
                    source_card: self.record.state().pouch_for(&self.target).unwrap().card,
                    strategy: SecretStrategy::ReturnSoul,
                },
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

/// 以合法轉職取得仙者，再讓地行錦囊在下一個回合壓制其冥思。牌序只安排
/// 轉職與成本所需的背景手牌；職業與 Lure 狀態一律由命令產生。
struct LureMeditationScenario {
    record: GameRecord,
    lurer: PlayerId,
    target: PlayerId,
    lure_source: CardInstanceId,
    mage_cards: Vec<CardInstanceId>,
    mage_guide_cards: Vec<CardInstanceId>,
    immortal_cards: Vec<CardInstanceId>,
    meditation_card: CardInstanceId,
    mage_turn_discard: CardInstanceId,
    mage_guide_turn_discard: CardInstanceId,
}

impl LureMeditationScenario {
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
        // 僅是背景：多個合法轉職與橋接回合不能在 Lure／冥思交互前結束。
        for team_hp in &mut setup.hp {
            team_hp.hp = 10_000;
        }
        let mut record = GameRecord::start(setup, Vec::new()).unwrap();

        let lure_source = card_in_deck(&record, &lurer, Element::Earth, 2);
        let target_pouch = card_in_deck(&record, &target, Element::Metal, 5);
        record
            .handle(Command::ChooseInitialPouch {
                player: lurer.clone(),
                card: lure_source,
            })
            .unwrap();
        record
            .handle(Command::ChooseInitialPouch {
                player: target.clone(),
                card: target_pouch,
            })
            .unwrap();

        let mage_cards = cards_in_deck(&record, &target, &[(Element::Fire, 1), (Element::Fire, 2)]);
        let immortal_cards = cards_in_deck(
            &record,
            &target,
            &[(Element::Wood, 5), (Element::Fire, 5), (Element::Earth, 5)],
        );
        let mage_guide_cards =
            cards_in_deck(&record, &target, &[(Element::Fire, 3), (Element::Fire, 3)]);
        let mage_turn_discard = card_in_deck(&record, &target, Element::Wood, 4);
        let meditation_card = card_in_deck(&record, &target, Element::Water, 4);
        let target_bridge_card = card_in_deck(&record, &target, Element::Metal, 1);
        let mage_guide_turn_discard = card_in_deck(&record, &target, Element::Earth, 1);
        let mut target_order = mage_cards.clone();
        target_order.extend(immortal_cards.iter().copied());
        target_order.extend(mage_guide_cards.iter().copied());
        target_order.push(mage_turn_discard);
        target_order.push(meditation_card);
        target_order.push(target_bridge_card);
        target_order.push(mage_guide_turn_discard);

        while let Some(request) = record.state().pending_randomness.clone() {
            let fewfc::domain::RandomnessOperation::DeckShuffle { deck } = &request.operation
            else {
                unreachable!("Pouch preparation only requests Personal Deck shuffles");
            };
            let shuffled_order = if matches!(deck, fewfc::domain::RandomnessDeck::Player(owner) if owner == &target)
            {
                append_remaining_cards(target_order.clone(), &request.current_order)
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
            mage_cards,
            mage_guide_cards,
            immortal_cards,
            meditation_card,
            mage_turn_discard,
            mage_guide_turn_discard,
        }
    }

    fn finish_lurers_ordinary_turn(&mut self) {
        assert_eq!(self.record.state().current_player(), Some(&self.lurer));
        perform_first_elemental_attack(&mut self.record, &self.lurer);
        finish_turn(&mut self.record, &self.lurer);
    }

    fn change_target_profession(
        &mut self,
        profession: &str,
        cards: &[CardInstanceId],
        previous: Option<&str>,
    ) {
        assert_eq!(self.record.state().current_player(), Some(&self.target));
        let events = self
            .record
            .handle(Command::ChangeProfession {
                player: self.target.clone(),
                profession: ProfessionId::new(profession),
                cards: cards.to_vec(),
            })
            .unwrap();
        assert!(matches!(
            events.as_slice(),
            [
                GameEvent::ActionStarted { player },
                GameEvent::ProfessionChanged {
                    player: changed,
                    previous: actual_previous,
                    profession: changed_to,
                    card_moves,
                },
            ] if player == &self.target
                && changed == &self.target
                && actual_previous.as_ref().map(ProfessionId::as_str) == previous
                && changed_to == &ProfessionId::new(profession)
                && has_hand_to_discard_moves(card_moves, &self.target, cards)
        ));
        assert_eq!(
            self.record.state().profession_for(&self.target),
            Some(&ProfessionId::new(profession))
        );
    }

    fn establish_immortal_through_legal_turns(&mut self) {
        self.finish_lurers_ordinary_turn();
        self.change_target_profession("mage", &self.mage_cards.clone(), None);
        let mage_turn_discard_level = self
            .record
            .state()
            .card_def(self.mage_turn_discard)
            .unwrap()
            .level
            .value();
        finish_turn_discarding_fact(
            &mut self.record,
            &self.target,
            Element::Wood,
            mage_turn_discard_level,
        );

        self.finish_lurers_ordinary_turn();
        self.change_target_profession("mage-guide", &self.mage_guide_cards.clone(), Some("mage"));
        let mage_guide_turn_discard_level = self
            .record
            .state()
            .card_def(self.mage_guide_turn_discard)
            .unwrap()
            .level
            .value();
        finish_turn_discarding_fact(
            &mut self.record,
            &self.target,
            Element::Earth,
            mage_guide_turn_discard_level,
        );

        self.finish_lurers_ordinary_turn();
        assert!(self.immortal_cards.iter().all(|card| {
            self.record
                .state()
                .hand(&self.target)
                .unwrap()
                .contains(card)
        }));
        self.change_target_profession("immortal", &self.immortal_cards.clone(), Some("mage-guide"));
        finish_turn(&mut self.record, &self.target);
        assert_eq!(self.record.state().current_player(), Some(&self.lurer));
    }

    fn trigger_lure(&mut self) -> Vec<GameEvent> {
        assert_eq!(self.record.state().current_player(), Some(&self.lurer));
        self.record
            .handle(Command::TriggerSecretStrategy {
                player: self.lurer.clone(),
                decision: SecretStrategyDecision::TargetPlayer {
                    source_card: self.record.state().pouch_for(&self.lurer).unwrap().card,
                    target_player: self.target.clone(),
                },
            })
            .unwrap()
    }

    fn assert_lure_lifecycle(&self, events: &[GameEvent]) {
        assert!(matches!(
            events,
            [
                GameEvent::PouchRevealed {
                    player,
                    owner: Some(owner),
                    card,
                    strategy: SecretStrategy::LureTheTigerAway,
                },
                GameEvent::StatusAdded { status: player_scope },
                GameEvent::StatusAdded { status: spirit_scope },
                GameEvent::PouchConsumed {
                    owner: Some(consumed_owner),
                    card: consumed_card,
                },
            ] if player == &self.lurer
                && owner == &self.lurer
                && *card == self.lure_source
                && matches!(player_scope, fewfc::domain::StatusEffect {
                    owner: fewfc::domain::StatusOwner::Player(owner),
                    kind,
                    duration: fewfc::domain::StatusDuration::UntilTurnEnd { player: expires },
                    ..
                } if owner == &self.target && kind == "PouchLurePlayer" && expires == &self.target)
                && matches!(spirit_scope, fewfc::domain::StatusEffect {
                    owner: fewfc::domain::StatusOwner::Player(owner),
                    kind,
                    duration: fewfc::domain::StatusDuration::UntilTurnEnd { player: expires },
                    ..
                } if owner == &self.target && kind == "PouchLureSpirit" && expires == &self.target)
                && consumed_owner == &self.lurer
                && *consumed_card == self.lure_source
        ));
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
        assert!(self.record.state().pouch_for(&self.lurer).is_none());
        assert!(
            self.record
                .state()
                .discard_for(&self.lurer)
                .unwrap()
                .contains(&self.lure_source)
        );
        let public = state_for(self.record.state(), Viewer::Observer);
        assert!(public.statuses.iter().any(|status| {
            status.owner == fewfc::domain::StatusOwner::Player(self.target.clone())
                && status.kind == "PouchLurePlayer"
        }));
        assert!(public.statuses.iter().any(|status| {
            status.owner == fewfc::domain::StatusOwner::Player(self.target.clone())
                && status.kind == "PouchLureSpirit"
        }));
    }

    fn offers_meditation(&self) -> bool {
        self.record
            .playable_actions(&self.target, &[self.meditation_card])
            .unwrap()
            .iter()
            .any(|action| {
                matches!(
                    action,
                    PlayableAction::ActivateProfessionAbility(candidate)
                        if candidate.ability_id == "meditation" && candidate.cards == vec![self.meditation_card]
                )
            })
    }

    fn activate_meditation(&mut self) -> Vec<GameEvent> {
        assert_eq!(self.record.state().current_player(), Some(&self.target));
        assert!(self.offers_meditation());
        self.record
            .handle(Command::ActivateProfessionAbility {
                player: self.target.clone(),
                ability_id: "meditation".to_string(),
                cards: vec![self.meditation_card],
                target_card: None,
                declared_element: None,
                declared_level: None,
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
        // 僅是背景：符合資格的攻擊不能在直接突破前結束遊戲。
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
                decision: SecretStrategyDecision::Star {
                    source_card: self.pouch_source,
                    operation: if break_star {
                        SecretStrategyStarOperation::Break { star }
                    } else {
                        SecretStrategyStarOperation::Gain { star }
                    },
                },
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
            GameEvent::ChoiceMade {
                answer: ChoiceAnswer::Chain {
                    decision: ChainPouchDecision::PlaceAndTrigger {
                        pouch_owner,
                        pouch_card: placed,
                        decision: SecretStrategyDecision::SheepStealing { source_card: trigger },
                    },
                },
                ..
            },
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
            GameEvent::FormationCardsDiscarded {
                player,
                formation_id,
                ..
            },
        ] if *card == scenario.sheep_trigger
            && player == &scenario.player
            && formation_id == "pouch:chain"
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
fn pouch_steal_the_beam_matrix_keeps_a_same_turn_later_entrant_outside_its_snapshot() {
    let mut scenario = StealTheBeamMageGuideScenario::new();
    scenario.change_to_mage_guide_through_legal_turns();

    scenario.assert_printed_five_five_four_cannot_change_to_immortal();
    let trigger_events = scenario.trigger_steal_the_beam();
    scenario.assert_steal_the_beam_bonus_snapshot(&trigger_events);
    scenario.assert_effective_five_five_five_offers_immortal();
    scenario.change_to_immortal_and_assert_canonical_commitment();
    scenario.resolve_turn_draw_with_a_later_entrant_outside_the_bonus_snapshot();
    scenario.assert_replay();
}

#[test]
fn pouch_lure_return_soul_matrix_removes_spirit_skill_actions_and_revives_at_one_power() {
    let mut scenario = LureReturnSoulScenario::new();
    scenario.change_to_mage_through_legal_turns();
    scenario.assert_mage_proficiency_baseline();
    scenario.summon_water_spirit_through_legal_command();
    scenario.advance_to_the_luring_players_turn();

    let lure_events = scenario.trigger_lure_the_tiger_away();
    scenario.assert_lure_scope_outcomes(&lure_events);
    scenario.advance_to_the_lured_players_turn();
    scenario.assert_lure_suppresses_mage_proficiency();

    let selected_card = scenario.mage_proficiency_cards[0];
    let actions = scenario
        .record
        .playable_actions(&scenario.target, &[selected_card])
        .unwrap();
    assert!(!actions.iter().any(|action| {
        matches!(
            action,
            PlayableAction::UseSpiritSkill(candidate) if candidate.skill == SpiritSkill::Flow
        )
    }));
    let state_before_stale_command = scenario.record.state().clone();
    let stale_command = scenario
        .record
        .handle(Command::UseSpiritSkill {
            player: scenario.target.clone(),
            skill: SpiritSkill::Flow,
            selected_card: Some(selected_card),
            declared_level: None,
        })
        .unwrap_err();
    assert!(matches!(
        stale_command,
        GameError::Validation(ValidationError::SpiritSkillUnavailable {
            spirit: SpiritKind::Water,
            skill: SpiritSkill::Flow,
        })
    ));
    assert_eq!(scenario.record.state(), &state_before_stale_command);

    let return_soul_events = scenario.trigger_return_soul();
    scenario.assert_return_soul_replaces_water_with_metal_at_one_power(&return_soul_events);
    scenario.assert_replay();
}

#[test]
fn pouch_lure_meditation_matrix_removes_the_option_and_rejects_a_stale_command() {
    // baseline：仙者以合法命令啟用冥思時，成本、使用記錄與抽牌加成都必須完整
    // 解析。
    let mut baseline = LureMeditationScenario::new();
    baseline.establish_immortal_through_legal_turns();
    baseline.finish_lurers_ordinary_turn();
    let baseline_turn = baseline.record.state().turn_number;
    let baseline_events = baseline.activate_meditation();
    assert!(matches!(
        baseline_events.as_slice(),
        [
            GameEvent::ProfessionAbilityActivated {
                player,
                ability_id,
                prepared: None,
            },
            GameEvent::CardsMoved { card_moves },
            GameEvent::TurnDrawBonusChanged {
                player: bonus_player,
                old_value: 0,
                delta: 1,
                new_value: 1,
            },
        ] if player == &baseline.target
            && ability_id == "meditation"
            && bonus_player == &baseline.target
            && has_hand_to_discard_moves(card_moves, &baseline.target, &[baseline.meditation_card])
    ));
    assert_eq!(
        baseline
            .record
            .state()
            .activated_profession_ability_turns
            .get(&baseline.target),
        Some(&baseline_turn)
    );
    assert_eq!(
        baseline
            .record
            .state()
            .turn_draw_bonus_by_player
            .get(&baseline.target),
        Some(&1)
    );
    assert!(
        baseline
            .record
            .state()
            .discard_for(&baseline.target)
            .unwrap()
            .contains(&baseline.meditation_card)
    );
    baseline.assert_replay();

    // modifier + interaction：離山以合法錦囊取得兩個獨立 scope；冥思不再是
    // 可採取動作，過期頁面提交同一命令也不得支付成本或留下使用紀錄。
    let mut interaction = LureMeditationScenario::new();
    interaction.establish_immortal_through_legal_turns();
    let lure_events = interaction.trigger_lure();
    interaction.assert_lure_lifecycle(&lure_events);
    interaction.finish_lurers_ordinary_turn();

    assert!(!interaction.offers_meditation());
    let state_before_stale_command = interaction.record.state().clone();
    let stale_command = interaction
        .record
        .handle(Command::ActivateProfessionAbility {
            player: interaction.target.clone(),
            ability_id: "meditation".to_string(),
            cards: vec![interaction.meditation_card],
            target_card: None,
            declared_element: None,
            declared_level: None,
        })
        .unwrap_err();
    assert!(matches!(
        stale_command,
        GameError::Validation(ValidationError::ProfessionAbilityUnavailable(ability_id))
            if ability_id == "meditation"
    ));
    assert_eq!(interaction.record.state(), &state_before_stale_command);
    assert_eq!(
        interaction.record.state().phase,
        fewfc::domain::Phase::ActiveEffects
    );

    // 冥思不消耗 Formation Use；仍在可提交 Formation 的 ActiveEffects，目標能完成
    // Action，兩個 Lure scope 都在目標回合自然到期。
    assert_eq!(
        interaction.record.state().phase,
        fewfc::domain::Phase::ActiveEffects
    );
    perform_first_elemental_attack(&mut interaction.record, &interaction.target);
    finish_turn(&mut interaction.record, &interaction.target);
    assert_eq!(
        interaction.record.state().current_player(),
        Some(&interaction.lurer)
    );
    assert!(!has_player_status(
        interaction.record.state(),
        &interaction.target,
        "PouchLurePlayer"
    ));
    assert!(!has_player_status(
        interaction.record.state(),
        &interaction.target,
        "PouchLureSpirit"
    ));
    assert_eq!(
        interaction
            .record
            .state()
            .profession_for(&interaction.target),
        Some(&ProfessionId::new("immortal"))
    );
    interaction.assert_replay();
}

#[test]
fn golden_cicada_lure_matrix_protects_only_player_scope_when_chain_triggers_lure() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = OfficialRules::new()
        .configure_game(
            vec![
                Player {
                    id: p1.clone(),
                    team: TeamId::new("team:p1"),
                },
                Player {
                    id: p2.clone(),
                    team: TeamId::new("team:p2"),
                },
            ],
            vec![p1.clone(), p2.clone()],
            POUCH_STRATEGY_MODULES
                .into_iter()
                .map(RuleModuleId::new)
                .collect(),
        )
        .unwrap();
    // 僅是背景：此矩陣需要多個合法轉職、召喚與 Chain 回合，不能在中途結束。
    for team_hp in &mut setup.hp {
        team_hp.hp = 10_000;
    }
    let mut record = GameRecord::start(setup, Vec::new()).unwrap();

    let p1_pouch = card_in_deck(&record, &p1, Element::Earth, 2);
    let golden_source = card_in_deck(&record, &p2, Element::Metal, 1);
    record
        .handle(Command::ChooseInitialPouch {
            player: p1.clone(),
            card: p1_pouch,
        })
        .unwrap();
    record
        .handle(Command::ChooseInitialPouch {
            player: p2.clone(),
            card: golden_source,
        })
        .unwrap();

    let mage_cards = cards_in_deck(&record, &p2, &[(Element::Fire, 1), (Element::Fire, 2)]);
    let water_summon_cards =
        cards_in_deck(&record, &p2, &[(Element::Water, 1), (Element::Water, 2)]);
    let chain_cards = cards_in_deck(
        &record,
        &p2,
        &[(Element::Metal, 2), (Element::Wood, 3), (Element::Water, 4)],
    );
    let first_draw_discard = card_in_deck(&record, &p2, Element::Wood, 5);
    let second_draw = cards_in_deck(
        &record,
        &p2,
        &[(Element::Fire, 3), (Element::Metal, 3), (Element::Earth, 2)],
    );
    let chain_pouch = card_in_deck(&record, &p2, Element::Fire, 5);
    let lure_trigger = card_in_deck(&record, &p2, Element::Earth, 1);
    let mut target_order = mage_cards.clone();
    target_order.extend(water_summon_cards.iter().copied());
    target_order.push(chain_cards[0]);
    target_order.extend(chain_cards[1..].iter().copied());
    target_order.push(first_draw_discard);
    target_order.extend(second_draw.iter().copied());
    target_order.push(chain_pouch);
    target_order.push(lure_trigger);

    while let Some(request) = record.state().pending_randomness.clone() {
        let fewfc::domain::RandomnessOperation::DeckShuffle { deck } = &request.operation else {
            unreachable!("Pouch preparation only requests Personal Deck shuffles");
        };
        let shuffled_order = if matches!(deck, fewfc::domain::RandomnessDeck::Player(owner) if owner == &p2)
        {
            append_remaining_cards(target_order.clone(), &request.current_order)
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
    assert_eq!(record.state().current_player(), Some(&p1));

    // P2 先合法成為 Mage 並召喚 Water Spirit；這些是 Lure 兩個獨立範圍的
    // 真實受測前置，而不是直接安排職業或 Spirit。
    perform_first_elemental_attack(&mut record, &p1);
    finish_turn(&mut record, &p1);
    let mage_events = record
        .handle(Command::ChangeProfession {
            player: p2.clone(),
            profession: ProfessionId::new("mage"),
            cards: mage_cards.clone(),
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
        ] if player == &p2
            && changed == &p2
            && profession == &ProfessionId::new("mage")
            && has_hand_to_discard_moves(card_moves, &p2, &mage_cards)
    ));
    finish_turn_discarding_fact(&mut record, &p2, Element::Wood, 5);
    assert_eq!(record.state().current_player(), Some(&p1));
    perform_first_elemental_attack(&mut record, &p1);
    finish_turn(&mut record, &p1);

    let summon_events = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "water-spirit-summoning".to_string(),
            cards: water_summon_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        summon_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                ..
            },
            GameEvent::SpiritSummoned {
                player: summoned,
                previous: None,
                spirit: SpiritKind::Water,
            },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded_formation,
                cards: discarded_cards,
            },
        ] if player == &p2
            && formation_id == "water-spirit-summoning"
            && cards == &water_summon_cards
            && summoned == &p2
            && discarded_by == &p2
            && discarded_formation == "water-spirit-summoning"
            && discarded_cards == cards
    ));
    finish_turn_discarding_fact(&mut record, &p2, Element::Earth, 2);
    perform_first_elemental_attack(&mut record, &p1);
    finish_turn(&mut record, &p1);
    assert_eq!(record.state().current_player(), Some(&p2));
    assert!(chain_cards.iter().all(|card| {
        record
            .state()
            .hand(&p2)
            .is_some_and(|hand| hand.contains(card))
    }));
    assert!(record.state().deck_for(&p2).unwrap().contains(&chain_pouch));
    assert!(
        record
            .state()
            .deck_for(&p2)
            .unwrap()
            .contains(&lure_trigger)
    );

    // Golden is an Active Effect in P2's protected turn. Chain is then its one
    // legal Formation Use and triggers an Earth Lure while Golden is still live.
    let golden_events = record
        .handle(Command::TriggerSecretStrategy {
            player: p2.clone(),
            decision: SecretStrategyDecision::NoInput {
                source_card: record.state().pouch_for(&p2).unwrap().card,
                strategy: SecretStrategy::GoldenCicada,
            },
        })
        .unwrap();
    assert!(matches!(
        golden_events.as_slice(),
        [
            GameEvent::PouchRevealed {
                player,
                owner: Some(owner),
                card,
                strategy: SecretStrategy::GoldenCicada,
            },
            GameEvent::StatusAdded { status },
            GameEvent::PouchConsumed {
                owner: Some(consumed_owner),
                card: consumed_card,
            },
        ] if player == &p2
            && owner == &p2
            && *card == golden_source
            && matches!(
                status,
                fewfc::domain::StatusEffect {
                    owner: fewfc::domain::StatusOwner::Player(status_owner),
                    kind,
                    duration: fewfc::domain::StatusDuration::UntilTurnEnd { player: expires },
                    ..
                } if status_owner == &p2 && kind == "PouchGoldenCicada" && expires == &p2
            )
            && consumed_owner == &p2
            && *consumed_card == golden_source
    ));
    assert!(has_player_status(record.state(), &p2, "PouchGoldenCicada"));

    let chain_events = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "pouch:chain".to_string(),
            cards: chain_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        chain_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                state: fewfc::domain::FormationAreaState::FaceUpResolving,
                ..
            },
            GameEvent::ChoiceRequested { choice },
        ] if player == &p2
            && formation_id == "pouch:chain"
            && cards == &chain_cards
            && matches!(
                choice.kind,
                fewfc::domain::PendingChoiceKind::Chain { ref pouch_owners, ref deck_cards }
                    if pouch_owners == &vec![p2.clone()]
                        && deck_cards.contains(&chain_pouch)
                        && deck_cards.contains(&lure_trigger)
            )
    ));
    let choice_id = record
        .state()
        .pending_choice
        .as_ref()
        .expect("Chain must create its typed choice")
        .choice_id;
    let lure_events = record
        .handle(Command::AnswerChoice {
            player: p2.clone(),
            choice_id,
            answer: ChoiceAnswer::Chain {
                decision: ChainPouchDecision::PlaceAndTrigger {
                    pouch_owner: p2.clone(),
                    pouch_card: chain_pouch,
                    decision: SecretStrategyDecision::TargetPlayer {
                        source_card: lure_trigger,
                        target_player: p2.clone(),
                    },
                },
            },
        })
        .unwrap();
    assert!(
        matches!(
            lure_events.as_slice(),
            [
                GameEvent::ChoiceMade {
                    player,
                    answer: ChoiceAnswer::Chain {
                        decision: ChainPouchDecision::PlaceAndTrigger {
                            pouch_owner,
                            pouch_card,
                            decision: SecretStrategyDecision::TargetPlayer {
                                source_card: trigger,
                                target_player: target,
                            },
                        },
                    },
                    ..
                },
                GameEvent::PouchPlaced {
                    source,
                    owner,
                    card: placed_card,
                    known_by,
                    previous: None,
                },
                GameEvent::PouchRevealed {
                    player: revealer,
                    owner: None,
                    card: revealed_card,
                    strategy: SecretStrategy::LureTheTigerAway,
                },
                GameEvent::StatusAdded { status: spirit_scope },
                GameEvent::PouchConsumed {
                    owner: None,
                    card: consumed_card,
                },
                GameEvent::RandomnessRequested { request },
            ] if player == &p2
                && pouch_owner == &p2
                && *pouch_card == chain_pouch
                && *trigger == lure_trigger
                && target == &p2
                && source == &p2
                && owner == &p2
                && *placed_card == chain_pouch
                && known_by == &vec![p2.clone()]
                && revealer == &p2
                && *revealed_card == lure_trigger
                && matches!(
                    spirit_scope,
                    fewfc::domain::StatusEffect {
                        owner: fewfc::domain::StatusOwner::Player(status_owner),
                        kind,
                        duration: fewfc::domain::StatusDuration::UntilTurnEnd { player: expires },
                        ..
                    } if status_owner == &p2 && kind == "PouchLureSpirit" && expires == &p2
                )
                && *consumed_card == lure_trigger
                && matches!(
                    request.continuation,
                    fewfc::domain::RandomnessContinuation::Pouch(
                        fewfc::domain::PouchRandomnessContinuation::ChainPostSearch { .. }
                    )
                )
        ),
        "unexpected Golden × Lure Chain request: {lure_events:#?}"
    );
    let request = record.state().pending_randomness.clone().unwrap();
    let mut shuffled_order = request.current_order.clone();
    shuffled_order.reverse();
    let lure_resolution_events = record
        .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
            request_id: request.request_id,
            shuffled_order,
        })
        .unwrap()
        .into_events();
    assert!(matches!(
        lure_resolution_events.as_slice(),
        [
            GameEvent::RandomnessResolved { .. },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded_formation,
                cards: discarded_cards,
            },
        ] if discarded_by == &p2
            && discarded_formation == "pouch:chain"
            && discarded_cards == &chain_cards
    ));
    // 交互：Golden 讓 Lure 不建立 Player scope，卻不擴張保護到 P2 的 Water
    // Spirit。既有 Lure-only matrix 以同樣合法 Mage/Water 前置證明此 Spirit
    // scope 的實際 Return Soul 解析；此處證明 Golden 不會改寫其 canonical scope。
    assert!(!has_player_status(record.state(), &p2, "PouchLurePlayer"));
    assert!(has_player_status(record.state(), &p2, "PouchLureSpirit"));
    assert!(has_player_status(record.state(), &p2, "PouchGoldenCicada"));
    assert_eq!(
        record.state().profession_for(&p2),
        Some(&ProfessionId::new("mage"))
    );
    assert_eq!(
        record.state().spirit_for(&p2),
        Some(&fewfc::domain::PlayerSpirit {
            player: p2.clone(),
            spirit: SpiritKind::Water,
            power: 2,
        })
    );
    assert_eq!(record.state().pouch_for(&p2).unwrap().card, chain_pouch);
    assert!(
        record
            .state()
            .discard_for(&p2)
            .unwrap()
            .contains(&golden_source)
    );
    assert!(
        record
            .state()
            .discard_for(&p2)
            .unwrap()
            .contains(&lure_trigger)
    );
    assert!(
        record
            .public_events_for(Viewer::Player(p2.clone()))
            .iter()
            .any(|event| matches!(
                event,
                PublicGameEvent::PouchPlaced { owner, card: Some(card) }
                    if owner == &p2 && *card == chain_pouch
            ))
    );
    assert!(
        record
            .public_events_for(Viewer::Player(p1.clone()))
            .iter()
            .any(|event| matches!(
                event,
                PublicGameEvent::PouchPlaced { owner, card: None }
                    if owner == &p2
            ))
    );

    assert!(
        chain_cards
            .iter()
            .all(|card| { record.state().discard_for(&p2).unwrap().contains(card) })
    );
    finish_turn(&mut record, &p2);
    assert_eq!(record.state().current_player(), Some(&p1));
    assert!(!has_player_status(record.state(), &p2, "PouchGoldenCicada"));
    assert!(!has_player_status(record.state(), &p2, "PouchLureSpirit"));
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

#[test]
fn pouch_chain_deceive_heaven_matrix_places_before_triggering_the_typed_temporary_star_effect() {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let mut setup = OfficialRules::new()
        .configure_game(
            vec![
                Player {
                    id: p1.clone(),
                    team: TeamId::new("team:p1"),
                },
                Player {
                    id: p2.clone(),
                    team: TeamId::new("team:p2"),
                },
            ],
            vec![p1.clone(), p2.clone()],
            POUCH_STRATEGY_MODULES
                .into_iter()
                .map(RuleModuleId::new)
                .collect(),
        )
        .unwrap();
    // 僅是背景：Chain 與其 typed continuation 必須在同一個合法 Action 回合
    // 完成，不能因攻擊提早結束。
    for hp in &mut setup.hp {
        hp.hp = 10_000;
    }
    let mut record = GameRecord::start(setup, Vec::new()).unwrap();
    let initial_pouch = card_in_deck(&record, &p1, Element::Earth, 5);
    let opponent_pouch = card_in_deck(&record, &p2, Element::Metal, 5);
    record
        .handle(Command::ChooseInitialPouch {
            player: p1.clone(),
            card: initial_pouch,
        })
        .unwrap();
    record
        .handle(Command::ChooseInitialPouch {
            player: p2.clone(),
            card: opponent_pouch,
        })
        .unwrap();

    let chain_cards = cards_in_deck(
        &record,
        &p1,
        &[(Element::Metal, 1), (Element::Wood, 2), (Element::Water, 3)],
    );
    let initial_hand_sibling = card_in_deck(&record, &p1, Element::Earth, 4);
    let placed_pouch = card_in_deck(&record, &p1, Element::Metal, 2);
    let deceive_trigger = card_in_deck(&record, &p1, Element::Fire, 4);
    let mut p1_order = chain_cards.clone();
    p1_order.push(initial_hand_sibling);
    p1_order.push(placed_pouch);
    p1_order.push(deceive_trigger);

    while let Some(request) = record.state().pending_randomness.clone() {
        let fewfc::domain::RandomnessOperation::DeckShuffle { deck } = &request.operation else {
            unreachable!("Pouch preparation only requests Personal Deck shuffles");
        };
        let shuffled_order = if matches!(deck, fewfc::domain::RandomnessDeck::Player(owner) if owner == &p1)
        {
            append_remaining_cards(p1_order.clone(), &request.current_order)
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
    assert_eq!(record.state().current_player(), Some(&p1));
    assert!(
        chain_cards
            .iter()
            .all(|card| record.state().hand(&p1).unwrap().contains(card))
    );

    // baseline：合法 Chain 先把其選擇變成 canonical typed Pending Choice。
    let chain_events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "pouch:chain".to_string(),
            cards: chain_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        chain_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                state: fewfc::domain::FormationAreaState::FaceUpResolving,
                ..
            },
            GameEvent::ChoiceRequested { choice },
        ] if player == &p1
            && formation_id == "pouch:chain"
            && cards == &chain_cards
            && matches!(
                choice.kind,
                fewfc::domain::PendingChoiceKind::Chain { ref pouch_owners, ref deck_cards }
                    if pouch_owners == &vec![p1.clone()]
                        && deck_cards.contains(&placed_pouch)
                        && deck_cards.contains(&deceive_trigger)
            )
    ));
    let choice_id = record
        .state()
        .pending_choice
        .as_ref()
        .expect("legal Chain must create its typed choice")
        .choice_id;

    // interaction：同一個 AnswerChoice 先替換 Pouch，才用不同的四級火行 trigger
    // 解析 Deceive Heaven；兩張牌絕不能是同一張。
    let events = record
        .handle(Command::AnswerChoice {
            player: p1.clone(),
            choice_id,
            answer: ChoiceAnswer::Chain {
                decision: ChainPouchDecision::PlaceAndTrigger {
                    pouch_owner: p1.clone(),
                    pouch_card: placed_pouch,
                    decision: SecretStrategyDecision::Star {
                        source_card: deceive_trigger,
                        operation: SecretStrategyStarOperation::Gain {
                            star: StarKind::Fire,
                        },
                    },
                },
            },
        })
        .unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::ChoiceMade {
                player,
                answer: ChoiceAnswer::Chain {
                    decision: ChainPouchDecision::PlaceAndTrigger {
                        pouch_owner,
                        pouch_card,
                        decision: SecretStrategyDecision::Star {
                            source_card: trigger,
                            operation: SecretStrategyStarOperation::Gain {
                                star: StarKind::Fire,
                            },
                        },
                    },
                },
                ..
            },
            GameEvent::PouchPlaced {
                source,
                owner,
                card,
                known_by,
                previous: Some(previous),
            },
                GameEvent::PouchRevealed {
                    player: revealer,
                    owner: None,
                    card: revealed,
                    strategy: SecretStrategy::DeceiveHeaven,
                },
                GameEvent::TemporaryStarEffectGranted { effect },
                GameEvent::PouchConsumed {
                    owner: None,
                    card: consumed,
                },
                GameEvent::RandomnessRequested { request },
            ] if player == &p1
            && pouch_owner == &p1
            && *pouch_card == placed_pouch
            && *trigger == deceive_trigger
            && source == &p1
            && owner == &p1
            && *card == placed_pouch
            && known_by == &vec![p1.clone()]
            && *previous == initial_pouch
            && revealer == &p1
            && *revealed == deceive_trigger
            && effect.player == p1
            && effect.star == StarKind::Fire
            && effect.applied_on_turn == record.state().turn_number
            && *consumed == deceive_trigger
            && matches!(
                request.continuation,
                fewfc::domain::RandomnessContinuation::Pouch(
                    fewfc::domain::PouchRandomnessContinuation::ChainPostSearch { .. }
                )
            )
    ));
    let request = record.state().pending_randomness.clone().unwrap();
    let mut shuffled_order = request.current_order.clone();
    shuffled_order.reverse();
    let shuffle_events = record
        .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
            request_id: request.request_id,
            shuffled_order,
        })
        .unwrap()
        .into_events();
    assert!(matches!(
        shuffle_events.as_slice(),
        [
            GameEvent::RandomnessResolved { .. },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id,
                cards,
            },
        ] if discarded_by == &p1
            && formation_id == "pouch:chain"
            && cards == &chain_cards
    ));
    assert_eq!(record.state().pouch_for(&p1).unwrap().card, placed_pouch);
    assert!(record.state().temporary_star_effects.iter().any(|effect| {
        effect.player == p1
            && effect.star == StarKind::Fire
            && effect.applied_on_turn == record.state().turn_number
    }));
    assert!(
        record
            .state()
            .discard_for(&p1)
            .unwrap()
            .contains(&initial_pouch)
    );
    assert!(
        record
            .state()
            .discard_for(&p1)
            .unwrap()
            .contains(&deceive_trigger)
    );
    assert!(
        chain_cards
            .iter()
            .all(|card| { record.state().discard_for(&p1).unwrap().contains(card) })
    );
    assert!(
        record
            .public_view(Viewer::Player(p1.clone()))
            .unwrap()
            .pouches
            .iter()
            .any(|pouch| pouch.owner == p1 && pouch.card == Some(placed_pouch))
    );
    assert!(
        record
            .public_view(Viewer::Player(p2.clone()))
            .unwrap()
            .pouches
            .iter()
            .any(|pouch| pouch.owner == p1 && pouch.card.is_none())
    );

    finish_turn(&mut record, &p1);
    assert_eq!(record.state().current_player(), Some(&p2));
    assert!(record.state().temporary_star_effects.is_empty());
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
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

#[test]
fn pouch_deceive_heaven_temporary_fire_star_matrix_keeps_draw_under_defense_and_preserves_water_star()
 {
    let p1 = PlayerId::new("p1");
    let p2 = PlayerId::new("p2");
    let p1_team = TeamId::new("team:p1");
    let mut setup = OfficialRules::new()
        .configure_game(
            vec![
                Player {
                    id: p1.clone(),
                    team: p1_team.clone(),
                },
                Player {
                    id: p2.clone(),
                    team: TeamId::new("team:p2"),
                },
            ],
            vec![p1.clone(), p2.clone()],
            POUCH_STRATEGY_MODULES
                .into_iter()
                .map(RuleModuleId::new)
                .collect(),
        )
        .unwrap();
    // 僅是背景：先合法召喚異名水星、再由對手覆蓋防禦的回合不能提早結束。
    for hp in &mut setup.hp {
        hp.hp = 10_000;
    }
    let mut record = GameRecord::start(setup, Vec::new()).unwrap();
    let deceive_source = card_in_deck(&record, &p1, Element::Fire, 4);
    let target_pouch = card_in_deck(&record, &p2, Element::Earth, 5);
    record
        .handle(Command::ChooseInitialPouch {
            player: p1.clone(),
            card: deceive_source,
        })
        .unwrap();
    record
        .handle(Command::ChooseInitialPouch {
            player: p2.clone(),
            card: target_pouch,
        })
        .unwrap();

    let water_summon_cards = cards_in_deck(
        &record,
        &p1,
        &[
            (Element::Water, 3),
            (Element::Water, 4),
            (Element::Water, 5),
        ],
    );
    let fire_star_cards = cards_in_deck(
        &record,
        &p1,
        &[(Element::Fire, 1), (Element::Fire, 2), (Element::Wood, 3)],
    );
    let p1_turn_draw_background = card_in_deck(&record, &p1, Element::Earth, 1);
    let defense_cards = cards_in_deck(&record, &p2, &[(Element::Wood, 1), (Element::Wood, 2)]);
    let mut p1_order = water_summon_cards.clone();
    p1_order.push(fire_star_cards[0]);
    p1_order.extend(fire_star_cards[1..].iter().copied());
    p1_order.push(p1_turn_draw_background);
    let mut p2_order = defense_cards.clone();
    p2_order.extend(cards_in_deck(
        &record,
        &p2,
        &[(Element::Fire, 1), (Element::Metal, 1), (Element::Water, 1)],
    ));

    while let Some(request) = record.state().pending_randomness.clone() {
        let fewfc::domain::RandomnessOperation::DeckShuffle { deck } = &request.operation else {
            unreachable!("Pouch preparation only requests Personal Deck shuffles");
        };
        let shuffled_order = match deck {
            fewfc::domain::RandomnessDeck::Player(owner) if owner == &p1 => {
                append_remaining_cards(p1_order.clone(), &request.current_order)
            }
            fewfc::domain::RandomnessDeck::Player(owner) if owner == &p2 => {
                append_remaining_cards(p2_order.clone(), &request.current_order)
            }
            _ => unreachable!("Pouch preparation has one Personal Deck per Player"),
        };
        record
            .resolve_randomness(fewfc::domain::TrustedRandomnessAnswer {
                request_id: request.request_id,
                shuffled_order,
            })
            .unwrap();
    }
    record.advance_automatic().unwrap();
    assert_eq!(record.state().current_player(), Some(&p1));

    // 修飾本身：P1 用合法三張水行 Formation 召喚異名水星。
    let summon_events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "triple-water".to_string(),
            cards: water_summon_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        summon_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                ..
            },
            GameEvent::AttackResolved {
                attacker,
                formation_id: attack_formation,
                ..
            },
            GameEvent::StarSummoned {
                player: summoner,
                team,
                star: StarKind::Water,
            },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded_formation,
                cards: discarded_cards,
            },
        ] if player == &p1
            && formation_id == "triple-water"
            && cards == &water_summon_cards
            && attacker == &p1
            && attack_formation == "triple-water"
            && summoner == &p1
            && team == &p1_team
            && discarded_by == &p1
            && discarded_formation == "triple-water"
            && discarded_cards == &water_summon_cards
    ));
    assert_eq!(
        record.state().star_for_team(&p1_team),
        Some(StarKind::Water)
    );
    assert_eq!(
        record.public_view(Viewer::Observer).unwrap().team_stars,
        vec![fewfc::domain::TeamStar {
            team: p1_team.clone(),
            star: StarKind::Water,
        }]
    );
    finish_turn(&mut record, &p1);

    // P2 的 Defense 也由完整命令流程覆蓋，並對 P1 保持蓋牌隱私。
    assert_eq!(record.state().current_player(), Some(&p2));
    let defense_events = record
        .handle(Command::PerformFormation {
            player: p2.clone(),
            formation_id: "defense".to_string(),
            cards: defense_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        defense_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                state: fewfc::domain::FormationAreaState::FaceDownResolving,
                ..
            },
            GameEvent::PassiveCovered {
                player: covered_by,
                formation_id: covered,
                cards: covered_cards,
                sealed: false,
                ..
            },
        ] if player == &p2
            && formation_id == "defense"
            && cards == &defense_cards
            && covered_by == &p2
            && covered == "defense"
            && covered_cards == &defense_cards
    ));
    assert!(matches!(
        record
            .public_view(Viewer::Player(p1.clone()))
            .unwrap()
            .covered_passives
            .as_slice(),
        [fewfc::public_view::PublicCoveredPassive {
            owner,
            formation_id: None,
            cards: fewfc::public_view::PublicCardRefs::Hidden { count: 2 },
            star_substitution: None,
        }] if owner == &p2
    ));
    finish_turn(&mut record, &p2);
    assert_eq!(record.state().current_player(), Some(&p1));

    // baseline：異名水星不會讓火星 Formation 出現在可用行動中。
    assert!(
        !record
            .playable_actions(&p1, &fire_star_cards)
            .unwrap()
            .iter()
            .any(|action| matches!(
                action,
                PlayableAction::PerformFormation(candidate)
                    if candidate.formation_id == "yinghuo-heaven-blazing"
            ))
    );

    // interaction：四級火行錦囊合法給予當回合火星；Defense 只阻止傷害，而不
    // 阻止星陣自身的抽牌＋1，也不會破除異名水星。
    let deceive_events = record
        .handle(Command::TriggerSecretStrategy {
            player: p1.clone(),
            decision: SecretStrategyDecision::Star {
                source_card: record.state().pouch_for(&p1).unwrap().card,
                operation: SecretStrategyStarOperation::Gain {
                    star: StarKind::Fire,
                },
            },
        })
        .unwrap();
    assert!(matches!(
        deceive_events.as_slice(),
        [
            GameEvent::PouchRevealed {
                player,
                owner: Some(owner),
                card,
                strategy: SecretStrategy::DeceiveHeaven,
            },
            GameEvent::TemporaryStarEffectGranted { effect },
            GameEvent::PouchConsumed {
                owner: Some(consumed_owner),
                card: consumed_card,
            },
        ] if player == &p1
            && owner == &p1
            && *card == deceive_source
            && effect.player == p1
            && effect.star == StarKind::Fire
            && effect.applied_on_turn == record.state().turn_number
            && consumed_owner == &p1
            && *consumed_card == deceive_source
    ));
    assert!(record.state().temporary_star_effects.iter().any(|effect| {
        effect.player == p1
            && effect.star == StarKind::Fire
            && effect.applied_on_turn == record.state().turn_number
    }));
    assert!(record
        .playable_actions(&p1, &fire_star_cards)
        .unwrap()
        .iter()
        .any(|action| matches!(
            action,
            PlayableAction::PerformFormation(candidate)
                if candidate.formation_id == "yinghuo-heaven-blazing" && candidate.cards == fire_star_cards
        )));

    let star_formation_events = record
        .handle(Command::PerformFormation {
            player: p1.clone(),
            formation_id: "yinghuo-heaven-blazing".to_string(),
            cards: fire_star_cards.clone(),
            declared_targets: Vec::new(),
        })
        .unwrap();
    assert!(matches!(
        star_formation_events.as_slice(),
        [
            GameEvent::FormationCommitted {
                player,
                formation_id,
                cards,
                ..
            },
            GameEvent::PassiveFlipped {
                owner,
                passive_id,
                outcome: fewfc::domain::PassiveFlipOutcome::Applied { modifications, .. },
                ..
            },
            GameEvent::AttackResolved {
                attacker,
                formation_id: attack_formation,
                point_breakdown,
                hp_change,
                elemental_context_update: Some(effects),
                ..
            },
            GameEvent::FormationCardsDiscarded {
                player: discarded_by,
                formation_id: discarded_formation,
                cards: discarded_cards,
            },
        ] if player == &p1
            && formation_id == "yinghuo-heaven-blazing"
            && cards == &fire_star_cards
            && owner == &p2
            && passive_id == "defense"
            && modifications == &vec![fewfc::domain::ActionModification::PreventDamage]
            && attacker == &p1
            && attack_formation == "yinghuo-heaven-blazing"
            && point_breakdown.base_points == 18
            && point_breakdown.final_amount == 18
            && hp_change.effective_delta == 0
            && effects.turn_draw_bonus_changes
                == vec![fewfc::domain::TurnDrawBonusDelta {
                    player: p1.clone(),
                    old_value: 0,
                    delta: 1,
                    new_value: 1,
                }]
            && discarded_by == &p1
            && discarded_formation == "yinghuo-heaven-blazing"
            && discarded_cards == &fire_star_cards
    ));
    assert!(
        !star_formation_events
            .iter()
            .any(|event| matches!(event, GameEvent::StarBroken { .. }))
    );
    assert!(record.state().covered_passive(&p2).is_none());
    assert_eq!(
        record.state().star_for_team(&p1_team),
        Some(StarKind::Water)
    );
    assert_eq!(record.state().turn_draw_bonus_by_player.get(&p1), Some(&1));
    for card in water_summon_cards
        .iter()
        .chain(fire_star_cards.iter())
        .chain(defense_cards.iter())
        .chain(std::iter::once(&deceive_source))
    {
        assert!(
            record
                .state()
                .discard_for(&p1)
                .is_some_and(|discard| discard.contains(card))
                || record
                    .state()
                    .discard_for(&p2)
                    .is_some_and(|discard| discard.contains(card))
        );
    }

    finish_turn(&mut record, &p1);
    assert_eq!(record.state().current_player(), Some(&p2));
    assert!(record.state().temporary_star_effects.is_empty());
    assert_eq!(
        record.state().star_for_team(&p1_team),
        Some(StarKind::Water)
    );
    assert_eq!(record.replay().unwrap(), record.state().clone());
    assert_eq!(record.verify_replay().unwrap(), record.state().clone());
}

/// 以最小可互動狀態固定決策邊界：這些測試不依賴 UI offer，直接驗證 Rust
/// Rules Engine 對封閉 Decision 的重新驗證、事件順序與 Card Origin。
fn ready_direct_pouch_state(
    pouch_element: Element,
    pouch_level: u32,
) -> (GameState, PlayerId, PlayerId, CardInstanceId) {
    let player = PlayerId::new("p1");
    let opponent = PlayerId::new("p2");
    let setup = OfficialRules::new()
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
    let mut state = GameState::from_setup(&setup);
    state.status = GameStatus::InProgress;
    state.phase = Phase::ActiveEffects;

    for owner in [&player, &opponent] {
        let deck = state
            .card_instances
            .iter()
            .filter_map(|instance| {
                matches!(&instance.origin, CardOrigin::Player(card_owner) if card_owner == owner)
                    .then_some(instance.instance)
            })
            .collect();
        *state.deck_for_mut(owner).unwrap() = deck;
    }

    let source_card = card_in_state_deck(&state, &player, pouch_element, pouch_level);
    state
        .deck_for_mut(&player)
        .unwrap()
        .retain(|card| *card != source_card);
    state.pouches.push(PlayerPouch {
        owner: player.clone(),
        card: source_card,
        known_by: vec![player.clone()],
    });
    (state, player, opponent, source_card)
}

fn move_owned_deck_card_to_hand(
    state: &mut GameState,
    player: &PlayerId,
    element: Element,
) -> CardInstanceId {
    let card = state
        .deck_for(player)
        .unwrap()
        .iter()
        .copied()
        .find(|card| state.card_element(*card) == Some(element))
        .expect("fixture requires an owned card of the requested element");
    state
        .deck_for_mut(player)
        .unwrap()
        .retain(|candidate| *candidate != card);
    state.hand_mut(player).unwrap().push(card);
    card
}

fn chain_choice(state: &mut GameState, player: &PlayerId, deck_cards: Vec<CardInstanceId>) {
    state.pending_choice = Some(PendingChoice {
        choice_id: ChoiceId::new(1),
        player: player.clone(),
        kind: PendingChoiceKind::Chain {
            pouch_owners: vec![player.clone()],
            deck_cards,
        },
        continuation: ChoiceContinuation::Pouch(PouchChoiceContinuation::Chain),
    });
}

#[test]
fn pouch_retreat_matrix_preserves_no_change_cases_and_card_origin() {
    let (clear_state, player, _, source_card) = ready_direct_pouch_state(Element::Fire, 5);
    let clear_events = handle_command(
        &clear_state,
        Command::TriggerSecretStrategy {
            player: player.clone(),
            decision: SecretStrategyDecision::Environment {
                source_card,
                operation: SecretStrategyEnvironmentOperation::Clear,
            },
        },
    )
    .unwrap();
    assert_eq!(
        clear_events,
        vec![
            GameEvent::PouchRevealed {
                player: player.clone(),
                owner: Some(player.clone()),
                card: source_card,
                strategy: SecretStrategy::Retreat,
            },
            GameEvent::PouchConsumed {
                owner: Some(player.clone()),
                card: source_card,
            },
        ],
        "Clear without an Environment is legal, reveals, and consumes without a clear event",
    );

    let (mut no_environment, player, _, source_card) = ready_direct_pouch_state(Element::Fire, 5);
    let ordinary_card = move_owned_deck_card_to_hand(&mut no_environment, &player, Element::Wood);
    let transfer_events = handle_command(
        &no_environment,
        Command::TriggerSecretStrategy {
            player: player.clone(),
            decision: SecretStrategyDecision::Environment {
                source_card,
                operation: SecretStrategyEnvironmentOperation::TransferByDiscard {
                    card: ordinary_card,
                },
            },
        },
    )
    .unwrap();
    assert!(matches!(
        transfer_events.as_slice(),
        [
            GameEvent::PouchRevealed { strategy: SecretStrategy::Retreat, .. },
            GameEvent::CardsMoved { card_moves },
            GameEvent::EnvironmentTransferred { from: None, to: Element::Wood, .. },
            GameEvent::PouchConsumed { .. },
        ] if card_moves == &vec![fewfc::domain::CardMoveDelta {
            card: ordinary_card,
            from: CardZone::Hand(player.clone()),
            to: CardZone::PlayerDiscard(player.clone()),
        }]
    ));
    for event in &transfer_events {
        apply_event(&mut no_environment, event);
    }
    assert_eq!(no_environment.environment, Some(Element::Wood));
    assert!(
        no_environment
            .discard_for(&player)
            .is_some_and(|discard| discard.contains(&ordinary_card))
    );

    let (mut same_environment, player, _, source_card) = ready_direct_pouch_state(Element::Fire, 5);
    same_environment.environment = Some(Element::Fire);
    let same_element_card =
        move_owned_deck_card_to_hand(&mut same_environment, &player, Element::Fire);
    let same_element_events = handle_command(
        &same_environment,
        Command::TriggerSecretStrategy {
            player: player.clone(),
            decision: SecretStrategyDecision::Environment {
                source_card,
                operation: SecretStrategyEnvironmentOperation::TransferByDiscard {
                    card: same_element_card,
                },
            },
        },
    )
    .unwrap();
    assert!(same_element_events.iter().any(|event| matches!(
        event,
        GameEvent::EnvironmentTransferred {
            from: Some(Element::Fire),
            to: Element::Fire,
            ..
        }
    )));

    let (mut foreign_state, player, opponent, source_card) =
        ready_direct_pouch_state(Element::Fire, 5);
    let foreign_card = foreign_state.deck_for(&opponent).unwrap()[0];
    foreign_state
        .deck_for_mut(&opponent)
        .unwrap()
        .retain(|card| *card != foreign_card);
    foreign_state.hand_mut(&player).unwrap().push(foreign_card);
    foreign_state.exposed_foreign_cards.push(foreign_card);
    let foreign_events = handle_command(
        &foreign_state,
        Command::TriggerSecretStrategy {
            player: player.clone(),
            decision: SecretStrategyDecision::Environment {
                source_card,
                operation: SecretStrategyEnvironmentOperation::TransferByDiscard {
                    card: foreign_card,
                },
            },
        },
    )
    .unwrap();
    assert!(matches!(
        foreign_events.as_slice(),
        [
            GameEvent::PouchRevealed { strategy: SecretStrategy::Retreat, .. },
            GameEvent::CardsMoved { card_moves },
            GameEvent::EnvironmentTransferred { .. },
            GameEvent::PouchConsumed { .. },
        ] if card_moves == &vec![fewfc::domain::CardMoveDelta {
            card: foreign_card,
            from: CardZone::Hand(player.clone()),
            to: CardZone::PlayerDiscard(opponent.clone()),
        }]
    ));
    for event in &foreign_events {
        apply_event(&mut foreign_state, event);
    }
    assert!(
        foreign_state
            .discard_for(&opponent)
            .is_some_and(|discard| discard.contains(&foreign_card))
    );
    assert!(!foreign_state.exposed_foreign_cards.contains(&foreign_card));
}

#[test]
fn pouch_secret_strategy_validation_failures_are_atomic_before_reveal_or_choice_made() {
    let (star_state, player, _, source_card) = ready_direct_pouch_state(Element::Fire, 4);
    let before_star = star_state.clone();
    assert_eq!(
        handle_command(
            &star_state,
            Command::TriggerSecretStrategy {
                player: player.clone(),
                decision: SecretStrategyDecision::Star {
                    source_card,
                    operation: SecretStrategyStarOperation::Break {
                        star: StarKind::Fire
                    },
                },
            },
        ),
        Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid
        )),
        "a stale Deceive Heaven Break must not become a legal no-op",
    );
    assert_eq!(star_state, before_star);

    let forged_source = card_in_state_deck(&star_state, &player, Element::Metal, 1);
    assert_eq!(
        handle_command(
            &star_state,
            Command::TriggerSecretStrategy {
                player: player.clone(),
                decision: SecretStrategyDecision::NoInput {
                    source_card: forged_source,
                    strategy: SecretStrategy::GoldenCicada,
                },
            },
        ),
        Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid
        )),
        "a Decision source must exactly match the current Pouch",
    );
    assert_eq!(
        handle_command(
            &star_state,
            Command::TriggerSecretStrategy {
                player: player.clone(),
                decision: SecretStrategyDecision::NoInput {
                    source_card,
                    strategy: SecretStrategy::GoldenCicada,
                },
            },
        ),
        Err(GameError::Validation(
            ValidationError::SecretStrategyConditionMismatch
        )),
        "a closed input family still revalidates its strategy against the source card",
    );
    assert_eq!(star_state, before_star);

    let (mut chain_state, player, _, _) = ready_direct_pouch_state(Element::Metal, 1);
    let pouch_card = card_in_state_deck(&chain_state, &player, Element::Wood, 2);
    let trigger_card = card_in_state_deck(&chain_state, &player, Element::Fire, 4);
    chain_choice(&mut chain_state, &player, vec![pouch_card, trigger_card]);
    let before_chain = chain_state.clone();
    assert_eq!(
        handle_command(
            &chain_state,
            Command::AnswerChoice {
                player: player.clone(),
                choice_id: ChoiceId::new(1),
                answer: ChoiceAnswer::Chain {
                    decision: ChainPouchDecision::PlaceAndTrigger {
                        pouch_owner: player.clone(),
                        pouch_card,
                        decision: SecretStrategyDecision::Star {
                            source_card: trigger_card,
                            operation: SecretStrategyStarOperation::Break {
                                star: StarKind::Fire
                            },
                        },
                    },
                },
            },
        ),
        Err(GameError::Validation(
            ValidationError::SecretStrategyInputInvalid
        )),
        "the Chain envelope must validate before ChoiceMade, PouchPlaced, or PouchRevealed",
    );
    assert_eq!(chain_state, before_chain);
}

#[test]
fn pouch_chain_place_only_uses_its_closed_envelope_without_a_strategy_decision() {
    let (mut state, player, _, previous_pouch) = ready_direct_pouch_state(Element::Metal, 1);
    let pouch_card = card_in_state_deck(&state, &player, Element::Wood, 2);
    chain_choice(&mut state, &player, vec![pouch_card]);

    let events = handle_command(
        &state,
        Command::AnswerChoice {
            player: player.clone(),
            choice_id: ChoiceId::new(1),
            answer: ChoiceAnswer::Chain {
                decision: ChainPouchDecision::PlaceOnly {
                    pouch_owner: player.clone(),
                    pouch_card,
                },
            },
        },
    )
    .unwrap();
    assert!(matches!(
        events.as_slice(),
        [
            GameEvent::ChoiceMade {
                answer: ChoiceAnswer::Chain {
                    decision: ChainPouchDecision::PlaceOnly { .. },
                },
                ..
            },
            GameEvent::PouchPlaced { card, previous: Some(previous), .. },
            GameEvent::RandomnessRequested { request },
        ] if *card == pouch_card
            && *previous == previous_pouch
            && matches!(
                request.continuation,
                fewfc::domain::RandomnessContinuation::Pouch(
                    fewfc::domain::PouchRandomnessContinuation::ChainPostSearch { .. }
                )
            )
    ));
}

fn card_in_state_deck(
    state: &GameState,
    player: &PlayerId,
    element: Element,
    level: u32,
) -> CardInstanceId {
    state
        .deck_for(player)
        .unwrap()
        .iter()
        .copied()
        .find(|card| {
            state.card_def(*card).is_some_and(|definition| {
                definition.element == element && definition.level.value() == level
            })
        })
        .expect("fixture requires a matching personal-deck card")
}
