import { expect, test } from 'bun:test'
import type { PublicGameState } from '../app/types/fewfc'
import { emptyPublicState } from './game-room'
import { nextRoomOperationClock } from './game-timing'

const state: PublicGameState = {
  ...emptyPublicState(['player-1', 'player-2']),
  status: 'InProgress',
  phase: 'ActiveEffects',
  currentPlayer: 'player-1',
  turnNumber: 1,
}

test('操作限時保留同一回合的期限，並對每個待選擇與新回合重新計時', () => {
  const action = nextRoomOperationClock(60, undefined, state, 1_000)!
  expect(action).toMatchObject({ players: ['player-1'], deadlineMs: 61_000 })
  expect(nextRoomOperationClock(60, action, state, 20_000)).toEqual(action)

  const awaitingChoice: PublicGameState = {
    ...state,
    phase: 'Action',
    pendingChoice: { visibility: 'hidden', player: 'player-2', reason: { type: 'earthRendingCard' } },
  }
  const choice = nextRoomOperationClock(60, action, awaitingChoice, 30_000)!
  expect(choice).toMatchObject({ players: ['player-2'], deadlineMs: 90_000 })
  expect(nextRoomOperationClock(60, choice, awaitingChoice, 31_000, true)?.deadlineMs).toBe(91_000)
  expect(nextRoomOperationClock(60, choice, { ...state, currentPlayer: 'player-2', turnNumber: 2 }, 32_000))
    .toMatchObject({ kind: 'action', players: ['player-2'], deadlineMs: 92_000 })
  expect(nextRoomOperationClock(60, choice, { ...state, status: 'Finished' }, 32_000)).toBeUndefined()
})

test('準備選牌沿用共同期限，並可停用操作時計', () => {
  const preparation: PublicGameState = {
    ...state,
    status: 'Preparing',
    phase: 'TurnStart',
    currentPlayer: null,
    initialPouchSelection: { remainingPlayers: ['player-1', 'player-2'] },
  }
  const clock = nextRoomOperationClock(30, undefined, preparation, 1_000)!
  expect(clock).toMatchObject({ players: ['player-1', 'player-2'], deadlineMs: 31_000 })
  expect(nextRoomOperationClock(30, clock, {
    ...preparation, initialPouchSelection: { remainingPlayers: ['player-2'] },
  }, 20_000)).toMatchObject({ players: ['player-2'], deadlineMs: 31_000 })
  expect(nextRoomOperationClock(undefined, undefined, state, 20_000)).toBeUndefined()
  expect(nextRoomOperationClock(30, clock, undefined, 20_000)).toBeUndefined()
})
