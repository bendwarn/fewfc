import { describe, expect, test } from 'bun:test'
import { rulesEngineError } from './rules-engine-error'

describe('rulesEngineError', () => {
  test('classifies a structured rules validation failure as bad input', () => {
    const error = rulesEngineError({
      game: { Validation: 'SecretStrategyInputInvalid' },
    })

    expect(error).toMatchObject({
      statusCode: 400,
      code: 'rulesValidation',
      message: '選擇無效，請重新選擇。',
    })
  })

  test('supports the legacy serialized error detail during rollout', () => {
  const error = rulesEngineError('{"game":{"Validation":"InvalidChoiceAnswer"}}')

    expect(error.statusCode).toBe(400)
  })

  test('keeps implementation failures internal', () => {
    const error = rulesEngineError({ game: { EngineInvariant: 'MissingState' } })

    expect(error).toMatchObject({
      statusCode: 500,
      code: 'rulesEngineFailure',
      message: '規則引擎處理失敗，請稍後再試。',
    })
  })
})
