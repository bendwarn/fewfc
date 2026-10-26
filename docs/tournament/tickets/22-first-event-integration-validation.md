# 22：正式辦賽範圍的集中總驗收

**Priority:** P2

**What to build:** 在本次必要功能元件關卡全部通過後，集中驗證新 WASM、型別／打包與真 Worker／Brave 全流程，確認四賽制、個人／團體及多階段的實際覆蓋。玩家能從報名截止到整場結束走完宣告範圍，不能把 mock 或單一循環個人賽成功外推所有分支。

**Blocked by:** [09：淘汰和局判定與時計沿用加賽](09-elimination-draw-continuation.md)、[13：瑞士多輪自動配對與排名](13-swiss-rounds-and-scoring.md)、[14：晉級與前三名的排序判定](14-qualification-and-podium-ranking.md)、[21：零隊／一隊後續階段與整場閉合](21-zero-one-team-stage-completion.md)

**Status:** ready-for-agent

**類型：** 集中整合驗收

**排序驗收：** 覆蓋自身積分高優先、同分才比勝分多、負分少，以及全同等待／接收主辦限定判斷、公開理由及利益衝突。核對瑞士自訂積分、未開打缺席配對的對手排除、中途只顯示目前自身勝場，以及血量／耗時不影響名次；不再要求排名樣本或平均值測試。驗證符合瑞士規則的平手配對使用固定保存的抽籤種子、重試不重抽且不決定正式名次，不自動產生排名同分加賽。主辦未判斷時不得猜出資格或宣布整場完成。09 仍須獨立驗收，不因移除排名加賽而刪除。

## 驗收條件

- [ ] 01–21 必要元件／純模組及指定完整現況審核皆通過後才集中總驗收；needs-info 或未決結果不能總放行。
- [ ] 使用新建 WASM、專案型別／打包及真 Worker／瀏覽器從 F 到整場結束驗證，不以舊產物、fake bridge 或單一循環個人賽替代。2026-10-26 以 `1339f7f` 為基底已重跑真 Worker／Chromium 正常 1v1 四階段流程；不等於 Brave 或完整情境矩陣通過，詳見本候選紀錄。
- [ ] 報告明列四賽制、個人／團體、必要準備選牌規則與多階段的覆蓋，包含普通下一局、淘汰 Draw、季軍及雙敗重置賽。
- [ ] 涵蓋單隊／雙隊缺席、先後／同時超時、退出／空缺、零／一隊及空缺與未決名次的區別；未決不得晉級／公開，明確空缺按政策收尾，不強求三個得獎隊伍。
- [ ] 核對所有排序項、對手集合、限定主辦判斷及瑞士固定抽籤配對；賽中勝場與賽後公開結果一致。
- [ ] 正常無爭議路徑從凍結、抽籤、逐輪開房至結束不需主辦介入；2026-10-26 以 `1339f7f` 為基底的真 Worker／Chromium 多 session 流程再次在開放報名後關閉主辦 session，由玩家 session 自動完成四階段。此為 1v1 正常路徑，不能代表其餘矩陣。
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

## 目前 HEAD 重驗（2026-10-25）

**整票仍為 HOLD。** 本輪檢查的是 `7e984048cea67f5ba8866ed21d69f3be5e17edef`（`Refactor game rules and improve web interactions`）。該提交變更了競賽房間 API、GameRoom 與即時同步，因此 10-16 的瀏覽器全流程只保留為舊版歷史證據；本輪不把它外推為目前 HEAD 已驗收。

### 通過的目前版本檢查

- `cargo test -- --quiet`：778 通過、0 失敗；`cargo fmt --all -- --check`：通過。
- 從 `apps/web` 執行 `bun run test:unit`：965 通過、6 略過、0 失敗；`bun run test:integration`：232 通過、0 失敗；`bun run test:component`：159 通過、0 失敗；`bun run typecheck`：通過。
- 從 `apps/web` 執行 `bun run build`：新建 WASM 與 `target/wasm32-unknown-unknown/release/fewfc.wasm` SHA-256 相同，均為 `7c3525bd51a9378ea1f34b81dcb59f2d246ccce34548dd516df9dd288d65ca9a`；Nuxt production build 通過。
- Rust 測例涵蓋淘汰 Draw 延續、單敗季軍、雙敗重置、雙負／空位與瑞士多輪；Web Worker／伺服器測例涵蓋 2v2 固定名單／準備、缺席、時計逾時、裁決、空缺與排名等元件分支。這些是目前來源的測例證據，並非真瀏覽器端到端紀錄。
- Storybook 靜態建置及 Codex 瀏覽器中的準備控制普通房狀態檢視通過。`bun run test:storybook` 因 Watchpack `EMFILE`、無法寫入使用者目錄的 Storybook 設定，以及 Chromium 啟動遭本機權限拒絕而未能執行；不列為產品測例失敗，也不列為通過。
- `git diff --check`：通過；重驗前工作目錄乾淨。

### 尚待的目前版本驗收

本輪沒有在目前 HEAD 以真 Worker／Brave 多 session 重跑正常流程，也沒有實際完成 2v2、完整 Swiss 排名／同分、淘汰 Draw 時計沿用、單敗季軍、雙敗重置及缺席／超時／退出／空缺矩陣。測例通過不能替代這些整合實證，因此驗收條件 1–6 目前均未放行；條件 7 的驗證程序與命令紀錄已完成。票 22 維持 HOLD。

