import type { H3Event } from 'h3'
import type { GameRoomMember, GameRoomObserver, GameRoomResponse, GameRoomStatus, GameRoomAccess } from '../../shared/game-room'
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
  room_code: string | null
  name: string
  access: GameRoomAccess
  status: GameRoomStatus
  owner_user_id: string
  players_json: string
  members_json: string
  observers_json?: string
  enabled_rule_modules_json: string
  created_at: number
  updated_at: number
}

export interface PublicRoomSummary {
  gameId: string
  roomCode: string
  name: string
  access: GameRoomAccess
  status: GameRoomStatus
  ownerUserId: string
  players: string[]
  members: GameRoomMember[]
  observers: GameRoomObserver[]
  capacity: number
  enabledRuleModules: string[]
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
        room_code text UNIQUE,
        name text NOT NULL,
        access text NOT NULL,
        status text NOT NULL,
        owner_user_id text NOT NULL,
        players_json text NOT NULL,
        members_json text NOT NULL,
        observers_json text NOT NULL DEFAULT '[]',
        enabled_rule_modules_json text NOT NULL DEFAULT '[]',
        created_at integer NOT NULL,
        updated_at integer NOT NULL
      )
    `)
    .run()

  try {
    await db(event).prepare("ALTER TABLE public_game_room ADD COLUMN observers_json text NOT NULL DEFAULT '[]'").run()
  } catch (error) {
    // 已建立的索引資料庫已經有這個欄位；其他 D1 錯誤不可被當作遷移成功。
    if (!(error instanceof Error) || !/duplicate column name/i.test(error.message)) {
      throw error
    }
  }

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
  const observers = parseJson<GameRoomObserver[]>(row.observers_json ?? '[]', [])

  return {
    gameId: row.game_id,
    roomCode: row.room_code || row.game_id,
    name: row.name,
    access: row.access,
    status: row.status,
    ownerUserId: row.owner_user_id,
    players,
    members,
    observers,
    capacity: players.length,
    enabledRuleModules: parseJson<string[]>(row.enabled_rule_modules_json, []),
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
  const invitation = response.invitation

  await db(event)
    .prepare(`
      INSERT INTO public_game_room (
        game_id,
        room_code,
        name,
        access,
        status,
        owner_user_id,
        players_json,
        members_json,
        observers_json,
        enabled_rule_modules_json,
        created_at,
        updated_at
      )
      VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
      ON CONFLICT(game_id) DO UPDATE SET
        room_code = CASE
          WHEN ? = 1 THEN excluded.room_code
          ELSE public_game_room.room_code
        END,
        name = CASE
          WHEN ? = 1 THEN excluded.name
          ELSE public_game_room.name
        END,
        access = excluded.access,
        status = excluded.status,
        owner_user_id = excluded.owner_user_id,
        players_json = excluded.players_json,
        members_json = excluded.members_json,
        observers_json = excluded.observers_json,
        enabled_rule_modules_json = excluded.enabled_rule_modules_json,
        updated_at = excluded.updated_at
    `)
    .bind(
      metadata.gameId,
      invitation?.roomCode ?? metadata.gameId,
      name,
      metadata.access,
      metadata.status,
      owner?.userId ?? '',
      JSON.stringify(metadata.players),
      JSON.stringify(metadata.members),
      JSON.stringify(metadata.observers),
      JSON.stringify(metadata.enabledRuleModules),
      timestamp(metadata.createdAt),
      timestamp(metadata.updatedAt),
      invitation ? 1 : 0,
      nameOverride ? 1 : 0,
    )
    .run()

  await db(event)
    .prepare('DELETE FROM game_room_member WHERE game_id = ?')
    .bind(metadata.gameId)
    .run()

  for (const member of [...metadata.members, ...metadata.observers]) {
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
        room_code,
        name,
        access,
        status,
        owner_user_id,
        players_json,
        members_json,
        observers_json,
        enabled_rule_modules_json,
        created_at,
        updated_at
      FROM public_game_room
      WHERE access = 'public' AND status IN ('Waiting', 'Active')
      ORDER BY updated_at DESC
      LIMIT 30
    `)
    .all<PublicRoomRow>()

  return (result.results ?? [])
    .map(rowToSummary)
    .filter((room) => room.status === 'Active' || room.members.length <= room.capacity)
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
        room.room_code,
        room.name,
        room.access,
        room.status,
        room.owner_user_id,
        room.players_json,
        room.members_json,
        room.observers_json,
        room.enabled_rule_modules_json,
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

export async function gameIdForRoomCode(
  event: H3Event,
  roomCode: string,
): Promise<string | undefined> {
  await ensurePublicRoomTable(event)

  const result = await db(event)
    .prepare(`
      SELECT game_id
      FROM public_game_room
      WHERE room_code = ? AND status IN ('Waiting', 'Active')
      LIMIT 1
    `)
    .bind(roomCode.toUpperCase())
    .all<{ game_id: string }>()

  return result.results?.[0]?.game_id
}
