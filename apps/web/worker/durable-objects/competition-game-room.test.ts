import { expect, mock, test } from 'bun:test'
import type {
  CompetitionClockControlRequest,
  CompetitionGameBinding,
  CompetitionGameClockControlEligibilityRequest,
  CompetitionGameCreateRequest,
} from '../../shared/competition-game-room'
import { competitionGameBindingRefForRequest } from '../../shared/competition-game-room'
import {
  isCompetitionRoomInitialGameStartedRequest,
  type CompetitionRoomInitialGameStartedRequest,
} from '../../shared/competition-room'
import { emptyPublicState, type GameRecord, type GameRoomMetadata } from '../../shared/game-room'
import type { PublicGameState } from '../../app/types/fewfc'

let rulesEngineMode:
  | 'normal'
  | 'finished'
  | 'finishedWithoutConclusion'
  | 'finishedMismatchedRecord'
  | 'finishedGameEndedFollowedByEvent'
  | 'finishedWithoutGameEnded'
  | 'randomFinished'
  | 'randomInterrupted'
  | 'clockTransition' = 'normal'
let randomResolutionCalls = 0
let rulesEngineStartFailureCount = 0
let rulesEngineStartGate: Promise<void> | null = null
let rulesEngineStartGateEntered: (() => void) | null = null
let rulesEngineStartStateOverride: Partial<PublicGameState> | null = null
let rulesModuleGate: Promise<void> | null = null
let rulesModuleGateEntered: (() => void) | null = null
const rulesEngineStartCalls: Array<Record<string, unknown>> = []
let rulesEngineBoundaryCalls = 0
let competitionContextPrivateHands = false

mock.module('cloudflare:workers', () => ({
  DurableObject: class {
    protected ctx: unknown
    protected env: unknown

    constructor(ctx: unknown, env: unknown) {
      this.ctx = ctx
      this.env = env
    }
  },
}))

mock.module('../rules-engine', () => ({
  callRuleModuleResolution: async (modules: string[] = []) => {
    rulesEngineBoundaryCalls += 1
    if (rulesModuleGate) {
      rulesModuleGateEntered?.()
      await rulesModuleGate
    }
    return { modules }
  },
  callCompetitionEngineWasm: async () => {
    rulesEngineBoundaryCalls += 1
    return {
      ok: false,
      error: { code: 'testUnavailable', layer: 'test', message: 'not used by GameRoom tests' },
    }
  },
  callRulesEngine: async (request: {
    action?: { type?: string; requestId?: string }
    viewer?: string
    record?: unknown[]
    setup?: unknown
    deckSeed?: string
  }) => {
    rulesEngineBoundaryCalls += 1
    if (request.action?.type === 'start') {
      rulesEngineStartCalls.push(structuredClone(request) as unknown as Record<string, unknown>)
      if (rulesEngineStartFailureCount > 0) {
        rulesEngineStartFailureCount -= 1
        throw new Error('simulated rules engine start failure')
      }
      if (rulesEngineStartGate) {
        rulesEngineStartGateEntered?.()
        await rulesEngineStartGate
      }
    }
    if (rulesEngineMode === 'randomInterrupted') {
      const hasPendingRandomness = request.record?.some(entry => (
        typeof entry === 'object' && entry !== null && 'pendingRandomness' in entry
      ))
      if (request.action?.type === 'passAction') {
        return {
          type: 'needsRandomness',
          record: [{ pendingRandomness: true }],
          request: {
            requestId: 'shuffle-interrupted',
            operation: { type: 'deckShuffle', deck: 'Shared' },
            currentOrder: [1, 2],
          },
        }
      }
      if (request.action?.type === 'refresh' && hasPendingRandomness) {
        return {
          type: 'needsRandomness',
          record: [{ pendingRandomness: true }],
          request: {
            requestId: 'shuffle-interrupted',
            operation: { type: 'deckShuffle', deck: 'Shared' },
            currentOrder: [1, 2],
          },
        }
      }
      if (request.action?.type === 'resolveRandomness') {
        randomResolutionCalls += 1
        if (randomResolutionCalls === 1) throw new Error('simulated response loss during trusted randomness drain')
      }
    }
    if (rulesEngineMode === 'randomFinished' && request.action?.type === 'passAction') {
      return {
        type: 'needsRandomness',
        record: [{ requestId: 'shuffle-terminal' }],
        request: {
          requestId: 'shuffle-terminal',
          operation: { type: 'deckShuffle', deck: 'Shared' },
          currentOrder: [1, 2],
        },
      }
    }
    const finished = (rulesEngineMode === 'finished'
      || rulesEngineMode === 'finishedWithoutConclusion'
      || rulesEngineMode === 'finishedMismatchedRecord'
      || rulesEngineMode === 'finishedGameEndedFollowedByEvent'
      || rulesEngineMode === 'finishedWithoutGameEnded'
      || (rulesEngineMode === 'randomFinished'
        && (request.action?.type === 'resolveRandomness' || request.action?.type === 'refresh')))
      && request.action?.type !== 'start'
      && request.action?.type !== 'trustedRandomHandCandidates'
    const stateConclusion = finished && rulesEngineMode !== 'finishedWithoutConclusion'
      ? { outcome: { type: 'winner', team: 'team-a' }, causes: [{ type: 'directVictory', rule: 'test', team: 'team-a' }] }
      : null
    const recordConclusion = rulesEngineMode === 'finishedMismatchedRecord'
      ? { outcome: { type: 'winner', team: 'team-b' }, causes: [{ type: 'directVictory', rule: 'test', team: 'team-b' }] }
      : stateConclusion
    const recordEvents = rulesEngineMode === 'finishedWithoutConclusion'
      ? [{ GameEnded: {} }]
      : rulesEngineMode === 'finishedWithoutGameEnded'
        ? []
      : recordConclusion
        ? [
            { GameEnded: { conclusion: recordConclusion } },
            ...(rulesEngineMode === 'finishedGameEndedFollowedByEvent' ? [{ AfterGameEnded: {} }] : []),
          ]
        : []
    const setup = request.setup as {
      ruleVersion?: PublicGameState['ruleVersion']
      players?: PublicGameState['players']
      turnOrder?: PublicGameState['turnOrder']
      enabledRuleModules?: string[]
      initialHp?: PublicGameState['hp']
    } | undefined
    const turnOrder = setup?.turnOrder ?? ['player-a', 'player-b']
    const isPreparationState = rulesEngineMode === 'clockTransition'
      && (request.action?.type === 'start' || request.action?.type === 'refresh')
    const state: PublicGameState = {
      ...emptyPublicState(turnOrder),
      ruleVersion: setup?.ruleVersion ?? '5.17',
      enabledRuleModules: setup?.enabledRuleModules ?? [],
      status: isPreparationState ? 'Preparing' : finished ? 'Finished' : 'InProgress',
      phase: isPreparationState ? 'TurnStart' : 'ActiveEffects',
      currentPlayer: isPreparationState
        ? null
        : rulesEngineMode === 'clockTransition' && request.action?.type !== 'start'
            && request.action?.type !== 'refresh'
          ? 'player-b'
          : turnOrder[0] ?? null,
      turnNumber: rulesEngineMode === 'clockTransition'
        && request.action?.type !== 'start'
        && request.action?.type !== 'refresh'
        ? 2
        : 1,
      players: setup?.players ?? emptyPublicState(turnOrder).players,
      turnOrder,
      hp: setup?.initialHp ?? emptyPublicState(turnOrder).hp,
      initialPouchSelection: isPreparationState
        ? { remainingPlayers: [...turnOrder] }
        : null,
      gameConclusion: stateConclusion,
    }
    if (competitionContextPrivateHands) {
      // Rules Engine 邊界 fixture：僅合法 viewer 的手牌可見，GameRoom 不得合併不同玩家的 view。
      state.hands = turnOrder.map((player, index): PublicGameState['hands'][number] => ({
        player,
        cards: player === request.viewer
          ? { kind: 'known', cards: [{ id: index + 1, label: `私有手牌:${player}`, element: null, level: null, secretStrategies: [] }] }
          : { kind: 'hidden', count: 1 },
      }))
    }
    const returnedState = request.action?.type === 'start' && rulesEngineStartStateOverride
      ? { ...state, ...rulesEngineStartStateOverride }
      : state
    return {
      type: 'ready',
      record: finished ? [{ events: recordEvents }] : [],
      state: returnedState,
      battleRecord: { preparation: { entries: [] }, turns: [] },
      playableActions: [],
      interaction: { canChooseInitialPouch: false },
    }
  },
}))

const { GameRoom, competitionTerminalIdentity } = await import('./game-room')

class MemoryStorage {
  readonly values = new Map<string, unknown>()
  alarm: number | null = null
  writeCount = 0
  failNextBatch = false
  failNextAlarm = false
  failNextGet = false
  readRequests: Array<string | string[]> = []
  sqlAccessCount = 0

  get sql(): never {
    this.sqlAccessCount += 1
    throw new Error('unexpected SQL access in GameRoom KV fixture')
  }

  get<T>(key: string): Promise<T | undefined>
  get<T>(keys: string[]): Promise<Map<string, T>>
  async get<T>(key: string | string[]): Promise<T | undefined | Map<string, T>> {
    this.readRequests.push(typeof key === 'string' ? key : [...key])
    if (this.failNextGet) {
      this.failNextGet = false
      throw new Error('simulated snapshot read failure')
    }
    if (typeof key === 'string') return this.values.get(key) as T | undefined
    return new Map(key.filter(entry => this.values.has(entry))
      .map(entry => [entry, this.values.get(entry) as T]))
  }

  async put(key: string | Record<string, unknown>, value?: unknown): Promise<void> {
    this.writeCount += 1
    if (typeof key !== 'string' && this.failNextBatch) {
      this.failNextBatch = false
      throw new Error('simulated atomic storage failure')
    }
    if (typeof key === 'string') {
      this.values.set(key, value)
      return
    }
    for (const [entry, entryValue] of Object.entries(key)) {
      this.values.set(entry, entryValue)
    }
  }

  async delete(key: string | string[]): Promise<boolean> {
    this.writeCount += 1
    if (Array.isArray(key)) {
      let deleted = false
      for (const entry of key) deleted = this.values.delete(entry) || deleted
      return deleted
    }
    return this.values.delete(key)
  }

  async setAlarm(scheduledTime: number | Date): Promise<void> {
    this.writeCount += 1
    if (this.failNextAlarm) {
      this.failNextAlarm = false
      throw new Error('simulated alarm transaction failure')
    }
    this.alarm = scheduledTime instanceof Date ? scheduledTime.getTime() : scheduledTime
  }

  async deleteAlarm(): Promise<void> {
    this.writeCount += 1
    if (this.failNextAlarm) {
      this.failNextAlarm = false
      throw new Error('simulated alarm transaction failure')
    }
    this.alarm = null
  }

  async transaction<T>(callback: (storage: MemoryStorage) => Promise<T>): Promise<T> {
    const values = new Map(this.values)
    const alarm = this.alarm
    try {
      return await callback(this)
    } catch (error) {
      this.values.clear()
      for (const [key, value] of values) this.values.set(key, value)
      this.alarm = alarm
      throw error
    }
  }

  async list<T>(options?: { prefix?: string }): Promise<Map<string, T>> {
    return new Map([...this.values.entries()]
      .filter(([key]) => !options?.prefix || key.startsWith(options.prefix))
      .map(([key, value]) => [key, value as T]))
  }
}

class AdvancingStartSampleStorage extends MemoryStorage {
  advanceNextSequenceRead = false
  advanceLateTransactionList = false

  constructor(private readonly time: { now: number }) {
    super()
  }

  override get<T>(key: string): Promise<T | undefined>
  override get<T>(keys: string[]): Promise<Map<string, T>>
  override async get<T>(key: string | string[]): Promise<T | undefined | Map<string, T>> {
    if (typeof key !== 'string') return await super.get<T>(key)
    const value = await super.get<T>(key)
    if (key === 'nextSequence' && this.advanceNextSequenceRead) {
      this.advanceNextSequenceRead = false
      this.time.now += 100
    }
    return value
  }

  override async list<T>(options?: { prefix?: string }): Promise<Map<string, T>> {
    const snapshot = await super.list<T>(options)
    if (!options?.prefix && this.advanceLateTransactionList) {
      this.advanceLateTransactionList = false
      this.time.now += 200
    }
    return snapshot
  }
}

async function prepareAllMembers(storage: MemoryStorage) {
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  await storage.put('metadata', {
    ...metadata,
    members: (metadata?.members as Array<Record<string, unknown>>).map(member => ({
      ...member,
      connected: true,
      ready: true,
    })),
  })
}

async function connectAllCompetitionMembers(storage: MemoryStorage) {
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  await storage.put('metadata', {
    ...metadata,
    members: (metadata?.members as Array<Record<string, unknown>>).map(member => ({
      ...member,
      connected: true,
    })),
  })
}

function gameRoom(
  storage: MemoryStorage,
  competitionRoom?: {
    idFromName(name: string): DurableObjectId
    get(id: DurableObjectId): {
      ingestInitialGameStarted?(request: CompetitionRoomInitialGameStartedRequest): Promise<unknown>
      executeMatch?(request: unknown): Promise<unknown>
      ingestGameTerminal(request: unknown): Promise<unknown>
    }
  },
) {
  const sockets: FakeSocket[] = []
  const pendingWaits: Promise<unknown>[] = []
  const state = {
    storage,
    waitUntil(value: Promise<unknown>) {
      pendingWaits.push(value.catch(() => undefined))
    },
    getWebSockets: () => sockets,
    acceptWebSocket(socket: FakeSocket) {
      sockets.push(socket)
    },
  }
  const env: Record<string, unknown> = {
    PLAYER_NOTIFICATIONS: {
      idFromName: () => 'global',
      get: () => ({ fetch: async () => new Response(null, { status: 204 }) }),
    },
  }
  if (competitionRoom) env.COMPETITION_ROOM = competitionRoom
  return {
    room: new GameRoom(state as unknown as DurableObjectState, env as never),
    sockets,
    pendingWaits,
  }
}

async function drainWaitUntil(pendingWaits: Promise<unknown>[]) {
  while (pendingWaits.length > 0) {
    const current = pendingWaits.splice(0)
    await Promise.all(current)
  }
}

class FakeSocket {
  readyState = 1
  attachment: unknown
  sent: string[] = []

  serializeAttachment(value: unknown) {
    this.attachment = value
  }

  deserializeAttachment() {
    return this.attachment
  }

  send(value: string) {
    this.sent.push(value)
  }

  close() {
    this.readyState = 3
  }
}

async function connectCompetitionMember(
  room: InstanceType<typeof GameRoom>,
  sockets: FakeSocket[],
  userId: string,
): Promise<FakeSocket> {
  const global = globalThis as Record<string, unknown>
  const originalPair = global.WebSocketPair
  global.WebSocketPair = class {
    client = new FakeSocket()
    server = new FakeSocket()
  }
  try {
    const response = await room.fetch(new Request('https://game-room.internal/socket', {
      headers: {
        Upgrade: 'websocket',
        'x-fewfc-user-id': userId,
        'x-fewfc-user-name': encodeURIComponent(userId),
      },
    }))
    expect(response.status).toBe(101)
  } finally {
    global.WebSocketPair = originalPair
  }
  return sockets.at(-1)!
}

function storedRoomEvents(storage: MemoryStorage): Array<Record<string, unknown>> {
  return [...storage.values.entries()]
    .filter(([key]) => key.startsWith('event:'))
    .map(([, value]) => value as Record<string, unknown>)
    .sort((left, right) => Number(left.sequence) - Number(right.sequence))
}

function binding(overrides: Partial<CompetitionGameBinding> = {}): CompetitionGameBinding {
  return {
    roundRef: {
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 1, roundGenerationId: 'generation-1',
    },
    roundStartMs: 1_700_000_000_000,
    competitionId: 'competition-1',
    matchId: 'match-1',
    gameNumber: 1,
    organizerUserId: 'organizer-1',
    expectedMatchRevision: 1,
    startKind: { type: 'ordinaryNewGame' },
    ruleVersion: '5.17',
    ruleset: 'fewfc-base',
    enabledRuleModules: ['personal-deck'],
    initialHp: [
      { team: 'team-a', hp: 20 },
      { team: 'team-b', hp: 20 },
    ],
    players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a' },
      { userId: 'user-b', player: 'player-b', team: 'team-b' },
    ],
    deckSnapshot: [
      { player: 'player-a', name: 'A locked deck', cards: ['a-1', 'a-2'] },
      { player: 'player-b', name: 'B locked deck', cards: ['b-1', 'b-2'] },
    ],
    firstPlayer: 'player-a',
    turnOrder: ['player-a', 'player-b'],
    deckSeed: 'competition-1:match-1:game-1',
    ...overrides,
  }
}

async function createReleasedCompetitionGame(room: InstanceType<typeof GameRoom>, request: CompetitionGameCreateRequest) {
  const ack = await room.createCompetitionGame(request)
  const bindingRef = await competitionGameBindingRefForRequest(request)
  expect(await room.releaseCompetitionGame({ bindingRef, commandId: 'formal-round-release' })).toBe(true)
  return ack
}

async function roundReleaseFixture(
  overrides: Partial<CompetitionGameBinding> = {},
  namespace?: Parameters<typeof gameRoom>[1],
  storage = new MemoryStorage(),
) {
  const created = gameRoom(storage, namespace)
  const fixed = binding({ roundStartMs: Date.now() + 600_000, ...overrides })
  const request = { gameId: 'round-release-game', binding: fixed }
  const ack = await created.room.createCompetitionGame(request)
  const bindingRef = await competitionGameBindingRefForRequest(request)
  const release = { bindingRef, commandId: 'round-release-decision' }
  return { ...created, storage, request, ack, release }
}

async function roundReleaseReady(fixture: Awaited<ReturnType<typeof roundReleaseFixture>>) {
  for (const player of fixture.request.binding.players) {
    await connectCompetitionMember(fixture.room, fixture.sockets, player.userId)
    expect(await fixture.room.setCompetitionReady({ actorUserId: player.userId, ready: true }))
      .toEqual({ ok: true, ready: true })
    const metadata = await fixture.storage.get<GameRoomMetadata>('metadata')
    if (!metadata) throw new Error('missing public-ready metadata')
    // Waiting 的缺欄不可在 ready writer 變成 own undefined，否則下一位 connect 會被 raw guard 拒絕。
    expect(Object.hasOwn(metadata, 'gameInstanceId')).toBe(false)
    expect(metadata.members.find(member => member.userId === player.userId))
      .toMatchObject({ ready: true, connected: true })
  }
  await drainWaitUntil(fixture.pendingWaits)
}

function roundReleaseNoStart(storage: MemoryStorage) {
  expect(storage.values.has('gameRecord')).toBe(false)
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toEqual([])
  expect(storage.values.get('competitionDeliveryQueue')).toMatchObject({ entries: [], revisionCursor: 1 })
}

test('round-release child exact3 ACK only proves immutable unreleased Waiting, not an Active retry', async () => {
  const fixture = await roundReleaseFixture()
  const { room, request, storage, ack, release } = fixture
  expect(ack).toEqual({ gameId: request.gameId, bindingDigest: release.bindingRef.bindingDigest, status: 'Waiting' })
  expect(Reflect.ownKeys(ack)).toEqual(['gameId', 'bindingDigest', 'status'])
  const before = structuredClone(storage.values)
  const writes = storage.writeCount
  expect(await room.createCompetitionGame(request)).toEqual(ack)
  expect(storage.values).toEqual(before)
  expect(storage.writeCount).toBe(writes)
  expect(storage.values.get('competitionGameAdmission')).toMatchObject({ status: 'unreleased' })
  roundReleaseNoStart(storage)
  expect(await room.releaseCompetitionGame(release)).toBe(true)
  await expect(room.createCompetitionGame(request)).rejects.toThrow('competition game binding does not match')
})

test('round-release child unreleased connected ready stays Waiting after T across direct start, alarm and restart', async () => {
  const fixture = await roundReleaseFixture()
  await roundReleaseReady(fixture)
  const realNow = Date.now
  try {
    Date.now = () => fixture.request.binding.roundStartMs + 1
    const engine = rulesEngineBoundaryCalls
    await expect(fixture.room.startCompetitionGame(fixture.request)).rejects.toThrow('all fixed competition members must be connected and ready')
    await gameRoom(fixture.storage).room.alarm()
    roundReleaseNoStart(fixture.storage)
    expect(rulesEngineBoundaryCalls).toBe(engine)
    expect(fixture.storage.alarm).toBeNull()
  } finally { Date.now = realNow }
})

test('round-release child release immediately starts a room that was already fully ready', async () => {
  let initialDispatches = 0
  const namespace = {
    idFromName: (name: string): DurableObjectId => ({ name, toString: () => name, equals: () => false }),
    get: () => ({
      ingestInitialGameStarted: async (request: CompetitionRoomInitialGameStartedRequest) => {
        initialDispatches += 1
        return initialFifoAck(request)
      },
      executeMatch: async () => { throw new Error('first start must not use generic clock RPC') },
      ingestGameTerminal: async () => { throw new Error('unexpected terminal RPC') },
    }),
  }
  const fixture = await roundReleaseFixture({}, namespace)
  await roundReleaseReady(fixture)
  const realNow = Date.now
  const startedAtMs = fixture.request.binding.roundStartMs - 60_000
  try {
    Date.now = () => startedAtMs
    expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
    await drainWaitUntil(fixture.pendingWaits)
    await fixture.room.alarm()
    await drainWaitUntil(fixture.pendingWaits)

    const startedEvents = storedRoomEvents(fixture.storage)
      .filter(event => event.type === 'GameStarted')
    const record = await fixture.storage.get<GameRecord>('gameRecord')
    const evidence = await fixture.storage.get<unknown>('competitionInitialStartEvidence')
    expect(startedEvents).toHaveLength(1)
    expect(startedEvents[0]?.createdAt).toBe(new Date(startedAtMs).toISOString())
    expect(record).toBeDefined()
    expect(record?.gameInstanceId).toEqual(expect.any(String))
    expect(evidence).toMatchObject({
      releaseDecisionId: fixture.release.commandId,
      startSequence: startedEvents[0]?.sequence,
      startedAtMs,
    })
    expect(initialDispatches).toBe(1)
  } finally { Date.now = realNow }
})

test('round-release child released early ready starts before T with the actual trusted sample and canonical sequence', async () => {
  let initialDispatches = 0
  const namespace = {
    idFromName: (name: string): DurableObjectId => ({ name, toString: () => name, equals: () => false }),
    get: () => ({
      ingestInitialGameStarted: async (request: CompetitionRoomInitialGameStartedRequest) => {
        initialDispatches += 1
        return initialFifoAck(request)
      },
      executeMatch: async () => { throw new Error('first start must not use generic clock RPC') },
      ingestGameTerminal: async () => { throw new Error('unexpected terminal RPC') },
    }),
  }
  const fixture = await roundReleaseFixture({}, namespace)
  const realNow = Date.now
  try {
    const startedAtMs = fixture.request.binding.roundStartMs - 1
    Date.now = () => startedAtMs
    expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
    await roundReleaseReady(fixture)
    expect((await fixture.room.startCompetitionGame(fixture.request)).idempotent).toBe(true)
    const restarted = gameRoom(fixture.storage, namespace)
    await restarted.room.alarm()
    await drainWaitUntil(restarted.pendingWaits)
    const event = storedRoomEvents(fixture.storage).find(value => value.type === 'GameStarted')
    expect(event).toBeDefined()
    const record = await fixture.storage.get<GameRecord>('gameRecord')
    if (!record) throw new Error('missing actual started record')
    expect(event?.payload).toEqual({ gameInstanceId: record.gameInstanceId, firstPlayer: fixture.request.binding.firstPlayer })
    expect(fixture.storage.values.get('competitionGameAdmission')).toMatchObject({
      status: 'started', releaseCommandId: fixture.release.commandId,
      start: { startSequence: event?.sequence, startedAtMs },
    })
    expect(fixture.storage.values.get('competitionInitialStartEvidence')).toEqual({
      bindingRef: fixture.release.bindingRef,
      releaseDecisionId: fixture.release.commandId,
      gameInstanceId: record.gameInstanceId,
      startSequence: event?.sequence,
      startedAtMs,
      initialWindow: { type: 'action', team: 'team-a', turn: 1 },
    })
    expect(fixture.storage.values.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 2, entries: [] })
    expect(fixture.storage.values.get('competitionInitialStartEvidence')).toMatchObject({
      startedAtMs, initialWindow: { type: 'action', team: 'team-a', turn: 1 },
    })
    expect((await restarted.room.startCompetitionGame(fixture.request)).idempotent).toBe(true)
    expect(storedRoomEvents(fixture.storage).filter(value => value.type === 'GameStarted')).toHaveLength(1)
    await expect(restarted.room.createCompetitionGame(fixture.request)).rejects.toThrow()
    await restarted.room.alarm()
    expect(initialDispatches).toBe(1)
    expect(fixture.storage.values.has('competitionInitialStartEvidence')).toBe(true)
  } finally { Date.now = realNow }
})

test('round-release child a released unready T alarm does not busyloop or claim a forfeit', async () => {
  const fixture = await roundReleaseFixture()
  expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
  expect(fixture.storage.alarm).toBe(fixture.request.binding.roundStartMs)
  const realNow = Date.now
  try {
    Date.now = () => fixture.request.binding.roundStartMs
    await fixture.room.alarm()
    expect(fixture.storage.alarm).toBeNull()
    roundReleaseNoStart(fixture.storage)
    expect(fixture.storage.values.get('metadata')).toMatchObject({ status: 'Waiting' })
    await fixture.room.alarm()
    expect(fixture.storage.alarm).toBeNull()
  } finally { Date.now = realNow }
})

test('round-release child revoke is terminal and immutable retries have no repeated writes, events or alarm', async () => {
  const fixture = await roundReleaseFixture()
  expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
  const revoke = { ...fixture.release, commandId: 'revoke-generation-1' }
  expect(await fixture.room.revokeCompetitionGame(revoke)).toBe('revoked')
  const before = structuredClone(fixture.storage.values)
  const writes = fixture.storage.writeCount
  const restarted = gameRoom(fixture.storage).room
  expect(await restarted.revokeCompetitionGame(revoke)).toBe('revoked')
  // true 只重播歷史 release receipt，不能恢復 revoked。
  expect(await restarted.releaseCompetitionGame(fixture.release)).toBe(true)
  expect(await restarted.releaseCompetitionGame({ ...fixture.release, commandId: 'another-release' })).toBe(false)
  expect(await restarted.setCompetitionReady({ actorUserId: 'user-a', ready: false }))
    .toEqual({ ok: false, code: 'invalidCompetitionRoom' })
  await expect(restarted.createCompetitionGame(fixture.request)).rejects.toThrow()
  await expect(restarted.startCompetitionGame(fixture.request)).rejects.toThrow()
  await restarted.alarm()
  roundReleaseNoStart(fixture.storage)
  expect(fixture.storage.values).toEqual(before)
  expect(fixture.storage.writeCount).toBe(writes + 1)
  expect(fixture.storage.alarm).toBeNull()
})

test('round-release child pristine tombstone blocks late create, ordinary create and purge resurrection', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'round-release-pristine', binding: binding() }
  const bindingRef = await competitionGameBindingRefForRequest(request)
  const revoke = { bindingRef, commandId: 'pristine-revoke' }
  expect(await room.revokeCompetitionGame(revoke)).toBe('revoked')
  expect([...storage.values.keys()]).toEqual(['competitionGameAdmission'])
  const before = structuredClone(storage.values)
  await expect(gameRoom(storage).room.createCompetitionGame(request)).rejects.toThrow()
  expect((await post(room, {
    type: 'createGame', gameId: request.gameId, actorUserId: 'owner', actorName: '主辦', capacity: 2,
    invitation: { roomCode: 'ABCD234', inviteToken: 'secret' },
  })).status).toBe(403)
  const purge = await room.fetch(new Request('https://game-room.internal/manage/purge-legacy', {
    method: 'POST', body: JSON.stringify({ epoch: 'do-not-resurrect' }),
  }))
  expect(purge.status).toBe(200)
  expect(await purge.json()).toMatchObject({ status: 'preserved' })
  expect(storage.values).toEqual(before)
})

