export interface ActiveGameVersion {
  gameInstanceId: string
  recordSequence: number
}

export interface RoomMetadataVersion {
  gameId: string
  updatedAt: string
}

/** 僅供瀏覽器使用的傳遞順序防護；標準提交仍由伺服器擁有。 */
export function shouldApplyActiveGameVersion(
  highest: ActiveGameVersion | undefined,
  incoming: ActiveGameVersion | undefined,
): boolean {
  return !(
    incoming
    && highest
    && incoming.gameInstanceId === highest.gameInstanceId
    && incoming.recordSequence < highest.recordSequence
  )
}

export function nextHighestActiveGameVersion(
  highest: ActiveGameVersion | undefined,
  incoming: ActiveGameVersion | undefined,
): ActiveGameVersion | undefined {
  if (!incoming || !shouldApplyActiveGameVersion(highest, incoming)) return highest
  return incoming
}

/** 同一個房間只接受不早於目前 metadata 的快照，避免慢回應蓋掉 WebSocket 更新。 */
export function shouldApplyRoomMetadataVersion(
  current: RoomMetadataVersion | undefined,
  incoming: RoomMetadataVersion,
): boolean {
  return !current || current.gameId !== incoming.gameId || incoming.updatedAt >= current.updatedAt
}
