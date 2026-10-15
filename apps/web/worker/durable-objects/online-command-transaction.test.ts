import { describe, expect, test } from 'bun:test'
import { executePlayerCommand } from './online-command-transaction'
import { RulesEngineError } from '../rules-engine-error'

describe('executePlayerCommand', () => {
  test('returns the original receipt and the latest authoritative state after a lost response retry', async () => {
    const room = fakeRoom()
    const request = {
      commandId: 'command-1',
      gameInstanceId: 'game-1',
      transactionId: 'transaction-1',
      actorUserId: 'alice-user',
      action: { type: 'passAction' as const, reason: 'NoCardsInHand' as const },
    }

    const first = await executePlayerCommand(room, request)
    room.authoritativeMarker = 'newer-state'
    const retry = await executePlayerCommand(room, request)

    expect(room.rulesCalls).toBe(1)
    expect(first.receipt).toStrictEqual(retry.receipt)
    expect(retry.response.marker).toBe('newer-state')
  })

  test('rejects a reused command id with a different action identity', async () => {
    const room = fakeRoom()

    await executePlayerCommand(room, {
      commandId: 'command-1', gameInstanceId: 'game-1', transactionId: 'transaction-1',
      actorUserId: 'alice-user', action: { type: 'passAction', reason: 'NoCardsInHand' },
    })

    await expect(executePlayerCommand(room, {
      commandId: 'command-1', gameInstanceId: 'game-1', transactionId: 'transaction-1',
      actorUserId: 'alice-user', action: { type: 'passAction', reason: 'CannotActByStatus' },
    })).rejects.toMatchObject({ code: 'idempotencyConflict', statusCode: 409 })
  })

  test('returns a completed accepted receipt after the competition timeout freeze', async () => {
    const room = fakeRoom()
    const request = {
      commandId: 'completed-before-timeout', gameInstanceId: 'game-1', transactionId: 'transaction-complete',
      actorUserId: 'alice-user', action: { type: 'passAction' as const, reason: 'NoCardsInHand' as const },
    }
    const first = await executePlayerCommand(room, request)
    room.frozen = true

    const retry = await executePlayerCommand(room, request)

    expect(first.receipt?.transactionStatus).toBe('complete')
    expect(retry.receipt).toStrictEqual(first.receipt)
    expect(room.rulesCalls).toBe(1)
  })

  test('does not freeze a randomness receipt whose resolution transaction already completed', async () => {
    const room = fakeRoom()
    const request = {
      commandId: 'randomness-already-complete', gameInstanceId: 'game-1', transactionId: 'transaction-randomness-complete',
      actorUserId: 'alice-user', action: { type: 'passAction' as const, reason: 'NoCardsInHand' as const },
    }
    const first = await executePlayerCommand(room, request)
    const legacyReceipt = { ...first.receipt!, transactionStatus: 'awaitingRandomness' as const }
    await room.storage.put({ 'commandReceipt:game-1:randomness-already-complete': legacyReceipt })
    room.frozen = true

    const retry = await executePlayerCommand(room, request)

    expect(retry.receipt).toStrictEqual(legacyReceipt)
    expect(room.rulesCalls).toBe(1)
  })

  test('returns a rejected receipt after the competition timeout freeze', async () => {
    const room = fakeRoom()
    const request = {
      commandId: 'rejected-before-timeout', gameInstanceId: 'game-1', transactionId: 'transaction-rejected',
      actorUserId: 'alice-user', action: { type: 'passAction' as const, reason: 'NoCardsInHand' as const },
    }
    let ruleCalls = 0
    room.callRules = async (action: { type: string }) => {
      ruleCalls += 1
      if (action.type === 'passAction') throw new RulesEngineError({ validation: 'illegal pass' })
      return { type: 'ready', record: [], state: { status: 'InProgress', currentPlayer: 'alice', pendingChoice: null } }
    }
    const first = await executePlayerCommand(room, request)
    const callsAfterFirst = ruleCalls
    room.frozen = true

    const retry = await executePlayerCommand(room, request)

    expect(first.receipt?.outcome).toBe('rejected')
    expect(retry.receipt).toStrictEqual(first.receipt)
    expect(ruleCalls).toBe(callsAfterFirst)
  })

  test('drains every trusted shuffle in one command transaction and returns only a redacted room response', async () => {
    const room = randomnessRoom()

    const result = await executePlayerCommand(room, {
      commandId: 'command-random', gameInstanceId: 'game-1', transactionId: 'transaction-random',
      actorUserId: 'alice-user', action: { type: 'passAction', reason: 'NoCardsInHand' },
    })

    expect(result.receipt?.transactionStatus).toBe('awaitingRandomness')
    expect(room.randomnessRequests).toStrictEqual(['shuffle-1', 'shuffle-2'])
    expect(room.committedSequences).toStrictEqual([8, 9, 10])
    expect(room.commandActions).toStrictEqual(['passAction'])
    expect(result.response).toStrictEqual({
      state: { pendingRandomness: null },
      events: [
        { eventType: 'RustedForestStarted' },
        { eventType: 'RustedForestCompleted' },
      ],
    })
    expect(JSON.stringify(result.response)).not.toContain('currentOrder')
    expect(JSON.stringify(result.response)).not.toContain('needsRandomness')
  })

  test('queues the competition clock transition with the final trusted-randomness checkpoint', async () => {
    const room = randomnessRoom()

    await executePlayerCommand(room, {
      commandId: 'command-random-clock', gameInstanceId: 'game-1', transactionId: 'transaction-random-clock',
      actorUserId: 'alice-user', action: { type: 'passAction', reason: 'NoCardsInHand' },
    })

    expect(room.competitionTransitions).toHaveLength(1)
    const [, previous, next, sourceCommandId, sequence] = room.competitionTransitions[0] as [
      unknown,
      { status: string; phase: string; turnNumber: number; currentPlayer: string },
      { state: { status: string; currentPlayer: string } },
      string,
      number,
    ]
    expect(previous).toMatchObject({
      status: 'InProgress', phase: 'Action', turnNumber: 1, currentPlayer: 'alice',
    })
    expect(next.state).toMatchObject({ status: 'InProgress', currentPlayer: 'bob' })
    expect(sourceCommandId).toBe('trusted-randomness:shuffle-2')
    expect(sequence).toBe(10)
  })

  test('retains the clock transition source when trusted randomness resumes after a failed attempt', async () => {
    const room = randomnessRoom({ failFirstRandomnessResolution: true })
    const request = {
      commandId: 'command-random-clock-retry', gameInstanceId: 'game-1',
      transactionId: 'transaction-random-clock-retry', actorUserId: 'alice-user',
      action: { type: 'passAction' as const, reason: 'NoCardsInHand' as const },
    }

    await expect(executePlayerCommand(room, request)).rejects.toThrow('temporary randomness failure')
    expect(await room.storage.get('transaction:game-1')).toMatchObject({
      clockTransitionState: {
        status: 'InProgress', phase: 'Action', turnNumber: 1, currentPlayer: 'alice',
      },
    })

    await executePlayerCommand(room, request)

    expect(room.competitionTransitions).toHaveLength(1)
    const [, previous, next, sourceCommandId, sequence] = room.competitionTransitions[0] as [
      unknown,
      { status: string; phase: string; turnNumber: number; currentPlayer: string },
      { state: { status: string; currentPlayer: string } },
      string,
      number,
    ]
    expect(previous).toMatchObject({ currentPlayer: 'alice', phase: 'Action' })
    expect(next.state).toMatchObject({ currentPlayer: 'bob' })
    expect(sourceCommandId).toBe('trusted-randomness:shuffle-2')
    expect(sequence).toBe(10)
  })

  test('keeps a deterministic validation failure as a noncanonical rejection receipt', async () => {
    const room = fakeRoom()
    room.callRules = async (action: { type: string }) => {
      if (action.type === 'passAction') throw new RulesEngineError({ validation: 'illegal pass' })
      return { type: 'ready', record: [], state: { status: 'InProgress', currentPlayer: 'alice', pendingChoice: null } }
    }
    const request = {
      commandId: 'invalid-command', gameInstanceId: 'game-1', transactionId: 'transaction-invalid',
      actorUserId: 'alice-user', action: { type: 'passAction' as const, reason: 'NoCardsInHand' as const },
    }

    const first = await executePlayerCommand(room, request)
    const retry = await executePlayerCommand(room, request)

    expect(first.receipt?.outcome).toBe('rejected')
    expect(first.receipt?.observedSequence).toBe(7)
    expect(retry.receipt).toStrictEqual(first.receipt)
  })
})

