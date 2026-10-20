export function battleRecordCurrentTurnNumber(
  status: 'Preparing' | 'InProgress' | 'Finished' | null | undefined,
  turnNumber: number | null | undefined,
): number | null {
  return status === 'InProgress' ? turnNumber ?? null : null
}
