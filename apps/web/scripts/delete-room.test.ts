import { afterEach, describe, expect, test } from 'bun:test'
import { $ } from 'bun'
import { deleteRoom, parseRoomId, runningWorkerProcesses } from './delete-room'

const roomId = '123e4567-e89b-12d3-a456-426614174000'
const tempDirectories: string[] = []

async function sqlite(database: string, sql: string): Promise<void> {
  const result = Bun.spawnSync(['sqlite3', database, sql], {
    stdout: 'pipe',
    stderr: 'pipe',
  })
  if (result.exitCode !== 0) {
    throw new Error(new TextDecoder().decode(result.stderr))
  }
}

afterEach(async () => {
  while (tempDirectories.length > 0) {
    const directory = tempDirectories.pop()
    if (directory) await $`rm -rf ${directory}`
  }
})

describe('delete-room command', () => {
  test('validates UUID arguments', () => {
    expect(parseRoomId(roomId)).toBe(roomId)
    expect(parseRoomId(roomId.toUpperCase())).toBe(roomId)
    expect(() => parseRoomId('not-a-uuid')).toThrow('invalid room UUID')
  })

  test('identifies only workspace Wrangler processes', () => {
    expect(runningWorkerProcesses([
      '123 bun apps/web/scripts/delete-room.ts',
      '124 node apps/web/node_modules/.bin/wrangler dev',
      '125 /Users/bendwarn/Documents/fewfc/apps/web/node_modules/@cloudflare/workerd-darwin-arm64/bin/workerd serve --socket-addr=entry=localhost:8787',
      '126 node another-project/node_modules/.bin/wrangler dev',
      '127 workerd serve --socket-addr=entry=localhost:8787',
    ].join('\n'))).toEqual([
      '124 node apps/web/node_modules/.bin/wrangler dev',
      '125 /Users/bendwarn/Documents/fewfc/apps/web/node_modules/@cloudflare/workerd-darwin-arm64/bin/workerd serve --socket-addr=entry=localhost:8787',
    ])
  })

  test('removes the room from DO files and every local D1 index', async () => {
    const root = `/tmp/fewfc-delete-room-${crypto.randomUUID()}`
    tempDirectories.push(root)
    const doDirectory = `${root}/state/v3/do/fewfc-web-GameRoom`
    const d1Directory = `${root}/state/v3/d1/miniflare-D1DatabaseObject`
    await $`mkdir -p ${doDirectory} ${d1Directory}`

    const doDatabase = `${doDirectory}/room.sqlite`
    const d1Database = `${d1Directory}/database.sqlite`
    await sqlite(doDatabase, `CREATE TABLE __miniflare_do_name (name TEXT); INSERT INTO __miniflare_do_name VALUES ('${roomId}');`)
    await sqlite(d1Database, `
      CREATE TABLE public_game_room (game_id TEXT PRIMARY KEY, name TEXT);
      CREATE TABLE game_room_member (game_id TEXT, user_id TEXT);
      INSERT INTO public_game_room VALUES ('${roomId}', 'test room');
      INSERT INTO game_room_member VALUES ('${roomId}', 'test user');
    `)

    const result = await deleteRoom(roomId.toUpperCase(), { wranglerRoot: root, runningProcesses: [] })

    expect(result.durableObjectDatabases).toEqual([doDatabase])
    expect(result.d1Databases).toEqual([d1Database])
    expect(await Bun.file(doDatabase).exists()).toBe(false)
    expect(Bun.spawnSync(['sqlite3', d1Database, `SELECT COUNT(*) FROM public_game_room WHERE game_id = '${roomId}';`]).stdout.toString().trim()).toBe('0')
    expect(Bun.spawnSync(['sqlite3', d1Database, `SELECT COUNT(*) FROM game_room_member WHERE game_id = '${roomId}';`]).stdout.toString().trim()).toBe('0')
  })
})
