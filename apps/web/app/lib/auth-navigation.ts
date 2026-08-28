import { safeInternalPath } from './navigation'

export interface AuthNavigationInput {
  authenticated: boolean
  path: string
  fullPath: string
  redirect?: unknown
}

export type AuthNavigationResult = { allow: true } | { redirect: string }

const AUTHENTICATION_PATHS = new Set(['/login'])
const PUBLIC_PATHS = new Set(['/', '/login', '/privacy'])

export function resolveAuthNavigation(input: AuthNavigationInput): AuthNavigationResult {
  if (input.authenticated) {
    if (AUTHENTICATION_PATHS.has(input.path) || input.path === '/') {
      return { redirect: safeInternalPath(input.redirect) ?? '/rooms' }
    }

    return { allow: true }
  }

  if (PUBLIC_PATHS.has(input.path)) {
    return { allow: true }
  }

  const redirect = safeInternalPath(input.fullPath)
  return {
    redirect: redirect ? `/login?redirect=${encodeURIComponent(redirect)}` : '/login',
  }
}
