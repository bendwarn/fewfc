/**
 * 建立只針對已明確刪除房間的 D1 清理陳述式。空集合會產生空陣列，讓
 * 管理端在沒有可證明關聯時保持 fail-closed，而不是意外清空所有回放資料。
 */
export function legacyReplayDeleteStatements(
  sourceGameIds: readonly string[],
  replayIds: readonly string[],
): Array<{ sql: string, bindings: string[] }> {
  if (sourceGameIds.length === 0 || replayIds.length === 0) return []

  const roomPlaceholders = sourceGameIds.map(() => '?').join(', ')
  const replayPlaceholders = replayIds.map(() => '?').join(', ')
  return [
    {
      sql: `DELETE FROM player_saved_replay WHERE source_game_id IN (${roomPlaceholders}) AND replay_id IN (${replayPlaceholders})`,
      bindings: [...sourceGameIds, ...replayIds],
    },
    {
      sql: `DELETE FROM replay_archive_lifecycle WHERE replay_id IN (${replayPlaceholders})`,
      bindings: [...replayIds],
    },
  ]
}
