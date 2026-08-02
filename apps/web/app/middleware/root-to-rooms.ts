export default defineNuxtRouteMiddleware(() => {
  return navigateTo('/rooms', { replace: true })
})
