# 32 Add replay verification mode

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 33。

## What to build

新增選用驗證模式，可針對已知歷史重新執行命令決策或自動推進，並比較產生的正規事件與已儲存事件日誌。純重播必須保持不變，繼續精確套用已記錄事件。

## Acceptance criteria

- [ ] 純重播繼續直接套用正規事件，不重新計算。
- [ ] 驗證模式可比較預期自動事件與已記錄自動事件。
- [ ] 有命令情境時，驗證模式可比較預期命令事件與已記錄命令事件。
- [ ] 驗證失敗回報序號及事件不一致細節。
- [ ] 測試涵蓋驗證成功、自動事件不一致、命令事件不一致及純重播獨立性。

## Blocked by

- #29
