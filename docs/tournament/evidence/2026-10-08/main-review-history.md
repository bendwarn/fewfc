# 2026-10-08 競賽功能接續

## 最新執行邊界

使用者要求繼續剩餘功能，先只做元件測試，全部功能完成後才總測試。中央只執行必要的新元件／純模組案例；暫不執行跨模組整合、WASM 建置／接線驗收、全專案型別／建置、原生 Worker／Brave、端到端或全套 Cargo。代理只 author 案例，不自行啟動驗證；生命值專項與舊資料相容不做。

已通過的歷史結果仍只證原 版本 與原範圍，不因恢復工作當成新功能已通過；不為維持活動重跑原 16／22 案例。完整現況兩軸審核仍是下游實作前置，未完成的最終整合不冒作 PASS。

## 恢復與實際失敗路徑

- 對 Maxwell 舊 ID 直接 `send_input` 回 agent not found，原串行六項派工在第一项失敗，不能聲稱六人都已收到。
- 改用 `resume_agent` 成功恢復 Maxwell，另五位依序恢復後，六個新提示皆取得 submission_id。不能把 Oct5 額度錯誤冒作 Oct8 仍不可工作的證據；等待實際新回報。
- `rg src/competition_tournament.rs` 失敗：該路徑不存在。既有賽程在 `src/tournament.rs`，改用正確來源，未新增第二份賽程模型。
- 合併過大的唯讀輸出超過工具預算而截斷；不作全文已讀證據，後續採有限行區段與較小輸出。未執行產品測試，沒有本輪 red／green。

## 本輪互斥派工

- Maxwell：只補完 `src/competition_registration.rs`／`tests/competition_registration_lifecycle.rs` 的首輪截止凍結、排除原因及元件案例；不碰其他 Rust／TS／DO。
- Einstein、Linnaeus：分別 child local Standards／Spec，只寫自己的審核報告，不改 frozen 四檔。
- Lagrange、Dirac：分別 parent P0a Standards／Spec，只寫自己的報告，不改 frozen 四檔。
- Jason：P0a 等審核，先在自己的 tmp 收斂下一個 ACK／release slice 與元件邊界，不默改 frozen wire／產品。

首輪人類已決定 F＝報名截止，所有資料同時不可再改；未齊者排除並明示安全原因；隊長資料確認與房間 connected／ready 分離。不重問上述決策，不自動延期，不猜淘汰 DoubleLoss 名次。

## 恢復時快照

- Rust registration：；lifecycle：。截止已改、public 理由欄位／enum 尚未補齊，屬 WIP，非可編譯或已測聲明。
- child source：；test：。歷史 16 green，兩軸尚待新報告；initial FIFO HOLD。
- parent source：；test：。歷史 22 green 依舊 WASM，不證新 Rust freeze 或 ACK／release 完成。

未 commit／push／部署、未刪除資料，所有其他 dirty 保留。

## 本輪首輪凍結元件實證

Maxwell交付 source ，lifecycle 不變。中央只跑 `cargo test --lib competition_registration::tests::opening_freeze_`：1abc88啟動／031f54完成，**7 passed／0 failed／166 filtered out、exit0**。沒有執行 lifecycle integration、HP resolver案例、WASM／全套測試。末SHA直接核 b5813c吻合。

七個案例覆蓋三個新截止action的F−1／F／after、required公開理由與private邊界、Lock角色／不得提前、晚收據不倒退Locked、disabled／withdrawn理由矩陣、換名單與重交後重確認、損壞CheckedIn資料失敗不留成功收據／不半分類其他隊伍。只證Rust元件，不證at-F牆鐘觀測或網站全部提前開房。

R1已交Einstein Standards／Dirac Spec有限現況審核；新的TS公共parser／UI及父權威提交仍HOLD。child型別窄修source ，其餘3SHA未變；原生storage Pick與transaction/alarm能力分離，9個double cast移除，無運行語意或cases變更。Einstein終核P3待件，原16不重跑。

