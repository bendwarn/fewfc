import { describe, expect, test } from 'bun:test'
import { roomRouteResult, safeInternalPath } from './navigation'

describe('safeInternalPath', () => {
  test('accepts only application routes', () => {
    expect(safeInternalPath('/rooms/abc?invite=secret')).toBe('/rooms/abc?invite=secret')
    expect(safeInternalPath('/rooms?tab=join')).toBe('/rooms?tab=join')
    expect(safeInternalPath('/deck')).toBe('/deck')
    expect(safeInternalPath('/login')).toBe(undefined)
  })

  test('rejects external and protocol-relative redirects', () => {
    expect(safeInternalPath('https://example.com/rooms')).toBe(undefined)
    expect(safeInternalPath('//example.com/rooms')).toBe(undefined)
    expect(safeInternalPath(undefined)).toBe(undefined)
  })
})

describe('roomRouteResult', () => {
  test('maps room failures to player-facing results', () => {
    expect(roomRouteResult(new Error('room is full'))).toBe('房間已滿')
    expect(roomRouteResult(new Error('room has already started'))).toBe('對局已開始')
    expect(roomRouteResult(new Error('room not found'))).toBe('找不到這個房間')
  })
})
