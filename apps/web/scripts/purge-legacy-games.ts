/**
 * One-off, deliberately remote-only management tool for issue #75's hard
 * cutover.  It never enables maintenance or deploys a Worker: an operator must
 * first deploy the maintenance configuration explicitly, then run this tool.
 */

export type PurgeEnvironment = 'staging' | 'production'

export interface PurgeArguments {
  environment: PurgeEnvironment
  epoch: string
  confirm: boolean
}

export interface PurgeConfig {
  environment: PurgeEnvironment
  epoch: string
  confirm: boolean
  accountId: string
  apiToken: string
  workerUrl: string
  d1DatabaseId: string
  gameRoomNamespaceId: string
  replayNamespaceId: string
  managementSecret?: string
}

export interface DurableObjectInventoryItem {
  id: string
  hasStoredData: boolean
}

export interface PurgeInventory {
  roomIds: string[]
  replayIds: string[]
  gameRoomObjects: DurableObjectInventoryItem[]
  replayObjects: DurableObjectInventoryItem[]
}

export interface PurgeRunSummary {
  mode: 'dry-run' | 'mutated'
  mutationCount: number
  rooms: number
  gameRoomObjects: number
  replayObjects: number
}

export interface FetchResponse {
  ok: boolean
  status: number
  json(): Promise<unknown>
  text(): Promise<string>
}

export type Fetcher = (input: string, init?: RequestInit) => Promise<FetchResponse>

const apiBase = 'https://api.cloudflare.com/client/v4'

export function parsePurgeArguments(argv: string[]): PurgeArguments {
  let environment: PurgeEnvironment | undefined
  let epoch: string | undefined
  let confirm = false

  for (let index = 0; index < argv.length; index += 1) {
    const argument = argv[index]
    if (argument === '--confirm') {
      if (confirm) throw new Error('--confirm may be provided only once')
      confirm = true
      continue
    }
    if (argument === '--env' || argument === '--epoch') {
      const value = argv[index + 1]
      if (!value || value.startsWith('--')) throw new Error(`${argument} requires a value`)
      index += 1
      if (argument === '--env') {
        if (environment) throw new Error('--env must be provided exactly once')
        if (value !== 'staging' && value !== 'production') {
          throw new Error('--env must be staging or production')
        }
        environment = value
      } else {
        if (epoch) throw new Error('--epoch may be provided only once')
        if (!/^[A-Za-z0-9][A-Za-z0-9._-]{2,127}$/.test(value)) {
          throw new Error('--epoch must be a stable identifier')
        }
        epoch = value
      }
      continue
    }
    throw new Error(`unexpected argument: ${argument}`)
  }

  if (!environment) throw new Error('--env must be provided exactly once')
  if (!epoch) throw new Error('--epoch is required')
  return { environment, epoch, confirm }
}

export function configFromEnvironment(
  args: PurgeArguments,
  source: Record<string, string | undefined> = Bun.env,
): PurgeConfig {
  const prefix = args.environment === 'staging' ? 'FEWFC_STAGING' : 'FEWFC_PRODUCTION'
  const required = (key: string): string => {
    const value = source[key]?.trim()
    if (!value) throw new Error(`missing required environment variable ${key}`)
    return value
  }

  return {
    ...args,
    accountId: required('CLOUDFLARE_ACCOUNT_ID'),
    apiToken: required('CLOUDFLARE_API_TOKEN'),
    workerUrl: required(`${prefix}_WORKER_URL`).replace(/\/$/, ''),
    d1DatabaseId: required(`${prefix}_D1_DATABASE_ID`),
    gameRoomNamespaceId: required(`${prefix}_GAME_ROOM_NAMESPACE_ID`),
    replayNamespaceId: required(`${prefix}_REPLAY_NAMESPACE_ID`),
    managementSecret: args.confirm ? required('LEGACY_PURGE_SECRET') : undefined,
  }
}

export async function listDurableObjects(
  config: Pick<PurgeConfig, 'accountId' | 'apiToken'>,
  namespaceId: string,
  fetcher: Fetcher = fetch,
): Promise<DurableObjectInventoryItem[]> {
  const objects: DurableObjectInventoryItem[] = []
  let cursor: string | undefined
  do {
    const search = new URLSearchParams({ limit: '10000' })
    if (cursor) search.set('cursor', cursor)
    const response = await cloudflareFetch(
      fetcher,
      `${apiBase}/accounts/${encodeURIComponent(config.accountId)}/workers/durable_objects/namespaces/${encodeURIComponent(namespaceId)}/objects?${search}`,
      config.apiToken,
    ) as {
      result?: Array<{ id?: unknown, hasStoredData?: unknown }>
      result_info?: { cursor?: unknown }
    }
    for (const item of response.result ?? []) {
      if (typeof item.id === 'string' && item.id) {
        objects.push({ id: item.id, hasStoredData: item.hasStoredData === true })
      }
    }
    cursor = typeof response.result_info?.cursor === 'string' && response.result_info.cursor
      ? response.result_info.cursor
      : undefined
  } while (cursor)
  return objects
}

