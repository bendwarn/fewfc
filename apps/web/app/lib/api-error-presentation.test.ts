import { describe, expect, test } from 'bun:test'
import { presentApiError } from './api-error-presentation'

describe('presentApiError', () => {
  test('uses a safe client error message for server failures', () => {
    expect(presentApiError({
      statusCode: 500,
      data: { statusMessage: '{"error":"EngineInvariant"}' },
    }, '操作失敗，請稍後再試。')).toBe('操作失敗，請稍後再試。')
  })

  test('uses an application message for rejected input', () => {
    expect(presentApiError({
      statusCode: 400,
      data: { statusMessage: '選擇無效，請重新選擇。' },
    }, '操作失敗')).toBe('選擇無效，請重新選擇。')
  })

  test('does not render transport messages or serialized response bodies', () => {
    expect(presentApiError({
      statusCode: 400,
      data: { statusMessage: '[POST] /api/games/room/commands: 400 Bad Request' },
    }, '操作失敗')).toBe('操作失敗')
    expect(presentApiError({
      statusCode: 400,
      data: { error: '{"game":{"Validation":"SecretStrategyInputInvalid"}}' },
    }, '操作失敗')).toBe('操作失敗')
  })
})
