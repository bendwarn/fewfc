import { mountSuspended, mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import CompetitionRulingCorrection from '~/components/CompetitionRulingCorrection.vue'
import CompetitionDetail from '~/pages/competitions/[id].vue'

const fetchMock = vi.hoisted(() => vi.fn())
mockNuxtImport('$fetch', () => fetchMock)
const getSessionMock = vi.hoisted(() => vi.fn())
vi.mock('~/lib/auth-client', () => ({ authClient: { getSession: getSessionMock } }))
const teams: [string, string] = ['b45abe94-a252-4e3d-bff2-7d26c6e2fabc', '535ec115-7bf4-4bbf-b0d3-c6e0ee76c8b7']
const commandId = 'b9ae10af-54da-429e-a10c-5e8de84021a2'
const correctionId = 'cd0d3e8b-7c87-46c1-b50d-deb6fb2ad5fa'
const replacementRulingRecordId = '9d3d7b53-a9e8-41f7-831b-028055661cdd'

// 此為依 actual Rust camelCase 的 HTTP fixture，不是 native proof。
function response(revision = 4) {
  return {
    competitionId: 'competition a', matchId: 'match/one', matchRevision: revision,
    matchProjection: { ok: true, operation: 'matchSubmitGame', public: { match: {
      config: { matchId: 'match/one', teams: [...teams], format: { type: 'fixedGames', value: 1 },
        firstGame: { type: 'assigned', team: teams[0] }, referees: [{ refereeId: 'referee-a', conflictedTeam: null }] },
      status: 'completed', score: [1, 0], games: [{ gameNumber: 1, gameRecordId: 'game-record', startKind: { type: 'ordinaryNewGame' },
        firstPlayer: teams[0], rulingRecordId: 'ruling-original', terminalEvidence: { type: 'canonicalGameEnded', conclusion: {
          outcome: { type: 'winner', team: teams[0] }, causes: [{ type: 'teamHpDepleted', teams: [teams[1]] }],
        } } }],
      rulings: [{ rulingRecordId: 'ruling-original', gameRecordId: 'game-record', ruling: { type: 'gameAccepted', outcome: { winner: teams[0] } } }],
      corrections: [], outcome: { type: 'winner', winnerId: teams[0], loserId: teams[1], score: [1, 0] }, nextFirstPlayer: null, nextGame: null,
    } } }, clockProjection: null,
  }
}
function ack() { return { competitionId: 'competition a', matchId: 'match/one', matchRevision: 5, commandId, idempotent: false } }
function deferred() {
  let resolve!: (value: unknown) => void
  let reject!: (value: unknown) => void
  const promise = new Promise<unknown>((accept, refuse) => { resolve = accept; reject = refuse })
  return { promise, resolve, reject }
}
function signalAt(index: number): AbortSignal {
  const value: unknown = fetchMock.mock.calls[index]?.[1]?.signal
  if (!(value instanceof AbortSignal)) throw new Error('HTTP 需要 abort signal')
  return value
}
function bodyAt(index: number): unknown { return fetchMock.mock.calls[index]?.[1]?.body }
const cleanups: (() => void)[] = []
async function mountCorrection(overrides: { competitionId?: string; matchId?: string; sessionUserId?: string | null; contextReady?: boolean } = {}) {
  const wrapper = await mountSuspended(CompetitionRulingCorrection, { props: {
    competitionId: 'competition a', matchId: 'match/one', sessionUserId: 'referee-a', contextReady: true, ...overrides,
  } })
  cleanups.push(() => { if (wrapper.exists()) wrapper.unmount() })
  await flushPromises()
  return wrapper
}
async function draft(wrapper: Awaited<ReturnType<typeof mountCorrection>>) {
  await wrapper.get('[data-testid="correction-game"]').setValue('game-record')
  await wrapper.get('[data-testid="correction-candidate"]').setValue('saved:ruling-original')
  await wrapper.get('[data-testid="correction-reason"]').setValue('核對原始結論')
}
beforeEach(() => {
  fetchMock.mockReset()
  getSessionMock.mockReset().mockResolvedValue({ data: { user: { id: 'referee-a', name: '裁判' } }, error: null })
  vi.spyOn(crypto, 'randomUUID').mockReturnValueOnce(commandId).mockReturnValueOnce(correctionId).mockReturnValueOnce(replacementRulingRecordId)
})
afterEach(() => {
  for (const cleanup of cleanups.splice(0)) cleanup()
  vi.restoreAllMocks()
})

it('已指派無衝突裁判明示送出七欄命令，matching 五欄 ACK 後 readonly refresh，不改原始結論', async () => {
  fetchMock.mockResolvedValueOnce(response()).mockResolvedValueOnce(ack()).mockResolvedValueOnce(response(5))
  const wrapper = await mountCorrection()
  expect(wrapper.text()).toContain('甲隊')
  expect(wrapper.text()).toContain('乙隊')
  expect(wrapper.text()).toContain('第 1 局')
  expect(wrapper.text()).toContain('原始遊戲結論：甲隊獲勝')
  for (const value of [...teams, 'game-record', 'ruling-original', 'canonicalGameEnded', 'gameAccepted']) expect(wrapper.text()).not.toContain(value)
  expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/competitions/competition%20a/matches/match%2Fone')
  expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: 'GET', retry: 0 })
  expect(bodyAt(0)).toBeUndefined()
  await draft(wrapper)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  await flushPromises()
  expect(bodyAt(1)).toEqual({ commandId, expectedMatchRevision: 4, correctionId, gameRecordId: 'game-record', replacementRulingRecordId,
    replacementRuling: { type: 'gameAccepted', outcome: { winner: teams[0] } }, reason: '核對原始結論' })
  expect(fetchMock.mock.calls[1]?.[0]).toBe('/api/competitions/competition%20a/matches/match%2Fone/correct-ruling')
  expect(fetchMock.mock.calls[1]?.[1]).toMatchObject({ method: 'POST', retry: 0 })
  expect(crypto.randomUUID).toHaveBeenCalledTimes(3)
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(wrapper.text()).toContain('更正已獲伺服器收據確認')
  expect(wrapper.find('[data-testid="same-command-retry"]').exists()).toBe(false)
})

