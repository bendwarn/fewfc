import { expect, test } from 'bun:test'
import { assertDevelopmentEnvironment } from '../../server/utils/app-environment'

test('development-only routes remain hidden in staging and production', () => {
  expect(() => assertDevelopmentEnvironment('development')).not.toThrow()
  for (const environment of ['staging', 'production'] as const) {
    let thrown: unknown
    try {
      assertDevelopmentEnvironment(environment)
    } catch (error) {
      thrown = error
    }
    expect(thrown).toMatchObject({ statusCode: 404 })
  }
})
