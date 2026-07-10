export const LOCAL_PASSWORD_RESET_ENABLED_VALUE = 'true'

export type LocalPasswordResetResult =
  | 'reset'
  | 'user-not-found'
  | 'no-credential'

export function isLocalPasswordResetEnabled(
  appEnvironment: string | undefined,
  enabled: string | undefined,
) {
  return appEnvironment === 'development'
    && enabled === LOCAL_PASSWORD_RESET_ENABLED_VALUE
}