it('POST 未完成時鎖草稿與 double-submit；只有明示操作才送出一次', async () => {
  const post = deferred()
  fetchMock.mockResolvedValueOnce(response()).mockReturnValueOnce(post.promise)
  const wrapper = await mountCorrection()
  await draft(wrapper)
  expect(fetchMock).toHaveBeenCalledTimes(1)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  expect(signalAt(1).aborted).toBe(false)
  expect(wrapper.get('[data-testid="correction-reason"]').attributes('disabled')).toBeDefined()
  expect(wrapper.get('[data-testid="same-command-retry"]').attributes('disabled')).toBeDefined()
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(crypto.randomUUID).toHaveBeenCalledTimes(3)
  post.reject({ statusCode: 503, message: 'secret transport detail' })
  await flushPromises()
  expect(wrapper.get('[data-testid="same-command-retry"]').attributes('disabled')).toBeUndefined()
  expect(wrapper.text()).not.toContain('secret transport detail')
})

it('unknown→read 新 revision→403/409/404 仍只重試原 snapshot；原 expected+1 receipt 才解除', async () => {
  fetchMock.mockResolvedValueOnce(response()).mockRejectedValueOnce({ statusCode: 503 })
    .mockResolvedValueOnce(response(18)).mockRejectedValueOnce({ statusCode: 403 })
    .mockRejectedValueOnce({ statusCode: 409 }).mockResolvedValueOnce(response(19))
    .mockRejectedValueOnce({ statusCode: 404 }).mockResolvedValueOnce({ ...ack(), idempotent: true }).mockResolvedValueOnce(response(19))
  const wrapper = await mountCorrection()
  await draft(wrapper)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  await flushPromises()
  const original = bodyAt(1)
  await wrapper.get('.lookup-form').trigger('submit')
  await flushPromises()
  expect(wrapper.find('[data-testid="same-command-retry"]').exists()).toBe(true)
  expect(wrapper.get('[data-testid="correction-reason"]').attributes('disabled')).toBeDefined()
  for (const index of [3, 4, 6]) {
    await wrapper.get('[data-testid="same-command-retry"]').trigger('click')
    await flushPromises()
    expect(bodyAt(index)).toEqual(original)
    expect(wrapper.find('[data-testid="same-command-retry"]').exists()).toBe(true)
  }
  await wrapper.get('[data-testid="same-command-retry"]').trigger('click')
  await flushPromises()
  expect(bodyAt(7)).toEqual(original)
  expect(wrapper.find('[data-testid="same-command-retry"]').exists()).toBe(false)
  expect(wrapper.text()).toContain('更正已獲伺服器收據確認')
  expect(crypto.randomUUID).toHaveBeenCalledTimes(3)
  expect(fetchMock).toHaveBeenCalledTimes(9)
})

