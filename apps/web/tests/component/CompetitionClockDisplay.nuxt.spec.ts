import { mountSuspended } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import CompetitionClockDisplay from '~/components/CompetitionClockDisplay.vue'
import type { CompetitionRoomMode } from '~/lib/competition-ready'
import type { CompetitionRoomMatchClockReadValue } from '../../shared/competition-room'

const fetchMock = vi.hoisted(() => vi.fn())

// 正常報名產生的機器識別碼不是玩家輸入的隊名，呈現測試必須使用中文映射。
const teamIds: [string, string] = [
  '2d77ff86-2e89-4ea3-a99a-0a938c104d0b',
  'c8ee83ba-4138-47be-9384-f9ad24ac6bd2',
]

interface ClockProps {
  readClock: (options: { signal: AbortSignal }) => Promise<unknown>
  mode: CompetitionRoomMode
  gameId: string
  roomGameId: string | null
  sessionUserId: string | null
  memberUserId: string | null
}

function props(): ClockProps {
  const mode: CompetitionRoomMode = {
    kind: 'competition', context: { competitionId: 'competition a', matchId: 'match/one', gameNumber: 2 },
  }
  return { readClock: fetchMock, mode, gameId: 'room/one', roomGameId: 'room/one', sessionUserId: 'member-a', memberUserId: 'member-a' }
}

function response(): CompetitionRoomMatchClockReadValue {
  return {
    competitionId: 'competition a', matchId: 'match/one', matchRevision: 4, matchStatus: 'open',
    game: { gameId: 'room/one', gameNumber: 2 },
    clock: {
      sampledAtMs: 4000,
      teams: [
        { team: teamIds[0], mainTimeRemainingMs: 299001, byoyomiTimeRemainingMs: 30000, byoyomiActive: false },
        { team: teamIds[1], mainTimeRemainingMs: 300000, byoyomiTimeRemainingMs: null, byoyomiActive: false },
      ],
      running: { type: 'action', team: teamIds[0], turn: 0 }, paused: null, awaitingAuthority: false,
    },
  }
}

function deferred() {
  let resolve!: (value: unknown) => void
  let reject!: (error: unknown) => void
  const promise = new Promise<unknown>((accept, refuse) => { resolve = accept; reject = refuse })
  return { promise, resolve, reject }
}

function signal(index: number): AbortSignal {
  const value: unknown = fetchMock.mock.calls[index]?.[0]?.signal
  if (!(value instanceof AbortSignal)) throw new Error('時計查詢應帶 AbortSignal')
  return value
}

let visibility: DocumentVisibilityState = 'visible'
const cleanups: (() => void)[] = []

async function mountClock(overrides: Partial<ReturnType<typeof props>> = {}) {
  const wrapper = await mountSuspended(CompetitionClockDisplay, { props: { ...props(), ...overrides } })
  cleanups.push(() => { if (wrapper.exists()) wrapper.unmount() })
  await flushPromises()
  return wrapper
}

function expectChineseTeamLabels(wrapper: Awaited<ReturnType<typeof mountClock>>) {
  expect(wrapper.findAll('.clock-team h3').map(heading => heading.text())).toEqual(['甲隊', '乙隊'])
  for (const teamId of teamIds) expect(wrapper.text()).not.toContain(teamId)
}

function setVisibility(value: DocumentVisibilityState) {
  visibility = value
  document.dispatchEvent(new Event('visibilitychange'))
}

beforeEach(() => {
  fetchMock.mockReset()
  visibility = 'visible'
  vi.useFakeTimers({ toFake: ['setTimeout', 'clearTimeout'] })
  vi.spyOn(document, 'visibilityState', 'get').mockImplementation(() => visibility)
})

afterEach(() => {
  for (const cleanup of cleanups.splice(0)) cleanup()
  vi.clearAllTimers()
  vi.useRealTimers()
  vi.restoreAllMocks()
})

it('visible 固定成員以房間 WS 單飛取樣，完成一秒後才再次取樣，不在 client 扣時', async () => {
  const first = deferred()
  const second = deferred()
  fetchMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
  const wrapper = await mountClock()
  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(fetchMock.mock.calls[0]?.[0]).toEqual({ signal: signal(0) })
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  await vi.advanceTimersByTimeAsync(5000)
  expect(fetchMock).toHaveBeenCalledTimes(1)
  first.resolve(response())
  await flushPromises()
  expect(wrapper.text()).toContain('299.001 秒')
  expect(wrapper.text()).toContain('回合編號 0')
  expect(wrapper.get('.clock-phase').text()).toBe('行動計時：甲隊，回合編號 0')
  expectChineseTeamLabels(wrapper)
  await vi.advanceTimersByTimeAsync(999)
  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(wrapper.text()).toContain('299.001 秒')
  await vi.advanceTimersByTimeAsync(1)
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  expect(wrapper.text()).toContain('數字停留在上次權威取樣')
  await vi.advanceTimersByTimeAsync(5000)
  expect(fetchMock).toHaveBeenCalledTimes(2)
  const next = response()
  next.clock.teams[0]!.mainTimeRemainingMs = 280002
  second.resolve(next)
  await flushPromises()
  expect(wrapper.text()).toContain('280.002 秒')
  expect(wrapper.text()).not.toContain('299.001 秒')
})

