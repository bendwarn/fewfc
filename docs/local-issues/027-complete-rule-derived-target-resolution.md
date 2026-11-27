# 27 Complete rule-derived target resolution

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 19、20 和已結案 GitHub 議題 #19。

## What to build

為由規則推導的語意目標新增可重用的目標解析器。引擎已為攻擊基本傷害推導上家；本工作應以相同模式處理自己、下家、我方與對方，使效果解析器不必要求玩家提交規則已確定的目標。

## Acceptance criteria

- [ ] 引擎提供自己、上家、下家、我方及對方的內部目標解析。
- [ ] 規則推導目標根據目前狀態及行動順序解析，而非取自 `declared_targets`。
- [ ] `declared_targets` 僅供效果要求玩家具體選擇的玩家／隊伍／牌使用。
- [ ] 無效的宣告目標驗證失敗，不產生事件或狀態變化。
- [ ] 測試涵蓋雙人與團隊模式下所有支援語意目標的解析。
- [ ] 測試涵蓋至少一條使用非上家推導目標的效果或攻擊路徑。

## Blocked by

- #25
