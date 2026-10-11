import { expect, test } from 'bun:test'
import type { H3Event } from 'h3'
import type { StageConfig } from '../../shared/competition'

const announcementPlan = {
  initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
  swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
  rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
  exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
  stages: [{
    stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
    clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    swissRounds: null, swissPolicy: null, qualification: { type: 'allEligibleTeams' },
  }],
}
import { validateCompetitionGameBinding, type CompetitionGameBinding } from '../../shared/competition-game-room'
import {
  boundedAuditLimit,
  buildCompetitionAnnouncement,
  buildAuditRequest,
  buildCreateTournamentCommand,
  buildCreateTournamentRequest,
  buildExecuteRequest,
  buildInitializeRequest,
  buildMatchReadRequest,
  buildReadRequest,
  buildRefereeCommand,
  buildRefereeRequest,
  buildRegistrationCommand,
  callCompetitionRoom,
  competitionRoomError,
  projectCreateTournamentPublicValue,
  buildStartScheduledMatchCommand,
  buildStartScheduledMatchCompositeRequests,
  deriveStartScheduledMatchChildCommandIds,
  projectProvisionedInitialGameValue,
  validateStartedScheduledMatchValue,
  serverTrustedTime,
  buildClockControlCommand,
  buildClockControlRequest,
  projectClockControlPublicValue,
  buildCaptainNextGameCommand,
  deriveCaptainNextGameChildCommandIds,
  buildCaptainNextGameCompositeRequests,
  buildCaptainNextGameProvisionRequest,
  validateCaptainFirstPlayerAssignmentValue,
  projectCaptainAssignedGameValue,
  type StartScheduledMatchPublicValue,
} from './competition-room'

const stageFixture = {
  stageId: 'stage-1',
  format: 'swiss',
  teams: [{ teamId: 'team-1', seed: 1 }],
  randomSeed: 42,
  matchFormat: { type: 'fixedGames', value: 1 },
  clockConfig: { mainTimeMs: null, actionTimeMs: null, outOfTurnChoiceTimeMs: null },
  swissRounds: 3,
  swissPolicy: null,
} satisfies StageConfig

// 只供 adapter 的確定性完整資料控制，不代表實際 manifest、房間 ACK 或開賽證據。
const roundBindingEpochMs = 1_791_158_400_000

function initialGameBindingFixture(): CompetitionGameBinding {
  return {
    roundRef: {
      competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 1,
      roundGenerationId: 'round-generation-1',
    },
    roundStartMs: roundBindingEpochMs + 600_000,
    competitionId: 'competition-1', matchId: 'match-1', gameNumber: 1,
    organizerUserId: 'organizer', expectedMatchRevision: 2, startKind: { type: 'ordinaryNewGame' },
    ruleVersion: '5.17', ruleset: 'fewfc-base', enabledRuleModules: [],
    initialHp: [{ team: 'team-a', hp: 20 }, { team: 'team-b', hp: 20 }],
    players: [
      { userId: 'user-a', player: 'player-a', team: 'team-a' },
      { userId: 'user-b', player: 'player-b', team: 'team-b' },
    ],
    deckSnapshot: [], firstPlayer: 'player-a', turnOrder: ['player-a', 'player-b'], deckSeed: 'secret-seed',
  }
}

function initialGameAdapterFixture(binding: unknown = initialGameBindingFixture()) {
  const request = buildStartScheduledMatchCompositeRequests(
    'competition-1', 'match-1', 'session-user',
    buildStartScheduledMatchCommand({ stageId: 'stage-1', expectedRevision: 4, commandId: 'root-1' }),
    roundBindingEpochMs,
  ).initialGame
  return {
    request,
    value: {
      competitionId: 'competition-1', matchId: 'match-1', matchRevision: 2, gameNumber: 1,
      gameId: 'game-1', commandId: request.commandId, idempotent: false, status: 'pending', binding,
    },
  }
}

test('server-derived competition inputs ignore spoofed identity, participants, and trusted time', () => {
  const announcement = buildCompetitionAnnouncement({
    organizer: 'attacker',
    participantUserIds: ['attacker'],
    trustedTimeMs: 1,
    ruleVersion: '5.17',
    ruleset: 'fewfc-base',
    enabledRuleModules: [],
    teamSize: 2,
    initialHp: 20,
    registrationMode: 'public',
    personalDecks: 'disabled',
    registrationDeadline: 60_000,
    checkInDeadline: 120_000,
    plan: announcementPlan,
  }, 'session-user')
  const command = buildRegistrationCommand({
    commandId: 'command-1',
    expectedRevision: 1,
    trustedTimeMs: 1,
    callerUserId: 'attacker',
    operation: {
      type: 'registrationExecute',
      action: {
        type: 'registerTeam',
        teamId: 'team-1',
        captain: 'attacker',
        members: ['session-user'],
      },
    },
  }, 'session-user')

  expect(announcement.organizer).toBe('session-user')
  expect(command.operation).toEqual({
    type: 'registrationExecute',
    commandId: 'command-1',
    expectedRevision: 1,
    action: {
      type: 'registerTeam',
      teamId: 'team-1',
      captain: 'session-user',
      members: ['session-user'],
    },
  })
  expect(serverTrustedTime(() => 1234)).toBe(1234)
})

test('clock-control builder emits only the scoped referee clock operation', () => {
  const command = buildClockControlCommand({
    action: 'pause', reason: '  處理斷線  ', expectedMatchRevision: 7, commandId: ' clock-command ',
  })
  expect(command).toEqual({
    action: 'pause', reason: '處理斷線', expectedMatchRevision: 7, commandId: 'clock-command',
  })
  expect(buildClockControlRequest(' comp ', ' match ', ' referee ', command, 123)).toEqual({
    competitionId: 'comp', matchId: 'match', callerUserId: 'referee', commandId: 'clock-command',
    expectedMatchRevision: 7, trustedTimeMs: 123,
    operation: { type: 'clockPause', matchId: 'match', reason: '處理斷線' },
  })
  const resume = buildClockControlCommand({
    action: 'resume', reason: '恢復計時', expectedMatchRevision: 8, commandId: 'resume-command',
  })
  expect(buildClockControlRequest('comp', 'match', 'referee', resume, 124).operation)
    .toEqual({ type: 'clockResume', matchId: 'match', reason: '恢復計時' })
})

