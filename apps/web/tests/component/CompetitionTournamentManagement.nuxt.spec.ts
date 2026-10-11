import { mountSuspended, mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { StageConfig } from '../../shared/competition'
import type { CompetitionRegistrationState } from '../../app/lib/competition-registration'
import CompetitionTournamentManagement from '~/components/CompetitionTournamentManagement.vue'

const fetchMock = vi.hoisted(() => vi.fn())
mockNuxtImport('$fetch', () => fetchMock)
const teamIds: [string, string] = ['b45abe94-a252-4e3d-bff2-7d26c6e2fabc', '535ec115-7bf4-4bbf-b0d3-c6e0ee76c8b7']
const stageId = 'cd0d3e8b-7c87-46c1-b50d-deb6fb2ad5fa'
const commandId = 'b9ae10af-54da-429e-a10c-5e8de84021a2'
const notReady = { statusCode: 409, data: { code: 'notReady' } }
function registration(revision = 4): CompetitionRegistrationState {
  return { competitionId: 'competition a', revision, organizer: 'organizer-a', ruleVersion: '5.17', ruleset: 'base',
    enabledRuleModules: [], teamSize: 1, initialHp: 200, registrationMode: 'public', personalDecks: 'disabled',
    registrationDeadline: 60_000, checkInDeadline: 120_000, lifecycle: 'locked', ownTeam: null,
    plan: {
      initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
      swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
      rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
      exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
      stages: [{ stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: null, swissMatchPoints: null, qualification: { type: 'allEligibleTeams' } }],
    },
    teams: teamIds.map((teamId, index) => ({ teamId, captain: `captain-${index}`, members: [`captain-${index}`],
      rosterVersion: 1, confirmedMembers: [`captain-${index}`], status: 'checkedIn', exclusionReasons: [] })) }
}
function stage(): StageConfig {
  return { stageId, format: 'roundRobin', teams: [{ teamId: teamIds[0], seed: 2 }, { teamId: teamIds[1], seed: 1 }], randomSeed: 77,
    matchFormat: { type: 'fixedGames', value: 3 }, clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    swissRounds: null, swissPolicy: null }
}
// 按 Rust camelCase 的 DTO fixture；此檔不證明 DO／native 或實際 390px 幾何。
function publicSnapshot(config: StageConfig = stage()) {
  return { tournamentId: 'competition a', stages: [{ config, status: 'active', rounds: [], standings: [], corrections: [],
    lastScheduleError: null, scheduleGeneration: 0, supersededRounds: [],
    teams: Object.fromEntries(config.teams.map(team => [team.teamId, { seed: team.seed, dropped: false, hadBye: false }])),
    doubleState: config.format === 'doubleElimination' ? { phase: 'winners', winnersPool: [...teamIds], losersWaiting: [], lowerPool: [],
      winnersChampion: null, losersChampion: null, grandFinalWinner: null } : null }] }
}
function read(revision = 5, config = stage()) {
  return { competitionId: 'competition a', revision, projection: { ok: true, operation: 'tournamentView', public: publicSnapshot(config) } }
}
function ack(config = stage()) {
  return { competitionId: 'competition a', revision: 5, commandId, idempotent: false,
    projection: { ok: true, operation: 'tournamentCreate', public: publicSnapshot(config) } }
}
function deferred() {
  let resolve!: (value: unknown) => void
  let reject!: (value: unknown) => void
  const promise = new Promise<unknown>((accept, refuse) => { resolve = accept; reject = refuse })
  return { promise, resolve, reject }
}
function signalAt(index: number): AbortSignal {
  const value: unknown = fetchMock.mock.calls[index]?.[1]?.signal
  if (!(value instanceof AbortSignal)) throw new Error('專用 HTTP request 需要 signal')
  return value
}
function bodyAt(index: number): unknown { return fetchMock.mock.calls[index]?.[1]?.body }
const cleanups: (() => void)[] = []
async function mountManagement(overrides: Partial<{
  competitionId: string, sessionUserId: string | null, registration: CompetitionRegistrationState | null, contextReady: boolean, busy: boolean,
}> = {}) {
  const host = document.createElement('div')
  document.body.append(host)
  const wrapper = await mountSuspended(CompetitionTournamentManagement, { attachTo: host, props: {
    competitionId: 'competition a', sessionUserId: 'organizer-a', registration: registration(), contextReady: true, busy: false, ...overrides,
  } })
  cleanups.push(() => { if (wrapper.exists()) wrapper.unmount(); host.remove() })
  await flushPromises()
  return wrapper
}
type ManagementWrapper = Awaited<ReturnType<typeof mountManagement>>
async function draft(wrapper: ManagementWrapper) {
  await wrapper.get('[data-testid="tournament-format"]').setValue('roundRobin')
  await wrapper.get('[data-testid="tournament-team-seed-0"]').setValue('2')
  await wrapper.get('[data-testid="tournament-team-seed-1"]').setValue('1')
  await wrapper.get('[data-testid="tournament-random-seed"]').setValue('77')
  await wrapper.get('[data-testid="tournament-games"]').setValue('3')
  await wrapper.get('[data-testid="tournament-byoyomi"]').setValue('30')
}
async function review(wrapper: ManagementWrapper) {
  await draft(wrapper)
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  await wrapper.get('[data-testid="tournament-acknowledge"]').setValue(true)
}
async function submit(wrapper: ManagementWrapper) {
  await review(wrapper)
  await wrapper.get('[data-testid="tournament-confirm-create"]').trigger('click')
  await flushPromises()
}
function expectDisabled(wrapper: ManagementWrapper, testId: string) {
  const element = wrapper.get(`[data-testid="${testId}"]`).element
  // happy-dom 的 :disabled 只驗 own attribute；原生 fieldset 的整組停用另驗公共 DOM。
  expect(element.matches(':disabled') || element.closest('fieldset[disabled]') !== null).toBe(true)
}
beforeEach(() => {
  fetchMock.mockReset()
  vi.spyOn(crypto, 'randomUUID').mockReturnValueOnce(stageId).mockReturnValueOnce(commandId)
})
afterEach(() => {
  for (const cleanup of cleanups.splice(0)) cleanup()
  vi.restoreAllMocks()
})

it('摘要與明示確認後才 POST exact3／stage8；五欄 ACK 後才 emit 並 readonly refresh', async () => {
  fetchMock.mockRejectedValueOnce(notReady).mockResolvedValueOnce(ack()).mockResolvedValueOnce(read())
  const wrapper = await mountManagement()
  expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/competitions/competition%20a/tournament')
  expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: 'GET', retry: 0 })
  expect(bodyAt(0)).toBeUndefined()
  await draft(wrapper)
  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(crypto.randomUUID).toHaveBeenCalledTimes(0)
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  expect(wrapper.get('[data-testid="tournament-summary"]').text()).toContain('排程亂數種子')
  expectDisabled(wrapper, 'tournament-confirm-create')
  expect(fetchMock).toHaveBeenCalledTimes(1)
  await wrapper.get('[data-testid="tournament-acknowledge"]').setValue(true)
  await wrapper.get('[data-testid="tournament-confirm-create"]').trigger('click')
  await flushPromises()
  expect(bodyAt(1)).toEqual({ commandId, expectedRevision: 4, stage: stage() })
  expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  expect(crypto.randomUUID).toHaveBeenCalledTimes(2)
  expect(wrapper.emitted('created')).toEqual([[{ competitionId: 'competition a', revision: 5 }]])
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(bodyAt(2)).toBeUndefined()
  expect(wrapper.text()).toContain('建立賽程已獲伺服器收據確認')
})

