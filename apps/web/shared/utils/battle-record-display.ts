import type { BattleRecord } from '../../app/types/fewfc'

/** 僅替換完整玩家識別碼；先處理較長 ID，避免 player-1 破壞 player-10。 */
export function replacePlayerLabels(
  value: string,
  players: Array<{ player: string; displayName: string }>,
): string {
  const labels = new Map(players.map(player => [player.player, player.displayName]))
  const ids = [...labels.keys()].sort((left, right) => right.length - left.length)
  const expression = ids.length
    ? new RegExp(ids.map(id => id.replace(/[.*+?^${}()|[\]\\]/g, '\\$&')).join('|'), 'g')
    : null
  return expression ? value.replace(expression, id => labels.get(id) ?? id) : value
}

export function replaceBattleRecordPlayerLabels(
  record: BattleRecord,
  players: Array<{ player: string; displayName: string }>,
): BattleRecord {
  const text = (value: string) => replacePlayerLabels(value, players)
  const entry = (value: BattleRecord['preparation']['entries'][number]) => ({
    ...value,
    title: text(value.title),
    summary: value.summary ? text(value.summary) : undefined,
  })
  return {
    preparation: { entries: record.preparation.entries.map(entry) },
    turns: record.turns.map(turn => ({ ...turn, title: text(turn.title), entries: turn.entries.map(entry) })),
  }
}
