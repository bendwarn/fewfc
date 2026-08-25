import {
  expect,
  test as base,
  type APIResponse,
  type Browser,
  type BrowserContext,
  type Page,
} from '@playwright/test'
import type { DevelopmentScenario } from '../../shared/development-scenarios'

const roomUrl = /\/rooms\/[0-9a-f-]+$/

async function waitForClientRoute(page: Page, navigate: () => Promise<unknown>) {
  await Promise.all([
    navigate(),
    apiText(
      'GET /api/auth/get-session for route',
      page.context().request.get('/api/auth/get-session'),
    ),
  ])
}

/**
 * Playwright 的文件導覽載入事件早於 Nuxt 水合。全域驗證中介軟體會在建立每個
 * 用戶端路由時更新工作階段，為這個由 Worker 支援的測試套件提供可觀察的路由
 * 就緒邊界，而不讓測試耦合到 Nuxt 內部實作。
 */
export async function gotoAppRoute(page: Page, path: string) {
  // 僅驗證測試會執行表單在用戶端工作階段更新後才掛載的路由。使用該路由自身
  // 可觀察的回應，避免欄位填入更新前的實例後，在重新掛載時被丟棄。
  const sessionRefresh = page.waitForResponse(response => (
    response.request().method() === 'GET'
    && new URL(response.url()).pathname === '/api/auth/get-session'
  ))
  const resetAvailability = path.startsWith('/reset-password')
    ? page.waitForResponse(response => (
      response.request().method() === 'GET'
      && new URL(response.url()).pathname === '/api/local-password-reset'
    ))
    : undefined
  await page.goto(path)
  await Promise.all([sessionRefresh, resetAvailability])
}

export async function reloadAppRoute(page: Page) {
  await waitForClientRoute(page, () => page.reload())
}

export async function activePlayerPage(pages: Page[]) {
  await expect.poll(async () => {
    const counts = await Promise.all(pages.map(page => (
      page.locator('.playing-card:enabled:not(.hidden)').count()
    )))
    return Math.max(...counts)
  }).toBeGreaterThan(0)

  for (const page of pages) {
    if (await page.locator('.playing-card:enabled:not(.hidden)').count()) {
      return page
    }
  }

  throw new Error('No active player page')
}

type CreateRoomOptions = {
  access?: 'private' | 'public'
  teamMode?: boolean
}

export async function createRoom(
  page: Page,
  roomName: string,
  { access = 'private', teamMode = false }: CreateRoomOptions = {},
) {
  await page.getByRole('button', { name: '建立房間', exact: true }).click()
  await expect(page.getByRole('dialog', { name: '建立房間' })).toBeVisible()
  await page.getByLabel('房間名稱').fill(roomName)

  if (teamMode) {
    await page.getByRole('button', { name: /團隊對戰/ }).click()
  }

  if (access === 'public') {
    await page.getByRole('button', { name: '公開房間', exact: true }).click()
  }

  await page.getByRole('button', { name: '建立房間 →' }).click()
  await expect(page).toHaveURL(roomUrl)
  const pouch = page.getByLabel('錦囊')
  if (await pouch.isChecked()) {
    await pouch.uncheck()
  }
}

export async function createPublicRoom(page: Page, roomName: string, teamMode = false) {
  await createRoom(page, roomName, { access: 'public', teamMode })
}

async function apiText(operation: string, responsePromise: Promise<APIResponse>): Promise<string> {
  const response = await responsePromise
  const body = await response.text()
  if (!response.ok()) {
    throw new Error(`${operation} failed with HTTP ${response.status()}: ${body}`)
  }
  return body
}

async function apiJson<T>(operation: string, responsePromise: Promise<APIResponse>): Promise<T> {
  const body = await apiText(operation, responsePromise)
  try {
    return JSON.parse(body) as T
  } catch {
    throw new Error(`${operation} returned invalid JSON: ${body}`)
  }
}

