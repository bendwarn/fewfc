# fewfc

CFECards 的 Rust 規則引擎與線上對局應用。規則核心負責命令驗證、回合流程、陣法結算與確定性事件重播；Nuxt Web 介面透過 Cloudflare Workers、Durable Objects 與 D1 提供帳號、同步房間與對局紀錄。

Rust crate 可獨立建置與測試，也會編譯為 WebAssembly 供 Web 後端使用。專案另包含競賽計時與賽事管理，相關範圍及進度由賽事文件維護。

## 快速開始

### 只開發規則引擎

安裝支援 Rust 2024 edition 的 Rust toolchain 與 Cargo，在 repository 根目錄執行：

```sh
cargo build --locked
cargo test --locked
```

### 開發 Web 應用

先依 [Worktree 開發環境](docs/worktree-setup.md)準備工具、初始化並啟動本機服務；Web 功能與日常開發操作見 [Web README](apps/web/README.md)。

## 目錄結構

| 路徑 | 內容 |
| --- | --- |
| `src/` | Rust 規則引擎、競賽與賽事模型、Web／Wasm 介面 |
| `tests/` | Rust 規則、重播、競賽與賽事測試 |
| `apps/web/` | Nuxt UI、Worker、資料庫 migrations 與 Web 測試 |
| `scripts/` | Checkout／worktree 初始化工具 |
| `docs/` | 規則、架構決策、開發文件與本機 issue tracker |
| `scratch/` | 不需納入版本控制的暫存工作檔案 |

## 文件入口

各主題的詳細說明維護於下列文件。

| 想了解的內容 | 文件 |
| --- | --- |
| 領域詞彙與模型 | [CONTEXT](CONTEXT.md) |
| 遊戲規則、回合與結算語意 | [規則參考](docs/rule.md) |
| 實作邊界、事件重播與架構決策 | [規則引擎決策](docs/rules-engine-decisions.md) |
| UI 驗收與測試分層 | [Storybook](apps/web/docs/storybook.md) |
| 瀏覽器測試問題 | [E2E 疑難排解](apps/web/docs/e2e-troubleshooting.md) |
| 房間與 WebSocket 同步 | [即時同步](apps/web/docs/realtime.md) |
| Cloudflare 設定與發佈流程 | [部署文件](apps/web/docs/deployment.md)；執行設定見 [CI/CD GitHub Workflow](.github/workflows/ci-cd.yml) |
| 競賽設計、票據與審核進度 | [賽事文件索引](docs/tournament/README.md) |
| 規則引擎與應用的議題紀錄 | [本機議題索引](docs/local-issues/README.md) |
| 協作與驗證規範 | [Agent Instructions](AGENTS.md) |
