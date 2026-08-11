type ErrorRecord = Record<string, unknown>

export class RulesEngineError extends Error {
  readonly statusCode: 400 | 500
  readonly code: string

  constructor(readonly detail: unknown) {
    const validation = validationErrorCode(detail)
    super(validation ? '選擇無效，請重新選擇。' : '規則引擎處理失敗，請稍後再試。')
    this.name = 'RulesEngineError'
    this.statusCode = validation ? 400 : 500
    this.code = validation ?? 'rulesEngineFailure'
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

function validationErrorCode(detail: unknown): string | undefined {
  if (!isRecord(detail)) return undefined

  for (const [key, value] of Object.entries(detail)) {
    if (key.toLowerCase() === 'validation') {
      return firstVariantName(value) ?? 'rulesValidation'
    }
    const nested = validationErrorCode(value)
    if (nested) return nested
  }
  return undefined
}

function firstVariantName(detail: unknown): string | undefined {
  if (!isRecord(detail)) return undefined
  return Object.keys(detail)[0]
}

function isRecord(value: unknown): value is ErrorRecord {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}