test('round-release child revoke wins while create is awaiting engine validation, with no late metadata', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'round-release-inflight-create', binding: binding() }
  const bindingRef = await competitionGameBindingRefForRequest(request)
  let unblock = () => {}
  let entered = () => {}
  rulesModuleGate = new Promise<void>(resolve => { unblock = resolve })
  const reached = new Promise<void>(resolve => { entered = resolve })
  rulesModuleGateEntered = entered
  try {
    const creating = room.createCompetitionGame(request)
    // Bun pending matcher 會等待 settlement；先捕捉兩分支，讓同一測試能執行真正的 revoke／unblock。
    const captured = creating.then(
      (): { status: 'fulfilled' } => ({ status: 'fulfilled' }),
      (reason: unknown): { status: 'rejected'; reason: unknown } => ({ status: 'rejected', reason }),
    )
    await reached
    expect(await room.revokeCompetitionGame({ bindingRef, commandId: 'revoke-before-create' })).toBe('revoked')
    unblock()
    const settled = await captured
    if (settled.status !== 'rejected') throw new Error('late create unexpectedly fulfilled')
    expect(() => { throw settled.reason }).toThrow('competition game binding does not match')
    expect([...storage.values.keys()]).toEqual(['competitionGameAdmission'])
  } finally { unblock(); rulesModuleGate = null; rulesModuleGateEntered = null }
})

test('round-release child create snapshots normal mutable input before digest await', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'round-release-snapshot', binding: binding() }
  const expected = await competitionGameBindingRefForRequest(request)
  const creating = room.createCompetitionGame(request)
  request.binding.deckSeed = 'mutated-after-promise-return'
  request.binding.roundStartMs += 1
  request.binding.roundRef.roundGenerationId = 'mutated-generation'
  expect(await creating).toEqual({ gameId: request.gameId, bindingDigest: expected.bindingDigest, status: 'Waiting' })
  expect(storage.values.get('competitionBinding')).toMatchObject({
    deckSeed: 'competition-1:match-1:game-1', roundStartMs: expected.roundStartMs,
    roundRef: expected.roundRef,
  })
})

test('round-release child revoke linearizes before a prepared engine result and zero GameStarted is committed', async () => {
  const fixture = await roundReleaseFixture()
  await roundReleaseReady(fixture)
  expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
  const realNow = Date.now
  let unblock = () => {}
  let entered = () => {}
  const reached = new Promise<void>(resolve => { entered = resolve })
  rulesEngineStartGate = new Promise<void>(resolve => { unblock = resolve })
  rulesEngineStartGateEntered = entered
  try {
    Date.now = () => fixture.request.binding.roundStartMs
    const starting = fixture.room.startCompetitionGame(fixture.request)
    const captured = starting.then(
      (): { status: 'fulfilled' } => ({ status: 'fulfilled' }),
      (reason: unknown): { status: 'rejected'; reason: unknown } => ({ status: 'rejected', reason }),
    )
    await reached
    expect(await fixture.room.revokeCompetitionGame({ ...fixture.release, commandId: 'revoke-during-prepare' })).toBe('revoked')
    unblock()
    const settled = await captured
    if (settled.status !== 'rejected') throw new Error('revoked preparation unexpectedly fulfilled')
    expect(() => { throw settled.reason }).toThrow('all fixed competition members must be connected and ready')
    roundReleaseNoStart(fixture.storage)
    expect(fixture.storage.alarm).toBeNull()
  } finally { unblock(); rulesEngineStartGate = null; rulesEngineStartGateEntered = null; Date.now = realNow }
})

test('round-release child unready invalidates in-flight preparation without clock begin', async () => {
  const fixture = await roundReleaseFixture()
  await roundReleaseReady(fixture)
  expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
  const realNow = Date.now
  let unblock = () => {}
  let entered = () => {}
  const reached = new Promise<void>(resolve => { entered = resolve })
  rulesEngineStartGate = new Promise<void>(resolve => { unblock = resolve })
  rulesEngineStartGateEntered = entered
  try {
    Date.now = () => fixture.request.binding.roundStartMs
    const starting = fixture.room.startCompetitionGame(fixture.request)
    const captured = starting.then(
      (): { status: 'fulfilled' } => ({ status: 'fulfilled' }),
      (reason: unknown): { status: 'rejected'; reason: unknown } => ({ status: 'rejected', reason }),
    )
    await reached
    expect(await fixture.room.setCompetitionReady({ actorUserId: 'user-b', ready: false }))
      .toEqual({ ok: true, ready: false })
    unblock()
    const settled = await captured
    if (settled.status !== 'rejected') throw new Error('unready preparation unexpectedly fulfilled')
    expect(() => { throw settled.reason }).toThrow('all fixed competition members must be connected and ready')
    await drainWaitUntil(fixture.pendingWaits)
    roundReleaseNoStart(fixture.storage)
    expect(fixture.storage.alarm).toBeNull()
  } finally { unblock(); rulesEngineStartGate = null; rulesEngineStartGateEntered = null; Date.now = realNow }
})

test('round-release child true durable GameStarted wins revoke, unlike a metadata Active label', async () => {
  const fixture = await roundReleaseFixture()
  await roundReleaseReady(fixture)
  expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
  const realNow = Date.now
  try {
    Date.now = () => fixture.request.binding.roundStartMs
    await fixture.room.startCompetitionGame(fixture.request)
    const before = structuredClone(fixture.storage.values)
    const alarm = fixture.storage.alarm
    expect(await fixture.room.revokeCompetitionGame({ ...fixture.release, commandId: 'start-won' })).toBe('started')
    expect(fixture.storage.values.get('gameRecord')).toEqual(before.get('gameRecord'))
    expect(fixture.storage.alarm).toBe(alarm)
    const current = await fixture.storage.get<GameRoomMetadata>('metadata')
    if (!current) throw new Error('missing started metadata')
    await fixture.storage.put('metadata', { ...current, status: 'Finished' })
    expect(await fixture.room.revokeCompetitionGame({ ...fixture.release, commandId: 'finished-start' })).toBe('started')
  } finally { Date.now = realNow }
  const fake = await roundReleaseFixture()
  const waiting = await fake.storage.get<GameRoomMetadata>('metadata')
  if (!waiting) throw new Error('missing Waiting metadata')
  await fake.storage.put('metadata', { ...waiting, status: 'Active', gameInstanceId: 'invented-instance' })
  const before = structuredClone(fake.storage.values)
  expect(await fake.room.revokeCompetitionGame({ ...fake.release, commandId: 'fake-active' })).toBe('invalid')
  expect(fake.storage.values).toEqual(before)
})

test('round-release child exact request guards reject symbols, hidden fields, prototypes and accessors without reads or writes', async () => {
  const fixture = await roundReleaseFixture()
  let getters = 0
  const inputs: unknown[] = [
    null, [], { ...fixture.release, private: 'secret' },
    { ...fixture.release, bindingRef: { ...fixture.release.bindingRef, private: 'secret' } },
    Object.assign({ ...fixture.release }, { [Symbol.dispose]: () => {} }),
    Object.defineProperty({ ...fixture.release }, 'hidden', { value: 'secret' }),
    Object.assign(Object.create({}), fixture.release),
    Object.defineProperty({ ...fixture.release }, 'bindingRef', {
      enumerable: true, get() { getters += 1; return fixture.release.bindingRef },
    }),
  ]
  const before = structuredClone(fixture.storage.values)
  const reads = fixture.storage.readRequests.length
  const writes = fixture.storage.writeCount
  for (const input of inputs) {
    expect(await Reflect.apply(fixture.room.releaseCompetitionGame, fixture.room, [input])).toBe(false)
    expect(await Reflect.apply(fixture.room.revokeCompetitionGame, fixture.room, [input])).toBe('invalid')
  }
  expect(getters).toBe(0)
  expect(fixture.storage.values).toEqual(before)
  expect(fixture.storage.readRequests.length).toBe(reads)
  expect(fixture.storage.writeCount).toBe(writes)
})

test('round-release child exact identity and corrupt stored admission fail closed without repair', async () => {
  const fixture = await roundReleaseFixture()
  for (const bindingRef of [
    { ...fixture.release.bindingRef, gameId: 'different-game' },
    { ...fixture.release.bindingRef, roundStartMs: fixture.release.bindingRef.roundStartMs + 1 },
    { ...fixture.release.bindingRef, bindingDigest: '0'.repeat(64) },
    { ...fixture.release.bindingRef, roundRef: { ...fixture.release.bindingRef.roundRef, roundGenerationId: 'stale-generation' } },
  ]) {
    const before = structuredClone(fixture.storage.values)
    expect(await fixture.room.releaseCompetitionGame({ ...fixture.release, bindingRef })).toBe(false)
    expect(await fixture.room.revokeCompetitionGame({ ...fixture.release, bindingRef })).toBe('invalid')
    expect(fixture.storage.values).toEqual(before)
  }
  const waiting = await fixture.storage.get<GameRoomMetadata>('metadata')
  if (!waiting) throw new Error('missing valid Waiting metadata')
  expect(Object.hasOwn(waiting, 'gameInstanceId')).toBe(false)
  expect(await fixture.room.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: false, code: 'playerNotConnected' })
  // 單欄 corruption control：保留 raw guard 對 own undefined 的拒絕，不把 writer 修正變成 allowundefined。
  await fixture.storage.put('metadata', { ...waiting, gameInstanceId: undefined })
  const malformedBefore = structuredClone(fixture.storage.values)
  const malformedWrites = fixture.storage.writeCount
  expect(await fixture.room.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: false, code: 'invalidCompetitionRoom' })
  expect(fixture.storage.values).toEqual(malformedBefore)
  expect(fixture.storage.writeCount).toBe(malformedWrites)
  await fixture.storage.put('metadata', waiting)
  const original = await fixture.storage.get<Record<string, unknown>>('competitionGameAdmission')
  if (!original) throw new Error('missing admission')
  for (const corrupt of [
    { ...original, status: 'unknown' }, { ...original, bindingFingerprint: 'partial-fingerprint' },
    { ...original, version: Number.MAX_SAFE_INTEGER }, Object.assign({ ...original }, { [Symbol('private')]: true }),
  ]) {
    await fixture.storage.put('competitionGameAdmission', corrupt)
    const before = new Map(fixture.storage.values)
    const writes = fixture.storage.writeCount
    expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(false)
    expect(await fixture.room.revokeCompetitionGame(fixture.release)).toBe('invalid')
    expect(fixture.storage.values).toEqual(before)
    expect(fixture.storage.writeCount).toBe(writes)
  }
})

test('round-release child release and revoke receipts roll back together with failed batch or alarm', async () => {
  for (const kind of ['release', 'revoke'] as const) {
    for (const fault of ['failNextBatch', 'failNextAlarm'] as const) {
      const fixture = await roundReleaseFixture()
      const before = structuredClone(fixture.storage.values)
      fixture.storage[fault] = true
      const operation = kind === 'release' ? fixture.room.releaseCompetitionGame(fixture.release)
        : fixture.room.revokeCompetitionGame(fixture.release)
      await expect(operation).rejects.toThrow()
      expect(fixture.storage.values).toEqual(before)
      expect(fixture.storage.alarm).toBeNull()
      expect(await (kind === 'release' ? fixture.room.releaseCompetitionGame(fixture.release)
        : fixture.room.revokeCompetitionGame(fixture.release))).toBe(kind === 'release' ? true : 'revoked')
    }
  }
})

test('round-release child later games keep RoundRef T and decision but never reuse the previous BindingRef', async () => {
  const first = await roundReleaseFixture()
  const nextRequest = {
    gameId: 'round-release-game-2',
    binding: { ...first.request.binding, gameNumber: 2, expectedMatchRevision: 4 },
  }
  const nextStorage = new MemoryStorage()
  const { room } = gameRoom(nextStorage)
  const ack = await room.createCompetitionGame(nextRequest)
  const bindingRef = await competitionGameBindingRefForRequest(nextRequest)
  expect(bindingRef.roundRef).toEqual(first.release.bindingRef.roundRef)
  expect(bindingRef.roundStartMs).toBe(first.release.bindingRef.roundStartMs)
  expect(bindingRef.bindingDigest).not.toBe(first.release.bindingRef.bindingDigest)
  expect(ack.bindingDigest).toBe(bindingRef.bindingDigest)
  expect(await room.releaseCompetitionGame(first.release)).toBe(false)
  expect(await room.releaseCompetitionGame({ bindingRef, commandId: first.release.commandId })).toBe(true)
})

test('round-release child release and revoke ACKs do not wait for a pending parent FIFO response', async () => {
  let unblock = () => {}
  let entered = () => {}
  const pending = new Promise<void>(resolve => { unblock = resolve })
  const reached = new Promise<void>(resolve => { entered = resolve })
  const namespace = {
    idFromName: (name: string): DurableObjectId => ({ name, toString: () => name, equals: () => false }),
    get: () => ({
      executeMatch: async (request: unknown) => {
        if (typeof request !== 'object' || request === null || Array.isArray(request)) throw new Error('invalid fixture request')
        entered()
        await pending
        return clockSuccess(Object.fromEntries(Object.entries(request)))
      },
      ingestGameTerminal: async () => { throw new Error('unexpected terminal') },
    }),
  }
  // 保留後續局原 FIFO 邊界；首局另有 typed initial pending-remote 正式案例。
  const fixture = await roundReleaseFixture({ gameNumber: 2 }, namespace)
  expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
  await roundReleaseReady(fixture)
  const realNow = Date.now
  let watchdog: ReturnType<typeof setTimeout> | undefined
  try {
    Date.now = () => fixture.request.binding.roundStartMs
    await fixture.room.startCompetitionGame(fixture.request)
    const delivery = fixture.room.alarm()
    await reached
    expect(await Promise.race([
      Promise.all([
        fixture.room.releaseCompetitionGame(fixture.release),
        fixture.room.revokeCompetitionGame({ ...fixture.release, commandId: 'pending-delivery-revoke' }),
      ]),
      new Promise<never>((_, reject) => { watchdog = setTimeout(() => reject(new Error('admission waited for parent RPC')), 250) }),
    ])).toEqual([true, 'started'])
    unblock()
    await delivery
    expect(fixture.storage.values.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 3, entries: [] })
  } finally { unblock(); if (watchdog) clearTimeout(watchdog); Date.now = realNow }
})

function contextBinding(capacity: 2 | 4, personal: boolean): CompetitionGameBinding {
  const players = [
    { userId: 'user-a', player: 'player-a', team: 'team-a' },
    { userId: 'user-b', player: 'player-b', team: 'team-b' },
    { userId: 'user-c', player: 'player-c', team: 'team-a' },
    { userId: 'user-d', player: 'player-d', team: 'team-b' },
  ].slice(0, capacity)
  return binding({
    players,
    enabledRuleModules: personal ? ['personal-deck'] : [],
    deckSnapshot: personal ? players.map(player => ({
      player: player.player, name: `鎖定牌組:${player.player}`, cards: [`鎖定牌:${player.player}`],
    })) : [],
    turnOrder: players.map(player => player.player),
  })
}

test('E4d3c-r3 clock-control eligibility rejects a provisioned Waiting room without writes', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const fixed = binding()
  const created = await createReleasedCompetitionGame(room, { gameId: 'eligibility-game', binding: fixed })
  expect(created.status).toBe('Waiting')
  const before = structuredClone(storage.values)
  const alarmBefore = storage.alarm
  const writesBefore = storage.writeCount
  expect(await room.getCompetitionClockControlEligibility({
    competitionId: fixed.competitionId, matchId: fixed.matchId,
    gameId: 'eligibility-game', gameNumber: fixed.gameNumber,
  })).toBe(false)
  expect(storage.values).toStrictEqual(before)
  expect(storage.alarm).toBe(alarmBefore)
  expect(storage.writeCount).toBe(writesBefore)
})

test('E4d3c-r3 clock-control eligibility accepts a normally started Active instance as a readonly boolean', async () => {
  rulesEngineMode = 'normal'
  const storage = new MemoryStorage()
  const { room, pendingWaits } = gameRoom(storage)
  const fixed = binding()
  const creation = { gameId: 'eligibility-game', binding: fixed }
  await createReleasedCompetitionGame(room, creation)
  await prepareAllMembers(storage)
  expect((await room.startCompetitionGame(creation)).status).toBe('Active')
  await drainWaitUntil(pendingWaits)
  const before = structuredClone(storage.values)
  const alarmBefore = storage.alarm
  const writesBefore = storage.writeCount
  expect(await room.getCompetitionClockControlEligibility({
    competitionId: fixed.competitionId, matchId: fixed.matchId,
    gameId: creation.gameId, gameNumber: fixed.gameNumber,
  })).toBe(true)
  expect(storage.values).toStrictEqual(before)
  expect(storage.alarm).toBe(alarmBefore)
  expect(storage.writeCount).toBe(writesBefore)
})

test('E4d3c-r3 clock-control eligibility fails closed on corrupt raw snapshots without invoking accessors', async () => {
  let getters = 0
  const faults: Array<[string, (value: unknown) => unknown]> = [
    ['competitionBinding', value => Object.assign({}, value, { [Symbol('unknown')]: 'secret' })],
    ['competitionBinding', value => Object.defineProperty(Object.assign({}, value), 'hidden', { value: 'secret' })],
    ['competitionBinding', value => Object.defineProperty(Object.assign({}, value), 'players', {
      enumerable: true, get() { getters += 1; return [] },
    })],
    ['competitionBinding', value => Object.assign({}, value, { players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a', private: 'secret' },
      { userId: 'user-b', player: 'player-b', team: 'team-b' },
    ] })],
    ['metadata', value => Object.assign({}, value, { [Symbol.dispose]: () => {} })],
    ['metadata', value => Object.assign({}, value, { capacity: 4 })],
    ['metadata', value => Object.assign({}, value, { gameInstanceId: 'wrong-instance' })],
    ['gameRecord', value => Object.assign({}, value, { private: 'secret' })],
    ['gameRecord', value => Object.assign({}, value, { gameInstanceId: 'wrong-instance' })],
    ['gameRecord', value => Object.assign({}, value, { sequence: 0 })],
    ['gameRecord', value => Object.assign({}, value, { rulesRecord: [{}] })],
    ['gameRecord', value => Object.assign({}, value, { rulesRecord: [{ source: 'Automatic', events: [{
      GameEnded: { conclusion: { outcome: { type: 'winner', team: 'team-a' },
        causes: [{ type: 'teamHpDepleted', teams: ['team-b'] }] } },
    }] }] })],
    ['gameRecord', value => Object.assign({}, value, { finishedAt: '2026-10-01T00:00:00.000Z' })],
    ['gameRecord', value => Object.assign({}, value, { rulesRecord: Object.assign([], { [Symbol('nested')]: 'secret' }) })],
    ['competitionDeliveryQueue', value => Object.assign({}, value, { [Symbol('unknown')]: 'secret' })],
    ['competitionDeliveryQueue', value => Object.assign({}, value, { revisionCursor: 0 })],
  ]
  for (const [key, corrupt] of faults) {
    rulesEngineMode = 'normal'
    const storage = new MemoryStorage()
    const { room, pendingWaits } = gameRoom(storage)
    const creation = { gameId: 'eligibility-game', binding: binding() }
    await createReleasedCompetitionGame(room, creation)
    await prepareAllMembers(storage)
    await room.startCompetitionGame(creation)
    await drainWaitUntil(pendingWaits)
    const request = { competitionId: 'competition-1', matchId: 'match-1', gameId: creation.gameId, gameNumber: 1 }
    expect(await room.getCompetitionClockControlEligibility(request)).toBe(true)
    await storage.put(key, corrupt(storage.values.get(key)))
    const before = new Map(storage.values)
    const writesBefore = storage.writeCount
    const alarmBefore = storage.alarm
    expect(await room.getCompetitionClockControlEligibility(request)).toBe(false)
    expect(getters).toBe(0)
    expect(storage.values.size).toBe(before.size)
    for (const [storedKey, value] of before) expect(Object.is(storage.values.get(storedKey), value)).toBe(true)
    expect(storage.writeCount).toBe(writesBefore)
    expect(storage.alarm).toBe(alarmBefore)
  }
})

test('E4d3c-r3 eligibility rejects malformed requests, missing state and mismatched identities without repair', async () => {
  rulesEngineMode = 'normal'
  const storage = new MemoryStorage()
  const { room, pendingWaits } = gameRoom(storage)
  const creation = { gameId: 'eligibility-game', binding: binding() }
  await createReleasedCompetitionGame(room, creation)
  await prepareAllMembers(storage)
  await room.startCompetitionGame(creation)
  await drainWaitUntil(pendingWaits)
  const request = { competitionId: 'competition-1', matchId: 'match-1', gameId: creation.gameId, gameNumber: 1 }
  const baseline = structuredClone(storage.values)
  const malformed: unknown[] = [null, {}, [], { ...request, private: 'secret' },
    Object.assign({ ...request }, { [Symbol.dispose]: () => {} }),
    { ...request, gameId: { value: creation.gameId } }]
  let getters = 0
  malformed.push(Object.defineProperty({ ...request }, 'gameId', {
    enumerable: true, get() { getters += 1; return creation.gameId },
  }))
  const readsBeforeInvalid = storage.readRequests.length
  for (const input of malformed) {
    // 模擬 RPC 的 unknown 入參，而非透過 TS cast 假裝 DTO 已安全。
    expect(await Reflect.apply(room.getCompetitionClockControlEligibility, room, [input])).toBe(false)
  }
  expect(getters).toBe(0)
  expect(storage.readRequests.length).toBe(readsBeforeInvalid)
  for (const mismatched of [
    { ...request, competitionId: 'other' }, { ...request, matchId: 'other' },
    { ...request, gameId: 'other' }, { ...request, gameNumber: 2 },
  ]) expect(await room.getCompetitionClockControlEligibility(mismatched)).toBe(false)
  expect(storage.values).toStrictEqual(baseline)
  for (const key of ['metadata', 'competitionBinding', 'gameRecord', 'competitionDeliveryQueue']) {
    const original = storage.values.get(key)
    await storage.delete(key)
    const before = structuredClone(storage.values)
    const writesBefore = storage.writeCount
    const alarmBefore = storage.alarm
    expect(await room.getCompetitionClockControlEligibility(request)).toBe(false)
    expect(storage.values).toStrictEqual(before)
    expect(storage.writeCount).toBe(writesBefore)
    expect(storage.alarm).toBe(alarmBefore)
    await storage.put(key, original)
  }
  for (const status of ['Waiting', 'Finished', 'Dissolved']) {
    const metadata = await storage.get<GameRoomMetadata>('metadata')
    await storage.put('metadata', { ...metadata, status })
    const before = structuredClone(storage.values)
    const writesBefore = storage.writeCount
    const alarmBefore = storage.alarm
    expect(await room.getCompetitionClockControlEligibility(request)).toBe(false)
    expect(storage.values).toStrictEqual(before)
    expect(storage.writeCount).toBe(writesBefore)
    expect(storage.alarm).toBe(alarmBefore)
  }
  const ordinary = new MemoryStorage()
  // 舊普通房故意缺 normalization 欄位；probe 不得補成新 schema 或建立 binding。
  await ordinary.put('metadata', { schemaVersion: 4, gameId: creation.gameId, status: 'Waiting' })
  const beforeOrdinary = structuredClone(ordinary.values)
  const ordinaryWrites = ordinary.writeCount
  expect(await gameRoom(ordinary).room.getCompetitionClockControlEligibility(request)).toBe(false)
  expect(ordinary.values).toStrictEqual(beforeOrdinary)
  expect(ordinary.writeCount).toBe(ordinaryWrites)
  expect(await gameRoom(new MemoryStorage()).room.getCompetitionClockControlEligibility(request)).toBe(false)
  const readFailure = new MemoryStorage()
  readFailure.failNextGet = true
  expect(await gameRoom(readFailure).room.getCompetitionClockControlEligibility(request)).toBe(false)
  expect(readFailure.writeCount).toBe(0)
})

test('E4d3c-r3 Active preparation, action, idle and paused instances remain eligible across restart', async () => {
  for (const capacity of [2, 4] as const) {
    for (const preparing of [false, true]) {
      rulesEngineMode = preparing ? 'clockTransition' : 'normal'
      const storage = new MemoryStorage()
      const namespace = {
        idFromName: (name: string) => name,
        get: () => ({
          executeMatch: async (request: Record<string, unknown>) => terminalSuccess(request),
          ingestGameTerminal: async () => { throw new Error('eligibility must not submit terminal data') },
        }),
      }
      const { room, pendingWaits } = gameRoom(storage, withInitialStartReceiver(namespace))
      const fixed = contextBinding(capacity, false)
      if (preparing) fixed.enabledRuleModules = ['pouch']
      const creation = { gameId: 'eligibility-game', binding: fixed }
      await createReleasedCompetitionGame(room, creation)
      await prepareAllMembers(storage)
      expect((await room.startCompetitionGame(creation)).status).toBe('Active')
      await drainWaitUntil(pendingWaits)
      const request = { competitionId: fixed.competitionId, matchId: fixed.matchId,
        gameId: creation.gameId, gameNumber: fixed.gameNumber }
      const check = async () => {
        const before = structuredClone(storage.values)
        const writesBefore = storage.writeCount
        const alarmBefore = storage.alarm
        const readCount = storage.readRequests.length
        expect(await room.getCompetitionClockControlEligibility(request)).toBe(true)
        expect(storage.readRequests.slice(readCount)).toStrictEqual([
          ['metadata', 'competitionBinding', 'gameRecord', 'competitionDeliveryQueue', 'competitionGameAdmission'],
        ])
        expect(storage.values).toStrictEqual(before)
        expect(storage.writeCount).toBe(writesBefore)
        expect(storage.alarm).toBe(alarmBefore)
      }
      const initial = await storage.get<{ entries: InitialFifoEntry[] }>('competitionDeliveryQueue')
      expect(initial?.entries[0]?.evidence.initialWindow.type).toBe(preparing ? 'preparation' : 'action')
      await check()
      await room.alarm()
      const queue = await storage.get<{ revisionCursor: number }>('competitionDeliveryQueue')
      if (!queue) throw new Error('start delivery queue is missing')
      const pause = clockControlRequest(fixed, { gameId: creation.gameId, matchRevision: queue.revisionCursor + 1 })
      await room.synchronizeCompetitionClock(pause)
      expect(await storage.get('competitionDeliveryQueue')).toMatchObject({ refereePaused: true })
      await check()
      const beforeRestart = structuredClone(storage.values)
      expect(await gameRoom(storage, namespace).room.getCompetitionClockControlEligibility(request)).toBe(true)
      expect(storage.values).toStrictEqual(beforeRestart)
      await room.synchronizeCompetitionClock({ ...pause, controlId: 'resume-control', sourceCommandId: 'resume-command',
        operation: 'clockResume', matchRevision: pause.matchRevision + 1 })
      expect(await storage.get('competitionDeliveryQueue')).toMatchObject({ refereePaused: false })
      await room.enqueueCompetitionClockMutation({ competitionId: fixed.competitionId, matchId: fixed.matchId,
        callerUserId: fixed.organizerUserId, commandId: 'end-window', trustedTimeMs: Date.now(),
        operation: { type: 'clockEndWindow', matchId: fixed.matchId } })
      await drainWaitUntil(pendingWaits)
      await room.alarm()
      expect(await storage.get('competitionDeliveryQueue')).toMatchObject({ entries: [], nextClockDeadlineMs: null, refereePaused: false })
      await check()
    }
  }
})

