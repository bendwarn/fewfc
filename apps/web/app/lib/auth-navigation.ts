import { safeInternalPath } from './navigation'

export interface AuthNavigationInput {
  authenticated: boolean
  path: string
  fullPath: string
  redirect?: unknown
  resetPasswordEnabled: boolean
}

export type AuthNavigationResult = { allow: true } | { redirect: string }

const AUTHENTICATION_PATHS = new Set(['/login', '/reset-password'])

export function resolveAuthNavigation(input: AuthNavigationInput): AuthNavigationResult {
  if (input.authenticated) {
    if (AUTHENTICATION_PATHS.has(input.path) || input.path === '/') {
      return { redirect: safeInternalPath(input.redirect) ?? '/rooms' }
    }

    return { allow: true }
  }

  if (input.path === '/reset-password') {
    return input.resetPasswordEnabled ? { allow: true } : { redirect: '/login' }
  }

  if (input.path === '/login') {
    return { allow: true }
  }

  const redirect = safeInternalPath(input.fullPath)
  return {
    redirect: redirect ? `/login?redirect=${encodeURIComponent(redirect)}` : '/login',
  }
}
