// 只由同目錄的正式測試以獨立 Bun 子行程執行，隔離 global mock.module。
import { expect, mock, test } from 'bun:test'
import type { CompetitionGameBinding, CompetitionClockOperation } from '../../shared/competition-game-room'
import { competitionGameBindingRefForRequest } from '../../shared/competition-game-room'
import type { GameRecord, GameRoomMetadata, RulesReadyResult } from '../../shared/game-room'

interface WasmExports extends WebAssembly.Exports {
  memory: WebAssembly.Memory
  fewfc_alloc(length: number): number
  fewfc_dealloc(pointer: number, length: number): void
  fewfc_handle_request(pointer: number, length: number): bigint
  fewfc_resolve_personal_deck(pointer: number, length: number): bigint
  fewfc_resolve_rule_modules(pointer: number, length: number): bigint
}

const wasmPath = Bun.env.FEWC_COMPETITION_TEST_WASM
if (!wasmPath) throw new Error('FEWC_COMPETITION_TEST_WASM must point to the formal Rust/WASM artifact')
const wasmBytes = await Bun.file(wasmPath).arrayBuffer()
const wasmModule = await WebAssembly.compile(wasmBytes)
const wasm = (await WebAssembly.instantiate(wasmModule, {})).exports as WasmExports

function callWasm<T>(
  request: unknown,
  invoke: (pointer: number, length: number) => bigint,
): T {
  const input = new TextEncoder().encode(JSON.stringify(request))
  const inputPointer = wasm.fewfc_alloc(input.length)
  new Uint8Array(wasm.memory.buffer).set(input, inputPointer)
  let packed: bigint
  try {
    packed = invoke(inputPointer, input.length)
  } finally {
    wasm.fewfc_dealloc(inputPointer, input.length)
  }
  const outputPointer = Number(packed >> 32n)
  const outputLength = Number(packed & 0xffff_ffffn)
  const output = new TextDecoder().decode(
    new Uint8Array(wasm.memory.buffer, outputPointer, outputLength),
  )
  wasm.fewfc_dealloc(outputPointer, outputLength)
  const result = JSON.parse(output) as T | { error: unknown }
  if (result && typeof result === 'object' && 'error' in result) {
    throw new Error(JSON.stringify(result.error))
  }
  return result as T
}

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
  callRulesEngine: async (request: unknown) => (
    callWasm<RulesReadyResult>(request, wasm.fewfc_handle_request)
  ),
  callRuleModuleResolution: async (candidate?: string[], ruleVersion = '5.17') => (
    callWasm<{ modules: string[] }>(
      { candidate, ruleVersion },
      wasm.fewfc_resolve_rule_modules,
    )
  ),
}))

const { GameRoom } = await import('./game-room')

class MemoryStorage {
  readonly values = new Map<string, unknown>()
  alarm: number | null = null

  get<T>(key: string): Promise<T | undefined>
  get<T>(keys: string[]): Promise<Map<string, T>>
  async get<T>(key: string | string[]): Promise<T | undefined | Map<string, T>> {
    if (typeof key === 'string') return this.values.get(key) as T | undefined
    return new Map(key.filter(entry => this.values.has(entry)).map(entry => [entry, this.values.get(entry) as T]))
  }

  async put(key: string | Record<string, unknown>, value?: unknown): Promise<void> {
    if (typeof key === 'string') {
      this.values.set(key, value)
      return
    }
    for (const [entry, entryValue] of Object.entries(key)) this.values.set(entry, entryValue)
  }

  async list<T>(): Promise<Map<string, T>> {
    return new Map([...this.values.entries()] as Array<[string, T]>)
  }

  async setAlarm(time: number | Date): Promise<void> {
    this.alarm = time instanceof Date ? time.getTime() : time
  }

  async deleteAlarm(): Promise<void> {
    this.alarm = null
  }

  async transaction<T>(operation: (storage: MemoryStorage) => Promise<T>): Promise<T> {
    const values = structuredClone([...this.values.entries()])
    const alarm = this.alarm
    try {
      return await operation(this)
    } catch (error) {
      this.values.clear()
      for (const [key, value] of values) this.values.set(key, value)
      this.alarm = alarm
      throw error
    }
  }
}

interface StartClockEntry {
  commandId: string
  trustedTimeMs: number
  operation: CompetitionClockOperation
}