export async function purgeInventory(
  config: Pick<PurgeConfig, 'accountId' | 'apiToken' | 'd1DatabaseId' | 'gameRoomNamespaceId' | 'replayNamespaceId'>,
  fetcher: Fetcher = fetch,
): Promise<PurgeInventory> {
  const [roomRows, replayRows, gameRoomObjects, replayObjects] = await Promise.all([
    d1Query<{ game_id?: unknown }>(config, 'SELECT game_id FROM public_game_room ORDER BY game_id', fetcher),
    d1Query<{ replay_id?: unknown }>(config, 'SELECT replay_id FROM player_saved_replay ORDER BY replay_id', fetcher),
    listDurableObjects(config, config.gameRoomNamespaceId, fetcher),
    listDurableObjects(config, config.replayNamespaceId, fetcher),
  ])
  return {
    roomIds: roomRows
      .map(row => row.game_id)
      .filter((value): value is string => typeof value === 'string' && value.length > 0),
    replayIds: replayRows
      .map(row => row.replay_id)
      .filter((value): value is string => typeof value === 'string' && value.length > 0),
    gameRoomObjects: gameRoomObjects.filter(object => object.hasStoredData),
    replayObjects: replayObjects.filter(object => object.hasStoredData),
  }
}

export async function runLegacyPurge(
  config: PurgeConfig,
  fetcher: Fetcher = fetch,
): Promise<{ inventory: PurgeInventory, mutated: boolean, mutationCount: number }> {
  const inventory = await purgeInventory(config, fetcher)
  if (!config.confirm) return { inventory, mutated: false, mutationCount: 0 }

  let mutationCount = 0
  for (const object of inventory.gameRoomObjects) {
    const result = await managementPost(config, 'room', { objectId: object.id }, fetcher) as {
      status?: unknown
    }
    if (result.status === 'purged') mutationCount += 1
  }
  const roomIndex = await managementPost(config, 'room-index', {}, fetcher) as {
    roomIndexUpdated?: unknown
  }
  mutationCount += Number(roomIndex.roomIndexUpdated ?? 0)
  for (const object of inventory.replayObjects) {
    await managementPost(config, 'replay', { objectId: object.id }, fetcher)
    mutationCount += 1
  }
  const replayIndex = await managementPost(config, 'replay-index', {}, fetcher) as {
    playerSavedReplayDeleted?: unknown
    replayArchiveLifecycleDeleted?: unknown
  }
  mutationCount += Number(replayIndex.playerSavedReplayDeleted ?? 0)
  mutationCount += Number(replayIndex.replayArchiveLifecycleDeleted ?? 0)
  await verifyPurge(config, inventory, fetcher)
  return { inventory, mutated: true, mutationCount }
}

/** The only CLI output shape: deliberately inventory-only, never credentials. */
export function purgeRunSummary(
  result: Awaited<ReturnType<typeof runLegacyPurge>>,
): PurgeRunSummary {
  return {
    mode: result.mutated ? 'mutated' : 'dry-run',
    mutationCount: result.mutationCount,
    rooms: result.inventory.roomIds.length,
    gameRoomObjects: result.inventory.gameRoomObjects.length,
    replayObjects: result.inventory.replayObjects.length,
  }
}

export function redactSecrets(message: string, secrets: Array<string | undefined>): string {
  return secrets.reduce(
    (redacted, secret) => secret ? redacted.replaceAll(secret, '[redacted]') : redacted,
    message,
  )
}

