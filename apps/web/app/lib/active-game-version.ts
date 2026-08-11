export interface ActiveGameVersion {
  gameInstanceId: string
  recordSequence: number
}

/** A browser-only delivery-order guard; canonical commits remain server-owned. */
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
