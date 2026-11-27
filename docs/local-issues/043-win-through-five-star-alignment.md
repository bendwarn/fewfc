# 43 Win through Five-Star Alignment

## Triage

ready-for-agent

## What to build

將五星連珠實作為星辰圖記中依個別玩家判定的特殊勝利條件。啟用模組時，玩家親自召喚全部五種星辰後，待符合條件的陣法施展完全結算，其隊伍獲勝。

以具語意的正規遊戲事件表示此成就，並投影至遊戲狀態、重播、公開狀態視圖、公開事件動態及 Web 結果呈現。

## Acceptance criteria

- [x] 召喚星辰歷史依玩家追蹤；星辰被取代或破除後仍保留不同星辰種類。
- [x] 判定五星連珠時不合併隊友歷史。
- [x] 玩家召喚第五種不同星辰時發出 `FiveStarAlignmentAchieved`。
- [x] 五星連珠決定遊戲結果前，完整陣法施展先結算。
- [x] 依生命值判定結果前，五星連珠使達成玩家的隊伍獲勝。
- [x] 同次陣法施展造成的生命值同時歸零不覆蓋五星連珠。
- [x] 公開狀態視圖及公開事件動態指出特殊勝利，不洩漏隱藏正規資料。
- [x] Web 結果呈現區分五星連珠與生命值勝利。
- [x] 直接執行及重播產生相同歷史、成就事件與遊戲結果。
- [x] 雙人及團隊測試涵蓋達成、重複召喚及不合併隊友歷史。

## Blocked by

- [#40 Summon and display all five Stars](040-summon-and-display-all-five-stars.md)