export async function signInAnonymously(context: BrowserContext, player: string) {
  await apiText(
    `POST /api/auth/sign-in/anonymous for ${player}`,
    context.request.post('/api/auth/sign-in/anonymous', { data: {} }),
  )
}

function gameIdFromResponse(body: { gameId?: unknown }, operation: string): string {
  if (typeof body.gameId !== 'string' || !body.gameId) {
    throw new Error(`${operation} returned no gameId: ${JSON.stringify(body)}`)
  }
  return body.gameId
}

/**
 * 供遊戲測試使用的確定性、僅 API 房間設定。只有在登入、大廳、建立、加入、
 * 準備與開始介面不是測試行為時，才會刻意略過這些介面。
 */
export type FastTwoPlayerGameOptions = {
  roomName?: string
  enabledRuleModules?: readonly string[]
  disabledRuleModules?: readonly string[]
  activeMatch?: boolean
}

export type FastTwoPlayerGame = {
  contexts: readonly [BrowserContext, BrowserContext]
  pages: readonly [Page, Page]
  hostContext: BrowserContext
  guestContext: BrowserContext
  host: Page
  guest: Page
  gameId: string
  readyGuest: () => Promise<void>
  start: () => Promise<void>
  close: () => Promise<void>
}

export type FastWaitingRoomOptions = Pick<FastTwoPlayerGameOptions,
  'roomName' | 'enabledRuleModules' | 'disabledRuleModules'
>

export type FastWaitingRoom = {
  context: BrowserContext
  page: Page
  gameId: string
  close: () => Promise<void>
}

export type FastFourPlayerGameOptions = Pick<FastTwoPlayerGameOptions,
  'roomName' | 'enabledRuleModules' | 'disabledRuleModules'
>

export type FastFourPlayerGame = {
  contexts: readonly BrowserContext[]
  pages: readonly Page[]
  gameId: string
  close: () => Promise<void>
}

const defaultFastTwoPlayerRuleModules = [
  'discard-retrieval',
  'personal-deck',
  'five-directions-legend',
  'star',
  'hero-schools',
  'spirit',
  'jianghu',
  'confluence-generation',
  'dark-glimmer',
  'echo',
  'tribulation',
] as const

const fastRuleModuleDependencies: Readonly<Record<string, readonly string[]>> = {
  spirit: ['star', 'five-directions-legend', 'hero-schools'],
  jianghu: ['star', 'five-directions-legend', 'hero-schools'],
  'confluence-generation': ['star', 'five-directions-legend', 'hero-schools'],
  'dark-glimmer': ['spirit'],
  echo: ['star', 'five-directions-legend', 'hero-schools'],
  tribulation: ['star', 'five-directions-legend', 'hero-schools'],
  pouch: ['personal-deck', 'spirit'],
}

function defaultFastRuleModules(disabledRuleModules: readonly string[]): string[] {
  let enabled = defaultFastTwoPlayerRuleModules.filter(module => !disabledRuleModules.includes(module))
  let changed = true

  while (changed) {
    changed = false
    enabled = enabled.filter((module) => {
      const dependencies = fastRuleModuleDependencies[module] ?? []
      const permitted = dependencies.every(dependency => enabled.includes(dependency))
      changed ||= !permitted
      return permitted
    })
  }

  return enabled
}

async function waitForFastGameRoute(
  page: Page,
  gameId: string,
  navigate: () => Promise<unknown>,
) {
  // 不要等待文件載入事件：它包含非必要的字型下載，會讓六個獨立玩家頁面
  // 同時進行冷載入。透過相同內容呼叫請求，避免路由的用戶端水合與測試的
  // 就緒邊界競爭；兩個回應仍能證明已登入頁面可觀察目前的遊戲。
  await Promise.all([
    navigate(),
    apiText(
      `GET /api/auth/get-session for ${gameId} route`,
      page.context().request.get('/api/auth/get-session'),
    ),
    apiText(
      `GET /api/games/${gameId} for route`,
      page.context().request.get(`/api/games/${gameId}`),
    ),
  ])
}