it('malformed 2xx/錯收據不作成功、不回顯 private，也不換命令', async () => {
  fetchMock.mockResolvedValueOnce(response()).mockResolvedValueOnce({ ...ack(), private: 'secret' })
    .mockResolvedValueOnce({ ...ack(), commandId: 'wrong' })
  const wrapper = await mountCorrection()
  await draft(wrapper)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  await flushPromises()
  expect(wrapper.text()).toContain('更正命令的結果尚未確認')
  expect(wrapper.text()).not.toContain('secret')
  await wrapper.get('[data-testid="same-command-retry"]').trigger('click')
  await flushPromises()
  expect(bodyAt(2)).toEqual(bodyAt(1))
  expect(wrapper.find('[data-testid="same-command-retry"]').exists()).toBe(true)
})

it('首次明確409只refresh而不 replay，重新檢視後仍需另一個明示操作', async () => {
  fetchMock.mockResolvedValueOnce(response()).mockRejectedValueOnce({ statusCode: 409 }).mockResolvedValueOnce(response(5))
  const wrapper = await mountCorrection()
  await draft(wrapper)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(fetchMock.mock.calls[2]?.[1]?.method).toBe('GET')
  expect(wrapper.text()).toContain('沒有自動重送或改動命令')
  expect(wrapper.find('[data-testid="same-command-retry"]').exists()).toBe(false)
})

it('精確 ACK 後 read 失敗不退回未知命令，提供唯讀重新整理', async () => {
  fetchMock.mockResolvedValueOnce(response()).mockResolvedValueOnce(ack()).mockRejectedValueOnce({ statusCode: 503 })
  const wrapper = await mountCorrection()
  await draft(wrapper)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  await flushPromises()
  expect(wrapper.text()).toContain('更正已獲伺服器收據確認，但交手顯示尚未更新')
  expect(wrapper.find('[data-testid="same-command-retry"]').exists()).toBe(false)
  expect(wrapper.get('[data-testid="correction-refresh"]').attributes('disabled')).toBeUndefined()
})

