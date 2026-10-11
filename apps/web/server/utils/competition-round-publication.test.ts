import { describe, expect, test } from 'bun:test'
import {
  assertCompetitionRoundPublicationNoQuery,
  buildCompetitionRoundAnnouncementRequest,
  buildCompetitionRoundPublicationReadRequest,
  normalizeCompetitionRoundPublicationPathId,
  parseCompetitionRoundPublicationRoundNumber,
  serverCompetitionRoundPublicationTime,
} from './competition-round-publication'

const roundStartMs = 1_800_000_000_000
const trustedTimeMs = roundStartMs - 300_000

function validAnnouncementBody() {
  return {
    commandId: 'round-command-1',
    expectedRevision: 8,
    roundStartMs,
    refereeAssignments: [{
      matchId: 'match-1',
      referees: [{ refereeId: 'referee-1', conflictedTeam: null }],
    }],
  }
}

function expectHttpFailure(operation: () => unknown, statusCode: number, code: string) {
  try {
    operation()
    throw new Error('expected HTTP boundary failure')
  } catch (error) {
    expect(error).toMatchObject({ statusCode, data: { code } })
  }
}

describe('round-publication HTTP module helper', () => {
  test('round-publication HTTP module trims only path ids and builds exact read request without time', () => {
    expect(normalizeCompetitionRoundPublicationPathId('  competition-1  ')).toBe('competition-1')
    expect(buildCompetitionRoundPublicationReadRequest(
      ' competition-1 ', ' stage-1 ', '12', 'session-user',
    )).toEqual({
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 12, callerUserId: 'session-user',
    })
  })

  test('round-publication HTTP module accepts only canonical uint32 positive decimal round paths', () => {
    expect(parseCompetitionRoundPublicationRoundNumber('1')).toBe(1)
    expect(parseCompetitionRoundPublicationRoundNumber('4294967295')).toBe(0xffff_ffff)
    for (const value of ['', '0', '00', '01', '-1', '+1', '1.0', '1e1', 'NaN', '4294967296', ' 1 ']) {
      expect(() => parseCompetitionRoundPublicationRoundNumber(value)).toThrow()
    }
    expect(() => parseCompetitionRoundPublicationRoundNumber(1)).toThrow()
  })

  test('round-publication HTTP module builds exact nine-field announce request with unchanged command intent', () => {
    let clockCalls = 0
    const request = buildCompetitionRoundAnnouncementRequest(
      ' competition-1 ', ' stage-1 ', '2', 'session-user', validAnnouncementBody(),
      () => { clockCalls += 1; return trustedTimeMs },
    )
    expect(Object.keys(request)).toEqual([
      'competitionId', 'stageId', 'roundNumber', 'callerUserId', 'commandId',
      'expectedRevision', 'trustedTimeMs', 'roundStartMs', 'refereeAssignments',
    ])
    expect(request).toEqual({
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 2,
      callerUserId: 'session-user', commandId: 'round-command-1', expectedRevision: 8,
      trustedTimeMs, roundStartMs,
      refereeAssignments: [{ matchId: 'match-1', referees: [{ refereeId: 'referee-1', conflictedTeam: null }] }],
    })
    expect(clockCalls).toBe(1)

    expectHttpFailure(() => buildCompetitionRoundAnnouncementRequest(
      'competition-1', 'stage-1', '2', 'session-user',
      { ...validAnnouncementBody(), commandId: ' round-command-1 ' }, () => trustedTimeMs,
    ), 400, 'invalidRequest')
  })

  test('round-publication HTTP module rejects spoofed fields, snake case, private data, and body accessors', () => {
    const invalidBodies = [
      { commandId: 'x', expectedRevision: 1, roundStartMs, refereeAssignments: [], callerUserId: 'attacker' },
      { commandId: 'x', expectedRevision: 1, roundStartMs, refereeAssignments: [], trustedTimeMs },
      { commandId: 'x', expectedRevision: 1, roundStartMs, refereeAssignments: [], round_start_ms: roundStartMs },
      { commandId: 'x', expectedRevision: 1, roundStartMs, refereeAssignments: [], binding: {} },
      { commandId: 'x', expectedRevision: 1, roundStartMs, refereeAssignments: [], deckSeed: 'private-seed' },
      Object.assign(validAnnouncementBody(), { [Symbol('private')]: 'hidden' }),
      Object.defineProperty(validAnnouncementBody(), 'callerUserId', { value: 'hidden', enumerable: false }),
    ]
    for (const body of invalidBodies) {
      expectHttpFailure(() => buildCompetitionRoundAnnouncementRequest(
        'competition-1', 'stage-1', '2', 'session-user', body, () => trustedTimeMs,
      ), 400, 'invalidRequest')
    }

    let getterCalls = 0
    const accessorBody = validAnnouncementBody()
    Object.defineProperty(accessorBody, 'commandId', {
      enumerable: true,
      get() { getterCalls += 1; return 'round-command-1' },
    })
    expectHttpFailure(() => buildCompetitionRoundAnnouncementRequest(
      'competition-1', 'stage-1', '2', 'session-user', accessorBody, () => trustedTimeMs,
    ), 400, 'invalidRequest')
    expect(getterCalls).toBe(0)
  })

  test('round-publication HTTP module uses the shared guard to reject sparse and duplicate referee assignments', () => {
    const sparseAssignments = Array(1)
    const duplicateMatchAssignments = [
      { matchId: 'match-1', referees: [] },
      { matchId: 'match-1', referees: [] },
    ]
    const duplicateReferees = [{
      matchId: 'match-1',
      referees: [
        { refereeId: 'referee-1', conflictedTeam: null },
        { refereeId: 'referee-1', conflictedTeam: null },
      ],
    }]
    for (const refereeAssignments of [sparseAssignments, duplicateMatchAssignments, duplicateReferees]) {
      expectHttpFailure(() => buildCompetitionRoundAnnouncementRequest(
        'competition-1', 'stage-1', '2', 'session-user',
        { ...validAnnouncementBody(), refereeAssignments }, () => trustedTimeMs,
      ), 400, 'invalidRequest')
    }
  })

  test('round-publication HTTP module rejects malformed identities and invalid round times without coercion', () => {
    expectHttpFailure(() => normalizeCompetitionRoundPublicationPathId('   '), 400, 'invalidRequest')
    expectHttpFailure(() => buildCompetitionRoundPublicationReadRequest(
      'competition-1', 'stage-1', '1', ' session-user ',
    ), 400, 'invalidRequest')
    for (const invalidRoundStartMs of [299_999, 1.5, Number.NaN, 8_640_000_000_000_001]) {
      expectHttpFailure(() => buildCompetitionRoundAnnouncementRequest(
        'competition-1', 'stage-1', '1', 'session-user',
        { ...validAnnouncementBody(), roundStartMs: invalidRoundStartMs }, () => trustedTimeMs,
      ), 400, 'invalidRequest')
    }
    for (const invalidTime of [-1, 1.5, Number.NaN, Number.POSITIVE_INFINITY, 8_640_000_000_000_001]) {
      expectHttpFailure(() => serverCompetitionRoundPublicationTime(() => invalidTime), 503, 'serverTimeUnavailable')
    }
    expect(serverCompetitionRoundPublicationTime(() => 0)).toBe(0)
    expect(serverCompetitionRoundPublicationTime(() => 8_640_000_000_000_000)).toBe(8_640_000_000_000_000)
    expectHttpFailure(() => serverCompetitionRoundPublicationTime(() => { throw new Error('private clock') }),
      503, 'serverTimeUnavailable')
  })

  test('round-publication HTTP module rejects every query marker, query key, body, or unsafe query shape', () => {
    let getterCalls = 0
    const accessorQuery = Object.defineProperty({}, 'callerUserId', {
      enumerable: true,
      get() { getterCalls += 1; return 'attacker' },
    })
    for (const [query, marker, body] of [
      [{}, true, false],
      [{ callerUserId: 'attacker' }, false, false],
      [{}, false, true],
      [Object.assign({}, { [Symbol('query')]: 'hidden' }), false, false],
      [accessorQuery, false, false],
    ] as const) {
      expectHttpFailure(() => assertCompetitionRoundPublicationNoQuery(query, marker, body), 400, 'invalidRequest')
    }
    expect(getterCalls).toBe(0)
    assertCompetitionRoundPublicationNoQuery({}, false, false)
  })
})
