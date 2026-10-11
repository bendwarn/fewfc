import { mountSuspended, mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { CompetitionRegistrationState } from '../../app/lib/competition-registration'
import { formatCompetitionDeadline } from '../../app/lib/competition-registration'
import { competitionRoundReadyCutoffMs } from '../../shared/competition-game-room'
import CompetitionRoundPublication from '~/components/CompetitionRoundPublication.vue'

const fetchMock = vi.hoisted(() => vi.fn())
mockNuxtImport('$fetch', () => fetchMock)

const competitionId = 'competition-a'
const organizerId = 'organizer-a'
const stageId = 'stage-a'
const matchIds = ['match-a', 'match-b']
const nowMs = new Date(2030, 0, 2, 12, 5).getTime()
const roundStartMs = new Date(2030, 0, 2, 12, 10).getTime()
const roundCutoffMs = competitionRoundReadyCutoffMs(roundStartMs)
if (roundCutoffMs === null) throw new Error('測試公告時間必須有可表示的房間確認截止時間')
const tournamentPath = '/api/competitions/competition-a/tournament'
const publicationPath = '/api/competitions/competition-a/stages/stage-a/rounds/1/publication'

function registration(id = competitionId): CompetitionRegistrationState {
  return {
    competitionId: id, revision: 4, organizer: organizerId, ruleVersion: '5.17', ruleset: 'base',
    enabledRuleModules: [], teamSize: 1, initialHp: 200, registrationMode: 'public', personalDecks: 'disabled',
    registrationDeadline: 1_893_456_000_000, checkInDeadline: 1_893_456_300_000, lifecycle: 'locked',
    plan: {
      initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
      swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
      rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
      exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
      stages: [{ stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: null, swissMatchPoints: null, qualification: { type: 'allEligibleTeams' } }],
    },
    teams: [], ownTeam: null,
  }
}

function match(matchId: string, status: 'pending' | 'inProgress' | 'completed' = 'pending', options: {
  roundNumber?: number, sequence?: number, participants?: string[], result?: unknown, started?: boolean,
} = {}) {
  const defaultParticipants = matchId === 'match-a' ? ['team-a', 'team-b'] : ['team-c', 'team-d']
  return { matchId, stageId, roundNumber: options.roundNumber ?? 1,
    sequence: options.sequence ?? Math.max(0, matchIds.indexOf(matchId)), kind: 'main',
    participants: options.participants ?? defaultParticipants, status,
    result: options.result ?? null, cancellationReason: null, started: options.started ?? false }
}

function tournament(revision = 17, id = competitionId, rounds: unknown[] = [{
  roundNumber: 1, kind: 'main', matches: matchIds.map(id => match(id)),
}]) {
  return {
    competitionId: id, revision,
    projection: { ok: true, operation: 'tournamentView', public: { tournamentId: id, stages: [{
      config: { stageId, format: 'roundRobin', teams: [
        { teamId: 'team-a', seed: 1 }, { teamId: 'team-b', seed: 2 },
        { teamId: 'team-c', seed: 3 }, { teamId: 'team-d', seed: 4 },
      ], randomSeed: 77, matchFormat: { type: 'fixedGames', value: 1 },
      clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 15_000 },
      swissRounds: null, swissPolicy: null },
      status: 'active', rounds,
      standings: [], corrections: [], lastScheduleError: null, scheduleGeneration: 0, supersededRounds: [],
      teams: {
        'team-a': { seed: 1, dropped: false, hadBye: false },
        'team-b': { seed: 2, dropped: false, hadBye: false },
        'team-c': { seed: 3, dropped: false, hadBye: false },
        'team-d': { seed: 4, dropped: false, hadBye: false },
      }, doubleState: null,
    }] } },
  }
}

