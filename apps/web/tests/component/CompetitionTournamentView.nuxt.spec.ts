import { mountSuspended } from '@nuxt/test-utils/runtime'
import { mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import CompetitionTournamentView from '~/components/CompetitionTournamentView.vue'

const fetchMock = vi.hoisted(() => vi.fn())
mockNuxtImport('$fetch', () => fetchMock)

function signalAt(index: number): AbortSignal {
  const signal: unknown = fetchMock.mock.calls[index]?.[1]?.signal
  if (!(signal instanceof AbortSignal)) throw new Error('expected tournament GET AbortSignal')
  return signal
}

function stageFixture(format: string, stageId: string) {
  const kind = format === 'doubleElimination' ? 'winners' : 'main'
  const participants = format === 'swiss' ? ['team-a'] : ['team-a', 'team-b']
  const result = format === 'roundRobin'
    ? { type: 'draw', firstGames: 1, secondGames: 1 }
    : format === 'doubleElimination'
      ? { type: 'doubleLoss' }
      : format === 'swiss'
        ? { type: 'bye', teamId: 'team-a' }
        : { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 2, loserGames: 1 }
  const matchId = `${stageId}-match-1`
  const match = {
    matchId,
    stageId,
    roundNumber: 1,
    sequence: 4,
    kind,
    participants,
    status: 'completed',
    result,
    cancellationReason: null,
    started: format !== 'swiss',
  }
  const oldRound = {
    roundNumber: 1,
    kind,
    matches: [{ ...match, matchId: `${stageId}-old-match`, status: 'cancelled', result: null, cancellationReason: 'private cancellation note', started: false }],
  }
  return {
    config: {
      stageId,
      format,
      teams: [{ teamId: 'team-a', seed: 1 }, { teamId: 'team-b', seed: 2 }],
      randomSeed: 72,
      matchFormat: {
        type: format === 'roundRobin' || format === 'swiss' ? 'fixedGames' : 'firstToWins',
        value: 2,
      },
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 20_000 },
      swissRounds: format === 'swiss' ? 4 : null,
      swissPolicy: format === 'swiss'
        ? {
            matchPoints: { win: 3, draw: 1, loss: 0, bye: 3 },
            drawCredit: { numerator: 1, denominator: 2 },
            opponentWinRateFloorPercent: 25,
            opponentWinRateCapPercent: 75,
            excludeByesFromOpponentWinRate: true,
            tieBreakers: ['matchPoints', 'opponentWinRate', 'seedOrder'],
          }
        : null,
    },
    status: format === 'roundRobin' ? 'finished' : 'active',
    rounds: [{ roundNumber: 1, kind, matches: [match] }],
    standings: [
      {
        teamId: 'team-b', matchPoints: '0', wins: 0, draws: 0, losses: 1,
        gameWins: 1, gameLosses: 2, netGameWins: -1,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: null,
      },
      {
        teamId: 'team-a', matchPoints: '3', wins: 1, draws: 0, losses: 0,
        gameWins: 2, gameLosses: 1, netGameWins: 1,
        opponentWinRateBasisPoints: 5_000, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: 1,
      },
    ],
    resolution: null,
    rankingTieDecisions: [],
    corrections: format === 'roundRobin'
      ? [{
          matchId,
          previousResult: { type: 'draw', firstGames: 1, secondGames: 1 },
          replacementResult: { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 2, loserGames: 1 },
          reason: 'private correction note',
        }]
      : [],
    lastScheduleError: format === 'doubleElimination' ? 'private referee detail' : null,
    scheduleGeneration: format === 'doubleElimination' ? 2 : 0,
    supersededRounds: format === 'singleElimination' ? [oldRound] : [],
    // 精確 wire 欄位依 Rust Serialize；UI 不展開 scheduler checkpoint。
    teams: {
      'team-a': { seed: 1, dropped: false, hadBye: format === 'swiss' },
      'team-b': { seed: 2, dropped: false, hadBye: false },
    },
    doubleState: format === 'doubleElimination'
      ? {
          phase: 'winners',
          winnersPool: ['team-a', 'team-b'],
          losersWaiting: [],
          lowerPool: [],
          winnersChampion: null,
          losersChampion: null,
          grandFinalWinner: null,
        }
      : null,
  }
}

function tournamentResponse(competitionId = 'competition-a', revision = 9) {
  return {
    competitionId,
    revision,
    projection: {
      ok: true,
      operation: 'tournamentView',
      public: {
        tournamentId: competitionId,
        stages: [
          stageFixture('singleElimination', 'single-stage'),
          stageFixture('roundRobin', 'round-robin-stage'),
          stageFixture('doubleElimination', 'double-stage'),
          stageFixture('swiss', 'swiss-stage'),
        ],
      },
    },
  }
}

function noMatchResolutionResponse(kind: 'winner' | 'cancelled') {
  const response = tournamentResponse('competition a', 10)
  const stage = response.projection.public.stages[0]!
  const teamId = kind === 'winner' ? '2f76d85c-512b-4ea6-9297-5a6953ed114c' : null
  const teamStanding = stage.standings.find(standing => standing.teamId === 'team-a')
  if (!teamStanding) throw new Error('missing single-elimination team standing fixture')
  Object.assign(stage, {
    config: {
      ...stage.config,
      teams: teamId ? [{ teamId, seed: 8 }] : [],
    },
    status: 'finished',
    rounds: [],
    standings: teamId ? [{ ...teamStanding, teamId, rank: 1, dropped: false }] : [],
    corrections: [],
    lastScheduleError: null,
    scheduleGeneration: 0,
    supersededRounds: [],
    teams: teamId ? { [teamId]: { seed: 8, dropped: false, hadBye: false } } : {},
    doubleState: null,
    resolution: teamId
      ? { type: 'winnerWithoutMatch', teamId }
      : { type: 'cancelled', reason: 'noQualifiedParticipants' },
  })
  return response
}

function rankingTieResponse(competitionId = 'competition a', revision = 9) {
  const response = tournamentResponse(competitionId, revision)
  const stage = response.projection.public.stages[1]!
  stage.status = 'finished'
  Object.assign(stage.rounds[0]!.matches[0]!, {
    status: 'completed', result: { type: 'draw', firstGames: 1, secondGames: 1 }, started: true,
  })
  stage.standings = [
    { teamId: 'team-a', matchPoints: '1', wins: 0, draws: 1, losses: 0,
      gameWins: 1, gameLosses: 1, netGameWins: 0, opponentWinRateBasisPoints: null,
      opponentWinScore: '2', opponentLossScore: '1', dropped: false, rank: null },
    { teamId: 'team-b', matchPoints: '1', wins: 0, draws: 1, losses: 0,
      gameWins: 1, gameLosses: 1, netGameWins: 0, opponentWinRateBasisPoints: null,
      opponentWinScore: '2', opponentLossScore: '1', dropped: false, rank: null },
  ]
  return response
}

function swissPlayedResponse(status: 'active' | 'finished' = 'active') {
  const response = tournamentResponse('competition a', 10)
  const swiss = response.projection.public.stages.find(stage => stage.config.stageId === 'swiss-stage')
  if (!swiss) throw new Error('missing Swiss fixture stage')
  swiss.status = status
  swiss.config.swissRounds = 1
  const match = swiss.rounds[0]?.matches[0]
  if (!match) throw new Error('missing Swiss fixture Match')
  Object.assign(match, {
    participants: ['team-a', 'team-b'],
    result: { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 2, loserGames: 1 },
    started: true,
  })
  swiss.teams['team-a'].hadBye = false
  return response
}

