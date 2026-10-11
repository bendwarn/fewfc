import { mountSuspended, mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { defineComponent, h, nextTick } from 'vue'
import { NuxtPage } from '#components'
import { useNuxtApp, useRouter } from '#app'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { StageConfig } from '../../shared/competition'
import { usePlayerSession } from '~/composables/usePlayerSession'
import CompetitionTournamentManagement from '~/components/CompetitionTournamentManagement.vue'
import CompetitionRoundPublication from '~/components/CompetitionRoundPublication.vue'
import { parseCompetitionTournamentViewResponse } from '~/lib/competition-tournament-view'

const { fetchMock, getSessionMock } = vi.hoisted(() => ({ fetchMock: vi.fn(), getSessionMock: vi.fn() }))
mockNuxtImport('$fetch', () => fetchMock)
vi.mock('~/lib/auth-client', () => ({ authClient: { getSession: getSessionMock } }))

const teamIds: [string, string] = ['b45abe94-a252-4e3d-bff2-7d26c6e2fabc', '535ec115-7bf4-4bbf-b0d3-c6e0ee76c8b7']
const stageId = 'cd0d3e8b-7c87-46c1-b50d-deb6fb2ad5fa'
const commandId = 'b9ae10af-54da-429e-a10c-5e8de84021a2'
const queryId = '8aa75538-3917-4f13-a676-c0ad58379636'
const notReady = { statusCode: 409, data: { code: 'notReady' } }
function stage(): StageConfig {
  return { stageId, format: 'roundRobin', teams: [{ teamId: teamIds[0], seed: 2 }, { teamId: teamIds[1], seed: 1 }], randomSeed: 77,
    matchFormat: { type: 'fixedGames', value: 3 }, clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    swissRounds: null, swissPolicy: null }
}
function publicSnapshot(competitionId = 'competition-a') {
  const config = stage()
  return { tournamentId: competitionId, stages: [{ config, status: 'active', rounds: [], standings: [], corrections: [],
    lastScheduleError: null, scheduleGeneration: 0, supersededRounds: [],
    teams: Object.fromEntries(config.teams.map(team => [team.teamId, { seed: team.seed, dropped: false, hadBye: false }])), doubleState: null }] }
}
function readTournament(competitionId = 'competition-a', revision = 20) {
  return { competitionId, revision, projection: { ok: true, operation: 'tournamentView', public: publicSnapshot(competitionId) } }
}
function ack() {
  return { competitionId: 'competition-a', revision: 5, commandId, idempotent: true,
    projection: { ok: true, operation: 'tournamentCreate', public: publicSnapshot() } }
}
function registrationResponse(competitionId = 'competition-a', revision = 4, lifecycle: 'locked' | 'open' = 'locked') {
  // 公告沿既有 public DTO；不測生命值、不假造新增建立 payload。
  return { competitionId, revision, projection: { ok: true, operation: 'registrationView', public: {
    announcement: { organizer: 'organizer-a', ruleVersion: '5.17', ruleset: 'base', enabledRuleModules: [], teamSize: 1, initialHp: 1,
      registrationMode: 'public', personalDecks: 'disabled', registrationDeadline: 60_000, checkInDeadline: 120_000,
      plan: {
        initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
        swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
        rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
        exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
        stages: [{ stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
          clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
          swissRounds: null, swissMatchPoints: null, qualification: { type: 'allEligibleTeams' } }],
      } },
    lifecycle, revision: 2, teams: teamIds.map((teamId, index) => ({ teamId, captain: `captain-${index}`, members: [`captain-${index}`],
      rosterVersion: 1, confirmedMembers: [`captain-${index}`], status: 'checkedIn', exclusionReasons: [] })),
  } } }
}
function deferred() {
  let resolve!: (value: unknown) => void
  let reject!: (reason: unknown) => void
  const promise = new Promise<unknown>((accept, refuse) => { resolve = accept; reject = refuse })
  return { promise, resolve, reject }
}
function method(options: unknown): unknown {
  return options && typeof options === 'object' && 'method' in options ? options.method : undefined
}
function operation(options: unknown): unknown {
  if (!options || typeof options !== 'object' || !('body' in options)) return undefined
  const body = options.body
  if (!body || typeof body !== 'object' || !('operation' in body)) return undefined
  const action = body.operation
  return action && typeof action === 'object' && 'type' in action ? action.type : undefined
}
function registrationAction(options: unknown): unknown {
  if (!options || typeof options !== 'object' || !('body' in options)) return undefined
  const body = options.body
  if (!body || typeof body !== 'object' || !('operation' in body)) return undefined
  const value = body.operation
  if (!value || typeof value !== 'object' || !('type' in value) || value.type !== 'registrationExecute' || !('action' in value)) return undefined
  const action = value.action
  return action && typeof action === 'object' && 'type' in action ? action.type : undefined
}
function registrationReads() {
  return fetchMock.mock.calls.filter(call => operation(call[1]) === 'registrationView')
}
function tournamentPosts() {
  return fetchMock.mock.calls.filter(call => typeof call[0] === 'string' && call[0].endsWith('/tournament') && method(call[1]) === 'POST')
}
function requestSignal(call: unknown[]): AbortSignal {
  const options: unknown = call[1]
  if (!options || typeof options !== 'object' || !('signal' in options) || !(options.signal instanceof AbortSignal)) {
    throw new Error('正式唯讀 transport 必須有 signal')
  }
  return options.signal
}

let post = deferred()
let registrationRevision = 4
let lifecycle: 'locked' | 'open' = 'locked'
let tournamentAvailable = false
const queuedRegistrationReads: ReturnType<typeof deferred>[] = []
const cleanups: (() => void)[] = []
const RouteHost = defineComponent({ setup: () => () => h(NuxtPage) })
function currentSession() { return useNuxtApp().runWithContext(() => usePlayerSession()) }
function currentRouter() { return useNuxtApp().runWithContext(() => useRouter()) }
async function mountPage() {
  const host = document.createElement('div')
  document.body.append(host)
  // 真 NuxtPage／RouterView；不以 standalone wrapper 或 mocked route 證明頁面 reuse。
  const wrapper = await mountSuspended(RouteHost, { route: '/competitions/competition-a', attachTo: host, global: { stubs: {
    CompetitionPersonalDeckSubmission: true, CompetitionParticipantGameEntry: true, CompetitionRulingCorrection: true,
  } } })
  cleanups.push(() => { if (wrapper.exists()) wrapper.unmount(); host.remove() })
  await flushPromises()
  await nextTick()
  await flushPromises()
  return wrapper
}
type PageWrapper = Awaited<ReturnType<typeof mountPage>>
async function prepareCreate(wrapper: PageWrapper) {
  const inputs: [string, string][] = [['tournament-format', 'roundRobin'], ['tournament-team-seed-0', '2'],
    ['tournament-team-seed-1', '1'], ['tournament-random-seed', '77'], ['tournament-games', '3'],
    ['tournament-clock-main', '5'], ['tournament-byoyomi', '30']]
  for (const [testId, value] of inputs) await wrapper.get(`[data-testid="${testId}"]`).setValue(value)
  await wrapper.get('[data-testid="tournament-form"]').trigger('submit')
  await wrapper.get('[data-testid="tournament-acknowledge"]').setValue(true)
}
async function sendCreate(wrapper: PageWrapper) {
  await prepareCreate(wrapper)
  const uuid = vi.mocked(crypto.randomUUID)
  uuid.mockReturnValueOnce(stageId).mockReturnValueOnce(commandId)
  const baseline = uuid.mock.calls.length
  await wrapper.get('[data-testid="tournament-confirm-create"]').trigger('click')
  expect(uuid.mock.calls.length - baseline).toBe(2)
  expect(uuid.mock.results.slice(baseline).map(result => result.type === 'return' ? result.value : undefined)).toEqual([stageId, commandId])
  expect(tournamentPosts()).toHaveLength(1)
  const body: unknown = tournamentPosts()[0]?.[1]?.body
  expect(body).toEqual({ commandId, expectedRevision: 4, stage: stage() })
  expect(tournamentPosts()[0]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
}

beforeEach(() => {
  queuedRegistrationReads.splice(0)
  registrationRevision = 4
  lifecycle = 'locked'
  tournamentAvailable = false
  post = deferred()
  currentSession().clear()
  getSessionMock.mockReset().mockResolvedValue({ data: { user: { id: 'organizer-a', name: '測試主辦' } }, error: null })
  vi.spyOn(crypto, 'randomUUID').mockReturnValue(queryId)
  fetchMock.mockReset().mockImplementation((path: unknown, options: unknown) => {
    if (typeof path !== 'string') throw new Error('fixture 只接受明確路徑')
    if (path.endsWith('/commands') && operation(options) === 'registrationView') {
      const competitionId = decodeURIComponent(path.split('/')[3] ?? '')
      return queuedRegistrationReads.shift()?.promise ?? Promise.resolve(registrationResponse(competitionId, registrationRevision, lifecycle))
    }
    if (path.endsWith('/tournament') && method(options) === 'GET') {
      return tournamentAvailable ? Promise.resolve(readTournament(decodeURIComponent(path.split('/')[3] ?? ''))) : Promise.reject(notReady)
    }
    if (path.endsWith('/tournament') && method(options) === 'POST') return post.promise
    throw new Error('fixture 非預期的 transport')
  })
})
afterEach(() => {
  for (const cleanup of cleanups.splice(0)) cleanup()
  currentSession().clear()
  vi.restoreAllMocks()
})

it('unknown 建立跨父刷新保留同 DOM／原 expected／兩 UUID，GET20 不解收據且 retry0 UUID', async () => {
  const page = await mountPage()
  const child = page.findComponent(CompetitionTournamentManagement)
  const panel = page.get('.tournament-management').element
  await sendCreate(page)
  post.reject({ statusCode: 503 })
  await flushPromises()
  const retry = page.get('[data-testid="tournament-same-command-retry"]').element
  const originalSummary = page.get('[data-testid="tournament-summary"]').text()
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeUndefined()
  const refresh = deferred()
  queuedRegistrationReads.push(refresh)
  await page.get('.detail-heading .refresh-button').trigger('click')
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.get('[data-testid="tournament-same-command-retry"]').element).toBe(retry)
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeDefined()
  expect(child.emitted('unresolved')).toBeUndefined()
  tournamentAvailable = true
  registrationRevision = 20
  refresh.resolve(registrationResponse('competition-a', 20))
  await flushPromises()
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.get('[data-testid="tournament-summary"]').text()).toBe(originalSummary)
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeUndefined()
  expect(child.emitted('created')).toBeUndefined()
  expect(child.emitted('unresolved')).toBeUndefined()
  expect(page.get('.tournament-view .revision-note').text()).toContain('20')
  post = deferred()
  const uuidBaseline = vi.mocked(crypto.randomUUID).mock.calls.length
  await page.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  expect(vi.mocked(crypto.randomUUID).mock.calls.length - uuidBaseline).toBe(0)
  expect(tournamentPosts()).toHaveLength(2)
  const firstBody: unknown = tournamentPosts()[0]?.[1]?.body
  const retryBody: unknown = tournamentPosts()[1]?.[1]?.body
  expect(retryBody).toEqual(firstBody)
  expect(retryBody).toEqual({ commandId, expectedRevision: 4, stage: stage() })
  post.resolve(ack())
  await flushPromises()
  expect(child.emitted('created')).toEqual([[{ competitionId: 'competition-a', revision: 5 }]])
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.find('[data-testid="tournament-same-command-retry"]').exists()).toBe(false)
  expect(page.get('.tournament-view .revision-note').text()).toContain('20')
  expect(registrationReads()).toHaveLength(3)
})

