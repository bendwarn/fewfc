import { expect, test } from 'bun:test'
import type { BattleRecord } from '../../app/types/fewfc'
import { replaceBattleRecordPlayerLabels, replacePlayerLabels } from './battle-record-display'

test('replay battle record keeps perspective words and replaces longer player IDs first', () => {
  const record: BattleRecord = {
    preparation: {
      entries: [{
        id: 'preparation',
        title: '你看見 player-10 與 player-1',
        summary: 'player-10 對 player-1 造成影響。',
      }],
    },
    turns: [],
  }

  const displayed = replaceBattleRecordPlayerLabels(record, [
    { player: 'player-1', displayName: '小明' },
    { player: 'player-10', displayName: '小華' },
  ])

  expect(displayed.preparation.entries[0]).toEqual({
    id: 'preparation',
    title: '你看見 小華 與 小明',
    summary: '小華 對 小明 造成影響。',
  })
  expect(replacePlayerLabels('player-10 幫助 player-1', [
    { player: 'player-1', displayName: '小明' },
    { player: 'player-10', displayName: '小華' },
  ])).toBe('小華 幫助 小明')
})
