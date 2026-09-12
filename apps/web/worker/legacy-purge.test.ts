import { expect, test } from 'bun:test'
import { legacyReplayDeleteStatements } from './legacy-purge'

test('legacy purge D1 mutations require explicit room and Replay associations', () => {
  expect(legacyReplayDeleteStatements([], [])).toEqual([])
  expect(legacyReplayDeleteStatements(['room-1'], ['replay-1'])).toEqual([
    {
      sql: 'DELETE FROM player_saved_replay WHERE source_game_id IN (?) AND replay_id IN (?)',
      bindings: ['room-1', 'replay-1'],
    },
    {
      sql: 'DELETE FROM replay_archive_lifecycle WHERE replay_id IN (?)',
      bindings: ['replay-1'],
    },
  ])
  expect(JSON.stringify(legacyReplayDeleteStatements(['room-1'], ['replay-1'])))
    .not.toMatch(/DELETE FROM public_game_room|DELETE FROM game_room_member/i)
})