test('clock-control builder rejects every extra field and malformed public field', () => {
  const valid = { action: 'pause', reason: '裁判處理', expectedMatchRevision: 1, commandId: 'command' }
  for (const extra of [
    'callerUserId', 'refereeId', 'trustedTimeMs', 'competitionId', 'matchId', 'operation', 'team',
    'turn', 'choiceId', 'serverState',
  ]) {
    expect(() => buildClockControlCommand({ ...valid, [extra]: 'spoof' })).toThrow()
  }
  for (const body of [
    null,
    { ...valid, action: 'clockPause' },
    { ...valid, reason: '   ' },
    { ...valid, expectedMatchRevision: 0 },
    { ...valid, expectedMatchRevision: Number.MAX_SAFE_INTEGER },
    { ...valid, commandId: '' },
    { action: 'pause', reason: '裁判處理', expectedMatchRevision: 1 },
  ]) {
    expect(() => buildClockControlCommand(body)).toThrow()
  }
})

test('clock-control response projection validates the exact value and strips every internal field', () => {
  const command = buildClockControlCommand({
    action: 'pause', reason: '秘密理由', expectedMatchRevision: 4, commandId: 'pause-1',
  })
  const request = buildClockControlRequest('comp', 'match', 'referee', command, 500)
  const value = projectClockControlPublicValue({
    competitionId: 'comp', matchId: 'match', matchRevision: 5, commandId: 'pause-1',
    idempotent: false, timeoutEvidence: null, nextClockDeadlineMs: null, matchProjection: null,
    clockProjection: { ok: true, operation: 'clockPause', public: { paused: true, metadata: [{ safe: true }] } },
  }, request, 'pause')
  expect(value).toEqual({
    competitionId: 'comp', matchId: 'match', matchRevision: 5, commandId: 'pause-1',
    action: 'pause', idempotent: false, nextClockDeadlineMs: null,
  })
  expect(JSON.stringify(value)).not.toMatch(/秘密理由|projection|timeoutEvidence|private|serverState/)

  const roomValue = {
    competitionId: 'comp', matchId: 'match', matchRevision: 5, commandId: 'pause-1',
    idempotent: false, timeoutEvidence: null, nextClockDeadlineMs: null, matchProjection: null,
    clockProjection: { ok: true, operation: 'clockPause', public: { paused: true } },
  }
  const malformedValues = [
    { ...roomValue, extra: 'secret' },
    { ...roomValue, private: { secret: true } },
    { ...roomValue, serverState: { secret: true } },
    { ...roomValue, matchRevision: 6 },
    { ...roomValue, commandId: 'other' },
    { ...roomValue, idempotent: 'false' },
    { ...roomValue, timeoutEvidence: { type: 'timeout' } },
    { ...roomValue, matchProjection: { ok: true } },
    { ...roomValue, nextClockDeadlineMs: -1 },
    { ...roomValue, nextClockDeadlineMs: Number.MAX_SAFE_INTEGER + 1 },
    { ...roomValue, clockProjection: { ok: true, operation: 'clockResume', public: {} } },
    { ...roomValue, clockProjection: { ok: true, operation: 'clockPause', public: { serverState: {} } } },
  ]
  for (const malformed of malformedValues) {
    expect(() => projectClockControlPublicValue(malformed, request, 'pause')).toThrow()
  }
})

test('E4d3c-r2 clock-control exact JSON rejects hidden, accessor, cyclic, sparse, and symbol payloads', () => {
  const command = { action: 'pause', reason: '裁判暫停', expectedMatchRevision: 3, commandId: 'strict-command' }
  let getterCalls = 0
  for (const body of [
    Object.assign({ ...command }, { [Symbol('unknown')]: 'secret' }),
    Object.defineProperty({ ...command }, 'hidden', { value: 'secret' }),
    Object.defineProperty({ ...command }, 'reason', { enumerable: true, get() { getterCalls += 1; return 'secret' } }),
  ]) expect(() => buildClockControlCommand(body)).toThrow()
  expect(getterCalls).toBe(0)
  const request = buildClockControlRequest('competition', 'match', 'referee', buildClockControlCommand(command), 100)
  const result = {
    competitionId: 'competition', matchId: 'match', matchRevision: 4, commandId: 'strict-command',
    idempotent: false, timeoutEvidence: null, nextClockDeadlineMs: null, matchProjection: null,
    clockProjection: { ok: true, operation: 'clockPause', public: { paused: true } },
  }
  const cycle: Record<string, unknown> = {}
  cycle.self = cycle
  const sparse = new Array(1)
  const symbolArray = Object.assign([], { [Symbol.dispose]: () => {} })
  const accessor = Object.defineProperty({}, 'private', { enumerable: true, get() { getterCalls += 1; return 'secret' } })
  for (const value of [
    Object.assign({ ...result }, { [Symbol('unknown')]: 'secret' }),
    Object.defineProperty({ ...result }, 'hidden', { value: 'secret' }),
    ...[cycle, sparse, symbolArray, accessor].map(publicValue => ({ ...result,
      clockProjection: { ok: true, operation: 'clockPause', public: publicValue },
    })),
  ]) expect(() => projectClockControlPublicValue(value, request, 'pause')).toThrow()
  expect(getterCalls).toBe(0)
})

