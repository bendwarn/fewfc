# ADR-0032 Rules Test Follow-ups

[清單](rule-test-inventory.md)已記錄全部 654 筆歷史列的目前來源位置與判定。第二輪核對剩餘 513 筆名稱相符的列，並新增 149 筆現行主張，包括三個圖騰陣法測試套件、具型別待處理規則流程、錦囊秘計決策、規則版本及遊戲結論。目前 Rust `single` 測試目標中的每個測試都有具名來源位置。[原始檢查點](refactor-result.md)保留起始數字。

## Replacements for the first pass's open claims

- **錦囊準備。** 矩陣檢查放置錦囊並發出 4／5 張起始手牌後，兩個個人牌組分別剩餘 55 及 54 張牌。
- **觀火（Watch the Fire）。** 一個案例比較金擊術（Metal Strike）對合法氣壁（Barrier）有無觀火的差異；另一個比較虛空斷脈術（Void Meridian）有無觀火，並檢查兩個分支均破除環境及捨棄所提交的牌。結合觀火時，雙方隊伍生命值均受保護。
- **蓋牌防禦。** 即時狀態斷言精確的手牌、陣法區蓋牌及轉入回合抽牌。
- **商調‧鳴金（Ringing Metal）。** 相同兩張牌面為金的牌同時提供鳴金及武器（Weapon）。另一案例發完測試牌堆，檢查具型別空洗棄牌、沒有選牌，以及等待邊界的重播一致性。
- **無法行動時的棄牌回收。** 個人牌組案例檢查推導出的牌、各牌區、牌堆頂公開展示，以及兩倍牌面等級的生命值代價。既有共用牌堆案例繼續保留。
- **金環境下的氣壁。** 四人互動檢查合法氣壁建立非零防護罩；之後在金環境施展的氣壁仍施展並捨棄牌，但不改變防護罩。

## Contract decisions

- 離山（Lure）以暫時失去能力移除受影響能力選項，並拒絕過期命令。[ADR-0033](../docs/adr/0033-treat-lure-as-temporary-ability-loss.md)
- 具型別待處理規則流程位於正規事件封套。Web 隨機性請求保留其操作及順序欄位，連環（Chain）答案使用封閉的錦囊決策結構。[ADR-0034](../docs/adr/0034-unify-waiting-states-under-pending-resolution.md)
- 刃影瞬殺（Instant Shadow Death） 使目前生命值減半並向下取整。Web 棄牌回收接點在強制無法行動略過前記錄回收。兩者均有明確斷言。

## Echo integration rewrites

八個既有測試已改用官方遊戲設定及已記錄命令，目前證據連結於清單。

- **商調‧鳴金。** 共用牌堆案例檢查選擇隱私、公開展示、精確的剩餘牌洗牌及迴響代價續接。回收案例以正常回合建立棄牌並抽完牌堆，分別檢查兩次隨機性請求及最終順序。個人牌組案例檢查施展玩家的檢索和洗牌、另一玩家的牌堆與手牌，以及棄牌目的地；空牌堆案例仍涵蓋具型別空回收請求。
- **宮調‧裂土（Split Earth）。** 合法回合顯示武器傷害在目標下回合被阻止，到期後恢復。另一比較檢查有無相符抑制時角調‧落木（Falling Wood）的回復與迴響代價選項。付費迴響檢視變動後的手牌，並在正常回合開始前結算。四人案例檢查席位順序及檢視隱私。西‧白虎（White Tiger）在三種抑制來源分支均保留環境轉移，只有迴響來源的裂土阻止傷害。

每個改寫後的測試均對照即時狀態檢查事件重播及已記錄決策驗證。清單保留歷史欄位，另行記錄目前斷言及判定。

## Integration test reconciliation

62 個整合測試候選的審查已完成：56 個改用合法命令、三個確認已有合法矩陣證據，另保留三個獨立匹配器、宣告與投影契約。最後 45 個改寫保留原測試名稱，目前證據連結於清單。

