import { expect, test } from 'bun:test'
import { legacyReplayDeleteStatements } from './legacy-purge'

test('legacy purge D1 mutations are restricted to replay references', () => {
  expect(legacyReplayDeleteStatements).toEqual([
    'DELETE FROM player_saved_replay',
    'DELETE FROM replay_archive_lifecycle',
  ])
  expect(legacyReplayDeleteStatements.join('\n'))
    .not.toMatch(/auth|profile|deck_list|DELETE FROM public_game_room/i)
})