test('announcement builder preserves public and invite-only registration modes', () => {
  const base = {
    ruleVersion: '5.17',
    ruleset: 'fewfc-base',
    enabledRuleModules: [],
    teamSize: 2,
    initialHp: 20,
    personalDecks: 'disabled',
    registrationDeadline: 60_000,
    checkInDeadline: 120_000,
    plan: announcementPlan,
  }
  const publicAnnouncement = buildCompetitionAnnouncement({ ...base, registrationMode: 'public' }, 'organizer')
  expect(publicAnnouncement)
    .toMatchObject({ organizer: 'organizer', registrationMode: 'public' })
  expect(publicAnnouncement.plan).toStrictEqual(announcementPlan)
  const inviteAnnouncement = buildCompetitionAnnouncement({
    ...base,
    registrationMode: { inviteOnly: { invitedCaptains: ['captain-1'] } },
  }, 'organizer')
  expect(inviteAnnouncement).toMatchObject({
    organizer: 'organizer',
    registrationMode: { inviteOnly: { invitedCaptains: ['captain-1'] } },
  })
  expect(inviteAnnouncement.plan).toStrictEqual(announcementPlan)
})

test('route request builders wire only server-derived caller and trusted time to each DO method', () => {
  const announcement = buildCompetitionAnnouncement({
    ruleVersion: '5.17',
    ruleset: 'fewfc-base',
    enabledRuleModules: [],
    teamSize: 1,
    initialHp: 20,
    registrationMode: 'public',
    personalDecks: 'disabled',
    registrationDeadline: 60_000,
    checkInDeadline: 120_000,
    plan: announcementPlan,
  }, 'session-user')
  const command = buildRegistrationCommand({
    commandId: 'command-1',
    expectedRevision: 1,
    operation: { type: 'registrationView' },
    trustedTimeMs: 999,
  }, 'session-user')
  const referee = buildRefereeCommand({ commandId: 'referee-1', expectedRevision: 2 }, 'target-user')

  expect(buildInitializeRequest('competition-1', 'session-user', announcement, () => 10)).toMatchObject({
    competitionId: 'competition-1',
    callerUserId: 'session-user',
    participantUserIds: [],
    trustedTimeMs: 10,
  })
  expect(buildExecuteRequest('competition-1', 'session-user', command, () => 11)).toMatchObject({
    competitionId: 'competition-1',
    callerUserId: 'session-user',
    trustedTimeMs: 11,
  })
  expect(buildRefereeRequest('competition-1', 'session-user', referee, () => 12)).toMatchObject({
    callerUserId: 'session-user',
    targetUserId: 'target-user',
    trustedTimeMs: 12,
  })
  expect(buildReadRequest('competition-1', 'session-user')).toEqual({
    competitionId: 'competition-1', callerUserId: 'session-user',
  })
  expect(buildMatchReadRequest('competition-1', 'match-1', 'session-user')).toEqual({
    competitionId: 'competition-1', matchId: 'match-1', callerUserId: 'session-user',
  })
  expect(buildAuditRequest('competition-1', 'session-user', 25)).toEqual({
    competitionId: 'competition-1', callerUserId: 'session-user', limit: 25,
  })
})

test('tournament HTTP builders emit the exact camelCase server request and preserve the opaque stage', () => {
  const stage = stageFixture
  const command = buildCreateTournamentCommand({
    stage,
    expectedRevision: 7,
    commandId: 'client-command',
  })
  expect(buildCreateTournamentRequest('path-competition', 'session-user', command, () => 1234))
    .toEqual({
      competitionId: 'path-competition',
      callerUserId: 'session-user',
      commandId: 'client-command',
      expectedRevision: 7,
      trustedTimeMs: 1234,
      stage,
    })
})

test('tournament builder rejects spoofed identity fields and generates a missing command id once', () => {
  expect(() => buildCreateTournamentCommand({
    stage: {},
    expectedRevision: 1,
    competitionId: 'spoofed',
    callerUserId: 'attacker',
    serverActor: { role: 'organizer', userId: 'attacker' },
    trustedTimeMs: 1,
    tournamentId: 'spoofed-tournament',
  })).toThrow(/欄位不受支援/)

  let generated = 0
  const command = buildCreateTournamentCommand({ stage: stageFixture, expectedRevision: 1 }, () => {
    generated += 1
    return 'server-command'
  })
  const request = buildCreateTournamentRequest('path-id', 'session-id', command, () => 5678)
  expect(generated).toBe(1)
  expect(request).toEqual({
    competitionId: 'path-id',
    callerUserId: 'session-id',
    commandId: 'server-command',
    expectedRevision: 1,
    trustedTimeMs: 5678,
    stage: stageFixture,
  })
})

test('invalid tournament body, revision, or missing stage is rejected before dispatch', () => {
  let dispatches = 0
  const invalidBodies = [
    null,
    [],
    { expectedRevision: 1 },
    { stage: 'not-an-object', expectedRevision: 1 },
    { stage: {}, expectedRevision: 1 },
    { stage: { ...stageFixture, extra: true }, expectedRevision: 1 },
    { stage: { ...stageFixture, format: 'unknown' }, expectedRevision: 1 },
    { stage: { ...stageFixture, teams: [{ teamId: 'team-1', seed: 1, extra: true }] }, expectedRevision: 1 },
    { stage: {}, expectedRevision: 0 },
    { stage: {}, expectedRevision: 1.5 },
    { stage: {}, expectedRevision: 1, commandId: '' },
  ]
  for (const body of invalidBodies) {
    expect(() => {
      const command = buildCreateTournamentCommand(body)
      dispatches += 1
      return command
    }).toThrow()
  }
  expect(dispatches).toBe(0)
})

