import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import type { H3Event } from 'h3'
import { convertV4MiniflareOptions, Miniflare } from 'miniflare'
import type { GameRoomMember, GameRoomObserver, GameRoomResponse } from '../../shared/game-room'
import { gameIdForRoomCode, listPlayerRooms, listPublicRooms, upsertPublicRoom } from './public-room-index'
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

function roomResponse(members: GameRoomMember[], observers: GameRoomObserver[] = []): GameRoomResponse {
  return {
    gameId,
    invitation: { roomCode: 'JOIN123', inviteToken: 'token-123' },
    metadata: {
      schemaVersion: 5,
      gameId,
      name: '已加入公開房間',
      access: 'public',
      capacity: 2,
      ruleset: 'fewfc-base',
      enabledRuleModules: ['star'],
      players: ['player-1', 'player-2'] as never,
      members,
      observers,
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
    miniflare = new Miniflare(convertV4MiniflareOptions({
      modules: true,
      script: "export default { fetch() { return new Response('ok') } }",
      d1Databases: ['DB'],
    }))
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
  }, { timeout: 15_000 })

  afterEach(async () => {
    if (previousEnv) globalThis.__env__ = previousEnv
    else delete globalThis.__env__
    await miniflare.dispose()
  })

  test('keeps full public rooms discoverable and indexes observers in my rooms', async () => {
    const event = { context: {} } as H3Event

    await upsertPublicRoom(event, roomResponse([host]), { name: '已加入公開房間' })
    expect((await listPublicRooms(event)).map(room => room.gameId)).toEqual([gameId])
    expect((await listPlayerRooms(event, host.userId)).map(room => room.gameId)).toEqual([gameId])

    const observer: GameRoomObserver = {
      userId: 'observer-user', displayName: '觀戰者', connected: true,
    }
    await upsertPublicRoom(event, roomResponse([host, guest], [observer]))

    expect((await listPublicRooms(event)).map(room => room.gameId)).toEqual([gameId])
    expect((await listPlayerRooms(event, host.userId)).map(room => room.name)).toEqual(['已加入公開房間'])
    expect((await listPlayerRooms(event, guest.userId)).map(room => room.gameId)).toEqual([gameId])
    expect((await listPlayerRooms(event, observer.userId)).map(room => room.gameId)).toEqual([gameId])
    expect(notificationCount).toBe(2)
  })

  test('discovers active public rooms but closes finished live-observation entry', async () => {
    const event = { context: {} } as H3Event
    const response = roomResponse([host, guest])
    response.metadata.status = 'Active'

    await upsertPublicRoom(event, response)
    expect((await listPublicRooms(event)).map(room => room.gameId)).toEqual([gameId])
    expect(await gameIdForRoomCode(event, 'join123')).toBe(gameId)

    response.metadata.status = 'Finished'
    await upsertPublicRoom(event, response)
    expect(await listPublicRooms(event)).toEqual([])
    expect(await gameIdForRoomCode(event, 'JOIN123')).toBeUndefined()
    expect((await listPlayerRooms(event, host.userId)).map(room => room.gameId)).toEqual([gameId])
  })

  test('keeps private rooms out of discovery while full and active room codes remain usable', async () => {
    const event = { context: {} } as H3Event
    const response = roomResponse([host, guest])
    response.metadata.access = 'private'

    await upsertPublicRoom(event, response)
    expect(await listPublicRooms(event)).toEqual([])
    expect(await gameIdForRoomCode(event, 'JOIN123')).toBe(gameId)

    response.metadata.status = 'Active'
    await upsertPublicRoom(event, response)
    expect(await listPublicRooms(event)).toEqual([])
    expect(await gameIdForRoomCode(event, 'JOIN123')).toBe(gameId)
  })

  test('preserves observer queue details and removes departed observers from my rooms', async () => {
    const event = { context: {} } as H3Event
    const first: GameRoomObserver = {
      userId: 'first-observer', displayName: '第一位', connected: false,
    }
    const second: GameRoomObserver = {
      userId: 'second-observer', displayName: '第二位', connected: true,
    }
    const response = roomResponse([host, guest], [first, second])

    await upsertPublicRoom(event, response)
    expect((await listPlayerRooms(event, first.userId))[0]?.observers).toEqual([first, second])

    response.metadata.observers = [second]
    await upsertPublicRoom(event, response)
    expect(await listPlayerRooms(event, first.userId)).toEqual([])
    expect((await listPlayerRooms(event, second.userId))[0]?.observers).toEqual([second])
  })

  test('adds observer storage to an existing room index without losing its rows', async () => {
    const database = await miniflare.getD1Database('DB')
    await database.prepare(`
      CREATE TABLE public_game_room (
        game_id text PRIMARY KEY NOT NULL,
        room_code text UNIQUE,
        name text NOT NULL,
        access text NOT NULL,
        status text NOT NULL,
        owner_user_id text NOT NULL,
        players_json text NOT NULL,
        members_json text NOT NULL,
        enabled_rule_modules_json text NOT NULL DEFAULT '[]',
        created_at integer NOT NULL,
        updated_at integer NOT NULL
      )
    `).run()
    await database.prepare(`
      INSERT INTO public_game_room
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
    `).bind(
      gameId, 'JOIN123', '既有房間', 'public', 'Waiting', host.userId,
      JSON.stringify(['player-1', 'player-2']), JSON.stringify([host]), '[]',
      Date.parse(createdAt), Date.parse(createdAt),
    ).run()

    const event = { context: {} } as H3Event
    expect((await listPublicRooms(event)).map(room => ({
      gameId: room.gameId, name: room.name, observers: room.observers,
    }))).toEqual([{ gameId, name: '既有房間', observers: [] }])

    const observer: GameRoomObserver = {
      userId: 'observer-user', displayName: '觀戰者', connected: true,
    }
    await upsertPublicRoom(event, roomResponse([host, guest], [observer]))
    expect((await listPlayerRooms(event, observer.userId))[0]?.observers).toEqual([observer])
  })
})
