# 賽事完賽與名次空缺判定：架構研究

日期：2026-11-13  
範圍：評估深化 Tournament 完賽與名次空缺判定時，現有 Rust 賽程 module 能否承接 Durable Object（DO）的語意檢查、哪些發布防線必須保留，以及 UI 如何區分「已確定空缺」與「尚未解決」。本文記錄研究、三種 interface 比較與已選定方案；使用者已於 2026-11-13 確認 Q1–Q4，採用方案 A 作為後續深化方向。本次確認設計方向，未修改產品程式或新增 issue／ADR。

## 結論摘要

**事實：** Rust 已有排名重算、已完成階段勝者解析、取消／無交手勝者表示，以及 `rankings_resolved()`；但 `rankings_resolved()` 不是完賽證據。它複製 Tournament、重算 standings，然後只確認至少有一階段、每階段 `Finished`，且循環／瑞士沒有未裁決的正式同分群組。淘汰階段的合法空缺、每個 rank 空洞與原始 checkpoint 的名次一致性，不是這個布林值所證明的內容。[`src/tournament.rs:1126`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1126)

**推論：** 單用此入口取代 `isCompletionCandidateStage`／`isClosedEliminationVacancy` 會失去對原始狀態的防毀損驗證。`ranking_projection()` 會以賽果重建名次；若 checkpoint 內名次已被竄改或不一致，投影可能把差異修正掉，而 `rankings_resolved()` 仍回傳 `true`。Rust snapshot deserializer 目前驗證結構、結果形狀與部分一致性，但沒有普遍驗證已結束階段的所有交手均已關閉，也沒有先證明儲存名次與重算名次一致。[`src/tournament.rs:1571`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1571) [`src/tournament.rs:1993`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1993) [`src/tournament.rs:3303`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:3303)

**已確認方案 A（Q4）：** Rust 一次評估同一份 Tournament，提供各階段結清、隊伍名次狀態及具體空缺原因。Web adapter 只把 UI 所需事實向外傳遞；DO 保留公告計畫、重播來源、鎖定牌組及並行狀態檢查。UI 對確定未取得名次的隊伍留白，對仍待賽果或裁決者顯示「排名未確定」，並保留具體空缺說明。下文比較其他 interface 及遷移成本；此方案符合已確認產品政策。[`docs/tournament/competitive-play-design.md:61`](/Users/bendwarn/Documents/fewfc/docs/tournament/competitive-play-design.md:61) [`docs/tournament/competitive-play-design.md:133`](/Users/bendwarn/Documents/fewfc/docs/tournament/competitive-play-design.md:133)

## 已確認產品契約

- **事實：** Tournament Completion 要求每個已公告 Stage 已完成或明確取消、所有 Match 結果收齊、沒有未決 adjudication；Stage 完成或房間關閉都不等於整場結束。[`CONTEXT.md:21`](/Users/bendwarn/Documents/fewfc/CONTEXT.md:21) [`docs/tournament/competitive-play-design.md:133`](/Users/bendwarn/Documents/fewfc/docs/tournament/competitive-play-design.md:133)
- **事實：** Stage Cancellation 是明確的無勝者終局。資格為零隊時記錄「無合格參賽者，階段取消」；一隊時直接成為階段勝者，不建立假比賽。未決賽果或裁判案件不能被當成零隊，也不能默默略過公告過的 Stage。[`CONTEXT.md:56`](/Users/bendwarn/Documents/fewfc/CONTEXT.md:56) [`docs/tournament/competitive-play-design.md:75`](/Users/bendwarn/Documents/fewfc/docs/tournament/competitive-play-design.md:75)
- **事實：** Tournament Placement Vacancy 是已確定無合格隊伍取得的位置，不是等待賽果／判定的空值。名次不遞補、不壓縮；保留真實已決名次。真正決賽雙負留下 Champion Vacancy，不得捏造或並列冠軍。[`CONTEXT.md:97`](/Users/bendwarn/Documents/fewfc/CONTEXT.md:97) [`docs/tournament/competitive-play-design.md:61`](/Users/bendwarn/Documents/fewfc/docs/tournament/competitive-play-design.md:61)
- **事實：** 循環／瑞士全項相同時，主辦或指定裁判的公開裁決前，名次與相依資格仍未決，不能自動開始下一階段或宣布整場完成。[`docs/tournament/competitive-play-design.md:83`](/Users/bendwarn/Documents/fewfc/docs/tournament/competitive-play-design.md:83)
- **事實：** ADR0039 要求保留 Game Conclusion，Competition Ruling 分開記錄；Double Loss 不造勝者。ADR0032 要求按獨立 claim 保留窄測試，刪除任何測試前須為其每個唯一 claim 指定替代證據。[`docs/adr/0039-separate-competition-rulings-from-game-conclusions.md:5`](/Users/bendwarn/Documents/fewfc/docs/adr/0039-separate-competition-rulings-from-game-conclusions.md:5) [`docs/adr/0032-model-and-test-rule-interactions-explicitly.md:5`](/Users/bendwarn/Documents/fewfc/docs/adr/0032-model-and-test-rule-interactions-explicitly.md:5)

## Rust 能力與缺口

### 現有語意入口

