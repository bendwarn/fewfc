import { mountSuspended } from '@nuxt/test-utils/runtime'
import { mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import { ofetch } from 'ofetch'
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
  window.sessionStorage.clear()
})

const nextGameTeams = ['team-a', 'team-b'] as const
const nextGameProps = {
  competitionId: 'competition-next-game', matchId: 'match-next-game', sessionUserId: 'captain-b',
  participantTeamId: nextGameTeams[1], participantIsCaptain: true, enabled: true,
}

function nextGameParticipant(game: unknown = null) {
  return {
    competitionId: nextGameProps.competitionId,
    matchId: nextGameProps.matchId,
    matchRevision: game ? 8 : 7,
    game,
  }
}

function nextGameMatch(assigned = false) {
  const ruling = { type: 'gameAccepted', outcome: { winner: nextGameTeams[0] } }
  const firstPlayer = assigned
    ? { type: 'assigned', team: nextGameTeams[0] }
    : { type: 'pending', reason: '敗方隊長選擇' }
  return {
    competitionId: nextGameProps.competitionId,
    matchId: nextGameProps.matchId,
    matchRevision: assigned ? 8 : 7,
    matchProjection: {
      ok: true,
      operation: 'matchView',
      public: { match: {
        config: {
          matchId: nextGameProps.matchId,
          teams: nextGameTeams,
          format: { type: 'fixedGames', value: 3 },
          firstGame: { type: 'assigned', team: nextGameTeams[0] },
          referees: [],
        },
        status: assigned ? 'open' : 'awaitingFirstPlayer',
        score: [1, 0],
        games: [{
          gameNumber: 1,
          gameRecordId: 'game-record-1',
          startKind: { type: 'ordinaryNewGame' },
          firstPlayer: nextGameTeams[0],
          terminalEvidence: { type: 'canonicalGameEnded', conclusion: {
            outcome: { type: 'winner', team: nextGameTeams[0] },
            causes: [{ type: 'teamHpDepleted', teams: [nextGameTeams[1]] }],
          } },
          rulingRecordId: 'ruling-1',
        }],
        rulings: [{ rulingRecordId: 'ruling-1', gameRecordId: 'game-record-1', ruling }],
        corrections: [],
        outcome: null,
        nextFirstPlayer: assigned ? nextGameTeams[0] : null,
        nextGame: {
          gameNumber: 2,
          kind: { type: 'ordinaryNewGame' },
          firstPlayer,
        },
      } },
    },
    clockProjection: null,
  }
}

function nextGameAcknowledgement(command: { commandId: string; expectedMatchRevision: number }) {
  return {
    competitionId: nextGameProps.competitionId,
    matchId: nextGameProps.matchId,
    matchRevision: command.expectedMatchRevision + 1,
    gameNumber: 2,
    gameId: 'next-game-room',
    commandId: command.commandId,
    status: 'provisioned',
  }
}

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

  expect(wrapper.get('.state-note').text()).toBe('房間尚未就緒，賽事頁會自動重新查詢；也可以立即重試。 立即重試')
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