function makeBinding(overrides: Partial<CompetitionGameBinding> = {}): CompetitionGameBinding {
  const players = [
    { userId: 'user-a', player: 'player-a', team: 'team-a' },
    { userId: 'user-b', player: 'player-b', team: 'team-b' },
    { userId: 'user-c', player: 'player-c', team: 'team-a' },
    { userId: 'user-d', player: 'player-d', team: 'team-b' },
  ]
  return {
    roundRef: {
      competitionId: 'start-clock-competition', stageId: 'stage-1', roundNumber: 1, roundGenerationId: 'generation-1',
    },
    roundStartMs: 1_700_000_000_000,
    competitionId: 'start-clock-competition',
    matchId: 'start-clock-match',
    gameNumber: 2,
    organizerUserId: 'organizer',
    expectedMatchRevision: 1,
    startKind: { type: 'ordinaryNewGame' },
    ruleVersion: '5.17',
    ruleset: 'fewfc-base',
    enabledRuleModules: [],
    initialHp: [{ team: 'team-a', hp: 200 }, { team: 'team-b', hp: 200 }],
    players,
    deckSnapshot: [],
    firstPlayer: 'player-a',
    turnOrder: players.map(player => player.player),
    deckSeed: 'start-clock-seed',
    ...overrides,
  }
}

function gameRoom(storage: MemoryStorage) {
  const pending: Promise<unknown>[] = []
  const sockets: FixtureSocket[] = []
  const ctx = {
    storage,
    waitUntil: (promise: Promise<unknown>) => pending.push(promise),
    getWebSockets: () => sockets,
    acceptWebSocket: (socket: FixtureSocket) => sockets.push(socket),
  } as unknown as DurableObjectState
  const env = {
    PLAYER_NOTIFICATIONS: {
      idFromName: () => 'start-clock-notifications' as unknown as DurableObjectId,
      get: () => ({ fetch: async () => new Response(null, { status: 204 }) }),
    },
  }
  return { room: new GameRoom(ctx, env as never), pending }
}

async function createAndStart(binding: CompetitionGameBinding) {
  const storage = new MemoryStorage()
  const { room, pending } = gameRoom(storage)
  const gameId = `game-${binding.matchId}`
  const request = { gameId, binding }
  const ack = await room.createCompetitionGame(request)
  const bindingRef = await competitionGameBindingRefForRequest(request)
  expect(ack).toEqual({ gameId, bindingDigest: bindingRef.bindingDigest, status: 'Waiting' })
  expect(await room.releaseCompetitionGame({ bindingRef, commandId: 'formal-start-clock-release' })).toBe(true)
  await connectAndReady(room, binding)
  await room.startCompetitionGame({ gameId, binding })
  while (pending.length > 0) await Promise.all(pending.splice(0))

  const record = await storage.get<GameRecord>('gameRecord')
  if (!record) throw new Error('started game record is missing')
  const result = callWasm<RulesReadyResult>({
    action: { type: 'refresh' },
    viewer: 'observer',
    setup: record.setup,
    deckSeed: record.deckSeed,
    record: record.rulesRecord,
  }, wasm.fewfc_handle_request)
  const queue = await storage.get<{ entries: StartClockEntry[] }>('competitionDeliveryQueue')
  if (!queue) throw new Error('competition clock queue is missing')
  return { result, entries: queue.entries, storage }
}

class FixtureSocket {
  readyState = 1
  attachment: unknown
  serializeAttachment(value: unknown) { this.attachment = value }
  deserializeAttachment() { return this.attachment }
  send(_value: string) {}
  close() { this.readyState = 3 }
}

async function connectAndReady(room: InstanceType<typeof GameRoom>, binding: CompetitionGameBinding) {
  const originalPair = Reflect.get(globalThis, 'WebSocketPair')
  const originalResponse = globalThis.Response
  // Bun 不提供 DO 的 101 Response transport；只替換 socket 平台邊界，不 mock GameRoom／Rust。
  class SocketResponse extends originalResponse {
    constructor(body?: BodyInit | null, init?: ResponseInit) {
      super(body, init?.status === 101 ? { ...init, status: 200 } : init)
      if (init?.status === 101) Object.defineProperty(this, 'status', { value: 101 })
    }
  }
  Reflect.set(globalThis, 'WebSocketPair', class {
    client = new FixtureSocket()
    server = new FixtureSocket()
  })
  globalThis.Response = SocketResponse
  try {
    for (const player of binding.players) {
      const response = await room.fetch(new Request('https://game-room.internal/socket', {
        headers: {
          Upgrade: 'websocket', 'x-fewfc-user-id': player.userId,
          'x-fewfc-user-name': encodeURIComponent(player.userId),
        },
      }))
      expect(response.status).toBe(101)
      expect(await room.setCompetitionReady({ actorUserId: player.userId, ready: true }))
        .toEqual({ ok: true, ready: true })
    }
  } finally {
    globalThis.Response = originalResponse
    Reflect.set(globalThis, 'WebSocketPair', originalPair)
  }
}