- **事實：** `StageResolution` 明確表示 `WinnerWithoutMatch` 或 `Cancelled(NoQualifiedParticipants)`。`winner()` 在取消時回傳 `None`；資格解析只有在 `has_explicit_vacant_winner()` 判為明確空缺時，才把沒有勝者視為空來源，否則回報 `WinnerNotDetermined`。這是區分「無勝者」與「尚不能確定勝者」的現成契約。[`src/tournament.rs:254`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:254) [`src/tournament.rs:1208`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1208) [`src/tournament.rs:1907`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1907)
- **事實：** `has_explicit_vacant_winner()` 可辨識已取消零隊階段、單敗最後晉級路徑全雙負、雙敗無冠軍等情況；單敗檢查還要求終局、無排程錯誤、各輪交手關閉及最後一輪標準賽皆雙負。[`src/tournament.rs:1937`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1937)
- **事實：** `refresh_standings()` 依賽果算 standings；單敗按 bracket exit 路徑算名次，雙敗依敗部深度復原位置；雙敗冠軍沿空位晉級時可將第二名留空。循環／瑞士全項相同且未裁決時則留空 rank。[`src/tournament.rs:3303`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:3303) [`src/tournament.rs:3433`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:3433)
- **事實：** `Stage::deserialize` 會呼叫 `validate_snapshot()`。驗證包含 config 與 teams 一致、resolution 形狀、Match 的 stage/round/kind、唯一 ID 與 sequence、參賽者、完成結果格式、取消原因，以及裁決紀錄格式；Tournament 層另拒絕空 stage 清單及重複 stage ID。[`src/tournament.rs:1571`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1571) [`src/tournament.rs:1993`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1993)

### 為何現有入口不夠

- **事實：** `rankings_resolved()` 先 `clone()`，呼叫 `refresh_rankings()`，然後只檢查所有 Stage 為 `Finished` 且 `unresolved_ranking_tie_groups()` 為空。後者只為循環／瑞士產生正式同分群組；因此 `true` 不代表淘汰賽的每個空名次均已由有效雙負退出路徑證明。[`src/tournament.rs:1126`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1126) [`src/tournament.rs:1733`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1733)
- **事實：** `validate_snapshot()` 對每個 Match 驗證狀態與結果欄位，但可見驗證段落沒有「若 Stage.status 是 Finished，則全部 rounds 的 Match 必須 closed」的普遍條件；它也沒有拿重算 standings 與 checkpoint standings 比對。已完成結果的資料正確，不等於賽程整體閉合或排名 checkpoint 正確。[`src/tournament.rs:2046`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:2046)
- **推論：** 不可讓 `refresh_rankings()` 靜默修正後就直接發布。需要先證明輸入 checkpoint 的 rank 欄位與 Rust 對相同賽果重算的結果相符，或另以等價驗證拒絕失配，再評估其 completion/vacancy 證據。現有 `tournamentView` 會回傳 ranking projection，DO 目前又要求該 projection 與原始 `serverState.tournament` 完全相同；這項相等檢查本身會暴露 rank 漂移，不應在移動邏輯時無意間拿掉。[`src/competition_web_api.rs:1108`](/Users/bendwarn/Documents/fewfc/src/competition_web_api.rs:1108) [`apps/web/worker/durable-objects/competition-room.ts:8610`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:8610)
- **已確認設計方向（Q4）：** 新 completion assessment 保留原始排名不一致時 fail-closed 的防線，不在完成 gate 修復或持久化 checkpoint；歷史排名投影仍可讀，不把投影當成原始權威已有效的證明。

## DO gate 的責任不可一起刪除

`completeCompetitionIfReady()` 是實際寫入 `competition_completion` 的 gate；其工作超出 Tournament 排名語意。[`apps/web/worker/durable-objects/competition-room.ts:6109`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6109)

1. **公告與 checkpoint 前提（必要）：** 只在 locked registration checkpoint 存在、Tournament 非空、每個 Stage 通過 stage check 時繼續。[`competition-room.ts:6111`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6111)
2. **完整公告計畫對應（必要）：** stage 數量及順序中的 stageId 必須逐一符合凍結公告，否則可能漏掉未建立的已公告階段或誤把額外階段當成整場結束。[`competition-room.ts:6122`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6122)
3. **賽果與名次語意（必要，待移交到 Rust 的候選部分）：** `isCompletionCandidateStage()` 要求 stage finished、無排程錯誤、隊伍／standings 對齊，Match completed 且有 result 或 cancelled 且有原因；零隊、一隊採封閉的 resolution 形狀。一般完賽要求唯一正整數 rank；若有空 rank，只有 `isClosedEliminationVacancy()` 在驗證淘汰賽 Double Loss、退出隊伍、 bracket 決賽／敗部路徑與保留 rank 後才放行。[`competition-room.ts:6190`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6190) [`competition-room.ts:6243`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6243)
4. **重播來源（必要）：** `win`、`draw`、`doubleLoss` 結果須有持久 Match 與非空 Games；每個已記錄 Game 須對應已確認的房間來源。Bye、No-Show 等未開局結果不可因此被要求捏造 Game Record。先檢查 replay provenance 再查私密鎖定名單，避免完成發布失敗時不必要重複查詢。[`competition-room.ts:6133`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6133) [`competition-room.ts:6687`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6687)
5. **鎖定牌組 roster 與參賽者交集（必要）：** 從當前 state 輸出 locked roster，重新比對公告計畫；只公開有實際參賽 Stage 的隊伍牌組。[`competition-room.ts:6139`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6139) [`competition-room.ts:6146`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6146)
6. **並行寫入防線（必要）：** roster export 有 await；transaction 內比對 competition ID、revision 與 `server_state_json`，只在狀態未變時插入完成紀錄。這不是排名引擎可代替的純 domain check。[`competition-room.ts:6173`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:6173)
7. **發布與讀取授權（必要）：** 完成列存在才允許非參與者讀取；publication 附帶完成時間、鎖定牌組及 replay refs。排名module不擁有這些存取控制與資料凍結責任。[`competition-room.ts:8562`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:8562) [`competition-room.ts:8622`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:8622)

