# 50 Resolve the Windwalker School

## Triage

ready-for-agent

## What to build

將完整風行者學派作為英雄學派端到端的一部分加入。依[風行者學派規則](https://www.cfecards.org/rule/latest/hero/windwalker)實作風行者、影行者及武者的全部官方 5.16 轉職、繼承能力、發動能力、專精能力、職業陣法、免疫、投影及 Web 互動。

## Acceptance criteria

- [x] 風行者學派三種轉職均執行官方先決條件及土行等級代價。
- [x] 風行術僅對計算後攻擊點數不超過 15 點的攻擊忽略陣法反制效果。
- [x] 環境及五行傷害變化不改變風行術門檻判定。
- [x] 影切捨棄所選牌、直接改變上家所屬隊伍生命值，數量為該牌等級兩倍；它消耗共用發動次數，但保留行動機會。
- [x] 幻化與混沌專精保留原陣法身分及效果。
- [x] 影襲是直接改變生命值的術式，不經防護罩及攻擊反制效果，仍受封印影響。
- [x] 土行抗性與影遁只適用於官方所指傷害及陣法效果。
- [x] 刃影瞬殺記為扣除生命，使上家所屬隊伍生命值成為 `floor(original HP / 2)`，不經防護罩傷害。
- [x] 直接執行、重播、公開視圖、Web DTO、可執行行動、能力面板及能力摘要一致。
- [x] 測試涵蓋行動前致命的影切、複製效果、環境互動、隊伍生命值及停用英雄學派行為。

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