另更正匿名重播證據：本次工作脈絡中的本機手動測試曾對競賽 replay endpoint 收到 HTTP 404（`Competition replay is unavailable`）。這與 10-16 runner 紀錄中的匿名重播成功敘述尚未對上；目前 HEAD 尚未重現並釐清，因此不把匿名公告／重播讀取列為已驗收。

## 目前驗收候選（2026-10-26；基底 `1339f7f`）

**整票仍為 HOLD。** 本輪使用 `1339f7f` 為基底，包含本地待提交的匿名重播修正。真 Worker／Chromium 多 session 的 1v1 正常路徑 1/1 通過，並實際觸發雙敗重置賽；但其餘矩陣沒有完整的真實瀏覽器／Worker 路徑，不能總放行。

### 本輪執行結果

- `cargo test -- --quiet`：778 通過、0 失敗；`cargo fmt --all -- --check`：通過。
- 從 `apps/web` 執行 `bun run test:integration`：233 通過、0 失敗、3,226 assertions；`bun run test:component`：159 通過。
- 從 `apps/web` 執行 `bun run test:unit`：970 通過、1 失敗、0 略過。唯一失敗是 `scripts/managed-server.test.ts` 的「stop shuts down owned descendants and leaves another worktree running」；此受限 PID 環境將已終止的後代保留為 zombie，process-group alive 探測因此未清零。競賽元件測例本身通過。
- `bun run typecheck`、`bun run build`、`git diff --check`：通過。fresh Worker／target WASM SHA-256 均為 `448a7b84b41968448c917d1f94d578d39e9ab784a9684153cfea3f1c435b0204`。
- 真 Worker／Chromium Playwright：1/1 通過，27.7 分鐘。因受管 runner 在這個 PID 環境無法清理 process group，使用手動啟動的 Wrangler Worker（隔離 port 8798）及 `PLAYWRIGHT_REUSE_SERVER=1` 執行同一 spec；沒有 mock/fake Worker。主辦開放報名後離線，兩個玩家 session 自動從凍結走完四階段並發布公告。賽事是 1v1 個人賽；循環賽完成固定三局及普通下一局，Swiss 設為單輪，雙敗實際走到第 3 個 round 節點並完成 reset。

### 情境矩陣

| 情境 | 本輪結果 | 證據邊界 |
| --- | --- | --- |
| 凍結、抽籤、主辦離線後逐階段開房與完賽 | **真 Worker／Chromium 通過** | 單一 1v1 個人賽；公開公告與 replay refs 清單可讀。 |
| 四賽制正常路徑 | **1v1 真實流程通過** | Round Robin、單輪 Swiss、單敗、雙敗；Swiss 多輪配對與完整排名不在此覆蓋。 |
| 普通下一局 | **真實流程通過** | Round Robin 第 2、3 局由敗方隊長選先手。 |
| 雙敗 reset | **真實流程通過** | 實際生成並完成 reset round；這次 run 不只是一般總決賽。 |
| 2v2／團體與固定成員準備 | **元件測例通過，E2E 未驗** | GameRoom／CompetitionRoom 來源測例有涵蓋，沒有 2v2 多 session 實跑。 |
| Swiss 自訂計分、多輪配對、種子固定、完整排名鍵／同分裁決與迴避 | **元件測例通過，E2E 未驗** | 本輪瀏覽器 Swiss 僅兩隊一輪，未覆蓋排名平手或對手集合差異。 |
| 淘汰 Draw、判定與時計沿用 | **Rust／Worker 元件測例通過，E2E 未驗** | 沒有真實瀏覽器的淘汰 Draw 對局。 |
| 單敗季軍賽 | **元件測例通過，E2E 未驗** | 沒有真實半決賽兩敗隊與季軍場流程。 |
| 缺席、先後／同時超時、退出、空缺、零隊／一隊、未決名次阻擋 | **元件測例通過，E2E 未驗** | 未跑例外整合路徑；空缺與未決的 live 狀態區別仍待驗。 |
| 匿名 replay frame | **本地修正後 HTTP 200** | 第一次 live 讀取重現 404；`Symbol.dispose` 被錯當成額外欄位，忽略 RPC 符號後又發現 Rust frame DTO 用 `battle_record`，造成 TypeScript 讀取 `battleRecord` 失敗。修正精確欄位檢查及 Rust camelCase 序列化後，重建產物對已完成賽事 step 0 回 200，含 `battleRecord`。 |

匿名 replay 修正與回歸測例不包含於基底 `1339f7f`，而是本輪後續候選變更。重播 frame 只以一個已完成遊戲 step 0 實測，沒有涵蓋所有 frame／權限分支。

### 尚待放行

尚缺 2v2 真實整合、多輪 Swiss 與排名同分／迴避、淘汰 Draw 時計沿用、單敗季軍、缺席／超時／退出／空缺及零隊／一隊／未決名次的真 Worker／瀏覽器矩陣。元件測例只能證明各模組分支，不能替代完整平台路徑。驗收條件 1、3–5 未放行；本輪限定通過正常 1v1 多階段路徑、雙敗 reset 與修正後的單一匿名 replay frame，票 22 維持 HOLD。
