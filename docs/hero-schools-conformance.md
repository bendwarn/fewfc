# 英雄學派 5.16 規則與測試對照

本文件整理英雄學派 18 個職業的 5.16 規則，以及目前可查閱的代表性測試。職業、能力與陣法名稱採用程式目錄及[規則參考](rule.md#54-hero-schools)中的中文名稱；完整規則及實作解讀分別以規則參考和[規則引擎決策](rules-engine-decisions.md)為準。本表供查找，不以測試名稱本身宣稱全部互動均已覆蓋。

英雄學派是可獨立設定的進階規則。對局所用規則版本與啟用模組分別設定；新房間預設 5.17，也可選擇 5.16。各版本可用的主題規則及房間預設依 [ADR-0038](adr/0038-bind-games-to-rule-versions.md) 判定，不以本表推定其他版本與 5.16 完全相同。

## 職業目錄

五大學派的一階職業無前置職業；以對應屬性的任意張牌轉職，等級總和須至少達表列數值。二階與三階分別以前一階為前置職業，並繼承低階能力。每列只列出該階新增的能力。初行客、仙者與聖者的轉職條件另列於表末；仙者和聖者不繼承前職業能力。完整目錄可對照[職業定義](../src/rules/hero.rs)。

| 學派 | 職業 | 一般轉職條件 | 新增能力與專精 | 職業陣法 |
| --- | --- | --- | --- | --- |
| 戰士 | 戰士 | 金行，等級總和至少 3 | 卸勁、武器專精、防禦專精、反震專精 | — |
| 戰士 | 戰神 | 戰士；金行，等級總和至少 6 | 武器精研、震暴專精 | 神兵 |
| 戰士 | 勇者 | 戰神；金行，等級總和至少 9 | 金行抗性、戰魄 | 落光斬 |
| 尋道者 | 尋道者 | 木行，等級總和至少 3 | 尋道術、生陣專精、剋陣專精 | — |
| 尋道者 | 述道者 | 尋道者；木行，等級總和至少 6 | 歸元專精、五行輪迴專精 | 道禦 |
| 尋道者 | 仁者 | 述道者；木行，等級總和至少 9 | 木行抗性、道源 | 轉生術 |
| 幻術師 | 幻術師 | 水行，等級總和至少 3 | 幻術、封印專精 | — |
| 幻術師 | 幻靈使 | 幻術師；水行，等級總和至少 6 | 幻術精研、氣壁專精 | 幻印 |
| 幻術師 | 隱者 | 幻靈使；水行，等級總和至少 9 | 水行抗性、幻朧 | 紫光防罩 |
| 法師 | 法師 | 火行，等級總和至少 3 | 五行三張攻擊專精 | — |
| 法師 | 法導 | 法師；火行，等級總和至少 6 | 光芒專精、五流歸一專精 | 法震 |
| 法師 | 智者 | 法導；火行，等級總和至少 9 | 火行抗性、法粹 | 魔法反光閃 |
| 風行者 | 風行者 | 土行，等級總和至少 3 | 風行術、幻化專精 | — |
| 風行者 | 影行者 | 風行者；土行，等級總和至少 6 | 影切、混沌專精 | 影襲 |
| 風行者 | 武者 | 影行者；土行，等級總和至少 9 | 土行抗性、影遁 | 刃影瞬殺 |
| 無學派 | 初行客 | 無前置職業；一張 1 級牌 | 抉擇、突破 | — |
| 無學派 | 仙者 | 任一五大學派二階職業；三張 5 級牌 | 仙術、冥思 | 凝華斷空箭、天弓嘯 |
| 無學派 | 聖者 | 任一五大學派二階職業；三張 4 級牌 | 聖術、啟示 | 聖光破、聖風 |

初行客的抉擇與突破提供額外轉職途徑；暫時失去能力時，不能使用依賴其職業能力的途徑。相關案例見 [`temporary_ability_loss_removes_first_wanderer_choice_and_breakthrough_paths`](../tests/single/hero_schools.rs#L353)。

虛空返璞術是英雄學派共通陣法，不屬於單一職業。其低等級保護傳說職業、高等級破除職業、生命代價、牌移動與結算順序，見 [`void_reversion_is_atomic_and_protects_low_level_legendary_professions`](../tests/single/hero_schools.rs#L935) 及 [`high_level_void_reversion_breaks_legendary_professions_before_outcome`](../tests/single/hero_schools.rs#L1071)。

## 職業測試索引

Rust 整合測試由 `tests/single.rs` 註冊，使用 `single` 測試目標。以下是目前的代表性案例；實際主張以測試斷言為準。

| 範圍 | 代表性測試 |
| --- | --- |
| 五大學派及無學派的轉職候選 | [`all_school_transitions_and_unaffiliated_routes_are_queryable`](../tests/single/hero_schools.rs#L229) |
| 戰士、戰神、勇者的合法進階與抗性 | [`metal_profession_ladder_and_hero_resistance_matrix_use_legal_turns`](../tests/single/hero_warrior_interaction_matrix.rs#L764) |
| 防禦專精的建立、翻開及消耗 | [`warrior_defense_proficiency_matrix_establishes_and_consumes_defense_without_empty_city_fallback`](../tests/single/hero_warrior_interaction_matrix.rs#L210) |
| 武器精研與原陣法身分 | [`war_god_weapon_mastery_matrix_keeps_weapon_identity_and_adds_turn_draw_through_legal_ladder`](../tests/single/hero_warrior_interaction_matrix.rs#L1152) |
| 卸勁與防護罩的結算順序 | [`warrior_resistance_shield_matrix_absorbs_unreduced_shock_burst_before_damage_reduction`](../tests/single/hero_warrior_interaction_matrix.rs#L578) |
| 尋道者轉職、棄牌回收代價、抗性及道源 | [`seeker_options_and_reincarnation_role_binding_are_exact`](../tests/single/hero_schools.rs#L419)、[`seeker_cost_counter_resistance_and_spell_protection_are_typed`](../tests/single/hero_schools.rs#L477) |
| 幻術、幻朧的虛擬牌及陣法要求 | [`illusion_and_phantasm_are_single_offers_with_virtual_card_input`](../tests/single/hero_schools.rs#L558)、[`phantasm_does_not_trigger_illusion_refinement_and_mesmer_formations_resolve`](../tests/single/hero_schools.rs#L639) |
| 法師專精、攻擊點數及星辰資格 | [`mage_point_modifiers_and_star_qualification_use_attack_points`](../tests/single/hero_schools.rs#L661) |
| 法師與風行者職業陣法 | [`mage_and_windwalker_profession_formations_keep_stage_semantics`](../tests/single/hero_schools.rs#L700) |
| 風行者、仙者及聖者能力 | [`windwalker_and_unaffiliated_effects_use_shared_pipelines`](../tests/single/hero_schools.rs#L855)、[`activated_abilities_require_action_permission_and_revelation_recycles_personal_cards`](../tests/single/hero_schools.rs#L965) |
| 暫時失去能力後的職業陣法選項與過期命令 | [`temporary_ability_loss_removes_profession_formation_options_and_stale_commands`](../tests/single/hero_schools.rs#L784) |

## 跨模組及線上接點

- **星辰圖記：** 法師點數與星辰資格見上表；星辰變牌及防禦互動見 [`owned_star_defense_matrix_records_selected_substitution_and_preserves_star_lifecycle`](../tests/single/star_defense_interaction_matrix.rs#L172) 及 [星辰測試](../tests/single/star_rules.rs)。
- **五方傳說：** 聖獸傷害、防護罩及職業抗性的順序見 [`sacred_beast_resistance_applies_only_after_shield_absorption`](../tests/single/hero_schools.rs#L811)。
- **錦囊：** 仁者道源、金蟬及幻印的無效依據與消耗見 [`no_effect_ground_matrix_benevolent_and_golden_cicada_both_consume_magic_seal`](../tests/single/pouch_interaction_matrix.rs#L1692)。
- **棄牌回收與個人牌組：** 尋道術代價及啟示的個人牌組回收見上表中的尋道者與發動能力測試。
- **重播與公開視圖：** [持久化測試](../tests/single/persistence.rs)、[規則引擎整合測試](../tests/single/rules_engine.rs)及 [Rust Web API 測試](../src/web_api.rs)涵蓋相關紀錄、驗證與視圖契約。
- **線上設定與行動介面：** [規則模組設定測試](../apps/web/app/lib/rule-modules.test.ts)及[可執行行動瀏覽器測試](../apps/web/tests/e2e/playable-actions.spec.ts)提供代表性接點。UI 排版以目前的[戰場介面設計](../apps/web/docs/ui-battlefield-design.md)為準。

執行單一 Rust 案例時，可使用 `cargo test --test single <測試名稱>`；完整規則引擎驗證使用 `cargo test`。測試通過與否以實際執行結果為準，本文件不記錄持續有效的通過狀態。