## Public projection 與 UI

- **事實：** Rust `tournamentView` 回傳由正式賽果重算名次的 public projection，同時不改 server state；DO 確認該投影等於 checkpoint 並沒有私密 roster 欄位才回傳。這是唯讀投影／checkpoint 相等性檢查，不是完成列的 gate。[`src/competition_web_api.rs:1108`](/Users/bendwarn/Documents/fewfc/src/competition_web_api.rs:1108) [`competition-room.ts:8598`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:8598)
- **事實：** UI parser 將公開 JSON 縮成 exact allowlist，不按 standings 陣列順序推導名次；`rank` 只接受正整數或 `null`，並保留 server 提供的 rank 值。[`competition-tournament-view.ts:141`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:141) [`competition-tournament-view.ts:503`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:503) [`competition-tournament-view.ts:921`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:921)
- **事實：** `singleEliminationSemifinalVacancy`、`singleEliminationDoubleLossFinal`、`singleEliminationVacancySummary` 與 `doubleEliminationVacancySummary` 不是純文案轉換：它們查看賽制、目前 rounds、match result/status、schedule warning、已指派 rank，以及部分 bracket 路徑，決定是否顯示空缺與顯示哪句摘要。這些是 UI 端的 domain-fact reconstruction 加文案 adapter；摘要由 helper 在呈現層推導，不是 server completion gate，也不能替代正式名次／賽事完成證據。[`competition-tournament-view.ts:336`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:336) [`competition-tournament-view.ts:395`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:395) [`competition-tournament-view.ts:434`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:434) [`competition-tournament-view.ts:474`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:474)
- **事實：** `CompetitionTournamentView.vue` 的 `rank === null` 分支一律顯示「排名未確定」；round-robin／Swiss 已結束階段的 null rank 另會依三項分數分組，顯示「同分順序待裁決」互動面板。該面板只在 `finished` 且格式為 round-robin／Swiss 時產生；淘汰賽的 `null` 不會因此自動變成正式同分裁決。[`CompetitionTournamentView.vue:275`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:275) [`CompetitionTournamentView.vue:283`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:283) [`CompetitionTournamentView.vue:307`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:307) [`CompetitionTournamentView.vue:529`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:529)
- **事實：** Stage badge 先看 `resolution.type === 'cancelled'`，顯示「階段已取消」；否則依 stage status 顯示「進行中」或「階段已結束」。它是階段狀態文案，不能單獨代表整場 Tournament Completion；頁面只有拿到 completion publication 時才顯示「本場賽事已完成」。[`competition-tournament-view.ts:276`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:276) [`CompetitionTournamentView.vue:40`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:40) [`CompetitionTournamentView.vue:107`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:107)
- **推論：** 現行 UI 把「沒有名次值」與「排名仍待確定」壓成同一個 badge 字串。使用者已確認 Q3：若賽果／判定已結清且隊伍確定未取得名次，該隊 rank 欄留白；只有仍待賽果或裁決的正式排名顯示「排名未確定」，不顯示「未取得名次」字樣。這項產品決定要求把 row 無 rank 與位置 vacancy分開，不是把每個 null 改成 vacancy 文案；詞彙與顯示決定已記入領域及賽事設計文件。[`CONTEXT.md:97`](/Users/bendwarn/Documents/fewfc/CONTEXT.md:97) [`docs/tournament/competitive-play-design.md:69`](/Users/bendwarn/Documents/fewfc/docs/tournament/competitive-play-design.md:69)

### UI claim → 目前證據與缺口

