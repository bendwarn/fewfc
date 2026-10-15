import { mountSuspended, mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { useNuxtApp } from '#app'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import CompetitionDetail from '~/pages/competitions/[id].vue'
import CompetitionPersonalDeckSubmission from '~/components/CompetitionPersonalDeckSubmission.vue'
import { usePlayerSession } from '~/composables/usePlayerSession'
import { formatCompetitionDeadline } from '~/lib/competition-registration'
import type { CompetitionPublicTeam, CompetitionRegistrationLifecycle } from '~/lib/competition-registration'
import { PLAYER_DECK_ELEMENT_ORDER, PLAYER_DECK_LEVEL_ORDER } from '~/lib/player-deck'
import type { RulesCatalog } from '~/types/fewfc'

const { fetchMock, getSessionMock } = vi.hoisted(() => ({ fetchMock: vi.fn(), getSessionMock: vi.fn() }))
mockNuxtImport('$fetch', () => fetchMock)
// 只 mock 登入／HTTP 邊界；頁面、parser、牌組元件與規則 policy 都是真程式。
vi.mock('~/lib/auth-client', () => ({ authClient: { getSession: getSessionMock } }))

const registrationDeadline = Date.UTC(2030, 0, 2, 13, 0)
const checkInDeadline = Date.UTC(2030, 0, 2, 14, 0)
const announcementPlan = {
  initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
  swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
  rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
  exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
  stages: [{ stageId: 'stage-1', format: 'roundRobin', matchFormat: { type: 'fixedGames', value: 1 },
    clockConfig: { mainTimeMs: 300_000, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
    swissRounds: null, swissPolicy: null, qualification: { type: 'allEligibleTeams' } }],
}
const catalog: RulesCatalog = {
  version: 1,
  ruleModules: [{ id: 'personal-deck', category: 'optional', defaultEnabled: false, dependencies: [] }],
  deckComposition: {
    cardDefinitions: PLAYER_DECK_ELEMENT_ORDER.flatMap(element => PLAYER_DECK_LEVEL_ORDER.map(level => ({
      id: `${element.toLowerCase()}-${level}`, name: `${element} ${level}`, element, level,
      sharedDeckCopies: 4, personalDeckCopyLimit: 2, preconstructedCopies: 1,
    }))),
    sharedDeck: { exactCardCount: 10 },
    personalDeck: { exactCardCount: 2, maximumLevelTotal: 4,
      preconstructed: { name: '測試預組', cards: ['metal-1', 'wood-1'] } },
  },
}
function team(overrides: Partial<CompetitionPublicTeam> = {}): CompetitionPublicTeam {
  return { teamId: 'team-a', captain: 'captain-a', members: ['captain-a', 'member-a'], rosterVersion: 3,
    confirmedMembers: ['captain-a', 'member-a'], status: 'registered', exclusionReasons: [], ...overrides }
}
let userId = 'organizer-a'
let lifecycle: CompetitionRegistrationLifecycle = 'open'
let personalDecks: 'disabled' | 'required' = 'disabled'
let teams: CompetitionPublicTeam[] = []
let revision = 8
let ownDeckSubmitted = false
let readOverride: unknown = undefined

function registrationResponse() {
  const publicView = {
    announcement: { organizer: 'organizer-a', ruleVersion: '5.17', ruleset: 'base',
      enabledRuleModules: personalDecks === 'required' ? ['personal-deck'] : [], teamSize: 2, initialHp: 200,
      registrationMode: 'public', personalDecks, registrationDeadline, checkInDeadline, plan: announcementPlan },
    lifecycle, revision: 2, teams,
  }
  const own = teams.find(value => value.members.includes(userId))
  // 真 core PrivateTeamView 原七欄，不把公開理由 spread 進 private ownTeam。
  const ownTeam = own ? { teamId: own.teamId, captain: own.captain, members: own.members,
    rosterVersion: own.rosterVersion, confirmedMembers: own.confirmedMembers, status: own.status,
    ownDeck: ownDeckSubmitted ? { player: userId, name: '私密賽事快照', cards: ['private-card'] } : null } : null
  return { competitionId: 'competition-a', revision, projection: { ok: true, operation: 'registrationView',
    public: publicView, private: { publicView, ownTeam } } }
}
function actionType(options: unknown): unknown {
  if (!options || typeof options !== 'object' || !('body' in options)) return undefined
  const body = options.body
  if (!body || typeof body !== 'object' || !('operation' in body)) return undefined
  const operation = body.operation
  if (!operation || typeof operation !== 'object' || !('type' in operation)) return undefined
  if (operation.type === 'registrationView') return 'registrationView'
  if (operation.type !== 'registrationExecute' || !('action' in operation)) return undefined
  const action = operation.action
  return action && typeof action === 'object' && 'type' in action ? action.type : undefined
}
function mutationCalls() {
  return fetchMock.mock.calls.filter(([path, options]) => path === '/api/competitions/competition-a/commands'
    && actionType(options) !== 'registrationView')
}
function acceptMutation(options: unknown) {
  if (!options || typeof options !== 'object' || !('body' in options)) throw new Error('需要命令 body')
  const body = options.body
  if (!body || typeof body !== 'object' || !('commandId' in body) || typeof body.commandId !== 'string') {
    throw new Error('需要 commandId')
  }
  const response = registrationResponse()
  return { ...response, commandId: body.commandId, idempotent: false,
    projection: { ...response.projection, operation: 'registrationExecute' } }
}
const cleanups: (() => void)[] = []
async function mountDetail() {
  const host = document.createElement('div')
  document.body.append(host)
  const wrapper = await mountSuspended(CompetitionDetail, { route: '/competitions/competition-a', attachTo: host,
    global: { stubs: { CompetitionParticipantGameEntry: true, CompetitionTournamentView: true,
      CompetitionTournamentManagement: true, CompetitionRulingCorrection: true } } })
  cleanups.push(() => { if (wrapper.exists()) wrapper.unmount(); host.remove() })
  await flushPromises()
  return wrapper
}
function button(wrapper: Awaited<ReturnType<typeof mountDetail>>, label: string) {
  const match = wrapper.findAll('button').find(value => value.text() === label)
  if (!match) throw new Error(`缺少公開按鈕：${label}`)
  return match
}
beforeEach(() => {
  useNuxtApp().runWithContext(() => usePlayerSession().clear())
  userId = 'organizer-a'
  lifecycle = 'open'
  personalDecks = 'disabled'
  teams = []
  revision = 8
  ownDeckSubmitted = false
  readOverride = undefined
  let uuidCounter = 0
  vi.spyOn(crypto, 'randomUUID').mockImplementation(() => `10000000-0000-4000-8000-${String(++uuidCounter).padStart(12, '0')}`)
  getSessionMock.mockReset().mockImplementation(() => Promise.resolve({ data: { user: { id: userId, name: '測試使用者' } }, error: null }))
  fetchMock.mockReset().mockImplementation((path: unknown, options: unknown) => {
    if (path === '/api/rules/catalog') return Promise.resolve(catalog)
    if (path === '/api/deck') return Promise.resolve({ deck: { name: '本人帳號牌組', cards: ['metal-1', 'wood-1'] },
      source: 'custom', validation: { valid: true, cardCount: 2, levelTotal: 2, errors: [] } })
    if (path === '/api/competitions/competition-a/commands' && actionType(options) === 'registrationView') {
      return Promise.resolve(readOverride ?? registrationResponse())
    }
    return Promise.reject({ statusCode: 503, message: 'private transport detail' })
  })
})
afterEach(() => {
  for (const cleanup of cleanups.splice(0)) cleanup()
  vi.restoreAllMocks()
})

describe('registration freeze public DOM', () => {
  it('explains that F automatically freezes registration without a participant mutation', async () => {
    const wrapper = await mountDetail()
    expect(wrapper.get('.announcement-card').text()).toContain('報名截止時，名單、成員確認、必要牌組與隊長資料完成確認一併凍結')
    expect(wrapper.get('.announcement-card').text()).toContain('資料完成確認不是房間報到或準備')
    expect(wrapper.get('.organizer-actions p').text())
      .toBe('系統會在報名截止時自動鎖定名單，不必等到比賽開始時間；未完成資料的隊伍會列出排除原因。')
    expect(mutationCalls()).toHaveLength(0)
  })

  it('renders the automatic freeze from an authoritative registration read without waiting for T', async () => {
    lifecycle = 'locked'
    revision = 9
    teams = [team({ status: 'checkedIn' })]
    const wrapper = await mountDetail()
    expect(wrapper.get('.lifecycle-badge').text()).toBe('名單已鎖定')
    expect(wrapper.get('.revision-label').text()).toBe('狀態版本 9')
    expect(wrapper.get('.announcement-card').text()).toContain(formatCompetitionDeadline(checkInDeadline))
    expect(wrapper.get('.team-status').text()).toBe('資料完成確認已完成')
    expect(mutationCalls()).toHaveLength(0)
  })

  it('captain data-completion control still sends exact checkInTeam and displays the accepted current version', async () => {
    userId = 'captain-a'
    teams = [team()]
    const transport = fetchMock.getMockImplementation()
    if (!transport) throw new Error('需要正式 HTTP 邊界 fixture')
    fetchMock.mockImplementation((path: unknown, options: unknown) => {
      if (actionType(options) === 'checkInTeam') {
        teams = [team({ status: 'checkedIn' })]
        revision = 9
        return Promise.resolve(acceptMutation(options))
      }
      return transport(path, options)
    })
    const wrapper = await mountDetail()
    await button(wrapper, '隊長資料完成確認').trigger('click')
    await flushPromises()
    expect(mutationCalls()).toHaveLength(1)
    expect(mutationCalls()[0]?.[1]?.body).toEqual({ commandId: '10000000-0000-4000-8000-000000000002', expectedRevision: 8,
      operation: { type: 'registrationExecute', action: { type: 'checkInTeam', teamId: 'team-a', rosterVersion: 3 } } })
    expect(wrapper.get('.team-status').text()).toBe('資料完成確認已完成')
    expect(wrapper.get('.revision-label').text()).toBe('狀態版本 9')
    expect(wrapper.find('[aria-label="未納入賽程原因"]').exists()).toBe(false)
  })

  it('shows only the closed reasons on notCheckedIn teams, including matching own-team data without private deck display', async () => {
    userId = 'member-a'
    lifecycle = 'locked'
    personalDecks = 'required'
    ownDeckSubmitted = true
    teams = [team({ status: 'notCheckedIn', confirmedMembers: ['captain-a'], exclusionReasons: [
      'missingMemberConfirmations', 'missingRequiredDecks', 'missingCaptainConfirmation',
    ] }), team({ teamId: 'team-b', captain: 'captain-b', members: ['captain-b', 'member-b'],
      confirmedMembers: ['captain-b', 'member-b'], status: 'checkedIn' }),
    team({ teamId: 'team-c', captain: 'captain-c', members: ['captain-c', 'member-c'],
      confirmedMembers: [], status: 'withdrawn' })]
    const wrapper = await mountDetail()
    const reasons = wrapper.findAll('[aria-label="未納入賽程原因"]')
    expect(reasons).toHaveLength(1)
    expect(reasons[0]?.findAll('li').map(value => value.text())).toEqual([
      '成員確認未完成', '必要牌組提交未完成', '隊長資料完成確認未完成',
    ])
    expect(wrapper.findAll('.team-status').map(value => value.text())).toEqual(['未納入賽程', '資料完成確認已完成', '已退出'])
    expect(wrapper.get('.snapshot-badge').text()).toBe('已提交快照')
    expect(reasons[0]?.text()).not.toMatch(/captain-|member-|missing|private-card/u)
    expect(wrapper.text()).not.toMatch(/私密賽事快照|private-card/u)
    expect(mutationCalls()).toHaveLength(0)
  })

  it('rejects an unknown reason rather than displaying raw enum/private text or inferring replacement eligibility', async () => {
    const response = registrationResponse()
    readOverride = { ...response, projection: { ...response.projection, public: { ...response.projection.public,
      lifecycle: 'locked', teams: [{ ...team(), status: 'notCheckedIn', exclusionReasons: ['secret-reason'] }] } } }
    const wrapper = await mountDetail()
    expect(wrapper.get('[role="alert"] > p').text()).toBe('無法載入賽事狀態，請重試。')
    expect(wrapper.text()).not.toContain('secret-reason')
    expect(wrapper.find('.team-card').exists()).toBe(false)
    expect(mutationCalls()).toHaveLength(0)
  })

  it('uses registrationDeadline for deck submission and preserves the literal snapshot/UUID on an unknown resubmission retry', async () => {
    userId = 'member-a'
    personalDecks = 'required'
    ownDeckSubmitted = true
    teams = [team({ status: 'checkedIn' })]
    const wrapper = await mountDetail()
    expect(wrapper.findComponent(CompetitionPersonalDeckSubmission).props('registrationDeadline')).toBe(registrationDeadline)
    expect(wrapper.get('.deadline-note').text()).toBe(`牌組提交截止（報名截止）：${formatCompetitionDeadline(registrationDeadline)}。報名截止時參賽資料一併凍結；截止時間與提交資格由伺服器核實，本頁不使用本機時間代替。`)
    expect(wrapper.get('.deadline-note').text()).not.toMatch(/\d{1,2}:\d{2}:\d{2}/u)
    expect(wrapper.get('.recheckin-warning').text()).toContain('並必須由隊長再次完成資料確認。這不是房間報到或準備。')
    expect(wrapper.findAll('.deck-slot')).toHaveLength(25)
    const submit = button(wrapper, '提交目前帳號牌組作為賽事快照')
    expect(submit.attributes('disabled')).toBeDefined()
    await wrapper.get('.acknowledgement input').setValue(true)
    expect(submit.attributes('disabled')).toBeUndefined()
    await submit.trigger('click')
    await flushPromises()
    const originalBody: unknown = mutationCalls()[0]?.[1]?.body
    expect(originalBody).toEqual({ commandId: '10000000-0000-4000-8000-000000000002', expectedRevision: 8,
      operation: { type: 'registrationExecute', action: { type: 'submitDeck', teamId: 'team-a', rosterVersion: 3,
        name: '本人帳號牌組', cards: ['metal-1', 'wood-1'] } } })
    const uuidCount = vi.mocked(crypto.randomUUID).mock.calls.length
    expect(wrapper.get('.submit-button').attributes('disabled')).toBeDefined()
    await button(wrapper, '以相同命令重試').trigger('click')
    await flushPromises()
    expect(mutationCalls()).toHaveLength(2)
    expect(mutationCalls()[1]?.[1]?.body).toEqual(originalBody)
    expect(vi.mocked(crypto.randomUUID).mock.calls.length).toBe(uuidCount)
    expect(wrapper.text()).not.toContain('private transport detail')
  })
})
