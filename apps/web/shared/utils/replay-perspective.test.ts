import { expect, test } from 'bun:test'
import { resolveReplayPerspective } from './replay-perspective'

test('replay perspective defaults to first player and rejects an archived outsider', () => {
  const players = ['player-1', 'player-10']
  expect(resolveReplayPerspective(undefined, 'player-10', players)).toBe('player-10')
  expect(resolveReplayPerspective('player-1', 'player-10', players)).toBe('player-1')
  expect(resolveReplayPerspective('outsider', 'player-10', players)).toBeUndefined()
})
