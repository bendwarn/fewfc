import { isSocketResult } from '#shared/realtime'

export class RealtimeError extends Error {
  readonly data: { code: string; message: string; data: { code: string } }
  constructor(readonly statusCode: number, code: string, message: string) {
    super(message)
    this.data = { code, message, data: { code } }
  }
}

/** 只配對目前連線的請求；斷線後不自動重送可能已經提交的玩家操作。 */
export function createSocketRequests(send: (wire: string) => boolean, timeoutMs = 15_000) {
  const pending = new Map<string, {
    resolve: (value: unknown) => void
    reject: (error: unknown) => void
    timer: ReturnType<typeof setTimeout>
    dispose: () => void
  }>()
  function receive(value: unknown): boolean {
    if (!isSocketResult(value)) return false
    const request = pending.get(value.requestId)
    if (!request) return true
    pending.delete(value.requestId)
    clearTimeout(request.timer)
    request.dispose()
    if (value.ok) request.resolve(value.data)
    else request.reject(new RealtimeError(value.error.statusCode, value.error.code, value.error.message))
    return true
  }
  function cancel(error = new RealtimeError(503, 'socketDisconnected', '連線已中斷；請等待重新連線後再試。')) {
    for (const request of pending.values()) {
      clearTimeout(request.timer)
      request.dispose()
      request.reject(error)
    }
    pending.clear()
  }
  function request(operation: string, input: unknown, signal?: AbortSignal): Promise<unknown> {
    const requestId = crypto.randomUUID()
    return new Promise((resolve, reject) => {
      if (signal?.aborted) { reject(new DOMException('Aborted', 'AbortError')); return }
      const abort = () => {
        pending.delete(requestId)
        clearTimeout(timer)
        reject(new DOMException('Aborted', 'AbortError'))
      }
      const dispose = () => signal?.removeEventListener('abort', abort)
      const timer = setTimeout(() => {
        pending.delete(requestId)
        dispose()
        reject(new RealtimeError(504, 'socketTimeout', '尚未收到操作結果，請確認最新狀態後再試。'))
      }, timeoutMs)
      pending.set(requestId, { resolve, reject, timer, dispose })
      signal?.addEventListener('abort', abort, { once: true })
      try {
        if (!send(JSON.stringify({ type: 'request', requestId, operation, input }))) {
          throw new RealtimeError(503, 'socketDisconnected', '尚未連線，請稍後再試。')
        }
      } catch (error) {
        pending.delete(requestId)
        clearTimeout(timer)
        dispose()
        reject(error)
      }
    })
  }
  return { request, receive, cancel }
}
