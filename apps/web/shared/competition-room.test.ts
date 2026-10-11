import { expect, test } from 'bun:test'
import type {
  CompetitionRoomExecuteRequest,
  CompetitionRoomInitializeRequest,
  CompetitionRoomMaterializeScheduledMatchValue,
  CompetitionRoomStartScheduledMatchRequest,
  CompetitionRoomMatchExecuteRequest,
  CompetitionRoomMatchReadRequest,
  CompetitionRoomReadRequest,
  CompetitionRoomTerminalIngestRequest,
  CompetitionRoomProvisionScheduledGameRequest,
  CompetitionRoomProvisionInitialScheduledGameRequest,
  CompetitionRoomProvisionAssignedScheduledGameRequest,
  CompetitionRoomCreateTournamentRequest,
  CompetitionRoomCreateTournamentValue,
  CompetitionRoomAssignNextFirstPlayerByCaptainRequest,
  CompetitionRoomProvisionCaptainAssignedScheduledGameRequest,
  CompetitionRoomParticipantGameReadRequest,
  CompetitionRoomParticipantGameReadValue,
  CompetitionRoomTournamentReadRequest,
  CompetitionRoomTournamentReadValue,
  CompetitionRoomMatchClockReadRequest,
  CompetitionRoomMatchClockReadValue,
  CompetitionRoomAnnounceRoundRequest,
  CompetitionRoomRoundPublication,
  CompetitionRoomInitialGameStartedRequest,
  CompetitionRoomMatchExecuteValue,
  CompetitionRoomAuditRecord,
} from './competition-room'
import {
  clockCheckpointDigest,
  isCompetitionRoomAnnounceRoundRequest,
  isCompetitionRoomAnnounceRoundValue,
  isCompetitionRoomRoundPublicationReadRequest,
  isCompetitionRoomRoundPublicationReadValue,
  isCompetitionRoomRoundPublication,
  isCompetitionRoomInitialGameStartedRequest,
} from './competition-room'
import type { CompetitionAnnouncement, StageConfig } from './competition'
import { isCompetitionGameInitialStartEvidence } from './competition-game-room'

test('E6e2a clock read contract has only trusted identity and minimal camelCase display fields', () => {
  const request = {
    competitionId: 'competition-1', matchId: 'match-1', callerUserId: 'player-1', trustedTimeMs: 100,
  } satisfies CompetitionRoomMatchClockReadRequest
  const value = {
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 2, matchStatus: 'open',
    game: { gameId: 'game-1', gameNumber: 1 },
    clock: {
      sampledAtMs: 100,
      teams: [
        { team: 'a', mainTimeRemainingMs: 200, byoyomiTimeRemainingMs: 30, byoyomiActive: false },
        { team: 'b', mainTimeRemainingMs: null, byoyomiTimeRemainingMs: null, byoyomiActive: false },
      ],
      running: { type: 'outOfTurnChoice', team: 'b', choiceId: 'choice-1', remainingMs: 20 },
      paused: null, awaitingAuthority: false,
    },
  } satisfies CompetitionRoomMatchClockReadValue
  expect(Object.keys(request).sort()).toEqual(['callerUserId', 'competitionId', 'matchId', 'trustedTimeMs'])
  expect(Object.keys(value).sort()).toEqual(['clock', 'competitionId', 'game', 'matchId', 'matchRevision', 'matchStatus'])
  const encoded = JSON.stringify(value)
  for (const field of ['matchRevision', 'matchStatus', 'gameNumber', 'sampledAtMs', 'mainTimeRemainingMs', 'byoyomiTimeRemainingMs', 'byoyomiActive', 'awaitingAuthority', 'remainingMs']) {
    expect(encoded).toContain(`"${field}"`)
  }
  expect(encoded).not.toMatch(/"(?:binding|players|referees|clockConfig|deckSeed|serverState|ruling|byoyomiEntryOrder)"\s*:/)
  expect(JSON.parse(encoded)).toEqual(value)
  // @ts-expect-error 純 metadata getter 不屬於 generic execute 契約。
  const forbidden: CompetitionRoomExecuteRequest['operation'] = { type: 'clockCheckpointMetadata' }
  void forbidden
})

const announcement = {
  organizer: 'organizer-1',
  ruleVersion: '5.17',
  ruleset: 'fewfc-base',
  enabledRuleModules: [],
  teamSize: 2,
  initialHp: 20,
  registrationMode: 'public',
  personalDecks: 'disabled',
  registrationDeadline: 60_000,
  checkInDeadline: 120_000,
  plan: {
    initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
    swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
    rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
    exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
    stages: [{
      stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
      swissRounds: null, swissMatchPoints: null, qualification: { type: 'allEligibleTeams' },
    }],
  },
} satisfies CompetitionAnnouncement

