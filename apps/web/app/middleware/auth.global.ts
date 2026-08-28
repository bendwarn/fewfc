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
    // 公開首頁與已驗證應用程式使用不同的版面提供者樹。已登入的根路徑造訪會
    // 在目標頁面掛載前跨越該邊界，因此以新文件啟動 /rooms，避免短暫掛載在
    // 登陸版面下。
    return navigateTo(decision.redirect, {
      replace: true,
      external: to.path === '/' && decision.redirect === '/rooms',
    })
  }
})
