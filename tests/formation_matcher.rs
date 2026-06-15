use fewfc::rules::{Element, ElementCount, FormationMatcher, FormationPattern};

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
            Element::Water,
            Element::Metal,
            Element::Fire,
            Element::Metal
        ]
    ));
    assert!(!matcher.matches(&pattern, &[Element::Water, Element::Metal, Element::Fire]));
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

    assert!(matcher.matches(&pattern, &[Element::Fire, Element::Metal, Element::Metal]));
    assert!(!matcher.matches(&pattern, &[Element::Fire, Element::Metal, Element::Water]));
}

#[test]
fn generating_sequence_patterns_match_without_requiring_submitted_order() {
    let matcher = FormationMatcher::default();
    let pattern = FormationPattern::GeneratingSequence { length: 3 };

    assert!(matcher.matches(&pattern, &[Element::Earth, Element::Wood, Element::Fire]));
    assert!(!matcher.matches(&pattern, &[Element::Metal, Element::Fire, Element::Water]));
}

#[test]
fn overcoming_sequence_patterns_match_without_requiring_submitted_order() {
    let matcher = FormationMatcher::default();
    let pattern = FormationPattern::OvercomingSequence { length: 3 };

    assert!(matcher.matches(&pattern, &[Element::Water, Element::Wood, Element::Earth]));
    assert!(!matcher.matches(&pattern, &[Element::Wood, Element::Fire, Element::Earth]));
}

#[test]
fn custom_patterns_delegate_to_registered_matcher_hooks() {
    let matcher = FormationMatcher::new().with_custom("all-water", |submitted| {
        submitted.len() == 3 && submitted.iter().all(|element| *element == Element::Water)
    });
    let pattern = FormationPattern::Custom("all-water".to_string());

    assert!(matcher.matches(&pattern, &[Element::Water, Element::Water, Element::Water]));
    assert!(!matcher.matches(&pattern, &[Element::Water, Element::Water, Element::Fire]));
}
