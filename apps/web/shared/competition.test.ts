import { expect, test } from 'bun:test'
import {
  emptyCompetitionServerState,
} from './competition'
import type {
  CompetitionAnnouncement,
  CompetitionOperation,
  CompetitionRequest,
  ClockConfig,
  ClockDisplay,
  ClockCheckpointMetadata,
  MatchResult,
  CompetitionMatchConfig,
  QualifiedStageTemplate,
  StageQualificationSlot,
  SwissStandingPolicy,
  TournamentFormat,
  TrustedGameResult,
  TournamentConfig,
} from './competition'

const announcement: CompetitionAnnouncement = {
  organizer: 'organizer',
  ruleVersion: '5.17',
  ruleset: 'base',
  enabledRuleModules: [],
  teamSize: 1,
  initialHp: 30,
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
      swissRounds: null, swissPolicy: null, qualification: { type: 'allEligibleTeams' },
    }],
  },
}

const clockConfig: ClockConfig = {
  mainTimeMs: 300000,
  actionTimeMs: 30000,
  outOfTurnChoiceTimeMs: 30000,
}

const tournamentConfig: TournamentConfig = {
  tournamentId: 'event-1',
  stage: {
    stageId: 'stage-1',
    format: 'singleElimination',
    teams: [
      { teamId: 'team-a', seed: 1 },
      { teamId: 'team-b', seed: 2 },
    ],
    randomSeed: 1,
    matchFormat: { type: 'firstToWins', value: 1 },
    clockConfig,
    swissRounds: null,
    swissPolicy: null,
  },
}

const qualifiedStageSourceSlots: StageQualificationSlot[] = [
  {
    sourceStageId: 'source-a',
    source: { type: 'finishedStageWinner' },
    destinationSeed: 7,
  },
  {
    sourceStageId: 'source-b',
    source: { type: 'uniqueStandingRank', rank: 2 },
    destinationSeed: 3,
  },
]

const qualifiedStageSwissPolicy: SwissStandingPolicy = {
  matchPoints: { win: 3, draw: 1, loss: 0, bye: 3 },
  drawCredit: { numerator: 1, denominator: 2 },
  opponentWinRateFloorPercent: null,
  opponentWinRateCapPercent: null,
  excludeByesFromOpponentWinRate: false,
  tieBreakers: ['opponentWinRate'],
}

const qualifiedStageTemplates: QualifiedStageTemplate[] = [
  {
    stageId: 'next-single', format: 'singleElimination', randomSeed: 19,
    matchFormat: { type: 'firstToWins', value: 1 }, clockConfig,
    swissRounds: null, swissPolicy: null, sourceSlots: qualifiedStageSourceSlots,
  },
  {
    stageId: 'next-round-robin', format: 'roundRobin', randomSeed: 19,
    matchFormat: { type: 'fixedGames', value: 1 }, clockConfig,
    swissRounds: null, swissPolicy: null, sourceSlots: qualifiedStageSourceSlots,
  },
  {
    stageId: 'next-double', format: 'doubleElimination', randomSeed: 19,
    matchFormat: { type: 'firstToWins', value: 1 }, clockConfig,
    swissRounds: null, swissPolicy: null, sourceSlots: qualifiedStageSourceSlots,
  },
  {
    stageId: 'next-swiss', format: 'swiss', randomSeed: 19,
    matchFormat: { type: 'fixedGames', value: 1 }, clockConfig,
    swissRounds: 1, swissPolicy: qualifiedStageSwissPolicy, sourceSlots: qualifiedStageSourceSlots,
  },
]

const matchResult: MatchResult = {
  type: 'win',
  winnerId: 'team-a',
  loserId: 'team-b',
  winnerGames: 1,
  loserGames: 0,
}

const matchConfig: CompetitionMatchConfig = {
  matchId: 'match-1',
  teams: ['team-a', 'team-b'],
  format: { type: 'fixedGames', value: 1 },
  firstGame: { type: 'assigned', team: 'team-a' },
  referees: [{ refereeId: 'ref-1', conflictedTeam: null }],
}