it('hidden 初掛不查詢，重見立即查；隱藏會 abort 並拒绝晚回應，無背景 polling', async () => {
  visibility = 'hidden'
  const first = deferred()
  const second = deferred()
  fetchMock.mockReturnValueOnce(first.promise).mockReturnValueOnce(second.promise)
  const wrapper = await mountClock()
  expect(fetchMock).not.toHaveBeenCalled()
  setVisibility('visible')
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(1)
  setVisibility('hidden')
  expect(signal(0).aborted).toBe(true)
  first.resolve(response())
  await flushPromises()
  expect(wrapper.text()).not.toContain('299.001 秒')
  await vi.advanceTimersByTimeAsync(10000)
  expect(fetchMock).toHaveBeenCalledTimes(1)
  setVisibility('visible')
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  second.resolve(response())
  await flushPromises()
  expect(wrapper.text()).toContain('299.001 秒')
})

it.each(['room', 'context', 'session'] as const)('%s 切換清舊取樣，舊 success/finally 不解鎖新查詢', async (change) => {
  const initial = response()
  const old = deferred()
  const next = deferred()
  fetchMock.mockResolvedValueOnce(initial).mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise)
  const wrapper = await mountClock()
  await flushPromises()
  expect(wrapper.text()).toContain('299.001 秒')
  await vi.advanceTimersByTimeAsync(1000)
  expect(fetchMock).toHaveBeenCalledTimes(2)
  const newResponse = response()
  if (change === 'room') {
    newResponse.game = { gameId: 'room/new', gameNumber: 3 }
    await wrapper.setProps({
      gameId: 'room/new', roomGameId: 'room/new',
      mode: { kind: 'competition', context: { competitionId: 'competition a', matchId: 'match/one', gameNumber: 3 } },
    })
  } else if (change === 'context') {
    newResponse.competitionId = 'competition new'
    newResponse.matchId = 'match/new'
    await wrapper.setProps({
      mode: { kind: 'competition', context: { competitionId: 'competition new', matchId: 'match/new', gameNumber: 2 } },
    })
  } else {
    await wrapper.setProps({ sessionUserId: 'member-b', memberUserId: 'member-b' })
  }
  expect(signal(1).aborted).toBe(true)
  expect(wrapper.text()).not.toContain('299.001 秒')
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  // setProps 等到 DOM tick；再 flush 已排程的 microtasks 才觀察新 GET，不能以送出次數取代立即停用斷言。
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  old.resolve(initial)
  await flushPromises()
  expect(wrapper.text()).not.toContain('299.001 秒')
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  await vi.advanceTimersByTimeAsync(5000)
  expect(fetchMock).toHaveBeenCalledTimes(3)
  newResponse.clock.teams[0]!.mainTimeRemainingMs = 270003
  next.resolve(newResponse)
  await flushPromises()
  expect(wrapper.text()).toContain('270.003 秒')
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeUndefined()
})

it('登出／失去固定成員立即 abort，晚 error 不復活；普通／invalid 不降級查詢', async () => {
  const pending = deferred()
  fetchMock.mockReturnValueOnce(pending.promise)
  const wrapper = await mountClock()
  await wrapper.setProps({ sessionUserId: null, memberUserId: null })
  expect(signal(0).aborted).toBe(true)
  pending.reject({ statusCode: 503, message: 'PRIVATE_INTERNAL_ERROR' })
  await flushPromises()
  expect(wrapper.text()).toContain('請登入後查詢競賽時計。')
  expect(wrapper.text()).not.toContain('時計服務暫時無法使用')
  expect(wrapper.text()).not.toContain('PRIVATE')
  await wrapper.setProps({ sessionUserId: 'member-a', memberUserId: 'member-b' })
  expect(fetchMock).toHaveBeenCalledTimes(1)
  await wrapper.setProps({ mode: { kind: 'ordinary' }, memberUserId: 'member-a' })
  await wrapper.setProps({ mode: { kind: 'invalid' } })
  const malformed = { kind: 'competition' as const, context: { competitionId: 'competition a', matchId: 'match/one', gameNumber: 2, private: 'PRIVATE' } }
  await wrapper.setProps({ mode: malformed })
  await vi.advanceTimersByTimeAsync(10000)
  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
})

