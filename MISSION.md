# Mission: fewfc 的 Cloudflare CI/CD

## Why
能為 fewfc 選擇、設定並維護一條可靠的 Cloudflare 部署流程，讓測試、資料庫 migration、staging 與 production 發佈各自有清楚且唯一的負責者。

## Success looks like
- 能判斷何時使用 Cloudflare Git integration，何時使用 GitHub Actions
- 能避免同一次 push 被兩套系統重複部署
- 能安全設定 staging 自動部署與 production 手動核准

## Constraints
- 專案是 Rust、Nuxt、Workers、D1 與 Durable Objects 的 monorepo
- 部署前需要 Rust、Web 與瀏覽器測試，也需要套用 D1 migrations
- 優先使用官方文件與可在 repository 中審查的設定

## Out of scope
- 其他雲端平台的 CI/CD
- 多雲或 Kubernetes 部署