function tournamentAfterProgress(state: 'nextRound' | 'blocked') {
  if (state === 'blocked') {
    return tournament(25, competitionId, [{
      roundNumber: 1, kind: 'main', matches: matchIds.map(id => match(id, 'inProgress', { started: true })),
    }])
  }
  return tournament(25, competitionId, [
    { roundNumber: 1, kind: 'main', matches: matchIds.map(id => match(id, 'completed', {
      started: true, result: { type: 'draw', firstGames: 1, secondGames: 1 },
    })) },
    { roundNumber: 2, kind: 'main', matches: [
      match('match-next-a', 'pending', { roundNumber: 2, sequence: 0, participants: ['team-a', 'team-c'] }),
      match('match-next-b', 'pending', { roundNumber: 2, sequence: 1, participants: ['team-b', 'team-d'] }),
    ] },
  ])
}

function publication(status: 'materializing' | 'released' | 'quiescing' | 'retired' = 'materializing', requiresReferee = false) {
  return {
    roundRef: { competitionId, stageId, roundNumber: 1, roundGenerationId: 'generation-a' },
    roundStartMs, publicationVersion: 1, status,
    releaseDecisionId: status === 'materializing' ? null : 'release-a', requiresReferee,
    rooms: matchIds.map((matchId, index) => ({ matchId, gameId: `game-${index}`,
      ackConfirmedAtMs: status === 'released' ? roundCutoffMs : index === 0 ? roundCutoffMs : null })),
  }
}

function publicationRead(pub: unknown = null, revision = 17, id = competitionId) {
  return { competitionId: id, revision, publication: pub }
}

function announcementAck(
  body: { commandId: string, expectedRevision: number, roundStartMs: number }, idempotent = false,
) {
  const initialPublication = publication()
  return {
    competitionId, revision: body.expectedRevision + 1, commandId: body.commandId, idempotent,
    publication: {
      ...initialPublication,
      roundStartMs: body.roundStartMs,
      rooms: initialPublication.rooms.map(room => ({ ...room, ackConfirmedAtMs: null })),
    },
  }
}

function deferred() {
  let resolve!: (value: unknown) => void
  let reject!: (value: unknown) => void
  const promise = new Promise<unknown>((accept, refuse) => { resolve = accept; reject = refuse })
  return { promise, resolve, reject }
}

function bodyAt(index: number): unknown { return fetchMock.mock.calls[index]?.[1]?.body }
function signalAt(index: number): AbortSignal {
  const value: unknown = fetchMock.mock.calls[index]?.[1]?.signal
  if (!(value instanceof AbortSignal)) throw new Error('HTTP request 必須帶有可取消 signal')
  return value
}

const cleanups: (() => void)[] = []
async function mountPublication(overrides: Partial<{
  competitionId: string, sessionUserId: string | null, registration: CompetitionRegistrationState | null,
  contextReady: boolean, busy: boolean,
}> = {}) {
  const host = document.createElement('div')
  document.body.append(host)
  const wrapper = await mountSuspended(CompetitionRoundPublication, { attachTo: host, props: {
    competitionId, sessionUserId: organizerId, registration: registration(), contextReady: true, busy: false, ...overrides,
  } })
  cleanups.push(() => { if (wrapper.exists()) wrapper.unmount(); host.remove() })
  await flushPromises()
  return wrapper
}

type PublicationWrapper = Awaited<ReturnType<typeof mountPublication>>
function installEmptyPublicationReads(viewRevision = 17) {
  fetchMock.mockImplementation((path: string, options?: { method?: string, body?: unknown }) => {
    if (path === tournamentPath && options?.method === 'GET') return Promise.resolve(tournament(viewRevision))
    if (path === publicationPath && options?.method === 'GET') return Promise.resolve(publicationRead(null, viewRevision))
    if (path === publicationPath && options?.method === 'POST') {
      const body = options.body as { commandId: string, expectedRevision: number, roundStartMs: number }
      return Promise.resolve(announcementAck(body))
    }
    return Promise.reject(new Error('unexpected test transport path'))
  })
}

beforeEach(() => {
  fetchMock.mockReset()
  vi.spyOn(Date, 'now').mockReturnValue(nowMs)
})
afterEach(() => {
  for (const cleanup of cleanups.splice(0)) cleanup()
  vi.restoreAllMocks()
})

