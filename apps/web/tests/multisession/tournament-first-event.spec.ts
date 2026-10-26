import type { Browser, BrowserContext, Page } from '@playwright/test'
import type { CompetitionAnnouncementPlan } from '../../shared/competition'
import { port } from '../../scripts/local-ports'
import {
  expect,
  gotoAppRoute,
  reloadAppRoute,
  signInAnonymously,
  test,
} from '../e2e/fixtures'
import {
  parseCompetitionTournamentViewResponse,
  type CompetitionTournamentViewData,
} from '../../app/lib/competition-tournament-view'
import { parseCompetitionRegistrationResponse } from '../../app/lib/competition-registration'

const pouchRuleModules = [
  'personal-deck',
  'five-directions-legend',
  'star',
  'hero-schools',
  'spirit',
  'pouch',
] as const

const stageFormats = [
  'roundRobin',
  'swiss',
  'singleElimination',
  'doubleElimination',
] as const

function topTwo(sourceStageId: string) {
  return [1, 2].map(rank => ({
    sourceStageId,
    source: { type: 'uniqueStandingRank' as const, rank },
    destinationSeed: rank,
  }))
}

type Player = {
  context: BrowserContext
  page: Page
  userId: string
  waitForGameCommandResponse: () => Promise<unknown>
}

type SocketMessage = {
  type?: unknown
  requestId?: unknown
  operation?: unknown
  ok?: unknown
  data?: unknown
  error?: unknown
}

type SocketResponseWaiter = {
  resolve: (value: unknown) => void
  reject: (error: Error) => void
  timer: ReturnType<typeof setTimeout>
}

type CompetitionCreated = {
  competitionId: string
  registrationDeadline: number
  checkInDeadline: number
}

async function until<T>(read: () => Promise<T | null>, timeoutMs: number, label: string): Promise<T> {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const value = await read()
    if (value !== null) return value
    await new Promise(resolve => setTimeout(resolve, 1_000))
  }
  throw new Error(`${label} was not ready within ${timeoutMs} ms.`)
}

