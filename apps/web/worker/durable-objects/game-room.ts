import { DurableObject } from 'cloudflare:workers'
import {
  emptyPublicState,
  invitationCredentialMatches,
  normalizeGameRoomMetadata,
  requireReadyRulesResult,
  type GameRoomAccess,
  type GameRoomCapacity,
  type CommandReceipt,
  type CompletedReplayDraft,
  type GameRoomInvitation,
  type GameRoomMember,
  type GameRoomMetadata,
  type GameRoomRequest,
  type GameRoomResponse,
  type GameRecord,
  type OnlineGameAction,
  type PlayerDeckList,
  type PlayerNotification,
  type RulesGameSetup,
  type RulesEngineResult,
  type RulesReadyResult,
  type StoredGameEvent,
} from '../../shared/game-room'
import type { PlayableAction, PlayerId } from '../../app/types/fewfc'
import {
  callRuleModuleResolution,
  callRulesEngine as callRulesEngineResult,
} from '../rules-engine'
import { RulesEngineError } from '../rules-engine-error'
import {
  executePlayerCommand,
  OnlineCommandTransactionError,
} from './online-command-transaction'

async function callRulesEngine(request: unknown): Promise<RulesReadyResult> {
  return requireReadyRulesResult(await callRulesEngineResult(request))
}

function canonicalEventTypes(record: unknown[]): string[] {
  return record.flatMap((decision) => {
    if (!decision || typeof decision !== 'object' || !('events' in decision)) return []
    const events = decision.events
    if (!Array.isArray(events)) return []
    return events.flatMap((event) => (
      event && typeof event === 'object' ? Object.keys(event) : []
    ))
  })
}

interface GameRoomEnv {
  PLAYER_NOTIFICATIONS: DurableObjectNamespace
}

interface SocketAttachment {
  userId: string
}

interface NotificationEnvelope {
  targetUserId: string
  notification: PlayerNotification
}

type RulesEngineAction =
  | OnlineGameAction
  | { type: 'startDevelopmentScenario'; player: PlayerId; scenario: string }
  | { type: 'developmentScenarioAction'; player: PlayerId; scenario: string }
  | { type: 'prepareDevelopmentScenario'; player: PlayerId; scenario: string }
  | {
      type: 'trustedRandomHandCandidates'
      player: PlayerId
      candidateAction: OnlineGameAction
    }
  | { type: 'resolveRandomness'; requestId: string; shuffledOrder: number[] }

