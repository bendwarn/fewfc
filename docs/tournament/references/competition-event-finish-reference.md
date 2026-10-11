# 整場收尾與賽後發布：Limitless 參考

查證：2026-10-01。官方文件觀察，非內部實作或產品驗收；本站自動結束、牌組公開及更正決定只見[設計](../competitive-play-design.md)。

## 官方可確認內容

| 來源 | 可確認內容 |
| --- | --- |
| [Tournament Structure](https://docs.limitlesstcg.com/organizer/structure) | 多 phase 可有不同設定；started phase 不換配對系統，但 active／upcoming phase 多數設定仍可改。 |
| [Running the Tournament](https://docs.limitlesstcg.com/organizer/procedures) | 前 phase 最後一輪完成、核排名及未決案件後移往下一 phase；轉移後先前 phase final。active phase 內可更正並重算。 |
| [End Tournament](https://docs.limitlesstcg.com/organizer/procedures) | 全部輪次結束後主辦核賽果並手動 End Tournament，移往 finished／archive；public 賽事資料會送 Trainer Hill。 |
| [Decklist Settings](https://docs.limitlesstcg.com/organizer/reference) | `open after tournament` 是選項，另有開放／裁判可見／只公開 archetype。 |
| [Webhooks](https://docs.limitlesstcg.com/developer/webhooks) | ended 時 HTTP POST `tournament:ended`，含識別資料與 secret 驗證；該頁當時只描述此事件。 |

## 證據界線

最後一輪完成、phase 轉移、整場封存與公開選項是不同事實；不能由 archive 或通知推導本站私密資料發布許可。這是流程推論，不是外部內部狀態機的證明。

上述頁面未說明 End Tournament 後更正／重開、已外送資料版本／撤回、webhook 重試順序／exactly-once 或公開讀取原子性；查不到不表示能力不存在。

早期「主辦顯式 close」候選沒有採用，勿當現行指令；目前發布接線見[關卡](../competition-review-gates.md)。