test('competition room contract uses explicit camelCase server-only fields', () => {
  const initialize = {
    competitionId: 'competition-1',
    callerUserId: 'organizer-1',
    announcement,
    participantUserIds: ['player-1'],
    trustedTimeMs: 100,
  } satisfies CompetitionRoomInitializeRequest
  const execute = {
    competitionId: 'competition-1',
    callerUserId: 'player-1',
    commandId: 'command-1',
    expectedRevision: 1,
    operation: { type: 'registrationView' },
    trustedTimeMs: 101,
  } satisfies CompetitionRoomExecuteRequest
  const read = {
    competitionId: 'competition-1',
    callerUserId: 'player-1',
  } satisfies CompetitionRoomReadRequest

  const encoded = JSON.stringify({ initialize, execute, read })
  expect(encoded).toContain('callerUserId')
  expect(encoded).toContain('participantUserIds')
  expect(encoded).toContain('expectedRevision')
  expect(encoded).toContain('trustedTimeMs')
  expect(encoded).not.toContain('serverState')
  expect(encoded).not.toContain('authorizedPrivate')
})

test('room contract type-checks registration, clock, and tournament operations', () => {
  const operations: CompetitionRoomExecuteRequest['operation'][] = [
    {
      type: 'registrationExecute',
      commandId: 'registration-command',
      expectedRevision: 1,
      action: { type: 'open' },
    },
    {
      type: 'clockCreate',
      teams: ['team-a', 'team-b'],
      config: { mainTimeMs: 1_000, actionTimeMs: 100, outOfTurnChoiceTimeMs: null },
    },
    {
      type: 'tournamentCreate',
      config: {
        tournamentId: 'tournament-1',
        stage: {
          stageId: 'stage-1',
          format: 'singleElimination',
          teams: [{ teamId: 'team-a', seed: 1 }, { teamId: 'team-b', seed: 2 }],
          randomSeed: 7,
          matchFormat: { type: 'firstToWins', value: 1 },
          clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
          swissRounds: null,
          swissPolicy: null,
        },
      },
    },
  ]

  expect(operations.map(operation => operation.type)).toEqual([
    'registrationExecute',
    'clockCreate',
    'tournamentCreate',
  ])
})

test('match-scoped contract carries the outer identity and independent revision', () => {
  const execute = {
    competitionId: 'competition-1',
    matchId: 'match-1',
    callerUserId: 'organizer-1',
    commandId: 'match-command-1',
    expectedMatchRevision: 0,
    trustedTimeMs: 100,
    operation: {
      type: 'clockCreate' as const,
      matchId: 'match-1',
      teams: ['team-a', 'team-b'] as [string, string],
      config: { mainTimeMs: 1_000, actionTimeMs: 300, outOfTurnChoiceTimeMs: null },
    },
  } satisfies CompetitionRoomMatchExecuteRequest
  const read = {
    competitionId: 'competition-1',
    matchId: 'match-1',
    callerUserId: 'viewer-1',
  } satisfies CompetitionRoomMatchReadRequest

  const encoded = JSON.stringify({ execute, read })
  expect(encoded).toContain('matchId')
  expect(encoded).toContain('expectedMatchRevision')
  expect(encoded).not.toContain('serverState')
})

test('captain first-player contract accepts only server-derived identity and selected team fields', () => {
  const request = {
    competitionId: 'competition-1',
    matchId: 'match-2',
    callerUserId: 'losing-captain',
    commandId: 'captain-pick-1',
    expectedMatchRevision: 3,
    trustedTimeMs: 200,
    selectedFirstPlayerTeam: 'team-a',
  } satisfies CompetitionRoomAssignNextFirstPlayerByCaptainRequest
  expect(Object.keys(request).sort()).toEqual([
    'callerUserId', 'commandId', 'competitionId', 'expectedMatchRevision',
    'matchId', 'selectedFirstPlayerTeam', 'trustedTimeMs',
  ])
})

test('captain provision contract links an exact prior assignment without accepting roster or lineup data', () => {
  const request = {
    competitionId: 'competition-1',
    matchId: 'match-2',
    callerUserId: 'losing-captain',
    commandId: 'captain-provision-1',
    assignmentCommandId: 'captain-pick-1',
    expectedMatchRevision: 4,
    trustedTimeMs: 210,
  } satisfies CompetitionRoomProvisionCaptainAssignedScheduledGameRequest
  expect(Object.keys(request).sort()).toEqual([
    'assignmentCommandId', 'callerUserId', 'commandId', 'competitionId',
    'expectedMatchRevision', 'matchId', 'trustedTimeMs',
  ])
})

