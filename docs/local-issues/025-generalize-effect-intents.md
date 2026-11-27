# 25 Generalize effect intents for spell resolvers

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 25 和已結案 GitHub 議題 #18。

## What to build

深化效果解析器邊界，使攻擊、立即術式及被動術式解析器回傳宣告式意圖，再由陣法流程轉為語意事件。這應支援生命值變化、防護罩變化、狀態、牌移動、行動修正及待選擇事項等結果，但不讓解析器直接修改 `GameState` 或發出事件。

## Acceptance criteria

- [ ] 效果意圖模型支援防護罩變化、狀態變化、牌移動、生命值變化、行動修正及待選擇事項。
- [ ] 既有 `barrier` 與 `metamorphosis` 行為透過通用意圖路徑表達。
- [ ] 陣法流程負責基本的用牌移動。
- [ ] 效果解析器維持確定性，不直接修改狀態或附加事件。
- [ ] 流程在發出事件前拒絕重複的待選擇事項。
- [ ] 測試透過公開命令 API 至少涵蓋一個防護罩意圖、狀態意圖、待選擇意圖及行動修正意圖。

## Blocked by

- 無，可立即開始
