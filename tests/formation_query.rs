use fewfc::domain::{
    BaseChoiceContinuation, CannotPerformFormationReason, CardDef, CardDefId, CardInstanceDef,
    CardInstanceId, ChoiceContinuation, ChoiceId, GameError, GameSetup, PendingChoice,
    PendingChoiceKind, Phase, PlayerId, RuleModuleId, StatusDuration, StatusEffect, StatusOwner,
    ValidationError,
};
use fewfc::rules::{
    Element, FormationCandidate, FormationCategory, ImmediateEffect, OfficialRules, PlayableAction,
    RuleConsequence,
};

fn card(id: u64) -> CardInstanceId {
    CardInstanceId::new(id)
}

fn card_def(id: &str, element: Element) -> CardDef {
    CardDef {
        id: CardDefId::new(id),
        name: id.to_string(),
        element,
        level: fewfc::domain::PrintedCardLevel::new(1),
    }
}

fn card_instance(instance: u64, def_id: &str) -> CardInstanceDef {
    CardInstanceDef {
        instance: card(instance),
        definition: CardDefId::new(def_id),
        origin: Default::default(),
    }
}

fn setup() -> GameSetup {
    GameSetup::two_player(PlayerId::new("p1"), PlayerId::new("p2"), 30).with_cards(
        vec![
            card_def("metal", Element::Metal),
            card_def("wood", Element::Wood),
            card_def("water", Element::Water),
            card_def("fire", Element::Fire),
            card_def("earth", Element::Earth),
        ],
        (1..=25)
            .map(|id| {
                let def_id = match id % 5 {
                    1 => "metal",
                    2 => "wood",
                    3 => "water",
                    4 => "fire",
                    _ => "earth",
                };
                card_instance(id, def_id)
            })
            .collect(),
    )
}

fn formations(actions: Vec<PlayableAction>) -> Vec<FormationCandidate> {
    actions
        .into_iter()
        .filter_map(|action| match action {
            PlayableAction::PerformFormation(candidate) => Some(candidate),
            PlayableAction::ChangeProfession(_)
            | PlayableAction::ActivateProfessionAbility(_)
            | PlayableAction::UseSpiritSkill(_)
            | PlayableAction::TriggerSecretStrategy(_)
            | PlayableAction::RetrievePreviousTurnDiscard(_)
            | PlayableAction::Pass { .. } => None,
        })
        .collect()
}

#[test]
fn five_directions_legend_adds_sacred_beast_formations() {
    let rules = OfficialRules::new();
    let mut state = fewfc::domain::GameState::from_setup(
        &setup().with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]),
    );
    state.phase = Phase::Main;
    state.hands = vec![
        fewfc::domain::PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(6), card(11), card(16), card(21)],
        ),
        fewfc::domain::PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let candidates = formations(
        rules
            .playable_actions(
                &state,
                &PlayerId::new("p1"),
                &[card(1), card(6), card(11), card(16), card(21)],
            )
            .unwrap(),
    );

    assert!(candidates.iter().any(|candidate| {
        candidate.formation_id == "west-white-tiger"
            && candidate.formation_name == "西‧白虎"
            && candidate.category == FormationCategory::Attack
    }));
}

#[test]
fn five_directions_legend_adds_void_meridian_severing_technique() {
    let rules = OfficialRules::new();
    let mut state = fewfc::domain::GameState::from_setup(
        &setup().with_rule_modules(vec![RuleModuleId::new("five-directions-legend")]),
    );
    state.phase = Phase::Main;
    state.hands = vec![
        fewfc::domain::PlayerHand::new(PlayerId::new("p1"), vec![card(1), card(2), card(3)]),
        fewfc::domain::PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let candidates = formations(
        rules
            .playable_actions(&state, &PlayerId::new("p1"), &[card(1), card(2), card(3)])
            .unwrap(),
    );

    assert!(candidates.iter().any(|candidate| {
        candidate.formation_id == "void-meridian-severing"
            && candidate.formation_name == "虛空斷脈術"
            && candidate.category == FormationCategory::Spell
    }));
}

#[test]
fn playable_actions_returns_formation_candidates_from_selected_hand_cards() {
    let rules = OfficialRules::new();
    let mut state = fewfc::domain::GameState::from_setup(&setup());
    state.phase = Phase::Main;
    state.hands = vec![
        fewfc::domain::PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(2), card(3), card(4), card(5)],
        ),
        fewfc::domain::PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let candidates = formations(
        rules
            .playable_actions(&state, &PlayerId::new("p1"), &[card(1)])
            .unwrap(),
    );

    assert_eq!(candidates.len(), 1);
    assert_eq!(candidates[0].formation_id, "metal-strike");
    assert_eq!(candidates[0].formation_name, "金擊術");
    assert!(matches!(
        candidates[0].detail.consequences.as_slice(),
        [RuleConsequence::ImmediateEffect {
            effect: ImmediateEffect::Attack { .. },
            ..
        }]
    ));
    assert_eq!(candidates[0].category, FormationCategory::Attack);
    assert_eq!(candidates[0].cards, vec![card(1)]);
}