test('match-scoped clock pause and resume carry non-empty audit reasons', () => {
  const operations: CompetitionRoomMatchExecuteRequest['operation'][] = [
    { type: 'clockPause', matchId: 'match-1', reason: '裁判處理斷線' },
    { type: 'clockResume', matchId: 'match-1', reason: '裁判處理完成' },
  ]
  expect(JSON.stringify(operations)).toContain('clockPause')
  expect(JSON.stringify(operations)).toContain('clockResume')
  expect(JSON.stringify(operations)).toContain('reason')
  expect(JSON.stringify(operations)).not.toContain('server_state')
})

test('scheduled start contract has only server-derived match and clock settings', () => {
  const request = {
    competitionId: 'competition-1',
    stageId: 'stage-1',
    matchId: 'match-1',
    callerUserId: 'organizer-1',
    commandId: 'materialize-1',
    expectedRevision: 7,
    trustedTimeMs: 100,
    referees: [{ refereeId: 'ref-1', conflictedTeam: null }],
  } satisfies CompetitionRoomStartScheduledMatchRequest
  const value = {
    competitionId: 'competition-1',
    stageId: 'stage-1',
    matchId: 'match-1',
    revision: 8,
    matchRevision: 1,
    commandId: 'materialize-1',
    idempotent: false,
    tournamentProjection: { ok: true, operation: 'tournamentStartMatch', public: {} },
    matchProjection: null,
    clockProjection: null,
  } satisfies CompetitionRoomMaterializeScheduledMatchValue

  const encoded = JSON.stringify({ request, value })
  expect(encoded).toContain('stageId')
  expect(encoded).toContain('expectedRevision')
  expect(encoded).toContain('referees')
  expect(encoded).toContain('matchRevision')
  expect(encoded).toContain('tournamentProjection')
  expect(encoded).not.toContain('teams')
  expect(encoded).not.toContain('matchFormat')
  expect(encoded).not.toContain('clockConfig')
  expect(encoded).not.toContain('firstPlayer')
})

test('locked-roster tournament creation contract has exact camelCase request and public-only result', () => {
  const stage = {
    stageId: 'stage-1', format: 'singleElimination',
    teams: [{ teamId: 'team-a', seed: 2 }, { teamId: 'team-b', seed: 1 }],
    randomSeed: 7, matchFormat: { type: 'firstToWins', value: 2 },
    clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    swissRounds: null, swissPolicy: null,
  } satisfies StageConfig
  const request = {
    competitionId: 'competition-1', callerUserId: 'organizer-1',
    commandId: 'create-tournament-1', expectedRevision: 1,
    trustedTimeMs: 101, stage,
  } satisfies CompetitionRoomCreateTournamentRequest
  const value = {
    competitionId: request.competitionId,
    revision: 2,
    commandId: request.commandId,
    idempotent: false,
    projection: { ok: true, operation: 'tournamentCreate', public: { tournamentId: request.competitionId } },
  } satisfies CompetitionRoomCreateTournamentValue
  const encoded = JSON.stringify({ request, value })
  for (const key of ['competitionId', 'callerUserId', 'commandId', 'expectedRevision',
    'trustedTimeMs', 'stage', 'idempotent', 'projection']) {
    expect(encoded).toContain(`"${key}"`)
  }
  for (const privateKey of ['serverState', 'lockedRoster', 'captain', 'members', 'decks']) {
    expect(encoded).not.toContain(`"${privateKey}"`)
  }
})

test('terminal ingest contract contains evidence only and shares stable checkpoint digesting', async () => {
  const request = {
    competitionId: 'competition-1',
    matchId: 'match-1',
    callerUserId: 'organizer-1',
    commandId: 'terminal-1',
    expectedMatchRevision: 1,
    trustedTimeMs: 101,
    submissionId: 'submission-1',
    gameRecordId: 'game-1',
    rulingRecordId: 'ruling-1',
    startKind: { type: 'ordinaryNewGame' },
    firstPlayer: 'team-a',
    terminalEvidence: {
      type: 'canonicalGameEnded',
      conclusion: { outcome: { type: 'draw' }, causes: [] },
    },
  } satisfies CompetitionRoomTerminalIngestRequest
  const checkpoint = {
    z: 1,
    a: { nested: true },
  } as const
  const encoded = JSON.stringify(request)
  expect(encoded).toContain('terminalEvidence')
  expect(encoded).not.toContain('"ruling":')
  expect(encoded).not.toContain('"mode":')
  expect(await clockCheckpointDigest(checkpoint)).toBe(
    await clockCheckpointDigest({ a: { nested: true }, z: 1 }),
  )
})

