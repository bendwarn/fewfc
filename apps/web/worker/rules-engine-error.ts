type ErrorRecord = Record<string, unknown>

export class RulesEngineError extends Error {
  readonly statusCode: 400 | 500
  readonly code: 'rulesValidation' | 'rulesEngineFailure'

  constructor(readonly detail: unknown) {
    const validation = containsValidationError(detail)
    super(validation ? '選擇無效，請重新選擇。' : '規則引擎處理失敗，請稍後再試。')
    this.name = 'RulesEngineError'
    this.statusCode = validation ? 400 : 500
    this.code = validation ? 'rulesValidation' : 'rulesEngineFailure'
  }
}

export function rulesEngineError(detail: unknown): RulesEngineError {
  return new RulesEngineError(parseSerializedDetail(detail))
}

function parseSerializedDetail(detail: unknown): unknown {
  if (typeof detail !== 'string') return detail

  try {
    return JSON.parse(detail) as unknown
  } catch {
    return detail
  }
}

function containsValidationError(detail: unknown): boolean {
  if (!isRecord(detail)) return false

  return Object.entries(detail).some(([key, value]) => (
    key.toLowerCase() === 'validation' || containsValidationError(value)
  ))
}

function isRecord(value: unknown): value is ErrorRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