async function gotoFastGameRoute(page: Page, gameId: string) {
  await waitForFastGameRoute(page, gameId, () => (
    page.goto(`/rooms/${gameId}`, { waitUntil: 'domcontentloaded' })
  ))
  await expect(page.locator('.waiting-overlay')).toBeVisible()
}

/**
 * 重新連線由 setupFastTwoPlayerGame 產生的頁面，不改變一般 UI 流程輔助工具
 * 使用的完整載入語意。
 */
export async function reloadFastGameRoute(page: Page, gameId: string) {
  await waitForFastGameRoute(page, gameId, () => (
    page.reload({ waitUntil: 'domcontentloaded' })
  ))
  await expect(page.locator('.connection-dot.connected')).toHaveCount(2)
}

async function waitForActiveMatch(page: Page) {
  // 等待房間仍會顯示此覆蓋層；對局開始後，戰局紀錄的準備組會先列出本局規則。
  // 兩者一起確認頁面已消費 Active 更新，而非呈現過期的等待房間。
  await expect(page.locator('.waiting-overlay')).toBeHidden()
  await battleRecordRuleEntry(page)
}

/** 戰局紀錄的準備組是開局後向玩家說明規則配置的公開介面。 */
export async function battleRecordEntry(page: Page, title: string) {
  await expect(page.getByRole('heading', { name: '戰局紀錄' })).toBeVisible()
  const entry = page.getByRole('listitem').filter({
    has: page.getByText(title, { exact: true }),
  })
  await expect(entry).toBeVisible()
  return entry
}

/** 戰局紀錄的準備組是開局後向玩家說明規則配置的公開介面。 */
export async function battleRecordRuleEntry(page: Page) {
  return await battleRecordEntry(page, '本局規則')
}

/**
 * 供房間設定測試使用的僅 API 設定；這些測試需要一位已登入擁有者，但不會
 * 執行大廳或第二位玩家的遊戲流程。
 */
export async function setupFastWaitingRoom(
  browser: Browser,
  {
    roomName = `快速 API 等待房間 ${Date.now()}`,
    enabledRuleModules,
    disabledRuleModules = [],
  }: FastWaitingRoomOptions = {},
): Promise<FastWaitingRoom> {
  const context = await browser.newContext()
  const close = async () => {
    await context.close()
  }
  const configuredRuleModules = enabledRuleModules
    ? [...enabledRuleModules]
    : defaultFastRuleModules(disabledRuleModules)

  try {
    await signInAnonymously(context, 'room owner')
    const created = await apiJson<{ gameId?: unknown }>(
      'POST /api/games',
      context.request.post('/api/games', {
        data: {
          name: roomName,
          access: 'public',
          capacity: 2,
          enabledRuleModules: configuredRuleModules,
        },
      }),
    )
    const gameId = gameIdFromResponse(created, 'POST /api/games')
    const page = await context.newPage()
    await gotoFastGameRoute(page, gameId)
    return { context, page, gameId, close }
  } catch (error) {
    await close()
    throw error
  }
}

