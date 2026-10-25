import { mountSuspended } from '@nuxt/test-utils/runtime'
import { flushPromises } from '@vue/test-utils'
import { defineComponent, h } from 'vue'
import { afterEach, expect, it, vi } from 'vitest'
import { useRealtimeSocket } from '~/composables/useRealtimeSocket'

class FakeSocket extends EventTarget {
  static OPEN = 1
  readyState = 0
  sent: { requestId: string }[] = []
  constructor(readonly url: string) { super() }
  send(wire: string) { this.sent.push(JSON.parse(wire)) }
  close() { this.readyState = 3 }
  open() { this.readyState = 1; this.dispatchEvent(new Event('open')) }
  receive(value: unknown) { this.dispatchEvent(new MessageEvent('message', { data: JSON.stringify(value) })) }
}

async function mountSocket() {
  const sockets: FakeSocket[] = []
  vi.stubGlobal('WebSocket', class extends FakeSocket {
    constructor(url: string) { super(url); sockets.push(this) }
  })
  const events = vi.fn()
  let channel!: ReturnType<typeof useRealtimeSocket<{ state: { input: undefined; output: string } }>>
  const wrapper = await mountSuspended(defineComponent({
    setup() { channel = useRealtimeSocket(events); return () => h('div') },
  }))
  channel.connect('/socket-a')
  return { wrapper, channel, sockets, events }
}

afterEach(() => { vi.useRealTimers(); vi.unstubAllGlobals() })

it('aborts while connecting without sending the canceled request after connection', async () => {
  const { wrapper, channel, sockets } = await mountSocket()
  try {
    const controller = new AbortController()
    const canceled = channel.request('state', undefined, { signal: controller.signal })
    controller.abort()
    await expect(canceled).rejects.toMatchObject({ name: 'AbortError' })
    sockets[0]!.open()
    await flushPromises()
    expect(sockets[0]!.sent).toHaveLength(0)
    expect(channel.connectionState.value).toBe('connected')
    const request = channel.request('state', undefined)
    await flushPromises()
    sockets[0]!.receive({ type: 'result', requestId: sockets[0]!.sent[0]!.requestId, ok: true, data: 'current' })
    expect(await request).toBe('current')
  } finally { await wrapper.unmount() }
})

it('rejects interrupted requests, reconnects without replaying them, and ignores a late old close', async () => {
  vi.useFakeTimers()
  const { wrapper, channel, sockets, events } = await mountSocket()
  try {
    const old = sockets[0]!
    old.open()
    const lost = channel.request('state', undefined).catch(error => error.data.code)
    await flushPromises()
    old.close()
    old.dispatchEvent(new CloseEvent('close', { code: 1006 }))
    expect(await lost).toBe('socketDisconnected')
    expect(channel.connectionState.value).toBe('reconnecting')
    await vi.advanceTimersByTimeAsync(1000)
    const current = sockets[1]!
    current.open()
    current.receive({ type: 'roomState', data: 'restored' })
    expect(events).toHaveBeenCalledWith({ type: 'roomState', data: 'restored' }, true)
    expect(current.sent).toHaveLength(0)
    const request = channel.request('state', undefined)
    await flushPromises()
    old.dispatchEvent(new CloseEvent('close', { code: 1000 }))
    expect(channel.connectionState.value).toBe('connected')
    current.receive({ type: 'result', requestId: current.sent[0]!.requestId, ok: true, data: 'current' })
    expect(await request).toBe('current')
  } finally { await wrapper.unmount() }
})
