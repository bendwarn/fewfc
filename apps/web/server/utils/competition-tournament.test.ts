import { expect, test } from 'bun:test'
import {
  assertCompetitionTournamentQuery,
  buildCompetitionTournamentReadRequest,
  normalizeCompetitionTournamentPathId,
  projectCompetitionTournamentReadResult,
} from './competition-tournament'

const competitionId = 'competition-1'

function tournament(format: 'singleElimination' | 'roundRobin' | 'doubleElimination' | 'swiss') {
  const rounds = format === 'swiss'
    ? [1, 2, 3]
    : format === 'doubleElimination'
      ? [1, 2, 3]
      : [1, 2]
  return {
    tournamentId: competitionId,
    stages: [{
      config: {
        format,
        matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: format === 'swiss' ? 3 : null,
        teams: [{ teamId: 'team-a', seed: 1 }, { teamId: 'team-b', seed: 2 }],
      },
      status: 'finished',
      standings: [
        { teamId: 'team-a', rank: 1, matchPoints: '3' },
        { teamId: 'team-b', rank: 2, matchPoints: '0' },
      ],
      rounds: rounds.map((roundNumber, index) => {
        const kind = format === 'doubleElimination'
          ? ['winners', 'grandFinal', 'grandFinalReset'][index]
          : 'main'
        if (kind === undefined) throw new Error(`缺少雙敗淘汰第 ${roundNumber} 輪 fixture 類型。`)
        return {
          roundNumber,
          kind,
          matches: [{
            matchId: `${format}-round-${roundNumber}`,
            participants: ['team-a', 'team-b'],
            status: 'completed',
            started: true,
            result: { type: 'win', winnerId: 'team-a', loserId: 'team-b' },
            cancellationReason: null,
          }],
        }
      }),
    }],
  }
}