export async function setupFastTwoPlayerGame(
  browser: Browser,
  {
    roomName = `快速 API 房間 ${Date.now()}`,
    enabledRuleModules,
    disabledRuleModules = [],
    activeMatch = true,
  }: FastTwoPlayerGameOptions = {},
): Promise<FastTwoPlayerGame> {
  const hostContext = await browser.newContext()
  const guestContext = await browser.newContext()
  const close = async () => {
    await Promise.allSettled([hostContext.close(), guestContext.close()])
  }
  const configuredRuleModules = enabledRuleModules
    ? [...enabledRuleModules]
    : defaultFastRuleModules(disabledRuleModules)

  try {
    await Promise.all([
      signInAnonymously(hostContext, 'host'),
      signInAnonymously(guestContext, 'guest'),
    ])

    const created = await apiJson<{ gameId?: unknown }>(
      'POST /api/games',
      hostContext.request.post('/api/games', {
        data: {
          name: roomName,
          access: 'public',
          capacity: 2,
          enabledRuleModules: configuredRuleModules,
        },
      }),
    )
    const gameId = gameIdFromResponse(created, 'POST /api/games')

    await apiText(
      `POST /api/games/${gameId}/join`,
      guestContext.request.post(`/api/games/${gameId}/join`, { data: {} }),
    )

    // 每個內容的請求用戶端會共用該內容的 Cookie，因此直接導覽的頁面會以自己
    // 的玩家身分登入，不會與另一位玩家共用工作階段。準備/開始必須在此導覽
    // 後執行，因為產品協定要求兩位玩家的房間 WebSocket 連線後，任一端點才會
    // 接受請求。
    const host = await hostContext.newPage()
    const guest = await guestContext.newPage()
    // 使用三個 worker 時，這會刻意讓每場對局最多只有一個冷房間路由在處理中。
    // 同時載入兩位玩家會建立六個並行瀏覽器渲染器，比小型的主機到訪客交接更慢。
    await gotoFastGameRoute(host, gameId)
    await gotoFastGameRoute(guest, gameId)
    // 兩個 Socket 都被接受後，訪客視圖會收到已加入的房間狀態。在此等待可證明
    // 訪客已準備好呼叫 /ready，同時避免本機 Durable Object 傳遞中的過期主機
    // 廣播競爭。
    await expect(guest.locator('.connection-dot.connected')).toHaveCount(2)

    const readyGuest = async () => {
      const room = await apiJson<{ lockedDeckName?: string }>(
        `GET /api/games/${gameId} before ready`,
        guestContext.request.get(`/api/games/${gameId}`),
      )
      if (room.lockedDeckName) return
      await apiText(
        `POST /api/games/${gameId}/ready`,
        guestContext.request.post(`/api/games/${gameId}/ready`, { data: {} }),
      )
    }

    let started = false
    const start = async () => {
      if (started) return
      await readyGuest()
      await apiText(
        `POST /api/games/${gameId}/start`,
        hostContext.request.post(`/api/games/${gameId}/start`, { data: {} }),
      )
      await Promise.all([
        waitForActiveMatch(host),
        waitForActiveMatch(guest),
      ])
      started = true
    }

    if (activeMatch) {
      await start()
    }

    return {
      contexts: [hostContext, guestContext],
      pages: [host, guest],
      hostContext,
      guestContext,
      host,
      guest,
      gameId,
      readyGuest,
      start,
      close,
    }
  } catch (error) {
    await close()
    throw error
  }
}

/**
 * 供作用中四人團隊遊戲使用的僅 API 設定。每位玩家都保有獨立的 BrowserContext
 * 與自己的請求 Cookie 罐。
 */