it('只在賽事已就緒、已鎖定且原公告主辦者 context 讀取；玩家、錯誤身分與載入中不呼叫 HTTP', async () => {
  const player = await mountPublication({ sessionUserId: 'player-a' })
  expect(fetchMock).toHaveBeenCalledTimes(0)
  expect(player.text()).toContain('僅原公告主辦者')
  await player.unmount()

  const mismatched = await mountPublication({ registration: registration('other-competition') })
  expect(fetchMock).toHaveBeenCalledTimes(0)
  expect(mismatched.text()).toContain('賽事身分無法確認')
  await mismatched.unmount()

  const loading = await mountPublication({ contextReady: false })
  expect(fetchMock).toHaveBeenCalledTimes(0)
  installEmptyPublicationReads()
  await loading.setProps({ contextReady: true })
  await flushPromises()
  expect(fetchMock.mock.calls[0]?.[0]).toBe(tournamentPath)
  expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: 'GET', retry: 0 })
  expect(loading.find('.technical-identifiers').text()).toContain('階段識別碼：stage-a')
  expect(loading.find('.technical-identifiers').text()).toContain('交手識別碼：match-a')
})

it('顯示四種公告狀態、每房 T−5 分鐘 ACK 與需裁判處置，不把 released 說成開局', async () => {
  const labels = [
    ['materializing', '整輪房間正在準備中'], ['released', '後端已釋出本輪公告'],
    ['quiescing', '本輪公告正在處置中'], ['retired', '本輪公告已退役'],
  ] as const
  for (const [status, label] of labels) {
    fetchMock.mockReset()
    fetchMock.mockResolvedValueOnce(tournament()).mockResolvedValueOnce(publicationRead(publication(status), 17))
    const wrapper = await mountPublication()
    expect(wrapper.text()).toContain(label)
    expect(wrapper.text()).toContain(formatCompetitionDeadline(roundStartMs))
    expect(wrapper.text()).toContain(formatCompetitionDeadline(roundCutoffMs))
    expect(wrapper.text()).toContain(status === 'released'
      ? `伺服器確認（ACK）：${formatCompetitionDeadline(roundCutoffMs)}`
      : '伺服器確認（ACK）：尚未收到')
    expect(wrapper.text()).toContain('公告不代表正式開局、全員已準備，或計時已開始')
    const publicStatusText = wrapper.findAll('.publication-state > p').map(paragraph => paragraph.text()).join(' ')
    expect(publicStatusText).not.toContain(stageId)
    expect(publicStatusText).not.toContain('match-a')
    expect(publicStatusText).not.toContain('game-0')
    const roomAckText = wrapper.find('.publication-state > ul').text()
    expect(roomAckText).toContain('房間 1 · 伺服器確認（ACK）：')
    expect(roomAckText).not.toContain('match-a')
    expect(roomAckText).not.toContain('game-0')
    const identifiers = wrapper.find('.publication-state .technical-identifiers')
    expect(identifiers.text()).toContain(`階段識別碼：${stageId}`)
    expect(identifiers.text()).toContain('交手識別碼：match-a')
    expect(identifiers.text()).toContain('房間識別碼：game-0')
    expect(wrapper.text()).toContain('目前輪次：第 1 輪')
    expect(wrapper.find('[data-testid="round-announcement-form"]').exists()).toBe(false)
    await wrapper.unmount()
  }

  fetchMock.mockReset()
  fetchMock.mockResolvedValueOnce(tournament()).mockResolvedValueOnce(publicationRead(publication('materializing', true), 17))
  const needsReferee = await mountPublication()
  expect(needsReferee.text()).toContain('需要裁判處置')
  expect(needsReferee.text()).not.toContain('已指派裁判')
})