| 可觀察 claim | 現有 owner／證據 | 剩餘缺口或界線 |
|---|---|---|
| 公開 projection 的 standings 順序不是名次來源；名次欄是服務提供的 rank。 | Unit test 名為「without inferring rank from array position」並保留初始 null rank；Nuxt render case也檢查服務順序。 [`competition-tournament-view.test.ts:215`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:215) [`CompetitionTournamentView.nuxt.spec.ts:965`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:965) | 不得在 UI 重排 standings 或由陣列位置補 rank；這不是要新增的 domain authority。 |
| `rank: null` 的現行顯示字串是「排名未確定」。 | Vue 模板單一條件；Nuxt 測試覆蓋已結束單敗真決賽雙負隊伍 team-c、team-a 的 null rank，也在相同場景斷言可存取的「決賽雙負，冠軍空缺」摘要；另一個 active pending single-elimination fixture也把兩隊 null顯示為相同文字。 [`CompetitionTournamentView.vue:283`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:283) [`CompetitionTournamentView.nuxt.spec.ts:817`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:817) [`CompetitionTournamentView.nuxt.spec.ts:992`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:992) [`CompetitionTournamentView.nuxt.spec.ts:1022`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:1022) [`CompetitionTournamentView.nuxt.spec.ts:1218`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:1218) | 前者是現有明確測試 expectation，但與已確認 Q3 的目標行為不同：確定未取得名次的 team-c/team-a rank欄目標應留白。需更新此完成案例 expectation及模板狀態依據；active等待賽果、正式 unresolved ranking tie仍保留「排名未確定」。不要以 `rank === null` 單欄判斷兩者，也不增加「未取得名次」字樣。 |
| 真正單敗決賽雙負與半決賽空缺路徑顯示可存取的摘要，並保留真實第三名。 | Unit tests覆蓋一般決賽含季軍賽、季軍位置由 bye 形成、半決賽雙負後正常敗隊輪空；Nuxt component test斷言 `role=status` 文字、輪空、不造季軍賽及第三名。 [`competition-tournament-view.test.ts:576`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:576) [`competition-tournament-view.test.ts:609`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:609) [`CompetitionTournamentView.nuxt.spec.ts:1032`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:1032) | Nuxt fixture 是公開 GET UI wire fixture，不是 Rust producer 或 canonical Tournament record；domain truth仍須由 Rust／DO相關證據驗證。[`CompetitionTournamentView.nuxt.spec.ts:923`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:923) |
| 雙敗空缺摘要依勝部／敗部晉級路徑，並避免一般完賽誤報空缺。 | Unit tests覆蓋敗部決賽雙負、敗部冠軍沿總決賽空位晉級及普通完賽不顯示空缺。 [`competition-tournament-view.test.ts:432`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:432) [`competition-tournament-view.test.ts:485`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:485) [`competition-tournament-view.test.ts:532`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:532) | Nuxt component test目前覆蓋普通雙敗決賽的輪次／結果，不覆蓋雙敗空缺摘要的實際 DOM 文案／可存取性；總決賽或重置賽雙負保留季軍的 Vue rendering claim也沒有相同層級的證據。[`CompetitionTournamentView.nuxt.spec.ts:539`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:539) |
| 結構不完整／尚待裁判處理時，不由缺配對或 warning 推出已確定空缺。 | Vacancy helpers在 warning、active status或不符合完成 bracket 形狀時回 `null`；component test在同一多 stage case確認 unresolved DE 顯示 schedule review note，且不顯示真決賽摘要。 [`competition-tournament-view.ts:395`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:395) [`competition-tournament-view.ts:434`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:434) [`CompetitionTournamentView.nuxt.spec.ts:1020`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:1020) | 需保留此 fail-closed observable；helper 的 TS 對抗資料檢查不能用來聲稱 Rust 完賽 authority 已等價。 |
| 零隊取消與一隊無交手勝者各自呈現。 | Unit test斷言 resolution label 與 cancellation badge；Nuxt test斷言繁體文案及不洩漏 machine team ID。 [`competition-tournament-view.test.ts:234`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:234) [`CompetitionTournamentView.nuxt.spec.ts:399`](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:399) | 尚無 Storybook 中的視覺／互動呈現案例。 |
| 無勝者 Double Loss 與 double no-show 結果都不捏造 winner。 | Parser/helper unit tests分別驗證 doubleLoss 及雙方缺席 noShow 結果文案；vacancy route helper也接受雙方缺席作為空缺路徑。 [`competition-tournament-view.test.ts:362`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:362) [`competition-tournament-view.test.ts:657`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.test.ts:657) [`competition-tournament-view.ts:362`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:362) | 現有 SE vacancy tests以明確 `doubleLoss` 建案；尚未看到雙方缺席 `noShow` 對同一 vacancy route的單獨 positive/negative 測試。 |
| Storybook能審查 vacancy、取消、未決和已結束但尚無 publication 的實際 DOM／版面。 | 目前 `CompetitionTournamentView.stories.ts` 只有 `ContextLoading`；另有 `tournament-preview.ts` 初始 round-robin fixture，但 Story未引用它。 [`CompetitionTournamentView.stories.ts:1`](/Users/bendwarn/Documents/fewfc/apps/web/stories/components/CompetitionTournamentView.stories.ts:1) [`CompetitionTournamentView.stories.ts:23`](/Users/bendwarn/Documents/fewfc/apps/web/stories/components/CompetitionTournamentView.stories.ts:23) [`tournament-preview.ts:1`](/Users/bendwarn/Documents/fewfc/apps/web/stories/fixtures/tournament-preview.ts:1) | 缺少空缺／stage cancellation／正式未決排名／整場 publication 對照的 deterministic story fixtures、full/component presentation及視覺 acceptance；現有 Nuxt assertions是 DOM contract，不提供 Storybook page appearance evidence。[`apps/web/docs/storybook.md:36`](/Users/bendwarn/Documents/fewfc/apps/web/docs/storybook.md:36) |

### 建議保留的責任分工與 observable