it.each(['gameRoomNotProvisioned', 'trustedTimeRegression'])(
  'retains and resumes the same captain command after the real H3 %s response and a component remount',
  async (code) => {
    window.sessionStorage.clear()
    fetchMock.mockResolvedValueOnce(nextGameParticipant())
    fetchMock.mockResolvedValueOnce(nextGameMatch())
    const first = await mountSuspended(CompetitionParticipantGameEntry, { props: nextGameProps })
    try {
      await flushPromises()
      await first.get('input[type=radio]').setValue()

      const h3Error = JSON.stringify({
        statusCode: 409,
        statusMessage: code === 'trustedTimeRegression' ? '賽事時間狀態無法倒退。' : '對局尚未就緒，計時無法操作。',
        stack: [],
        data: { code },
      })
      const actualOfetch = ofetch.create({}, {
        fetch: async () => new Response(h3Error, {
          status: 409,
          headers: { 'content-type': 'application/json' },
        }),
      })
      const parsedFetchError = await actualOfetch('/api/competitions/next-game', {
        method: 'POST', retry: 0,
      }).then(() => { throw new Error('expected a conflict response') }, error => error)
      expect(parsedFetchError).toMatchObject({ status: 409, data: { data: { code } } })
      const storageKey = 'fewfc:competition-next-game:' + JSON.stringify([
        nextGameProps.competitionId, nextGameProps.matchId, nextGameProps.sessionUserId,
      ])
      fetchMock.mockImplementationOnce((_url, options) => {
        expect(JSON.parse(window.sessionStorage.getItem(storageKey) ?? 'null')).toEqual(options.body)
        return Promise.reject(parsedFetchError)
      })

      await first.get('.first-player-form').trigger('submit')
      await flushPromises()
      const originalCommand = JSON.parse(window.sessionStorage.getItem(storageKey) ?? 'null')
      expect(first.find('.pending-command button').exists()).toBe(true)
      expect(originalCommand).toMatchObject({
        selectedFirstPlayerTeam: nextGameTeams[0],
        expectedMatchRevision: 7,
      })
    } finally {
      await first.unmount()
    }

    fetchMock.mockResolvedValueOnce(nextGameParticipant())
    fetchMock.mockResolvedValueOnce(nextGameMatch(true))
    const restored = await mountSuspended(CompetitionParticipantGameEntry, { props: nextGameProps })
    try {
      await flushPromises()
      expect(restored.find('.first-player-form').exists()).toBe(false)
      expect(restored.find('.pending-command button').exists()).toBe(true)
      const originalCommand = JSON.parse(window.sessionStorage.getItem(
        'fewfc:competition-next-game:' + JSON.stringify([
          nextGameProps.competitionId, nextGameProps.matchId, nextGameProps.sessionUserId,
        ]),
      ) ?? 'null')
      fetchMock.mockImplementationOnce((_url, options) => Promise.resolve(nextGameAcknowledgement(options.body)))
      fetchMock.mockResolvedValueOnce(nextGameParticipant({ gameId: 'next-game-room', gameNumber: 2 }))
      await restored.get('.pending-command button').trigger('click')
      await flushPromises()

      const posts = fetchMock.mock.calls.filter(([_url, options]) => options?.method === 'POST')
      expect(posts).toHaveLength(2)
      expect(posts[0]?.[1]?.body).toEqual(originalCommand)
      expect(posts[1]?.[1]?.body).toEqual(originalCommand)
      expect(restored.get('a.room-link').attributes('href')).toBe('/rooms/next-game-room')
      expect(window.sessionStorage.getItem('fewfc:competition-next-game:' + JSON.stringify([
        nextGameProps.competitionId, nextGameProps.matchId, nextGameProps.sessionUserId,
      ]))).toBeNull()
    } finally {
      await restored.unmount()
    }
  },
)

