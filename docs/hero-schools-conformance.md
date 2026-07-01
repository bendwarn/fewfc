# Hero Schools 5.16 Conformance Inventory

This inventory pins the 18-Profession implementation to the official 5.16
catalog. The executable references are integration-test names; shared behavior
is intentionally tested through public commands and events rather than private
helpers.

## Profession catalog

| School | Profession | Transition and inheritance | Automatic / activated abilities and proficiencies | Profession Formations | Focused tests |
| --- | --- | --- | --- | --- | --- |
| Warrior | Warrior | Metal 3, no prerequisite | Unloading; Weapon, Defense, Countershock Proficiencies | — | `profession_change_progresses_in_order_and_replays_explicit_card_moves`, `warrior_proficiencies_keep_original_formation_identity_and_effect`, `warrior_and_hero_damage_hooks_modify_only_matching_damage` |
| Warrior | War God | Warrior + Metal 6; inherits Warrior | Weapon Mastery; Shock Burst Proficiency | Divine Weapon | `profession_change_progresses_in_order_and_replays_explicit_card_moves`, `hero_formations_and_battle_soul_work_in_four_player_games` |
| Warrior | Hero | War God + Metal 9; inherits both tiers | Metal Resistance; Battle Soul | Falling Light Slash | `profession_change_progresses_in_order_and_replays_explicit_card_moves`, `shield_receives_unreduced_damage_before_warrior_resistance`, `hero_formations_and_battle_soul_work_in_four_player_games` |
| Seeker | Seeker | Wood 3, no prerequisite | Discard Retrieval discount; Generating and Overcoming Proficiencies | — | `all_school_transitions_and_unaffiliated_routes_are_queryable`, `seeker_cost_counter_resistance_and_spell_protection_are_typed` |
| Seeker | Expounder | Seeker + Wood 6; inherits Seeker | Return to Origin and Five Elements Cycle Proficiencies | Dao Defense | `seeker_options_and_reincarnation_role_binding_are_exact`, `seeker_cost_counter_resistance_and_spell_protection_are_typed` |
| Seeker | Benevolent | Expounder + Wood 9; inherits both tiers | Wood Resistance; spell-counter protection | Reincarnation | `seeker_options_and_reincarnation_role_binding_are_exact`, `seeker_cost_counter_resistance_and_spell_protection_are_typed`, `sacred_beast_resistance_applies_only_after_shield_absorption` |
| Mesmer | Mesmer | Water 3, no prerequisite | Illusion; Seal Proficiency | — | `all_school_transitions_and_unaffiliated_routes_are_queryable`, `mesmer_preparation_is_public_shared_and_clears_after_action` |
| Mesmer | Spirit Mesmer | Mesmer + Water 6; inherits Mesmer | Illusion Refinement; Barrier Proficiency | Magic Seal | `mesmer_preparation_is_public_shared_and_clears_after_action`, `phantasm_does_not_trigger_illusion_refinement_and_mesmer_formations_resolve` |
| Mesmer | Hermit | Spirit Mesmer + Water 9; inherits both tiers | Phantasm; Water Resistance | Purple Light Shield | `phantasm_does_not_trigger_illusion_refinement_and_mesmer_formations_resolve` |
| Mage | Mage | Fire 3, no prerequisite | five three-Card Attack Proficiencies with point modifier | — | `all_school_transitions_and_unaffiliated_routes_are_queryable`, `mage_point_modifiers_and_star_qualification_use_attack_points` |
| Mage | Mage Guide | Mage + Fire 6; inherits Mage | Radiance and Five Streams Unite Proficiencies | Magic Shock | `mage_point_modifiers_and_star_qualification_use_attack_points`, `mage_and_windwalker_profession_formations_keep_stage_semantics` |
| Mage | Sage | Mage Guide + Fire 9; inherits both tiers | Fire Resistance; Arcane Essence | Magic Reflection Flash | `mage_point_modifiers_and_star_qualification_use_attack_points`, `mage_and_windwalker_profession_formations_keep_stage_semantics` |
| Windwalker | Windwalker | Earth 3, no prerequisite | Windwalking; Metamorphosis Proficiency | — | `all_school_transitions_and_unaffiliated_routes_are_queryable`, `windwalker_and_unaffiliated_effects_use_shared_pipelines` |
| Windwalker | Shadow Walker | Windwalker + Earth 6; inherits Windwalker | Shadow Cut; Chaos Proficiency | Shadow Assault | `windwalker_and_unaffiliated_effects_use_shared_pipelines`, `mage_and_windwalker_profession_formations_keep_stage_semantics` |
| Windwalker | Martial Artist | Shadow Walker + Earth 9; inherits both tiers | Earth Resistance; Shadow Escape | Instant Shadow Death | `mage_and_windwalker_profession_formations_keep_stage_semantics` |
| Unaffiliated | First Wanderer | one level-1 Card; Choice and Breakthrough replace it | Choice; Breakthrough | — | `all_school_transitions_and_unaffiliated_routes_are_queryable` |
| Unaffiliated | Immortal | any second-tier School Profession + 555; does not inherit | Immortal draw bonus; Meditation | Condensed Void Arrow; Sky Bow Roar | `all_school_transitions_and_unaffiliated_routes_are_queryable`, `windwalker_and_unaffiliated_effects_use_shared_pipelines` |
| Unaffiliated | Saint | any second-tier School Profession + 444; does not inherit | Sacred Art; Revelation | Holy Light Break; Holy Wind | `all_school_transitions_and_unaffiliated_routes_are_queryable`, `windwalker_and_unaffiliated_effects_use_shared_pipelines`, `activated_abilities_require_action_permission_and_revelation_recycles_personal_cards` |