it('預填五分鐘主時間與三十秒讀秒；必填與四賽制標籤／勝局門檻為真 UI，不從名單推排名', async () => {
  fetchMock.mockRejectedValueOnce(notReady)
  const wrapper = await mountManagement()
  expect(wrapper.get('[data-testid="tournament-clock-main"]').element).toHaveProperty('value', '5')
  expect(wrapper.get('[data-testid="tournament-byoyomi"]').element).toHaveProperty('value', '30')
  expect(wrapper.get('[data-testid="tournament-random-seed"]').element).toHaveProperty('value', '')
  const formats: [string, string][] = [['singleElimination', '單敗淘汰'], ['roundRobin', '循環賽'], ['doubleElimination', '雙敗淘汰'], ['swiss', '瑞士制']]
  for (const [format, label] of formats) {
    await wrapper.get('[data-testid="tournament-format"]').setValue(format)
    expect(wrapper.get('[data-testid="tournament-format"]').text()).toContain(label)
    expect(wrapper.text()).toContain(format.includes('Elimination') ? '勝局門檻' : '每場局數')
  }
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  expect(wrapper.find('[role="alert"]').text()).toContain('整數設定')
  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(crypto.randomUUID).toHaveBeenCalledTimes(0)
  expect(wrapper.text()).toContain('不是既有排名')
})