## parent P2 修補元件实证

Jason交付source ／test ，shared兩檔原SHA不變。主審讀全部新receipt分支、original lookup、private parser與18-case區段及owner紀錄。

中央只跑 `bun test apps/web/worker/durable-objects/competition-room.test.ts --test-name-pattern 'round-release parent receipt module'`：7825f9啟動／9af36b完成，**18 pass／0 fail／264 filtered／51 expect calls、exit0**。保存層fixture明標已持久模組狀態，engine/random/foreign能力一呼就throw，公開DO重試是被測者；不是假造全程ACK／GameStarted證據。沒有呼叫WASM或真房間，原22不執行。

新增private manifest required `announcementCommandId`隨原交易保存，查原command恰一歷史header（包含retired），核完整原generation/T/房間身分與順序，不用mutable current回填。不增加公共DTO、SQL表/版本、舊資料fallback或新的勝負政策。錯誤回安全固定碼且無寫入／交易／engine/platform/random/alarm變化。Lagrange Standards、Dirac Spec有限終核待件，P0a下游仍未放行。

## 元件關卡後互斥相依

- R1兩軸report ／已主審全文核，0 findings、末產品SHA一致，限定gate PASS。Maxwell轉唯一frontend registration schema／ownTeam鏡像／資料期限／排除文案及Nuxt元件工作；Rust54bb/d471凍結。
- child P3closure report 已核，0 findings，未動運行語意的type delta與原Spec0相容。Lagrange於有限parentStandards終核後，續唯一child4檔 initial FIFO／strict ACK／cursor／remote lane接線；shared wire已過才准import，不證父receiver存在。
- parent修補末兩軸report ／已核：Standards0／原P2closed且Spec0，末4SHA同freeze。结合本轮18组件结果，限定P0a PASS；已交Jason唯一parent4续claims／strict Waiting ACK／父实际C cutoff／durable release／singlealarm。initialS-A/P8、jointcorrection与整链验收仍後续，不能提前叫Timing完成。
- stage研究92行／版本 已主審全文核。已完成且無產品owner的Linnaeus正常close，沒有中止活躍工作或删除記錄。新Descartes `01a11748-0ae5-7de1-b6b9-902b882180a8`只擁有src/tournament.rs的純資格解析與unit cases，one readonly Interface、明確Finished winner或RR/Swiss unique standing rank→explicit seed；不建Stage或改Tournament schema、不猜淘汰低名次/遞補/並列，不接I/O／WASM。其他研究/產品dirty保留。

## 新公告HTTP切片

已完成且目前無產品ownership的Einstein正常close，保留審核報告，不中止任何活躍實作。新Mencius `01a11750-023c-7341-a424-2de52523aacb`以無model／effort override的spawn建立，只擁有新 `competition-round-publication` helper／test與 `/stages/[stageId]/rounds/[roundNumber]/publication` GET／POST及各自tests。SQLite唯讀核新代理實際模型／effort符合現有設定，沒有修改設定檔或在此重複管理預設值。

主審先核父P0a有限PASS與現有exact9 request／exact5 receipt／exact3 readonly DTO、capability seam與HTTP convention，再明定path selector＋POST exact4 body。身份与server time只能伺服器補入，GET不取時間、不帶body或query。舊HTTPhelper與shared維持freeze，actual claims／ACK／release由Jason權威實作，不在HTTP做第2演算法或默認T調整。代理僅author新元件案例，不执行；主審續其餘非重疊整合盤點。

## 文末政策訪談與事實盤點

使用者明示 grill-with-docs。主審完整讀 grilling／domain-modeling，先同步已確認的F凍結與排除原因，不重問已決的同時讀秒耗盡雙負／RR基本積分。當前只更新政策文件與已解析的詞彙，不在未完成訪談前改相關產品。