it('matching ACK 只啟動 registrationView，等待權威刷新才呈現新版本與賽程', async () => {
  const page = await mountPage()
  const panel = page.get('.tournament-management').element
  await sendCreate(page)
  const refresh = deferred()
  queuedRegistrationReads.push(refresh)
  const readBaseline = registrationReads().length
  tournamentAvailable = true
  post.resolve(ack())
  await flushPromises()
  expect(registrationReads().length - readBaseline).toBe(1)
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.text()).toContain('正在載入賽事報名狀態')
  expect(page.find('.tournament-view .revision-note').exists()).toBe(false)
  expect(page.find('[data-testid="tournament-same-command-retry"]').exists()).toBe(false)
  const queryOptions: unknown = registrationReads().at(-1)?.[1]
  expect(queryOptions).toMatchObject({ method: 'POST', retry: 0, body: {
    commandId: queryId, expectedRevision: 4, operation: { type: 'registrationView' },
  } })
  refresh.resolve(registrationResponse('competition-a', 20))
  await flushPromises()
  expect(page.get('.revision-label').text()).toBe('狀態版本 20')
  expect(page.get('.tournament-view .revision-note').text()).toContain('20')
  expect(tournamentPosts()).toHaveLength(1)
  const reads = fetchMock.mock.calls.filter(call => typeof call[0] === 'string' && call[0].endsWith('/tournament') && method(call[1]) === 'GET')
  for (const call of reads) {
    expect(call[0]).toBe('/api/competitions/competition-a/tournament')
    expect(call[1]).toMatchObject({ method: 'GET', retry: 0 })
    expect(call[1]?.body).toBeUndefined()
    expect(call[1]?.query).toBeUndefined()
  }
})

it.each(['非主辦', '未登入', '未鎖定'])('%s 沒有建立能力，不從公告角色提示產生 mutation', async kind => {
  if (kind === '非主辦') getSessionMock.mockResolvedValue({ data: { user: { id: 'viewer', name: '觀看者' } }, error: null })
  if (kind === '未登入') getSessionMock.mockResolvedValue({ data: null, error: null })
  if (kind === '未鎖定') lifecycle = 'open'
  const page = await mountPage()
  expect(page.get('.tournament-management').text()).toContain(kind === '非主辦'
    ? '目前帳號不是公告所列主辦方' : kind === '未登入' ? '請先登入主辦帳號' : '正在確認賽事狀態')
  expect(page.find('[data-testid="tournament-form"]').exists()).toBe(false)
  expect(page.get('[data-testid="tournament-refresh"]').attributes('disabled')).toBeDefined()
  expect(tournamentPosts()).toHaveLength(0)
  // 事件是公開 seam；foreign／extra／unsafe event 不得觸發 parent query。
  const child = page.findComponent(CompetitionTournamentManagement)
  const baseline = registrationReads().length
  child.vm.$emit('created', { competitionId: 'competition-a', revision: 5 })
  await flushPromises()
  expect(registrationReads()).toHaveLength(baseline)
})

it('created adapter 拒絕 foreign／extra／不安全 event，當前有效 receipt 才 readonly refresh', async () => {
  const page = await mountPage()
  const child = page.findComponent(CompetitionTournamentManagement)
  const baseline = registrationReads().length
  const invalidEvents: unknown[] = [{ competitionId: 'competition-b', revision: 5 }, { competitionId: 'competition-a', revision: 5, callerUserId: 'organizer-a' },
    { competitionId: 'competition-a', revision: 0 }, { competitionId: 'competition-a', revision: Number.MAX_SAFE_INTEGER + 1 }]
  for (const value of invalidEvents) child.vm.$emit('created', value)
  await flushPromises()
  expect(registrationReads()).toHaveLength(baseline)
  expect(tournamentPosts()).toHaveLength(0)
  child.vm.$emit('created', { competitionId: 'competition-a', revision: 5 })
  await flushPromises()
  expect(registrationReads()).toHaveLength(baseline + 1)
  expect(tournamentPosts()).toHaveLength(0)
})

