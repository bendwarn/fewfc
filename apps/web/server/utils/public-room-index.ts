import type { H3Event } from 'h3'
import type { GameRoomMember, GameRoomResponse, GameRoomStatus, GameRoomAccess } from '../../shared/game-room'
import { workerEnv } from './worker-env'

interface PublicRoomStatement {
  bind(...values: unknown[]): PublicRoomStatement
  run(): Promise<unknown>
  all<T>(): Promise<{ results?: T[] }>
}

interface PublicRoomDatabase {
  prepare(query: string): PublicRoomStatement
}

interface PublicRoomRow {
  game_id: string
  name: string
  access: GameRoomAccess
  status: GameRoomStatus
  owner_user_id: string
  players_json: string
  members_json: string
  created_at: number
  updated_at: number
}

export interface PublicRoomSummary {
  gameId: string
  name: string
  access: GameRoomAccess
  status: GameRoomStatus
  ownerUserId: string
  players: string[]
  members: GameRoomMember[]
  capacity: number
  createdAt: string
  updatedAt: string
}

function db(event: H3Event): PublicRoomDatabase {
  return workerEnv(event).DB as PublicRoomDatabase
}

async function ensurePublicRoomTable(event: H3Event) {
  await db(event)
    .prepare(`
      CREATE TABLE IF NOT EXISTS public_game_room (
        game_id text PRIMARY KEY NOT NULL,
        name text NOT NULL,
        access text NOT NULL,
        status text NOT NULL,
        owner_user_id text NOT NULL,
        players_json text NOT NULL,
        members_json text NOT NULL,
        created_at integer NOT NULL,
        updated_at integer NOT NULL
      )
    `)
    .run()

  await db(event)
    .prepare(`
      CREATE TABLE IF NOT EXISTS game_room_member (
        game_id text NOT NULL,
        user_id text NOT NULL,
        PRIMARY KEY (game_id, user_id)
      )
    `)
    .run()

  await db(event)
    .prepare('CREATE INDEX IF NOT EXISTS game_room_member_user_idx ON game_room_member (user_id)')
    .run()

  await db(event)
    .prepare('CREATE INDEX IF NOT EXISTS public_game_room_access_status_idx ON public_game_room (access, status)')
    .run()

  await db(event)
    .prepare('CREATE INDEX IF NOT EXISTS public_game_room_updated_at_idx ON public_game_room (updated_at)')
    .run()
}

function timestamp(value: string): number {
  return Date.parse(value) || Date.now()
}

function parseJson<T>(value: string, fallback: T): T {
  try {
    return JSON.parse(value) as T
  } catch {
    return fallback
  }
}

function rowToSummary(row: PublicRoomRow): PublicRoomSummary {
  const players = parseJson<string[]>(row.players_json, [])
  const members = parseJson<GameRoomMember[]>(row.members_json, [])

  return {
    gameId: row.game_id,
    name: row.name,
    access: row.access,
    status: row.status,
    ownerUserId: row.owner_user_id,
    players,
    members,
    capacity: players.length,
    createdAt: new Date(row.created_at).toISOString(),
    updatedAt: new Date(row.updated_at).toISOString(),
  }
}

export async function upsertPublicRoom(
  event: H3Event,
  response: GameRoomResponse,
  options: {
    name?: string
  } = {},
) {
  const metadata = response.metadata

  await ensurePublicRoomTable(event)

  const owner = metadata.members[0]
  const nameOverride = options.name?.trim()
  const name = nameOverride || metadata.gameId

  await db(event)
    .prepare(`
      INSERT INTO public_game_room (
        game_id,
        name,
        access,
        status,
        owner_user_id,
        players_json,
        members_json,
        created_at,
        updated_at
      )
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(game_id) DO UPDATE SET
        name = CASE
          WHEN ? = 1 THEN excluded.name
          ELSE public_game_room.name
        END,
        access = excluded.access,
        status = excluded.status,
        owner_user_id = excluded.owner_user_id,
        players_json = excluded.players_json,
        members_json = excluded.members_json,
        updated_at = excluded.updated_at
    `)
    .bind(
      metadata.gameId,
      name,
      metadata.access,
      metadata.status,
      owner?.userId ?? '',
      JSON.stringify(metadata.players),
      JSON.stringify(metadata.members),
      timestamp(metadata.createdAt),
      timestamp(metadata.updatedAt),
      nameOverride ? 1 : 0,
    )
    .run()

  await db(event)
    .prepare('DELETE FROM game_room_member WHERE game_id = ?')
    .bind(metadata.gameId)
    .run()

  for (const member of metadata.members) {
    await db(event)
      .prepare('INSERT OR IGNORE INTO game_room_member (game_id, user_id) VALUES (?, ?)')
      .bind(metadata.gameId, member.userId)
      .run()
  }

  const notifications = workerEnv(event).PLAYER_NOTIFICATIONS
  const notificationHub = notifications.get(notifications.idFromName('global'))
  await notificationHub.fetch('https://player-notifications.internal/rooms-changed', {
    method: 'POST',
  })
}

export async function updatePublicRoom(event: H3Event, response: GameRoomResponse) {
  await upsertPublicRoom(event, response)
}

export async function listPublicRooms(event: H3Event): Promise<PublicRoomSummary[]> {
  await ensurePublicRoomTable(event)

  const result = await db(event)
    .prepare(`
      SELECT
        game_id,
        name,
        access,
        status,
        owner_user_id,
        players_json,
        members_json,
        created_at,
        updated_at
      FROM public_game_room
      WHERE access = 'public' AND status = 'Waiting'
      ORDER BY updated_at DESC
      LIMIT 30
    `)
    .all<PublicRoomRow>()

  return (result.results ?? [])
    .map(rowToSummary)
    .filter((room) => room.members.length < room.capacity)
}

export async function listPlayerRooms(
  event: H3Event,
  userId: string,
): Promise<PublicRoomSummary[]> {
  await ensurePublicRoomTable(event)

  const result = await db(event)
    .prepare(`
      SELECT
        room.game_id,
        room.name,
        room.access,
        room.status,
        room.owner_user_id,
        room.players_json,
        room.members_json,
        room.created_at,
        room.updated_at
      FROM public_game_room AS room
      INNER JOIN game_room_member AS member ON member.game_id = room.game_id
      WHERE member.user_id = ? AND room.status != 'Dissolved'
      ORDER BY room.updated_at DESC
      LIMIT 50
    `)
    .bind(userId)
    .all<PublicRoomRow>()

  return (result.results ?? []).map(rowToSummary)
}
