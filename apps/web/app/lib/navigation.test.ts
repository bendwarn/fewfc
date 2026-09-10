import { describe, expect, test } from 'bun:test'
import { roomRouteResult, safeInternalPath } from './navigation'

describe('safeInternalPath', () => {
  test('accepts only application routes', () => {
    expect(safeInternalPath('/rooms/abc?invite=secret')).toBe('/rooms/abc?invite=secret')
    expect(safeInternalPath('/rooms?tab=join')).toBe('/rooms?tab=join')
    expect(safeInternalPath('/deck')).toBe('/deck')
    expect(safeInternalPath('/replays/replay-7?step=12')).toBe('/replays/replay-7?step=12')
    expect(safeInternalPath('/login')).toBe(undefined)
  })

  test('rejects external and protocol-relative redirects', () => {
    expect(safeInternalPath('https://example.com/rooms')).toBe(undefined)
    expect(safeInternalPath('//example.com/rooms')).toBe(undefined)
    expect(safeInternalPath(undefined)).toBe(undefined)
  })
})

describe('roomRouteResult', () => {
  test('only presents an actual 404 as a missing room', () => {
    expect(roomRouteResult(new Error('room is full'))).toBe('房間已滿')
    expect(roomRouteResult(new Error('room has already started'))).toBe('對局已開始')
    expect(roomRouteResult({ status: 404 })).toBe('找不到這個房間')
    expect(roomRouteResult({ statusCode: 403 })).toBe('你沒有權限進入這個房間')
    expect(roomRouteResult({ status: 500 })).toBe('房間服務暫時無法處理要求，請稍後再試。')
    expect(roomRouteResult({ status: 429 })).toBe('無法載入這個房間，請稍後再試。')
  })
})