test('tournament path calls the dedicated stub RPC without using generic execute', async () => {
  let tournamentCalls = 0
  let executeCalls = 0
  const event = {
    context: {
      _platform: {
        cloudflare: {
          env: {
            APP_ENV: 'development',
            COMPETITION_ROOM: {
              idFromName: () => 'id',
              get: () => ({
                createTournamentFromLockedRoster: async () => {
                  tournamentCalls += 1
                  return {
                    ok: true,
                    value: {
                      competitionId: 'competition-1',
                      revision: 1,
                      commandId: 'generated-command',
                      idempotent: false,
                      projection: { ok: true, operation: 'tournamentCreate', public: {} },
                    },
                  }
                },
                execute: async () => {
                  executeCalls += 1
                  throw new Error('generic execute must not be called')
                },
              }),
            },
          },
        },
      },
    },
  } as unknown as H3Event
  const command = buildCreateTournamentCommand({ stage: stageFixture, expectedRevision: 1 })
  const request = buildCreateTournamentRequest('competition-1', 'session-user', command)
  const value = await callCompetitionRoom(event, 'competition-1', room =>
    room.createTournamentFromLockedRoster(request),
  )
  expect(value).toMatchObject({ competitionId: 'competition-1', projection: { operation: 'tournamentCreate' } })
  expect(tournamentCalls).toBe(1)
  expect(executeCalls).toBe(0)
})

test('registration HTTP command allowlist rejects match, clock, and tournament operations before DO dispatch', () => {
  expect(() => buildRegistrationCommand({
    commandId: 'dangerous',
    expectedRevision: 1,
    operation: { type: 'matchSubmitGame' },
  }, 'session-user')).toThrow(/不可由 HTTP API/)
  expect(() => buildRegistrationCommand({
    commandId: 'dangerous',
    expectedRevision: 1,
    operation: { type: 'clockObserve' },
  }, 'session-user')).toThrow(/不可由 HTTP API/)
  expect(() => buildRegistrationCommand({
    commandId: 'dangerous',
    expectedRevision: 1,
    operation: { type: 'tournamentCreate' },
  }, 'session-user')).toThrow(/不可由 HTTP API/)
})

test('missing command id is generated once and registration metadata stays consistent', () => {
  const command = buildRegistrationCommand({
    expectedRevision: 3,
    operation: {
      type: 'registrationExecute',
      action: { type: 'open' },
    },
  }, 'session-user')
  expect(command.commandId.length).toBeGreaterThan(0)
  expect(command.operation).toMatchObject({
    commandId: command.commandId,
    expectedRevision: 3,
  })
  expect(() => buildRegistrationCommand({
    commandId: 'command-1',
    expectedRevision: 1.5,
    operation: { type: 'registrationView' },
  }, 'session-user')).toThrow()
})

test('referee and audit request helpers use route identity and bounded limits', () => {
  expect(buildRefereeCommand({
    targetUserId: 'referee-1',
    commandId: 'grant-1',
    expectedRevision: 2,
    trustedTimeMs: 1,
    callerUserId: 'attacker',
  })).toEqual({ targetUserId: 'referee-1', commandId: 'grant-1', expectedRevision: 2 })
  expect(buildRefereeCommand({ commandId: 'revoke-1', expectedRevision: 3 }, 'referee-1'))
    .toEqual({ targetUserId: 'referee-1', commandId: 'revoke-1', expectedRevision: 3 })
  expect(boundedAuditLimit('100')).toBe(100)
  expect(() => boundedAuditLimit('101')).toThrow()
  expect(() => boundedAuditLimit('0')).toThrow()
})

test('competition room errors expose only a fixed safe message and machine code', () => {
  const error = competitionRoomError({ code: 'engine:secretInternalMessage' })
  expect(error.statusCode).toBe(422)
  expect(error.statusMessage).toBe('賽程要求無法完成。')
  expect(error.data).toEqual({ code: 'engineRejected' })
  expect(JSON.stringify({ statusCode: error.statusCode, statusMessage: error.statusMessage, data: error.data }))
    .not.toContain('secretInternalMessage')
})

test('create tournament public projector keeps only the public DTO and rejects malformed RPC values safely', () => {
  const projected = projectCreateTournamentPublicValue({
    competitionId: 'competition-1',
    revision: 4,
    commandId: 'command-1',
    idempotent: false,
    serverState: { secret: true },
    private: { secret: true },
    extra: 'secret',
    projection: {
      ok: true,
      operation: 'tournamentCreate',
      public: { tournamentId: 'competition-1' },
      serverState: { secret: true },
      private: { secret: true },
      extra: 'secret',
    },
  })
  expect(projected).toEqual({
    competitionId: 'competition-1',
    revision: 4,
    commandId: 'command-1',
    idempotent: false,
    projection: {
      ok: true,
      operation: 'tournamentCreate',
      public: { tournamentId: 'competition-1' },
    },
  })
  expect(() => projectCreateTournamentPublicValue({
    competitionId: 'competition-1',
    revision: '4',
    commandId: 'command-1',
    idempotent: false,
    projection: { ok: true, operation: 'tournamentCreate', public: {} },
  })).toThrow(/賽事服務暫時無法處理要求/)
})

test('E5a tournament errors map to stable statuses and fixed safe messages', () => {
  const cases = [
    ['invalidStageConfig', 400, '賽程階段設定格式不正確。'],
    ['invalidStageTeams', 400, '賽程隊伍設定格式不正確。'],
    ['registrationNotLocked', 409, '報名尚未鎖定，無法建立賽程。'],
    ['stageTeamMismatch', 409, '賽程隊伍與鎖定報名名單不一致。'],
    ['tournamentAlreadyExists', 409, '賽程已經建立。'],
    ['engineRejected', 422, '賽程要求無法完成。'],
    ['engineProtocolError', 502, '賽事服務暫時無法處理要求。'],
  ] as const
  for (const [code, statusCode, statusMessage] of cases) {
    const error = competitionRoomError({ code, message: 'private engine detail' } as { code: string })
    expect(error.statusCode).toBe(statusCode)
    expect(error.statusMessage).toBe(statusMessage)
    expect(error.data).toEqual({ code })
    expect(error.statusMessage).not.toContain('private engine detail')
  }
})