it('keeps a partially submitted command across a later authentication rejection and same-account remount', async () => {
  const h3FetchError = async (statusCode: number, statusMessage: string, code?: string) => {
    const body = JSON.stringify({
      statusCode,
      statusMessage,
      stack: [],
      ...(code ? { data: { code } } : {}),
    })
    const fetcher = ofetch.create({}, {
      fetch: async () => new Response(body, {
        status: statusCode,
        headers: { 'content-type': 'application/json' },
      }),
    })
    return fetcher('/api/competitions/next-game', { method: 'POST', retry: 0 })
      .then(() => { throw new Error('expected an H3 error') }, error => error)
  }
  const secondStepFailure = await h3FetchError(502, '賽事服務暫時無法處理要求。', 'engineProtocolError')
  const authenticationFailure = await h3FetchError(401, 'Authentication required.')
  expect(secondStepFailure).toMatchObject({ status: 502 })
  expect(authenticationFailure).toMatchObject({ status: 401 })

  fetchMock.mockResolvedValueOnce(nextGameParticipant())
  fetchMock.mockResolvedValueOnce(nextGameMatch())
  const storageKey = 'fewfc:competition-next-game:' + JSON.stringify([
    nextGameProps.competitionId, nextGameProps.matchId, nextGameProps.sessionUserId,
  ])
  const first = await mountSuspended(CompetitionParticipantGameEntry, { props: nextGameProps })
  let originalCommand: unknown
  try {
    await flushPromises()
    await first.get('input[type=radio]').setValue()

    fetchMock.mockRejectedValueOnce(secondStepFailure)
    await first.get('.first-player-form').trigger('submit')
    await flushPromises()
    originalCommand = JSON.parse(window.sessionStorage.getItem(storageKey) ?? 'null')
    expect(first.find('.pending-command button').exists()).toBe(true)
    expect(originalCommand).toMatchObject({
      selectedFirstPlayerTeam: nextGameTeams[0],
      expectedMatchRevision: 7,
    })

    // 這次 401 尚未進入 DO，不能清除前次 502 留下的未知命令。
    fetchMock.mockRejectedValueOnce(authenticationFailure)
    await first.get('.pending-command button').trigger('click')
    await flushPromises()
    expect(first.find('.pending-command button').exists()).toBe(true)
    expect(JSON.parse(window.sessionStorage.getItem(storageKey) ?? 'null')).toEqual(originalCommand)
  } finally {
    await first.unmount()
  }

  fetchMock.mockResolvedValueOnce(nextGameParticipant())
  fetchMock.mockResolvedValueOnce(nextGameMatch(true))
  const restored = await mountSuspended(CompetitionParticipantGameEntry, { props: nextGameProps })
  try {
    await flushPromises()
    expect(restored.find('.first-player-form').exists()).toBe(false)
    expect(restored.find('.pending-command button').exists()).toBe(true)

    fetchMock.mockImplementationOnce((_url, options) => Promise.resolve(nextGameAcknowledgement(options.body)))
    fetchMock.mockResolvedValueOnce(nextGameParticipant({ gameId: 'next-game-room', gameNumber: 2 }))
    await restored.get('.pending-command button').trigger('click')
    await flushPromises()

    const posts = fetchMock.mock.calls.filter(([_url, options]) => options?.method === 'POST')
    expect(posts).toHaveLength(3)
    expect(posts.map(([, options]) => options?.body)).toEqual([
      originalCommand, originalCommand, originalCommand,
    ])
    expect(restored.get('a.room-link').attributes('href')).toBe('/rooms/next-game-room')
    expect(window.sessionStorage.getItem(storageKey)).toBeNull()
  } finally {
    await restored.unmount()
  }
})

it('clears a first captain command rejected by authentication before any outcome becomes unknown', async () => {
  fetchMock.mockResolvedValueOnce(nextGameParticipant())
  fetchMock.mockResolvedValueOnce(nextGameMatch())
  const storageKey = 'fewfc:competition-next-game:' + JSON.stringify([
    nextGameProps.competitionId, nextGameProps.matchId, nextGameProps.sessionUserId,
  ])
  const unauthorizedFetch = ofetch.create({}, {
    fetch: async () => new Response(JSON.stringify({
      statusCode: 401,
      statusMessage: 'Authentication required.',
      stack: [],
    }), {
      status: 401,
      headers: { 'content-type': 'application/json' },
    }),
  })
  const unauthorized = await unauthorizedFetch('/api/competitions/next-game', {
    method: 'POST', retry: 0,
  }).then(() => { throw new Error('expected an H3 authentication error') }, error => error)
  expect(unauthorized).toMatchObject({ status: 401 })

  const wrapper = await mountSuspended(CompetitionParticipantGameEntry, { props: nextGameProps })
  try {
    await flushPromises()
    await wrapper.get('input[type=radio]').setValue()
    fetchMock.mockRejectedValueOnce(unauthorized)
    await wrapper.get('.first-player-form').trigger('submit')
    await flushPromises()

    expect(wrapper.find('.pending-command button').exists()).toBe(false)
    expect(window.sessionStorage.getItem(storageKey)).toBeNull()
  } finally {
    await wrapper.unmount()
  }
})
