use crate::domain::{
    CONFLUENCE_GENERATION_MODULE_ID, DARK_GLIMMER_MODULE_ID, GameState, HERO_SCHOOLS_MODULE_ID,
    JIANGHU_MODULE_ID, PlayerId, ProfessionId, RuleModuleId,
};
use std::collections::HashSet;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ProfessionCatalogEntry {
    pub(crate) id: ProfessionId,
    pub(crate) module_id: &'static str,
    pub(crate) name: &'static str,
    pub(crate) rule_text: &'static str,
    pub(crate) parents: Vec<ProfessionId>,
    pub(crate) ability_ids: Vec<&'static str>,
}

pub(crate) fn catalog(enabled_modules: &[RuleModuleId]) -> Vec<ProfessionCatalogEntry> {
    let mut entries = Vec::new();
    if enabled_modules
        .iter()
        .any(|module| module.as_str() == HERO_SCHOOLS_MODULE_ID)
    {
        entries.extend(crate::rules::hero::profession_catalog_entries());
    }
    if enabled_modules
        .iter()
        .any(|module| module.as_str() == JIANGHU_MODULE_ID)
    {
        entries.extend(crate::rules::jianghu::profession_catalog_entries());
    }
    if enabled_modules
        .iter()
        .any(|module| module.as_str() == CONFLUENCE_GENERATION_MODULE_ID)
    {
        entries.extend(crate::rules::confluence::profession_catalog_entries());
    }
    if enabled_modules
        .iter()
        .any(|module| module.as_str() == DARK_GLIMMER_MODULE_ID)
    {
        entries.extend(crate::rules::dark::profession_catalog_entries());
    }
    entries
}

pub(crate) fn ability_ids_in_effect(state: &GameState, player: &PlayerId) -> Vec<&'static str> {
    if crate::rules::pouch::profession_is_suppressed(state, player) {
        return Vec::new();
    }
    state
        .profession_for(player)
        .map(|profession| effective_ability_ids(&state.enabled_rule_modules, profession))
        .unwrap_or_default()
}

pub(crate) fn playable_profession_changes(
    state: &crate::domain::GameState,
    player: &crate::domain::PlayerId,
    cards: &[crate::domain::CardInstanceId],
) -> crate::domain::GameResult<Vec<crate::rules::ProfessionChangeCandidate>> {
    if !ordinary_profession_change_is_permitted(state, player) {
        return Ok(Vec::new());
    }
    let mut candidates = crate::rules::hero::playable_profession_changes(state, player, cards)?;
    candidates.extend(crate::rules::jianghu::playable_profession_changes(
        state, player, cards,
    )?);
    candidates.extend(crate::rules::confluence::playable_profession_changes(
        state, player, cards,
    )?);
    candidates.extend(crate::rules::dark::playable_profession_changes(
        state, player, cards,
    )?);
    Ok(candidates)
}

pub(crate) fn playable_profession_abilities(
    state: &crate::domain::GameState,
    player: &crate::domain::PlayerId,
    cards: &[crate::domain::CardInstanceId],
) -> crate::domain::GameResult<Vec<crate::rules::ProfessionAbilityCandidate>> {
    let mut candidates = crate::rules::hero::playable_profession_abilities(state, player, cards)?;
    candidates.extend(crate::rules::jianghu::playable_profession_abilities(
        state, player, cards,
    )?);
    candidates.extend(crate::rules::confluence::playable_profession_abilities(
        state, player, cards,
    )?);
    candidates.extend(crate::rules::dark::playable_profession_abilities(
        state, player, cards,
    )?);
    Ok(candidates)
}

pub(crate) fn activate_profession_ability(
    state: &crate::domain::GameState,
    player: &crate::domain::PlayerId,
    ability_id: &str,
    cards: &[crate::domain::CardInstanceId],
    target_card: Option<crate::domain::CardInstanceId>,
    declared_element: Option<crate::domain::Element>,
    declared_level: Option<u32>,
) -> crate::domain::GameResult<Vec<crate::domain::GameEvent>> {
    if ability_id.starts_with("jianghu:") {
        crate::rules::jianghu::activate_profession_ability(
            state,
            player,
            ability_id,
            cards,
            target_card,
            declared_element,
            declared_level,
        )
    } else if ability_id.starts_with("confluence:") {
        crate::rules::confluence::activate_profession_ability(
            state,
            player,
            ability_id,
            cards,
            target_card,
            declared_element,
            declared_level,
        )
    } else if ability_id.starts_with("dark:") {
        crate::rules::dark::activate_profession_ability(
            state,
            player,
            ability_id,
            cards,
            target_card,
            declared_element,
            declared_level,
        )
    } else {
        crate::rules::hero::activate_profession_ability(
            state,
            player,
            ability_id,
            cards,
            target_card,
            declared_element,
            declared_level,
        )
    }
}

