# 22：正式辦賽範圍的集中總驗收

**Priority:** P2

**What to build:** 在本次必要功能元件關卡全部通過後，集中驗證新 WASM、型別／打包與真 Worker／Brave 全流程，確認四賽制、個人／團體及多階段的實際覆蓋。玩家能從報名截止到整場結束走完宣告範圍，不能把 mock 或單一循環個人賽成功外推所有分支。

**Blocked by:** [09：淘汰和局判定與時計沿用加賽](09-elimination-draw-continuation.md)、[13：瑞士多輪自動配對與排名](13-swiss-rounds-and-scoring.md)、[14：晉級與前三名的排序判定](14-qualification-and-podium-ranking.md)、[21：零隊／一隊後續階段與整場閉合](21-zero-one-team-stage-completion.md)

**Status:** ready-for-agent

**類型：** 集中整合驗收

**排序驗收：** 覆蓋自身積分高優先、同分才比勝分多、負分少，以及全同等待／接收主辦限定判斷、公開理由及利益衝突。核對瑞士自訂積分、未開打缺席配對的對手排除、中途只顯示目前自身勝場，以及血量／耗時不影響名次；不再要求排名樣本或平均值測試。驗證符合瑞士規則的平手配對使用固定保存的抽籤種子、重試不重抽且不決定正式名次，不自動產生排名同分加賽。主辦未判斷時不得猜出資格或宣布整場完成。09 仍須獨立驗收，不因移除排名加賽而刪除。

## 驗收條件

- [ ] 01–21 必要元件／純模組及指定完整現況審核皆通過後才集中總驗收；needs-info 或未決結果不能總放行。
- [x] 使用新建 WASM、專案型別／打包及真 Worker／Brave 從 F 到整場結束驗證，不以舊產物、fake bridge 或單一循環個人賽替代。本輪由專用多 session runner 實際完成四個階段的 1v1 賽事；完整賽制／例外矩陣見第 3–5 項。
- [ ] 報告明列四賽制、個人／團體、必要準備選牌規則與多階段的覆蓋，包含普通下一局、淘汰 Draw、季軍及雙敗重置賽。
- [ ] 涵蓋單隊／雙隊缺席、先後／同時超時、退出／空缺、零／一隊及空缺與未決名次的區別；未決不得晉級／公開，明確空缺按政策收尾，不強求三個得獎隊伍。
- [ ] 核對所有排序項、對手集合、限定主辦判斷及瑞士固定抽籤配對；賽中勝場與賽後公開結果一致。
- [x] 正常無爭議路徑從凍結、抽籤、逐輪開房至結束不需主辦介入；本輪在開放報名後關閉主辦 session，由玩家 session 自動走到結束。全同比較的主辦判斷與異常處置另列例外。
- [x] 主審集中一次一個程序，記錄實際命令、產物與結果；P3 及進階故障矩陣不作首場前置。

## 本次整合結果（2026-10-16）

**整票判定：HOLD。** 第 2、6、7 項在下述受限範圍內通過；第 1、3–5 項仍未勾選。真實 Worker／Brave 多 session 路徑已實際完成四階段的個人 1v1 賽事，不外推為團體賽及例外矩陣已放行。

### 已驗證路徑與命令

- `cargo test`：通過。`cargo fmt --all -- --check`：通過。
- `cd apps/web && bun run typecheck`：通過。E2E 啟動本地伺服器時執行 `bun run build:rules-wasm && nuxt build`；新建 Worker WASM SHA-256 為 `163f5e120cfb71e54771a848476d040dbdba44a44d86dca091cf9ad2ba0a8ffd`。
- `cd apps/web && bun test worker/durable-objects/competition-game-room.test.ts`：145 通過、0 失敗。`bun test worker/durable-objects/competition-room.test.ts`：389 通過、0 失敗。
- `cd apps/web && bun run test:unit`：954 通過、6 略過、0 失敗（共 960 測例）。
- `git diff --check`：通過。
- 從 `apps/web` 執行 `FEWFC_E2E_PORT=8798 FEWFC_E2E_INSPECTOR_PORT=9298 FEWC_E2E_PRESERVE_FAILURE=1 bun run test:ticket22:multisession`：Playwright 多 session 驗收 1/1 通過，測例 23.2 分鐘、總耗時 23.7 分鐘。本機 `.env` 的 Brave 執行檔設定有效；由主辦建立賽事並開放報名後立即關閉主辦 context，兩個玩家 context 從報名一路完成賽事，最後另用匿名 context 讀取完成公告與重播。隔離 Worker 以測試專用設定將登記回應初始血量覆寫成 20，正式規則與非 development Worker 不受影響。完整命令、覆寫邊界與產物記錄見[本輪紀錄](../../../scratch/tour-attempt/ticket22-first-event-2026-10-16/report.md)。

此多 session 流程設定 `teamSize: 1`、要求個人牌組快照，使用 base rules 及六個必要規則模組。循環賽的 1v1 Match 完成固定三局，含第 2、3 局由敗方隊長選擇先手並建立普通下一局；Swiss、單敗、雙敗均有實戰配對並完成。每一場入口發布後，玩家 session 都會按下「準備」再繼續對局；沒有安排在入口發布後等到 T 才開打。階段間仍依產品時序於前段結果時刻 E 後約 5 分鐘 A 發布入口（A=E+5 分鐘）。雙敗實際推進到第 2 輪並自動完成整場。

### 尚未通過的範圍

本輪沒有實際驗證 2v2／團體對局、多隊 Swiss 配對與完整排名平手矩陣、淘汰 Draw 及時計沿用、單敗季軍賽、雙敗總決賽的重置賽、缺席／讀秒耗盡／退出／空缺分支、零隊與未決名次的區別。因此驗收條件 1、3–5 維持未勾選；Rust／Web 元件測例不替代這些真實整合路徑。

本輪 Standards／Spec 兩軸差異審查未發現文件規範硬性違反或明確錯誤實作；Spec 審查確認上述矩陣仍缺。Standards 僅提出一項低嚴重度判斷建議：`game-room.ts` 多處重複判斷 `gameNumber === 1`，可抽成明確 helper；此建議不阻擋驗收。Ticket 22 尚未完成整體放行。

共用驗收與工作規則見[票據索引](README.md#共用驗收與工作規則)；產品政策見[競賽設計](../competitive-play-design.md)，現況與放行證據見[審核關卡](../competition-review-gates.md)。
