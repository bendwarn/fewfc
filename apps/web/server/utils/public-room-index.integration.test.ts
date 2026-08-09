import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import type { H3Event } from 'h3'
import { Miniflare } from 'miniflare'
import type { GameRoomMember, GameRoomResponse } from '../../shared/game-room'
import { listPlayerRooms, listPublicRooms, upsertPublicRoom } from './public-room-index'
import type { WorkerEnv } from './worker-env'

const gameId = 'room-joined-public'
const createdAt = '2026-08-10T00:00:00.000Z'
const host: GameRoomMember = {
  userId: 'host-user',
  displayName: '房主',
  player: 'player-1' as never,
  ready: false,
  connected: true,
  owner: true,
}
const guest: GameRoomMember = {
  userId: 'guest-user',
  displayName: '訪客',
  player: 'player-2' as never,
  ready: false,
  connected: true,
  owner: false,
}

function roomResponse(members: GameRoomMember[]): GameRoomResponse {
  return {
    gameId,
    invitation: { roomCode: 'JOIN123', inviteToken: 'token-123' },
    metadata: {
      schemaVersion: 4,
      gameId,
      name: '已加入公開房間',
      access: 'public',
      capacity: 2,
      ruleset: 'fewfc-base',
      enabledRuleModules: ['star'],
      players: ['player-1', 'player-2'] as never,
      members,
      status: 'Waiting',
      createdAt,
      updatedAt: createdAt,
    },
  } as unknown as GameRoomResponse
}

describe('public room index', () => {
  let miniflare: Miniflare
  let previousEnv: WorkerEnv | undefined
  let notificationCount = 0

  beforeEach(async () => {
    miniflare = new Miniflare({
      modules: true,
      script: "export default { fetch() { return new Response('ok') } }",
      d1Databases: ['DB'],
    })
    const DB = await miniflare.getD1Database('DB')
    previousEnv = globalThis.__env__
    notificationCount = 0
    const notifications = {
      idFromName: () => 'global',
      get: () => ({
        fetch: async () => {
          notificationCount += 1
          return new Response(null, { status: 204 })
        },
      }),
    }
    globalThis.__env__ = {
      APP_ENV: 'development',
      DB,
      GAME_ROOM: notifications,
      PLAYER_NOTIFICATIONS: notifications,
      REPLAY: notifications,
    }
  })

  afterEach(async () => {
    if (previousEnv) globalThis.__env__ = previousEnv
    else delete globalThis.__env__
    await miniflare.dispose()
  })

  test('indexes a joined public room for both members and removes a full room from public discovery', async () => {
    const event = { context: {} } as H3Event

    await upsertPublicRoom(event, roomResponse([host]), { name: '已加入公開房間' })
    expect((await listPublicRooms(event)).map(room => room.gameId)).toEqual([gameId])
    expect((await listPlayerRooms(event, host.userId)).map(room => room.gameId)).toEqual([gameId])

    await upsertPublicRoom(event, roomResponse([host, guest]))

    expect(await listPublicRooms(event)).toEqual([])
    expect((await listPlayerRooms(event, host.userId)).map(room => room.name)).toEqual(['已加入公開房間'])
    expect((await listPlayerRooms(event, guest.userId)).map(room => room.gameId)).toEqual([gameId])
    expect(notificationCount).toBe(2)
  })
})