it.each(['competition', 'match', 'session'])('讀取中切換%s，舊 response/finally 不能復活投影或解鎖新讀取', async (change) => {
  const old = deferred(), current = deferred()
  fetchMock.mockReturnValueOnce(old.promise).mockReturnValueOnce(current.promise)
  const wrapper = await mountCorrection()
  const oldSignal = signalAt(0)
  await wrapper.setProps(change === 'competition' ? { competitionId: 'competition b' }
    : change === 'match' ? { matchId: 'match/two' } : { sessionUserId: 'referee-b' })
  expect(oldSignal.aborted).toBe(true)
  expect(wrapper.text()).not.toContain('原始遊戲結論：甲隊獲勝')
  await wrapper.get('.lookup-form').trigger('submit')
  expect(wrapper.get('[data-testid="correction-refresh"]').attributes('disabled')).toBeDefined()
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(2)
  old.resolve(response())
  await flushPromises()
  expect(wrapper.text()).not.toContain('原始遊戲結論：甲隊獲勝')
  expect(wrapper.get('[data-testid="correction-refresh"]').attributes('disabled')).toBeDefined()
  const next = response()
  if (change === 'competition') next.competitionId = 'competition b'
  if (change === 'match') { next.matchId = 'match/two'; next.matchProjection.public.match.config.matchId = 'match/two' }
  current.resolve(next)
  await flushPromises()
  expect(wrapper.get('[data-testid="correction-refresh"]').attributes('disabled')).toBeUndefined()
  expect(wrapper.text()).toContain('原始遊戲結論：甲隊獲勝')
})

it('同 context 暫不可用時 abort 未知 POST但保留原intent；晚ACK與finally不能解鎖新retry', async () => {
  const oldPost = deferred(), retry = deferred()
  fetchMock.mockResolvedValueOnce(response()).mockReturnValueOnce(oldPost.promise)
    .mockResolvedValueOnce(response(5)).mockReturnValueOnce(retry.promise)
  const wrapper = await mountCorrection()
  await draft(wrapper)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  const original = bodyAt(1)
  await wrapper.setProps({ contextReady: false })
  expect(signalAt(1).aborted).toBe(true)
  expect(wrapper.get('[data-testid="same-command-retry"]').attributes('disabled')).toBeDefined()
  await wrapper.setProps({ contextReady: true })
  await flushPromises()
  await wrapper.get('[data-testid="same-command-retry"]').trigger('click')
  expect(bodyAt(3)).toEqual(original)
  oldPost.resolve(ack())
  await flushPromises()
  expect(wrapper.get('[data-testid="same-command-retry"]').attributes('disabled')).toBeDefined()
  expect(wrapper.text()).not.toContain('更正已獲伺服器收據確認')
  retry.reject({ statusCode: 503 })
  await flushPromises()
  expect(wrapper.get('[data-testid="same-command-retry"]').attributes('disabled')).toBeUndefined()
})

it('pending scope/session 改變保留明確未知提示，不跨身份送出，晚錯誤不可反射或清新busy', async () => {
  const oldPost = deferred(), newRead = deferred()
  fetchMock.mockResolvedValueOnce(response()).mockReturnValueOnce(oldPost.promise).mockReturnValueOnce(newRead.promise)
  const wrapper = await mountCorrection()
  await draft(wrapper)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  await wrapper.setProps({ sessionUserId: 'referee-b' })
  expect(signalAt(1).aborted).toBe(true)
  expect(wrapper.text()).toContain('請切回原賽事、交手及同一帳號')
  expect(wrapper.find('[data-testid="same-command-retry"]').exists()).toBe(false)
  await wrapper.get('.lookup-form').trigger('submit')
  oldPost.reject({ statusCode: 403, message: 'secret old identity' })
  await flushPromises()
  expect(wrapper.text()).not.toContain('secret old identity')
  expect(wrapper.get('[data-testid="correction-refresh"]').attributes('disabled')).toBeDefined()
  newRead.resolve(response(5))
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(wrapper.find('[data-testid="correction-form"]').exists()).toBe(false)
})

