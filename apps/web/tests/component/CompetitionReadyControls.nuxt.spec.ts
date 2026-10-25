import { mountSuspended } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { afterEach, beforeEach, expect, it, vi } from 'vitest'
import type { CompetitionRoomMode } from '~/lib/competition-ready'
import CompetitionReadyControls from '~/components/CompetitionReadyControls.vue'

const fetchMock = vi.hoisted(() => vi.fn())

const context = { competitionId: 'competition-a', matchId: 'match-a', gameNumber: 1 }

function initialProps() {
  return {
    submitReady: fetchMock,
    gameId: 'room/a',
    mode: { kind: 'competition' as const, context },
    sessionUserId: 'member-a',
    roomStatus: 'Waiting',
    memberUserId: 'member-a',
    memberReady: false,
    memberConnected: true,
    socketConnected: true,
    loading: false,
  }
}

function signalAt(index: number): AbortSignal {
  const signal: unknown = fetchMock.mock.calls[index]?.[1]?.signal
  if (!(signal instanceof AbortSignal)) throw new Error('expected competition-ready AbortSignal')
  return signal
}

function requestBodies(): unknown[] {
  return fetchMock.mock.calls.map(([ready]) => ({ ready }))
}

beforeEach(() => fetchMock.mockReset())

afterEach(() => vi.restoreAllMocks())

it('sends an explicit desired state for any fixed member and waits for server refresh instead of optimistic metadata', async () => {
  fetchMock.mockResolvedValue({ ready: true })
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  await flushPromises()

  expect(fetchMock).toHaveBeenCalledTimes(1)
  expect(fetchMock.mock.calls[0]?.[0]).toBe(true)
  expect(signalAt(0).aborted).toBe(false)
  expect(wrapper.text()).toContain('目前狀態：尚未準備')
  expect(wrapper.text()).toContain('正在同步房間狀態')
  expect(wrapper.emitted('refresh')).toHaveLength(1)
  await wrapper.unmount()
})

it('locks out duplicate submissions while the explicit request is pending', async () => {
  let resolveRequest!: (value: unknown) => void
  fetchMock.mockReturnValueOnce(new Promise<unknown>((resolve) => { resolveRequest = resolve }))
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()
  await wrapper.get('button.ready-action').trigger('click')
  expect(fetchMock).toHaveBeenCalledTimes(1)
  resolveRequest({ ready: true })
  await flushPromises()
  expect(fetchMock).toHaveBeenCalledTimes(1)
  await wrapper.unmount()
})

it('retains the exact desired boolean after response loss and retries only that same explicit state', async () => {
  fetchMock
    .mockRejectedValueOnce({ statusCode: 503, message: 'private-rpc-detail' })
    .mockResolvedValueOnce({ ready: true })
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  await flushPromises()
  expect(wrapper.get('button.same-state-retry').text()).toContain('以相同狀態重試')
  expect(wrapper.text()).not.toContain('private-rpc-detail')
  expect(requestBodies()).toEqual([{ ready: true }])
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()

  await wrapper.get('button.same-state-retry').trigger('click')
  await flushPromises()
  expect(requestBodies()).toEqual([{ ready: true }, { ready: true }])
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()
  expect(wrapper.emitted('refresh')).toHaveLength(2)

  await wrapper.setProps({ memberReady: true })
  expect(wrapper.text()).toContain('目前狀態：已準備')
  expect(wrapper.find('button.same-state-retry').exists()).toBe(false)
  await wrapper.unmount()
})

it('retains an unknown desired state across a same-room disconnect and retries it after reconnect', async () => {
  fetchMock
    .mockRejectedValueOnce({ statusCode: 503, message: 'private-rpc-detail' })
    .mockResolvedValueOnce({ ready: true })
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  await flushPromises()
  expect(wrapper.find('button.same-state-retry').exists()).toBe(true)
  expect(wrapper.emitted('refresh')).toHaveLength(1)

  await wrapper.setProps({ socketConnected: false })
  expect(wrapper.find('button.same-state-retry').exists()).toBe(true)
  expect(wrapper.get('button.same-state-retry').attributes('disabled')).toBeDefined()
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()

  await wrapper.setProps({ socketConnected: true })
  expect(wrapper.find('button.same-state-retry').exists()).toBe(true)
  expect(wrapper.get('button.same-state-retry').attributes('disabled')).toBeUndefined()
  await wrapper.get('button.same-state-retry').trigger('click')
  await flushPromises()

  expect(requestBodies()).toEqual([{ ready: true }, { ready: true }])
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()
  expect(wrapper.emitted('refresh')).toHaveLength(2)
  await wrapper.setProps({ memberReady: true })
  expect(wrapper.find('button.same-state-retry').exists()).toBe(false)
  await wrapper.unmount()
})

it('keeps malformed success as unknown, requests a read refresh, and never treats it as an acknowledgement', async () => {
  fetchMock.mockResolvedValue({ ready: false, privateHistory: 'secret' })
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  await flushPromises()

  expect(wrapper.text()).toContain('回應格式不正確')
  expect(wrapper.text()).not.toContain('secret')
  expect(wrapper.get('button.same-state-retry').exists()).toBe(true)
  expect(wrapper.emitted('refresh')).toHaveLength(1)
  await wrapper.unmount()
})