最後三項斷言缺口已補齊：符合條件的重複召喚保留星辰擁有權與個人歷史；點水扇（Water-Dotting Fan）在正規攻擊狀態清單中區分較早陣法與緊接的上回合陣法；四人震暴（Shock Burst）為另外三位玩家各記錄兩個戰魄（Battle Soul）狀態。

每個改寫案例均檢查事件重播及已記錄決策驗證。同回合精靈次數案例以精靈召喚事件的單獨投影斷言隔離重設；該測試的無法行動及技能行為則透過已記錄命令執行。

### Existing matrices and narrow contracts

- 保留真實檢索答案對非擁有者的精確 `ChoiceMade` 投影；測試資料隔離公開事件 payload。[`clear_wind_ten_thousand_miles_kept_cards_are_hidden_from_other_players`](../tests/single/confluence_generation.rs#L515)
- 保留階級模式與無學派路徑匹配器，在已知階級查詢 `ChangeProfession` 候選。[`all_school_transitions_and_unaffiliated_routes_are_queryable`](../tests/single/hero_schools.rs#L229)
- 合法金系職業進階矩陣檢查每次轉職牌移動、最終觀察者職業及兩條重播路徑；歷史 JSON 往返另作獨立斷言。[`metal_profession_ladder_and_hero_resistance_matrix_use_legal_turns`](../tests/single/hero_warrior_interaction_matrix.rs#L764)、[`profession_change_progresses_in_order_and_replays_explicit_card_moves`](../tests/single/hero_warrior.rs#L107)
- 戰神武器精通矩陣證明合法取得後的普通武器身分及具型別回合抽牌加成；保留歷史神兵選項斷言。[`war_god_weapon_mastery_matrix_keeps_weapon_identity_and_adds_turn_draw_through_legal_ladder`](../tests/single/hero_warrior_interaction_matrix.rs#L1152)、[`warrior_proficiencies_keep_original_formation_identity_and_effect`](../tests/single/hero_warrior.rs#L224)
- 合法木星辰與防禦矩陣檢查精確變牌、蓋牌及擁有者／對手視圖；保留觀察者遮蔽斷言。[`owned_star_defense_matrix_records_selected_substitution_and_preserves_star_lifecycle`](../tests/single/star_defense_interaction_matrix.rs#L172)、[`substituted_passive_records_and_redacts_the_declared_card`](../tests/single/star_rules.rs#L267)
- 保留兩個精確變牌選項、缺少宣告的驗證錯誤，以及接受明確選定替代項的斷言。[`multiple_star_substitution_options_require_the_selected_card`](../tests/single/star_rules.rs#L331)

## Validation and completion

合併的工作分支記錄 `cargo test --locked` 通過 805 個 Rust 測試，其中 `single` 有 395 個；先前排定的 45 個測試均在該目標中執行。此數字記錄該分支當時的驗證，不能代替後續變更的測試結果。

瀏覽器審查將既有隱私、重新連線、設定及遊戲結束接點與具名 Rust 測試配對。連環及 飛刃（Flying Blade） 測試分別新增等待選擇時重新連線、觀戰者獨立重新連線的瀏覽器斷言。清單中的八個 Playwright 情境當時均以工作區工具鏈在本機 Worker 通過。

新增矩陣時，須以合法命令建立受測效果，提供可獨立失敗的基準、修飾及組合案例，並視情況檢查正規結果、受影響及不受影響的後果、代價、牌移動、最終狀態與重播。多個無效依據以集合比較並拒絕重複。見[規則測試檢查表](../docs/rule.md#9-testing-checklist)。

移除測試仍須為每項獨有斷言提供具名且通過的證據，並共同審查判定。保留獨立匹配器、目錄、序列化、驗證原子性、公開視圖及重播契約。清單中歷史 `cargo test --test <old-suite>` 命令記錄較早執行方式；目前重點測試使用 `cargo test --test single <module>`。