function announcementPlan(): CompetitionAnnouncementPlan {
  return {
    initialSeedPolicy: 'randomDrawAfterRegistrationDeadline',
    swissPairingPolicy: 'publishedRulesWithPersistedRandomSeed',
    rankingPolicy: 'matchPointsThenOpponentWinsThenOpponentLosses',
    exactRankingTiePolicy: 'organizerOrUnconflictedRefereeWithPublicReason',
    stages: [
      {
        stageId: 'first-round-robin',
        format: 'roundRobin',
        matchFormat: { type: 'fixedGames', value: 3 },
        clockConfig: { mainTimeMs: 0, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: null,
        swissPolicy: null,
        qualification: { type: 'allEligibleTeams' },
      },
      {
        stageId: 'second-swiss',
        format: 'swiss',
        matchFormat: { type: 'fixedGames', value: 1 },
        clockConfig: { mainTimeMs: 0, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: 1,
        swissPolicy: {
          matchPoints: { win: 3, draw: 1, loss: 0, bye: 0 },
          drawCredit: { numerator: 1, denominator: 2 },
          opponentWinRateFloorPercent: null,
          opponentWinRateCapPercent: null,
          excludeByesFromOpponentWinRate: true,
          tieBreakers: ['matchPoints', 'seedOrder'],
        },
        qualification: { type: 'sourceSlots', slots: topTwo('first-round-robin') },
      },
      {
        stageId: 'third-single-elimination',
        format: 'singleElimination',
        matchFormat: { type: 'firstToWins', value: 1 },
        clockConfig: { mainTimeMs: 0, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: null,
        swissPolicy: null,
        qualification: { type: 'sourceSlots', slots: topTwo('second-swiss') },
      },
      {
        stageId: 'fourth-double-elimination',
        format: 'doubleElimination',
        matchFormat: { type: 'firstToWins', value: 1 },
        clockConfig: { mainTimeMs: 0, actionTimeMs: 30_000, outOfTurnChoiceTimeMs: 30_000 },
        swissRounds: null,
        swissPolicy: null,
        qualification: { type: 'sourceSlots', slots: topTwo('second-swiss') },
      },
    ],
  }
}

async function responseJson<T>(operation: string, responsePromise: Promise<import('@playwright/test').APIResponse>): Promise<T> {
  const response = await responsePromise
  const body = await response.text()
  if (!response.ok()) throw new Error(`${operation} failed with HTTP ${response.status()}: ${body}`)
  try {
    return JSON.parse(body) as T
  } catch {
    throw new Error(`${operation} returned invalid JSON: ${body}`)
  }
}

function parseSocketMessage(payload: string | Buffer): SocketMessage | null {
  try {
    const value: unknown = JSON.parse(typeof payload === 'string' ? payload : payload.toString('utf8'))
    return value && typeof value === 'object' && !Array.isArray(value) ? value as SocketMessage : null
  } catch {
    return null
  }
}

function trackGameCommandResponses(page: Page) {
  const queued: SocketResponseWaiter[] = []
  const pending = new Map<string, SocketResponseWaiter>()

  page.on('websocket', socket => {
    if (!new URL(socket.url()).pathname.startsWith('/api/games/')) return

    socket.on('framesent', event => {
      const message = parseSocketMessage(event.payload)
      if (message?.type !== 'request' || message.operation !== 'command'
        || typeof message.requestId !== 'string') return
      const waiter = queued.shift()
      if (waiter) pending.set(message.requestId, waiter)
    })

    socket.on('framereceived', event => {
      const message = parseSocketMessage(event.payload)
      if (message?.type !== 'result' || typeof message.requestId !== 'string') return
      const waiter = pending.get(message.requestId)
      if (!waiter) return
      pending.delete(message.requestId)
      clearTimeout(waiter.timer)
      if (message.ok === true) {
        waiter.resolve(message.data)
      } else {
        const error = message.error && typeof message.error === 'object'
          ? (message.error as { message?: unknown }).message
          : null
        waiter.reject(new Error(typeof error === 'string' ? error : 'Game command failed over WebSocket.'))
      }
    })
  })

  return () => new Promise<unknown>((resolve, reject) => {
    const waiter = {
      resolve,
      reject,
      timer: setTimeout(() => {
        const index = queued.indexOf(waiter)
        if (index >= 0) queued.splice(index, 1)
        for (const [requestId, current] of pending) {
          if (current === waiter) pending.delete(requestId)
        }
        reject(new Error('Timed out waiting for a game command WebSocket response.'))
      }, 20_000),
    }
    queued.push(waiter)
  })
}

async function createPlayer(browser: Browser, name: string): Promise<Player> {
  const context = await browser.newContext({ baseURL: `http://localhost:${port('FEWFC_E2E_PORT', 8727)}` })
  try {
    await signInAnonymously(context, name)
    const session = await responseJson<{ user?: { id?: unknown } }>(
      `GET /api/auth/get-session for ${name}`,
      context.request.get('/api/auth/get-session'),
    )
    if (typeof session.user?.id !== 'string' || !session.user.id) throw new Error(`${name} has no session user id`)
    const page = await context.newPage()
    return { context, page, userId: session.user.id, waitForGameCommandResponse: trackGameCommandResponses(page) }
  } catch (error) {
    await context.close()
    throw error
  }
}

async function createCompetition(organizer: Player): Promise<CompetitionCreated> {
  const registrationDeadline = (Math.floor(Date.now() / 60_000) + 2) * 60_000
  const checkInDeadline = registrationDeadline + 6 * 60_000
  const created = await responseJson<{ competitionId?: unknown }>(
    'POST /api/competitions',
    organizer.context.request.post('/api/competitions', {
      data: {
        ruleVersion: '5.17',
        ruleset: 'base',
        enabledRuleModules: pouchRuleModules,
        teamSize: 1,
        registrationMode: 'public',
        personalDecks: 'required',
        registrationDeadline,
        checkInDeadline,
        plan: announcementPlan(),
      },
    }),
  )
  if (typeof created.competitionId !== 'string' || !created.competitionId) {
    throw new Error(`POST /api/competitions returned no competition id: ${JSON.stringify(created)}`)
  }
  return { competitionId: created.competitionId, registrationDeadline, checkInDeadline }
}

async function registerIndividual(player: Player, competitionId: string) {
  await gotoAppRoute(player.page, `/competitions/${encodeURIComponent(competitionId)}`)
  await expect(player.page.getByRole('button', { name: '登記隊伍', exact: true })).toBeVisible()
  const registrationResponse = player.page.waitForResponse(response => {
    if (response.request().method() !== 'POST'
      || new URL(response.url()).pathname !== `/api/competitions/${competitionId}/commands`) return false
    try { return response.request().postDataJSON()?.operation?.type === 'registrationExecute' } catch { return false }
  })
  await player.page.getByRole('button', { name: '登記隊伍', exact: true }).click()
  const registered = await registrationResponse
  const registrationBody = await registered.text()
  if (!registered.ok()) {
    throw new Error(`Team registration failed with HTTP ${registered.status()}: ${registrationBody}`)
  }
  const registration = parseCompetitionRegistrationResponse(
    JSON.parse(registrationBody), competitionId, undefined, player.userId,
  )
  expect(registration.initialHp).toBe(20)
  if (!registration.ownTeam) {
    throw new Error(`Team registration response did not include the caller's team: ${registrationBody}`)
  }
  const ownTeam = player.page.locator('.team-card').filter({ hasText: registration.ownTeam.teamId })
  await expect(ownTeam).toBeVisible()
  await ownTeam.getByRole('button', { name: '確認本人隊員身分', exact: true }).click()
  await expect(player.page.getByRole('button', { name: '提交目前帳號牌組作為賽事快照', exact: true })).toBeEnabled()
  await player.page.getByRole('button', { name: '提交目前帳號牌組作為賽事快照', exact: true }).click()
  await expect(player.page.getByText(
    '目前賽事已保存本人牌組快照。下方帳號牌組可能已更新；兩者是不同資料。',
    { exact: true },
  )).toBeVisible()
  await expect(player.page.getByRole('button', { name: '隊長資料完成確認', exact: true })).toBeVisible()
  await player.page.getByRole('button', { name: '隊長資料完成確認', exact: true }).click()
  await expect(ownTeam.locator('.team-status')).toContainText('資料完成')
}

async function tournamentView(context: BrowserContext, competitionId: string): Promise<CompetitionTournamentViewData> {
  const payload = await responseJson<unknown>(
    `GET /api/competitions/${competitionId}/tournament`,
    context.request.get(`/api/competitions/${encodeURIComponent(competitionId)}/tournament`),
  )
  return parseCompetitionTournamentViewResponse(payload, competitionId)
}

function allMatches(view: CompetitionTournamentViewData) {
  return view.stages.flatMap(stage => stage.rounds.flatMap(round => round.matches))
}

async function enterTournamentMatch(
  player: Player,
  competitionId: string,
  matchId: string,
  linkTimeoutMs = 8 * 60_000,
) {
  await gotoAppRoute(player.page, `/competitions/${encodeURIComponent(competitionId)}`)
  await expect(player.page.locator('.lifecycle-badge')).toHaveText('名單已鎖定')
  await expect.poll(async () => (
    await player.page.locator('.match-card').filter({ hasText: matchId })
      .getByRole('link', { name: '進入競賽對局', exact: true }).count()
  ), { timeout: linkTimeoutMs }).toBe(1)
  await player.page.locator('.match-card').filter({ hasText: matchId })
    .getByRole('link', { name: '進入競賽對局', exact: true }).click()
  await expect(player.page).toHaveURL(/\/rooms\/[^/]+$/u)
  await expect(player.page.locator('.competition-ready')).toBeVisible({ timeout: 30_000 })
  await expect(player.page.getByRole('button', { name: '我準備好了', exact: true }))
    .toBeEnabled({ timeout: 60_000 })
}

async function waitForInitialPouchSelection(players: readonly Player[]) {
  await Promise.all(players.map(async player => {
    const dialog = player.page.getByRole('dialog', { name: '選擇初始錦囊' })
    await expect(dialog).toBeVisible({ timeout: 90_000 })
    await expect(dialog.getByRole('button', { name: /作為初始錦囊/ }).first()).toBeVisible()
  }))
}

async function chooseInitialPouch(players: readonly Player[]) {
  await waitForInitialPouchSelection(players)
  await Promise.all(players.map(async player => {
    await player.page.getByRole('dialog', { name: '選擇初始錦囊' })
      .getByRole('button', { name: /作為初始錦囊/ }).first().click()
    await expect(player.page.getByRole('dialog', { name: '選擇初始錦囊' })).toHaveCount(0)
  }))
}

async function selectFirstPlayableMainAction(player: Player) {
  const page = player.page
  const cards = page.locator('.playing-card:enabled:not(.hidden)')
  const actionCandidates = page.locator('.action-panel .action-candidates button:not(.skip-action):enabled')
  const pass = page.locator('.action-panel .action-candidates button.skip-action:enabled')

  if (await actionCandidates.count()) return actionCandidates.first()
  if (await pass.count()) return pass

  for (let index = 0; index < await cards.count(); index += 1) {
    const card = cards.nth(index)
    await expect(card).toBeVisible({ timeout: 10_000 })
    const responsePromise = player.waitForGameCommandResponse()
    await card.click({ timeout: 10_000 })
    await expect(card).toHaveAttribute('aria-pressed', 'true')
    const playableActions = await responsePromise as { playableActions?: Array<{ commandRole?: unknown; type?: unknown }> }
    const actions = playableActions.playableActions
    if (!Array.isArray(actions)) throw new Error('The player action response did not include playable actions.')
    if (actions.some(action => action.commandRole === 'action' && action.type !== 'pass')) {
      await expect(actionCandidates.first()).toBeVisible({ timeout: 15_000 })
      return actionCandidates.first()
    }
    if (await pass.count()) return pass

    const selected = page.locator('.playing-card[aria-pressed="true"]')
    if (await selected.count()) {
      const deselectionResponse = player.waitForGameCommandResponse()
      await selected.first().click()
      const deselected = await deselectionResponse as { playableActions?: Array<{ commandRole?: unknown; type?: unknown }> }
      if (deselected.playableActions?.some(action => action.commandRole === 'action' && action.type !== 'pass')) {
        await expect(actionCandidates.first()).toBeVisible({ timeout: 15_000 })
        return actionCandidates.first()
      }
      if (await pass.count()) return pass
    }
  }

  return null
}

async function resolveVisibleChoice(player: Player) {
  const page = player.page
  const overlay = page.locator('.choice-overlay').filter({ has: page.locator('h2') }).last()
  if (!await overlay.isVisible()) return

  const cards = overlay.locator('.choice-cards button:enabled')
  const options = overlay.locator('.choice-options button:enabled')
  await expect.poll(async () => await cards.count() + await options.count(), {
    timeout: 15_000,
  }).toBeGreaterThan(0)
  const cardCount = await cards.count()
  const optionCount = await options.count()

  const submit = overlay.getByRole('button', { name: '確認選擇', exact: true })
  const needsSubmit = await submit.count() > 0
  const command = player.waitForGameCommandResponse()
  if (cardCount > 0) await cards.first().click()
  else await options.first().click()
  if (needsSubmit) {
    while (!await submit.isEnabled()) {
      const selectable = overlay.locator('.choice-cards button:enabled:not([aria-pressed="true"])')
      if (!await selectable.count()) break
      await selectable.first().click()
    }
    if (await submit.isEnabled()) await submit.click()
  }
  await command
}

async function finishGameThroughPlayerActions(players: readonly Player[]) {
  const maximumTurns = 120
  for (let turn = 0; turn < maximumTurns; turn += 1) {
    if ((await Promise.all(players.map(player => player.page.locator('.result-panel').isVisible()))).some(Boolean)) return

    let chooser: Player | undefined
    for (const player of players) {
      if (await player.page.locator('.choice-overlay').isVisible()) {
        chooser = player
        break
      }
    }
    if (chooser) {
      await resolveVisibleChoice(chooser)
      continue
    }

    let activeState = ''
    await expect.poll(async () => {
      if ((await Promise.all(players.map(player => player.page.locator('.result-panel').isVisible()))).some(Boolean)) {
        activeState = 'finished'
        return activeState
      }
      const available = await Promise.all(players.map(player => (
        player.page.locator('.playing-card:enabled:not(.hidden)').count()
      )))
      const index = available.findIndex(count => count > 0)
      activeState = index < 0 ? '' : String(index)
      return activeState
    }, { timeout: 60_000 }).not.toBe('')
    if (activeState === 'finished') return
    const active = players[Number(activeState)]
    if (!active) throw new Error('The game has no active player or visible choice.')

    const action = await selectFirstPlayableMainAction(active)
    if (!action) throw new Error('No selectable card offered a legal main action or pass.')
    const attack = active.page.locator('.action-panel .action-candidates button[title*="進行五行攻擊"]:enabled').first()
    const chosenAction = await attack.count() ? attack : action
    const command = active.waitForGameCommandResponse()
    await chosenAction.click()
    await command
    await expect.poll(async () => (
      (await Promise.all(players.map(player => player.page.locator('.result-panel').isVisible()))).some(Boolean)
        || (await Promise.all(players.map(player => player.page.locator('.choice-overlay').isVisible()))).some(Boolean)
        || (await Promise.all(players.map(player => player.page.locator('.playing-card:enabled:not(.hidden)').count() > 0))).some(Boolean)
    ), { timeout: 20_000 }).toBe(true)
    for (const player of players) await resolveVisibleChoice(player)
  }
  throw new Error(`The game did not finish after ${maximumTurns} ordinary player turns.`)
}

async function createOrdinaryNextGame(
  players: readonly Player[],
  competitionId: string,
  matchId: string,
  gameNumber: number,
) {
  await Promise.all(players.map(async player => {
    await gotoAppRoute(player.page, `/competitions/${encodeURIComponent(competitionId)}`)
    const refresh = player.page.locator('.match-card').filter({ hasText: matchId })
      .getByRole('button', { name: '重新查詢', exact: true })
    if (await refresh.count()) await refresh.click()
  }))
  await expect.poll(async () => {
    const response = await players[0]!.context.request.get(
      `/api/competitions/${encodeURIComponent(competitionId)}/matches/${encodeURIComponent(matchId)}`,
    )
    const body = await response.json().catch(() => null) as {
      matchRevision?: unknown
      matchProjection?: { public?: { match?: {
        status?: unknown
        games?: unknown
        nextGame?: { gameNumber?: unknown; firstPlayer?: { type?: unknown } } | null
      } } }
    } | null
    const match = body?.matchProjection?.public?.match
    return {
      httpStatus: response.status(),
      matchStatus: match?.status ?? null,
      matchRevision: body?.matchRevision ?? null,
      gameCount: Array.isArray(match?.games) ? match.games.length : null,
      nextGameNumber: match?.nextGame?.gameNumber ?? null,
      firstPlayerType: match?.nextGame?.firstPlayer?.type ?? null,
    }
  }, { timeout: 60_000 }).toMatchObject({
    httpStatus: 200,
    matchStatus: 'awaitingFirstPlayer',
    gameCount: gameNumber - 1,
    nextGameNumber: gameNumber,
    firstPlayerType: 'pending',
  })
  let captainIndex = -1
  await expect.poll(async () => {
    for (const player of players) {
      if (await player.page.getByRole('button', { name: '確認先手並建立下一局', exact: true }).count()) {
        captainIndex = players.indexOf(player)
        return captainIndex
      }
    }
    return -1
  }, { timeout: 60_000 }).toBeGreaterThanOrEqual(0)
  const captain = players[captainIndex]
  if (!captain) throw new Error('The losing team captain did not receive the next-game choice.')
  const firstPlayerInputs = captain.page.getByRole('radio', { name: /甲隊|乙隊/u })
  await expect(firstPlayerInputs.first()).toBeVisible()
  await firstPlayerInputs.first().check()
  await captain.page.getByRole('button', { name: '確認先手並建立下一局', exact: true }).click()
  await expect.poll(async () => (
    await captain.page.getByRole('link', { name: '進入競賽對局', exact: true }).count()
  ), { timeout: 90_000 }).toBe(1)

  await Promise.all(players.map(player => enterTournamentMatch(player, competitionId, matchId, 90_000)))
  await Promise.all(players.map(async player => {
    await expect(player.page.getByRole('button', { name: '我準備好了', exact: true })).toBeVisible()
    await player.page.getByRole('button', { name: '我準備好了', exact: true }).click()
  }))
  await chooseInitialPouch(players)
  return new URL(players[0]!.page.url()).pathname.split('/').at(-1)!
}

async function nextStageWork(
  player: Player,
  competitionId: string,
  stageId: string,
): Promise<{ type: 'finished' } | { type: 'match'; matchId: string }> {
  return until(async () => {
    const view = await tournamentView(player.context, competitionId).catch(() => null)
    const stage = view?.stages.find(candidate => candidate.config.stageId === stageId)
    if (!stage) return null
    if (stage.status === 'finished') return { type: 'finished' as const }
    if (stage.hasScheduleWarning) throw new Error(`Stage ${stageId} has a schedule warning.`)
    const match = stage.rounds.flatMap(round => round.matches)
      .find(candidate => candidate.result === null && candidate.participants.length === 2)
    return match ? { type: 'match' as const, matchId: match.matchId } : null
  }, 90_000, `A real match or automatic closure in stage ${stageId}`)
}

async function ordinaryNextGameNumber(
  player: Player,
  competitionId: string,
  matchId: string,
): Promise<number | null> {
  const response = await player.context.request.get(
    `/api/competitions/${encodeURIComponent(competitionId)}/matches/${encodeURIComponent(matchId)}`,
  )
  if (!response.ok()) return null
  const body = await response.json().catch(() => null) as {
    matchProjection?: { public?: { match?: {
      status?: unknown
      games?: unknown
      nextGame?: { gameNumber?: unknown; firstPlayer?: { type?: unknown } } | null
    } } }
  } | null
  const match = body?.matchProjection?.public?.match
  if (match?.status !== 'awaitingFirstPlayer' || !Array.isArray(match.games)
    || match.nextGame?.firstPlayer?.type !== 'pending'
    || match.nextGame.gameNumber !== match.games.length + 1) return null
  return match.nextGame.gameNumber
}

async function finishScheduledMatch(
  players: readonly Player[],
  competitionId: string,
  matchId: string,
) {
  await Promise.all(players.map(player => enterTournamentMatch(player, competitionId, matchId)))
  await Promise.all(players.map(async player => {
    await expect(player.page.getByRole('button', { name: '我準備好了', exact: true })).toBeVisible()
    await player.page.getByRole('button', { name: '我準備好了', exact: true }).click()
  }))
  await chooseInitialPouch(players)

  for (let gameNumber = 1; gameNumber <= 8; gameNumber += 1) {
    await finishGameThroughPlayerActions(players)
    await Promise.all(players.map(player => expect(player.page.locator('.result-panel')).toBeVisible()))
    const next = await until(async () => {
      const view = await tournamentView(players[0]!.context, competitionId).catch(() => null)
      const match = view && allMatches(view).find(candidate => candidate.matchId === matchId)
      if (!match) return null
      if (match.result !== null || match.status === 'completed') return { type: 'finished' as const }
      const gameNumber = await ordinaryNextGameNumber(players[0]!, competitionId, matchId)
      if (gameNumber !== null) return { type: 'nextGame' as const, gameNumber }
      return null
    }, 90_000, `A result or ordinary continuation for match ${matchId}`)
    if (next.type === 'finished') return
    await createOrdinaryNextGame(players, competitionId, matchId, next.gameNumber)
  }
  throw new Error(`Match ${matchId} did not finish within eight game results.`)
}

test('主辦離線後，玩家多 session 從 F 自動凍結並完成賽事', async ({ browser }) => {
  // 淘汰輪之間依正式規則保留兩段五分鐘窗口；完整雙敗總決賽可能再多一輪。
  test.setTimeout(60 * 60_000)
  const organizer = await createPlayer(browser, 'ticket 22 organizer')
  let organizerContextOpen = true
  const firstPlayer = await createPlayer(browser, 'ticket 22 first player')
  const secondPlayer = await createPlayer(browser, 'ticket 22 second player')
  const players = [firstPlayer, secondPlayer] as const
  let competitionId: string | null = null

  try {
    const created = await createCompetition(organizer)
    competitionId = created.competitionId
    await gotoAppRoute(organizer.page, `/competitions/${encodeURIComponent(competitionId)}`)
    await organizer.page.getByRole('button', { name: '開放報名', exact: true }).click()
    await organizer.context.close()
    organizerContextOpen = false

    const unfinishedPublicRead = await firstPlayer.context.request.get(
      `/api/competitions/${encodeURIComponent(competitionId)}/tournament`,
    )
    expect(unfinishedPublicRead.ok()).toBe(false)

    await registerIndividual(firstPlayer, competitionId)
    await registerIndividual(secondPlayer, competitionId)

    await expect.poll(async () => {
      const view = await responseJson<{ projection?: { public?: { lifecycle?: string } } }>(
        'GET competition registration',
        firstPlayer.context.request.post(`/api/competitions/${encodeURIComponent(competitionId!)}/commands`, {
          data: {
            commandId: `ticket22-view-${crypto.randomUUID()}`,
            expectedRevision: 1,
            operation: { type: 'registrationView' },
          },
        }),
      )
      return view.projection?.public?.lifecycle ?? ''
    }, { timeout: Math.max(180_000, created.registrationDeadline - Date.now() + 90_000), intervals: [1_000, 3_000, 5_000] })
      .toBe('locked')

    await Promise.all(players.map(player => reloadAppRoute(player.page)))
    await Promise.all(players.map(player => expect(player.page.locator('.lifecycle-badge')).toHaveText('名單已鎖定')))

    const initialSchedule = await until(async () => {
      try { return await tournamentView(firstPlayer.context, competitionId) } catch { return null }
    }, 90_000, 'The automatic initial tournament schedule')
    expect(initialSchedule.stages.map(stage => stage.config.format)).toEqual(['roundRobin'])
    expect(initialSchedule.stages[0]?.config.teams).toHaveLength(2)
    const firstMatch = allMatches(initialSchedule).find(match => match.stageId === 'first-round-robin')
    expect(firstMatch).toBeDefined()
    expect(firstMatch?.participants).toHaveLength(2)
    expect(firstMatch?.status).toMatch(/pending|inProgress/u)

    await Promise.all(players.map(player => enterTournamentMatch(player, competitionId!, firstMatch!.matchId)))
    await Promise.all(players.map(async player => {
      await expect(player.page.locator('.competition-ready')).toBeVisible()
      await player.page.getByRole('button', { name: '我準備好了', exact: true }).click()
    }))
    await chooseInitialPouch(players)

    await finishGameThroughPlayerActions(players)
    await Promise.all(players.map(player => expect(player.page.locator('.result-panel')).toBeVisible()))

    for (const gameNumber of [2, 3]) {
      const betweenGames = await until(async () => {
        const view = await tournamentView(firstPlayer.context, competitionId!)
        const match = allMatches(view).find(candidate => candidate.matchId === firstMatch!.matchId)
        return match?.status === 'inProgress' && match.started ? view : null
      }, 45_000, `Game ${gameNumber} in the fixed-game match`)
      expect(allMatches(betweenGames).find(match => match.matchId === firstMatch!.matchId)?.result).toBeNull()
      await createOrdinaryNextGame(players, competitionId!, firstMatch!.matchId, gameNumber)
      await finishGameThroughPlayerActions(players)
      await Promise.all(players.map(player => expect(player.page.locator('.result-panel')).toBeVisible()))
    }

    for (const stageId of ['second-swiss', 'third-single-elimination', 'fourth-double-elimination']) {
      while (true) {
        const work = await nextStageWork(firstPlayer, competitionId, stageId)
        if (work.type === 'finished') break
        await finishScheduledMatch(players, competitionId, work.matchId)
      }
    }

    const completed = await expect.poll(async () => {
      try {
        const view = await tournamentView(firstPlayer.context, competitionId!)
        return view.publication ? view : null
      } catch { return null }
    }, { timeout: 90_000 }).not.toBeNull().then(() => tournamentView(firstPlayer.context, competitionId!))
    expect(completed.stages).toHaveLength(4)
    expect(completed.stages.map(stage => stage.config.format)).toEqual(stageFormats)
    expect(completed.stages.every(stage => stage.status === 'finished')).toBe(true)
    expect(completed.stages.slice(1).every(stage => (
      stage.rounds.some(round => round.matches.some(match => match.result !== null))
    ))).toBe(true)
    expect(completed.publication?.lockedDecks).toHaveLength(2)
    expect(completed.publication?.replays.length).toBeGreaterThanOrEqual(2)

    const anonymous = await browser.newContext({ baseURL: `http://localhost:${port('FEWFC_E2E_PORT', 8727)}` })
    try {
      const publicResponse = await anonymous.request.get(`/api/competitions/${encodeURIComponent(competitionId)}/tournament`)
      expect(publicResponse.ok()).toBe(true)
      const publicView = parseCompetitionTournamentViewResponse(await publicResponse.json(), competitionId)
      expect(publicView.publication?.completedAtTrustedTimeMs).toBeGreaterThan(0)
      expect(publicView.publication?.replays).toHaveLength(completed.publication?.replays.length)
      const publicPage = await anonymous.newPage()
      await gotoAppRoute(publicPage, `/competitions/${encodeURIComponent(competitionId)}`)
      await expect(publicPage.getByRole('heading', { name: '本場賽事已完成', exact: true })).toBeVisible()
      await expect(publicPage.getByRole('heading', { name: '公開重播', exact: true })).toBeVisible()
    } finally {
      await anonymous.close()
    }
  } finally {
    await Promise.all([
      ...(organizerContextOpen ? [organizer.context] : []),
      ...players.map(player => player.context),
    ].map(context => context.close()))
  }
})