function fakeRoom() {
  const values = new Map<string, unknown>()
  const metadata = {
    schemaVersion: 4 as const,
    gameId: 'room-1',
    gameInstanceId: 'game-1',
    name: 'Room', access: 'private' as const, capacity: 2 as const, ruleset: 'fewfc-base' as const,
    enabledRuleModules: [], players: ['alice', 'bob'], members: [
      { userId: 'alice-user', displayName: 'Alice', player: 'alice', ready: false, connected: true, owner: true },
    ], status: 'Active' as const, createdAt: 'now', updatedAt: 'now',
  }
  const record = {
    schemaVersion: 7 as const, gameInstanceId: 'game-1', sequence: 7, firstPlayer: 'alice', deckSeed: 'seed',
    setup: { players: [], turnOrder: [], enabledRuleModules: [], deckLists: [] }, rulesRecord: [],
  }
  values.set('nextSequence', 8)
  values.set('gameRecord', record)

  return {
    authoritativeMarker: 'first-state',
    frozen: false,
    rulesCalls: 0,
    storage: {
      async get<T>(key: string) { return values.get(key) as T | undefined },
      async put(entries: Record<string, unknown>) { Object.entries(entries).forEach(([key, value]) => values.set(key, value)) },
      async delete(key: string) { return values.delete(key) },
      async transaction<T>(callback: (storage: unknown) => Promise<T>) { return await callback(this) },
    },
    async metadata() { return metadata },
    async gameRecord() { return record },
    playerFor(_metadata: unknown, userId: string) { return userId === 'alice-user' ? 'alice' : undefined },
    actionForPlayer(action: unknown) { return action },
    async callRules(action: { type: string }) {
      if (action.type !== 'refresh') this.rulesCalls += 1
      return { type: 'ready', record: [], state: { status: 'InProgress', currentPlayer: 'bob', pendingChoice: null } }
    },
    async response() { return { marker: this.authoritativeMarker } },
    shuffle<T>(values: T[]) { return values },
    competitionCommandFrozen() { return this.frozen },
  }
}

