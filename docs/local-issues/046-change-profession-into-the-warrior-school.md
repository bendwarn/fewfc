# 46 Change Profession into the Warrior School

## Triage

ready-for-agent

## What to build

透過戰士學派交付首個完整英雄學派端到端實作，同時以發布閘門隱藏尚未完成的模組。新增玩家持有的職業狀態、官方職業目錄與繼承模型、作為獨立行動命令的轉職、具型別職業能力鉤子、正規事件、重播、公開視圖、可執行行動及席位徽章。

依[戰士學派規則](https://www.cfecards.org/rule/latest/hero/warrior)實作戰士、戰神與勇者的所有官方 5.16 能力、專精能力、職業陣法、轉職條件及繼承能力。

## Acceptance criteria

- [x] 每位玩家開始時沒有職業；遊戲狀態為每位玩家儲存至多一個 `ProfessionId`。
- [x] 轉職以行動命令而非陣法施展驗證目標職業、先決條件及確切所選實體牌。
- [x] 已接受轉職進入共用行動開始流程，消耗行動機會、記錄明確牌移動並發出 `ProfessionChanged`。
- [x] 新職業取代原職業；有效能力由目錄繼承推導，不複製狀態。
- [x] 具型別鉤子回傳宣告式修正或意圖，絕不直接修改遊戲狀態或發出事件。
- [x] 戰士、戰神與勇者實作所有官方轉職、繼承能力、專精及職業陣法。
- [x] 職業專精保留原陣法身分與效果。
- [x] 公開狀態視圖、公開事件動態、Web DTO、已記錄事件中繼資料、直接執行及重播一致。
- [x] 持有職業的玩家顯示公開席位徽章及唯讀有效能力摘要；沒有職業時不顯示徽章。
- [x] 所選牌的行動面板呈現合法轉職，不枚舉其他牌組合。
- [x] 未完成的英雄學派模組不能在正式環境房間設定中啟用。
- [x] 雙人及四人測試涵蓋合法進階、非法跳級、取代、破除職業及停用英雄學派的回歸。

## Blocked by

- [#45 Deepen Card selection into playable Actions](045-deepen-card-selection-into-playable-actions.md)