it('A 真 unknown intent 在合法 open B 停用父 mutation 且零 dispatch，readonly 可刷新，回 A 原命令 retry', async () => {
  let aRevision = 4
  let aTournamentExists = false
  // 兩個不同賽事各自合法：A 始終 locked，B 始終 open；不將同一名單解鎖來製造操作。
  fetchMock.mockImplementation((path: unknown, options: unknown) => {
    if (path === '/api/competitions/competition-a/commands' && operation(options) === 'registrationView') {
      return Promise.resolve(registrationResponse('competition-a', aRevision, 'locked'))
    }
    if (path === '/api/competitions/competition-b/commands' && operation(options) === 'registrationView') {
      return Promise.resolve(registrationResponse('competition-b', 7, 'open'))
    }
    if (operation(options) === 'registrationExecute') return Promise.reject({ statusCode: 503 })
    if (path === '/api/competitions/competition-a/tournament' && method(options) === 'POST') return post.promise
    if (path === '/api/competitions/competition-b/tournament' && method(options) === 'POST') return Promise.reject({ statusCode: 503 })
    if (typeof path === 'string' && path.endsWith('/tournament') && method(options) === 'GET') {
      return path === '/api/competitions/competition-a/tournament' && aTournamentExists
        ? Promise.resolve(readTournament('competition-a', aRevision)) : Promise.reject(notReady)
    }
    throw new Error('fixture 非預期的 transport')
  })
  const page = await mountPage()
  const router = currentRouter()
  const child = page.findComponent(CompetitionTournamentManagement)
  const panel = page.get('.tournament-management').element
  await sendCreate(page)
  post.reject({ statusCode: 503 })
  await flushPromises()
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeUndefined()
  await router.push('/competitions/competition-b')
  await nextTick()
  await flushPromises()
  expect(router.currentRoute.value.path).toBe('/competitions/competition-b')
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeDefined()
  const register = page.get('.register-form button[type="submit"]')
  expect(register.text()).toBe('登記隊伍')
  expect(register.attributes('disabled')).toBeDefined()
  await register.trigger('click')
  await flushPromises()
  expect(fetchMock.mock.calls.filter(call => operation(call[1]) === 'registrationExecute')).toHaveLength(0)
  expect(tournamentPosts()).toHaveLength(1)
  expect(tournamentPosts().filter(call => call[0] === '/api/competitions/competition-b/tournament')).toHaveLength(0)
  const bReadBaseline = registrationReads().filter(call => call[0] === '/api/competitions/competition-b/commands').length
  expect(page.get('.detail-heading .refresh-button').attributes('disabled')).toBeUndefined()
  await page.get('.detail-heading .refresh-button').trigger('click')
  await flushPromises()
  const bReads = registrationReads().filter(call => call[0] === '/api/competitions/competition-b/commands')
  expect(bReads).toHaveLength(bReadBaseline + 1)
  const queryBody: unknown = bReads.at(-1)?.[1]?.body
  expect(queryBody).toEqual({ commandId: queryId, expectedRevision: 7, operation: { type: 'registrationView' } })
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.get('.register-form button[type="submit"]').attributes('disabled')).toBeDefined()
  expect(fetchMock.mock.calls.filter(call => operation(call[1]) === 'registrationExecute')).toHaveLength(0)
  expect(tournamentPosts()).toHaveLength(1)
  expect(child.emitted('created')).toBeUndefined()
  expect(child.emitted('unresolved')).toBeUndefined()
  await router.push('/competitions/competition-a')
  await nextTick()
  await flushPromises()
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.get('[data-testid="tournament-summary"]').text()).toContain('77')
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeUndefined()
  post = deferred()
  const retryUuidBaseline = vi.mocked(crypto.randomUUID).mock.calls.length
  await page.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  expect(vi.mocked(crypto.randomUUID).mock.calls.length - retryUuidBaseline).toBe(0)
  expect(tournamentPosts()).toHaveLength(2)
  expect(tournamentPosts().map(call => call[0])).toEqual(['/api/competitions/competition-a/tournament', '/api/competitions/competition-a/tournament'])
  const firstBody: unknown = tournamentPosts()[0]?.[1]?.body
  const retryBody: unknown = tournamentPosts()[1]?.[1]?.body
  expect(firstBody).toEqual({ commandId, expectedRevision: 4, stage: stage() })
  expect(retryBody).toEqual(firstBody)
  aRevision = 5
  aTournamentExists = true
  post.resolve(ack())
  await flushPromises()
  expect(child.emitted('created')).toEqual([[{ competitionId: 'competition-a', revision: 5 }]])
  expect(child.emitted('unresolved')).toBeUndefined()
  // matching ACK 解除建立賽程互斥後，B 的普通報名操作恢復可用。
  await router.push('/competitions/competition-b')
  await nextTick()
  await flushPromises()
  const unlockedControl = page.get('.register-form button[type="submit"]')
  expect(unlockedControl.text()).toBe('登記隊伍')
  expect(unlockedControl.attributes('disabled')).toBeUndefined()
  await unlockedControl.trigger('click')
  await flushPromises()
  const commands = fetchMock.mock.calls.filter(call => method(call[1]) === 'POST' && registrationAction(call[1]) === 'registerTeam')
  expect(commands).toHaveLength(1)
  expect(commands[0]?.[0]).toBe('/api/competitions/competition-b/commands')
  const controlBody: unknown = commands[0]?.[1]?.body
  expect(controlBody).toMatchObject({ commandId: queryId, expectedRevision: 7,
    operation: { type: 'registrationExecute', action: { type: 'registerTeam', members: ['organizer-a'] } } })
  expect(tournamentPosts()).toHaveLength(2)
})

it.each(['success', 'error'])('session 切換時 readonly 舊 %s／finally 不能清新 loading 或回灌舊 context', async result => {
  const page = await mountPage()
  const panel = page.get('.tournament-management').element
  const oldRead = deferred()
  queuedRegistrationReads.push(oldRead)
  await page.get('.detail-heading .refresh-button').trigger('click')
  const oldSignal = requestSignal(registrationReads().at(-1) ?? [])
  const newRead = deferred()
  queuedRegistrationReads.push(newRead)
  currentSession().setUser({ id: 'viewer', name: '觀看者' })
  await nextTick()
  expect(oldSignal.aborted).toBe(true)
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.get('[data-testid="tournament-refresh"]').attributes('disabled')).toBeDefined()
  if (result === 'success') oldRead.resolve(registrationResponse('competition-a', 999))
  else oldRead.reject({ statusCode: 503, message: '不可反射的内部訊息' })
  await flushPromises()
  expect(page.text()).toContain('正在載入賽事報名狀態')
  expect(page.find('.revision-label').exists()).toBe(false)
  expect(page.get('[data-testid="tournament-refresh"]').attributes('disabled')).toBeDefined()
  expect(page.text()).not.toContain('999')
  expect(page.text()).not.toContain('不可反射的内部訊息')
  newRead.resolve(registrationResponse('competition-a', 20))
  await flushPromises()
  expect(page.get('.revision-label').text()).toBe('狀態版本 20')
  expect(page.get('.tournament-management').text()).toContain('目前帳號不是公告所列主辦方')
  expect(tournamentPosts()).toHaveLength(0)
})

