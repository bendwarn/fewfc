import { afterEach, beforeEach, describe, expect, test } from 'bun:test'
import type { D1Database } from '@cloudflare/workers-types'
import { Database } from 'bun:sqlite'
import { activeAnonymousUpgradeMessage, migrateAnonymousData, unlinkAccountAtomically } from './social-auth'

/**
 * 以 Bun SQLite 執行和 D1 相同的 SQLite SQL 介面；這讓遷移測試實際驗證
 * INSERT OR IGNORE、EXISTS DELETE 與生命週期計數，而非只檢查策略文字。
 */
function d1Harness() {
  const sqlite = new Database(':memory:')
  const prepared = (query: string) => {
    const statement = sqlite.prepare(query)
    let values: unknown[] = []
    return {
      bind(...next: unknown[]) {
        values = next
        return this
      },
      async first<T>() {
        return (statement.all(...(values as any))[0] as T | undefined) ?? null
      },
      async all<T>() {
        return { results: statement.all(...(values as any)) as T[] }
      },
      async run() {
        statement.run(...(values as any))
        const changeRow = sqlite.prepare('SELECT changes() AS changes').get() as { changes?: number }
        return { meta: { changes: Number(changeRow.changes ?? 0) } }
      },
    }
  }
  return {
    database: {
      prepare: prepared,
      async batch(statements: Array<ReturnType<typeof prepared>>) {
        return await Promise.all(statements.map(statement => statement.run()))
      },
    } as unknown as D1Database,
    close() {
      sqlite.close()
    },
  }
}

async function createSchema(database: D1Database) {
  for (const statement of [
    `CREATE TABLE user (id text PRIMARY KEY, name text NOT NULL, created_at integer NOT NULL, updated_at integer NOT NULL)`,
    `CREATE TABLE player_profile (user_id text PRIMARY KEY, display_name text NOT NULL, avatar_url text, created_at integer NOT NULL, updated_at integer NOT NULL)`,
    `CREATE TABLE player_deck (user_id text PRIMARY KEY, name text NOT NULL, cards_json text NOT NULL, created_at integer NOT NULL, updated_at integer NOT NULL)`,
    `CREATE TABLE player_saved_replay (user_id text NOT NULL, replay_id text NOT NULL, source_game_id text NOT NULL, room_name text NOT NULL, players_json text NOT NULL, result_json text NOT NULL, finished_at text NOT NULL, saved_at text NOT NULL, PRIMARY KEY (user_id, replay_id))`,
    `CREATE TABLE replay_archive_lifecycle (replay_id text PRIMARY KEY, reference_count integer NOT NULL, version integer NOT NULL)`,
    `CREATE TABLE public_game_room (game_id text PRIMARY KEY, status text NOT NULL)`,
    `CREATE TABLE game_room_member (game_id text NOT NULL, user_id text NOT NULL, PRIMARY KEY (game_id, user_id))`,
    `CREATE TABLE account (id text PRIMARY KEY, user_id text NOT NULL)`,
  ]) await database.prepare(statement).run()
}

async function user(database: D1Database, id: string, name: string, createdAt: number) {
  await database.prepare('INSERT INTO user (id, name, created_at, updated_at) VALUES (?, ?, ?, ?)')
    .bind(id, name, createdAt, createdAt).run()
}

async function profile(database: D1Database, id: string, displayName: string, avatarUrl: string | null = null) {
  await database.prepare('INSERT INTO player_profile (user_id, display_name, avatar_url, created_at, updated_at) VALUES (?, ?, ?, ?, ?)')
    .bind(id, displayName, avatarUrl, 1, 1).run()
}

async function replay(database: D1Database, userId: string, replayId: string) {
  await database.prepare(
    `INSERT INTO player_saved_replay
       (user_id, replay_id, source_game_id, room_name, players_json, result_json, finished_at, saved_at)
     VALUES (?, ?, 'game', 'room', '[]', 'null', '2026-01-01', '2026-01-01')`,
  ).bind(userId, replayId).run()
}