function swissMultiRoundResponse() {
  const response = swissPlayedResponse()
  const swiss = response.projection.public.stages.find(stage => stage.config.stageId === 'swiss-stage')
  if (!swiss) throw new Error('missing Swiss fixture stage')
  const teamIds = ['team-a', 'team-b', 'team-c', 'team-d', 'team-e']
  Object.assign(swiss.config, {
    teams: teamIds.map((teamId, index) => ({ teamId, seed: index + 1 })),
    swissRounds: 3,
  })
  Object.assign(swiss.teams, {
    'team-c': { seed: 3, dropped: false, hadBye: false },
    'team-d': { seed: 4, dropped: false, hadBye: false },
    'team-e': { seed: 5, dropped: false, hadBye: true },
  })
  swiss.teams['team-a'].hadBye = true
  swiss.teams['team-c'].hadBye = true
  const makeMatch = (
    roundNumber: number,
    sequence: number,
    participants: string[],
    status: 'pending' | 'completed',
    result: unknown,
    started: boolean,
  ) => ({
    matchId: `swiss-stage-g0-r${roundNumber}-m${sequence}`,
    stageId: 'swiss-stage', roundNumber, sequence, kind: 'main',
    participants, status, result, cancellationReason: null, started,
  })
  Object.assign(swiss, {
    rounds: [
      { roundNumber: 1, kind: 'main', matches: [
        makeMatch(1, 1, ['team-a', 'team-b'], 'completed',
          { type: 'win', winnerId: 'team-a', loserId: 'team-b', winnerGames: 2, loserGames: 1 }, true),
        makeMatch(1, 2, ['team-c', 'team-d'], 'completed',
          { type: 'win', winnerId: 'team-c', loserId: 'team-d', winnerGames: 2, loserGames: 1 }, true),
        makeMatch(1, 3, ['team-e'], 'completed', { type: 'bye', teamId: 'team-e' }, false),
      ] },
      { roundNumber: 2, kind: 'main', matches: [
        makeMatch(2, 1, ['team-a', 'team-d'], 'completed',
          { type: 'win', winnerId: 'team-a', loserId: 'team-d', winnerGames: 2, loserGames: 1 }, true),
        makeMatch(2, 2, ['team-b', 'team-e'], 'completed',
          { type: 'win', winnerId: 'team-b', loserId: 'team-e', winnerGames: 2, loserGames: 1 }, true),
        makeMatch(2, 3, ['team-c'], 'completed', { type: 'bye', teamId: 'team-c' }, false),
      ] },
      { roundNumber: 3, kind: 'main', matches: [
        makeMatch(3, 1, ['team-a'], 'completed', { type: 'bye', teamId: 'team-a' }, false),
        makeMatch(3, 2, ['team-b', 'team-c'], 'pending', null, false),
        makeMatch(3, 3, ['team-d', 'team-e'], 'pending', null, false),
      ] },
    ],
    standings: [
      { teamId: 'team-a', matchPoints: '9', wins: 2, draws: 0, losses: 0, gameWins: 4, gameLosses: 1,
        netGameWins: 3, opponentWinRateBasisPoints: 6_000, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: 1 },
      { teamId: 'team-b', matchPoints: '3', wins: 1, draws: 0, losses: 1, gameWins: 3, gameLosses: 3,
        netGameWins: 0, opponentWinRateBasisPoints: 5_000, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: 2 },
      { teamId: 'team-c', matchPoints: '6', wins: 1, draws: 0, losses: 0, gameWins: 2, gameLosses: 1,
        netGameWins: 1, opponentWinRateBasisPoints: 5_500, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: 3 },
      { teamId: 'team-d', matchPoints: '0', wins: 0, draws: 0, losses: 2, gameWins: 2, gameLosses: 4,
        netGameWins: -2, opponentWinRateBasisPoints: 4_000, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: 4 },
      { teamId: 'team-e', matchPoints: '3', wins: 0, draws: 0, losses: 1, gameWins: 1, gameLosses: 2,
        netGameWins: -1, opponentWinRateBasisPoints: 4_500, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: 5 },
    ],
  })
  return response
}

function doubleEliminationFinalsResponse(withReset: boolean) {
  const response = tournamentResponse('competition a', 10)
  const stage = response.projection.public.stages[2]!
  const teamIds = ['team-a', 'team-b', 'team-c', 'team-d']
  const makeMatch = (
    roundNumber: number,
    kind: 'winners' | 'losers' | 'grandFinal' | 'grandFinalReset',
    sequence: number,
    winnerId: string,
    loserId: string,
  ) => ({
    matchId: `double-stage-r${roundNumber}-m${sequence}`,
    stageId: 'double-stage',
    roundNumber,
    sequence,
    kind,
    participants: [winnerId, loserId],
    status: 'completed',
    result: { type: 'win', winnerId, loserId, winnerGames: 2, loserGames: 0 },
    cancellationReason: null,
    started: true,
  })

  Object.assign(stage, {
    config: {
      ...stage.config,
      teams: teamIds.map((teamId, index) => ({ teamId, seed: index + 1 })),
    },
    status: 'finished',
    rounds: [
      { roundNumber: 1, kind: 'winners', matches: [
        makeMatch(1, 'winners', 1, 'team-a', 'team-d'),
        makeMatch(1, 'winners', 2, 'team-b', 'team-c'),
      ] },
      { roundNumber: 2, kind: 'losers', matches: [
        makeMatch(2, 'losers', 1, 'team-d', 'team-c'),
      ] },
      { roundNumber: 3, kind: 'winners', matches: [
        makeMatch(3, 'winners', 1, 'team-a', 'team-b'),
      ] },
      { roundNumber: 4, kind: 'losers', matches: [
        makeMatch(4, 'losers', 1, 'team-d', 'team-b'),
      ] },
      { roundNumber: 5, kind: 'grandFinal', matches: [
        makeMatch(5, 'grandFinal', 1, withReset ? 'team-d' : 'team-a', withReset ? 'team-a' : 'team-d'),
      ] },
      ...(withReset ? [{ roundNumber: 6, kind: 'grandFinalReset', matches: [
        makeMatch(6, 'grandFinalReset', 1, 'team-a', 'team-d'),
      ] }] : []),
    ],
    standings: teamIds.map((teamId, index) => ({
      teamId,
      matchPoints: String([9, 3, 0, withReset ? 9 : 6][index]),
      wins: [3, 1, 0, withReset ? 3 : 2][index],
      draws: 0,
      losses: [withReset ? 1 : 0, 2, 2, 2][index],
      gameWins: [6, 2, 0, withReset ? 6 : 4][index],
      gameLosses: [withReset ? 2 : 0, 4, 4, 4][index],
      netGameWins: [withReset ? 4 : 6, -2, -4, withReset ? 2 : 0][index],
      opponentWinRateBasisPoints: null,
      opponentWinScore: '0', opponentLossScore: '0', dropped: false,
      rank: [1, 3, 4, 2][index],
    })),
    teams: Object.fromEntries(teamIds.map((teamId, index) => [teamId, {
      seed: index + 1, dropped: false, hadBye: false,
    }])),
    doubleState: {
      phase: 'finished',
      winnersPool: [],
      losersWaiting: [],
      lowerPool: [],
      winnersChampion: 'team-a',
      losersChampion: 'team-d',
      grandFinalWinner: 'team-a',
    },
  })
  return response
}

