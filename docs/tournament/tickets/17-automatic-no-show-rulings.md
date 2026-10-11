# 17：報到截止自動處理缺席

**Priority:** P2

**What to build:** 尚未開局的交手在公告時間到達時，依全隊目前連線與準備狀態自動判缺席；一隊未齊判該隊負，兩隊未齊皆負。結果及原因可查看，不補造真正開局、時計或對局紀錄。

**Blocked by:** [02：開局前斷線取消準備與報到](02-disconnect-clears-ready.md)、[06：固定輪次自動開房、發布時間與玩家入口](06-round-rooms-and-notifications.md)

**Status:** ready-for-agent

**類型：** 新增／接線

## 驗收條件

- [ ] 只處理到 T 仍未真正開局的交手，依固定成員目前連線及準備狀態判全隊是否報到。
- [ ] 一隊未完成判該隊負，兩隊未完成判 DoubleLoss，原因與遊戲內計時超時分開。
- [ ] 截止與真正開局競態只判定一次；提前已開局不再因後續斷線被判賽前缺席。
- [ ] 網站可查結果與原因，不補造 GameStarted、時計消耗或 Game Record；個人／團體未齊、雙缺席及一般重試案例通過。

共用驗收與工作規則見[票據索引](README.md#共用驗收與工作規則)；產品政策見[競賽設計](../competitive-play-design.md)，現況與放行證據見[審核關卡](../competition-review-gates.md)。