const trustedGameResult: TrustedGameResult = {
  submissionId: 'submission-1',
  gameRecordId: 'game-1',
  rulingRecordId: 'ruling-1',
  startKind: { type: 'ordinaryNewGame' },
  firstPlayer: 'team-a',
  terminalEvidence: {
    type: 'canonicalGameEnded',
    conclusion: {
      outcome: { type: 'winner', team: 'team-a' },
      causes: [{ type: 'teamHpDepleted', teams: ['team-b'] }],
    },
  },
  ruling: { type: 'gameAccepted', outcome: { winner: 'team-a' } },
}

test('competition request covers every existing Rust core operation', () => {
  const operations: CompetitionOperation[] = [
    { type: 'registrationResolveSettings', ruleVersion: '5.17', ruleset: 'base', enabledRuleModules: [], teamSize: 1 },
    { type: 'registrationCreate', announcement },
    {
      type: 'registrationExecute',
      commandId: 'command-1',
      expectedRevision: 0,
      action: { type: 'open' },
    },
    { type: 'registrationView' },
    { type: 'registrationExportLockedRoster' },
    { type: 'clockCreate', teams: ['team-a', 'team-b'], config: clockConfig },
    { type: 'clockBeginPreparation' },
    { type: 'clockFinishPreparation', team: 'team-a' },
    { type: 'clockBeginAction', team: 'team-a', turn: 1 },
    { type: 'clockBeginOutOfTurnChoice', team: 'team-b', choiceId: 'choice-1' },
    { type: 'clockObserve' },
    { type: 'clockView' },
    { type: 'clockCheckpointMetadata' },
    { type: 'clockEndWindow' },
    { type: 'clockPause', reason: '裁判處理中' },
    { type: 'clockResume', reason: '裁判處理完成' },
    { type: 'clockNewGame', time: 'carryForward' },
    {
      type: 'clockRuleGame',
      conclusion: {
        outcome: { type: 'draw' },
        causes: [{ type: 'teamHpDepleted', teams: ['team-a', 'team-b'] }],
      },
      mode: 'elimination',
      byoyomiEntryOrder: { type: 'simultaneous' },
    },
    { type: 'tournamentCreate', config: tournamentConfig },
    { type: 'tournamentPreviewQualifiedStage', template: qualifiedStageTemplates[0]! },
    { type: 'tournamentCreateQualifiedStage', template: qualifiedStageTemplates[1]! },
    { type: 'tournamentView' },
    { type: 'tournamentStartMatch', stageId: 'stage-1', matchId: 'stage-1-g0-r1-m1' },
    {
      type: 'tournamentSubmitResult',
      stageId: 'stage-1',
      matchId: 'stage-1-g0-r1-m1',
      result: matchResult,
    },
    {
      type: 'tournamentCorrectResult',
      stageId: 'stage-1',
      matchId: 'stage-1-g0-r1-m1',
      replacement: { type: 'doubleLoss' },
      reason: 'referee ruling',
    },
    { type: 'tournamentDropTeam', stageId: 'stage-1', teamId: 'team-b' },
    { type: 'matchCreate', config: matchConfig },
    { type: 'matchView', matchId: 'match-1' },
    { type: 'matchAssignFirstPlayer', matchId: 'match-1', team: 'team-a', reason: '裁判指定' },
    { type: 'matchSubmitGame', matchId: 'match-1', submission: trustedGameResult },
    {
      type: 'matchCorrectRuling',
      matchId: 'match-1',
      correctionId: 'correction-1',
      refereeId: 'ref-1',
      gameRecordId: 'game-1',
      replacementRulingRecordId: 'ruling-2',
      replacementRuling: { type: 'gameAccepted', outcome: { winner: 'team-a' } },
      reason: '複核',
    },
  ]

  expect(operations).toHaveLength(31)
  expect(JSON.stringify(operations)).not.toMatch(/_[a-z]/)
})

