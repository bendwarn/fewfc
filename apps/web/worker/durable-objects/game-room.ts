import { DurableObject } from 'cloudflare:workers'
import {
  emptyPublicState,
  type GameRoomMetadata,
  type GameRoomRequest,
  type GameRoomResponse,
  type GameRoomSnapshot,
  type GameRoomAccess,
  type OnlineGameAction,
  type StoredGameEvent,
} from '../../shared/game-room'
import type { PlayerId } from '../../app/types/fewfc'
import { callRulesEngine } from '../rules-engine'

export class GameRoom extends DurableObject {
  async fetch(request: Request): Promise<Response> {
    if (request.method === 'GET') {
      return this.json(await this.response())
    }

    if (request.method !== 'POST') {
      return this.json({ error: 'method not allowed' }, 405)
    }

    const body = (await request.json()) as GameRoomRequest

    switch (body.type) {
      case 'createGame':
        return this.json(await this.createGame(body.gameId, body.actorUserId, body.access, body.players))
      case 'joinGame':
        return await this.joinGame(body.actorUserId)
      case 'readyGame':
        return await this.readyGame(body.actorUserId)
      case 'startGame':
        return await this.startGame(body.actorUserId)
      case 'getState':
        return this.json(await this.response(undefined, body.actorUserId))
      case 'submitCommand':
        return await this.submitCommand(body)
      default:
        return this.json({ error: 'unknown game-room request' }, 400)
    }
  }

  private async createGame(
    gameId: string,
    ownerUserId: string,
    access: GameRoomAccess = 'private',
    players: PlayerId[] = ['alice', 'bob'],
  ) {
    return await this.ctx.storage.transaction(async () => {
      const existing = await this.metadata()

      if (existing) {
        return await this.response(existing, ownerUserId)
      }

      const now = new Date().toISOString()
      const metadata: GameRoomMetadata = {
        schemaVersion: 1,
        gameId,
        access,
        ruleset: 'fewfc-base',
        players,
        members: [{ userId: ownerUserId, player: players[0] ?? 'alice', ready: true }],
        status: 'Waiting',
        createdAt: now,
        updatedAt: now,
      }
      const initialEvent: StoredGameEvent = {
        sequence: 1,
        type: 'GameCreated',
        payload: {
          players,
          ruleset: metadata.ruleset,
        },
        createdAt: now,
      }

      await this.ctx.storage.put('metadata', metadata)
      await this.ctx.storage.put('nextSequence', 2)
      await this.ctx.storage.put(this.eventKey(initialEvent.sequence), initialEvent)

      return await this.response(metadata, ownerUserId)
    })
  }