export async function setupFastFourPlayerGame(
  browser: Browser,
  {
    roomName = `快速 API 四人房間 ${Date.now()}`,
    enabledRuleModules,
    disabledRuleModules = [],
  }: FastFourPlayerGameOptions = {},
): Promise<FastFourPlayerGame> {
  const contexts = await Promise.all(Array.from({ length: 4 }, () => browser.newContext()))
  const close = async () => {
    await Promise.allSettled(contexts.map(context => context.close()))
  }
  const configuredRuleModules = enabledRuleModules
    ? [...enabledRuleModules]
    : defaultFastRuleModules(disabledRuleModules)

  try {
    await Promise.all(contexts.map((context, index) => (
      signInAnonymously(context, `four-player ${index + 1}`)
    )))
    const hostContext = contexts[0]!
    const created = await apiJson<{ gameId?: unknown }>(
      'POST /api/games for four-player game',
      hostContext.request.post('/api/games', {
        data: {
          name: roomName,
          access: 'public',
          capacity: 4,
          enabledRuleModules: configuredRuleModules,
        },
      }),
    )
    const gameId = gameIdFromResponse(created, 'POST /api/games for four-player game')

    for (const guestContext of contexts.slice(1)) {
      await apiText(
        `POST /api/games/${gameId}/join for four-player guest`,
        guestContext.request.post(`/api/games/${gameId}/join`, { data: {} }),
      )
    }

    const pages = await Promise.all(contexts.map(context => context.newPage()))
    for (const page of pages) {
      await gotoFastGameRoute(page, gameId)
    }

    for (const guestContext of contexts.slice(1)) {
      await apiText(
        `POST /api/games/${gameId}/ready for four-player guest`,
        guestContext.request.post(`/api/games/${gameId}/ready`, { data: {} }),
      )
    }
    await apiText(
      `POST /api/games/${gameId}/start for four-player host`,
      hostContext.request.post(`/api/games/${gameId}/start`, { data: {} }),
    )
    await Promise.all(pages.map(waitForActiveMatch))

    return { contexts, pages, gameId, close }
  } catch (error) {
    await close()
    throw error
  }
}

export async function joinListedRoom(
  page: Page,
  roomName: string,
  expectedRuleText?: string,
) {
  const room = page.locator('.public-room-list button').filter({ hasText: roomName })
  if (expectedRuleText) {
    await expect(room).toContainText(expectedRuleText)
  }
  await room.click()
  await expect(page).toHaveURL(roomUrl)
}

export async function seedDevelopmentScenario<T = unknown>(
  page: Page,
  scenario: DevelopmentScenario,
): Promise<T> {
  const gameId = new URL(page.url()).pathname.split('/').at(-1)
  if (!gameId) {
    throw new Error(`Could not seed development scenario without a game route: ${page.url()}`)
  }
  return await apiJson<T>(
    `POST /api/games/${gameId}/test-scenarios`,
    page.context().request.post(`/api/games/${gameId}/test-scenarios`, { data: scenario }),
  )
}

export async function startTwoPlayerMatch(
  host: Page,
  guest: Page,
  roomName: string,
  configure?: (host: Page) => Promise<void>,
): Promise<string> {
  await createPublicRoom(host, roomName)
  await configure?.(host)
  await joinListedRoom(guest, roomName)
  await guest.getByRole('button', { name: '準備 →' }).click()
  await host.getByRole('button', { name: '開始遊戲 →' }).click()
  await battleRecordRuleEntry(host)
  await battleRecordRuleEntry(guest)
  return new URL(host.url()).pathname.split('/').at(-1) ?? ''
}

export { expect }

/**
 * 供主題不是登入的測試使用的單一已登入玩家頁面。該頁面仍會執行產品的大廳與
 * 房間 UI，而其隔離的 BrowserContext 會透過與 setupFastTwoPlayerGame 相同的
 * 請求/Cookie 路徑取得工作階段。
 */
export const fastPageTest = base.extend({
  page: async ({ browser }, use) => {
    const context = await browser.newContext()
    try {
      await signInAnonymously(context, 'fast page')
      const page = await context.newPage()
      await gotoAppRoute(page, '/rooms')
      await expect(page.getByRole('heading', { name: '房間', exact: true })).toBeVisible()
      await use(page)
    } finally {
      await context.close()
    }
  },
})

export const test = base.extend({
  page: async ({ browser }, use) => {
    const context = await browser.newContext()
    try {
      await signInAnonymously(context, 'default page')
      const page = await context.newPage()
      await gotoAppRoute(page, '/rooms')
      await expect(page.getByRole('heading', { name: '房間', exact: true })).toBeVisible()
      await use(page)
    } finally {
      await context.close()
    }
  },
})
