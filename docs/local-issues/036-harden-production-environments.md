# 36 Harden production environments

## Triage

ready-for-agent

## What to build

作為操作人員，我希望開發便利功能與類正式環境隔離，使 staging 與 production 一律依公開規則建立及呈現遊戲。

只有本機開發可公開測試資料、以測試資料為基礎的設定遞補、診斷介面或僅供開發的介面卡。staging 與 production 必須使用相同遊戲玩法、權限及玩家介面，只在部署來源、綁定、秘密與持久化資料上不同。

## Acceptance criteria

- [x] 應用程式明確區分本機開發、staging 及 production 環境。
- [x] `APP_ENV=development|staging|production` 是唯一環境政策輸入；應用程式行為不從 URL、`NODE_ENV` 或 Wrangler 環境名稱推導。
- [x] 類正式部署缺少或提供無效 `APP_ENV` 時明確失敗，不預設使用開發行為。
- [x] staging 與 production 使用相同遊戲規則、權限及玩家介面。
- [x] 只有本機開發環境允許公開測試便利功能。
- [x] 類正式環境建立遊戲時，使用權威房間參與者及規則集定義的預設值，不使用測試玩家、測試順序或固定遞補種子。
- [x] 房間提供實際參與者及玩家人數；所選規則集提供初始生命值及官方牌組等預設值。
- [x] 對局開始時隨機決定先手、四人隊伍及行動順序與洗牌種子。
- [x] 產生的設定及隨機結果只持久化一次，供重新連線及重播重用。
- [x] 基礎規則集提供正式設定路徑，結合實際房間參與者、官方規則集預設值及牌組成。
- [x] `sample_game_setup` 與測試身分或順序僅限測試及僅供開發使用的介面。
- [x] 正式程式碼不透過範例或測試設定間接取得官方牌組成。
- [x] 類正式玩家頁面不含診斷控制、測試身分、內部事件名稱或僅原型使用的標籤。
- [x] 匿名訪客遊戲在 staging 及 production 仍作為產品功能提供，標示 `以訪客身份遊玩`，而非快速開始或測試捷徑。
- [x] 訪客顯示名稱使用適合產品的產生身分，絕不暴露 Alice 或 Bob 等測試名稱。
- [x] 移除多餘原型標示（`GAME LOBBY`、`MATCH READY`、`WAITING ROOM` 及 `GAME SET`），不翻譯後保留。
- [x] 初始客戶端狀態不預填 `玩家 Alice` 等測試身分。
- [x] 新房間預設為可編輯名稱 `{host display name}的房間`，不使用原型名稱 `五行練習場`。
- [x] 僅供開發的伺服器路由及介面卡在 staging 與 production 不可用。
- [x] 僅供開發的介面排除在 staging 與 production bundle 外，而非僅隱藏。
- [x] 類正式環境中直接請求僅供開發的 API 會回傳 `404`，且不能呼叫本機程序。
- [x] 無論客戶端呈現哪些控制，伺服器授權及規則驗證仍具權威。
- [x] 即使在本機開發期間，正常登入、房間及對局畫面仍保有正式介面。
- [x] 測試資料或手動引擎控制只存在於僅供開發的 `/dev` 路由；staging 與 production 不含該路由。
- [x] 本機遊戲及 `/api/local-game` 僅供開發；staging 與 production 只能透過線上房間建立對局。
- [x] 類正式環境缺少綁定或秘密，或使用佔位值時，部署或啟動明確失敗，不靜默使用本機預設值。
- [x] Wrangler 定義明確隔離的 `staging` 及 `production` 環境，各含適合該環境的 `APP_ENV`、來源、儲存及 Durable Object 綁定。
- [x] 套件腳本只公開明確的類正式部署命令，例如 `cf:deploy:staging` 及 `cf:deploy:production`；未限定環境的 `cf:deploy` 不能發布本機預設值。
- [x] 環境矩陣測試驗證 `/dev` 及 `/api/local-game` 只在 development 運作，在 staging 與 production 回傳 `404`。
- [x] 類正式介面測試驗證不存在測試身分、原型英文標示及開發控制。
- [x] 設定測試驗證缺少或無效 `APP_ENV` 及類正式佔位綁定會明確失敗。
- [x] 對局開始測試驗證房間參與者、產生設定、隨機順序、官方牌組及洗牌種子均已持久化。

## Non-goals

- 改變遊戲規則。
- 讓 staging 共用 production 資料或秘密。
- 新增初學者或學習模式。

## Blocked by

- 無，可立即開始
