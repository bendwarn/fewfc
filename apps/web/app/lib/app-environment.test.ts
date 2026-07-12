import assert from 'node:assert/strict'
import { test } from 'bun:test'
import { assertDevelopmentEnvironment } from '../../server/utils/app-environment'

test('development-only routes remain hidden in staging and production', () => {
  assert.doesNotThrow(() => assertDevelopmentEnvironment('development'))
  for (const environment of ['staging', 'production'] as const) {
    assert.throws(
      () => assertDevelopmentEnvironment(environment),
      (error: unknown) => Boolean(
        error
        && typeof error === 'object'
        && 'statusCode' in error
        && error.statusCode === 404,
      ),
    )
  }
})
