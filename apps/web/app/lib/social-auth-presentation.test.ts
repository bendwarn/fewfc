import { describe, expect, test } from 'bun:test'
import { SOCIAL_AUTH_FAILURE_MESSAGE, socialAuthFailureMessage } from './social-auth-presentation'

describe('social authentication error presentation', () => {
  test('shows a fixed safe message instead of reflecting an OAuth error query', () => {
    expect(socialAuthFailureMessage('provider_denied')).toBe(SOCIAL_AUTH_FAILURE_MESSAGE)
    expect(socialAuthFailureMessage('<img src=x onerror=alert(1)>')).toBe(SOCIAL_AUTH_FAILURE_MESSAGE)
    expect(socialAuthFailureMessage(undefined)).toBeUndefined()
  })
})