it('aborts and ignores late responses when room context, session, membership, or readiness eligibility changes', async () => {
  let resolveFirst!: (value: unknown) => void
  fetchMock.mockReturnValueOnce(new Promise<unknown>((resolve) => { resolveFirst = resolve }))
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  expect(signalAt(0).aborted).toBe(false)
  await wrapper.setProps({
    gameId: 'room/b',
    mode: { kind: 'competition', context: { ...context, gameNumber: 2 } },
    sessionUserId: 'member-b',
    memberUserId: 'member-b',
  })
  expect(signalAt(0).aborted).toBe(true)
  resolveFirst({ ready: true })
  await flushPromises()
  expect(wrapper.emitted('refresh')).toBeUndefined()
  expect(wrapper.text()).not.toContain('正在同步房間狀態')

  let rejectSecond!: (reason: unknown) => void
  fetchMock.mockReturnValueOnce(new Promise<unknown>((_resolve, reject) => { rejectSecond = reject }))
  await wrapper.get('button.ready-action').trigger('click')
  expect(signalAt(1).aborted).toBe(false)
  await wrapper.setProps({ roomStatus: 'Active' })
  expect(signalAt(1).aborted).toBe(true)
  rejectSecond({ statusCode: 503, message: 'late-secret' })
  await flushPromises()
  expect(wrapper.text()).not.toContain('late-secret')
  expect(wrapper.find('button.same-state-retry').exists()).toBe(false)
  await wrapper.unmount()
})

it('does not revive a retry after auto-start changes the room to Active', async () => {
  let rejectRetry!: (reason: unknown) => void
  fetchMock
    .mockRejectedValueOnce(new Error('response lost'))
    .mockReturnValueOnce(new Promise<unknown>((_resolve, reject) => { rejectRetry = reject }))
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  await flushPromises()
  await wrapper.get('button.same-state-retry').trigger('click')
  expect(signalAt(1).aborted).toBe(false)

  await wrapper.setProps({ roomStatus: 'Active' })
  expect(signalAt(1).aborted).toBe(true)
  expect(wrapper.find('button.same-state-retry').exists()).toBe(false)
  rejectRetry({ statusCode: 503, message: 'late-secret' })
  await flushPromises()
  expect(wrapper.find('button.same-state-retry').exists()).toBe(false)
  expect(wrapper.text()).not.toContain('late-secret')
  await wrapper.unmount()
})

it('aborts an in-flight request but preserves its same desired retry across a same-room disconnect', async () => {
  let rejectRequest!: (reason: unknown) => void
  fetchMock
    .mockReturnValueOnce(new Promise<unknown>((_resolve, reject) => { rejectRequest = reject }))
    .mockResolvedValueOnce({ ready: true })
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  await wrapper.setProps({ socketConnected: false })
  expect(signalAt(0).aborted).toBe(true)
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()
  expect(wrapper.find('button.same-state-retry').exists()).toBe(true)
  expect(wrapper.get('button.same-state-retry').attributes('disabled')).toBeDefined()
  rejectRequest({ statusCode: 503, message: 'late-secret' })
  await flushPromises()
  expect(wrapper.emitted('refresh')).toBeUndefined()
  expect(wrapper.find('button.same-state-retry').exists()).toBe(true)
  expect(wrapper.text()).not.toContain('late-secret')

  await wrapper.setProps({ socketConnected: true })
  expect(wrapper.get('button.same-state-retry').attributes('disabled')).toBeUndefined()
  await wrapper.get('button.same-state-retry').trigger('click')
  await flushPromises()
  expect(requestBodies()).toEqual([{ ready: true }, { ready: true }])
  expect(wrapper.find('button.same-state-retry').exists()).toBe(false)
  expect(wrapper.find('button.refresh-status').exists()).toBe(true)
  await wrapper.setProps({ memberReady: true })
  expect(wrapper.find('button.refresh-status').exists()).toBe(false)
  await wrapper.unmount()
})

it('keeps an acknowledged desired state pending for authoritative read after disconnect', async () => {
  fetchMock.mockResolvedValueOnce({ ready: true })
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })

  await wrapper.get('button.ready-action').trigger('click')
  await flushPromises()
  expect(wrapper.find('button.refresh-status').exists()).toBe(true)
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()

  await wrapper.setProps({ socketConnected: false })
  expect(wrapper.find('button.refresh-status').exists()).toBe(true)
  expect(wrapper.get('button.refresh-status').attributes('disabled')).toBeDefined()
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()

  await wrapper.setProps({ socketConnected: true })
  expect(wrapper.find('button.refresh-status').exists()).toBe(true)
  expect(wrapper.get('button.refresh-status').attributes('disabled')).toBeUndefined()
  expect(wrapper.find('button.same-state-retry').exists()).toBe(false)

  await wrapper.setProps({ memberReady: true })
  expect(wrapper.find('button.refresh-status').exists()).toBe(false)
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeUndefined()
  await wrapper.unmount()
})

it('fails closed for ordinary and malformed modes and disables submission without live eligibility', async () => {
  const wrapper = await mountSuspended(CompetitionReadyControls, { props: initialProps() })
  await wrapper.setProps({ mode: { kind: 'ordinary' } satisfies CompetitionRoomMode })
  expect(wrapper.find('button.ready-action').exists()).toBe(false)
  expect(fetchMock).not.toHaveBeenCalled()

  await wrapper.setProps({ mode: { kind: 'invalid' } satisfies CompetitionRoomMode })
  expect(wrapper.text()).toContain('競賽房間資訊無法確認')
  expect(wrapper.find('button.ready-action').exists()).toBe(false)

  await wrapper.setProps({ mode: { kind: 'competition', context }, socketConnected: false })
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()
  await wrapper.setProps({ socketConnected: true, memberConnected: false })
  expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()
  expect(fetchMock).not.toHaveBeenCalled()
  await wrapper.unmount()
})
