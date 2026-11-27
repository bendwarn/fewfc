# Distinguish Jianghu states from generic status effects

保留既有的 `StatusEffect` 模型及序列化 `statuses` 契約，作為規則引擎表示既有一般持續效果的方式。另建立具型別的**江湖狀態（Jianghu State）**模型，僅包含千鋒、踏雪及中毒，並記錄其公開規則所需的來源與持續時間。這既保留重播與 Web API 相容性，也避免將江湖規則術語錯用於所有持續效果。