test('E6e2a checkpoint metadata is a fieldless camelCase getter with one exact public field', () => {
  const operation = { type: 'clockCheckpointMetadata' } satisfies CompetitionOperation
  expect(JSON.stringify(operation)).toBe('{"type":"clockCheckpointMetadata"}')
  for (const deadline of [null, 0, 4100]) {
    const metadata = { nextTimeoutDeadlineMs: deadline } satisfies ClockCheckpointMetadata
    expect(Object.keys(metadata)).toEqual(['nextTimeoutDeadlineMs'])
    expect(JSON.stringify(metadata)).not.toMatch(/_[a-z]|ruling|winner|private|sampledAtMs/)
  }
  // @ts-expect-error checkpoint getter 不接受 caller 覆寫期限。
  const spoofed: CompetitionOperation = { type: 'clockCheckpointMetadata', nextTimeoutDeadlineMs: 5000 }
  void spoofed
})

test('E6e1 clockView and its display DTO have an exact camelCase contract', () => {
  const operation = { type: 'clockView' } satisfies CompetitionOperation
  const display = {
    sampledAtMs: 125,
    teams: [
      { team: 'a', mainTimeRemainingMs: null, byoyomiTimeRemainingMs: null, byoyomiActive: false },
      { team: 'b', mainTimeRemainingMs: 0, byoyomiTimeRemainingMs: 5, byoyomiActive: true },
    ],
    running: { type: 'outOfTurnChoice', team: 'a', choiceId: 'choice', remainingMs: 7 },
    paused: null,
    awaitingAuthority: false,
  } satisfies ClockDisplay
  expect(JSON.stringify(operation)).toBe('{"type":"clockView"}')
  expect(Object.keys(display)).toEqual(['sampledAtMs', 'teams', 'running', 'paused', 'awaitingAuthority'])
  expect(Object.keys(display.teams[0]!)).toEqual(['team', 'mainTimeRemainingMs', 'byoyomiTimeRemainingMs', 'byoyomiActive'])
  expect(JSON.stringify(display)).not.toMatch(/_[a-z]|ruling|winner|needsDecision|entrySequence/)
  // @ts-expect-error 純查詢不接受 caller 覆寫時間、設定或 checkpoint。
  const spoofed: CompetitionOperation = { type: 'clockView', trustedTimeMs: 1 }
  void spoofed
})

test('E6d1a tournamentView has an exact fieldless camelCase server-only operation', () => {
  const operation = { type: 'tournamentView' } satisfies CompetitionOperation
  const request = {
    serverActor: { type: 'player', playerId: 'player-1' },
    trustedTimeMs: 42,
    operation,
    serverState: emptyCompetitionServerState(),
  } satisfies CompetitionRequest
  expect(JSON.stringify(operation)).toBe('{"type":"tournamentView"}')
  expect(Object.keys(request.operation)).toEqual(['type'])
  expect(JSON.stringify(request)).not.toContain('tournament_view')
  // 查詢沒有 caller 可覆寫的 stage／seed／config 欄位。
  // @ts-expect-error tournamentView 不接受額外的 stageId。
  const invalid: CompetitionOperation = { type: 'tournamentView', stageId: 'other-stage' }
  void invalid
})

test('server request keeps actor, trusted time, and checkpoint in explicit camelCase fields', () => {
  const request = {
    serverActor: { type: 'referee', playerId: 'referee-1' },
    trustedTimeMs: 42,
    operation: { type: 'tournamentDropTeam', stageId: 'stage-1', teamId: 'team-b' },
    serverState: emptyCompetitionServerState(),
  } satisfies CompetitionRequest

  const wire = JSON.stringify(request)
  expect(wire).toContain('serverActor')
  expect(wire).toContain('trustedTimeMs')
  expect(wire).toContain('stageId')
  expect(wire).toContain('teamId')
  expect(wire).toContain('serverState')
  expect(wire).not.toContain('server_actor')
  expect(wire).not.toContain('trusted_time_ms')
})

test('stage settings use the exact camelCase contract and one match format source', () => {
  const encoded = JSON.stringify(tournamentConfig)
  expect(encoded).toContain('matchFormat')
  expect(encoded).toContain('clockConfig')
  expect(encoded).toContain('firstToWins')
  expect(encoded).not.toContain('matchWinsRequired')
  expect(encoded).not.toContain('match_format')
  expect(encoded).not.toContain('clock_config')
})