function randomnessRoom(options: { failFirstRandomnessResolution?: boolean } = {}) {
  const values = new Map<string, unknown>()
  const metadata = {
    schemaVersion: 4 as const, gameId: 'room-1', gameInstanceId: 'game-1', name: 'Room',
    access: 'private' as const, capacity: 2 as const, ruleset: 'fewfc-base' as const,
    enabledRuleModules: [], players: ['alice', 'bob'], members: [
      { userId: 'alice-user', displayName: 'Alice', player: 'alice', ready: false, connected: true, owner: true },
    ], status: 'Active' as const, createdAt: 'now', updatedAt: 'now',
  }
  const record = {
    schemaVersion: 7 as const, gameInstanceId: 'game-1', sequence: 7, firstPlayer: 'alice', deckSeed: 'seed',
    setup: { players: [], turnOrder: [], enabledRuleModules: [], deckLists: [] }, rulesRecord: [],
  }
  const randomnessRequests: string[] = []
  const committedSequences: number[] = []
  const commandActions: string[] = []
  const competitionTransitions: unknown[][] = []
  let pendingRequestId: string | null = null
  let failFirstRandomnessResolution = options.failFirstRandomnessResolution ?? false
  values.set('nextSequence', 8)
  values.set('gameRecord', record)

  const needs = (requestId: string, currentOrder: number[]) => ({
    type: 'needsRandomness' as const,
    record: [{ requestId }],
    request: {
      requestId,
      operation: { type: 'deckShuffle' as const, deck: 'Shared' as const },
      currentOrder,
    },
  })
  const ready = {
    type: 'ready' as const,
    record: [{ complete: true }],
    state: { status: 'InProgress', currentPlayer: 'bob', pendingChoice: null },
  }

  return {
    randomnessRequests,
    committedSequences,
    commandActions,
    competitionTransitions,
    storage: {
      async get<T>(key: string) { return values.get(key) as T | undefined },
      async put(entries: Record<string, unknown>) {
        Object.entries(entries).forEach(([key, value]) => {
          values.set(key, value)
          if (key.startsWith('event:')) committedSequences.push((value as { sequence: number }).sequence)
        })
      },
      async delete(key: string) { return values.delete(key) },
      async transaction<T>(callback: (storage: unknown) => Promise<T>) { return await callback(this) },
    },
    async metadata() { return metadata },
    async gameRecord() { return values.get('gameRecord') as typeof record },
    playerFor(_metadata: unknown, userId: string) { return userId === 'alice-user' ? 'alice' : undefined },
    actionForPlayer(action: unknown) { return action },
    async callRules(action: { type: string; requestId?: string }) {
      if (action.type === 'refresh') {
        if (pendingRequestId) return needs(pendingRequestId, [1, 2])
        return {
          type: 'ready' as const,
          record: [],
          state: {
            status: 'InProgress', phase: 'Action', turnNumber: 1, currentPlayer: 'alice',
            initialPouchSelection: null, pendingChoice: null,
          },
        }
      }
      if (action.type === 'resolveRandomness') {
        randomnessRequests.push(action.requestId as string)
        if (failFirstRandomnessResolution) {
          failFirstRandomnessResolution = false
          throw new Error('temporary randomness failure')
        }
        if (action.requestId === 'shuffle-1') {
          pendingRequestId = 'shuffle-2'
          return needs('shuffle-2', [3, 4])
        }
        pendingRequestId = null
        return ready
      }
      if (action.type === 'passAction') {
        commandActions.push(action.type)
        pendingRequestId = 'shuffle-1'
        return needs('shuffle-1', [1, 2])
      }
      return ready
    },
    async response() {
      return {
        state: { pendingRandomness: null },
        events: [
          { eventType: 'RustedForestStarted' },
          { eventType: 'RustedForestCompleted' },
        ],
      }
    },
    competitionDelivery(...args: unknown[]) {
      competitionTransitions.push(args)
      return undefined
    },
    shuffle<T>(cards: T[]) { return [...cards].reverse() },
  }
}
