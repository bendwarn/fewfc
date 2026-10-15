import { mountSuspended, mockNuxtImport } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { expect, it, vi } from 'vitest'
import CompetitionReadyControls from '~/components/CompetitionReadyControls.vue'

// 主審獨立回歸：同房斷線僅失效 transport，不代表未知狀態已被確認。
const fetchMock = vi.hoisted(() => vi.fn())
mockNuxtImport('$fetch', () => fetchMock)

it('shows a fresh ready action when the authoritative room clears readiness on disconnect', async () => {
  fetchMock.mockReset()
  const wrapper = await mountSuspended(CompetitionReadyControls, {
    props: {
      gameId: 'room-a',
      mode: { kind: 'competition', context: { competitionId: 'c', matchId: 'm', gameNumber: 1 } },
      sessionUserId: 'member-a', memberUserId: 'member-a', roomStatus: 'Waiting',
      memberReady: true, memberConnected: true, socketConnected: true, loading: false,
    },
  })
  try {
    expect(wrapper.text()).toContain('目前狀態：已準備')
    expect(wrapper.get('button.ready-action').text()).toBe('取消準備')

    await wrapper.setProps({ memberReady: false, memberConnected: false, socketConnected: false })
    expect(wrapper.text()).toContain('目前狀態：尚未準備')
    expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()

    await wrapper.setProps({ memberConnected: true, socketConnected: true })
    expect(wrapper.text()).toContain('目前狀態：尚未準備')
    expect(wrapper.get('button.ready-action').text()).toBe('我準備好了')
    expect(wrapper.get('button.ready-action').attributes('disabled')).toBeUndefined()
    expect(wrapper.find('button.same-state-retry').exists()).toBe(false)
    expect(fetchMock).not.toHaveBeenCalled()
  } finally { await wrapper.unmount() }
})

it('keeps an unresolved user intent available only for explicit retry after reconnect', async () => {
  fetchMock.mockRejectedValueOnce({ statusCode: 503 }).mockResolvedValueOnce({ ready: true })
  const wrapper = await mountSuspended(CompetitionReadyControls, {
    props: {
      gameId: 'room-a',
      mode: { kind: 'competition', context: { competitionId: 'c', matchId: 'm', gameNumber: 1 } },
      sessionUserId: 'member-a', memberUserId: 'member-a', roomStatus: 'Waiting',
      memberReady: false, memberConnected: true, socketConnected: true, loading: false,
    },
  })
  try {
    await wrapper.get('button.ready-action').trigger('click')
    await flushPromises()
    expect(wrapper.find('button.same-state-retry').exists()).toBe(true)
    await wrapper.setProps({ socketConnected: false })
    expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()
    await wrapper.setProps({ socketConnected: true })
    // 尚未有新的權威 ready=true，不能重連後將未知 intent 視為完成。
    expect(wrapper.find('button.same-state-retry').exists()).toBe(true)
    expect(wrapper.get('button.ready-action').attributes('disabled')).toBeDefined()
    await wrapper.get('button.same-state-retry').trigger('click')
    await flushPromises()
    expect(fetchMock.mock.calls.map(([, options]) => options.body)).toEqual([{ ready: true }, { ready: true }])
  } finally { await wrapper.unmount() }
})