#[test]
fn playable_actions_does_not_return_matches_from_unselected_hand_cards() {
    let rules = OfficialRules::new();
    let mut state = fewfc::domain::GameState::from_setup(&setup());
    state.phase = Phase::Main;
    state.hands = vec![
        fewfc::domain::PlayerHand::new(
            PlayerId::new("p1"),
            vec![card(1), card(2), card(3), card(4), card(5)],
        ),
        fewfc::domain::PlayerHand::new(PlayerId::new("p2"), Vec::new()),
    ];

    let candidates = formations(
        rules
            .playable_actions(
                &state,
                &PlayerId::new("p1"),
                &[card(1), card(2), card(3), card(4), card(5)],
            )
            .unwrap(),
    );

    assert!(candidates.iter().any(|candidate| {
        candidate.formation_id == "five-elements-cycle"
            && candidate.formation_name == "五行輪迴"
            && candidate.category == FormationCategory::Spell
            && candidate.cards == vec![card(1), card(2), card(3), card(4), card(5)]
    }));
    assert!(
        !candidates
            .iter()
            .any(|candidate| candidate.formation_id == "metal-strike")
    );
}

#[test]
fn playable_actions_returns_error_when_player_cannot_act_now() {
    let rules = OfficialRules::new();
    let mut state = fewfc::domain::GameState::from_setup(&setup());
    state.phase = Phase::TurnDraw;

    assert_eq!(
        rules.playable_actions(&state, &PlayerId::new("p1"), &[]),
        Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::WrongPhase {
                    expected: Phase::Main,
                    actual: Phase::TurnDraw,
                },
            }
        ))
    );
}

#[test]
fn playable_actions_returns_error_for_non_current_player() {
    let rules = OfficialRules::new();
    let mut state = fewfc::domain::GameState::from_setup(&setup());
    state.phase = Phase::Main;

    assert_eq!(
        rules.playable_actions(&state, &PlayerId::new("p2"), &[]),
        Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::WrongPlayer {
                    expected: PlayerId::new("p1"),
                    actual: PlayerId::new("p2"),
                },
            }
        ))
    );
}

#[test]
fn playable_actions_returns_error_while_choice_is_pending() {
    let rules = OfficialRules::new();
    let mut state = fewfc::domain::GameState::from_setup(&setup());
    state.phase = Phase::Main;
    state.pending_choice = Some(PendingChoice {
        choice_id: ChoiceId::new(1),
        player: PlayerId::new("p1"),
        kind: PendingChoiceKind::Card {
            cards: vec![card(1)],
            minimum: 1,
            maximum: 1,
            can_decline: false,
        },
        continuation: ChoiceContinuation::Base(BaseChoiceContinuation::ChaosReturnTwo),
    });

    assert_eq!(
        rules.playable_actions(&state, &PlayerId::new("p1"), &[]),
        Err(GameError::Validation(
            ValidationError::CannotPerformFormation {
                reason: CannotPerformFormationReason::PendingChoiceInProgress {
                    player: PlayerId::new("p1"),
                },
            }
        ))
    );
}

#[test]
fn playable_actions_returns_no_formations_when_player_has_cannot_act_status() {
    let rules = OfficialRules::new();
    let mut state = fewfc::domain::GameState::from_setup(&setup());
    state.phase = Phase::Main;
    state.statuses.push(StatusEffect {
        id: "cannot-act-p1".to_string(),
        owner: StatusOwner::Player(PlayerId::new("p1")),
        kind: "CannotAct".to_string(),
        value: None,
        duration: StatusDuration::Permanent,
    });

    assert!(matches!(
        rules
            .playable_actions(&state, &PlayerId::new("p1"), &[])
            .unwrap()
            .as_slice(),
        [PlayableAction::Pass {
            reason: fewfc::domain::PassActionReason::NoCardsInHand,
        }]
    ));
}