function props(overrides: Partial<{
  registrationLocked: boolean
  participantEntryEnabled: boolean
  participantTeamId: string
  participantIsCaptain: boolean
}> = {}) {
  return {
    competitionId: ' competition a ',
    sessionIdentity: 'member-a',
    contextRevision: 7,
    contextReady: true,
    ...overrides,
  }
}

beforeEach(() => fetchMock.mockReset())
afterEach(() => {
  vi.useRealTimers()
  vi.restoreAllMocks()
})

it('loads all four formats through the exact session-backed GET and displays server rank/result without raw history', async () => {
  fetchMock.mockResolvedValue(tournamentResponse('competition a'))
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  await flushPromises()

  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/competitions/competition%20a/tournament')
  expect(fetchMock.mock.calls[0]?.[1]?.method).toBe('GET')
  expect(Object.keys(fetchMock.mock.calls[0]?.[1] ?? {}).sort()).toEqual(['method', 'retry', 'signal'])
  expect(fetchMock.mock.calls[0]?.[1]?.retry).toBe(0)
  expect(JSON.stringify(fetchMock.mock.calls[0])).not.toContain('member-a')
  expect(wrapper.text()).toContain('單敗淘汰')
  expect(wrapper.text()).toContain('循環賽')
  expect(wrapper.text()).toContain('雙敗淘汰')
  expect(wrapper.text()).toContain('瑞士制')
  expect(wrapper.text()).toContain('固定 2 局')
  expect(wrapper.text()).toContain('先取得 2 勝')
  expect(wrapper.text()).toContain('回合外選擇時間')
  expect(wrapper.text()).toContain('同分比較順序')
  const swissFacts = wrapper.findAll('.swiss-policy-block dl > div')
  const matchPoints = swissFacts.find(fact => fact.get('dt').text() === '勝／和／負／輪空積分')
  const drawCredit = swissFacts.find(fact => fact.get('dt').text() === '對手勝率的和局折算')
  expect(matchPoints?.get('dd').text()).toBe('3／1／0／3')
  expect(drawCredit?.get('dd').text()).toBe('1／2')
  expect(wrapper.text()).toContain('交手 4')
  expect(wrapper.text()).toContain('雙方判負（不推定勝者）')
  expect(wrapper.text()).not.toContain('淘汰賽待裁判處理')
  expect(wrapper.text()).toContain('team-a 輪空')
  expect(wrapper.text()).toContain('已被替代的舊輪次')
  expect(wrapper.text()).toContain('排名未確定')
  expect(wrapper.text()).toContain('第 1 名')
  expect(wrapper.text()).not.toContain('private referee detail')
  expect(wrapper.text()).not.toContain('private correction note')
  expect(wrapper.text()).not.toContain('private cancellation note')
  expect(wrapper.text()).not.toContain('member-a')
  await wrapper.unmount()
})

it('shows Chinese no-match winner and cancellation states without exposing a team machine id', async () => {
  const machineTeamId = '2f76d85c-512b-4ea6-9297-5a6953ed114c'
  for (const kind of ['winner', 'cancelled'] as const) {
    fetchMock.mockResolvedValueOnce(noMatchResolutionResponse(kind))
    const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
    try {
      await flushPromises()
      const stage = wrapper.findAll('article.stage-card')[0]!
      const message = stage.get('.review-note').text()
      expect(stage.get('.status-badge').text()).toBe(kind === 'winner' ? '階段已結束' : '階段已取消')
      expect(message).not.toContain(machineTeamId)
      expect(message).not.toContain('Game')
      if (kind === 'winner') {
        expect(message).toContain('唯一合格隊伍直接成為階段勝者')
        expect(message).toContain('未建立交手或遊戲')
      } else {
        expect(message).toContain('無合格參賽者，階段已取消')
        expect(message).toContain('原因')
      }
    } finally {
      await wrapper.unmount()
    }
  }
})

it('shows a newly advanced stage and its participant game entry from the saved tournament projection', async () => {
  const response = tournamentResponse('competition a', 10)
  const source = stageFixture('roundRobin', 'stage-source')
  source.status = 'finished'
  const followup = stageFixture('roundRobin', 'stage-followup')
  followup.status = 'active'
  Object.assign(followup.rounds[0]!.matches[0]!, {
    participants: ['team-a', 'team-b'], status: 'pending', result: null, started: false,
  })
  response.projection.public.stages = [source, followup]
  fetchMock.mockResolvedValueOnce(response)

  const wrapper = await mountSuspended(CompetitionTournamentView, {
    props: props({
      registrationLocked: true,
      participantEntryEnabled: true,
      participantTeamId: 'team-a',
      participantIsCaptain: true,
    }),
    global: {
      stubs: {
        CompetitionParticipantGameEntry: { template: '<a data-testid="participant-game-entry">參加交手</a>' },
      },
    },
  })
  try {
    await flushPromises()
    expect(wrapper.findAll('article.stage-card')).toHaveLength(2)
    const newStage = wrapper.get('article[aria-labelledby="tournament-stage-stage-followup"]')
    expect(newStage.get('.stage-heading h3').text()).toBe('循環賽')
    expect(newStage.get('.rounds-section .match-heading .identifier').text()).toBe('stage-followup-match-1')
    expect(newStage.get('[data-testid="participant-game-entry"]').text()).toBe('參加交手')
  } finally {
    await wrapper.unmount()
  }
})

it('shows every team Match wins to participants and spectators during Swiss rounds, then publishes standings', async () => {
  const expectedWins = [
    ['team-a', '2'], ['team-b', '1'], ['team-c', '1'], ['team-d', '0'], ['team-e', '0'],
  ] as const

  for (const participantTeamId of ['team-a', undefined]) {
    fetchMock.mockResolvedValueOnce(swissMultiRoundResponse())
    const wrapper = await mountSuspended(CompetitionTournamentView, {
      props: { ...props(), ...(participantTeamId ? { participantTeamId } : {}) },
    })
    await flushPromises()

    const swiss = wrapper.get('article[aria-labelledby="tournament-stage-swiss-stage"]')
    const progress = swiss.get('.standings-section')
    expect(progress.text()).toContain('瑞士輪進度')
    expect(progress.findAll('.swiss-progress-item')).toHaveLength(expectedWins.length)
    for (const [teamId, wins] of expectedWins) {
      const teamProgress = progress.get(`[data-team-id="${teamId}"]`)
      expect(teamProgress.text()).toContain(teamId)
      expect(teamProgress.text()).toContain(`自身交手勝場：${wins}`)
    }
    expect(swiss.findAll('.rounds-section .round-card')).toHaveLength(3)
    expect(swiss.findAll('.standing-card')).toHaveLength(0)
    expect(progress.text()).not.toContain('賽事積分')
    expect(progress.text()).not.toContain('對手勝率')
    expect(progress.text()).not.toContain('第 1 名')
    await wrapper.unmount()
  }

  fetchMock.mockResolvedValueOnce(swissPlayedResponse('finished'))
  const finished = await mountSuspended(CompetitionTournamentView, { props: props() })
  await flushPromises()
  const finishedSwiss = finished.get('article[aria-labelledby="tournament-stage-swiss-stage"]')
  expect(finishedSwiss.findAll('.standing-card')).toHaveLength(2)
  expect(finishedSwiss.get('.standings-section').text()).toContain('伺服器排名與積分')
  await finished.unmount()
})