pub(crate) fn validate_profession_change(
    state: &crate::domain::GameState,
    player: &crate::domain::PlayerId,
    target: &ProfessionId,
    cards: &[crate::domain::CardInstanceId],
) -> crate::domain::GameResult<()> {
    if !ordinary_profession_change_is_permitted(state, player) {
        return Err(crate::domain::GameError::Validation(
            crate::domain::ValidationError::ProfessionChangePatternMismatch {
                profession: target.clone(),
            },
        ));
    }
    if crate::rules::hero::profession(target).is_some()
        && !state.has_rule_module(HERO_SCHOOLS_MODULE_ID)
    {
        return Err(crate::domain::GameError::Validation(
            crate::domain::ValidationError::HeroSchoolsDisabled,
        ));
    }
    let definition = definition(&state.enabled_rule_modules, target).ok_or_else(|| {
        crate::domain::GameError::Validation(crate::domain::ValidationError::UnknownProfession(
            target.clone(),
        ))
    })?;
    match definition.module_id {
        HERO_SCHOOLS_MODULE_ID => {
            crate::rules::hero::validate_profession_change(state, player, target, cards)
        }
        JIANGHU_MODULE_ID => {
            crate::rules::jianghu::validate_profession_change(state, player, target, cards)
        }
        CONFLUENCE_GENERATION_MODULE_ID => {
            crate::rules::confluence::validate_profession_change(state, player, target, cards)
        }
        DARK_GLIMMER_MODULE_ID => {
            crate::rules::dark::validate_profession_change(state, player, target, cards)
        }
        _ => Err(crate::domain::GameError::Validation(
            crate::domain::ValidationError::UnknownProfession(target.clone()),
        )),
    }
}

fn ordinary_profession_change_is_permitted(state: &GameState, player: &PlayerId) -> bool {
    !state.profession_for(player).is_some_and(|profession| {
        inherits_from(
            &state.enabled_rule_modules,
            profession,
            &ProfessionId::new(crate::rules::dark::DARK_WALKER_ID),
        )
    })
}

pub(crate) fn definition(
    enabled_modules: &[RuleModuleId],
    id: &ProfessionId,
) -> Option<ProfessionCatalogEntry> {
    catalog(enabled_modules)
        .into_iter()
        .find(|profession| &profession.id == id)
}

pub(crate) fn effective_ability_ids(
    enabled_modules: &[RuleModuleId],
    id: &ProfessionId,
) -> Vec<&'static str> {
    effective_ability_ids_in_catalog(&catalog(enabled_modules), id)
}

pub(crate) fn effective_ability_summaries(
    enabled_modules: &[RuleModuleId],
    id: &ProfessionId,
) -> Vec<&'static str> {
    let mut summaries = crate::rules::hero::effective_ability_summaries(enabled_modules, id);
    summaries.extend(crate::rules::jianghu::effective_ability_summaries(
        enabled_modules,
        id,
    ));
    summaries.extend(crate::rules::confluence::effective_ability_summaries(
        enabled_modules,
        id,
    ));
    summaries.extend(crate::rules::dark::effective_ability_summaries(
        enabled_modules,
        id,
    ));
    summaries
}

pub(crate) fn profession_formation_summaries(
    enabled_modules: &[RuleModuleId],
    id: &ProfessionId,
) -> Vec<(String, String)> {
    let mut summaries = crate::rules::hero::profession_formation_summaries(enabled_modules, id);
    summaries.extend(crate::rules::jianghu::profession_formation_summaries(
        enabled_modules,
        id,
    ));
    summaries.extend(crate::rules::confluence::profession_formation_summaries(
        enabled_modules,
        id,
    ));
    summaries.extend(crate::rules::dark::profession_formation_summaries(
        enabled_modules,
        id,
    ));
    summaries
}

