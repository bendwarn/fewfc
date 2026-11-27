# 49 Resolve the Mage School

## Triage

ready-for-agent

## What to build

將完整法師學派作為英雄學派端到端的一部分加入。依[法師學派規則](https://www.cfecards.org/rule/latest/hero/mage)實作法師、法導及智者的全部官方 5.16 轉職、繼承能力、專精能力、職業陣法、點數修正、反制效果、投影及 Web 互動。

## Acceptance criteria

- [x] 法師學派三種轉職均執行官方先決條件及火行等級代價。
- [x] 每個官方普通能力及專精能力僅能透過有效的法師學派職業取得。
- [x] 每個法師職業陣法均使用官方組合方式、類別、點數公式、目標及附加效果。
- [x] 職業點數修正在計算攻擊點數時套用，先於門檻判定與傷害轉換。
- [x] 法師反制效果使用共用行動開始及修正流程，不另設平行觸發系統。
- [x] 攻擊事件分別保留攻擊點數、反制效果、五行、環境、防護罩及最終生命值結果。
- [x] 職業被取代或破除時立即移除自動修正，但依各自生命週期保留已成立的公開反制效果。
- [x] 直接執行、重播、公開視圖、Web DTO、可執行行動及能力摘要一致。
- [x] 測試涵蓋幻化複製、防護罩目標、傷害被防止但附加效果仍存在、團隊模式及停用英雄學派行為。

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