it('shows formal opponent scores and submits a complete public ranking tie decision', async () => {
  fetchMock
    .mockResolvedValueOnce(rankingTieResponse())
    .mockResolvedValueOnce({ ok: true })
    .mockResolvedValueOnce(rankingTieResponse())
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  await flushPromises()

  const roundRobin = wrapper.get('article[aria-labelledby="tournament-stage-round-robin-stage"]')
  const tiePanel = roundRobin.get('.ranking-tie-panel')
  expect(roundRobin.text()).toContain('不同對手勝分')
  expect(roundRobin.text()).toContain('不同對手負分')
  expect(roundRobin.text()).toContain('積分 1・勝分 2・負分 1')
  expect(roundRobin.text()).not.toContain('淨勝局')

  const positions = tiePanel.findAll('select')
  expect(positions).toHaveLength(2)
  await positions[0]!.setValue('team-b')
  await positions[1]!.setValue('team-a')
  await tiePanel.get('textarea').setValue('依公開的同分處理理由排序')
  const submit = tiePanel.get('button.tie-submit')
  expect((submit.element as HTMLButtonElement).disabled).toBe(false)
  await submit.trigger('click')
  await flushPromises()

  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(fetchMock.mock.calls.map(([, options]) => options?.method)).toEqual(['GET', 'POST', 'GET'])
  expect(fetchMock.mock.calls[1]?.[0]).toBe('/api/competitions/competition%20a/tournament/ranking-tie')
  expect(fetchMock.mock.calls[1]?.[1]?.body).toMatchObject({
    expectedRevision: 9,
    stageId: 'round-robin-stage',
    orderedTeamIds: ['team-b', 'team-a'],
    reason: '依公開的同分處理理由排序',
  })
  expect(fetchMock.mock.calls[1]?.[1]?.body.commandId).toMatch(/^ranking-tie-/)
  expect(JSON.stringify(fetchMock.mock.calls[1]?.[1]?.body)).not.toContain('member-a')
  expect(wrapper.text()).toContain('同分順序已提交，正在確認公開賽程。')
  await wrapper.unmount()
})

it('shows the complete double-elimination route with and without the grand-final reset Match', async () => {
  for (const withReset of [false, true]) {
    fetchMock.mockResolvedValue(doubleEliminationFinalsResponse(withReset))
    const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
    await flushPromises()

    const doubleStage = wrapper.findAll('.stage-card')
      .find(card => card.find('h3').text() === '雙敗淘汰')
    expect(doubleStage).toBeDefined()
    expect(doubleStage!.findAll('.round-card > h5').map(heading => heading.text())).toEqual([
      '勝部・第 1 輪',
      '敗部・第 2 輪',
      '勝部・第 3 輪',
      '敗部・第 4 輪',
      '總決賽・第 5 輪',
      ...(withReset ? ['總決賽重置・第 6 輪'] : []),
    ])
    const finalMatchId = withReset ? 'double-stage-r6-m1' : 'double-stage-r5-m1'
    const finalMatch = doubleStage!.findAll('.match-card')
      .find(match => match.text().includes(finalMatchId))
    expect(finalMatch?.text()).toContain('team-a')
    expect(finalMatch?.text()).toContain('team-d')
    expect(finalMatch?.text()).toContain('team-a 勝出 2–0')
    expect(doubleStage!.text()).toContain('第 1 名')
    await wrapper.unmount()
  }
})

it('shows accessible loading state and only issues another GET after explicit refresh', async () => {
  let resolveInitial!: (value: unknown) => void
  fetchMock
    .mockReturnValueOnce(new Promise<unknown>((resolve) => { resolveInitial = resolve }))
    .mockResolvedValueOnce(tournamentResponse('competition a', 10))
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })

  expect(wrapper.get('[role="status"]').text()).toContain('正在載入目前賽程')
  resolveInitial(tournamentResponse('competition a'))
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(1)

  await wrapper.get('button.refresh-button').trigger('click')
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(fetchMock.mock.calls.every(([, options]) => options?.method === 'GET')).toBe(true)
  expect(wrapper.text()).toContain('賽程版本 10')
  await wrapper.unmount()
})

it('maps not-ready and forbidden responses to fixed safe states with manual retry only', async () => {
  fetchMock
    .mockRejectedValueOnce({
      statusCode: 409,
      message: 'private server detail',
      data: { statusCode: 409, data: { code: 'notReady', message: 'private server detail' } },
    })
    .mockRejectedValueOnce({ statusCode: 403, message: 'private ACL detail' })
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  await flushPromises()

  expect(wrapper.get('[role="status"]').text()).toBe('目前尚未有可顯示賽程。')
  expect(wrapper.text()).not.toContain('private server detail')
  await wrapper.get('button.refresh-button').trigger('click')
  await flushPromises()
  expect(wrapper.get('[role="alert"]').text()).toBe('此賽事目前沒有可公開查閱的賽程。')
  expect(wrapper.text()).not.toContain('private ACL detail')
  expect(fetchMock).toHaveBeenCalledTimes(2)
  await wrapper.unmount()
})

it('automatically refreshes a locked registration after notReady and renders the persisted seeded teams', async () => {
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
  fetchMock
    .mockRejectedValueOnce({ statusCode: 409, data: { code: 'notReady' } })
    .mockResolvedValueOnce(tournamentResponse('competition a', 10))
  const wrapper = await mountSuspended(CompetitionTournamentView, {
    props: { ...props(), registrationLocked: true },
  })
  await flushPromises()

  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(wrapper.get('[role="status"]').text()).toContain('系統正在依公告自動抽籤並建立首輪')
  await vi.advanceTimersByTimeAsync(4_999)
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(1)

  await vi.advanceTimersByTimeAsync(1)
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({ method: 'GET', retry: 0 })
  expect(wrapper.get('.revision-note').text()).toBe('賽程版本 10')
  expect(wrapper.get('.seed-list').text()).toContain('種子 1team-a')
  expect(wrapper.get('.seed-list').text()).toContain('種子 2team-b')
  await wrapper.unmount()
})

it('renders frozen deck labels in Chinese, shows a readable fallback, and retries the card catalog lookup', async () => {
  const completed = {
    ...tournamentResponse('competition a'),
    publication: {
      completedAtTrustedTimeMs: 2_000,
      lockedDecks: [{ teamId: '00000000-0000-4000-8000-000000000001', decks: [{
        player: '00000000-0000-4000-8000-000000000002', name: 'frozen deck', cards: ['metal-1', 'metal-1', 'unlisted-card'],
      }] }],
      replays: [{
        matchId: '00000000-0000-4000-8000-000000000003', gameId: 'game-1', gameNumber: 1,
        replayId: 'competition:competition%20a:game-1',
      }],
    },
  }
  let catalogCalls = 0
  let rejectFirstCatalog!: (error: Error) => void
  fetchMock.mockImplementation(async (url: string) => {
    if (url === '/api/rules/catalog') {
      catalogCalls += 1
      if (catalogCalls === 1) {
        return await new Promise<unknown>((_resolve, reject) => { rejectFirstCatalog = reject })
      }
      return { deckComposition: { cardDefinitions: [{ id: 'metal-1', name: '金一' }] } }
    }
    return completed
  })

  const wrapper = await mountSuspended(CompetitionTournamentView, {
    props: { ...props(), sessionIdentity: null, contextRevision: null },
  })
  try {
    await flushPromises()
    expect(wrapper.get('.publication-card').text()).toContain('本場賽事已完成')
    expect(wrapper.get('.publication-card').text()).toContain('正在載入牌組卡牌名稱')
    expect(wrapper.get('.published-replay-list').text()).toContain('重播 1・第 1 局')
    expect(wrapper.get('.published-replay-list a').attributes('href')).toContain('/replays/')
    expect(wrapper.find('.published-card-list').exists()).toBe(false)

    rejectFirstCatalog(new Error('private catalog failure'))
    await flushPromises()
    expect(wrapper.get('.publication-card').text()).toContain('卡牌名稱暫時無法載入')
    expect(wrapper.get('.published-card-list').text()).toContain('未知卡牌 × 2')
    expect(wrapper.get('.published-card-list').text()).not.toContain('metal-1')
    expect(wrapper.get('.published-card-list').text()).not.toContain('unlisted-card')
    expect(catalogCalls).toBe(1)

    await wrapper.get('.catalog-retry-button').trigger('click')
    await flushPromises()
    expect(wrapper.get('.published-card-list').text()).toContain('金一 × 2')
    expect(wrapper.get('.published-card-list').text()).toContain('未知卡牌')
    expect(wrapper.get('.published-card-list').text()).not.toContain('metal-1')
    expect(wrapper.get('.published-card-list').text()).not.toContain('unlisted-card')
    expect(catalogCalls).toBe(2)

    const publicationText = wrapper.get('.publication-card').text()
    expect(publicationText).toContain('隊伍 1')
    expect(publicationText).toContain('frozen deck・隊員 1')
    expect(publicationText).not.toContain('00000000-0000-4000-8000-000000000001')
    expect(publicationText).not.toContain('00000000-0000-4000-8000-000000000002')
    expect(publicationText).not.toContain('00000000-0000-4000-8000-000000000003')
  } finally {
    await wrapper.unmount()
  }
})