test('E4d3c-r3 snapshot resolves while serialized delivery is waiting for CompetitionRoom', async () => {
  rulesEngineMode = 'normal'
  const storage = new MemoryStorage()
  let release = () => {}
  let entered = () => {}
  const deliveryGate = new Promise<void>(resolve => { release = resolve })
  const deliveryEntered = new Promise<void>(resolve => { entered = resolve })
  let deliveries = 0
  const namespace = {
    idFromName: (name: string) => name,
    get: () => ({
      ingestInitialGameStarted: async (request: CompetitionRoomInitialGameStartedRequest) => {
        deliveries += 1
        entered()
        await deliveryGate
        return initialFifoAck(request)
      },
      ingestGameTerminal: async () => { throw new Error('unexpected terminal delivery') },
    }),
  }
  const { room, pendingWaits } = gameRoom(storage, namespace)
  const creation = { gameId: 'eligibility-game', binding: binding() }
  await createReleasedCompetitionGame(room, creation)
  await prepareAllMembers(storage)
  await room.startCompetitionGame(creation)
  await drainWaitUntil(pendingWaits)
  const delivery = room.alarm()
  await deliveryEntered
  const before = structuredClone(storage.values)
  const writesBefore = storage.writeCount
  const alarmBefore = storage.alarm
  const readsBefore = storage.readRequests.length
  let watchdog: ReturnType<typeof setTimeout> | undefined
  try {
    // 若 probe 誤進 serialized tail，這次查詢會等待未釋放的 delivery，不能成功。
    const result = await Promise.race([
      room.getCompetitionClockControlEligibility({ competitionId: 'competition-1', matchId: 'match-1',
        gameId: creation.gameId, gameNumber: 1 }),
      new Promise<never>((_, reject) => {
        watchdog = setTimeout(() => reject(new Error('snapshot waited for serialized delivery')), 250)
      }),
    ])
    expect(result).toBe(true)
    expect(deliveries).toBe(1)
    expect(storage.readRequests.slice(readsBefore)).toStrictEqual([
      ['metadata', 'competitionBinding', 'gameRecord', 'competitionDeliveryQueue', 'competitionGameAdmission'],
    ])
    expect(storage.values).toStrictEqual(before)
    expect(storage.writeCount).toBe(writesBefore)
    expect(storage.alarm).toBe(alarmBefore)
  } finally {
    if (watchdog !== undefined) clearTimeout(watchdog)
    release()
    await delivery
  }
})

test('E4d3c-r3 eligibility rejects a clock-control receipt bound to another current game', async () => {
  rulesEngineMode = 'normal'
  const storage = new MemoryStorage()
  const namespace = {
    idFromName: (name: string) => name,
    get: () => ({ executeMatch: async (request: Record<string, unknown>) => terminalSuccess(request),
      ingestGameTerminal: async () => { throw new Error('unexpected terminal') } }),
  }
  const { room, pendingWaits } = gameRoom(storage, withInitialStartReceiver(namespace))
  const fixed = binding()
  const creation = { gameId: 'eligibility-game', binding: fixed }
  await createReleasedCompetitionGame(room, creation)
  await prepareAllMembers(storage)
  await room.startCompetitionGame(creation)
  await drainWaitUntil(pendingWaits)
  await room.alarm()
  const cursor = await storage.get<{ revisionCursor: number }>('competitionDeliveryQueue')
  if (!cursor) throw new Error('missing delivery queue')
  await room.synchronizeCompetitionClock(clockControlRequest(fixed, {
    gameId: creation.gameId, matchRevision: cursor.revisionCursor + 1,
  }))
  const request = { competitionId: fixed.competitionId, matchId: fixed.matchId,
    gameId: creation.gameId, gameNumber: fixed.gameNumber }
  expect(await room.getCompetitionClockControlEligibility(request)).toBe(true)
  const queue = await storage.get<{ clockControlReceipts: Array<{
    request: CompetitionClockControlRequest; ack: CompetitionClockControlRequest & { idempotent: boolean }
  }> }>('competitionDeliveryQueue')
  if (!queue || queue.clockControlReceipts.length !== 1) throw new Error('pause receipt is missing')
  const wrong = structuredClone(queue)
  const receipt = wrong.clockControlReceipts[0]
  if (!receipt) throw new Error('pause receipt is missing')
  receipt.request.gameId = 'another-game'
  receipt.ack.gameId = 'another-game'
  await storage.put('competitionDeliveryQueue', wrong)
  const before = structuredClone(storage.values)
  const writesBefore = storage.writeCount
  const alarmBefore = storage.alarm
  expect(await room.getCompetitionClockControlEligibility(request)).toBe(false)
  expect(storage.values).toStrictEqual(before)
  expect(storage.writeCount).toBe(writesBefore)
  expect(storage.alarm).toBe(alarmBefore)
})

async function startEligibilityFaultFixture() {
  rulesEngineMode = 'normal'
  const storage = new MemoryStorage()
  let remoteCalls = 0
  const namespace = {
    idFromName: (name: string) => name,
    get: () => ({
      ingestInitialGameStarted: async (request: CompetitionRoomInitialGameStartedRequest) => {
        remoteCalls += 1
        return initialFifoAck(request)
      },
      executeMatch: async (request: Record<string, unknown>) => {
        remoteCalls += 1
        return terminalSuccess(request)
      },
      ingestGameTerminal: async () => {
        remoteCalls += 1
        throw new Error('unexpected terminal delivery in eligibility fixture')
      },
    }),
  }
  const created = gameRoom(storage, namespace)
  const fixed = binding()
  const creation = { gameId: 'eligibility-fault-game', binding: fixed }
  await createReleasedCompetitionGame(created.room, creation)
  await prepareAllMembers(storage)
  expect((await created.room.startCompetitionGame(creation)).status).toBe('Active')
  await drainWaitUntil(created.pendingWaits)
  // 僅平台 socket fixture；不冒稱真原生 WebSocket 驗收。
  const socket = new FakeSocket()
  socket.serializeAttachment({ userId: 'user-a' })
  created.sockets.push(socket)
  const request: CompetitionGameClockControlEligibilityRequest = {
    competitionId: fixed.competitionId, matchId: fixed.matchId,
    gameId: creation.gameId, gameNumber: fixed.gameNumber,
  }
  const check = async (expected: boolean) => {
    const before = structuredClone(storage.values)
    const writesBefore = storage.writeCount
    const alarmBefore = storage.alarm
    const sqlBefore = storage.sqlAccessCount
    const engineBefore = rulesEngineBoundaryCalls
    const remoteBefore = remoteCalls
    const waitsBefore = created.pendingWaits.length
    const readsBefore = storage.readRequests.length
    const socketsBefore = structuredClone(created.sockets.map(current => ({
      sent: current.sent, readyState: current.readyState, attachment: current.attachment,
    })))
    const actual = await created.room.getCompetitionClockControlEligibility(request)
    // 即使最後 eligibility assertion 是 red，所有不變性仍先逐一執行。
    expect(storage.values).toStrictEqual(before)
    expect(storage.writeCount).toBe(writesBefore)
    expect(storage.alarm).toBe(alarmBefore)
    expect(storage.sqlAccessCount).toBe(sqlBefore)
    expect(rulesEngineBoundaryCalls).toBe(engineBefore)
    expect(remoteCalls).toBe(remoteBefore)
    expect(created.pendingWaits).toHaveLength(waitsBefore)
    expect(created.sockets.map(current => ({
      sent: current.sent, readyState: current.readyState, attachment: current.attachment,
    }))).toStrictEqual(socketsBefore)
    expect(storage.readRequests.slice(readsBefore)).toStrictEqual([
      ['metadata', 'competitionBinding', 'gameRecord', 'competitionDeliveryQueue', 'competitionGameAdmission'],
    ])
    expect(actual).toBe(expected)
  }
  return { ...created, storage, fixed, request, check }
}

test('E4d3c-r3 receipt revision cannot be ahead of the cursor after a legitimate start ACK and pause', async () => {
  const { room, storage, fixed, request, check } = await startEligibilityFaultFixture()
  await room.alarm()
  expect(await storage.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 2, entries: [] })
  expect(await room.synchronizeCompetitionClock(clockControlRequest(fixed, {
    gameId: request.gameId, matchRevision: 3,
  }))).toMatchObject({ ok: true, value: { matchRevision: 3 } })
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  if (!queue) throw new Error('pause queue is missing')
  expect(queue).toMatchObject({ revisionCursor: 3, refereePaused: true, clockControlReceipts: [
    { request: { matchRevision: 3 }, ack: { matchRevision: 3 } },
  ] })
  await check(true)
  // 只改 cursor；合法 pause receipt／ACK 及其他全部欄位維持原狀。
  await storage.put('competitionDeliveryQueue', { ...structuredClone(queue), revisionCursor: 2 })
  await check(false)
})

test('E4d3c-r3 pending initial evidence must identify the same match as its outer entry and binding', async () => {
  const { storage, fixed, check } = await startEligibilityFaultFixture()
  const queue = await storage.get<{ revisionCursor: number; entries: InitialFifoEntry[] }>('competitionDeliveryQueue')
  if (!queue) throw new Error('pending start queue is missing')
  expect(queue.revisionCursor).toBe(1)
  expect(queue.entries).toHaveLength(1)
  expect(queue.entries[0]).toMatchObject({
    kind: 'competitionInitialGameStarted', matchId: fixed.matchId,
    evidence: { bindingRef: { matchId: fixed.matchId } },
  })
  await check(true)
  const wrong = structuredClone(queue)
  const entry = wrong.entries[0]
  if (!entry) throw new Error('pending start entry is missing')
  // 只改內層 bindingRef.matchId；外層 identity／binding 及其餘快照保持原狀。
  entry.evidence.bindingRef.matchId = 'another-match'
  await storage.put('competitionDeliveryQueue', wrong)
  await check(false)
})

test('E4d3c-r3 canonical finish and acknowledged timeout deny eligibility without changing game records', async () => {
  const realNow = Date.now
  const startedAt = realNow()
  let now = startedAt
  Date.now = () => now
  try {
    for (const timedOut of [false, true]) {
      now = startedAt
      rulesEngineMode = 'normal'
      const storage = new MemoryStorage()
      const evidence = { type: 'timeout', timeoutId: 'eligibility-timeout',
        expiries: [{ team: 'team-a', deadlineMonotonicMs: startedAt + 500 }],
        observedAtMonotonicMs: startedAt + 1000, clockCheckpointDigest: 'a'.repeat(64) }
      const namespace = {
        idFromName: (name: string) => name,
        get: () => ({
          ingestInitialGameStarted: async (request: CompetitionRoomInitialGameStartedRequest) => initialFifoAck(request, startedAt + 500),
          executeMatch: async (request: Record<string, unknown>) => {
            const operation = request.operation
            const expired = timedOut && typeof operation === 'object' && operation !== null
              && 'type' in operation && operation.type === 'clockObserve'
            const result = clockSuccess(request, expired ? evidence : null)
            // mock 權威 ACK 的開始期限與後續 timeout evidence 必須是同一個 500ms。
            return expired ? result : { ...result, value: { ...result.value, nextClockDeadlineMs: startedAt + 500 } }
          },
          ingestGameTerminal: async (request: Record<string, unknown>) => terminalSuccess(request),
        }),
      }
      const { room, pendingWaits } = gameRoom(storage, namespace)
      const fixed = binding()
      const creation = { gameId: 'eligibility-game', binding: fixed }
      await createReleasedCompetitionGame(room, creation)
      await prepareAllMembers(storage)
      await room.startCompetitionGame(creation)
      await drainWaitUntil(pendingWaits)
      await room.alarm()
      const request = { competitionId: fixed.competitionId, matchId: fixed.matchId,
        gameId: creation.gameId, gameNumber: fixed.gameNumber }
      expect(await room.getCompetitionClockControlEligibility(request)).toBe(true)
      const recordBefore = await storage.get<GameRecord>('gameRecord')
      if (!recordBefore) throw new Error('active record is missing')
      if (timedOut) {
        now = startedAt + 1000
        await room.enqueueCompetitionClockMutation({ competitionId: fixed.competitionId, matchId: fixed.matchId,
          callerUserId: fixed.organizerUserId, commandId: 'observe-expiry', trustedTimeMs: now,
          operation: { type: 'clockObserve', matchId: fixed.matchId } })
        await drainWaitUntil(pendingWaits)
        await room.alarm()
        expect(await storage.get('competitionDeliveryQueue')).toMatchObject({ terminalTimeout: evidence })
        expect(await storage.get('gameRecord')).toStrictEqual(recordBefore)
        expect((await storage.get<GameRoomMetadata>('metadata'))?.status).toBe('Active')
      } else {
        rulesEngineMode = 'finished'
        expect((await post(room, { type: 'submitCommand', commandId: 'finish-game',
          gameInstanceId: recordBefore.gameInstanceId, transactionId: 'finish-transaction', actorUserId: 'user-a',
          action: { type: 'passAction', reason: 'NoCardsInHand' } })).status).toBe(200)
        await drainWaitUntil(pendingWaits)
        expect((await storage.get<GameRoomMetadata>('metadata'))?.status).toBe('Finished')
      }
      const before = structuredClone(storage.values)
      const writesBefore = storage.writeCount
      const alarmBefore = storage.alarm
      expect(await room.getCompetitionClockControlEligibility(request)).toBe(false)
      expect(await gameRoom(storage, namespace).room.getCompetitionClockControlEligibility(request)).toBe(false)
      expect(storage.values).toStrictEqual(before)
      expect(storage.writeCount).toBe(writesBefore)
      expect(storage.alarm).toBe(alarmBefore)
    }
  } finally {
    Date.now = realNow
    rulesEngineMode = 'normal'
  }
})

function expectSafeCompetitionContext(value: unknown) {
  expect(value).toHaveProperty('competition', {
    competitionId: 'competition-1', matchId: 'match-1', gameNumber: 1,
  })
  const wire = JSON.stringify(value)
  for (const privatePart of ['deckSnapshot', 'deckSeed', 'expectedMatchRevision', 'organizer-1',
    'competition-1:match-1:game-1', '鎖定牌組:', '鎖定牌:']) {
    expect(wire).not.toContain(privatePart)
  }
  expect(value).not.toHaveProperty('binding')
  expect(value).not.toHaveProperty('turnOrder')
  expect(value).not.toHaveProperty('players')
  if (typeof value !== 'object' || value === null || !('competition' in value)
    || typeof value.competition !== 'object' || value.competition === null) {
    throw new Error('競賽公開 context fixture 格式錯誤')
  }
  expect(Reflect.ownKeys(value.competition)).toEqual(['competitionId', 'matchId', 'gameNumber'])
}

async function post(room: InstanceType<typeof GameRoom>, body: unknown) {
  return await room.fetch(new Request('https://game-room.internal/', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(body),
  }))
}

test('競賽 binding 只能由可信 RPC 建立，重試相同 binding 冪等，衝突則拒絕', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-game-1', binding: binding() }

  const created = await room.createCompetitionGame(request)
  expect(created.status).toBe('Waiting')
  expect(Reflect.ownKeys(created)).toEqual(['gameId', 'bindingDigest', 'status'])

  const retry = await room.createCompetitionGame(request)
  expect(retry).toEqual(created)
  await expect(createReleasedCompetitionGame(room, {
    ...request,
    binding: { ...request.binding, gameNumber: 2 },
  })).rejects.toThrow('competition game binding does not match')

  const response = await post(room, { type: 'getState', actorUserId: 'user-a' })
  const publicBody = await response.json() as Record<string, unknown>
  expect(response.status).toBe(200)
  expect(publicBody.competition).toEqual({
    competitionId: 'competition-1', matchId: 'match-1', gameNumber: 1,
  })
  expect(publicBody).not.toHaveProperty('competitionId')
  expect(publicBody).not.toHaveProperty('binding')
  expect(publicBody).not.toHaveProperty('lockedDeckName')
  expect(JSON.stringify(publicBody)).not.toContain('A locked deck')
  expect(JSON.stringify(publicBody)).not.toContain('a-1')
  expect(storage.values.get('competitionBinding')).toEqual(request.binding)
})

test('安全競賽 context 唯讀查詢不寫 metadata、events、receipts、alarm 或時計交付', async () => {
  const storage = new MemoryStorage()
  const { room, pendingWaits } = gameRoom(storage)
  await createReleasedCompetitionGame(room, { gameId: 'competition-context-readonly', binding: binding() })
  const before = structuredClone(storage.values)
  const beforeWrites = storage.writeCount
  const beforeAlarm = storage.alarm
  for (const actorUserId of ['user-a', 'user-b']) {
    const response = await post(room, { type: 'getState', actorUserId })
    expect(response.status).toBe(200)
    const value: unknown = await response.json()
    expect(value).toMatchObject({
      competition: { competitionId: 'competition-1', matchId: 'match-1', gameNumber: 1 },
    })
  }
  expect(storage.writeCount).toBe(beforeWrites)
  expect(storage.values).toEqual(before)
  expect(storage.alarm).toBe(beforeAlarm)
  expect(pendingWaits).toHaveLength(0)
})

test('損壞的 present binding 唯讀查詢 fail closed 且不先寫回 metadata', async () => {
  const storage = new MemoryStorage()
  const { room, pendingWaits } = gameRoom(storage)
  await createReleasedCompetitionGame(room, { gameId: 'competition-context-corrupt', binding: binding() })
  await storage.put('competitionBinding', { ...binding(), deckSeed: '' })
  const before = structuredClone(storage.values)
  const beforeWrites = storage.writeCount
  const beforeAlarm = storage.alarm
  const response = await post(room, { type: 'getState', actorUserId: 'user-a' })
  expect(response.status).toBe(500)
  const value: unknown = await response.json()
  expect(value).toMatchObject({ code: 'gameRoomFailure' })
  expect(value).not.toHaveProperty('competition')
  expect(value).not.toHaveProperty('metadata')
  expect(value).not.toHaveProperty('binding')
  expect(storage.values).toEqual(before)
  expect(storage.writeCount).toBe(beforeWrites)
  expect(storage.alarm).toBe(beforeAlarm)
  expect(pendingWaits).toHaveLength(0)
})

test('競賽 context 在 1v1／2v2、個人／預組的 HTTP、WS、重啟與 canonical Finished 保持相同安全 view', async () => {
  competitionContextPrivateHands = true
  rulesEngineMode = 'normal'
  try {
    for (const capacity of [2, 4] satisfies Array<2 | 4>) {
      for (const personal of [true, false]) {
        const storage = new MemoryStorage()
        const created = gameRoom(storage)
        const request = { gameId: `competition-mode-${capacity}-${personal}`, binding: contextBinding(capacity, personal) }
        await createReleasedCompetitionGame(created.room, request)

        const readOnlyViews = async (status: 'Waiting' | 'Active' | 'Finished') => {
          const before = structuredClone(storage.values)
          const beforeWrites = storage.writeCount
          const beforeAlarm = storage.alarm
          for (const actor of request.binding.players) {
            const response = await post(created.room, { type: 'getState', actorUserId: actor.userId })
            expect(response.status).toBe(200)
            const value: unknown = await response.json()
            expectSafeCompetitionContext(value)
            expect(value).toHaveProperty('metadata.status', status)
            if (status !== 'Waiting') {
              expect(JSON.stringify(value)).toContain(`私有手牌:${actor.player}`)
              for (const other of request.binding.players.filter(player => player.player !== actor.player)) {
                expect(JSON.stringify(value)).not.toContain(`私有手牌:${other.player}`)
              }
            }
          }
          for (const outsider of ['outsider', 'observer']) {
            const denied = await post(created.room, { type: 'getState', actorUserId: outsider })
            expect(denied.status).toBe(404)
            const deniedValue: unknown = await denied.json()
            expect(deniedValue).not.toHaveProperty('competition')
            expect(deniedValue).not.toHaveProperty('metadata')
            const deniedSocket = await created.room.fetch(new Request('https://game-room.internal/socket', {
              headers: { Upgrade: 'websocket', 'x-fewfc-user-id': outsider, 'x-fewfc-user-name': outsider },
            }))
            expect(deniedSocket.status).toBe(403)
          }
          const anonymous = await created.room.fetch(new Request('https://game-room.internal/?actorUserId=user-a&competitionId=forged'))
          expect(anonymous.status).toBe(403)
          expect(await anonymous.json()).toMatchObject({ code: 'competitionMemberRequired' })
          expect(storage.values).toStrictEqual(before)
          expect(storage.writeCount).toBe(beforeWrites)
          expect(storage.alarm).toBe(beforeAlarm)
        }
        const socketViews = (status: 'Waiting' | 'Active' | 'Finished') => {
          for (const actor of request.binding.players) {
            const socket = created.sockets.find(candidate => candidate.readyState === 1
              && typeof candidate.attachment === 'object' && candidate.attachment !== null
              && 'userId' in candidate.attachment && candidate.attachment.userId === actor.userId)
            if (!socket) throw new Error('固定成員 socket fixture 未連線')
            const wire = socket.sent.at(-1)
            if (!wire) throw new Error('固定成員 socket fixture 未收到狀態')
            const value: unknown = JSON.parse(wire)
            expect(value).toHaveProperty('type', 'roomState')
            if (typeof value !== 'object' || value === null || !('data' in value)) {
              throw new Error('roomState fixture 格式錯誤')
            }
            expectSafeCompetitionContext(value.data)
            expect(value.data).toHaveProperty('metadata.status', status)
            if (status !== 'Waiting') {
              expect(wire).toContain(`私有手牌:${actor.player}`)
              for (const other of request.binding.players.filter(player => player.player !== actor.player)) {
                expect(wire).not.toContain(`私有手牌:${other.player}`)
              }
            }
          }
        }
        const reconnectAndRestart = async (status: 'Waiting' | 'Active' | 'Finished') => {
          const beforeRecord = structuredClone(storage.values.get('gameRecord'))
          const socketA = created.sockets.find(socket => socket.readyState === 1
            && typeof socket.attachment === 'object' && socket.attachment !== null
            && 'userId' in socket.attachment && socket.attachment.userId === 'user-a')
          if (!socketA) throw new Error('固定成員重連 fixture 未連線')
          socketA.readyState = 3
          await created.room.webSocketClose(socketA)
          await connectCompetitionMember(created.room, created.sockets, 'user-a')
          await drainWaitUntil(created.pendingWaits)
          socketViews(status)
          expect(await storage.get('gameRecord')).toStrictEqual(beforeRecord)

          const beforeRestart = structuredClone(storage.values)
          const beforeRestartWrites = storage.writeCount
          const beforeRestartAlarm = storage.alarm
          const restarted = gameRoom(storage)
          const afterRestart = await post(restarted.room, { type: 'getState', actorUserId: 'user-b' })
          expect(afterRestart.status).toBe(200)
          const value: unknown = await afterRestart.json()
          expectSafeCompetitionContext(value)
          expect(value).toHaveProperty('metadata.status', status)
          expect(storage.values).toStrictEqual(beforeRestart)
          expect(storage.writeCount).toBe(beforeRestartWrites)
          expect(storage.alarm).toBe(beforeRestartAlarm)
          expect(restarted.pendingWaits).toHaveLength(0)
        }

        await readOnlyViews('Waiting')
        const invalidSockets = ['outsider', 'observer'].map(userId => {
          const socket = new FakeSocket()
          socket.serializeAttachment({ userId })
          created.sockets.push(socket)
          return socket
        })
        for (const actor of request.binding.players) {
          await connectCompetitionMember(created.room, created.sockets, actor.userId)
          await drainWaitUntil(created.pendingWaits)
        }
        for (const socket of invalidSockets) {
          expect(socket.readyState).toBe(3)
          expect(socket.sent).toEqual([])
        }
        socketViews('Waiting')
        await reconnectAndRestart('Waiting')
        for (const actor of request.binding.players) {
          expect(await created.room.setCompetitionReady({ actorUserId: actor.userId, ready: true }))
            .toEqual({ ok: true, ready: true })
        }
        await drainWaitUntil(created.pendingWaits)
        await readOnlyViews('Active')
        socketViews('Active')
        await reconnectAndRestart('Active')

        const record = await storage.get<GameRecord>('gameRecord')
        if (!record) throw new Error('可信 ready fixture 未建立 game record')
        rulesEngineMode = 'finished'
        const terminal = await post(created.room, {
          type: 'submitCommand', commandId: 'context-terminal', transactionId: 'context-terminal-transaction',
          gameInstanceId: record.gameInstanceId, actorUserId: 'user-a',
          action: { type: 'passAction', reason: 'NoCardsInHand' },
        })
        expect(terminal.status).toBe(200)
        expectSafeCompetitionContext(await terminal.json())
        await drainWaitUntil(created.pendingWaits)
        await readOnlyViews('Finished')
        socketViews('Finished')
        await reconnectAndRestart('Finished')
        rulesEngineMode = 'normal'
      }
    }
  } finally {
    competitionContextPrivateHands = false
    rulesEngineMode = 'normal'
  }
})

test('普通房不含 competition，payload／query 不能偽造或覆蓋持久競賽 context', async () => {
  const storage = new MemoryStorage()
  const ordinary = gameRoom(storage).room
  const fakeContext = { competitionId: 'forged-competition', matchId: 'forged-match', gameNumber: 99 }
  const created = await post(ordinary, {
    type: 'createGame', gameId: 'ordinary-context', actorUserId: 'owner', actorName: 'Owner',
    capacity: 2, invitation: { roomCode: 'ABCD234', inviteToken: 'ordinary-secret' },
    competition: fakeContext, binding: fakeContext,
  })
  expect(created.status).toBe(200)
  expect(await created.json()).not.toHaveProperty('competition')
  const anonymous = await ordinary.fetch(new Request('https://game-room.internal/?competitionId=forged-competition&matchId=forged-match&gameNumber=99&competition=true'))
  expect(anonymous.status).toBe(200)
  expect(await anonymous.json()).not.toHaveProperty('competition')
  const state = await post(ordinary, { type: 'getState', actorUserId: 'owner', competition: fakeContext })
  expect(state.status).toBe(200)
  expect(await state.json()).not.toHaveProperty('competition')
  expect(storage.values.has('competitionBinding')).toBe(false)

  const competitionStorage = new MemoryStorage()
  const competition = gameRoom(competitionStorage).room
  await createReleasedCompetitionGame(competition, { gameId: 'real-context', binding: binding() })
  const before = structuredClone(competitionStorage.values)
  const beforeWrites = competitionStorage.writeCount
  const response = await competition.fetch(new Request('https://game-room.internal/?competitionId=forged-competition&gameNumber=99', {
    method: 'POST', headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ type: 'getState', actorUserId: 'user-a', competition: fakeContext, binding: fakeContext }),
  }))
  expect(response.status).toBe(200)
  const value: unknown = await response.json()
  expectSafeCompetitionContext(value)
  expect(JSON.stringify(value)).not.toContain('forged-competition')
  expect(competitionStorage.values).toStrictEqual(before)
  expect(competitionStorage.writeCount).toBe(beforeWrites)
})