it('route 已切換但 metadata/context 尚未同步時不查舊房間時計', async () => {
  fetchMock.mockResolvedValue(response())
  const wrapper = await mountClock({ gameId: 'room/new' })
  expect(fetchMock).not.toHaveBeenCalled()
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  await wrapper.setProps({ gameId: 'room/one' })
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(wrapper.text()).toContain('299.001 秒')
  await wrapper.setProps({ gameId: 'room/new' })
  expect(wrapper.text()).not.toContain('299.001 秒')
  await vi.advanceTimersByTimeAsync(5000)
  expect(fetchMock).toHaveBeenCalledTimes(1)
})

it('unknown response loss 清舊值、停自動、手動 retry 仍只有同一專用 GET', async () => {
  fetchMock.mockResolvedValueOnce(response()).mockRejectedValueOnce({ statusCode: 503, data: { message: 'PRIVATE_ENGINE' } })
  const wrapper = await mountClock()
  await flushPromises()
  expect(wrapper.text()).toContain('299.001 秒')
  await vi.advanceTimersByTimeAsync(1000)
  await flushPromises()
  expect(wrapper.text()).not.toContain('299.001 秒')
  expect(wrapper.text()).toContain('時計服務暫時無法使用，已停止自動更新。請手動重試。')
  expect(wrapper.text()).not.toContain('PRIVATE')
  await vi.advanceTimersByTimeAsync(10000)
  expect(fetchMock).toHaveBeenCalledTimes(2)
  const retry = deferred()
  fetchMock.mockReturnValueOnce(retry.promise)
  await wrapper.get('.clock-refresh').trigger('click')
  await wrapper.get('.clock-refresh').trigger('click')
  expect(fetchMock).toHaveBeenCalledTimes(3)
  expect(fetchMock.mock.calls[2]?.[0]).toEqual({ signal: signal(2) })
  retry.resolve(response())
  await flushPromises()
  expect(wrapper.text()).toContain('299.001 秒')
})

it.each(['wrongGame', 'privateExtra', 'unsafeClock'] as const)('%s 不渲染、停自動，不改 GameRoom 正式終局', async (bad) => {
  const current = response()
  if (bad === 'wrongGame') current.game = { gameId: 'PRIVATE_OTHER_ROOM', gameNumber: 2 }
  else if (bad === 'privateExtra') Object.assign(current.clock, { privateDeck: 'PRIVATE_DECK' })
  else current.clock.teams[0]!.mainTimeRemainingMs = NaN
  fetchMock.mockResolvedValue(current)
  const wrapper = await mountClock()
  await flushPromises()
  expect(wrapper.text()).toContain('時計回應無法安全驗證，已停止自動更新。')
  expect(wrapper.find('.clock-teams').exists()).toBe(false)
  expect(wrapper.text()).not.toContain('PRIVATE')
  expect(wrapper.emitted()).toEqual({})
  await vi.advanceTimersByTimeAsync(10000)
  expect(fetchMock).toHaveBeenCalledTimes(1)
})

it('權威準備／action／choice／paused 與讀秒 flag 原樣呈現；0/unlimited 不推定終局', async () => {
  const preparation = response()
  preparation.clock.running = { type: 'preparation', teams: [teamIds[1], teamIds[0]] }
  preparation.clock.teams[0]!.mainTimeRemainingMs = null
  preparation.clock.teams[0]!.byoyomiTimeRemainingMs = null
  fetchMock.mockResolvedValueOnce(preparation)
  const wrapper = await mountClock()
  await flushPromises()
  expect(wrapper.get('.clock-phase').text()).toBe('準備計時：乙隊、甲隊')
  expectChineseTeamLabels(wrapper)
  expect(wrapper.findAll('.clock-team dt').map(label => label.text())).toEqual(['主時間', '讀秒額度', '主時間', '讀秒額度'])
  expect(wrapper.text()).toContain('不限時')
  expect(wrapper.text()).toContain('讀秒額度')
  const paused = response()
  paused.clock.running = null
  paused.clock.paused = { type: 'action', team: teamIds[1], turn: 1 }
  fetchMock.mockResolvedValueOnce(paused)
  await wrapper.get('.clock-refresh').trigger('click')
  await flushPromises()
  expect(wrapper.get('.clock-phase').text()).toBe('計時已暫停：行動計時：乙隊，回合編號 1')
  expectChineseTeamLabels(wrapper)
  const choice = response()
  choice.clock.running = { type: 'outOfTurnChoice', team: teamIds[0], choiceId: 'choice-a', remainingMs: 0 }
  choice.clock.teams[0]!.mainTimeRemainingMs = 0
  choice.clock.teams[0]!.byoyomiTimeRemainingMs = 0
  choice.clock.teams[0]!.byoyomiActive = true
  choice.clock.awaitingAuthority = true
  fetchMock.mockResolvedValueOnce(choice)
  await wrapper.get('.clock-refresh').trigger('click')
  await flushPromises()
  expect(wrapper.text()).toContain('回合外選擇計時')
  expect(wrapper.get('.clock-phase').text()).toBe('回合外選擇計時：甲隊')
  expectChineseTeamLabels(wrapper)
  expect(wrapper.text()).toContain('回合外選擇剩餘：0 秒')
  expect(wrapper.text()).toContain('讀秒剩餘')
  expect(wrapper.text()).toContain('時間顯示為 0 不代表判負，請等待伺服器權威判定。')
  expect(wrapper.text()).toContain('時計正在等待伺服器權威確認')
  expect(wrapper.emitted('refresh')).toBeUndefined()
  expect(wrapper.get('.clock-teams').attributes('aria-live')).toBe('off')
})

