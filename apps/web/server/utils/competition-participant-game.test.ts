import { expect, test } from 'bun:test'
import {
  assertCompetitionParticipantGameQuery,
  buildCompetitionParticipantGameReadRequest,
  normalizeCompetitionParticipantGamePathId,
  projectCompetitionParticipantGameResult,
} from './competition-participant-game'

const expectedCompetitionId = 'competition-1'
const expectedMatchId = 'match-1'

function result(game: unknown, overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId: expectedCompetitionId,
      matchId: expectedMatchId,
      matchRevision: 4,
      game,
      ...overrides,
    },
  }
}

function captureError(operation: () => unknown): unknown {
  try {
    operation()
  } catch (error) {
    return error
  }
  throw new Error('預期拒絕不可信的 participant game 資料。')
}

test('normalizes only route identifiers and builds the exact request with the session actor', () => {
  expect(normalizeCompetitionParticipantGamePathId('  competition-1  ')).toBe('competition-1')
  expect(buildCompetitionParticipantGameReadRequest(
    ' competition-1 ', ' match-1 ', 'session-user',
  )).toEqual({
    competitionId: 'competition-1',
    matchId: 'match-1',
    callerUserId: 'session-user',
  })
  expect(Object.keys(buildCompetitionParticipantGameReadRequest(
    'competition-1', 'match-1', 'session-user',
  )).sort()).toEqual(['callerUserId', 'competitionId', 'matchId'])
  for (const invalid of ['', '   ', null, 1]) {
    expect(() => normalizeCompetitionParticipantGamePathId(invalid)).toThrow()
  }
  expect(() => buildCompetitionParticipantGameReadRequest(
    'competition-1', 'match-1', '  ',
  )).toThrow()
})

test('the GET query contract accepts no client-supplied fields', () => {
  expect(() => assertCompetitionParticipantGameQuery({})).not.toThrow()
  for (const query of [
    { callerUserId: 'attacker' },
    { gameId: 'forged-game' },
    { matchRevision: '999' },
    { trustedTimeMs: '1' },
  ]) {
    expect(() => assertCompetitionParticipantGameQuery(query)).toThrow()
  }
})

test('projects only the exact null, 1v1, and 2v2 current-game result fields', () => {
  expect(projectCompetitionParticipantGameResult(
    result(null), expectedCompetitionId, expectedMatchId,
  )).toEqual({
    competitionId: expectedCompetitionId,
    matchId: expectedMatchId,
    matchRevision: 4,
    game: null,
  })

  for (const [gameId, gameNumber] of [
    ['competition-game-one', 1],
    ['competition-game-two', 2],
  ] as const) {
    const projected = projectCompetitionParticipantGameResult(
      result({ gameId, gameNumber }), expectedCompetitionId, expectedMatchId,
    )
    expect(projected).toEqual({
      competitionId: expectedCompetitionId,
      matchId: expectedMatchId,
      matchRevision: 4,
      game: { gameId, gameNumber },
    })
    expect(Object.keys(projected).sort()).toEqual(['competitionId', 'game', 'matchId', 'matchRevision'])
    expect(Object.keys(projected.game!).sort()).toEqual(['gameId', 'gameNumber'])
  }
})

test('rejects private extras, route identity mismatch, and invalid revision or game identifiers', () => {
  const malformed: unknown[] = [
    { ...result(null), private: { secret: 'root-secret' } },
    result(null, { serverState: { secret: 'state-secret' } }),
    result({ gameId: 'game-1', gameNumber: 1, binding: { secret: 'binding-secret' } }),
    result(null, { competitionId: 'other-competition' }),
    result(null, { matchId: 'other-match' }),
    result(null, { matchRevision: 0 }),
    result(null, { matchRevision: 1.5 }),
    result(null, { matchRevision: Number.MAX_SAFE_INTEGER + 1 }),
    result({ gameId: '', gameNumber: 1 }),
    result({ gameId: ' game-1 ', gameNumber: 1 }),
    result({ gameId: 'game-1', gameNumber: 0 }),
    result({ gameId: 'game-1', gameNumber: 1.5 }),
    result({ gameId: 'game-1', gameNumber: Number.MAX_SAFE_INTEGER + 1 }),
  ]

  for (const input of malformed) {
    const error = captureError(() => projectCompetitionParticipantGameResult(
      input, expectedCompetitionId, expectedMatchId,
    ))
    expect(error).toMatchObject({
      statusCode: 502,
      statusMessage: 'Bad Gateway',
      data: { statusMessage: '競賽房間回應格式不正確。' },
    })
    expect(JSON.stringify(error)).not.toMatch(/root-secret|state-secret|binding-secret/)
  }
})

test('maps ACL and checkpoint errors to fixed safe responses without echoing RPC text', () => {
  const forbidden = captureError(() => projectCompetitionParticipantGameResult({
    ok: false,
    error: { code: 'forbidden', message: 'private roster and user detail' },
  }, expectedCompetitionId, expectedMatchId))
  expect(forbidden).toMatchObject({
    statusCode: 403,
    statusMessage: 'Forbidden',
    data: { statusMessage: '你沒有這場競賽對局的存取權。' },
  })
  expect(JSON.stringify(forbidden)).not.toContain('private roster and user detail')

  const checkpoint = captureError(() => projectCompetitionParticipantGameResult({
    ok: false,
    error: { code: 'invalidMatchCheckpoint', message: 'private checkpoint detail' },
  }, expectedCompetitionId, expectedMatchId))
  expect(checkpoint).toMatchObject({
    statusCode: 409,
    statusMessage: 'Conflict',
    data: { statusMessage: '對局狀態目前無法安全確認，請稍後重試。' },
  })
  expect(JSON.stringify(checkpoint)).not.toContain('private checkpoint detail')

  const unknown = captureError(() => projectCompetitionParticipantGameResult({
    ok: false,
    error: { code: 'unknown-private-code', message: 'private service detail' },
  }, expectedCompetitionId, expectedMatchId))
  expect(unknown).toMatchObject({
    statusCode: 502,
    statusMessage: 'Bad Gateway',
    data: { statusMessage: '競賽房間回應格式不正確。' },
  })
  expect(JSON.stringify(unknown)).not.toMatch(/unknown-private-code|private service detail/)
})
