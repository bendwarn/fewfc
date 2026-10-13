import { expect, test } from 'bun:test'
import type { CompetitionRegistrationState } from './competition-registration'
import {
  parseCompetitionTournamentViewResponse,
  type TournamentFormatView,
  type TournamentRoundKindView,
} from './competition-tournament-view'
import {
  buildCompetitionRoundPublicationIntent,
  createCompetitionRoundPublicationContext,
  findCurrentCompetitionRoundPublicationTarget,
  keepCompetitionRoundPublicationIntent,
  parseCompetitionRoundAnnouncementAck,
  parseCompetitionRoundPublicationReadSnapshot,
  type CompetitionRoundPublicationIntent,
} from './competition-round-publication'

const competitionId = 'competition-a'
const organizerId = 'organizer-a'
const stageId = 'stage-a'
const roundStartMs = new Date(2030, 0, 2, 12, 10).getTime()
const expectedMatchIds = ['match-a', 'match-b']

function registration(overrides: Partial<CompetitionRegistrationState> = {}): CompetitionRegistrationState {
  return {
    competitionId, revision: 4, organizer: organizerId, ruleVersion: '5.17', ruleset: 'base',
    enabledRuleModules: [], teamSize: 1, initialHp: 200, registrationMode: 'public', personalDecks: 'disabled',
    registrationDeadline: 1_893_456_000_000, checkInDeadline: 1_893_456_300_000, lifecycle: 'locked',
    plan: {
      initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
      swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
      rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
      exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
      stages: [{ stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: null, swissPolicy: null, qualification: { type: 'allEligibleTeams' } }],
    },
    teams: [], ownTeam: null, ...overrides,
  }
}

function match(
  matchId: string, roundNumber: number, status: 'pending' | 'completed' | 'inProgress' | 'cancelled',
  participants: string[], options: {
    result?: unknown, started?: boolean, cancellationReason?: string | null,
    sequence?: number, kind?: TournamentRoundKindView,
  } = {},
) {
  return {
    matchId, stageId, roundNumber, sequence: options.sequence ?? 0, kind: options.kind ?? 'main', participants, status,
    result: options.result ?? null, cancellationReason: options.cancellationReason ?? null,
    started: options.started ?? false,
  }
}

function finishedRoundRobinStage(id: string) {
  const teams = [{ teamId: 'team-a', seed: 1 }, { teamId: 'team-b', seed: 2 }]
  return {
    config: {
      stageId: id, format: 'roundRobin', teams, randomSeed: 11,
      matchFormat: { type: 'fixedGames', value: 1 },
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 15_000 },
      swissRounds: null, swissPolicy: null,
    },
    status: 'finished',
    rounds: [{ roundNumber: 1, kind: 'main', matches: [{
      matchId: `${id}-match-1`, stageId: id, roundNumber: 1, sequence: 0, kind: 'main',
      participants: ['team-a', 'team-b'], status: 'completed',
      result: { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 1, loserGames: 0 },
      cancellationReason: null, started: true,
    }] }],
    standings: [
      { teamId: 'team-a', matchPoints: '3', wins: 1, draws: 0, losses: 0, gameWins: 1, gameLosses: 0,
        netGameWins: 1, opponentWinRateBasisPoints: null, dropped: false, rank: 1 },
      { teamId: 'team-b', matchPoints: '0', wins: 0, draws: 0, losses: 1, gameWins: 0, gameLosses: 1,
        netGameWins: -1, opponentWinRateBasisPoints: null, dropped: false, rank: 2 },
    ],
    corrections: [], lastScheduleError: null, scheduleGeneration: 0, supersededRounds: [],
    teams: { 'team-a': { seed: 1, dropped: false, hadBye: false }, 'team-b': { seed: 2, dropped: false, hadBye: false } },
    doubleState: null,
  }
}

