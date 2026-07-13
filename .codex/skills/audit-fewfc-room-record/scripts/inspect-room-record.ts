#!/usr/bin/env bun

type RecordValue = Record<string, unknown>

interface Options {
  roomId: string
  repo: string
  database: string | null
  commandId: number | null
  matches: string[]
  untilCommand: number | null
  includeSetup: boolean
  includeMetadata: boolean
}

interface Snapshot extends RecordValue {
  rulesRecord?: unknown
  schemaVersion?: unknown
  sequence?: unknown
  setup?: unknown
}

class CliError extends Error {}

function fail(message: string): never {
  throw new CliError(message)
}

function usage(): void {
  console.log('Usage: inspect-room-record.ts ROOM_UUID [options]\n\nOptions:\n  --repo PATH          FEWFC repository root (default: current directory)\n  --database PATH      Durable Object SQLite file to inspect directly\n  --command-id N       Print the canonical entry for command N\n  --match TEXT         Print entries containing TEXT (repeatable, OR semantics)\n  --until-command N    Exclude entries after command N\n  --setup              Include canonical setup\n  --metadata           Include room metadata\n  --help               Show this help')
}

function parseArgs(argv: string[]): Options {
  const roomId = argv.shift()
  if (!roomId) fail('ROOM_UUID is required')
  const options: Options = {
    roomId,
    repo: process.cwd(),
    database: null,
    commandId: null,
    matches: [],
    untilCommand: null,
    includeSetup: false,
    includeMetadata: false,
  }
  while (argv.length) {
    const flag = argv.shift()
    if (flag === '--repo') options.repo = argv.shift() ?? fail('--repo requires a path')
    else if (flag === '--database') options.database = argv.shift() ?? fail('--database requires a path')
    else if (flag === '--command-id') options.commandId = integer(argv.shift(), '--command-id')
    else if (flag === '--match') options.matches.push(argv.shift() ?? fail('--match requires text'))
    else if (flag === '--until-command') options.untilCommand = integer(argv.shift(), '--until-command')
    else if (flag === '--setup') options.includeSetup = true
    else if (flag === '--metadata') options.includeMetadata = true
    else fail(`unknown option: ${flag}`)
  }
  return options
}

function integer(value: string | undefined, flag: string): number {
  if (!/^\d+$/.test(value ?? '')) fail(`${flag} requires a non-negative integer`)
  return Number(value)
}

function text(bytes: Uint8Array): string {
  return new TextDecoder().decode(bytes).trim()
}

function sqlite(database: string, sql: string): string {
  try {
    const result = Bun.spawnSync(['sqlite3', '-json', database, sql], {
      stdout: 'pipe',
      stderr: 'pipe',
    })
    if (result.exitCode === 0) return text(result.stdout)
    fail(`sqlite3 failed for ${database}: ${text(result.stderr) || `exit ${result.exitCode}`}`)
  } catch (error) {
    fail(`sqlite3 failed for ${database}: ${error instanceof Error ? error.message : String(error)}`)
  }
}

function absolutePath(path: string): string {
  return Bun.fileURLToPath(new URL(path, Bun.pathToFileURL('.')))
}

function isRecord(value: unknown): value is RecordValue {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
}

function roomName(database: string): string | null {
  try {
    const raw = sqlite(database, 'select name from __miniflare_do_name limit 1')
    if (!raw) return null
    const first = JSON.parse(raw)[0]
    return isRecord(first) && typeof first.name === 'string' ? first.name : null
  } catch {
    return null
  }
}

function sqliteFiles(root: string): string[] {
  return Array.from(new Bun.Glob('**/*.sqlite').scanSync({
    cwd: root,
    absolute: true,
    onlyFiles: true,
  })).filter(path =>
    path.split('/').at(-1) !== 'metadata.sqlite'
    && path.includes('/do/')
    && path.includes('GameRoom'))
}

function locateDatabase(options: Options): string {
  if (options.database) return absolutePath(options.database)
  const wrangler = Bun.fileURLToPath(new URL('apps/web/.wrangler', Bun.pathToFileURL(`${options.repo}/`)))
  const matches = sqliteFiles(wrangler).filter(database => roomName(database) === options.roomId)
  if (matches.length === 0) fail(`room ${options.roomId} was not found under ${wrangler}`)
  if (matches.length > 1) {
    const candidates = matches
      .map(path => `${path} (${new Date(Bun.file(path).lastModified).toISOString()})`)
      .join('\n  ')
    fail(`room exists in multiple Wrangler stores; rerun with --database:\n  ${candidates}`)
  }
  return matches[0]
}