it('fails closed for a malformed projection and displays an empty round without inferring a final state', async () => {
  const malformed = tournamentResponse('competition a')
  const firstStage = malformed.projection.public.stages[0]
  if (!firstStage) throw new Error('missing tournament fixture stage')
  const malformedStage = { ...firstStage, privateHistory: ['secret projection detail'] }
  malformed.projection.public.stages[0] = malformedStage
  fetchMock.mockResolvedValueOnce(malformed)
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  await flushPromises()

  expect(wrapper.get('[role="alert"]').text()).toBe('賽程資料格式不正確，請重新查詢。')
  expect(wrapper.text()).not.toContain('secret projection detail')
  await wrapper.unmount()

  const noRounds = tournamentResponse('competition a')
  const swissStage = noRounds.projection.public.stages[3]
  if (!swissStage) throw new Error('missing Swiss fixture stage')
  swissStage.rounds = []
  fetchMock.mockResolvedValueOnce(noRounds)
  const emptyWrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  await flushPromises()
  expect(emptyWrapper.text()).toContain('目前沒有可顯示輪次；不推定尚未開賽或已結束。')
  await emptyWrapper.unmount()
})

it('aborts old reads and ignores late projections after competition, session, or eligibility context changes', async () => {
  let resolveOld!: (value: unknown) => void
  fetchMock
    .mockReturnValueOnce(new Promise<unknown>((resolve) => { resolveOld = resolve }))
    .mockResolvedValueOnce(tournamentResponse('competition-b', 14))
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  const oldSignal = signalAt(0)

  await wrapper.setProps({
    competitionId: 'competition-b',
    sessionIdentity: 'member-b',
    contextRevision: 8,
  })
  expect(oldSignal.aborted).toBe(true)
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(fetchMock.mock.calls[1]?.[0]).toBe('/api/competitions/competition-b/tournament')
  expect(wrapper.text()).toContain('賽程版本 14')

  resolveOld(tournamentResponse('competition-a', 9))
  await flushPromises()
  expect(wrapper.text()).toContain('賽程版本 14')
  expect(wrapper.text()).not.toContain('賽程版本 9')

  let resolveIneligible!: (value: unknown) => void
  fetchMock.mockReturnValueOnce(new Promise<unknown>((resolve) => { resolveIneligible = resolve }))
  await wrapper.get('button.refresh-button').trigger('click')
  const ineligibleSignal = signalAt(2)
  await wrapper.setProps({ contextReady: false })
  expect(ineligibleSignal.aborted).toBe(true)
  expect(wrapper.text()).not.toContain('賽程版本 14')
  resolveIneligible(tournamentResponse('competition-b', 15))
  await flushPromises()
  expect(wrapper.text()).not.toContain('賽程版本 15')
  expect(fetchMock).toHaveBeenCalledTimes(3)
  await wrapper.unmount()
})

it('does not query before the parent supplies a verified ready session context', async () => {
  const wrapper = await mountSuspended(CompetitionTournamentView, {
    props: { ...props(), sessionIdentity: null, contextReady: false },
  })

  expect(fetchMock).not.toHaveBeenCalled()
  expect(wrapper.get('[role="status"]').text()).toContain('登入狀態尚未確認')
  fetchMock.mockResolvedValueOnce(tournamentResponse('competition a'))
  await wrapper.setProps({ sessionIdentity: 'member-a', contextReady: true })
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(1)
  await wrapper.unmount()
})

it('clears a previous session projection immediately and ignores an old rejection/finally while the new read is pending', async () => {
  let rejectOld!: (reason: unknown) => void
  let resolveNew!: (value: unknown) => void
  fetchMock
    .mockResolvedValueOnce(tournamentResponse('competition a', 9))
    .mockReturnValueOnce(new Promise<unknown>((_, reject) => { rejectOld = reject }))
    .mockReturnValueOnce(new Promise<unknown>((resolve) => { resolveNew = resolve }))
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  await flushPromises()
  expect(wrapper.text()).toContain('賽程版本 9')

  await wrapper.get('button.refresh-button').trigger('click')
  const oldSignal = signalAt(1)
  await wrapper.setProps({ sessionIdentity: 'member-b' })
  expect(oldSignal.aborted).toBe(true)
  expect(wrapper.text()).not.toContain('賽程版本 9')
  expect(wrapper.get('button.refresh-button').attributes()).toHaveProperty('disabled')
  expect(wrapper.get('[role="status"]').text()).toContain('正在載入目前賽程')
  // 保留第一輪 DOM 的停用斷言；再驗排程完成後只發一個新 GET，且持續停用。
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(signalAt(2).aborted).toBe(false)
  expect(wrapper.get('button.refresh-button').attributes()).toHaveProperty('disabled')
  await wrapper.get('button.refresh-button').trigger('click')
  expect(fetchMock).toHaveBeenCalledTimes(3)
  rejectOld({ statusCode: 403, message: 'private old session error' })
  await flushPromises()
  expect(wrapper.find('[role="alert"]').exists()).toBe(false)
  expect(wrapper.get('[role="status"]').text()).toContain('正在載入目前賽程')
  expect(wrapper.get('button.refresh-button').attributes()).toHaveProperty('disabled')

  resolveNew(tournamentResponse('competition a', 10))
  await flushPromises()
  expect(wrapper.text()).toContain('賽程版本 10')
  expect(wrapper.get('button.refresh-button').attributes()).not.toHaveProperty('disabled')
  await wrapper.setProps({ sessionIdentity: null })
  expect(wrapper.text()).not.toContain('賽程版本 10')
  expect(fetchMock).toHaveBeenCalledTimes(3)
  await wrapper.unmount()
})

