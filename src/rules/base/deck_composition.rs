use crate::domain::{
    CardDef, CardDefId, CardInstanceDef, CardInstanceId, CardOrigin, Element, PlayerDeckList,
    PlayerId,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

const PERSONAL_DECK_CARD_COUNT: usize = 60;
const PERSONAL_DECK_LEVEL_LIMIT: u32 = 170;
const SHARED_DECK_CARD_COUNT: usize = 90;
const PRECONSTRUCTED_DECK_NAME: &str = "五行均衡預組";
const PRECONSTRUCTED_COPIES_BY_LEVEL: [usize; 5] = [3, 2, 3, 2, 2];

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCompositionCatalog {
    pub card_definitions: Vec<DeckCompositionCardDefinition>,
    pub shared_deck: SharedDeckComposition,
    pub personal_deck: PersonalDeckComposition,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SharedDeckComposition {
    pub exact_card_count: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckCompositionCardDefinition {
    pub id: CardDefId,
    pub name: String,
    pub element: Element,
    pub level: u32,
    pub shared_deck_copies: usize,
    pub personal_deck_copy_limit: usize,
    pub preconstructed_copies: usize,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalDeckComposition {
    pub exact_card_count: usize,
    pub maximum_level_total: u32,
    pub preconstructed: DeckListTemplate,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckListTemplate {
    pub name: String,
    pub cards: Vec<CardDefId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeckListValidation {
    pub valid: bool,
    pub card_count: usize,
    pub level_total: u32,
    pub issues: Vec<DeckListIssue>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "code", rename_all = "kebab-case")]
pub enum DeckListIssue {
    WrongPlayer,
    UnknownCardDefinition {
        card_definition: CardDefId,
    },
    WrongCardCount {
        expected: usize,
        actual: usize,
    },
    LevelTotalExceeded {
        maximum: u32,
        actual: u32,
    },
    CopyLimitExceeded {
        card_definition: CardDefId,
        maximum: usize,
        actual: usize,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DeckListSource {
    Custom,
    Preconstructed,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ResolvedPersonalDeck {
    pub candidate_validation: Option<DeckListValidation>,
    pub source: DeckListSource,
    pub effective: PlayerDeckList,
    pub effective_validation: DeckListValidation,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct DeckComposition;

impl DeckComposition {
    pub(crate) fn catalog(self) -> DeckCompositionCatalog {
        DeckCompositionCatalog {
            card_definitions: official_card_defs()
                .into_iter()
                .map(|definition| DeckCompositionCardDefinition {
                    id: definition.id,
                    name: definition.name,
                    element: definition.element,
                    level: definition.level,
                    shared_deck_copies: official_copy_count(definition.level),
                    personal_deck_copy_limit: personal_copy_limit(definition.level),
                    preconstructed_copies: preconstructed_copy_count(definition.level),
                })
                .collect(),
            shared_deck: SharedDeckComposition {
                exact_card_count: SHARED_DECK_CARD_COUNT,
            },
            personal_deck: PersonalDeckComposition {
                exact_card_count: PERSONAL_DECK_CARD_COUNT,
                maximum_level_total: PERSONAL_DECK_LEVEL_LIMIT,
                preconstructed: DeckListTemplate {
                    name: PRECONSTRUCTED_DECK_NAME.to_string(),
                    cards: preconstructed_cards(),
                },
            },
        }
    }

    pub(crate) fn official_card_defs(self) -> Vec<CardDef> {
        official_card_defs()
    }

    pub(crate) fn official_card_instances(self) -> Vec<CardInstanceDef> {
        let mut next_instance = 1;
        let mut instances = Vec::new();
        for definition in official_card_defs() {
            for _ in 0..official_copy_count(definition.level) {
                instances.push(CardInstanceDef {
                    instance: CardInstanceId::new(next_instance),
                    definition: definition.id.clone(),
                    origin: CardOrigin::Shared,
                });
                next_instance += 1;
            }
        }
        instances
    }

    pub(crate) fn preconstructed_deck(self, player: PlayerId) -> PlayerDeckList {
        PlayerDeckList {
            player,
            name: PRECONSTRUCTED_DECK_NAME.to_string(),
            cards: preconstructed_cards(),
        }
    }

    pub(crate) fn validate(self, player: &PlayerId, deck: &PlayerDeckList) -> DeckListValidation {
        let definitions = official_card_defs()
            .into_iter()
            .map(|definition| (definition.id.clone(), definition))
            .collect::<HashMap<_, _>>();
        let mut issues = Vec::new();
        let mut level_total = 0;
        let mut copies = HashMap::<CardDefId, usize>::new();

        if &deck.player != player {
            issues.push(DeckListIssue::WrongPlayer);
        }
        for card in &deck.cards {
            if let Some(definition) = definitions.get(card) {
                level_total += definition.level;
                *copies.entry(card.clone()).or_default() += 1;
            } else {
                issues.push(DeckListIssue::UnknownCardDefinition {
                    card_definition: card.clone(),
                });
            }
        }
        if deck.cards.len() != PERSONAL_DECK_CARD_COUNT {
            issues.push(DeckListIssue::WrongCardCount {
                expected: PERSONAL_DECK_CARD_COUNT,
                actual: deck.cards.len(),
            });
        }
        if level_total > PERSONAL_DECK_LEVEL_LIMIT {
            issues.push(DeckListIssue::LevelTotalExceeded {
                maximum: PERSONAL_DECK_LEVEL_LIMIT,
                actual: level_total,
            });
        }
        for definition in official_card_defs() {
            let actual = copies.get(&definition.id).copied().unwrap_or_default();
            let maximum = personal_copy_limit(definition.level);
            if actual > maximum {
                issues.push(DeckListIssue::CopyLimitExceeded {
                    card_definition: definition.id,
                    maximum,
                    actual,
                });
            }
        }

        DeckListValidation {
            valid: issues.is_empty(),
            card_count: deck.cards.len(),
            level_total,
            issues,
        }
    }

    pub(crate) fn resolve(
        self,
        player: PlayerId,
        candidate: Option<PlayerDeckList>,
    ) -> ResolvedPersonalDeck {
        let candidate_validation = candidate.as_ref().map(|deck| self.validate(&player, deck));
        let custom = candidate.filter(|_| {
            candidate_validation
                .as_ref()
                .is_some_and(|result| result.valid)
        });
        match custom {
            Some(effective) => {
                let effective_validation = self.validate(&player, &effective);
                ResolvedPersonalDeck {
                    candidate_validation,
                    source: DeckListSource::Custom,
                    effective,
                    effective_validation,
                }
            }
            None => {
                let effective = self.preconstructed_deck(player.clone());
                let effective_validation = self.validate(&player, &effective);
                ResolvedPersonalDeck {
                    candidate_validation,
                    source: DeckListSource::Preconstructed,
                    effective,
                    effective_validation,
                }
            }
        }
    }
}

fn official_card_defs() -> Vec<CardDef> {
    elements()
        .into_iter()
        .flat_map(|element| {
            (1..=5).map(move |level| CardDef {
                id: CardDefId::new(format!("{}-{level}", element.id)),
                name: element.name.to_string(),
                element: element.element,
                level,
            })
        })
        .collect()
}

fn preconstructed_cards() -> Vec<CardDefId> {
    official_card_defs()
        .into_iter()
        .flat_map(|definition| {
            let copies = preconstructed_copy_count(definition.level);
            std::iter::repeat_n(definition.id, copies)
        })
        .collect()
}

fn official_copy_count(level: u32) -> usize {
    match level {
        1..=3 => 4,
        4..=5 => 3,
        _ => 0,
    }
}

fn personal_copy_limit(level: u32) -> usize {
    official_copy_count(level)
}

fn preconstructed_copy_count(level: u32) -> usize {
    level
        .checked_sub(1)
        .and_then(|index| PRECONSTRUCTED_COPIES_BY_LEVEL.get(index as usize))
        .copied()
        .unwrap_or_default()
}

fn elements() -> [ElementSpec; 5] {
    [
        ElementSpec {
            id: "metal",
            name: "金",
            element: Element::Metal,
        },
        ElementSpec {
            id: "wood",
            name: "木",
            element: Element::Wood,
        },
        ElementSpec {
            id: "water",
            name: "水",
            element: Element::Water,
        },
        ElementSpec {
            id: "fire",
            name: "火",
            element: Element::Fire,
        },
        ElementSpec {
            id: "earth",
            name: "土",
            element: Element::Earth,
        },
    ]
}

#[derive(Clone, Copy)]
struct ElementSpec {
    id: &'static str,
    name: &'static str,
    element: Element,
}
