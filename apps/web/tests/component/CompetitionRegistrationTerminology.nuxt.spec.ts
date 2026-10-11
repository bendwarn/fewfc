import { mountSuspended, mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { beforeEach, expect, it, vi } from 'vitest'
import CompetitionIndex from '~/pages/competitions/index.vue'
import CompetitionDetail from '~/pages/competitions/[id].vue'
import { formatCompetitionDeadline, formatCompetitionRegistrationDeadline } from '~/lib/competition-registration'
import { PLAYER_DECK_ELEMENT_ORDER, PLAYER_DECK_LEVEL_ORDER } from '~/lib/player-deck'
import type { RulesCatalog } from '~/types/fewfc'
import { createCompetitionAnnouncementFixture } from '../fixtures/competition-announcement'

const { fetchMock, getSessionMock } = vi.hoisted(() => ({
  fetchMock: vi.fn(),
  getSessionMock: vi.fn(),
}))
mockNuxtImport('$fetch', () => fetchMock)
// 只 stub 外部登入／HTTP 邊界；頁面、期限 helper、規則目錄與 session composable 都是真實程式。
vi.mock('~/lib/auth-client', () => ({ authClient: { getSession: getSessionMock } }))

const catalog: RulesCatalog = {
  version: 1,
  ruleModules: [
    { id: 'star', category: 'advanced', defaultEnabled: true, dependencies: [] },
    { id: 'personal-deck', category: 'optional', defaultEnabled: false, dependencies: [] },
  ],
  deckComposition: {
    cardDefinitions: PLAYER_DECK_ELEMENT_ORDER.flatMap(element => PLAYER_DECK_LEVEL_ORDER.map(level => ({
      id: `${element.toLowerCase()}-${level}`, name: `${element} ${level}`, element, level,
      sharedDeckCopies: 4, personalDeckCopyLimit: 2, preconstructedCopies: 1,
    }))),
    sharedDeck: { exactCardCount: 10 },
    personalDeck: {
      exactCardCount: 2, maximumLevelTotal: 4,
      preconstructed: { name: '測試預組', cards: ['metal-1', 'wood-1'] },
    },
  },
}

const registrationDeadline = new Date(2026, 8, 30, 13, 14).getTime()
const checkInDeadline = new Date(2026, 8, 30, 14, 0).getTime()
const announcement = createCompetitionAnnouncementFixture({
  organizer: 'organizer-a',
  initialHp: 137,
  enabledRuleModules: ['star'],
  teamSize: 1,
  registrationDeadline,
  checkInDeadline,
})
let announcementTeamSize = 1
let announcementLifecycle: 'draft' | 'open' = 'draft'

function registrationResponse() {
  return {
    competitionId: 'competition-a', revision: 8,
    projection: {
      ok: true, operation: 'registrationView',
      public: {
        announcement: {
          ...structuredClone(announcement),
          teamSize: announcementTeamSize,
        },
        lifecycle: announcementLifecycle, revision: 2, teams: [],
      },
    },
  }
}

function createCalls() {
  return fetchMock.mock.calls.filter(([path]) => path === '/api/competitions')
}

beforeEach(() => {
  announcementTeamSize = 1
  announcementLifecycle = 'draft'
  getSessionMock.mockReset().mockResolvedValue({ data: { user: { id: 'organizer-a', name: '測試主辦' } }, error: null })
  fetchMock.mockReset().mockImplementation((path: unknown) => {
    if (path === '/api/rules/catalog') return Promise.resolve(catalog)
    if (path === '/api/competitions/competition-a/commands') return Promise.resolve(registrationResponse())
    if (path === '/api/competitions') return Promise.reject({ statusCode: 503 })
    return Promise.reject(new Error('unexpected test transport path'))
  })
})

it('offers individual/team terminology and minute-precision announcement fields', async () => {
  const wrapper = await mountSuspended(CompetitionIndex)
  await flushPromises()

  expect(wrapper.get('label[for="team-size"]').text()).toContain('參賽型態')
  expect(wrapper.findAll('#team-size option').map(option => [option.attributes('value'), option.text()]))
    .toEqual([['1', '個人'], ['2', '團體（每隊 2 人）']])
  expect(wrapper.get('#registration-deadline').attributes()).toMatchObject({ type: 'datetime-local', step: '60' })
  expect(wrapper.get('#registration-deadline').attributes('aria-describedby')).toBe('registration-deadline-help')
  expect(wrapper.get('#registration-deadline-help').text()).toBe('報名截止以分鐘為單位，不含秒或毫秒。')
  expect(wrapper.get('label[for="check-in-deadline"]').text()).toContain('比賽開始時間')
  expect(wrapper.get('label[for="check-in-deadline"]').text()).not.toContain('報到期限')
  expect(wrapper.get('#check-in-deadline').attributes()).toMatchObject({ type: 'datetime-local', step: '60' })
  expect(wrapper.get('#check-in-deadline').attributes('aria-describedby')).toBe('check-in-deadline-help')
  expect(wrapper.get('#check-in-deadline-help').text()).toBe('比賽開始時間以分鐘為單位，不含秒或毫秒。')
  expect(wrapper.get<HTMLInputElement>('#stage-match-count-0').element.value).toBe('')
  expect(wrapper.get<HTMLInputElement>('#stage-main-time-0').element.value).toBe('5')
  expect(wrapper.get<HTMLSelectElement>('#stage-byoyomi-0').element.value).toBe('30')
  expect(createCalls()).toHaveLength(0)
  await wrapper.unmount()
})

it.each([1, 2])('serializes the selected teamSize %i with exact minute registration and start-time wire', async (teamSize) => {
  const wrapper = await mountSuspended(CompetitionIndex)
  await flushPromises()
  await wrapper.get('#team-size').setValue(String(teamSize))
  await wrapper.get('#registration-deadline').setValue('2026-09-30T13:14')
  await wrapper.get('#check-in-deadline').setValue('2026-09-30T14:00')
  await wrapper.get('#stage-format-0').setValue('roundRobin')
  await wrapper.get('#stage-match-count-0').setValue('1')
  await wrapper.get('form.announcement-form').trigger('submit')
  await flushPromises()

  const body: unknown = createCalls()[0]?.[1]?.body
  expect(body).toMatchObject({
    ruleVersion: '5.17', ruleset: 'base', enabledRuleModules: ['star'], teamSize,
    registrationMode: 'public', personalDecks: 'disabled',
    registrationDeadline: new Date(2026, 8, 30, 13, 14).getTime(),
    checkInDeadline: new Date(2026, 8, 30, 14, 0).getTime(),
    plan: {
      initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
      swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
      rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
      exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
      stages: [{ stageId: expect.stringMatching(/^stage-/u), format: 'roundRobin',
        matchFormat: { type: 'fixedGames', value: 1 }, swissRounds: null, swissPolicy: null,
        qualification: { type: 'allEligibleTeams' } }],
    },
  })
  expect(wrapper.get('[role="alert"]').text()).toContain('建立結果無法確認')
  // 名稱／精度修正不能破壞現有未知 create 結果的單次送出保護。
  await wrapper.get('form.announcement-form').trigger('submit')
  await flushPromises()
  expect(createCalls()).toHaveLength(1)
  await wrapper.unmount()
})

it.each([
  { field: '#registration-deadline', value: '2026-09-30T13:14:15', expectedInputValue: '2026-09-30T13:14:15', message: '報名截止時間請以分鐘為單位輸入，不含秒或毫秒。' },
  { field: '#registration-deadline', value: '2026-09-30T13:14:15.006', expectedInputValue: '2026-09-30T13:14:15.006', message: '報名截止時間請以分鐘為單位輸入，不含秒或毫秒。' },
  { field: '#check-in-deadline', value: '2026-09-30T14:00:15', expectedInputValue: '2026-09-30T14:00:15', message: '比賽開始時間請以分鐘為單位輸入，不含秒或毫秒。' },
  { field: '#check-in-deadline', value: '2026-09-30T14:00:15.006', expectedInputValue: '2026-09-30T14:00:15.006', message: '比賽開始時間請以分鐘為單位輸入，不含秒或毫秒。' },
  { field: '#registration-deadline', value: '2026-02-30T12:00', expectedInputValue: '', message: '時間' },
])('refuses sub-minute or invalid date input in $field without dispatch', async ({ field, value, expectedInputValue, message }) => {
  const wrapper = await mountSuspended(CompetitionIndex)
  await flushPromises()
  const input = wrapper.get<HTMLInputElement>(field)
  await input.setValue(value)
  expect(input.element.value).toBe(expectedInputValue)
  await wrapper.get(field === '#registration-deadline' ? '#check-in-deadline' : '#registration-deadline')
    .setValue(field === '#registration-deadline' ? '2026-09-30T14:00' : '2026-09-30T13:14')
  await wrapper.get('form.announcement-form').trigger('submit')
  await flushPromises()

  expect(wrapper.get('[role="alert"]').text()).toContain(message)
  if (field === '#check-in-deadline') {
    expect(wrapper.get('[role="alert"]').text()).not.toContain('報名截止時間')
  }
  expect(createCalls()).toHaveLength(0)
  await wrapper.unmount()
})

it('shows competition start terminology for reversed deadlines without dispatching a create request', async () => {
  const wrapper = await mountSuspended(CompetitionIndex)
  await flushPromises()
  await wrapper.get('#registration-deadline').setValue('2026-09-30T15:00')
  await wrapper.get('#check-in-deadline').setValue('2026-09-30T14:00')
  await wrapper.get('form.announcement-form').trigger('submit')
  await flushPromises()

  expect(wrapper.get('[role="alert"]').text()).toBe('報名截止時間不可晚於比賽開始時間。')
  expect(createCalls()).toHaveLength(0)
  await wrapper.unmount()
})

it.each([
  { teamSize: 1, label: '個人' },
  { teamSize: 2, label: '團體（每隊 2 人）' },
])('renders $label in the announcement and displays only registration to the minute', async ({ teamSize, label }) => {
  announcementTeamSize = teamSize
  const wrapper = await mountSuspended(CompetitionDetail, {
    route: '/competitions/competition-a',
    // 已過子切片不屬於本次 labels seam；不在這裡重測或呼叫它們的 GET。
    global: { stubs: { CompetitionPersonalDeckSubmission: true, CompetitionParticipantGameEntry: true, CompetitionTournamentView: true } },
  })
  await flushPromises()

  const facts = wrapper.findAll('.announcement-facts > div')
  const fact = (heading: string) => facts.find(item => item.get('dt').text() === heading)?.get('dd').text()
  expect(fact('參賽型態')).toBe(label)
  expect(fact('報名截止')).toBe(formatCompetitionRegistrationDeadline(registrationDeadline))
  expect(fact('報名截止')).not.toContain('006')
  expect(fact('比賽開始時間')).toBe(formatCompetitionDeadline(checkInDeadline))
  expect(fact('比賽開始時間')).not.toContain('006')
  expect(fact('比賽開始時間')).not.toMatch(/\d{1,2}:\d{2}:\d{2}/u)
  expect(fact('每隊初始生命值')).toBe('137')
  expect(wrapper.get('.announcement-plan-card').text()).toContain('自身交手積分較高優先')
  expect(wrapper.findAll('.ranking-score-policy li').map(item => item.text())).toEqual([
    '勝分：本階段每位不同對手在本階段的最終賽事交手勝場總和。',
    '負分：同一組對手在本階段的最終賽事交手敗場總和。',
    '每位對手只計一次，無論本隊對該隊的結果是勝、和或負；對手戰績包含其與本隊的那次交手，不跨階段累計。',
    '未真正開打的缺席配對不納入對手集合；合法缺席判定保留為該隊自身的賽事交手結果，但不會因此納入本項的對手集合。和局不增加勝敗場，雙方判負各增加一敗；輪空與取消不增加勝敗場。',
    '不以單局勝敗、勝率、生命、主時間或耗時代替交手勝敗，也不把勝分與負分相減成淨分。',
  ])
  expect(wrapper.get('.announcement-plan-card').text()).toContain('第 1 階段：循環賽')
  expect(wrapper.find('.organizer-actions').text()).toContain('開放報名')
  await wrapper.unmount()
})

it('registration freeze describes automatic locking without claiming room ACK or presence', async () => {
  announcementLifecycle = 'open'
  const wrapper = await mountSuspended(CompetitionDetail, {
    route: '/competitions/competition-a',
    global: { stubs: { CompetitionPersonalDeckSubmission: true, CompetitionParticipantGameEntry: true, CompetitionTournamentView: true } },
  })
  await flushPromises()

  expect(wrapper.get('.organizer-actions p').text())
    .toBe('系統會在報名截止時自動鎖定名單，不必等到比賽開始時間；未完成資料的隊伍會列出排除原因。')
  expect(createCalls()).toHaveLength(0)
  await wrapper.unmount()
})