function tournament(
  rounds: unknown[], options: {
    revision?: number, activeStageCount?: number, finishedStageCount?: number,
    warning?: boolean, format?: TournamentFormatView,
  } = {},
) {
  const format = options.format ?? 'roundRobin'
  const stage = (id: string, status: 'active' | 'finished') => ({
    config: {
      stageId: id, format,
      teams: [
        { teamId: 'team-a', seed: 1 }, { teamId: 'team-b', seed: 2 }, { teamId: 'team-c', seed: 3 },
        { teamId: 'team-d', seed: 4 }, { teamId: 'team-e', seed: 5 },
      ],
      randomSeed: 77,
      matchFormat: {
        type: format === 'singleElimination' || format === 'doubleElimination' ? 'firstToWins' : 'fixedGames',
        value: 1,
      },
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 15_000 },
      swissRounds: null, swissPolicy: null,
    },
    status, rounds: id === stageId ? rounds : [], standings: [], corrections: [],
    lastScheduleError: options.warning && id === stageId ? 'stale schedule' : null,
    scheduleGeneration: 0, supersededRounds: [],
    teams: {
      'team-a': { seed: 1, dropped: false, hadBye: false },
      'team-b': { seed: 2, dropped: false, hadBye: false },
      'team-c': { seed: 3, dropped: false, hadBye: false },
      'team-d': { seed: 4, dropped: false, hadBye: false },
      'team-e': { seed: 5, dropped: false, hadBye: false },
    }, doubleState: format === 'doubleElimination'
      ? {
          phase: 'losers', winnersPool: ['team-a', 'team-c'], losersWaiting: ['team-b', 'team-d'],
          lowerPool: ['team-e'], winnersChampion: null, losersChampion: null, grandFinalWinner: null,
        }
      : null,
  })
  const activeCount = options.activeStageCount ?? 1
  const stages: unknown[] = [stage(stageId, 'active')]
  for (let index = 1; index < activeCount; index += 1) stages.push(stage(`stage-${index}`, 'active'))
  if (activeCount === 0) stages[0] = stage(stageId, 'finished')
  for (let index = 0; index < (options.finishedStageCount ?? 0); index += 1) {
    stages.push(finishedRoundRobinStage(`finished-stage-${index + 1}`))
  }
  return parseCompetitionTournamentViewResponse({
    competitionId, revision: options.revision ?? 17,
    projection: { ok: true, operation: 'tournamentView', public: { tournamentId: competitionId, stages } },
  }, competitionId)
}

function currentRound() {
  return [{ roundNumber: 1, kind: 'main', matches: [
    match('match-a', 1, 'pending', ['team-a', 'team-b']),
    match('match-b', 1, 'pending', ['team-c', 'team-d']),
    match('current-bye', 1, 'completed', ['team-e'], { result: { type: 'bye', teamId: 'team-e' } }),
  ] }]
}

/** 五隊循環初始化一次產生五個 main round；每輪固定兩場和一個 canonical bye。 */
function fiveTeamRoundRobinView(roundOptions: {
  completedThrough?: number, inProgressRound?: number, partialRound?: number,
} = {}) {
  const schedule = [
    { pairs: [['team-b', 'team-e'], ['team-c', 'team-d']], bye: 'team-a' },
    { pairs: [['team-a', 'team-e'], ['team-b', 'team-c']], bye: 'team-d' },
    { pairs: [['team-a', 'team-d'], ['team-e', 'team-c']], bye: 'team-b' },
    { pairs: [['team-a', 'team-c'], ['team-d', 'team-b']], bye: 'team-e' },
    { pairs: [['team-a', 'team-b'], ['team-d', 'team-e']], bye: 'team-c' },
  ] as const
  const rounds = schedule.map((entry, index) => {
    const roundNumber = index + 1
    const completed = roundNumber <= (roundOptions.completedThrough ?? 0)
    const inProgress = roundNumber === roundOptions.inProgressRound
    const partial = roundNumber === roundOptions.partialRound
    const matches = entry.pairs.map((participants, pairIndex) => {
      const isCompleted = completed || (partial && pairIndex === 0)
      const isInProgress = inProgress && pairIndex === 0
      return match(
        `rr-r${roundNumber}-match-${pairIndex + 1}`,
        roundNumber,
        isCompleted ? 'completed' : isInProgress ? 'inProgress' : 'pending',
        [...participants],
        {
          sequence: pairIndex,
          started: isCompleted || isInProgress,
          result: isCompleted ? { type: 'draw', firstGames: 1, secondGames: 1 } : null,
        },
      )
    })
    matches.push(match(`rr-r${roundNumber}-bye`, roundNumber, 'completed', [entry.bye], {
      sequence: 2, result: { type: 'bye', teamId: entry.bye },
    }))
    return { roundNumber, kind: 'main', matches }
  })
  return rounds
}

function context(overrides: {
  id?: string, user?: string, registration?: CompetitionRegistrationState,
} = {}) {
  return createCompetitionRoundPublicationContext(
    overrides.id ?? competitionId, overrides.user ?? organizerId, overrides.registration ?? registration(),
  )
}

