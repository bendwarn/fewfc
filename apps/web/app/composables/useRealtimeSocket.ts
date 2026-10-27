import { useWebSocket } from '@vueuse/core'
import { computed, ref, onScopeDispose } from 'vue'
import { createSocketRequests, RealtimeError } from '../lib/socket-requests'

export function useRealtimeSocket<Operations extends { [Name in keyof Operations]: { input: unknown; output: unknown } }>(
  onEvent: (value: unknown, reconnected: boolean) => void,
) {
  const url = ref<string>()
  const enabled = ref(false)
  const reconnecting = ref(false)
  let opened = false
  let reconnected = false
  const waiters = new Set<{ resolve: () => void; reject: (error: unknown) => void }>()
  const requests = createSocketRequests(wire => socket.send(wire, false))
  const socket = useWebSocket(url, {
    immediate: false,
    autoConnect: false,
    autoClose: true,
    autoReconnect: {
      retries: () => enabled.value,
      delay: attempt => Math.min(1000 * 2 ** (attempt - 1), 10_000),
    },
    onConnected() {
      reconnected = opened
      opened = true
      reconnecting.value = false
      for (const waiter of waiters) waiter.resolve()
      waiters.clear()
    },
    onDisconnected(disconnected, event) {
      if (disconnected !== socket.ws.value) return
      requests.cancel()
      if ([1000, 4000, 4001, 4003].includes(event.code)) {
        close()
      } else if (enabled.value) reconnecting.value = true
    },
    onMessage(_socket, event) {
      if (typeof event.data !== 'string') return
      try {
        const message: unknown = JSON.parse(event.data)
        if (!requests.receive(message)) {
          onEvent(message, reconnected)
          reconnected = false
        }
      } catch {
        // 格式錯誤不能改變既有狀態；未完成請求仍受逾時機制保護。
      }
    },
  })
  const connectionState = computed<'idle' | 'connecting' | 'connected' | 'reconnecting'>(() => {
    if (!enabled.value) return 'idle'
    if (socket.status.value === 'OPEN') return 'connected'
    return reconnecting.value ? 'reconnecting' : 'connecting'
  })
  function connect(path: string) {
    if (!import.meta.client) return
    const next = `${window.location.protocol === 'https:' ? 'wss:' : 'ws:'}//${window.location.host}${path}`
    if (enabled.value && url.value === next) return
    close()
    url.value = next
    enabled.value = true
    socket.open()
  }
  function close() {
    enabled.value = false
    reconnecting.value = false
    opened = false
    requests.cancel()
    for (const waiter of waiters) waiter.reject(new RealtimeError(503, 'socketDisconnected', '連線已中斷。'))
    waiters.clear()
    socket.close()
  }
  async function ready(signal?: AbortSignal) {
    if (signal?.aborted) throw new DOMException('Aborted', 'AbortError')
    if (socket.status.value === 'OPEN') return
    if (!enabled.value) throw new RealtimeError(503, 'socketDisconnected', '尚未連線。')
    await new Promise<void>((resolve, reject) => {
      const dispose = () => {
        clearTimeout(timer)
        waiters.delete(waiter)
        signal?.removeEventListener('abort', abort)
      }
      const waiter = {
        resolve() { dispose(); resolve() },
        reject(error: unknown) { dispose(); reject(error) },
      }
      const abort = () => waiter.reject(new DOMException('Aborted', 'AbortError'))
      const timer = setTimeout(() => {
        waiter.reject(new RealtimeError(504, 'socketTimeout', '連線逾時，請稍後再試。'))
      }, 15_000)
      waiters.add(waiter)
      signal?.addEventListener('abort', abort, { once: true })
    })
  }
  async function request<Name extends keyof Operations & string>(
    operation: Name,
    input: Operations[Name]['input'],
    options?: { signal: AbortSignal },
  ): Promise<Operations[Name]['output']> {
    await ready(options?.signal)
    return await requests.request(operation, input, options?.signal) as Operations[Name]['output']
  }
  onScopeDispose(close)
  return { connect, close, request, connectionState }
}
