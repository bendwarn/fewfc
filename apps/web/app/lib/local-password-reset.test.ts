import { describe, expect, test } from 'bun:test'
import { isLocalPasswordResetEnabled } from '../../shared/local-password-reset'

describe('isLocalPasswordResetEnabled', () => {
  test('requires both the development environment and explicit enablement', () => {
    expect(isLocalPasswordResetEnabled('development', 'true')).toBe(true)
    expect(isLocalPasswordResetEnabled('development', undefined)).toBe(false)
    expect(isLocalPasswordResetEnabled('staging', 'true')).toBe(false)
    expect(isLocalPasswordResetEnabled('production', 'true')).toBe(false)
  })
})