- **事實：** `CONTEXT.md` 與 competitive design 定義名次空缺和未決位置不同；ADR-0039 保留原 Game Conclusion 與獨立 Competition Ruling；ADR-0032 要求按獨立 claim 保留精確測試證據。UI只能顯示這些事實，不改寫 Double Loss為勝者或把 Stage 結束當作 Tournament Completion。[`CONTEXT.md:97`](/Users/bendwarn/Documents/fewfc/CONTEXT.md:97) [`docs/tournament/competitive-play-design.md:69`](/Users/bendwarn/Documents/fewfc/docs/tournament/competitive-play-design.md:69) [`docs/adr/0039-separate-competition-rulings-from-game-conclusions.md:5`](/Users/bendwarn/Documents/fewfc/docs/adr/0039-separate-competition-rulings-from-game-conclusions.md:5) [`docs/adr/0032-model-and-test-rule-interactions-explicitly.md:22`](/Users/bendwarn/Documents/fewfc/docs/adr/0032-model-and-test-rule-interactions-explicitly.md:22)
- **已確認產品偏好（Q2）：** 保留 vacancy 的具體原因，由 Rust 提供語意事實，UI 負責繁體中文文案；interface 方向另由 Q4 確認，詳細 DTO 欄位仍由實作契約決定。
- **設計推論（責任分工建議，非新 interface決定）：** Rust／DO 及產品政策負責 authoritative ranking、vacancy、unresolved 與 completion 證據；Rust public facts應能承載具體 vacancy原因，UI adapter把它映成繁體中文；TypeScript parser負責精確 wire parsing並拒絕未知形狀；Vue負責無障礙呈現和事實格式化。現有 TS vacancy helper重建了 bracket 語意，未來若移交該責任，逐條 claim應由 domain/transport證據接手後再調整 helper與其測試，保留純呈現層的DOM／accessibility claim。DO的完成發布條件仍由DO維持。Rust已存在資格來源閉合檢查，拒絕有schedule error、未結束Stage或pending/in-progress Match的來源；可視為既有 close-evidence參考，但不能據此認定 `rankings_resolved()` 本身證明Tournament Completion。[`src/tournament.rs:1516`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:1516)
- **推薦保留的可觀察行為：** 保留 server rank與standings順序，不由 array index補排名；區分「階段已取消」與「階段已結束」；只對已結清且結構一致的 elimination result 顯示 vacancy summary；保留真實季軍與中間名次，不遞補、不壓縮；分別呈現 unresolved ranking tie面板和明確 vacancy；普通完賽不報空缺；Tournament Completion只在有 publication時顯示；所有取消／空缺／未決文案為繁體中文並可被輔助科技讀取。相關source contracts見 [`CompetitionTournamentView.vue:191`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:191)、[`CompetitionTournamentView.vue:197`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:197)、[`CompetitionTournamentView.vue:295`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:295)及 [`CompetitionTournamentView.vue:40`](/Users/bendwarn/Documents/fewfc/apps/web/app/components/CompetitionTournamentView.vue:40)。
- **設計狀態：** 使用者已確認採用下文方案 A：單次 assessment 加公開階段 facts。詳細 wire 型別須同時保護既有 projection 相等性、DO gate 與歷史讀取契約。

## 公開事實的傳遞限制

目前的 seam 是 Rust `tournamentView` → CompetitionRoom → HTTP adapter → UI parser → 顯示。這條路徑有三種不同檢查，深化時不能一起移除：

| 位置 | 現有檢查 | 新設計必須保留的意義 |
|---|---|---|
| Rust 查詢與 CompetitionRoom | 公開排名投影不改 `serverState`；DO 要求 `raw.public` 等於原始 `serverState.tournament`。 | 拒絕來源不符、原始名次漂移或查詢改寫狀態。新增公開事實後，必須明確重新界定比較對象，不能直接刪掉相等檢查。 |
| HTTP adapter | 外層身分、revision、操作名稱及公開內容的 exact keys；公開 Tournament 根只有 `tournamentId`、`stages`。 | 新欄位須有明確傳遞契約，未知資料與私密內容仍拒絕。 |
| UI parser | Stage 的 exact keys；驗證 `teams`、`doubleState` 等排程內部資料後，將它們排除於 UI view。 | 收斂成所需公開事實後，UI 不再需要理解排程內部形狀；仍驗證公開型別與互相關聯的身分。 |

來源：[`src/competition_web_api.rs:1108`](/Users/bendwarn/Documents/fewfc/src/competition_web_api.rs:1108)、[`competition-room.ts:8610`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.ts:8610)、[`competition-tournament.ts:170`](/Users/bendwarn/Documents/fewfc/apps/web/server/utils/competition-tournament.ts:170)、[`competition-tournament-view.ts:503`](/Users/bendwarn/Documents/fewfc/apps/web/app/lib/competition-tournament-view.ts:503)。

**設計限制：** 新的結清、名次與空缺原因是從 Tournament 推導的事實，不應為了維持上述物件比較而加入 Game Record、Game Event，或持久化成第二份權威資料。歷史 snapshot 的讀取與排名投影能力仍需保留；現有 Rust 有專門的歷史投影測試，不能為了簡化當前完成檢查而一律拒絕歷史輸入。[`src/tournament.rs:5125`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:5125)

**顯示限制：** 階段名次可以在整場尚未完成時確定；`publication` 表示整場完成後公開資料，不能作為判斷某一 Stage 名次的唯一輸入。已完成淘汰 Stage 的留白／原因，應與仍在進行的後續 Stage 同時可表達；整場完成字樣與公開存取仍受 DO 的發布條件控制。

## 三種 interface 比較與已選定方案

使用者已選定方案 A；保留 B、C 作為取捨紀錄。以下名稱及 wire 欄位仍是示意，詳細編碼由實作契約決定。三者皆沿用 Rust in-process 計算、既有 WASM adapter 與 DO 儲存 seam；沒有新增遠端依賴或通用可替換規則框架。

### A．一次評估，同一份結果供兩種呼叫端使用（已採用）

```rust
impl Tournament {
    pub fn assess_completion(&self) -> Result<TournamentAssessment, AssessmentError>;
}
struct TournamentAssessment {
    stages: Vec<StageAssessment>,
    readiness: CompletionReadiness,
}
struct StageAssessment {
    stage_id: StageId,
    settlement: StageSettlement,
    placements: Vec<TeamPlacementFact>,
    vacancies: Vec<PlacementVacancyFact>,
    explanations: Vec<PlacementExplanation>,
}
enum TeamPlacement { Ranked(u32), NoPlacement, Unresolved }
```

