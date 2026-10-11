# 2026-10-11 Git 工作基線

本輪將先前的競賽工作分批提交，供後續審核從 Git 基準 ref 與差異取得變更範圍。

| 批次 | Git 提交 | 範圍 |
| --- | --- | --- |
| 開發設定 | `fd8e16a` | 開發設定、scratch／Wrangler cache 忽略規則及代理規則。 |
| Rust 核心 | `d2a4a38` | 報名、時計、交手、四賽制與競賽 JSON／WASM 入口，領域詞彙、ADR 與回歸案例。 |
| 服務／API | `706350c` | CompetitionRoom、GameRoom 競賽接線、共享契約、HTTP／RPC helpers、Worker bindings 與案例。 |
| 網站介面 | `819e056` | 競賽管理、公告、牌組、玩家入口、準備、時計、裁判介面與 Nuxt／純模組案例。 |
| 文件整理 | 與本文件同批提交 | 設計、28 票、參考、審核關卡與歷史紀錄；審核改用 Git。 |

整理提交保存目前工作，不代表所有競賽功能已完成或通過。01–04 的限定通過範圍、其他已驗證切片與 HOLD 仍以[目前關卡](../competition-review-gates.md)為準；05 自動首輪及其他後續票須各自實作與驗證。

本輪各批已檢查 staged diff 與 whitespace，產品程式內容沿用既有工作，沒有重新執行全量測試。22 的全量 Cargo、型別、打包、Brave／真 Worker 與完整辦賽驗收仍待集中執行；瑞士雙負重配對、legacy initial binding 等既存問題維持原關卡限制。

歷史 Markdown 已移除逐檔版本摘要清單與核對要求，保留測試結果、失敗原因和證據入口。Scratch 原始 logs 與暫存產物依 ignore 規則留在本機；產品協議需要的 binding／checkpoint digest 定義維持原契約。
