# 競賽功能審核與相依工作

更新：2026-10-05。

使用者已同意以目前完整檔案對照已確認規格審核；核心檔案未追蹤，沒有可精確分離 Helmholtz 最後一輪的 Git 差異。本紀錄不把完整檔案的每一行都歸因於該代理。

本任務沿用互斥檔案所有權。

## 每個新功能開工前：開源實作參考

2026-10-03 使用者新增要求：每當開始下一個功能，先參考 [evroon/bracket](https://github.com/evroon/bracket) 或實際相關的類似專案；語言不同不構成排除理由，須評估翻譯成 Rust 的好處。此步是新增實作的前置，不只是整個競賽專案查一次；當前修復／交付仍依原互斥所有權進行，相依新功能也須先完成此步。

1. 依功能選定相關模組，讀官方原始碼、資料模型、實際介面及測試；記錄查閱日期、固定 commit／版本與來源永久連結。只有 README 的功能宣稱不足以推定實作細節；查不到則明列缺口，再找相關專案，不憑空補演算法。
2. 對照本 repo 已有模組，比較直接採用、將可獨立的邏輯翻譯成 Rust、只借鑑模型／流程，以及維持自行實作。評估正確性／測試證據、Rust／WASM 適配、效能、維護成本與重用收益；不因語言不同就拒絕，也不把 Python／TypeScript 一律搬進 Rust。
3. 查實際授權與依賴條件，區分概念參考與取用／翻譯程式碼；未確認可採用前不直接複製程式、加依賴或改變本 repo 授權。不把換成 Rust 視為已解決授權問題。
4. 記錄「功能、參考模組／測試、已核實行為、與本站差異、Rust 移植效益／成本、採用或不採用理由、授權及未解決事項」。評估通過後，才分配有明確互斥檔案範圍的 Luna 實作。
5. 參考不取代已確認產品規格；先讀秒判負、DoubleLoss、名次、觀戰／賽後公開、可信時間及私密資料保護仍依本站規格。不重新建立已完成的賽程／時計演算法，也不恢復已取消的生命值專項測試或舊賽事相容性工作。

首份逐模組參考已由背景 Luna Hume 完成，主審全文讀 [Bracket 功能參考](bracket-feature-reference.md)，另直接核對固定 的 rounds planning、stage input model與授權標示。Timing 切片採模型／流程參考：plan／apply 分離、來源名額與相依狀態；保留現有 Rust/WASM 與 DO 責任，不直接導入另一套 runtime、場地排程或隨機建議器。只有出現既有核心缺少的純領域需求，才另評估在 Rust 建立相依／階段變更計畫；本次沒有翻譯／複製外部程式、新增依賴或變更授權。這份評估不取代後續每個新功能的參考前置。

Timing 的產品政策已確認：[開賽與裁判](../competitive-play-design.md#開賽與裁判) 規定尚未開打的受影響房間允許撤銷重建，由主辦重新公告開始時間，再滿足全房提前五分鐘就緒。Lagrange 正依此修訂共同協議；舊 proposal 的「物化即拒絕影響配對的改判」不採用。外部來源沒有證成本站跨房撤銷／重新公告協議，須審核持久撤銷確認、GameStarted 競態與新舊 generation 隔離後，才分配產品接線。既有修復／驗證繼續。

主審已分段讀修訂共同協議 §1–14，撤銷／start 本地線性化、完整 ledger／quiescing、真 start 證據不可丟棄、全撤銷後共同 late apply、原七欄命令／五欄成功收據、主辦 fresh 公告的方向通過。接受到 T 才派 release 及真實分散式交付延遲，不承諾同毫秒開局；exact shared shape／release 身分及首局與後續局的分流仍先收斂。這是方案審核，不是時間功能放行；當前 native 尚未收尾，產品來源仍凍結。

共同方案最終 已補閉 release commandId＝decisionId、後局新BindingRef沿同輪決議／不重套首局屏障、真canonical startSequence；主審已讀並批准此階段界線。本機Worker45651停止exit0、自有Brave關閉exit0後，已派Lagrange獨占shared competition-game-room兩檔的第一步：只新增共同types／strict validators及`round-release`前綴測試，不改既有binding必填欄位／runtime行為、不接DO／HTTP，不允許optional／legacy降級。集中驗證此共同schema後才分派父／子producer-consumer同步接線；此派工不是五分鐘／撤銷／實際開賽功能通過。

## 審核關卡

| 工作 | 擁有範圍 | 啟動下一階段的條件 |
| --- | --- | --- |
| Helmholtz 核心成果 | competition_clock、tournament 與生命週期測試 | 規範／規格審核，計時恢復及結果更正回歸測試通過 |
| Harvey 網站橋接 | competition_web_api、web_api、wasm、lib 與橋接測試 | 可信輸入、camelCase、錯誤與部分成功狀態、私密資訊投影通過審核 |
| Laplace 交手編排 | competition_match 與自身測試 | 結果冪等、一般局／加賽、裁判權限與更正歷史通過審核 |

尚未通過審核不能當作下一階段的穩定契約。原有全套測試通過紀錄不能替代新增回歸案例。

## 已識別的待驗證問題

- 準備階段雙方進入讀秒後完成準備，checkpoint 仍保留沒有 action turn 的讀秒額度，但恢復檢查只允許 running preparation 的此類狀態，可能拒絕合法 checkpoint。
- 最小重現已放入 `tests/competition_review_regressions.rs`。`cargo test --test competition_review_regressions --quiet` 實測 1 失敗，錯誤為 `InvalidCheckpoint("action time cannot exist without an action turn")`。已交回 Helmholtz 修復；重審通過前不啟動依賴計時恢復的持久化接線。

## 通過後的相依工作

### 本輪追蹤結果

最新接續（完成通知後）：

- 已收到所有上一輪回報，不等待定時排程；重複排程已刪除。
- 主代理核實橋接為可信伺服器內部 ABI，接收 actor 與回傳 serverState 本身不是公開 HTTP 漏洞。不可把未來直接轉送 HTTP 的假設當成現存漏洞。Harvey 的 Rust 橋接限定範圍通過，已指派同一 Luna 實作 TypeScript/WASM adapter 及安全投影；不授權公開 HTTP 或 DO 接線。
- 計時修復新增測試有一處 bool 直接指定給 serde_json::Value 的編譯錯誤，主代理已修正。集中執行 clock lifecycle 17、review regression 1、web bridge 5，共 23 項通過；接續 `cargo test --quiet` 602 項通過。這是本次編譯快照的結果，不代表尚在修正的交手規格問題已解決。
- Laplace 的已報編譯／固定局數問題已修改，但先手指派可覆寫 Assigned 與 NeedsDecision 原因未驗證原始結論仍待修正，已交回同一 Luna。主代理加入正式 crate export，要求其測試改用公開 API。交手相依工作不放行。
- Standards 軸撤回「timeout 覆蓋正常勝負」指控；Spec 軸確認 DoubleLoss 結果有保存，typed 裁判狀態仍為後續接口風險，而非已遺失結果。任意篡改整份內部快照的歷史真偽與快照內部一致性須分開評估。

### 2026-09-20 放行與下一階段

- Helmholtz 計時修復通過：合法準備完成 checkpoint、非法額度與矛盾狀態皆有回歸覆蓋。
- Laplace 交手編排修復通過：正式 crate export、固定局數、累積比分、先手冪等／不可覆寫、和局交換與 NeedsDecision 對應結論已驗證。
- Harvey 的 TypeScript／WASM adapter 通過：7 項 Bun 測試與 Nuxt typecheck 通過；公開／私人投影不包含 serverState。
- 最新完整 `cargo test --quiet` 共 604 項通過，`cargo check --target wasm32-unknown-unknown` 通過。
- 已依 Durable Objects、Workers best practices 與 Wrangler 技能查核官方現行文件，啟動相依階段 D：每個賽事一個 SQLite-backed CompetitionRoom，集中 serverState、ACL、revision、冪等與脫敏稽核。此階段不開公開 HTTP，也不宣稱 match-level 裁判利益衝突已完成。
- 階段 D 已於完整現況重審後放行：公開報名者不必預先存在 ACL；純查詢不推進 revision 或寫入私密投影；mutation 使用非遞減可信時間、語意冪等收據與原子 SQLite 交易；裁判授權／撤銷可安全重試；舊無版本 schema 可原地升級至 v3。主代理重跑 20 項 Bun 測試共 121 個 assertion 通過，Wrangler 4.130 staging dry-run 正確辨識 `COMPETITION_ROOM`，`git diff --check` 通過。另以 `WebAssembly.compile` 後呼叫 module overload 修正既存測試型別歧義；Nuxt typecheck 與該整合測試 1 項／21 個 assertion 已全數通過。
- 下一關卡為階段 E1：將已通過的 `competition_match` 接入可信 Rust/JSON/WASM bridge，checkpoint 必須以可重播命令歷史恢復並驗證，不接受呼叫端直接覆寫完整交手快照；裁判更正的 actor 必須由伺服器身分衍生。E1 通過後才建立每場交手各自的 match/clock SQLite 狀態與 revision。現有單一 `serverState.clock` 不能被當作可同時承載多場交手的完成方案。
- 階段 E1 已通過：五種 match operation 使用明確 camelCase Web DTO；`matches` 依 match id 保存 config 加已接受事件，恢復時一律重播核心方法；map key 必須與 `config.matchId` 一致，空 ID 與不合法歷史 fail closed；提交／更正冪等不重複寫 history；裁判 actor 與 payload 身分一致且自己的交手由核心拒絕；DoubleLoss 保留等待裁判，不產生虛構勝者。主代理重跑完整 Rust 613 項、WASM target、前端相關 24 項／155 assertions、Nuxt typecheck、`cargo fmt --check` 與 `git diff --check` 均通過。
- 已啟動階段 E2：在 CompetitionRoom SQLite 內按 match id 分離 match 與 clock checkpoint、revision、可信時間及稽核。全域報名／賽程 revision 不得被其他交手的計時觀測推進；clock 仍可重用既有 Rust operation，但持久化必須按交手隔離。E2 不公開 HTTP，也不把尚未接線的 GameRoom 視為可信結果來源。
- 階段 E2 已通過：`competition_matches` 依 match id 隔離 match／clock checkpoint、公開投影、revision 與非遞減可信時間；全域入口拒絕 match／clock 操作，scoped 入口拒絕跨交手或全域狀態污染；命令收據與稽核保留 match identity，成功 mutation 與全域 checkpoint 鏡像在同一 SQLite transaction 內完成。主代理重跑集中 30 項／203 assertions、Nuxt typecheck、`git diff --check` 與 Wrangler 4.130 staging dry-run 均通過；完整 web suite 的競賽相關測試全過，唯一失敗為未改動的 `managed-server.test.ts` 在建立暫存 PID 前逾時，單獨重跑可重現，與 E2 無耦合。
- 下一關卡為階段 E3：建立登入衍生 caller、伺服器產生可信時間與安全 HTTP allowlist。瀏覽器只能使用建立／讀取／報名與明確管理 API；`matchSubmitGame`、clock mutation 等可信內部操作不得由一般 HTTP payload 直送。E3 只建立伺服器邊界與測試，不接 GameRoom，也不開 UI。
- 階段 E3 已通過：新增建立、讀取、報名、裁判授權／撤銷、稽核與交手公開讀取共七個 HTTP route；caller、organizer、`registerTeam.captain`、可信時間、competition id 與空白初始 participant 清單都由伺服器衍生。公開 command allowlist 僅接受 `registrationView`／`registrationExecute`，match、clock、tournament mutation 在 DO dispatch 前即拒絕；錯誤映射不反射 DO message。主代理修正並驗證 invite-only 解析後，重跑 E1–E3 集中 37 項／210 assertions、Nuxt typecheck 與 `git diff --check` 全數通過。
- 下一關卡為階段 E4：GameRoom 與 match／clock 的可信內部編排。跨 Durable Object 更新必須採持久化 outbox 與固定 command id 可重試，不宣稱分散式原子交易；賽事房間名單、禁止觀戰與重播延後發布須由伺服器契約強制。
- 階段 E4a 已通過：新增 server-only competition game binding 與 GameRoom RPC，以單次 batch 原子建立／啟動固定成員、席位、隊伍、牌組、規則、先手及 turn order；同 binding 重試保留同一 Game Record。競賽房在 DO 內拒絕 outsider、observer、補位、一般建立／改房／重置與匿名 live-state GET；全員 connected＋ready 後才能由可信 RPC 啟動，斷線重連不更換席位、`gameInstanceId` 或紀錄。主代理重跑 E4a 與既有 GameRoom 聚焦 25 項／159 assertions、Nuxt typecheck、`git diff --check` 全過。完整 shared／DO suite 當時被平行 E4b1 尚未完成的 v5 migration 半成品阻斷，已交回該工作修正，不能把該次失敗歸入 E4a。
- 階段 E4b1 已通過：裁判 pause／resume 先以可信時間結算再凍結目前視窗，resume 只重設計時基準；三種視窗、期限優先、paused checkpoint v2 與 v1 向後讀取皆有 Rust 回歸。WASM／TS 使用明確 camelCase operation 與非空理由；CompetitionRoom 只允許該交手已指派、active 且無利益衝突的裁判操作，按交手 revision 原子保存 checkpoint、收據及理由稽核。主代理重跑完整 Rust 622 項、WASM target、完整 shared／Durable Objects 59 項／389 assertions、Nuxt typecheck、`cargo fmt --check` 與 `git diff --check` 全數通過。成功 resume、長時間暫停不扣時、冪等重試不重複推進 revision 也已有整合覆蓋。
- 下一關卡為階段 E4b2：把每個賽事階段的交手格式與時鐘設定固定為唯一可信來源；建立賽程交手與時鐘時只能由伺服器依階段設定物化，禁止呼叫端覆寫。此切片通過後才接 GameRoom terminal evidence／持久化 outbox；timeout terminal evidence、outbox 與 GameRoom 時鐘事件映射仍屬後續切片。
- 已指派 Luna 進行 E4b2a：先統一 Rust 階段設定、賽程比分驗證、JSON／WASM 與 TypeScript 契約；CompetitionRoom 依階段原子物化列為 E4b2b，待 E4b2a 契約通過後再啟動，避免同時改寫共用 DTO 與持久化邏輯。
- 階段 E4b2a 已通過：`StageConfig` 以 `matchFormat` 與 `clockConfig` 作為唯一交手／時鐘設定來源，移除重複的 `matchWinsRequired`；淘汰賽只接受 first-to-wins，循環／Swiss 只接受 fixed-games。賽程重用交手與時計核心驗證；fixed-games 的賽程比分不假定能從雙方勝局數重建單局和局數。主代理重跑完整 Rust 626 項、WASM target、完整 shared／Durable Objects 60 項／395 assertions、Nuxt typecheck、`cargo fmt --check` 與 `git diff --check` 全過。
- 下一切片 E4b2b 將在 CompetitionRoom 新增由已開始排程交手與階段設定驅動的原子物化入口，並關閉可自行指定 `matchCreate`／`clockCreate` 設定的旁路；完成後才進入 terminal evidence／outbox。
- 已指派 Luna 進行 E4b2b；寫入範圍限 CompetitionRoom contract／DO／tests，不與 GameRoom 或後續 outbox 重疊。
- 階段 E4b2b 已通過：新增 server-only `materializeScheduledMatch`，只從已開始排程交手與 StageConfig 衍生 teams、match format、clock config，首局保留等待可信隨機先手；direct `matchCreate`／`clockCreate` 旁路已封鎖。match＋clock 兩次純 engine 計算全部成功後才在單一 SQLite transaction 寫 row、global mirror、audit 與 receipt；晚期 receipt 寫入故障可完整回滾並安全重試。parser 對賽制、狀態、round/match、局次、結果與取消矛盾 fail closed。主代理重跑 29 項 CompetitionRoom／258 assertions、完整 shared／Durable Objects 70 項／487 assertions、Nuxt typecheck 與 `git diff --check` 全過。
- 下一關卡為 E4c：先建立 CompetitionRoom 的 typed terminal evidence 接收與原子交手提交；接收契約通過後，才讓 GameRoom 建立持久化 outbox，避免先產生沒有穩定接收端的跨 DO 訊息。
- 已指派 Luna 進行 E4c1：先把交手核心的單局終止來源改為 `canonicalGameEnded`／`timeout` 明確 union，讓超時不必偽造 GameConclusion；此契約通過後才實作 CompetitionRoom 的 E4c2 接收與判定。
- 階段 E4c1 已通過：`TrustedGameResult` 與 match `GameRecord` 改存 `terminalEvidence` union；canonical 結論只存在 `canonicalGameEnded`，timeout 則保存 timeout id、每隊權威 deadline、觀測時間與 clock checkpoint digest。單方／同 deadline 雙方超時、不同 deadline、錯隊、空識別／digest、觀測過早、錯 ruling pairing、冪等、checkpoint replay 與 correction 共用驗證皆有回歸。主代理重跑完整 Rust 629 項、WASM target、bridge＋shared＋Durable Objects 78 項／515 assertions、Nuxt typecheck、`cargo fmt --check` 與 `git diff --check` 全過。此為尚未部署契約的破壞性調整：舊頂層 `conclusion` payload／history 會 fail closed。
- 下一切片 E4c2 將新增 CompetitionRoom server-only terminal ingest：canonical 結論由持久化 clock 狀態產生競賽判定，timeout evidence 必須與同一 match 的權威 clock checkpoint／digest 一致，再與 match 提交於單一交易保存；呼叫端不得直接指定 ruling。
- 階段 E4c2 已通過：CompetitionRoom 新增 server-only `ingestGameTerminal`，呼叫端只交 canonical／timeout evidence，賽制 mode、競賽 ruling 與 timeout 判負皆由持久化 match／clock checkpoint 衍生。`clockRuleGame` 與 `matchSubmitGame` 成功後才在單一 SQLite transaction 推進一次 match revision、checkpoint、projection、global mirror、audit 與 receipt；直接提交旁路已封鎖。receipt 語意排除 trusted time 且先於 revision／時間／row 檢查；暫時性 adapter throw 不形成黏住重試的 receipt，確定性 engine failure 使用固定脫敏訊息。主代理退回一次修正後，重跑 focused 46 項／381 assertions、完整 shared／Durable Objects 83 項／595 assertions、Nuxt typecheck 與 `git diff --check` 全過。
- 下一切片 E4c3 將在 GameRoom 的 canonical checkpoint 交易中同時建立 terminal outbox，並以固定 command id、CompetitionRoom RPC、alarm 至少一次重送直到取得 E4c2 receipt；跨 Durable Object 不宣稱原子交易。此切片先處理規則引擎已完成的 canonical 結論，不自行偽造 timeout；timeout 仍須依賴後續權威 room clock 事件接線。
- 階段 E4c3 已通過：CompetitionGameBinding 固定 organizer 與建立該局時的 match revision；規則引擎首次進入 Finished 時，canonical conclusion、GameRoom checkpoint、metadata、receipt、event、terminal outbox 與 alarm 在同一 storage transaction 提交，缺少合法結論則整批 fail closed。outbox 使用 stable JSON tuple 的 版本 身分，固定 command／submission／game／ruling id，低延遲 waitUntil 與 alarm 共用 serialized delivery；RPC 失敗、格式錯誤、重啟及遠端已提交但回應遺失皆保留同一訊息安全重送，嚴格成功 ACK 後才清除。主代理兩次退回補足 transient/restart/strict ACK/collision 測試後，重跑 focused 24 項／172 assertions、完整 shared／Durable Objects 94 項／673 assertions、Nuxt typecheck 與 `git diff --check` 全過。跨 Durable Object 仍只宣稱 at-least-once delivery，不宣稱原子交易。
- 下一關卡 E4d 是把 GameRoom 真實流程映射到 CompetitionRoom 的權威隊伍時鐘：準備、行動、回合外選擇與期限觀測須以 server-only 固定命令持久重送；timeout terminal evidence 必須由已確認的 clock checkpoint／digest 產生。E4c3 binding 中的初始 match revision 只是起點，後續每個 clock ACK 必須推進 GameRoom 內持久的 revision cursor，terminal outbox 不得繼續使用過期 revision。
- 已指派 Luna 進行 E4d1：先讓 CompetitionRoom 對 clock mutation 的首次單方／雙方超時回傳由 pre-checkpoint 真實 deadline 與 post-checkpoint digest 組成的 typed timeout evidence；此切片不改 GameRoom。證據契約通過後才建立 ordered clock outbox、revision cursor 與房間流程映射。
- 階段 E4d1 已通過：CompetitionRoom 的 clock mutation 成功值以 exact camelCase 回傳 nullable typed timeout evidence；單方、同 deadline 雙方、不同 deadline、action、並行準備與回合外選擇皆由持久化 pre-checkpoint 的實際 deadline 推導，證據綁定 post-checkpoint digest 與 observation。主代理退回一次修正 Rust checkpoint invariant、active-byoyomi deadline 分支、unknown／extra result、算術溢位與非 timeout 邊界後，重跑 focused 53 項／446 assertions、完整 shared／Durable Objects 101 項／738 assertions、Nuxt typecheck 與 `git diff --check` 全過。成功收據重試與重啟保持同一 timeout id，交易失敗不推進 revision。
- 下一切片 E4d2 建立 GameRoom 內唯一 ordered competition outbox 與持久 revision cursor：clock mutation 與 terminal ingest 共用 FIFO、固定事件時間與 command identity；嚴格 ACK 後才原子推進 cursor／刪除 head，遠端成功但回應遺失、重啟、alarm 重送及既有 terminal 相容性必須通過。E4d2 先做傳輸基礎，不在同一切片猜測遊戲 phase 到 clock window 的完整映射。
- 階段 E4d2 已通過：GameRoom 以單一持久 `competitionDeliveryQueue` 保存 clock／terminal FIFO、match revision cursor、固定 trusted time／command identity、完整本局 clock enqueue receipt 與 pending timeout。主代理退回三輪修正 64 筆收據淘汰、無尾項 timeout alarm、strict projection／timeout ACK、clock 遠端已提交但回應遺失、clock→terminal 連續 revision，以及 canonical `GameEnded` 與公開結論交叉核對；玩家可見競賽拒絕文案同步改為繁中。主代理重跑 focused 35 項／283 assertions、完整 shared／Durable Objects 112 項／849 assertions、Nuxt typecheck 與 `git diff --check` 全過。跨 Durable Object 仍只宣稱 at-least-once delivery；未加入 revision rebase。
- 下一切片 E4d3 將把 GameRoom 真實狀態轉換映射為 clock window 命令，且命令必須和對應 canonical checkpoint 在同一房間 storage transaction 內加入既有 FIFO：遊戲開始開啟準備，隊伍全員完成初始準備才完成該隊準備，普通行動與回合外 pending choice 使用實際 team／turn／choice identity。`pendingTimeout` 必須先轉成 timeout terminal delivery 並凍結後續玩家命令；此切片不得猜測淘汰賽 DoubleLoss 晉級、零主時間或和局後先手等未決政策。
- E4d3 依共用契約再分兩步：E4d3a 先由 Rust clock 公開下一個權威轉換／到期 deadline，並讓 CompetitionRoom ACK／持久收據帶回 exact nullable deadline；E4d3b 才在 GameRoom 依該欄位共用 alarm 產生固定 `clockObserve`，同時做真實 phase mapping。這可避免 GameRoom 另寫一份主時間／讀秒／準備並行 deadline 演算法，也讓暫停、恢復及回合外選擇沿用同一權威 clock。
- 階段 E4d3a 已通過：Rust clock 由完整 checkpoint 狀態推導 nullable `nextTimeoutDeadlineMs`，涵蓋準備、主時間＋讀秒、回合外選擇、暫停、已超時、待裁決、無限時間與溢位；CompetitionRoom 的 live ACK 與持久收據皆嚴格驗證 deadline，不接受過期或與 clock projection 矛盾的值。零主時間準備沿用既有 `NeedsDecision::ZeroTimeControl`，未擅自產生 timeout。主代理重跑完整 Rust 656 項、focused Web 81 項／784 assertions、完整 shared／Durable Objects 115 項／942 assertions、Nuxt typecheck、`cargo fmt --check` 與 `git diff --check` 全過。
- 下一切片 E4d3b 為 GameRoom 房間計時編排：持久保存權威 deadline，alarm 取 delivery retry 與 deadline 的較早者；到期時以固定 identity 加入 `clockObserve`，且準備／行動／回合外選擇的 clock 命令須與 canonical game checkpoint 同一 storage transaction 入列。`pendingTimeout` 必須先轉成 timeout terminal delivery並凍結玩家命令；裁判 pause／resume 對 GameRoom alarm 的通知另列緊接相依切片，不得只依 CompetitionRoom 內部 deadline 宣稱已完成。
- 已指派 Luna「E4d3b－房間計時編排」，寫入範圍限 GameRoom、competition-game-room contract 與 online command transaction。
- 可獨立平行的 E4e1「賽果推進賽程」亦已指派 Luna，寫入範圍限 CompetitionRoom contract／DO／tests：正常已完成 match 必須在同一原子提交中推進 tournament；DoubleLoss 只保存 `pendingReferee` 並保持賽程未推進，裁判更正另列後續切片。
- 階段 E4d3b 已通過：GameRoom 以權威 `nextClockDeadlineMs` 與 delivery retry 共用單一 alarm，期限到達時建立固定 identity／trusted time 的 `clockObserve`；開始遊戲、隊伍完成準備、首個 action、換手及可驗證的回合外選擇皆與 canonical checkpoint 同一 storage transaction 入列。hidden choice 缺少 identity 時 fail closed。timeout evidence 先轉為同一 FIFO 的 timeout terminal delivery，pending／已確認 timeout 都凍結後續 mutation；主審另補抓並退修「重啟後 existing awaiting-randomness receipt 可越過 timeout freeze」缺陷，完整收據仍保持冪等。主代理重跑 E4d3b focused 49 項／411 assertions 通過。
- 階段 E4e1 已通過：terminal ingest 僅在 match 完成時，從持久 tournament 唯一定位 stage/match 並由已重播 match 的 games／rulings 映射 Win／Draw，比賽 checkpoint、clock、tournament、公開投影、audit 與 receipt 同一 SQLite transaction 保存；Swiss、循環、單敗、雙敗與 fixed-games 有回歸。DoubleLoss／NeedsDecision 保留 match `pendingReferee` 且不推賽程。主審退修延遲 outbox 被無關 global trusted time 阻擋及兩處型別缺口後，focused 63 項／579 assertions、完整 shared／Durable Objects 136 項／1110 assertions、Nuxt typecheck 與 `git diff --check` 全過。
- 下一個相依切片為 E4d3c：裁判 pause／resume 成功後，CompetitionRoom 必須以持久、可重試通知讓對應 GameRoom 更新／取消同一權威 deadline alarm；接著 E4e2 才處理裁判更正已完成賽果後的 tournament correction。兩者不可擅定 DoubleLoss 淘汰晉級。
- 主審發現 E4d3c 尚缺正式的 CompetitionRoom→GameRoom linkage：既有 create/start RPC 只有接收端與測試，沒有賽程物化呼叫或持久 game id。已先啟動 Luna「E4f1－賽程對局房物化與連結」，以 server-only request、鎖定名單衍生 binding、SQLite outbox 與固定 game id 建立 Waiting GameRoom；不自動 start，也不替 pending 先手或 2v2 隊內順序做產品決策。E4f1 通過後再回到 E4d3c。
- 階段 E4f1 已通過：CompetitionRoom v7 以 SQLite linkage／outbox 持久保存每局固定 game id、隨機 deck seed 與完整 binding，至少一次呼叫 `GameRoom.createCompetitionGame`；回應遺失、重啟、alarm 重送與 stale match／nextGame 阻擋皆有回歸。binding 只由已鎖定 roster、賽程 nextGame 與可信 lineup 派生；啟用個人牌組時完整鎖定並 exact-match，未啟用時強制空 snapshot，GameRoom ready 不落地 HTTP 附帶牌組且由 Rust 使用預組牌。主審先退修 TypeScript narrowing 與 `alarm` override，再重跑 focused 116 項／1059 assertions、完整 shared／Durable Objects 147 項／1201 assertions、Nuxt typecheck 與 `git diff --check` 全過。
- 下一切片回到 E4d3c：裁判 pause／resume 成功後，以 E4f1 持久 linkage 建立 CompetitionRoom→GameRoom 的可重試通知，同步 match revision cursor 與權威 deadline／alarm。不得讓舊 deadline 在暫停期間觸發，也不得因回應遺失重複推進 revision；CompetitionRoom 的既有 game provision retry 與後續 clock-control delivery 必須共用安全的單一 alarm 排程。
- 已指派 Luna「E4d3c－裁判暫停／恢復同步」，寫入範圍限 CompetitionRoom／GameRoom contract、DO 與各自測試。控制通知遇到一般 pending delivery 時不得擅自丟棄或 rebase；先 fail closed 並凍結新增玩家 mutation。只有能以既有 deadline 與固定 identity 證明是暫停交付空窗產生的舊 `clockObserve`，才可安全消除。
- 階段 E4d3c 已通過：pause／resume 與 match checkpoint、revision、audit、receipt、clock-control outbox 在同一 SQLite transaction 保存；CompetitionRoom 的 provision／control 兩種 outbox 共用最早期限 alarm，回應遺失與重啟重送同 payload。GameRoom 以持久 receipt 原子推進 revision cursor、deadline 與 `refereePaused`，暫停時凍結新玩家 mutation；一般 pending delivery 不丟棄／不 rebase，僅能以固定 identity 證明的舊 deadline `clockObserve` 可安全消除。主審退修 pending provision 先 pause 造成無房可送的死鎖後，新增 gate：有 linkage 時只允許目前 `nextGame` 且 status=`provisioned` 的 Active GameRoom；pending、blocked、上一局或 Waiting 房在引擎前 fail closed。主審重跑 focused 128 項／1177 assertions、完整 shared／Durable Objects 159 項／1319 assertions、Nuxt typecheck 與 `git diff --check` 全過。
- 下一切片 E4e2：裁判更正交手判定時，依目前已持久化 tournament result 決定首次提交或 `tournamentCorrectResult`，並與 match correction 在單一 CompetitionRoom transaction 保存。DoubleLoss／NeedsDecision 在沒有合法勝負前仍不得推進淘汰賽或虛構名次。
- 已指派 Luna「E4e2－裁判更正同步賽程」，寫入限 CompetitionRoom DO 與測試。若已提交的 tournament result 經更正後會讓 match 重新變成未完成，現有 tournament core 沒有 clear／reopen result；本切片先整批 fail closed，不能只保存 match correction 造成分歧。
- 階段 E4e2 已通過：裁判更正完成交手時，CompetitionRoom 依持久賽程現況選擇首次 `tournamentSubmitResult` 或 `tournamentCorrectResult`；match、tournament、公開投影、可信時間、audit 與 receipt 在同一 SQLite transaction 保存。已提交賽果若會被重新打開則整批 fail closed，DoubleLoss／NeedsDecision 不推賽程。主審退修內部賽程操作錯把裁判冒充為 organizer 的身分錯誤後，確認首次提交與更正皆保留 referee actor；重跑 focused 78 項／857 assertions、完整 shared／Durable Objects 166 項／1475 assertions、Nuxt typecheck 與 `git diff --check` 全過。
- 階段 E4f2a 已通過：已物化的 Waiting 競賽 GameRoom 在固定成員全數 connected＋ready 時，將 metadata／ready event 與 immediate alarm 同一 storage transaction 保存；低延遲自動開局失敗或物件重啟後由 alarm 重試。開局仍原子提交唯一 GameStarted、GameRecord、clockBeginPreparation 與 delivery alarm，RPC／重複 alarm 競爭保持同一 game instance；斷線與 unready 在提交前會重新檢查並阻止開局。主審重跑 focused 45 項／470 assertions、完整 shared／Durable Objects 173 項／1542 assertions、Nuxt typecheck 與 `git diff --check` 全過。
- 下一切片 E4f2b 補齊多局交手的時計生命週期：第一局仍直接開始準備；後續普通新局必須先送 `clockNewGame(reset)`，淘汰賽和局續局則先送 `clockNewGame(carryForward)`，再以同一 GameRoom FIFO 開始準備。兩步都須使用固定 identity 並依嚴格 ACK 推進 revision，不能由 GameRoom 重算剩餘時間。
- 階段 E4f2b 已通過：第一局維持單一 `clockBeginPreparation`；後續普通局以同一可信時間排入 `clockNewGame(reset)` → `clockBeginPreparation`，淘汰賽和局續局改用 `carryForward`。兩筆命令使用不同固定 identity，response loss、重啟與非法 ACK 不跳過或重排，嚴格 ACK 後 revision cursor 逐筆推進。主審重跑 focused 49 項／517 assertions、完整 shared／Durable Objects 177 項／1589 assertions、Nuxt typecheck 與 `git diff --check` 全過。
- 下一切片 E5a 建立唯一可信賽程建立入口：主辦方提供階段設定與種子順序，但 CompetitionRoom 必須重新匯出鎖定報名名單，確認 seed team set 恰好等於鎖定隊伍，並在單一 SQLite transaction 保存 tournament、projection、audit 與 receipt；既有可直接送任意 `tournamentCreate` 的內部旁路必須封鎖。
- 階段 E5a 已由主審放行：僅 active organizer 可建立賽程；以鎖定 roster 精確核對 team set 並保留 seed 順序，封鎖 direct `tournamentCreate`。同時驗證 organizer、team、member、deck 一致性，private roster 不進 projection、audit 或 receipt；嚴格驗證 Rust `Tournament::new` 初始輸出與 StageConfig 前置條件，並維持 receipt-first idempotency 和單一 SQLite transaction 原子提交。主審以正式目前 Rust WASM 驗證 2／3／5 隊 × 四賽制；focused 93 tests／1330 assertions、完整 shared／DO 185 tests／2027 assertions、Nuxt typecheck、`git diff --check` 全通過。
- 階段 E5b 已通過：新增 authenticated `POST /api/competitions/:id/tournament`；`competitionId`、`callerUserId` 與 `trustedTimeMs` 分別由 path、登入 session 與伺服器時間推導，路由只呼叫 dedicated `createTournamentFromLockedRoster` RPC。HTTP 僅執行 StageConfig DTO 結構層 shallow validation，賽程語意仍由 DO／Rust 權威判定；HTTP 成功回應以白名單重建，移除 projection 及頂層的 `private`、`serverState` 與額外欄位，並將 `engine:*` 內部代碼正規化為固定 `engineRejected`。真實 H3 handler 測試涵蓋 session、path、body、伺服器時間與 dedicated RPC；未授權或惡意輸入不 dispatch，成功輸出不洩漏內部欄位。E5b focused 17 tests／101 assertions；E5a+E5b combined 110 tests／1431 assertions；Nuxt typecheck 與 `git diff --check` 通過。完整 server 套件另受已知 `managed-server` timeout 與 Miniflare loopback `EADDRINUSE` 環境問題阻擋，非本切片 focused failure。
- 下一階段 E5c 為比賽對局啟動生命週期：從 pending schedule 啟動並物化 match／clock，再使用伺服器可信隨機指派初局先手，並由鎖定名單推導 turn order。以下規則仍未決策，不得在 E5c 擅自實作或推定：DoubleLoss 的淘汰晉級／名次；和局轉為競賽負後，下一普通局的先手；零主時間、同時進讀秒及不限時淘汰和局的處理。

### E5c 主審放行與切片範圍

- E5c1 已通過主審：`startScheduledMatch` 以 receipt-first 語意冪等啟動指定 pending 賽程交手，嚴格驗證 Rust start checkpoint，再於單一 SQLite transaction 同時保存 tournament、match／clock、global mirror、audit 與 receipt；direct start／materialize 旁路已封鎖。主審驗證 focused shared＋DO **100 tests／1477 assertions**、完整 shared＋DO **191 tests／2157 assertions**、Nuxt typecheck 與 whitespace check 全過。
- E5c2 新增唯一 server-only 首局入口：從 E5c1 的 pending-first-player 狀態，使用 Web Crypto 隨機選先手隊，透過 Rust `matchAssignFirstPlayer` 驗證指派，再由鎖定 roster member 順序推導 1v1／2v2 座次；使用既有 GameRoom provision linkage/outbox 交付固定 game id／deck seed。請求不得提供先手、隊伍、座次、binding、seed 或 game id；first-player checkpoint、全域 mirror、match revision、linkage、audit 與 receipt 必須原子提交，並保留 response-loss／重啟／alarm 重送能力。
- E5c2 不處理 DoubleLoss 淘汰晉級／名次、競賽改判 Draw 後下一普通局先手、零主時間／同時進讀秒／不限時淘汰和局，也不處理 subsequent-game captain choice、HTTP 或 UI。
- E5c2 已通過主審：唯一 server-only `provisionInitialScheduledGame` 從 E5c1 revision-1 首局 pending 交手出發；先手由 Web Crypto 隨機 bit 決定，Rust `matchAssignFirstPlayer` 回應嚴格驗證，1v1／2v2 座次只依 locked roster 的 members 順序推導。match revision 1→2、global match mirror、GameRoom linkage/outbox、audit 與 receipt 單一 SQLite transaction 提交；GameRoom strict ACK、response loss、重啟、alarm 排程失敗及 matching receipt recovery 皆有回歸。主審驗證聚焦 shared＋DO **109 tests／1678 assertions**、完整 shared＋DO **201 tests／2375 assertions**、目前 Rust/WASM 實測 1v1＋2v2、Nuxt typecheck 與 whitespace 全通過。未決規則及後續局／HTTP／UI 仍在 E5c2 外。
- E5c3 範圍：新增 authenticated `POST /api/competitions/:id/matches/:matchId/start`，以同一 root command id 穩定派生兩個子命令，依序呼叫 dedicated `startScheduledMatch` 與 `provisionInitialScheduledGame`；path、session、可信伺服器時間及 body exact schema 由 HTTP 層驗證，第二步只能使用伺服器衍生的 match revision。成功值以白名單回傳，不暴露 binding／roster／seed／projection；錯誤訊息固定且不反射 room 內部錯誤。不得涉及 shared／DO／Rust、UI 或未決賽制規則。
- E5c3 已由主審放行：authenticated match-start HTTP 以同一 root command id 穩定派生 `startScheduledMatch` 與 `provisionInitialScheduledGame` 子命令，嚴格驗證 path/session/server time/body exact schema、第二步 server-derived revision 與完整 binding；成功值白名單輸出且不洩漏 private binding、roster、seed 或 projection，固定錯誤不反射房間內容。聚焦測試 **28 tests／205 assertions**、Nuxt typecheck 與 `git diff --check` 全通過。

- E5d1 已通過主審：新增唯一 server-only `provisionAssignedScheduledGame`，request 僅含 competition/match/caller/command、expected match revision 與 trusted time；receipt-first fingerprint 排除 trusted time、保留 expected revision，較新時間的同語意重試回傳同一 persisted binding/gameId/deckSeed，語意衝突固定拒絕。僅 active persisted organizer 可呼叫；嚴格核對 competition identity、revision、trusted time、active scheduled match、checkpoint/projection/global mirror、open/null outcome 與 assigned next first player。由 locked roster 匯出推導 1v1／2v2 座次，不接受 caller lineup；pending、completed、referee、malformed checkpoint、malformed roster/deck 與 elimination continuation 尚待先手政策時均 fail closed。checkpoint history 事件數受 `row.revision >= 1 + history.length` 下限約束，最小假 history、落後 revision、合法 correction／多次 assignment 與 matchView replay 均有回歸。普通局沿用既有 `startKind` 與 GameRoom provision outbox，match revision 不推進；舊 `provisionScheduledGame` caller-lineup 旁路封鎖。主代理驗證：完整 shared+DO **211 tests／2429 assertions**、Nuxt typecheck 與 `git diff --check` 全通過；formal ordinary-draw continuation 使用指定 Rust/WASM 實測通過。
- E5e1 已通過主審：新增 authenticated 裁判 clock-control exact route，驗證 session、path、command payload 與房間回應，且僅呼叫 dedicated clock-control RPC。主審驗證 server **35 tests／311 assertions** 與 Nuxt typecheck 全通過。
- E5d2a 已通過主審：新增 `assignNextFirstPlayerByCaptain` server-only RPC；只允許前局有效 winner／timeout-loss 的敗方 locked-roster captain 選本場任一隊先手。透過 Rust/WASM `matchView` 重播 persisted history、鎖定名冊驗證隊長身分，並嚴格驗證核心只追加固定原因的 `matchAssignFirstPlayer`；單一 SQLite transaction 一起更新 match checkpoint/projection/revision、global mirror、audit 與 receipt。Draw、DoubleLoss、NeedsTiebreak、NeedsDecision、`eliminationDrawLoss`、缺失或矛盾判定與 malformed state fail closed，不建立 GameRoom。主代理驗證 focused shared+CompetitionRoom **131 tests／1803 assertions**、完整 shared+Durable Object **223 tests／2500 assertions**、Nuxt typecheck 與 `git diff --check` 全通過；Rust/WASM 實測涵蓋 1v1、2v2、timeoutLoss、DoubleLoss、NeedsTiebreak 及 `eliminationDrawLoss` 拒絕路徑。

- E5d2b 已通過主審：新增唯一 server-only `provisionCaptainAssignedScheduledGame`，僅接受敗方 active participant captain，並要求同一 match／revision 的成功 captain-assignment receipt、固定原因 audit 與目前 checkpoint 的最後 assignment 事件完全吻合；再次由 Rust/WASM `matchView` 與 locked roster 推導隊伍、先手玩家及 1v1／2v2 座次。首局不適用，GameRoom linkage、audit、receipt 由共用 `persistScheduledGameProvision` 單一交易原子寫入；交易內完整重驗 assignment receipt 欄位與 response，以及 audit actor／role／operation／match／command／revision／outcome／trusted time（精確等於 assignment row 的 `last_trusted_time_ms`）／reason。delivery 與 receipt retry 共用 outbox helper。正式 WASM 測試覆蓋 1v1／2v2、兩側隊長選擇、preconstructed／personal deck、response-loss／restart／strict ACK retry、ACL、assignment evidence tampering（含較早 audit trusted time）、match／roster 損壞、duplicate linkage、SQL rollback 與 organizer 入口維持。主代理驗證 focused shared+CompetitionRoom **141 tests／2017 assertions**、完整 shared+Durable Object **233 tests／2714 assertions**、Nuxt typecheck 與 `git diff --check` 全通過。未決淘汰和局／DoubleLoss 規則維持不實作。
- E5d2c 已通過主審：新增 authenticated `POST /api/competitions/:id/matches/:matchId/next-game`，由伺服器以同一 root command 派生隊長先手指派與後續局 provision 兩個專用 RPC；session、正規化 path、body exact schema 與單次可信伺服器時間由 HTTP 層提供，第二步 match revision 只取自第一步已驗證的公開投影。第一步公開 match projection 必須符合完整 exact 欄位集合，且拒絕 private、serverState、名冊／牌組／隊長及 locked-roster metadata。回應採白名單，不輸出子命令、projection 或 private/server 欄位；錯誤回應固定，不反射 room 內訊息。主代理驗證 utility＋next-game 真 H3 handler＋start／clock-control／tournament routes **44 tests／557 assertions**，Nuxt typecheck 與 `git diff --check` 全通過。未決先手／淘汰賽規則維持 fail closed。

- E5d3 已通過主審：`ingestGameTerminal` 僅在 Rust/WASM 接受普通局和局、賽事仍處於固定局數 open 狀態且核心已給出下一普通局 assigned first player 時，才從 locked roster 衍生 binding，將 terminal match／clock／projection 更新與下一局 game linkage、audit、terminal receipt 放進同一 SQLite transaction。非普通局、timeout、非和局、referee／淘汰待決及無有效 continuation 不自動建立下一局；原 terminal receipt 重試不重跑 engine／名冊匯出，若 linkage 仍 pending 則重試 alarm 與 strict GameRoom delivery。正式 Rust/WASM 1v1／2v2 與 personal-deck 路徑、最後一局無 game3、既有 game2 linkage 保留、非 strict ACK／alarm recovery、私密資料不洩漏、malformed roster／export 拒絕及 late SQL rollback 均通過主審。主代理重跑 focused shared＋CompetitionRoom **148 tests／2118 assertions**、完整 shared＋Durable Object **240 tests／2815 assertions**、Nuxt typecheck、`git diff --check` 與目標檔 whitespace 全通過。

### 2026-09-30 接續

- E5d1 的已通過主審狀態已復原；此前「待主審／代理自測」為舊對話覆寫造成的紀錄回退。
- E5d4 已通過主審：適用 `NeedsTiebreak` 的 pending／malformed continuation 整批 fail closed；重啟交付精確核對 binding.startKind 與權威 nextGame.kind（含 carry-forward 毫秒）；terminal success receipt 核對原 request competition／match／command／提交後 revision，舊收據不受後續 revision 推進影響。三項缺陷均有正式 WASM red→green 證據，1v1／2v2 連續和局精確扣時 5000→3766/4333→3544/4222ms、game3 決勝 1–0、單次賽程推進、無 game4 及冪等重試皆驗證。主代理重跑完整 Rust **636 tests**、release WASM build、完整 shared＋DO **250 tests／2962 assertions**、Nuxt typecheck、cargo fmt 與 git whitespace 全通過。Lagrange／Hume 本輪審核已完成並關閉。
- 預設 Web 測試載入的 `apps/web/worker/wasm/fewfc.wasm` 是 9 月 12 日產物，缺少競賽入口，導致完整 shared＋DO 207 pass／36 fail；重建目前 Rust，改以 `FEWC_COMPETITION_TEST_WASM` 指向 `target/wasm32-unknown-unknown/release/fewfc.wasm` 後，243 tests／2858 assertions 全過。Rust clock 26、match 11、review regressions 3 共 40 tests、Nuxt typecheck 與 `git diff --check` 通過。這是基線，不能替代仍待補足的 malformed continuation 與精確剩餘時間回歸。
- E6a 已指派 Luna Einstein，寫入限新增競賽建立／報名頁、其 UI 元件與 app lib／tests；只呼叫既有已放行 HTTP 契約，與 E5d4 核心及 DO 沒有共用寫入檔案。所有恢復及新代理皆已收到先讀並維護 `./tmp/tour-attempt` 的要求。
- E6a2 已交由同一 Luna Jason 接續 CompetitionRoom／其測試：主代理以未替換任何核心 operation 的正式 WASM 回歸實測，房間 revision=1 的第一筆 open 命令遭 registration revision=0 拒絕；須由可信 checkpoint 轉譯子領域 revision，維持 HTTP global revision 契約及 receipt-first 冪等，不能由 UI 固定減 1。另須修正 Rust locked roster 的 `base` 到 GameRoom binding `fewfc-base` 的既有 DTO 邊界；真正 registrationCreate→open→register→confirm→checkIn→lock→export→tournament→provision 皆須驗證，不再以手造 export 代替完整接線。此關尚未放行。
- E6a UI 切片已通過主審：pure tests **14 tests／47 assertions**、主代理全 Nuxt typecheck 通過；兩軸終審再發現未知 mutation 結果後刷新會隱藏唯一重試入口，已以 Brave red→green 核實修復。刷新後保留同一 pending command，重試的完整 body 不變；409 僅刷新 query 不重播 mutation、退出後可再登記、日期含時區及毫秒、390px 無水平 overflow、未知建立結果僅送一次且停用再次建立均通過。Brave 測試使用 HTTP mock 並隔離 component preview 不支援的通知 socket，不宣稱 E6a2 的真正後端接線已完成。
- E6b 新元件／pure helper 經主審與 **9 tests／29 assertions** 驗證，已授權 Einstein 最小整合父頁面既有 mutation/retry。只讀自己的帳號牌組並依既有 catalog 驗證、explicit emit 凍結快照，已報到隊伍重送前必須確認再次報到。整合後仍待型別與 Brave 的提交／response loss／新版 query／帳號牌組變更／相同命令重試／ACK 解鎖驗證，不宣稱完整 UI 已放行。
- E6c1 平行實作 GameRoom 的固定牌組專用準備 RPC：依賽事 binding 而非可變帳號牌組作準備，只接收固定 caller 與明確 ready 狀態；HTTP／UI 接線須待 RPC 授權、冪等、transaction／alarm／autoStart 與資料安全放行。此項依既有已通過的 E4f2 與 E5d4，不共寫 CompetitionRoom 或 app UI。
- E6c1 已交付，主代理聚焦重跑 **67 tests／686 assertions** 通過，規範／規格兩軸正在審核。獨立正式 WASM 開局回歸另重現既有 no-Pouch 的 InProgress 開局卻只交付 clockBeginPreparation，已交同一 GameRoom owner 進行 E6c0 修復；此時計初始化關卡通過前不接玩家 HTTP／UI。
- E6b 最小父頁整合主代理驗證 **24 tests／80 assertions**、全 Nuxt typecheck、Brave explicit submit／重新報到確認／commit response loss／新版 query／帳號牌組變更／完整同命令 retry／ACK 後明確 reload 及 390px 版面皆通過，兩軸終審尚待回報。刷新期間元件會卸載，不要求 ACK 後自動重讀帳號牌組，控制已實際解鎖。
- E6c1 Spec P2 已主審實測重現：raw 2v2 metadata.capacity=2 被正規化成 4 後仍接受 ready 並寫入，0 pass／1 fail／1 assertion。已交原 owner 先驗 raw shape/binding 一致性；修復前不放行。
- E6b Spec P2 已主審實測重現：catalog maximumLevelTotal="oops" 繞過前端 policy 數值比較，0 pass／1 fail／1 assertion。已交原 owner 在新 helper 增加消費欄位的 runtime guard；不修改共用驗牌演算法，也不宣稱 Rust 會保存非法牌組。修復前不放行。
- E6a2 正式集中13 tests／272 assertions 及 scratch首次open1／3已過；完整 shared＋DO **268 pass／1 fail／3359 assertions** 的唯一失敗為舊 canonical checkpoint fixture 斷言，已交 owner 更新但保留所有隱私斷言。兩軸完整現況審核與修正後集中重跑尚待完成。
- E6a2 已通過主審：Standards／Spec 皆無 actionable finding；舊 checkpoint 期待已改為完整 announcement＋commands 且保留隱私斷言。主代理完整 CompetitionRoom **159 tests／2482 assertions**、HTTP **44 tests／557 assertions**、全 Nuxt typecheck、scratch 首次 open **1／3** 通過，真實雙 DO／WebSocket／正式 WASM 的報名到準備開局串接也已 **1 test／20 assertions** 通過，+1000ms 只扣當前隊伍。該串接驗證依賴 E6c0 現況修復，E6c0 尚有正式回歸檔整理，不宣稱整個 shared＋DO／平台 E2E 已放行。
- E6b catalog 產品 P2 已主審原 scratch **1／1**、pure helpers **25／103**、Brave 正常與 malformed catalog 中文錯誤／no submission、全型別驗證 green。兩軸重核均指出新增測試 withDefinitions 回裸 array 且使用本身超限 deck，造成逐欄 guard 假覆蓋；已交 owner 只修 test builder／合法對照。此項是測試缺口，非另一個產品漏洞。
- E6d1a 已交同一 Luna Jason：僅 Rust competition Web API、shared competition DTO、JSON bridge 與相應測試新增唯讀 tournamentView，不共写其他代理的 GameRoom／UI。query 使用已驗證的 Tournament snapshot，不推進 state／重排賽程、不從最後 global projection 假稱當前賽程；橋接放行後才接 DO 專用 query／HTTP／管理 UI。Cargo／WASM 建置仍由主代理單槽集中。
- E6b 已通過主審：兩軸限定終審皆確認產品 P2 與 fixture 假覆蓋已關閉；合法完整 composition、未使用 definition 突變與成功對照皆已核對。主代理 helpers＋原 malformed catalog scratch **26 tests／105 assertions**、全 Nuxt typecheck 及既有 Brave 提交／固定快照重試／ACK 解鎖／malformed no-emit 驗證通過。未宣稱本機 Worker 平台驗收完成。
- E6c0／E6c1 已通過主審：兩軸無新 actionable finding，raw metadata-before-normalize 與 no-Pouch 行動／合法 Pouch 準備分流通過。主代理集中競賽 7 檔 **251 tests／3281 assertions**、開局與 raw capacity scratch **2／5**、真正兩 DO／WS／WASM flow **1／20**、env-unset wrapper **1／1**、Nuxt typecheck 與 whitespace 通過。更廣 shared＋DO＋bridge **278 pass／1 fail／3401 assertions** 的唯一失敗是正在實作的新 tournamentView bridge tracer，並非上述切片回歸；query 完成後仍須重跑全套。
- E6c2a 已交同一 Luna Maxwell：寫入改限 shared competition-room DTO／tests、CompetitionRoom／tests，建立固定參賽者專用唯讀 readParticipantGame；現有 organizer-only 含私密 binding 的 readProvisionedGame 不降權。只輸出目前可加入的 provisioned gameId／gameNumber 或 null，核對 active player 與固定名單、目前權威 nextGame／linkage，查詢不得寫入、交付或扣時。HTTP／房間 UI 待此投影放行；Jason 的 Rust／shared competition／bridge 與 Einstein 的 app lib 不共寫。
- 本機 Worker 驗收準備已在全新 tmp/tour-attempt/worker-review.6JFsf3 完成本機 D1 10 個 migration，不碰既有 .wrangler 資料。Nuxt build 兩次在 generated competition-registration SSR chunk 的 shared utils relative import 解析失敗；已交原 Luna Einstein 僅修 app lib import 接線，不擴改全域 Nuxt 設定／shared policy。此阻斷與單元／型別 green 分開記錄，未部署。

- E6d1a 已通過主審：Rust 新 query 五例與完整 **641 tests**、正常 release WASM 建置與 bundled 更新、shared／bridge **15／70**、正式 export smoke **1／8**、完整 shared＋DO＋bridge **285／3450**、全 Nuxt typecheck、cargo fmt 與兩軸 0 actionable finding 均通過。查詢完整已驗證賽程且不變更 checkpoint；DO／HTTP 相依實作仍須遵守 CompetitionRoom 的 Maxwell 互斥所有權。
- E6c3 已交 Jason 平行實作專用 competition-ready HTTP／helper：只接受明確 ready boolean、登入身分衍生 actor、嚴格安全回傳，不重用會讀帳號牌組並 toggle 的舊 ready。寫入只限新增 server helper／games route／tests，依 E6c0／E6c1 已過，不與 E6c2a 或 UI 共寫；尚未放行。
- Nuxt 打包定位補充：無 dev 的原相對 import 仍失敗；暫時 #shared 可 build 但正常 Bun 無法載入，不可採用。只加 .ts 副檔名的第五次 build 仍在相同 SSR import 失敗，已交 owner 依 resolver 證據尋找最小修復；全型別 green 不等同 production build green。
- E6c2a 已交付並立即審核，主審重新跑 shared／CompetitionRoom **181／2646** 通過，但獨立雙 DO／正式 WASM scratch 重現普通非終局勝負後 awaitingFirstPlayer 查詢被錯拒為 invalidMatchCheckpoint（**1 pass／1 fail／43 assertions**）。已交原 owner 修復並補正式 pending／NeedsDecision 回歸；兩軸持續審核，修正前不接相依 HTTP／UI。
- E6a-build owner 已讀實際 generated chunk／source map 與 installed Nitro resolver，確認 relative external 解析失敗機制。已授權唯一最小 config 擴充：package.json 的兩個精確 #shared utils imports mapping，加 source 對應 imports，不改 Nuxt 全域 config／tsconfig／runner／依賴或 lockfile；仍待正常 Bun 與集中 production build／typecheck，未宣稱修復。
- E6c2a 已通過主審：合法初局與普通續局 pending、正式 DoubleLoss 與 NeedsDecision query、currently provisioned/continuation/null、固定成員 ACL／隱私／唯讀均經兩軸終審確認。主代理原雙 DO／WS／WASM scratch **2／44**、owned **184／2662**、全 shared＋DO＋bridge **295／3553** 通過。最新 typecheck 僅剩平行 E6c3 的四項 JSON test 型別錯，未把它宣稱為全型別 green 或歸入 E6c2a 缺陷；修正後仍集中重跑。
- E6d1b 已交同一 Luna Maxwell：沿 E6d1a 純核心 query 建立 dedicated readTournament DO/RPC、exact request 與 current-safe public projection；只寫同一 CompetitionRoom／shared contract／tests。不開 HTTP，不降 ACL、不擴 global mutation allowlist，不共寫 Jason server helper／Einstein app。通過後才接賽程 GET／管理 UI。
- E6a-build 最小 imports 修復已經正常 Bun **25／104**、穩定 production Nuxt build exit0 與 bounded 兩軸 0 findings 驗證；最新型別剩餘錯誤屬 E6c3 的 JSON fixture，不宣稱全專案 typecheck 過。Einstein 接續互斥 E6c2b：session-derived 固定參賽者 participant-game GET／專用 safe helper，依 E6c2a 已過；不動 UI、shared、DO 或既有 server helper。
- E6c3 完整交付後主代理 focused **45／583** 與全 Nuxt typecheck 已通過，兩軸開始審核；未以先前 WIP build 代替最終 HTTP 放行。Jason 接獨立 E6c4 安全競賽房模式標記，依已過 E4a／E6c0／E6c1：只在 shared game-room 公共 response 與 GameRoom／tests 派生最小 competition context，不動 metadata/canonical event，不與 CompetitionRoom／server／UI 共寫；已交付 ready source 保持凍結。
- 真本機 Worker／D1／SQLite DO／正式 bundled Rust＋隔離 Brave 的建立→open→兩隊登記／確認／報到→主辦 refresh→全域8及 caller-bound private query 全流程已過，無 HTTP／engine mock；賽事 ID 2bc80c2b-c2db-47e9-a706-6dcd2618c94f 存於新的 local-only worker-review.6JFsf3。只驗這段報名流程，不宣稱房间／牌組／整場賽程 UI 平台全完成。
- E6c3 原生首次準備 HTTP502 已重現並確認根因：平台 RPC object own keys 額外帶同步 `Symbol.dispose`，不是 DO 業務欄位。只於 transport 邊界辨識／釋放該鍵，仍 exact 拒 private／unknown strings／symbols；native ready/retry200、forged400、outsider403。最終無 TEMP source 主審 **57／830**、獨立 tracer **1／3**、全 Nuxt typecheck、第八次 production build 均通過；完整原生超時流程及最終兩軸仍待驗。
- 原生 harness 的 async waitForFunction 在此環境以永遠 false predicate 仍 **48ms resolved=false**，不能作 Finished／completed 的完成證據。已改真 HTTP 有界輪詢，撤回先前 early-return 推論；沒有為腳本錯誤放寬產品 ACL 或結果判定。
- E6c2b source 主審 **13／168**、修後全 Nuxt typecheck 通過；兩軸正在審核，participant GET 本身尚未 native 驗證，不能借 ready RPC 的平台觀測宣稱已放行。
- E6d1b 主審 owned **192／2941**、Standards0；Spec 發現 DE／Swiss 缺正式 finished＋restart 前綴測試，已交 owner 補齊，未證實 query 產品缺陷。補測與終審通過前不啟動相依 HTTP／管理 UI。
- E6c3／E6c2b 已通過最終主審：ready修後兩軸0、participant GET兩軸0皆完整讀；final無TEMP build/type與真native harness **91300 exit0**，固定成員 GET／ACL、準備及重試、真alarm主時間＋讀秒判負、terminal跨DO、validcommand409永久凍結、canonical recordSequence不變、完成後game=null及earlyreplay403皆過。超時不合成canonical Finished，之前腳本期待已修正；尚未實作時計顯示或賽後公開。
- Jason 已解除 E6c4 source hold，接續安全competition context與metadata唯讀red；Einstein接互斥E6c5a固定玩家目前房間入口 UI（新helper/component＋原競賽頁），只依已過participant GET，不接未審E6c4／準備 UI。提供可重用matchId props供後續賽程card沿用，不把客戶端ID當授權。
- E6d1b 補 DE總決賽／Swiss五隊三輪實際完成、finished query＋restart全表alarm不變回歸後，主審 owned **193／3043** 通過；query產品未變，兩軸正重核缺測關閉，尚未放行相依HTTP。
- E6d1b 補測兩軸終審皆0且已完整讀，原coverage P2關閉；DO/RPC唯讀切片放行。Maxwell 接互斥E6d1c目前賽程 GET／新safe helper＋tests，只依專用readTournament、不fallback舊globalprojection，不改DO/shared/Rust/UI。HTTP通過才啟動賽程／管理介面。
- E6c5a owner已交付，主審 lib **4／22**、單檔component **5／5**、Nuxt typecheck通過。Spec P2確認locked頁未排notCheckedIn，已交回同Luna要求checkedIn與正式eligibility回歸；null完整文案同步收斂為中性，原可能性文案符合最初授權（主審摘要落差已記錄），不冒作原需求違反。修後兩軸/native UI前不放行。
- E6c4 owner完成freeze／自測 **73／3094**，已修合法／損壞binding唯讀前metadata寫回red，保留普通房與Waiting alarm不可開局收尾；新context從validatedbinding派生，覆蓋1v1/2v2與個人/預組牌組的fetch、WS、重連、restart、各viewer私密隔離。主審集中測試與兩軸正在進行，不借舊native結果放行。
- E6c4 主審 focused **73／3094**、完整 shared＋DO＋bridge **310／6335** 通過；Spec完整現況0已讀，Standards待回報。E6c5a 修後主審 lib **5／33**、單檔component **5／5** 通過，Spec bounded終審0已讀；兩項仍須最新集中type/build及修後native驗證。
- E6d1c 主審 focused **12／156** 通過，但最新全Nuxt typecheck exit2：新增測試的DE round.kind array[index]推成string或undefined，不能符合JsonValue。已交原Luna最小修fixture，不以cast／放寬契約或忽略matcher掩蓋；此輪不宣稱全型別green。
- E6c4／E6c5a／E6d1c 已通過最終主審：修後兩軸皆0且報告完整讀，最新Nuxt typecheck exit0、第九production build exit0；D1c focused重跑 **12／160**。真local-only Worker／D1／SQLite DO／bundled Rust／Brave harness **17148 exit0**（賽事75019ea9-b9cc-49f7-931d-cbd3b49d869e）走實際賽事頁查當局→進房、HTTP與WS最小competition context；新GET notReady409、兩位固定玩家200、outsider403、forgedquery400、重查revision不變及實際alarm完賽後current finished賽程均過。不借舊91300證明修後功能，尚不宣稱準備按鈕、時計顯示或整場辦賽管理完成。
- Einstein 接 E6c5b 競賽房準備操作介面，只改app新lib/component、useGameRoom的context接線與房頁bounded分支；依已過C4/C3，不與後端共寫。普通房仍走原流程，競賽unknown retry保留同desired boolean，模式不由URL造、不改canonical timeout Finished。
- Jason 接互斥 E6e1 唯讀時計顯示Rust／WASM契約：嚴格checkpoint，可信query取樣，在獨立clone使用既有core計算顯示剩餘時間，原serverState不變；不把虛擬超時／進讀秒順序提交為競賽判定。尚未放行，不啟動DO／HTTP／倒數相依。
- Maxwell 接互斥 E7a1 裁判更正HTTP，只寫新route/helper/tests，依已過E4e2的matchCorrectRuling；actor/refereeId與可信時間由server衍生，最小safe ACK，權限、冲突及downstream failclosed由權威DO/Rust判定。不實作未定雙敗名次、forfeit或rematch。
- 2026-10-01 E6b補真平台驗收：native personal harness **33052 exit0**，賽事214b8d4a-07b3-4fac-8867-d46161aa1b04；五獨立身份、兩隊2v2，四位本人實際UI submitDeck後各改合法帳號牌組，鎖定及開房交付仍保存原名稱／cards並經GameRoom ACK，四人原生進房／WS connected、無observer。檢查限定本機CompetitionRoom SELECT且rows_written=0，不讀受限GameRoom KV、不新增HTTPprivate欄位；未啟動本局，不宣稱已驗canonical抽牌或新準備按鈕。
- E7a1交付後主審focused **11／110** green；獨立H3 scratch **0／1／1** 重現actual `engine:scheduleError` 被當unknown502而非safe409。Rust／DO固定typedcode已實讀，不以internal subreason假造新碼或parse私密message；已交原Luna窄修map並補正式fixture，修後主審／兩軸/native前不放行。
- 2026-10-01 E7a1 scheduleError修後主審原scratch **1／2**、formal **11／129** green；Standards新增rawmalformedJSON須fixedsafeenvelope P2與集中type的四處ownedfixture型別red，均交同owner最小修，尚未gatepass。
- E6e1 集中Rust focused **33＋30**、全Rust **653** green；release及bundledWASM同步新query，真WASM/shared/bridge **22／171**（零skip），八檔shared＋DO＋bridge **302／6357** green。完整snapshot兩軸尚待審，未先啟動DO/HTTP/倒數。
- E6c5b owner交付後main pure **10／43**、Nuxt component＋composable共 **10／10** green；普通房控制隔離／準備狀態及同值重試兩軸及原生UI仍待驗。新全type阻於E7a1 testfixture窄修，未冒用前輪type/build成功。
- E7a1上述兩項P2及test型別窄修後main **12／137**、原scratch **1／2** green；全type及第十productionbuild exit0，兩軸boundedreport皆0。native仍待本機驗收，尚未gatepass。
- E6e1 Spec新增positive main＋zero action的exactdeadline等待旗標P2：main獨立正式WASM **0／1／1**真red，合法prefix100ms回awaitingAuthorityfalse。已交Jasondisplay-only窄修與formalcase，不改核心判負或zero政策；全組green不能替代本回歸，DO/HTTP/countdown仍不啟動。
- E6e1 P2修後正式放行：兩軸bounded全文已讀／0，mainfocused **35＋31**、全Rust **656**、freshrelease／bundledWASM同sha ，formalshared/bridge23＋獨立scratch1共 **24／242**零skip、八檔shared/DO/bridge **303／6426**、全Nuxttype97372 exit0。原scratch partial-array matcher另有harness錯誤，改強化完整two-team strict DTO後green；原awaitfalse產品red及fixture錯分開記。只放行core purequery，不宣稱DO/HTTP/countdown已完成。
- 相依 E6e2a 改派熟悉core及CompetitionRoom的Luna Jason：僅shared room contract／CompetitionRoom DO／tests建立權威唯讀clock RPC，與Maxwell正在修的新裁判HTTP/helper互斥；不改Rust/bridge/GameRoom/UI。HTTP及倒數等此RPC審核通過再啟動。
- E6c5b reconnect P2修後main正式Nuxt＋原獨立tracer三檔 **13／13** green、type97372 exit0；Spec bounded0，Standards及新穩定bundle/native仍待。第十bundle實際以Brave驗證準備、取消準備、再準備，以及 B 已準備的狀態；ordinary controls 隱藏及原生超時，但不是修後reconnect native證據。
- E7a1平台未放行：main原生55900到首筆主辦（非actingreferee）更正應403實503；獨立class-likenamespace/stub的真helper/H3第二case亦expected403 actual503（整組1pass/1fail/3 assertions）。capability不可套plainJSONDTOprototypeguard是強候選，交同Maxwell窄修/補formalcase，保持body/root/nested exact guard；新native重驗前仍hold。
- E6c5b 正式放行：兩軸bounded全文0＋修後main Nuxt三檔 **13／13**、type22586/build70042 exit0；第十一穩定artifact真Brave/native31587 exit0，兩位固定玩家UI送exact true/false/true/Btrue、權威ready確認／autoStart、ordinarycontrols隔離／local返回，並保留nativealarm與canonical凍結契約。同房response-loss/重連意圖回歸由真正Nuxt案例驗證，不冒稱此native另注入網路失敗。
- E7a1能力窄修 main **13／147**、原scratch **2／4**、第十一type/build exit0；原生31587 exit0（賽事0cda643d-06f0-4f88-9a79-b0e55f188404），org/player/outsider非actingreferee403、forged400、不合法timeout勝者更正422、assignedreferee合法同判定更正200、同commandretry不加revision/correction、證據／canonical保存皆過。原native503症狀已清；不是直接取得nativeprototype或所有影響下輪更正的完整平台前綴。Spec bounded0，Standards最末capability窄修終核仍待，未先宣告E7gatepass。
- E7a1 正式放行：最末 Standards capability 窄修報告完整已讀，兩軸完整現況及 bounded 修後皆為 0；配合上述 main formal／scratch／type／build 與原生修後證據，裁判更正專用 HTTP 關卡通過。其他裁判 UI、棄權／重賽與賽後公開不因此視為完成。
- 下一階段互斥 Luna 已啟動：Jason E6e2a 權威唯讀時計 DO／RPC；Einstein E6d2a 四賽制目前賽程／排名唯讀 UI；Maxwell 僅盤點整場賽事結束與重播釋出的既有邊界。時計 HTTP／倒數仍等待 E6e2a，管理 mutation UI 仍等待唯讀 UI 放行；不重複啟動尚在實作的工作。
- 新 E4d3c-r1 回歸缺陷：main 真 WASM 正常 pause／resume 都成功，但 production bridge closed operation 名單漏列兩名，獨立 tracer **0／2／5** 拋 operation invalid。Maxwell 已接只 bridge／test 的窄修；裁判暫停／恢復的網站平台能力重新列為待修後驗證，不以舊 mock／核心驗證放行。時計 query 與賽程 UI 可獨立續作。
- 整場收尾／賽後公開盤點與[Limitless 官方研究](competition-event-finish-reference.md)已讀。既有 stage finished 不等於 event closed／replay released；已非阻塞詢問使用者是否採「全部預定階段完成後主辦顯式結束、結束後暫凍結賽果」。未收到選擇前不擅定閉賽／發布後更正政策；其餘已批准實作持續進行。
- 2026-10-01 使用者新增規格已記錄但尚未實作／驗收：參賽型態用「個人／團體」；生命由既有規則衍生、不供自訂；報名期限分鐘精度；「報到期限」改為「比賽開始時間」；每輪提前五分鐘就緒房間供報到。現有 roster lock 等 checkInDeadline 才能執行，不能直接滿足首輪提前開房；需獨立跨層切片，先收斂鎖定／輪次時間契約。既有 E6a／E6a2 驗證保留為歷史，不視為新規格驗收。

- E6e2a 主審讀完兩軸完整現況：Standards 0，但 Spec 發現 stored `nextTimeoutDeadlineMs` 可與 Rust checkpoint 矛盾而 query 仍接受的 P2。Jason 已接正式 WASM 單欄儲存 fault 重現／修正；不得另抄 TS 時計算法或以 observe 作唯讀 query。CLOCK HTTP／倒數仍 hold，原 34／666 及 broader green 不覆蓋此 finding。
- E4d3c-r1 真 bridge 修後 main **20／270** green，原 operation-invalid red 已關閉；完整平台仍未過。native **31158 exit1** 重現合法 assigned-ref pause 已在 DO 提交（revision+1、pausedWindow action），HTTP 卻因正常非 null matchProjection 被 helper 拒為502。Maxwell 接互斥 r2 helper／formal H3 修正；Jason 接 Waiting 必須在 engine 前 failclosed 的靜態 P2 重現，後者尚非實測 red。
- E6d2a main pure **8／68** green，但真正 Nuxt **18027 exit1，11 pass／1 fail**：切換 session 後新 query pending 時 refresh-button 尚未 disabled（正式 test:310）。已交原 Einstein 區分元件／測試排程根因，保持 context 清除、abort 與舊 finally 不可解鎖新請求的要求；兩軸與修後集中驗證前不放行。
- E6d2a disabled／drawCredit 窄修後兩軸 bounded 全文0已讀，main Nuxt **62481 exit0，12／12**。全type **38453 exit2** 僅 Jason query nullable 與 Maxwell r2 H3 test 型別錯，已分別退修；新type/build/native前 UI gate仍hold。Einstein 可接互斥 E6a3d 個人／團體用詞、分鐘報名輸入／呈現與比賽開始時間標籤；不改報名wire／名單流程，不移除HP必填直至新核心／createHTTP契約放行，不把部分文案修正當提前開房能力。
- E6e2a 期限／Waiting 已由 owner 正式 WASM 真實重現 **0／3／16**：期限5000/null矛盾仍query成功；Waiting pause外層實為`engine:clockError`並新增failed receipt/audit（未跑HTTP，不能冒稱422）。批准純 fieldless `clockCheckpointMetadata`／exact單欄deadline，Jason接Rust/JSON/shared/bridge及原DO互斥所有權；main正式Cargo tracer **53302**已取得unknown variant真red。Luna Lagrange接獨立GameRoom唯讀身份bound eligibility query，通過再由Jason接preenginecheck；不可用clockidle代替Active、不可形成雙DO serialized等待循環。
- E4d3c-r2 ownerfreeze後 main正式H3/helper **35／547** green；最新type **66203 exit2**只餘ownedtest disposer receiver matcher窄型別錯，已交Maxwell。與Hume平行的Lagrange bounded Standards插入終審，GameRoomprobe WIP暫存不rollback；修後type/build/native前裁判網站平台仍hold。
- E4d3c-r2 HTTP 正式放行：兩軸終審完整已讀0、窄型別修後type95200／50583 exit0、fullHTTP **88／1287**、正常第十二build13012 exit0。fresh release/bundled同sha 、真WASM bridge再跑 **20／270**。native **46640 exit0** 真五身份assignedpause最小ACK／samecommand冪等／跨舊期限11秒仍停表／resume及真正alarm／canonical不變通過，原DOcommit／HTTP502已清。此放行只限r2 adapter及Active路徑，Waiting pre-engine P2／r3 probe仍open，不稱完整控制鏈完成。
- E6d2a 唯讀賽程UI正式放行：兩軸完整及修後bounded0／main pure8/68、Nuxt原12cases修後green與最新3檔20/20、type/build上述green；同46640真RR目前交手／排名／兩位玩家readonly手動refresh、390px document/card geometry、完賽後current顯示通過。其他三format由formalcomponent案例驗證，不冒稱全部native完賽或管理mutation UI已完成。
- E6a3d 個人／團體與分鐘第一段：兩軸0已讀、pure17/70／最新真Nuxt8case及type/build過，同46640個人公告分鐘／本地時區／開始label真native已確認，HP和五分鐘開房仍未完成。main另找到deadline逆序錯誤與舊lock說明仍用報到截止，交Einstein忠實舊流程的詞彙窄修／正式badDates測試；這不是先前native已驗過的分支。
- E6a3a 純規則設定 Rust foundation 正式放行：兩軸 bounded 報告全文已讀、Standards hard／smell 0、Spec 0；main focused55946 **7／7**，完整 Cargo70472 及 compact81569 重跑皆 exit0，合計 **668 tests、零 filtered／skip**。重用實際等大兩隊座次與 OfficialRules 配置衍生唯一生命值，exact 四項設定不接受 caller HP，其他 checkpoint／raw shape 不變。只放行 Rust query；TS／DO 建立入口、HTTP 與移除生命輸入尚待相依接線，網站仍限 teamSize1／2，不宣稱最大合法 u16 或 3v3 網站能力。
- E6e2a metadata TS／DO ownerfreeze 後 main34600 **36／1125** green，包含真期限4100對 stored5000／null矛盾拒絕及全 DB／alarm 不變；Waiting known red 明確排除。八檔獨立兩軸正在審核，尚未放行 clock HTTP／倒數；不能用 focused green 宣稱 Waiting 控制已修。
- E6a3d 舊詞窄修 main44557 Nuxt三檔 **22／22** green（terminology10＋原12），補 deadline逆序不 dispatch 與現行 post-start lock 文案。未改比較式、時間 wire、HP 或鎖定政策；先前 native46640 不包含這兩個新分支。
- E6e2a 權威唯讀時計 RPC 正式放行：八檔兩軸 bounded 完整報告已讀0，hash與ownerfreeze一致；main36／1125、fresh HP＋metadata release/bundled 下 shared／bridge31／722、全type82342 exit0。stored期限P2關閉，未接HTTP／倒數、未宣稱query原生GET或Waiting控制已過。Jason接互斥無callerHP的規則建立RPC／typed bridge；Maxwell先規劃新create HTTP，等待RPCgate才實作。
- E4d3c-r3 probe 尚未放行：兩軸完整報告已讀，Standards0，Spec新P2指合法pause receipt rev3但cursor單欄回2、或clock entry內外matchId矛盾仍回true。這是可證的內部一致性缺口，尚屬靜態結論；交Lagrange先正式fault tracer取真red，再只補probe guards，不擴改既有delivery pipeline。Jason pre-engine相依仍hold。
- E6e2b 相依時計 GET 已交同一 Luna Maxwell，在HP新RPC等待期間平行：只新專用safe helper／match clock GET與各自tests，不與Jason shared／DO或Einstein UI共寫。只session/path/server單樣本、單次readMatchClock，exact6顯示DTO及root disposer；HTTP／倒數不以舊核心green冒整體完成。
- 最新人類覆寫：不需要舊賽事紀錄相容性、生命值沿用規則不需測試。停止新的HP專項tests／數值矩陣／兩軸放行前置與相容分支設計；歷史已完成驗證保留紀錄，不再當必要工作。FromRules入口已完成的source直接供正常create接線，不為取消相容再重命名／搬legacyfixtures，不加入舊initializer fallback或相容承諾。其他時計／排程／授權與一般型別驗證繼續；各owner已通知，不刪現有dirty／已完成tests。
- 新建立流程接線互斥：Jason 負責新 `competition-create-from-rules.ts` 與建立 POST；Einstein 移除生命輸入／建立 payload 已交付 freeze，只送八欄 announcement draft。Maxwell 只寫時計 GET 四檔，不再共寫建立 HTTP。這是正常功能接線，不新增或執行生命專項測試；本輪最新一般 type／build 尚未集中重跑。
- E4d3c-r3 修後 main77926 **10 tests／430 assertions** 通過。Hume bounded Spec 報告全文已讀，receipt revision 超前 cursor 與 entry 內外 matchId 兩個 P2 分支均關閉、新增 finding0；合法 public-start／ACK／pause 前綴與全 KV／alarm／socket／engine／remote 無副作用斷言已核。Einstein 正進行兩個 guard 的 bounded Standards 終核；這不是 native primitive／雙 DO 互鎖驗收，也尚未放行 Jason 的 Waiting pre-engine 接線。
- E4d3c-r3 probe source 關卡放行：Einstein bounded Standards 追加全文已讀，documented hard／12-smell 均0，修後四 hash 與凍結現況吻合，兩軸皆0。可接 Jason 的 current-game pre-engine strict boolean guard；目前先維持集中 type／build source hold，尚未實作 caller，不消除原 Waiting receipt／audit red，也不宣稱 native probe 全矩陣已驗。
- 新正常接線集中型別 main99522 exit2：僅兩個 owner 檔案出錯，bridge 的 operation union 尚未 discriminator 縮窄、clock GET test 的 headers union 帶 optional undefined。已交 Jason／Maxwell 各自最小窄修，沒有生命算法／相容測試；建立 helper 與 UI 本輪無型別診斷。時計 HTTP main94674 **19 tests／496 assertions** 通過，兩軸仍審核，不以 focused green 代替一般 type／build／native GET。
- 普通型別窄修已集中 green：bridge validator 改為 unknown runtime boundary＋明確 type guards，不放寬 DTO；GET headers 矩陣明確 Record、原三 cases 保留。main47033 Nuxt typecheck exit0，clock修後獨立 focused 再次 **19／496**；Hume Spec 完整全文已讀0（修後四 hash），Standards 尚待。正常 build 進行中且所有來源維持 hold，沒有新增或執行生命專項測試。
- 正常 build main9196 exit0（release WASM＋Nuxt client／server／Nitro完整結束），新建立 helper／表單与時計 GET 均可打包。依人類覆寫不另加生命 gate，舊紀錄相容不在工作清單。無 native 服務運作時，已解除 Jason 的 CompetitionRoom source／test hold，接 E4d3c-r4 Waiting pre-engine strict boolean gate；只跑時計 focused，不跑生命或全套測試，不與 Lagrange GameRoom共寫。
- E6e2b 時計 GET source 關卡放行：修後四檔完整兩軸報告全文已讀0、main修後19／496、一般type47033與build9196過，release／bundled hash 同9f836207…。原生 GET 尚待 r4 source freeze後一起驗，未宣稱已驗 native。穩定契約允許同 Luna Einstein 平行接 E6e3a 新時計 helper／component／component test及房間頁最小整合；只單次權威取樣顯示、可見時 single-flight GET，不在瀏覽器另算扣時／讀秒切換／判負，不與 DO或server共寫。UI仍待獨立審核與集中驗證。
- E4d3c-r4 ownerfreeze後 main53045 聚焦實跑 **54 tests／1042 assertions** exit0，173明確filtered、無選中生命專項。原 Waiting pause／resume、非primitive true／缺capability、ACL／版號／時間、成功receipt-first restart與既有控制 outbox／query回歸均過；注入SQL failure是保留的rollback案例，不是未完成失敗。Maxwell Standards／Hume Spec完整bounded審核已啟動，尚不放行整鏈native，UI來源仍實作中，未重跑當前type／build。
- E4d3c-r4 caller source 關卡放行：Maxwell Standards（hard／12-smell0）與 Hume Spec0完整報告全文均已讀，兩 source/test hash審前後符合 freeze678a8080…／73d33749…。配合main54／1042，原 Waiting engine前拒絕及receipt/audit副作用缺陷在正式核心＋平台fixture seam已清。沒有由fixture推定真雙DO互鎖／primitive／native整鏈已過；當前一般type/build仍待clockUI sourcefreeze後集中，提前五分鐘完整排程尚未實作。
- E6e3a 時計 UI 首次main59265 pure **11／117** 過，但真Nuxt18659 exit1 **15pass／4fail**：room/context/session切換後fetch次數預期3實2、晚error/finally預期2實1。已交原Einstein定位元件或nextTick合併排程的fixture同步差異，禁止skip／降低abort／舊finally不得解鎖／立即disabled斷言；source兩軸仍審核，不以puregreen放行UI，也未開始目前全type/build/native。無生命專項測試。
- E6e3a fixture排程窄修：保留立即abort／clear／disabled，僅等待queued nextTick派送才數原exact GET，原19cases／舊success error finally／防重全部保留。main42186同正式Nuxt **19／19** exit0，新specsha ab04e9fe…；其餘四檔不變，未改產品排程來迎合fixture。全type目前集中進行、兩軸仍審核；noHPspecial／未claimnative。
- E6e3a 正常型別 main25766 exit2僅puretest windows tag widening，owner用type-only imports/contextual typed矩陣與strict expected ClockDisplay窄修，未降cases；元件label改主時間。新snapshot main pure **11／117**、Nuxt61305 **19／19**、全type55482 exit0，包含r4backend；當前正常build83053進行中，兩軸需新puretest0b7c…/component4ff…修後終核。原生GET／UI尚未驗證，不以型別與fixturegreen視為整鏈放行。
- build83053已exit0，但E6e3a暫不放行：Spec修後0完整已讀，Standards完整報告1P2指clock cards/window直接以隊伍UUID作玩家標籤。main已核web AGENTS全文及正常registerTeam以crypto.randomUUID產ID，接受finding並收回之前允許rawteamID的代理指導（不是人類例外）。交Einstein僅以已驗teams順序統一甲隊／乙隊呈現cards/preparation/action/choice/paused，保ID內部關聯、不猜本隊對手／隊名、不添private查詢或DTO，19cases與所有race要求保持。修後type/build/native待重驗；noHPspecial。
- E6e3a 中性標籤修後 main17940 原正式 **19／19** 通過（含合法UUID／card與所有window一致映射／原race斷言）；全type31474 exit0。component7233f954…／specb5265859…freeze，其餘三檔未改、沿既有pure11／117證據不重跑；兩軸窄終核與本次正常build仍進行，native尚未執行，沒有生命專項測試。
- E6e3a source關卡通過：Maxwell Standards窄追加全文已讀，原UUID P2 source關閉、hard／12smell0；Hume Spec窄追加全文已讀0，新五hash一致。配合19／19、type31474、正常build32261 exit0及release/bundled9f836207…一致，允許真Worker／Brave驗收，未提前宣稱native已過。所有產品freeze，計時／Waiting guard與新GET同鏈驗收中；Hume只平行primary-source輪次政策調研。
- 真native main40683 actualexit1：Waiting pause/resume409且完整投影不變、時計GET exact6／ACL、兩隊中文主時間／甲乙／390px、準備操作與 HTTP／WS context 均已通過，但正式Active/running action後裁判pause仍409 gameRoomNotProvisioned，沒有成功ACK。暫停／恢復、超時完整鏈與paused/completed時計UI尚未通過；r3／r4 source/fixture證據保留但依賴native gate HOLD。localWrangler48332已退出0，禁止source/artifact熱換；Lagrange僅GameRoom source/test診斷formal red→minimalfix，Jason只readonly RPC/caller分析，不與其他產品共寫。不得用mock true或目前post-finally timeout推定當時拒絕原因。
- 原生RPC根因已隔離確認：main188931真Miniflare/workerd同compatdate，直接method／Reflect.apply各true，method.call實際TypeError「RPC receiver does not implement method call」。故r4 `.call`不是可靠原receiver呼叫，而是RPC代理遠端屬性鏈，catch後錯拒Active；不因這點放寬r3資料驗證。改交Jason2檔formalred→Reflect.apply窄修，Lagrange停止非根因產品修復、維持r3freeze。main後續focused／兩軸／typebuild／originalnative皆須重核，未宣稱已修好。
- r4 native caller窄修凍結bb04215a…／84268fa0…：owner完整診斷及formal red0/1/2→green1/17log全文已讀、source與新增test實讀。main78475相同clockfocused **55tests／1059assertions exit0**，173明確filtered／noHPspecial，原54所有拒絕／receipt／strictACK與新callable案例保留。Maxwell Standards與Lagrange Spec只bounded窄核新seam；集中type開始，原native尚待重驗，沒有放寬GameRoom資格或更動timepolicy。
- r4新caller type62807／build85824皆exit0，Lagrange極窄Spec報告全文已讀0、sha一致；Standards尚待，不提前放整鏈。原真Worker／Braveharness50571重驗中。每輪時間primary研究已保存competition-round-timing-reference.md/main完整已讀並再查官方三頁；外部不提供本站五分鐘延誤的統一政策，已向人類新async確認人工重新公布或系統自動順延，不默認任一答案，也不因此停時計獨立驗收。
- r4／時計 GET＋UI 原生關卡正式放行：Maxwell新Standards完整追加全文已讀hard／12smell0、Lagrange Spec0，修後兩sha與UI／r3／release/bundled全部freeze一致。main50571原完整真Worker／Braveharnessactualexit0，comp c663f81b-aff8-48d0-a4fa-fc11184b7cd9：Waiting雙control拒絕完整投影不變、GET exact6／ACL／readonly、甲乙與主時間／390px、Activepause／resume及同receipt重試、暫停11秒超過舊8+2deadline仍凍結、恢復真alarmtimeout／跨DOterminal與canonicalrecord不變、paused/completed UI、目前賽程與更正HTTP/證據/ACL/receipt／earlyreplay403全部通過。這是RR1v1單局真鏈，不宣稱native所有format／2v2／restart全矩陣、五分鐘排程或管理UI完成；Worker20117已退出0、本機資料保留，noHPspecial。
- 穩定改判HTTP有本次native再次佐證，允許同Einstein獨占新app裁判更正helper/component及tests、[id].vue最小mount。最小選項為同局已保存判定及與canonical一致gameAccepted（draw僅fixedGames），不新造歷史讀秒/加賽時間/DoubleLoss名次或下游政策。GET指派只作顯示提示而非activeACL承諾；已unknown命令不能靠新GET/後續4xx解決，exact receipt ACK／以相同 body ID 重試。此片開始實作，未審/驗收；共用排程時間仍等新policy選擇，不停独立UI。
- 人類回覆輪次時間問題：前輪未結束卻開始下輪不應發生，屬賽程問題。依此把前輪完成／配對確定／全輪提前五分鐘房間就緒列為硬性開賽前置，不靠時間觸發越過前序，不默認人工或自動順延已獲選。未滿足就 fail closed、不回填 ACK／猜勝者／靜默改時間；共同 binding／release／失效／初始交付協議仍須閉合後才啟用，不以正常時程假設放寬。舊政策提問已收到回覆，後續不重複詢問同一問題；獨立裁判 UI 繼續。
- 裁判自隊邊界唯讀盤點已完成並全文讀；main核對start／execute與正常locked fixture後接受1項靜態缺口：conflictedTeam=null只靠主辦annotation，未衍生權威members。尚非actual red／原生漏洞或已修復，現只授權Jason獨占DO test新增正式合法前綴聚焦重現，產品仍freeze；不以participant flag禁全賽裁判、不改r4 primitive/native既有結論、不混HP／兼容／排程。新UI資格僅提示、server權威仍必須核實。
- 自隊裁判正式 tracer 已 owner red 並由 main94691獨立重現 **0pass／1fail／10assertions、228filtered**：真registration→lock→tournament→grant member→current provision→有限clock action，null conflict的自身裁判pause成功、1次WASM mutation/1次控制交付且全DB/alarm快照改變。平台primitive/ACK是清楚限定adapter，這是parent/WASM actualred，不冒作nativeGameRoom證據。已授權Jason僅DO source/test唯一新mutation守衛修復，三裁判操作從權威兩隊/locked members核自己，保成功receipt-first、跨交手合法ref與非captain；產品未修前此hard-policy gate HOLD，UI獨立實作不中斷。noHP/兼容/全suite。

### 後續實作邊界（不是已完成清單）

2026-10-03 接續：自隊裁判 source/test 已凍結 ；主審實讀新增 guard／14-case matrix、owner log與獨立兩軸（Spec 0；Standards hard／12-smell 0），獨立重跑正式 WASM clock/r4/self-match focused **66／960、exit0、176 filtered**。最新一般 type73387／build38045 均 exit0；原生新 guard 仍待，平台 adapter 不視為真 GameRoom Active，hard-policy gate 尚未放行。

裁判更正 UI 五檔已交付 freeze，主審 pure **14／144** 通過；真正 Nuxt 單檔 main65978 **19 pass／1 fail／20 cases**，父頁 fresh wrapper.vm 身分比較失敗。installed VTU 每 wrapper 產生新 Proxy 已直接核實；原 Einstein 僅窄修 fixture，保留公共 DOM 身分／未知命令 snapshot／exact 原 retry，兩軸與修後重驗仍待。main15925 額外 `--` 誤選全 component 範圍的 101／102 結果只記為命令失敗，不當聚焦 green；未新增／執行生命專項測試。

第一 fixture 窄修 `8bc9ee…` 的 main8881 仍 **19／20、exit1**；DOM／unknown 保留斷言通過，接著失敗於父頁初始化 registrationView 也會耗 UUID，導致首命令 literal 偏移。已實讀呼叫鏈，原 owner 只調整父頁 case 的提交／重試測量區間，驗新命令三 UUID、重試零新 UUID及 exact 同 body；不是產品修復或減少 cases。Lagrange UI Standards 此 snapshot 0 已全文讀；最終新 spec／Spec及集中重驗仍待。

最新父頁 fixture main64014 正確單檔 **20／20、exit0**。但 UI 未放行：Maxwell Spec新增1 P2指 timeout evidence與saved loser跨欄矛盾未拒；main自有公開 DTO scratch actual **1 pass／1 fail／2 assertions**，正常control過、單欄wrong loser仍接受。已交原Einstein僅helper/puretest/log closed關係窄修，禁止前端重算時計／比分／勝者；兩軸窄終核、新一般type/build/native仍待，保原20cases及未知receipt要求。

timeout evidence 修後 helper ／pure test ，其餘三 UI 檔維持凍結。主審獨立重跑正式 pure 與原未改預期的 scratch **17／159、exit0**，最新 Nuxt main75709 **20／20、exit0**。兩軸窄終核與新一般 type/build/native 尚待，未以測試數字提前放行。

一般 type85412 的兩處 test literal widening 已由原 owner 只加既有 `CompetitionRuling` annotation 修正，test 新 freeze ；main fresh pure＋原 scratch **17／159、exit0**、type73387 **exit0**、正常 build38045 **exit0**。guard 修後兩軸均 0，Spec 的 test annotation 窄終核亦 0；Standards 最末 annotation 窄核待回。原生新 script 在 bootstrap 因 CLI sandbox 無 `URL` 停止，未到產品流程；原 owner 依實際診斷修精確 endpoint／輪詢 adapter 後重跑，不能以語法 green 或舊 native 放行本片。

原生 main33292 已實際走過 self pause/resume/correct-ruling 三次 forbidden／全 public match 不变、獨立裁判操作／成功 receipt 重試、真 alarm timeout 及 canonical/evidence 保留。更正 UI readonly lookup 通過，keyboard 段因 harness 未包含「已保存判定 N：」的真正 option label 停止；這是已核 source 的 fixture 錯，不是產品錯。新 script 已精確修 prefix／同局 current ordinal、保留焦點斷言、nodecheck0，main53708 正重跑；尚未宣稱 UI POST／ACK／390px 全部通過。

最新兩軸最末 annotation 窄核皆 0，產品七檔保持原 freeze。main53708 後續實際 exit1 在精確選局 locator；隔離真 DOM probe證 `getByLabel exact` 無匹配、同名 `combobox exact` 可匹配，前述 prefix 是另個來源確證的 latent fixture錯，不能當已證runtime首因。原owner僅換兩個角色locator，新script 的真 native42421 已通過選局、替代選項完整文案及鍵盤焦點，但仍 exit1 於 POST／ACK／refresh 廣義段，正增加固定子stage定位；UI 原生關卡仍不放行、不擅改產品或削斷言。

固定子stage新版 的 main23671 真 native 已確認按下按鈕後會送出 POST，exit1定位於exact-body compound guard；目前細分原欄位及只輸出布林的差異判別，不先假設是產品或key-order問題。裁判介面仍HOLD。與其無相依的設定／種子／建立片可獨立進行：主審全文讀 Einstein160行方案，另直接讀固定Bracket modal及activation正式test（晉級隊伍assertion被註解，不視為已證成）；只採確認／權威重讀流程，不翻譯外部碼／加依賴。已批准Einstein四個新management helper／component／tests檔，禁止修改既有頁面／shared／server／DO，不mount、不使用中央編譯／測試槽；回合外設定與 Swiss 未定欄位都必須填寫，開始時間依賴另待契約。此派工不代表更正native或新管理UI已放行。

自隊裁判後端防護切片獨立放行：兩檔freeze ／、正式66／960、fresh一般type／build與兩軸0，真RR1v1原生多次已走完三個self403／全public match不變、獨立positive／receipt重試、真alarm timeout／evidence／canonical保留。2v2全成員及other-match正例來自formal，不冒稱全賽制／DB／native矩陣；更正UI仍在後續POST比較診斷，兩個gate不可混用。本機驗收服務／自有瀏覽器已正常關閉，持久測試資料保留，沒有commit／push／部署。中央槽回主審，代理僅在明確互斥檔案寫實作／測試，不能自行編譯或測試。

### 2026-10-05 接續驗證

共同排程純契約兩檔 ／ 通過限定關卡：主審全文核 §15 的 exact DTO／expected 身分／完整 內容比較 定義，獨立重跑 `round-release` **14 tests／443 assertions、exit0**（既有14項未執行），Standards hard／smell 0、Spec 0。required T、五分鐘 cutoff、首局 evidence guard 只證結構，不冒作真 GameStarted、撤銷或全輪就緒證據；完整 binding 內容比較／父子 producer-consumer／持久化接線仍未實作。

管理 A 四新檔 ／／／ 通過未掛載元件關卡。修前 main75605 的19／20跨帳號摘要真失敗保留；原 owner 僅修 summary scope 並增加跨賽事回歸，未知 intent／body／expectedRevision／UUID 不清除。主審修後正確單檔 main73557 **21／21、exit0**，pure **12／110、exit0**；最新兩軸完整六檔版本 對齊，管理 A Standards hard／smell 0、Spec 0。一般 type5907／正常 build14471 均 exit0，release／bundled WASM 版本一致。這只允許後續最小父頁掛載，不代表管理介面 native、開交手／下一局、輪次排程或整場競賽完成。

裁判更正 UI 本次原生關卡通過。main79553 真失敗定位到 ruling 比較，安全三布林為 rawStringEqual=false／closedTimeoutLossFieldsEqual=true／keyOrderEqual=false，確認只因 JSON 物件鍵插入順序；產品五檔完全未動。主審只窄修 native 腳本該 timeoutLoss 比較，仍要求雙端 exact type／loser、相同分支與字串 loser；其餘全域 same、POST7／ACK5、UUID／request identity／順序／audit 斷言不改。擷取實際函式的 scratch 先 **0／3 red**，修後 **3／20、exit0**；原真 Worker／Brave 完整流程 main87091／7fe03f **exit0**：exact 單次 UI POST、matching ACK 後 GET、持久改判歷史、原 canonical／terminal evidence、鍵盤焦點、安全繁中文案、390px 幾何皆通過，並再次核自隊 forbidden／獨立裁判及真 alarm。限定 RR1v1 已保存同值判定，不聲稱四賽制／2v2／重啟 native 全矩陣。腳本新 freeze ，自有 Brave69e498與Worker86825退出0，保留本機資料，沒有 commit／push／部署。

已依上述關卡啟動管理 A 父頁最小掛載：Einstein 唯一擁有 `[id].vue` 及新 `CompetitionTournamentManagementParent.nuxt.spec.ts`，原四個管理檔與其他產品仍凍結。穩定 sibling 掛載、readonly registration refresh、unknown intent 保留是本片要求，不新增開交手／下一局／時間政策；Maxwell 僅在 tmp 另寫原生管理驗收 harness。Lagrange 僅先收斂 required 完整 binding 下一切片方案，未授權同步改寫父／子 DO。

後續 §16 方向已由主審核實：完整 binding required17、完整 request 的唯一 fingerprint helper、同步擷取 snapshot 後才 await；Lagrange 先寫 tmp staging patch，不直接改 shared 產品。父頁完整 freeze、兩軸／集中驗證與管理 A 真原生關卡通過且服務關閉後，才授權正式 shared 切換，再按互斥 ownership 接父／子 producer-consumer；目前不是 Timing workflow PASS。主審另成功全文取得同 pinned Bracket rounds.py／rescheduling_test.py，僅借 plan→apply、不導入或翻譯外部實作，預計 end_time 不當全輪完成／房間 ACK 證據。

管理 A 父頁／真原生關卡已通過：parent ／new spec ，原A4不變。首次新Nuxt的2項fixture失敗已窄修；Spec另P3父mutation互斥缺測以不同賽事A locked unknown→B合法open補公開DOM／HTTP正反對照，原12cases逐位元保留。最新三正式檔 main15018／d6fc00 **54／54、exit0**、一般type78889 **exit0**，產品仍同正常build6192／release-bundled，兩軸終核0，來源未變。真Worker／Brave main19600／a16984 **exit0**：設定摘要／keyboard／單次POST3及Stage8／两UUID／matchingACK5後雙權威refresh／相同managedDOM／非主辦停用／繁中privacy與390px皆過。native前兩次只因CLI VM與Playwright JSON realm不同造成harness exact4誤拒，真安全probe確認wire同值／欄位／descriptor均正確；主審僅窄修native nativeObject prototype判別，原closedkeys／所有驗收要求保留，自有actualexact scratch3／18及nodecheck0。只RR1v1單階段建立，不當開房／全賽制native／五分鐘或整場辦賽完成。自有Brave32193／Worker43759退出0，資料保留。

上述管理關卡後，正式shared下一切片已交Lagrange唯一兩檔實作：required17、完整strict validation/equality及唯一完整fingerprint helper。staging patch已主審全文讀、readonly apply-check0；尚不表示shared新tests或Timing通過。父／子DO與HTTP仍等shared正式freeze／審核後互斥授權，未同步時不部署、不做optional／legacy回退。

required17 shared 限定關卡現已通過：兩正式檔 ／，主審全文讀批准diff及actual修改seams，readonly reverse-check0、whitespace0；集中 `round-release` **29 tests／1066 assertions、exit0**（其他14項未執行），沿Nuxt strict基底、既有Bun／Node型別的隔離兩檔檢查exit0。首次只缺測試型別路徑，修檢查環境而非產品；原28cases保留、新增15，沒有HP專項或舊紀錄相容。獨立Standards hard／smell0、Spec0，完整report已核，來源未變。此關卡只放行完整資料／內容比較，不證current manifest／實體全房ACK／ACL／GameStarted；最新wholeproject type／build及Timing仍HOLD，不能部署半套。

主審依此啟動互斥相依實作：Jason唯一父 `shared/competition-room`及父DO／tests，Lagrange唯一子DO及competition／start-clock tests／fixture，Maxwell唯一既有HTTP helper／tests與start route test。完整binding兩shared檔保持freeze；先收斂父首次開局交付exact介面，再讓子FIFO接入，禁止各自猜wire。子release／revoke已定caps可先獨立實作；父需完成持久整輪manifest／實際全部ACK與cutoff／到T release／Waiting撤銷共同更正／fresh主辦公告；HTTP此片只同步required17驗證、不啟用新公告route或回假已start。Einstein作獨立審核准备／合法延遲clock anchor證據盤點，產品不改。所有執行仍由主審集中，未freeze／審核不得放行下游HTTP或UI。

首輪仍有已核實相依缺口：`competition_registration.rs:842`的Lock只接受開始期限後，detail頁也仍顯示此限制。不能直接把同一開始時間當父manifest的T又宣稱提前五分鐘全房就緒；首輪公告wire／提前名單與牌組凍結須另接專用流程。當前父切片可先處理真locked roster→主辦設定 future T，但不因此放行首輪全流程、不假取可信now、不靜默延T。此缺口不阻止互斥父／子／HTTP實作進行，也不准任一owner越界改核心或前端。

HTTP required17 三檔通過限定關卡：freeze ／／，集中daf319 **40 tests／418 assertions、exit0**（原32＋新8）。唯一完整shared validator先驗raw binding，再核linkage；公開八欄及root平台RPC cleanup範圍保留，route source不改。獨立Standards hard／heuristic0、Spec0，主審全文讀兩report及實際整合seam／cases，來源未變、直接搜尋無trailing空白／conflictmarker。這只證HTTP資料接線與mock RPC/H3邊界，不證native、首輪公告／release或提前開房；父／子source仍在互斥實作，wholeproject型別／建置等同步freeze後才驗，新公告HTTP／UI等待專用父RPC關卡。

父共同 wire 小checkpoint ／已交付；中央b641be **7 tests／108 assertions、exit0**（既有15項filtered），同Nuxt strict基底限定型別0。主審已全文讀兩檔及owner checkpoint；Standards／Spec正分開審核，還不能批准child import或稱父runtime／Timing完成。Jason保共同wire凍結、續唯一父DO實作；Lagrange另獲最小shape-only admission parser export兩shared互斥授權，避免首次命令以received ID自比冒授權，該delta仍須獨立凍結驗證。

Admission shape-only delta ／已freeze；central2cf6e9 **32／1155、exit0**（原29＋新3、14filtered）、isolatedstrict8cc7e5 exit0。main唯讀確認移除新增區段後與原凍結內容相同，所有舊內容保留。新入口只轉接唯一parser，不改expected身分guard／required17／digest；兩軸終核仍待，不由pure32宣稱child/runtimePASS。父wire Standards0已全文核，Spec仍待；首輪freeze的primary研究由Maxwell平行進行、無產品授權。

Admission 最小delta兩軸完整report現已主審全文核、來源未變，Standards hard／heuristic0、Spec0，限定gate PASS；Dirac本片審完正常close、無資料删除，Lagrange可消費唯一shape入口但不得當授權本身。父wire Spec提出公告重試原snapshot與mutable current狀態的1P2，正在核純wire與持久receipt責任邊界；不直接当未實作runtime red，父wire/childinitial依賴仍HOLD。

父公告成功值已收斂成第一次semantic receipt的原snapshot，當前mutable狀態由read取得；不要求pureguard知道未給的原manifest/generation。main46261e公開guard純DTO tracer **1pass／5fail／14assertions、exit1**，接受currentversion/ACK/status的缺口已重現（非runtime/native/歷史receipt漏洞）。已只授權Jason最小共同guard/正式cases與其必要的一個announceRound auditliteral/serializationcase，其他wire/initial九欄不變；修後兩軸/central green前仍hold initialimport，父DO publication有界實作繼續。

首輪freeze研究已全文核：[primary參考](competition-opening-roster-freeze-reference.md)明確分離參賽資料凍結與真正入房/ready。主審另查Limitless官方四頁，不照搬外部自動排除/早開局/改T。現只非阻塞詢問人類「何時凍結」與「未完成資料隊伍處置」；未回覆不當建議已批准。此首輪跨層slice待決，其他父/子已批准範圍持續，不重派既有核心/HP/舊相容工作。

人類現已選定：**報名截止時一併凍結參賽資料**；未完成成員確認、必要牌組提交或隊長資料完成確認的隊伍**不納入賽程，並說明排除原因**。隊長確認與房間連線／準備仍分開；不能以截止後仍可修改或等待原比賽開始時間才鎖定的舊邏輯替代。Maxwell先補研究及互斥切片規劃，未授權越界寫入Rust或前端；父／子已批准工作继续。

子房間bounded檢查點第一次集中驗證未通過：逐案輸出有3pass／13fail，其中10項逾時；runner未正常結束，由主審停止並取得exit130，**不當作完整正常總結**。隔離「未release、玩家準備」正式案例58c446 actual0pass／1fail／2assertions、exit1，沒有逾時。正式normalizer與正式raw snapshot guard的自有診斷771018 actual1／5、exit0，確認ready流程新增own `gameInstanceId: undefined`，下一次讀取會被原strict guard拒絕；不能因此放寬guard、假設其他逾時同一原因或放行initial交付。修正仍由唯一child owner負責。

父共同wire退修限定關卡現已通過：新freeze ／，formal16df2f **9／138、exit0**、原red tracer另e0c970 **6／19、exit0**、isolatedstrict1c3aae **exit0**。完整兩軸報告主審全文核：Standards hard／heuristic0、Spec原P2已閉且無open finding、來源未變。只放行共同interface及 truthful `announceRound` auditliteral，父runtime／childlocal／Timing未通過；child先退修local，再接唯一初局FIFO。Bun pending拒絕matcher在解barrier前先阻塞的自有有界diagnostic55f24a actual0／1、exit1，證明另一測試死鎖原因；不以此推其他案例已綠或改產品競態協議。

子房間兩根因限定修正freeze ／（正式clockfixture ／wrapper 不變）：中央1ca558 **16／222、exit0，其他70項filtered**。競賽metadata改validated owned clone，普通房normalizer與strictguard不動；三競態case普通Promise capture後真entered／revoke／unblock，再驗settled，保70＋16原案例及own-undefined正反control。Standards／Spec全文審正在進行；首局FIFO仍HOLD，不部署。

父P0a publication／ledger／純read檢查點freeze ／：中央c967c2以原正式WASM **22／214、exit0，其他242项filtered**。正常報名Lock／tournament前綴、全輪Pending物化計畫、atomic保存／receipt／audit及corruption讀取拒絕有證據；平台只阻dispatch，沒有child ACK、release或GameStarted證據。此片兩軸全文審中，下一步claims／實際確認／初局S-A／更正仍HOLD；Rust首輪freeze另由互斥owner實作，舊WASM結果不當新凍結語意已過。

| 項目 | 尚缺的接線／能力 | 放行相依 |
| --- | --- | --- |
| 固定玩家進房／準備 | 入口、context、專用 HTTP、準備 UI／同 desired 重試及時計整合已過；保留後續排程與整場流程驗收 | E6c5b／E6c4／E6c3／E6e3a 已過 |
| 計時顯示 | 權威查詢、房間時計 UI 與暫停／恢復目前 RR 1v1 原生鏈已過；後續只驗新排程／多階段整合，不重做已完成切片，瀏覽器仍不得判負 | E6e2b／E6e3a／E4d3c-r4 已過 |
| 賽程／排名管理 | 目前完整賽程GET、四賽制單階段設定／種子／建立介面已過（native限定RR1v1）；尚缺輪次開始時間／開交手與下一局選先手管理UI | E6d1c／管理A已過；時間管理依新父子協議 |
| 多階段／自動開賽 | 目前核心只建立單一 stage；後續 stage 生命週期、階段獨立時計、最早開賽時間與依賽程自動串接仍需實作 | 賽程共用模型／查詢關卡 |
| 裁判與賽後公開 | 改判 HTTP／更正 UI 的目前 RR1v1 同值原生链已過；尚缺棄權／重賽、暫停受影響後續賽程及賽事結束後公開重播 | 權威裁判契約／房間 metadata，未定雙敗名次不可猜 |
| 全流程驗收 | Nuxt 打包、報名／牌組／玩家入口、準備 UI、時計、裁判暫停／恢復、更正 UI及單階段賽程建立的已記錄原生鏈已過；新的輪次時間／開交手與下一局、多階段／整場賽後公開仍須真 Worker／Brave 驗收，不把 mock 當完成 | 各切片審核後集中驗證；不部署、不新增或執行生命值專項測試 |

上述共用契約不可分給多位同時修改；新的 HTTP／UI 只有在其專用 RPC 及投影放行後才啟動。天梯仍不在本次範圍。
- 後續尚需：交手操作介面、每輪開始時間／提前五分鐘開房與開局門檻、多階段自動串接、棄權／重賽及受影響賽程處置、整場結束與賽後重播發布，以及新增功能的原生全流程驗收。已完成的管理A、更正UI、玩家入口、準備、時計與個人牌組原生切片不重複派工；不維護舊賽事相容性，不新增或執行生命值專項測試。各項先確認既有契約與相依條件，再派互斥切片。

### 2026-10-08 元件優先接續

使用者要求繼續剩餘功能，**先只做元件測試，全部功能完成後才總測試**。各 owner 先編寫純模組／有限元件案例，仍由主審中央執行；WASM 整合、跨模組／全套 Cargo、全專案型別與打包、Worker／Brave／端到端驗收延後。歷史已過的16／22案例不無故重跑，新 版本 不沿用舊綠當成新結果。生命值專項、舊紀錄相容、commit／push／部署仍不在範圍。

已恢復原六位代理且各派工成功；第一個直接 send 到尚未恢復的 Maxwell 回 not found，該批未發送，改 resume 後才重新完整派工。不得把 Oct5 額度錯誤當作本輪仍無法工作的證據。

兩軸完整現況審核報告已到件並由主審全文核：

- parent P0a Standards hard／heuristic 0；Spec 有1個P2：成功公告 receipt 只驗 shape／request，未與原 header／完整 manifest 身分交叉核對；另一合法 generation 或房間集合缺漏仍可回成功。主審直接核848–880及 manifest helper確認靜態缺口，尚未執行重現。已退回原 owner Jason，要求原 command 的不可變持久關聯、完整房間集合比對及不讀 mutable current 回填的元件回歸；shared wire 不因此解除 freeze。
- child local Spec actionable 0；Standards 有1個P3：新路徑 double cast 掩蓋自訂完整 storage／原生 transaction 能力不符。主審直接核111–119與 installed transaction 型別確認；未見 nested transaction、未觀測 runtime red。已退回原 owner Lagrange只窄修最小能力與型別 bypass，不改 guard／wire／運行政策；初局 FIFO 維持 HOLD。

上述修正尚未終核，不因歷史集中16／22綠而放行相依。Maxwell另續作首輪 Rust 凍結切片，尚未交付最終 freeze；核心元件證據及新公共 `exclusionReasons` 同步審核仍待，未完成 TS parser／UI 不能宣稱整條首輪功能已完成。Linnaeus開始有界多階段 primary 研究，只写研究與自有 tmp，不猜並列晉級／淘汰 DoubleLoss 名次，不另建配對權威。

詳細本輪失敗路徑與起始 版本 記於 `tmp/tour-attempt/main-competition-2026-10-08.md`；所有其他 dirty 保留。

首輪 Rust R1 限定關卡已通過：source ／lifecycle ，中央 `031f54` **7模組tests／0fail／166filtered、exit0**，不執行 lifecycle integration／HP案例／WASM。主審全文核 Einstein Standards0（report ）及 Dirac Spec0（），起末產品 版本 吻合；required公開 `exclusionReasons` 與三個closed camelCase literal、receipt／截止／Lock／重確認／故障rollback有元件證據。已按互斥 ownership交 Maxwell同步 TS exact公共parser及ownTeam映射／前端文案、資料期限與Nuxt元件案例；Rust兩檔凍結，不因前端待接聲稱正常首輪全流程完成。

child P3型別窄修 ／其餘來源未變，主審核全部精確型別delta／原生能力，Einstein bounded closure documented hard／heuristic0，原Linnaeus Spec0仍適用未變的運行語意。原16不重跑或升格成修後整合驗證。已授權原owner Lagrange在父P2 Standards有限終核後，沿既有已過的shared初局wire接唯一initialFIFO／strict9 ACK／持久cursor／remote-outside-mutator；禁止越界改parent/shared／Rust，component mocks先行，不放行wholeTiming／native。

parent P2窄修元件亦已通過：source ／test ，shared兩檔不變。中央 `9af36b` **18pass／0fail／264filtered／51expect、exit0**；保存層receipt-only fixture經公開DO測試，engine/random／foreign一呼就throw，不呼WASM／子房或跨模組報名前綴。原command required private manifest關聯與generation／完整rooms一致、missing/corrupt拒絕、retired＋newcurrent仍恢復原snapshot有證據。兩軸bounded終核仍待，不以18元件綠直接放行dispatcher／release／receiver或重跑旧22。

parent P2的兩軸有限終核現已到件：Standards 0、Spec 原P2關閉且0，主審全文核，來源未變，限定P0a gate PASS。Jason續唯一parent4檔claims／實際Waiting ACK／父C cutoff／durable release／singlealarm；初局S-A receiver及共同撤銷仍未放行。新輪次HTTP adapter由Mencius只寫新helper與兩條GET／POST及各自tests，不碰凍結shared／parent／child或Maxwell頁面；契約只接受path selector、sessionactor及exact公告輸入，不讓HTTP client自證全房就緒或GameStarted。此派工不代表整條Timing完成。

多階段primary研究92行 已主審全文核。Descartes只擁有 `src/tournament.rs` 的純資格解析元件：Finished winner，或RR／Swiss真正且唯一的standing rank，映射主辦明列seed；不建立階段／改schema／猜淘汰名次、並列或遞補。不與parent、HTTP、frontend共享寫入；整場收尾及多階段持久串接仍待後續契約。

### 早期退修紀錄（歷史，不代表目前放行狀態）

- Helmholtz 當時修復中的現況：集中重跑 `competition_review_regressions` 已 1 項通過；當時仍待修復者交付及完整不變式重審，後續放行見上方紀錄。
- Harvey 已交付。主代理重跑 `competition_web_api` 5 項通過；Lagrange 正進行橋接授權、公私投影與錯誤契約審核，尚未放行 TS 接線。
- Laplace 已交付但未通過。主代理編譯 `competition_match_lifecycle` 得到 4 個錯誤：TeamId Display、DoubleLoss 建構語法、重播時借用衝突。已交回修復。另指出固定局數重播錯用整份 games.len() 提前結束，以及最後一局和局忽略累積比分；要求測試修正。Hume 正審核規格。
- 舊核心審核意見需核實：合法超時判負優先並不等於「僅進入讀秒就覆蓋正常勝負」，不可將兩者混同；任意改寫完整伺服器快照不可能只靠結構驗證查出真歷史，但內部矛盾、超出額度与衍生狀態不一致應驗證。typed 裁判狀態仍是橋接審核項目。

1. Rust 網站橋接通過後，才建立對應 TypeScript 契約與 WASM 呼叫接線。不要另寫一份配對或計時演算法。
2. 交手編排與橋接均通過後，再整合可信伺服器的交手入口、持久化與登入授權；資料儲存與 API 契約集中由同一負責者維護。
3. 房間接線需依賴已驗證的時鐘恢復、權威時間、結果提交與隊伍名單。房間 metadata、觀戰限制和重播發布狀態是共用契約，不拆給多位同時改寫。
4. UI 可在公開／私人投影與 HTTP 契約穩定後獨立實作。賽事結束前的觀戰與公開重播限制必須在伺服器驗證，不能只靠隱藏按鈕。

2026-10-08 後續政策訪談已確認雙負兩隊退出本階段（雙敗亦不入敗部）、真正決賽雙負冠軍空缺、既有空位的輪空／空缺傳播，以及後續階段資格不足的收尾；以[競賽設計文件](../competitive-play-design.md)為準，不再把這些規則當成未定事項。不得因此虛構交手勝局、對局紀錄或未確認的其他淘汰名次。整份共識摘要待使用者最後確認，新政策的產品實作及元件驗證尚未放行；歷史測試與代理完成通知不代替新關卡。與本次新政策無關的已批准工作仍保留原互斥範圍。