一次驗證來源並重算，隱藏賽制分支、淘汰路徑、原始排名比對及未決裁決。`readiness` 只代表 snapshot 中現有 Stage 是否足以完成，不證明已公告 Stage 齊全或允許公開。Web adapter 將 `stages` 的顯示事實轉為公開 DTO；DO 使用 readiness 並完成外部檢查。UI 不必接收完整 assessment 或認識完成發布的 blocker。

`vacancies` 表示無人取得的名次位置；`explanations` 表示已判定路徑，例如「勝部冠軍沿空位晉級」及「保留已決季軍」。後兩者不是空缺位置本身，不應硬塞成某隊的 vacancy 狀態。封閉 enum 保留原因，UI 負責繁體中文。名次可合法並列；不能用 `rank → 單一 Team` 的 map，也不能把所有未出現的整數名次視為 vacancy。

### B．經驗證的不可變 handle，再作封閉查詢

```rust
let facts = tournament.completion_facts()?;
facts.completion();
facts.stage(&stage_id)?.team_placement(&team_id)?;
facts.stage(&stage_id)?.vacancies();
facts.resolve_qualification(&slots)?;
```

handle 借用同一份 Tournament，初始化時驗證，之後查詢不會落後來源。這讓資格解析與多種名次查詢具有較高 leverage；成本是公開型別、生命週期與多個查詢入口。WASM 仍須一次組裝結果，不能把逐隊查詢變成多次呼叫。目前並沒有需要這種彈性的新增呼叫端；將資格 interface 一併改造會放大首個切片。

### C．專用 UI 讀取投影，另設完成判定

```rust
tournament.public_read_projection()?; // 階段、交手、名次狀態與原因
tournament.assess_completion()?;      // 只供完成流程
```

兩入口共用 private facts 計算。專用 DTO 可省去 UI 現在驗證後丟棄的排程內部欄位，讓日常讀取最簡單。成本是新舊投影並存、完整 DTO 遷移及多層 exact parser 改造；為保留原始排名漂移檢查，過渡期仍須同次產生 legacy projection 供 DO 精確比對。適合後續縮小公開 read contract，首個切片較大。

| 面向 | A：一次評估 | B：handle／queries | C：專用 read projection |
|---|---|---|---|
| Depth | 一個入口隱藏結清、排名與原因推演 | 多種查詢共用驗證，但呼叫端須組裝 | UI 一次取得最小可顯示結果 |
| Locality | 規則集中 Rust；兩端消費同源事實 | 規則集中，但同時碰資格與查詢模型 | 規則集中；另需維護完整公開 DTO |
| Seam | 現有 WASM 傳遞獨立 facts | Rust handle 留在程序內，WASM 批次轉換 | 新 read contract 加 legacy 比對投影 |
| 主要代價 | 新 facts 契約、精確來源對應 | 更多 public interface 與抽象 | 跨 Rust／DO／HTTP／UI 的較大遷移 |

**已選定 A，採納 C 的「只輸出呼叫端所需資料」原則。** Rust assessment 不等於公開 wire DTO。先集中已重複的語意，再由現有 Web adapter 選出 UI facts；不為這次重構一併重寫所有公開賽程資料。若內部需要不可變索引，可採 B 的實作方式，暫不把 handle 與資格查詢暴露成新 public interface。

### 已選定方案的錯誤與傳遞契約

1. 保留歷史 `ranking_projection()` 的讀取能力和 snapshot deserialize 契約。新的 assessment 對來源矛盾／原始排名漂移回傳具名錯誤，完成流程 fail closed；不得把所有歷史讀取強迫導向新的嚴格 assessment，也不在完成 gate 修復或持久化排名。
2. 正常 active Stage、未決同分屬有效未決事實，與損壞 checkpoint 分開。後續 Stage active 時，前一個已結清 Stage 仍可提供 `NoPlacement` 與原因。`Finished` 卻含未關閉 Match 不可取得結清證據。僅有 `Finished`、null rank 或重算一致也不足以證明空缺：必須驗證合法終局與退出路徑，避免把缺少必要輪次／交手的資料誤認為已結清。
3. 新 facts 與原 `public` 並列傳遞，不寫進 Tournament snapshot 或 canonical events。DO 維持 `public === serverState.tournament` 及 unchanged `serverState` 防線，再驗 facts 的 Tournament／Stage／Team 身分與覆蓋範圍。公開 HTTP 回應只帶階段顯示事實；exact allowlist 要有意識地逐層更新。歷史投影入口與新的嚴格評估如何在 Web operation 中組合，屬實作契約，必須有舊讀取測試保護。
4. stages／standings 沿來源順序；名次使用明示 rank，合法並列不壓縮。隊伍狀態與既有 rank 必須一致。缺少有效 facts 時，不得將 `rank: null` 當成已確定無名次，也不能回頭以 TS bracket 推演替代證據。
5. DO 先驗 domain readiness 與公告完整性，再驗 replay refs，最後取得私密 roster；任何新增 await 後仍以同一 competition ID、revision、完整 state 作 CAS。新的 Rust 成功結果不能繞過這些檢查。

### 可觀察的 UI 例子

