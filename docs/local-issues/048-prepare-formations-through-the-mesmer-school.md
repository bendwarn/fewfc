# 48 Prepare Formations through the Mesmer School

> **部分內容已被取代：**
> [ADR 0025](../adr/0025-model-illusion-cards-as-virtual-formation-components.md)
> 取代本議題的幻術與幻朧準備模型。下方已完成清單保留作為歷史紀錄，但第三張實體牌、可選後續行動及互斥解讀來源的條件不再具權威性。幻術及幻朧現在產生虛擬牌，並要求當回合完成陣法。

## Triage

ready-for-agent

## What to build

將完整幻術師學派作為英雄學派端到端的一部分加入。依[幻術師學派規則](https://www.cfecards.org/rule/latest/hero/mesmer)，實作幻術師、幻靈使及隱者的全部官方 5.16 轉職、繼承能力、發動能力、專精能力、職業陣法、投影及互動。

## Acceptance criteria

- [x] 幻術師學派三種轉職均執行官方先決條件及水行等級代價。
- [x] 職業發動能力使用每位玩家每回合一次的共用次數，不消耗行動機會。
- [x] 幻術與幻朧建立可序列化的已準備職業能力，包含宣告牌、屬性、等級與陣法範圍，不修改牌資料。
- [x] 幻朧不觸發幻術精研，因為兩者是不同的發動能力。
- [x] 準備詳情立即公開、在重新連線及重播後保留，並於任何行動後或回合結束時清除。
- [x] 發動時至少須有一個可能的合法後續，但下一行動不強制使用準備結果，且已支付代價絕不退還。
- [x] 每個陣法匹配選項中，一張實體牌至多選用牌面、準備結果或星辰替代其中一種解讀。
- [x] 幻術師學派專精與職業陣法實作官方組合及效果，同時保留正確陣法身分。
- [x] 屬性抗性、封印互動、防護罩行為、抽牌加成及職業取代使用具型別鉤子。
- [x] 直接執行、重播、公開視圖、Web DTO、可執行行動、能力面板及能力摘要一致。
- [x] 測試涵蓋準備後斷線、未使用的準備、無效發動、複製效果、停用英雄學派行為及團隊模式。

## Superseded acceptance criteria

ADR 0025 取代下列條件：將幻術或幻朧建模為以實體牌為目標的 `PreparedProfessionAbility`、允許玩家不使用產生的牌即結束行動，或在牌面、準備結果及星辰解讀中恰選一種。其餘幻術師學派交付條件仍描述此已完成議題的歷史範圍。

## Blocked by

- [#46 Change Profession into the Warrior School](046-change-profession-into-the-warrior-school.md)
