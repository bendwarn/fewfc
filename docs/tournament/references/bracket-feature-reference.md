# Bracket：排程、更正與公開模型

查證：2026-10-03。這是固定來源的觀察，不是 FEWFC 規格或驗收；讀到外部 tests 不等於執行過。多階段詳細證據只放[階段轉移參考](competition-stage-transition-reference.md)，目前進度只讀[關卡](../competition-review-gates.md)。

## 固定版本與授權

| 來源 | 固定提交／用途 |
| --- | --- |
| evroon/bracket | [71b01fd](https://github.com/evroon/bracket/commit/71b01fd102d820bfd748ffef194a2ba09f87e6a2)，2026-10-01；主參考 |
| brackets-manager.js | [02027cf](https://github.com/Drarig29/brackets-manager.js/commit/02027cff5d687068719bebbc765072f382e81390)，2026-09-20；下游鎖定／reset 補充 |

Bracket 是 Python／FastAPI／SQLAlchemy／PostgreSQL、React／Vite 平台，不是可直接放進 Rust/WASM 的純邏輯函式庫。[後端 metadata](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/pyproject.toml)、[前端 metadata](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/frontend/package.json)

固定 [Bracket LICENSE](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/LICENSE) 為 AGPLv3；§0、§5、§13 分別涉及改作、修改版本傳遞及遠端互動的源碼提供。換語言或透過 API 不自動免除義務。本次只借概念，未複製／翻譯 source 或 fixtures；導入前仍需確認實際使用方式的授權相容性。補充 manager 的[固定 LICENSE](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/LICENSE) 是 MIT，也須保留其要求。

## 排程：plan 不是實際終局／交付

Match 有 start、duration、margin、court；end 由預計開始與時長計算。排程函式回傳時間／場地變更 tuples，判斷上一輪仍進行也比較計算 end。可借 plan→apply 分離，不能當成實際前輪完成或全房持久就緒的證據。[Match](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/models/db/match.py#L13-L38)、[planning](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/logic/planning/rounds.py#L24-L94)

Swiss 建輪先建立 draft、逐筆套用排程，再取消 draft。已讀入口未證成 FEWFC 跨 DO 開局協議。[route](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/routes/stage_items.py#L133-L238)。正式 tests 比對 tuples／margin 與建輪回應，不驗遠端 ACK、全員 ready、重啟或 response loss。[rescheduling tests](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/tests/unit_tests/rescheduling_test.py#L49-L119)

## 更正：可借相依圖，不借清歷史

Bracket score PUT 更新比分再重算排名；SE 的純函式先算受影響 matches map，另一函式套用 inputs。切回前階段會清比分並解除來源隊伍，不能等同本站保留原始結論／更正歷史或撤銷實體房間。[match route](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/routes/matches.py#L156-L185)、[pure plan／apply](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/logic/ranking/elimination.py#L13-L86)、[deactivation](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/logic/scheduling/handle_stage_activation.py#L142-L153)

manager reset 在非已完成 BYE 的後繼達 Running 以上時拒絕。其 Ready 是庫內參賽／結果狀態，不是玩家連線或房間就緒；storage update 與本程序事件也不是跨 DO transaction／ACK。[reset](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/src/reset.ts#L16-L52)、[status](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/src/helpers.ts#L732-L795)、[storage events](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/src/manager.ts#L179-L204)

manager 的 double_forfeit 下游設 BYE。這是該庫行為，不取代本站 DoubleLoss 事實與政策。[downstream rule](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/src/base/updater.ts#L301-L309)。正式 update／lock tests 只證庫內依賴與已開始鎖定。[tests](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/test/update.spec.js#L80-L116)

## 雙敗與重置賽：交手資料，不是另一種房間

2026-10-09 補查同一固定版本的 source；未執行外部程式或 tests。`evroon/bracket` 的官方 [README](https://github.com/evroon/bracket#readme) 列單敗、循環及瑞士，不能把下述 manager 能力算成 Bracket 已實作的雙敗。

- manager 以勝部、敗部及決賽群組表示雙敗。`grandFinal: 'double'` 在建賽程時預留兩場邏輯交手，第二場先放待定人選，不是建立遠端遊戲房。[creator](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/src/base/stage/creator.ts#L201-L214)、[第二場預留](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/src/base/stage/creator.ts#L907-L920)
- 更新結果時分別推進勝方與敗方；決賽群組的後繼更新會把原兩位對手放入下一場。選擇決勝交手另有明確判斷：第一場敗部側 `opponent2` 勝才取第二場，否則取第一場。不能將「第二場已有參賽資料」誤當「第二場必須開房」。[updater](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/src/base/updater.ts#L326-L360)、[decisive match](https://github.com/Drarig29/brackets-manager.js/blob/02027cff5d687068719bebbc765072f382e81390/src/helpers.ts#L994-L1007)
- manager 官方文件明示交手的開始／結束日期不在庫的範圍；Ready／Running 等是賽程資料狀態，沒有替應用端完成房間建置、玩家報到、五分鐘就緒或跨房收據。[Match 範圍](https://drarig29.github.io/brackets-docs/user-guide/glossary/#match)

本站可借其後繼映射、待定名額及決勝條件；共用 GameRoom，僅由編排層為真正需要的交手呼叫既有開房／結果回收。改判保護與跨房協議的差異見上節，不另實作「重置賽房型」，也不以資料庫更新成功推定遠端房間已撤銷或已開局。

## 授權與公開：模型不等價

Bracket 使用 club OWNER／COLLABORATOR 與 membership，不能直接替代本站指定裁判／自身交手衝突。[roles](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/models/db/user_x_club.py#L8-L16)、[auth](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/routes/auth.py#L101-L113)

dashboard_public 與 OPEN／ARCHIVED 分開；公開 gate 未檢查整場 Finished，archive／unarchive 不是本站正式終結或重開許可。[public guard](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/routes/auth.py#L131-L152)、[status route](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/routes/tournaments.py#L136-L155)

## 重用結論

借模型、preview、純變更計畫與受影響圖；需要新 seam 時自行設計在既有 Rust 權威旁。直接導入平台或逐行翻譯不提供本站時計、授權、原始紀錄與跨房交付保證。現行產品決定只見[設計](../competitive-play-design.md)。