function readValue(database: string, key: 'metadata' | 'snapshot'): unknown {
  const raw = sqlite(database, `select hex(value) as hex from _cf_KV where key='${key}'`)
  const first = raw ? JSON.parse(raw)[0] : null
  const hex = isRecord(first) && typeof first.hex === 'string' ? first.hex : null
  if (!hex) fail(`missing ${key} in ${database}`)
  try {
    const decoder = Bun.which('node')
    if (!decoder) fail('Node.js is required to decode Miniflare V8 values')
    const bridge = Bun.fileURLToPath(new URL('./decode-v8.mjs', import.meta.url))
    const result = Bun.spawnSync([decoder, bridge, Uint8Array.fromHex(hex).toBase64()], {
      stdout: 'pipe',
      stderr: 'pipe',
    })
    if (result.exitCode !== 0) {
      fail(`cannot decode ${key}: ${text(result.stderr) || `decoder exited ${result.exitCode}`}`)
    }
    return JSON.parse(text(result.stdout))
  } catch (error) {
    if (error instanceof CliError) throw error
    fail(`cannot decode ${key}: ${error instanceof Error ? error.message : String(error)}`)
  }
}

function commandSource(entry: unknown): RecordValue | null {
  if (!isRecord(entry) || !isRecord(entry.source) || !isRecord(entry.source.Command)) return null
  return entry.source.Command
}

function commandSummary(entry: unknown, recordIndex: number): RecordValue | null {
  const source = commandSource(entry)
  if (!source) return null
  const command = isRecord(source.command) ? source.command : {}
  const type = Object.keys(command)[0] ?? 'Unknown'
  const payload = isRecord(command[type]) ? command[type] : {}
  const events = isRecord(entry) && Array.isArray(entry.events) ? entry.events : []
  return {
    recordIndex,
    commandId: source.command_id ?? null,
    type,
    player: payload.player ?? null,
    formationId: payload.formation_id ?? null,
    abilityId: payload.ability_id ?? null,
    eventTypes: events.map(event =>
      typeof event === 'string' ? event : isRecord(event) ? Object.keys(event)[0] ?? 'Unknown' : 'Unknown'),
  }
}

async function main(): Promise<void> {
  const argv = Bun.argv.slice(2)
  if (argv.includes('--help')) {
    usage()
    return
  }
  const options = parseArgs(argv)
  const database = locateDatabase(options)
  const snapshotValue = readValue(database, 'snapshot')
  if (!isRecord(snapshotValue)) fail('snapshot is not an object')
  const snapshot: Snapshot = snapshotValue
  const record = snapshot.rulesRecord
  if (!Array.isArray(record)) fail('snapshot.rulesRecord is not an array')

  let limit = record.length - 1
  if (options.untilCommand !== null) {
    limit = record.findIndex(entry => commandSource(entry)?.command_id === options.untilCommand)
    if (limit < 0) fail(`command ${options.untilCommand} was not found`)
  }

  const indexed = record.map((entry, recordIndex) => ({ recordIndex, entry }))
    .filter(item => item.recordIndex <= limit)
  let selected: Array<{ recordIndex: number; entry: unknown }> | RecordValue[]
  if (options.commandId !== null) {
    selected = indexed.filter(item => commandSource(item.entry)?.command_id === options.commandId)
    if (selected.length === 0) fail(`command ${options.commandId} was not found in the selected range`)
  } else if (options.matches.length) {
    selected = indexed.filter(item => {
      const serialized = JSON.stringify(item.entry) ?? ''
      return options.matches.some(term => serialized.includes(term))
    })
  } else {
    selected = indexed
      .map(item => commandSummary(item.entry, item.recordIndex))
      .filter((item): item is RecordValue => item !== null)
  }

  const output: RecordValue = {
    roomId: options.roomId,
    database,
    snapshot: {
      schemaVersion: snapshot.schemaVersion,
      sequence: snapshot.sequence,
      recordEntries: record.length,
    },
  }
  if (options.includeMetadata) output.metadata = readValue(database, 'metadata')
  if (options.includeSetup) output.setup = snapshot.setup
  if (options.commandId !== null || options.matches.length) {
    output.matches = (selected as Array<{ recordIndex: number; entry: unknown }>)
      .map(({ recordIndex, entry }) => ({ recordIndex, ...(isRecord(entry) ? entry : { entry }) }))
  } else {
    output.commands = selected
  }

  await Bun.write(Bun.stdout, `${JSON.stringify(output, null, 2)}\n`)
}

try {
  await main()
} catch (error) {
  const message = error instanceof Error ? error.message : String(error)
  await Bun.write(Bun.stderr, `error: ${message}\n`)
  process.exitCode = 1
}