test('game provision request exposes only trusted lineup inputs in camelCase', () => {
  const request = {
    competitionId: 'competition-1',
    matchId: 'match-1',
    callerUserId: 'organizer-1',
    commandId: 'provision-1',
    expectedMatchRevision: 2,
    trustedTimeMs: 102,
    firstPlayer: 'player-a',
    turnOrder: ['player-a', 'player-b'],
  } satisfies CompetitionRoomProvisionScheduledGameRequest
  const encoded = JSON.stringify(request)
  expect(encoded).toContain('expectedMatchRevision')
  expect(encoded).toContain('firstPlayer')
  expect(encoded).toContain('turnOrder')
  for (const forbidden of ['gameId', 'deckSeed', 'deckSnapshot', 'ruleset', 'initialHp', 'startKind']) {
    expect(encoded).not.toContain(`"${forbidden}"`)
  }
})

test('initial scheduled game contract excludes caller-controlled first-player and room binding data', () => {
  const request = {
    competitionId: 'competition-1',
    matchId: 'match-1',
    callerUserId: 'organizer-1',
    commandId: 'initial-game-1',
    expectedMatchRevision: 1,
    trustedTimeMs: 200,
  } satisfies CompetitionRoomProvisionInitialScheduledGameRequest
  const encoded = JSON.stringify(request)
  expect(Object.keys(request).sort()).toEqual([
    'callerUserId', 'commandId', 'competitionId', 'expectedMatchRevision', 'matchId', 'trustedTimeMs',
  ])
  expect(encoded).not.toMatch(/firstPlayer|team|turnOrder|binding|deckSeed|gameId/)
})

test('assigned scheduled game contract has an exact server-derived request schema', () => {
  const request = {
    competitionId: 'competition-1',
    matchId: 'match-1',
    callerUserId: 'organizer-1',
    commandId: 'assigned-game-1',
    expectedMatchRevision: 4,
    trustedTimeMs: 300,
  } satisfies CompetitionRoomProvisionAssignedScheduledGameRequest
  expect(Object.keys(request).sort()).toEqual([
    'callerUserId', 'commandId', 'competitionId', 'expectedMatchRevision', 'matchId', 'trustedTimeMs',
  ])
  expect(JSON.stringify(request)).not.toMatch(/firstPlayer|team|turnOrder|binding|deckSeed|gameId/)
})

test('participant game query has an exact identity request and a safe minimal result', () => {
  const request = {
    competitionId: 'competition-1',
    matchId: 'match-1',
    callerUserId: 'player-a',
  } satisfies CompetitionRoomParticipantGameReadRequest
  const value = {
    competitionId: request.competitionId,
    matchId: request.matchId,
    matchRevision: 7,
    game: { gameId: 'competition-game-1', gameNumber: 2 },
  } satisfies CompetitionRoomParticipantGameReadValue

  expect(Object.keys(request).sort()).toEqual(['callerUserId', 'competitionId', 'matchId'])
  expect(Object.keys(value).sort()).toEqual(['competitionId', 'game', 'matchId', 'matchRevision'])
  expect(Object.keys(value.game!).sort()).toEqual(['gameId', 'gameNumber'])
  expect(JSON.stringify(value)).not.toMatch(
    /"(?:binding|roster|decks|deckSnapshot|deckSeed|history|commandId|callerUserId|serverState)"\s*:/,
  )
})

test('E6d1b tournament query contract is exact and carries only the complete public tournament', () => {
  const request = {
    competitionId: 'competition-1',
    callerUserId: 'referee-1',
  } satisfies CompetitionRoomTournamentReadRequest
  const value = {
    competitionId: request.competitionId,
    revision: 9,
    projection: {
      ok: true,
      operation: 'tournamentView',
      public: {
        tournamentId: 'competition-1',
        stages: [{ config: { clockConfig: { mainTimeMs: 300_000 } }, rounds: [] }],
      },
    },
  } satisfies CompetitionRoomTournamentReadValue

  expect(Object.keys(request).sort()).toEqual(['callerUserId', 'competitionId'])
  expect(Object.keys(value).sort()).toEqual(['competitionId', 'projection', 'revision'])
  expect(Object.keys(value.projection).sort()).toEqual(['ok', 'operation', 'public'])
  expect(JSON.stringify(value)).toContain('clockConfig')
  expect(JSON.stringify(value)).not.toMatch(
    /"(?:serverState|private|roster|captain|members|decks|binding|history|commandId|callerUserId)"\s*:/,
  )
})

