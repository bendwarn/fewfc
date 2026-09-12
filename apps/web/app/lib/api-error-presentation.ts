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

const KNOWN_ERROR_MESSAGES: Record<string, string> = {
  'Invalid Rule Module configuration.': '規則版本或模組設定無效。',
  'Invalid email or password.': 'Email 或密碼錯誤。',
  'Invalid email or password': 'Email 或密碼錯誤。',
  'Invalid password.': '密碼錯誤。',
  'Invalid password': '密碼錯誤。',
  'Invalid email.': 'Email 格式無效。',
  'Invalid email': 'Email 格式無效。',
  'User already exists.': '這個 Email 已經註冊。',
  'User already exists': '這個 Email 已經註冊。',
  'Email already exists.': '這個 Email 已經註冊。',
  'Email already exists': '這個 Email 已經註冊。',
}

export function presentApiError(error: unknown, fallback: string): string {
  const apiError = error as ApiErrorLike
  const status = numericStatus(apiError?.statusCode) ?? numericStatus(apiError?.status)

  if (status !== undefined && status >= 500) return fallback

  const message = firstString(
    apiError?.data?.statusMessage,
    apiError?.data?.message,
    apiError?.data?.error,
  )

  if (!message || UNSAFE_MESSAGE_PATTERN.test(message)) return fallback
  const localized = Object.hasOwn(KNOWN_ERROR_MESSAGES, message)
    ? KNOWN_ERROR_MESSAGES[message]
    : undefined
  return localized ?? (containsChinese(message) ? message : fallback)
}

function numericStatus(value: unknown): number | undefined {
  return typeof value === 'number' && Number.isFinite(value) ? value : undefined
}

function firstString(...values: unknown[]): string | undefined {
  return values.find((value): value is string => (
    typeof value === 'string' && value.trim().length > 0
  ))?.trim()
}

function containsChinese(value: string): boolean {
  return /[\u3400-\u9fff]/u.test(value)
}