test('present binding 與固定 metadata 的矛盾／畸形不能變成普通房或唯讀修補', async () => {
  const validBinding = binding()
  const bindingCases: unknown[] = [null, {}, { ...validBinding, gameNumber: 0 },
    { ...validBinding, firstPlayer: 'outsider' }, { ...validBinding, turnOrder: ['player-b', 'player-a'] }]
  for (const damaged of bindingCases) {
    const storage = new MemoryStorage()
    const { room, pendingWaits } = gameRoom(storage)
    await createReleasedCompetitionGame(room, { gameId: 'damaged-binding-context', binding: validBinding })
    await storage.put('competitionBinding', damaged)
    const before = structuredClone(storage.values)
    const beforeWrites = storage.writeCount
    const response = await post(room, { type: 'getState', actorUserId: 'user-a' })
    expect(response.status).toBe(500)
    expect(await response.json()).toMatchObject({ code: 'gameRoomFailure' })
    expect(storage.values).toStrictEqual(before)
    expect(storage.writeCount).toBe(beforeWrites)
    expect(storage.alarm).toBeNull()
    expect(pendingWaits).toHaveLength(0)
    await expect(room.fetch(new Request('https://game-room.internal/'))).rejects.toThrow()
    expect(storage.values).toStrictEqual(before)
    expect(storage.writeCount).toBe(beforeWrites)
  }
  const storage = new MemoryStorage()
  const { room, pendingWaits } = gameRoom(storage)
  await createReleasedCompetitionGame(room, { gameId: 'damaged-metadata-context', binding: validBinding })
  const metadata = await storage.get<GameRoomMetadata>('metadata')
  if (!metadata) throw new Error('固定 metadata fixture 尚未建立')
  const overrides: Array<Record<string, unknown>> = [
    { schemaVersion: 4 }, { capacity: 4 }, { access: 'public' },
    { players: ['player-b', 'player-a'] }, { enabledRuleModules: [] },
    { members: metadata.members.map(member => ({ ...member, owner: false })) },
    { members: metadata.members.map(member => ({ ...member, ready: undefined })) },
    { members: metadata.members.map(member => ({ ...member, userId: 'user-a' })) },
    { observers: [{ userId: 'observer', displayName: 'Observer', connected: true }] },
    { private: 'private-metadata-secret' },
  ]
  for (const override of overrides) {
    await storage.put('metadata', { ...metadata, ...override })
    const before = structuredClone(storage.values)
    const beforeWrites = storage.writeCount
    const response = await post(room, { type: 'getState', actorUserId: 'user-a' })
    expect(response.status).toBe(500)
    const text = await response.text()
    expect(JSON.parse(text)).toMatchObject({ code: 'gameRoomFailure' })
    expect(text).not.toMatch(/private-metadata-secret|deckSeed|competitionId|matchId/)
    expect(storage.values).toStrictEqual(before)
    expect(storage.writeCount).toBe(beforeWrites)
    expect(storage.alarm).toBeNull()
    expect(pendingWaits).toHaveLength(0)
  }
})

test('競賽 create/start 的 batch 失敗不留下半成品，清除故障後可安全重試', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-game-atomic', binding: binding() }

  storage.failNextBatch = true
  await expect(createReleasedCompetitionGame(room, request)).rejects.toThrow('simulated atomic storage failure')
  expect(storage.values.has('metadata')).toBe(false)
  expect(storage.values.has('competitionBinding')).toBe(false)

  await createReleasedCompetitionGame(room, request)
  await prepareAllMembers(storage)
  storage.failNextBatch = true
  await expect(room.startCompetitionGame(request)).rejects.toThrow('simulated atomic storage failure')
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(storage.values.has('gameRecord')).toBe(false)

  const started = await room.startCompetitionGame(request)
  expect(started.status).toBe('Active')
})

test('start transaction atomically records GameStarted and the no-preparation action clock with its alarm', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-start-clock', binding: binding() }
  await createReleasedCompetitionGame(room, request)
  await prepareAllMembers(storage)

  storage.failNextBatch = true
  await expect(room.startCompetitionGame(request)).rejects.toThrow('simulated atomic storage failure')
  expect(await storage.get('gameRecord')).toBeUndefined()
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toEqual([])
  expect(storage.alarm).toBeNull()

  await room.startCompetitionGame(request)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Active')
  expect(record?.sequence).toBe(2)
  expect(queue?.revisionCursor).toBe(request.binding.expectedMatchRevision)
  expect(queue?.entries).toEqual([expect.objectContaining({
    kind: 'competitionInitialGameStarted',
    evidence: expect.objectContaining({ initialWindow: { type: 'action', team: 'team-a', turn: 1 } }),
  })])
  expect(storage.alarm).not.toBeNull()
})

test('multi-game start queues reset or carryForward before its action clock with shared trusted time', async () => {
  const continuation = {
    type: 'eliminationDrawContinuation' as const,
    continuation: {
      teams: [
        { team: 'team-a', mainTimeRemainingMs: 1_237 },
        { team: 'team-b', mainTimeRemainingMs: null },
      ],
    },
  }
  const cases = [
    { gameNumber: 2, startKind: { type: 'ordinaryNewGame' as const }, time: 'reset' },
    { gameNumber: 4, startKind: { type: 'ordinaryNewGame' as const }, time: 'reset' },
    { gameNumber: 2, startKind: continuation, time: 'carryForward' },
  ] as const

  for (const [index, scenario] of cases.entries()) {
    const storage = new MemoryStorage()
    const { room } = gameRoom(storage)
    const gameBinding = binding({
      gameNumber: scenario.gameNumber,
      expectedMatchRevision: 10 + index,
      startKind: scenario.startKind,
    })
    const request = { gameId: `competition-multi-game-${index}`, binding: gameBinding }
    await createReleasedCompetitionGame(room, request)
    await prepareAllMembers(storage)
    await room.startCompetitionGame(request)

    const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
    const entries = queue?.entries as Array<Record<string, unknown>>
    expect(entries.map(entry => entry.operation)).toEqual([
      { type: 'clockNewGame', matchId: gameBinding.matchId, time: scenario.time },
      { type: 'clockBeginAction', matchId: gameBinding.matchId, team: 'team-a', turn: 1 },
    ])
    expect(new Set(entries.map(entry => entry.commandId)).size).toBe(2)
    expect(new Set(entries.map(entry => entry.trustedTimeMs)).size).toBe(1)
    expect(queue?.revisionCursor).toBe(gameBinding.expectedMatchRevision)
    expect(await storage.get('competitionBinding')).toEqual(gameBinding)
  }
})

test('multi-game clock FIFO advances only after strict ACK and survives first response loss plus restart', async () => {
  const storage = new MemoryStorage()
  const requests: Array<Record<string, unknown>> = []
  let responseMode: 'lost' | 'invalid' | 'ack' = 'lost'
  const namespace = {
    idFromName: () => 'competition-object-id' as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        const current = request as Record<string, unknown>
        requests.push(structuredClone(current))
        if (responseMode === 'lost') throw new Error('simulated committed response loss')
        const expectedRevision = Number(current.expectedMatchRevision) + 1
        return {
          ok: true,
          value: {
            competitionId: current.competitionId,
            matchId: current.matchId,
            matchRevision: responseMode === 'invalid' ? expectedRevision + 1 : expectedRevision,
            commandId: current.commandId,
            idempotent: requests.length > 1,
            timeoutEvidence: null,
            nextClockDeadlineMs: null,
            matchProjection: null,
            clockProjection: null,
          },
        }
      },
      ingestGameTerminal: async () => { throw new Error('not used') },
    }),
  }
  const gameBinding = binding({ gameNumber: 2, expectedMatchRevision: 23 })
  const startRequest = { gameId: 'competition-start-fifo-restart', binding: gameBinding }
  const { room } = gameRoom(storage, namespace)
  await createReleasedCompetitionGame(room, startRequest)
  await prepareAllMembers(storage)
  await room.startCompetitionGame(startRequest)
  const initialQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  const initialEntries = initialQueue?.entries as Array<Record<string, unknown>>

  await room.alarm()
  expect(requests.map(request => (request.operation as Record<string, unknown>).type))
    .toEqual(['clockNewGame'])
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.revisionCursor)
    .toBe(23)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries)
    .toHaveLength(2)

  const retryQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  const retryEntries = retryQueue?.entries as Array<Record<string, unknown>>
  await storage.put('competitionDeliveryQueue', {
    ...retryQueue,
    entries: [{ ...retryEntries[0], nextAttemptAt: 0 }, retryEntries[1]],
  })
  responseMode = 'invalid'
  await gameRoom(storage, namespace).room.alarm()
  expect(requests.map(request => (request.operation as Record<string, unknown>).type))
    .toEqual(['clockNewGame', 'clockNewGame'])
  expect(requests[0]?.commandId).toBe(requests[1]?.commandId)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.revisionCursor)
    .toBe(23)

  const invalidAckQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  const invalidAckEntries = invalidAckQueue?.entries as Array<Record<string, unknown>>
  await storage.put('competitionDeliveryQueue', {
    ...invalidAckQueue,
    entries: [{ ...invalidAckEntries[0], nextAttemptAt: 0 }, invalidAckEntries[1]],
  })
  responseMode = 'ack'
  await gameRoom(storage, namespace).room.alarm()

  expect(requests.map(request => (request.operation as Record<string, unknown>).type))
    .toEqual(['clockNewGame', 'clockNewGame', 'clockNewGame', 'clockBeginAction'])
  expect(requests[0]?.commandId).toBe(requests[2]?.commandId)
  expect(requests[0]?.trustedTimeMs).toBe(requests[3]?.trustedTimeMs)
  expect(requests[0]?.expectedMatchRevision).toBe(23)
  expect(requests[3]?.expectedMatchRevision).toBe(24)
  const finalQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect(finalQueue?.revisionCursor).toBe(25)
  expect(finalQueue?.entries).toEqual([])
  expect(new Set(initialEntries.map(entry => entry.commandId)).size).toBe(2)
})

test('multi-game start batch or alarm failure rolls back the complete start and clock FIFO', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = {
    gameId: 'competition-start-fifo-atomic',
    binding: binding({ gameNumber: 2, expectedMatchRevision: 9 }),
  }
  await createReleasedCompetitionGame(room, request)
  await prepareAllMembers(storage)
  const initialEvents = storedRoomEvents(storage)

  storage.failNextBatch = true
  await expect(room.startCompetitionGame(request)).rejects.toThrow('simulated atomic storage failure')
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(await storage.get('gameRecord')).toBeUndefined()
  expect(storedRoomEvents(storage)).toEqual(initialEvents)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toEqual([])
  expect(storage.alarm).toBeNull()

  storage.failNextAlarm = true
  await expect(room.startCompetitionGame(request)).rejects.toThrow('simulated alarm transaction failure')
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(await storage.get('gameRecord')).toBeUndefined()
  expect(storedRoomEvents(storage)).toEqual(initialEvents)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toEqual([])
  expect(storage.alarm).toBeNull()

  await room.startCompetitionGame(request)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries)
    .toHaveLength(2)
})

test('canonical public state transition and GameRoom checkpoint commit atomically', async () => {
  rulesEngineMode = 'clockTransition'
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage, withInitialStartReceiver())
  const request = {
    gameId: 'competition-clock-transition',
    binding: binding({ enabledRuleModules: ['personal-deck', 'pouch'] }),
  }
  await createReleasedCompetitionGame(room, request)
  await prepareAllMembers(storage)
  await room.startCompetitionGame(request)
  await room.alarm()
  const recordBefore = await storage.get('gameRecord') as { gameInstanceId: string }

  storage.failNextBatch = true
  const rejected = await post(room, {
    type: 'submitCommand',
    commandId: 'clock-transition-atomic',
    gameInstanceId: recordBefore.gameInstanceId,
    transactionId: 'clock-transition-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(rejected.status).toBe(500)
  expect(await storage.get('gameRecord')).toEqual(recordBefore)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toEqual([])
  expect(storage.alarm).toBeNull()

  const accepted = await post(room, {
    type: 'submitCommand',
    commandId: 'clock-transition-atomic',
    gameInstanceId: recordBefore.gameInstanceId,
    transactionId: 'clock-transition-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(accepted.status).toBe(200)
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect(queue?.entries).toEqual([
    expect.objectContaining({ operation: { type: 'clockFinishPreparation', matchId: 'match-1', team: 'team-a' } }),
    expect.objectContaining({ operation: { type: 'clockFinishPreparation', matchId: 'match-1', team: 'team-b' } }),
    expect.objectContaining({ operation: { type: 'clockBeginAction', matchId: 'match-1', team: 'team-b', turn: 2 } }),
  ])
  expect(storage.alarm).not.toBeNull()
  rulesEngineMode = 'normal'
})

test('矛盾的正式開局 state 在 GameStarted transaction 前拒絕且不寫入', async () => {
  rulesEngineMode = 'normal'
  const invalidStates: Array<Partial<PublicGameState>> = [
    {
      status: 'Preparing',
      phase: 'TurnStart',
      currentPlayer: null,
      turnNumber: 1,
      initialPouchSelection: null,
    },
    {
      status: 'InProgress',
      phase: 'ActiveEffects',
      currentPlayer: 'unbound-player',
      turnNumber: 1,
      initialPouchSelection: null,
    },
  ]

  try {
    for (const [index, override] of invalidStates.entries()) {
      const storage = new MemoryStorage()
      const { room } = gameRoom(storage)
      const request = { gameId: `competition-invalid-start-state-${index}`, binding: binding() }
      await createReleasedCompetitionGame(room, request)
      await prepareAllMembers(storage)
      const before = structuredClone([...storage.values.entries()])
      rulesEngineStartStateOverride = override

      await expect(room.startCompetitionGame(request))
        .rejects.toMatchObject({ code: 'invalidCompetitionBinding' })

      expect([...storage.values.entries()]).toEqual(before)
      expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
      expect(await storage.get('gameRecord')).toBeUndefined()
      expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toEqual([])
      expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toEqual([])
      expect(storage.alarm).toBeNull()
    }
  } finally {
    rulesEngineStartStateOverride = null
    rulesEngineMode = 'normal'
  }
})

test('競賽 start 固定 setup、首位、turn order 與 locked decks，且重試不換 game instance', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-game-2', binding: binding() }
  await createReleasedCompetitionGame(room, request)
  await prepareAllMembers(storage)

  const started = await room.startCompetitionGame(request)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  expect(started.status).toBe('Active')
  expect(started.idempotent).toBe(false)
  expect(record?.gameInstanceId).toBe(started.gameInstanceId)
  expect(record?.firstPlayer).toBe('player-a')
  expect(record?.setup).toEqual({
    ruleVersion: '5.17',
    players: [
      { id: 'player-a', team: 'team-a' },
      { id: 'player-b', team: 'team-b' },
    ],
    turnOrder: ['player-a', 'player-b'],
    enabledRuleModules: ['personal-deck'],
    deckLists: [
      { player: 'player-a', name: 'A locked deck', cards: ['a-1', 'a-2'] },
      { player: 'player-b', name: 'B locked deck', cards: ['b-1', 'b-2'] },
    ],
    initialHp: [
      { team: 'team-a', hp: 20 },
      { team: 'team-b', hp: 20 },
    ],
  })

  const retry = await room.startCompetitionGame(request)
  expect(retry).toEqual({ ...started, idempotent: true })
  expect((await storage.get<Record<string, unknown>>('gameRecord'))?.gameInstanceId)
    .toBe(started.gameInstanceId)

  const response = await room.fetch(new Request('https://game-room.internal/'))
  const publicBody = await response.json() as Record<string, unknown>
  expect(response.status).toBe(403)
  expect(publicBody).toEqual({
    error: '競賽進行中僅限固定成員查詢房間狀態。',
    code: 'competitionMemberRequired',
  })
  expect(JSON.stringify(publicBody)).not.toContain('A locked deck')
  expect(JSON.stringify(publicBody)).not.toContain('a-1')
})

test('最後一位固定成員 ready 後自動開始，並沿用可信 start 的 setup 與 clock 首筆', async () => {
  rulesEngineStartCalls.length = 0
  const storage = new MemoryStorage()
  const { room, pendingWaits } = gameRoom(storage)
  const request = { gameId: 'competition-auto-ready', binding: binding() }
  await createReleasedCompetitionGame(room, request)
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  await storage.put('metadata', {
    ...metadata,
    members: (metadata?.members as Array<Record<string, unknown>>).map(member => ({
      ...member,
      connected: true,
    })),
  })

  const firstReady = await post(room, {
    type: 'toggleReady',
    actorUserId: 'user-a',
    deckList: { name: 'A locked deck', cards: ['a-1', 'a-2'] },
  })
  expect(firstReady.status).toBe(200)
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')

  const lastReady = await post(room, {
    type: 'toggleReady',
    actorUserId: 'user-b',
    deckList: { name: 'B locked deck', cards: ['b-1', 'b-2'] },
  })
  expect(lastReady.status).toBe(200)
  expect(storage.alarm).not.toBeNull()
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')

  await drainWaitUntil(pendingWaits)
  const startedMetadata = await storage.get<Record<string, unknown>>('metadata')
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const queue = await storage.get<{ entries: InitialFifoEntry[] }>('competitionDeliveryQueue')
  const startEvents = storedRoomEvents(storage).filter(event => event.type === 'GameStarted')
  expect(startedMetadata?.status).toBe('Active')
  expect(record?.firstPlayer).toBe('player-a')
  expect(record?.deckSeed).toBe(request.binding.deckSeed)
  expect((record?.setup as Record<string, unknown>).deckLists).toEqual(request.binding.deckSnapshot)
  expect(startEvents).toHaveLength(1)
  expect(queue?.entries.filter(entry => (
    entry.kind === 'competitionInitialGameStarted'
  ))).toHaveLength(1)
  expect(rulesEngineStartCalls).toHaveLength(1)

  const trustedStorage = new MemoryStorage()
  const { room: trustedRoom } = gameRoom(trustedStorage)
  await createReleasedCompetitionGame(trustedRoom, request)
  await prepareAllMembers(trustedStorage)
  await trustedRoom.startCompetitionGame(request)
  expect(rulesEngineStartCalls[1]).toEqual(rulesEngineStartCalls[0])
  expect((await trustedStorage.get<Record<string, unknown>>('gameRecord'))?.setup).toEqual(record?.setup)
  const trustedEntries = (await trustedStorage.get<{ entries: InitialFifoEntry[] }>('competitionDeliveryQueue'))?.entries
  expect(trustedEntries?.[0]?.evidence.initialWindow).toEqual(queue?.entries[0]?.evidence.initialWindow)
})

test('ready transaction 已提交但低延遲工作未執行時，重啟後的 immediate alarm 會開局', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-auto-alarm', binding: binding() }
  await createReleasedCompetitionGame(room, request)
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  await storage.put('metadata', {
    ...metadata,
    members: (metadata?.members as Array<Record<string, unknown>>).map(member => ({
      ...member,
      connected: true,
    })),
  })
  for (const [actorUserId, deckList] of [
    ['user-a', { name: 'A locked deck', cards: ['a-1', 'a-2'] }],
    ['user-b', { name: 'B locked deck', cards: ['b-1', 'b-2'] }],
  ] as const) {
    const response = await post(room, { type: 'toggleReady', actorUserId, deckList })
    expect(response.status).toBe(200)
  }
  expect(storage.alarm).not.toBeNull()
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')

  const restarted = gameRoom(storage).room
  await restarted.alarm()
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Active')
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
})

test('start 引擎或原子交易暫時失敗後 alarm 可重試，最後只提交一次 start 與 clock entry', async () => {
  rulesEngineStartFailureCount = 1
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-auto-retry', binding: binding() }
  await createReleasedCompetitionGame(room, request)
  await prepareAllMembers(storage)
  await storage.setAlarm(Date.now())

  await room.alarm()
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(await storage.get('gameRecord')).toBeUndefined()
  expect(storage.alarm).not.toBeNull()

  storage.failNextBatch = true
  await room.alarm()
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(await storage.get('gameRecord')).toBeUndefined()
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(0)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toEqual([])
  expect(storage.alarm).not.toBeNull()

  await room.alarm()
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  const queue = await storage.get<{ entries: InitialFifoEntry[] }>('competitionDeliveryQueue')
  expect(metadata?.status).toBe('Active')
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
  expect(queue?.entries).toHaveLength(1)
  expect(queue?.entries[0]?.evidence.initialWindow)
    .toEqual({ type: 'action', team: 'team-a', turn: 1 })
})

test('最後 ready 的 metadata、event 與 alarm 在同一交易失敗時完整回滾', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-ready-atomic', binding: binding() }
  await createReleasedCompetitionGame(room, request)
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  await storage.put('metadata', {
    ...metadata,
    members: (metadata?.members as Array<Record<string, unknown>>).map(member => ({
      ...member,
      connected: true,
    })),
  })
  await post(room, {
    type: 'toggleReady', actorUserId: 'user-a',
    deckList: { name: 'A locked deck', cards: ['a-1', 'a-2'] },
  })
  const eventsBefore = storedRoomEvents(storage)
  const metadataBefore = await storage.get('metadata')

  storage.failNextBatch = true
  const failedBatch = await post(room, {
    type: 'toggleReady', actorUserId: 'user-b',
    deckList: { name: 'B locked deck', cards: ['b-1', 'b-2'] },
  })
  expect(failedBatch.status).toBe(500)
  expect(await storage.get('metadata')).toEqual(metadataBefore)
  expect(storedRoomEvents(storage)).toEqual(eventsBefore)
  expect(storage.alarm).toBeNull()

  storage.failNextAlarm = true
  const failedAlarm = await post(room, {
    type: 'toggleReady', actorUserId: 'user-b',
    deckList: { name: 'B locked deck', cards: ['b-1', 'b-2'] },
  })
  expect(failedAlarm.status).toBe(500)
  expect(await storage.get('metadata')).toEqual(metadataBefore)
  expect(storedRoomEvents(storage)).toEqual(eventsBefore)
  expect(storage.alarm).toBeNull()

  const retry = await post(room, {
    type: 'toggleReady', actorUserId: 'user-b',
    deckList: { name: 'B locked deck', cards: ['b-1', 'b-2'] },
  })
  expect(retry.status).toBe(200)
  expect(storage.alarm).not.toBeNull()
  expect(storedRoomEvents(storage).filter(event => event.type === 'PlayerReady')).toHaveLength(2)
})

