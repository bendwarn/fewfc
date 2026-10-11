# 競賽功能票據索引

2026-10-10 已依使用者核准發布 28 票，沿用原編號與功能相依；這是本機文件發布，不是派工、實作完成或放行。所有票的 triage role 為 `ready-for-agent`；驗收勾選見逐票文件，當前實作與放行證據見[審核關卡](../competition-review-gates.md)，須按目前版本取得實證。

本目錄是交付範圍、優先級、逐票驗收及 Blocked by 的唯一權威；[競賽設計](../competitive-play-design.md)管理產品政策，[審核關卡](../competition-review-gates.md)管理當前實作、互斥範圍及放行證據，[文件索引](../README.md)提供參考分支。不另在其他 tracker 或 scratch 建立票據副本，不改動既有 parent issue。

## 優先級與首場界線

| 優先級 | 票號 | 數量 | 交付界線 |
| --- | --- | --- | --- |
| P0 | 01–02 | 2 | 先修正既有開局／報到與最新政策的差距 |
| P1 | 03–16 | 14 | 正常自動完賽；排序全項相同時有主辦限定判斷入口 |
| P2 | 17–22 | 6 | 首場必需的自動例外及集中總驗收，不是可選配件 |
| P3 | 23–28 | 6 | 首場後的人工更正及進階恢復，不阻擋 22 |

P1 包含淘汰遊戲和局判定／時計沿用加賽、晉級與前三名排序、季軍賽及雙敗重置賽；不包含排名同分加賽。P2 補齊缺席、真正超時、雙負與資格不足收尾，才能交付完整首場範圍。

優先級只決定工作排序，不是 [triage role](../../agents/triage-labels.md) 或審核 finding 的嚴重度。`ready-for-agent` 表示規格已確認，不代表前置票完成、已派工、可同時寫入或已通過測試；新遇到未涵蓋的決策，先查參考，仍無法決定才詢問，不自訂隱藏預設。

## 逐票入口與功能相依

Blocked by 只列直接且真正阻擋開始的功能票，省略傳遞相依；共用檔案的互斥寫入另協調，不添加假的功能依賴。

| 票號 | 優先級 | 票據 | Blocked by |
| --- | --- | --- | --- |
| 01 | P0 | [準備即報到，全員準備立即開局](01-ready-check-in-start.md) | 無 |
| 02 | P0 | [開局前斷線取消準備與報到](02-disconnect-clears-ready.md) | 無 |
| 03 | P1 | [賽前公告完整競賽方案](03-competition-announcement.md) | 無 |
| 04 | P1 | [報名截止自動鎖定名單與牌組](04-registration-freeze.md) | 無 |
| 05 | P1 | [自動抽籤並建立首輪賽程](05-initial-seeding.md) | [03](03-competition-announcement.md)、[04](04-registration-freeze.md) |
| 06 | P1 | [固定輪次自動開房、發布時間與玩家入口](06-round-rooms-and-notifications.md) | [01](01-ready-check-in-start.md)、[05](05-initial-seeding.md) |
| 07 | P1 | [權威隊伍時計接通真正對局](07-authoritative-game-clock.md) | [01](01-ready-check-in-start.md) |
| 08 | P1 | [普通下一局與敗方隊長先後手選擇](08-next-game-first-player.md) | [07](07-authoritative-game-clock.md) |
| 09 | P1 | [淘汰和局判定與時計沿用加賽](09-elimination-draw-continuation.md) | [08](08-next-game-first-player.md) |
| 10 | P1 | [結果驅動循環與單敗自動下一輪](10-automatic-round-progression.md) | [06](06-round-rooms-and-notifications.md)、[07](07-authoritative-game-clock.md) |
| 11 | P1 | [單敗自動季軍賽與階段收尾](11-single-elimination-bronze.md) | [10](10-automatic-round-progression.md) |
| 12 | P1 | [雙敗勝敗部與總決賽重置賽](12-double-elimination-finals.md) | [10](10-automatic-round-progression.md) |
| 13 | P1 | [瑞士多輪自動配對與排名](13-swiss-rounds-and-scoring.md) | [10](10-automatic-round-progression.md) |
| 14 | P1 | [晉級與前三名的排序判定](14-qualification-and-podium-ranking.md) | [10](10-automatic-round-progression.md) |
| 15 | P1 | [至少兩隊的多階段自動晉級](15-automatic-stage-transition.md) | [10](10-automatic-round-progression.md) |
| 16 | P1 | [整場完成權威與賽後公開](16-completion-and-publication.md) | [10](10-automatic-round-progression.md) |
| 17 | P2 | [報到截止自動處理缺席](17-automatic-no-show-rulings.md) | [02](02-disconnect-clears-ready.md)、[06](06-round-rooms-and-notifications.md) |
| 18 | P2 | [真讀秒耗盡與同時耗盡雙負](18-byoyomi-expiration-rulings.md) | [07](07-authoritative-game-clock.md) |
| 19 | P2 | [修復瑞士雙負隊伍再被配入後輪](19-swiss-double-loss-exit.md) | 無 |
| 20 | P2 | [循環與淘汰雙負、空缺自動收尾](20-double-loss-vacancy-completion.md) | [11](11-single-elimination-bronze.md)、[12](12-double-elimination-finals.md)、[17](17-automatic-no-show-rulings.md)、[18](18-byoyomi-expiration-rulings.md) |
| 21 | P2 | [零隊／一隊後續階段與整場閉合](21-zero-one-team-stage-completion.md) | [15](15-automatic-stage-transition.md)、[16](16-completion-and-publication.md)、[19](19-swiss-double-loss-exit.md)、[20](20-double-loss-vacancy-completion.md) |
| 22 | P2 | [正式辦賽範圍的集中總驗收](22-first-event-integration-validation.md) | [09](09-elimination-draw-continuation.md)、[13](13-swiss-rounds-and-scoring.md)、[14](14-qualification-and-podium-ranking.md)、[21](21-zero-one-team-stage-completion.md) |
| 23 | P3 | [裁判暫停、恢復與判棄權介面](23-referee-clock-and-forfeit.md) | [10](10-automatic-round-progression.md) |
| 24 | P3 | [賽中改判與未開局房間撤銷重建](24-correction-and-waiting-room-rebuild.md) | [10](10-automatic-round-progression.md) |
| 25 | P3 | [重賽與已開打後續交手的處置](25-same-slot-rematch.md) | [23](23-referee-clock-and-forfeit.md)、[24](24-correction-and-waiting-room-rebuild.md) |
| 26 | P3 | [跨階段改判的資格與賽程影響](26-cross-stage-correction.md) | [15](15-automatic-stage-transition.md)、[25](25-same-slot-rematch.md) |
| 27 | P3 | [賽後更正留痕且不重開賽程](27-post-event-correction.md) | [16](16-completion-and-publication.md)、[24](24-correction-and-waiting-room-rebuild.md) |
| 28 | P3 | [跨房故障、撤銷競態與平台重啟矩陣](28-failure-and-restart-matrix.md) | [22](22-first-event-integration-validation.md)、[26](26-cross-stage-correction.md)、[27](27-post-event-correction.md) |

