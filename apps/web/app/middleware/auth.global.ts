import { resolveAuthNavigation } from '~/lib/auth-navigation'

export default defineNuxtRouteMiddleware(async (to) => {
  if (import.meta.server) {
    return
  }

  const session = usePlayerSession()
  const authenticated = await session.refresh()
  let resetPasswordEnabled = false

  if (!authenticated && to.path === '/reset-password') {
    try {
      resetPasswordEnabled = (await $fetch<{ enabled: boolean }>('/api/local-password-reset')).enabled
    } catch {
      resetPasswordEnabled = false
    }
  }

  const decision = resolveAuthNavigation({
    authenticated,
    path: to.path,
    fullPath: to.fullPath,
    redirect: to.query.redirect,
    resetPasswordEnabled,
  })

  if ('redirect' in decision) {
    return navigateTo(decision.redirect, { replace: true })
  }
})