test('E5c start command rejects missing root id, spoofed fields, and malformed referee entries', () => {
  const invalid = [
    { stageId: 'stage', expectedRevision: 1 },
    { stageId: 'stage', expectedRevision: 1, commandId: '' },
    { stageId: 'stage', expectedRevision: 1, commandId: 'root', callerUserId: 'spoof' },
    { stageId: 'stage', expectedRevision: 1, commandId: 'root', trustedTimeMs: 1 },
    { stageId: 'stage', expectedRevision: 1, commandId: 'root', firstPlayer: 'team-a' },
    { stageId: 'stage', expectedRevision: 1, commandId: 'root', referees: [{ refereeId: 'ref', conflictedTeam: null, extra: true }] },
    { stageId: 'stage', expectedRevision: 1, commandId: 'root', referees: [{ refereeId: 'ref' }] },
    { stageId: 'stage', expectedRevision: 1, commandId: 'root', referees: [{ refereeId: 'ref', conflictedTeam: 1 }] },
  ]
  for (const value of invalid) expect(() => buildStartScheduledMatchCommand(value)).toThrow()
  expect(buildStartScheduledMatchCommand({
    stageId: ' stage-1 ', expectedRevision: 3, commandId: 'root-1',
  })).toEqual({ stageId: 'stage-1', expectedRevision: 3, commandId: 'root-1', referees: [] })
  expect(buildStartScheduledMatchCommand({
    stageId: 'stage-1', expectedRevision: 3, commandId: 'root-1',
    referees: [{ refereeId: 'ref-1', conflictedTeam: null }],
  }).referees).toEqual([{ refereeId: 'ref-1', conflictedTeam: null }])
})

test('E5c root command derives stable distinct child ids and exact two-step payloads with shared time', () => {
  const command = buildStartScheduledMatchCommand({
    stageId: 'stage-1', expectedRevision: 8, commandId: 'root-1',
    referees: [{ refereeId: 'ref-1', conflictedTeam: 'team-a' }],
  })
  const ids = deriveStartScheduledMatchChildCommandIds(command.commandId)
  expect(ids.startCommandId).not.toBe(ids.initialGameCommandId)
  expect(deriveStartScheduledMatchChildCommandIds(command.commandId)).toEqual(ids)
  const requests = buildStartScheduledMatchCompositeRequests(
    'competition-1', 'match-1', 'session-user', command, 54321,
  )
  expect(requests.start).toEqual({
    competitionId: 'competition-1', stageId: 'stage-1', matchId: 'match-1',
    callerUserId: 'session-user', commandId: 'root-1:start-schedule', expectedRevision: 8,
    trustedTimeMs: 54321, referees: [{ refereeId: 'ref-1', conflictedTeam: 'team-a' }],
  })
  expect(requests.initialGame).toEqual({
    competitionId: 'competition-1', matchId: 'match-1', callerUserId: 'session-user',
    commandId: 'root-1:initial-game', expectedMatchRevision: 1, trustedTimeMs: 54321,
  })
  expect(requests.initialGame.trustedTimeMs).toBe(requests.start.trustedTimeMs)
})

test('E5c first-step receipt contract requires exact identity, revision, and public projection operations', () => {
  const request = buildStartScheduledMatchCompositeRequests(
    'competition-1', 'match-1', 'session-user',
    buildStartScheduledMatchCommand({ stageId: 'stage-1', expectedRevision: 4, commandId: 'root-1' }),
    99,
  ).start
  const valid = {
    competitionId: 'competition-1', stageId: 'stage-1', matchId: 'match-1',
    revision: 5, matchRevision: 1, commandId: request.commandId, idempotent: false,
    tournamentProjection: { ok: true, operation: 'tournamentStartMatch', public: {} },
    matchProjection: { ok: true, operation: 'matchCreate', public: {} },
    clockProjection: { ok: true, operation: 'clockCreate', public: {} },
  }
  expect(() => validateStartedScheduledMatchValue(valid, request)).not.toThrow()
  const invalidValues = [
    { ...valid, extra: 'not-allowed' },
    { ...valid, revision: 6 },
    { ...valid, matchRevision: 2 },
    { ...valid, commandId: 'other' },
    { ...valid, clockProjection: { ...valid.clockProjection, operation: 'clockPause' } },
    { ...valid, tournamentProjection: { ...valid.tournamentProjection, serverState: {} } },
    { ...valid, matchProjection: { ...valid.matchProjection, public: { nested: { private: {} } } } },
  ]
  for (const value of invalidValues) expect(() => validateStartedScheduledMatchValue(value, request)).toThrow()
})

test('E5c final public projector strips private binding and maps only allowlisted success fields', () => {
  const request = buildStartScheduledMatchCompositeRequests(
    'competition-1', 'match-1', 'session-user',
    buildStartScheduledMatchCommand({ stageId: 'stage-1', expectedRevision: 4, commandId: 'root-1' }),
    roundBindingEpochMs,
  ).initialGame
  const value = projectProvisionedInitialGameValue({
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 2, gameNumber: 1,
    gameId: 'game-1', commandId: request.commandId, idempotent: false, status: 'pending',
    binding: initialGameBindingFixture(),
    private: { secret: true }, serverState: { secret: true }, extra: 'secret-extra',
  }, request, 5, 'root-1')
  expect(value).toEqual({
    competitionId: 'competition-1', matchId: 'match-1', competitionRevision: 5,
    matchRevision: 2, gameNumber: 1, gameId: 'game-1', commandId: 'root-1', status: 'pending',
  })
  expect(JSON.stringify(value)).not.toMatch(/binding|players|deck|turnOrder|secret|roundRef|roundStartMs|bindingDigest|round-generation/)
  expect(() => projectProvisionedInitialGameValue({
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 2, gameNumber: 1,
    gameId: 'game-1', commandId: request.commandId, idempotent: false, status: 'pending', binding: {}, extra: true,
  }, request, 5, 'root-1')).toThrow()

  const completeBinding = initialGameBindingFixture()
  expect(() => validateCompetitionGameBinding(completeBinding)).not.toThrow()
  const malformedFullBinding = { ...completeBinding, turnOrder: ['player-a', 'player-a'] }
  expect(() => validateCompetitionGameBinding(malformedFullBinding)).toThrow()
  try {
    projectProvisionedInitialGameValue({
      competitionId: 'competition-1', matchId: 'match-1', matchRevision: 2, gameNumber: 1,
      gameId: 'game-1', commandId: request.commandId, idempotent: false, status: 'pending',
      binding: malformedFullBinding,
    }, request, 5, 'root-1')
    throw new Error('malformed binding unexpectedly passed validation')
  } catch (error) {
    expect(error).toMatchObject({ statusCode: 502, data: { code: 'engineProtocolError' } })
    if (!(error instanceof Error)) throw error
    expect(error.message).toBe('賽事服務暫時無法處理要求。')
    expect(error.message).not.toContain('secret-seed')
  }
})

