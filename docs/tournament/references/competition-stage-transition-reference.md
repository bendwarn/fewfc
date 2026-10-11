# 多階段來源名額解析：Bracket 參考

查證：2026-10-08，固定 [71b01fd](https://github.com/evroon/bracket/tree/71b01fd102d820bfd748ffef194a2ba09f87e6a2)。直接讀 source／tests，未執行。產品名額不足、空缺與裁判政策只見[設計](../competitive-play-design.md)；目前實作只見[關卡](../competition-review-gates.md)。

## 來源模型與解析

StageItemInput 有 tentative（來源 StageItem＋名次，無 team）、final（有 team，可保留來源）及 empty（無來源）。可借「先描述名額來源、後解析確定人選」；empty 與尚未決定不能混用。[input model](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/models/db/stage_item_inputs.py#L43-L93)

resolver 以 `winner_position - 1` 取排名陣列，越界 assert，來源 empty 回 400；這不證排名唯一、前階段全部完成或裁判案件已結。排名只按 points，Swiss 計分含 Elo，不能直接搬作本站名次。[resolver](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/logic/scheduling/handle_stage_activation.py#L41-L85)、[ranking](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/logic/ranking/calculation.py#L48-L99)

## plan、apply 與更正

可先建立 inactive Stage；activation 計算 tentative→team 更新、套用再 active。前端提供晉級 preview。Stage 有 active flag，StageType 是 RR／SE／Swiss，未列 DE。[helper](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/logic/scheduling/handle_stage_activation.py#L88-L139)、[Stage](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/models/db/stage.py#L9-L25)、[formats](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/models/db/stage_item.py#L13-L29)、[preview](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/frontend/src/components/modals/activate_next_stage_modal.tsx#L16-L110)

比分更新重算來源排名，SE 另更新後輪 inputs；activation helper 只重算 tentative，已 final 不在該集合。previous activation 會清被退出階段比分及來源 team，不是本站保留紀錄的退階段協議。[match route](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/routes/matches.py#L156-L185)、[deactivate](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/logic/scheduling/handle_stage_activation.py#L142-L153)

## 外部 tests 的實際證据

| Test | 有／沒有驗到 |
| --- | --- |
| [activate_next_stage](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/tests/integration_tests/api/activate_next_stage_test.py#L33-L138) | 一筆比分後 next 成功／型別；134–138 的後階段晉級隊伍斷言被註解，不證完整來源或 tie blocking。 |
| [inputs](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/tests/integration_tests/api/inputs_test.py#L22-L110) | 可用隊伍、PUT success、不存在隊伍拒絕；不驗來源名次解析或不可變公告。 |
| [ranking](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/tests/unit_tests/ranking_calculation_test.py#L26-L273) | 比對分數 dict；未驗並列如何分配名額／seed。 |

## Rust 重用取捨

自行設計小型來源解析／影響計畫 seam，保留既有賽程權威；解析、Stage 建立、房間交付、真正開局分開。名次要讀真 rank 而非陣列位置，解析證據在 apply 前重新核來源 authority；RR ranking revision 不等同 schedule generation。

可重用的是來源映射與 plan→apply，不是外部 Elo、清歷史、抽樣配對或授權。平台堆疊、授權評估只見 [Bracket 參考](bracket-feature-reference.md)，不另導入／翻譯 runtime。

實際本機查閱與失敗見[研究日誌](../../../scratch/tour-attempt/linnaeus-stage-transition-research.md)；參考完成不等於跨階段產品驗證。
