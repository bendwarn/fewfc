import { createError, type H3Event } from 'h3'
import type { CompletedReplayDraft } from '../../shared/game-room'
import type { ReplayArchiveLifecycle, ReplayFrame } from '../../worker/durable-objects/replay-archive'
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

export async function createReplayArchive(
  event: H3Event,
  draft: CompletedReplayDraft,
  lifecycle: ReplayArchiveLifecycle,
) {
  const response = await replayRequest(event, draft.replayId, 'create', {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({ archive: draft, lifecycle }),
  })
  if (!response.ok) throw createError({ statusCode: response.status, statusMessage: 'Unable to create replay archive.' })
}

async function lifecycleFor(database: D1Database, replayId: string): Promise<ReplayArchiveLifecycle> {
  const result = await database.prepare(
    'SELECT reference_count, version FROM replay_archive_lifecycle WHERE replay_id = ?',
  ).bind(replayId).all<{ reference_count: number; version: number }>()
  const row = result.results?.[0]
  if (!row) throw new Error('replay archive lifecycle is missing')
  return { referenceCount: Number(row.reference_count), version: Number(row.version) }
}

async function retainReplayReference(database: D1Database, replayId: string) {
  await database.prepare(
    `INSERT INTO replay_archive_lifecycle (replay_id, reference_count, version)
     VALUES (?, 1, 1)
     ON CONFLICT(replay_id) DO UPDATE SET
       reference_count = reference_count + 1,
       version = version + 1`,
  ).bind(replayId).run()
  return await lifecycleFor(database, replayId)
}

async function releaseReplayReference(database: D1Database, replayId: string) {
  await database.prepare(
    `UPDATE replay_archive_lifecycle
     SET reference_count = CASE WHEN reference_count > 0 THEN reference_count - 1 ELSE 0 END,
         version = version + 1
     WHERE replay_id = ?`,
  ).bind(replayId).run()
  return await lifecycleFor(database, replayId)
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
  if ((duplicate.results?.length ?? 0) > 0) {
    const lifecycle = await lifecycleFor(database, draft.replayId)
    await createReplayArchive(event, draft, lifecycle)
    return { saved: true, replayId: draft.replayId }
  }

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
  // D1 是玩家參照與單調生命週期的真實來源。將這些陳述式放在同一批次中，
  // 可避免替代操作發布新參照卻留下舊封存數量（或反過來）。
  const retain = database.prepare(
    `INSERT INTO replay_archive_lifecycle (replay_id, reference_count, version)
     SELECT ?, 1, 1 WHERE changes() = 1
     ON CONFLICT(replay_id) DO UPDATE SET
       reference_count = reference_count + 1,
       version = version + 1`,
  ).bind(draft.replayId)
  const release = replaceReplayId
    ? database.prepare(
        `UPDATE replay_archive_lifecycle
         SET reference_count = CASE WHEN reference_count > 0 THEN reference_count - 1 ELSE 0 END,
             version = version + 1
         WHERE replay_id = ? AND changes() = 1`,
      ).bind(replaceReplayId)
    : undefined
  const results = replaceReplayId
    ? await database.batch([
        database.prepare('DELETE FROM player_saved_replay WHERE user_id = ? AND replay_id = ?').bind(userId, replaceReplayId),
        release!,
        insert,
        retain,
      ])
    : await database.batch([insert, retain])
  const inserted = results.at(replaceReplayId ? 2 : 0)?.meta?.changes ?? 0
  if (inserted !== 1) throw createError({ statusCode: 409, statusMessage: 'Replay library is full.', data: { code: 'replayLibraryFull' } })
  const lifecycle = await lifecycleFor(database, draft.replayId)
  await createReplayArchive(event, draft, lifecycle)
  if (replaceReplayId && replaceReplayId !== draft.replayId) {
    const replacedLifecycle = await lifecycleFor(database, replaceReplayId)
    if (replacedLifecycle.referenceCount === 0) {
      await replayRequest(event, replaceReplayId, '', {
        method: 'DELETE',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify(replacedLifecycle),
      })
    }
  }
  return { saved: true, replayId: draft.replayId }
}

export async function deleteReplayReference(event: H3Event, userId: string, replayId: string) {
  const database = db(event)
  const results = await database.batch([
    database.prepare('DELETE FROM player_saved_replay WHERE user_id = ? AND replay_id = ?').bind(userId, replayId),
    database.prepare(
      `UPDATE replay_archive_lifecycle
       SET reference_count = CASE WHEN reference_count > 0 THEN reference_count - 1 ELSE 0 END,
           version = version + 1
       WHERE replay_id = ? AND changes() = 1`,
    ).bind(replayId),
  ])
  if ((results[0]?.meta?.changes ?? 0) === 0) return

  const lifecycle = await lifecycleFor(database, replayId)
  if (lifecycle.referenceCount === 0) {
    await replayRequest(event, replayId, '', {
      method: 'DELETE',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify(lifecycle),
    })
  }
}