function intent(overrides: {
  rounds?: unknown[], input?: string, now?: number, viewRevision?: number, commandId?: string,
} = {}): CompetitionRoundPublicationIntent {
  const view = tournament(overrides.rounds ?? currentRound(), { revision: overrides.viewRevision ?? 17 })
  const value = context()
  if (!value) throw new Error('合法主辦 context fixture 無法建立')
  return buildCompetitionRoundPublicationIntent(
    value, view, overrides.input ?? '2030-01-02T12:10', overrides.now ?? new Date(2030, 0, 2, 12, 5).getTime(),
    () => overrides.commandId ?? '8f4d34de-2b13-4bce-9f10-2a9f648db1c8',
  )
}

function publication(overrides: Record<string, unknown> = {}) {
  return {
    roundRef: { competitionId, stageId, roundNumber: 1, roundGenerationId: 'generation-a' },
    roundStartMs, publicationVersion: 1, status: 'materializing', releaseDecisionId: null,
    requiresReferee: false,
    rooms: expectedMatchIds.map((matchId, index) => ({ matchId, gameId: `game-${index}`, ackConfirmedAtMs: null })),
    ...overrides,
  }
}

function readValue(pub: unknown = publication(), overrides: Record<string, unknown> = {}) {
  return { competitionId, revision: 17, publication: pub, ...overrides }
}

function ack(value: CompetitionRoundPublicationIntent, pub: unknown = publication(), overrides: Record<string, unknown> = {}) {
  return {
    competitionId, revision: value.body.expectedRevision + 1, commandId: value.body.commandId,
    idempotent: false, publication: pub, ...overrides,
  }
}

function expectBlocked(rounds: unknown[], options: { activeStageCount?: number, warning?: boolean } = {}) {
  expect(findCurrentCompetitionRoundPublicationTarget(tournament(rounds, options))).toMatchObject({ kind: 'blocked' })
}

test('輪次公告權限要求同賽事、已鎖定且原 organizer；registration revision 不成為命令版本', () => {
  const value = context()
  expect(value).toEqual({ competitionId, sessionUserId: organizerId })
  expect(context({ user: 'player-a' })).toBeNull()
  expect(context({ id: 'other-competition' })).toBeNull()
  expect(context({ registration: registration({ lifecycle: 'open' }) })).toBeNull()

  const command = intent({ viewRevision: 17 })
  expect(command.body.expectedRevision).toBe(17)
  expect(command.body.expectedRevision).not.toBe(registration().revision)
  expect(command.body.refereeAssignments).toEqual([])
  expect(Object.keys(command.body)).toEqual(['commandId', 'expectedRevision', 'roundStartMs', 'refereeAssignments'])
  expect(Object.isFrozen(command)).toBe(true)
  expect(Object.isFrozen(command.body.refereeAssignments)).toBe(true)
})

test('只取單一進行中階段最早未完成輪的完整待公告房間集；前輪須完成，已完成 bye 可略過', () => {
  const prior = { roundNumber: 1, kind: 'main', matches: [
    match('prior-match', 1, 'completed', ['team-a', 'team-b'], { result: { type: 'draw', firstGames: 1, secondGames: 1 }, started: true }),
    match('prior-match-two', 1, 'completed', ['team-c', 'team-d'], { result: { type: 'draw', firstGames: 1, secondGames: 1 }, started: true }),
    match('prior-bye', 1, 'completed', ['team-e'], { result: { type: 'bye', teamId: 'team-e' } }),
  ] }
  const next = { roundNumber: 2, kind: 'main', matches: [
    match('match-a', 2, 'pending', ['team-a', 'team-c']),
    match('match-b', 2, 'pending', ['team-b', 'team-d']),
    match('current-bye', 2, 'completed', ['team-e'], { result: { type: 'bye', teamId: 'team-e' } }),
  ] }
  const target = findCurrentCompetitionRoundPublicationTarget(tournament([prior, next]))
  expect(target).toEqual({ kind: 'ready', target: { stageId, roundNumber: 2, matchIds: ['match-a', 'match-b'] } })
  expect(findCurrentCompetitionRoundPublicationTarget(tournament(currentRound(), { activeStageCount: 2 })))
    .toMatchObject({ kind: 'blocked' })
  expect(findCurrentCompetitionRoundPublicationTarget(tournament(currentRound(), { activeStageCount: 0 })))
    .toMatchObject({ kind: 'blocked' })
  expect(findCurrentCompetitionRoundPublicationTarget(tournament(currentRound(), { warning: true })))
    .toMatchObject({ kind: 'blocked' })
  expect(findCurrentCompetitionRoundPublicationTarget(tournament([
    { roundNumber: 1, kind: 'main', matches: [match('unfinished', 1, 'pending', ['team-a', 'team-b'])] },
    { roundNumber: 2, kind: 'main', matches: [match('match-a', 2, 'pending', ['team-a', 'team-c'])] },
  ]))).toEqual({
    kind: 'ready', target: { stageId, roundNumber: 1, matchIds: ['unfinished'] },
  })
  expectBlocked([
    { roundNumber: 1, kind: 'main', matches: [
      match('started', 1, 'inProgress', ['team-a', 'team-b'], { started: true }),
    ] },
  ])
})

