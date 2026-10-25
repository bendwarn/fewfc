# 即時通訊

房間、對局、大廳與通知由兩條已驗證登入身分的 WebSocket 處理。
前端 `useRealtimeSocket` 使用 VueUse `useWebSocket` 管理連線、重連與卸載；
後端使用 H3 `defineWebSocketHandler` 與 CrossWS Cloudflare adapter，仍在原有
`GameRoom`／`PlayerNotifications` Durable Object 內執行並保留休眠支援。

## 通道與操作

| 通道 | 請求 | 推播 |
| --- | --- | --- |
| `/api/games/:id/socket` | `state`、`command`、`ready`、`start`、`reset`、`leave`、`dissolve`、`remove`、`rules`、`competitionReady`、`clock` | `roomState`、`roomDissolved` |
| `/api/notifications/socket` | `list`、`create`、`joinCode`、`enter`、`saveReplay` | `notification`、`roomsChanged` |

`command` 包含既有出牌、待選擇回覆、錦囊與可用行動查詢。`ready` 傳入明確
`{ ready: boolean }`，不以重送切換狀態。競賽準備與時鐘繼續使用伺服器持有的
競賽綁定，不接受客戶端指定另一場賽事。

HTTP 保留登入／登出、靜態規則目錄、牌組、獨立戰報與賽事管理等未遷移範圍。
舊房間 HTTP endpoints 仍可供相容與測試佈置使用，產品頁面不再透過它們提交
上述操作；WS 與既有入口共用 DO 的權限、序列化執行與規則引擎。

## 請求與回覆

公開型別與操作白名單位於 `shared/realtime.ts`。

```ts
{ type: 'request', requestId, operation, input }
{ type: 'result', requestId, ok: true, data }
{ type: 'result', requestId, ok: false, error: { statusCode, code, message } }
```

`requestId` 只負責配對這次傳輸。遊戲命令仍保留 `commandId`、
`gameInstanceId`、`transactionId`；重送相同命令會沿用既有收據，不能再執行一次。
完整收據含私人指令內容，因此 WS 回覆只提供完成結果與該玩家可見的房間投影。

一般房間操作的發起 socket 收到一份包含權威狀態的 `result`；同一操作的
`roomState` 廣播排除該 socket，其他玩家、觀戰者及同帳號其他分頁照常同步。
初次連線、重連及其他來源的更新仍可能收到 `roomState`，前端繼續用既有版本
檢查拒收過期狀態。競賽準備保留 exact `{ ready }` 確認與獨立房間狀態同步。
大廳索引更新在一般房間回覆送出後於背景依序完成，不延長出牌操作鎖，
也不讓同房較早提交的索引寫入排到較新的寫入之後。

## 失敗與生命週期

- 握手檢查登入及來源；每次業務請求重新查核 session 是否仍存在且未過期。
  使用者身分由伺服器 attachment 提供，公開輸入不能指定 `actorUserId`。
- 房間權限、觀戰限制、競賽固定席位及規則引擎校驗維持原有檢查。
  維護模式拒絕寫操作；單一訊息限制 64 KiB。
- 斷線立即拒絕未完成請求；連線按 1、2、4、8、10 秒間隔重試。
  不緩存或自動重送操作，避免在重連後執行已經過時的意圖。
- 連線等待和結果等待各有 15 秒逾時。出牌結果逾時會查詢最新狀態；
  玩家明確重試相同操作時沿用遊戲命令身分。
- AbortSignal 取消的是本地等待，不代表伺服器撤回已送出的命令。
  已結束或舊連線的回覆不能完成新的請求。
- 查詢、遊戲提交與房間操作各自持有忙碌狀態；查詢先完成不能解除遊戲提交鎖。
  待選擇期間不再發送一般手牌可用行動查詢。出牌動畫維持原有行為。

## 驗證

`socket-requests.test.ts` 驗證結果配對、斷線、逾時、取消與錯誤；
`useGameRoomCompetitionContext.nuxt.spec.ts` 驗證查詢與提交交錯時的操作鎖。
`realtime-transport.spec.ts` 在封鎖業務 HTTP 的真實登入／Rust／DO 流程中驗證
大廳、準備、開局、出牌後立即棄牌、發起端去除重複投影、對手同步、命令重送、
重新載入及登出後 session 撤銷。
其餘規則、觀戰、通知與路由測試直接觀測 WS 請求及結果。