function success(publicValue: unknown, overrides: Record<string, unknown> = {}) {
  return {
    ok: true,
    value: {
      competitionId,
      revision: 8,
      projection: { ok: true, operation: 'tournamentView', public: publicValue },
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
  throw new Error('預期拒絕不可信的目前賽程回應。')
}

test('normalizes only the path id and builds the exact read request from the session actor', () => {
  expect(normalizeCompetitionTournamentPathId('  competition-1  ')).toBe(competitionId)
  expect(buildCompetitionTournamentReadRequest(' competition-1 ', 'session-user')).toEqual({
    competitionId,
    callerUserId: 'session-user',
  })
  expect(Object.keys(buildCompetitionTournamentReadRequest(competitionId, 'session-user')).sort())
    .toEqual(['callerUserId', 'competitionId'])
  for (const invalid of ['', '  ', null, 1]) {
    expect(() => normalizeCompetitionTournamentPathId(invalid)).toThrow()
  }
  expect(() => buildCompetitionTournamentReadRequest(competitionId, '  ')).toThrow()
})

test('the GET query contract rejects every client field and any body before RPC dispatch', () => {
  expect(() => assertCompetitionTournamentQuery({})).not.toThrow()
  for (const query of [
    { callerUserId: 'attacker' },
    { revision: '999' },
    { expectedRevision: '999' },
    { trustedTimeMs: '1' },
    { projection: 'latest' },
  ]) {
    expect(() => assertCompetitionTournamentQuery(query)).toThrow()
  }
  expect(() => assertCompetitionTournamentQuery({}, true)).toThrow()
})

test('projects the complete finished current public tournament for all four formats', () => {
  for (const format of ['singleElimination', 'roundRobin', 'doubleElimination', 'swiss'] as const) {
    const publicValue = tournament(format)
    const projected = projectCompetitionTournamentReadResult(success(publicValue), competitionId)
    expect(projected).toEqual({
      competitionId,
      revision: 8,
      projection: { ok: true, operation: 'tournamentView', public: publicValue },
    })
    expect(projected.projection.public).toStrictEqual(publicValue)
  }
})

test('maps typed missing, ACL and not-ready failures to fixed safe HTTP errors', () => {
  for (const [code, statusCode, statusMessage, safeMessage] of [
    ['notReady', 409, 'Conflict', '賽事目前尚未準備就緒。'],
    ['notInitialized', 404, 'Not Found', '找不到這場賽事。'],
    ['competitionIdMismatch', 404, 'Not Found', '找不到這場賽事。'],
    ['notParticipant', 403, 'Forbidden', '你沒有這場賽事的存取權。'],
  ] as const) {
    const error = captureError(() => projectCompetitionTournamentReadResult({
      ok: false,
      error: { code, message: 'private internal RPC detail' },
    }, competitionId))
    expect(error).toMatchObject({
      statusCode,
      statusMessage,
      message: safeMessage,
      data: { code, message: safeMessage },
    })
    expect(JSON.stringify(error)).not.toContain('private internal RPC detail')
  }
})

test('rejects malformed envelopes, mismatched identities, private fields and invalid JSON without stripping', () => {
  const malformed: unknown[] = [
    { ...success(tournament('roundRobin')), private: { token: 'root-secret' } },
    success(tournament('roundRobin'), { serverState: { token: 'state-secret' } }),
    { ok: true, value: { competitionId, revision: 8, projection: { ok: true, operation: 'tournamentView', public: tournament('roundRobin'), extra: true } } },
    success({ ...tournament('roundRobin'), extra: true }),
    success({ ...tournament('roundRobin'), tournamentId: 'other-competition' }),
    success({ tournamentId: competitionId, stages: [] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], decks: [] }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], lockedDeck: [] }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], standings: [{ members: ['secret'] }] }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], authorizedPrivate: {} }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], binding: { deckSeed: 'secret' } }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], rosterVersion: 1 }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], captain: 'secret' }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], serverState: {} }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], payload: undefined }] }),
    success({ tournamentId: competitionId, stages: [{ ...tournament('roundRobin').stages[0], payload: Number.NaN }] }),
    success(null),
    success('not a tournament'),
    { ok: true, value: { competitionId, revision: 1.5, projection: { ok: true, operation: 'tournamentView', public: tournament('roundRobin') } } },
    { ok: true, value: { competitionId, revision: 8, projection: { ok: true, operation: 'read', public: tournament('roundRobin') } } },
    { ok: false, error: { code: 'privateFailure', message: 'private code detail' } },
  ]

  for (const input of malformed) {
    const error = captureError(() => projectCompetitionTournamentReadResult(input, competitionId))
    expect(error).toMatchObject({
      statusCode: 502,
      statusMessage: 'Bad Gateway',
      message: '賽事房間回應格式不正確。',
      data: { code: 'competitionRoomProtocolError' },
    })
    expect(JSON.stringify(error)).not.toMatch(/root-secret|state-secret|secret|privateFailure|private code detail/)
  }
})

test('rejects nested and root unknown symbols instead of treating them as JSON', () => {
  const publicWithSymbol = tournament('roundRobin')
  Object.defineProperty(publicWithSymbol, Symbol.for('private-protocol'), { value: true })
  expect(captureError(() => projectCompetitionTournamentReadResult(
    success(publicWithSymbol), competitionId,
  ))).toMatchObject({ statusCode: 502, data: { code: 'competitionRoomProtocolError' } })

  const publicWithNestedSymbol = tournament('roundRobin')
  Object.defineProperty(publicWithNestedSymbol.stages[0], Symbol.for('private-protocol'), { value: true })
  expect(captureError(() => projectCompetitionTournamentReadResult(
    success(publicWithNestedSymbol), competitionId,
  ))).toMatchObject({ statusCode: 502, data: { code: 'competitionRoomProtocolError' } })

  const response = success(tournament('roundRobin'))
  Object.defineProperty(response, Symbol.for('unknown-envelope'), { value: true })
  expect(captureError(() => projectCompetitionTournamentReadResult(response, competitionId)))
    .toMatchObject({ statusCode: 502, data: { code: 'competitionRoomProtocolError' } })
})
