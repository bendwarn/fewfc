# 31 Add serialization and filesystem persistence adapters

## Type

AFK

## Parent

源自 `docs/rules-engine-decisions.md` 第 4、33 和已結案 GitHub 議題 #22。

## What to build

在持久化資料結構外新增可實際序列化的介面卡。事件日誌應能編碼／解碼為穩定的外部資料；檔案系統介面卡應能儲存／載入正規事件日誌與選用快照，且不改變重播語意。

## Acceptance criteria

- [ ] 持久化資料結構衍生或實作穩定的序列化／反序列化。
- [ ] 檔案系統介面卡實作事件日誌儲存及快照儲存端口。
- [ ] 序列化事件日誌包含中繼資料、設定、已記錄事件及選用快照資料。
- [ ] 從序列化事件日誌載入時，透過重播重建狀態，不重新決定命令。
- [ ] 快照載入為選用，絕不取代權威事件日誌重播。
- [ ] 測試涵蓋序列化往返、檔案系統儲存／載入、缺少快照的行為及載入後重播等價性。

## Blocked by

- #30
