# 59 Release the Jianghu Rule Module

## Triage

ready-for-agent

## What to build

使用官方 5.16 規則，將江湖作為完整的單一主題規則模組加入。交付全部九種職業、轉職、能力及陣法；以具型別的江湖狀態建模千鋒、踏雪與中毒；並透過正規重播、公開視圖、可執行行動、線上房間及戰場呈現完整行為。

## Acceptance criteria

- [x] 江湖可獨立選用，依賴全部三個進階規則模組，並在新遊戲及房間預設啟用。
- [x] 每個江湖職業轉職、繼承能力、發動能力、普通能力及職業陣法均符合 5.16 PDF。
- [x] 千鋒與踏雪立即生效，保留公開規則的持續時間及疊加行為，並整合到正確攻擊結算階段。
- [x] 中毒合併剩餘回合，於受影響玩家回合結束時扣除生命，只阻止陣法造成的生命回復，並套用唯一上家來源的毒絕。
- [x] 星行牌讀取與施展玩家隊伍星辰相同的牌面屬性；流星步使用此定義，不使用星辰變牌。
- [x] 正規事件、重播、驗證、觀看者過濾、重新連線、職業徽章、江湖狀態呈現及合法行動控制一致。
- [x] 重點 Rust、Web 單元及自足的 Brave Playwright 測試涵蓋所有職業系統、狀態時點、雙人及四人遊戲、依賴與全啟用相容性。

## Validation

- `cargo test` 與嚴格 Clippy 通過，包含 `tests/jianghu.rs` 及全啟用回歸測試。
- Web 單元測試、Nuxt 型別檢查、正式建置及 Playwright 規格探索通過。
- 目錄契約測試涵蓋全部 9 個職業及 14 個職業陣法。
- 完整自足 Brave 測試套件 19 項全部通過，包含 `apps/web/tests/e2e/jianghu-rule-module.spec.ts`。

## Blocked by

- [#58 Compose Profession and Rule Module catalogs](058-compose-profession-and-rule-module-catalogs.md)
