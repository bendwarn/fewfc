import { expect, spyOn, test } from 'bun:test'
import { createHash } from 'node:crypto'
import {
  COMPETITION_ROUND_ROOM_LEAD_TIME_MS,
  COMPETITION_GAME_BINDING_DIGEST_DOMAIN,
  competitionRoundReadyCutoffMs,
  isCompetitionRoundRef,
  isCompetitionGameBindingRef,
  isCompetitionRoomReadyAck,
  isCompetitionGameAdmissionRequestShape,
  isCompetitionGameAdmissionRequest,
  isCompetitionGameReleaseResult,
  isCompetitionGameRevokeResult,
  isCompetitionGameInitialStartWindow,
  isCompetitionGameInitialStartEvidence,
  type CompetitionRoundRef,
  type CompetitionGameBindingRef,
  type CompetitionGameReleaseRequest,
  type CompetitionGameRevokeRequest,
  type CompetitionGameInitialStartEvidence,
  competitionClockOperationsForTransition,
  competitionDeckEquals,
  competitionGameBindingEquals,
  competitionGameBindingRefForRequest,
  CompetitionGameBindingError,
  competitionRulesSetup,
  isCompetitionGameConclusion,
  isCompetitionGameReadyRequest,
  isCompetitionGameClockControlEligibilityRequest,
  isCompetitionClockControlAck,
  validateCompetitionGameBinding,
  validateCompetitionGameRequest,
  type CompetitionGameBinding,
} from './competition-game-room'
import type { PublicGameState } from '../app/types/fewfc'

test('E4d3c-r3 eligibility request is exact primitive identity data without getters or symbols', () => {
  const request = { competitionId: 'competition-1', matchId: 'match-1', gameId: 'game-1', gameNumber: 1 }
  expect(isCompetitionGameClockControlEligibilityRequest(request)).toBe(true)
  let getters = 0
  const malformed: unknown[] = [
    null, [], true, 'game-1', { ...request, gameNumber: 0 }, { ...request, gameNumber: 1.5 },
    { ...request, gameNumber: Number.MAX_SAFE_INTEGER + 1 }, { ...request, matchId: ' ' },
    { ...request, competitionId: ' competition-1' }, { ...request, gameId: '' },
    { ...request, gameId: { id: 'game-1', [Symbol('nested')]: 'secret' } },
    Object.assign({ ...request }, { [Symbol.dispose]: () => {} }),
    Object.defineProperty({ ...request }, 'hidden', { value: 'secret' }),
    Object.defineProperty({ ...request }, 'matchId', {
      enumerable: true, get() { getters += 1; return 'match-1' },
    }),
    Object.create(request),
  ]
  for (const extra of ['callerUserId', 'trustedTimeMs', 'operation', 'binding', 'private', 'gameInstanceId']) {
    malformed.push({ ...request, [extra]: 'secret' })
  }
  for (const value of malformed) expect(isCompetitionGameClockControlEligibilityRequest(value)).toBe(false)
  expect(getters).toBe(0)
})