## 可開始的邊界與放行順序

01、02、03、04 沒有功能票前置；19 也能沿既有完整結果入口獨立修復，但按 P2 排序，不把它當成新瑞士自動開房的前置。這只表示相依圖的起點，真正開工仍須核對代理狀態、Git 差異及唯一寫入權；發布票據沒有啟動任何代理或 runner。

每票是可獨立展示或驗證的窄功能路徑，涵蓋所需權威資料、服務／API、網站投影／操作與案例；不按 Rust、API、UI 拆成水平票，也不要求重寫已有能力。必要共同契約先併入 01，不先新增缺乏明確收益的大改構。20 維持一票，以循環、單敗、雙敗的驗收分支證明共用退出規則。

09 依賴 08：淘汰 Draw 若已判出競賽敗方，後續須走普通下一局及隊長選先後手，不能只完成時計沿用加賽。14、15、16 可先用各自窄路徑驗證，不要求等所有賽制接線；14 的限定主辦判斷入口不依賴一般裁判介面，未決資格／名次仍阻擋受影響的推進或完成。

22 的相依閉包涵蓋 01–21，沒有 P3；28 的閉包才涵蓋 01–27。P3 是工作排序上延後，不是讓 22 成為所有人工功能的技術前置。未來執行應沿已完成前置的 frontier 推進，不因編號較小就重做已驗成果。

## 共用驗收與工作規則

以下適用每票，不在 28 個文件複製：

- 保留固定身分、參賽／裁判授權、過期版本拒絕、持久交付及一般重試防重複；需要時驗證 FIFO／版本競態。新多字 Rust Web 動作欄位須有 exact camelCase 序列化契約案例，不以 28 延後基本正確性。
- 01–21、23–27 先做指定元件／純模組案例及指定檔案完整現況的 Standards／Spec 兩軸審核；22 才做首場集中總驗收，28 才做延後的進階故障矩陣。主審是唯一 runner，實作者寫案例，不平行啟動編譯／測試；交付及審核依[Git 審核流程](../../agents/git-review.md)核對差異。
- 開始新功能前依[參考索引](../README.md#功能開始前的參考)查 Bracket 或類似專案的相關分支，評估概念／演算法重用、Rust 翻譯收益與授權；參考不能代替本站產品決定。
- 已通過成果只按其真實範圍重用，不重做核心、不外推其他版本的 PASS。保留不相關 dirty，不加舊賽事相容、初始生命專項測試、天梯或不限範圍的大改構。
- 互斥範圍、當前狀態與實證只記在[審核關卡](../competition-review-gates.md)；失敗路徑及不可行原因記入 [tour-attempt 工作紀錄](../../../scratch/tour-attempt/README.md)，不在票據另建進度／證據副本。

驗收條件未勾選不否定關卡中既有的限定 PASS；它表示本票完整範圍尚未按當前版本驗收。完成通知不是放行，只有指定案例實證、兩軸審核及相符的 Git 差異才可更新本票的完成情況。
