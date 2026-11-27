# 26 Implement remaining base active spell effects

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 25、31 和已結案 GitHub 議題 #11、#18。

## What to build

實作目前回傳 `EffectNotImplemented` 的其餘基礎主動術式效果。每個術式均應經正常 `PerformFormation` 流程結算，僅在驗證成功後消耗行動，產生可重播語意事件，並使用效果意圖而非直接修改狀態。

## Acceptance criteria

- [ ] `generating-formation` 有確定性的首版效果。
- [ ] `overcoming-formation` 有確定性的首版效果。
- [ ] `radiance` 有確定性的首版效果。
- [ ] `return-to-origin` 有確定性的首版效果。
- [ ] `chaos` 有確定性的首版效果。
- [ ] `five-elements-cycle` 有確定性的首版效果。
- [ ] 命令層整合測試與重播一致性測試涵蓋已知主動術式行為。
- [ ] 若仍有合法但刻意延後的效果，以明確實作錯誤表示。

## Blocked by

- #25