function binding(overrides: Partial<CompetitionGameBinding> = {}): CompetitionGameBinding {
  return {
    roundRef: roundReleaseRoundRef(),
    roundStartMs: 1_800_000_000_000,
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

test('競賽準備 RPC payload 僅接受固定 actor 與明確 ready 值', () => {
  expect(isCompetitionGameReadyRequest({ actorUserId: 'user-a', ready: true })).toBe(true)
  expect(isCompetitionGameReadyRequest({ actorUserId: 'user-a', ready: false })).toBe(true)

  for (const value of [
    {},
    { actorUserId: 'user-a' },
    { ready: true },
    { actorUserId: '', ready: true },
    { actorUserId: ' user-a ', ready: true },
    { actorUserId: 'user-a', ready: 'true' },
    { actorUserId: 'user-a', ready: true, deckList: { name: 'live deck', cards: [] } },
    { actorUserId: 'user-a', ready: true, trustedTimeMs: 1 },
  ]) {
    expect(isCompetitionGameReadyRequest(value)).toBe(false)
  }
})

test('competition binding converts only its fixed setup and preserves camelCase fields', () => {
  const setup = competitionRulesSetup(binding())

  expect(setup).toEqual({
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
  const wire = JSON.stringify(binding())
  expect(wire).toContain('competitionId')
  expect(wire).toContain('matchId')
  expect(wire).toContain('gameNumber')
  expect(wire).toContain('startKind')
  expect(wire).toContain('firstPlayer')
  expect(wire).not.toContain('competition_id')
})

test('clock control acknowledgement validates exact camelCase identity and nullable deadline', () => {
  const request = {
    controlId: 'clock-control-1',
    competitionId: 'competition-1',
    matchId: 'match-1',
    gameId: 'game-1',
    gameNumber: 1,
    sourceCommandId: 'referee-pause-1',
    matchRevision: 2,
    operation: 'clockPause' as const,
    trustedTimeMs: 500,
    nextClockDeadlineMs: null,
  }
  const ack = { ok: true, value: { ...request, idempotent: false } }
  expect(JSON.stringify(request)).toContain('nextClockDeadlineMs')
  expect(JSON.stringify(request)).toContain('sourceCommandId')
  expect(isCompetitionClockControlAck(ack, request)).toBe(true)
  expect(isCompetitionClockControlAck({
    ok: true,
    value: { ...ack.value, next_clock_deadline_ms: null },
  }, request)).toBe(false)
  expect(isCompetitionClockControlAck({
    ok: true,
    value: { ...ack.value, matchRevision: 3 },
  }, request)).toBe(false)
})

test('competition binding rejects duplicate users, uneven teams, and deck omissions', () => {
  expect(() => validateCompetitionGameBinding(binding({
    players: [
      { userId: 'same-user', player: 'player-a', team: 'team-a' },
      { userId: 'same-user', player: 'player-b', team: 'team-b' },
    ],
  }))).toThrow()

  expect(() => validateCompetitionGameBinding(binding({
    players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a' },
      { userId: 'user-b', player: 'player-b', team: 'team-a' },
    ],
  }))).toThrow()

  expect(() => validateCompetitionGameBinding(binding({
    deckSnapshot: [{ player: 'player-a', name: 'only one', cards: ['a-1'] }],
  }))).toThrow()
})

test('未啟用個人牌組時只接受空快照並輸出空 deckLists', () => {
  const withoutPersonalDeck = binding({
    enabledRuleModules: [],
    deckSnapshot: [],
  })
  expect(() => validateCompetitionGameBinding(withoutPersonalDeck)).not.toThrow()
  expect(competitionRulesSetup(withoutPersonalDeck).deckLists).toEqual([])
  expect(() => validateCompetitionGameBinding({
    ...withoutPersonalDeck,
    deckSnapshot: [{ player: 'player-a', name: '不得使用', cards: ['a-1'] }],
  })).toThrow()
})

test('competition binding rejects a non-alternating turn order or a changed first player', () => {
  const fourPlayer = binding({
    players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a' },
      { userId: 'user-b', player: 'player-b', team: 'team-b' },
      { userId: 'user-c', player: 'player-c', team: 'team-a' },
      { userId: 'user-d', player: 'player-d', team: 'team-b' },
    ],
    deckSnapshot: [
      { player: 'player-a', name: 'A', cards: [] },
      { player: 'player-b', name: 'B', cards: [] },
      { player: 'player-c', name: 'C', cards: [] },
      { player: 'player-d', name: 'D', cards: [] },
    ],
    initialHp: [
      { team: 'team-a', hp: 20 },
      { team: 'team-b', hp: 20 },
    ],
    firstPlayer: 'player-a',
    turnOrder: ['player-a', 'player-b', 'player-c', 'player-d'],
  })
  expect(() => validateCompetitionGameBinding(fourPlayer)).not.toThrow()
  expect(() => validateCompetitionGameBinding({
    ...fourPlayer,
    turnOrder: ['player-a', 'player-c', 'player-b', 'player-d'],
  })).toThrow()
  expect(() => validateCompetitionGameBinding({
    ...fourPlayer,
    firstPlayer: 'player-b',
  })).toThrow()
})

test('GameConclusion requires at least one cause at runtime', () => {
  const legal = {
    outcome: { type: 'winner', team: 'team-a' },
    causes: [{ type: 'directVictory', rule: 'test-rule', team: 'team-a' }],
  }
  expect(isCompetitionGameConclusion(legal)).toBe(true)
  expect(isCompetitionGameConclusion({ ...legal, causes: [] })).toBe(false)
})

test('same binding is idempotent while a changed binding is not', () => {
  const original = binding()
  expect(competitionGameBindingEquals(original, structuredClone(original))).toBe(true)
  expect(competitionGameBindingEquals(original, {
    ...original,
    gameNumber: 2,
  })).toBe(false)
  expect(competitionGameBindingEquals(original, {
    ...original,
    organizerUserId: 'another-organizer',
  })).toBe(false)
  expect(competitionGameBindingEquals(original, {
    ...original,
    expectedMatchRevision: 2,
  })).toBe(false)
  expect(competitionDeckEquals(
    { name: 'A locked deck', cards: ['a-1', 'a-2'] },
    original.deckSnapshot[0]!,
  )).toBe(true)
  expect(competitionDeckEquals(
    { name: 'tampered', cards: ['a-1', 'a-2'] },
    original.deckSnapshot[0]!,
  )).toBe(false)
})

test('competition binding requires a trusted organizer and positive match revision', () => {
  expect(() => validateCompetitionGameBinding(binding({ organizerUserId: ' ' }))).toThrow()
  expect(() => validateCompetitionGameBinding(binding({ expectedMatchRevision: 0 }))).toThrow()
  expect(() => validateCompetitionGameBinding(binding({ expectedMatchRevision: 1.5 }))).toThrow()
})

function publicState(overrides: Partial<PublicGameState> = {}): PublicGameState {
  return {
    status: 'Preparing',
    currentPlayer: null,
    phase: 'ActiveEffects',
    turnNumber: 0,
    initialPouchSelection: { remainingPlayers: ['player-a', 'player-b'] },
    pendingChoice: null,
    ...overrides,
  } as unknown as PublicGameState
}

test('clock transition maps team preparation completion and then starts the first action', () => {
  const four = binding({
    players: [
      { userId: 'a', player: 'player-a', team: 'team-a' },
      { userId: 'b', player: 'player-b', team: 'team-b' },
      { userId: 'c', player: 'player-c', team: 'team-a' },
      { userId: 'd', player: 'player-d', team: 'team-b' },
    ],
    deckSnapshot: ['player-a', 'player-b', 'player-c', 'player-d']
      .map(player => ({ player, name: player, cards: [] })),
    turnOrder: ['player-a', 'player-b', 'player-c', 'player-d'],
  })
  expect(competitionClockOperationsForTransition(
    four,
    publicState(),
    publicState({ initialPouchSelection: { remainingPlayers: ['player-b', 'player-d'] } }),
  )).toEqual([{ type: 'clockFinishPreparation', matchId: 'match-1', team: 'team-a' }])
  expect(competitionClockOperationsForTransition(
    four,
    publicState({ initialPouchSelection: { remainingPlayers: ['player-b', 'player-d'] } }),
    publicState({
      status: 'InProgress', currentPlayer: 'player-a', turnNumber: 1, initialPouchSelection: null,
    }),
  )).toEqual([
    { type: 'clockFinishPreparation', matchId: 'match-1', team: 'team-b' },
    { type: 'clockBeginAction', matchId: 'match-1', team: 'team-a', turn: 1 },
  ])
})

test('clock transition ends the old action window before switching player and team', () => {
  expect(competitionClockOperationsForTransition(
    binding(),
    publicState({ status: 'InProgress', currentPlayer: 'player-a', turnNumber: 1 }),
    publicState({ status: 'InProgress', currentPlayer: 'player-b', turnNumber: 2, phase: 'ActiveEffects' }),
  )).toEqual([
    { type: 'clockEndWindow', matchId: 'match-1' },
    { type: 'clockBeginAction', matchId: 'match-1', team: 'team-b', turn: 2 },
  ])
})

test('out-of-turn choice uses verified player, team, and visible choice ID on entry and exit', () => {
  const active = publicState({ status: 'InProgress', currentPlayer: 'player-a', turnNumber: 1 })
  const waiting = publicState({
    status: 'InProgress',
    currentPlayer: 'player-a',
    turnNumber: 1,
    phase: 'Action',
    pendingChoice: {
      visibility: 'visible', choiceId: 42, player: 'player-b', reason: {},
      choice: { type: 'card', cards: [] },
    },
  } as unknown as Partial<PublicGameState>)
  expect(competitionClockOperationsForTransition(binding(), active, waiting)).toEqual([
    { type: 'clockEndWindow', matchId: 'match-1' },
    { type: 'clockBeginOutOfTurnChoice', matchId: 'match-1', team: 'team-b', choiceId: '42' },
  ])
  expect(competitionClockOperationsForTransition(binding(), waiting, active)).toEqual([
    { type: 'clockEndWindow', matchId: 'match-1' },
    { type: 'clockBeginAction', matchId: 'match-1', team: 'team-a', turn: 1 },
  ])
})

test('hidden out-of-turn choice fails closed instead of guessing its identity', () => {
  const active = publicState({ status: 'InProgress', currentPlayer: 'player-a', turnNumber: 1 })
  const hidden = publicState({
    status: 'InProgress',
    currentPlayer: 'player-a',
    pendingChoice: { visibility: 'hidden', player: 'player-b', reason: {} },
  } as unknown as Partial<PublicGameState>)
  expect(() => competitionClockOperationsForTransition(binding(), active, hidden)).toThrow(
    'competition out-of-turn choice lacks a verifiable public identity',
  )
})

function roundReleaseRoundRef() {
  return {
    competitionId: 'competition-1',
    stageId: 'stage-1',
    roundNumber: 1,
    roundGenerationId: 'publication-1',
  } satisfies CompetitionRoundRef
}

function roundReleaseBindingRef() {
  return {
    roundRef: roundReleaseRoundRef(),
    roundStartMs: 1_800_000_000_000,
    matchId: 'match-1',
    gameId: 'game-1',
    gameNumber: 1,
    // 僅為格式 fixture，不聲稱是由已驗完整 binding 計算的 fingerprint。
    bindingDigest: 'deadbeef'.repeat(8),
  } satisfies CompetitionGameBindingRef
}

function roundReleaseRequest() {
  return { bindingRef: roundReleaseBindingRef(), commandId: 'release-1' } satisfies CompetitionGameReleaseRequest
}

function roundReleaseEvidence() {
  const release = roundReleaseRequest()
  return {
    bindingRef: release.bindingRef,
    releaseDecisionId: release.commandId,
    gameInstanceId: 'instance-1',
    startSequence: 0,
    startedAtMs: release.bindingRef.roundStartMs,
    initialWindow: { type: 'preparation' },
  } satisfies CompetitionGameInitialStartEvidence
}

test('round-release RoundRef 使用完整公告身分、normalized IDs 及 Rust u32 輪次範圍', () => {
  const ref = roundReleaseRoundRef()
  expect(isCompetitionRoundRef(ref)).toBe(true)
  expect(isCompetitionRoundRef({ ...ref, roundNumber: 0xffff_ffff })).toBe(true)
  for (const roundNumber of [0, -1, 1.5, 0x1_0000_0000, NaN, Infinity, '1']) {
    expect(isCompetitionRoundRef({ ...ref, roundNumber })).toBe(false)
  }
  for (const key of ['competitionId', 'stageId', 'roundGenerationId']) {
    for (const id of ['', ' ', ' leading', 'trailing ', null, 1]) {
      expect(isCompetitionRoundRef({ ...ref, [key]: id })).toBe(false)
    }
  }
  expect(isCompetitionRoundRef({ ...ref, round_number: 1 })).toBe(false)
})

test('round-release BindingRef required T、u16 gameNumber 與 SHA256 hex 僅做結構核對', () => {
  const ref = roundReleaseBindingRef()
  expect(COMPETITION_GAME_BINDING_DIGEST_DOMAIN).toBe('fewfc.competitionGameBinding.roundRelease.v1')
  expect(isCompetitionGameBindingRef(ref)).toBe(true)
  expect(isCompetitionGameBindingRef({ ...ref, gameNumber: 0xffff })).toBe(true)
  for (const gameNumber of [0, -1, 1.5, 0x1_0000, Number.MAX_SAFE_INTEGER, '1']) {
    expect(isCompetitionGameBindingRef({ ...ref, gameNumber })).toBe(false)
  }
  for (const key of ['matchId', 'gameId']) {
    for (const id of ['', ' id ', null, {}]) expect(isCompetitionGameBindingRef({ ...ref, [key]: id })).toBe(false)
  }
  for (const bindingDigest of [
    '', 'a'.repeat(63), 'a'.repeat(65), 'x'.repeat(64), ref.bindingDigest.toUpperCase(),
    `sha256:${ref.bindingDigest}`, `${ref.bindingDigest}\n`, null, 1,
  ]) expect(isCompetitionGameBindingRef({ ...ref, bindingDigest })).toBe(false)
  for (const roundStartMs of [undefined, null, 0, 299_999, -1, 300_000.5, NaN, Infinity, '300000']) {
    expect(isCompetitionGameBindingRef({ ...ref, roundStartMs })).toBe(false)
  }
  expect(isCompetitionGameBindingRef({ ...ref, round_start_ms: ref.roundStartMs })).toBe(false)
})

test('round-release cutoff 安全 Date 範圍與邊界，不讀 now、不以0 fallback', () => {
  expect(COMPETITION_ROUND_ROOM_LEAD_TIME_MS).toBe(300_000)
  expect(competitionRoundReadyCutoffMs(300_000)).toBe(0)
  expect(competitionRoundReadyCutoffMs(300_001)).toBe(1)
  expect(competitionRoundReadyCutoffMs(1_800_000_000_000)).toBe(1_799_999_700_000)
  expect(competitionRoundReadyCutoffMs(8_640_000_000_000_000)).toBe(8_639_999_999_700_000)
  for (const value of [
    8_640_000_000_000_001, Number.MAX_SAFE_INTEGER, Number.MAX_SAFE_INTEGER + 1,
    -1, 0, 299_999, 300_000.5, NaN, Infinity, '300000', undefined, null, true, {},
  ]) expect(competitionRoundReadyCutoffMs(value)).toBeNull()
})

test('round-release exact3 Waiting ACK 只接受 matching gameId／digest，不反射私密 binding', () => {
  const ref = roundReleaseBindingRef()
  const ack = { gameId: ref.gameId, bindingDigest: ref.bindingDigest, status: 'Waiting' }
  expect(isCompetitionRoomReadyAck(ack, ref)).toBe(true)
  for (const patch of [
    { gameId: 'other' }, { bindingDigest: 'a'.repeat(64) }, { status: 'Active' },
    { binding: { deckSeed: 'secret' } }, { private: {} }, { idempotent: true },
    { ackConfirmedAtMs: ref.roundStartMs - 300_000 }, { ok: true },
  ]) expect(isCompetitionRoomReadyAck({ ...ack, ...patch }, ref)).toBe(false)
  expect(isCompetitionRoomReadyAck(ack, { ...ref, roundStartMs: 0 })).toBe(false)
  const corruptedExpected = { ...ref, roundRef: { ...ref.roundRef, private: true } }
  expect(isCompetitionRoomReadyAck(ack, corruptedExpected)).toBe(false)
})

test('round-release 首次 unknown exact2 shape 不是授權，也不取代持久 expected matching', () => {
  const persisted = roundReleaseRequest()
  const first: unknown = { ...roundReleaseRequest(), commandId: 'first-parent-command' }
  const before = JSON.stringify({ first, persisted })
  expect(isCompetitionGameAdmissionRequestShape(first)).toBe(true)
  // 結構通過不代表 capability／current manifest／持久命令授權已成立。
  if (!isCompetitionGameAdmissionRequestShape(first)) throw new Error('valid shape fixture rejected')
  expect(Object.keys(first)).toEqual(['bindingRef', 'commandId'])
  expect(first.commandId).toBe('first-parent-command')
  expect(isCompetitionGameAdmissionRequest(first, persisted)).toBe(false)
  expect(isCompetitionGameAdmissionRequest(persisted, persisted)).toBe(true)
  expect(JSON.stringify({ first, persisted })).toBe(before)

  const frozen = roundReleaseRequest()
  Object.freeze(frozen.bindingRef.roundRef)
  Object.freeze(frozen.bindingRef)
  Object.freeze(frozen)
  expect(isCompetitionGameAdmissionRequestShape(frozen)).toBe(true)
  const nullRound: unknown = Object.assign(Object.create(null), roundReleaseRoundRef())
  const nullRef: unknown = Object.assign(Object.create(null), { ...roundReleaseBindingRef(), roundRef: nullRound })
  const nullRequest: unknown = Object.assign(Object.create(null), { bindingRef: nullRef, commandId: 'first-parent-command' })
  expect(isCompetitionGameAdmissionRequestShape(nullRequest)).toBe(true)
})

test('round-release shape-only root／ref exact，missing／extra／getter／symbol 均拒絕且不取值', () => {
  const request = roundReleaseRequest()
  const getters = { count: 0 }
  expect(isCompetitionGameAdmissionRequestShape(request)).toBe(true)
  for (const malformed of roundReleaseMalformedObjects(request, 'commandId', getters)) {
    expect(isCompetitionGameAdmissionRequestShape(malformed)).toBe(false)
  }
  for (const key of Object.keys(request)) {
    const missing = Object.fromEntries(Object.entries(request).filter(([name]) => name !== key))
    expect(isCompetitionGameAdmissionRequestShape(missing)).toBe(false)
  }
  for (const commandId of ['', ' ', ' first-parent-command ', null, undefined, true, 1, {}]) {
    expect(isCompetitionGameAdmissionRequestShape({ ...request, commandId })).toBe(false)
  }
  for (const bindingRef of roundReleaseMalformedObjects(request.bindingRef, 'roundStartMs', getters)) {
    expect(isCompetitionGameAdmissionRequestShape({ ...request, bindingRef })).toBe(false)
  }
  for (const key of Object.keys(request.bindingRef)) {
    const bindingRef = Object.fromEntries(Object.entries(request.bindingRef).filter(([name]) => name !== key))
    expect(isCompetitionGameAdmissionRequestShape({ ...request, bindingRef })).toBe(false)
  }
  for (const roundRef of roundReleaseMalformedObjects(request.bindingRef.roundRef, 'roundNumber', getters)) {
    expect(isCompetitionGameAdmissionRequestShape({ ...request, bindingRef: { ...request.bindingRef, roundRef } })).toBe(false)
  }
  for (const key of Object.keys(request.bindingRef.roundRef)) {
    const roundRef = Object.fromEntries(Object.entries(request.bindingRef.roundRef).filter(([name]) => name !== key))
    expect(isCompetitionGameAdmissionRequestShape({ ...request, bindingRef: { ...request.bindingRef, roundRef } })).toBe(false)
  }
  expect(isCompetitionGameAdmissionRequestShape({ ...request, bindingRef: { ...request.bindingRef, gameId: ' game-1 ' } })).toBe(false)
  expect(isCompetitionGameAdmissionRequestShape({ ...request, bindingRef: { ...request.bindingRef, bindingDigest: 'private' } })).toBe(false)
  expect(getters.count).toBe(0)
})

test('round-release shape-only root／nested reflection throws 一律 failclosed', () => {
  const request = roundReleaseRequest()
  const handlers: ProxyHandler<object>[] = [
    { ownKeys() { throw new Error('private reflection failure') } },
    { getPrototypeOf() { throw new Error('private reflection failure') } },
    { getOwnPropertyDescriptor() { throw new Error('private reflection failure') } },
  ]
  for (const handler of handlers) {
    expect(isCompetitionGameAdmissionRequestShape(new Proxy(request, handler))).toBe(false)
    expect(isCompetitionGameAdmissionRequestShape({ ...request, bindingRef: new Proxy(request.bindingRef, handler) })).toBe(false)
    expect(isCompetitionGameAdmissionRequestShape({
      ...request,
      bindingRef: { ...request.bindingRef, roundRef: new Proxy(request.bindingRef.roundRef, handler) },
    })).toBe(false)
  }
})

test('round-release release／revoke 共用 exact2 request，command ID 沿可信 expected 固定', () => {
  const release: CompetitionGameReleaseRequest = roundReleaseRequest()
  const revoke: CompetitionGameRevokeRequest = { ...release, commandId: 'revoke-1' }
  expect(isCompetitionGameAdmissionRequest(release, release)).toBe(true)
  expect(isCompetitionGameAdmissionRequest(revoke, revoke)).toBe(true)
  expect(isCompetitionGameAdmissionRequest(release, revoke)).toBe(false)
  expect(isCompetitionGameAdmissionRequest({ ...release, commandId: 'another-release' }, release)).toBe(false)
  for (const commandId of ['', ' release-1 ', undefined, null, 1]) {
    expect(isCompetitionGameAdmissionRequest({ ...release, commandId }, release)).toBe(false)
  }
  for (const extra of ['releaseDecisionId', 'decision', 'callerUserId', 'trustedTimeMs', 'binding', 'private']) {
    expect(isCompetitionGameAdmissionRequest({ ...release, [extra]: 'secret' }, release)).toBe(false)
  }
  const corruptedExpected = { ...release, private: true }
  expect(isCompetitionGameAdmissionRequest(release, corruptedExpected)).toBe(false)
})

test('round-release expected 完整 BindingRef 單欄 mismatch 不靠 global revision 或 roundNumber 猜同一房', () => {
  const expected = roundReleaseRequest()
  const ref = expected.bindingRef
  const changed: CompetitionGameBindingRef[] = [
    { ...ref, roundRef: { ...ref.roundRef, competitionId: 'other' } },
    { ...ref, roundRef: { ...ref.roundRef, stageId: 'other' } },
    { ...ref, roundRef: { ...ref.roundRef, roundNumber: 2 } },
    { ...ref, roundRef: { ...ref.roundRef, roundGenerationId: 'publication-2' } },
    { ...ref, roundStartMs: ref.roundStartMs + 1 }, { ...ref, matchId: 'other' },
    { ...ref, gameId: 'other' }, { ...ref, gameNumber: 2 }, { ...ref, bindingDigest: 'a'.repeat(64) },
  ]
  for (const bindingRef of changed) {
    expect(isCompetitionGameBindingRef(bindingRef)).toBe(true)
    expect(isCompetitionGameAdmissionRequest({ ...expected, bindingRef }, expected)).toBe(false)
    expect(isCompetitionGameInitialStartEvidence({ ...roundReleaseEvidence(), bindingRef }, expected)).toBe(false)
  }
})

test('round-release 後續局沿同RoundRef／T／decision但有不同BindingRef，不重送首局 evidence', () => {
  const first = roundReleaseRequest()
  const later: CompetitionGameReleaseRequest = { ...first, bindingRef: {
    ...first.bindingRef, gameId: 'game-2', gameNumber: 2, bindingDigest: 'a'.repeat(64),
  } }
  expect(isCompetitionGameAdmissionRequest(later, later)).toBe(true)
  expect(isCompetitionGameAdmissionRequest(later, first)).toBe(false)
  expect(later.commandId).toBe(first.commandId)
  expect(later.bindingRef.roundRef).toEqual(first.bindingRef.roundRef)
  expect(later.bindingRef.roundStartMs).toBe(first.bindingRef.roundStartMs)
  expect(isCompetitionRoomReadyAck({ gameId: 'game-2', bindingDigest: later.bindingRef.bindingDigest, status: 'Waiting' }, later.bindingRef)).toBe(true)
  expect(isCompetitionGameInitialStartEvidence({ ...roundReleaseEvidence(), bindingRef: later.bindingRef }, later)).toBe(false)
})

test('round-release 結果只接受 primitive boolean／closed revoke string，未知不當revoked', () => {
  expect(isCompetitionGameReleaseResult(true)).toBe(true)
  expect(isCompetitionGameReleaseResult(false)).toBe(true)
  for (const value of ['revoked', 'started', 'invalid']) expect(isCompetitionGameRevokeResult(value)).toBe(true)
  for (const value of [null, undefined, 0, 1, 'true', [], {}, { ok: true }, new Boolean(true)]) {
    expect(isCompetitionGameReleaseResult(value)).toBe(false)
  }
  for (const value of [null, undefined, true, false, 0, 'unknown', ' revoked ', 'Revoked', [], { status: 'revoked' }, new String('revoked')]) {
    expect(isCompetitionGameRevokeResult(value)).toBe(false)
  }
  expect(['revoked', 'started', 'invalid'].filter(value => isCompetitionGameRevokeResult(value) && value === 'revoked')).toEqual(['revoked'])
})

test('round-release closed 初始prep／action視窗不帶clock／choice／private或猜時間', () => {
  expect(isCompetitionGameInitialStartWindow({ type: 'preparation' })).toBe(true)
  expect(isCompetitionGameInitialStartWindow({ type: 'action', team: 'team-a', turn: 0 })).toBe(true)
  expect(isCompetitionGameInitialStartWindow({ type: 'action', team: 'team-a', turn: Number.MAX_SAFE_INTEGER })).toBe(true)
  for (const value of [
    { type: 'preparation', team: 'team-a' }, { type: 'preparation', turn: 1 },
    { type: 'action', team: 'team-a' }, { type: 'action', team: ' team-a ', turn: 1 },
    { type: 'action', team: '', turn: 1 }, { type: 'action', team: 'team-a', turn: '1' },
    { type: 'action', team: 'team-a', turn: -1 }, { type: 'action', team: 'team-a', turn: 1.5 },
    { type: 'action', team: 'team-a', turn: Number.MAX_SAFE_INTEGER + 1 },
    { type: 'action', team: 'team-a', turn: Infinity },
    { type: 'action', team: 'team-a', turn: 1, choiceId: 'choice-1' }, { type: 'choice' },
    { type: 'idle' }, { type: 'preparation', private: true },
  ]) expect(isCompetitionGameInitialStartWindow(value)).toBe(false)
})

test('round-release 首局 evidence 綁定 decision 與可信 sample 範圍，sequence0只證結構非真event授權', () => {
  const expected = roundReleaseRequest()
  const evidence = roundReleaseEvidence()
  expect(evidence.releaseDecisionId).toBe(expected.commandId)
  expect(isCompetitionGameInitialStartEvidence(evidence, expected)).toBe(true)
  expect(isCompetitionGameInitialStartEvidence({ ...evidence,
    startedAtMs: expected.bindingRef.roundStartMs - 1 }, expected)).toBe(true)
  expect(isCompetitionGameInitialStartEvidence({ ...evidence, startSequence: Number.MAX_SAFE_INTEGER }, expected)).toBe(true)
  expect(isCompetitionGameInitialStartEvidence({ ...evidence, initialWindow: { type: 'action', team: 'team-a', turn: 0 } }, expected)).toBe(true)
  for (const patch of [
    { releaseDecisionId: 'other' }, { gameInstanceId: '' }, { gameInstanceId: ' instance-1 ' },
    { startSequence: -1 }, { startSequence: 0.5 }, { startSequence: '0' },
    { startSequence: Number.MAX_SAFE_INTEGER + 1 }, { startSequence: NaN },
    { startedAtMs: -1 }, { startedAtMs: 8_640_000_000_000_001 },
    { startedAtMs: undefined }, { startedAtMs: Infinity }, { startedAtMs: '1800000000000' },
    { initialWindow: { type: 'choice' } }, { initialWindow: { type: 'preparation', deck: [] } },
    { start_sequence: 0 }, { record: { private: true } }, { revision: 1 },
  ]) expect(isCompetitionGameInitialStartEvidence({ ...evidence, ...patch }, expected)).toBe(false)
  // 非空 instance／合法序號不是存在性證據，consumer 仍須對真record做late核對。
  expect(isCompetitionGameInitialStartEvidence({ ...evidence, gameInstanceId: 'another-instance' }, expected)).toBe(true)
  expect(isCompetitionGameInitialStartEvidence(evidence, { ...expected, commandId: 'another-release' })).toBe(false)
  expect(isCompetitionGameInitialStartEvidence(evidence, { ...expected, bindingRef: { ...expected.bindingRef, roundStartMs: 0 } })).toBe(false)
})

function roundReleaseMalformedObjects(value: object, requiredKey: string, getters: { count: number }): unknown[] {
  return [
    null, [], true, 'object', new Date(0), Object.create(value),
    Object.setPrototypeOf({ ...value }, { inherited: true }),
    Object.assign({ ...value }, { [Symbol('extra')]: 'secret' }),
    Object.assign({ ...value }, { [Symbol.dispose]: () => {} }),
    Object.defineProperty({ ...value }, 'hidden', { value: 'secret' }),
    Object.defineProperty({ ...value }, requiredKey, { enumerable: false }),
    Object.defineProperty({ ...value }, requiredKey, { enumerable: true, get() { getters.count += 1; return 'secret' } }),
    Object.defineProperty({ ...value }, 'private', { enumerable: true, get() { getters.count += 1; return 'secret' } }),
    new Proxy(value, { ownKeys() { throw new Error('reflection failed') } }),
    new Proxy(value, { getPrototypeOf() { throw new Error('reflection failed') } }),
    new Proxy(value, { getOwnPropertyDescriptor() { throw new Error('reflection failed') } }),
  ]
}

test('round-release root strict keys／descriptors／prototype reject且getter不執行', () => {
  const request = roundReleaseRequest()
  const ref = request.bindingRef
  const getters = { count: 0 }
  const contracts: { value: object; key: string; guard: (value: unknown) => boolean }[] = [
    { value: ref.roundRef, key: 'competitionId', guard: isCompetitionRoundRef },
    { value: ref, key: 'roundRef', guard: isCompetitionGameBindingRef },
    { value: { gameId: ref.gameId, bindingDigest: ref.bindingDigest, status: 'Waiting' }, key: 'gameId', guard: value => isCompetitionRoomReadyAck(value, ref) },
    { value: request, key: 'bindingRef', guard: value => isCompetitionGameAdmissionRequest(value, request) },
    { value: { type: 'preparation' }, key: 'type', guard: isCompetitionGameInitialStartWindow },
    { value: { type: 'action', team: 'team-a', turn: 0 }, key: 'turn', guard: isCompetitionGameInitialStartWindow },
    { value: roundReleaseEvidence(), key: 'initialWindow', guard: value => isCompetitionGameInitialStartEvidence(value, request) },
  ]
  for (const contract of contracts) {
    expect(contract.guard(contract.value)).toBe(true)
    for (const malformed of roundReleaseMalformedObjects(contract.value, contract.key, getters)) {
      expect(contract.guard(malformed)).toBe(false)
    }
  }
  expect(getters.count).toBe(0)
})

test('round-release nested及expected也exact，不能用root檢查掩蓋getter／symbol／extra', () => {
  const expected = roundReleaseRequest()
  const getters = { count: 0 }
  for (const roundRef of roundReleaseMalformedObjects(expected.bindingRef.roundRef, 'roundNumber', getters)) {
    const bindingRef = { ...expected.bindingRef, roundRef }
    expect(isCompetitionGameBindingRef(bindingRef)).toBe(false)
    expect(isCompetitionGameAdmissionRequest({ ...expected, bindingRef }, expected)).toBe(false)
    expect(isCompetitionGameInitialStartEvidence({ ...roundReleaseEvidence(), bindingRef }, expected)).toBe(false)
  }
  for (const bindingRef of roundReleaseMalformedObjects(expected.bindingRef, 'roundStartMs', getters)) {
    expect(isCompetitionGameAdmissionRequest({ ...expected, bindingRef }, expected)).toBe(false)
    expect(isCompetitionGameInitialStartEvidence({ ...roundReleaseEvidence(), bindingRef }, expected)).toBe(false)
  }
  for (const initialWindow of roundReleaseMalformedObjects({ type: 'preparation' }, 'type', getters)) {
    expect(isCompetitionGameInitialStartEvidence({ ...roundReleaseEvidence(), initialWindow }, expected)).toBe(false)
  }
  const badExpected = Object.defineProperty({ ...expected }, 'commandId', {
    enumerable: true, get() { getters.count += 1; return 'release-1' },
  })
  expect(isCompetitionGameAdmissionRequest(expected, badExpected)).toBe(false)
  expect(isCompetitionGameInitialStartEvidence(roundReleaseEvidence(), badExpected)).toBe(false)
  expect(getters.count).toBe(0)
})

test('round-release frozen／null-prototype plain資料可驗且guard不mutate輸入或expected', () => {
  const expected = roundReleaseRequest()
  Object.freeze(expected.bindingRef.roundRef)
  Object.freeze(expected.bindingRef)
  Object.freeze(expected)
  const evidence = Object.freeze(roundReleaseEvidence())
  const before = JSON.stringify({ expected, evidence })
  expect(isCompetitionGameAdmissionRequest(expected, expected)).toBe(true)
  expect(isCompetitionGameInitialStartEvidence(evidence, expected)).toBe(true)
  const nullPrototypeRound: unknown = Object.assign(Object.create(null), roundReleaseRoundRef())
  expect(isCompetitionRoundRef(nullPrototypeRound)).toBe(true)
  expect(isCompetitionGameBindingRef({ ...expected.bindingRef, roundRef: nullPrototypeRound })).toBe(true)
  expect(JSON.stringify({ expected, evidence })).toBe(before)
})

test('round-release JSON roundtrip 保留 exactcamelCase／minimalshape，沒有private或snake欄', () => {
  const expected = roundReleaseRequest()
  const evidence = roundReleaseEvidence()
  const requestWire: unknown = JSON.parse(JSON.stringify(expected))
  const evidenceWire: unknown = JSON.parse(JSON.stringify(evidence))
  const ackWire: unknown = JSON.parse(JSON.stringify({ gameId: expected.bindingRef.gameId, bindingDigest: expected.bindingRef.bindingDigest, status: 'Waiting' }))
  expect(isCompetitionGameAdmissionRequest(requestWire, expected)).toBe(true)
  expect(isCompetitionGameInitialStartEvidence(evidenceWire, expected)).toBe(true)
  expect(isCompetitionRoomReadyAck(ackWire, expected.bindingRef)).toBe(true)
  expect(Object.keys(expected)).toEqual(['bindingRef', 'commandId'])
  expect(Object.keys(evidence)).toEqual(['bindingRef', 'releaseDecisionId', 'gameInstanceId', 'startSequence', 'startedAtMs', 'initialWindow'])
  expect(JSON.stringify(evidence)).toBe(JSON.stringify({
    bindingRef: {
      roundRef: { competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 1, roundGenerationId: 'publication-1' },
      roundStartMs: 1_800_000_000_000, matchId: 'match-1', gameId: 'game-1', gameNumber: 1, bindingDigest: 'deadbeef'.repeat(8),
    },
    releaseDecisionId: 'release-1', gameInstanceId: 'instance-1', startSequence: 0,
    startedAtMs: 1_800_000_000_000, initialWindow: { type: 'preparation' },
  }))
  expect(ackWire).toEqual({ gameId: 'game-1', bindingDigest: 'deadbeef'.repeat(8), status: 'Waiting' })
})

// 固定完整 v1 bytes 為獨立 oracle；包含原 fixture 的 HP，沒有新增 HP 政策或控制值。
const ROUND_RELEASE_FULL_BINDING_GOLDEN_JSON = '{"domain":"fewfc.competitionGameBinding.roundRelease.v1","gameId":"game-1","roundRef":{"competitionId":"competition-1","stageId":"stage-1","roundNumber":1,"roundGenerationId":"publication-1"},"roundStartMs":1800000000000,"binding":{"competitionId":"competition-1","matchId":"match-1","gameNumber":1,"organizerUserId":"organizer-1","expectedMatchRevision":1,"startKind":{"type":"ordinaryNewGame"},"ruleVersion":"5.17","ruleset":"fewfc-base","enabledRuleModules":["personal-deck"],"initialHp":[{"team":"team-a","hp":20},{"team":"team-b","hp":20}],"players":[{"userId":"user-a","player":"player-a","team":"team-a"},{"userId":"user-b","player":"player-b","team":"team-b"}],"deckSnapshot":[{"player":"player-a","name":"A locked deck","cards":["a-1","a-2"]},{"player":"player-b","name":"B locked deck","cards":["b-1","b-2"]}],"firstPlayer":"player-a","turnOrder":["player-a","player-b"],"deckSeed":"competition-1:match-1:game-1"}}'
const ROUND_RELEASE_FULL_BINDING_GOLDEN_DIGEST = '54bd0eb8622c6abe28af6f63c368975cfb970cc9d7e31f6cf5192c015bf22bd5'

function roundReleaseFullRequest() {
  return { gameId: 'game-1', binding: binding() }
}

function roundReleaseContinuationBinding(): CompetitionGameBinding {
  return binding({
    gameNumber: 2,
    deckSeed: 'competition-1:match-1:game-2',
    startKind: {
      type: 'eliminationDrawContinuation',
      continuation: { teams: [
        { team: 'team-a', mainTimeRemainingMs: 1_250 },
        { team: 'team-b', mainTimeRemainingMs: 2_500 },
      ] },
    },
  })
}

function roundReleaseMalformedArrays(value: readonly unknown[], getters: { count: number }): unknown[] {
  const hole = [...value]
  Reflect.deleteProperty(hole, '0')
  return [
    null, {}, Object.create(value), hole,
    Object.assign([...value], { extra: 'secret' }),
    Object.assign([...value], { [Symbol('extra')]: 'secret' }),
    Object.assign([...value], { [Symbol.dispose]: () => {} }),
    Object.defineProperty([...value], 'hidden', { value: 'secret' }),
    Object.defineProperty([...value], '0', { enumerable: false }),
    Object.defineProperty([...value], '0', {
      enumerable: true, get() { getters.count += 1; return value[0] },
    }),
    Object.setPrototypeOf([...value], null),
    Object.setPrototypeOf([...value], { inherited: true }),
    new Proxy([...value], { ownKeys() { throw new Error('private reflection failed') } }),
    new Proxy([...value], { getPrototypeOf() { throw new Error('private reflection failed') } }),
    new Proxy([...value], { getOwnPropertyDescriptor() { throw new Error('private reflection failed') } }),
  ]
}

function roundReleaseFreezeData(value: unknown): void {
  if (typeof value !== 'object' || value === null) return
  for (const nested of Object.values(value)) roundReleaseFreezeData(nested)
  Object.freeze(value)
}

test('round-release 完整 binding required17，原合法 fixture／fault controls 不因缺新欄被提前擋', () => {
  const request = roundReleaseFullRequest()
  expect(() => validateCompetitionGameRequest(request)).not.toThrow()
  expect(Object.keys(request.binding)).toHaveLength(17)
  expect(request.binding.roundRef).toEqual(roundReleaseRoundRef())
  expect(request.binding.roundStartMs).toBe(1_800_000_000_000)
  for (const key of ['roundRef', 'roundStartMs']) {
    const missing = { ...request.binding }
    Reflect.deleteProperty(missing, key)
    expect(() => validateCompetitionGameBinding(missing)).toThrow(CompetitionGameBindingError)
  }
  const oldModel = { ...request.binding }
  Reflect.deleteProperty(oldModel, 'roundRef')
  Reflect.deleteProperty(oldModel, 'roundStartMs')
  expect(() => validateCompetitionGameBinding(oldModel)).toThrow('competition game binding is invalid')
  expect(() => validateCompetitionGameBinding({ ...request.binding, private: true })).toThrow()
  expect(() => validateCompetitionGameBinding({ ...request.binding, round_start_ms: request.binding.roundStartMs })).toThrow()
  // 原負向沿合法 RoundRef/T 到原 roster／deck／turn／revision 守衛；不改 HP。
  expect(() => validateCompetitionGameBinding(binding({ deckSnapshot: [] }))).toThrow()
  expect(() => validateCompetitionGameBinding(binding({ expectedMatchRevision: 0 }))).toThrow()
})

test('round-release request exact2 與完整 binding root拒getter／symbols／extra，固定安全錯誤', async () => {
  const request = roundReleaseFullRequest()
  const getters = { count: 0 }
  for (const malformed of roundReleaseMalformedObjects(request, 'binding', getters)) {
    expect(() => validateCompetitionGameRequest(malformed)).toThrow('competition game binding is invalid')
    await expect(competitionGameBindingRefForRequest(malformed)).rejects.toMatchObject({
      name: 'CompetitionGameBindingError', code: 'invalidCompetitionBinding',
      message: 'competition game binding is invalid',
    })
  }
  for (const malformed of roundReleaseMalformedObjects(request.binding, 'roundStartMs', getters)) {
    expect(() => validateCompetitionGameBinding(malformed)).toThrow('competition game binding is invalid')
    await expect(competitionGameBindingRefForRequest({ ...request, binding: malformed })).rejects.toMatchObject({
      code: 'invalidCompetitionBinding', message: 'competition game binding is invalid',
    })
  }
  for (const gameId of ['', ' game-1 ', null, 1]) {
    expect(() => validateCompetitionGameRequest({ ...request, gameId })).toThrow()
  }
  expect(getters.count).toBe(0)
})

test('round-release 完整 binding nested資料先驗descriptor再讀，額外欄位不得投影掉後hash', async () => {
  const original = binding()
  const continuation = roundReleaseContinuationBinding()
  if (continuation.startKind.type !== 'eliminationDrawContinuation') throw new Error('invalid test control')
  const getters = { count: 0 }
  const nodes: { value: object; key: string; wrap: (value: unknown) => unknown }[] = [
    { value: original.roundRef, key: 'roundNumber', wrap: roundRef => ({ ...original, roundRef }) },
    { value: original.startKind, key: 'type', wrap: startKind => ({ ...original, startKind }) },
    { value: original.players[0]!, key: 'userId', wrap: player => ({ ...original, players: [player, original.players[1]] }) },
    { value: original.deckSnapshot[0]!, key: 'name', wrap: deck => ({ ...original, deckSnapshot: [deck, original.deckSnapshot[1]] }) },
    { value: original.initialHp[0]!, key: 'team', wrap: entry => ({ ...original, initialHp: [entry, original.initialHp[1]] }) },
    { value: continuation.startKind, key: 'type', wrap: startKind => ({ ...continuation, startKind }) },
    { value: continuation.startKind.continuation, key: 'teams', wrap: value => ({
      ...continuation, startKind: { type: 'eliminationDrawContinuation', continuation: value },
    }) },
    { value: continuation.startKind.continuation.teams[0]!, key: 'mainTimeRemainingMs', wrap: entry => ({
      ...continuation, startKind: {
        type: 'eliminationDrawContinuation',
        continuation: { teams: [entry, { team: 'team-b', mainTimeRemainingMs: 2_500 }] },
      },
    }) },
  ]
  for (const node of nodes) {
    expect(() => validateCompetitionGameBinding(node.wrap(node.value))).not.toThrow()
    for (const malformed of roundReleaseMalformedObjects(node.value, node.key, getters)) {
      const candidate = node.wrap(malformed)
      expect(() => validateCompetitionGameBinding(candidate)).toThrow('competition game binding is invalid')
      await expect(competitionGameBindingRefForRequest({ gameId: 'game-1', binding: candidate })).rejects.toMatchObject({
        code: 'invalidCompetitionBinding',
      })
    }
  }
  expect(getters.count).toBe(0)
})

test('round-release 全部 binding array 必為dense plain資料，holes／indexaccessor／extra failclosed', () => {
  const original = binding()
  const continuation = roundReleaseContinuationBinding()
  if (continuation.startKind.type !== 'eliminationDrawContinuation') throw new Error('invalid test control')
  const getters = { count: 0 }
  const arrays: { value: readonly unknown[]; wrap: (value: unknown) => unknown }[] = [
    { value: original.enabledRuleModules, wrap: enabledRuleModules => ({ ...original, enabledRuleModules }) },
    { value: original.initialHp, wrap: initialHp => ({ ...original, initialHp }) },
    { value: original.players, wrap: players => ({ ...original, players }) },
    { value: original.deckSnapshot, wrap: deckSnapshot => ({ ...original, deckSnapshot }) },
    { value: original.turnOrder, wrap: turnOrder => ({ ...original, turnOrder }) },
    { value: original.deckSnapshot[0]!.cards, wrap: cards => ({
      ...original, deckSnapshot: [{ ...original.deckSnapshot[0], cards }, original.deckSnapshot[1]],
    }) },
    { value: continuation.startKind.continuation.teams, wrap: teams => ({
      ...continuation, startKind: { type: 'eliminationDrawContinuation', continuation: { teams } },
    }) },
  ]
  for (const array of arrays) {
    expect(() => validateCompetitionGameBinding(array.wrap(array.value))).not.toThrow()
    expect(() => validateCompetitionGameBinding(array.wrap(Object.freeze([...array.value])))).not.toThrow()
    for (const malformed of roundReleaseMalformedArrays(array.value, getters)) {
      expect(() => validateCompetitionGameBinding(array.wrap(malformed))).toThrow('competition game binding is invalid')
    }
  }
  expect(getters.count).toBe(0)
})

test('round-release 完整 binding RoundRef identity／safe T／Rust輪局範圍及revision均closed', () => {
  const original = binding()
  for (const roundStartMs of [undefined, null, 0, 299_999, 300_000.5, Infinity, NaN, '300000', 8_640_000_000_000_001]) {
    expect(() => validateCompetitionGameBinding({ ...original, roundStartMs })).toThrow()
  }
  for (const roundStartMs of [300_000, 8_640_000_000_000_000]) {
    expect(() => validateCompetitionGameBinding({ ...original, roundStartMs })).not.toThrow()
  }
  for (const roundNumber of [0, -1, 1.5, 0x1_0000_0000, '1']) {
    expect(() => validateCompetitionGameBinding({ ...original, roundRef: { ...original.roundRef, roundNumber } })).toThrow()
  }
  expect(() => validateCompetitionGameBinding({
    ...original, roundRef: { ...original.roundRef, roundNumber: 0xffff_ffff },
  })).not.toThrow()
  expect(() => validateCompetitionGameBinding({
    ...original, roundRef: { ...original.roundRef, competitionId: 'other-competition' },
  })).toThrow()
  for (const gameNumber of [0, -1, 1.5, 0x1_0000, Number.MAX_SAFE_INTEGER, '1']) {
    expect(() => validateCompetitionGameBinding({ ...original, gameNumber })).toThrow()
  }
  expect(() => validateCompetitionGameBinding({ ...original, gameNumber: 0xffff })).not.toThrow()
  for (const expectedMatchRevision of [0, -1, 1.5, Infinity, Number.MAX_SAFE_INTEGER + 1, '1']) {
    expect(() => validateCompetitionGameBinding({ ...original, expectedMatchRevision })).toThrow()
  }
})

test('round-release binding IDs不normalize／猜來源，opaque deck文字保留原樣', async () => {
  const original = binding()
  const faults: unknown[] = [
    { ...original, matchId: ' match-1 ' },
    { ...original, organizerUserId: ' organizer-1 ' },
    { ...original, firstPlayer: ' player-a ' },
    { ...original, roundRef: { ...original.roundRef, stageId: ' stage-1 ' } },
    { ...original, roundRef: { ...original.roundRef, roundGenerationId: ' publication-1 ' } },
    { ...original, enabledRuleModules: [' personal-deck '] },
    { ...original, turnOrder: [' player-a ', 'player-b'] },
    { ...original, players: [{ ...original.players[0], userId: ' user-a ' }, original.players[1]] },
    { ...original, players: [{ ...original.players[0], player: ' player-a ' }, original.players[1]] },
    { ...original, players: [{ ...original.players[0], team: ' team-a ' }, original.players[1]] },
    { ...original, deckSnapshot: [{ ...original.deckSnapshot[0], player: ' player-a ' }, original.deckSnapshot[1]] },
  ]
  for (const fault of faults) expect(() => validateCompetitionGameBinding(fault)).toThrow()
  const opaque = binding({
    deckSnapshot: [
      { player: 'player-a', name: '  牌組甲  ', cards: ['金-1', ''] },
      original.deckSnapshot[1]!,
    ],
    deckSeed: 'opaque seed: 中文',
  })
  const before = JSON.stringify(opaque)
  const ref = await competitionGameBindingRefForRequest({ gameId: 'game-1', binding: opaque })
  expect(isCompetitionGameBindingRef(ref)).toBe(true)
  expect(JSON.stringify(opaque)).toBe(before)
  expect(competitionRulesSetup(opaque).deckLists[0]).toEqual({
    player: 'player-a', name: '  牌組甲  ', cards: ['金-1', ''],
  })
})

test('round-release 完整 canonical goldenvector包含原HP及全部私密輸入，回覆只exact6ref', async () => {
  expect(createHash('sha256').update(ROUND_RELEASE_FULL_BINDING_GOLDEN_JSON, 'utf8').digest('hex'))
    .toBe(ROUND_RELEASE_FULL_BINDING_GOLDEN_DIGEST)
  const request = roundReleaseFullRequest()
  const ref = await competitionGameBindingRefForRequest(request)
  expect(ref).toEqual({
    roundRef: roundReleaseRoundRef(),
    roundStartMs: 1_800_000_000_000,
    matchId: 'match-1',
    gameId: 'game-1',
    gameNumber: 1,
    bindingDigest: ROUND_RELEASE_FULL_BINDING_GOLDEN_DIGEST,
  })
  expect(Object.keys(ref)).toEqual(['roundRef', 'roundStartMs', 'matchId', 'gameId', 'gameNumber', 'bindingDigest'])
  expect(isCompetitionGameBindingRef(ref)).toBe(true)
  expect(isCompetitionRoomReadyAck({ gameId: ref.gameId, bindingDigest: ref.bindingDigest, status: 'Waiting' }, ref)).toBe(true)
  expect(JSON.stringify(ref)).not.toContain('deckSeed')
  expect(JSON.stringify(ref)).not.toContain('deckSnapshot')
  expect(JSON.stringify(ref)).not.toContain('organizerUserId')
  expect(JSON.stringify(ref)).not.toContain('user-a')
  expect(JSON.stringify(ref)).not.toContain('initialHp')
})

test('round-release equality與fingerprint包括required T／generation及完整binding，不能以部分欄判冪等', async () => {
  const original = binding()
  const originalRef = await competitionGameBindingRefForRequest({ gameId: 'game-1', binding: original })
  const changed: CompetitionGameBinding[] = [
    { ...original, roundStartMs: original.roundStartMs + 1 },
    { ...original, roundRef: { ...original.roundRef, stageId: 'stage-2' } },
    { ...original, roundRef: { ...original.roundRef, roundNumber: 2 } },
    { ...original, roundRef: { ...original.roundRef, roundGenerationId: 'publication-2' } },
    { ...original, competitionId: 'competition-2', roundRef: { ...original.roundRef, competitionId: 'competition-2' } },
    { ...original, matchId: 'match-2' },
    { ...original, gameNumber: 2 },
    { ...original, organizerUserId: 'organizer-2' },
    { ...original, expectedMatchRevision: 2 },
    { ...original, ruleVersion: '5.16' },
    { ...original, enabledRuleModules: ['personal-deck', 'pouch'] },
    { ...original, players: [{ ...original.players[0]!, userId: 'another-user' }, original.players[1]!] },
    { ...original, deckSnapshot: [{ ...original.deckSnapshot[0]!, name: 'another deck' }, original.deckSnapshot[1]!] },
    { ...original, firstPlayer: 'player-b', turnOrder: ['player-b', 'player-a'] },
    { ...original, deckSeed: 'another seed' },
    roundReleaseContinuationBinding(),
  ]
  for (const candidate of changed) {
    expect(() => validateCompetitionGameBinding(candidate)).not.toThrow()
    expect(competitionGameBindingEquals(original, candidate)).toBe(false)
    const ref = await competitionGameBindingRefForRequest({ gameId: 'game-1', binding: candidate })
    expect(ref.bindingDigest).not.toBe(originalRef.bindingDigest)
  }
  const extra = Object.assign({ ...original }, { private: 'secret' })
  expect(() => competitionGameBindingEquals(original, extra)).toThrow('competition game binding is invalid')
  expect(() => competitionGameBindingEquals(extra, original)).toThrow('competition game binding is invalid')
})

test('round-release gameId為完整envelope的一部分，binding equality不冒routing identity', async () => {
  const original = binding()
  const first = await competitionGameBindingRefForRequest({ gameId: 'game-1', binding: original })
  const other = await competitionGameBindingRefForRequest({ gameId: 'game-2', binding: original })
  expect(competitionGameBindingEquals(original, structuredClone(original))).toBe(true)
  expect(first.bindingDigest).not.toBe(other.bindingDigest)
  expect(other).toEqual({ ...first, gameId: 'game-2', bindingDigest: other.bindingDigest })
})

test('round-release canonical固定nestedobject鍵序，不改權威array順序', async () => {
  const original = binding()
  const reordered: CompetitionGameBinding = {
    deckSeed: original.deckSeed,
    turnOrder: [...original.turnOrder],
    firstPlayer: original.firstPlayer,
    deckSnapshot: original.deckSnapshot.map(deck => ({ cards: [...deck.cards], name: deck.name, player: deck.player })),
    players: original.players.map(player => ({ team: player.team, player: player.player, userId: player.userId })),
    initialHp: original.initialHp.map(entry => ({ hp: entry.hp, team: entry.team })),
    enabledRuleModules: [...original.enabledRuleModules],
    ruleset: original.ruleset,
    ruleVersion: original.ruleVersion,
    startKind: { type: 'ordinaryNewGame' },
    expectedMatchRevision: original.expectedMatchRevision,
    organizerUserId: original.organizerUserId,
    gameNumber: original.gameNumber,
    matchId: original.matchId,
    competitionId: original.competitionId,
    roundStartMs: original.roundStartMs,
    roundRef: {
      roundGenerationId: original.roundRef.roundGenerationId,
      roundNumber: original.roundRef.roundNumber,
      stageId: original.roundRef.stageId,
      competitionId: original.roundRef.competitionId,
    },
  }
  expect(competitionGameBindingEquals(original, reordered)).toBe(true)
  expect((await competitionGameBindingRefForRequest({ binding: reordered, gameId: 'game-1' })).bindingDigest)
    .toBe(ROUND_RELEASE_FULL_BINDING_GOLDEN_DIGEST)

  const ordered = binding({ enabledRuleModules: ['personal-deck', 'pouch'] })
  const changes: CompetitionGameBinding[] = [
    { ...ordered, enabledRuleModules: [...ordered.enabledRuleModules].reverse() },
    { ...ordered, players: [...ordered.players].reverse() },
    { ...ordered, deckSnapshot: [...ordered.deckSnapshot].reverse() },
    { ...ordered, deckSnapshot: [{ ...ordered.deckSnapshot[0]!, cards: [...ordered.deckSnapshot[0]!.cards].reverse() }, ordered.deckSnapshot[1]!] },
  ]
  const expected = await competitionGameBindingRefForRequest({ gameId: 'game-1', binding: ordered })
  for (const candidate of changes) {
    expect(competitionGameBindingEquals(ordered, candidate)).toBe(false)
    expect((await competitionGameBindingRefForRequest({ gameId: 'game-1', binding: candidate })).bindingDigest)
      .not.toBe(expected.bindingDigest)
  }
})

test('round-release 後局carry-forward完整驗證／canonical順序，shared不重算clock', async () => {
  const continuation = roundReleaseContinuationBinding()
  if (continuation.startKind.type !== 'eliminationDrawContinuation') throw new Error('invalid test control')
  expect(() => validateCompetitionGameBinding(continuation)).not.toThrow()
  const reordered: CompetitionGameBinding = {
    ...continuation,
    startKind: {
      continuation: { teams: continuation.startKind.continuation.teams.map(entry => ({
        mainTimeRemainingMs: entry.mainTimeRemainingMs, team: entry.team,
      })) },
      type: 'eliminationDrawContinuation',
    },
  }
  const ref = await competitionGameBindingRefForRequest({ gameId: 'game-2', binding: continuation })
  expect(competitionGameBindingEquals(continuation, reordered)).toBe(true)
  expect(await competitionGameBindingRefForRequest({ gameId: 'game-2', binding: reordered })).toEqual(ref)
  expect(ref.roundRef).toEqual(roundReleaseRoundRef())
  expect(ref.roundStartMs).toBe(1_800_000_000_000)
  expect(ref.gameNumber).toBe(2)
  const reverse: CompetitionGameBinding = {
    ...continuation, startKind: {
      type: 'eliminationDrawContinuation',
      continuation: { teams: [...continuation.startKind.continuation.teams].reverse() },
    },
  }
  expect(competitionGameBindingEquals(continuation, reverse)).toBe(false)
  expect((await competitionGameBindingRefForRequest({ gameId: 'game-2', binding: reverse })).bindingDigest).not.toBe(ref.bindingDigest)
  for (const mainTimeRemainingMs of [-1, 1.5, Infinity, NaN, Number.MAX_SAFE_INTEGER + 1, '1250']) {
    expect(() => validateCompetitionGameBinding({
      ...continuation, startKind: { type: 'eliminationDrawContinuation', continuation: { teams: [
        { team: 'team-a', mainTimeRemainingMs }, { team: 'team-b', mainTimeRemainingMs: 2_500 },
      ] } },
    })).toThrow()
  }
  expect(() => validateCompetitionGameBinding({
    ...continuation, startKind: { type: 'eliminationDrawContinuation', continuation: { teams: [
      { team: 'other-team', mainTimeRemainingMs: 1_250 }, { team: 'team-b', mainTimeRemainingMs: 2_500 },
    ] } },
  })).toThrow()
  expect(() => validateCompetitionGameBinding({
    ...continuation, startKind: { type: 'eliminationDrawContinuation', continuation: { teams: [
      { team: 'team-a', mainTimeRemainingMs: null }, { team: 'team-b', mainTimeRemainingMs: 0 },
    ] } },
  })).not.toThrow()
})

test('round-release snapshot-before-await正常mutable輸入改欄，不改已捕獲digest或回覆identity', async () => {
  const request = roundReleaseFullRequest()
  const pending = competitionGameBindingRefForRequest(request)
  request.gameId = 'game-2'
  request.binding.matchId = 'match-2'
  request.binding.gameNumber = 2
  request.binding.roundStartMs += 1
  request.binding.roundRef.roundGenerationId = 'publication-2'
  request.binding.deckSeed = 'new seed'
  request.binding.players[0]!.userId = 'new-user'
  request.binding.deckSnapshot[0]!.cards[0] = 'new-card'
  const ref = await pending
  expect(ref).toEqual({
    roundRef: roundReleaseRoundRef(), roundStartMs: 1_800_000_000_000, matchId: 'match-1',
    gameId: 'game-1', gameNumber: 1, bindingDigest: ROUND_RELEASE_FULL_BINDING_GOLDEN_DIGEST,
  })
  expect(ref.roundRef).not.toBe(request.binding.roundRef)
  const changed = await competitionGameBindingRefForRequest(request)
  expect(changed.bindingDigest).not.toBe(ref.bindingDigest)
  expect(changed.roundRef.roundGenerationId).toBe('publication-2')
})

test('round-release frozen／nullprototype完整request合法，不mutate且wire保持exactcamelCase', async () => {
  const request = roundReleaseFullRequest()
  roundReleaseFreezeData(request)
  const before = JSON.stringify(request)
  expect(() => validateCompetitionGameRequest(request)).not.toThrow()
  const ref = await competitionGameBindingRefForRequest(request)
  expect(ref.bindingDigest).toBe(ROUND_RELEASE_FULL_BINDING_GOLDEN_DIGEST)
  expect(JSON.stringify(request)).toBe(before)
  const nullPrototype: unknown = Object.assign(Object.create(null), {
    gameId: request.gameId,
    binding: Object.assign(Object.create(null), {
      ...request.binding, roundRef: Object.assign(Object.create(null), request.binding.roundRef),
    }),
  })
  expect(() => validateCompetitionGameRequest(nullPrototype)).not.toThrow()
  expect(await competitionGameBindingRefForRequest(nullPrototype)).toEqual(ref)
  const requestWire: unknown = JSON.parse(before)
  const refWire: unknown = JSON.parse(JSON.stringify(ref))
  expect(() => validateCompetitionGameRequest(requestWire)).not.toThrow()
  expect(await competitionGameBindingRefForRequest(requestWire)).toEqual(ref)
  expect(isCompetitionGameBindingRef(refWire)).toBe(true)
  expect(JSON.stringify(ref)).toContain('roundStartMs')
  expect(JSON.stringify(ref)).toContain('bindingDigest')
  expect(JSON.stringify(ref)).not.toContain('round_start_ms')
})

test('round-release 原生crypto邊界失敗只固定closederror、不回fallback或私密例外', async () => {
  const digest = spyOn(crypto.subtle, 'digest').mockRejectedValue(new Error('secret deck / credential'))
  try {
    await expect(competitionGameBindingRefForRequest(roundReleaseFullRequest())).rejects.toMatchObject({
      name: 'CompetitionGameBindingError', code: 'competitionBindingDigestFailed',
      message: 'competition game binding fingerprint could not be computed',
    })
    digest.mockResolvedValue(new ArrayBuffer(1))
    await expect(competitionGameBindingRefForRequest(roundReleaseFullRequest())).rejects.toMatchObject({
      code: 'competitionBindingDigestFailed', message: 'competition game binding fingerprint could not be computed',
    })
  } finally {
    digest.mockRestore()
  }
  expect((await competitionGameBindingRefForRequest(roundReleaseFullRequest())).bindingDigest)
    .toBe(ROUND_RELEASE_FULL_BINDING_GOLDEN_DIGEST)
})

test('round-release 完整2v2 roster／lockeddecks沿原HPcontrol，helper不縮成兩名玩家', async () => {
  const fourPlayer = binding({
    players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a' },
      { userId: 'user-b', player: 'player-b', team: 'team-b' },
      { userId: 'user-c', player: 'player-c', team: 'team-a' },
      { userId: 'user-d', player: 'player-d', team: 'team-b' },
    ],
    deckSnapshot: [
      { player: 'player-a', name: 'A', cards: [] },
      { player: 'player-b', name: 'B', cards: [] },
      { player: 'player-c', name: 'C', cards: [] },
      { player: 'player-d', name: 'D', cards: [] },
    ],
    turnOrder: ['player-a', 'player-b', 'player-c', 'player-d'],
  })
  expect(() => validateCompetitionGameBinding(fourPlayer)).not.toThrow()
  expect(competitionRulesSetup(fourPlayer).players).toEqual([
    { id: 'player-a', team: 'team-a' }, { id: 'player-b', team: 'team-b' },
    { id: 'player-c', team: 'team-a' }, { id: 'player-d', team: 'team-b' },
  ])
  const ref = await competitionGameBindingRefForRequest({ gameId: 'game-4', binding: fourPlayer })
  expect(isCompetitionGameBindingRef(ref)).toBe(true)
  const changed: CompetitionGameBinding = {
    ...fourPlayer, players: fourPlayer.players.map(player => player.player === 'player-c'
      ? { ...player, userId: 'another-user-c' } : player),
  }
  expect(competitionGameBindingEquals(fourPlayer, changed)).toBe(false)
  expect((await competitionGameBindingRefForRequest({ gameId: 'game-4', binding: changed })).bindingDigest)
    .not.toBe(ref.bindingDigest)
  expect(JSON.stringify(ref)).not.toContain('user-c')
})

test('same-team teammate choice entry and exit preserve the current action clock window', async () => {
  const { emptyPublicState } = await import('./game-room')
  const fourPlayer = binding({
    enabledRuleModules: ['star', 'five-directions-legend', 'hero-schools', 'tribulation'],
    players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a' },
      { userId: 'user-b', player: 'player-b', team: 'team-b' },
      { userId: 'user-c', player: 'player-c', team: 'team-a' },
      { userId: 'user-d', player: 'player-d', team: 'team-b' },
    ],
    deckSnapshot: [],
    turnOrder: ['player-a', 'player-b', 'player-c', 'player-d'],
  })
  expect(() => validateCompetitionGameBinding(fourPlayer)).not.toThrow()
  expect(Object.keys(fourPlayer)).toHaveLength(17)
  expect(fourPlayer.players).toEqual([
    { userId: 'user-a', player: 'player-a', team: 'team-a' },
    { userId: 'user-b', player: 'player-b', team: 'team-b' },
    { userId: 'user-c', player: 'player-c', team: 'team-a' },
    { userId: 'user-d', player: 'player-d', team: 'team-b' },
  ])
  expect(fourPlayer.turnOrder).toEqual(['player-a', 'player-b', 'player-c', 'player-d'])

  // 沿用完整公開狀態 fixture，不使用裁剪 cast 或私有 pending-resolution state。
  const active: PublicGameState = {
    ...emptyPublicState(fourPlayer.turnOrder),
    ruleVersion: fourPlayer.ruleVersion,
    enabledRuleModules: [...fourPlayer.enabledRuleModules],
    phase: 'Action',
    currentPlayer: 'player-a',
    turnNumber: 7,
    players: fourPlayer.players.map(member => ({ id: member.player, team: member.team })),
    hp: fourPlayer.initialHp.map(entry => ({ ...entry })),
    hands: [
      { player: 'player-a', cards: { kind: 'hidden', count: 4 } },
      { player: 'player-b', cards: { kind: 'hidden', count: 5 } },
      { player: 'player-c', cards: { kind: 'known', cards: [
        { id: 7, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] },
      ] } },
      { player: 'player-d', cards: { kind: 'hidden', count: 5 } },
    ],
  }
  const waiting: PublicGameState = {
    ...active,
    pendingChoice: {
      visibility: 'visible',
      choiceId: 42,
      player: 'player-c',
      reason: { type: 'earthRendingCard' },
      choice: {
        type: 'card',
        cards: [{ id: 7, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] }],
        minimum: 1,
        maximum: 1,
        canDecline: false,
      },
    },
  }
  const answered: PublicGameState = { ...waiting, pendingChoice: null }
  for (const state of [active, waiting, answered]) {
    expect(state.status).toBe('InProgress')
    expect(state.phase).toBe('Action')
    expect(state.currentPlayer).toBe('player-a')
    expect(state.turnNumber).toBe(7)
    expect(state.initialPouchSelection).toBeNull()
  }
  expect(active.pendingChoice).toBeNull()
  expect(waiting.pendingChoice).toEqual({
    visibility: 'visible', choiceId: 42, player: 'player-c', reason: { type: 'earthRendingCard' },
    choice: {
      type: 'card',
      cards: [{ id: 7, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] }],
      minimum: 1, maximum: 1, canDecline: false,
    },
  })
  expect(answered.pendingChoice).toBeNull()

  // 玩家不同但同隊同回合：保留原 Action 視窗，不另開或重置任何時計額度。
  expect(competitionClockOperationsForTransition(fourPlayer, active, waiting)).toEqual([])
  expect(competitionClockOperationsForTransition(fourPlayer, waiting, answered)).toEqual([])
})