it('POST 只送 exact 四欄與全域 GET revision；成功 receipt 精確核對後才 emit announced', async () => {
  installEmptyPublicationReads(23)
  const wrapper = await mountPublication()
  await wrapper.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  expect(wrapper.text()).toContain(formatCompetitionDeadline(roundStartMs))
  expect(wrapper.text()).toContain('本元件不含裁判指派編輯')
  await wrapper.get('[data-testid="round-announcement-form"]').trigger('submit')
  await flushPromises()

  expect(fetchMock.mock.calls.map(call => call[0])).toEqual([tournamentPath, publicationPath, publicationPath])
  expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({ method: 'GET', retry: 0 })
  expect(fetchMock.mock.calls[2]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  const body = bodyAt(2) as Record<string, unknown>
  expect(Object.keys(body)).toEqual(['commandId', 'expectedRevision', 'roundStartMs', 'refereeAssignments'])
  expect(body).toMatchObject({ expectedRevision: 23, roundStartMs, refereeAssignments: [] })
  expect(typeof body.commandId).toBe('string')
  expect(Object.keys(body)).not.toContain('callerUserId')
  expect(Object.keys(body)).not.toContain('trustedTimeMs')
  expect(wrapper.emitted('announced')?.[0]?.[0]).toMatchObject({ revision: 24, roundStartMs })
  expect(wrapper.text()).toContain('公告收據已確認')
})

it('T 不足五分鐘或含秒／毫秒時不送出；時限提示不自動改 T', async () => {
  installEmptyPublicationReads()
  const wrapper = await mountPublication()
  const field = wrapper.get('[data-testid="round-start-time"]')
  for (const value of ['2030-01-02T12:10:15', '2030-01-02T12:10:15.006', '2030-01-02T12:09']) {
    await field.setValue(value)
    await wrapper.get('[data-testid="round-announcement-form"]').trigger('submit')
    await flushPromises()
    expect(fetchMock.mock.calls.some(call => call[1]?.method === 'POST')).toBe(false)
    expect(wrapper.text()).toContain(value.endsWith('12:09')
      ? '至少須比目前時間晚五分鐘'
      : '比賽開始時間請以分鐘為單位輸入')
  }
  expect(fetchMock).toHaveBeenCalledTimes(2)
})

it('unknown POST 保留原 literal body；refresh 即使讀到相同公告也不結清，只能手動同命令重試', async () => {
  let tournamentReads = 0
  let publicationReads = 0
  let posts = 0
  fetchMock.mockImplementation((path: string, options?: { method?: string, body?: unknown }) => {
    if (path === tournamentPath && options?.method === 'GET') {
      tournamentReads += 1
      return Promise.resolve(tournament(tournamentReads === 1 ? 17 : 18))
    }
    if (path === publicationPath && options?.method === 'GET') {
      publicationReads += 1
      return Promise.resolve(publicationRead(publicationReads === 1 ? null : publication(), publicationReads === 1 ? 17 : 18))
    }
    if (path === publicationPath && options?.method === 'POST') {
      posts += 1
      if (posts === 1) return Promise.reject({ statusCode: 503, message: 'private server detail' })
      const body = options.body as { commandId: string, expectedRevision: number, roundStartMs: number }
      return Promise.resolve(announcementAck(body))
    }
    return Promise.reject(new Error('unexpected test transport path'))
  })
  const wrapper = await mountPublication()
  await wrapper.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  await wrapper.get('[data-testid="round-announcement-form"]').trigger('submit')
  await flushPromises()
  const firstBody = bodyAt(2)
  expect(wrapper.text()).not.toContain('private server detail')
  expect(wrapper.get('[data-testid="round-same-command-retry"]').exists()).toBe(true)

  await wrapper.setProps({ busy: true })
  expect(wrapper.get('[data-testid="round-same-command-retry"]').element.disabled).toBe(true)
  expect(wrapper.text()).toContain('原公告命令仍未確認')
  await wrapper.setProps({ busy: false })
  expect(wrapper.get('[data-testid="round-same-command-retry"]').element.disabled).toBe(false)

  await wrapper.get('[data-testid="round-publication-refresh"]').trigger('click')
  await flushPromises()
  expect(wrapper.get('[data-testid="round-same-command-retry"]').exists()).toBe(true)
  expect(wrapper.text()).toContain('原公告命令仍未確認')
  await wrapper.get('[data-testid="round-same-command-retry"]').trigger('click')
  await flushPromises()
  const postCalls = fetchMock.mock.calls.filter(call => call[1]?.method === 'POST')
  expect(postCalls[1]?.[1]?.body).toBe(firstBody)
  expect(wrapper.emitted('announced')?.[0]?.[0]).toMatchObject({ revision: 18, roundStartMs })
  expect(wrapper.find('[data-testid="round-same-command-retry"]').exists()).toBe(false)
})

it.each([
  ['下一輪已成為可公告輪次', 'nextRound'],
  ['目前輪次已開始而無可公告目標', 'blocked'],
] as const)('同身分賽程進度改變後仍可手動重送並接收原公告收據（%s）', async (_label, state) => {
  let tournamentReads = 0
  let posts = 0
  let committedReceipt: ReturnType<typeof announcementAck> | null = null
  const progressedView = tournamentAfterProgress(state)
  const nextPublicationPath = '/api/competitions/competition-a/stages/stage-a/rounds/2/publication'
  fetchMock.mockImplementation((path: string, options?: { method?: string, body?: unknown }) => {
    if (path === tournamentPath && options?.method === 'GET') {
      tournamentReads += 1
      return Promise.resolve(tournamentReads === 1 ? tournament() : progressedView)
    }
    if (path === publicationPath && options?.method === 'GET') return Promise.resolve(publicationRead(null, 17))
    if (path === nextPublicationPath && options?.method === 'GET') return Promise.resolve(publicationRead(null, 25))
    if (path === publicationPath && options?.method === 'POST') {
      posts += 1
      if (posts === 1) {
        committedReceipt = announcementAck(
          options.body as { commandId: string, expectedRevision: number, roundStartMs: number }, true,
        )
        return Promise.reject({ statusCode: 503, message: 'private server detail' })
      }
      return committedReceipt === null
        ? Promise.reject(new Error('unknown command was not committed before the response was lost'))
        : Promise.resolve(committedReceipt)
    }
    return Promise.reject(new Error('unexpected test transport path'))
  })

  const wrapper = await mountPublication()
  await wrapper.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  await wrapper.get('[data-testid="round-announcement-form"]').trigger('submit')
  await flushPromises()
  const firstPost = fetchMock.mock.calls.find(call => call[1]?.method === 'POST')
  const originalBody = firstPost?.[1]?.body as {
    commandId: string, expectedRevision: number, roundStartMs: number, refereeAssignments: unknown[],
  }
  expect(firstPost?.[0]).toBe(publicationPath)
  expect(originalBody).toMatchObject({ expectedRevision: 17, roundStartMs, refereeAssignments: [] })
  expect(wrapper.text()).toContain('原公告命令仍未確認')

  await wrapper.get('[data-testid="round-publication-refresh"]').trigger('click')
  await flushPromises()
  expect(tournamentReads).toBe(2)
  expect(posts).toBe(1)
  if (state === 'nextRound') {
    expect(wrapper.text()).toContain('目前輪次：第 2 輪')
    expect(fetchMock.mock.calls.some(call => call[0] === nextPublicationPath && call[1]?.method === 'GET')).toBe(true)
  } else {
    expect(wrapper.text()).toContain('目前輪次已有交手開始')
  }
  expect(wrapper.text()).toContain('原公告命令仍未確認')
  expect(committedReceipt).toMatchObject({
    commandId: originalBody.commandId, revision: 18, idempotent: true,
    publication: { roundStartMs, roundRef: { competitionId, stageId, roundNumber: 1 } },
  })
  expect(wrapper.find('[data-testid="round-same-command-retry"]').exists()).toBe(true)

  await wrapper.get('[data-testid="round-same-command-retry"]').trigger('click')
  await flushPromises()
  const postCalls = fetchMock.mock.calls.filter(call => call[1]?.method === 'POST')
  expect(postCalls).toHaveLength(2)
  expect(postCalls[1]?.[0]).toBe(publicationPath)
  expect(postCalls[1]?.[1]?.body).toBe(originalBody)
  expect(postCalls[1]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  expect(postCalls[1]?.[1]?.body).toMatchObject({
    commandId: originalBody.commandId, expectedRevision: 17, roundStartMs, refereeAssignments: [],
  })
  expect(wrapper.emitted('announced')?.[0]?.[0]).toMatchObject({
    competitionId, stageId, roundNumber: 1, commandId: originalBody.commandId, revision: 18, roundStartMs,
  })
  expect(wrapper.text()).toContain('公告收據已確認')
})

it('跨 identity 的舊 GET／POST 都 abort 並忽略晚結果；未知命令保留並向父回報，unmount 再通知 unresolved', async () => {
  const tournamentRead = deferred()
  fetchMock.mockReturnValueOnce(tournamentRead.promise)
  const wrapper = await mountPublication()
  expect(fetchMock).toHaveBeenCalledTimes(1)
  await wrapper.setProps({ sessionUserId: 'player-a' })
  expect(signalAt(0).aborted).toBe(true)
  tournamentRead.resolve(tournament())
  await flushPromises()
  expect(wrapper.find('[data-testid="round-announcement-form"]').exists()).toBe(false)

  wrapper.unmount()
  fetchMock.mockReset()
  installEmptyPublicationReads()
  const mounted = await mountPublication()
  const post = deferred()
  fetchMock.mockImplementation((path: string, options?: { method?: string }) => {
    if (path === tournamentPath) return Promise.resolve(tournament())
    if (path === publicationPath && options?.method === 'GET') return Promise.resolve(publicationRead(null))
    if (options?.method === 'POST') return post.promise
    return Promise.reject(new Error('unexpected test transport path'))
  })
  await mounted.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  await mounted.get('[data-testid="round-announcement-form"]').trigger('submit')
  await flushPromises()
  const postIndex = fetchMock.mock.calls.findIndex(call => call[1]?.method === 'POST')
  expect(postIndex).toBeGreaterThanOrEqual(0)
  await mounted.setProps({ busy: true })
  expect(signalAt(postIndex).aborted).toBe(false)
  await mounted.setProps({ busy: false })
  expect(signalAt(postIndex).aborted).toBe(false)
  await mounted.setProps({ sessionUserId: 'player-a' })
  expect(signalAt(postIndex).aborted).toBe(true)
  expect(mounted.text()).toContain('原公告命令仍未確認')
  expect(mounted.emitted('unresolved')).toBeTruthy()
  post.resolve(announcementAck(bodyAt(postIndex) as { commandId: string, expectedRevision: number, roundStartMs: number }))
  await flushPromises()
  expect(mounted.emitted('announced')).toBeUndefined()
  expect(fetchMock.mock.calls.filter(call => call[1]?.method === 'POST')).toHaveLength(1)
  mounted.unmount()
  expect(mounted.emitted('unresolved')?.length).toBeGreaterThanOrEqual(1)

  fetchMock.mockReset()
  installEmptyPublicationReads()
  const unmounted = await mountPublication()
  const abandonedPost = deferred()
  fetchMock.mockImplementation((path: string, options?: { method?: string }) => {
    if (path === tournamentPath) return Promise.resolve(tournament())
    if (path === publicationPath && options?.method === 'GET') return Promise.resolve(publicationRead(null))
    if (options?.method === 'POST') return abandonedPost.promise
    return Promise.reject(new Error('unexpected test transport path'))
  })
  await unmounted.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  await unmounted.get('[data-testid="round-announcement-form"]').trigger('submit')
  await flushPromises()
  const abandonedIndex = fetchMock.mock.calls.findIndex(call => call[1]?.method === 'POST')
  expect(abandonedIndex).toBeGreaterThanOrEqual(0)
  unmounted.unmount()
  expect(signalAt(abandonedIndex).aborted).toBe(true)
  expect(unmounted.emitted('unresolved')).toBeTruthy()
  abandonedPost.resolve(announcementAck(bodyAt(abandonedIndex) as {
    commandId: string, expectedRevision: number, roundStartMs: number,
  }))
  await flushPromises()
  expect(unmounted.emitted('announced')).toBeUndefined()
})