it('displays an accessible champion vacancy for a finished true single-elimination final double loss without treating it as unresolved', async () => {
  const baseStage = stageFixture('singleElimination', 'vacant-final-stage')
  const vacantStage = {
    ...baseStage,
    config: {
      ...baseStage.config,
      teams: [
        { teamId: 'team-a', seed: 1 },
        { teamId: 'team-b', seed: 2 },
        { teamId: 'team-c', seed: 3 },
        { teamId: 'team-d', seed: 4 },
      ],
      matchFormat: { type: 'firstToWins', value: 1 },
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    },
    status: 'finished',
    rounds: [
      {
        roundNumber: 1,
        kind: 'main',
        matches: [
          {
            matchId: 'vacant-final-stage-g0-r1-m1', stageId: 'vacant-final-stage',
            roundNumber: 1, sequence: 1, kind: 'main', participants: ['team-a', 'team-d'],
            status: 'completed',
            result: { type: 'win', winnerId: 'team-a', loserId: 'team-d', winnerGames: 1, loserGames: 0 },
            cancellationReason: null, started: true,
          },
          {
            matchId: 'vacant-final-stage-g0-r1-m2', stageId: 'vacant-final-stage',
            roundNumber: 1, sequence: 2, kind: 'main', participants: ['team-b', 'team-c'],
            status: 'completed',
            result: { type: 'forfeit', winnerId: 'team-c', loserId: 'team-b' },
            cancellationReason: null, started: true,
          },
        ],
      },
      {
        roundNumber: 2,
        kind: 'main',
        matches: [
          {
            matchId: 'vacant-final-stage-g1-r2-m1', stageId: 'vacant-final-stage',
            roundNumber: 2, sequence: 1, kind: 'main', participants: ['team-a', 'team-c'],
            status: 'completed', result: { type: 'doubleLoss' }, cancellationReason: null, started: true,
          },
          {
            matchId: 'vacant-final-stage-g1-r2-m2', stageId: 'vacant-final-stage',
            roundNumber: 2, sequence: 2, kind: 'main', purpose: 'bronze', participants: ['team-b', 'team-d'],
            status: 'completed',
            result: { type: 'win', winnerId: 'team-b', loserId: 'team-d', winnerGames: 1, loserGames: 0 },
            cancellationReason: null, started: true,
          },
        ],
      },
    ],
    // 保留服務提供的順序；位置與 null rank 都不是冠軍判定的來源。
    standings: [
      {
        teamId: 'team-b', matchPoints: '3', wins: 1, draws: 0, losses: 1,
        gameWins: 1, gameLosses: 1, netGameWins: 0,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: 3,
      },
      {
        teamId: 'team-c', matchPoints: '3', wins: 1, draws: 0, losses: 1,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: true, rank: null,
      },
      {
        teamId: 'team-d', matchPoints: '0', wins: 0, draws: 0, losses: 2,
        gameWins: 0, gameLosses: 2, netGameWins: -2,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: 4,
      },
      {
        teamId: 'team-a', matchPoints: '3', wins: 1, draws: 0, losses: 1,
        gameWins: 1, gameLosses: 0, netGameWins: 1,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: true, rank: null,
      },
    ],
    resolution: null,
    rankingTieDecisions: [],
    corrections: [{
      matchId: 'vacant-final-stage-g0-r1-m1',
      previousResult: { type: 'win', winnerId: 'team-d', loserId: 'team-a', winnerGames: 1, loserGames: 0 },
      replacementResult: { type: 'win', winnerId: 'team-a', loserId: 'team-d', winnerGames: 1, loserGames: 0 },
      reason: 'private semifinal correction history',
    }],
    lastScheduleError: null,
    scheduleGeneration: 1,
    supersededRounds: [{
      roundNumber: 2,
      kind: 'main',
      matches: [{
        matchId: 'vacant-final-stage-g0-r2-m1', stageId: 'vacant-final-stage',
        roundNumber: 2, sequence: 1, kind: 'main', participants: ['team-d', 'team-c'],
        status: 'pending', result: null, cancellationReason: null, started: false,
      }],
    }],
    teams: {
      'team-a': { seed: 1, dropped: false, hadBye: false },
      'team-b': { seed: 2, dropped: false, hadBye: false },
      'team-c': { seed: 3, dropped: false, hadBye: false },
      'team-d': { seed: 4, dropped: false, hadBye: false },
    },
    doubleState: null,
  }
  // 這是外部 GET 的 UI wire fixture，不是 Rust／native producer 或 canonical record。
  const response: unknown = JSON.parse(JSON.stringify({
    competitionId: 'competition a',
    revision: 12,
    projection: {
      ok: true,
      operation: 'tournamentView',
      public: {
        tournamentId: 'competition a',
        stages: [vacantStage, stageFixture('doubleElimination', 'unresolved-de-stage')],
      },
    },
  }))
  const { parseCompetitionTournamentViewResponse } = await import('~/lib/competition-tournament-view')
  const parsed = parseCompetitionTournamentViewResponse(response, 'competition a')
  expect(parsed.stages).toHaveLength(2)
  const parsedFinal = parsed.stages.find(stage => stage.config.stageId === 'vacant-final-stage')
  if (!parsedFinal) throw new Error('missing parsed single-elimination final fixture')
  expect(parsedFinal.config.format).toBe('singleElimination')
  expect(parsedFinal.status).toBe('finished')
  expect(parsedFinal.hasScheduleWarning).toBe(false)
  expect(parsedFinal.rounds[0]?.matches.map(match => [match.status, match.result?.type])).toEqual([
    ['completed', 'win'], ['completed', 'forfeit'],
  ])
  expect(parsedFinal.rounds[1]).toEqual({
    roundNumber: 2,
    kind: 'main',
    matches: [
      {
        matchId: 'vacant-final-stage-g1-r2-m1', stageId: 'vacant-final-stage',
        roundNumber: 2, sequence: 1, kind: 'main', purpose: 'standard', participants: ['team-a', 'team-c'],
        status: 'completed', result: { type: 'doubleLoss' }, cancellationReason: null, started: true,
      },
      {
        matchId: 'vacant-final-stage-g1-r2-m2', stageId: 'vacant-final-stage',
        roundNumber: 2, sequence: 2, kind: 'main', purpose: 'bronze', participants: ['team-b', 'team-d'],
        status: 'completed',
        result: { type: 'win', winnerId: 'team-b', loserId: 'team-d', winnerGames: 1, loserGames: 0 },
        cancellationReason: null, started: true,
      },
    ],
  })
  expect(parsedFinal.standings.map(row => [row.teamId, row.rank])).toEqual([
    ['team-b', 3], ['team-c', null], ['team-d', 4], ['team-a', null],
  ])

  fetchMock.mockResolvedValueOnce(response)
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  try {
    await flushPromises()
    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/competitions/competition%20a/tournament')
    expect(fetchMock.mock.calls[0]?.[1]?.method).toBe('GET')
    expect(Object.keys(fetchMock.mock.calls[0]?.[1] ?? {}).sort()).toEqual(['method', 'retry', 'signal'])
    expect(fetchMock.mock.calls[0]?.[1]?.retry).toBe(0)
    expect(signalAt(0).aborted).toBe(false)
    expect(JSON.stringify(fetchMock.mock.calls[0])).not.toContain('member-a')
    expect(wrapper.find('.state-message[role="alert"]').exists()).toBe(false)
    expect(wrapper.findAll('article.stage-card')).toHaveLength(2)

    const finalStage = wrapper.get('article[aria-labelledby="tournament-stage-vacant-final-stage"]')
    const unresolvedStage = wrapper.get('article[aria-labelledby="tournament-stage-unresolved-de-stage"]')
    expect(finalStage.text()).toContain('階段已結束')
    expect(finalStage.get('.rounds-section').findAll('.match-card').map(match =>
      match.get('.match-heading h6 .identifier').text())).toEqual([
      'vacant-final-stage-g0-r1-m1', 'vacant-final-stage-g0-r1-m2',
      'vacant-final-stage-g1-r2-m1', 'vacant-final-stage-g1-r2-m2',
    ])
    expect(finalStage.get('.rounds-section').text()).toContain('雙方判負（不推定勝者）')
    expect(finalStage.findAll('.standing-card').map(card => ({
      team: card.get('h5').text(),
      rank: card.get('.rank-label').text(),
      facts: card.findAll('.standing-facts > div').map(fact => [fact.get('dt').text(), fact.get('dd').text()]),
    }))).toEqual([
      {
        team: 'team-b', rank: '第 3 名',
        facts: [['賽事積分', '3'], ['勝／和／負', '1／0／1'], ['不同對手勝分', '0'], ['不同對手負分', '0'], ['已退賽', '否']],
      },
      {
        team: 'team-c', rank: '排名未確定',
        facts: [['賽事積分', '3'], ['勝／和／負', '1／0／1'], ['不同對手勝分', '0'], ['不同對手負分', '0'], ['已退賽', '是']],
      },
      {
        team: 'team-d', rank: '第 4 名',
        facts: [['賽事積分', '0'], ['勝／和／負', '0／0／2'], ['不同對手勝分', '0'], ['不同對手負分', '0'], ['已退賽', '否']],
      },
      {
        team: 'team-a', rank: '排名未確定',
        facts: [['賽事積分', '3'], ['勝／和／負', '1／0／1'], ['不同對手勝分', '0'], ['不同對手負分', '0'], ['已退賽', '是']],
      },
    ])
    expect(finalStage.text()).not.toContain('第 1 名')
    expect(finalStage.text()).not.toContain('第 2 名')
    expect(finalStage.get('.correction-note').text()).toContain('有 1 筆賽果更正紀錄')
    expect(finalStage.get('.superseded-section').text()).toContain('vacant-final-stage-g0-r2-m1')
    expect(wrapper.text()).not.toContain('private semifinal correction history')
    expect(wrapper.text()).not.toContain('private referee detail')
    expect(unresolvedStage.text()).toContain('賽程狀態待主辦方或裁判確認')

    // 第一個新政策 target：已結清的 SE 真決賽要有可存取的冠軍空缺訊息。
    expect(finalStage.findAll('[role="status"], [role="alert"], [aria-live]').map(message => message.text()))
      .toContain('決賽雙負，冠軍空缺')
    expect(finalStage.text()).not.toContain('淘汰賽待裁判處理')
    expect(unresolvedStage.text()).not.toContain('決賽雙負，冠軍空缺')
  } finally {
    await wrapper.unmount()
  }
})

