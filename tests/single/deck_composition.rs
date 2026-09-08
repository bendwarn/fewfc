use fewfc::domain::{CardDefId, PlayerDeckList, PlayerId};
use fewfc::rules::{DeckListSource, OfficialRules};

#[test]
fn deck_composition_catalog_and_resolution_share_one_authoritative_interface() {
    let rules = OfficialRules::new();
    let catalog = rules.deck_composition_catalog();

    assert_eq!(catalog.card_definitions.len(), 25);
    assert_eq!(catalog.shared_deck.exact_card_count, 90);
    assert_eq!(
        catalog
            .card_definitions
            .iter()
            .map(|definition| definition.shared_deck_copies)
            .sum::<usize>(),
        90
    );
    assert_eq!(catalog.personal_deck.exact_card_count, 60);
    assert_eq!(catalog.personal_deck.maximum_level_total, 170);
    assert_eq!(catalog.personal_deck.preconstructed.cards.len(), 60);

    for definition in &catalog.card_definitions {
        assert_eq!(
            definition.shared_deck_copies,
            if definition.level <= 3 { 4 } else { 3 }
        );
        assert_eq!(
            definition.personal_deck_copy_limit,
            if definition.level <= 3 { 4 } else { 3 }
        );
        assert_eq!(
            definition.preconstructed_copies,
            [3, 2, 3, 2, 2][definition.level as usize - 1]
        );
    }

    let player = PlayerId::new("p1");
    let invalid = PlayerDeckList {
        player: player.clone(),
        name: "invalid".to_string(),
        cards: vec![CardDefId::new("metal-1")],
    };
    let resolved = rules.resolve_personal_deck(player.clone(), Some(invalid));
    assert_eq!(resolved.source, DeckListSource::Preconstructed);
    assert_eq!(resolved.effective.player, player);
    assert_eq!(resolved.effective.cards.len(), 60);
    assert!(
        resolved
            .candidate_validation
            .is_some_and(|result| !result.valid)
    );
}
