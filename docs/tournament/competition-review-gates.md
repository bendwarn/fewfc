# 競賽功能：目前審核關卡

2026-10-11 本輪重審 03 公告、05 初始種子、07 權威時計與 17 缺席：四票均在各自範圍限定 PASS，依使用者要求按功能提交。17 最新截止／舊準備端點／noShow 後斷線修正已經獨立兩軸與固定時間案例驗證。19、既有 AGENTS.md 與其他 dirty 保留。產品政策以[設計](competitive-play-design.md)為準。本文件只管理現況、互斥範圍、驗證與相依放行，不複述玩法規則。

## 執行邊界與完成條件

依 2026-10-09 使用者新指示，先閉合正常完賽主線，再補會卡住賽程的自動例外；人工裁判介面與更正流程延後。交付順序、範圍及直接功能相依只見[正式票據索引](tickets/README.md)，不另建待辦副本。

先完成本次交付範圍的元件／純模組測試，該範圍全部實作後才做總測試。主審集中一次一個驗證程序；實作者只編寫案例。兩軸審核涵蓋指定檔案完整現況與 Git 差異。延後功能不再作為首場賽事交付範圍總驗收的前置。

完成通知只代表交付；放行須指定範圍的元件實證、Standards／Spec 審核及相符的 Git 差異。沒有執行、僅來源斷言或其他版本的綠燈，均不能升格為新關卡通過。產品最後完成須通過集中整合／全流程驗收，不能以 mock 代替真房間。

本輪不做舊賽事相容或生命值專項驗證。先前兩筆前端提交為 `f8de7be` 基礎契約／解析與必要 import aliases、`cd3ab24` 賽程／時計設定元件；2026-10-11 其餘工作按設定、核心、服務、網站及文件分批保存至 Git。提交保留既有 PASS／HOLD 界線，沒有 push 或部署。設定與工具命令從實際設定檔／專案查取，不在此另管理模型預設。

## 現在交付與互斥範圍

