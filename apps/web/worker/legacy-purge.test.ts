import { expect, test } from 'bun:test'
import {
  legacyReplayDeleteStatements,
  resetLegacyActiveRoomStatusStatement,
} from './legacy-purge'

test('legacy purge D1 mutations are restricted to replay references and room status', () => {
  expect(legacyReplayDeleteStatements).toEqual([
    'DELETE FROM player_saved_replay',
    'DELETE FROM replay_archive_lifecycle',
  ])
  expect(resetLegacyActiveRoomStatusStatement).toBe(
    "UPDATE public_game_room SET status = 'Waiting' WHERE status IN ('Active', 'Finished')",
  )
  expect([...legacyReplayDeleteStatements, resetLegacyActiveRoomStatusStatement].join('\n'))
    .not.toMatch(/auth|profile|deck_list|DELETE FROM public_game_room/i)
})