test('terminal evidence uses the exact camelCase union contract', () => {
  const timeoutEvidence = {
    type: 'timeout',
    timeoutId: 'timeout-1',
    expiries: [{ team: 'team-a', deadlineMonotonicMs: 130 }],
    observedAtMonotonicMs: 140,
    clockCheckpointDigest: 'digest-1',
  } satisfies NonNullable<TrustedGameResult['terminalEvidence']>
  const encoded = JSON.stringify(timeoutEvidence)
  expect(encoded).toContain('timeoutId')
  expect(encoded).toContain('deadlineMonotonicMs')
  expect(encoded).toContain('observedAtMonotonicMs')
  expect(encoded).toContain('clockCheckpointDigest')
  expect(encoded).not.toMatch(/timeout_id|deadline_monotonic_ms|observed_at_monotonic_ms|clock_checkpoint_digest/)
  expect(JSON.stringify(trustedGameResult)).toContain('canonicalGameEnded')
  expect(JSON.stringify(trustedGameResult)).toContain('terminalEvidence')
})

test('qualified-stage TS bridge contract encodes both operations and the exact camelCase template', () => {
  const operations = [
    { type: 'tournamentPreviewQualifiedStage', template: qualifiedStageTemplates[0]! },
    { type: 'tournamentCreateQualifiedStage', template: qualifiedStageTemplates[0]! },
  ] satisfies CompetitionOperation[]

  expect(operations.map(operation => operation.type)).toEqual([
    'tournamentPreviewQualifiedStage', 'tournamentCreateQualifiedStage',
  ])
  for (const operation of operations) {
    expect(Object.keys(operation)).toEqual(['type', 'template'])
    expect(Object.keys(operation.template)).toEqual([
      'stageId', 'format', 'randomSeed', 'matchFormat', 'clockConfig',
      'swissRounds', 'swissPolicy', 'sourceSlots',
    ])
    expect(Object.hasOwn(operation.template, 'swissRounds')).toBe(true)
    expect(Object.hasOwn(operation.template, 'swissPolicy')).toBe(true)
  }
  expect(operations[0]!.template.swissRounds).toBeNull()
  expect(operations[0]!.template.swissPolicy).toBeNull()
  expect(Object.keys(qualifiedStageSourceSlots[0]!)).toEqual([
    'sourceStageId', 'source', 'destinationSeed',
  ])
  expect(Object.keys(qualifiedStageSourceSlots[0]!.source)).toEqual(['type'])
  expect(Object.keys(qualifiedStageSourceSlots[1]!.source)).toEqual(['type', 'rank'])
  expect(JSON.stringify(operations)).not.toMatch(/random_seed|match_format|clock_config|swiss_rounds|swiss_policy|source_slots|source_stage_id|destination_seed/)

  // @ts-expect-error caller 只能傳來源名額，不能偽造已解析 teams 或 plan。
  const forgedTeams: QualifiedStageTemplate = { ...qualifiedStageTemplates[0]!, teams: [{ teamId: 'forged', seed: 1 }] }
  // @ts-expect-error template 欄位使用 camelCase wire contract。
  const forgedSnakeCase: QualifiedStageTemplate = { ...qualifiedStageTemplates[0]!, random_seed: 19 }
  // @ts-expect-error 來源只能是核心支援的封閉 union，不能由 caller 直接指定隊伍。
  const forgedSource: StageQualificationSlot = {
    sourceStageId: 'source-a', source: { type: 'callerTeam', teamId: 'forged' }, destinationSeed: 1,
  }
  void [forgedTeams, forgedSnakeCase, forgedSource]
})

test('qualified-stage TS bridge contract transports all four formats without deriving teams', () => {
  const formats: TournamentFormat[] = ['singleElimination', 'roundRobin', 'doubleElimination', 'swiss']
  expect(qualifiedStageTemplates.map(template => template.format)).toEqual(formats)
  for (const template of qualifiedStageTemplates) {
    expect(template.sourceSlots).toStrictEqual(qualifiedStageSourceSlots)
    expect(template).not.toHaveProperty('teams')
    expect(template).not.toHaveProperty('seededTeams')
    expect(template).not.toHaveProperty('plan')
    expect(template).not.toHaveProperty('sourceProof')
  }
  expect(JSON.parse(JSON.stringify(qualifiedStageTemplates))).toStrictEqual(qualifiedStageTemplates)
})