it('displays the authoritative single-elimination final vacancy when the bronze slot came through a semifinal bye', async () => {
  const baseStage = stageFixture('singleElimination', 'prior-vacancy-stage')
  const teamIds = Array.from({ length: 8 }, (_, index) => `team-${index + 1}`)
  const match = (roundNumber: number, sequence: number, participants: string[], result: Record<string, unknown>) => ({
    matchId: `prior-vacancy-stage-r${roundNumber}-m${sequence}`,
    stageId: 'prior-vacancy-stage', roundNumber, sequence, kind: 'main', participants,
    status: 'completed', result, cancellationReason: null, started: result.type !== 'bye',
  })
  const outcome = (winnerId: string, loserId: string) => ({
    type: 'win', winnerId, loserId, winnerGames: 1, loserGames: 0,
  })
  const standings = [
    [teamIds[0]!, 0, 0, 1, 0, 0, true, null],
    [teamIds[1]!, 6, 2, 1, 2, 0, true, null],
    [teamIds[2]!, 3, 1, 1, 1, 1, false, 3],
    [teamIds[3]!, 3, 1, 1, 1, 0, true, null],
    [teamIds[4]!, 0, 0, 1, 0, 1, false, 5],
    [teamIds[5]!, 0, 0, 1, 0, 1, false, 5],
    [teamIds[6]!, 0, 0, 1, 0, 1, false, 5],
    [teamIds[7]!, 0, 0, 1, 0, 0, true, null],
  ].map(([teamId, points, wins, losses, gameWins, gameLosses, dropped, rank]) => ({
    teamId, matchPoints: String(points), wins, draws: 0, losses, gameWins, gameLosses,
    netGameWins: Number(gameWins) - Number(gameLosses), opponentWinRateBasisPoints: null,
    opponentWinScore: '0', opponentLossScore: '0', dropped, rank,
  }))
  const priorVacancyStage = {
    ...baseStage,
    config: { ...baseStage.config, teams: teamIds.map((teamId, index) => ({ teamId, seed: index + 1 })) },
    status: 'finished',
    rounds: [
      { roundNumber: 1, kind: 'main', matches: [
        match(1, 1, [teamIds[0]!, teamIds[7]!], { type: 'doubleLoss' }),
        match(1, 2, [teamIds[3]!, teamIds[4]!], outcome(teamIds[3]!, teamIds[4]!)),
        match(1, 3, [teamIds[1]!, teamIds[6]!], outcome(teamIds[1]!, teamIds[6]!)),
        match(1, 4, [teamIds[2]!, teamIds[5]!], outcome(teamIds[2]!, teamIds[5]!)),
      ] },
      { roundNumber: 2, kind: 'main', matches: [
        match(2, 1, [teamIds[3]!], { type: 'bye', teamId: teamIds[3]! }),
        match(2, 2, [teamIds[1]!, teamIds[2]!], outcome(teamIds[1]!, teamIds[2]!)),
      ] },
      { roundNumber: 3, kind: 'main', matches: [
        match(3, 1, [teamIds[3]!, teamIds[1]!], { type: 'doubleLoss' }),
      ] },
    ],
    standings,
    resolution: null,
    rankingTieDecisions: [],
    corrections: [],
    lastScheduleError: null,
    scheduleGeneration: 0,
    supersededRounds: [],
    teams: Object.fromEntries(teamIds.map((teamId, index) => [teamId, {
      seed: index + 1, dropped: false, hadBye: index === 3,
    }])),
    doubleState: null,
  }
  const response: unknown = {
    competitionId: 'competition a',
    revision: 13,
    projection: { ok: true, operation: 'tournamentView', public: {
      tournamentId: 'competition a', stages: [priorVacancyStage],
    } },
  }
  fetchMock.mockResolvedValueOnce(response)
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  try {
    await flushPromises()
    const stage = wrapper.get('article[aria-labelledby="tournament-stage-prior-vacancy-stage"]')
    expect(stage.findAll('[role="status"]').map(note => note.text())).toEqual([
      '決賽雙負，冠軍空缺',
      '決賽雙負後，唯一合格半決賽敗隊沿原季軍空位取得第三名；冠亞軍空缺',
    ])
    expect(stage.get('.rounds-section').text()).toContain('輪空')
    expect(stage.get('.rounds-section').text()).not.toContain('季軍賽')
    const bronzeStanding = stage.findAll('.standing-card').find(card => card.get('h5').text() === teamIds[2])
    expect(bronzeStanding?.get('.rank-label').text()).toBe('第 3 名')
    expect(stage.text()).not.toContain('第 2 名')
  } finally {
    await wrapper.unmount()
  }
})