  private async joinGame(actorUserId: string): Promise<Response> {
    return await this.ctx.storage.transaction(async () => {
      const metadata = await this.requireMetadata()

      if (metadata.status !== 'Waiting') {
        return this.json({ error: 'room has already started' }, 409)
      }

      if (metadata.access !== 'public') {
        return this.json({ error: 'room is not public' }, 403)
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
      const updatedMetadata: GameRoomMetadata = {
        ...metadata,
        members: [
          ...metadata.members,
          { userId: actorUserId, player: openPlayer, ready: false },
        ],
        updatedAt: now,
      }
      const sequence = await this.nextSequence()
      const event: StoredGameEvent = {
        sequence,
        type: 'PlayerJoined',
        actor: openPlayer,
        payload: {
          player: openPlayer,
        },
        createdAt: now,
      }

      await this.ctx.storage.put('metadata', updatedMetadata)
      await this.ctx.storage.put('nextSequence', sequence + 1)
      await this.ctx.storage.put(this.eventKey(sequence), event)

      return this.json(await this.response(updatedMetadata, actorUserId))
    })
  }

  private async readyGame(actorUserId: string): Promise<Response> {
    return await this.ctx.storage.transaction(async () => {
      const metadata = await this.requireMetadata()
      const actor = this.memberFor(metadata, actorUserId)

      if (!actor) {
        return this.json({ error: 'only room players may ready' }, 403)
      }

      if (metadata.status !== 'Waiting') {
        return this.json({ error: 'room has already started' }, 409)
      }

      const now = new Date().toISOString()
      const updatedMetadata: GameRoomMetadata = {
        ...metadata,
        members: metadata.members.map((member) => (
          member.userId === actorUserId ? { ...member, ready: true } : member
        )),
        updatedAt: now,
      }
      const sequence = await this.nextSequence()
      const event: StoredGameEvent = {
        sequence,
        type: 'PlayerReady',
        actor: actor.player,
        payload: {
          player: actor.player,
        },
        createdAt: now,
      }

      await this.ctx.storage.put('metadata', updatedMetadata)
      await this.ctx.storage.put('nextSequence', sequence + 1)
      await this.ctx.storage.put(this.eventKey(sequence), event)

      return this.json(await this.response(updatedMetadata, actorUserId))
    })
  }

  private async startGame(actorUserId: string): Promise<Response> {
    return await this.ctx.storage.transaction(async () => {
      const metadata = await this.requireMetadata()
      const owner = metadata.members[0]

      if (owner?.userId !== actorUserId) {
        return this.json({ error: 'only room owner may start' }, 403)
      }

      if (metadata.status !== 'Waiting') {
        return this.json({ error: 'room has already started' }, 409)
      }

      if (metadata.members.length < metadata.players.length) {
        return this.json({ error: 'room is waiting for players' }, 409)
      }

      const requiresReadyCheck = metadata.players.length > 2
      const unreadyMember = requiresReadyCheck
        ? metadata.members.slice(1).find((member) => !member.ready)
        : undefined

      if (unreadyMember) {
        return this.json({ error: 'not all joined players are ready' }, 409)
      }

      const rules = await callRulesEngine({
        action: { type: 'start' },
        viewer: 'observer',
      })
      const now = new Date().toISOString()
      const sequence = await this.nextSequence()
      const event: StoredGameEvent = {
        sequence,
        type: 'GameStarted',
        actor: owner.player,
        payload: {
          players: metadata.players,
        },
        createdAt: now,
      }
      const snapshot: GameRoomSnapshot = {
        schemaVersion: 2,
        sequence,
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

      return this.json(await this.response(updatedMetadata, actorUserId))
    })
  }

  private async submitCommand(request: Extract<GameRoomRequest, { type: 'submitCommand' }>): Promise<Response> {
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

    if (request.action.type === 'playableFormations') {
      const snapshot = await this.requireSnapshot()
      const rules = await this.callRules(action, viewer, snapshot)

      return this.json(await this.response(metadata, request.actorUserId, rules.playableFormations))
    }

    return await this.ctx.storage.transaction(async () => {
      const duplicate = await this.findEventByCommandId(request.commandId)

      if (duplicate) {
        return this.json(await this.response(metadata, request.actorUserId))
      }

      const previousSnapshot = await this.requireSnapshot()
      const rules = await this.callRules(action, viewer, previousSnapshot)
      const now = new Date().toISOString()
      const sequence = await this.nextSequence()
      const event: StoredGameEvent = {
        sequence,
        type: 'RulesCommandApplied',
        commandId: request.commandId,
        actor,
        payload: action,
        createdAt: now,
      }
      const snapshot: GameRoomSnapshot = {
        schemaVersion: 2,
        sequence,
        rulesRecord: rules.record,
      }

      await this.ctx.storage.put(this.eventKey(sequence), event)
      await this.ctx.storage.put('nextSequence', sequence + 1)
      await this.ctx.storage.put('snapshot', snapshot)
      await this.ctx.storage.put('metadata', {
        ...metadata,
        status: rules.state.status === 'Finished' ? 'Finished' : 'Active',
        updatedAt: now,
      })

      return this.json(await this.response(
        {
          ...metadata,
          status: rules.state.status === 'Finished' ? 'Finished' : 'Active',
          updatedAt: now,
        },
        request.actorUserId,
      ))
    })
  }

  private async response(
    metadata?: GameRoomMetadata,
    actorUserId?: string,
    playableFormations = [],
  ): Promise<GameRoomResponse> {
    const currentMetadata = metadata ?? (await this.requireMetadata())
    const viewer = actorUserId ? (this.playerFor(currentMetadata, actorUserId) ?? 'observer') : 'observer'

    if (currentMetadata.status === 'Waiting') {
      return {
        gameId: currentMetadata.gameId,
        metadata: currentMetadata,
        state: emptyPublicState(currentMetadata.players),
        events: (await this.events()).map((event) => ({
          id: `room-event-${event.sequence}`,
          eventType: event.type,
          summary: this.eventSummary(event),
        })).reverse(),
        playableFormations,
      }
    }

    const snapshot = await this.requireSnapshot()
    const publicRules = await this.callRules({ type: 'refresh' }, viewer, snapshot)

    return {
      gameId: currentMetadata.gameId,
      metadata: currentMetadata,
      state: publicRules.state,
      events: publicRules.events,
      playableFormations,
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
        return { ...action, player }
      case 'chooseTurnDiscard':
        return { ...action, player }
      case 'answerEffectChoice':
        return { ...action, player }
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
      record: snapshot.rulesRecord,
    })
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

  private async metadata(): Promise<GameRoomMetadata | undefined> {
    return await this.ctx.storage.get<GameRoomMetadata>('metadata')
  }

  private async nextSequence(): Promise<number> {
    return (await this.ctx.storage.get<number>('nextSequence')) ?? 1
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
    if (event.type === 'GameCreated') {
      return '遊戲房間已建立。'
    }

    if (event.type === 'RulesCommandApplied') {
      return `收到 ${event.actor ?? '系統'} 的 ${this.payloadType(event.payload)} 指令。`
    }

    if (event.type === 'PlayerJoined') {
      return `${event.actor ?? '玩家'} 已加入房間。`
    }

    if (event.type === 'PlayerReady') {
      return `${event.actor ?? '玩家'} 已準備。`
    }

    if (event.type === 'GameStarted') {
      return '遊戲已開始。'
    }

    return event.type
  }

  private payloadType(payload: unknown): string {
    if (payload && typeof payload === 'object' && 'type' in payload) {
      return String(payload.type)
    }

    return 'unknown'
  }

  private json(body: unknown, status = 200): Response {
    return Response.json(body, { status })
  }
}