test('round-release HTTP adapter accepts complete required17 bindings without exposing private fields', () => {
  const personal = initialGameBindingFixture()
  personal.enabledRuleModules = ['personal-deck']
  personal.deckSnapshot = personal.players.map(player => ({
    player: player.player, name: '鎖定私密牌組', cards: ['a-1', 'b-2'],
  }))
  const snapshot = JSON.stringify(personal)
  const statuses: StartScheduledMatchPublicValue['status'][] = ['pending', 'provisioned', 'blocked']
  for (const status of statuses) {
    const fixture = initialGameAdapterFixture(personal)
    const result = projectProvisionedInitialGameValue({ ...fixture.value, status }, fixture.request, 5, 'root-1')
    expect(result).toEqual({
      competitionId: 'competition-1', matchId: 'match-1', competitionRevision: 5,
      matchRevision: 2, gameNumber: 1, gameId: 'game-1', commandId: 'root-1', status,
    })
    expect(Object.keys(result).sort()).toEqual([
      'commandId', 'competitionId', 'competitionRevision', 'gameId', 'gameNumber', 'matchId', 'matchRevision', 'status',
    ])
    expect(JSON.stringify(result)).not.toMatch(/roundRef|roundStartMs|bindingDigest|binding|deck|roster|player|secret|鎖定私密/)
  }
  expect(JSON.stringify(personal)).toBe(snapshot)
})

test('round-release HTTP adapter accepts complete 2v2 locked snapshots without publishing member data', () => {
  const binding = initialGameBindingFixture()
  binding.enabledRuleModules = ['personal-deck']
  binding.players.push(
    { userId: 'user-c', player: 'player-c', team: 'team-a' },
    { userId: 'user-d', player: 'player-d', team: 'team-b' },
  )
  binding.turnOrder = ['player-a', 'player-b', 'player-c', 'player-d']
  binding.deckSnapshot = binding.players.map(player => ({
    player: player.player, name: '固定名單牌組', cards: ['a-1', 'b-2'],
  }))
  expect(() => validateCompetitionGameBinding(binding)).not.toThrow()
  const fixture = initialGameAdapterFixture(binding)
  const result = projectProvisionedInitialGameValue(fixture.value, fixture.request, 5, 'root-1')
  expect(result).toEqual({
    competitionId: 'competition-1', matchId: 'match-1', competitionRevision: 5,
    matchRevision: 2, gameNumber: 1, gameId: 'game-1', commandId: 'root-1', status: 'pending',
  })
  expect(JSON.stringify(result)).not.toMatch(/round|binding|deck|player|user|固定名單/)
})

test('round-release HTTP adapter rejects missing fields, unknown keys, and invalid round identity or time', () => {
  const complete = initialGameBindingFixture()
  const { roundRef, roundStartMs, ...missingBoth } = complete
  const malformed = [
    missingBoth, { ...missingBoth, roundRef }, { ...missingBoth, roundStartMs },
    { ...complete, round_start_ms: roundStartMs },
    { ...complete, bindingDigest: 'a'.repeat(64) },
    { ...complete, roundRef: { ...roundRef, competitionId: 'other-competition' } },
    { ...complete, roundRef: { ...roundRef, stageId: ' stage-1 ' } },
    { ...complete, roundRef: { ...roundRef, roundNumber: 0 } },
    { ...complete, roundRef: { ...roundRef, roundGenerationId: '' } },
    { ...complete, roundRef: { ...roundRef, private: 'secret' } },
    ...[undefined, null, 0, 299_999, -1, 300_000.5, Number.NaN, Number.MAX_SAFE_INTEGER]
      .map(time => ({ ...complete, roundStartMs: time })),
  ]
  for (const binding of malformed) {
    const fixture = initialGameAdapterFixture(binding)
    expect(() => projectProvisionedInitialGameValue(fixture.value, fixture.request, 5, 'root-1'))
      .toThrow('賽事服務暫時無法處理要求。')
  }
})

