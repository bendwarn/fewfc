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
        teamId: 'team-b', matchPoints: 0, wins: 0, draws: 0, losses: 1,
        gameWins: 1, gameLosses: 2, netGameWins: -1,
        opponentWinRateBasisPoints: null, dropped: false, rank: null,
      },
      {
        teamId: 'team-a', matchPoints: 3, wins: 1, draws: 0, losses: 0,
        gameWins: 2, gameLosses: 1, netGameWins: 1,
        opponentWinRateBasisPoints: 5_000, dropped: false, rank: 1,
      },
    ],
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

function props() {
  return {
    competitionId: ' competition a ',
    sessionIdentity: 'member-a',
    contextRevision: 7,
    contextReady: true,
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
  expect(wrapper.text()).toContain('淘汰賽待裁判處理')
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
  expect(wrapper.get('[role="alert"]').text()).toBe('目前帳號沒有這場賽事的賽程讀取權限。')
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
        matches: [{
          matchId: 'vacant-final-stage-g1-r2-m1', stageId: 'vacant-final-stage',
          roundNumber: 2, sequence: 1, kind: 'main', participants: ['team-a', 'team-c'],
          status: 'completed', result: { type: 'doubleLoss' }, cancellationReason: null, started: true,
        }],
      },
    ],
    // 保留服務提供的順序；位置與 null rank 都不是冠軍判定的來源。
    standings: [
      {
        teamId: 'team-b', matchPoints: 0, wins: 0, draws: 0, losses: 1,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, dropped: false, rank: null,
      },
      {
        teamId: 'team-c', matchPoints: 3, wins: 1, draws: 0, losses: 1,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, dropped: false, rank: null,
      },
      {
        teamId: 'team-d', matchPoints: 0, wins: 0, draws: 0, losses: 1,
        gameWins: 0, gameLosses: 1, netGameWins: -1,
        opponentWinRateBasisPoints: null, dropped: false, rank: null,
      },
      {
        teamId: 'team-a', matchPoints: 3, wins: 1, draws: 0, losses: 1,
        gameWins: 1, gameLosses: 0, netGameWins: 1,
        opponentWinRateBasisPoints: null, dropped: false, rank: null,
      },
    ],
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
    matches: [{
      matchId: 'vacant-final-stage-g1-r2-m1', stageId: 'vacant-final-stage',
      roundNumber: 2, sequence: 1, kind: 'main', participants: ['team-a', 'team-c'],
      status: 'completed', result: { type: 'doubleLoss' }, cancellationReason: null, started: true,
    }],
  })
  expect(parsedFinal.standings.map(row => [row.teamId, row.rank])).toEqual([
    ['team-b', null], ['team-c', null], ['team-d', null], ['team-a', null],
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
      'vacant-final-stage-g0-r1-m1', 'vacant-final-stage-g0-r1-m2', 'vacant-final-stage-g1-r2-m1',
    ])
    expect(finalStage.get('.rounds-section').text()).toContain('雙方判負（不推定勝者）')
    expect(finalStage.findAll('.standing-card').map(card => ({
      team: card.get('h5').text(),
      rank: card.get('.rank-label').text(),
      facts: card.findAll('.standing-facts > div').map(fact => [fact.get('dt').text(), fact.get('dd').text()]),
    }))).toEqual([
      {
        team: 'team-b', rank: '排名未確定',
        facts: [['賽事積分', '0'], ['勝／和／負', '0／0／1'], ['勝／負局', '0／0'], ['淨勝局', '0'], ['對手勝率基點', '未提供'], ['已退賽', '否']],
      },
      {
        team: 'team-c', rank: '排名未確定',
        facts: [['賽事積分', '3'], ['勝／和／負', '1／0／1'], ['勝／負局', '0／0'], ['淨勝局', '0'], ['對手勝率基點', '未提供'], ['已退賽', '否']],
      },
      {
        team: 'team-d', rank: '排名未確定',
        facts: [['賽事積分', '0'], ['勝／和／負', '0／0／1'], ['勝／負局', '0／1'], ['淨勝局', '-1'], ['對手勝率基點', '未提供'], ['已退賽', '否']],
      },
      {
        team: 'team-a', rank: '排名未確定',
        facts: [['賽事積分', '3'], ['勝／和／負', '1／0／1'], ['勝／負局', '1／0'], ['淨勝局', '1'], ['對手勝率基點', '未提供'], ['已退賽', '否']],
      },
    ])
    expect(finalStage.text()).not.toContain('第 1 名')
    expect(finalStage.text()).not.toContain('第 2 名')
    expect(finalStage.get('.correction-note').text()).toContain('有 1 筆賽果更正紀錄')
    expect(finalStage.get('.superseded-section').text()).toContain('vacant-final-stage-g0-r2-m1')
    expect(wrapper.text()).not.toContain('private semifinal correction history')
    expect(wrapper.text()).not.toContain('private referee detail')
    expect(unresolvedStage.text()).toContain('淘汰賽待裁判處理')

    // 第一個新政策 target：已結清的 SE 真決賽要有可存取的冠軍空缺訊息。
    expect(finalStage.findAll('[role="status"], [role="alert"], [aria-live]').map(message => message.text()))
      .toContain('決賽雙負，冠軍空缺')
    expect(finalStage.text()).not.toContain('淘汰賽待裁判處理')
    expect(unresolvedStage.text()).not.toContain('決賽雙負，冠軍空缺')
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
        teamId: 'team-b', matchPoints: 0, wins: 0, draws: 0, losses: 0,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, dropped: false, rank: null,
      },
      {
        teamId: 'team-a', matchPoints: 0, wins: 0, draws: 0, losses: 0,
        gameWins: 0, gameLosses: 0, netGameWins: 0,
        opponentWinRateBasisPoints: null, dropped: false, rank: null,
      },
    ],
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
    roundNumber: 1, sequence: 1, kind: 'main', participants: ['team-a', 'team-b'],
    status: 'pending', result: null, cancellationReason: null, started: false,
  })
  expect(parsed.stages[0]?.standings.map(row => [row.teamId, row.matchPoints, row.wins, row.draws, row.losses, row.rank]))
    .toEqual([['team-b', 0, 0, 0, 0, null], ['team-a', 0, 0, 0, 0, null]])
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
        facts: [['賽事積分', '0'], ['勝／和／負', '0／0／0'], ['勝／負局', '0／0'], ['淨勝局', '0'], ['對手勝率基點', '未提供'], ['已退賽', '否']],
      },
      {
        team: 'team-a', rank: '排名未確定',
        facts: [['賽事積分', '0'], ['勝／和／負', '0／0／0'], ['勝／負局', '0／0'], ['淨勝局', '0'], ['對手勝率基點', '未提供'], ['已退賽', '否']],
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
