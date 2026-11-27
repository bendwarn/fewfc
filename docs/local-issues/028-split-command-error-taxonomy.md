# 28 Split command errors by validation, implementation, and invariant layers

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 35。

## What to build

將扁平命令錯誤介面拆為明確的驗證、規則實作及引擎不變條件層。玩家可見的非法命令錯誤應能與未完成規則實作及不可能的引擎狀態區分，同時保留命令處理的原子語意。

## Acceptance criteria

- [ ] 命令／應用程式錯誤區分驗證失敗、規則實作失敗及引擎不變條件失敗。
- [ ] 既有驗證錯誤歸入驗證錯誤層。
- [ ] `EffectNotImplemented` 維持為規則實作錯誤。
- [ ] 引擎不變條件錯誤涵蓋不可能的抽牌供給、重複待選擇事項、重複蓋牌或區域擁有權不一致等情況。
- [ ] 命令及自動推進失敗時仍不發出事件、不修改狀態。
- [ ] 測試涵蓋至少一個驗證錯誤、規則實作錯誤、引擎不變條件錯誤，以及各類錯誤的原子性。

## Blocked by

- 無，可立即開始
