---
status: accepted
---

# Separate card origin from current zone

每張實體牌（Card Instance）都有不可變的牌來源（Card Origin），指向共用牌堆或原始個人牌組的玩家。牌堆及棄牌堆各有堆所有者（Pile Owner），標示其屬於共用遊戲或某位玩家；牌目前所在區域與持有者則分別追蹤。

因此，棄牌回收可讓其他玩家的牌經過目前玩家的牌堆與手牌，而不改變該牌最終應捨棄的位置。不在每次移動時改變來源，因為那會丟失確定性歸還、重播與公開視圖所需的來源資訊。