| 切片／負責者 | 狀態與唯一寫入範圍 | 下一關卡／證據 |
| --- | --- | --- |
| 07 權威隊伍時計／本輪獨立主審 | 限定 PASS：終局先關玩家視窗，再依 FIFO 送終局；重試、restart、strict ACK 及固定時計 ID 斷言均通過。Standards 硬性 0、Spec 0；GameRoom 完整 125 pass／0 fail，ClockDisplay 共用 Nuxt 驗證通過 | [本輪報告／兩軸／原始 log](../../scratch/four-feature-review/report.md)。原 FIFO 測試 P2 已閉合，07 六項驗收已勾選；既有同隊／回合外、有限額度、零不限時路由保留。18 真超時裁決與 22／28 集中界線保留。 |
| 05 自動抽籤並建立首輪／本輪獨立主審 | 限定 PASS：固定亂數的兩隊／奇數隊抽籤、正常輪空、持久建立意圖、相同收據及 alarm 重試均有正向案例；父頁與 notReady 自動更新案例通過。Standards 硬性 0、Spec 0 | [本輪報告／兩軸／原始 log](../../scratch/four-feature-review/report.md)。CompetitionRoom 完整 349 pass／0 fail；沿公告與凍結名單建立同一首輪，不要求 F 後確認；05 四項驗收已勾選。完整 Swiss 政策擴充已有來源使用者授權；指定種子來源僅在支援時適用，不外推 22／28。 |
| 04 報名截止自動凍結／來源對話最後修正→主審末審 | 限定 PASS：alarm P2 與最後測試預期 P2 均閉合；Standards 硬性 0／heuristic 0，Spec 0。本輪作者 E6a2 15 pass／0 fail、288 assertions，1v1／2v2 正式交付與重放全綠。僅測試檔改為從已提交 checkpoint 取預期版號，其他來源未改。沿用同版本 Nuxt／pure／獨立故障證據，驗證集合跨輪合計 258 pass／6 skip／0 fail，本輪新執行只有 15 例 | [最後修正報告／固定版本／原始 log](../../scratch/tour-attempt/ticket04-final-review/report.md)、[alarm 修復與原紅例](../../scratch/tour-attempt/ticket03-04-review/report.md)、[前次失敗](../../scratch/tour-attempt/ticket04-review/report.md)。04 四項驗收已勾選；放行 F 自動凍結、遲交接受點、排除理由、固定參賽快照交付與正常重試。05 的 03＋04 前置已通過，可依唯一寫入權開始其實作；legacy initial 障礙仍既存 HOLD，不外推 22 真 Worker／Brave／全流程。 |
| 03 賽前完整公告／本輪獨立主審 | 限定 PASS：完整 SwissStandingPolicy 公告契約、Rust／TypeScript 驗證與玩家可讀政策已核對；Standards 硬性 0、Spec 0。共用純模組／API 230 pass、6 skip、Nuxt 八檔 107 pass；Rust DTO 與公告案例通過 | [本輪報告／兩軸／原始 log](../../scratch/four-feature-review/report.md)。本輪按 HEAD `653a516` 的指定功能差異提交；全量 Cargo 的既有單敗決賽 API oracle 與型別檢查的既有測試錯誤仍保留，不外推 22／28。 |
| 02 等待房斷線清除準備／來源對話交付→主審驗證 | 限定 PASS：DO 全檔 118 green／1 既存 fail（修改前同樣失敗），元件 12 green，主審獨立 4 green。Standards 0／Spec 0 個有效 finding， | [本輪審核與原始 log](../../scratch/tour-attempt/ticket02-review/report.md)。只放行等待房斷線／重連、2v2 開局競態、同人多分頁／舊回呼、Preparing／Active 負控制；不外推 01 提前開局、17 缺席或 22 全流程。全檔既存 randomness-restart payload 失敗另保留，不宣稱全綠。 |
| 首輪 Rust 凍結 R1 | 限定 PASS：7 元件案例、兩軸無 finding；兩 Rust 檔凍結 | [主審原始證據](evidence/2026-10-08/main-review-history.md) |
| 前端凍結／Maxwell→Socrates 分鐘修復 | 限定 PASS：pure40／198 assertions、Nuxt兩檔18 green；Standards 無 finding／Spec 無 P1/P2、DST-gap 覆蓋 P3 非阻擋。七檔凍結；不代表後端已強制所有公告 T 的分鐘精度 | [修復](../../scratch/tour-attempt/frontend-announcement-minute-policy.md)、[Standards](../../scratch/tour-attempt/delivery-wave-e-frontend-minute-standards-review.md)、[Spec](../../scratch/tour-attempt/delivery-wave-e-frontend-minute-spec-review.md)、[集中實證](../../scratch/tour-attempt/main-competition-2026-10-08.md) |
| 父公告 P0a | 原 receipt-only 限定 PASS：18 元件案例、兩軸無 finding；不證後续 delivery | [原始關卡](evidence/2026-10-08/competition-review-history.md) |
| 父 claims／ACK／release／Jason→Nietzsche 公平排程修復 | 限定PASS：本輪三prefix74／474green `b376e1`。完整末兩軸審核同一範圍，Standards hard0／測試樣板P3非阻擋、Spec0，J-1／F-2及舊F findings限定閉合；可開始相依initial receiver。此PASS只涵蓋該dispatcher來源內容 | [修復](../../scratch/tour-attempt/parent-alarm-fairness-repair.md)、[末Standards](../../scratch/tour-attempt/delivery-wave-l-parent-standards-review.md)、[末Spec](../../scratch/tour-attempt/delivery-wave-l-parent-spec-review.md)、[集中紀錄](../../scratch/tour-attempt/main-competition-2026-10-09.md)。childcursor／17欄contract不放寬，舊producer15→17另HOLD，不外推native lineage |
| 01 準備即報到與提前開局／來源對話交付→獨立主審 | 限定 PASS：主審依序重跑指定 223 cases／3665 assertions，額外正式 Rust/WASM 首局 2 cases／47 assertions；合計 225 pass／0 fail。Standards 硬性 0／測試 helper 重複 P3 建議 1，Spec 0；兩種準備／放行順序、早於 T、單一交手不等其他玩家、直接 action／Pouch Preparing、父房賽程／時計、FIFO 與重試均有實證 | [主審報告／固定版本／原始 log](../../scratch/tour-attempt/ticket01-review/report.md)、[作者交付](../../scratch/tour-attempt/ticket01-ready-check-in-implementation.md)。原父子房 Pouch 案例 mock 遊戲引擎；額外兩例使用既有正式 WASM，證明真正 GameRoom 首局自動開局與持久 evidence，平台邊界仍為 fixture。未重建 WASM、未跑 native DO／Brave／平台重啟或全流程，這些仍由 22 集中驗收；缺席處置仍屬 17，不外推其他票或新變更。 |
| 子首局 FIFO／Lagrange→Goodall 窄修 | 限定 PASS：延遲2例／60 assertions、原31例／808 assertions green；Standards 無硬性 finding／P3 非阻擋，Spec 無 finding；四檔凍結 | [修復](../../scratch/tour-attempt/child-final-start-sample-repair.md)、[Standards](../../scratch/tour-attempt/delivery-wave-e-child-final-sample-standards-review.md)、[Spec](../../scratch/tour-attempt/delivery-wave-e-child-final-sample-spec-review.md)；父 initial receiver 仍須等待父公平排程關卡，不外推跨房協議 |
| 輪次公告 HTTP／Mencius→Goodall 修復 | 限定 PASS：元件32 pass／323 assertions；兩軸無 finding。裸問號／cleanup 變動已修，無第2算法 | [原交付](../../scratch/tour-attempt/round-publication-http-implementation.md)、[修復](../../scratch/tour-attempt/delivery-http-parent-regression-repairs.md)、[Standards](../../scratch/tour-attempt/delivery-wave-c-http-standards-review.md)、[Spec](../../scratch/tour-attempt/delivery-wave-c-http-spec-review.md)；不代表父／子、時間政策或整條開局流程通過 |
| 純階段資格解析／Descartes | 限定 PASS：10 `stage_qualification_`／0 fail，兩軸無阻擋 finding，Standards P3 非阻擋。只解析來源→隊伍→seed | [交付](../../scratch/tour-attempt/stage-qualification-implementation.md)、[Standards](../../scratch/tour-attempt/delivery-wave-a-standards-review.md)、[Spec](../../scratch/tour-attempt/delivery-wave-a-spec-review.md)；下一切片開始修改後不外推新 版本 的 PASS |
| ≥2 隊下一階段計畫／套用／Nietzsche | 限定 PASS：`src/tournament.rs` ，新8＋原10元件 green；Standards 無硬性 finding／3個 P3非阻擋，獨立 Spec 無阻擋 finding。不做 0／1 隊收尾或 persistence／bridge | [交付](../../scratch/tour-attempt/qualified-stage-plan-implementation.md)、[Standards](../../scratch/tour-attempt/delivery-wave-d-stage-plan-standards-review.md)、[Spec](../../scratch/tour-attempt/delivery-wave-d-stage-plan-spec-review.md)；相依 bridge 已分配互斥所有權 |
| ≥2 隊階段 JSON bridge／Socrates→獨立審核 | 限定 PASS：新增7例與bridge全模組10例 green；Standards 硬性0／P3維護風險非阻擋，Spec 無阻擋 finding。閉合 preview／同呼叫 plan→apply；三 Rust 檔凍結 | [交付](../../scratch/tour-attempt/qualified-stage-json-bridge-implementation.md)、[Standards](../../scratch/tour-attempt/delivery-wave-f-stage-bridge-standards-review.md)、[Spec](../../scratch/tour-attempt/delivery-wave-f-stage-bridge-spec-review.md)。可準備相依 TS 契約；持久化／授權仍需父關卡與互斥 owner，不代表 WASM／DO 接線或 stale-preview lease 通過 |
| 輪次公告網站元件／Goodall | 限定PASS：pure 元件全9／52 assertions green；Nuxt 元件全8 green。Pascal完整現況Spec無finding；Hypatia Standards hard0、命名P3非阻擋；四檔維持凍結 | [交付](../../scratch/tour-attempt/round-publication-ui-implementation.md)、[最新Spec](../../scratch/tour-attempt/delivery-wave-i-round-ui-spec-review.md)、[原Spec](../../scratch/tour-attempt/delivery-wave-g-round-ui-spec-review.md)、[Standards](../../scratch/tour-attempt/delivery-wave-g-round-ui-standards-review.md)；只放行此獨立元件，頁面另做case-first，不改父多階段contract補UI |
| 輪次公告頁面接線／Goodall→Heisenberg→Ohm→Socrates | 限定PASS：page642／Parenttest736／child Vue 及三相依檔六來源未變；35元件/pure green。Main完整讀修後兩軸：Spec0／原P2閉合；Standards hard0／兩個P3非阻擋 | [identity／ABA](../../scratch/tour-attempt/round-publication-parent-identity-guards.md)、[新case／修復](../../scratch/tour-attempt/round-publication-receipt-projection-repair.md)、[修後Standards](../../scratch/tour-attempt/round-publication-receipt-final-standards-review.md)、[修後Spec](../../scratch/tour-attempt/round-publication-receipt-final-spec-review.md)、[集中實證](../../scratch/tour-attempt/main-competition-2026-10-09.md)。只放行公告父頁／收據投影隔離，不外推整頁所有流程或端到端PASS |
| ≥2 隊階段 TS 契約／adapter／Hypatia | 限定PASS：新12green，翻註解後full40／6既有skip／523green。Goodall獨立Spec原2P2已修／新增0，Pascal Standards硬性0／2heuristic P3非阻擋；末四來源一致並凍結 | [交付](../../scratch/tour-attempt/qualified-stage-ts-bridge-implementation.md)、[Spec](../../scratch/tour-attempt/delivery-wave-h-stage-ts-spec-review.md)、[正確Standards](../../scratch/tour-attempt/delivery-wave-i-stage-ts-standards-review.md)。只證fake-WASM傳輸邊界；actual DO／HTTP／WASM／static type仍未驗。持久化／授權相依父DO owner及關卡，不逕開第二parent writer |
| 主時間0不限時／Pascal→C1修復 | 限定PASS：本輪policy11＋lifecycle35 `1aa286`、lib12 `d6c9f4`，58個不重複元件green。修後完整兩軸審核同一範圍，Spec0／C1閉合，Standards hard0／predicate命名P3非阻擋。三檔凍結；可開始相依有限主時間choice政策切片 | [交付](../../scratch/tour-attempt/clock-zero-unlimited-policy.md)、[末Standards](../../scratch/tour-attempt/clock-zero-unlimited-final-standards-review.md)、[末Spec](../../scratch/tour-attempt/clock-zero-unlimited-final-spec-review.md)、[集中紀錄](../../scratch/tour-attempt/main-competition-2026-10-09.md)。finite choice共享扣時、simultaneous及DTO／WASM／DO另HOLD；元件green不代表whole時計PASS |
| 有限主時間他隊choice／Chandrasekhar→獨立末審 | 限定PASS：lifecycle ca0872來源未變。60distinct元件green，Main完整讀審核同一範圍末Standards hard0／P3非阻擋及Spec0／F1閉合；三檔凍結 | [首片](../../scratch/tour-attempt/clock-finite-choice-main-first.md)、[P2 case／修復](../../scratch/tour-attempt/clock-paused-choice-display-repair.md)、[末Standards](../../scratch/tour-attempt/clock-paused-choice-final-standards-review.md)、[末Spec](../../scratch/tour-attempt/clock-paused-choice-final-spec-review.md)、[集中紀錄](../../scratch/tour-attempt/main-competition-2026-10-09.md)。同隊共享／simultaneous／DTO／DO不外推已完成 |
| 同隊隊友choice路由／Chandrasekhar→獨立末審 | 限定PASS：／testa994來源未變，48元件green。Main全文核末Standards hard0／typed fixture重複P3非阻擋及Spec0，各再核兩來源；完成兩reviewers已回收，兩產品檔凍結 | [case／紀錄](../../scratch/tour-attempt/same-team-choice-clock-routing.md)、[末Standards](../../scratch/tour-attempt/same-team-choice-routing-standards-review.md)、[末Spec](../../scratch/tour-attempt/same-team-choice-routing-spec-review.md)、[集中紀錄](../../scratch/tour-attempt/main-competition-2026-10-09.md)。只放行同隊路由／他隊choice恢復同Action，不外推canonical Rules／DO／native接線或全部時計矩陣 |
| 單敗決賽雙負／Newton→Socrates F1修復 | 限定PASS：49distinct元件green。Main全文讀末Standards `623b06` hard0／既有P3非阻擋與Spec `d49e97` 0／F1閉合，各再核來源未變。放行相依SE空位與UI切片，後續變更需另行審核 | [F1 case／修復](../../scratch/tour-attempt/tournament-final-vacancy-rank-repair.md)、[末Standards](../../scratch/tour-attempt/tournament-final-vacancy-rank-final-standards-review.md)、[末Spec](../../scratch/tour-attempt/tournament-final-vacancy-rank-final-spec-review.md)、[集中紀錄](../../scratch/tour-attempt/main-competition-2026-10-09.md)。不造冠亞軍但保留統計，不擅定DE／其他未知名次；current serde不是歷史migration |
| 單敗非final空位／Socrates P1窄修 | ：exact `bf070b` green；policy5+lifecycle16 `63e3ce`、lib30 `920bb9`，共51distinct。Main完整讀最小predicate／ownlog及來源內容；Socrates交付已回收 | [單空位](../../scratch/tour-attempt/tournament-single-vacancy-progression.md)、[P1新case／修復](../../scratch/tour-attempt/tournament-prior-vacancy-final-double-loss.md)、[原Standards／policy發現](../../scratch/tour-attempt/tournament-single-vacancy-final-standards-review.md)、[原Spec P1](../../scratch/tour-attempt/tournament-single-vacancy-final-spec-review.md)、[集中派工](../../scratch/tour-attempt/main-competition-2026-10-09.md)。current末審HOLD；獨立RR切片序列修改同core後再審完整現況，不以51green外推Swiss／knownempty資格／twoempty |
| 循環雙負退出／Newton Active更正窄修 | 原改判 tracer `7c0a62` 真 red 後，來源修正。Main補跑 `cargo test --test tournament_double_loss_exit_policy`，`c91e03` actual2green：原雙負退出與Active改判恢復；2026-10-09再核兩來源不變 | [case／紀錄](../../scratch/tour-attempt/tournament-round-robin-double-loss-exit.md)、[集中派工](../../scratch/tour-attempt/main-competition-2026-10-09.md)。不以兩例升格完整core PASS。未改判的雙負退出屬自動例外；改判恢復的擴充邊界列延後人工更正，既有修補保留且納入核心審核；末兩軸HOLD |
| 瑞士雙負退出／Socrates case→等core移交 | testacfb357單6隊／2輪真DL tracer全430行及ownlog已讀； actualexact `b3d147` exit101真236target DL隊再被配入，合法首輪3組true start/results/stats先過。作者已交付回收，無core寫權 | [case／紀錄](../../scratch/tour-attempt/tournament-swiss-double-loss-exit.md)。等RR唯一writerfreeze／Main回歸後才授Swiss runtime；不能拿既有1輪DL積分green當退出後輪PASS |
| 決賽冠軍空缺UI／Popper P2修復→獨立末審 | 限定PASS：／helperdef7e／pure2073／Nuxt e5f070，17distinctgreen。Main完整讀修後Standards `1ffa9b` hard0／fixture重複非阻擋與Spec `01bc7c` 0／原P2 closed，再fresh4檔案 `0888f2`一致；reviewers已回收，四檔凍結 | [原case](../../scratch/tour-attempt/tournament-final-vacancy-ui.md)、[P2 case／修復](../../scratch/tour-attempt/tournament-view-zero-main-display.md)、[末Standards](../../scratch/tour-attempt/tournament-view-zero-main-final-standards-review.md)、[末Spec](../../scratch/tour-attempt/tournament-view-zero-main-final-spec-review.md)、[集中派工](../../scratch/tour-attempt/main-competition-2026-10-09.md)。只main設定0→不限時及真final空缺呈現，genericduration0／finite等控制保留；不外推native/全矩陣 |
| 有限choice Rust JSON contract／Chandrasekhar→Newton舊oracle對齊 | 限定PASS：，20distinctgreen。Main全讀末Standards `3a7292` hard0／既有P3非阻擋及獨立末Spec `f1fa2f` 0，各再核來源內容；兩檔凍結，reviewers完成已回收 | [新tracer](../../scratch/tour-attempt/clock-finite-choice-json-contract.md)、[choice/C1對齊](../../scratch/tour-attempt/clock-json-legacy-oracle-alignment.md)、[末Standards](../../scratch/tour-attempt/clock-json-contract-final-standards-review.md)、[末Spec](../../scratch/tour-attempt/clock-json-contract-final-spec-review.md)、[集中紀錄](../../scratch/tour-attempt/main-competition-2026-10-09.md)。真handle_request_json／camelCase／mutation view與readonly display分層；不外推freshWASM／DO或全部時計矩陣 |
| 網站分鐘主時間／固定讀秒／Pascal→獨立審核 | 限定PASS：四來源未變，pure 元件全12／113green `184fe1`；Nuxt 元件完整22green `568dda`。Standards hard0／選項集合重複P3非阻擋；Spec無P1/P2／摘要單位P3非阻擋。四檔凍結 | [交付／失敗](../../scratch/tour-attempt/clock-settings-ui-policy.md)、[Standards](../../scratch/tour-attempt/clock-settings-ui-standards-review.md)、[Spec](../../scratch/tour-attempt/clock-settings-ui-spec-review.md)。只證分鐘／固定讀秒及既有毫秒wire；核心C1、後端時計／頁面／WASM／native仍未放行，兩項P3留作維護追蹤 |
| 後端開始時間／原Laplace派工待接手 | 原代理受用量限制，沒有交付新分鐘測例或產品修復。正常逐輪改為自動推進後，先在真正backend接受點驗證自動產生T的分鐘精度與完整五分鐘窗口；原手動公告測例計畫不直接當作新自動流程的驗收 | 尚未實作／驗證；產品及fixture須協調唯一owner，不沿用「已派工」當進度。既有binding在建房前固定T，確立T與實際就緒的銜接須先做case-first切片，不以通知時間或瀏覽器時間補造權威 |

