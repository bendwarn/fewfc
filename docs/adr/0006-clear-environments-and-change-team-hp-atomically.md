---
status: accepted
---

# Clear Environments and change Team HP atomically

成功施展虛空斷脈術時，以單一正規 `EnvironmentCleared` 事件同時結算破除環境與雙方各扣除 20 點生命值。投影器先套用其中所有生命值變化，再判定遊戲結果；因此同時敗北會造成平局，不受事件序列化順序影響。不使用分開的 `HpChanged` 事件，因為那會在規則視為同時發生的變化之間暴露虛假的中間勝者。