test('等待房斷線原子取消準備；重連須重新準備，開局後斷線維持對局與時計', async () => {
  rulesEngineMode = 'normal'
  rulesEngineStartFailureCount = 0
  rulesEngineStartCalls.length = 0
  let releaseStart!: () => void
  let signalStartEntered!: () => void
  const startEntered = new Promise<void>(resolve => {
    signalStartEntered = resolve
  })
  rulesEngineStartGate = new Promise<void>(resolve => {
    releaseStart = resolve
  })
  rulesEngineStartGateEntered = signalStartEntered
  const storage = new MemoryStorage()
  try {
    const created = gameRoom(storage)
    const fourPlayerBinding = binding({
      players: [
        { userId: 'user-a', player: 'player-a', team: 'team-a' },
        { userId: 'user-b', player: 'player-b', team: 'team-b' },
        { userId: 'user-c', player: 'player-c', team: 'team-a' },
        { userId: 'user-d', player: 'player-d', team: 'team-b' },
      ],
      deckSnapshot: [
        { player: 'player-a', name: 'A locked deck', cards: ['a-1', 'a-2'] },
        { player: 'player-b', name: 'B locked deck', cards: ['b-1', 'b-2'] },
        { player: 'player-c', name: 'C locked deck', cards: ['c-1', 'c-2'] },
        { player: 'player-d', name: 'D locked deck', cards: ['d-1', 'd-2'] },
      ],
      turnOrder: ['player-a', 'player-b', 'player-c', 'player-d'],
    })
    const request = { gameId: 'competition-auto-reconnect', binding: fourPlayerBinding }
    await createReleasedCompetitionGame(created.room, request)
    const socketA = await connectCompetitionMember(created.room, created.sockets, 'user-a')
    await connectCompetitionMember(created.room, created.sockets, 'user-b')
    await connectCompetitionMember(created.room, created.sockets, 'user-c')
    await connectCompetitionMember(created.room, created.sockets, 'user-d')
    for (const player of fourPlayerBinding.players) {
      expect(await created.room.setCompetitionReady({ actorUserId: player.userId, ready: true }))
        .toEqual({ ok: true, ready: true })
    }
    await startEntered
    expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
    expect(storage.alarm).not.toBeNull()

    socketA.readyState = 3
    await created.room.webSocketClose(socketA)
    const disconnected = await storage.get<GameRoomMetadata>('metadata')
    expect(disconnected?.status).toBe('Waiting')
    expect(disconnected?.members.find(member => member.userId === 'user-a'))
      .toMatchObject({ connected: false, ready: false })
    expect(disconnected?.members.filter(member => member.userId !== 'user-a')
      .every(member => member.connected && member.ready)).toBe(true)
    const unreadyEvents = storedRoomEvents(storage).filter(event => event.type === 'PlayerUnready')
    expect(unreadyEvents).toHaveLength(1)
    expect(unreadyEvents[0]).toMatchObject({ actor: 'player-a', payload: { player: 'player-a' } })

    releaseStart()
    await drainWaitUntil(created.pendingWaits)
    expect(storage.alarm).toBeNull()
    await created.room.alarm()
    expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
    expect(await storage.get('gameRecord')).toBeUndefined()

    rulesEngineStartGate = null
    rulesEngineStartGateEntered = null
    const reconnected = await connectCompetitionMember(created.room, created.sockets, 'user-a')
    const connectedMetadata = await storage.get<GameRoomMetadata>('metadata')
    expect(connectedMetadata?.members.find(member => member.userId === 'user-a'))
      .toMatchObject({ connected: true, ready: false })
    const roomState = await post(created.room, { type: 'getState', actorUserId: 'user-a' })
    expect(roomState.status).toBe(200)
    const roomStateBody = await roomState.json() as { metadata?: GameRoomMetadata }
    expect(roomStateBody.metadata?.members.find(member => member.userId === 'user-a'))
      .toMatchObject({ connected: true, ready: false })
    expect(storage.alarm).toBeNull()
    await drainWaitUntil(created.pendingWaits)
    expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
    expect(await storage.get('gameRecord')).toBeUndefined()

    expect(await created.room.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
      .toEqual({ ok: true, ready: true })
    await drainWaitUntil(created.pendingWaits)
    expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Active')

    const activeRecord = await storage.get('gameRecord')
    const activeQueue = await storage.get('competitionDeliveryQueue')
    const activeAdmission = await storage.get<Record<string, unknown>>('competitionGameAdmission')
    const activeAlarm = storage.alarm
    expect(activeAlarm).not.toBeNull()
    reconnected.readyState = 3
    await created.room.webSocketClose(reconnected)
    const afterStartedDisconnect = await storage.get<GameRoomMetadata>('metadata')
    expect(afterStartedDisconnect?.status).toBe('Active')
    expect(afterStartedDisconnect?.members.find(member => member.userId === 'user-a'))
      .toMatchObject({ connected: false, ready: true })
    expect(await storage.get('gameRecord')).toEqual(activeRecord)
    expect(await storage.get('competitionDeliveryQueue')).toEqual(activeQueue)
    expect(await storage.get('competitionGameAdmission')).toMatchObject({ status: 'started' })
    expect(activeAdmission).toMatchObject({ status: 'started' })
    expect(storage.alarm).toBe(activeAlarm)
    expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
  } finally {
    releaseStart()
    rulesEngineStartGate = null
    rulesEngineStartGateEntered = null
  }
})

test('不完整或衝突的固定成員 binding 不自動開局，普通房 ready 仍不會自動開局', async () => {
  const competitionStorage = new MemoryStorage()
  const competition = gameRoom(competitionStorage).room
  const request = { gameId: 'competition-conflicting-members', binding: binding() }
  await createReleasedCompetitionGame(competition, request)
  await prepareAllMembers(competitionStorage)
  const metadata = await competitionStorage.get<Record<string, unknown>>('metadata')
  await competitionStorage.put('metadata', {
    ...metadata,
    members: (metadata?.members as Array<Record<string, unknown>>).map(member => (
      member.userId === 'user-b' ? { ...member, userId: 'user-a' } : member
    )),
  })
  await competitionStorage.setAlarm(Date.now())
  await competition.alarm()
  expect((await competitionStorage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(competitionStorage.alarm).toBeNull()
  expect(storedRoomEvents(competitionStorage).filter(event => event.type === 'GameStarted')).toHaveLength(0)

  const regularStorage = new MemoryStorage()
  const regular = gameRoom(regularStorage).room
  await post(regular, {
    type: 'createGame', gameId: 'ordinary-room-auto-start-regression', actorUserId: 'owner',
    actorName: 'Owner', capacity: 2,
    invitation: { roomCode: 'ABCD234', inviteToken: 'secret' },
  })
  const regularMetadata = await regularStorage.get<Record<string, unknown>>('metadata')
  await regularStorage.put('metadata', {
    ...regularMetadata,
    members: [
      { userId: 'owner', player: 'player-1', owner: true, ready: false, connected: true, displayName: 'Owner' },
      { userId: 'guest', player: 'player-2', owner: false, ready: false, connected: true, displayName: 'Guest' },
    ],
  })
  const regularReady = await post(regular, {
    type: 'toggleReady', actorUserId: 'guest',
    deckList: { name: 'Guest deck', cards: [] },
  })
  expect(regularReady.status).toBe(200)
  expect((await regularStorage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(regularStorage.alarm).toBeNull()
})

test('alarm、重複 alarm 與 trusted start RPC 競爭只建立一份 game instance 與 clock entry', async () => {
  rulesEngineStartCalls.length = 0
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-auto-start-race', binding: binding() }
  await createReleasedCompetitionGame(room, request)
  await prepareAllMembers(storage)
  await storage.setAlarm(Date.now())

  await Promise.all([
    room.alarm(),
    room.startCompetitionGame(request),
    room.alarm(),
  ])
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect(metadata?.status).toBe('Active')
  expect((await storage.get<Record<string, unknown>>('gameRecord'))?.gameInstanceId)
    .toBe(metadata?.gameInstanceId)
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
  expect((queue?.entries as Array<Record<string, unknown>>).filter(entry => (
    entry.kind === 'competitionInitialGameStarted'
  ))).toHaveLength(1)
  expect(rulesEngineStartCalls.length).toBeGreaterThan(0)
  expect(rulesEngineStartCalls.every(call => JSON.stringify(call.setup) === JSON.stringify(
    (rulesEngineStartCalls[0]?.setup),
  ))).toBe(true)
})

test('multi-game auto-start alarm 與 server RPC 重試只保留一組兩筆初始 FIFO', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = {
    gameId: 'competition-multi-game-start-race',
    binding: binding({ gameNumber: 3, expectedMatchRevision: 17 }),
  }
  await createReleasedCompetitionGame(room, request)
  await prepareAllMembers(storage)
  await storage.setAlarm(Date.now())

  await Promise.all([
    room.startCompetitionGame(request),
    room.alarm(),
    room.alarm(),
    room.startCompetitionGame(request),
  ])
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  const entries = queue?.entries as Array<Record<string, unknown>>
  expect(metadata?.status).toBe('Active')
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
  expect(entries.map(entry => entry.operation)).toEqual([
    { type: 'clockNewGame', matchId: request.binding.matchId, time: 'reset' },
    { type: 'clockBeginAction', matchId: request.binding.matchId, team: 'team-a', turn: 1 },
  ])
  expect(new Set(entries.map(entry => entry.commandId)).size).toBe(2)
  expect(queue?.revisionCursor).toBe(request.binding.expectedMatchRevision)
})

test('競賽 ready 只接受 exact locked deck，且一般 member/setup mutation 全部拒絕', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'competition-game-3', binding: binding() }
  await createReleasedCompetitionGame(room, request)
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  await storage.put('metadata', {
    ...metadata,
    members: (metadata?.members as Array<Record<string, unknown>>).map(member => (
      member.userId === 'user-a' ? { ...member, connected: true } : member
    )),
  })

  await expect(room.startCompetitionGame(request)).rejects.toThrow(
    'all fixed competition members must be connected and ready',
  )

  const mismatchedReady = await post(room, {
    type: 'toggleReady',
    actorUserId: 'user-a',
    deckList: { name: 'tampered', cards: ['a-1', 'a-2'] },
  })
  expect(mismatchedReady.status).toBe(409)

  const ready = await post(room, {
    type: 'toggleReady',
    actorUserId: 'user-a',
    deckList: { name: 'A locked deck', cards: ['a-1', 'a-2'] },
  })
  expect(ready.status).toBe(200)
  expect(storage.values.has('lockedDeck:user-a')).toBe(false)
  await expect(room.startCompetitionGame(request)).rejects.toThrow(
    'all fixed competition members must be connected and ready',
  )

  for (const body of [
    { type: 'startGame', actorUserId: 'user-a', deckList: { name: 'A locked deck', cards: [] } },
    { type: 'updateRuleModules', actorUserId: 'user-a', enabledRuleModules: [] },
    { type: 'removePlayer', actorUserId: 'user-a', targetUserId: 'user-b' },
    { type: 'leaveGame', actorUserId: 'user-a' },
    { type: 'dissolveGame', actorUserId: 'user-a' },
    { type: 'resetGame', actorUserId: 'user-a' },
    { type: 'seedDevelopmentScenario', actorUserId: 'user-a', scenario: { name: 'star-endgame' } },
    { type: 'getCompletedReplayDraft', actorUserId: 'user-a' },
  ]) {
    const response = await post(room, body)
    expect(response.status).toBe(403)
    expect(await response.json()).toMatchObject({
      error: '競賽房間由賽事伺服器控制。',
      code: 'competitionServerOnly',
    })
  }
})

test('未啟用個人牌組時 ready 忽略 HTTP 牌組且不落盤，start 使用預組牌契約', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = {
    gameId: 'competition-game-without-personal-deck',
    binding: binding({ enabledRuleModules: [], deckSnapshot: [] }),
  }
  await createReleasedCompetitionGame(room, request)
  const metadata = await storage.get<Record<string, unknown>>('metadata')
  await storage.put('metadata', {
    ...metadata,
    members: (metadata?.members as Array<Record<string, unknown>>).map(member => ({
      ...member,
      connected: true,
    })),
  })

  for (const userId of ['user-a', 'user-b']) {
    const ready = await post(room, {
      type: 'toggleReady',
      actorUserId: userId,
      deckList: { name: 'HTTP 必填但競賽房忽略', cards: ['不得持久化'] },
    })
    expect(ready.status).toBe(200)
    expect(storage.values.has(`lockedDeck:${userId}`)).toBe(false)
  }

  const started = await room.startCompetitionGame(request)
  expect(started.status).toBe('Active')
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  expect((record?.setup as Record<string, unknown>).deckLists).toEqual([])
  expect(JSON.stringify([...storage.values.entries()])).not.toContain('不得持久化')
})

test('固定牌組準備 RPC 支援 1v1 與 2v2，開局沿用 binding snapshots', async () => {
  rulesEngineMode = 'normal'
  rulesEngineStartFailureCount = 0
  rulesEngineStartGate = null
  rulesEngineStartGateEntered = null
  rulesEngineStartCalls.length = 0

  const fourPlayerBinding = binding({
    players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a' },
      { userId: 'user-b', player: 'player-b', team: 'team-b' },
      { userId: 'user-c', player: 'player-c', team: 'team-a' },
      { userId: 'user-d', player: 'player-d', team: 'team-b' },
    ],
    deckSnapshot: [
      { player: 'player-a', name: 'Locked A', cards: ['a-1'] },
      { player: 'player-b', name: 'Locked B', cards: ['b-1'] },
      { player: 'player-c', name: 'Locked C', cards: ['c-1'] },
      { player: 'player-d', name: 'Locked D', cards: ['d-1'] },
    ],
    turnOrder: ['player-a', 'player-b', 'player-c', 'player-d'],
  })
  const scenarios = [
    { gameId: 'ready-rpc-1v1', binding: binding() },
    { gameId: 'ready-rpc-2v2', binding: fourPlayerBinding },
  ]

  for (const request of scenarios) {
    const storage = new MemoryStorage()
    const created = gameRoom(storage)
    await createReleasedCompetitionGame(created.room, request)
    await connectAllCompetitionMembers(storage)

    for (const player of request.binding.players) {
      const result = await created.room.setCompetitionReady({
        actorUserId: player.userId,
        ready: true,
      })
      expect(result).toEqual({ ok: true, ready: true })
      expect(JSON.stringify(result)).not.toContain('binding')
      expect(JSON.stringify(result)).not.toContain(player.userId)
      expect(JSON.stringify(result)).not.toContain('Locked')
    }

    await drainWaitUntil(created.pendingWaits)
    const metadata = await storage.get<Record<string, unknown>>('metadata')
    const record = await storage.get<Record<string, unknown>>('gameRecord')
    expect(metadata?.status).toBe('Active')
    expect((record?.setup as Record<string, unknown>).deckLists)
      .toEqual(request.binding.deckSnapshot)
    expect(record?.firstPlayer).toBe(request.binding.firstPlayer)
    expect(storage.values.has('lockedDeck:user-a')).toBe(false)
    expect([...storage.values.keys()].some(key => key.startsWith('lockedDeck:'))).toBe(false)
    expect(storedRoomEvents(storage).filter(event => event.type === 'PlayerReady'))
      .toHaveLength(request.binding.players.length)
    expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
  }

  expect(rulesEngineStartCalls).toHaveLength(2)
  expect(rulesEngineStartCalls.map(call => (call.setup as Record<string, unknown>).deckLists))
    .toEqual(scenarios.map(scenario => scenario.binding.deckSnapshot))
})

test('不啟用個人牌組的 ready RPC 保持預組牌組輸入不變', async () => {
  rulesEngineMode = 'normal'
  const storage = new MemoryStorage()
  const created = gameRoom(storage)
  const request = {
    gameId: 'ready-rpc-preconstructed',
    binding: binding({ enabledRuleModules: [], deckSnapshot: [] }),
  }
  await createReleasedCompetitionGame(created.room, request)
  await connectAllCompetitionMembers(storage)

  for (const player of request.binding.players) {
    expect(await created.room.setCompetitionReady({
      actorUserId: player.userId,
      ready: true,
    })).toEqual({ ok: true, ready: true })
  }
  await drainWaitUntil(created.pendingWaits)

  const record = await storage.get<Record<string, unknown>>('gameRecord')
  expect((record?.setup as Record<string, unknown>).enabledRuleModules).toEqual([])
  expect((record?.setup as Record<string, unknown>).deckLists).toEqual([])
  expect([...storage.values.keys()].some(key => key.startsWith('lockedDeck:'))).toBe(false)
})

test('ready RPC 對普通房、額外 payload、陌生成員、observer、斷線成員與損壞狀態不寫入', async () => {
  const ordinaryStorage = new MemoryStorage()
  const ordinaryRoom = gameRoom(ordinaryStorage).room
  await post(ordinaryRoom, {
    type: 'createGame', gameId: 'ordinary-ready-rpc', actorUserId: 'owner', actorName: 'Owner',
    invitation: { roomCode: 'ABCD234', inviteToken: 'secret' },
  })
  const ordinaryBefore = structuredClone(ordinaryStorage.values)
  expect(await ordinaryRoom.setCompetitionReady({ actorUserId: 'owner', ready: true }))
    .toEqual({ ok: false, code: 'notCompetitionRoom' })
  expect(ordinaryStorage.values).toEqual(ordinaryBefore)

  const malformedStorage = new MemoryStorage()
  const malformedRoom = gameRoom(malformedStorage).room
  await createReleasedCompetitionGame(malformedRoom, { gameId: 'ready-rpc-malformed', binding: binding() })
  for (const payload of [
    {},
    { actorUserId: 'user-a' },
    { actorUserId: 'user-a', ready: true, deckList: { name: 'live', cards: ['live-card'] } },
  ]) {
    const before = structuredClone(malformedStorage.values)
    expect(await malformedRoom.setCompetitionReady(payload as never))
      .toEqual({ ok: false, code: 'invalidRequest' })
    expect(malformedStorage.values).toEqual(before)
    expect(malformedStorage.alarm).toBeNull()
  }

  const outsiderStorage = new MemoryStorage()
  const outsiderRoom = gameRoom(outsiderStorage).room
  await createReleasedCompetitionGame(outsiderRoom, { gameId: 'ready-rpc-outsider', binding: binding() })
  const outsiderBefore = structuredClone(outsiderStorage.values)
  expect(await outsiderRoom.setCompetitionReady({ actorUserId: 'outsider', ready: true }))
    .toEqual({ ok: false, code: 'fixedMemberRequired' })
  expect(outsiderStorage.values).toEqual(outsiderBefore)

  const observerMetadata = await outsiderStorage.get<Record<string, unknown>>('metadata')
  await outsiderStorage.put('metadata', {
    ...observerMetadata,
    observers: [{ userId: 'observer', displayName: 'Observer', connected: true }],
  })
  const observerBefore = structuredClone(outsiderStorage.values)
  expect(await outsiderRoom.setCompetitionReady({ actorUserId: 'observer', ready: true }))
    .toEqual({ ok: false, code: 'invalidCompetitionRoom' })
  expect(outsiderStorage.values).toEqual(observerBefore)

  const malformedBindingStorage = new MemoryStorage()
  const malformedBindingRoom = gameRoom(malformedBindingStorage).room
  await createReleasedCompetitionGame(malformedBindingRoom, {
    gameId: 'ready-rpc-malformed-binding',
    binding: binding(),
  })
  await malformedBindingStorage.put('competitionBinding', {
    ...binding(),
    expectedMatchRevision: 0,
  })
  const malformedBindingBefore = structuredClone(malformedBindingStorage.values)
  expect(await malformedBindingRoom.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: false, code: 'invalidCompetitionRoom' })
  expect(malformedBindingStorage.values).toEqual(malformedBindingBefore)
  expect(malformedBindingStorage.alarm).toBeNull()

  const disconnectedStorage = new MemoryStorage()
  const disconnectedRoom = gameRoom(disconnectedStorage).room
  await createReleasedCompetitionGame(disconnectedRoom, { gameId: 'ready-rpc-disconnected', binding: binding() })
  const disconnectedBefore = structuredClone(disconnectedStorage.values)
  expect(await disconnectedRoom.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: false, code: 'playerNotConnected' })
  expect(disconnectedStorage.values).toEqual(disconnectedBefore)
  expect(disconnectedStorage.alarm).toBeNull()

  const mismatchStorage = new MemoryStorage()
  const mismatchRoom = gameRoom(mismatchStorage).room
  await createReleasedCompetitionGame(mismatchRoom, { gameId: 'ready-rpc-mismatch', binding: binding() })
  const mismatchMetadata = await mismatchStorage.get<Record<string, unknown>>('metadata')
  const mismatchMembers = mismatchMetadata?.members as Array<Record<string, unknown>>
  await mismatchStorage.put('metadata', {
    ...mismatchMetadata,
    members: mismatchMembers.map(member => member.userId === 'user-a'
      ? { ...member, player: 'player-b' }
      : member),
  })
  const mismatchBefore = structuredClone(mismatchStorage.values)
  expect(await mismatchRoom.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: false, code: 'invalidCompetitionRoom' })
  expect(mismatchStorage.values).toEqual(mismatchBefore)

  const missingDeckStorage = new MemoryStorage()
  const missingDeckRoom = gameRoom(missingDeckStorage).room
  await createReleasedCompetitionGame(missingDeckRoom, { gameId: 'ready-rpc-missing-deck', binding: binding() })
  await missingDeckStorage.put('competitionBinding', binding({
    deckSnapshot: [{ player: 'player-b', name: 'B locked deck', cards: ['b-1', 'b-2'] }],
  }))
  const missingDeckBefore = structuredClone(missingDeckStorage.values)
  expect(await missingDeckRoom.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: false, code: 'lockedDeckMissing' })
  expect(missingDeckStorage.values).toEqual(missingDeckBefore)
  expect(missingDeckStorage.alarm).toBeNull()

  const dissolvedStorage = new MemoryStorage()
  const dissolvedRoom = gameRoom(dissolvedStorage).room
  await createReleasedCompetitionGame(dissolvedRoom, { gameId: 'ready-rpc-dissolved', binding: binding() })
  const waitingMetadata = await dissolvedStorage.get<Record<string, unknown>>('metadata')
  await dissolvedStorage.put('metadata', { ...waitingMetadata, status: 'Dissolved' })
  const dissolvedBefore = structuredClone(dissolvedStorage.values)
  expect(await dissolvedRoom.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: false, code: 'roomNotWaiting' })
  expect(dissolvedStorage.values).toEqual(dissolvedBefore)
  expect(dissolvedStorage.alarm).toBeNull()
})

test('ready true/false 重試冪等；真實 unready 阻止 autostart，已開局重試不翻轉狀態', async () => {
  rulesEngineMode = 'normal'
  rulesEngineStartFailureCount = 0
  rulesEngineStartCalls.length = 0
  let releaseStart!: () => void
  let signalStartEntered!: () => void
  const startEntered = new Promise<void>(resolve => {
    signalStartEntered = resolve
  })
  rulesEngineStartGate = new Promise<void>(resolve => {
    releaseStart = resolve
  })
  rulesEngineStartGateEntered = signalStartEntered

  let pendingWaits: Promise<unknown>[] = []
  try {
  const storage = new MemoryStorage()
  const created = gameRoom(storage)
  pendingWaits = created.pendingWaits
  const request = { gameId: 'ready-rpc-idempotent', binding: binding() }
  await createReleasedCompetitionGame(created.room, request)
  await connectAllCompetitionMembers(storage)

  expect(await created.room.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: true, ready: true })
  expect(await created.room.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: true, ready: true })
  expect(storedRoomEvents(storage).filter(event => event.type === 'PlayerReady')).toHaveLength(1)

  expect(await created.room.setCompetitionReady({ actorUserId: 'user-b', ready: true }))
    .toEqual({ ok: true, ready: true })
  await startEntered
  expect(storage.alarm).not.toBeNull()

  expect(await created.room.setCompetitionReady({ actorUserId: 'user-b', ready: false }))
    .toEqual({ ok: true, ready: false })
  expect(await created.room.setCompetitionReady({ actorUserId: 'user-b', ready: false }))
    .toEqual({ ok: true, ready: false })
  expect(storedRoomEvents(storage).filter(event => event.type === 'PlayerReady')).toHaveLength(2)
  expect(storedRoomEvents(storage).filter(event => event.type === 'PlayerUnready')).toHaveLength(1)
  expect(storage.alarm).toBeNull()

  releaseStart()
  await drainWaitUntil(created.pendingWaits)
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(await storage.get('gameRecord')).toBeUndefined()
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(0)

  rulesEngineStartGate = null
  rulesEngineStartGateEntered = null
  expect(await created.room.setCompetitionReady({ actorUserId: 'user-b', ready: true }))
    .toEqual({ ok: true, ready: true })
  await drainWaitUntil(created.pendingWaits)
  const activeMetadata = await storage.get<Record<string, unknown>>('metadata')
  const activeRecord = await storage.get('gameRecord')
  const activeAlarm = storage.alarm
  expect(activeMetadata?.status).toBe('Active')
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)

  expect(await created.room.setCompetitionReady({ actorUserId: 'user-b', ready: true }))
    .toEqual({ ok: false, code: 'gameAlreadyStarted' })
  expect(await created.room.setCompetitionReady({ actorUserId: 'user-b', ready: false }))
    .toEqual({ ok: false, code: 'gameAlreadyStarted' })
  expect(await storage.get('metadata')).toEqual(activeMetadata)
  expect(await storage.get('gameRecord')).toEqual(activeRecord)
  expect(storage.alarm).toBe(activeAlarm)
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)

  await storage.put('metadata', { ...activeMetadata, status: 'Finished' })
  const finishedMetadata = await storage.get('metadata')
  const finishedRecord = await storage.get('gameRecord')
  expect(await created.room.setCompetitionReady({ actorUserId: 'user-b', ready: false }))
    .toEqual({ ok: false, code: 'gameFinished' })
  expect(await storage.get('metadata')).toEqual(finishedMetadata)
  expect(await storage.get('gameRecord')).toEqual(finishedRecord)
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
  } finally {
    releaseStart()
    await drainWaitUntil(pendingWaits).catch(() => undefined)
    rulesEngineStartGate = null
    rulesEngineStartGateEntered = null
  }
})

