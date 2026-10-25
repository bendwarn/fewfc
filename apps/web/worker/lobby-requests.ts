import type { WorkerEnv } from '../server/utils/worker-env'
import { callGameRoom } from '../server/utils/game-room'
import { gameIdForRoomCode, listPlayerRooms, listPublicRooms, updatePublicRoom, upsertPublicRoom } from '../server/utils/public-room-index'
import type { GameRoomLobbyItem, GameRoomResponse } from '../shared/game-room'
import type { LobbyOperations, SocketRequest } from '../shared/realtime'
import { callRuleModuleResolution } from './rules-engine'
import { realtimeEvent, type RealtimeIdentity } from './realtime-handler'
import { completedReplayDraft, saveReplayReference } from '../server/utils/replay'

export async function executeLobbyRequest(env: WorkerEnv, actor: RealtimeIdentity, request: SocketRequest<LobbyOperations>) {
  const event = realtimeEvent(env)
  switch (request.operation) {
    case 'saveReplay': {
      const draft = await completedReplayDraft(event, request.input.sourceGameId, actor.userId)
      return await saveReplayReference(event, actor.userId, draft)
    }
    case 'list': {
      const [rooms, myRooms] = await Promise.all([listPublicRooms(event), listPlayerRooms(event, actor.userId)])
      const item = (room: typeof rooms[number]): GameRoomLobbyItem => ({
        gameId: room.gameId, roomCode: room.roomCode, name: room.name, status: room.status,
        memberCount: room.members.length, observerCount: room.observers.length, capacity: room.capacity,
        ruleVersion: room.ruleVersion, enabledRuleModules: room.enabledRuleModules,
        isMember: room.members.some(member => member.userId === actor.userId),
        isObserver: room.observers.some(observer => observer.userId === actor.userId),
      })
      return { rooms: rooms.map(item), myRooms: myRooms.map(item) }
    }
    case 'create': {
      const input = request.input
      const ruleVersion = input.ruleVersion ?? '5.17'
      const { modules } = await callRuleModuleResolution(input.enabledRuleModules, ruleVersion)
      const alphabet = 'ABCDEFGHJKLMNPQRSTUVWXYZ23456789'
      const roomCode = [...crypto.getRandomValues(new Uint8Array(7))].map(byte => alphabet[byte % alphabet.length]).join('')
      // 同一個建立請求重送時使用同一個房間身分，不會建立第二間房。
      const digest = await crypto.subtle.digest('SHA-256', new TextEncoder().encode(`${actor.userId}:${request.requestId}`))
      const gameId = [...new Uint8Array(digest)].map(byte => byte.toString(16).padStart(2, '0')).join('')
      const name = input.name?.trim() || `${actor.userName || '玩家'}的房間`
      const response = await callGameRoom(event, gameId, {
        type: 'createGame', gameId, actorUserId: actor.userId, actorName: actor.userName,
        access: input.access ?? 'private', capacity: input.capacity ?? 2, name, ruleVersion,
        enabledRuleModules: modules, invitation: { roomCode, inviteToken: crypto.randomUUID() },
      })
      await upsertPublicRoom(event, response, { name })
      return { gameId: response.gameId }
    }
    case 'joinCode': {
      const code = request.input.code.trim().toUpperCase()
      const gameId = await gameIdForRoomCode(event, code)
      if (!gameId) throw { statusCode: 404, code: 'roomNotFound', message: '找不到這個房間。' }
      const response = await callGameRoom(event, gameId, {
        type: 'joinGame', actorUserId: actor.userId, actorName: actor.userName, credential: { type: 'code', value: code },
      })
      await updatePublicRoom(event, response)
      return { gameId: response.gameId }
    }
    case 'enter': {
      const { gameId, invite } = request.input
      let response: GameRoomResponse
      try {
        response = await callGameRoom(event, gameId, { type: 'getState', actorUserId: actor.userId })
      } catch (error) {
        const status = (error as { statusCode?: number }).statusCode
        if (status !== 403 && status !== 404) throw error
        response = await callGameRoom(event, gameId, {
          type: 'joinGame', actorUserId: actor.userId, actorName: actor.userName,
          credential: invite ? { type: 'token', value: invite } : undefined,
        })
      }
      await updatePublicRoom(event, response)
      return response
    }
  }
}