export async function verifyPurge(
  config: PurgeConfig,
  inventory: PurgeInventory,
  fetcher: Fetcher = fetch,
): Promise<void> {
  const [roomRows, replayCounts, nonWaiting] = await Promise.all([
    d1Query<{ game_id?: unknown }>(config, 'SELECT game_id FROM public_game_room ORDER BY game_id', fetcher),
    d1Query<{ saved_count?: unknown, lifecycle_count?: unknown }>(
      config,
      'SELECT (SELECT COUNT(*) FROM player_saved_replay) AS saved_count, (SELECT COUNT(*) FROM replay_archive_lifecycle) AS lifecycle_count',
      fetcher,
    ),
    d1Query<{ count?: unknown }>(
      config,
      "SELECT COUNT(*) AS count FROM public_game_room WHERE status IN ('Active', 'Finished')",
      fetcher,
    ),
  ])
  const roomIds = roomRows
    .map(row => row.game_id)
    .filter((value): value is string => typeof value === 'string')
  if (JSON.stringify(roomIds) !== JSON.stringify(inventory.roomIds)) {
    throw new Error('verification failed: preserved room identities do not match the dry-run inventory')
  }
  const counts = replayCounts[0] ?? {}
  if (Number(counts.saved_count ?? -1) !== 0 || Number(counts.lifecycle_count ?? -1) !== 0) {
    throw new Error('verification failed: legacy replay tables are not empty')
  }
  if (Number(nonWaiting[0]?.count ?? -1) !== 0) {
    throw new Error('verification failed: an Active or Finished room remains in the public index')
  }

  for (const object of inventory.gameRoomObjects) {
    const verification = await managementPost(config, 'room-verify', { objectId: object.id }, fetcher) as {
      status?: unknown
      gameInstanceId?: unknown
      hasGameRecord?: unknown
    }
    if (
      verification.hasGameRecord === true
      || verification.status === 'Active'
      || verification.status === 'Finished'
      || typeof verification.gameInstanceId === 'string'
    ) {
      throw new Error(`verification failed: legacy Game Record remains in ${object.id}`)
    }
  }
  for (const object of inventory.replayObjects) {
    const verification = await managementPost(config, 'replay-verify', { objectId: object.id }, fetcher) as {
      keyCount?: unknown
    }
    if (Number(verification.keyCount ?? -1) !== 0) {
      throw new Error(`verification failed: legacy ReplayArchive remains in ${object.id}`)
    }
  }
  for (const replayId of inventory.replayIds.slice(0, 5)) {
    const sample = await managementPost(config, 'replay-sample', { replayId }, fetcher) as {
      status?: unknown
    }
    if (sample.status !== 404) {
      throw new Error(`verification failed: sampled legacy Replay ${replayId} did not return 404`)
    }
  }
}

async function managementPost(
  config: PurgeConfig,
  path: string,
  body: Record<string, unknown>,
  fetcher: Fetcher,
): Promise<unknown> {
  if (!config.managementSecret) throw new Error('LEGACY_PURGE_SECRET is required for --confirm')
  const response = await fetcher(`${config.workerUrl}/internal/legacy-purge/${path}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-fewfc-legacy-purge-secret': config.managementSecret,
    },
    body: JSON.stringify({ epoch: config.epoch, ...body }),
  })
  if (!response.ok) throw new Error(`management request ${path} failed with HTTP ${response.status}`)
  return await response.json()
}

async function d1Query<T>(
  config: Pick<PurgeConfig, 'accountId' | 'apiToken' | 'd1DatabaseId'>,
  sql: string,
  fetcher: Fetcher,
): Promise<T[]> {
  const response = await cloudflareFetch(
    fetcher,
    `${apiBase}/accounts/${encodeURIComponent(config.accountId)}/d1/database/${encodeURIComponent(config.d1DatabaseId)}/query`,
    config.apiToken,
    { method: 'POST', body: JSON.stringify({ sql }) },
  ) as Array<{ results?: T[] }> | { result?: Array<{ results?: T[] }> }
  if (Array.isArray(response)) return response[0]?.results ?? []
  return response.result?.[0]?.results ?? []
}

async function cloudflareFetch(
  fetcher: Fetcher,
  url: string,
  apiToken: string,
  init: RequestInit = {},
): Promise<unknown> {
  const response = await fetcher(url, {
    ...init,
    headers: {
      authorization: `Bearer ${apiToken}`,
      'content-type': 'application/json',
      ...(init.headers ?? {}),
    },
  })
  if (!response.ok) throw new Error(`Cloudflare API request failed with HTTP ${response.status}`)
  const body = await response.json() as { success?: unknown, result?: unknown, errors?: Array<{ message?: unknown }> }
  if (body.success !== true) {
    throw new Error(`Cloudflare API request failed: ${String(body.errors?.[0]?.message ?? 'unknown error')}`)
  }
  return body
}

function usage(): string {
  return `Usage: bun apps/web/scripts/purge-legacy-games.ts --env staging|production --epoch STABLE_EPOCH [--confirm]\n\nDefaults to dry-run. --confirm mutates only an explicitly maintenance-gated deployment.\n`
}

if (import.meta.main) {
  try {
    const args = parsePurgeArguments(Bun.argv.slice(2))
    const result = await runLegacyPurge(configFromEnvironment(args))
    console.log(JSON.stringify(purgeRunSummary(result)))
  } catch (error) {
    // Never interpolate configuration or error objects here: an SDK/fetch
    // error can retain request headers, including management credentials.
    const message = error instanceof Error ? error.message : 'legacy purge failed'
    console.error(`Error: ${redactSecrets(message, [
      Bun.env.CLOUDFLARE_API_TOKEN,
      Bun.env.LEGACY_PURGE_SECRET,
    ])}`)
    console.error(usage())
    process.exitCode = 1
  }
}
