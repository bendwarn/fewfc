import { DurableObject } from 'cloudflare:workers'
import {
  type GameRoomMetadata,
  type GameRoomRequest,
  type GameRoomResponse,
  type GameRoomSnapshot,
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
        return this.json(await this.createGame(body.gameId, body.actorUserId, body.players))
      case 'getState':
        return this.json(await this.response(undefined, body.actorUserId))
      case 'submitCommand':
        return this.json(await this.submitCommand(body))
      default:
        return this.json({ error: 'unknown game-room request' }, 400)
    }
  }

  private async createGame(
    gameId: string,
    ownerUserId: string,
    players: PlayerId[] = ['alice', 'bob'],
  ) {
    const rules = await callRulesEngine({
      action: { type: 'start' },
      viewer: 'observer',
    })

    return await this.ctx.storage.transaction(async () => {
      const existing = await this.metadata()

      if (existing) {
        return await this.response(existing, ownerUserId)
      }

      const now = new Date().toISOString()
      const metadata: GameRoomMetadata = {
        schemaVersion: 1,
        gameId,
        ruleset: 'fewfc-base',
        players,
        members: [{ userId: ownerUserId, player: players[0] ?? 'alice' }],
        status: 'Active',
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
      const snapshot: GameRoomSnapshot = {
        schemaVersion: 2,
        sequence: initialEvent.sequence,
        rulesRecord: rules.record,
      }

      await this.ctx.storage.put('metadata', metadata)
      await this.ctx.storage.put('nextSequence', 2)
      await this.ctx.storage.put('snapshot', snapshot)
      await this.ctx.storage.put(this.eventKey(initialEvent.sequence), initialEvent)

      return await this.response(metadata, ownerUserId)
    })
  }

  private async submitCommand(request: Extract<GameRoomRequest, { type: 'submitCommand' }>) {
    const metadata = await this.requireMetadata()
    const actor = this.playerFor(metadata, request.actorUserId)

    if (!actor) {
      return this.json({ error: 'only room players may submit commands' }, 403)
    }

    const action = this.actionForPlayer(request.action, actor)
    const viewer = actor

    if (request.action.type === 'playableFormations') {
      const snapshot = await this.requireSnapshot()
      const rules = await this.callRules(action, viewer, snapshot)

      return await this.response(metadata, request.actorUserId, rules.playableFormations)
    }

    return await this.ctx.storage.transaction(async () => {
      const duplicate = await this.findEventByCommandId(request.commandId)

      if (duplicate) {
        return await this.response(metadata, request.actorUserId)
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

      return await this.response(
        {
          ...metadata,
          status: rules.state.status === 'Finished' ? 'Finished' : 'Active',
          updatedAt: now,
        },
        request.actorUserId,
      )
    })
  }

  private async response(
    metadata?: GameRoomMetadata,
    actorUserId?: string,
    playableFormations = [],
  ): Promise<GameRoomResponse> {
    const currentMetadata = metadata ?? (await this.requireMetadata())
    const snapshot = await this.requireSnapshot()
    const viewer = actorUserId ? (this.playerFor(currentMetadata, actorUserId) ?? 'observer') : 'observer'
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
    return metadata.members.find((member) => member.userId === userId)?.player
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
