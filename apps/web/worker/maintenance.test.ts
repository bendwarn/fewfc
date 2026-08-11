import { expect, test } from 'bun:test'
import { maintenanceBlocksPlayerMutation, maintenanceEnabled } from './maintenance'

test('maintenance is explicit and fails closed for Commands and Replay creation only', () => {
  expect(maintenanceEnabled(undefined)).toBe(false)
  expect(maintenanceEnabled('false')).toBe(false)
  expect(maintenanceEnabled('true')).toBe(true)
  expect(maintenanceBlocksPlayerMutation('POST', '/api/games/room-1/commands')).toBe(true)
  expect(maintenanceBlocksPlayerMutation('POST', '/api/replays')).toBe(true)
  expect(maintenanceBlocksPlayerMutation('GET', '/api/games/room-1/commands')).toBe(false)
  expect(maintenanceBlocksPlayerMutation('POST', '/api/games/room-1/start')).toBe(false)
})