pub(crate) fn inherits_from(
    enabled_modules: &[RuleModuleId],
    profession: &ProfessionId,
    ancestor: &ProfessionId,
) -> bool {
    if profession == ancestor {
        return true;
    }
    let catalog = catalog(enabled_modules);
    let mut visited = HashSet::new();
    inherits_from_catalog(&catalog, profession, ancestor, &mut visited)
}

pub(crate) fn is_legendary(id: &ProfessionId) -> bool {
    crate::rules::hero::is_legendary(id)
        || matches!(
            id.as_str(),
            crate::rules::jianghu::SWORD_SAGE_ID
                | crate::rules::jianghu::QI_GRANDMASTER_ID
                | crate::rules::jianghu::BOOK_IMMORTAL_ID
                | crate::rules::jianghu::POISON_SAINT_ID
                | crate::rules::confluence::HEAVENLY_RESONATOR_ID
                | crate::rules::confluence::DAO_SAINT_ID
                | crate::rules::confluence::VOID_DESTROYER_ID
                | crate::rules::dark::SHADOW_BERSERKER_ID
        )
}

fn effective_ability_ids_in_catalog(
    catalog: &[ProfessionCatalogEntry],
    id: &ProfessionId,
) -> Vec<&'static str> {
    fn collect(
        catalog: &[ProfessionCatalogEntry],
        id: &ProfessionId,
        visiting: &mut HashSet<ProfessionId>,
        seen: &mut HashSet<&'static str>,
        abilities: &mut Vec<&'static str>,
    ) {
        if !visiting.insert(id.clone()) {
            return;
        }
        let Some(definition) = catalog.iter().find(|profession| &profession.id == id) else {
            visiting.remove(id);
            return;
        };
        for parent in &definition.parents {
            collect(catalog, parent, visiting, seen, abilities);
        }
        for ability in &definition.ability_ids {
            if seen.insert(*ability) {
                abilities.push(*ability);
            }
        }
        visiting.remove(id);
    }

    let mut visiting = HashSet::new();
    let mut seen = HashSet::new();
    let mut abilities = Vec::new();
    collect(catalog, id, &mut visiting, &mut seen, &mut abilities);
    abilities
}

fn inherits_from_catalog(
    catalog: &[ProfessionCatalogEntry],
    profession: &ProfessionId,
    ancestor: &ProfessionId,
    visited: &mut HashSet<ProfessionId>,
) -> bool {
    if !visited.insert(profession.clone()) {
        return false;
    }
    catalog
        .iter()
        .find(|definition| &definition.id == profession)
        .is_some_and(|definition| {
            definition.parents.iter().any(|parent| {
                parent == ancestor || inherits_from_catalog(catalog, parent, ancestor, visited)
            })
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::RuleModuleId;

    #[test]
    fn disabled_modules_contribute_no_professions() {
        assert!(catalog(&[]).is_empty());
        assert_eq!(
            catalog(&[RuleModuleId::new(HERO_SCHOOLS_MODULE_ID)]).len(),
            18
        );
    }

    #[test]
    fn multi_parent_abilities_are_stable_and_deduplicated() {
        let catalog = vec![
            entry("left", &[], &["shared", "left"]),
            entry("right", &[], &["shared", "right"]),
            entry("child", &["left", "right"], &["child"]),
        ];

        assert_eq!(
            effective_ability_ids_in_catalog(&catalog, &ProfessionId::new("child")),
            vec!["shared", "left", "right", "child"]
        );
    }

    #[test]
    fn hero_inheritance_uses_the_composed_catalog() {
        let modules = [RuleModuleId::new(HERO_SCHOOLS_MODULE_ID)];
        assert!(inherits_from(
            &modules,
            &ProfessionId::new(crate::rules::hero::HERO_ID),
            &ProfessionId::new(crate::rules::hero::WARRIOR_ID),
        ));
        assert!(!inherits_from(
            &[],
            &ProfessionId::new(crate::rules::hero::HERO_ID),
            &ProfessionId::new(crate::rules::hero::WARRIOR_ID),
        ));
    }

    fn entry(
        id: &'static str,
        parents: &[&'static str],
        abilities: &[&'static str],
    ) -> ProfessionCatalogEntry {
        ProfessionCatalogEntry {
            id: ProfessionId::new(id),
            module_id: "test",
            name: id,
            rule_text: "",
            parents: parents
                .iter()
                .map(|parent| ProfessionId::new(*parent))
                .collect(),
            ability_ids: abilities.to_vec(),
        }
    }
}
