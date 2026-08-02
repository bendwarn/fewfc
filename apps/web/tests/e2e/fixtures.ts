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
 * Playwright's document-navigation load event precedes Nuxt hydration. The
 * global auth middleware refreshes the session while establishing every client
 * route, giving this Worker-backed suite an observable route-ready boundary
 * without coupling tests to Nuxt internals.
 */
export async function gotoAppRoute(page: Page, path: string) {
  // Auth-only tests exercise a route whose form mounts after the client-side
  // session refresh. Use that route's own observable response so fields are
  // not filled into the pre-refresh instance and then discarded on remount.
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
 * A deterministic API-only room setup for gameplay tests. It deliberately
 * bypasses login, lobby, create, join, ready, and start UI only when those
 * controls are not the behavior under test.
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
  // Do not wait for the document load event: it includes nonessential font
  // downloads and turns six independent player pages into a cold-load herd.
  // Request through the same context avoids making the route's client-side
  // hydration compete with the test's ready boundary; both responses still
  // prove the signed-in page can observe its current game.
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
 * Reconnect a page produced by setupFastTwoPlayerGame without changing the
 * full-load semantics used by ordinary UI flow helpers.
 */
export async function reloadFastGameRoute(page: Page, gameId: string) {
  await waitForFastGameRoute(page, gameId, () => (
    page.reload({ waitUntil: 'domcontentloaded' })
  ))
  await expect(page.locator('.connection-dot.connected')).toHaveCount(2)
}

async function waitForActiveMatch(page: Page) {
  // Unlike the enabled-rules event, this overlay is present while the room is
  // still waiting. Its disappearance therefore proves that the page consumed
  // the Active room update rather than merely rendering stale game data.
  await expect(page.locator('.waiting-overlay')).toBeHidden()
  await expect(page.getByRole('region', { name: '啟用規則' })).toBeVisible()
}

/**
 * API-only setup for room-configuration tests that need one signed-in owner,
 * but do not exercise the lobby or a second player's gameplay.
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

    // Each context's request client shares that context's cookies, so the
    // directly navigated page begins signed in as its own player without
    // sharing a session with the other player. Ready/start must follow this
    // navigation because the product contract requires both players' room
    // WebSockets to be connected before either endpoint accepts the request.
    const host = await hostContext.newPage()
    const guest = await guestContext.newPage()
    // With three workers this intentionally keeps at most one cold room route
    // per match in flight. Loading both players together creates six concurrent
    // browser renderers and is slower than the small host-to-guest handoff.
    await gotoFastGameRoute(host, gameId)
    await gotoFastGameRoute(guest, gameId)
    // The guest view receives the joined room-state after both sockets are
    // accepted. Waiting there proves the guest is ready to call /ready while
    // avoiding a stale host broadcast race in local Durable Object delivery.
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
 * API-only setup for an active four-player team game. Every player retains a
 * distinct BrowserContext and its own request cookie jar.
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
  await expect(host.getByRole('region', { name: '啟用規則' })).toBeVisible()
  await expect(guest.getByRole('region', { name: '啟用規則' })).toBeVisible()
  return new URL(host.url()).pathname.split('/').at(-1) ?? ''
}

export { expect }

/**
 * A signed-in one-player page for tests whose subject is not login. The page
 * still exercises the product's lobby and room UI, while its own isolated
 * BrowserContext receives the session through the same request/cookie path as
 * setupFastTwoPlayerGame.
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
