---
status: accepted
---

# Record every Environment Transfer

每次施展聖獸陣法，其攻擊結算後都會產生一個正規的 `EnvironmentTransferred` 事件，即使轉移前後的環境屬性相同也不例外。同屬性轉移雖不改變投影狀態，保留這項語意事實可避免事件動態及未來跨模組反應必須依照後續規則版本重新解讀陣法歷史。