test('ready RPC transaction rollback, alarm restart 與 clock delivery 保持同一 autostart 管線', async () => {
  rulesEngineMode = 'normal'
  rulesEngineStartFailureCount = 0
  rulesEngineStartCalls.length = 0
  const delivered: CompetitionRoomInitialGameStartedRequest[] = []
  const namespace = {
    idFromName: () => 'competition-ready-rpc-object' as unknown as DurableObjectId,
    get: () => ({
      ingestInitialGameStarted: async (request: CompetitionRoomInitialGameStartedRequest) => {
        delivered.push(structuredClone(request))
        return initialFifoAck(request)
      },
      ingestGameTerminal: async () => { throw new Error('not used') },
    }),
  }
  const storage = new MemoryStorage()
  const created = gameRoom(storage, namespace)
  const request = { gameId: 'ready-rpc-restart', binding: binding() }
  await createReleasedCompetitionGame(created.room, request)
  await connectAllCompetitionMembers(storage)
  await created.room.setCompetitionReady({ actorUserId: 'user-a', ready: true })
  const metadataBefore = await storage.get('metadata')
  const eventsBefore = storedRoomEvents(storage)

  storage.failNextBatch = true
  await expect(created.room.setCompetitionReady({ actorUserId: 'user-b', ready: true }))
    .rejects.toThrow('simulated atomic storage failure')
  expect(await storage.get('metadata')).toEqual(metadataBefore)
  expect(storedRoomEvents(storage)).toEqual(eventsBefore)
  expect(storage.alarm).toBeNull()

  storage.failNextAlarm = true
  await expect(created.room.setCompetitionReady({ actorUserId: 'user-b', ready: true }))
    .rejects.toThrow('simulated alarm transaction failure')
  expect(await storage.get('metadata')).toEqual(metadataBefore)
  expect(storedRoomEvents(storage)).toEqual(eventsBefore)
  expect(storage.alarm).toBeNull()

  rulesEngineStartFailureCount = 1
  expect(await created.room.setCompetitionReady({ actorUserId: 'user-b', ready: true }))
    .toEqual({ ok: true, ready: true })
  await drainWaitUntil(created.pendingWaits)
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Waiting')
  expect(storage.alarm).not.toBeNull()
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(0)

  const restarted = gameRoom(storage, namespace).room
  await restarted.alarm()
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Active')
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries)
    .toEqual([expect.objectContaining({
      kind: 'competitionInitialGameStarted',
      evidence: expect.objectContaining({ initialWindow: { type: 'action', team: 'team-a', turn: 1 } }),
    })])
  expect(storage.alarm).not.toBeNull()

  await gameRoom(storage, namespace).room.alarm()
  expect(delivered).toHaveLength(1)
  expect(delivered[0]?.evidence.initialWindow).toEqual({ type: 'action', team: 'team-a', turn: 1 })
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toEqual([])
  expect(storage.alarm).toBeNull()
  expect(storedRoomEvents(storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
})

test('競賽 outsider 不可 join、observer 或補位；fixed member 斷線重連保留席位與 game record', async () => {
  const storage = new MemoryStorage()
  const { room, sockets } = gameRoom(storage)
  const request = { gameId: 'competition-game-4', binding: binding() }
  await createReleasedCompetitionGame(room, request)

  const anonymousGet = await room.fetch(new Request('https://game-room.internal/'))
  expect(anonymousGet.status).toBe(403)
  expect(await anonymousGet.json()).toEqual({
    error: '競賽進行中僅限固定成員查詢房間狀態。',
    code: 'competitionMemberRequired',
  })

  const outsider = await post(room, {
    type: 'joinGame',
    actorUserId: 'outsider',
    actorName: 'Outsider',
  })
  expect(outsider.status).toBe(403)
  expect(await outsider.json()).toEqual({ error: '競賽房間僅允許已固定的成員加入。' })
  expect((await storage.get<Record<string, unknown>>('metadata'))?.observers).toEqual([])

  const outsiderCreate = await post(room, {
    type: 'createGame',
    gameId: 'competition-game-4',
    actorUserId: 'outsider',
    actorName: 'Outsider',
    invitation: { roomCode: 'ABCD234', inviteToken: 'secret' },
  })
  expect(outsiderCreate.status).toBe(403)
  expect(await outsiderCreate.json()).toMatchObject({
    error: '競賽房間由賽事伺服器控制。',
    code: 'competitionServerOnly',
  })

  await prepareAllMembers(storage)
  await room.startCompetitionGame(request)
  const beforeRecord = await storage.get<Record<string, unknown>>('gameRecord')
  const beforeMetadata = await storage.get<Record<string, unknown>>('metadata')
  const memberBefore = (beforeMetadata?.members as Array<Record<string, unknown>>)
    .find(member => member.userId === 'user-a')

  const originalPair = (globalThis as Record<string, unknown>).WebSocketPair
  ;(globalThis as Record<string, unknown>).WebSocketPair = class {
    client = new FakeSocket()
    server = new FakeSocket()
  }
  try {
    const connected = await (room as unknown as {
      connect(request: Request): Promise<Response>
    }).connect(new Request('https://game-room.internal/socket', {
      headers: {
        Upgrade: 'websocket',
        'x-fewfc-user-id': 'user-a',
        'x-fewfc-user-name': encodeURIComponent('改名攻擊'),
      },
    }))
    expect(connected.status).toBe(101)
    expect(sockets).toHaveLength(1)
    sockets[0]!.readyState = 3
    await room.webSocketClose(sockets[0]!)

    const disconnectedMetadata = await storage.get<Record<string, unknown>>('metadata')
    const disconnectedMember = (disconnectedMetadata?.members as Array<Record<string, unknown>>)
      .find(member => member.userId === 'user-a')
    expect(disconnectedMember).toMatchObject({
      userId: 'user-a',
      player: 'player-a',
      connected: false,
    })

    const reconnected = await (room as unknown as {
      connect(request: Request): Promise<Response>
    }).connect(new Request('https://game-room.internal/socket', {
      headers: {
        Upgrade: 'websocket',
        'x-fewfc-user-id': 'user-a',
        'x-fewfc-user-name': encodeURIComponent('另一個名字'),
      },
    }))
    expect(reconnected.status).toBe(101)
    const afterMetadata = await storage.get<Record<string, unknown>>('metadata')
    const memberAfter = (afterMetadata?.members as Array<Record<string, unknown>>)
      .find(member => member.userId === 'user-a')
    expect(memberAfter).toEqual({ ...memberBefore, connected: true })
    expect(afterMetadata?.gameInstanceId).toBe(beforeMetadata?.gameInstanceId)
    expect(await storage.get('gameRecord')).toEqual(beforeRecord)
  } finally {
    ;(globalThis as Record<string, unknown>).WebSocketPair = originalPair
  }
})

function terminalSuccess(request: Record<string, unknown>) {
  return {
    ok: true,
    value: {
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: (request.expectedMatchRevision as number) + 1,
      commandId: request.commandId,
      idempotent: false,
      timeoutEvidence: null,
      nextClockDeadlineMs: null,
      matchProjection: null,
      clockProjection: null,
    },
  }
}

async function startTerminalRoom(
  storage: MemoryStorage,
  competitionRoom?: Parameters<typeof gameRoom>[1],
) {
  const created = gameRoom(storage, withInitialStartReceiver(competitionRoom))
  const request = { gameId: 'competition-terminal-game', binding: binding() }
  await createReleasedCompetitionGame(created.room, request)
  await prepareAllMembers(storage)
  await created.room.startCompetitionGame(request)
  // 原 clock／terminal 元件案例從真正 initial ACK 後開始，不手寫 pop 或回填 cursor。
  await created.room.alarm()
  expect(await storage.get('competitionDeliveryQueue')).toMatchObject({
    revisionCursor: request.binding.expectedMatchRevision + 1, entries: [],
  })
  return { ...created, request }
}

function clockControlRequest(
  binding: CompetitionGameBinding,
  overrides: Partial<CompetitionClockControlRequest> = {},
): CompetitionClockControlRequest {
  return {
    controlId: 'clock-control-1',
    competitionId: binding.competitionId,
    matchId: binding.matchId,
    gameId: 'competition-terminal-game',
    gameNumber: binding.gameNumber,
    sourceCommandId: 'referee-pause-1',
    matchRevision: binding.expectedMatchRevision + 2,
    operation: 'clockPause',
    trustedTimeMs: Date.now(),
    nextClockDeadlineMs: null,
    ...overrides,
  }
}

/** 只提供元件邊界的首局正式 ACK；後續 RPC 仍由每個原案例的 fault control 決定。 */
function withInitialStartReceiver(namespace?: Parameters<typeof gameRoom>[1]): NonNullable<Parameters<typeof gameRoom>[1]> {
  return {
    idFromName: name => namespace ? namespace.idFromName(name) : { name, toString: () => name, equals: () => false },
    get(id) {
      const target = namespace?.get(id)
      return {
        async ingestInitialGameStarted(request: CompetitionRoomInitialGameStartedRequest) {
          expect(isCompetitionRoomInitialGameStartedRequest(request)).toBe(true)
          return initialFifoAck(request)
        },
        async executeMatch(request: unknown) {
          if (!target?.executeMatch) throw new Error('clock fixture receiver unavailable')
          return await Reflect.apply(target.executeMatch, target, [request])
        },
        async ingestGameTerminal(request: unknown) {
          if (!target) throw new Error('terminal fixture receiver unavailable')
          return await Reflect.apply(target.ingestGameTerminal, target, [request])
        },
      }
    },
  }
}

test('referee pause and resume atomically advance cursor, freeze mutations and restore the authoritative alarm', async () => {
  const storage = new MemoryStorage()
  const { room, request } = await startTerminalRoom(storage)
  const initialRecord = await storage.get<Record<string, unknown>>('gameRecord')
  const replayableCommand = {
    type: 'submitCommand',
    commandId: 'completed-before-pause',
    gameInstanceId: (initialRecord as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'completed-before-pause-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  }
  expect((await post(room, replayableCommand)).status).toBe(200)
  const initialQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  const oldDeadline = Date.now() + 30_000
  await storage.put('competitionDeliveryQueue', {
    ...initialQueue,
    nextClockDeadlineMs: oldDeadline,
  })
  await storage.setAlarm(oldDeadline)

  const pausedRequest = clockControlRequest(request.binding, { trustedTimeMs: Date.now() })
  const paused = await room.synchronizeCompetitionClock(pausedRequest)
  expect(paused).toMatchObject({ ok: true, value: { matchRevision: 3, idempotent: false } })
  expect(await storage.get('competitionDeliveryQueue')).toMatchObject({
    revisionCursor: 3,
    nextClockDeadlineMs: null,
    refereePaused: true,
  })
  expect(storage.alarm).toBeNull()
  const restarted = gameRoom(storage).room
  expect(await restarted.synchronizeCompetitionClock(pausedRequest))
    .toMatchObject({ ok: true, value: { matchRevision: 3, idempotent: true } })

  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const frozen = await post(room, {
    type: 'submitCommand', commandId: 'paused-player-mutation',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    actorUserId: 'user-a', action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(frozen.status).toBe(409)
  expect(await storage.get('gameRecord')).toEqual(record)
  expect((await post(room, replayableCommand)).status).toBe(200)
  expect(await storage.get('gameRecord')).toEqual(record)

  const resumeTime = Date.now()
  const resumeDeadline = resumeTime + 20_000
  const resumed = await room.synchronizeCompetitionClock(clockControlRequest(request.binding, {
    controlId: 'clock-control-2',
    sourceCommandId: 'referee-resume-1',
    matchRevision: 4,
    operation: 'clockResume',
    trustedTimeMs: resumeTime,
    nextClockDeadlineMs: resumeDeadline,
  }))
  expect(resumed).toMatchObject({ ok: true, value: { matchRevision: 4, idempotent: false } })
  expect(await storage.get('competitionDeliveryQueue')).toMatchObject({
    revisionCursor: 4,
    nextClockDeadlineMs: resumeDeadline,
    refereePaused: false,
  })
  expect(storage.alarm).toBe(resumeDeadline)

  const metadata = await storage.get<Record<string, unknown>>('metadata')
  await storage.put('metadata', { ...metadata, status: 'Finished' })
  const retry = await room.synchronizeCompetitionClock(pausedRequest)
  expect(retry).toMatchObject({ ok: true, value: { matchRevision: 3, idempotent: true } })
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.revisionCursor).toBe(4)
  await expect(room.synchronizeCompetitionClock(clockControlRequest(request.binding, {
    controlId: 'new-control-after-finish',
    sourceCommandId: 'new-control-after-finish',
    matchRevision: 5,
    operation: 'clockResume',
    nextClockDeadlineMs: Date.now() + 10_000,
  }))).rejects.toThrow('no active game record')
})

test('a Waiting competition GameRoom rejects a new clock control', async () => {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const request = { gameId: 'waiting-clock-control-game', binding: binding() }
  await createReleasedCompetitionGame(room, request)

  await expect(room.synchronizeCompetitionClock(clockControlRequest(request.binding, {
    gameId: request.gameId,
  }))).rejects.toThrow('no active game record')
  expect(await storage.get('gameRecord')).toBeUndefined()
  expect(await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))
    .toMatchObject({ revisionCursor: request.binding.expectedMatchRevision, refereePaused: false })
})

test('clock control rejects conflicts, wrong bindings, revision gaps and rolls back transaction failures', async () => {
  const storage = new MemoryStorage()
  const { room, request } = await startTerminalRoom(storage)
  const control = clockControlRequest(request.binding)
  storage.failNextBatch = true
  await expect(room.synchronizeCompetitionClock(control)).rejects.toThrow('simulated atomic storage failure')
  expect(await storage.get('competitionDeliveryQueue')).toMatchObject({
    revisionCursor: request.binding.expectedMatchRevision + 1,
    refereePaused: false,
    clockControlReceipts: [],
  })
  expect(storage.alarm).toBeNull()

  const retried = await room.synchronizeCompetitionClock(control)
  expect(retried).toMatchObject({ ok: true, value: { matchRevision: 3, idempotent: false } })
  expect(await room.synchronizeCompetitionClock(control))
    .toMatchObject({ ok: true, value: { matchRevision: 3, idempotent: true } })
  await expect(room.synchronizeCompetitionClock({
    ...control,
    trustedTimeMs: control.trustedTimeMs + 1,
  })).rejects.toThrow('identity conflict')
  await expect(room.synchronizeCompetitionClock({ ...control, gameId: 'wrong-game' }))
    .rejects.toThrow('binding mismatch')
  await expect(room.synchronizeCompetitionClock({
    ...control,
    controlId: 'clock-control-gap',
    sourceCommandId: 'referee-pause-gap',
    matchRevision: 5,
  }))
    .rejects.toThrow('revision gap')
  expect(await storage.get('competitionDeliveryQueue')).toMatchObject({ refereePaused: true })

  const invalidDeadlineRoom = await startTerminalRoom(new MemoryStorage())
  await expect(invalidDeadlineRoom.room.synchronizeCompetitionClock({
    ...control,
    controlId: 'pause-with-deadline',
    sourceCommandId: 'invalid-pause',
    nextClockDeadlineMs: control.trustedTimeMs + 1,
  })).rejects.toThrow('deadline is invalid')
  await expect(invalidDeadlineRoom.room.synchronizeCompetitionClock({
    ...control,
    controlId: 'resume-with-stale-deadline',
    sourceCommandId: 'invalid-resume',
    operation: 'clockResume',
    nextClockDeadlineMs: control.trustedTimeMs - 1,
  })).rejects.toThrow('deadline is invalid')
  await expect(invalidDeadlineRoom.room.synchronizeCompetitionClock({
    ...control,
    nextClockDeadlineMs: -1,
  })).rejects.toThrow('invalid competition clock control request')
})

test('control delivery fails closed with a normal pending entry and preserves its exact payload', async () => {
  const storage = new MemoryStorage()
  const { room, request } = await startTerminalRoom(storage)
  const before = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  const pending = {
    schemaVersion: 1,
    kind: 'competitionClockMutation',
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    commandId: 'unacked-action-clock',
    trustedTimeMs: 10,
    operation: { type: 'clockBeginAction', matchId: request.binding.matchId, team: 'team-a', turn: 1 },
    attempts: 0,
    nextAttemptAt: Date.now() + 10_000,
  }
  await storage.put('competitionDeliveryQueue', { ...before, entries: [pending] })
  const result = clockControlRequest(request.binding)
  await expect(room.synchronizeCompetitionClock(result)).rejects.toThrow('pending delivery')
  expect(await storage.get('competitionDeliveryQueue')).toMatchObject({
    revisionCursor: request.binding.expectedMatchRevision + 1,
    entries: [pending],
    refereePaused: true,
  })
  expect(storage.alarm).toBe((pending as { nextAttemptAt: number }).nextAttemptAt)
})

test('pause removes only the exact same-deadline clockObserve created in its delivery gap', async () => {
  const storage = new MemoryStorage()
  const { room, request } = await startTerminalRoom(storage)
  const deadline = Date.now() - 10
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  await storage.put('competitionDeliveryQueue', { ...queue, nextClockDeadlineMs: deadline })
  await storage.setAlarm(deadline)
  await room.alarm()
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries)
    .toHaveLength(1)
  const paused = await room.synchronizeCompetitionClock(clockControlRequest(request.binding))
  expect(paused).toMatchObject({ ok: true, value: { matchRevision: 3 } })
  expect(await storage.get('competitionDeliveryQueue')).toMatchObject({
    revisionCursor: 3,
    entries: [],
    nextClockDeadlineMs: null,
    refereePaused: true,
  })
  expect(storage.alarm).toBeNull()
})

test('terminal identity is collision-free for colon-containing tuples and stable on retry', async () => {
  const left = await competitionTerminalIdentity('competition', 'match:1', 2, 'instance')
  const right = await competitionTerminalIdentity('competition:match', '1', 2, 'instance')
  const retry = await competitionTerminalIdentity('competition', 'match:1', 2, 'instance')

  expect(['competition', 'match:1', 2, 'instance'].join(':'))
    .toBe(['competition:match', '1', 2, 'instance'].join(':'))
  expect(left).not.toBe(right)
  expect(retry).toBe(left)
})

test('canonical Finished commits game record, metadata, receipt, event, terminal outbox and alarm atomically', async () => {
  rulesEngineMode = 'finished'
  const storage = new MemoryStorage()
  const before = new Map(storage.values)
  const { room, request } = await startTerminalRoom(storage)
  const recordBefore = await storage.get<Record<string, unknown>>('gameRecord')
  const response = await post(room, {
    type: 'submitCommand',
    commandId: 'player-terminal-command',
    gameInstanceId: (recordBefore as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'player-terminal-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(response.status).toBe(200)

  const record = await storage.get<Record<string, unknown>>('gameRecord')
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Finished')
  expect(record?.sequence).toBe((recordBefore as { sequence: number }).sequence + 1)
  expect([...storage.values.keys()].some(key => key.startsWith('event:'))).toBe(true)
  const outbox = await storage.get<Record<string, unknown>>(
    'competitionDeliveryQueue',
  )
  expect(outbox).toMatchObject({ schemaVersion: 1, revisionCursor: request.binding.expectedMatchRevision + 1 })
  expect(outbox?.entries).toEqual(expect.arrayContaining([expect.objectContaining({
    kind: 'competitionGameTerminal',
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    firstPlayer: 'team-a',
    terminalEvidence: expect.objectContaining({ type: 'canonicalGameEnded' }),
  })]))
  expect(storage.alarm).not.toBeNull()

  const publicBody = await response.json() as Record<string, unknown>
  const publicStoredPayload = JSON.stringify({
    metadata: await storage.get('metadata'),
    events: [...storage.values.entries()]
      .filter(([key]) => key.startsWith('event:'))
      .map(([, value]) => value),
  })
  for (const payload of [JSON.stringify(publicBody), publicStoredPayload]) {
    expect(payload).not.toContain('organizer-1')
    expect(payload).not.toContain('expectedMatchRevision')
    expect(payload).not.toContain('competitionTerminalOutbox')
    expect(payload).not.toContain('competition-terminal:')
  }

  // 防止測試 helper 的初始快照變成隱含寫入證據，並確認 terminal 不是從未提交狀態產生。
  expect(before.size).toBe(0)
  rulesEngineMode = 'normal'
})

test('non-terminal canonical checkpoints do not create a terminal outbox or alarm', async () => {
  rulesEngineMode = 'normal'
  const storage = new MemoryStorage()
  const { room } = await startTerminalRoom(storage)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const response = await post(room, {
    type: 'submitCommand',
    commandId: 'non-terminal-command',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'non-terminal-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(response.status).toBe(200)
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Active')
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
  expect(storage.alarm).toBeNull()
})

test('Finished without a canonical conclusion fails closed and leaves no terminal checkpoint', async () => {
  rulesEngineMode = 'finishedWithoutConclusion'
  const storage = new MemoryStorage()
  const { room } = await startTerminalRoom(storage)
  const beforeRecord = await storage.get('gameRecord')
  const beforeMetadata = await storage.get('metadata')
  const response = await post(room, {
    type: 'submitCommand',
    commandId: 'missing-conclusion-command',
    gameInstanceId: (beforeRecord as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'missing-conclusion-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(response.status).toBe(500)
  expect(await storage.get('gameRecord')).toEqual(beforeRecord)
  expect(await storage.get('metadata')).toEqual(beforeMetadata)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
  expect([...storage.values.keys()].filter(key => key.startsWith('commandReceipt:'))).toHaveLength(0)
  rulesEngineMode = 'normal'
})

test('Finished requires the final canonical record event to match the public conclusion', async () => {
  for (const mode of [
    'finishedMismatchedRecord',
    'finishedGameEndedFollowedByEvent',
    'finishedWithoutGameEnded',
  ] as const) {
    rulesEngineMode = mode
    const storage = new MemoryStorage()
    const { room } = await startTerminalRoom(storage)
    const beforeRecord = await storage.get('gameRecord')
    const beforeMetadata = await storage.get('metadata')
    const beforeQueue = await storage.get('competitionDeliveryQueue')
    const beforeEvents = [...storage.values.entries()]
      .filter(([key]) => key.startsWith('event:'))
    const record = beforeRecord as { gameInstanceId: string }
    const response = await post(room, {
      type: 'submitCommand',
      commandId: `noncanonical-record-${mode}`,
      gameInstanceId: record.gameInstanceId,
      transactionId: `noncanonical-record-transaction-${mode}`,
      actorUserId: 'user-a',
      action: { type: 'passAction', reason: 'NoCardsInHand' },
    })
    expect(response.status, mode).toBe(500)
    expect(await storage.get('gameRecord'), mode).toEqual(beforeRecord)
    expect(await storage.get('metadata'), mode).toEqual(beforeMetadata)
    expect(await storage.get('competitionDeliveryQueue'), mode).toEqual(beforeQueue)
    expect([...storage.values.keys()].filter(key => key.startsWith('commandReceipt:')), mode)
      .toHaveLength(0)
    expect([...storage.values.entries()].filter(([key]) => key.startsWith('event:')), mode)
      .toEqual(beforeEvents)
    expect(storage.alarm, mode).toBeNull()
  }
  rulesEngineMode = 'normal'
})

test('trusted randomness that reaches Finished creates the same atomic terminal outbox', async () => {
  rulesEngineMode = 'randomFinished'
  const storage = new MemoryStorage()
  const { room } = await startTerminalRoom(storage)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const response = await post(room, {
    type: 'submitCommand',
    commandId: 'random-terminal-command',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'random-terminal-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(response.status).toBe(200)
  expect((await storage.get<Record<string, unknown>>('metadata'))?.status).toBe('Finished')
  const nextRecord = await storage.get<Record<string, unknown>>('gameRecord')
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries)
    .toEqual(expect.arrayContaining([expect.objectContaining({ kind: 'competitionGameTerminal' })]))
  expect([...storage.values.keys()].filter(key => key.startsWith('event:'))).toHaveLength(4)
  rulesEngineMode = 'normal'
})

test('restart retry of awaiting randomness is frozen by timeout while completed delivery remains idempotent', async () => {
  rulesEngineMode = 'normal'
  randomResolutionCalls = 0
  const storage = new MemoryStorage()
  let loseTerminalResponse = true
  const evidence = {
    type: 'timeout',
    timeoutId: 'timeout-after-response-loss',
    expiries: [{ team: 'team-a', deadlineMonotonicMs: 500 }],
    observedAtMonotonicMs: 600,
    clockCheckpointDigest: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
  }
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => clockSuccess(request as Record<string, unknown>, evidence),
      ingestGameTerminal: async (request: unknown) => {
        if (loseTerminalResponse) {
          loseTerminalResponse = false
          throw new Error('simulated timeout terminal response loss')
        }
        return terminalSuccess(request as Record<string, unknown>)
      },
    }),
  }
  const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
  const initialRecord = await storage.get<Record<string, unknown>>('gameRecord')
  const command = {
    type: 'submitCommand',
    commandId: 'response-lost-random-command',
    gameInstanceId: (initialRecord as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'response-lost-random-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  }

  rulesEngineMode = 'randomInterrupted'
  const lostResponse = await post(room, command)
  await drainWaitUntil(pendingWaits)
  expect(lostResponse.status).toBe(500)
  const acceptedReceipt = await storage.get<Record<string, unknown>>(
    `commandReceipt:${command.gameInstanceId}:${command.commandId}`,
  )
  expect(acceptedReceipt).toMatchObject({ outcome: 'accepted', transactionStatus: 'awaitingRandomness' })
  expect(randomResolutionCalls).toBe(1)
  const acceptedRecord = await storage.get<Record<string, unknown>>('gameRecord')
  const recordSequence = acceptedRecord?.sequence

  await room.enqueueCompetitionClockMutation({
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    commandId: 'timeout-after-random-response-loss',
    trustedTimeMs: 100,
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
  })
  await drainWaitUntil(pendingWaits)
  const pendingQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect(pendingQueue?.pendingTimeout).toEqual(expect.objectContaining({ timeoutEvidence: evidence }))
  expect(pendingQueue?.entries).toHaveLength(1)

  const restartedPendingRoom = gameRoom(storage, namespace).room
  const pendingRetry = await post(restartedPendingRoom, command)
  expect(pendingRetry.status).toBe(409)
  expect(await pendingRetry.json()).toMatchObject({
    error: '本場競賽已因時間結束，正在提交競賽判定。',
    code: 'commandValidation',
  })
  expect(randomResolutionCalls).toBe(1)
  expect((await storage.get<Record<string, unknown>>('gameRecord'))?.sequence).toBe(recordSequence)
  expect(await storage.get<Record<string, unknown>>('competitionDeliveryQueue')).toEqual(pendingQueue)

  const terminalQueue = pendingQueue as Record<string, unknown>
  const terminalEntries = terminalQueue.entries as Array<Record<string, unknown>>
  await storage.put('competitionDeliveryQueue', {
    ...terminalQueue,
    entries: terminalEntries.map(entry => ({ ...entry, nextAttemptAt: 0 })),
  })
  const restartedTerminalRoom = gameRoom(storage, namespace).room
  await restartedTerminalRoom.alarm()
  const terminalAckQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect(terminalAckQueue?.terminalTimeout).toEqual(evidence)
  expect(terminalAckQueue?.entries).toHaveLength(0)

  const finalRetry = await post(gameRoom(storage, namespace).room, command)
  expect(finalRetry.status).toBe(409)
  expect(await finalRetry.json()).toMatchObject({ code: 'commandValidation' })
  expect(randomResolutionCalls).toBe(1)
  expect((await storage.get<Record<string, unknown>>('gameRecord'))?.sequence).toBe(recordSequence)
  expect(await storage.get<Record<string, unknown>>('commandReceipt:'
    + `${command.gameInstanceId}:${command.commandId}`)).toEqual(acceptedReceipt)
  expect(await storage.get<Record<string, unknown>>('competitionDeliveryQueue')).toEqual(terminalAckQueue)
  rulesEngineMode = 'normal'
})

test('terminal transaction failure rolls back checkpoint, receipt, event, outbox and alarm', async () => {
  rulesEngineMode = 'finished'
  const storage = new MemoryStorage()
  const { room } = await startTerminalRoom(storage)
  const before = new Map(storage.values)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  storage.failNextBatch = true
  const response = await post(room, {
    type: 'submitCommand',
    commandId: 'atomic-terminal-command',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'atomic-terminal-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(response.status).toBe(500)
  expect(storage.values).toEqual(before)
  expect(storage.alarm).toBeNull()
  rulesEngineMode = 'normal'
})

test('terminal outbox retries the same request after RPC failure and clears itself only after strict success', async () => {
  rulesEngineMode = 'finished'
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  let remoteReceipt: ReturnType<typeof terminalSuccess> | undefined
  let loseFirstResponse = true
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      ingestGameTerminal: async (request: unknown) => {
        const typedRequest = request as Record<string, unknown>
        calls.push(structuredClone(typedRequest))
        if (remoteReceipt) {
          return {
            ...remoteReceipt,
            value: { ...remoteReceipt.value, idempotent: true },
          }
        }
        remoteReceipt = terminalSuccess(typedRequest)
        if (loseFirstResponse) {
          loseFirstResponse = false
          throw new Error('response lost after remote commit')
        }
        return remoteReceipt
      },
    }),
  }
  const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const playerCommand = {
    type: 'submitCommand',
    commandId: 'duplicate-terminal-command',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'duplicate-terminal-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  }
  await post(room, playerCommand)
  await drainWaitUntil(pendingWaits)
  const outboxKey = 'competitionDeliveryQueue'
  const firstOutbox = structuredClone(await storage.get(outboxKey))
  expect((await storage.get<Record<string, unknown>>(outboxKey))?.entries)
    .toEqual(expect.arrayContaining([expect.objectContaining({ attempts: expect.any(Number) })]))
  const retryQueue = await storage.get<Record<string, unknown>>(outboxKey)
  await storage.put(outboxKey, {
    ...retryQueue,
    entries: (retryQueue?.entries as Array<Record<string, unknown>>).map(entry => ({ ...entry, nextAttemptAt: 0 })),
  })
  await room.alarm()
  expect((await storage.get<Record<string, unknown>>(outboxKey))?.entries).toHaveLength(0)
  expect(storage.alarm).toBeNull()
  expect(calls).toHaveLength(2)
  expect({ ...calls[0], trustedTimeMs: 0 }).toEqual({ ...calls[1], trustedTimeMs: 0 })
  expect(calls[0]).toMatchObject({
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    expectedMatchRevision: request.binding.expectedMatchRevision + 1,
    firstPlayer: 'team-a',
  })
  expect(calls[0]).not.toHaveProperty('ruling')
  expect(calls[0]).not.toHaveProperty('mode')
  expect((calls[0] as { trustedTimeMs: number }).trustedTimeMs).toEqual(expect.any(Number))
  expect(firstOutbox?.entries).toEqual(expect.arrayContaining([
    expect.objectContaining({ commandId: (calls[0] as { commandId: string }).commandId }),
  ]))

  // 已完成的玩家 command retry 先取 receipt，不得因 Finished 或重送而生成新 outbox。
  const retry = await post(room, playerCommand)
  expect(retry.status).toBe(200)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
  rulesEngineMode = 'normal'
})

test('successful low-latency terminal delivery drains after serialized command release without an alarm', async () => {
  rulesEngineMode = 'finished'
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      ingestGameTerminal: async (request: unknown) => {
        calls.push(structuredClone(request as Record<string, unknown>))
        return terminalSuccess(request as Record<string, unknown>)
      },
    }),
  }
  const { room, pendingWaits } = await startTerminalRoom(storage, namespace)
  await drainWaitUntil(pendingWaits)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  await post(room, {
    type: 'submitCommand',
    commandId: 'low-latency-terminal-command',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'low-latency-terminal-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  await drainWaitUntil(pendingWaits)

  expect(calls).toHaveLength(1)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
  expect(storage.alarm).toBeNull()
  rulesEngineMode = 'normal'
})

test('a restarted GameRoom instance delivers the persisted outbox and clears its alarm', async () => {
  rulesEngineMode = 'finished'
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  let firstAttempt = true
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      ingestGameTerminal: async (request: unknown) => {
        calls.push(structuredClone(request as Record<string, unknown>))
        if (firstAttempt) {
          firstAttempt = false
          throw new Error('simulated crash before response')
        }
        return terminalSuccess(request as Record<string, unknown>)
      },
    }),
  }
  const { room, pendingWaits } = await startTerminalRoom(storage, namespace)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  await post(room, {
    type: 'submitCommand',
    commandId: 'restart-terminal-command',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'restart-terminal-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  await drainWaitUntil(pendingWaits)
  const outboxKey = 'competitionDeliveryQueue'
  expect(await storage.get(outboxKey)).toBeDefined()
  const retryQueue = await storage.get<Record<string, unknown>>(outboxKey)
  await storage.put(outboxKey, {
    ...retryQueue,
    entries: (retryQueue?.entries as Array<Record<string, unknown>>).map(entry => ({ ...entry, nextAttemptAt: 0 })),
  })

  const restarted = gameRoom(storage, namespace).room
  await restarted.alarm()

  expect((await storage.get<Record<string, unknown>>(outboxKey))?.entries).toHaveLength(0)
  expect(storage.alarm).toBeNull()
  expect(calls).toHaveLength(2)
  expect({ ...calls[0], trustedTimeMs: 0 }).toEqual({ ...calls[1], trustedTimeMs: 0 })
  rulesEngineMode = 'normal'
})

test('strict terminal ACK validation retains the outbox and reschedules every malformed acknowledgement', async () => {
  const badAcknowledgements = [
    ['wrong competition', (value: Record<string, unknown>) => ({ ...value, competitionId: 'other-competition' })],
    ['wrong match', (value: Record<string, unknown>) => ({ ...value, matchId: 'other-match' })],
    ['wrong command', (value: Record<string, unknown>) => ({ ...value, commandId: 'other-command' })],
    ['wrong revision', (value: Record<string, unknown>) => ({ ...value, matchRevision: 999 })],
    ['non-boolean idempotent', (value: Record<string, unknown>) => ({ ...value, idempotent: 'yes' })],
    ['malformed projection', (value: Record<string, unknown>) => ({ ...value, matchProjection: {} })],
    ['missing deadline', (value: Record<string, unknown>) => {
      const { nextClockDeadlineMs: _nextClockDeadlineMs, ...withoutDeadline } = value
      return withoutDeadline
    }],
    ['extra deadline field', (value: Record<string, unknown>) => ({ ...value, extraDeadline: true })],
    ['wrong deadline type', (value: Record<string, unknown>) => ({ ...value, nextClockDeadlineMs: 'later' })],
    ['negative deadline', (value: Record<string, unknown>) => ({ ...value, nextClockDeadlineMs: -1 })],
    ['stale deadline', (value: Record<string, unknown>) => ({ ...value, nextClockDeadlineMs: 99 })],
    ['unsafe deadline', (value: Record<string, unknown>) => ({
      ...value, nextClockDeadlineMs: Number.MAX_SAFE_INTEGER + 1,
    })],
    ['terminal deadline must be null', (value: Record<string, unknown>) => ({
      ...value, nextClockDeadlineMs: 0,
    })],
    ['missing timeout evidence', (value: Record<string, unknown>) => {
      const { timeoutEvidence: _timeoutEvidence, ...withoutTimeoutEvidence } = value
      return withoutTimeoutEvidence
    }],
    ['extra acknowledgement key', (value: Record<string, unknown>) => ({ ...value, extra: true })],
  ] as const

  for (const [label, mutate] of badAcknowledgements) {
    rulesEngineMode = 'finished'
    const storage = new MemoryStorage()
    const calls: Record<string, unknown>[] = []
    const namespace = {
      idFromName: (name: string) => name as unknown as DurableObjectId,
      get: () => ({
        ingestGameTerminal: async (request: unknown) => {
          const typedRequest = request as Record<string, unknown>
          calls.push(structuredClone(typedRequest))
          const success = terminalSuccess(typedRequest)
          return { ...success, value: mutate(success.value) }
        },
      }),
    }
    const { room, pendingWaits } = await startTerminalRoom(storage, namespace)
    await drainWaitUntil(pendingWaits)
    const record = await storage.get<Record<string, unknown>>('gameRecord')
    await post(room, {
      type: 'submitCommand',
      commandId: `strict-ack-${label}`,
      gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
      transactionId: `strict-ack-transaction-${label}`,
      actorUserId: 'user-a',
      action: { type: 'passAction', reason: 'NoCardsInHand' },
    })
    await drainWaitUntil(pendingWaits)
    const outboxKey = 'competitionDeliveryQueue'
    expect(await storage.get(outboxKey), label).toBeDefined()
    expect(storage.alarm, label).not.toBeNull()
    expect(calls, label).toHaveLength(1)
  }
  rulesEngineMode = 'normal'
})

test('strict clock ACK deadline validation retains the outbox head and cursor', async () => {
  const badDeadlines = [
    ['missing deadline', (value: Record<string, unknown>) => {
      const { nextClockDeadlineMs: _nextClockDeadlineMs, ...withoutDeadline } = value
      return withoutDeadline
    }],
    ['extra deadline field', (value: Record<string, unknown>) => ({ ...value, extraDeadline: true })],
    ['wrong deadline type', (value: Record<string, unknown>) => ({ ...value, nextClockDeadlineMs: 'later' })],
    ['negative deadline', (value: Record<string, unknown>) => ({ ...value, nextClockDeadlineMs: -1 })],
    ['stale deadline', (value: Record<string, unknown>) => ({ ...value, nextClockDeadlineMs: 99 })],
    ['unsafe deadline', (value: Record<string, unknown>) => ({
      ...value, nextClockDeadlineMs: Number.MAX_SAFE_INTEGER + 1,
    })],
    ['timeout evidence with a live deadline', (value: Record<string, unknown>) => ({
      ...value,
      timeoutEvidence: {
        type: 'timeout',
        timeoutId: 'timeout-with-live-deadline',
        expiries: [{ team: 'team-a', deadlineMonotonicMs: 500 }],
        observedAtMonotonicMs: 600,
        clockCheckpointDigest: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
      },
      nextClockDeadlineMs: 700,
    })],
  ] as const

  for (const [label, mutate] of badDeadlines) {
    const storage = new MemoryStorage()
    const calls: Record<string, unknown>[] = []
    const namespace = {
      idFromName: (name: string) => name as unknown as DurableObjectId,
      get: () => ({
        executeMatch: async (request: unknown) => {
          calls.push(structuredClone(request as Record<string, unknown>))
          const success = clockSuccess(request as Record<string, unknown>)
          return { ...success, value: mutate(success.value) }
        },
        ingestGameTerminal: async () => { throw new Error('not used') },
      }),
    }
    const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
    await room.enqueueCompetitionClockMutation({
      competitionId: request.binding.competitionId,
      matchId: request.binding.matchId,
      callerUserId: request.binding.organizerUserId,
      commandId: `invalid-deadline-${label}`,
      trustedTimeMs: 100,
      operation: { type: 'clockObserve', matchId: request.binding.matchId },
    })
    await drainWaitUntil(pendingWaits)
    const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
    expect(calls, label).toHaveLength(1)
    expect(queue?.revisionCursor, label).toBe(2)
    expect(queue?.entries, label).toHaveLength(1)
    expect(storage.alarm, label).not.toBeNull()
  }
})

function clockSuccess(request: Record<string, unknown>, timeoutEvidence: unknown = null) {
  return {
    ok: true,
    value: {
      competitionId: request.competitionId,
      matchId: request.matchId,
      matchRevision: (request.expectedMatchRevision as number) + 1,
      commandId: request.commandId,
      idempotent: false,
      timeoutEvidence,
      nextClockDeadlineMs: timeoutEvidence === null
        ? Math.max(request.trustedTimeMs as number, Date.now()) + 23_000
        : null,
      matchProjection: null,
      clockProjection: null,
    },
  }
}

test('one alarm schedules the earlier clock deadline over a later delivery retry', async () => {
  const storage = new MemoryStorage()
  const { room, request } = await startTerminalRoom(storage)
  const now = Date.now()
  const deadline = now + 5_000
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  await storage.put('competitionDeliveryQueue', {
    ...queue,
    nextClockDeadlineMs: deadline,
    entries: [{
      schemaVersion: 1,
      kind: 'competitionClockMutation',
      competitionId: request.binding.competitionId,
      matchId: request.binding.matchId,
      callerUserId: request.binding.organizerUserId,
      commandId: 'delayed-head',
      trustedTimeMs: now,
      operation: { type: 'clockObserve', matchId: request.binding.matchId },
      attempts: 1,
      nextAttemptAt: deadline + 10_000,
    }],
  })

  await room.alarm()
  expect(storage.alarm).toBe(deadline)
})

test('deadline observation keeps one stable identity and trusted time across retry and restart', async () => {
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  let remoteReceipt: ReturnType<typeof clockSuccess> | undefined
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        const typed = request as Record<string, unknown>
        calls.push(structuredClone(typed))
        if (!remoteReceipt) {
          remoteReceipt = clockSuccess(typed)
          throw new Error('clock deadline committed but response was lost')
        }
        return { ...remoteReceipt, value: { ...remoteReceipt.value, idempotent: true } }
      },
      ingestGameTerminal: async () => { throw new Error('not used') },
    }),
  }
  const { room, request } = await startTerminalRoom(storage, namespace)
  const deadline = Date.now() - 1
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  await storage.put('competitionDeliveryQueue', { ...queue, nextClockDeadlineMs: deadline })

  await room.alarm()
  const afterLostResponse = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  const firstEntry = (afterLostResponse?.entries as Array<Record<string, unknown>>)[0]
  expect(firstEntry).toMatchObject({
    kind: 'competitionClockMutation',
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    trustedTimeMs: deadline,
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
  })
  const firstCommandId = firstEntry?.commandId

  const restarted = gameRoom(storage, namespace).room
  const retryQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  await storage.put('competitionDeliveryQueue', {
    ...retryQueue,
    entries: (retryQueue?.entries as Array<Record<string, unknown>>)
      .map(entry => ({ ...entry, nextAttemptAt: 0 })),
  })
  await restarted.alarm()

  expect(calls).toHaveLength(2)
  expect(calls[0]?.commandId).toBe(firstCommandId)
  expect(calls[1]?.commandId).toBe(firstCommandId)
  expect(calls.map(call => call.trustedTimeMs)).toEqual([deadline, deadline])
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.nextClockDeadlineMs)
    .toBeGreaterThan(Date.now())
})

test('strict clock ACK projection rejects undefined, non-JSON and unknown operations', async () => {
  const invalidProjections = [
    ['undefined public', (projection: Record<string, unknown>) => ({ ...projection, public: undefined })],
    ['non-JSON public', (projection: Record<string, unknown>) => ({
      ...projection,
      public: { nested: undefined },
    })],
    ['unknown operation', (projection: Record<string, unknown>) => ({ ...projection, operation: 'unknown' })],
    ['empty operation', (projection: Record<string, unknown>) => ({ ...projection, operation: '' })],
  ] as const

  for (const [label, mutate] of invalidProjections) {
    const storage = new MemoryStorage()
    const calls: Record<string, unknown>[] = []
    const namespace = {
      idFromName: (name: string) => name as unknown as DurableObjectId,
      get: () => ({
        executeMatch: async (request: unknown) => {
          const typedRequest = request as Record<string, unknown>
          calls.push(structuredClone(typedRequest))
          const success = clockSuccess(typedRequest)
          const projection = mutate({ ok: true, operation: 'clockObserve', public: {} })
          return {
            ...success,
            value: { ...success.value, clockProjection: projection },
          }
        },
        ingestGameTerminal: async () => { throw new Error('not used') },
      }),
    }
    const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
    await room.enqueueCompetitionClockMutation({
      competitionId: request.binding.competitionId,
      matchId: request.binding.matchId,
      callerUserId: request.binding.organizerUserId,
      commandId: `invalid-projection-${label}`,
      trustedTimeMs: 100,
      operation: { type: 'clockObserve', matchId: request.binding.matchId },
    })
    await drainWaitUntil(pendingWaits)
    const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
    expect(calls, label).toHaveLength(1)
    expect(queue?.revisionCursor, label).toBe(2)
    expect(queue?.entries, label).toHaveLength(1)
    expect(storage.alarm, label).not.toBeNull()
  }
})

test('clock delivery uses one FIFO, advances the cursor only after strict ACK, and deduplicates after removal', async () => {
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        calls.push(structuredClone(request as Record<string, unknown>))
        return clockSuccess(request as Record<string, unknown>)
      },
      ingestGameTerminal: async () => { throw new Error('not used') },
    }),
  }
  const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
  const first = {
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    commandId: 'clock-1',
    trustedTimeMs: 100,
    operation: { type: 'clockObserve' as const, matchId: request.binding.matchId },
  }
  const second = { ...first, commandId: 'clock-2', trustedTimeMs: 200 }
  const firstQueued = await room.enqueueCompetitionClockMutation(first)
  const secondQueued = await room.enqueueCompetitionClockMutation(second)
  await drainWaitUntil(pendingWaits)
  expect(calls.map(call => call.expectedMatchRevision)).toEqual([2, 3])
  expect(calls.map(call => call.trustedTimeMs)).toEqual([100, 200])
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.revisionCursor).toBe(4)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.receipts).toHaveLength(2)

  const retry = await room.enqueueCompetitionClockMutation(first)
  expect(retry).toEqual({ commandId: firstQueued.commandId, idempotent: true, queued: true })
  expect(secondQueued.idempotent).toBe(false)
  await expect(room.enqueueCompetitionClockMutation({ ...first, trustedTimeMs: 101 })).rejects.toThrow(
    'competition clock command identity conflict',
  )
  expect(calls).toHaveLength(2)
})

