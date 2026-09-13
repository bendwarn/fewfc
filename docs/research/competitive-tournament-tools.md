# FEWFC 線上賽工具研究

研究日期：2026-09-12。狀態：研究建議，尚非架構決策或實作規格。

## 研究結論

建議先驗證 **brackets-manager.js＋brackets-viewer.js，搭配 FEWFC 自有賽事流程**：以小型單淘汰賽完成報名、隊伍確認、抽籤、賽程、開房、結果回收、晉級與裁判介入，所有使用者操作都在本站。這組工具的價值是重用賽程結構與結果傳播；不能把它當成已包含報名、身分驗證與線上遊戲串接的完整平台。[manager 官方說明](https://github.com/Drarig29/brackets-manager.js)、[viewer 官方說明](https://github.com/Drarig29/brackets-viewer.js)

若首版必須有瑞士制或複雜多階段賽事，優先評估 **Toornament API**；若希望外包常見配對與晉級、保留本站隊伍模型，可把 **Challonge v2.1** 當第二個 API 候選。兩者都仍需本站 UI 與遊戲結果轉接，也都有外部服務及方案依賴。自架完整平台 **Bracket（evroon/bracket）** 則適合願意承擔額外服務部署的情況。這個排序是依本次需求作出的工程判斷，並非已決定採用。

## 已知需求與本庫邊界

以下來自使用者本次討論，不是工具提供的能力：

- 隊伍是參賽單位，包含單人隊；不將首版限縮成僅支援雙人對局。
- 報名、配對、賽程、晉級及主辦操作都在 FEWFC 網站完成；天梯暫緩。
- 沿用現有遊戲規則。對局、計時及正式結果由 FEWFC 對局系統負責。
- 已同意五分鐘主時間，耗盡後每回合 30 秒讀秒；耗盡當回合可接完整讀秒，兩個時間值可自訂。等待玩家操作才扣時間，系統結算不扣；斷線繼續計時，競賽超時判負。
- 目前玩法名稱為「自由對局・切磋」，計時玩法為「競賽對局・論劍」，未來保留「天梯對局・問鼎」。賽事可以由多場競賽對局組成。

本庫的 `Game Outcome` 是單局勝方或 Draw，`Game Conclusion` 包含結果與結束原因；賽事勝場數及排名積分應另行命名，不能拿攻擊點數或剩餘 HP 代替。外部工具的 `match` 通常代表一次賽程交手，可包含 BO 系列多局，不能直接等同一份 `Game Record`。[本庫詞彙](../../CONTEXT.md)

現有隊伍設定要求兩隊等人數及交錯座位。以隊伍報名可以沿用此規則，但跨賽事的隊伍身分、凍結名單與每局 `TeamId` 的映射仍須由網站處理；單人隊與多人隊共用報名模型，不代表允許 1 人對 3 人開局。[隊伍設定約束](../rules-engine-decisions.md#28-team-setup-validation)

「引擎負責計時」需要在設計時釐清實作邊界：Rust Rules Engine 被定義為純函式庫，規則解析禁止依賴 wall-clock 或 IO。可研究由權威對局服務讀取時間，把可驗證的超時事實交給決定性邏輯處理；本報告沒有決定新命令或事件格式。重播仍應依既有權威紀錄，且 canonical 格式變動須有 migration。[UI／平台決策](../ui-platform-decision.md)、[引擎決策](../rules-engine-decisions.md)、[持久化與重播規則](../rule.md#6-persistence-and-replay)

隊伍參賽已確認；多人隊究竟共用一份主時間，還是每名玩家各有主時間，並未因「以隊伍為單位」而自動確定，見文末待決策。

## 工具比較

表內「未確認」表示本次官方證據不足，不等於功能不存在。費用以研究日可取得的官方資料為準。

| 面向 | brackets-manager＋viewer | Challonge API v2.1 | Toornament API v2 | Bracket 自架平台 |
| --- | --- | --- | --- | --- |
| 形態 | 嵌入 JS／TS 函式庫；本站持有資料 | API SaaS，可另嵌入外部賽程圖 | API SaaS，本站可自製 UI | 完整自架網站與後端 |
| 主要賽制 | 單淘汰、雙淘汰、循環；多 stage | 單淘汰、雙淘汰、循環、瑞士；分組接決賽 | 單／雙淘汰、循環、瑞士、聯賽、自訂 bracket 等 | 單淘汰、循環、瑞士、多 stage |
| 隊伍 | 一個 participant 對應一隊；名單由本站管理 | participant 可對應隊名；本站保留完整成員模型 | 原生 team participant、lineup、外部識別碼 | 原生隊伍及成員；官方指南以隊伍表達個人參賽 |
| 報名 | 本站實作 | 可由本站後端新增 participant；核准流程仍須設計 | Registrations API 可建立、核准／拒絕等 | 官方流程偏主辦新增隊伍；完整玩家自助報名未確認 |
| 結果與晉級 | 更新 match／matchGame，自動傳播勝敗方 | API 回報比分及晉級者；處理賽事狀態 | match／game／report API；結構與晉級設定 | 後台輸入結果、推進階段；遊戲接入須另做 |
| 平手／BO | 循環可平手，淘汰拒絕；支援 BO 子局 | v2.1 循環／瑞士可 tie；有 score_set，BO 停止條件須驗證 | win／draw／loss；原生 best-of 與多種局數格式 | 勝和負積分可設；BO 自動化與淘汰平手未確認 |
| 日曆式排程 | 本站補開賽時間、報到及並行限制 | 有 scheduled_time；自動時間衝突排除未確認 | 可排定 match 時間；完整自動排程策略須驗證 | 可安排開始時間、場地，提供自動排程 |
| 外部／技術依賴 | manager 偏 Node 環境；viewer 原生 JS，無特定 UI 框架要求 | Challonge 服務、主辦憑證、網路及配額；iframe 另依賴外部頁面 | Toornament project、API key、OAuth token、合適方案 | Python FastAPI、Vite／Mantine、PostgreSQL；額外部署與登入整合 |
| 授權／成本 | LICENSE 為 MIT；manager metadata 有 ISC 差異，見下節；自付開發與運維 | 商業服務；API 方案與 Premier 分開，完整 API 價格未確認 | 商業訂閱；2026 已改方案，完整 API 報價未確認 | AGPL-3.0；自付基礎設施、維護及改造成本 |

各欄的官方依據及限制如下。

### brackets-manager.js／brackets-viewer.js

manager 官方列出三種賽制、BYE、棄權、多 stage 與抽象儲存介面；沒有建立或修改賽事的成品 GUI。多 stage 能表達循環接淘汰，但 FEWFC 仍須定義前幾名晉級、同分排序及何時建立下一階段；不能把「支援多 stage」推論成任何跨階段規則都會自動完成。[manager README](https://github.com/Drarig29/brackets-manager.js)

更新介面會把勝敗方送入後續交手；BO 子局可更新父 match。循環賽可 Draw，淘汰賽拒絕 Draw，BO 若形成無法決勝的平手也可能報錯。官方建議在下游開始前修改結果。因此 FEWFC 的真實平局、加賽與裁判改判必須先有映射規則；不能把平局改成棄權來滿足 API。[更新與 BO 文件](https://drarig29.github.io/brackets-docs/user-guide/updating-matches/)

viewer 是原生 JS，支援翻譯、CSS 主題及點擊交手事件，可在 Nuxt 的客戶端元件內包裝，點擊後導向本站房間；這是整合推論，尚未實測。可自行打包 JS／CSS，沒有必須使用公共 CDN 的產品要求。[viewer README](https://github.com/Drarig29/brackets-viewer.js)

授權查核發現不一致：manager 與 viewer 的 `LICENSE` 都是 MIT，但查到 manager 主分支 `package.json` 的 `license` 為 ISC。該檔版本為 `1.11.0`、Node 要求為 `>=14`；這是主分支資訊，**不保證等於 npm 最新發布包或網站文件對應版本**。正式採用前應固定發布版，檢查 tarball 中的 LICENSE、metadata 與所用 API；此報告未安裝套件。[manager LICENSE](https://raw.githubusercontent.com/Drarig29/brackets-manager.js/main/LICENSE)、[viewer LICENSE](https://raw.githubusercontent.com/Drarig29/brackets-viewer.js/master/LICENSE)、[manager package.json](https://raw.githubusercontent.com/Drarig29/brackets-manager.js/main/package.json)

儲存介面可接多種資料庫，不代表在 FEWFC 的 Cloudflare runtime 已驗證可用。POC 應測試打包、Node 依賴、一次結果更新涉及多筆資料時的交易，以及重複呼叫；不要直接採用 README 的 JSON 檔案範例作正式儲存。這是依本庫平台與套件介面的工程建議。

### Challonge

官方目前將 v2.1 列為現行 API，v1／v2.0 標為 deprecated。2026-06-19 更新的遷移指南說明：v2.1 採 snake_case、恢復穩定數字 tournament ID、增加分組賽控制，循環及瑞士制可用 `tie` 回報平手，平手時各方不能標為 advancing。新整合應鎖定 v2.1，不要照抄舊 `winner_id="tie"` 或 v2.0 camelCase payload。[現行文件入口](https://challonge.apidog.io/)、[v2.1 遷移指南](https://challonge.apidog.io/upgrading-from-v2-0-to-v2-1-1726691m0)

官方 schema 列出單／雙淘汰、循環、瑞士等賽制；但舊 Connect schema 在部分欄位仍有 camelCase，應與現行 v2.1 文件交叉核對，不直接生成正式 client。[官方 schema](https://connect.challonge.com/docs.json)、[建立賽事](https://challonge.apidog.io/create-tournament-23619740e0)

本站可將每隊映射成一個 participant，保存供應商 ID 與本站隊伍 ID 的對照。新增 participant 的 `name`、`seed`、`misc` 足夠承載基本映射；API 會在提供 email／username 時邀請 Challonge 使用者。因此以主辦憑證代建、不發送邀請，是值得驗證的免玩家跨平台帳號流程。此次未確認 API 有等同本站需求的完整名單／替補模型。[Create Participant](https://challonge.apidog.io/create-participant-23619750e0)

v2.1 `MatchInput` 有 participant_id、score_set、rank、advancing、tie 與 scheduled_time，可回報系列比分及開賽時間；但 schema 本身不足以證明會自動依 FEWFC 的 BO／平局重賽政策結束系列，須以測試賽驗證。iframe 的 JSAPI 主要提供顯示控制與事件，不能取代報名與權威結果 API。[MatchInput](https://challonge.apidog.io/matchinput-10582249d0)、[Bracket JSAPI](https://challonge.apidog.io/getting-started-1726701m0)

官方功能表顯示 Standard 的 Sandbox 為每月 500 requests、Premier 的 Sandbox+ 為 2,000 requests，Premier 有付費 API 方案每月 US$7 折扣，且 API 方案與 Premier 分開管理。這不代表正式賽事可免費無限制使用；本次 Connect 定價頁未能取得完整內容，付費級距、超額規則、速率及 SLA 均待登入或詢價確認。[官方 API 功能／配額](https://docs.challonge.com/features/integrations)、[Connect 方案入口](https://connect.challonge.com/)

Challonge 是商業 SaaS；官方 schema 的 MIT 標記不能解讀為整個服務可自架或免費商用的授權。整合前須確認帳號方案的 API 使用、品牌顯示及資料處理條件；本研究不作法律結論。[服務條款入口](https://challonge.com/terms_of_service)

### Toornament

Structure API 列出單／雙淘汰、分組循環、瑞士、league、自訂 bracket 等，且 stage 可設定 best-of。瑞士制具配對方法、積分計算及 tiebreaker 設定；若這些是首版必要功能，比自行擴充僅有三種賽制的 manager 更值得驗證。[Stages API](https://developer.toornament.com/v2/doc/organizer_stages)、[Stage 概念與瑞士制設定](https://developer.toornament.com/v2/core-concepts/structure/stage)

Registrations API 支援建立及更新報名狀態；team participant 有 lineup 與外部 custom_user_identifier。可讓本站管理報名 UI，再由後端送出隊伍資料。官方範例包含只有一位成員的 lineup，但 FEWFC discipline 的最小隊伍人數仍須在測試專案確認。賽事 participant 的 lineup 不等於已取得 Platform API 的跨賽事隊伍帳號能力。[Registrations API](https://developer.toornament.com/v2/doc/organizer_registrations)、[Participants API](https://developer.toornament.com/v2/doc/organizer_participants)

Match API 可更新賽程時間與交手結果，另有 match games 及 match reports；結果支援 win／draw／loss。格式文件提供 BO、固定局數等及依結果計算系列勝場，適合映射 FEWFC 單局 Game Conclusion。但「淘汰系列最後平手如何加賽」仍是賽事政策，不能只靠 draw 欄位決定。[Match API](https://developer.toornament.com/v2/doc/match_overview)、[Matches API](https://developer.toornament.com/v2/doc/organizer_matches)、[Match Formats](https://help.toornament.com/match/define-your-match-format)

可用 match.result_updated／match.status_updated webhook 接收變化；通知提供資源識別資訊，再查資源取得內容。本站仍須處理重複、延遲與重新同步，避免供應商後台改動和引擎結果互相覆寫。[Webhook 文件](https://developer.toornament.com/v2/overview/webhook)

API 需要有 API access 的 project plan、application、API key 與 OAuth Client Credentials token，application 的資料範圍限所屬 project。2026-03 官方公告已更新方案：Organizer 的 Arena 包含 API；Game Studio 的 Starter 也列 essential API。它們不保證涵蓋相同 API 範圍。[API 入門](https://developer.toornament.com/v2/overview/get-started)、[2026 新方案公告](https://blog.toornament.com/2026/03/new-plans-and-pricing/)

本次官方 pricing 頁未取得可讀價格表，故不引用第三方報出的月費，也不沿用舊 Pro／API 方案價格。需詢問 FEWFC 自製遊戲的 discipline、報名與 Structure／Match／Webhook API 權限、流量、白牌／本站登入需求，以及月繳／年繳、稅額與超額成本。API SaaS 未提供本次可確認的公開 self-host 方案。[官方定價](https://www.toornament.com/en_GB/pricing)、[API Terms and Guidelines](https://developer.toornament.com/terms-guidelines)

### Bracket（evroon/bracket）

這是完整自架平台，官方列出單淘汰、循環、瑞士、多階段、隊伍成員與排程能力；使用 FastAPI、Vite／Mantine 與 PostgreSQL，可透過 Docker 部署。相較於嵌入函式庫，需要額外維運服務、資料庫及前端，與本站 Nuxt 的整合成本較高。[官方專案](https://github.com/evroon/bracket)

2026-09-08 更新的使用指南以隊伍為單位，個人賽也建立隊伍；成員資料被描述為展示用途，不能直接取代 FEWFC 入房授權。指南支援勝和負積分、安排場地／時間與階段晉級，但呈現的是主辦新增隊伍及輸入結果流程，未證實有完整玩家自助報名、FEWFC BO 自動化或權威結果 webhook。這些列為未確認，而非宣稱不支援。[官方使用指南](https://docs.bracketapp.nl/docs/usage/guide/)

專案標示 AGPL-3.0。選用前應讀取鎖定版本 LICENSE，評估修改服務及與本站組合方式的義務；不能因為可免費下載就視為與 MIT 相同。研究排序較後，原因是額外平台整合與維運，不是已判定授權不可用。[專案與 LICENSE 入口](https://github.com/evroon/bracket)

## 小範圍整合建議（待決策）

建議首個可驗收範圍為 **4–16 隊單淘汰、BO1 或 BO3、完整站內流程**。隊伍人數由賽事固定，可舉辦單人隊或多人隊的賽事；這是縮小驗證規模的建議，不是工具上限，也不是把多人隊延後。

| 責任 | 建議由誰持有 | 核心交付 |
| --- | --- | --- |
| 隊伍與報名 | FEWFC 網站 | 隊長報名、隊員確認、名單鎖定、報到與退賽 |
| 配對與晉級 | FEWFC 賽事服務呼叫 manager | 種子／抽籤結果、BYE、交手依賴、結果傳播 |
| 賽程與開房 | FEWFC 網站／權威服務 | 開始時間或前場結束後開賽、並行限制、指定成員入房 |
| 單局 | FEWFC 對局系統 | 規則與計時、重連、正式 Game Conclusion、Game Record |
| BO 與平局 | FEWFC 系列規則，轉接套件 | 每局引用、勝場累計、加賽及系列完成條件 |
| 賽程顯示 | viewer＋本站列表 | 圖形賽程、手機可讀列表、房間入口、結果與紀錄入口 |
| 裁判與稽核 | FEWFC 網站 | 棄權、缺席、爭議暫停、改判原因、操作紀錄 |

建議以「賽事交手 → 系列局次 → 房間／Game Record → 單局隊伍」保存明確對照。比賽結束後，以穩定識別碼與版本接受結果，重試不得重複加勝場；先持久化單局正式結果，再更新系列與賽程。若改用 SaaS，加入待同步狀態、重試及讀回核對，不讓服務暫時不可用導致單局結果遺失。以上是工程建議，未指定資料庫或建立新的正式領域術語。

對 BO3，建議網站先保存所有真實單局，包含 Draw；若選擇「平局不算勝場、加賽到先勝兩局」，不要假設三個固定 matchGame 槽能容納所有實際對局。可評估由本站管理整個系列，決勝後僅回報父 match 的 2–0／2–1 與勝方，viewer 點入本站再看完整局史。若決定使用套件子局自動計分，則先驗證加賽與平局政策與套件行為完全相符。

排程要區分「誰對誰」和「何時開打」。五分鐘主時間加每回合讀秒，沒有單局總時長上限；不能保證每局固定十分鐘結束。小型賽可先採前置交手結束後開放下一場，加上主辦設定的最早開賽時間與報到期限。若需要固定直播時段，須另訂緩衝與超出時段政策。

採用前 POC 的必要驗證：

1. 固定套件發布版、確認授權差異及所需 API；驗證既有 runtime 可打包及持久化。
2. 用非二次方隊伍數驗證 BYE、種子及正確晉級；同時驗證 1 人隊與多人隊的入房／座位映射。
3. 驗證 BO3 的 2–0、2–1、途中 Draw、超時、缺席及雙方棄權；不以虛構單局掩蓋賽事行政判定。
4. 驗證重複結果通知、服務重啟、並行寫入及已開始下游交手後的改判處置；保存原始 Game Record 與裁判裁決兩種證據。
5. 玩家與主辦都從本站完成報名到頒定名次；若選 SaaS，另測憑證失效、配額不足與暫時斷線，確認無必須跳往供應商網站的操作。

若首版必須瑞士制，建議改做一場小型 **Toornament API 測試賽**，先取得可覆蓋報名、結構、結果及 webhook 的確定報價，驗證相同驗收清單。Challonge 可以用更簡單的「本站完整名單＋遠端 participant／bracket」模式作比較，不預先承諾哪家更便宜。

## 待使用者決策

1. **首版賽制**：單淘汰是否足夠，或瑞士制／循環分組接淘汰是開賽必要條件？這最直接影響選庫。
2. **系列與平局**：BO1／BO3 如何配置；平局是否不計勝場並加賽；連續平局、雙方缺席如何結案？賽事規則不得篡改引擎 Draw。
3. **隊伍與計時**：每隊共用主時間或每人獨立；一名隊員超時如何影響整隊；一場賽事固定幾人上場、可否替補及鎖名單時點？
4. **報名與開賽**：公開報名或邀請、隊長權限、核准／報到期限、種子或隨機抽籤；前場完成即開賽或固定時段？
5. **裁判範圍**：誰能判棄權、暫停及改判；下游已開賽時是否凍結整條晉級路徑？
6. **外部依賴與預算**：是否接受 SaaS 保存隊名／名單與賽事結果；每月上限、預期隊數與場次、是否要求去品牌與完全免供應商帳號？

## 研究方法與限制

已先檢查 `docs/` 慣例；未發現既有 `docs/research/` 或更合適的研究集合，因此使用指定路徑。只建立本文件，未修改架構文件、程式、依賴或既有未提交變更。

採 research 技能的原始來源查證與單份 Markdown 產出流程。本次環境沒有背景代理工具，改由主代理直接 web 搜尋並閱讀官方 repo、API、產品公告及文件。未建立外部帳號、未呼叫需憑證的賽事 API、未安裝或執行候選工具；整合相容性屬待驗證推論。動態定價頁無法讀取、方案權限及發布包差異已逐項標明；沒有以搜尋摘要中的第三方價格補足。