// 此組只驗共同 wire；fixture 不冒充持久 manifest／全房 ACK／canonical start 證據。
function roundAnnouncement(): CompetitionRoomAnnounceRoundRequest {
  return {
    competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 2,
    callerUserId: 'organizer-1', commandId: 'announce-1', expectedRevision: 10,
    trustedTimeMs: 1_000_000, roundStartMs: 1_600_000,
    refereeAssignments: [{ matchId: 'match-1', referees: [{ refereeId: 'ref-1', conflictedTeam: null }] }],
  }
}

function roundPublication(): CompetitionRoomRoundPublication {
  return {
    roundRef: { competitionId: 'competition-1', stageId: 'stage-1', roundNumber: 2, roundGenerationId: 'generation-1' },
    roundStartMs: 1_600_000, publicationVersion: 1, status: 'materializing',
    releaseDecisionId: null, requiresReferee: false,
    rooms: [{ matchId: 'match-1', gameId: 'game-1', ackConfirmedAtMs: null }],
  }
}

test('round-release parent announce/read wire is closed, normalized and public-only', () => {
  const request = roundAnnouncement()
  const read = {
    competitionId: request.competitionId, stageId: request.stageId,
    roundNumber: request.roundNumber, callerUserId: request.callerUserId,
  }
  const publication = roundPublication()
  const announced = {
    competitionId: request.competitionId, revision: 11,
    commandId: request.commandId, idempotent: false, publication,
  }
  expect(isCompetitionRoomAnnounceRoundRequest(request)).toBe(true)
  expect(isCompetitionRoomRoundPublicationReadRequest(read)).toBe(true)
  expect(isCompetitionRoomAnnounceRoundValue(announced, request)).toBe(true)
  expect(isCompetitionRoomRoundPublicationReadValue({ competitionId: read.competitionId, revision: 11, publication }, read)).toBe(true)
  expect(isCompetitionRoomRoundPublicationReadValue({ competitionId: read.competitionId, revision: 11, publication: null }, read)).toBe(true)
  for (const key of ['binding', 'deckSeed', 'firstPlayer', 'turnOrder', 'gameId', 'roundRef', 'organizerUserId', 'initialHp']) {
    expect(isCompetitionRoomAnnounceRoundRequest({ ...request, [key]: 'forged' })).toBe(false)
  }
  expect(isCompetitionRoomAnnounceRoundRequest({ ...request, competitionId: ' competition-1 ' })).toBe(false)
  expect(isCompetitionRoomAnnounceRoundRequest({ ...request, roundNumber: 0 })).toBe(false)
  expect(isCompetitionRoomAnnounceRoundRequest({ ...request, refereeAssignments: [
    { matchId: 'match-1', referees: [{ refereeId: 'ref-1', conflictedTeam: null, private: true }] },
  ] })).toBe(false)
  expect(isCompetitionRoomAnnounceRoundRequest({ ...request, refereeAssignments: [
    ...request.refereeAssignments, ...request.refereeAssignments,
  ] })).toBe(false)
  expect(isCompetitionRoomRoundPublicationReadRequest({ ...read, trustedTimeMs: 1 })).toBe(false)
  expect(isCompetitionRoomAnnounceRoundValue({ ...announced, binding: {} }, request)).toBe(false)
  expect(isCompetitionRoomAnnounceRoundValue({ ...announced, revision: 12 }, request)).toBe(false)
  expect(isCompetitionRoomAnnounceRoundValue({ ...announced, publication: { ...publication, roundStartMs: 1_700_000 } }, request)).toBe(false)
  expect(isCompetitionRoomRoundPublicationReadValue({ competitionId: read.competitionId, revision: 11,
    publication: { ...publication, roundRef: { ...publication.roundRef, stageId: 'other-stage' } } }, read)).toBe(false)
  expect(JSON.parse(JSON.stringify(announced))).toEqual(announced)
  expect(JSON.stringify(announced)).not.toMatch(/"(?:binding|deckSeed|players|referees|serverState|private)"\s*:/)
})

