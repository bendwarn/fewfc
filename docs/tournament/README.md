# 線上賽事文件索引

依工作選擇文件，不需先讀整個目錄。

| 工作 | 入口／唯一權威 |
| --- | --- |
| 確認玩法、計時、報名、晉級與公開政策 | [競賽設計](competitive-play-design.md) |
| 查實作現況、互斥範圍、安排驗證或放行證據 | [目前審核關卡](competition-review-gates.md) |
| 查首場完賽／延後範圍、待辦優先級、每票驗收與功能阻擋 | [正式票據索引](tickets/README.md) |
| 查領域詞義 | [CONTEXT](../../CONTEXT.md) |
| 修改原始遊戲結論與競賽判定的邊界 | [ADR0039](../adr/0039-separate-competition-rulings-from-game-conclusions.md) |
| 查歷史驗證／審核的範圍與結果 | [驗證歷史](evidence/README.md) |
| 審核變更或整理提交 | [Git 審核流程](../agents/git-review.md) |
| 查曾失敗的做法與工作產物 | [tour-attempt 索引](../../scratch/tour-attempt/README.md) |

## 功能開始前的參考

沿用使用者要求：每個新功能先查 Bracket 或類似專案，只讀相關分支；評估直接使用、概念重用或自行設計 Rust 邏輯的收益及授權。以下是有日期的查證，不是永久放行或現行產品規格。

| 分支 | 參考 |
| --- | --- |
| 選庫／外部 API | [工具選型](references/competitive-tournament-tools.md) |
| Bracket 的排程、更正、權限與公開模型 | [Bracket 功能參考](references/bracket-feature-reference.md) |
| 首輪參賽資料凍結 | [名單與牌組](references/competition-opening-roster-freeze-reference.md) |
| 輪次開始時間、晚配對、撤銷重建 | [時間與更正](references/competition-round-timing-reference.md) |
| 後續階段資格／種子來源 | [多階段轉移](references/competition-stage-transition-reference.md) |
| 整場收尾／賽後發布 | [結束與公開](references/competition-event-finish-reference.md) |
| 瑞士配對／排名 | [Limitless 瑞士制](references/limitless-swiss-reference.md) |
| 外部淘汰賽超時案例 | [逾時參考](references/elimination-timeout-official-reference.md) |

## 文件維護

產品決定只改設計文件；交付範圍、優先級、逐票驗收與功能相依只改票據；目前進度、互斥範圍與放行證據只改關卡；參考文件保留來源、日期、觀察與未知。原始報告是當時範圍與執行的證據，不拿歷史結論覆蓋目前權威。

本次整理依 [OpenAI 文章](https://developers.openai.com/blog/rethinking-skills-and-prompts-for-gpt-6-astra)：短索引、按需展開、明確完成條件。設定檔與程式可直接查得的資料不另做文件副本。重要的審核原始紀錄見[驗證歷史](evidence/README.md)，版本與修改範圍由 Git 管理。
