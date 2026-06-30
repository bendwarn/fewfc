export type RoomRouteResult = '房間已滿' | '對局已開始' | '找不到這個房間'

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
  )
    ? `${url.pathname}${url.search}${url.hash}`
    : undefined
}

export function roomRouteResult(error: unknown): RoomRouteResult {
  const message = error instanceof Error ? error.message.toLowerCase() : ''

  if (message.includes('full')) return '房間已滿'
  if (message.includes('started') || message.includes('active')) return '對局已開始'
  return '找不到這個房間'
}