it.each(['open', 'awaitingFirstPlayer', 'awaitingReferee', 'completed'] as const)('%s/null 不推測房間 Finished；只有明確 completed 顯示交手完成', async (status) => {
  const current = response()
  current.matchStatus = status
  current.game = null
  current.clock.running = null
  fetchMock.mockResolvedValue(current)
  const wrapper = await mountClock()
  await flushPromises()
  expect(wrapper.find('.clock-teams').exists()).toBe(false)
  expect(wrapper.text().includes('交手已完成')).toBe(status === 'completed')
  if (status !== 'completed') expect(wrapper.text()).toContain('目前沒有可顯示的當局時計，請稍後重新查詢或向主辦方確認。')
  if (status === 'awaitingReferee') expect(wrapper.text()).toContain('等待裁判確認。')
  expect(wrapper.emitted()).toEqual({})
})

it('unmount abort 並停止 scheduler，晚結果不再讀取或排程', async () => {
  const pending = deferred()
  fetchMock.mockReturnValueOnce(pending.promise)
  const wrapper = await mountClock()
  wrapper.unmount()
  expect(signal(0).aborted).toBe(true)
  pending.resolve(response())
  await flushPromises()
  await vi.advanceTimersByTimeAsync(10000)
  expect(fetchMock).toHaveBeenCalledTimes(1)
})

it('晚 error/finally 不解鎖另一個新查詢；新取樣顯示前不能重複送出', async () => {
  const old = deferred()
  const next = deferred()
  fetchMock.mockReturnValueOnce(old.promise).mockReturnValueOnce(next.promise)
  const wrapper = await mountClock()
  await wrapper.setProps({ sessionUserId: 'member-b', memberUserId: 'member-b' })
  expect(signal(0).aborted).toBe(true)
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  expect(wrapper.find('.clock-teams').exists()).toBe(false)
  // 保留排程空窗的立即停用；等待 queued microtasks 派送後才驗證 request 數量。
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(2)
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  old.reject({ statusCode: 403, message: 'PRIVATE_OLD_SCOPE' })
  await flushPromises()
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeDefined()
  expect(wrapper.text()).not.toContain('你沒有讀取')
  await wrapper.get('.clock-refresh').trigger('click')
  expect(fetchMock).toHaveBeenCalledTimes(2)
  next.resolve(response())
  await flushPromises()
  expect(wrapper.text()).toContain('299.001 秒')
  expect(wrapper.get('.clock-refresh').attributes('disabled')).toBeUndefined()
})

it('時計有標題關聯，按鈕鍵盤不冒泡到外層牌局快捷鍵且不 trap Tab', async () => {
  fetchMock.mockResolvedValue(response())
  const wrapper = await mountClock()
  const section = wrapper.get('section')
  const heading = wrapper.get('h2')
  expect(section.attributes('aria-labelledby')).toBe(heading.attributes('id'))
  const pageKeydown = vi.fn()
  section.element.addEventListener('keydown', pageKeydown)
  try {
    section.element.dispatchEvent(new KeyboardEvent('keydown', { key: '1', bubbles: true }))
    expect(pageKeydown).toHaveBeenCalledTimes(1)
    pageKeydown.mockClear()
    const button = wrapper.get('.clock-refresh')
    for (const key of ['1', 'Enter', ' ', 'Tab']) {
      const event = new KeyboardEvent('keydown', { key, bubbles: true, cancelable: true })
      button.element.dispatchEvent(event)
      expect(event.defaultPrevented).toBe(false)
    }
    expect(pageKeydown).not.toHaveBeenCalled()
  } finally {
    section.element.removeEventListener('keydown', pageKeydown)
  }
})