Void Reversion Technique is module-wide rather than owned by one Profession.
Its low-level Legendary protection, high-level breaking, Card movement, HP cost,
and outcome ordering are covered by
`void_reversion_is_atomic_and_protects_low_level_legendary_professions` and
`high_level_void_reversion_breaks_legendary_professions_before_outcome`.

## Cross-module gates

| Boundary | Executable coverage |
| --- | --- |
| Star attack-point qualification and exact substitution | `mage_point_modifiers_and_star_qualification_use_attack_points` plus `tests/star_rules.rs` |
| Prepared versus printed interpretation and Sacred Art multiplicity | `mesmer_preparation_is_public_shared_and_clears_after_action`, `windwalker_and_unaffiliated_effects_use_shared_pipelines` |
| Five Directions Environment, Sacred Beast, Shield, and resistance order | `tests/rules_engine.rs` Environment tests and `sacred_beast_resistance_applies_only_after_shield_absorption` |
| Discard Retrieval cost, lethal/shared Team HP, and Cannot Act | `seeker_cost_counter_resistance_and_spell_protection_are_typed`, existing Discard Retrieval integration tests, and `activated_abilities_require_action_permission_and_revelation_recycles_personal_cards` |
| Personal Deck origin, owner discard, recycling, and hidden-card projection | `activated_abilities_require_action_permission_and_revelation_recycles_personal_cards` plus `tests/persistence.rs`, `tests/rules_engine.rs`, and `apps/web/tests/e2e/personal-deck.spec.ts` |
| Replay, verification, viewer filtering, and Web DTOs | `tests/persistence.rs`, `tests/rules_engine.rs`, and `src/web_api.rs` unit tests |
| Online default, stored-room compatibility, vertical controls, and teaching catalog | `apps/web/app/lib/rule-modules.test.ts` and `apps/web/tests/e2e/playable-actions.spec.ts` |

The online release gate stayed closed while issues #47–#52 were incomplete.
Issue #53 opens the single `hero-schools` room toggle only after this inventory
and its referenced tests are present.