test('輪次公告先取 core 預生成循環賽的最早未完成輪，不跳到未來輪', () => {
  const allPreGenerated = tournament(fiveTeamRoundRobinView())
  expect(allPreGenerated.stages[0]?.rounds).toHaveLength(5)
  expect(findCurrentCompetitionRoundPublicationTarget(allPreGenerated)).toEqual({
    kind: 'ready', target: { stageId, roundNumber: 1, matchIds: ['rr-r1-match-1', 'rr-r1-match-2'] },
  })

  const afterOpeningRound = tournament(fiveTeamRoundRobinView({ completedThrough: 1 }))
  expect(findCurrentCompetitionRoundPublicationTarget(afterOpeningRound)).toEqual({
    kind: 'ready', target: { stageId, roundNumber: 2, matchIds: ['rr-r2-match-1', 'rr-r2-match-2'] },
  })

  expect(findCurrentCompetitionRoundPublicationTarget(
    tournament(fiveTeamRoundRobinView({ completedThrough: 1, inProgressRound: 2 })),
  )).toMatchObject({ kind: 'blocked' })
  expect(findCurrentCompetitionRoundPublicationTarget(
    tournament(fiveTeamRoundRobinView({ completedThrough: 1, partialRound: 2 })),
  )).toMatchObject({ kind: 'blocked' })

  expect(findCurrentCompetitionRoundPublicationTarget(tournament([
    { roundNumber: 1, kind: 'main', matches: [
      match('round-one-result-missing', 1, 'completed', ['team-a', 'team-b'], { started: true }),
    ] },
    { roundNumber: 2, kind: 'main', matches: [
      match('round-two-future', 2, 'pending', ['team-c', 'team-d']),
    ] },
  ]))).toMatchObject({ kind: 'blocked' })
})

test('輪次公告不合併相同數字的不同 bracket kind', () => {
  const parsed = tournament([
    { roundNumber: 1, kind: 'winners', matches: [match(
      'de-r1-winners-a', 1, 'completed', ['team-a', 'team-b'], {
        kind: 'winners', result: { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 1, loserGames: 0 },
        started: true,
      },
    )] },
    { roundNumber: 2, kind: 'winners', matches: [match(
      'de-r2-winners-a', 2, 'pending', ['team-a', 'team-c'], { kind: 'winners' },
    )] },
    { roundNumber: 2, kind: 'losers', matches: [match(
      'de-r2-losers-a', 2, 'pending', ['team-b', 'team-d'], { kind: 'losers' },
    )] },
  ], { format: 'doubleElimination' })

  expect(parsed.stages[0]?.rounds.map(round => [round.kind, round.roundNumber])).toEqual([
    ['winners', 1], ['winners', 2], ['losers', 2],
  ])
  expect(parsed.stages[0]?.rounds.slice(1).flatMap(round => round.matches.map(entry => entry.matchId)))
    .toEqual(['de-r2-winners-a', 'de-r2-losers-a'])
  expect(findCurrentCompetitionRoundPublicationTarget(parsed)).toMatchObject({ kind: 'blocked' })
})

