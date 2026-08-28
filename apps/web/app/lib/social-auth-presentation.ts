export const SOCIAL_AUTH_FAILURE_MESSAGE = '社群登入或連結未完成，請再試一次。'

export function socialAuthFailureMessage(error: unknown): string | undefined {
  return typeof error === 'string' && error.trim()
    ? SOCIAL_AUTH_FAILURE_MESSAGE
    : undefined
}