test('round-release parent publication keeps late confirmation visible without inventing release', () => {
  const publication = roundPublication()
  expect(isCompetitionRoomRoundPublication({ ...publication, rooms: [
    { ...publication.rooms[0], ackConfirmedAtMs: publication.roundStartMs - 299_999 },
  ] })).toBe(true)
  expect(isCompetitionRoomRoundPublication({ ...publication, status: 'released' })).toBe(false)
  expect(isCompetitionRoomRoundPublication({ ...publication, status: ['released'], releaseDecisionId: 'release-1' })).toBe(false)
  expect(isCompetitionRoomRoundPublication({ ...publication, rooms: [] })).toBe(false)
  expect(isCompetitionRoomRoundPublication({ ...publication, rooms: [...publication.rooms, ...publication.rooms] })).toBe(false)
  expect(isCompetitionRoomRoundPublication({ ...publication, status: 'released', releaseDecisionId: 'release-1',
    rooms: [{ ...publication.rooms[0], ackConfirmedAtMs: publication.roundStartMs - 300_000 }] })).toBe(true)
  expect(isCompetitionRoomRoundPublication({ ...publication, status: 'released', releaseDecisionId: 'release-1',
    rooms: [{ ...publication.rooms[0], ackConfirmedAtMs: publication.roundStartMs - 299_999 }] })).toBe(false)
  for (const status of ['quiescing', 'retired']) {
    expect(isCompetitionRoomRoundPublication({ ...publication, status, releaseDecisionId: 'release-1' })).toBe(true)
  }
})

test('round-release parent guards reject hidden, symbol and getter payloads without executing getters', () => {
  const request = roundAnnouncement()
  let calls = 0
  const getter = Object.defineProperty({ ...request }, 'roundStartMs', {
    enumerable: true, get: () => { calls += 1; return 1_600_000 },
  })
  expect(isCompetitionRoomAnnounceRoundRequest(getter)).toBe(false)
  expect(calls).toBe(0)
  expect(isCompetitionRoomAnnounceRoundRequest(Object.defineProperty({ ...request }, 'private', { value: true }))).toBe(false)
  expect(isCompetitionRoomAnnounceRoundRequest({ ...request, [Symbol('extra')]: true })).toBe(false)
  expect(isCompetitionRoomAnnounceRoundRequest(new Proxy(request, { ownKeys: () => { throw new Error('reflection') } }))).toBe(false)
  expect(isCompetitionRoomRoundPublication({ ...roundPublication(), binding: {} })).toBe(false)
})

test('round-release initial receiver uses persisted release expectation and the existing nine-field clock ACK', () => {
  const publication = roundPublication()
  const bindingRef = {
    roundRef: publication.roundRef, roundStartMs: publication.roundStartMs,
    matchId: 'match-1', gameId: 'game-1', gameNumber: 1, bindingDigest: 'a'.repeat(64),
  }
  const expected = { bindingRef, commandId: 'release-1' }
  const request: CompetitionRoomInitialGameStartedRequest = {
    competitionId: 'competition-1', matchId: 'match-1', callerUserId: 'organizer-1',
    commandId: 'initial-start-1', expectedMatchRevision: 2,
    evidence: { bindingRef, releaseDecisionId: 'release-1', gameInstanceId: 'instance-1',
      startSequence: 0, startedAtMs: publication.roundStartMs, initialWindow: { type: 'preparation' } },
  }
  expect(isCompetitionRoomInitialGameStartedRequest(request)).toBe(true)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, evidence: { ...request.evidence,
    initialWindow: { type: 'action', team: 'team-a', turn: 1 } } })).toBe(true)
  expect(isCompetitionGameInitialStartEvidence(request.evidence, expected)).toBe(true)
  expect(isCompetitionGameInitialStartEvidence(request.evidence, { ...expected, commandId: 'different-release' })).toBe(false)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, matchId: 'other-match' })).toBe(false)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, trustedTimeMs: 1_600_100 })).toBe(false)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, evidence: { ...request.evidence,
    startedAtMs: publication.roundStartMs - 1 } })).toBe(true)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, evidence: { ...request.evidence,
    startedAtMs: -1 } })).toBe(false)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, evidence: { ...request.evidence,
    initialWindow: { type: 'preparation', team: 'team-a' } } })).toBe(false)
  const ack: CompetitionRoomMatchExecuteValue = {
    competitionId: 'competition-1', matchId: 'match-1', matchRevision: 3,
    commandId: request.commandId, idempotent: false, timeoutEvidence: null,
    nextClockDeadlineMs: 1_900_000, matchProjection: null,
    clockProjection: { ok: true, operation: 'clockBeginPreparation', public: {} },
  }
  expect(Object.keys(ack).sort()).toEqual([
    'clockProjection', 'commandId', 'competitionId', 'idempotent', 'matchId',
    'matchProjection', 'matchRevision', 'nextClockDeadlineMs', 'timeoutEvidence',
  ])
  expect(JSON.parse(JSON.stringify({ request, ack }))).toEqual({ request, ack })
})

