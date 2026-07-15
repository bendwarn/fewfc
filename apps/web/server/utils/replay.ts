import { createError, type H3Event } from 'h3'
import type { CompletedReplayDraft } from '../../shared/game-room'
import type { ReplayFrame } from '../../worker/durable-objects/replay-archive'
import { workerEnv, type DurableObjectNamespaceBinding } from './worker-env'

interface D1Result<T = Record<string, unknown>> {
  results?: T[]
  meta?: { changes?: number }
}
interface D1Statement {
  bind(...values: unknown[]): D1Statement
  run<T = Record<string, unknown>>(): Promise<D1Result<T>>
  all<T = Record<string, unknown>>(): Promise<D1Result<T>>
}
interface D1Database {
  prepare(query: string): D1Statement
  batch(statements: D1Statement[]): Promise<D1Result[]>
}

export interface ReplaySummary {
  replayId: string
  sourceGameId: string
  roomName: string
  players: Array<{ player: string; displayName: string }>
  result: unknown
  finishedAt: string
  savedAt: string
}

function db(event: H3Event): D1Database {
  return workerEnv(event).DB as D1Database
}

function replayNamespace(event: H3Event): DurableObjectNamespaceBinding {
  const binding = workerEnv(event).REPLAY
  if (!binding) throw createError({ statusCode: 501, statusMessage: 'Replay storage is unavailable.' })
  return binding
}

async function replayRequest(
  event: H3Event,
  replayId: string,
  input: string,
  init?: RequestInit,
): Promise<Response> {
  const stub = replayNamespace(event).get(replayNamespace(event).idFromName(replayId))
  return await stub.fetch(`https://replay.internal/${input}`, init)
}

export async function completedReplayDraft(
  event: H3Event,
  sourceGameId: string,
  userId: string,
): Promise<CompletedReplayDraft> {
  const room = workerEnv(event).GAME_ROOM.get(workerEnv(event).GAME_ROOM.idFromName(sourceGameId))
  const response = await room.fetch('https://game-room.internal/', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ type: 'getCompletedReplayDraft', actorUserId: userId }),
  })
  if (!response.ok) {
    throw createError({
      statusCode: response.status === 409 ? 409 : response.status,
      statusMessage: 'This completed match is no longer available.',
      data: { code: 'replayNoLongerAvailable' },
    })
  }
  return await response.json() as CompletedReplayDraft
}

export async function createReplayArchive(event: H3Event, draft: CompletedReplayDraft) {
  const response = await replayRequest(event, draft.replayId, 'create', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(draft),
  })
  if (!response.ok) throw createError({ statusCode: response.status, statusMessage: 'Unable to create replay archive.' })
}

export async function replayFrame(event: H3Event, replayId: string, step: number): Promise<ReplayFrame | null> {
  const response = await replayRequest(event, replayId, `?step=${step}`)
  if (response.status === 404) return null
  if (!response.ok) throw createError({ statusCode: response.status, statusMessage: 'Unable to load replay.' })
  return await response.json() as ReplayFrame
}

export async function savedReplaySummaries(event: H3Event, userId: string): Promise<ReplaySummary[]> {
  const result = await db(event).prepare(
    'SELECT replay_id, source_game_id, room_name, players_json, result_json, finished_at, saved_at FROM player_saved_replay WHERE user_id = ? ORDER BY saved_at DESC LIMIT 10',
  ).bind(userId).all<Record<string, string>>()
  return (result.results ?? []).map(row => ({
    replayId: row.replay_id ?? '',
    sourceGameId: row.source_game_id ?? '',
    roomName: row.room_name ?? '',
    players: JSON.parse(row.players_json ?? '[]'),
    result: JSON.parse(row.result_json ?? 'null'),
    finishedAt: row.finished_at ?? '',
    savedAt: row.saved_at ?? '',
  }))
}

export async function saveReplayReference(
  event: H3Event,
  userId: string,
  draft: CompletedReplayDraft,
  replaceReplayId?: string,
) {
  const database = db(event)
  const duplicate = await database.prepare(
    'SELECT replay_id FROM player_saved_replay WHERE user_id = ? AND replay_id = ?',
  ).bind(userId, draft.replayId).all()
  if ((duplicate.results?.length ?? 0) > 0) return { saved: true, replayId: draft.replayId }

  const count = await database.prepare(
    'SELECT COUNT(*) AS count FROM player_saved_replay WHERE user_id = ?',
  ).bind(userId).all<{ count: number }>()
  const used = Number(count.results?.[0]?.count ?? 0)
  if (used >= 10 && !replaceReplayId) {
    throw createError({ statusCode: 409, statusMessage: 'Replay library is full.', data: { code: 'replayLibraryFull' } })
  }
  if (replaceReplayId) {
    const owned = await database.prepare(
      'SELECT replay_id FROM player_saved_replay WHERE user_id = ? AND replay_id = ?',
    ).bind(userId, replaceReplayId).all()
    if ((owned.results?.length ?? 0) === 0) {
      throw createError({ statusCode: 409, statusMessage: 'Replay replacement is unavailable.', data: { code: 'replayLibraryFull' } })
    }
  }

  const insert = database.prepare(
    `INSERT INTO player_saved_replay (user_id, replay_id, source_game_id, room_name, players_json, result_json, finished_at, saved_at)
     SELECT ?, ?, ?, ?, ?, ?, ?, ?
     WHERE (SELECT COUNT(*) FROM player_saved_replay WHERE user_id = ?) < 10`,
  ).bind(
    userId, draft.replayId, draft.sourceGameId, draft.roomName,
    JSON.stringify(draft.players), JSON.stringify(draft.result), draft.finishedAt, new Date().toISOString(), userId,
  )
  const results = replaceReplayId
    ? await database.batch([
        database.prepare('DELETE FROM player_saved_replay WHERE user_id = ? AND replay_id = ?').bind(userId, replaceReplayId),
        insert,
      ])
    : [await insert.run()]
  const inserted = results.at(-1)?.meta?.changes ?? 0
  if (inserted !== 1) throw createError({ statusCode: 409, statusMessage: 'Replay library is full.', data: { code: 'replayLibraryFull' } })
  return { saved: true, replayId: draft.replayId }
}

export async function deleteReplayReference(event: H3Event, userId: string, replayId: string) {
  await db(event).prepare('DELETE FROM player_saved_replay WHERE user_id = ? AND replay_id = ?').bind(userId, replayId).run()
  const remaining = await db(event).prepare(
    'SELECT COUNT(*) AS count FROM player_saved_replay WHERE replay_id = ?',
  ).bind(replayId).all<{ count: number }>()
  if (Number(remaining.results?.[0]?.count ?? 0) === 0) {
    await replayRequest(event, replayId, '', { method: 'DELETE' })
  }
}
