> *本內容由 AI 在議題分流期間產生。*

# 52 Conform Hero Schools across Rule Modules

## Triage

ready-for-agent

## What to build

完成英雄學派 5.16 與基礎規則集及所有可用規則模組之間的符合性。執行已接受的具型別鉤子及攻擊階段決策、補齊跨學派缺口、驗證完整 18 職業目錄，並在模組發布前確保直接執行、重播、公開視圖、Web DTO 及所選牌查詢一致。

## Acceptance criteria

- [x] 符合性盤點將每個官方轉職、繼承能力、普通能力、專精能力、發動能力及職業陣法映射至重點測試。
- [x] 英雄學派與星辰測試涵蓋依點數判定召喚資格、確切星辰替代牌宣告、準備後解讀、聖術位置及五星連珠順序。
- [x] 英雄學派與五方傳說測試證明聖獸使用先前環境並忽略陣法效果，但玩家承受傷害時仍受職業抗性影響。
- [x] 考慮職業抗性前，防護罩承受完整聖獸傷害；抗性不減少或防止防護罩傷害。
- [x] 英雄學派與五方傳說測試涵蓋環境傷害、陣法無效、反制效果免疫及虛空結算順序。
- [x] 英雄學派與棄牌回收測試涵蓋尋道者代價修正、無法行動時點、致命代價及共享隊伍生命值。
- [x] 英雄學派與個人牌組測試涵蓋發動代價、牌來源、公開的外來牌、各玩家牌堆、回收及重新連線。
- [x] 牌解讀層依效果順序按維度組合，後一層只取代其指定維度。聖術視為兩張在其後套用，不能使用星辰變牌，且使兩個匹配位置具有相同屬性與等級解讀。
- [x] 跨模組攻擊結算依序進行匹配、攻擊點數、門檻判定、反制效果、傷害轉換及防護罩，最後處理陣法後意圖。
- [x] 玩家無法行動或完整效果無法結算時拒絕發動職業能力；需要後續陣法的準備若無合法後續則拒絕。
- [x] 雙人與四人測試涵蓋玩家持有職業、隊伍持有生命值、上家目標及作用於所有其他玩家的效果。
- [x] 僅基礎規則、停用英雄學派、驗證原子性、紀錄驗證、觀看者過濾及既有房間重播維持回歸閘門。
- [x] 英雄學派發布閘門維持關閉。

## Blocked by

- [#47 Progress through the Seeker School](047-progress-through-the-seeker-school.md)
- [#48 Prepare Formations through the Mesmer School](048-prepare-formations-through-the-mesmer-school.md)
- [#49 Resolve the Mage School](049-resolve-the-mage-school.md)
- [#50 Resolve the Windwalker School](050-resolve-the-windwalker-school.md)
- [#51 Add Unaffiliated Professions and Void Reversion](051-add-unaffiliated-professions-and-void-reversion.md)
- [#40 Summon and display all five Stars](040-summon-and-display-all-five-stars.md)
- [#41 Use Star Formations and Star Element Substitution](041-use-star-formations-and-element-substitution.md)
- [#42 Resolve Void Star Breaking atomically](042-resolve-void-star-breaking-atomically.md)
- [#43 Win through Five-Star Alignment](043-win-through-five-star-alignment.md)
- [#54 Conform Five Directions Legend to the 5.16 PDF clarifications](054-conform-five-directions-legend-to-pdf-clarifications.md)
