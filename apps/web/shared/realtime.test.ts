import { expect, test } from 'bun:test'
import { parseSocketRequest } from './realtime'

test('wire operations accept explicit readiness and reject actor spoofing or internal actions', () => {
  const request = { type: 'request', requestId: 'request-a', operation: 'ready', input: { ready: true } } as const
  expect(parseSocketRequest(request, 'room')).toEqual(request)
  expect(parseSocketRequest({ ...request, input: { ready: true, actorUserId: 'other' } }, 'room')).toBeUndefined()
  expect(parseSocketRequest({ ...request, operation: 'createGame' }, 'room')).toBeUndefined()
  expect(parseSocketRequest({ ...request, operation: 'command', input: {
    commandId: 'command-a', transactionId: 'transaction-a', gameInstanceId: 'game-a', action: { type: 'seedDevelopmentScenario' },
  } }, 'room')).toBeUndefined()
})

test('lobby requests are bounded, typed and separated from room commands', () => {
  const request = { type: 'request', requestId: 'request-a', operation: 'create', input: { name: '客棧', capacity: 4, ruleVersion: '5.17' } } as const
  expect(parseSocketRequest(request, 'lobby')).toEqual(request)
  expect(parseSocketRequest(request, 'room')).toBeUndefined()
  expect(parseSocketRequest({ ...request, input: { capacity: 99 } }, 'lobby')).toBeUndefined()
  expect(parseSocketRequest({ ...request, input: { name: 'a'.repeat(201) } }, 'lobby')).toBeUndefined()
})

test('clock reads use the connected room binding without client identity or time overrides', () => {
  const request = { type: 'request', requestId: 'clock-a', operation: 'clock' } as const
  expect(parseSocketRequest(request, 'room')).toEqual(request)
  for (const input of [{ competitionId: 'another' }, { matchId: 'another' }, { callerUserId: 'another' }, { trustedTimeMs: 0 }]) {
    expect(parseSocketRequest({ ...request, input }, 'room')).toBeUndefined()
  }
})
