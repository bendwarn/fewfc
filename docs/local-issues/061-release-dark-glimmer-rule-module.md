# 61 Release the Dark Glimmer Rule Module

## Triage

ready-for-agent

## What to build

使用官方 5.16 規則，將黑暗微光作為完整的單一主題規則模組加入。交付全部五個職業、暗黑陣法、惡精靈與死精靈、常駐精靈技能，以及它們與環境、英雄學派、靈力、虛空碎靈術、重播、線上房間及戰場的跨模組互動。

## Acceptance criteria

- [x] 黑暗微光除全部三個進階規則模組外，還間接依賴精靈；它可獨立選用，並在新遊戲及房間預設啟用。
- [x] 所有黑暗微光轉職、繼承能力、暗黑陣法，以及環境或影遁互動均符合 5.16 PDF。
- [x] 惡精靈與死精靈占用既有單一精靈欄位，使用零至六點靈力模型，並區分變化與新召喚兩點靈力精靈。
- [x] 戲鬧只依實際檢視的牌計算；可信任隨機選擇記錄完整正規結果，不洩漏其他隱藏牌。
- [x] 同命依死精靈擁有者所屬隊伍實際因陣法效果扣除生命而觸發，排除一般攻擊傷害，且不能遞迴觸發自身。
- [x] 虛空碎靈術對存活死精靈觸發同命，對靈力歸零而破除的死精靈不觸發，也不追溯觸發魔靈復甦召喚的精靈。
- [x] 虛空碎靈術快照初始擁有者，並以原子且可重播方式結算精靈變化、復甦、隊伍生命值、同命及遊戲結果。
- [x] 重點 Rust、Web 單元及自足 Brave Playwright 測試涵蓋所有職業、暗黑陣法、兩種精靈、雙人及四人隊伍生命值互動、依賴與全啟用相容性。

## Validation

- `cargo test` 與嚴格 Clippy 通過，包含 `tests/dark_glimmer.rs`、可信任隨機手牌選擇、雙人及四人受影響集合、原子碎靈、重播及全啟用回歸測試。
- Web 單元測試、Nuxt 型別檢查、正式建置及 Playwright 規格探索通過。
- 目錄契約測試涵蓋全部 5 個職業及 10 個暗黑陣法。
- 完整自足 Brave 測試套件 19 項全部通過，包含 `apps/web/tests/e2e/dark-glimmer.spec.ts`。

## Blocked by

- [#57 Resolve Bloom and Void Spirit-Shattering atomically](057-resolve-bloom-and-void-spirit-shattering.md)
- [#58 Compose Profession and Rule Module catalogs](058-compose-profession-and-rule-module-catalogs.md)
- [#60 Release the Confluence Generation Rule Module](060-release-confluence-generation-rule-module.md)