it('真 NuxtPage 同 record A→B→A 保留 DOM／未知 snapshot，舊 success/error 不回灌新 scope', async () => {
  const page = await mountPage()
  const router = currentRouter()
  const child = page.findComponent(CompetitionTournamentManagement)
  const panel = page.get('.tournament-management').element
  await sendCreate(page)
  const originalPost = post
  const originalSignal = requestSignal(tournamentPosts()[0] ?? [])
  const readB = deferred()
  queuedRegistrationReads.push(readB)
  await router.push('/competitions/competition-b')
  await nextTick()
  await flushPromises()
  expect(router.currentRoute.value.path).toBe('/competitions/competition-b')
  expect(originalSignal.aborted).toBe(true)
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeDefined()
  const bSignal = requestSignal(registrationReads().at(-1) ?? [])
  const readA = deferred()
  queuedRegistrationReads.push(readA)
  await router.push('/competitions/competition-a')
  await nextTick()
  await flushPromises()
  expect(bSignal.aborted).toBe(true)
  originalPost.resolve(ack())
  readB.reject({ statusCode: 503 })
  await flushPromises()
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.text()).toContain('正在載入賽事報名狀態')
  expect(child.emitted('created')).toBeUndefined()
  expect(child.emitted('unresolved')).toBeUndefined()
  readA.resolve(registrationResponse('competition-a', 20))
  await flushPromises()
  expect(page.get('[data-testid="tournament-summary"]').text()).toContain('77')
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeUndefined()
  post = deferred()
  const baseline = vi.mocked(crypto.randomUUID).mock.calls.length
  await page.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  expect(vi.mocked(crypto.randomUUID).mock.calls.length - baseline).toBe(0)
  const body: unknown = tournamentPosts()[1]?.[1]?.body
  expect(body).toEqual({ commandId, expectedRevision: 4, stage: stage() })
  expect(tournamentPosts()).toHaveLength(2)
})

it('未知 intent 跨 session／登出遮蔽摘要，回原帳號同 body retry 不自我 busy 鎖死', async () => {
  const page = await mountPage()
  const panel = page.get('.tournament-management').element
  await sendCreate(page)
  post.reject({ statusCode: 503 })
  await flushPromises()
  currentSession().setUser({ id: 'viewer' })
  await nextTick()
  await flushPromises()
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeDefined()
  currentSession().clear()
  await nextTick()
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(page.get('.tournament-management').text()).toContain('請先登入主辦帳號')
  expect(page.find('[data-testid="tournament-summary"]').exists()).toBe(false)
  currentSession().setUser({ id: 'organizer-a' })
  await nextTick()
  await flushPromises()
  expect(page.get('[data-testid="tournament-summary"]').text()).toContain('77')
  expect(page.get('[data-testid="tournament-same-command-retry"]').attributes('disabled')).toBeUndefined()
  post = deferred()
  const baseline = vi.mocked(crypto.randomUUID).mock.calls.length
  await page.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  expect(vi.mocked(crypto.randomUUID).mock.calls.length - baseline).toBe(0)
  const body: unknown = tournamentPosts()[1]?.[1]?.body
  expect(body).toEqual({ commandId, expectedRevision: 4, stage: stage() })
})

it('不同 record 離頁需明示確認，拒絕時保留 DOM／intent；確認後不宣稱取消或跨卸載保存', async () => {
  const page = await mountPage()
  const router = currentRouter()
  const removeRoute = router.addRoute({ name: 'management-test-exit', path: '/management-test-exit', component: defineComponent({
    setup: () => () => h('p', '正式 fixture 離頁目的地'),
  }) })
  cleanups.push(removeRoute)
  const child = page.findComponent(CompetitionTournamentManagement)
  const panel = page.get('.tournament-management').element
  await sendCreate(page)
  post.reject({ statusCode: 503 })
  await flushPromises()
  // Nuxt DOM 未必提供 confirm；明示補外部瀏覽器 seam，並恢復原 descriptor／缺省狀態。
  const confirm = vi.fn<(message?: string) => boolean>().mockReturnValue(false).mockReturnValueOnce(false).mockReturnValueOnce(true)
  const originalConfirm = Object.getOwnPropertyDescriptor(window, 'confirm')
  Object.defineProperty(window, 'confirm', { configurable: true, writable: true, value: confirm })
  cleanups.push(() => {
    if (originalConfirm) Object.defineProperty(window, 'confirm', originalConfirm)
    else Reflect.deleteProperty(window, 'confirm')
  })
  await router.push('/management-test-exit')
  await nextTick()
  expect(router.currentRoute.value.path).toBe('/competitions/competition-a')
  expect(page.get('.tournament-management').element).toBe(panel)
  expect(child.emitted('unresolved')).toBeUndefined()
  expect(page.text()).toContain('離開不會取消建立')
  expect(confirm).toHaveBeenCalledWith('建立賽程的結果仍未確認，可能已建立。離開不會取消伺服器操作，尚未確認的命令也不會跨頁面保存。確定要離開嗎？')
  const unload = new Event('beforeunload', { cancelable: true })
  window.dispatchEvent(unload)
  expect(unload.defaultPrevented).toBe(true)
  post = deferred()
  const retryBaseline = vi.mocked(crypto.randomUUID).mock.calls.length
  await page.get('[data-testid="tournament-same-command-retry"]').trigger('click')
  expect(vi.mocked(crypto.randomUUID).mock.calls.length - retryBaseline).toBe(0)
  const pendingSignal = requestSignal(tournamentPosts()[1] ?? [])
  const readsBeforeExit = registrationReads().length
  await router.push('/management-test-exit')
  await flushPromises()
  expect(confirm).toHaveBeenCalledTimes(2)
  expect(confirm).toHaveBeenNthCalledWith(2, '建立賽程的結果仍未確認，可能已建立。離開不會取消伺服器操作，尚未確認的命令也不會跨頁面保存。確定要離開嗎？')
  expect(router.currentRoute.value.path).toBe('/management-test-exit')
  expect(child.emitted('unresolved')).toEqual([[]])
  expect(pendingSignal.aborted).toBe(true)
  expect(page.find('.tournament-management').exists()).toBe(false)
  post.resolve(ack())
  await flushPromises()
  expect(child.emitted('created')).toBeUndefined()
  expect(registrationReads()).toHaveLength(readsBeforeExit)
  expect(tournamentPosts()).toHaveLength(2)
})

it('registration submission pending 阻止換 competition，不把原 commands body 送新路徑', async () => {
  lifecycle = 'open'
  fetchMock.mockImplementation((path: unknown, options: unknown) => {
    if (path === '/api/competitions/competition-a/commands') {
      if (operation(options) === 'registrationView') return Promise.resolve(registrationResponse('competition-a', 4, 'open'))
      if (method(options) === 'POST' && registrationAction(options) === 'registerTeam') return Promise.reject({ statusCode: 503 })
      throw new Error('fixture 只接受 registrationExecute／registerTeam mutation')
    }
    if (typeof path === 'string' && path.endsWith('/tournament')) return Promise.reject(notReady)
    throw new Error('fixture 不允許跨賽事重送報名命令')
  })
  const page = await mountPage()
  await page.get('.register-form').trigger('submit')
  await flushPromises()
  expect(page.text()).toContain('以相同命令重試')
  await currentRouter().push('/competitions/competition-b')
  await nextTick()
  expect(currentRouter().currentRoute.value.path).toBe('/competitions/competition-a')
  expect(page.text()).toContain('報名命令的結果仍未確認')
  expect(fetchMock.mock.calls.filter(call => operation(call[1]) === 'registrationExecute')).toHaveLength(1)
  const commands = fetchMock.mock.calls.filter(call => method(call[1]) === 'POST' && registrationAction(call[1]) === 'registerTeam')
  expect(commands).toHaveLength(1)
  expect(commands[0]?.[0]).toBe('/api/competitions/competition-a/commands')
  const body: unknown = commands[0]?.[1]?.body
  expect(body).toMatchObject({ commandId: queryId, expectedRevision: 4,
    operation: { type: 'registrationExecute', action: { type: 'registerTeam', members: ['organizer-a'] } } })
  expect(tournamentPosts()).toHaveLength(0)
})

