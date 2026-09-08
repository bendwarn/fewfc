// 將 Rust integration suites 收斂為單一 Cargo test target；各 suite 仍保留獨立 module namespace。
#[path = "single/activated_profession_ability.rs"]
mod activated_profession_ability;
#[path = "single/card_interpretation.rs"]
mod card_interpretation;
#[path = "single/choices_randomness.rs"]
mod choices_randomness;
#[path = "single/confluence_generation.rs"]
mod confluence_generation;
#[path = "single/dark_glimmer.rs"]
mod dark_glimmer;
#[path = "single/deck_composition.rs"]
mod deck_composition;
#[path = "single/echo.rs"]
mod echo;
#[path = "single/echo_interaction_matrix.rs"]
mod echo_interaction_matrix;
#[path = "single/formation_query.rs"]
mod formation_query;
#[path = "single/hero_schools.rs"]
mod hero_schools;
#[path = "single/hero_warrior.rs"]
mod hero_warrior;
#[path = "single/hero_warrior_interaction_matrix.rs"]
mod hero_warrior_interaction_matrix;
#[path = "single/jianghu.rs"]
mod jianghu;
#[path = "single/jianghu_interaction_matrix.rs"]
mod jianghu_interaction_matrix;
#[path = "single/optional_rules.rs"]
mod optional_rules;
#[path = "single/pending_choice_lifecycle.rs"]
mod pending_choice_lifecycle;
#[path = "single/persistence.rs"]
mod persistence;
#[path = "single/pouch_interaction_matrix.rs"]
mod pouch_interaction_matrix;
#[path = "single/pouch_strategy_interaction_matrix.rs"]
mod pouch_strategy_interaction_matrix;
#[path = "single/rule_versions.rs"]
mod rule_versions;
#[path = "single/rules_engine.rs"]
mod rules_engine;
#[path = "single/ruleset_configuration.rs"]
mod ruleset_configuration;
#[path = "single/spirit_interaction_matrix.rs"]
mod spirit_interaction_matrix;
#[path = "single/spirit_rules.rs"]
mod spirit_rules;
#[path = "single/star_defense_interaction_matrix.rs"]
mod star_defense_interaction_matrix;
#[path = "single/star_rules.rs"]
mod star_rules;
#[path = "single/totem_combat_matrix.rs"]
mod totem_combat_matrix;
#[path = "single/totem_interaction_matrix.rs"]
mod totem_interaction_matrix;
#[path = "single/totem_passive_matrix.rs"]
mod totem_passive_matrix;
#[path = "single/tribulation.rs"]
mod tribulation;
#[path = "single/web_rules_catalog.rs"]
mod web_rules_catalog;
