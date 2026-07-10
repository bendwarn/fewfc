import assert from 'node:assert/strict'
import { describe, test } from 'node:test'
import { isLocalPasswordResetEnabled } from '../../shared/local-password-reset'

describe('isLocalPasswordResetEnabled', () => {
  test('requires both the development environment and explicit enablement', () => {
    assert.equal(isLocalPasswordResetEnabled('development', 'true'), true)
    assert.equal(isLocalPasswordResetEnabled('development', undefined), false)
    assert.equal(isLocalPasswordResetEnabled('staging', 'true'), false)
    assert.equal(isLocalPasswordResetEnabled('production', 'true'), false)
  })
})