it.each(['organizer', 'unassigned', 'conflicted'])('%s只有公共證據呈現，不能被client冒充active裁判', async (kind) => {
  const value = response()
  const raw: unknown = kind === 'conflicted' ? { ...value, matchProjection: { ...value.matchProjection, public: { match: {
    ...value.matchProjection.public.match, config: { ...value.matchProjection.public.match.config, referees: [{ refereeId: 'referee-a', conflictedTeam: teams[0] }] },
  } } } } : value
  fetchMock.mockResolvedValueOnce(raw)
  const wrapper = await mountCorrection({ sessionUserId: kind === 'conflicted' ? 'referee-a' : kind })
  expect(wrapper.text()).toContain('原始遊戲結論：甲隊獲勝')
  expect(wrapper.text()).toContain('此交手未指派你為無利益衝突的裁判')
  expect(wrapper.find('[data-testid="correction-form"]').exists()).toBe(false)
  expect(fetchMock).toHaveBeenCalledTimes(1)
})

it.each([{ sessionUserId: null }, { contextReady: false }])('無 session 或未確認 context 不查交手', async (overrides) => {
  const wrapper = await mountCorrection(overrides)
  expect(wrapper.text()).toContain('請登入並等待目前賽事狀態確認')
  expect(wrapper.get('[data-testid="correction-refresh"]').attributes('disabled')).toBeDefined()
  expect(fetchMock).not.toHaveBeenCalled()
})

it.each(['null', 'malformed'])('%s projection安全停用；不 render私密值/機器狀態也不能submit', async (kind) => {
  fetchMock.mockResolvedValueOnce(kind === 'null' ? { ...response(), matchProjection: null } : { ...response(), private: 'secret deck' })
  const wrapper = await mountCorrection()
  expect(wrapper.text()).toContain(kind === 'null' ? '目前沒有可顯示的交手紀錄' : '無法安全驗證')
  expect(wrapper.text()).not.toContain('secret deck')
  expect(wrapper.find('[data-testid="correction-form"]').exists()).toBe(false)
})

it('pending卸載中止request且回報未確認，不假稱server取消；beforeunload明示阻止無提示離開', async () => {
  const post = deferred()
  fetchMock.mockResolvedValueOnce(response()).mockReturnValueOnce(post.promise)
  const wrapper = await mountCorrection()
  await draft(wrapper)
  await wrapper.get('[data-testid="correction-form"]').trigger('submit')
  const event = new Event('beforeunload', { cancelable: true })
  window.dispatchEvent(event)
  expect(event.defaultPrevented).toBe(true)
  wrapper.unmount()
  expect(signalAt(1).aborted).toBe(true)
  expect(wrapper.emitted('unresolved')).toHaveLength(1)
  post.resolve(ack())
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(2)
})

