# 33 Add deterministic deck preparation adapter

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 5。

## What to build

新增介面卡，在記錄 `DeckPrepared` 前以確定性方式準備牌堆順序。重播必須繼續只依賴已記錄牌序；建立新遊戲時則可選擇使用種子或注入的隨機來源產生牌序。

## Acceptance criteria

- [ ] 牌堆準備端口可在遊戲開始前產生有序牌堆。
- [ ] 確定性的記憶體或種子介面卡可為測試產生可重複牌序。
- [ ] `GameRecord::start` 仍記錄完整 `DeckPrepared.deck_order`。
- [ ] 重播忽略種子及 RNG 介面卡，僅使用已記錄牌序。
- [ ] 測試涵蓋確定性準備、事件日誌保留準備後牌序及重播獨立於 RNG。

## Blocked by

- 無，可立即開始
