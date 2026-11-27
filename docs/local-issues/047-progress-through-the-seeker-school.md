# 47 Progress through the Seeker School

## Triage

ready-for-agent

## What to build

將完整尋道者學派作為英雄學派端到端的一部分加入。依[尋道者學派規則](https://www.cfecards.org/rule/latest/hero/seeker)實作尋道者、述道者及仁者的官方 5.16 轉職、繼承能力、專精能力、職業陣法、戰鬥修正、公開視圖呈現及 Web 行動候選。

## Acceptance criteria

- [x] 尋道者學派三種轉職皆執行官方先決條件及木行等級代價。
- [x] 僅在有效持有尋道者能力時，其棄牌回收修正才改變生命值代價。
- [x] 生陣、剋陣、歸元及五行輪迴專精增加替代匹配方式，不改變陣法身分。
- [x] 道禦透過共用流程結算官方攻擊與反制效果。
- [x] 仁者的木行抗性與術式保護在具型別結算階段生效，並在職業被取代或破除後消失。
- [x] 轉生術實作官方組合方式與回復公式。
- [x] 轉生術的獨立牌只能合法分配一種方式時回傳一個陣法匹配選項；兩種分配均合法且回復值不同時，回傳各自帶正確預覽的選項並要求明確選擇。
- [x] 公開事件記錄選定的角色綁定及結算回復值，重播時不重新計算規則。
- [x] 直接執行、重播、可執行行動、Web DTO 及能力摘要一致。
- [x] 測試涵蓋雙人及團隊目標、啟用及停用棄牌回收、職業取代及停用英雄學派行為。

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