test('clock enqueue keeps every completed receipt for long-lived idempotency', async () => {
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        calls.push(structuredClone(request as Record<string, unknown>))
        return clockSuccess(request as Record<string, unknown>)
      },
      ingestGameTerminal: async () => { throw new Error('not used') },
    }),
  }
  const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
  const base = {
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    trustedTimeMs: 100,
  }
  const firstRequest = {
    ...base,
    commandId: 'clock-long-lived-1',
    operation: { type: 'clockObserve' as const, matchId: request.binding.matchId },
  }
  const firstQueued = await room.enqueueCompetitionClockMutation(firstRequest)
  for (let index = 2; index <= 65; index += 1) {
    await room.enqueueCompetitionClockMutation({
      ...base,
      commandId: `clock-long-lived-${index}`,
      trustedTimeMs: 100 + index,
      operation: { type: 'clockObserve' as const, matchId: request.binding.matchId },
    })
  }
  await drainWaitUntil(pendingWaits)
  const beforeRetryCalls = calls.length
  const beforeRetryCursor = (await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.revisionCursor
  const retry = await room.enqueueCompetitionClockMutation(firstRequest)
  await drainWaitUntil(pendingWaits)
  expect(beforeRetryCalls).toBe(65)
  expect(beforeRetryCursor).toBe(67)
  expect(retry).toEqual({ commandId: firstQueued.commandId, idempotent: true, queued: true })
  expect(calls).toHaveLength(beforeRetryCalls)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.revisionCursor).toBe(beforeRetryCursor)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
  await expect(room.enqueueCompetitionClockMutation({ ...firstRequest, trustedTimeMs: 101 })).rejects.toThrow(
    'competition clock command identity conflict',
  )
})

test('a failed FIFO head blocks the following clock entry until the head ACKs', async () => {
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        const typed = request as Record<string, unknown>
        calls.push(structuredClone(typed))
        if (calls.length === 1) {
          throw new Error('temporary head failure')
        }
        return clockSuccess(typed)
      },
      ingestGameTerminal: async () => { throw new Error('not used') },
    }),
  }
  const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
  const base = {
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    trustedTimeMs: 100,
  }
  await room.enqueueCompetitionClockMutation({
    ...base,
    commandId: 'clock-head',
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
  })
  await room.enqueueCompetitionClockMutation({
    ...base,
    commandId: 'clock-tail',
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
  })
  await drainWaitUntil(pendingWaits)
  expect(calls).toHaveLength(1)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(2)

  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  await storage.put('competitionDeliveryQueue', {
    ...queue,
    entries: (queue?.entries as Array<Record<string, unknown>>).map(entry => ({ ...entry, nextAttemptAt: 0 })),
  })
  await room.alarm()
  expect(calls).toHaveLength(3)
  expect(calls[1]?.expectedMatchRevision).toBe(2)
  expect(calls[2]?.expectedMatchRevision).toBe(3)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
})

test('clock timeout evidence is persisted before head removal and blocks later entries', async () => {
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  const terminalCalls: Record<string, unknown>[] = []
  const evidence = {
    type: 'timeout',
    timeoutId: 'timeout-1',
    expiries: [{ team: 'team-a', deadlineMonotonicMs: 500 }],
    observedAtMonotonicMs: 600,
    clockCheckpointDigest: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
  }
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        calls.push(structuredClone(request as Record<string, unknown>))
        return clockSuccess(request as Record<string, unknown>, evidence)
      },
      ingestGameTerminal: async (request: unknown) => {
        terminalCalls.push(structuredClone(request as Record<string, unknown>))
        throw new Error('temporary terminal delivery failure')
      },
    }),
  }
  const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
  const base = {
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    trustedTimeMs: 100,
  }
  await room.enqueueCompetitionClockMutation({
    ...base,
    commandId: 'timeout-clock',
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
  })
  await room.enqueueCompetitionClockMutation({
    ...base,
    commandId: 'blocked-clock',
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
  })
  await drainWaitUntil(pendingWaits)
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect(calls).toHaveLength(1)
  expect(queue?.revisionCursor).toBe(3)
  expect(queue?.pendingTimeout).toEqual(expect.objectContaining({ timeoutEvidence: evidence }))
  expect(queue?.entries).toEqual(expect.arrayContaining([expect.objectContaining({
    kind: 'competitionGameTerminal',
    terminalEvidence: evidence,
  })]))
  expect((queue?.entries as Array<Record<string, unknown>>)[0]?.kind)
    .toBe('competitionGameTerminal')
  expect(terminalCalls).toHaveLength(1)
  expect(terminalCalls[0]?.terminalEvidence).toEqual(evidence)
  expect(terminalCalls[0]).not.toHaveProperty('conclusion')
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const frozen = await post(room, {
    type: 'submitCommand',
    commandId: 'must-stay-frozen',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'timeout-frozen-command',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(frozen.status).toBe(409)
  expect(await frozen.json()).toMatchObject({ code: 'commandValidation' })
  const outsider = await post(room, {
    type: 'submitCommand',
    commandId: 'unauthorized-during-timeout',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'unauthorized-timeout-command',
    actorUserId: 'outsider-user',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(outsider.status).toBe(403)
  const outsiderBody = await outsider.json()
  expect(outsiderBody).toMatchObject({ code: 'commandValidation' })
  expect(outsiderBody).not.toHaveProperty('error', '本場競賽已因時間結束，正在提交競賽判定。')
})

test('a timeout with no tail keeps its pending outcome and alarm across restart', async () => {
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  const terminalCalls: Record<string, unknown>[] = []
  const evidence = {
    type: 'timeout',
    timeoutId: 'timeout-no-tail',
    expiries: [{ team: 'team-a', deadlineMonotonicMs: 500 }],
    observedAtMonotonicMs: 600,
    clockCheckpointDigest: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
  }
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        calls.push(structuredClone(request as Record<string, unknown>))
        return clockSuccess(request as Record<string, unknown>, evidence)
      },
      ingestGameTerminal: async (request: unknown) => {
        terminalCalls.push(structuredClone(request as Record<string, unknown>))
        throw new Error('temporary terminal delivery failure')
      },
    }),
  }
  const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
  await room.enqueueCompetitionClockMutation({
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    commandId: 'timeout-no-tail-command',
    trustedTimeMs: 100,
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
  })
  await drainWaitUntil(pendingWaits)
  const beforeAlarm = structuredClone(await storage.get('competitionDeliveryQueue'))
  expect(calls).toHaveLength(1)
  expect(storage.alarm).not.toBeNull()
  expect((beforeAlarm as Record<string, unknown>).pendingTimeout).toEqual(expect.objectContaining({ timeoutEvidence: evidence }))
  const beforeEntry = ((beforeAlarm as Record<string, unknown>).entries as Array<Record<string, unknown>>)[0]
  expect(beforeEntry?.terminalEvidence).toEqual(evidence)
  expect(terminalCalls).toHaveLength(1)

  const restarted = gameRoom(storage, namespace).room
  const retryQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  await storage.put('competitionDeliveryQueue', {
    ...retryQueue,
    entries: (retryQueue?.entries as Array<Record<string, unknown>>)
      .map(entry => ({ ...entry, nextAttemptAt: 0 })),
  })
  await restarted.alarm()
  expect(calls).toHaveLength(1)
  expect(terminalCalls).toHaveLength(2)
  expect(terminalCalls[0]?.commandId).toBe(terminalCalls[1]?.commandId)
  expect(terminalCalls[0]?.trustedTimeMs).toBe(terminalCalls[1]?.trustedTimeMs)
  expect(storage.alarm).not.toBeNull()
  const afterRetry = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect(afterRetry?.pendingTimeout).toEqual((beforeAlarm as Record<string, unknown>).pendingTimeout)
  expect(afterRetry?.entries).toHaveLength(1)
})

test('timeout terminal ACK clears delivery but permanently freezes the ended game', async () => {
  const storage = new MemoryStorage()
  const clockCalls: Record<string, unknown>[] = []
  const terminalCalls: Record<string, unknown>[] = []
  const evidence = {
    type: 'timeout',
    timeoutId: 'timeout-terminal-ack',
    expiries: [{ team: 'team-a', deadlineMonotonicMs: 500 }],
    observedAtMonotonicMs: 600,
    clockCheckpointDigest: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
  }
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        clockCalls.push(structuredClone(request as Record<string, unknown>))
        return clockSuccess(request as Record<string, unknown>, evidence)
      },
      ingestGameTerminal: async (request: unknown) => {
        terminalCalls.push(structuredClone(request as Record<string, unknown>))
        return terminalSuccess(request as Record<string, unknown>)
      },
    }),
  }
  const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
  await room.enqueueCompetitionClockMutation({
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    commandId: 'timeout-terminal-command',
    trustedTimeMs: 100,
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
  })
  await drainWaitUntil(pendingWaits)

  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  expect(clockCalls).toHaveLength(1)
  expect(terminalCalls).toHaveLength(1)
  expect(terminalCalls[0]?.terminalEvidence).toEqual(evidence)
  expect(terminalCalls[0]).not.toHaveProperty('conclusion')
  expect(queue?.entries).toEqual([])
  expect(queue?.pendingTimeout).toBeNull()
  expect(queue?.terminalTimeout).toEqual(evidence)
  expect(storage.alarm).toBeNull()

  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const response = await post(room, {
    type: 'submitCommand',
    commandId: 'after-timeout-ack',
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    transactionId: 'after-timeout-transaction',
    actorUserId: 'user-a',
    action: { type: 'passAction', reason: 'NoCardsInHand' },
  })
  expect(response.status).toBe(409)
})

test('invalid clock timeout evidence retains the head and cursor', async () => {
  const invalidEvidence = [
    ['extra key', (value: Record<string, unknown>) => ({ ...value, extra: true })],
    ['duplicate team', (value: Record<string, unknown>) => ({
      ...value,
      expiries: [
        { team: 'team-a', deadlineMonotonicMs: 500 },
        { team: 'team-a', deadlineMonotonicMs: 550 },
      ],
    })],
    ['deadline after observation', (value: Record<string, unknown>) => ({
      ...value,
      expiries: [{ team: 'team-a', deadlineMonotonicMs: 601 }],
    })],
    ['non hex digest', (value: Record<string, unknown>) => ({
      ...value,
      clockCheckpointDigest: 'not-a-sha256-digest',
    })],
  ] as const

  for (const [label, mutate] of invalidEvidence) {
    const storage = new MemoryStorage()
    const calls: Record<string, unknown>[] = []
    const namespace = {
      idFromName: (name: string) => name as unknown as DurableObjectId,
      get: () => ({
        executeMatch: async (request: unknown) => {
          calls.push(structuredClone(request as Record<string, unknown>))
          const evidence = {
            type: 'timeout',
            timeoutId: 'invalid-timeout',
            expiries: [{ team: 'team-a', deadlineMonotonicMs: 500 }],
            observedAtMonotonicMs: 600,
            clockCheckpointDigest: '0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef',
          }
          return clockSuccess(request as Record<string, unknown>, mutate(evidence))
        },
        ingestGameTerminal: async () => { throw new Error('not used') },
      }),
    }
    const { room, request, pendingWaits } = await startTerminalRoom(storage, namespace)
    await room.enqueueCompetitionClockMutation({
      competitionId: request.binding.competitionId,
      matchId: request.binding.matchId,
      callerUserId: request.binding.organizerUserId,
      commandId: `invalid-timeout-${label}`,
      trustedTimeMs: 100,
      operation: { type: 'clockObserve', matchId: request.binding.matchId },
    })
    await drainWaitUntil(pendingWaits)
    const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
    expect(calls, label).toHaveLength(1)
    expect(queue?.revisionCursor, label).toBe(2)
    expect(queue?.entries, label).toHaveLength(1)
    expect(storage.alarm, label).not.toBeNull()
  }
})

test('a restarted room drains two persisted clock entries from one alarm in FIFO order', async () => {
  const storage = new MemoryStorage()
  const calls: Record<string, unknown>[] = []
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        calls.push(structuredClone(request as Record<string, unknown>))
        return clockSuccess(request as Record<string, unknown>)
      },
      ingestGameTerminal: async () => { throw new Error('not used') },
    }),
  }
  const created = gameRoom(storage, withInitialStartReceiver(namespace))
  const request = { gameId: 'competition-restart-queue', binding: binding() }
  await createReleasedCompetitionGame(created.room, request)
  await prepareAllMembers(storage)
  await created.room.startCompetitionGame(request)
  await created.room.alarm()
  const queue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  const entry = (commandId: string, trustedTimeMs: number) => ({
    schemaVersion: 1,
    kind: 'competitionClockMutation',
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    commandId,
    trustedTimeMs,
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
    attempts: 0,
    nextAttemptAt: 0,
  })
  await storage.put('competitionDeliveryQueue', {
    ...queue,
    entries: [entry('persisted-1', 100), entry('persisted-2', 200)],
  })
  const restarted = gameRoom(storage, namespace).room
  await restarted.alarm()
  expect(calls.map(call => call.commandId)).toEqual(['persisted-1', 'persisted-2'])
  expect(calls.map(call => call.expectedMatchRevision)).toEqual([2, 3])
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
})

test('a clock retry followed by terminal delivery preserves one FIFO and continuous revisions', async () => {
  const storage = new MemoryStorage()
  const clockCalls: Record<string, unknown>[] = []
  const terminalCalls: Record<string, unknown>[] = []
  let remoteClockReceipt: ReturnType<typeof clockSuccess> | undefined
  const namespace = {
    idFromName: (name: string) => name as unknown as DurableObjectId,
    get: () => ({
      executeMatch: async (request: unknown) => {
        const typedRequest = request as Record<string, unknown>
        clockCalls.push(structuredClone(typedRequest))
        if (!remoteClockReceipt) {
          remoteClockReceipt = clockSuccess(typedRequest)
          throw new Error('clock response lost after remote commit')
        }
        return {
          ...remoteClockReceipt,
          value: { ...remoteClockReceipt.value, idempotent: true },
        }
      },
      ingestGameTerminal: async (request: unknown) => {
        const typedRequest = request as Record<string, unknown>
        terminalCalls.push(structuredClone(typedRequest))
        return terminalSuccess(typedRequest)
      },
    }),
  }
  const { room, request } = await startTerminalRoom(storage, namespace)
  const record = await storage.get<Record<string, unknown>>('gameRecord')
  const conclusion = {
    outcome: { type: 'winner', team: 'team-a' },
    causes: [{ type: 'directVictory', rule: 'test', team: 'team-a' }],
  }
  const clockEntry = {
    schemaVersion: 1,
    kind: 'competitionClockMutation',
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    callerUserId: request.binding.organizerUserId,
    commandId: 'mixed-clock-command',
    trustedTimeMs: 100,
    operation: { type: 'clockObserve', matchId: request.binding.matchId },
    attempts: 0,
    nextAttemptAt: 0,
  }
  const terminalEntry = {
    schemaVersion: 1,
    kind: 'competitionGameTerminal',
    competitionId: request.binding.competitionId,
    matchId: request.binding.matchId,
    gameNumber: request.binding.gameNumber,
    gameInstanceId: (record as { gameInstanceId: string }).gameInstanceId,
    commandId: 'mixed-terminal-command',
    submissionId: 'mixed-terminal-submission',
    gameRecordId: 'mixed-terminal-game-record',
    rulingRecordId: 'mixed-terminal-ruling',
    callerUserId: request.binding.organizerUserId,
    startKind: request.binding.startKind,
    firstPlayer: 'team-a',
    trustedTimeMs: 200,
    terminalEvidence: { type: 'canonicalGameEnded', conclusion },
    attempts: 0,
    nextAttemptAt: 0,
  }
  await storage.put('competitionDeliveryQueue', {
    schemaVersion: 1,
    revisionCursor: request.binding.expectedMatchRevision + 1,
    entries: [clockEntry, terminalEntry],
    receipts: [],
    pendingTimeout: null,
  })

  await room.alarm()
  expect(clockCalls).toHaveLength(1)
  expect(terminalCalls).toHaveLength(0)
  const retryQueue = await storage.get<Record<string, unknown>>('competitionDeliveryQueue')
  await storage.put('competitionDeliveryQueue', {
    ...retryQueue,
    entries: (retryQueue?.entries as Array<Record<string, unknown>>)
      .map(entry => ({ ...entry, nextAttemptAt: 0 })),
  })
  await room.alarm()

  expect(clockCalls).toHaveLength(2)
  expect(clockCalls[0]).toEqual(clockCalls[1])
  expect(clockCalls.map(call => call.expectedMatchRevision)).toEqual([2, 2])
  expect(clockCalls.map(call => call.commandId)).toEqual(['mixed-clock-command', 'mixed-clock-command'])
  expect(clockCalls.map(call => call.trustedTimeMs)).toEqual([100, 100])
  expect(terminalCalls).toHaveLength(1)
  expect(terminalCalls[0]?.expectedMatchRevision).toBe(3)
  expect(terminalCalls[0]?.commandId).toBe('mixed-terminal-command')
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.revisionCursor).toBe(4)
  expect((await storage.get<Record<string, unknown>>('competitionDeliveryQueue'))?.entries).toHaveLength(0)
  expect(storage.alarm).toBeNull()
})