test('T 僅接受嚴格 HH:mm，使用 current global revision 並要求至少五分鐘而不自動順延', () => {
  const now = new Date(2030, 0, 2, 12, 5).getTime()
  const command = intent({ now, input: '2030-01-02T12:10', viewRevision: 23 })
  expect(command.body).toEqual({
    commandId: '8f4d34de-2b13-4bce-9f10-2a9f648db1c8', expectedRevision: 23,
    roundStartMs: new Date(2030, 0, 2, 12, 10).getTime(), refereeAssignments: [],
  })
  expect(command.path).toBe('/api/competitions/competition-a/stages/stage-a/rounds/1/publication')
  for (const invalid of [
    '2030-01-02T12:10:00', '2030-01-02T12:10:00.000',
    '2030-01-02T12:10:15', '2030-01-02T12:10:15.006', '2030-02-30T12:10',
  ]) {
    expect(() => intent({ now, input: invalid })).toThrow()
  }
  expect(() => intent({ now, input: '2030-01-02T12:09' })).toThrow()
})

test('announcement read exact scope yields an owned immutable public snapshot and accepts explicit null', () => {
  const value = intent()
  const rawPublication = publication()
  const raw = readValue(rawPublication)
  const snapshot = parseCompetitionRoundPublicationReadSnapshot(raw, value.scope)
  const firstRoom = rawPublication.rooms[0]
  if (!firstRoom) throw new Error('publication fixture must contain a room')
  firstRoom.gameId = 'mutated-after-parse'
  expect(snapshot.publication?.rooms[0]?.gameId).toBe('game-0')
  expect(Object.isFrozen(snapshot.publication?.rooms[0])).toBe(true)
  expect(parseCompetitionRoundPublicationReadSnapshot(readValue(null), value.scope).publication).toBeNull()
  expect(() => parseCompetitionRoundPublicationReadSnapshot(readValue(publication({
    roundRef: { competitionId, stageId: 'other-stage', roundNumber: 1, roundGenerationId: 'generation-a' },
  })), value.scope)).toThrow()
  expect(() => parseCompetitionRoundPublicationReadSnapshot(readValue({ ...publication(), privateBinding: { secret: true } }), value.scope)).toThrow()
  expect(() => parseCompetitionRoundPublicationReadSnapshot({ ...readValue(), privateSnapshot: { secret: true } }, value.scope)).toThrow()
})

test('announcement ACK must match exact command, expected global revision plus one, T and full room set', () => {
  const value = intent({ viewRevision: 17 })
  const receipt = parseCompetitionRoundAnnouncementAck(ack(value), value)
  expect(receipt).toMatchObject({ competitionId, stageId, roundNumber: 1, commandId: value.body.commandId,
    revision: 18, idempotent: false, publication: { roundStartMs } })
  expect(receipt.publication.rooms.map(room => room.matchId)).toEqual(expectedMatchIds)
  expect(() => parseCompetitionRoundAnnouncementAck(ack(value, publication(), { revision: 19 }), value)).toThrow()
  expect(() => parseCompetitionRoundAnnouncementAck(ack(value, publication(), { commandId: 'other-command' }), value)).toThrow()
  expect(() => parseCompetitionRoundAnnouncementAck(ack(value, publication({
    rooms: [{ matchId: 'match-a', gameId: 'game-0', ackConfirmedAtMs: null }],
  })), value)).toThrow()
  expect(() => parseCompetitionRoundAnnouncementAck(ack(value, { ...publication(), privateManifest: {} }), value)).toThrow()
})

test('unknown or malformed receipt keeps original intent; only first definitive refusal may clear it', () => {
  const command = intent()
  expect(keepCompetitionRoundPublicationIntent(false, { statusCode: 409 })).toBe(false)
  expect(keepCompetitionRoundPublicationIntent(true, { statusCode: 409, message: 'secret' })).toBe(true)
  expect(keepCompetitionRoundPublicationIntent(false, new Error('network unavailable'))).toBe(true)
  expect(command.body.commandId).toBe('8f4d34de-2b13-4bce-9f10-2a9f648db1c8')
})

test('輪次公告選取目前進行階段，不受已結束階段影響', () => {
  const parsed = tournament(currentRound(), { finishedStageCount: 1 })
  expect(parsed.stages.map(stage => [stage.config.stageId, stage.status])).toEqual([
    [stageId, 'active'], ['finished-stage-1', 'finished'],
  ])
  expect(parsed.stages.map(stage => stage.rounds.flatMap(round => round.matches.map(match => match.matchId)))).toEqual([
    ['match-a', 'match-b', 'current-bye'], ['finished-stage-1-match-1'],
  ])
  expect(findCurrentCompetitionRoundPublicationTarget(parsed)).toEqual({
    kind: 'ready',
    target: { stageId, roundNumber: 1, matchIds: ['match-a', 'match-b'] },
  })
})