接續派工前重新確認代理狀態與檔案所有權，不把舊派工視為仍在實作。當前相關核心維持凍結；01 父 initial receiver 以本票限定 PASS（Preparing 測例及 mock 範圍見上列與交付紀錄），native／fresh Rules Engine 全流程留待 22；分鐘政策尚無交付。優先按票據索引 P0／P1 安排唯一 writer，Swiss 自動例外再序列接手，Active 改判新功能延後。Main 獨占關卡／索引與 runner；作者不末自審，既有 dirty 與限定 PASS 成果保留。

工作紀錄在 `scratch/tour-attempt`，原始報告保留當時測試結果、範圍與限制；待審交付須依 Git 基準閱讀完整原始碼、測試與差異，不能以表中摘要放行。歷史 PASS 只涵蓋已驗證的內容與切片。

## 接續實作與放行

交付範圍、逐票驗收、優先級及直接功能相依只見[正式票據索引](tickets/README.md)。本節保留來源核對與放行限制，不再維護第二份待辦細分；票據發布不代表新關卡通過。

### 正常完賽主線

按票據 P0／P1 接續。14 的排序政策阻擋已解除，限定主辦入口不依賴一般裁判介面；相關公告、排名、資格與總驗收仍未完成，不因政策確認或發布票據放行，詳見 [14](tickets/14-qualification-and-podium-ranking.md) 及[設計](competitive-play-design.md#晉級與前三名的排序判定)。未受影響的切片仍可獨立進行。

2026-10-10 排序訪談唯讀核對：現有賽程排名仍使用舊比較鍵，瑞士合法非零敗分的累計亦有下述待修正事項。取消血量／耗時排序後，原本終局血量、終局主時間回算與平均樣本的調查不再是 14 的實作或放行前置；不新增排名用帳本。這不移除真對局的權威時計、超時判定或 09 的淘汰和局／時計接續關卡。此為來源核對與政策同步，不是元件驗證或放行；本輪沒有程式修改或測試執行。

瑞士積分唯讀核對：契約有勝／和／敗／輪空四個非負整數欄位，但主積分累計未套用非零敗場積分；記入 [13](tickets/13-swiss-rounds-and-scoring.md) 的待修正範圍。賽前設定與公布值不能不同於實際累計，也不把原對手勝率設定當成新勝分／負分排序。

2026-10-09通知鏈核對：一般GameRoom的joinGame會經afterRoomMutation通知成員；createCompetitionGame直接寫入固定成員，create／release目前沒有呼叫該通知鏈，因此還要補觸發點，不能把已有通知通道當成競賽已接線。PlayerNotifications目前只推給在線socket，未收到推播者仍由賽事頁取得入口；本片不另建離線訊息系統。手動公告元件的歷史限定PASS保留，但不代表自動推進PASS。

### 自動例外收尾

P2 的 [17–21](tickets/README.md#逐票入口與功能相依) 是正式辦賽必需，不依賴一般裁判介面。[19](tickets/19-swiss-double-loss-exit.md) 的瑞士後輪重新配入缺陷已有真 red，尚未修復；既有單敗／循環仍只按各自已驗範圍計。

[17](tickets/17-automatic-no-show-rulings.md) **限定 PASS**：2026-10-11 [本輪兩軸報告與原始證據](../../scratch/ticket17-ready-endpoints-review/report.md)確認先前截止快照／即時開局資格、舊準備端點旁路 P1 與 noShow 後斷線寫入 P2 全部閉合。Standards 硬性 0／重複 request 建構非阻擋建議 1，Spec 0；主審精確路徑 child 131／0、parent 349／0、五檔 helpers 106／0、Nuxt 元件 10／0、獨立固定時間 6／0、typecheck 與 diff check 通過。排除 19 的提交快照重新建置 Cargo 733 pass／1 fail，唯一失敗是 19 的已提交 Swiss 退出紅例；原始基準同例亦 0／1，非 17 回歸，三個 NoShow 新 Rust 案例全綠。分離後 view 測試 9／0。17 四項驗收勾選；只提交 17，不帶入 19 或 AGENTS.md 修改，不外推真 Worker／Brave／22／28 整合驗收。

[20](tickets/20-double-loss-vacancy-completion.md) 維持一票，名次空缺政策已確認、needs-info 阻擋解除；當前雙敗阻擋路徑的修復與本票完整現況／元件實證仍未完成，不以政策確認代替放行。產品取值只見[設計](competitive-play-design.md#雙負空缺與後續階段)。

### 延後功能

P3 範圍及相依只見[票據索引](tickets/README.md#優先級與首場界線)，不作 22 的前置；既有實作與測例保留，基本授權、順序與防重複不能跟著延後。

[25](tickets/25-same-slot-rematch.md) 的重賽政策已確認、needs-info 阻擋解除，本輪未實作或驗證重賽；產品取值只見[設計](competitive-play-design.md#競賽重賽)，不影響首場／P3 的界線。

### 首場賽事驗收

[22](tickets/22-first-event-integration-validation.md) 的 01–21 前置全部實作並通過指定元件／純模組關卡後，才由主審集中執行 Cargo、WASM／跨模組、專案型別與打包、真 Worker／Brave／端到端驗收。驗收報告明列賽制、參賽型態、規則與階段覆蓋範圍，不能把單一 RR 1v1 成功外推完整矩陣。具體分支只在 22 維護；沒有實證或相關案件／名次仍未決時，不得宣告正式辦賽範圍通過。

## 歷史能力與證據

報名／個人牌組、固定玩家入口／準備、目前賽程 GET／單階段管理、更正 HTTP／UI、權威時計顯示與暫停恢復已有歷史限定驗收；不重複派工。原生鏈多數只證 RR 1v1；2v2 牌組快照另有獨立證據，均非全賽制／重啟矩陣。

查某次結果時才讀[驗證歷史索引](evidence/README.md)或 [tour-attempt 對應報告](../../scratch/tour-attempt/README.md)。它們記錄歷史事實，不是現行派工入口。
