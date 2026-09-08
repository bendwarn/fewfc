# Worktree 開發環境

本機需先安裝 Bun、package.json 指定版本的 pnpm，以及 Rust/rustup。
從 repository 根目錄執行：

```sh
bun scripts/setup-worktree.ts
```

初始化會產生 apps/web/.env、使用 pnpm frozen install、安裝 Wasm target、建置並套用本機 D1 migrations。原有 OAuth credentials 與 auth secret 會保留；新環境會產生獨立 auth secret，OAuth credentials 需自行設定。macOS 有安裝 Brave 時會自動設為測試瀏覽器。

本機每個 checkout 分配五個獨立 port：開發 HTTP、開發 inspector、E2E HTTP、E2E inspector、瀏覽器元件測試 HTTP。分配紀錄在 Git common directory 的 fewfc-ports.json，初始化以 fewfc-ports.lock 目錄互斥；同一 checkout 重跑沿用原配號。已刪除目錄的紀錄會於下次初始化清理。異常中斷若留下鎖，確認沒有初始化程序運作後才移除該鎖目錄。

僅產生設定可加 `--configure-only`。分配時會檢查當下可用 port；若之後被其他程式占用，啟動會失敗，停止占用程式後重試。port 不是作業系統永久保留。

在 apps/web 執行 `bun run dev` 啟動已建置的應用，`bun run cf:dev` 先建置與遷移。E2E 與瀏覽器元件測試使用各自設定的 port。E2E 預設啟動新 server，並使用各自 checkout 的 .wrangler/e2e；明確設定 PLAYWRIGHT_REUSE_SERVER=1 才會重用既有 E2E server 及資料。

初始化的並行配號、重跑穩定性與占用保護可在 apps/web 執行 `bun run test:worktree` 驗證；測試使用暫存 Git repository 與本機 TCP ports。

## 停止 server

刪除 worktree 前，在 apps/web 執行 `bun run dev:stop`，或使用 Codex 的 **Stop servers** action。此指令停止目前 worktree 透過 local-server.ts 啟動的開發、E2E 與元件測試 server，以及它們的子程序。先送出 SIGTERM，最多等待五秒後強制結束未退出的程序群組。Ctrl+C 與 SIGTERM 也會走相同的清理流程。

控制 socket 位於各 worktree 的 `.wrangler/servers`，不會依據 port 或殘留 PID 猜測程序。舊版啟動方式或手動執行的 Wrangler 不受此指令管理，需在原終端機停止。若曾以 SIGKILL 強制終止管理程序，確認該 worktree 已無 server 運作後移除錯誤訊息指出的殘留 socket，再重新啟動。

## Codex

Repository 已提供 `.codex/environments/environment.toml`。Local environment 的 setup command 使用 `bun scripts/setup-worktree.ts`，在建立 worktree 時執行。一般 `git worktree add` 不會觸發 Codex setup；建立後需手動執行相同命令。

Cloud 的 Setup script 與 Maintenance script 都設定：

```sh
bun scripts/setup-worktree.ts --cloud
```

Cloud 容器需具備上述工具。此模式使用固定 port，不寫入本機配號紀錄；隔離由各 task 的容器提供。Maintenance 重跑會保留 .env 的 secret、重新安裝符合 lockfile 的依賴並建置。Cloud 設定需在 Cloud environment 設定頁儲存，repository 的 local environment 設定不會取代它。
