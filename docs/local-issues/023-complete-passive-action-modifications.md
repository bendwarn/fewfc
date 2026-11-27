# 23 Complete passive action modifications and sealed passives

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 25 和已結案 GitHub 議題 #17、#18、#21。

## What to build

完成被動術式的結算，不只記錄翻開或無效。被動效果應回傳行動修正，由傳入行動的結算以確定性方式套用。防禦應影響傳入攻擊，封印應影響傳入術式；被封印的蓋牌之後應翻開且無效，不得提前洩漏額外公開資訊。

## Acceptance criteria

- [ ] 被動結算產生行動修正，而不只回傳 `PassiveFlipOutcome::Applied`。
- [ ] `defense` 依選定的首版規則行為改變傳入攻擊結算，並記錄可重播結果資料。
- [ ] `seal` 僅作用於傳入術式。
- [ ] `seal` 作用於傳入的被動術式覆蓋行動時，該蓋牌仍保持覆蓋，並在內部標記為已封印。
- [ ] 被封印蓋牌於自己的觸發時點翻開、無效並正常捨棄。
- [ ] 公開／玩家視圖不會在正常蓋牌可見性規則之外暴露封印標記。
- [ ] 測試涵蓋防禦對攻擊、封印對主動術式、封印對被動覆蓋、被封印蓋牌後續無效捨棄及重播一致性。

## Blocked by

- #25