test('other-team choice hands back the same action window before a teammate choice', async () => {
  const { emptyPublicState } = await import('./game-room')
  const fourPlayer = binding({
    enabledRuleModules: ['star', 'five-directions-legend', 'hero-schools', 'tribulation'],
    players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a' },
      { userId: 'user-b', player: 'player-b', team: 'team-b' },
      { userId: 'user-c', player: 'player-c', team: 'team-a' },
      { userId: 'user-d', player: 'player-d', team: 'team-b' },
    ],
    deckSnapshot: [],
    turnOrder: ['player-a', 'player-b', 'player-c', 'player-d'],
  })
  expect(() => validateCompetitionGameBinding(fourPlayer)).not.toThrow()
  expect(Object.keys(fourPlayer)).toHaveLength(17)
  expect(fourPlayer.players).toEqual([
    { userId: 'user-a', player: 'player-a', team: 'team-a' },
    { userId: 'user-b', player: 'player-b', team: 'team-b' },
    { userId: 'user-c', player: 'player-c', team: 'team-a' },
    { userId: 'user-d', player: 'player-d', team: 'team-b' },
  ])
  expect(fourPlayer.turnOrder).toEqual(['player-a', 'player-b', 'player-c', 'player-d'])

  // 只用完整 typed 公開投影表達連續選擇，不捏造私有 canonical resolution。
  const otherTeamChoice: PublicGameState = {
    ...emptyPublicState(fourPlayer.turnOrder),
    ruleVersion: fourPlayer.ruleVersion,
    enabledRuleModules: [...fourPlayer.enabledRuleModules],
    phase: 'Action',
    currentPlayer: 'player-a',
    turnNumber: 7,
    players: fourPlayer.players.map(member => ({ id: member.player, team: member.team })),
    hp: fourPlayer.initialHp.map(entry => ({ ...entry })),
    hands: [
      { player: 'player-a', cards: { kind: 'hidden', count: 4 } },
      { player: 'player-b', cards: { kind: 'known', cards: [
        { id: 6, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] },
      ] } },
      { player: 'player-c', cards: { kind: 'hidden', count: 1 } },
      { player: 'player-d', cards: { kind: 'hidden', count: 5 } },
    ],
    pendingChoice: {
      visibility: 'visible', choiceId: 41, player: 'player-b', reason: { type: 'earthRendingCard' },
      choice: {
        type: 'card',
        cards: [{ id: 6, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] }],
        minimum: 1, maximum: 1, canDecline: false,
      },
    },
  }
  const teammateChoice: PublicGameState = {
    ...otherTeamChoice,
    hands: [
      { player: 'player-a', cards: { kind: 'hidden', count: 4 } },
      { player: 'player-b', cards: { kind: 'hidden', count: 1 } },
      { player: 'player-c', cards: { kind: 'known', cards: [
        { id: 7, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] },
      ] } },
      { player: 'player-d', cards: { kind: 'hidden', count: 5 } },
    ],
    pendingChoice: {
      visibility: 'visible', choiceId: 42, player: 'player-c', reason: { type: 'earthRendingCard' },
      choice: {
        type: 'card',
        cards: [{ id: 7, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] }],
        minimum: 1, maximum: 1, canDecline: false,
      },
    },
  }
  const answered: PublicGameState = { ...teammateChoice, pendingChoice: null }
  for (const state of [otherTeamChoice, teammateChoice, answered]) {
    expect(state.status).toBe('InProgress')
    expect(state.phase).toBe('Action')
    expect(state.currentPlayer).toBe('player-a')
    expect(state.turnNumber).toBe(7)
    expect(state.initialPouchSelection).toBeNull()
  }
  expect(otherTeamChoice.pendingChoice).toEqual({
    visibility: 'visible', choiceId: 41, player: 'player-b', reason: { type: 'earthRendingCard' },
    choice: {
      type: 'card',
      cards: [{ id: 6, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] }],
      minimum: 1, maximum: 1, canDecline: false,
    },
  })
  expect(teammateChoice.pendingChoice).toEqual({
    visibility: 'visible', choiceId: 42, player: 'player-c', reason: { type: 'earthRendingCard' },
    choice: {
      type: 'card',
      cards: [{ id: 7, label: '土行牌', element: 'Earth', level: 1, secretStrategies: [] }],
      minimum: 1, maximum: 1, canDecline: false,
    },
  })
  expect(answered.pendingChoice).toBeNull()

  // 他隊獨立 choice 結束後，隊友沿用原 A／turn 7 Action，而非取得新 choice 額度。
  expect(competitionClockOperationsForTransition(fourPlayer, otherTeamChoice, teammateChoice)).toEqual([
    { type: 'clockEndWindow', matchId: 'match-1' },
    { type: 'clockBeginAction', matchId: 'match-1', team: 'team-a', turn: 7 },
  ])
  expect(competitionClockOperationsForTransition(fourPlayer, teammateChoice, answered)).toEqual([])
})