it('明示指定 1..N 與一次公開 Uint32 種子，摘要顯示數字且未確認前不送出', async () => {
  fetchMock.mockRejectedValueOnce(notReady)
  const fill = vi.spyOn(crypto, 'getRandomValues').mockImplementation(target => {
    if (!(target instanceof Uint32Array)) throw new Error('需要 Uint32 排程種子')
    target[0] = 123456
    return target
  })
  const wrapper = await mountManagement()
  await draft(wrapper)
  await wrapper.get('[data-testid="tournament-assign-seeds"]').trigger('click')
  await wrapper.get('[data-testid="tournament-generate-seed"]').trigger('click')
  expect(fill).toHaveBeenCalledTimes(1)
  expect(wrapper.get('[data-testid="tournament-team-seed-0"]').element).toHaveProperty('value', '1')
  expect(wrapper.get('[data-testid="tournament-team-seed-1"]').element).toHaveProperty('value', '2')
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  expect(wrapper.get('[data-testid="tournament-summary"]').text()).toContain('123456')
  expect(fill).toHaveBeenCalledTimes(1)
  expect(crypto.randomUUID).toHaveBeenCalledTimes(0)
  expect(fetchMock).toHaveBeenCalledTimes(1)
})

it('摘要後修改草稿必須重新檢視及明示確認，不能送過期摘要', async () => {
  fetchMock.mockRejectedValueOnce(notReady)
  const wrapper = await mountManagement()
  await review(wrapper)
  await wrapper.get('[data-testid="tournament-random-seed"]').setValue('78')
  expect(wrapper.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  expectDisabled(wrapper, 'tournament-confirm-create')
  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(crypto.randomUUID).toHaveBeenCalledTimes(0)
})

it('瑞士制要求明示完整政策並原樣送出，和局折算不混和局積分', async () => {
  const config: StageConfig = { ...stage(), format: 'swiss', swissRounds: 3,
    swissPolicy: { matchPoints: { win: 3, draw: 1, loss: 0, bye: 3 }, drawCredit: { numerator: 1, denominator: 2 },
      opponentWinRateFloorPercent: 33, opponentWinRateCapPercent: null, excludeByesFromOpponentWinRate: true,
      tieBreakers: ['matchPoints', 'opponentWinRate'] } }
  fetchMock.mockRejectedValueOnce(notReady).mockResolvedValueOnce(ack(config)).mockResolvedValueOnce(read(5, config))
  const wrapper = await mountManagement()
  await draft(wrapper)
  await wrapper.get('[data-testid="tournament-format"]').setValue('swiss')
  const inputs: [string, string][] = [
    ['tournament-swiss-rounds', '3'], ['tournament-points-win', '3'], ['tournament-points-draw', '1'],
    ['tournament-points-loss', '0'], ['tournament-points-bye', '3'], ['tournament-credit-numerator', '1'],
    ['tournament-credit-denominator', '2'], ['tournament-floor-mode', 'value'], ['tournament-floor-value', '33'],
    ['tournament-cap-mode', 'none'], ['tournament-exclude-byes', 'yes'], ['tournament-tie-mode', 'configured'],
  ]
  for (const [testId, value] of inputs) await wrapper.get(`[data-testid="${testId}"]`).setValue(value)
  await wrapper.get('[data-testid="tournament-add-tie"]').trigger('click')
  await wrapper.get('[data-testid="tournament-tie-0"]').setValue('matchPoints')
  await wrapper.get('[data-testid="tournament-add-tie"]').trigger('click')
  await wrapper.get('[data-testid="tournament-tie-1"]').setValue('opponentWinRate')
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  const summary = wrapper.get('[data-testid="tournament-summary"]').text()
  expect(summary).toContain('對手勝率的和局折算：1／2')
  expect(summary).toContain('勝場／和局／敗場／輪空積分：3／1／0／3')
  expect(summary).toContain('賽事積分 → 對手勝率')
  await wrapper.get('[data-testid="tournament-acknowledge"]').setValue(true)
  await wrapper.get('[data-testid="tournament-confirm-create"]').trigger('click')
  await flushPromises()
  expect(bodyAt(1)).toEqual({ commandId, expectedRevision: 4, stage: config })
  expect(wrapper.emitted('created')).toHaveLength(1)
})

it('整數分鐘主時間與共用讀秒呈現並送出既有毫秒設定', async () => {
  const config: StageConfig = { ...stage(), clockConfig: { mainTimeMs: 60_000, actionTimeMs: 180_000, outOfTurnChoiceTimeMs: 180_000 } }
  fetchMock.mockRejectedValueOnce(notReady).mockResolvedValueOnce(ack(config)).mockResolvedValueOnce(read(5, config))
  const wrapper = await mountManagement()
  await draft(wrapper)
  await wrapper.get('[data-testid="tournament-clock-main"]').setValue('1')
  await wrapper.get('[data-testid="tournament-byoyomi"]').setValue('180')
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  expect(wrapper.get('[data-testid="tournament-summary"]').text()).toContain('1 分鐘')
  expect(wrapper.get('[data-testid="tournament-summary"]').text()).toContain('180 秒')
  expect(wrapper.get('[data-testid="tournament-summary"]').text()).toContain('讀秒（行動與回合外選擇）')
  await wrapper.get('[data-testid="tournament-acknowledge"]').setValue(true)
  await wrapper.get('[data-testid="tournament-confirm-create"]').trigger('click')
  await flushPromises()
  expect(bodyAt(1)).toEqual({ commandId, expectedRevision: 4, stage: config })
})

it('整數分鐘主時間零不限時／讀秒選項共用 60 秒送出既有毫秒設定', async () => {
  const config: StageConfig = { ...stage(), clockConfig: { mainTimeMs: 0, actionTimeMs: 60_000, outOfTurnChoiceTimeMs: 60_000 } }
  fetchMock.mockRejectedValueOnce(notReady).mockResolvedValueOnce(ack(config)).mockResolvedValueOnce(read(5, config))
  const wrapper = await mountManagement()
  const mainMinutes = wrapper.get('[data-testid="tournament-clock-main"]')
  expect(mainMinutes.element.closest('label')?.textContent).toContain('分鐘')
  const byoyomi = wrapper.get('[data-testid="tournament-byoyomi"]')
  expect(byoyomi.element.tagName).toBe('SELECT')
  expect(byoyomi.element.closest('label')?.textContent).toContain('讀秒')
  expect(byoyomi.findAll('option').map(option => option.attributes('value'))).toEqual(['30', '60', '180'])

  await wrapper.get('[data-testid="tournament-format"]').setValue('roundRobin')
  await wrapper.get('[data-testid="tournament-team-seed-0"]').setValue('2')
  await wrapper.get('[data-testid="tournament-team-seed-1"]').setValue('1')
  await wrapper.get('[data-testid="tournament-random-seed"]').setValue('77')
  await wrapper.get('[data-testid="tournament-games"]').setValue('3')
  await mainMinutes.setValue('0')
  await byoyomi.setValue('60')
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  expect(wrapper.find('[data-testid="tournament-summary"]').exists()).toBe(true)
  await wrapper.get('[data-testid="tournament-acknowledge"]').setValue(true)
  await wrapper.get('[data-testid="tournament-confirm-create"]').trigger('click')
  await flushPromises()

  expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  expect(bodyAt(1)).toEqual({ commandId, expectedRevision: 4, stage: config })
  expect(wrapper.emitted('created')).toEqual([[{ competitionId: 'competition a', revision: 5 }]])
})

it('POST pending 鎖全部語意欄位與重生 seed，double-submit 仍僅一 POST／兩 UUID', async () => {
  const post = deferred()
  fetchMock.mockRejectedValueOnce(notReady).mockReturnValueOnce(post.promise)
  const wrapper = await mountManagement()
  await submit(wrapper)
  expectDisabled(wrapper, 'tournament-format')
  expectDisabled(wrapper, 'tournament-random-seed')
  expectDisabled(wrapper, 'tournament-generate-seed')
  expectDisabled(wrapper, 'tournament-team-seed-0')
  expectDisabled(wrapper, 'tournament-same-command-retry')
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(crypto.randomUUID).toHaveBeenCalledTimes(2)
  post.reject({ statusCode: 503, message: 'private transport' })
  await flushPromises()
  expect(wrapper.get('[data-testid="tournament-same-command-retry"]').element.matches(':disabled')).toBe(false)
  expect(wrapper.text()).not.toContain('private transport')
})

it('503→GET 已存在新 revision→403／409／404 皆不能解 receipt；same body／UUID0 重試直到 exact ACK', async () => {
  fetchMock.mockRejectedValueOnce(notReady).mockRejectedValueOnce({ statusCode: 503 }).mockResolvedValueOnce(read(20))
    .mockRejectedValueOnce({ statusCode: 403, data: { code: 'forbidden' } })
    .mockRejectedValueOnce({ statusCode: 409, data: { code: 'revisionConflict' } })
    .mockRejectedValueOnce({ statusCode: 404, data: { code: 'notInitialized' } })
    .mockResolvedValueOnce({ ...ack(), idempotent: true }).mockResolvedValueOnce(read(20))
  const wrapper = await mountManagement()
  await submit(wrapper)
  const original = bodyAt(1)
  await wrapper.get('[data-testid="tournament-refresh"]').trigger('click')
  await flushPromises()
  expect(wrapper.text()).toContain('已存在 1 個階段')
  expect(wrapper.get('[data-testid="tournament-same-command-retry"]').element.matches(':disabled')).toBe(false)
  for (const index of [3, 4, 5]) {
    await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
    await flushPromises()
    expect(bodyAt(index)).toEqual(original)
    expect(wrapper.emitted('created')).toBeUndefined()
    expect(wrapper.get('[data-testid="tournament-same-command-retry"]').exists()).toBe(true)
  }
  await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  await flushPromises()
  expect(bodyAt(6)).toEqual(original)
  expect(crypto.randomUUID).toHaveBeenCalledTimes(2)
  expect(wrapper.emitted('created')).toEqual([[{ competitionId: 'competition a', revision: 5 }]])
})

it('malformed success／合法 GET operation 不是 ACK；unknown 原 snapshot 保留', async () => {
  fetchMock.mockRejectedValueOnce(notReady).mockResolvedValueOnce(read())
    .mockResolvedValueOnce({ ...ack(), projection: { ...ack().projection, public: { ...publicSnapshot(), cards: ['secret'] } } })
  const wrapper = await mountManagement()
  await submit(wrapper)
  expect(wrapper.text()).toContain('建立結果尚未確認')
  await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  await flushPromises()
  expect(wrapper.emitted('created')).toBeUndefined()
  expect(bodyAt(2)).toEqual(bodyAt(1))
  expect(wrapper.text()).not.toContain('secret')
  expect(crypto.randomUUID).toHaveBeenCalledTimes(2)
})

it('首次已辨識 revisionConflict 不重送／rebase，只提示重讀後由使用者重新設定', async () => {
  fetchMock.mockRejectedValueOnce(notReady).mockRejectedValueOnce({ statusCode: 409, data: { code: 'revisionConflict' } })
    .mockRejectedValueOnce(notReady)
  const wrapper = await mountManagement()
  await submit(wrapper)
  expect(wrapper.text()).toContain('伺服器未接受本次建立設定')
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(wrapper.emitted('created')).toBeUndefined()
  await wrapper.get('[data-testid="tournament-refresh"]').trigger('click')
  await flushPromises()
  expect(fetchMock.mock.calls[2]?.[1]).toMatchObject({ method: 'GET', retry: 0 })
  expect(bodyAt(2)).toBeUndefined()
})

it('matching ACK 後 readonly GET 失敗仍是已確認，不復活未知命令', async () => {
  fetchMock.mockRejectedValueOnce(notReady).mockResolvedValueOnce(ack()).mockRejectedValueOnce({ statusCode: 503, message: 'secret' })
  const wrapper = await mountManagement()
  await submit(wrapper)
  expect(wrapper.text()).toContain('建立賽程已獲伺服器收據確認')
  expect(wrapper.text()).toContain('賽事服務暫時無法使用')
  expect(wrapper.emitted('pending-change')).toEqual([[true], [false]])
  expect(wrapper.find('[data-testid="tournament-same-command-retry"]').exists()).toBe(false)
  expect(wrapper.text()).not.toContain('secret')
})

it('同 scope parent refresh 暫停 abort 原 POST，保留未知 intent；新 GET 不解收據且 old ACK 不生效', async () => {
  const post = deferred()
  const refresh = deferred()
  fetchMock.mockRejectedValueOnce(notReady).mockReturnValueOnce(post.promise).mockReturnValueOnce(refresh.promise)
    .mockResolvedValueOnce({ ...ack(), idempotent: true }).mockResolvedValueOnce(read(20))
  const wrapper = await mountManagement()
  await submit(wrapper)
  const section = wrapper.get('section').element
  await wrapper.setProps({ contextReady: false, busy: true })
  expect(signalAt(1).aborted).toBe(true)
  expectDisabled(wrapper, 'tournament-same-command-retry')
  expect(wrapper.get('[data-testid="tournament-summary"]').text()).toContain('原命令的固定設定摘要')
  await wrapper.setProps({ contextReady: true, busy: false, registration: registration(20) })
  expectDisabled(wrapper, 'tournament-refresh')
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(3)
  post.resolve(ack())
  await flushPromises()
  expectDisabled(wrapper, 'tournament-refresh')
  expect(wrapper.emitted('created')).toBeUndefined()
  expect(wrapper.get('section').element).toBe(section)
  refresh.resolve(read(20))
  await flushPromises()
  await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  await flushPromises()
  expect(bodyAt(3)).toEqual(bodyAt(1))
  expect(crypto.randomUUID).toHaveBeenCalledTimes(2)
  expect(wrapper.emitted('created')).toEqual([[{ competitionId: 'competition a', revision: 5 }]])
})

for (const change of ['competition', 'session', 'eligibility']) {
  it(`${change} 切換立即 abort／清投影，新 read pending 的 old error／finally 不解鎖；不跨身份重送`, async () => {
    const first = deferred()
    const next = deferred()
    fetchMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(next.promise)
    const wrapper = await mountManagement()
    if (change === 'competition') {
      await wrapper.setProps({ competitionId: 'competition b', registration: { ...registration(), competitionId: 'competition b' } })
    } else if (change === 'session') {
      await wrapper.setProps({ sessionUserId: 'organizer-b', registration: { ...registration(), organizer: 'organizer-b' } })
    } else {
      await wrapper.setProps({ contextReady: false })
      await wrapper.setProps({ contextReady: true })
    }
    expect(signalAt(0).aborted).toBe(true)
    expectDisabled(wrapper, 'tournament-refresh')
    await flushPromises()
    expect(fetchMock).toHaveBeenCalledTimes(2)
    first.reject({ statusCode: 503, message: 'late secret' })
    await flushPromises()
    expectDisabled(wrapper, 'tournament-refresh')
    expect(wrapper.text()).not.toContain('late secret')
    next.reject(notReady)
    await flushPromises()
    expect(wrapper.get('[data-testid="tournament-refresh"]').element.matches(':disabled')).toBe(false)
    expect(crypto.randomUUID).toHaveBeenCalledTimes(0)
  })
}

it('已顯示投影在切換身份時即清；late read 不復活舊賽事', async () => {
  const next = deferred()
  fetchMock.mockResolvedValueOnce(read()).mockReturnValueOnce(next.promise)
  const wrapper = await mountManagement()
  expect(wrapper.text()).toContain('已存在 1 個階段')
  await wrapper.setProps({ competitionId: 'competition b', registration: { ...registration(), competitionId: 'competition b' } })
  expect(wrapper.text()).not.toContain('已存在 1 個階段')
  expectDisabled(wrapper, 'tournament-refresh')
  await flushPromises()
  next.resolve(read())
  await flushPromises()
  expect(wrapper.text()).toContain('賽程資料格式不正確')
  expect(wrapper.text()).not.toContain('已存在 1 個階段')
})

it('未知命令跨帳號保留警告且不可重送，回原身份才能以同 body／expected retry', async () => {
  fetchMock.mockRejectedValueOnce(notReady).mockRejectedValueOnce({ statusCode: 503 }).mockResolvedValueOnce(read(20))
    .mockResolvedValueOnce(read(20)).mockResolvedValueOnce({ ...ack(), idempotent: true }).mockResolvedValueOnce(read(20))
  const wrapper = await mountManagement()
  await submit(wrapper)
  const originalSummary = wrapper.get('[data-testid="tournament-summary"]').text()
  expect(originalSummary).toContain('原命令的固定設定摘要')
  await wrapper.setProps({ sessionUserId: 'organizer-b', registration: { ...registration(20), organizer: 'organizer-b' } })
  expectDisabled(wrapper, 'tournament-same-command-retry')
  expect(wrapper.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  await flushPromises()
  expect(wrapper.text()).toContain('不可換身份重送')
  await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(wrapper.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  await wrapper.setProps({ sessionUserId: 'organizer-a', registration: registration(20) })
  await flushPromises()
  expect(wrapper.get('[data-testid="tournament-summary"]').text()).toBe(originalSummary)
  await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  await flushPromises()
  expect(bodyAt(4)).toEqual(bodyAt(1))
  expect(crypto.randomUUID).toHaveBeenCalledTimes(2)
})

it('未知命令跨賽事不回退舊 reviewed 或新摘要；回原賽事只呈現並重試原固定 snapshot', async () => {
  fetchMock.mockRejectedValueOnce(notReady).mockRejectedValueOnce({ statusCode: 503 })
    .mockRejectedValueOnce(notReady).mockResolvedValueOnce(read(20))
    .mockResolvedValueOnce({ ...ack(), idempotent: true }).mockResolvedValueOnce(read(20))
  const wrapper = await mountManagement()
  await submit(wrapper)
  const originalSummary = wrapper.get('[data-testid="tournament-summary"]').text()
  expect(bodyAt(1)).toEqual({ commandId, expectedRevision: 4, stage: stage() })
  await wrapper.setProps({ competitionId: 'competition b', registration: { ...registration(20), competitionId: 'competition b' } })
  expect(wrapper.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  expectDisabled(wrapper, 'tournament-same-command-retry')
  await flushPromises()
  expect(fetchMock.mock.calls[2]?.[0]).toBe('/api/competitions/competition%20b/tournament')
  expect(wrapper.text()).toContain('目前尚未有可顯示賽程')
  expect(wrapper.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  expect(wrapper.find('[data-testid="tournament-form"]').exists()).toBe(false)
  expect(wrapper.text()).toContain('不可換身份重送')
  await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(wrapper.emitted('created')).toBeUndefined()
  expect(wrapper.emitted('pending-change')).toEqual([[true]])
  await wrapper.setProps({ competitionId: 'competition a', registration: registration(20) })
  await flushPromises()
  expect(wrapper.get('[data-testid="tournament-summary"]').text()).toBe(originalSummary)
  await wrapper.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  await flushPromises()
  expect(bodyAt(4)).toEqual({ commandId, expectedRevision: 4, stage: stage() })
  expect(bodyAt(4)).toEqual(bodyAt(1))
  expect(crypto.randomUUID).toHaveBeenCalledTimes(2)
  expect(wrapper.emitted('created')).toEqual([[{ competitionId: 'competition a', revision: 5 }]])
})

it('未登入／非公告主辦／未 lock／checkedIn 不足不查詢；機器 ID 不作玩家文案', async () => {
  const overrides = [
    { sessionUserId: null }, { sessionUserId: 'outsider' },
    { registration: { ...registration(), lifecycle: 'open' } satisfies CompetitionRegistrationState },
    { registration: { ...registration(), teams: registration().teams.slice(0, 1) } },
  ]
  for (const value of overrides) {
    const wrapper = await mountManagement(value)
    expectDisabled(wrapper, 'tournament-refresh')
  }
  expect(fetchMock).toHaveBeenCalledTimes(0)
  fetchMock.mockRejectedValueOnce(notReady)
  const wrapper = await mountManagement()
  await review(wrapper)
  expect(wrapper.text()).toContain('第 1 隊：種子 2')
  expect(wrapper.text()).toContain('第 2 隊：種子 1')
  for (const id of [...teamIds, stageId, commandId]) expect(wrapper.text()).not.toContain(id)
})

it('pending 卸載／離頁警告不宣稱取消 server，晚 ACK 不 emit 完成', async () => {
  const post = deferred()
  fetchMock.mockRejectedValueOnce(notReady).mockReturnValueOnce(post.promise)
  const wrapper = await mountManagement()
  await submit(wrapper)
  const unload = new Event('beforeunload', { cancelable: true })
  window.dispatchEvent(unload)
  expect(unload.defaultPrevented).toBe(true)
  wrapper.unmount()
  expect(signalAt(1).aborted).toBe(true)
  expect(wrapper.emitted('unresolved')).toEqual([[]])
  post.resolve(ack())
  await flushPromises()
  expect(wrapper.emitted('created')).toBeUndefined()
})

it('表單 label／可鍵盤操作的確認與窄 viewport 狀態保留；幾何交 main 真 browser', async () => {
  const originalWidth = window.innerWidth
  Object.defineProperty(window, 'innerWidth', { configurable: true, value: 390 })
  try {
    fetchMock.mockRejectedValueOnce(notReady)
    const wrapper = await mountManagement()
    const inputs = wrapper.findAll('input:not([type="checkbox"]), select')
    for (const input of inputs) {
      const id = input.attributes('id')
      expect(id).toBeTruthy()
      expect(wrapper.find(`label[for="${id}"]`).exists()).toBe(true)
    }
    await review(wrapper)
    const confirm = wrapper.get('[data-testid="tournament-confirm-create"]').element
    if (!(confirm instanceof HTMLButtonElement)) throw new Error('明示確認必須為按鈕')
    confirm.focus()
    expect(document.activeElement).toBe(confirm)
    expect(confirm.disabled).toBe(false)
    expect(wrapper.get('[data-testid="tournament-summary"]').text()).toContain('循環賽')
  } finally { Object.defineProperty(window, 'innerWidth', { configurable: true, value: originalWidth }) }
})
