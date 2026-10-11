# 輪次時間、晚配對與更正：Limitless 參考

查證：2026-10-01；2026-10-09重讀官方Schedule。官方文件與一次匿名頁面觀察，非管理員實測或本站驗收。本站開始時間、重新公告與撤銷政策只見[設計](../competitive-play-design.md)；跨房實作只見[關卡](../competition-review-gates.md)。

## 時間與角色

- schedule在賽事開始時自動產生，預設每次新輪配對後自動更新；主辦不必逐輪手填。timer依各輪schedule時間，預設pairing後調至下一整分鐘，關閉自動調整則自訂。可借自動推進與分鐘取整概念；其pairing時間不等於本站整輪實際開房就緒，不能省掉五分鐘窗口。admin可改該輪及後續間隔，更新公開Schedule與active timer。[Schedule](https://docs.limitlesstcg.com/organizer/schedule)
- 官方例子允許 timer 前交手；該例不證本站的開局授權或時計契約，本站提早開局政策仍只見[設計](../competitive-play-design.md#輪次開賽與裁判)。概述的「調至當前時間」不足推導 exact-ms 契約。[Procedures](https://docs.limitlesstcg.com/organizer/procedures.html)、[Reference](https://docs.limitlesstcg.com/organizer/reference)
- 排程編輯屬 tournament admin；judge 可查交手／聊天／reset／report，但不能編輯 tournament。外部角色不自動決定本站授權。[Staff](https://docs.limitlesstcg.com/organizer/basics)
- 當時匿名 [Schedule 樣本](https://play.limitlesstcg.com/tournament/6ab24fe3e905c1db68747a2b/schedule)分列 registration、check-in、teamlist 截止及 UTC 輪次開始。未觀察 admin 改動或推播；第二次取得失敗，未證連續可用性。

## 晚配對不能套用遲來玩家規則

Late Players 限首兩輪 Swiss 的參賽者，漏輪記負；Check-in Timer 處理交手缺席。兩者不是主辦晚配對／晚開房的統一處置。[Settings](https://docs.limitlesstcg.com/organizer/reference)

live bracket 在局部前置交手完成即可配對，Schedule 只列階段開始；不等於本站逐輪完整屏障。已讀文件未找到固定公告已到但開房才完成的統一 grace、最低重公告間隔或必須自動延後政策。[Schedule](https://docs.limitlesstcg.com/organizer/schedule)、[Pairing System](https://docs.limitlesstcg.com/organizer/reference)

[Help Match Rules](https://play.limitlesstcg.com/help/rules)屬 2021-06-07 Limitless Online Series／PTCGO，不當目前所有賽事通則。

## 更正與失效協議的公開範圍

active phase 可 reset／更正並更新 standings；移往下一 phase 後，前 phase final。Delete round 退回前輪且不可逆刪掉當輪交手，文件建議避免重配已開始場次；未說明遠端房間／WS／ACK 失效協議。[Procedures](https://docs.limitlesstcg.com/organizer/procedures.html)

公開 pairings API 沒有 schedule revision、superseded 身分、實際開局或房間撤銷欄位；已公布 webhook 只描述 ended。這只界定已讀公開契約，不反推內部沒做。[Tournament API](https://docs.limitlesstcg.com/developer/tournaments)、[Webhooks](https://docs.limitlesstcg.com/developer/webhooks)

實際查閱、公開頁重取失敗與替代見 [Hume 日誌](../../../scratch/tour-attempt/hume-round-timing-research.md)，舊待決清單不取代目前設計。