it('輪次公告父頁接線：真 NuxtPage 掛載公告元件並傳入目前 scope 身分與鎖定 context', async () => {
  const snapshot = publicSnapshot()
  const currentStage = snapshot.stages[0]
  if (!currentStage) throw new Error('父接線 fixture 必須包含合法階段')
  const tournamentRead = { ...readTournament(), projection: { ok: true, operation: 'tournamentView', public: {
    ...snapshot, stages: [{ ...currentStage, rounds: [{ roundNumber: 1, kind: 'main', matches: [{
      matchId: `${stageId}-g0-r1-m1`, stageId, roundNumber: 1, sequence: 1, kind: 'main',
      participants: [...teamIds], status: 'pending', result: null, cancellationReason: null, started: false,
    }] }] }],
  } } }
  // 先驗證公開 fixture，再由真 NuxtPage 檢查缺少接線的 target；不 mock child 或 selector。
  const parsed = parseCompetitionTournamentViewResponse(tournamentRead, 'competition-a')
  expect(parsed.revision).toBe(20)
  expect(parsed.stages[0]?.rounds[0]?.matches[0]?.status).toBe('pending')
  const publicationPath = `/api/competitions/competition-a/stages/${stageId}/rounds/1/publication`
  fetchMock.mockImplementation((path: unknown, options: unknown) => {
    if (path === '/api/competitions/competition-a/commands' && operation(options) === 'registrationView') {
      return Promise.resolve(registrationResponse())
    }
    if (path === '/api/competitions/competition-a/tournament' && method(options) === 'GET') {
      return Promise.resolve(structuredClone(tournamentRead))
    }
    if (path === publicationPath && method(options) === 'GET') {
      return Promise.resolve({ competitionId: 'competition-a', revision: 20, publication: null })
    }
    throw new Error('父公告接線 fixture 非預期的 transport')
  })
  const page = await mountPage()
  expect(page.get('.revision-label').text()).toBe('狀態版本 4')
  const child = page.findComponent(CompetitionRoundPublication)
  expect(child.exists()).toBe(true)
  expect(child.props()).toMatchObject({
    competitionId: 'competition-a', sessionUserId: 'organizer-a', contextReady: true, busy: false,
    registration: { competitionId: 'competition-a', revision: 4, organizer: 'organizer-a', lifecycle: 'locked' },
  })
})

