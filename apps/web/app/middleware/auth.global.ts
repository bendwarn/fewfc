import { resolveAuthNavigation } from '~/lib/auth-navigation'

export default defineNuxtRouteMiddleware(async (to) => {
  if (import.meta.server) {
    return
  }

  const session = usePlayerSession()
  const authenticated = await session.refresh()

  const decision = resolveAuthNavigation({
    authenticated,
    path: to.path,
    fullPath: to.fullPath,
    redirect: to.query.redirect,
  })

  if ('redirect' in decision) {
    return navigateTo(decision.redirect, { replace: true })
  }
})