test('round-release HTTP adapter validates full raw binding before reading nested identity or dropping extras', () => {
  let getterCalls = 0
  const complete = initialGameBindingFixture()
  const accessor = () => { getterCalls += 1; return 'secret' }
  const malformed = [
    Object.assign({ ...complete }, { [Symbol.dispose]: () => {} }),
    Object.defineProperty({ ...complete }, 'hidden', { value: 'secret' }),
    Object.defineProperty({ ...complete }, 'competitionId', { enumerable: true, get: accessor }),
    { ...complete, roundRef: Object.defineProperty({ ...complete.roundRef }, 'competitionId', { enumerable: true, get: accessor }) },
    { ...complete, players: [Object.defineProperty({ ...complete.players[0] }, 'userId', { enumerable: true, get: accessor }), complete.players[1]] },
    { ...complete, startKind: { type: 'ordinaryNewGame', private: 'secret' } },
    { ...complete, turnOrder: Object.assign([...complete.turnOrder], { [Symbol('unknown')]: 'secret' }) },
    { ...complete, turnOrder: Object.defineProperty([...complete.turnOrder], '0', { get: accessor }) },
    { ...complete, turnOrder: new Array(2) },
    new Proxy(complete, { ownKeys() { throw new Error('secret reflection detail') } }),
  ]
  for (const binding of malformed) {
    const fixture = initialGameAdapterFixture(binding)
    expect(() => projectProvisionedInitialGameValue(fixture.value, fixture.request, 5, 'root-1'))
      .toThrow('賽事服務暫時無法處理要求。')
  }
  const fixture = initialGameAdapterFixture()
  expect(() => projectProvisionedInitialGameValue(
    Object.defineProperty({ ...fixture.value }, 'binding', { enumerable: true, get: accessor }),
    fixture.request, 5, 'root-1',
  )).toThrow('賽事服務暫時無法處理要求。')
  expect(getterCalls).toBe(0)
})

test('round-release HTTP adapter retains request identity checks after complete shared validation', () => {
  const complete = initialGameBindingFixture()
  const malformed = [
    { ...complete, competitionId: 'other-competition', roundRef: { ...complete.roundRef, competitionId: 'other-competition' } },
    { ...complete, matchId: 'other-match' },
    { ...complete, gameNumber: 2 },
    { ...complete, expectedMatchRevision: 3 },
  ]
  for (const binding of malformed) {
    expect(() => validateCompetitionGameBinding(binding)).not.toThrow()
    const fixture = initialGameAdapterFixture(binding)
    expect(() => projectProvisionedInitialGameValue(fixture.value, fixture.request, 5, 'root-1'))
      .toThrow('賽事服務暫時無法處理要求。')
  }
})

test('E5c start and provisioning room errors map safely, including random and linkage failures', () => {
  const cases = [
    ['matchNotFound', 404], ['forbidden', 403], ['invalidRequest', 400],
    ['revisionConflict', 409], ['matchRevisionConflict', 409], ['gameAlreadyLinked', 409],
    ['randomSourceFailure', 502], ['engine:privateRandomError', 422], ['engineProtocolError', 502],
  ] as const
  for (const [code, statusCode] of cases) {
    const error = competitionRoomError({ code })
    expect(error.statusCode).toBe(statusCode)
    expect(JSON.stringify(error)).not.toContain('privateRandomError')
  }
  expect(competitionRoomError({ code: 'unknownPrivateRoomCode' })).toMatchObject({
    statusCode: 502,
    data: { code: 'competitionRoomFailure' },
  })
})

test('missing competition namespace returns 501 and room failures do not reflect DO messages', async () => {
  const event = {
    context: { _platform: { cloudflare: { env: { APP_ENV: 'development' } } } },
  } as unknown as H3Event
  await expect(callCompetitionRoom(event, 'competition-1', async () => ({
    ok: false,
    error: { code: 'internalFailure', message: 'database password leaked' },
  }))).rejects.toMatchObject({ statusCode: 501 })

  const namespaceEvent = {
    context: {
      _platform: {
        cloudflare: {
          env: {
            APP_ENV: 'development',
            COMPETITION_ROOM: {
              idFromName: () => 'id',
              get: () => ({}),
            },
          },
        },
      },
    },
  } as unknown as H3Event
  await expect(callCompetitionRoom(namespaceEvent, 'competition-1', async () => ({
    ok: false,
    error: { code: 'internalFailure', message: 'database password leaked' },
  }))).rejects.toMatchObject({
    statusCode: 502,
    statusMessage: '賽事服務暫時無法處理要求。',
  })
})

function captainGameCommand() {
  return buildCaptainNextGameCommand({
    selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: 7, commandId: 'root-captain',
  })
}

function captainAssignmentProjection() {
  return {
    ok: true,
    operation: 'matchAssignFirstPlayer',
    public: { match: {
      config: {
        matchId: 'match-1', teams: ['team-a', 'team-b'],
        format: { type: 'fixedGames', value: 3 },
        firstGame: { type: 'assigned', team: 'team-a' }, referees: [],
      },
      status: 'open', score: [0, 0],
      games: [{
        gameNumber: 1, gameRecordId: 'game-record-1', startKind: { type: 'ordinaryNewGame' },
        firstPlayer: 'team-b',
        terminalEvidence: {
          type: 'canonicalGameEnded',
          conclusion: { outcome: { type: 'draw' }, causes: [] },
        },
        rulingRecordId: 'ruling-1',
      }],
      rulings: [{
        rulingRecordId: 'ruling-1', gameRecordId: 'game-record-1',
        ruling: { type: 'gameAccepted', outcome: 'draw' },
      }],
      corrections: [], outcome: null, nextFirstPlayer: 'team-a',
      nextGame: {
        gameNumber: 2, kind: { type: 'ordinaryNewGame' },
        firstPlayer: { type: 'assigned', team: 'team-a' },
      },
    } },
  }
}

function captainAssignmentValue(overrides: Record<string, unknown> = {}) {
  return {
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 8,
    commandId: 'root-captain:assign-first-player', idempotent: false,
    matchProjection: captainAssignmentProjection(), ...overrides,
  }
}

function captainProvisionValue(overrides: Record<string, unknown> = {}) {
  return {
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 8,
    gameNumber: 2, gameId: 'game-2', commandId: 'root-captain:captain-game',
    idempotent: false, status: 'pending', ...overrides,
  }
}

