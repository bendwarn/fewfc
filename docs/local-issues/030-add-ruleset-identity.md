# 30 Add ruleset identity to setup and persistence

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 4、27、31 和已結案 GitHub 議題 #22。

## What to build

使規則集識別成為遊戲設定及持久化紀錄的明確部分。目前持久化中繼資料已有規則集／版本資訊，但權威設定也應識別用來驗證設定、結算陣法及重播紀錄的規則集。

## Acceptance criteria

- [ ] `GameSetup` 包含規則集識別。
- [ ] 官方設定建構器填入基礎規則集識別。
- [ ] 持久化事件日誌包含與設定中繼資料一致的規則集識別。
- [ ] 載入／重播時驗證持久化中繼資料與設定的規則集識別一致。
- [ ] 測試涵蓋基礎設定建構器識別、持久化往返及拒絕不一致。

## Blocked by

- 無，可立即開始