it('輪次公告父頁接線：unknown 刷新後保留原命令手動 retry 並隔離其他寫操作', async () => {
  const publicationPath = `/api/competitions/competition-a/stages/${stageId}/rounds/1/publication`
  const announcementId = '167ea0c8-9f27-4d91-9cf4-430f451ba12b'
  const roundStartMs = new Date(2030, 0, 2, 12, 10).getTime()
  const now = vi.spyOn(Date, 'now').mockReturnValue(new Date(2030, 0, 2, 12, 5).getTime())
  let revision = 20
  let started = false
  let announcementPost = deferred()
  const snapshot = publicSnapshot()
  const currentStage = snapshot.stages[0]
  if (!currentStage) throw new Error('父 unknown fixture 必須包含合法階段')
  const tournamentRead = () => ({ competitionId: 'competition-a', revision, projection: {
    ok: true, operation: 'tournamentView', public: { ...snapshot, stages: [{ ...currentStage, rounds: [{
      roundNumber: 1, kind: 'main', matches: [{ matchId: `${stageId}-g0-r1-m1`, stageId, roundNumber: 1,
        sequence: 1, kind: 'main', participants: [...teamIds], status: started ? 'inProgress' : 'pending',
        result: null, cancellationReason: null, started }],
    }] }] },
  } })
  const posts = () => fetchMock.mock.calls.filter(call => call[0] === publicationPath && method(call[1]) === 'POST')
  const initial = parseCompetitionTournamentViewResponse(tournamentRead(), 'competition-a')
  expect(initial.revision).toBe(20)
  expect(initial.stages[0]?.rounds[0]?.matches[0]?.status).toBe('pending')
  fetchMock.mockImplementation((path: unknown, options: unknown) => {
    if (path === '/api/competitions/competition-a/commands' && operation(options) === 'registrationView') {
      return queuedRegistrationReads.shift()?.promise ?? Promise.resolve(registrationResponse('competition-a', registrationRevision))
    }
    if (path === '/api/competitions/competition-a/tournament' && method(options) === 'GET') {
      return Promise.resolve(tournamentRead())
    }
    if (path === publicationPath && method(options) === 'GET') {
      return Promise.resolve({ competitionId: 'competition-a', revision, publication: null })
    }
    if (path === publicationPath && method(options) === 'POST') return announcementPost.promise
    throw new Error('父 unknown fixture 非預期的 transport')
  })
  const page = await mountPage()
  const child = page.findComponent(CompetitionRoundPublication)
  const management = page.findComponent(CompetitionTournamentManagement)
  expect(child.exists()).toBe(true)
  expect(page.get('.revision-label').text()).toBe('狀態版本 4')
  const panel = page.get('.round-publication').element
  await page.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  const uuid = vi.mocked(crypto.randomUUID)
  uuid.mockReturnValueOnce(announcementId)
  const uuidBaseline = uuid.mock.calls.length
  await page.get('[data-testid="round-announcement-form"]').trigger('submit')
  expect(uuid.mock.calls.length - uuidBaseline).toBe(1)
  expect(posts()).toHaveLength(1)
  const originalBody: unknown = posts()[0]?.[1]?.body
  const literalBody = { commandId: announcementId, expectedRevision: 20, roundStartMs, refereeAssignments: [] }
  expect(originalBody).toEqual(literalBody)
  expect(posts()[0]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  announcementPost.reject({ statusCode: 503 })
  await flushPromises()
  expect(child.emitted('unresolved')).toHaveLength(1)
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeUndefined()
  expect(child.props('busy')).toBe(false)
  // 以另一個真元件的公開 busy／DOM 門檻驗父互斥；不假造其他命令或直接改 pending。
  expect(management.props('busy')).toBe(true)
  expect(page.get('[data-testid="tournament-refresh"]').attributes('disabled')).toBeDefined()

  const retry = page.get('[data-testid="round-same-command-retry"]').element
  const refresh = deferred()
  queuedRegistrationReads.push(refresh)
  await page.get('.detail-heading .refresh-button').trigger('click')
  expect(page.get('.round-publication').element).toBe(panel)
  expect(page.get('[data-testid="round-same-command-retry"]').element).toBe(retry)
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeDefined()
  // 公開賽程在原 T 後進展為已開始；原命令不因此改輪次、版本或時間。
  revision = 25
  started = true
  now.mockReturnValue(roundStartMs + 1000)
  const progressed = parseCompetitionTournamentViewResponse(tournamentRead(), 'competition-a')
  expect(progressed.stages[0]?.rounds[0]?.matches[0]?.status).toBe('inProgress')
  registrationRevision = 25
  refresh.resolve(registrationResponse('competition-a', 25))
  await flushPromises()
  await nextTick()
  await flushPromises()
  expect(page.get('.round-publication').element).toBe(panel)
  expect(page.get('.revision-label').text()).toBe('狀態版本 25')
  expect(page.get('.round-publication').text()).toContain('目前輪次已有交手開始')
  expect(child.emitted('announced')).toBeUndefined()
  expect(posts()).toHaveLength(1)
  expect(management.props('busy')).toBe(true)
  expect(child.props('busy')).toBe(false)
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeUndefined()
  announcementPost = deferred()
  const retryUuidBaseline = uuid.mock.calls.length
  await page.get('[data-testid="round-same-command-retry"]').trigger('click')
  expect(uuid.mock.calls.length - retryUuidBaseline).toBe(0)
  expect(posts()).toHaveLength(2)
  expect(posts()[1]?.[0]).toBe(publicationPath)
  expect(posts()[1]?.[1]?.body).toBe(originalBody)
  expect(posts()[1]?.[1]?.body).toEqual(literalBody)
  expect(posts()[1]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  const uuidBeforeAck = uuid.mock.calls.length
  const mutationsBeforeAck = fetchMock.mock.calls.filter(call => method(call[1]) === 'POST' && operation(call[1]) !== 'registrationView')
  announcementPost.resolve({ competitionId: 'competition-a', revision: 21, commandId: announcementId, idempotent: true,
    publication: { roundRef: { competitionId: 'competition-a', stageId, roundNumber: 1, roundGenerationId: 'round-generation-a' },
      roundStartMs, publicationVersion: 1, status: 'materializing', releaseDecisionId: null, requiresReferee: false,
      rooms: [{ matchId: `${stageId}-g0-r1-m1`, gameId: 'fixed-room-1', ackConfirmedAtMs: null }] } })
  await flushPromises()
  await nextTick()
  await flushPromises()
  expect(child.emitted('announced')).toEqual([[{ competitionId: 'competition-a', stageId, roundNumber: 1,
    commandId: announcementId, revision: 21, roundStartMs }]])
  expect(page.find('[data-testid="round-same-command-retry"]').exists()).toBe(false)
  expect(management.props('busy')).toBe(false)
  expect(posts()).toHaveLength(2)
  expect(tournamentPosts()).toHaveLength(0)
  expect(fetchMock.mock.calls.filter(call => operation(call[1]) === 'registrationExecute')).toHaveLength(0)
  // 原收據低於最新權威查詢版本；ACK 不降版本，也不隱式發新命令或 UUID。
  expect(page.get('.revision-label').text()).toBe('狀態版本 25')
  expect(uuid.mock.calls.length).toBe(uuidBeforeAck)
  expect(fetchMock.mock.calls.filter(call => method(call[1]) === 'POST' && operation(call[1]) !== 'registrationView')).toEqual(mutationsBeforeAck)
})

it('輪次公告父頁接線：unknown 離不同 record 只需 child 一份確認且不假取消或保存', async () => {
  const publicationPath = `/api/competitions/competition-a/stages/${stageId}/rounds/1/publication`
  const announcementId = '167ea0c8-9f27-4d91-9cf4-430f451ba12b'
  const roundStartMs = new Date(2030, 0, 2, 12, 10).getTime()
  vi.spyOn(Date, 'now').mockReturnValue(new Date(2030, 0, 2, 12, 5).getTime())
  let announcementPost = deferred()
  const snapshot = publicSnapshot()
  const currentStage = snapshot.stages[0]
  if (!currentStage) throw new Error('父離頁 fixture 必須包含合法階段')
  const tournamentRead = { competitionId: 'competition-a', revision: 20, projection: {
    ok: true, operation: 'tournamentView', public: { ...snapshot, stages: [{ ...currentStage, rounds: [{
      roundNumber: 1, kind: 'main', matches: [{ matchId: `${stageId}-g0-r1-m1`, stageId, roundNumber: 1,
        sequence: 1, kind: 'main', participants: [...teamIds], status: 'pending',
        result: null, cancellationReason: null, started: false }],
    }] }] },
  } }
  const initial = parseCompetitionTournamentViewResponse(tournamentRead, 'competition-a')
  expect(initial.revision).toBe(20)
  expect(initial.stages[0]?.rounds[0]?.matches[0]?.status).toBe('pending')
  fetchMock.mockImplementation((path: unknown, options: unknown) => {
    if (path === '/api/competitions/competition-a/commands' && operation(options) === 'registrationView') {
      return Promise.resolve(registrationResponse())
    }
    if (path === '/api/competitions/competition-a/tournament' && method(options) === 'GET') return Promise.resolve(tournamentRead)
    if (path === publicationPath && method(options) === 'GET') {
      return Promise.resolve({ competitionId: 'competition-a', revision: 20, publication: null })
    }
    if (path === publicationPath && method(options) === 'POST') return announcementPost.promise
    throw new Error('父離頁 fixture 非預期的 transport')
  })
  const posts = () => fetchMock.mock.calls.filter(call => call[0] === publicationPath && method(call[1]) === 'POST')
  const page = await mountPage()
  const router = currentRouter()
  const removeRoute = router.addRoute({ name: 'publication-test-exit', path: '/publication-test-exit', component: defineComponent({
    setup: () => () => h('p', '正式公告離頁目的地'),
  }) })
  cleanups.push(removeRoute)
  const child = page.findComponent(CompetitionRoundPublication)
  const panel = page.get('.round-publication').element
  expect(child.exists()).toBe(true)
  await page.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  const uuid = vi.mocked(crypto.randomUUID)
  uuid.mockReturnValueOnce(announcementId)
  await page.get('[data-testid="round-announcement-form"]').trigger('submit')
  expect(posts()).toHaveLength(1)
  const originalBody: unknown = posts()[0]?.[1]?.body
  const literalBody = { commandId: announcementId, expectedRevision: 20, roundStartMs, refereeAssignments: [] }
  expect(originalBody).toEqual(literalBody)
  announcementPost.reject({ statusCode: 503 })
  await flushPromises()
  expect(child.emitted('unresolved')).toHaveLength(1)
  const retry = page.get('[data-testid="round-same-command-retry"]').element
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeUndefined()
  // 沿現有管理離頁案例的瀏覽器 seam；不 mock route guard 或注入私有 pending。
  const confirm = vi.fn<(message?: string) => boolean>().mockReturnValue(false).mockReturnValueOnce(false).mockReturnValueOnce(true)
  const originalConfirm = Object.getOwnPropertyDescriptor(window, 'confirm')
  Object.defineProperty(window, 'confirm', { configurable: true, writable: true, value: confirm })
  cleanups.push(() => {
    if (originalConfirm) Object.defineProperty(window, 'confirm', originalConfirm)
    else Reflect.deleteProperty(window, 'confirm')
  })
  const warning = '原公告命令尚未取得收據；離開不代表伺服器取消。仍要離開嗎？'
  await router.push('/publication-test-exit')
  await nextTick()
  await flushPromises()
  expect(confirm).toHaveBeenCalledTimes(1)
  expect(confirm).toHaveBeenNthCalledWith(1, warning)
  expect(router.currentRoute.value.path).toBe('/competitions/competition-a')
  expect(page.get('.round-publication').element).toBe(panel)
  expect(page.get('[data-testid="round-same-command-retry"]').element).toBe(retry)
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeUndefined()
  expect(child.emitted('unresolved')).toHaveLength(1)
  expect(posts()).toHaveLength(1)
  const unload = new Event('beforeunload', { cancelable: true })
  window.dispatchEvent(unload)
  expect(unload.defaultPrevented).toBe(true)
  announcementPost = deferred()
  const retryUuidBaseline = uuid.mock.calls.length
  await page.get('[data-testid="round-same-command-retry"]').trigger('click')
  expect(uuid.mock.calls.length).toBe(retryUuidBaseline)
  expect(posts()).toHaveLength(2)
  expect(posts()[1]?.[0]).toBe(publicationPath)
  expect(posts()[1]?.[1]?.body).toBe(originalBody)
  expect(posts()[1]?.[1]?.body).toEqual(literalBody)
  expect(posts()[1]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  const pendingSignal = requestSignal(posts()[1] ?? [])
  const readsBeforeExit = registrationReads().length
  await router.push('/publication-test-exit')
  await flushPromises()
  expect(confirm).toHaveBeenCalledTimes(2)
  expect(confirm).toHaveBeenNthCalledWith(2, warning)
  expect(router.currentRoute.value.path).toBe('/publication-test-exit')
  expect(page.text()).toContain('正式公告離頁目的地')
  expect(page.find('.round-publication').exists()).toBe(false)
  expect(pendingSignal.aborted).toBe(true)
  expect(child.emitted('unresolved')).toHaveLength(2)
  expect(child.emitted('unresolved')?.[1]?.[0]).toBe(child.emitted('unresolved')?.[0]?.[0])
  // Abort 只是本地停止接收；原命令仍 unresolved，晚到收據不得冒充離頁後成功。
  const uuidBeforeLateAck = uuid.mock.calls.length
  announcementPost.resolve({ competitionId: 'competition-a', revision: 21, commandId: announcementId, idempotent: true,
    publication: { roundRef: { competitionId: 'competition-a', stageId, roundNumber: 1, roundGenerationId: 'round-generation-a' },
      roundStartMs, publicationVersion: 1, status: 'materializing', releaseDecisionId: null, requiresReferee: false,
      rooms: [{ matchId: `${stageId}-g0-r1-m1`, gameId: 'fixed-room-1', ackConfirmedAtMs: null }] } })
  await flushPromises()
  expect(child.emitted('announced')).toBeUndefined()
  expect(registrationReads()).toHaveLength(readsBeforeExit)
  expect(uuid.mock.calls.length).toBe(uuidBeforeLateAck)
  expect(posts()).toHaveLength(2)
  expect(tournamentPosts()).toHaveLength(0)
  expect(fetchMock.mock.calls.filter(call => operation(call[1]) === 'registrationExecute')).toHaveLength(0)
})

it('真 NuxtPage 輪次公告 A→B→A 後拒收舊 valid ACK，保留原 intent 與目前投影', async () => {
  const competitionA = 'competition-a'
  const competitionB = 'competition-b'
  const announcementId = 'e62f7c1c-d998-4c7b-bb03-fc8c0f304ab8'
  const roundStartMs = new Date(2030, 0, 2, 12, 10).getTime()
  vi.spyOn(Date, 'now').mockReturnValue(new Date(2030, 0, 2, 12, 5).getTime())
  let announcementPost = deferred()

  const tournamentReadFor = (competitionId: string, revision: number) => {
    const snapshot = publicSnapshot(competitionId)
    const currentStage = snapshot.stages[0]
    if (!currentStage) throw new Error('ABA fixture 必須包含合法階段')
    return { competitionId, revision, projection: { ok: true, operation: 'tournamentView', public: {
      ...snapshot, stages: [{ ...currentStage, rounds: [{ roundNumber: 1, kind: 'main', matches: [{
        matchId: `${stageId}-g0-r1-m1`, stageId, roundNumber: 1, sequence: 1, kind: 'main',
        participants: [...teamIds], status: 'pending', result: null, cancellationReason: null, started: false,
      }] }] }],
    } } }
  }
  const publicationPathA = `/api/competitions/${competitionA}/stages/${stageId}/rounds/1/publication`
  const posts = () => fetchMock.mock.calls.filter(call => call[0] === publicationPathA && method(call[1]) === 'POST')

  for (const [competitionId, revision] of [[competitionA, 20], [competitionB, 30]] as const) {
    const parsed = parseCompetitionTournamentViewResponse(tournamentReadFor(competitionId, revision), competitionId)
    expect(parsed.revision).toBe(revision)
    expect(parsed.stages[0]?.rounds[0]?.matches[0]?.status).toBe('pending')
  }

  fetchMock.mockImplementation((path: unknown, options: unknown) => {
    if (typeof path !== 'string') throw new Error('ABA fixture 只接受明確路徑')
    const competitionId = decodeURIComponent(path.split('/')[3] ?? '')
    if (path.endsWith('/commands') && operation(options) === 'registrationView') {
      return Promise.resolve(registrationResponse(competitionId, competitionId === competitionA ? 4 : 7))
    }
    if (path.endsWith('/tournament') && method(options) === 'GET') {
      return Promise.resolve(tournamentReadFor(competitionId, competitionId === competitionA ? 20 : 30))
    }
    if (path.endsWith('/rounds/1/publication') && method(options) === 'GET') {
      return Promise.resolve({ competitionId, revision: competitionId === competitionA ? 20 : 30, publication: null })
    }
    if (path === publicationPathA && method(options) === 'POST') return announcementPost.promise
    throw new Error('ABA fixture 非預期的 transport')
  })

  const page = await mountPage()
  const router = currentRouter()
  const panel = page.get('.round-publication').element
  const uuid = vi.mocked(crypto.randomUUID)
  uuid.mockReturnValueOnce(announcementId)
  await page.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  await page.get('[data-testid="round-announcement-form"]').trigger('submit')
  expect(posts()).toHaveLength(1)
  const originalBody = { commandId: announcementId, expectedRevision: 20, roundStartMs, refereeAssignments: [] }
  expect(posts()[0]?.[1]?.body).toEqual(originalBody)
  expect(posts()[0]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  const originalSignal = requestSignal(posts()[0] ?? [])

  await router.push(`/competitions/${competitionB}`)
  await nextTick()
  await flushPromises()
  await nextTick()
  await flushPromises()
  expect(router.currentRoute.value.path).toBe(`/competitions/${competitionB}`)
  expect(originalSignal.aborted).toBe(true)
  expect(page.get('.detail-heading').text()).toContain(competitionB)
  expect(page.get('.round-publication').element).toBe(panel)
  expect(page.get('.round-publication').text()).toContain('原公告命令仍未確認')
  expect(posts()).toHaveLength(1)

  await router.push(`/competitions/${competitionA}`)
  await nextTick()
  await flushPromises()
  await nextTick()
  await flushPromises()
  expect(router.currentRoute.value.path).toBe(`/competitions/${competitionA}`)
  expect(page.get('.detail-heading').text()).toContain(competitionA)
  expect(page.get('.round-publication').element).toBe(panel)
  expect(page.get('.revision-label').text()).toBe('狀態版本 4')
  expect(page.get('.tournament-view .revision-note').text()).toBe('賽程版本 20')
  const retry = page.get('[data-testid="round-same-command-retry"]').element
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeUndefined()
  expect(page.get('.round-publication').text()).toContain('原公告命令仍未確認')
  expect(fetchMock.mock.calls.filter(call => call[0] === publicationPathA && method(call[1]) === 'GET').length).toBeGreaterThan(1)

  const uuidBeforeLateAck = uuid.mock.calls.length
  announcementPost.resolve({ competitionId: competitionA, revision: 21, commandId: announcementId, idempotent: true,
    publication: { roundRef: { competitionId: competitionA, stageId, roundNumber: 1, roundGenerationId: 'round-generation-aba' },
      roundStartMs, publicationVersion: 1, status: 'materializing', releaseDecisionId: null, requiresReferee: false,
      rooms: [{ matchId: `${stageId}-g0-r1-m1`, gameId: 'fixed-room-aba', ackConfirmedAtMs: null }] } })
  await flushPromises()
  await nextTick()
  await flushPromises()

  expect(page.get('.round-publication').element).toBe(panel)
  expect(page.get('[data-testid="round-same-command-retry"]').element).toBe(retry)
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeUndefined()
  expect(page.get('.round-publication').text()).toContain('原公告命令仍未確認')
  expect(page.find('.publication-state').exists()).toBe(false)
  expect(page.text()).not.toContain('公告收據已確認')
  expect(page.get('.revision-label').text()).toBe('狀態版本 4')
  expect(page.get('.tournament-view .revision-note').text()).toBe('賽程版本 20')
  expect(page.findComponent(CompetitionTournamentManagement).props('busy')).toBe(true)
  expect(posts()).toHaveLength(1)
  expect(posts()[0]?.[1]?.body).toEqual(originalBody)
  expect(uuid.mock.calls.length).toBe(uuidBeforeLateAck)
  expect(uuid.mock.results.filter(result => result.type === 'return' && result.value === announcementId)).toHaveLength(1)

  // 只有明確點擊原 retry 才可再送；沿用相同 body 與 UUID。
  announcementPost = deferred()
  const uuidBeforeManualRetry = uuid.mock.calls.length
  await page.get('[data-testid="round-same-command-retry"]').trigger('click')
  expect(posts()).toHaveLength(2)
  expect(posts()[1]?.[1]?.body).toBe(posts()[0]?.[1]?.body)
  expect(posts()[1]?.[1]?.body).toEqual(originalBody)
  expect(uuid.mock.calls.length).toBe(uuidBeforeManualRetry)
})

it('輪次公告父頁接線：原收據結清 unknown 命令但不覆寫同輪 GET 已確認 ACK', async () => {
  const { buildCompetitionRoundPublicationIntent, parseCompetitionRoundPublicationReadSnapshot,
    parseCompetitionRoundAnnouncementAck } = await import('~/lib/competition-round-publication')
  const announcementId = '9ec4eab8-41ae-46b9-8b78-d607d2fa23e9'
  const roundStartMs = new Date(2030, 0, 2, 12, 10).getTime()
  const ackConfirmedAtMs = new Date(2030, 0, 2, 12, 5).getTime()
  vi.spyOn(Date, 'now').mockReturnValue(ackConfirmedAtMs)
  const context = { competitionId: 'competition-a', sessionUserId: 'organizer-a' }
  const scope = { ...context, stageId, roundNumber: 1 }
  const roundRef = { competitionId: 'competition-a', stageId, roundNumber: 1,
    roundGenerationId: 'round-generation-receipt-projection' }
  const matchId = `${stageId}-g0-r1-m1`
  const gameId = 'fixed-room-receipt-projection'
  const publicationPath = `/api/competitions/competition-a/stages/${stageId}/rounds/1/publication`
  const snapshot = publicSnapshot()
  const currentStage = snapshot.stages[0]
  if (!currentStage) throw new Error('收據投影 fixture 必須包含合法階段')
  const tournamentRead = (revision: number) => ({ competitionId: 'competition-a', revision, projection: {
    ok: true, operation: 'tournamentView', public: { ...snapshot, stages: [{ ...currentStage, rounds: [{
      roundNumber: 1, kind: 'main', matches: [{ matchId, stageId, roundNumber: 1, sequence: 1, kind: 'main',
        participants: [...teamIds], status: 'pending', result: null, cancellationReason: null, started: false }],
    }] }] },
  } })
  const emptyRead = { competitionId: 'competition-a', revision: 20, publication: null }
  const newerRead = { competitionId: 'competition-a', revision: 21, publication: {
    roundRef, roundStartMs, publicationVersion: 2, status: 'materializing', releaseDecisionId: null,
    requiresReferee: false, rooms: [{ matchId, gameId, ackConfirmedAtMs }],
  } }
  // 原始五欄收據保持 v1／ACK null；不能從較新 GET fixture 複製出假收據。
  const originalAck = { competitionId: 'competition-a', revision: 21, commandId: announcementId, idempotent: true,
    publication: { roundRef, roundStartMs, publicationVersion: 1, status: 'materializing', releaseDecisionId: null,
      requiresReferee: false, rooms: [{ matchId, gameId, ackConfirmedAtMs: null }] } }
  const initialTournament = parseCompetitionTournamentViewResponse(tournamentRead(20), 'competition-a')
  const refreshedTournament = parseCompetitionTournamentViewResponse(tournamentRead(21), 'competition-a')
  expect(initialTournament.revision).toBe(20)
  expect(refreshedTournament.revision).toBe(21)
  expect(refreshedTournament.stages[0]?.rounds[0]?.matches[0]?.status).toBe('pending')
  expect(parseCompetitionRoundPublicationReadSnapshot(emptyRead, scope).publication).toBeNull()
  const parsedRead = parseCompetitionRoundPublicationReadSnapshot(newerRead, scope)
  expect(parsedRead.revision).toBe(21)
  expect(parsedRead.publication).toMatchObject({ roundRef, roundStartMs, publicationVersion: 2,
    rooms: [{ matchId, gameId, ackConfirmedAtMs }] })
  expect(ackConfirmedAtMs).toBeLessThanOrEqual(roundStartMs - 300_000)
  // builder 僅供公開 parser 的合法性前提；送出的預期 body 另用獨立 literal。
  const fixtureIntent = buildCompetitionRoundPublicationIntent(context, initialTournament,
    '2030-01-02T12:10', ackConfirmedAtMs, () => announcementId)
  expect(parseCompetitionRoundAnnouncementAck(originalAck, fixtureIntent)).toMatchObject({
    commandId: announcementId, revision: 21, publication: { publicationVersion: 1, rooms: [{ ackConfirmedAtMs: null }] },
  })

  let refreshed = false
  let announcementPost = deferred()
  const posts = () => fetchMock.mock.calls.filter(call => call[0] === publicationPath && method(call[1]) === 'POST')
  fetchMock.mockImplementation((path: unknown, options: unknown) => {
    if (path === '/api/competitions/competition-a/commands' && operation(options) === 'registrationView') {
      return Promise.resolve(registrationResponse())
    }
    if (path === '/api/competitions/competition-a/tournament' && method(options) === 'GET') {
      return Promise.resolve(tournamentRead(refreshed ? 21 : 20))
    }
    if (path === publicationPath && method(options) === 'GET') {
      return Promise.resolve(structuredClone(refreshed ? newerRead : emptyRead))
    }
    if (path === publicationPath && method(options) === 'POST') return announcementPost.promise
    throw new Error('收據投影 fixture 非預期的 transport')
  })
  const page = await mountPage()
  const child = page.findComponent(CompetitionRoundPublication)
  const management = page.findComponent(CompetitionTournamentManagement)
  expect(child.exists()).toBe(true)
  expect(page.get('.revision-label').text()).toBe('狀態版本 4')
  const uuid = vi.mocked(crypto.randomUUID)
  uuid.mockReturnValueOnce(announcementId)
  await page.get('[data-testid="round-start-time"]').setValue('2030-01-02T12:10')
  const uuidBeforeSubmit = uuid.mock.calls.length
  await page.get('[data-testid="round-announcement-form"]').trigger('submit')
  expect(uuid.mock.calls.length).toBe(uuidBeforeSubmit + 1)
  expect(posts()).toHaveLength(1)
  const originalBody: unknown = posts()[0]?.[1]?.body
  const literalBody = { commandId: announcementId, expectedRevision: 20, roundStartMs, refereeAssignments: [] }
  expect(originalBody).toEqual(literalBody)
  expect(posts()[0]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  announcementPost.reject({ statusCode: 503 })
  await flushPromises()
  expect(child.emitted('unresolved')).toHaveLength(1)
  expect(management.props('busy')).toBe(true)
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeUndefined()

  refreshed = true
  const uuidBeforeRefresh = uuid.mock.calls.length
  await page.get('[data-testid="round-publication-refresh"]').trigger('click')
  await flushPromises()
  await nextTick()
  await flushPromises()
  const ackSelector = '.publication-state ul[aria-label="每間房間的伺服器確認狀態"] li'
  const confirmedAckText = page.get(ackSelector).text()
  expect(confirmedAckText).toContain('12:05')
  expect(confirmedAckText).not.toContain('尚未收到')
  expect(page.get('.round-publication').text()).toContain('原公告命令仍未確認')
  expect(page.get('[data-testid="round-same-command-retry"]').attributes('disabled')).toBeUndefined()
  expect(child.emitted('announced')).toBeUndefined()
  expect(management.props('busy')).toBe(true)
  expect(posts()).toHaveLength(1)
  expect(posts()[0]?.[1]?.body).toEqual(literalBody)
  expect(uuid.mock.calls.length).toBe(uuidBeforeRefresh)

  announcementPost = deferred()
  const uuidBeforeRetry = uuid.mock.calls.length
  await page.get('[data-testid="round-same-command-retry"]').trigger('click')
  expect(posts()).toHaveLength(2)
  expect(posts()[1]?.[0]).toBe(publicationPath)
  expect(posts()[1]?.[1]?.body).toBe(originalBody)
  expect(posts()[1]?.[1]?.body).toEqual(literalBody)
  expect(posts()[1]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  expect(uuid.mock.calls.length).toBe(uuidBeforeRetry)
  announcementPost.resolve(originalAck)
  await flushPromises()
  await nextTick()
  await flushPromises()
  expect(child.emitted('announced')).toEqual([[{ competitionId: 'competition-a', stageId, roundNumber: 1,
    commandId: announcementId, revision: 21, roundStartMs }]])
  expect(page.find('[data-testid="round-same-command-retry"]').exists()).toBe(false)
  expect(management.props('busy')).toBe(false)
  expect(page.get('.revision-label').text()).toBe('狀態版本 4')
  expect(posts()).toHaveLength(2)
  expect(uuid.mock.calls.length).toBe(uuidBeforeRetry)
  // 收據結清 command，但不能把同輪 GET 已確認的房間 ACK 倒退為尚未收到。
  expect(page.get(ackSelector).text()).toBe(confirmedAckText)
  expect(page.get(ackSelector).text()).not.toContain('尚未收到')
})
