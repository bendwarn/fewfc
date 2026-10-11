# 賽程工具選型參考

查證：2026-09-12／13；能力、版本及費用只代表當日來源。現行採用政策見[設計](../competitive-play-design.md)，不由本報告新增功能或改選庫。

## Rust tournaments：特定發布版的落差

2026-09-13 實讀本機 `tournaments-0.1.0` 發布原始碼，對照[發布版文件](https://docs.rs/tournaments/0.1.0/tournaments/)：MIT，原生單敗／循環，沒有現成雙敗／瑞士管理。

- `single_elimination.rs` 限制二次方隊數，但輪數用截斷的 `sqrt(n)` 而非 `log2(n)`；按該版程式八隊只建兩輪。
- `round_robin.rs` 使用 circle method，但拒絕奇數隊。
- 完成結果只有 winner／loser，缺 Draw、局比分、更正原因與本站序列化契約；累積勝場排名不能直接代表本站積分政策。

這些是 0.1.0 的查驗，不外推其他版本。它支持「借 bracket／round builder 概念及 circle method，自行設計本站狀態」的選擇，不能宣稱直接依賴即可得到四賽制。當前 dependency 以 Cargo.toml 為準。

## 其他工具：只保留比較入口

| 候選／官方來源 | 形態及適配界線 |
| --- | --- |
| [brackets-manager.js](https://github.com/Drarig29/brackets-manager.js)＋[viewer](https://github.com/Drarig29/brackets-viewer.js) | JS／TS 排程與呈現；需本站身分、報名、時計、系列與持久化。下游鎖定的固定版本證據見 [Bracket 參考](bracket-feature-reference.md)。 |
| [Challonge v2.1](https://challonge.apidog.io/upgrading-from-v2-0-to-v2-1-1726691m0) | SaaS API；v2.1 的 snake_case、tie 與比分欄位不能證成本站 BO／和局續局自動化；完整報價／權限當時未確認。 |
| [Toornament API](https://developer.toornament.com/v2/overview/get-started) | SaaS 的 Structure／Registration／Match／Webhook，另需 project、憑證、方案與本站授權整合；當時無可核實完整價格表。 |
| [evroon/bracket](https://github.com/evroon/bracket) | 完整自架平台，不是 Rust crate；固定 source、重用及授權見 [Bracket 參考](bracket-feature-reference.md)。 |
| [Limitless Developer](https://docs.limitlesstcg.com/developer.html) | 公開查詢與結束 webhook，不是本站可用的配對管理寫入 API；細節見 [瑞士參考](limitless-swiss-reference.md)。 |

候選的 match 不直接等於本站一份 Game Record；版本／授權、Draw／DoubleLoss、結果更正、重試及 runtime 必須按實際採用方式驗證。外部價格與方案不作目前預算依據。

本整理沒有重新安裝、執行或查證候選服務。