interface InitialFifoEntry extends CompetitionRoomInitialGameStartedRequest {
  schemaVersion: 1
  kind: 'competitionInitialGameStarted'
  attempts: number
  nextAttemptAt: number
}

function initialFifoRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

type InitialDeliverySettlement = { completed: true } | { completed: false; error: unknown }

function captureInitialDelivery(promise: Promise<void>): Promise<InitialDeliverySettlement> {
  return promise.then<InitialDeliverySettlement, InitialDeliverySettlement>(
    () => ({ completed: true }), error => ({ completed: false, error }),
  )
}

async function waitForInitialReceiver(entered: Promise<void>, settled: Promise<InitialDeliverySettlement>) {
  await Promise.race([entered, settled.then(result => {
    if (!result.completed) throw result.error
    throw new Error('initial delivery settled before the gated RPC entered')
  })])
}

test('round-release initial FIFO begins only after an early durable GameStarted exists', async () => {
  await withInitialFifoTime(async time => {
    let remoteCalls = 0
    const fixture = await roundReleaseFixture({ roundStartMs: 1_900_000_000_000 }, {
      idFromName: (name: string): DurableObjectId => ({ name, toString: () => name, equals: () => false }),
      get: () => ({
        async ingestInitialGameStarted() { remoteCalls += 1; throw new Error('early initial RPC') },
        async ingestGameTerminal() { remoteCalls += 1; throw new Error('early terminal RPC') },
      }),
    })
    await roundReleaseReady(fixture)
    const startedAtMs = fixture.request.binding.roundStartMs - 1
    time.now = fixture.request.binding.roundStartMs - 1
    expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
    await fixture.room.alarm()
    expect(storedRoomEvents(fixture.storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
    expect(fixture.storage.values.get('competitionGameAdmission')).toMatchObject({
      status: 'started', start: { startedAtMs },
    })
    await fixture.room.alarm()
    await drainWaitUntil(fixture.pendingWaits)
    expect(remoteCalls).toBe(1)
  })
})

function initialFifoAck(request: CompetitionRoomInitialGameStartedRequest, nextClockDeadlineMs: number | null = null) {
  return { ok: true, value: {
    competitionId: request.competitionId, matchId: request.matchId,
    matchRevision: request.expectedMatchRevision + 1, commandId: request.commandId,
    idempotent: false, timeoutEvidence: null, nextClockDeadlineMs,
    matchProjection: null, clockProjection: null,
  } }
}

async function withInitialFifoTime(run: (time: { now: number }) => Promise<void>) {
  const realNow = Date.now
  const time = { now: 1_900_000_000_000 - 600_000 }
  Date.now = () => time.now
  try { await run(time) } finally {
    Date.now = realNow
    rulesEngineMode = 'normal'
  }
}

/** 真公開 create/release/presence/desired ready/start；父端只 mock 唯一六欄 RPC 邊界。 */
async function initialFifoFixture(
  time: { now: number },
  receive: (request: CompetitionRoomInitialGameStartedRequest) => Promise<unknown> = async request => initialFifoAck(request),
  preparing = false,
) {
  rulesEngineMode = preparing ? 'clockTransition' : 'normal'
  const initialCalls: CompetitionRoomInitialGameStartedRequest[] = []
  const clockCalls: unknown[] = []
  const terminalCalls: unknown[] = []
  let namespaceDisposals = 0
  let stubDisposals = 0
  const target = {
    async ingestInitialGameStarted(request: CompetitionRoomInitialGameStartedRequest): Promise<unknown> {
      expect(this).toBe(target)
      expect(isCompetitionRoomInitialGameStartedRequest(request)).toBe(true)
      expect(Reflect.ownKeys(request).sort()).toEqual([
        'callerUserId', 'commandId', 'competitionId', 'evidence', 'expectedMatchRevision', 'matchId',
      ])
      initialCalls.push(structuredClone(request))
      return await receive(request)
    },
    async executeMatch(request: unknown) {
      clockCalls.push(structuredClone(request))
      if (!initialFifoRecord(request)) throw new Error('invalid clock fixture request')
      return terminalSuccess(request)
    },
    async ingestGameTerminal(request: unknown) {
      terminalCalls.push(structuredClone(request))
      if (!initialFifoRecord(request)) throw new Error('invalid terminal fixture request')
      return terminalSuccess(request)
    },
    [Symbol.dispose]() { stubDisposals += 1 },
  }
  const namespace = {
    idFromName: (name: string): DurableObjectId => ({ name, toString: () => name, equals: () => false }),
    get: () => target,
    [Symbol.dispose]() { namespaceDisposals += 1 },
  }
  const fixture = await roundReleaseFixture({ roundStartMs: 1_900_000_000_000,
    ...(preparing ? { enabledRuleModules: ['pouch', 'personal-deck'] } : {}),
  }, namespace)
  await roundReleaseReady(fixture)
  roundReleaseNoStart(fixture.storage)
  time.now = fixture.request.binding.roundStartMs - 17
  expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
  await fixture.room.startCompetitionGame(fixture.request)
  await drainWaitUntil(fixture.pendingWaits)
  return { ...fixture, namespace, initialCalls, clockCalls, terminalCalls,
    disposals: () => ({ namespaceDisposals, stubDisposals }) }
}

for (const preparing of [false, true]) {
  test(`round-release initial FIFO persists the actual ${preparing ? 'preparation' : 'action'} start with one immutable entry`, async () => {
    await withInitialFifoTime(async time => {
      const fixture = await initialFifoFixture(time, undefined, preparing)
      const record = await fixture.storage.get<GameRecord>('gameRecord')
      const event = storedRoomEvents(fixture.storage).find(value => value.type === 'GameStarted')
      const evidence = await fixture.storage.get<CompetitionRoomInitialGameStartedRequest['evidence']>('competitionInitialStartEvidence')
      const queue = await fixture.storage.get<{ revisionCursor: number; entries: InitialFifoEntry[] }>('competitionDeliveryQueue')
      if (!record || !evidence || !queue) throw new Error('missing actual initial commit')
      expect(queue.revisionCursor).toBe(fixture.request.binding.expectedMatchRevision)
      expect(queue.entries).toHaveLength(1)
      expect(queue.entries[0]).toMatchObject({ kind: 'competitionInitialGameStarted',
        expectedMatchRevision: fixture.request.binding.expectedMatchRevision, evidence })
      expect(evidence).toEqual({ bindingRef: fixture.release.bindingRef,
        releaseDecisionId: fixture.release.commandId, gameInstanceId: record.gameInstanceId,
        startSequence: event?.sequence, startedAtMs: time.now,
        initialWindow: preparing ? { type: 'preparation' } : { type: 'action', team: 'team-a', turn: 1 },
      })
      expect(event?.createdAt).toBe(new Date(evidence.startedAtMs).toISOString())
      expect(fixture.storage.alarm).toBe(evidence.startedAtMs)
      await fixture.room.alarm()
      expect(fixture.initialCalls).toHaveLength(1)
      expect(fixture.clockCalls).toEqual([])
      expect(fixture.terminalCalls).toEqual([])
      expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({
        revisionCursor: fixture.request.binding.expectedMatchRevision + 1, entries: [],
      })
      expect(fixture.disposals()).toEqual({ namespaceDisposals: 0, stubDisposals: 0 })
    })
  })
}

for (const fault of ['null', 'rootExtra', 'valueExtra', 'competition', 'match', 'command', 'cursor',
  'deadline', 'timeout', 'idempotent', 'symbol', 'nestedSymbol', 'getter', 'cleanupThrow',
  'rootGetter', 'hiddenValue', 'prototype', 'invalidDisposer', 'failure']) {
  test(`round-release initial FIFO retains its exact immutable head after invalid ${fault} ACK and cleans only the root result`, async () => {
    await withInitialFifoTime(async time => {
      let disposed = 0
      let getters = 0
      const fixture = await initialFifoFixture(time, async request => {
        if (fault === 'null') return null
        const result = initialFifoAck(request)
        Object.defineProperty(result, Symbol.dispose, { configurable: true, value() {
          expect(this).toBe(result)
          disposed += 1
          if (fault === 'cleanupThrow') throw new Error('private cleanup exception')
        } })
        if (fault === 'rootExtra') Reflect.set(result, 'private', 'PRIVATE_RPC_RESULT')
        if (fault === 'valueExtra') Reflect.set(result.value, 'private', 'PRIVATE_RPC_VALUE')
        if (fault === 'competition') result.value.competitionId = 'another-competition'
        if (fault === 'match') result.value.matchId = 'another-match'
        if (fault === 'command') result.value.commandId = 'another-command'
        if (fault === 'cursor') result.value.matchRevision += 1
        if (fault === 'deadline') result.value.nextClockDeadlineMs = request.evidence.startedAtMs - 1
        if (fault === 'timeout') Reflect.set(result.value, 'timeoutEvidence', 'not-an-initial-timeout')
        if (fault === 'idempotent') Reflect.set(result.value, 'idempotent', 'unknown')
        if (fault === 'symbol') Reflect.set(result, Symbol('unknown'), 'PRIVATE_SYMBOL')
        if (fault === 'nestedSymbol') Reflect.set(result.value, Symbol.dispose, () => { getters += 1 })
        if (fault === 'getter') Object.defineProperty(result.value, 'commandId', { enumerable: true,
          get() { getters += 1; throw new Error('PRIVATE_GETTER') } })
        if (fault === 'rootGetter') Object.defineProperty(result, 'ok', { enumerable: true,
          get() { getters += 1; throw new Error('PRIVATE_ROOT_GETTER') } })
        if (fault === 'hiddenValue') Object.defineProperty(result, 'value', { value: result.value, enumerable: false })
        if (fault === 'prototype') Object.setPrototypeOf(result, { private: 'PRIVATE_PROTOTYPE' })
        if (fault === 'invalidDisposer') Object.defineProperty(result, Symbol.dispose, { value: 'unknown-resource' })
        if (fault === 'failure') Reflect.set(result, 'ok', false)
        return result
      })
      const before = await fixture.storage.get<{ revisionCursor: number; entries: InitialFifoEntry[] }>('competitionDeliveryQueue')
      const record = await fixture.storage.get('gameRecord')
      const events = storedRoomEvents(fixture.storage)
      const evidence = await fixture.storage.get('competitionInitialStartEvidence')
      if (!before?.entries[0]) throw new Error('missing first initial entry')
      await fixture.room.alarm()
      const after = await fixture.storage.get<typeof before>('competitionDeliveryQueue')
      expect(after?.revisionCursor).toBe(before.revisionCursor)
      expect(after?.entries).toHaveLength(1)
      expect(after?.entries[0]).toEqual({ ...before.entries[0], attempts: 1, nextAttemptAt: time.now + 2_000 })
      expect(fixture.storage.alarm).toBe(time.now + 2_000)
      expect(await fixture.storage.get('gameRecord')).toStrictEqual(record)
      expect(storedRoomEvents(fixture.storage)).toStrictEqual(events)
      expect(await fixture.storage.get('competitionInitialStartEvidence')).toStrictEqual(evidence)
      expect(disposed).toBe(fault === 'null' || fault === 'invalidDisposer' ? 0 : 1)
      expect(getters).toBe(0)
      expect(fixture.clockCalls).toEqual([])
      expect(fixture.disposals()).toEqual({ namespaceDisposals: 0, stubDisposals: 0 })
    })
  })
}

test('round-release initial FIFO response loss and restart preserve command, cursor, sample and canonical evidence', async () => {
  await withInitialFifoTime(async time => {
    let receipt: ReturnType<typeof initialFifoAck> | undefined
    let losses = 1
    const fixture = await initialFifoFixture(time, async request => {
      receipt ??= initialFifoAck(request)
      if (losses-- > 0) throw new Error('initial response lost after parent commit')
      return { ...receipt, value: { ...receipt.value, idempotent: true } }
    })
    const originalEvidence = await fixture.storage.get('competitionInitialStartEvidence')
    const originalEvents = storedRoomEvents(fixture.storage)
    const originalRecord = await fixture.storage.get('gameRecord')
    await fixture.room.alarm()
    expect(fixture.initialCalls).toHaveLength(1)
    time.now += 2_000
    const reopened = gameRoom(fixture.storage, fixture.namespace)
    await reopened.room.alarm()
    expect(fixture.initialCalls).toHaveLength(2)
    expect(fixture.initialCalls[1]).toStrictEqual(fixture.initialCalls[0])
    expect(await fixture.storage.get('competitionInitialStartEvidence')).toStrictEqual(originalEvidence)
    expect(storedRoomEvents(fixture.storage)).toStrictEqual(originalEvents)
    expect(await fixture.storage.get('gameRecord')).toStrictEqual(originalRecord)
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({ entries: [], revisionCursor: 2 })
    await reopened.room.alarm()
    expect(fixture.initialCalls).toHaveLength(2)
  })
})

test('round-release initial FIFO pending remote is singleflight and cannot block local release or revoke ACK', async () => {
  await withInitialFifoTime(async time => {
    let enter = () => {}
    let unblock = () => {}
    const entered = new Promise<void>(resolve => { enter = resolve })
    const blocked = new Promise<void>(resolve => { unblock = resolve })
    const fixture = await initialFifoFixture(time, async request => {
      enter()
      await blocked
      return initialFifoAck(request)
    })
    const first = captureInitialDelivery(fixture.room.alarm())
    let second: Promise<InitialDeliverySettlement> | undefined
    try {
      await waitForInitialReceiver(entered, first)
      second = captureInitialDelivery(fixture.room.alarm())
      expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
      expect(await fixture.room.revokeCompetitionGame({ ...fixture.release, commandId: 'after-real-start' })).toBe('started')
      expect(fixture.initialCalls).toHaveLength(1)
      expect(storedRoomEvents(fixture.storage).filter(event => event.type === 'GameStarted')).toHaveLength(1)
    } finally {
      unblock()
      expect(await first).toEqual({ completed: true })
      if (second) expect(await second).toEqual({ completed: true })
    }
    expect(fixture.initialCalls).toHaveLength(1)
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({ entries: [], revisionCursor: 2 })
  })
})

test('round-release initial FIFO keeps later clock and terminal behind its ACK through an actual player command', async () => {
  await withInitialFifoTime(async time => {
    let enter = () => {}
    let unblock = () => {}
    const entered = new Promise<void>(resolve => { enter = resolve })
    const blocked = new Promise<void>(resolve => { unblock = resolve })
    const fixture = await initialFifoFixture(time, async request => {
      enter()
      await blocked
      return initialFifoAck(request)
    })
    const delivery = captureInitialDelivery(fixture.room.alarm())
    try {
      await waitForInitialReceiver(entered, delivery)
      const queued = await fixture.room.enqueueCompetitionClockMutation({
        competitionId: fixture.request.binding.competitionId, matchId: fixture.request.binding.matchId,
        callerUserId: fixture.request.binding.organizerUserId, commandId: 'after-initial-end-window',
        trustedTimeMs: time.now, operation: { type: 'clockEndWindow', matchId: fixture.request.binding.matchId },
      })
      expect(queued.queued).toBe(true)
      rulesEngineMode = 'finished'
      const record = await fixture.storage.get<GameRecord>('gameRecord')
      if (!record) throw new Error('missing started record')
      expect((await post(fixture.room, { type: 'submitCommand', commandId: 'after-initial-finish',
        gameInstanceId: record.gameInstanceId, transactionId: 'after-initial-finish-transaction',
        actorUserId: 'user-a', action: { type: 'passAction', reason: 'NoCardsInHand' },
      })).status).toBe(200)
      expect(fixture.clockCalls).toEqual([])
      expect(fixture.terminalCalls).toEqual([])
      const queue = await fixture.storage.get<{ entries: { kind: string }[] }>('competitionDeliveryQueue')
      expect(queue?.entries.map(entry => entry.kind)).toEqual([
        'competitionInitialGameStarted', 'competitionClockMutation', 'competitionGameTerminal',
      ])
    } finally { unblock(); expect(await delivery).toEqual({ completed: true }) }
    await drainWaitUntil(fixture.pendingWaits)
    expect(fixture.initialCalls).toHaveLength(1)
    expect(fixture.clockCalls).toHaveLength(1)
    expect(fixture.terminalCalls).toHaveLength(1)
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({ entries: [], revisionCursor: 4 })
  })
})

test('round-release initial FIFO validates local immutable evidence before RPC instead of repairing a corrupt snapshot', async () => {
  await withInitialFifoTime(async time => {
    const fixture = await initialFifoFixture(time)
    const evidence = await fixture.storage.get<CompetitionRoomInitialGameStartedRequest['evidence']>('competitionInitialStartEvidence')
    if (!evidence) throw new Error('missing actual evidence')
    await fixture.storage.put('competitionInitialStartEvidence', { ...evidence, releaseDecisionId: 'another-release' })
    const record = await fixture.storage.get('gameRecord')
    const events = storedRoomEvents(fixture.storage)
    await fixture.room.alarm()
    expect(fixture.initialCalls).toEqual([])
    expect(await fixture.storage.get('gameRecord')).toStrictEqual(record)
    expect(storedRoomEvents(fixture.storage)).toStrictEqual(events)
    expect(await fixture.storage.get('competitionInitialStartEvidence')).toEqual({ ...evidence, releaseDecisionId: 'another-release' })
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 1,
      entries: [{ kind: 'competitionInitialGameStarted', attempts: 1 }],
    })
  })
})

test('round-release initial FIFO accepts the exact nine-field ACK with root disposer and preserves the authoritative deadline', async () => {
  await withInitialFifoTime(async time => {
    let disposed = 0
    const fixture = await initialFifoFixture(time, async request => {
      const ack = initialFifoAck(request, request.evidence.startedAtMs + 1_000)
      Object.defineProperty(ack, Symbol.dispose, { value() { expect(this).toBe(ack); disposed += 1 } })
      return ack
    })
    await fixture.room.alarm()
    expect(disposed).toBe(1)
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({
      revisionCursor: 2, entries: [], nextClockDeadlineMs: time.now + 1_000,
    })
    expect(fixture.storage.alarm).toBe(time.now + 1_000)
    expect(fixture.disposals()).toEqual({ namespaceDisposals: 0, stubDisposals: 0 })
    const restarted = gameRoom(fixture.storage, fixture.namespace).room
    time.now += 1_000
    await restarted.alarm()
    expect(fixture.initialCalls).toHaveLength(1)
    expect(fixture.clockCalls).toHaveLength(1)
    expect(fixture.clockCalls[0]).toMatchObject({ expectedMatchRevision: 2,
      trustedTimeMs: time.now, operation: { type: 'clockObserve', matchId: fixture.request.binding.matchId } })
  })
})

test('round-release initial FIFO missing capability retains its entry and never falls back to generic clock execution', async () => {
  await withInitialFifoTime(async time => {
    const fixture = await initialFifoFixture(time)
    Reflect.deleteProperty(fixture.namespace.get(), 'ingestInitialGameStarted')
    await fixture.room.alarm()
    expect(fixture.initialCalls).toEqual([])
    expect(fixture.clockCalls).toEqual([])
    expect(fixture.terminalCalls).toEqual([])
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 1,
      entries: [{ kind: 'competitionInitialGameStarted', attempts: 1, nextAttemptAt: time.now + 2_000 }] })
    expect(fixture.storage.alarm).toBe(time.now + 2_000)
    expect(fixture.disposals()).toEqual({ namespaceDisposals: 0, stubDisposals: 0 })
  })
})

test('round-release initial FIFO root cleanup cannot replace the accepted semantic snapshot or inject private data', async () => {
  await withInitialFifoTime(async time => {
    let disposed = 0
    const fixture = await initialFifoFixture(time, async request => {
      const ack = initialFifoAck(request, request.evidence.startedAtMs + 1_000)
      Object.defineProperty(ack, Symbol.dispose, { value() {
        disposed += 1
        ack.value.commandId = 'after-cleanup-command'
        ack.value.nextClockDeadlineMs = request.evidence.startedAtMs - 1
        Reflect.set(ack.value, 'timeoutEvidence', { private: 'PRIVATE_CLEANUP_EVIDENCE' })
        Reflect.set(ack.value, 'private', 'PRIVATE_CLEANUP_VALUE')
      } })
      return ack
    })
    await fixture.room.alarm()
    expect(disposed).toBe(1)
    const queue = await fixture.storage.get('competitionDeliveryQueue')
    expect(queue).toMatchObject({ revisionCursor: 2, entries: [], pendingTimeout: null,
      nextClockDeadlineMs: time.now + 1_000 })
    expect(JSON.stringify(queue)).not.toContain('PRIVATE_CLEANUP')
    expect(fixture.storage.alarm).toBe(time.now + 1_000)
    expect(fixture.disposals()).toEqual({ namespaceDisposals: 0, stubDisposals: 0 })
  })
})

test('round-release initial FIFO a late ACK cannot pop a changed local head and restart retries the same semantic request', async () => {
  await withInitialFifoTime(async time => {
    let enter = () => {}
    let unblock = () => {}
    const entered = new Promise<void>(resolve => { enter = resolve })
    const blocked = new Promise<void>(resolve => { unblock = resolve })
    const fixture = await initialFifoFixture(time, async request => {
      enter()
      await blocked
      return initialFifoAck(request)
    })
    const delivery = captureInitialDelivery(fixture.room.alarm())
    try {
      await waitForInitialReceiver(entered, delivery)
      const queue = await fixture.storage.get<{ entries: InitialFifoEntry[] }>('competitionDeliveryQueue')
      if (!queue?.entries[0]) throw new Error('missing actual initial head')
      await fixture.storage.put('competitionDeliveryQueue', {
        ...queue, entries: [{ ...queue.entries[0], attempts: 7 }],
      })
    } finally { unblock(); expect(await delivery).toEqual({ completed: true }) }
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 1,
      entries: [{ kind: 'competitionInitialGameStarted', attempts: 7 }] })
    await gameRoom(fixture.storage, fixture.namespace).room.alarm()
    expect(fixture.initialCalls).toHaveLength(2)
    expect(fixture.initialCalls[1]).toStrictEqual(fixture.initialCalls[0])
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 2, entries: [] })
  })
})

test('round-release initial FIFO ACK transaction failure keeps evidence and queue together for idempotent restart', async () => {
  await withInitialFifoTime(async time => {
    const fixture = await initialFifoFixture(time)
    const before = structuredClone(fixture.storage.values)
    const alarm = fixture.storage.alarm
    fixture.storage.failNextBatch = true
    const outcome = await fixture.room.alarm().then(() => ({ error: null }), error => ({ error }))
    expect(outcome.error).toBeInstanceOf(Error)
    expect(fixture.storage.values).toStrictEqual(before)
    expect(fixture.storage.alarm).toBe(alarm)
    await gameRoom(fixture.storage, fixture.namespace).room.alarm()
    expect(fixture.initialCalls).toHaveLength(2)
    expect(fixture.initialCalls[1]).toStrictEqual(fixture.initialCalls[0])
    expect(await fixture.storage.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 2, entries: [] })
  })
})

for (const preparing of [false, true]) {
  test(`round-release final start sample follows delayed storage reads in the actual ${preparing ? 'preparation' : 'action'} start and retries the same identity`, async () => {
    await withInitialFifoTime(async time => {
      const roundStartMs = 1_900_000_000_000
      const initialCalls: CompetitionRoomInitialGameStartedRequest[] = []
      const receiver = {
        async ingestInitialGameStarted(request: CompetitionRoomInitialGameStartedRequest) {
          expect(this).toBe(receiver)
          initialCalls.push(structuredClone(request))
          if (initialCalls.length === 1) throw new Error('initial ACK lost after parent commit')
          return initialFifoAck(request)
        },
        async executeMatch() { throw new Error('initial start must not use generic clock RPC') },
        async ingestGameTerminal() { throw new Error('unexpected terminal RPC') },
      }
      const namespace = {
        idFromName: (name: string): DurableObjectId => ({ name, toString: () => name, equals: () => false }),
        get: () => receiver,
      }
      const storage = new AdvancingStartSampleStorage(time)
      const fixture = await roundReleaseFixture({ roundStartMs,
        ...(preparing ? { enabledRuleModules: ['pouch', 'personal-deck'] } : {}),
      }, namespace, storage)
      rulesEngineMode = preparing ? 'clockTransition' : 'normal'
      await roundReleaseReady(fixture)

      time.now = roundStartMs + 17
      expect(await fixture.room.releaseCompetitionGame(fixture.release)).toBe(true)
      storage.advanceNextSequenceRead = true
      storage.advanceLateTransactionList = true
      await fixture.room.startCompetitionGame(fixture.request)
      await drainWaitUntil(fixture.pendingWaits)

      // 公開 storage 邊界在 nextSequence read 與 transaction snapshot 各前進一次。
      const finalSample = roundStartMs + 317
      expect(storage.advanceNextSequenceRead).toBe(false)
      expect(storage.advanceLateTransactionList).toBe(false)
      const event = storedRoomEvents(storage).find(value => value.type === 'GameStarted')
      const metadata = await storage.get<GameRoomMetadata>('metadata')
      const admission = await storage.get<Record<string, unknown>>('competitionGameAdmission')
      const evidence = await storage.get<CompetitionRoomInitialGameStartedRequest['evidence']>(
        'competitionInitialStartEvidence',
      )
      const queue = await storage.get<{ revisionCursor: number; entries: InitialFifoEntry[] }>(
        'competitionDeliveryQueue',
      )
      const entry = queue?.entries[0]
      if (!event || !metadata || !admission || !evidence || !entry) {
        throw new Error('missing public-flow initial start snapshot')
      }
      const expectedWindow = preparing
        ? { type: 'preparation' }
        : { type: 'action', team: 'team-a', turn: 1 }
      expect(event.createdAt).toBe(new Date(finalSample).toISOString())
      expect(metadata.updatedAt).toBe(new Date(finalSample).toISOString())
      expect(admission).toMatchObject({
        status: 'started', start: { startSequence: event.sequence, startedAtMs: finalSample },
      })
      expect(evidence).toMatchObject({
        startSequence: event.sequence, startedAtMs: finalSample, initialWindow: expectedWindow,
      })
      expect(entry).toMatchObject({
        kind: 'competitionInitialGameStarted', nextAttemptAt: finalSample,
        evidence: { startedAtMs: finalSample, initialWindow: expectedWindow },
      })
      expect(entry.commandId.endsWith(`:${finalSample}`)).toBe(true)
      expect(storage.alarm).toBe(finalSample)
      expect(initialCalls).toEqual([])

      await fixture.room.alarm()
      const retryQueue = await storage.get<{ revisionCursor: number; entries: InitialFifoEntry[] }>(
        'competitionDeliveryQueue',
      )
      expect(retryQueue).toMatchObject({ revisionCursor: 1,
        entries: [{ commandId: entry.commandId, evidence: entry.evidence,
          attempts: 1, nextAttemptAt: finalSample + 2_000 }],
      })
      expect(initialCalls).toHaveLength(1)
      expect(initialCalls[0]).toMatchObject({
        commandId: entry.commandId, expectedMatchRevision: entry.expectedMatchRevision,
        evidence: entry.evidence,
      })

      time.now = finalSample + 2_000
      await fixture.room.alarm()
      expect(initialCalls).toHaveLength(2)
      expect(initialCalls[1]).toStrictEqual(initialCalls[0])
      expect(await storage.get('competitionDeliveryQueue')).toMatchObject({ revisionCursor: 2, entries: [] })
      expect(storedRoomEvents(storage).filter(value => value.type === 'GameStarted')).toHaveLength(1)
      expect((await fixture.room.startCompetitionGame(fixture.request)).idempotent).toBe(true)
      expect(initialCalls).toHaveLength(2)
    })
  })
}
