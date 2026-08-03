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
    // The public homepage and authenticated application use different layout
    // provider trees. A signed-in root visit crosses that boundary before the
    // target Page can mount, so start /rooms as a fresh document instead of
    // briefly mounting it under the landing layout.
    return navigateTo(decision.redirect, {
      replace: true,
      external: to.path === '/' && decision.redirect === '/rooms',
    })
  }
})