test('round-release parent wire rejects missing fields, malformed ranges and nested dense-array faults', () => {
  const request = roundAnnouncement()
  for (const key of Object.keys(request)) {
    const missing: Record<string, unknown> = { ...request }
    delete missing[key]
    expect(isCompetitionRoomAnnounceRoundRequest(missing)).toBe(false)
  }
  for (const roundNumber of [0, -1, 1.5, 0x1_0000_0000, Number.NaN]) {
    expect(isCompetitionRoomAnnounceRoundRequest({ ...request, roundNumber })).toBe(false)
  }
  for (const roundStartMs of [0, 299_999, 300_000.5, 8_640_000_000_000_001, Number.POSITIVE_INFINITY]) {
    expect(isCompetitionRoomAnnounceRoundRequest({ ...request, roundStartMs })).toBe(false)
  }
  expect(isCompetitionRoomAnnounceRoundRequest({ ...request, refereeAssignments: Array(1) })).toBe(false)
  expect(isCompetitionRoomAnnounceRoundRequest({ ...request, refereeAssignments: [{ matchId: 'match-1', referees: Array(1) }] })).toBe(false)
  const duplicated = { refereeId: 'ref-1', conflictedTeam: null }
  expect(isCompetitionRoomAnnounceRoundRequest({ ...request, refereeAssignments: [
    { matchId: 'match-1', referees: [duplicated, duplicated] },
  ] })).toBe(false)
  const publication = roundPublication()
  for (const key of Object.keys(publication)) {
    const missing: Record<string, unknown> = { ...publication }
    delete missing[key]
    expect(isCompetitionRoomRoundPublication(missing)).toBe(false)
  }
  expect(isCompetitionRoomRoundPublication({ ...publication, rooms: Array(1) })).toBe(false)
  expect(isCompetitionRoomRoundPublication({ ...publication, publicationVersion: 0 })).toBe(false)
})

test('round-release initial wire rejects altered identity, later-game evidence and caller clock/private fields', () => {
  const publication = roundPublication()
  const bindingRef = { roundRef: publication.roundRef, roundStartMs: publication.roundStartMs,
    matchId: 'match-1', gameId: 'game-1', gameNumber: 1, bindingDigest: 'a'.repeat(64) }
  const request: CompetitionRoomInitialGameStartedRequest = {
    competitionId: 'competition-1', matchId: 'match-1', callerUserId: 'organizer-1',
    commandId: 'initial-start-1', expectedMatchRevision: 2,
    evidence: { bindingRef, releaseDecisionId: 'release-1', gameInstanceId: 'instance-1',
      startSequence: 0, startedAtMs: publication.roundStartMs, initialWindow: { type: 'action', team: 'team-a', turn: 1 } },
  }
  for (const key of Object.keys(request)) {
    const missing: Record<string, unknown> = { ...request }
    delete missing[key]
    expect(isCompetitionRoomInitialGameStartedRequest(missing)).toBe(false)
  }
  for (const key of ['operation', 'binding', 'deckSeed', 'trustedTimeMs', 'arrivalTimeMs', 'parentAuditTimeMs']) {
    expect(isCompetitionRoomInitialGameStartedRequest({ ...request, [key]: 'forged' })).toBe(false)
  }
  for (const startSequence of [-1, 1.5, Number.MAX_SAFE_INTEGER + 1]) {
    expect(isCompetitionRoomInitialGameStartedRequest({ ...request, evidence: { ...request.evidence, startSequence } })).toBe(false)
  }
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, expectedMatchRevision: 0 })).toBe(false)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, competitionId: 'other-competition' })).toBe(false)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, evidence: { ...request.evidence,
    bindingRef: { ...bindingRef, gameNumber: 2 } } })).toBe(false)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, evidence: { ...request.evidence, serverState: {} } })).toBe(false)
  expect(isCompetitionRoomInitialGameStartedRequest({ ...request, evidence: { ...request.evidence,
    initialWindow: { type: 'action', team: 'team-a', turn: 1, private: {} } } })).toBe(false)
})

