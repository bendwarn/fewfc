# 首輪名單與牌組凍結：參考

查證：2026-10-05。只讀外部 source／正式 tests，未執行。F、排除原因與隊長資料確認的產品決定只見[設計](../competitive-play-design.md)；原研究的獨立 freeze plan／proof 候選已不採用。

## Bracket：身份關聯，不是參賽同意或到場證據

固定版本與授權只見 [Bracket 參考](bracket-feature-reference.md)。

- 帳號 signup／session 建立 token，tests 驗憑證及 tournament 權限；不等於參賽 membership、鎖定名單或到房。[signup](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/routes/users.py#L80-L105)、[auth tests](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/tests/integration_tests/api/auth_test.py#L25-L101)
- 隊伍更新 player ID 集合，驗相同 tournament；tests 有非法玩家／跨賽事隔離。可借 owner／identity 關聯，這些 seam 不提供本站順序座次、逐人確認或牌組凍結契約。[team routes](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/bracket/routes/teams.py#L58-L110)、[team tests](https://github.com/evroon/bracket/blob/71b01fd102d820bfd748ffef194a2ba09f87e6a2/backend/tests/integration_tests/api/teams_test.py#L54-L185)

## Limitless：資料程序、報到與 timer 分開

| 官方來源 | 可確認行為／適配界線 |
| --- | --- |
| [Tournament Registration](https://docs.limitlesstcg.com/player/registration#team-tournaments) | 帳號之外另有賽事報名／team selection；隊長、加入順序影響其座次，不直接採其 3v3 對局或隊長轉移規則。 |
| [Decklist Submission](https://docs.limitlesstcg.com/player/decklists) | 窗口內重交取最後一份，未及時提交者被移除；是外部政策，不是本站排除規則的授權來源。 |
| [Tournament Check-in](https://docs.limitlesstcg.com/player/procedures#tournament-check-in) | 賽前 tournament check-in 與開始後 match check-in 是不同程序。 |
| [Settings](https://docs.limitlesstcg.com/organizer/reference#registration) | 有 late registration／check-in；[admin 名單](https://docs.limitlesstcg.com/organizer/players)編輯不表示本站 locked snapshot 可改。 |
| [Schedule](https://docs.limitlesstcg.com/organizer/schedule) | pairing availability 與 timer start 分離，文件允許 timer 前開打及自動調整時間；不證成本站實際開局或全房交付門檻。 |

已讀來源沒有提供本站全部遠端固定房持久 ACK、Waiting 撤銷／GameStarted 競態協議。查閱不到不表示外部全專案不存在。

## 本站接線入口

重用既有 registration 核心、LockedRoster 及 room presence／ready，避免新增第 2 名單或資格副本。讀源碼查實際契約，唯讀研究不能批准新 API 或宣稱跨 DO 原子。持久化與網站接線的目前 owner／驗證範圍只見[關卡](../competition-review-gates.md)。

實際本機查閱與失敗見 [Maxwell 工作紀錄](../../../scratch/tour-attempt/maxwell-opening-roster-freeze-research.md)，不是現行待決清單。
