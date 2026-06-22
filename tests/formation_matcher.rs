use fewfc::rules::{Element, ElementCount, FormationMatcher, FormationPattern, SubmittedCardFacts};

fn card(element: Element) -> SubmittedCardFacts {
    leveled_card(element, 1)
}

fn leveled_card(element: Element, level: u32) -> SubmittedCardFacts {
    SubmittedCardFacts { element, level }
}

#[test]
fn exact_element_patterns_match_without_requiring_submitted_order() {
    let matcher = FormationMatcher::default();
    let pattern = FormationPattern::ExactElements(vec![
        Element::Metal,
        Element::Metal,
        Element::Fire,
        Element::Water,
    ]);

    assert!(matcher.matches(
        &pattern,
        &[
            card(Element::Water),
            card(Element::Metal),
            card(Element::Fire),
            card(Element::Metal)
        ]
    ));
    assert!(!matcher.matches(
        &pattern,
        &[
            card(Element::Water),
            card(Element::Metal),
            card(Element::Fire)
        ]
    ));
}

#[test]
fn element_count_patterns_match_required_counts_without_requiring_order() {
    let matcher = FormationMatcher::default();
    let pattern = FormationPattern::ElementCounts(vec![
        ElementCount {
            element: Element::Metal,
            count: 2,
        },
        ElementCount {
            element: Element::Fire,
            count: 1,
        },
    ]);

    assert!(matcher.matches(
        &pattern,
        &[
            card(Element::Fire),
            card(Element::Metal),
            card(Element::Metal)
        ]
    ));
    assert!(!matcher.matches(
        &pattern,
        &[
            card(Element::Fire),
            card(Element::Metal),
            card(Element::Water)
        ]
    ));
}

#[test]
fn generating_sequence_patterns_match_without_requiring_submitted_order() {
    let matcher = FormationMatcher::default();
    let pattern = FormationPattern::GeneratingSequence { length: 3 };

    assert!(matcher.matches(
        &pattern,
        &[
            card(Element::Earth),
            card(Element::Wood),
            card(Element::Fire)
        ]
    ));
    assert!(!matcher.matches(
        &pattern,
        &[
            card(Element::Metal),
            card(Element::Fire),
            card(Element::Water)
        ]
    ));
}

#[test]
fn overcoming_sequence_patterns_match_without_requiring_submitted_order() {
    let matcher = FormationMatcher::default();
    let pattern = FormationPattern::OvercomingSequence { length: 3 };

    assert!(matcher.matches(
        &pattern,
        &[
            card(Element::Water),
            card(Element::Wood),
            card(Element::Earth)
        ]
    ));
    assert!(!matcher.matches(
        &pattern,
        &[
            card(Element::Wood),
            card(Element::Fire),
            card(Element::Earth)
        ]
    ));
}

#[test]
fn custom_patterns_delegate_to_registered_card_fact_matcher_hooks() {
    let matcher = FormationMatcher::new().with_custom("all-water", |submitted| {
        submitted.len() == 3
            && submitted
                .iter()
                .all(|card| card.element == Element::Water && card.level > 1)
    });
    let pattern = FormationPattern::Custom("all-water".to_string());

    assert!(matcher.matches(
        &pattern,
        &[
            leveled_card(Element::Water, 2),
            leveled_card(Element::Water, 3),
            leveled_card(Element::Water, 4),
        ]
    ));
    assert!(!matcher.matches(
        &pattern,
        &[
            leveled_card(Element::Water, 2),
            leveled_card(Element::Water, 1),
            leveled_card(Element::Water, 4),
        ]
    ));
}