| 情境 | 隊伍名次欄 | 階段說明 |
|---|---|---|
| 真決賽雙負，兩隊已確定無名次 | 留白 | 保留「決賽雙負，冠軍空缺」等已證明原因 |
| 重置賽雙負，季軍已決 | 決賽兩隊留白；季軍顯示第三名 | 冠亞軍空缺，保留已決季軍 |
| 勝部／敗部冠軍沿空位晉級 | 冠軍顯示第一名；確定無名次者留白 | 保留實際晉級路徑及空缺位置 |
| 等待賽果或循環／瑞士正式同分裁決 | 「排名未確定」 | 保留既有待裁決互動 |
| 前一 Stage 已結清，下一 Stage 進行中 | 各 Stage 依自己的 facts 顯示 | 不顯示整場完成，直到 DO publication 存在 |

## 測試證據與 claim 遷移

**已存在且應保留的證據：** Rust 已測正式同分阻止完賽並於裁決後解除、舊 snapshot 重算排名、空來源 Stage 的明確取消、單敗全路徑雙負空冠軍及排名空缺資格解析、已完成的雙負空缺可供下一階段使用。[`src/tournament.rs:5006`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:5006) [`src/tournament.rs:5125`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:5125) [`src/tournament.rs:6798`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:6798) [`src/tournament.rs:7273`](/Users/bendwarn/Documents/fewfc/src/tournament.rs:7273)

**DO 測試已有 claims：** 完成單敗決賽雙負但保留已判季軍、半決賽 bye 與終局雙負、雙敗 lower-final／grand-final 空缺、共享下位名次與錯置排名拒絕、排名未決時不發布，以及公告 Stage 不齊時不發布。[`competition-room.test.ts:5743`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:5743) [`competition-room.test.ts:5904`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:5904) [`competition-room.test.ts:12352`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:12352)

下表指定實作時的承接位置；「應承接」不表示新測試已存在。既有 DO 情境有透過實際 WASM 產生賽程的價值，其 private helper 斷言應在新公開入口有等價證據後才移除。

| 唯一 claim | 現有證據 | 新 interface 應承接／仍保留處 |
|---|---|---|
| 真單敗決賽雙負，已完成季軍賽仍有效 | [ticket 16](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:5743) | Rust assessment 同時證明無冠亞軍、已決季軍及可結清；UI 測原因文案。 |
| 先前半決賽 bye 後決賽雙負，不造季軍賽；pending Match／schedule error 拒絕 | [ticket 20 正反例](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:5806) | Rust assessment 測成功及兩種阻擋原因；閉合判定屬新入口，不普遍加嚴歷史 deserialize。 |
| 雙敗敗部決賽／總決賽雙負，已結清空缺可完成 | [雙敗終局](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:5904) | Rust 提供具體終局原因與位置；另補重置賽相同責任的明確證據。 |
| 八隊合法共享下位名次；刪除已決第四名、錯配賽果或捏造亞軍拒絕 | [八隊正反例](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:5978) | Rust assessment 驗 raw standings 與推導結果，不能只檢查 rank 唯一。 |
| 勝部半決賽皆雙負後，敗部敗隊保持第五名，不能升為亞軍 | [不可遞補](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:6108) | Rust 以敗部路徑證明名次；UI 僅格式化結果。 |
| DE16 跨後續／跳過輪次保留第六、第四名，刪除已決名次拒絕 | [DE16](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:6204) | Rust assessment 保留完整敗部深度案例。 |
| 早期雙負與無冠軍終局仍保留已決第七名 | [早期／無冠軍案例](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:6308) | Rust assessment 拒絕改為第六名或 null；不能用「無冠軍」抹掉所有名次。 |
| DE13 新敗隊及空層改變實際敗部位置，不能沿用一般賽程名次 | [DE13](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:6424) | Rust assessment 覆蓋 `[4,4,3]` 與空層第四名，拒絕套用普通路徑的 `[9,4,6]`。 |
| 單敗早期空缺及同輪下位並列可結清 | [單敗空缺／並列](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:6607) | Rust facts 不把合法共享名次的數字間隔標為 vacancy。 |
| 零隊明確取消、一隊無交手勝者；重試不重複建立 | [一隊](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:10546)、[零隊](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:10598) | Rust 承接 resolution 與結清；DO 保留重試與發布整合證據。 |
| 正式同分未決不完成；漏掉公告 Stage 不發布；發布重試資料不可變 | [完成發布](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:12352)、[未決](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:12454)、[公告未齊](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:12475) | Rust 承接未決排名；DO 保留公告、replay、roster、CAS 與不可變發布證據。 |
| 已結清決賽兩隊無 rank 與 active pending 不能同文案 | [現有完成案例](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:992)、[active 案例](/Users/bendwarn/Documents/fewfc/apps/web/tests/component/CompetitionTournamentView.nuxt.spec.ts:1218) | 完成案例改成名次欄留白；active 與正式同分仍顯示「排名未確定」。保留 DOM／accessibility 測試。 |

**新增契約所需證據：** 原始 rank 漂移與 facts 身分／coverage 不符須拒絕；既有歷史 projection 仍可讀；read 不改 state；新多字欄位精確 camelCase；前 Stage 已結清、後 Stage active 時仍能顯示原因與留白。這些新測試須對公開 Rust／Web interface 驗證，不以 private helper 名稱綁住實作。

依 ADR0032，不能只按測試名稱或數量刪除現有測試；每一項唯一 claim 都要先有命名替代證據。[`docs/adr/0032-model-and-test-rule-interactions-explicitly.md:8`](/Users/bendwarn/Documents/fewfc/docs/adr/0032-model-and-test-rule-interactions-explicitly.md:8)

## 已跑驗證紀錄