function continuation() {
  return {
    type: 'eliminationDrawContinuation' as const,
    continuation: {
      teams: [
        { team: 'team-a', mainTimeRemainingMs: 1_237 },
        { team: 'team-b', mainTimeRemainingMs: 987 },
      ],
    },
  }
}

test('正式 Rust/WASM 無準備開局依四人 2v2 首位與 turn 建立行動時計', async () => {
  const binding = makeBinding()
  const started = await createAndStart(binding)

  expect(started.result.state).toMatchObject({
    status: 'InProgress',
    phase: 'ActiveEffects',
    currentPlayer: 'player-a',
    turnNumber: 1,
    initialPouchSelection: null,
  })
  expect(started.entries.map(entry => entry.operation)).toEqual([
    { type: 'clockNewGame', matchId: binding.matchId, time: 'reset' },
    { type: 'clockBeginAction', matchId: binding.matchId, team: 'team-a', turn: 1 },
  ])
  expect(new Set(started.entries.map(entry => entry.trustedTimeMs)).size).toBe(1)
})

test('正式 Rust/WASM 淘汰加賽先 carryForward 再啟動無準備首位行動時計', async () => {
  const binding = makeBinding({ startKind: continuation() })
  const started = await createAndStart(binding)

  expect(started.result.state).toMatchObject({
    status: 'InProgress',
    currentPlayer: 'player-a',
    turnNumber: 1,
    initialPouchSelection: null,
  })
  expect(started.entries.map(entry => entry.operation)).toEqual([
    { type: 'clockNewGame', matchId: binding.matchId, time: 'carryForward' },
    { type: 'clockBeginAction', matchId: binding.matchId, team: 'team-a', turn: 1 },
  ])
  expect(new Set(started.entries.map(entry => entry.trustedTimeMs)).size).toBe(1)
})

test('正式 Rust/WASM 有合法初始錦囊選擇時保留準備時計與 reset 前綴', async () => {
  const ruleModules = callWasm<{ modules: string[] }>(
    { ruleVersion: '5.17' },
    wasm.fewfc_resolve_rule_modules,
  ).modules
  expect(ruleModules).toContain('pouch')
  const players = makeBinding().players
  const deckSnapshot = players.map(({ player }) => {
    const resolved = callWasm<{ effective: { name: string; cards: string[] } }>(
      { player },
      wasm.fewfc_resolve_personal_deck,
    )
    return { player, ...resolved.effective }
  })
  const binding = makeBinding({ enabledRuleModules: ruleModules, deckSnapshot })
  const started = await createAndStart(binding)

  expect(started.result.state).toMatchObject({
    status: 'Preparing',
    phase: 'TurnStart',
    turnNumber: 1,
    initialPouchSelection: { remainingPlayers: binding.turnOrder },
  })
  expect(started.entries.map(entry => entry.operation)).toEqual([
    { type: 'clockNewGame', matchId: binding.matchId, time: 'reset' },
    { type: 'clockBeginPreparation', matchId: binding.matchId },
  ])
  expect(new Set(started.entries.map(entry => entry.trustedTimeMs)).size).toBe(1)
})

async function expectReadyRejectsRawMetadataMutation(
  suffix: string,
  mutate: (metadata: GameRoomMetadata) => GameRoomMetadata,
): Promise<void> {
  const storage = new MemoryStorage()
  const { room } = gameRoom(storage)
  const binding = makeBinding({ matchId: `ready-shape-match-${suffix}` })
  await room.createCompetitionGame({ gameId: `ready-shape-game-${suffix}`, binding })
  const metadata = await storage.get<GameRoomMetadata>('metadata')
  if (!metadata) throw new Error('created room metadata is missing')
  const corrupted = mutate(metadata)
  await storage.put('metadata', {
    ...corrupted,
    members: corrupted.members.map(member => ({ ...member, connected: true })),
  })
  const before = structuredClone([...storage.values.entries()])

  expect(await room.setCompetitionReady({ actorUserId: 'user-a', ready: true }))
    .toEqual({ ok: false, code: 'invalidCompetitionRoom' })
  expect([...storage.values.entries()]).toEqual(before)
  expect(storage.alarm).toBeNull()
}

test('準備 RPC 不會以 normalize 修復 raw 2v2 capacity 矛盾', async () => {
  await expectReadyRejectsRawMetadataMutation('capacity', metadata => ({
    ...metadata,
    capacity: 2,
  }))
})

test('準備 RPC 不會以 normalize 修復 raw owner 旗標矛盾', async () => {
  await expectReadyRejectsRawMetadataMutation('owner', metadata => ({
    ...metadata,
    members: metadata.members.map((member, index) => index === 1
      ? { ...member, owner: true }
      : member),
  }))
})