it('displays configured zero Team Main Time as unlimited through the valid tournament GET without changing ordinary duration labels', async () => {
  const baseStage = stageFixture('singleElimination', 'zero-main-stage')
  const pendingMatch = {
    matchId: 'zero-main-stage-g0-r1-m1', stageId: 'zero-main-stage',
    roundNumber: 1, sequence: 1, kind: 'main', participants: ['team-a', 'team-b'],
    status: 'pending', result: null, cancellationReason: null, started: false,
  }
  const zeroMainStage = {
    ...baseStage,
    config: {
      ...baseStage.config,
      matchFormat: { type: 'firstToWins', value: 1 },
      clockConfig: { mainTimeMs: 0, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    },
    status: 'active',
    rounds: [{ roundNumber: 1, kind: 'main', matches: [pendingMatch] }],
    standings: [
      {
        teamId: 'team-b', matchPoints: '0', wins: 0, draws: 0, losses: 0,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: null,
      },
      {
        teamId: 'team-a', matchPoints: '0', wins: 0, draws: 0, losses: 0,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, opponentWinScore: '0', opponentLossScore: '0', dropped: false, rank: null,
      },
    ],
    resolution: null,
    rankingTieDecisions: [],
    corrections: [],
    lastScheduleError: null,
    scheduleGeneration: 0,
    supersededRounds: [],
  }
  const finiteMainStage = {
    ...zeroMainStage,
    config: {
      ...zeroMainStage.config,
      stageId: 'finite-main-stage',
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    },
    rounds: [{
      roundNumber: 1,
      kind: 'main',
      matches: [{ ...pendingMatch, matchId: 'finite-main-stage-g0-r1-m1', stageId: 'finite-main-stage' }],
    }],
  }
  // 僅提供合法 GET 的公開 wire；配置主時間 0 不是私有時計耗盡後的剩餘 0。
  const response: unknown = JSON.parse(JSON.stringify({
    competitionId: 'competition a',
    revision: 13,
    projection: {
      ok: true,
      operation: 'tournamentView',
      public: { tournamentId: 'competition a', stages: [zeroMainStage, finiteMainStage] },
    },
  }))
  const { parseCompetitionTournamentViewResponse, tournamentTimeLimitLabel } = await import('~/lib/competition-tournament-view')
  const parsed = parseCompetitionTournamentViewResponse(response, 'competition a')
  expect(parsed.competitionId).toBe('competition a')
  expect(parsed.tournamentId).toBe('competition a')
  expect(parsed.revision).toBe(13)
  expect(parsed.stages.map(stage => ({
    id: stage.config.stageId,
    format: stage.config.format,
    status: stage.status,
    clock: stage.config.clockConfig,
    warning: stage.hasScheduleWarning,
  }))).toEqual([
    {
      id: 'zero-main-stage', format: 'singleElimination', status: 'active',
      clock: { mainTimeMs: 0, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 }, warning: false,
    },
    {
      id: 'finite-main-stage', format: 'singleElimination', status: 'active',
      clock: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 }, warning: false,
    },
  ])
  expect(parsed.stages[0]?.rounds[0]?.matches[0]).toEqual({
    matchId: 'zero-main-stage-g0-r1-m1', stageId: 'zero-main-stage',
    roundNumber: 1, sequence: 1, kind: 'main', purpose: 'standard', participants: ['team-a', 'team-b'],
    status: 'pending', result: null, cancellationReason: null, started: false,
  })
  expect(parsed.stages[0]?.standings.map(row => [row.teamId, row.matchPoints, row.wins, row.draws, row.losses, row.rank]))
    .toEqual([['team-b', '0', 0, 0, 0, null], ['team-a', '0', 0, 0, 0, null]])
  // 保護既有純 duration observable；不以其輸出計算後面的 DOM 政策期望。
  expect(tournamentTimeLimitLabel(0)).toBe('0 秒')
  expect(tournamentTimeLimitLabel(30_000)).toBe('30 秒')

  fetchMock.mockResolvedValueOnce(response)
  const wrapper = await mountSuspended(CompetitionTournamentView, { props: props() })
  try {
    await flushPromises()
    expect(fetchMock).toHaveBeenCalledTimes(1)
    expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/competitions/competition%20a/tournament')
    expect(Object.keys(fetchMock.mock.calls[0]?.[1] ?? {}).sort()).toEqual(['method', 'retry', 'signal'])
    expect(fetchMock.mock.calls[0]?.[1]?.method).toBe('GET')
    expect(fetchMock.mock.calls[0]?.[1]?.retry).toBe(0)
    expect(signalAt(0).aborted).toBe(false)
    expect(JSON.stringify(fetchMock.mock.calls[0])).not.toContain('member-a')
    expect(wrapper.findAll('article.stage-card')).toHaveLength(2)
    expect(wrapper.get('.revision-note').text()).toBe('賽程版本 13')

    const zeroStage = wrapper.get('article[aria-labelledby="tournament-stage-zero-main-stage"]')
    const finiteStage = wrapper.get('article[aria-labelledby="tournament-stage-finite-main-stage"]')
    expect(zeroStage.get('.stage-heading h3').text()).toBe('單敗淘汰')
    expect(zeroStage.get('.stage-heading .status-badge').text()).toBe('進行中')
    expect(zeroStage.get('.rounds-section .match-heading .identifier').text()).toBe('zero-main-stage-g0-r1-m1')
    expect(zeroStage.get('.rounds-section').text()).toContain('尚無賽果')
    expect(zeroStage.findAll('.standing-card').map(card => ({
      team: card.get('h5').text(),
      rank: card.get('.rank-label').text(),
      facts: card.findAll('.standing-facts > div').map(fact => [fact.get('dt').text(), fact.get('dd').text()]),
    }))).toEqual([
      {
        team: 'team-b', rank: '排名未確定',
        facts: [['賽事積分', '0'], ['勝／和／負', '0／0／0'], ['不同對手勝分', '0'], ['不同對手負分', '0'], ['已退賽', '否']],
      },
      {
        team: 'team-a', rank: '排名未確定',
        facts: [['賽事積分', '0'], ['勝／和／負', '0／0／0'], ['不同對手勝分', '0'], ['不同對手負分', '0'], ['已退賽', '否']],
      },
    ])
    expect(finiteStage.get('.stage-heading .status-badge').text()).toBe('進行中')
    expect(finiteStage.get('.rounds-section .match-heading .identifier').text()).toBe('finite-main-stage-g0-r1-m1')
    expect(finiteStage.findAll('.config-section .config-list > div')
      .filter(fact => ['隊伍主時間', '行動讀秒', '回合外選擇時間'].includes(fact.get('dt').text()))
      .map(fact => [fact.get('dt').text(), fact.get('dd').text()]))
      .toEqual([['隊伍主時間', '300 秒'], ['行動讀秒', '30 秒'], ['回合外選擇時間', '30 秒']])
    const zeroFacts = zeroStage.findAll('.config-section .config-list > div')
    expect(zeroFacts.filter(fact => ['行動讀秒', '回合外選擇時間'].includes(fact.get('dt').text()))
      .map(fact => [fact.get('dt').text(), fact.get('dd').text()]))
      .toEqual([['行動讀秒', '30 秒'], ['回合外選擇時間', '30 秒']])
    const zeroMainFact = zeroFacts.find(fact => fact.get('dt').text() === '隊伍主時間')
    if (!zeroMainFact) throw new Error('missing configured Team Main Time fact')

    // 第一個政策 target 在真 DOM 設定列；期望直接來自已批准 design:27。
    expect(zeroMainFact.get('dd').text()).toBe('不限時')
  } finally {
    await wrapper.unmount()
  }
})