export class GameRoom extends DurableObject<GameRoomEnv> {
  private operationTail: Promise<void> = Promise.resolve()

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)

    if (request.method === 'POST' && url.pathname.endsWith('/manage/purge-legacy')) {
      const body = await request.json() as { epoch?: unknown }
      return await this.serialized(async () => {
        if (typeof body.epoch !== 'string' || !body.epoch.trim()) {
          return this.json({ error: 'a purge epoch is required' }, 400)
        }
        return this.json(await this.purgeLegacyGameData(body.epoch.trim()))
      })
    }

    if (request.method === 'POST' && url.pathname.endsWith('/manage/verify-legacy')) {
      return await this.serialized(async () => this.json(await this.legacyPurgeVerification()))
    }

    if (request.method === 'GET' && url.pathname.endsWith('/socket')) {
      return await this.serialized(() => this.connect(request))
    }

    if (request.method === 'GET') {
      return this.json(await this.response())
    }

    if (request.method !== 'POST') {
      return this.json({ error: 'method not allowed' }, 405)
    }

    const body = (await request.json()) as GameRoomRequest

    return await this.serialized(async () => {
      try {
      switch (body.type) {
        case 'createGame':
          return this.json(await this.createGame(body))
        case 'joinGame':
          return await this.joinGame(body.actorUserId, body.actorName, body.credential)
        case 'toggleReady':
          return await this.toggleReady(body.actorUserId, body.deckList)
        case 'updateRuleModules':
          return await this.updateRuleModules(body.actorUserId, body.enabledRuleModules)
        case 'leaveGame':
          return await this.leaveGame(body.actorUserId)
        case 'removePlayer':
          return await this.removePlayer(body.actorUserId, body.targetUserId)
        case 'dissolveGame':
          return await this.dissolveGame(body.actorUserId)
        case 'startGame':
          return await this.startGame(body.actorUserId, body.deckList)
        case 'resetGame':
          return await this.resetGame(body.actorUserId)
        case 'getCompletedReplayDraft':
          return await this.getCompletedReplayDraft(body.actorUserId)
        case 'seedDevelopmentScenario':
          return await this.seedDevelopmentScenario(body.actorUserId, body.scenario)
        case 'inspectDevelopmentRecord':
          return await this.inspectDevelopmentRecord(body.actorUserId, body.commandId)
        case 'getState':
          return await this.getState(body.actorUserId)
        case 'submitCommand':
          return await this.submitCommand(body)
        default:
          return this.json({ error: 'unknown game-room request' }, 400)
      }
      } catch (error) {
        if (error instanceof RulesEngineError) {
          if (error.statusCode === 500) console.error('Rules Engine request failed', error.detail)
          return this.json({ error: error.message, code: error.code }, error.statusCode)
        }

        if (error instanceof OnlineCommandTransactionError) {
          return this.json({ error: error.message, code: error.code }, error.statusCode)
        }

        if (error instanceof Error && error.message === 'game room has not been created') {
          return this.json({ error: '找不到遊戲房間。', code: 'roomNotFound' }, 404)
        }

        console.error('Game Room request failed', error)
        return this.json({
          error: '房間服務暫時無法處理要求，請稍後再試。',
          code: 'gameRoomFailure',
        }, 500)
      }
    })
  }

  async webSocketMessage(socket: WebSocket, message: string | ArrayBuffer) {
    if (typeof message === 'string' && message === 'ping') {
      socket.send('pong')
    }
  }

  async webSocketClose(socket: WebSocket) {
    await this.serialized(() => this.disconnect(socket))
  }

  async webSocketError(socket: WebSocket) {
    await this.serialized(() => this.disconnect(socket))
  }

  private async serialized<T>(operation: () => Promise<T>): Promise<T> {
    const previous = this.operationTail
    let release = () => {}
    this.operationTail = new Promise<void>((resolve) => {
      release = resolve
    })

    await previous

    try {
      return await operation()
    } finally {
      release()
    }
  }

  private async createGame(
    request: Extract<GameRoomRequest, { type: 'createGame' }>,
  ): Promise<GameRoomResponse> {
    const existing = await this.metadata()

    if (existing) {
      return await this.response(existing, request.actorUserId)
    }

    const capacity = request.capacity === 4 ? 4 : 2
    const players = Array.from({ length: capacity }, (_, index) => `player-${index + 1}`)
    const now = new Date().toISOString()
    const metadata: GameRoomMetadata = {
      schemaVersion: 4,
      gameId: request.gameId,
      name: request.name?.trim() || request.gameId,
      access: request.access ?? 'private',
      capacity,
      ruleset: 'fewfc-base',
      enabledRuleModules: (await callRuleModuleResolution(request.enabledRuleModules)).modules,
      players,
      members: [{
        userId: request.actorUserId,
        displayName: request.actorName,
        player: players[0] ?? 'player-1',
        ready: false,
        connected: false,
        owner: true,
      }],
      status: 'Waiting',
      createdAt: now,
      updatedAt: now,
    }
    const initialEvent: StoredGameEvent = {
      sequence: 1,
      type: 'GameCreated',
      payload: {
        capacity,
        access: metadata.access,
        ruleset: metadata.ruleset,
        enabledRuleModules: [...metadata.enabledRuleModules],
      },
      createdAt: now,
    }

    await this.ctx.storage.put('metadata', metadata)
    await this.ctx.storage.put('invitation', request.invitation)
    await this.ctx.storage.put('nextSequence', 2)
    await this.ctx.storage.put(this.eventKey(initialEvent.sequence), initialEvent)

    return await this.response(metadata, request.actorUserId)
  }

  private async joinGame(
    actorUserId: string,
    actorName: string,
    credential?: Extract<GameRoomRequest, { type: 'joinGame' }>['credential'],
  ): Promise<Response> {
    const metadata = await this.requireMetadata()

    if (metadata.members.some((member) => member.userId === actorUserId)) {
      return this.json(await this.response(metadata, actorUserId))
    }

    if (metadata.access === 'private' && !await this.validCredential(credential)) {
      return this.json({ error: 'room not found' }, 404)
    }

    if (metadata.status === 'Dissolved') {
      return this.json({ error: 'room not found' }, 404)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'room has already started' }, 409)
    }

    const occupiedPlayers = new Set(metadata.members.map((member) => member.player))
    const openPlayer = metadata.players.find((player) => !occupiedPlayers.has(player))

    if (!openPlayer) {
      return this.json({ error: 'room is full' }, 409)
    }

    const now = new Date().toISOString()
    const member: GameRoomMember = {
      userId: actorUserId,
      displayName: actorName,
      player: openPlayer,
      ready: false,
      connected: false,
      owner: false,
    }
    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      members: [...metadata.members, member],
      updatedAt: now,
    }, 'PlayerJoined', openPlayer, { player: openPlayer })

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata, {
      kind: 'roomChanged',
      message: `${actorName} 已加入「${metadata.name}」。`,
    }))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async getState(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()

    if (!this.memberFor(metadata, actorUserId)) {
      return this.json(
        { error: metadata.access === 'private' ? 'room not found' : 'player is not in this room' },
        metadata.access === 'private' ? 404 : 403,
      )
    }

    return this.json(await this.response(metadata, actorUserId))
  }

  private async validCredential(
    credential?: Extract<GameRoomRequest, { type: 'joinGame' }>['credential'],
  ): Promise<boolean> {
    if (!credential?.value) {
      return false
    }

    const invitation = await this.ctx.storage.get<GameRoomInvitation>('invitation')
    return invitationCredentialMatches(invitation, credential)
  }

  private async toggleReady(
    actorUserId: string,
    deckList: PlayerDeckList,
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor) {
      return this.json({ error: 'only room players may ready' }, 403)
    }

    if (actor.owner) {
      return this.json({ error: 'room owner does not ready' }, 409)
    }

    if (!actor.connected) {
      return this.json({ error: 'player must be connected to ready' }, 409)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'room has already started' }, 409)
    }

    const ready = !actor.ready
    if (ready) {
      await this.ctx.storage.put(this.lockedDeckKey(actorUserId), deckList)
    } else {
      await this.ctx.storage.delete(this.lockedDeckKey(actorUserId))
    }
    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      members: metadata.members.map((member) => (
        member.userId === actorUserId ? { ...member, ready } : member
      )),
      updatedAt: new Date().toISOString(),
    }, ready ? 'PlayerReady' : 'PlayerUnready', actor.player, {
      player: actor.player,
    })

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async updateRuleModules(
    actorUserId: string,
    enabledRuleModules: string[],
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may update rules' }, 403)
    }
    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'room has already started' }, 409)
    }

    const modules = (await callRuleModuleResolution(enabledRuleModules)).modules
    for (const member of metadata.members) {
      await this.ctx.storage.delete(this.lockedDeckKey(member.userId))
    }
    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      enabledRuleModules: modules,
      members: metadata.members.map(member => ({ ...member, ready: false })),
      updatedAt: new Date().toISOString(),
    }, 'RuleModulesChanged', actor.player, { enabledRuleModules: modules })

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))
    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async leaveGame(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor) {
      return this.json({ error: 'player is not in this room' }, 404)
    }

    if (actor.owner) {
      return this.json({ error: 'room owner must dissolve the room' }, 409)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'players cannot leave an active match' }, 409)
    }

    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      members: metadata.members.filter((member) => member.userId !== actorUserId),
      updatedAt: new Date().toISOString(),
    }, 'PlayerLeft', actor.player, { player: actor.player })
    await this.ctx.storage.delete(this.lockedDeckKey(actorUserId))

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata, {
      kind: 'roomChanged',
      message: `${actor.displayName} 已離開「${metadata.name}」。`,
    }))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async removePlayer(actorUserId: string, targetUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)
    const target = this.memberFor(metadata, targetUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may remove players' }, 403)
    }

    if (!target || target.owner) {
      return this.json({ error: 'target player cannot be removed' }, 409)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'players cannot be removed from an active match' }, 409)
    }

    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      members: metadata.members.filter((member) => member.userId !== targetUserId),
      updatedAt: new Date().toISOString(),
    }, 'PlayerRemoved', target.player, { player: target.player })
    await this.ctx.storage.delete(this.lockedDeckKey(targetUserId))

    this.ctx.waitUntil(Promise.all([
      this.afterRoomMutation(updatedMetadata, {
        kind: 'roomChanged',
        message: `${target.displayName} 已被移出「${metadata.name}」。`,
      }),
      this.sendNotification(targetUserId, {
        kind: 'removed',
        message: `你已被移出「${metadata.name}」。`,
      }),
    ]).then(() => {
      this.closeUserSockets(targetUserId, 4003, 'removed from room')
    }))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async dissolveGame(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may dissolve the room' }, 403)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'an active match cannot be dissolved' }, 409)
    }

    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      status: 'Dissolved',
      updatedAt: new Date().toISOString(),
    }

    await this.ctx.storage.put('metadata', updatedMetadata)
    this.ctx.waitUntil(this.notifyMembers(metadata, {
      kind: 'dissolved',
      message: `「${metadata.name}」已解散。`,
    }))
    this.broadcastDissolved(metadata.gameId)

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async startGame(
    actorUserId: string,
    deckList: PlayerDeckList,
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const owner = metadata.members.find((member) => member.owner)

    if (owner?.userId !== actorUserId) {
      return this.json({ error: 'only room owner may start' }, 403)
    }

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'room has already started' }, 409)
    }

    if (metadata.members.length !== metadata.capacity) {
      return this.json({ error: 'room is waiting for players' }, 409)
    }

    if (metadata.members.some((member) => !member.connected)) {
      return this.json({ error: 'all players must be connected' }, 409)
    }

    if (metadata.members.some((member) => !member.owner && !member.ready)) {
      return this.json({ error: 'not all joined players are ready' }, 409)
    }

    // A completed draft is only saveable until the next match starts.
    await this.ctx.storage.delete('lastCompletedReplayDraft')

    await this.ctx.storage.put(this.lockedDeckKey(actorUserId), deckList)
    const lockedDecks = await Promise.all(metadata.members.map(async member => ({
      player: member.player,
      ...await this.requireLockedDeck(member.userId),
    })))
    const setup = this.randomSetup(metadata, lockedDecks)
    const firstPlayer = setup.turnOrder[0] ?? metadata.players[0] ?? 'player-1'
    const deckSeed = crypto.randomUUID()
    const gameInstanceId = crypto.randomUUID()
    const rules = await callRulesEngine({
      action: { type: 'start' },
      viewer: 'observer',
      setup,
      deckSeed,
    })
    const now = new Date().toISOString()
    const sequence = await this.nextSequence()
    const event: StoredGameEvent = {
      sequence,
      type: 'GameStarted',
      actor: owner.player,
      payload: {
        gameInstanceId,
        setup,
        firstPlayer,
        deckSeed,
      },
      createdAt: now,
    }
    const snapshot: GameRecord = {
      schemaVersion: 6,
      gameInstanceId,
      sequence,
      firstPlayer,
      deckSeed,
      setup,
      rulesRecord: rules.record,
    }
    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      gameInstanceId,
      status: 'Active',
      updatedAt: now,
    }

    await this.ctx.storage.put('metadata', updatedMetadata)
    await this.ctx.storage.put('nextSequence', sequence + 1)
    await this.ctx.storage.put('gameRecord', snapshot)
    await this.ctx.storage.put(this.eventKey(sequence), event)

    this.ctx.waitUntil(Promise.all([
      this.broadcast(updatedMetadata),
      this.notifyMembers(updatedMetadata, {
        kind: 'gameStarted',
        message: `「${metadata.name}」已開始。`,
      }),
    ]).then(() => undefined))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async resetGame(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()

    if (!this.memberFor(metadata, actorUserId)) {
      return this.json({ error: 'only room players may return to the room' }, 403)
    }

    if (metadata.status !== 'Finished') {
      return this.json({ error: 'match has not finished' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    const finished = await this.callReadyRules({ type: 'refresh' }, 'observer', snapshot)
    const draft: CompletedReplayDraft = {
      schemaVersion: 1,
      replayId: crypto.randomUUID(),
      sourceGameId: metadata.gameId,
      finishedAt: snapshot.finishedAt ?? new Date().toISOString(),
      setup: snapshot.setup,
      record: snapshot.rulesRecord,
      players: metadata.members.map(member => ({
        player: member.player,
        displayName: member.displayName,
      })),
      originalUserIds: metadata.members.map(member => member.userId),
      roomName: metadata.name,
      result: {
        status: finished.state.status,
        hp: finished.state.hp,
      },
      firstPlayer: snapshot.firstPlayer,
    }
    await this.ctx.storage.put('lastCompletedReplayDraft', draft)

    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      gameInstanceId: undefined,
      members: metadata.members.map((member) => ({
        ...member,
        ready: false,
      })),
      status: 'Waiting',
      updatedAt: new Date().toISOString(),
    }, 'PlayersReturnedToRoom', undefined, {})
    for (const member of metadata.members) {
      await this.ctx.storage.delete(this.lockedDeckKey(member.userId))
    }
    await this.ctx.storage.delete('gameRecord')
    await this.deleteGameInstanceTransactions(snapshot.gameInstanceId)

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async getCompletedReplayDraft(actorUserId: string): Promise<Response> {
    const draft = await this.ctx.storage.get<CompletedReplayDraft>('lastCompletedReplayDraft')
    if (!draft) {
      return this.json({ error: 'replay is no longer available', code: 'replayNoLongerAvailable' }, 409)
    }
    if (!draft.originalUserIds.includes(actorUserId)) {
      return this.json({ error: 'only original players may save this replay' }, 403)
    }
    return this.json(draft)
  }

  private async seedDevelopmentScenario(
    actorUserId: string,
    scenario: import('../../shared/development-scenarios').DevelopmentScenario,
  ): Promise<Response> {
    switch (scenario.name) {
      case 'star-endgame':
        return await this.seedEndgameFixture(actorUserId)
      case 'hero-schools-transition':
        return await this.seedHeroSchoolsFixture(actorUserId)
      case 'spirit-skill':
        return await this.seedSpiritFixture(actorUserId, scenario.options?.spirit)
      case 'echo-pure-fire':
        return await this.seedEchoFixture(actorUserId, 'pureFire', scenario.options?.mode)
      case 'echo-ringing-metal':
        return await this.seedEchoFixture(actorUserId, 'ringingMetal')
      case 'echo-split-earth':
        return await this.seedEchoFixture(actorUserId, 'splitEarth')
      case 'tribulation-earth-rending':
        return await this.seedTribulationFixture(actorUserId)
      case 'tribulation-rusted-forest':
        return await this.seedRustedForestFixture(actorUserId)
      case 'pouch-chain-sheep':
        return await this.seedPouchChainSheepFixture(actorUserId)
    }
  }

  private async inspectDevelopmentRecord(
    actorUserId: string,
    commandId: string,
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)
    if (!actor?.owner) {
      return this.json({ error: 'only room owner may inspect a test record' }, 403)
    }

    const commandCommitCount = (await this.events()).filter((event) => (
      event.type === 'RulesCommandApplied' && event.commandId === commandId
    )).length
    const snapshot = await this.requireGameRecord()

    return this.json({
      commandCommitCount,
      canonicalEventTypes: canonicalEventTypes(snapshot.rulesRecord),
    })
  }

  private async seedEndgameFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    const teams = [...new Set(snapshot.setup.players.map(player => player.team))]
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      initialHp: teams.map(team => ({ team, hp: 1 })),
    }
    const rules = await callRulesEngine({
      action: { type: 'start' },
      viewer: 'observer',
      setup,
      deckSeed: snapshot.deckSeed,
    })

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      setup,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json(await this.response(metadata, actorUserId))
  }

  private async seedHeroSchoolsFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('hero-schools')) {
      return this.json({ error: 'test fixture requires Hero Schools' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }
    const deckSeed = 'development:hero-schools-transition'
    let rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: 'hero-schools-transition',
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const transition = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: 'hero-schools-transition',
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const profession = transition.playableActions.find(
      action => action.type === 'changeProfession',
    )
    if (!profession || profession.type !== 'changeProfession') {
      return this.json({ error: 'test fixture could not find a Mesmer transition Card' }, 500)
    }
    rules = await callRulesEngine({
      action: {
        type: 'changeProfession',
        player: actor.player,
        professionId: profession.id,
        cards: profession.cards,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const discardChoice = rules.state.pendingChoice?.visibility === 'visible'
      && rules.state.pendingChoice.choice.type === 'card'
      ? { choiceId: rules.state.pendingChoice.choiceId, card: rules.state.pendingChoice.choice.cards[0] }
      : undefined
    if (discardChoice) {
      rules = await callRulesEngine({
        action: {
          type: 'answerChoice',
          player: actor.player,
          choiceId: discardChoice.choiceId,
          answer: { type: 'cards', cards: [discardChoice.card.id] },
        },
        viewer: actor.player,
        setup,
        deckSeed,
        record: rules.record,
      })
    }
    const opponent = setup.turnOrder.find(player => player !== actor.player)
    if (!opponent || rules.state.currentPlayer !== opponent) {
      return this.json({ error: 'test fixture did not advance to the opponent' }, 500)
    }
    rules = await callRulesEngine({
      action: { type: 'refresh' },
      viewer: opponent,
      setup,
      deckSeed,
      record: rules.record,
    })
    const opponentHand = rules.state.hands.find(entry => entry.player === opponent)
    const opponentCard = opponentHand?.cards.kind === 'known'
      ? opponentHand.cards.cards[0]
      : undefined
    if (!opponentCard) {
      return this.json({ error: 'test fixture could not inspect the opponent hand' }, 500)
    }
    const options = await callRulesEngine({
      action: {
        type: 'playableActions',
        player: opponent,
        cards: [opponentCard.id],
      },
      viewer: opponent,
      setup,
      deckSeed,
      record: rules.record,
    })
    const formation = options.playableActions.find(action => action.type === 'performFormation')
    if (!formation || formation.type !== 'performFormation') {
      return this.json({ error: 'test fixture could not find an opponent Formation' }, 500)
    }
    rules = await callRulesEngine({
      action: {
        type: 'performFormation',
        player: opponent,
        formationId: formation.id,
        cards: formation.cards,
        starSubstitutionCard: formation.starSubstitution?.card,
        matchOptionRole: formation.matchOption?.role,
        matchOptionCard: formation.matchOption?.card,
        matchOptionSlots: formation.matchOption?.slots,
      },
      viewer: opponent,
      setup,
      deckSeed,
      record: rules.record,
    })
    const opponentDiscardChoice = rules.state.pendingChoice?.visibility === 'visible'
      && rules.state.pendingChoice.choice.type === 'card'
      ? { choiceId: rules.state.pendingChoice.choiceId, card: rules.state.pendingChoice.choice.cards[0] }
      : undefined
    if (opponentDiscardChoice) {
      rules = await callRulesEngine({
        action: {
          type: 'answerChoice',
          player: opponent,
          choiceId: opponentDiscardChoice.choiceId,
          answer: { type: 'cards', cards: [opponentDiscardChoice.card.id] },
        },
        viewer: opponent,
        setup,
        deckSeed,
        record: rules.record,
      })
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      firstPlayer: actor.player,
      deckSeed,
      setup,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json(await this.response(metadata, actorUserId))
  }

  private async seedSpiritFixture(
    actorUserId: string,
    spirit: 'Metal' | 'Fire' = 'Metal',
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('spirit')) {
      return this.json({ error: 'test fixture requires Spirit' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }

    const scenarioName = spirit === 'Fire' ? 'spirit-fire' : 'spirit-metal'
    const deckSeed = `development:${scenarioName}`
    let rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: scenarioName,
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const scenario = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: scenarioName,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const summoningAction = scenario.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation'
      ),
    )
    if (!summoningAction) {
      return this.json({ error: `test fixture could not find two ${spirit} Cards` }, 500)
    }

    rules = await callRulesEngine({
      action: {
        type: 'performFormation',
        player: actor.player,
        formationId: summoningAction.id,
        cards: summoningAction.cards,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })

    rules = await callRulesEngine({
      action: {
        type: 'prepareDevelopmentScenario',
        player: actor.player,
        scenario: scenarioName,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json(await this.response(metadata, actorUserId))
  }

  private async seedEchoFixture(
    actorUserId: string,
    melody: 'pureFire' | 'ringingMetal' | 'splitEarth',
    mode?: 'actionDetail',
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('echo')) {
      return this.json({ error: 'test fixture requires Echo' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }

    const scenario = {
      pureFire: 'echo-pure-fire',
      ringingMetal: 'echo-ringing-metal',
      splitEarth: 'echo-split-earth',
    }[melody]
    const formationId = {
      pureFire: 'echo:pure-fire',
      ringingMetal: 'echo:ringing-metal',
      splitEarth: 'echo:split-earth',
    }[melody]
    const formationName = {
      pureFire: 'Pure Fire',
      ringingMetal: 'Ringing Metal',
      splitEarth: 'Split Earth',
    }[melody]
    const deckSeed = `development:${scenario}`
    let rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario,
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const actions = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario,
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const formation = actions.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation' && action.id === formationId
      ),
    )
    if (!formation) {
      return this.json({ error: `test fixture could not find ${formationName} Cards` }, 500)
    }

    if (mode === 'actionDetail') {
      await this.ctx.storage.put('gameRecord', {
        ...snapshot,
        setup,
        deckSeed,
        rulesRecord: rules.record,
      } satisfies GameRecord)
      this.ctx.waitUntil(this.broadcast(metadata))

      return this.json({
        ...await this.response(metadata, actorUserId),
        fixtureCards: formation.cards,
      })
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)

    // A Pending Choice is a canonical transaction checkpoint.  Create it by
    // exercising the same command path as a player, so the fixture also has
    // the durable transaction that authorizes the later choice response.
    const commandId = `development:${scenario}:${crypto.randomUUID()}`
    const response = await this.submitCommand({
      type: 'submitCommand',
      commandId,
      gameInstanceId: snapshot.gameInstanceId,
      transactionId: `transaction:${commandId}`,
      actorUserId,
      action: {
        type: 'performFormation',
        player: actor.player,
        formationId: formation.id,
        cards: formation.cards,
      },
    })
    const body = await response.clone().json<GameRoomResponse>()
    if (!response.ok || body.receipt?.outcome !== 'accepted' || !body.state.pendingChoice) {
      return this.json({ error: `test fixture did not reach ${formationName} choice` }, 500)
    }

    return response
  }

  private async seedTribulationFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('tribulation')) {
      return this.json({ error: 'test fixture requires Tribulation' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }
    const deckSeed = 'development:tribulation-earth-rending'
    let rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: 'tribulation-earth-rending',
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const actions = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: 'tribulation-earth-rending',
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const earthRending = actions.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation'
      ),
    )
    if (!earthRending) {
      return this.json({ error: 'test fixture could not find Earth Rending Cards' }, 500)
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)

    const commandId = `development:tribulation-earth-rending:${crypto.randomUUID()}`
    const response = await this.submitCommand({
      type: 'submitCommand',
      commandId,
      gameInstanceId: snapshot.gameInstanceId,
      transactionId: `transaction:${commandId}`,
      actorUserId,
      action: {
        type: 'performFormation',
        player: actor.player,
        formationId: earthRending.id,
        cards: earthRending.cards,
      },
    })
    const body = await response.clone().json<GameRoomResponse>()
    if (
      !response.ok
      || body.receipt?.outcome !== 'accepted'
      || body.state.pendingChoice?.choice.type !== 'environment'
      || body.state.pendingChoice.choice.environments.length !== 5
    ) {
      return this.json({ error: 'test fixture did not reach Environment choice' }, 500)
    }

    return response
  }

  private async seedRustedForestFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('tribulation')) {
      return this.json({ error: 'test fixture requires Tribulation' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }
    const deckSeed = 'development:tribulation-rusted-forest'
    const rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: 'tribulation-rusted-forest',
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const actions = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: 'tribulation-rusted-forest',
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const rustedForest = actions.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation'
        && action.id === 'tribulation:rusted-forest'
      ),
    )
    if (!rustedForest) {
      return this.json({ error: 'test fixture could not find Rusted Forest Cards' }, 500)
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      firstPlayer: actor.player,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json({
      ...await this.response(metadata, actorUserId),
      fixtureAction: {
        type: 'performFormation',
        player: actor.player,
        formationId: rustedForest.id,
        cards: rustedForest.cards,
      },
    })
  }

  private async seedPouchChainSheepFixture(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)
    if (!actor?.owner) {
      return this.json({ error: 'only room owner may seed a test fixture' }, 403)
    }
    if (metadata.status !== 'Active') {
      return this.json({ error: 'test fixture requires an active match' }, 409)
    }

    const snapshot = await this.requireGameRecord()
    if (!snapshot.setup.enabledRuleModules.includes('pouch')) {
      return this.json({ error: 'test fixture requires Pouch' }, 409)
    }
    const setup: RulesGameSetup = {
      ...snapshot.setup,
      turnOrder: [
        actor.player,
        ...snapshot.setup.turnOrder.filter(player => player !== actor.player),
      ],
    }
    const deckSeed = 'development:pouch-chain-sheep'
    const rules = await callRulesEngine({
      action: {
        type: 'startDevelopmentScenario',
        player: actor.player,
        scenario: 'pouch-chain-sheep',
      },
      viewer: actor.player,
      setup,
      deckSeed,
    })
    const scenario = await callRulesEngine({
      action: {
        type: 'developmentScenarioAction',
        player: actor.player,
        scenario: 'pouch-chain-sheep',
      },
      viewer: actor.player,
      setup,
      deckSeed,
      record: rules.record,
    })
    const chain = scenario.playableActions.find(
      (action): action is Extract<PlayableAction, { type: 'performFormation' }> => (
        action.type === 'performFormation' && action.id === 'pouch:chain'
      ),
    )
    if (!chain) {
      return this.json({ error: 'test fixture could not find Chain Cards' }, 500)
    }

    await this.ctx.storage.put('gameRecord', {
      ...snapshot,
      firstPlayer: actor.player,
      setup,
      deckSeed,
      rulesRecord: rules.record,
    } satisfies GameRecord)
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json({
      ...await this.response(metadata, actorUserId),
      fixtureAction: {
        type: 'performFormation',
        player: actor.player,
        formationId: chain.id,
        cards: chain.cards,
      },
    })
  }

  private async submitCommand(
    request: Extract<GameRoomRequest, { type: 'submitCommand' }>,
  ): Promise<Response> {
    const executed = await executePlayerCommand({
      storage: this.ctx.storage,
      metadata: () => this.metadata(),
      gameRecord: () => this.requireGameRecord(),
      playerFor: (metadata, userId) => this.playerFor(metadata, userId),
      actionForPlayer: (action, player) => this.actionForPlayer(action, player),
      callRules: (action, viewer, record) => this.callRules(action, viewer, record),
      response: (metadata, actorUserId, playableActions, receipt) => (
        this.response(metadata, actorUserId, playableActions, receipt)
      ),
      shuffle: values => this.shuffle(values),
    }, request)

    if (executed.committed) {
      this.ctx.waitUntil(Promise.all([
        this.broadcast(executed.metadata),
        executed.nextPlayer && executed.nextPlayer !== executed.previousPlayer
          ? this.notifyPlayer(executed.metadata, executed.nextPlayer, {
              kind: 'yourTurn',
              message: `「${executed.metadata.name}」輪到你行動。`,
            })
          : Promise.resolve(),
      ]).catch(error => {
        console.error('post-commit game-room delivery failed', error)
      }))
    }

    return this.json(executed.response)
  }

  private async connect(request: Request): Promise<Response> {
    if (request.headers.get('Upgrade')?.toLowerCase() !== 'websocket') {
      return this.json({ error: 'websocket upgrade required' }, 426)
    }

    const actorUserId = request.headers.get('x-fewfc-user-id')
    const encodedActorName = request.headers.get('x-fewfc-user-name')

    if (!actorUserId || !encodedActorName) {
      return this.json({ error: 'authenticated player required' }, 401)
    }

    const actorName = decodeURIComponent(encodedActorName)

    const metadata = await this.requireMetadata()
    const actor = this.memberFor(metadata, actorUserId)

    if (!actor) {
      return this.json({ error: 'only room players may connect' }, 403)
    }

    const pair = new WebSocketPair()
    const [client, server] = Object.values(pair)

    this.ctx.acceptWebSocket(server)
    server.serializeAttachment({ userId: actorUserId } satisfies SocketAttachment)

    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      members: metadata.members.map((member) => (
        member.userId === actorUserId
          ? { ...member, displayName: actorName, connected: true }
          : member
      )),
      updatedAt: new Date().toISOString(),
    }

    await this.ctx.storage.put('metadata', updatedMetadata)
    server.send(JSON.stringify({
      type: 'roomState',
      data: await this.response(updatedMetadata, actorUserId),
    }))
    this.ctx.waitUntil(this.broadcast(updatedMetadata))

    return new Response(null, { status: 101, webSocket: client })
  }

  private async disconnect(socket: WebSocket) {
    const attachment = socket.deserializeAttachment() as SocketAttachment | null

    if (!attachment?.userId) {
      return
    }

    const hasAnotherConnection = this.ctx.getWebSockets().some((candidate) => {
      if (candidate === socket || candidate.readyState !== WebSocket.OPEN) {
        return false
      }

      const candidateAttachment = candidate.deserializeAttachment() as SocketAttachment | null
      return candidateAttachment?.userId === attachment.userId
    })

    if (hasAnotherConnection) {
      return
    }

    const metadata = await this.metadata()

    if (!metadata) {
      return
    }

    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      members: metadata.members.map((member) => (
        member.userId === attachment.userId
          ? {
              ...member,
              connected: false,
              ready: metadata.status === 'Waiting' && !member.owner ? false : member.ready,
            }
          : member
      )),
      updatedAt: new Date().toISOString(),
    }

    await this.ctx.storage.put('metadata', updatedMetadata)
    await this.broadcast(updatedMetadata)
  }

  private async afterRoomMutation(
    metadata: GameRoomMetadata,
    notification?: Pick<PlayerNotification, 'kind' | 'message'>,
  ) {
    await this.broadcast(metadata)

    if (notification) {
      await this.notifyMembers(metadata, notification)
    }
  }

  private async broadcast(metadata?: GameRoomMetadata) {
    const currentMetadata = metadata ?? await this.requireMetadata()

    await Promise.all(this.ctx.getWebSockets().map(async (socket) => {
      if (socket.readyState !== WebSocket.OPEN) {
        return
      }

      const attachment = socket.deserializeAttachment() as SocketAttachment | null

      if (!attachment?.userId) {
        return
      }

      try {
        socket.send(JSON.stringify({
          type: 'roomState',
          data: await this.response(currentMetadata, attachment.userId),
        }))
      } catch {
        socket.close(1011, 'state update failed')
      }
    }))
  }

  private broadcastDissolved(gameId: string) {
    for (const socket of this.ctx.getWebSockets()) {
      if (socket.readyState === WebSocket.OPEN) {
        socket.send(JSON.stringify({ type: 'roomDissolved', gameId }))
        socket.close(1000, 'room dissolved')
      }
    }
  }

  private closeUserSockets(userId: string, code: number, reason: string) {
    for (const socket of this.ctx.getWebSockets()) {
      const attachment = socket.deserializeAttachment() as SocketAttachment | null

      if (attachment?.userId === userId && socket.readyState === WebSocket.OPEN) {
        socket.close(code, reason)
      }
    }
  }

  private async notifyMembers(
    metadata: GameRoomMetadata,
    notification: Pick<PlayerNotification, 'kind' | 'message'>,
  ) {
    await Promise.all(metadata.members.map((member) => (
      this.sendNotification(member.userId, notification)
    )))
  }

  private async notifyPlayer(
    metadata: GameRoomMetadata,
    player: PlayerId,
    notification: Pick<PlayerNotification, 'kind' | 'message'>,
  ) {
    const member = metadata.members.find((candidate) => candidate.player === player)

    if (member) {
      await this.sendNotification(member.userId, notification)
    }
  }

  private async sendNotification(
    userId: string,
    notification: Pick<PlayerNotification, 'kind' | 'message'>,
  ) {
    const id = this.env.PLAYER_NOTIFICATIONS.idFromName('global')
    const target = this.env.PLAYER_NOTIFICATIONS.get(id)

    await target.fetch('https://player-notifications.internal/notify', {
      method: 'POST',
      headers: {
        'content-type': 'application/json',
      },
      body: JSON.stringify({
        targetUserId: userId,
        notification: {
          id: crypto.randomUUID(),
          gameId: (await this.requireMetadata()).gameId,
          kind: notification.kind,
          message: notification.message,
          createdAt: new Date().toISOString(),
        },
      } satisfies NotificationEnvelope),
    })
  }

  private randomSetup(
    metadata: GameRoomMetadata,
    deckLists: Array<PlayerDeckList & { player: PlayerId }>,
  ): RulesGameSetup {
    const turnOrder = this.shuffle(metadata.members.map((member) => member.player))

    if (metadata.capacity === 2) {
      return {
        players: turnOrder.map((player, index) => ({
          id: player,
          team: `team-${index + 1}`,
        })),
        turnOrder,
        enabledRuleModules: metadata.enabledRuleModules,
        deckLists,
      }
    }

    const teamByPlayer = new Map<PlayerId, string>()
    turnOrder.forEach((player, index) => {
      teamByPlayer.set(player, index % 2 === 0 ? 'team-a' : 'team-b')
    })

    return {
      players: metadata.members.map((member) => ({
        id: member.player,
        team: teamByPlayer.get(member.player) ?? 'team-a',
      })),
      turnOrder,
      enabledRuleModules: [...metadata.enabledRuleModules],
      deckLists,
    }
  }

  private shuffle<T>(values: T[]): T[] {
    const shuffled = [...values]

    for (let index = shuffled.length - 1; index > 0; index -= 1) {
      const random = new Uint32Array(1)
      crypto.getRandomValues(random)
      const swapIndex = (random[0] ?? 0) % (index + 1)
      const current = shuffled[index]
      shuffled[index] = shuffled[swapIndex] as T
      shuffled[swapIndex] = current as T
    }

    return shuffled
  }

  private async response(
    metadata?: GameRoomMetadata,
    actorUserId?: string,
    playableActions?: PlayableAction[],
    receipt?: CommandReceipt,
  ): Promise<GameRoomResponse> {
    const currentMetadata = metadata ?? await this.requireMetadata()
    const lockedDeckName = actorUserId
      ? (await this.ctx.storage.get<PlayerDeckList>(this.lockedDeckKey(actorUserId)))?.name
      : undefined
    const viewer = actorUserId
      ? (this.playerFor(currentMetadata, actorUserId) ?? 'observer')
      : 'observer'

    if (currentMetadata.status === 'Waiting' || currentMetadata.status === 'Dissolved') {
      const completed = actorUserId
        ? await this.ctx.storage.get<CompletedReplayDraft>('lastCompletedReplayDraft')
        : undefined
      return {
        gameId: currentMetadata.gameId,
        metadata: currentMetadata,
        invitation: await this.invitationFor(currentMetadata, actorUserId),
        lockedDeckName,
        savableReplay: completed && completed.originalUserIds.includes(actorUserId ?? '')
          ? {
              replayId: completed.replayId,
              sourceGameId: completed.sourceGameId,
              finishedAt: completed.finishedAt,
            }
          : undefined,
        state: emptyPublicState(currentMetadata.players),
        events: (await this.events()).map((event) => ({
          id: `room-event-${event.sequence}`,
          eventType: event.type,
          title: this.eventTitle(event),
          summary: this.displaySummary(currentMetadata, this.eventSummary(event)),
        })).reverse(),
        playableActions: playableActions ?? [],
        interaction: {
          canChooseInitialPouch: false,
        },
      }
    }

    const snapshot = await this.requireGameRecord()
    const publicRules = await this.callReadyRules({ type: 'refresh' }, viewer, snapshot)
    const responseMetadata: GameRoomMetadata = (
      currentMetadata.status === 'Active'
      && publicRules.state.status === 'Finished'
    )
      ? {
          ...currentMetadata,
          status: 'Finished',
          updatedAt: new Date().toISOString(),
        }
      : currentMetadata

    if (responseMetadata !== currentMetadata) {
      await this.ctx.storage.put('metadata', responseMetadata)
    }
    const activeTransactionId = responseMetadata.gameInstanceId
      ? (await this.ctx.storage.get<{ transactionId: string }>(
          `transaction:${responseMetadata.gameInstanceId}`,
        ))?.transactionId
      : undefined

    return {
      gameId: currentMetadata.gameId,
      metadata: responseMetadata,
      invitation: await this.invitationFor(currentMetadata, actorUserId),
      lockedDeckName,
      state: publicRules.state,
      events: publicRules.events.map((event) => ({
        ...event,
        summary: this.displaySummary(currentMetadata, event.summary),
      })),
      playableActions: playableActions ?? publicRules.playableActions,
      interaction: publicRules.interaction,
      activeTransactionId,
      activeGameVersion: {
        gameInstanceId: snapshot.gameInstanceId,
        recordSequence: snapshot.sequence,
      },
      receipt,
    }
  }

  private async invitationFor(
    metadata: GameRoomMetadata,
    actorUserId?: string,
  ): Promise<GameRoomInvitation | undefined> {
    const actor = actorUserId ? this.memberFor(metadata, actorUserId) : undefined
    if (!actor?.owner || metadata.status !== 'Waiting') {
      return undefined
    }

    return await this.ctx.storage.get<GameRoomInvitation>('invitation')
  }

  private playerFor(metadata: GameRoomMetadata, userId: string): PlayerId | undefined {
    return this.memberFor(metadata, userId)?.player
  }

  private memberFor(metadata: GameRoomMetadata, userId: string) {
    return metadata.members.find((member) => member.userId === userId)
  }

  private lockedDeckKey(userId: string) {
    return `lockedDeck:${userId}`
  }

  private async requireLockedDeck(userId: string): Promise<PlayerDeckList> {
    const deck = await this.ctx.storage.get<PlayerDeckList>(this.lockedDeckKey(userId))
    if (!deck) {
      throw new Error('player deck was not locked')
    }
    return deck
  }

  private actionForPlayer(action: OnlineGameAction, player: PlayerId): OnlineGameAction {
    switch (action.type) {
      case 'performFormation':
        return {
          type: action.type,
          player,
          formationId: action.formationId,
          cards: action.cards,
          starSubstitutionCard: action.starSubstitutionCard,
          matchOptionRole: action.matchOptionRole,
          matchOptionCard: action.matchOptionCard,
          matchOptionSlots: action.matchOptionSlots,
        }
      case 'useSpiritSkill':
        return {
          type: action.type,
          player,
          skill: action.skill,
          selectedCard: action.selectedCard,
          declaredLevel: action.declaredLevel,
        }
      case 'activateProfessionAbility':
      case 'changeProfession':
      case 'chooseInitialPouch':
      case 'triggerSecretStrategy':
      case 'answerChoice':
      case 'retrievePreviousTurnDiscard':
      case 'playableActions':
        return { ...action, player }
      default:
        return action
    }
  }

  private async callRules(
    action: RulesEngineAction,
    viewer: string | undefined,
    snapshot: GameRecord,
  ): Promise<RulesEngineResult> {
    return await callRulesEngineResult({
      action,
      viewer,
      setup: snapshot.setup,
      deckSeed: snapshot.deckSeed,
      record: snapshot.rulesRecord,
    })
  }

  private async callReadyRules(
    action: RulesEngineAction,
    viewer: string | undefined,
    snapshot: GameRecord,
  ): Promise<RulesReadyResult> {
    return requireReadyRulesResult(await this.callRules(action, viewer, snapshot))
  }

  private async storeRoomEvent(
    metadata: GameRoomMetadata,
    type: string,
    actor: PlayerId | undefined,
    payload: unknown,
  ): Promise<GameRoomMetadata> {
    const sequence = await this.nextSequence()
    const event: StoredGameEvent = {
      sequence,
      type,
      actor,
      payload,
      createdAt: metadata.updatedAt,
    }

    await this.ctx.storage.put('metadata', metadata)
    await this.ctx.storage.put('nextSequence', sequence + 1)
    await this.ctx.storage.put(this.eventKey(sequence), event)

    return metadata
  }

  private async requireMetadata(): Promise<GameRoomMetadata> {
    const metadata = await this.metadata()

    if (!metadata) {
      throw new Error('game room has not been created')
    }

    return metadata
  }

  private async requireGameRecord(): Promise<GameRecord> {
    const snapshot = await this.ctx.storage.get<GameRecord>('gameRecord')

    if (!snapshot) {
      throw new Error('game room Game Record is missing')
    }

    return snapshot
  }

  /**
   * One-way management cutover for a legacy Game Record. This is intentionally
   * an operation on the existing room object so room identity and waiting-room
   * configuration remain intact. The epoch marker makes a retry a no-op even
   * after Players have prepared a replacement game.
   */
  private async purgeLegacyGameData(epoch: string): Promise<{
    status: 'purged' | 'alreadyPurged' | 'dissolved' | 'absent'
    gameId?: string
  }> {
    const metadata = await this.metadata()
    if (!metadata) {
      // An object without room metadata cannot be a surviving room. Clear any
      // stranded record rather than allowing an orphaned legacy Game Record to
      // evade the namespace inventory.
      await this.ctx.storage.deleteAll()
      return { status: 'absent' }
    }
    if (metadata.status === 'Dissolved') return { status: 'dissolved', gameId: metadata.gameId }

    const previousEpoch = await this.ctx.storage.get<string>('legacyPurgeEpoch')
    if (previousEpoch === epoch) {
      return { status: 'alreadyPurged', gameId: metadata.gameId }
    }

    const activeOrFinished = metadata.status === 'Active' || metadata.status === 'Finished'
    const now = new Date().toISOString()
    const replacementMetadata: GameRoomMetadata = activeOrFinished
      ? {
          ...metadata,
          gameInstanceId: undefined,
          status: 'Waiting',
          members: metadata.members.map(member => ({
            ...member,
            ready: false,
            connected: false,
          })),
          updatedAt: now,
        }
      : {
          ...metadata,
          updatedAt: now,
        }

    const eventEntries = await this.ctx.storage.list({ prefix: 'event:' })
    const replacementEvent: StoredGameEvent = {
      sequence: 1,
      type: 'LegacyGamePurged',
      payload: { epoch },
      createdAt: now,
    }
    const entries: Record<string, unknown> = {
      metadata: replacementMetadata,
      nextSequence: 2,
      legacyPurgeEpoch: epoch,
      [this.eventKey(1)]: replacementEvent,
    }
    await this.ctx.storage.put(entries)
    await this.ctx.storage.delete([...eventEntries.keys()])
    // Put the replacement event again because the old log may have contained
    // sequence one.
    await this.ctx.storage.put(this.eventKey(1), replacementEvent)
    await this.ctx.storage.delete('lastCompletedReplayDraft')

    if (activeOrFinished) {
      await this.ctx.storage.delete('gameRecord')
      for (const member of metadata.members) {
        await this.ctx.storage.delete(this.lockedDeckKey(member.userId))
      }
      for (const prefix of ['commandReceipt:', 'trustedReceipt:', 'transaction:']) {
        const transactionEntries = await this.ctx.storage.list({ prefix })
        await this.ctx.storage.delete([...transactionEntries.keys()])
      }
    }

    return { status: 'purged', gameId: metadata.gameId }
  }

  private async legacyPurgeVerification(): Promise<{
    status: GameRoomMetadata['status'] | 'Absent'
    gameInstanceId?: string
    hasGameRecord: boolean
    eventTypes: string[]
  }> {
    const metadata = await this.metadata()
    if (!metadata) {
      return { status: 'Absent', hasGameRecord: false, eventTypes: [] }
    }
    const events = await this.events()
    return {
      status: metadata.status,
      gameInstanceId: metadata.gameInstanceId,
      hasGameRecord: Boolean(await this.ctx.storage.get<GameRecord>('gameRecord')),
      eventTypes: events.map(event => event.type),
    }
  }

  private async metadata(): Promise<GameRoomMetadata | undefined> {
    const stored = await this.ctx.storage.get<GameRoomMetadata>('metadata')

    if (!stored) {
      return undefined
    }

    const metadata = normalizeGameRoomMetadata(stored)

    if (JSON.stringify(metadata) !== JSON.stringify(stored)) {
      await this.ctx.storage.put('metadata', metadata)
    }

    return metadata
  }

  private async nextSequence(): Promise<number> {
    return await this.ctx.storage.get<number>('nextSequence') ?? 1
  }

  private async events(): Promise<StoredGameEvent[]> {
    const entries = await this.ctx.storage.list<StoredGameEvent>({
      prefix: 'event:',
    })

    return [...entries.values()].sort((left, right) => left.sequence - right.sequence)
  }

  private async deleteGameInstanceTransactions(gameInstanceId: string) {
    const prefixes = [
      `commandReceipt:${gameInstanceId}:`,
      `trustedReceipt:${gameInstanceId}:`,
      `transaction:${gameInstanceId}`,
    ]
    for (const prefix of prefixes) {
      const entries = await this.ctx.storage.list({ prefix })
      await Promise.all([...entries.keys()].map(key => this.ctx.storage.delete(key)))
    }
  }

  private eventKey(sequence: number): string {
    return `event:${sequence.toString().padStart(12, '0')}`
  }

  private eventSummary(event: StoredGameEvent): string {
    const labels: Record<string, string> = {
      GameCreated: '遊戲房間已建立。',
      PlayerJoined: `${event.actor ?? '玩家'} 已加入房間。`,
      PlayerLeft: `${event.actor ?? '玩家'} 已離開房間。`,
      PlayerRemoved: `${event.actor ?? '玩家'} 已被移出房間。`,
      PlayerReady: `${event.actor ?? '玩家'} 已準備。`,
      PlayerUnready: `${event.actor ?? '玩家'} 已取消準備。`,
      RuleModulesChanged: '房主已更新選用規則，所有玩家需重新準備。',
      PlayersReturnedToRoom: '玩家已返回等待房間。',
      LegacyGamePurged: '系統版本更新，上一局已清除，請重新準備。',
    }

    if (event.type in labels) {
      return labels[event.type] as string
    }

    if (event.type === 'RulesCommandApplied') {
      const actor = event.actor ?? '玩家'
      const action = this.payloadType(event.payload)
      const summaries: Record<string, string> = {
        passAction: `${actor} 已跳過行動。`,
        performFormation: `${actor} 已完成陣法行動。`,
        activateProfessionAbility: `${actor} 已發動職業能力。`,
        performFormationWithChoices: `${actor} 已完成陣法與效果選擇。`,
        answerChoice: `${actor} 已完成選擇。`,
        retrievePreviousTurnDiscard: `${actor} 已發動棄牌回收。`,
      }
      return summaries[action] ?? '戰局狀態已更新。'
    }

    if (event.type === 'GameStarted') {
      const firstPlayer = this.payloadValue(event.payload, 'firstPlayer')
      return firstPlayer ? `遊戲已開始，${firstPlayer} 先手。` : '遊戲已開始。'
    }

    return '房間狀態已更新。'
  }

  private eventTitle(event: StoredGameEvent): string {
    const titles: Record<string, string> = {
      GameCreated: '建立房間',
      PlayerJoined: '玩家加入',
      PlayerLeft: '玩家離開',
      PlayerRemoved: '移除玩家',
      PlayerReady: '玩家準備',
      PlayerUnready: '取消準備',
      RuleModulesChanged: '更新規則',
      PlayersReturnedToRoom: '返回房間',
      GameStarted: '對局開始',
      RulesCommandApplied: '戰局更新',
    }

    return titles[event.type] ?? '房間更新'
  }

  private payloadType(payload: unknown): string {
    if (payload && typeof payload === 'object' && 'type' in payload) {
      return String(payload.type)
    }

    return 'unknown'
  }

  private displaySummary(metadata: GameRoomMetadata, summary: string): string {
    return metadata.members.reduce(
      (text, member) => text.replaceAll(member.player, member.displayName),
      summary,
    )
  }

  private payloadValue(payload: unknown, key: string): string | undefined {
    if (payload && typeof payload === 'object') {
      const record = payload as Record<string, unknown>

      if (key in record) {
        return String(record[key])
      }
    }

    return undefined
  }

  private json(body: unknown, status = 200): Response {
    return Response.json(body, { status })
  }
}
