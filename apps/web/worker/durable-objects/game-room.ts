import { DurableObject } from 'cloudflare:workers'
import {
  emptyPublicState,
  normalizeGameRoomMetadata,
  type GameRoomAccess,
  type GameRoomCapacity,
  type GameRoomMember,
  type GameRoomMetadata,
  type GameRoomRequest,
  type GameRoomResponse,
  type GameRoomSnapshot,
  type OnlineGameAction,
  type PlayerNotification,
  type RulesGameSetup,
  type StoredGameEvent,
} from '../../shared/game-room'
import type { PlayerId } from '../../app/types/fewfc'
import { callRulesEngine } from '../rules-engine'

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

interface PendingCommandDraft {
  actorUserId: string
  commandId: string
  action: Extract<OnlineGameAction, { type: 'performFormation' }>
  snapshot: GameRoomSnapshot
}

export class GameRoom extends DurableObject<GameRoomEnv> {
  private operationTail: Promise<void> = Promise.resolve()

  async fetch(request: Request): Promise<Response> {
    const url = new URL(request.url)

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
          return await this.joinGame(body.actorUserId, body.actorName)
        case 'toggleReady':
          return await this.toggleReady(body.actorUserId)
        case 'leaveGame':
          return await this.leaveGame(body.actorUserId)
        case 'removePlayer':
          return await this.removePlayer(body.actorUserId, body.targetUserId)
        case 'dissolveGame':
          return await this.dissolveGame(body.actorUserId)
        case 'startGame':
          return await this.startGame(body.actorUserId)
        case 'resetGame':
          return await this.resetGame(body.actorUserId)
        case 'cancelPendingCommand':
          return await this.cancelPendingCommand(body.actorUserId)
        case 'getState':
          return this.json(await this.response(undefined, body.actorUserId))
        case 'submitCommand':
          return await this.submitCommand(body)
        default:
          return this.json({ error: 'unknown game-room request' }, 400)
      }
      } catch (error) {
        return this.json({
          error: error instanceof Error ? error.message : 'game room request failed',
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
      schemaVersion: 2,
      gameId: request.gameId,
      name: request.name?.trim() || request.gameId,
      access: request.access ?? 'private',
      capacity,
      ruleset: 'fewfc-base',
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
      },
      createdAt: now,
    }

    await this.ctx.storage.put('metadata', metadata)
    await this.ctx.storage.put('nextSequence', 2)
    await this.ctx.storage.put(this.eventKey(initialEvent.sequence), initialEvent)

    return await this.response(metadata, request.actorUserId)
  }

  private async joinGame(actorUserId: string, actorName: string): Promise<Response> {
    const metadata = await this.requireMetadata()

    if (metadata.status !== 'Waiting') {
      return this.json({ error: 'room has already started' }, 409)
    }

    if (metadata.members.some((member) => member.userId === actorUserId)) {
      return this.json(await this.response(metadata, actorUserId))
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

  private async toggleReady(actorUserId: string): Promise<Response> {
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

  private async startGame(actorUserId: string): Promise<Response> {
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

    const setup = this.randomSetup(metadata)
    const firstPlayer = setup.turnOrder[0] ?? metadata.players[0] ?? 'player-1'
    const deckSeed = crypto.randomUUID()
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
        setup,
        firstPlayer,
        deckSeed,
      },
      createdAt: now,
    }
    const snapshot: GameRoomSnapshot = {
      schemaVersion: 4,
      sequence,
      firstPlayer,
      deckSeed,
      setup,
      rulesRecord: rules.record,
    }
    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      status: 'Active',
      updatedAt: now,
    }

    await this.ctx.storage.put('metadata', updatedMetadata)
    await this.ctx.storage.put('nextSequence', sequence + 1)
    await this.ctx.storage.put('snapshot', snapshot)
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

    const updatedMetadata = await this.storeRoomEvent({
      ...metadata,
      members: metadata.members.map((member) => ({
        ...member,
        ready: false,
      })),
      status: 'Waiting',
      updatedAt: new Date().toISOString(),
    }, 'PlayersReturnedToRoom', undefined, {})

    this.ctx.waitUntil(this.afterRoomMutation(updatedMetadata))

    return this.json(await this.response(updatedMetadata, actorUserId))
  }

  private async submitCommand(
    request: Extract<GameRoomRequest, { type: 'submitCommand' }>,
  ): Promise<Response> {
    const metadata = await this.requireMetadata()
    const actor = this.playerFor(metadata, request.actorUserId)

    if (metadata.status !== 'Active') {
      return this.json({ error: 'room has not started' }, 409)
    }

    if (!actor) {
      return this.json({ error: 'only room players may submit commands' }, 403)
    }

    const action = this.actionForPlayer(request.action, actor)
    const viewer = actor
    const existingDraft = await this.pendingDraft()

    if (request.action.type === 'playableFormations') {
      const snapshot = await this.requireSnapshot()
      const rules = await this.callRules(action, viewer, snapshot)

      return this.json(await this.response(metadata, request.actorUserId, rules.playableFormations))
    }

    if (existingDraft && existingDraft.actorUserId !== request.actorUserId) {
      return this.json({ error: 'another player decision is pending' }, 409)
    }

    if (existingDraft && action.type !== 'answerEffectChoice') {
      return this.json({ error: 'complete or cancel the pending effect choice first' }, 409)
    }

    const duplicate = await this.findEventByCommandId(request.commandId)

    if (duplicate) {
      return this.json(await this.response(metadata, request.actorUserId))
    }

    const canonicalSnapshot = await this.requireSnapshot()
    const previousSnapshot = existingDraft?.snapshot ?? canonicalSnapshot
    const previousRules = await this.callRules({ type: 'refresh' }, 'observer', previousSnapshot)
    const rules = await this.callRules(action, viewer, previousSnapshot)

    if (!existingDraft && action.type === 'performFormation' && rules.state.pendingChoice) {
      const draft: PendingCommandDraft = {
        actorUserId: request.actorUserId,
        commandId: request.commandId,
        action,
        snapshot: {
          ...previousSnapshot,
          rulesRecord: rules.record,
        },
      }

      await this.ctx.storage.put('pendingCommandDraft', draft)
      this.ctx.waitUntil(this.broadcast(metadata))

      return this.json(await this.response(metadata, request.actorUserId))
    }

    if (existingDraft && rules.state.pendingChoice) {
      await this.ctx.storage.put('pendingCommandDraft', {
        ...existingDraft,
        snapshot: {
          ...existingDraft.snapshot,
          rulesRecord: rules.record,
        },
      } satisfies PendingCommandDraft)
      this.ctx.waitUntil(this.broadcast(metadata))

      return this.json(await this.response(metadata, request.actorUserId))
    }

    const now = new Date().toISOString()
    const sequence = await this.nextSequence()
    const event: StoredGameEvent = {
      sequence,
      type: 'RulesCommandApplied',
      commandId: existingDraft?.commandId ?? request.commandId,
      actor,
      payload: existingDraft
        ? {
            type: 'performFormationWithChoices',
            formation: existingDraft.action,
          }
        : action,
      createdAt: now,
    }
    const snapshot: GameRoomSnapshot = {
      ...previousSnapshot,
      sequence,
      rulesRecord: rules.record,
    }
    const status = rules.state.status === 'Finished' ? 'Finished' : 'Active'
    const updatedMetadata: GameRoomMetadata = {
      ...metadata,
      status,
      updatedAt: now,
    }

    await this.ctx.storage.put(this.eventKey(sequence), event)
    await this.ctx.storage.put('nextSequence', sequence + 1)
    await this.ctx.storage.put('snapshot', snapshot)
    await this.ctx.storage.put('metadata', updatedMetadata)
    await this.ctx.storage.delete('pendingCommandDraft')

    const nextPlayer = rules.state.currentPlayer
    const previousPlayer = previousRules.state.currentPlayer

    this.ctx.waitUntil(Promise.all([
      this.broadcast(updatedMetadata),
      nextPlayer && nextPlayer !== previousPlayer
        ? this.notifyPlayer(metadata, nextPlayer, {
            kind: 'yourTurn',
            message: `「${metadata.name}」輪到你行動。`,
          })
        : Promise.resolve(),
    ]).then(() => undefined))

    return this.json(await this.response(updatedMetadata, request.actorUserId))
  }

  private async cancelPendingCommand(actorUserId: string): Promise<Response> {
    const metadata = await this.requireMetadata()
    const draft = await this.pendingDraft()

    if (!draft || draft.actorUserId !== actorUserId) {
      return this.json({ error: 'no cancellable player choice is pending' }, 409)
    }

    await this.ctx.storage.delete('pendingCommandDraft')
    this.ctx.waitUntil(this.broadcast(metadata))

    return this.json(await this.response(metadata, actorUserId))
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

  private randomSetup(metadata: GameRoomMetadata): RulesGameSetup {
    const turnOrder = this.shuffle(metadata.members.map((member) => member.player))

    if (metadata.capacity === 2) {
      return {
        players: turnOrder.map((player, index) => ({
          id: player,
          team: `team-${index + 1}`,
        })),
        turnOrder,
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
    playableFormations = [],
  ): Promise<GameRoomResponse> {
    const currentMetadata = metadata ?? await this.requireMetadata()
    const viewer = actorUserId
      ? (this.playerFor(currentMetadata, actorUserId) ?? 'observer')
      : 'observer'

    if (currentMetadata.status === 'Waiting' || currentMetadata.status === 'Dissolved') {
      return {
        gameId: currentMetadata.gameId,
        metadata: currentMetadata,
        state: emptyPublicState(currentMetadata.players),
        events: (await this.events()).map((event) => ({
          id: `room-event-${event.sequence}`,
          eventType: event.type,
          title: this.eventTitle(event),
          summary: this.displaySummary(currentMetadata, this.eventSummary(event)),
        })).reverse(),
        playableFormations,
        canCancelPendingCommand: false,
        interaction: {
          canPass: false,
          hasOptionalEffect: false,
        },
      }
    }

    const draft = actorUserId ? await this.pendingDraft() : undefined
    const canCancelPendingCommand = draft?.actorUserId === actorUserId
    const snapshot = canCancelPendingCommand ? draft.snapshot : await this.requireSnapshot()
    const publicRules = await this.callRules({ type: 'refresh' }, viewer, snapshot)

    return {
      gameId: currentMetadata.gameId,
      metadata: currentMetadata,
      state: publicRules.state,
      events: publicRules.events.map((event) => ({
        ...event,
        summary: this.displaySummary(currentMetadata, event.summary),
      })),
      playableFormations,
      canCancelPendingCommand,
      interaction: publicRules.interaction,
    }
  }

  private playerFor(metadata: GameRoomMetadata, userId: string): PlayerId | undefined {
    return this.memberFor(metadata, userId)?.player
  }

  private memberFor(metadata: GameRoomMetadata, userId: string) {
    return metadata.members.find((member) => member.userId === userId)
  }

  private actionForPlayer(action: OnlineGameAction, player: PlayerId): OnlineGameAction {
    switch (action.type) {
      case 'performFormation':
      case 'chooseTurnDiscard':
      case 'answerEffectChoice':
      case 'playableFormations':
        return { ...action, player }
      default:
        return action
    }
  }

  private async callRules(
    action: OnlineGameAction,
    viewer: string | undefined,
    snapshot: GameRoomSnapshot,
  ) {
    return await callRulesEngine({
      action,
      viewer,
      setup: snapshot.setup,
      deckSeed: snapshot.deckSeed,
      record: snapshot.rulesRecord,
    })
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

  private async requireSnapshot(): Promise<GameRoomSnapshot> {
    const snapshot = await this.ctx.storage.get<GameRoomSnapshot>('snapshot')

    if (!snapshot) {
      throw new Error('game room snapshot is missing')
    }

    return snapshot
  }

  private async pendingDraft(): Promise<PendingCommandDraft | undefined> {
    return await this.ctx.storage.get<PendingCommandDraft>('pendingCommandDraft')
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

  private async findEventByCommandId(commandId: string): Promise<StoredGameEvent | undefined> {
    return (await this.events()).find((event) => event.commandId === commandId)
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
      PlayersReturnedToRoom: '玩家已返回等待房間。',
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
        performFormationWithChoices: `${actor} 已完成陣法與效果選擇。`,
        chooseTurnDiscard: `${actor} 已完成棄牌。`,
        answerEffectChoice: `${actor} 已完成效果選擇。`,
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
