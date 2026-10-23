# 本地技能更新

更新本頁列出的技能時，先確認下列本地適配，再比對上游變更。此文件記錄維護方式，不要求一般開發工作讀取。

## 維護來源

- `~/.codex/AGENTS.md`：個人偏好，直接維護；套件管理使用 pnpm，腳本與測試 runtime 依專案設定。
- `~/.codex/skills/cx/SKILL.md`：本次未在 `~/.agents/.skill-lock.json` 找到來源紀錄。本地差異保存於 `docs/skill-overrides/cx.patch`，相對路徑以 `~/.codex/skills` 為根。
- `.codex/skills/audit-fewfc-room-record/`：本專案維護。純紀錄解說以事件與規則為證據；實作或 replay 正確性調查使用相關測試，並揭露無法驗證的範圍。

## 更新方式

1. 更新前保存目前版本與差異。將上游新版放到暫存目錄比較，再整合需要的變更；直接對安裝目錄更新可能覆蓋本地修改。
2. cx 更新後，在 `~/.codex/skills` 執行 `git apply --check /絕對路徑/fewfc/docs/skill-overrides/cx.patch`。可套用時才執行 `git apply`；若已包含本地修改或上游結構已變，先檢查差異，按語意整合，避免強制覆蓋。重新產生 patch 時以未修改的新版作為基底。
3. 檢查 frontmatter、相對連結及差異；以技能的實際使用情境檢查是否仍有多餘停止條件。只有工具程式或執行契約變更時才追加相應執行驗證。

本流程由更新者執行，沒有自動重套 patch 的 hook。若長期頻繁追蹤上游，改由個人 fork 維護本地差異，再讓安裝工具指向該 fork，可避免每次直接修改安裝產物。

## 第二批技能適配

- `diagnosing-bugs`：保留以證據驗證的原則，允許先讀程式形成暫定假說；沒有重現時揭露不確定性並繼續可做的分析。patch 根目錄為 `~/.agents/skills`。
- `cloudflare`、`agents-sdk`、`cloudflare-email-service`、`sandbox-sdk`：依 Cloudflare 產品與版本縮小觸發範圍。patch 根目錄為 `~/.codex/skills`。
- `wrangler`：入口保留版本、環境與操作邊界；詳細範例移至 `references/commands.md`。更新時一起比對 `wrangler.patch` 與 `wrangler-reference.patch`，兩者根目錄都是 `~/.codex/skills`。

上述 patch 都位於 `docs/skill-overrides/`，使用與 cx 相同的先 check、再套用流程。安裝紀錄仍代表上游來源，並不代表本地修改已由安裝工具保存；更新後需重新比對。專案的 Web AGENTS、E2E troubleshooting 與 domain 文件由本專案 Git 維護。