it('真正父頁重新整理報名loading不卸載child，也不遺失未知更正snapshot', async () => {
  // 父頁 registrationView 也正常產 UUID；不沿用純元件 beforeEach 的三個 once 值。
  const uuidSpy = vi.mocked(crypto.randomUUID)
  uuidSpy.mockReset().mockReturnValueOnce('22c0dda2-58f6-4a79-b033-217f6a8dfe25')
    .mockReturnValue('e35dfb1a-d5bb-4e64-a893-7c2a7ffb91c8')
  const registrationRefresh = deferred()
  let registrationReads = 0
  const registration = {
    competitionId: 'competition-a', revision: 8, projection: { ok: true, operation: 'registrationView', public: {
      announcement: { organizer: 'organizer', ruleVersion: '5.17', ruleset: 'base', enabledRuleModules: [], teamSize: 1, initialHp: 1,
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
      lifecycle: 'locked', revision: 2, teams: [],
    } },
  }
  const matchResponse = { ...response(), competitionId: 'competition-a' }
  fetchMock.mockImplementation((path: unknown, options: unknown) => {
    if (path === '/api/competitions/competition-a/commands') return ++registrationReads === 1 ? Promise.resolve(registration) : registrationRefresh.promise
    if (path === '/api/competitions/competition-a/matches/match%2Fone/correct-ruling') return Promise.reject({ statusCode: 503 })
    if (path === '/api/competitions/competition-a/matches/match%2Fone') return Promise.resolve(matchResponse)
    throw new Error(`fixture 非預期的 transport method：${typeof options}`)
  })
  const page = await mountSuspended(CompetitionDetail, { route: '/competitions/competition-a', global: { stubs: {
    CompetitionPersonalDeckSubmission: true, CompetitionParticipantGameEntry: true, CompetitionTournamentView: true,
  } } })
  cleanups.push(() => { if (page.exists()) page.unmount() })
  await flushPromises()
  const child = page.findComponent(CompetitionRulingCorrection)
  await child.get('[data-testid="correction-match"]').setValue('match/one')
  await child.get('.lookup-form').trigger('submit')
  await flushPromises()
  await child.get('[data-testid="correction-game"]').setValue('game-record')
  await child.get('[data-testid="correction-candidate"]').setValue('saved:ruling-original')
  await child.get('[data-testid="correction-reason"]').setValue('核對')
  uuidSpy.mockReturnValueOnce(commandId).mockReturnValueOnce(correctionId).mockReturnValueOnce(replacementRulingRecordId)
  const submissionUuidBaseline = uuidSpy.mock.calls.length
  await child.get('[data-testid="correction-form"]').trigger('submit')
  await flushPromises()
  expect(uuidSpy.mock.calls.length - submissionUuidBaseline).toBe(3)
  expect(uuidSpy.mock.results.slice(submissionUuidBaseline).map(result => result.type === 'return' ? result.value : undefined))
    .toEqual([commandId, correctionId, replacementRulingRecordId])
  const submittedBody: unknown = fetchMock.mock.calls.find(call => typeof call[0] === 'string' && call[0].endsWith('/correct-ruling'))?.[1]?.body
  expect(submittedBody).toEqual({ commandId, expectedMatchRevision: 4, correctionId, gameRecordId: 'game-record', replacementRulingRecordId,
    replacementRuling: { type: 'gameAccepted', outcome: { winner: teams[0] } }, reason: '核對' })
  // VTU fresh wrapper 會建立新 VM proxy；以目前頁面的同一 DOM 與事件驗不卸載。
  const panel = child.get('.correction-panel').element
  const retryButton = child.get('[data-testid="same-command-retry"]').element
  expect(child.emitted('unresolved')).toBeUndefined()
  await page.get('.detail-heading .refresh-button').trigger('click')
  expect(page.get('.correction-panel').element).toBe(panel)
  expect(page.get('[data-testid="same-command-retry"]').element).toBe(retryButton)
  expect(child.emitted('unresolved')).toBeUndefined()
  expect(page.get('.correction-panel').text()).toContain('更正命令的結果尚未確認')
  expect(page.get('[data-testid="same-command-retry"]').attributes('disabled')).toBeDefined()
  registrationRefresh.resolve({ ...registration, revision: 9 })
  await flushPromises()
  expect(page.get('.correction-panel').element).toBe(panel)
  expect(page.get('[data-testid="same-command-retry"]').element).toBe(retryButton)
  expect(child.emitted('unresolved')).toBeUndefined()
  expect(page.get('.correction-panel').text()).toContain('更正命令的結果尚未確認')
  expect(page.get('[data-testid="same-command-retry"]').attributes('disabled')).toBeUndefined()
  expect(registrationReads).toBe(2)
  const retryUuidBaseline = uuidSpy.mock.calls.length
  await page.get('[data-testid="same-command-retry"]').trigger('click')
  await flushPromises()
  expect(uuidSpy.mock.calls.length - retryUuidBaseline).toBe(0)
  const posts = fetchMock.mock.calls.filter(call => typeof call[0] === 'string' && call[0].endsWith('/correct-ruling'))
  expect(posts).toHaveLength(2)
  const firstBody: unknown = posts[0]?.[1]?.body
  const secondBody: unknown = posts[1]?.[1]?.body
  expect(firstBody).toEqual(submittedBody)
  expect(secondBody).toEqual(firstBody)
})