test('E5d2c parser derives exact stable child requests and rejects spoof fields', () => {
  const invalid = [
    null, {},
    { selectedFirstPlayerTeam: '', expectedMatchRevision: 1, commandId: 'root' },
    { selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: 0, commandId: 'root' },
    { selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: 1.5, commandId: 'root' },
    { selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: Number.MAX_SAFE_INTEGER, commandId: 'root' },
    { selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: 1, commandId: '' },
    ...['callerUserId', 'competitionId', 'matchId', 'trustedTimeMs', 'assignmentCommandId', 'turnOrder', 'binding', 'deckSeed', 'gameId']
      .map(field => ({ selectedFirstPlayerTeam: 'team-a', expectedMatchRevision: 1, commandId: 'root', [field]: 'spoof' })),
  ]
  for (const value of invalid) expect(() => buildCaptainNextGameCommand(value)).toThrow()
  const command = captainGameCommand()
  const ids = deriveCaptainNextGameChildCommandIds(command.commandId)
  expect(ids).toEqual({
    assignmentCommandId: 'root-captain:assign-first-player',
    provisionCommandId: 'root-captain:captain-game',
  })
  expect(ids.assignmentCommandId).not.toBe(ids.provisionCommandId)
  const composite = buildCaptainNextGameCompositeRequests(' comp ', ' match ', ' session ', command, 1234)
  expect(composite.assignment).toEqual({
    competitionId: 'comp', matchId: 'match', callerUserId: 'session',
    commandId: ids.assignmentCommandId, expectedMatchRevision: 7,
    trustedTimeMs: 1234, selectedFirstPlayerTeam: 'team-a',
  })
  expect(buildCaptainNextGameProvisionRequest(composite, 8)).toEqual({
    competitionId: 'comp', matchId: 'match', callerUserId: 'session',
    commandId: ids.provisionCommandId, assignmentCommandId: ids.assignmentCommandId,
    expectedMatchRevision: 8, trustedTimeMs: 1234,
  })
})

test('E5d2c validators enforce ordinary assigned projection and exact safe provision value', () => {
  const composite = buildCaptainNextGameCompositeRequests(
    'competition-1', 'match-1', 'captain-1', captainGameCommand(), 1234,
  )
  const assignment = validateCaptainFirstPlayerAssignmentValue(captainAssignmentValue(), composite.assignment)
  const publicMatch = captainAssignmentProjection().public.match
  const invalidAssignments = [
    { ...captainAssignmentValue(), extra: true },
    { ...captainAssignmentValue(), private: {} },
    { ...captainAssignmentValue(), competitionId: 'wrong' },
    { ...captainAssignmentValue(), matchRevision: 9 },
    { ...captainAssignmentValue(), commandId: 'wrong' },
    { ...captainAssignmentValue(), matchProjection: { ...captainAssignmentProjection(), operation: 'matchView' } },
    { ...captainAssignmentValue(), matchProjection: { ...captainAssignmentProjection(), serverState: {} } },
    { ...captainAssignmentValue(), matchProjection: { ...captainAssignmentProjection(), public: {
      match: { ...publicMatch, extra: 'not-allowed' },
    } } },
    ...['roster', 'deck', 'captain', 'members', 'authorizedPrivate', 'announcement', 'decks', 'rosterVersion', 'lockedRoster'].map(key => ({
      ...captainAssignmentValue(),
      matchProjection: { ...captainAssignmentProjection(), public: { match: { ...publicMatch, [key]: true } } },
    })),
    { ...captainAssignmentValue(), matchProjection: { ...captainAssignmentProjection(), public: { match: { ...publicMatch, status: 'completed' } } } },
    { ...captainAssignmentValue(), matchProjection: { ...captainAssignmentProjection(), public: { match: { ...publicMatch, outcome: { winner: 'team-a' } } } } },
    {
      ...captainAssignmentValue(),
      matchProjection: {
        ...captainAssignmentProjection(),
        public: { match: { ...publicMatch, nextGame: {
          ...publicMatch.nextGame, kind: { type: 'eliminationContinuation' },
        } } },
      },
    },
    {
      ...captainAssignmentValue(),
      matchProjection: {
        ...captainAssignmentProjection(),
        public: { match: { ...publicMatch, nextGame: {
          gameNumber: 2, kind: { type: 'ordinaryNewGame' }, firstPlayer: { type: 'pending' },
        } } },
      },
    },
  ]
  for (const value of invalidAssignments) {
    expect(() => validateCaptainFirstPlayerAssignmentValue(value, composite.assignment)).toThrow()
  }

  const request = buildCaptainNextGameProvisionRequest(composite, assignment.matchRevision)
  expect(projectCaptainAssignedGameValue(
    captainProvisionValue(), request, assignment, composite.rootCommandId,
  )).toEqual({
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 8,
    gameNumber: 2, gameId: 'game-2', commandId: 'root-captain', status: 'pending',
  })
  for (const status of ['provisioned', 'blocked'] as const) {
    expect(projectCaptainAssignedGameValue(
      captainProvisionValue({ status }), request, assignment, composite.rootCommandId,
    ).status).toBe(status)
  }
  const invalidProvisions = [
    { ...captainProvisionValue(), extra: true }, { ...captainProvisionValue(), binding: {} },
    { ...captainProvisionValue(), private: {} }, { ...captainProvisionValue(), competitionId: 'wrong' },
    { ...captainProvisionValue(), matchRevision: 9 }, { ...captainProvisionValue(), gameNumber: 1 },
    { ...captainProvisionValue(), gameNumber: 3 }, { ...captainProvisionValue(), gameId: '' },
    { ...captainProvisionValue(), commandId: 'wrong' }, { ...captainProvisionValue(), idempotent: 'false' },
    { ...captainProvisionValue(), status: 'unknown' },
  ]
  for (const value of invalidProvisions) {
    expect(() => projectCaptainAssignedGameValue(value, request, assignment, composite.rootCommandId)).toThrow()
  }
})

test('E5d2c policy errors map to fixed safe conflict responses', () => {
  for (const code of ['assignmentNotCurrent', 'firstPlayerPolicyUnresolved', 'eliminationDrawLossUnresolved']) {
    const error = competitionRoomError({ code })
    expect(error.statusCode).toBe(409)
    expect(error.statusMessage).not.toContain(code)
  }
})
