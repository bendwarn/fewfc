# 51 Add Unaffiliated Professions and Void Reversion

## Triage

ready-for-agent

## What to build

依[無學派職業規則](https://www.cfecards.org/rule/latest/hero/others)加入初行客、仙者及聖者的全部官方 5.16 轉職、能力、職業陣法與不繼承規則。實作聖術的實體牌與匹配位置語意，並完成作為整個英雄學派破除職業陣法的虛空返璞術。

## Acceptance criteria

- [x] 初行客透過轉職取得，而非在設定時指派。
- [x] 無學派職業永遠不算同一學派，即使兩位玩家持有相同無學派職業亦然。
- [x] 抉擇與突破是合法轉職途徑，不消耗職業發動能力次數。
- [x] 仙者及聖者只能由官方先決職業轉入，不保留被取代職業的能力。
- [x] 每項無學派職業能力及職業陣法實作官方組合方式、公式、目標及效果。
- [x] 聖術可將一張實體牌展開為兩個匹配位置，但牌移動及一般等級總和公式只計一次。
- [x] Web 陣法匹配選項將聖術牌標示為 `×2`，同時顯示實體捨棄數及點數預覽。
- [x] 只有啟用英雄學派時，虛空返璞術才匹配三張同級牌。
- [x] `VoidReversionResolved` 原子記錄施展隊伍扣除 20 點生命、牌移動、所有被破除職業及因低等級保護而保留的傳說職業。
- [x] 一級與二級虛空返璞術保留傳說職業；三級以上則破除。
- [x] 判定遊戲結果前套用全部生命值及職業變化，包括代價使施展隊伍敗北的情況。
- [x] 直接執行、重播、公開視圖、Web DTO、可執行行動及能力摘要一致。
- [x] 測試涵蓋所有轉職來源、共享隊伍生命值、同時結果、無職業目標及停用英雄學派行為。

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
