export interface ActiveGameVersion {
  gameInstanceId: string
  recordSequence: number
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