describe('social-auth D1 migration', () => {
  let harness: ReturnType<typeof d1Harness>
  let database: D1Database

  beforeEach(async () => {
    harness = d1Harness()
    database = harness.database
    await createSchema(database)
  })

  afterEach(() => {
    harness.close()
  })

  test('moves a guest name, Deck List, and Replay to a newly-created social user', async () => {
    await user(database, 'guest', '旅人原名', 100)
    await user(database, 'social-new', 'Provider Name', 200)
    await profile(database, 'guest', '旅人原名', 'guest.png')
    await profile(database, 'social-new', 'Provider Name', 'provider.png')
    await database.prepare(
      `INSERT INTO player_deck (user_id, name, cards_json, created_at, updated_at)
       VALUES ('guest', '訪客牌組', '["earth-1"]', 1, 1)`,
    ).run()
    await replay(database, 'guest', 'replay-new')
    await database.prepare('INSERT INTO replay_archive_lifecycle VALUES (?, ?, ?)').bind('replay-new', 1, 1).run()

    await expect(migrateAnonymousData(database, 'guest', 'social-new')).resolves.toEqual({ targetName: '旅人原名' })
    expect(await database.prepare('SELECT name FROM user WHERE id = ?').bind('social-new').first<{ name: string }>()).toEqual({ name: '旅人原名' })
    expect(await database.prepare('SELECT display_name, avatar_url FROM player_profile WHERE user_id = ?').bind('social-new').first<{ display_name: string, avatar_url: string }>())
      .toEqual({ display_name: '旅人原名', avatar_url: 'guest.png' })
    expect(await database.prepare('SELECT name FROM player_deck WHERE user_id = ?').bind('social-new').first<{ name: string }>()).toEqual({ name: '訪客牌組' })
    expect(await database.prepare('SELECT user_id FROM player_saved_replay WHERE replay_id = ?').bind('replay-new').first<{ user_id: string }>()).toEqual({ user_id: 'social-new' })
    expect(await database.prepare('SELECT 1 FROM player_saved_replay WHERE user_id = ?').bind('guest').first()).toBeNull()
  })

  test('preserves an existing user name and Deck List while decrementing a conflicting Replay lifecycle', async () => {
    await user(database, 'guest', '訪客原名', 200)
    await user(database, 'member', '正式名稱', 100)
    await profile(database, 'guest', '訪客原名', 'guest.png')
    await profile(database, 'member', '正式名稱', 'member.png')
    await database.prepare(`INSERT INTO player_deck VALUES ('guest', '訪客牌組', '["earth-1"]', 1, 1)`).run()
    await database.prepare(`INSERT INTO player_deck VALUES ('member', '正式牌組', '["fire-1"]', 1, 1)`).run()
    await replay(database, 'guest', 'replay-conflict')
    await replay(database, 'member', 'replay-conflict')
    await database.prepare('INSERT INTO replay_archive_lifecycle VALUES (?, ?, ?)').bind('replay-conflict', 2, 4).run()

    await expect(migrateAnonymousData(database, 'guest', 'member')).resolves.toEqual({})
    expect(await database.prepare('SELECT name FROM user WHERE id = ?').bind('member').first<{ name: string }>()).toEqual({ name: '正式名稱' })
    expect(await database.prepare('SELECT name FROM player_deck WHERE user_id = ?').bind('member').first<{ name: string }>()).toEqual({ name: '正式牌組' })
    expect((await database.prepare('SELECT user_id FROM player_saved_replay WHERE replay_id = ?').bind('replay-conflict').all()).results)
      .toEqual([{ user_id: 'member' }])
    expect(await database.prepare('SELECT reference_count, version FROM replay_archive_lifecycle WHERE replay_id = ?').bind('replay-conflict').first<{ reference_count: number, version: number }>())
      .toEqual({ reference_count: 1, version: 5 })
  })

  test('rejects a guest migration while that guest is in a waiting room', async () => {
    await user(database, 'guest', '旅人', 100)
    await user(database, 'member', '正式', 200)
    await profile(database, 'guest', '旅人')
    await profile(database, 'member', '正式')
    await database.prepare(`INSERT INTO public_game_room VALUES ('room', 'Waiting')`).run()
    await database.prepare(`INSERT INTO game_room_member VALUES ('room', 'guest')`).run()

    await expect(migrateAnonymousData(database, 'guest', 'member')).rejects.toThrow(activeAnonymousUpgradeMessage)
    expect(await database.prepare('SELECT name FROM user WHERE id = ?').bind('member').first<{ name: string }>()).toEqual({ name: '正式' })
  })

  test('atomically retains one Authentication Method under concurrent unlinks', async () => {
    await database.prepare(`INSERT INTO account VALUES ('email', 'member')`).run()
    await database.prepare(`INSERT INTO account VALUES ('google', 'member')`).run()
    const outcomes = await Promise.all([
      unlinkAccountAtomically(database, 'member', 'email'),
      unlinkAccountAtomically(database, 'member', 'google'),
    ])
    expect(outcomes.sort()).toEqual(['last-authentication-method', 'unlinked'])
    expect(await database.prepare('SELECT COUNT(*) AS count FROM account WHERE user_id = ?').bind('member').first<{ count: number }>())
      .toEqual({ count: 1 })
  })
})
