import { mountSuspended } from '@nuxt/test-utils/runtime'
import { mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import CompetitionParticipantGameEntry from '~/components/CompetitionParticipantGameEntry.vue'

const fetchMock = vi.hoisted(() => vi.fn())
mockNuxtImport('$fetch', () => fetchMock)

function abortSignalFromCall(index: number): AbortSignal {
  const signal: unknown = fetchMock.mock.calls[index]?.[1]?.signal
  if (!(signal instanceof AbortSignal)) throw new Error('expected participant GET AbortSignal')
  return signal
}

beforeEach(() => {
  fetchMock.mockReset()
})

afterEach(() => {
  vi.restoreAllMocks()
})

it('queries the exact participant path only after explicit ID submission and links only a valid current game', async () => {
  fetchMock.mockResolvedValue({
    competitionId: 'competition a',
    matchId: 'match/one',
    matchRevision: 3,
    game: { gameId: 'room/one', gameNumber: 1 },
  })
  const wrapper = await mountSuspended(CompetitionParticipantGameEntry, {
    props: {
      competitionId: ' competition a ',
      matchId: '',
      sessionUserId: 'member-a',
      enabled: true,
      editable: true,
    },
  })

  expect(fetchMock).not.toHaveBeenCalled()
  await wrapper.get('input').setValue('match/one')
  await wrapper.setProps({ matchId: 'match/one' })
  await wrapper.get('form').trigger('submit')
  await flushPromises()

  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(fetchMock.mock.calls[0]?.[0]).toBe('/api/competitions/competition%20a/matches/match%2Fone/participant-game')
  expect(fetchMock.mock.calls[0]?.[1]).toMatchObject({ method: 'GET' })
  expect(Object.keys(fetchMock.mock.calls[0]?.[1] ?? {}).sort()).toEqual(['method', 'signal'])
  expect(wrapper.get('a.room-link').attributes('href')).toBe('/rooms/room%2Fone')
  expect(wrapper.text()).not.toContain('member-a')
  await wrapper.unmount()
})

it('clears and ignores an in-flight result when the match or signed-in identity changes', async () => {
  let resolveFirst!: (value: unknown) => void
  const firstResponse = new Promise<unknown>((resolve) => { resolveFirst = resolve })
  fetchMock.mockReturnValueOnce(firstResponse)
  const wrapper = await mountSuspended(CompetitionParticipantGameEntry, {
    props: {
      competitionId: 'competition-a',
      matchId: 'match-a',
      sessionUserId: 'member-a',
      enabled: true,
      editable: true,
    },
  })

  await wrapper.get('form').trigger('submit')
  const firstSignal = abortSignalFromCall(0)
  await wrapper.setProps({ matchId: 'match-b' })
  expect(firstSignal.aborted).toBe(true)
  resolveFirst({
    competitionId: 'competition-a',
    matchId: 'match-a',
    matchRevision: 1,
    game: { gameId: 'stale-room', gameNumber: 1 },
  })
  await flushPromises()
  expect(wrapper.find('a.room-link').exists()).toBe(false)
  expect(wrapper.text()).not.toContain('stale-room')

  let resolveSecond!: (value: unknown) => void
  const secondResponse = new Promise<unknown>((resolve) => { resolveSecond = resolve })
  fetchMock.mockReturnValueOnce(secondResponse)
  await wrapper.get('form').trigger('submit')
  const secondSignal = abortSignalFromCall(1)
  await wrapper.setProps({ competitionId: 'competition-b' })
  expect(secondSignal.aborted).toBe(true)
  resolveSecond({
    competitionId: 'competition-a',
    matchId: 'match-b',
    matchRevision: 2,
    game: { gameId: 'other-account-room', gameNumber: 1 },
  })
  await flushPromises()
  expect(wrapper.find('a.room-link').exists()).toBe(false)

  let resolveThird!: (value: unknown) => void
  const thirdResponse = new Promise<unknown>((resolve) => { resolveThird = resolve })
  fetchMock.mockReturnValueOnce(thirdResponse)
  await wrapper.get('form').trigger('submit')
  const thirdSignal = abortSignalFromCall(2)
  await wrapper.setProps({ sessionUserId: 'member-b', enabled: false })
  expect(thirdSignal.aborted).toBe(true)
  resolveThird({
    competitionId: 'competition-b',
    matchId: 'match-b',
    matchRevision: 3,
    game: { gameId: 'other-account-room', gameNumber: 1 },
  })
  await flushPromises()
  expect(wrapper.find('a.room-link').exists()).toBe(false)
  expect(wrapper.text()).not.toContain('other-account-room')
  await wrapper.unmount()
})

it('explains a successful null result without inferring whether the match is pending or over', async () => {
  fetchMock.mockResolvedValue({
    competitionId: 'competition-a',
    matchId: 'match-a',
    matchRevision: 5,
    game: null,
  })
  const wrapper = await mountSuspended(CompetitionParticipantGameEntry, {
    props: {
      competitionId: 'competition-a',
      matchId: 'match-a',
      sessionUserId: 'member-a',
      enabled: true,
    },
  })
  await flushPromises()

  expect(wrapper.get('.state-note').text()).toBe('目前沒有可加入當局，請稍後重新查詢或向主辦方確認。')
  expect(wrapper.find('a.room-link').exists()).toBe(false)
  await wrapper.unmount()
})

it('renders only fixed safe errors and keeps a disabled identity from triggering a request', async () => {
  fetchMock.mockRejectedValue({
    statusCode: 403,
    message: 'secret-rpc-detail',
    data: { private: 'secret-rpc-detail' },
  })
  const wrapper = await mountSuspended(CompetitionParticipantGameEntry, {
    props: {
      competitionId: 'competition-a',
      matchId: 'match-a',
      sessionUserId: null,
      enabled: false,
      editable: true,
    },
  })

  expect(fetchMock).not.toHaveBeenCalled()
  expect(wrapper.get('input').attributes('disabled')).toBeDefined()
  await wrapper.setProps({ sessionUserId: 'member-a', enabled: true })
  await wrapper.get('form').trigger('submit')
  await flushPromises()
  expect(wrapper.text()).toContain('你沒有這場競賽對局的存取權。')
  expect(wrapper.text()).not.toContain('secret-rpc-detail')
  expect(wrapper.find('a.room-link').exists()).toBe(false)
  await wrapper.get('.load-error button').trigger('click')
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(fetchMock.mock.calls.map(([url, options]) => [url, options?.method])).toEqual([
    ['/api/competitions/competition-a/matches/match-a/participant-game', 'GET'],
    ['/api/competitions/competition-a/matches/match-a/participant-game', 'GET'],
  ])
  await wrapper.unmount()
})

it('never creates a room link from a response containing an unknown field', async () => {
  fetchMock.mockResolvedValue({
    competitionId: 'competition-a',
    matchId: 'match-a',
    matchRevision: 1,
    game: { gameId: 'forged-room', gameNumber: 1 },
    privateHistory: ['must-not-render'],
  })
  const wrapper = await mountSuspended(CompetitionParticipantGameEntry, {
    props: {
      competitionId: 'competition-a',
      matchId: 'match-a',
      sessionUserId: 'member-a',
      enabled: true,
    },
  })
  await flushPromises()

  expect(wrapper.text()).toContain('競賽房間回應格式不正確')
  expect(wrapper.text()).not.toContain('must-not-render')
  expect(wrapper.find('a.room-link').exists()).toBe(false)
  await wrapper.unmount()
})
