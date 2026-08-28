export const ROOM_ID_PATTERN =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i

interface CommandResult {
  stdout: Uint8Array
  stderr: Uint8Array
  exitCode: number
}

interface DeleteRoomOptions {
  wranglerRoot?: string
  runningProcesses?: string[]
}

export interface RoomDeletionResult {
  roomId: string
  durableObjectDatabases: string[]
  d1Databases: string[]
}

const decoder = new TextDecoder()

function output(result: CommandResult): string {
  return decoder.decode(result.stdout).trim()
}

function errorOutput(result: CommandResult): string {
  return decoder.decode(result.stderr).trim()
}

function runSqlite(database: string, sql: string): Record<string, unknown>[] {
  const result = Bun.spawnSync(['sqlite3', '-json', database, sql], {
    stdout: 'pipe',
    stderr: 'pipe',
  }) as CommandResult

  if (result.exitCode !== 0) {
    throw new Error(`sqlite3 failed for ${database}: ${errorOutput(result) || `exit ${result.exitCode}`}`)
  }

  const raw = output(result)
  if (!raw) return []

  const parsed: unknown = JSON.parse(raw)
  if (!Array.isArray(parsed)) {
    throw new Error(`sqlite3 returned a non-array result for ${database}`)
  }
  return parsed.filter((row): row is Record<string, unknown> =>
    typeof row === 'object' && row !== null && !Array.isArray(row),
  )
}

function sqlString(value: string): string {
  return `'${value.replaceAll("'", "''")}'`
}

export function parseRoomId(value: string | undefined): string {
  const roomId = value?.trim() ?? ''
  if (!ROOM_ID_PATTERN.test(roomId)) {
    throw new Error(`invalid room UUID: ${value ?? '(missing)'}`)
  }
  return roomId.toLowerCase()
}

export function sqliteFiles(root: string): string[] {
  return Array.from(new Bun.Glob('**/*.sqlite').scanSync({
    cwd: root,
    absolute: true,
    onlyFiles: true,
  })).filter(path => !path.endsWith('/metadata.sqlite'))
}

function durableObjectFiles(root: string): string[] {
  return sqliteFiles(root).filter(path => {
    const segments = path.split('/')
    const parent = segments.at(-2) ?? ''
    return path.includes('/do/') && parent.endsWith('-GameRoom')
  })
}

function d1Files(root: string): string[] {
  return sqliteFiles(root).filter(path => path.includes('/d1/'))
}

function roomExistsInDurableObject(database: string, roomId: string): boolean {
  const rows = runSqlite(
    database,
    `SELECT name FROM __miniflare_do_name WHERE name = ${sqlString(roomId)} LIMIT 1`,
  )
  return rows.length > 0
}

function roomExistsInD1(database: string, roomId: string): boolean {
  const table = runSqlite(
    database,
    "SELECT name FROM sqlite_master WHERE type = 'table' AND name = 'public_game_room' LIMIT 1",
  )
  if (table.length === 0) return false

  const rows = runSqlite(
    database,
    `SELECT game_id FROM public_game_room WHERE game_id = ${sqlString(roomId)} LIMIT 1`,
  )
  return rows.length > 0
}

export function runningWorkerProcesses(processTable: string): string[] {
  return processTable
    .split('\n')
    .map(line => line.trim())
    .filter(line =>
      line.includes('apps/web')
      && (/\bwrangler(?:\.js)?\s+dev\b/.test(line) || /workerd\s+serve\b/.test(line)),
    )
}

function currentWorkerProcesses(): string[] {
  const result = Bun.spawnSync(['ps', '-axo', 'pid=,command='], {
    stdout: 'pipe',
    stderr: 'pipe',
  }) as CommandResult
  if (result.exitCode !== 0) {
    throw new Error(`could not inspect running processes: ${errorOutput(result) || `exit ${result.exitCode}`}`)
  }
  return runningWorkerProcesses(decoder.decode(result.stdout))
}

async function unlinkIfPresent(path: string): Promise<void> {
  const file = Bun.file(path)
  if (await file.exists()) await file.delete()
}

async function removeDurableObjectDatabase(database: string): Promise<void> {
  await unlinkIfPresent(database)
  await unlinkIfPresent(`${database}-shm`)
  await unlinkIfPresent(`${database}-wal`)
}

function deleteD1Room(database: string, roomId: string): void {
  runSqlite(
    database,
    `BEGIN;
DELETE FROM game_room_member WHERE game_id = ${sqlString(roomId)};
DELETE FROM public_game_room WHERE game_id = ${sqlString(roomId)};
COMMIT;`,
  )
}

export async function deleteRoom(
  roomId: string,
  options: DeleteRoomOptions = {},
): Promise<RoomDeletionResult> {
  const normalizedRoomId = parseRoomId(roomId)
  const wranglerRoot = options.wranglerRoot ?? `${import.meta.dir}/../.wrangler`
  const processes = options.runningProcesses ?? currentWorkerProcesses()

  if (processes.length > 0) {
    throw new Error(
      `Wrangler is running for this workspace. Stop \n ${processes.join("\n")}`,
    );
  }

  const durableObjectDatabases = durableObjectFiles(wranglerRoot)
    .filter(database => roomExistsInDurableObject(database, normalizedRoomId))
  const d1Databases = d1Files(wranglerRoot)
    .filter(database => roomExistsInD1(database, normalizedRoomId))

  if (durableObjectDatabases.length === 0 && d1Databases.length === 0) {
    throw new Error(`room ${normalizedRoomId} was not found under ${wranglerRoot}`)
  }

  for (const database of d1Databases) {
    deleteD1Room(database, normalizedRoomId)
  }
  for (const database of durableObjectDatabases) {
    await removeDurableObjectDatabase(database)
  }

  return {
    roomId: normalizedRoomId,
    durableObjectDatabases,
    d1Databases,
  }
}

function usage(): void {
  console.log(`Usage: bun apps/web/scripts/delete-room.ts ROOM_UUID

Deletes matching local Wrangler GameRoom Durable Object storage and D1 room indexes.
Stop the local Wrangler server before running this command.
`)
}

async function main(): Promise<void> {
  const [argument, ...rest] = Bun.argv.slice(2)
  if (!argument || argument === '--help' || argument === '-h') {
    usage()
    if (!argument) process.exitCode = 1
    return
  }
  if (rest.length > 0) {
    throw new Error(`unexpected arguments: ${rest.join(' ')}`)
  }

  const result = await deleteRoom(argument)
  console.log(`Deleted room ${result.roomId}`)
  console.log(`Durable Object databases removed: ${result.durableObjectDatabases.length}`)
  console.log(`D1 indexes updated: ${result.d1Databases.length}`)
}

if (import.meta.main) {
  await main().catch(error => {
    console.error(`Error: ${error instanceof Error ? error.message : String(error)}`)
    process.exitCode = 1
  })
}