test('round-release wire checks are pure and accept frozen JSON data without accepting hidden result fields', () => {
  const request = roundAnnouncement()
  const publication = roundPublication()
  const read = { competitionId: request.competitionId, stageId: request.stageId,
    roundNumber: request.roundNumber, callerUserId: request.callerUserId }
  const value = { competitionId: request.competitionId, revision: 11, publication }
  const before = JSON.stringify({ request, publication, read, value })
  Object.freeze(request)
  Object.freeze(publication)
  Object.freeze(publication.rooms)
  Object.freeze(read)
  Object.freeze(value)
  expect(isCompetitionRoomAnnounceRoundRequest(request)).toBe(true)
  expect(isCompetitionRoomRoundPublicationReadValue(value, read)).toBe(true)
  expect(isCompetitionRoomRoundPublicationReadValue({ ...value, private: {} }, read)).toBe(false)
  expect(isCompetitionRoomRoundPublicationReadValue(Object.defineProperty({ ...value }, 'private', { value: {} }), read)).toBe(false)
  expect(isCompetitionRoomRoundPublicationReadValue({ ...value, [Symbol('extra')]: true }, read)).toBe(false)
  let calls = 0
  const malicious = Object.defineProperty({ ...value }, 'publication', {
    enumerable: true, get: () => { calls += 1; return publication },
  })
  expect(isCompetitionRoomRoundPublicationReadValue(malicious, read)).toBe(false)
  expect(calls).toBe(0)
  expect(JSON.stringify({ request, publication, read, value })).toBe(before)
})

test('round-release announce success and retry keep the immutable initial snapshot, unlike current publication reads', () => {
  const request = roundAnnouncement()
  const publication = roundPublication()
  const read = { competitionId: request.competitionId, stageId: request.stageId,
    roundNumber: request.roundNumber, callerUserId: request.callerUserId }
  const initial = { competitionId: request.competitionId, revision: request.expectedRevision + 1,
    commandId: request.commandId, publication }
  for (const idempotent of [false, true]) {
    expect(isCompetitionRoomAnnounceRoundValue({ ...initial, idempotent }, request)).toBe(true)
    expect(isCompetitionRoomAnnounceRoundValue({ ...initial, idempotent }, {
      ...request, trustedTimeMs: request.roundStartMs + 1,
    })).toBe(true)
  }
  const room = publication.rooms[0]
  if (!room) throw new Error('共同 wire fixture 缺少房間')
  // 單欄故障與合法 current 狀態都不許取代原 receipt；不冒充真 proof 或持久 generation。
  const currentSnapshots: CompetitionRoomRoundPublication[] = [
    { ...publication, publicationVersion: 2 },
    { ...publication, requiresReferee: true },
    { ...publication, rooms: [{ ...room, ackConfirmedAtMs: publication.roundStartMs - 300_000 }] },
    { ...publication, rooms: [{ ...room, ackConfirmedAtMs: publication.roundStartMs - 299_999 }] },
    { ...publication, status: 'quiescing' },
    { ...publication, status: 'retired' },
    { ...publication, status: 'released', releaseDecisionId: 'release-1',
      rooms: [{ ...room, ackConfirmedAtMs: publication.roundStartMs - 300_000 }] },
  ]
  for (const current of currentSnapshots) {
    expect(isCompetitionRoomRoundPublicationReadValue({ competitionId: read.competitionId,
      revision: 20, publication: current }, read)).toBe(true)
    for (const idempotent of [false, true]) {
      expect(isCompetitionRoomAnnounceRoundValue({ ...initial, publication: current, idempotent }, request)).toBe(false)
    }
  }
  expect(isCompetitionRoomAnnounceRoundValue({ ...initial, idempotent: true,
    publication: { ...publication, releaseDecisionId: 'forged-release' } }, request)).toBe(false)
  expect(isCompetitionRoomRoundPublicationReadValue({ competitionId: read.competitionId,
    revision: 20, publication: { ...publication, releaseDecisionId: 'forged-release' } }, read)).toBe(false)
})

test('round-release announcement audit serializes one truthful operation with exact public camelCase fields', () => {
  const record: CompetitionRoomAuditRecord = {
    sequence: 1, matchId: null, actorUserId: 'organizer-1', role: 'organizer', operationType: 'announceRound',
    commandId: 'announce-1', revision: 11, outcomeCode: 'success', trustedTimeMs: 1_000_000, reason: null,
  }
  expect(Object.keys(record).sort()).toEqual([
    'actorUserId', 'commandId', 'matchId', 'operationType', 'outcomeCode', 'reason',
    'revision', 'role', 'sequence', 'trustedTimeMs',
  ])
  expect(JSON.parse(JSON.stringify(record))).toEqual({
    sequence: 1, matchId: null, actorUserId: 'organizer-1', role: 'organizer', operationType: 'announceRound',
    commandId: 'announce-1', revision: 11, outcomeCode: 'success', trustedTimeMs: 1_000_000, reason: null,
  })
  expect(JSON.stringify(record)).not.toMatch(/"(?:binding|manifest|players|deckSeed|serverState|private)"\s*:/)
})
