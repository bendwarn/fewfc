# 60 Release the Confluence Generation Rule Module

## Triage

ready-for-agent

## What to build

使用官方 5.16 規則，將匯流世代作為完整的單一主題規則模組加入。交付全部九種職業、跨模組繼承、能力及陣法；持久化公開次數限制；並透過確定性事件及重播結算所有回收、牌堆、保留牌及具型別選擇流程。

## Acceptance criteria

- [x] 匯流世代可獨立選用，依賴全部三個進階規則模組，並在新遊戲及房間預設啟用。
- [x] 所有職業轉職、繼承能力、陣法及順序規則均符合 5.16 PDF，包括道法師及虛空破滅者互動。
- [x] 餘行與餘級讀取現存可回收棄牌的牌面定義；棄牌不存在或被移走時，依賴它的行動不可用。
- [x] 調律執行回合內用牌義務，易弦只擴大公開規則指定範圍，天響回收則不受此限制。
- [x] 次數消耗、上限變化、回復、洗牌回復及重新取得職業時重設均為正規、可重播且公開可見。
- [x] 共用牌堆洗牌回復所有符合條件的順風；個人牌組洗牌只回復受影響牌堆所有者的次數。
- [x] 具型別待選擇事項涵蓋所有牌堆檢視、保留牌集合、五鳴術選擇及晴風牌頂決策，不洩漏隱藏資訊。
- [x] 重點 Rust、Web 單元及自足 Brave Playwright 測試涵蓋所有職業系統、兩種牌堆模式、重播、依賴及全啟用相容性。

## Validation

- `cargo test` 與嚴格 Clippy 通過，包含 `tests/confluence_generation.rs`、兩種牌堆模式、重播及全啟用回歸測試。
- Web 單元測試、Nuxt 型別檢查、正式建置及 Playwright 規格探索通過。
- 目錄契約測試涵蓋全部 9 個職業及 13 個職業陣法。
- 完整自足 Brave 測試套件 19 項全部通過，包含 `apps/web/tests/e2e/confluence-generation.spec.ts`。

## Blocked by

- [#58 Compose Profession and Rule Module catalogs](058-compose-profession-and-rule-module-catalogs.md)
- [#59 Release the Jianghu Rule Module](059-release-jianghu-rule-module.md)
