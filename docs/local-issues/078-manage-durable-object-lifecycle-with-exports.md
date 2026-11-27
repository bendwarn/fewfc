# 78 Manage Durable Object Lifecycle with Wrangler Exports

## Triage

ready-for-agent

## Goal

使用 Wrangler `exports` 宣告式管理 Web Worker Durable Object 生命週期；新命名空間使用 SQLite，並保留既有 GameRoom、PlayerNotifications、ReplayArchive 及 CompetitionRoom 命名空間。

## Confirmed Decisions

- 保留四個類別名稱、綁定及既有 SQLite 儲存後端。default、staging 與 production 設定透過 `new_sqlite_classes` 建立這些類別；本工作不重新命名、刪除、轉移或重新配置命名空間。
- 以四個類別目前狀態的 `exports` 宣告取代每個 Worker 環境的 Durable Object `migrations` 歷史。Durable Object `exports` 與 `migrations` 不可共存；首次 exports 部署後，Worker 必須持續採用 exports 生命週期。
- 保持 D1 `migrations_dir` 設定及 SQL migration 檔案不變。
- 在已部署環境管理 `MAINTENANCE_MODE` Worker 變數，並透過與 Durable Object exports 相容的設定更新立即套用每個值。
- Durable Object 生命週期變更只透過 `wrangler deploy` 套用，不使用漸進部署。此實作只變更本機設定及文件，不部署至 Cloudflare。

## Plan

1. 從 Worker 程式碼及原 Wrangler migration 項目確認每個現有類別與儲存後端。
2. 在 default、staging 及 production 的 `exports` 宣告目前類別集合，再只移除 Durable Object migration 項目。
3. 更新部署指南，透過與 Durable Object exports 相容的 Worker 變數設定管理 `MAINTENANCE_MODE`。
4. 審查設定及 diff 後記錄議題完成；首次遠端部署留待一般受保護部署 GitHub Workflow。

## Acceptance Criteria

- [x] 每個 Worker 環境均在 `exports` 下以 `storage = "sqlite"` 宣告目前四個 Durable Object 類別。
- [x] Wrangler 檔案沒有 Durable Object `migrations` 陣列；D1 資料庫 migration 設定保持完整。
- [x] 維護模式使用 Worker 變數，與 Durable Object exports 相容。
- [x] 部署文件說明 exports 生命週期及維護開關行為。
- [x] 不改變 Durable Object 執行期程式碼、schema、儲存資料或命名空間身分，且不執行遠端部署。

## Implementation And Verification

已更新 default、staging 及 production Wrangler 設定，透過 `exports` 宣告既有四個 SQLite 後端命名空間，並保留 D1 migration 目錄。已以 Worker 變數設定 `MAINTENANCE_MODE`，並修訂部署指南，說明單向 exports 切換及部署限制。

已審查設定宣告，且 `git diff --check` 通過；未執行測試或遠端部署。
