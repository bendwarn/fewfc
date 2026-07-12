interface ApiErrorData {
  statusMessage?: unknown
  message?: unknown
  error?: unknown
}

interface ApiErrorLike {
  status?: unknown
  statusCode?: unknown
  data?: ApiErrorData
}

const UNSAFE_MESSAGE_PATTERN = /internal server error|server error|\[\w+\]\s+\/api\/|^[\[{]|\n\s*at\s+/i

export function presentApiError(error: unknown, fallback: string): string {
  const apiError = error as ApiErrorLike
  const status = numericStatus(apiError?.statusCode) ?? numericStatus(apiError?.status)

  if (status !== undefined && status >= 500) return fallback

  const message = firstString(
    apiError?.data?.statusMessage,
    apiError?.data?.message,
    apiError?.data?.error,
  )

  return message && !UNSAFE_MESSAGE_PATTERN.test(message) ? message : fallback
}

function numericStatus(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

function firstString(...values: unknown[]): string | undefined {
  return values.find((value): value is string => (
    typeof value === 'string' && value.trim().length > 0
  ))?.trim()
}