另有 204 個既有 Bun pass（`competition-game-room`、`online-command-transaction`、`shared/competition-game-room`）是鄰近回歸證據，**不覆蓋本候選的完賽／空缺判定 claims**。

- `cargo test --lib tournament::tests`：53 passed，173 filtered out。
- `bun test worker/durable-objects/competition-room.test.ts -t 'ticket 16|ticket 20|ticket 21'`：14 passed，375 filtered out，226 assertions。

**驗證限制：** `competition-room.test.ts` 未設定 `FEWC_COMPETITION_TEST_WASM` 時會使用 `apps/web/worker/wasm/fewfc.wasm` 預建檔；因此另以目前原始碼新建 WASM，重跑候選測試。`cargo build --locked --release --target wasm32-unknown-unknown` 成功（57.42 秒）；於 `apps/web` 執行 `FEWC_COMPETITION_TEST_WASM=/Users/bendwarn/Documents/fewfc/target/wasm32-unknown-unknown/release/fewfc.wasm bun test worker/durable-objects/competition-room.test.ts -t 'ticket 16|ticket 20|ticket 21'`，結果 14 passed、375 filtered、0 failed、226 assertions。[`competition-room.test.ts:92`](/Users/bendwarn/Documents/fewfc/apps/web/worker/durable-objects/competition-room.test.ts:92)

本輪另執行的針對性回歸：`bun test app/lib/competition-tournament-view.test.ts` 為 22 passed／134 assertions；`bun run test:component tests/component/CompetitionTournamentView.nuxt.spec.ts` 為 17 passed；`bun test --isolate server/utils/competition-tournament.test.ts 'server/api/competitions/[id]/tournament.get.test.ts'` 為 14 passed／172 assertions；`cargo test --test competition_web_api e6d1a_tournament_view` 為 5 passed、38 filtered。Nuxt result 同時確認現有 finished final-double-loss 案例讓 null rank 文案與 vacancy summary並存；這支持「目前行為」證據，不代表與新確認產品偏好相符。

目前結果支持 Rust 規則與 DO gate 的基線。上表已指定關鍵 claims 的承接位置；尚未實作的新 Rust assessment 不具有替代證據，因此現在不能刪除 TS semantic guards 或其測試。

## 深化的驗收準則與決策樹

**設計推論：** 深化既有 Tournament module，讓完賽語意的 implementation 位於 Rust，透過既有 WASM seam 由 Web adapter 讀取。interface 的 depth 應以呼叫端不再理解淘汰名次推演來衡量；locality 是名次規則與驗證集中，leverage 是完成流程與後續資格解析可共用已驗證的事實。領域計算屬 in-process，無須為它新增可替換 adapter；既有 WASM adapter 與 DO 持久化測試各驗證其真實 seam。

**Deletion test：** 直接刪除 TS 檢查會失去拒絕矛盾資料的能力；只搬進另一個 TypeScript 檔案也不增加 depth。只有 Rust 承接每項語意 claim、DO 保留發布防線後，刪除重複推演才有價值。interface 測試應經正式 Tournament／Web 入口建立情境，逐步取代繞過 private 限制的呼叫；不得先刪除現有測試。

- 已確定：研究目標為 Tournament 完賽與名次空缺判定；本輪 UI 空缺語意也已由使用者選定納入研究（Q1），不再列為後續範圍。產品規則、原始 Game Conclusion、公開條件維持既有契約。
- 已確認 UI產品方向：保留 vacancy的具體原因，由 Rust輸出事實、UI負責繁體中文（Q2）；確定未取得名次的隊伍 rank欄留白，只有仍待賽果／裁決的排名顯示「排名未確定」，不顯示「未取得名次」（Q3）。因此現有 finished final-double-loss Nuxt expectation是目標差異，必須在實作時修改；active未決與正式 unresolved tie的文字仍保留。
- 已確認的設計方向（Q4，2026-11-13）：採 A 的單次 Rust assessment，Web adapter 分流 DO readiness 與 UI 階段事實；保留現有歷史投影及 DO 發布防線。詳細欄位名稱、operation 組合及 enum 編碼由實作時的契約測試約束，不要求使用者決定機械性細節。

## 建議實作順序與完成條件

1. 從公開 Tournament interface 建立 assessment 與上表的語意正反例。先集中現有排名／資格閉合計算，避免在 Rust 再寫第二套 bracket 模擬。保留歷史投影回歸。
2. 接上現有 WASM／DO seam，保護來源身分、原始排名、唯讀及 camelCase 契約。DO 繼續驗公告、重播、roster 與 CAS；其每項語意 claim 有替代證據後，才移除 `isClosedEliminationVacancy` 的重複推演。
3. 將精確階段 facts 經 HTTP／parser 送至 UI，更新 finished null-rank 案例為留白，移除已被 facts 取代的 bracket helper，保留中文、顯示順序、同分互動及 accessibility 測試。
4. 增加 Storybook 情境：正常未決、已結清留白、單敗／雙敗／重置賽空缺、保留季軍、零／一隊及前 Stage 完成後續仍 active。依 Web 規範完成 Storybook 視覺驗收；Nuxt 行為測試不能替代它。

完成標準是規則變更只需改 Rust 語意及必要呈現 mapping，DO／UI 不再重建淘汰路徑；所有既有獨立 claims 都有證據，且 Rust／DO／UI 分工及顯示符合 Q1–Q4。此研究未進行產品程式修改，也未執行新畫面的 Storybook 視覺驗收。
