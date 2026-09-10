export type RoomRouteResult =
  | '房間已滿'
  | '對局已開始'
  | '找不到這個房間'
  | '你沒有權限進入這個房間'
  | '房間服務暫時無法處理要求，請稍後再試。'
  | '無法載入這個房間，請稍後再試。'

export function safeInternalPath(value: unknown): string | undefined {
  if (
    typeof value !== 'string'
    || !value.startsWith('/')
    || value.startsWith('//')
  ) {
    return undefined
  }

  const url = new URL(value, 'https://fewfc.invalid')
  return url.origin === 'https://fewfc.invalid' && (
    url.pathname === '/'
    || url.pathname === '/deck'
    || url.pathname === '/rooms'
    || url.pathname.startsWith('/rooms/')
    || url.pathname === '/replays'
    || url.pathname.startsWith('/replays/')
    || url.pathname === '/account'
    || url.pathname === '/privacy'
  )
    ? `${url.pathname}${url.search}${url.hash}`
    : undefined
}

export function roomRouteResult(error: unknown): RoomRouteResult {
  const responseError = error as {
    status?: unknown
    statusCode?: unknown
    message?: unknown
    data?: { statusMessage?: unknown, message?: unknown, error?: unknown }
  }
  const status = numericStatus(responseError?.statusCode) ?? numericStatus(responseError?.status)
  const message = errorMessage(responseError).toLowerCase()

  if (status === 404) return '找不到這個房間'
  if (status !== undefined && status >= 500) return '房間服務暫時無法處理要求，請稍後再試。'
  if (message.includes('full')) return '房間已滿'
  if (message.includes('started') || message.includes('active')) return '對局已開始'
  if (status === 403) return '你沒有權限進入這個房間'
  return '無法載入這個房間，請稍後再試。'
}

function numericStatus(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

function errorMessage(error: {
  message?: unknown
  data?: { statusMessage?: unknown, message?: unknown, error?: unknown }
}): string {
  for (const value of [
    error?.message,
    error?.data?.statusMessage,
    error?.data?.message,
    error?.data?.error,
  ]) {
    if (typeof value === 'string') return value
  }
  return ''
}
