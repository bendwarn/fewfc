# 29 Enrich recorded event metadata

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 34 和已結案 GitHub 議題 #4。

## What to build

擴充已記錄事件的中繼資料，讓事件動態能辨識設定、自動及命令產生的事件，並提供足夠資訊供除錯、介面確認、網路傳輸與稽核。重播仍應只依賴事件順序及正規事件 payload。

## Acceptance criteria

- [ ] `RecordedEvent` 中繼資料保留單調遞增序號。
- [ ] 事件來源區分設定、自動及命令事件。
- [ ] 自動事件中繼資料在有幫助時包含確定性原因。
- [ ] 命令事件中繼資料可辨識命令／玩家情境，無須持久化獨立命令日誌。
- [ ] 重播僅使用正規 `GameEvent` 序列，不依賴命令中繼資料。
- [ ] 測試涵蓋設定、自動及命令事件的中繼資料來源分類，以及重播獨立於中繼資料。

## Blocked by

- 無，可立即開始
