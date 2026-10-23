import { reactive, ref } from 'vue'

type Options = { method?: string; body?: any; query?: Record<string, any>; signal?: AbortSignal; retry?: number }
type Handler = (path: string, options: Options) => unknown | Promise<unknown>
const route = reactive({ params: {} as Record<string, string>, query: {} as Record<string, string>, fullPath: '/' })
const userId = ref('player-story')
const displayName = ref('小明')
let handler: Handler = path => { throw new Error(`尚未設定頁面資料：${path}`) }
export const pageRequests: Array<{ path: string; options: Options }> = []
export const pageNavigations: unknown[] = []

export function configurePage(options: { path?: string; params?: Record<string, string>; query?: Record<string, string>; userId?: string; fetch: Handler }) {
  route.params = options.params ?? {}
  route.query = options.query ?? {}
  route.fullPath = options.path ?? '/'
  userId.value = options.userId ?? 'player-story'
  handler = options.fetch
  pageRequests.length = 0
  pageNavigations.length = 0
}
export function useRoute() { return route }
export function useRouter() { return { push: async (path: unknown) => { pageNavigations.push(path) }, replace: async (path: unknown) => { pageNavigations.push(path) } } }
export function usePlayerSession() { return { userId, displayName, refresh: async () => {}, clear: () => { userId.value = '' } } }
export function definePageMeta(_meta: unknown) {}
export async function $fetch<T = unknown>(path: string, options: Options = {}): Promise<T> {
  pageRequests.push({ path, options })
  return await handler(path, options) as T
}

export function clearError(options: { redirect: string }) { pageNavigations.push(options.redirect) }
