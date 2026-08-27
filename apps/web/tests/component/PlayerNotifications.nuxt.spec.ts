import { mountSuspended } from '@nuxt/test-utils/runtime'
import { defineComponent, h } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import type { PlayerNotification } from '#shared/game-room'
import { usePlayerNotifications } from '~/composables/usePlayerNotifications'
import {
  PLAYER_NOTIFICATION_TTL_MS,
  playerNotificationStorageKey,
  restorePlayerNotifications,
  serializePlayerNotifications,
} from '~/lib/player-notifications'

class FakeWebSocket extends EventTarget {
  static CONNECTING = 0
  static OPEN = 1
  static CLOSING = 2
  readyState = FakeWebSocket.OPEN
  readonly url: string

  constructor(url: string) {
    super()
    this.url = url
  }

  close() {
    this.readyState = 3
    this.dispatchEvent(new Event('close'))
  }

  receive(notification: PlayerNotification) {
    this.dispatchEvent(new MessageEvent('message', {
      data: JSON.stringify({ type: 'notification', data: notification }),
    }))
  }
}

function notification(
  id: string,
  gameId: string,
  kind: PlayerNotification['kind'] = 'yourTurn',
): PlayerNotification {
  return {
    id,
    gameId,
    kind,
    message: `房間 ${gameId} 輪到你行動。`,
    createdAt: new Date().toISOString(),
  }
}

function installWebSocket(sockets: FakeWebSocket[]) {
  vi.stubGlobal('WebSocket', class extends FakeWebSocket {
    constructor(url: string) {
      super(url)
      sockets.push(this)
    }
  })
}

async function mountNotifications() {
  const component = defineComponent({
    setup() {
      return { playerNotifications: usePlayerNotifications() }
    },
    render: () => h('div'),
  })
  const wrapper = await mountSuspended(component)
  return {
    wrapper,
    notifications: (wrapper.vm as unknown as {
      playerNotifications: ReturnType<typeof usePlayerNotifications>
    }).playerNotifications,
  }
}

afterEach(() => {
  vi.useRealTimers()
  vi.unstubAllGlobals()
  vi.restoreAllMocks()
  window.localStorage.clear()
})

it('keeps new room dots in memory when localStorage accepts reads but rejects writes', async () => {
  const { wrapper, notifications } = await mountNotifications()
  notifications.setUserId('player-a')
  const setItem = vi.spyOn(Storage.prototype, 'setItem').mockImplementation(() => {
    throw new DOMException('quota exceeded', 'QuotaExceededError')
  })

  const socket = new FakeWebSocket('ws://example.test')
  vi.stubGlobal('WebSocket', class extends FakeWebSocket {
    constructor(url: string) {
      super(url)
      return socket
    }
  })
  notifications.connect()
  socket.receive(notification('1', 'room-a'))
  socket.receive(notification('2', 'room-b'))

  expect(notifications.notifications.value.map(item => item.gameId)).toEqual(['room-b', 'room-a'])
  setItem.mockRestore()
  await wrapper.unmount()
})

it('ignores a prior account socket after switching accounts', async () => {
  const sockets: FakeWebSocket[] = []
  installWebSocket(sockets)
  const { wrapper, notifications } = await mountNotifications()
  notifications.setUserId('player-a')
  notifications.connect()
  notifications.setUserId('player-b')
  notifications.connect()

  sockets[0]?.receive(notification('old', 'room-a'))
  sockets[1]?.receive(notification('new', 'room-b'))

  expect(notifications.notifications.value.map(item => item.gameId)).toEqual(['room-b'])
  expect(window.localStorage.getItem(playerNotificationStorageKey('player-a'))).toBeNull()
  await wrapper.unmount()
})

it('does not restore a consumed dot from a delayed storage event', async () => {
  const { wrapper, notifications } = await mountNotifications()
  const key = playerNotificationStorageKey('player-a')
  const oldValue = serializePlayerNotifications([notification('1', 'room-a')])
  window.localStorage.setItem(key, oldValue)
  notifications.setUserId('player-a')
  window.localStorage.removeItem(key)

  window.dispatchEvent(new StorageEvent('storage', { key, newValue: oldValue }))

  expect(notifications.notifications.value).toEqual([])
  await wrapper.unmount()
})

it('hides ordinary prompts after five seconds or dismissal without consuming their dots', async () => {
  const sockets: FakeWebSocket[] = []
  installWebSocket(sockets)
  const { wrapper, notifications } = await mountNotifications()
  notifications.setUserId('player-a')
  notifications.connect()
  vi.useFakeTimers()

  sockets[0]?.receive(notification('1', 'room-a'))
  expect(notifications.visibleNotifications.value.map(item => item.gameId)).toEqual(['room-a'])
  notifications.dismiss('1')
  expect(notifications.notifications.value.map(item => item.gameId)).toEqual(['room-a'])

  sockets[0]?.receive(notification('2', 'room-b'))
  await vi.advanceTimersByTimeAsync(5000)
  expect(notifications.visibleNotifications.value).toEqual([])
  expect(notifications.notifications.value.map(item => item.gameId)).toEqual(['room-b', 'room-a'])
  await wrapper.unmount()
})

it('preserves an account storage entry through logout and restores only its dot', async () => {
  const sockets: FakeWebSocket[] = []
  installWebSocket(sockets)
  const { wrapper, notifications } = await mountNotifications()
  notifications.setUserId('player-a')
  notifications.connect()
  sockets[0]?.receive(notification('1', 'room-a'))
  notifications.dismiss('1')

  notifications.setUserId('')
  notifications.setUserId('player-b')
  expect(notifications.notifications.value).toEqual([])
  notifications.setUserId('player-a')

  expect(notifications.notifications.value.map(item => item.gameId)).toEqual(['room-a'])
  expect(notifications.visibleNotifications.value).toEqual([])
  expect(restorePlayerNotifications(window.localStorage.getItem(playerNotificationStorageKey('player-a')))).toHaveLength(1)
  await wrapper.unmount()
})

it('expires retained dots while the page remains open', async () => {
  const sockets: FakeWebSocket[] = []
  installWebSocket(sockets)
  vi.useFakeTimers({ now: Date.parse('2026-08-27T12:00:00.000Z') })
  const { wrapper, notifications } = await mountNotifications()
  notifications.setUserId('player-a')
  notifications.connect()
  sockets[0]?.receive(notification('1', 'room-a'))

  await vi.advanceTimersByTimeAsync(PLAYER_NOTIFICATION_TTL_MS)
  expect(notifications.notifications.value).toEqual([])
  expect(restorePlayerNotifications(window.localStorage.getItem(playerNotificationStorageKey('player-a')))).toEqual([])
  await wrapper.unmount()
})

it('keeps removal and dissolution prompts transient while clearing older room dots', async () => {
  const sockets: FakeWebSocket[] = []
  installWebSocket(sockets)
  const { wrapper, notifications } = await mountNotifications()
  notifications.setUserId('player-a')
  notifications.connect()
  sockets[0]?.receive(notification('1', 'room-a'))
  sockets[0]?.receive(notification('2', 'room-a', 'removed'))
  sockets[0]?.receive(notification('3', 'room-b'))
  sockets[0]?.receive(notification('4', 'room-b', 'dissolved'))

  expect(notifications.notifications.value).toEqual([])
  expect(notifications.visibleNotifications.value.map(item => item.kind)).toEqual(['dissolved', 'removed'])
  expect(restorePlayerNotifications(window.localStorage.getItem(playerNotificationStorageKey('player-a')))).toEqual([])
  await wrapper.unmount()
})
