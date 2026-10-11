# Limitless 與 Pokémon TCG 淘汰賽逾時參考

查核日：2026-09-13。範圍為 Limitless 平台文件與 TPCi／Play! Pokémon 英文規則；不宣稱適用日本、臺灣等地區的所有官方賽事。此為歷史查證，本站政策只見[設計](../competitive-play-design.md)。

## Limitless 平台

- 需要同步開始或計時的單敗賽，使用 Single Elimination Rounds；Live Bracket 則在對手確定後儘快配對。[設定文件：Pairing System](https://docs.limitlesstcg.com/organizer/reference#pairing-system)
- Round Timer 是輪次倒數，不是玩家各自的思考時間。關閉自動結算時僅供顯示；開啟時可依局數領先判勝或判和，但文件明確限制自動結算只適用瑞士制與循環賽，排除淘汰賽。[設定文件：Round Timer](https://docs.limitlesstcg.com/organizer/reference#round-timer)
- Check-in Timer 可對未按時報到者自動判負，這與對局中思考超時不同。[設定文件：Check-in Timer](https://docs.limitlesstcg.com/organizer/reference#check-in-timer)
- 裁判可查看對局、參與聊天、重設對局與填報結果；玩家可提出問題供裁判處理。[操作文件：Resolving Match Issues](https://docs.limitlesstcg.com/organizer/procedures)

因此，平台文件沒有提供「淘汰賽平手超時自動選出勝者」的通用方案。由賽事規則與裁判處理是上述能力邊界的推論；不能把下述實體賽規則當成 Limitless 自動執行的功能，也不能據此決定 FEWFC 雙方同時進入讀秒的處理。

## 結論與版本

查核時最新可核實的更新公告為 **2026-09-01（Q3）**，其中 TCG 項目提到向對手提供牌表的時點澄清；公告並非完整手冊，不能據其未提逾時變更就斷言條文未變。[官方公告，〈Pokémon TCG Rules & Resources〉](https://www.pokemon.com/uk/news/play-pokemon-rules-and-resources-updated-for-q3-2026)

當時取得的最新可讀手冊全文則標示 **2026-05-21**（封面及 §9，頁 27）。以下淘汰賽細節只核實到這一版。官方主要下載網址回傳不可讀 HTML，未嘗試通過或繞過驗證；另找到 Pokémon 官方 CDN 的合法公開 PDF。[官方文件入口](https://play.pokemon.com/en-us/resources/documents/)；[官方 CDN 手冊](https://mcdn.pokemon.com/pokemon-prod/raw/upload/v1/live/static-assets/content-assets/cms2/pdf/play-pokemon/rules/play-pokemon-tcg-tournament-handbook-en.pdf)

## 時間到與淘汰賽處理

2026-01-26 官方公告明確規定：自 **2026-02-13** 起，時間到先完成當前回合，再由對手開始並結束合計三個額外回合；加時鐘為 15 分鐘。它取代 2025-09-01 起的 +1 回合／10 分鐘安排。[官方公告，正文第 1–3 段](https://www.pokemon.com/uk/pokemon-news/an-update-to-end-of-round-procedures-at-pokemon-tcg-championship-events)

以下均據 [2026-05-21 手冊 §7.4.3–7.4.4.3，頁 21–24](https://mcdn.pokemon.com/pokemon-prod/raw/upload/v1/live/static-assets/content-assets/cms2/pdf/play-pokemon/rules/play-pokemon-tcg-tournament-handbook-en.pdf#page=21)：

- 加時由全場同時起算；Checkup 中叫時間，下一回合算額外第 1 回合（§7.4.3）。
- 先適用遲到／未經裁判許可離席條款：單方符合者，時間到及 +3 回合後輸整場（§7.4.4.1）。

| 仍需判定的淘汰賽狀態 | 整場結果（§7.4.4.3） |
| --- | --- |
| BO1 第一局；BO3 第一局或第三局未完成 | 剩餘獎賞卡較少者勝 |
| BO3 第一、二局之間；第二局未完成 | 第一局勝者勝 |
| BO3 第二、三局之間（1–1） | tiebreaker 勝者勝 |

獎賞卡同數等尚未滿足判準時，續打至符合判準或自然勝出。Tiebreaker 重設六張獎賞卡並擲幣決定先手，先取得獎賞卡優勢或以其他方式勝出者贏（§7.4.3.1）。

**未釐清：** §7.4.3 說加時 15 分鐘後 round 結束，§7.4.4.3 又要求同獎賞卡時續打；本次未取得官方解釋兩者相遇時的裁判操作，不能推測硬切、強制和局或自動判種子勝。

## Sudden Death 與線上差異

官方 Professor Community 的 **2026-02-26** 文章〈Au revoir, Sudden Death〉確認：Play! Pokémon 自 2018 年 7 月已採 tiebreaker；一般 Rulebook 在 2026 年 2 月也將一張獎賞卡的 Sudden Death 換成六張獎賞卡的 tiebreaker。文章明說 TCG Online／Live 的對戰不屬於 Play! Pokémon matches，且 **當時 Live 尚未更新**。[官方 Professor Community，該小節](https://professorprogram.pokemon.com/news/11473085)

因此，實體賽的整場逾時判定，不能直接當作 Live 客戶端的計時／淘汰賽規則。上述文章足以證明當時存在差異，但不能證明截至查核日 Live 是否已完成更新；本次未核實最新官方客戶端版本或官方線上淘汰賽專用逾時條文。官方論壇的一般玩家貼文不作為官方規則證據。

本次整理未重新查證手冊或執行測試。