實際派工失敗：對原Dirac直接send回not found；首次新fact-agent spawn遭thread limit。正常close已完成的Descartes（沒有中止實作／刪資料），再以無model／effort override的新Peirce成功收斂純讀fact。這些是協調失敗，不是產品red。Descartes的stage-qualification與Mencius HTTP交付已收到，仍待元件執行及兩軸審核，不以completed通知當gate PASS。

Peirce自有 `competition-open-decisions-facts.md` 原稿及時間單位附節已主審全文讀。其他時間輸入仍是任意秒、開賽datetime細至ms、Rust／DTO零與null不同，皆為source事實而非政策批准。回合外限時獨立於主時間，因此主時間0的不限時是否涵蓋所有視窗、是否共用單一讀秒設定，須釐清，不默認沿舊值。

新批准政策已記design／glossary：賽後公開完整鎖定牌組；系統核全部公告階段與結果／未決案件後自動整場結束；主時間輸入0＝不限時、讀秒僅30／60／180秒；同時進讀秒Draw與不限時Draw都加賽；時間判負後普通局由競賽敗方隊長選先手；RR空輪0分且不算勝負。Q9淘汰DoubleLoss進位與Q10公開後補賽只得到追問，尚未批准；不能按「其他照建議」把這兩題一起結案。未執行任何新測試／build／總驗收／commit／push／部署。

後續human已確認：其他設定用分鐘、開始時間分鐘精度，淘汰前輪完成後公告下一輪T且仍提前五分鐘開房；雙方未按時報到皆負；初版不重開已結束賽程。實際超時與缺席的reason仍分開，不能因結論都是雙負抹去真GameStarted／時計證據。雙負空位與決賽名次尚未定，亦不以既有四制mock測試冒作網站已支援。Q12只得到『哪裡用到回合外選擇』追問，Peirce續唯讀查真pending-choice producer與扣時計receiver，不擅自把此題當批准合併欄位／不限時計。

Peirce回合外附節已主審全文讀，另直接核rule的裂地崩山段、實際tribulation producer的指定player／一張合法Card choice與共同時計分流。證實不是純預留能力：A回合裂地可依序要求B及同隊C答，current仍A；當回合TurnDraw則為反例。現行helper按player不同而非team不同分類，core out-of-turn只扣獨立額度，未先扣主時間。這是source fact，不批准該計時政策；補glossary與具體例，時鐘修正仍待user確認，未執行測試或改產品。

最新human「皆同意」已批准Q12／14／15／16：他隊必答選擇先扣作答隊主時間、耗盡再用該選擇的獨立讀秒；同隊隊友沿用原行動回合額度，不給新讀秒；同階段一種30／60／180秒讀秒、不另設回合外欄位、主時間輸入0涵蓋全部視窗不限時。DoubleLoss兩隊退出當前Stage，雙敗亦不落敗部；真正決賽雙負明示冠軍空缺，不造冠亞軍或並列。賽後可保留原紀錄及原因更正賽果／名次，但首版拒絕必須新增比賽或重賽的更正，不重開完成賽程。同步design及現有詞彙，不更改產品或執行測試；一般空缺傳播與後續階段不足的處理尚需human決定，最後共享理解仍待確認。

接續human「皆同意」已批准Q17／18：既有淘汰空位由另一合法隊伍輪空晉級，雙空位繼續傳播空缺，不遞補／復活／造Game勝局或紀錄。尚未開始的已公告後續Stage於前階段及資格確定後，至少2隊按實際資格與原seed來源進行；1隊直接為Stage勝者而不造比賽；0隊明示無合格參賽者、記取消原因且無winner。明確取消須納入整場完成處置，不能默默略過尚未建立的Stage；未決賽果／裁判案件不當作0隊。design、glossary及gate的新政策提示同步，文末訪談frontier已空，依grill-with-docs只待最後整份共識確認後啟動相依產品實作。本輪只改文件，沒有測試／build／commit／push／部署。
