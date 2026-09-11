/**
 * 專為 issue #75 硬切換設計的一次性、刻意僅操作遠端的管理工具。它永遠不會
 * 啟用維護或部署 Worker：操作人員必須先明確部署維護設定，再執行此工具。
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
  /** 從操作人員 Wrangler 設定檔取得的暫時性 OAuth/API 權杖。 */
  authorizationToken: string
  workerName: string
  workerUrl: string
  d1DatabaseId: string
  gameRoomClassName: string
  replayClassName: string
}

export interface DurableObjectInventoryItem {
  id: string
  hasStoredData: boolean
}

export interface DurableObjectNamespace {
  id: string
  className: string
  scriptName: string
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
  /** dry-run 額外回報權威 probe 判定的清除候選數。 */
  candidateRooms?: number
}

export interface LegacyRoomProbe {
  kind: 'room-index' | 'game-room-object'
  id: string
  status: 'preserved' | 'absent' | 'broken'
}

export interface FetchResponse {
  ok: boolean
  status: number
  json(): Promise<unknown>
  text(): Promise<string>
}

interface ManagementPostOptions {
  allowNotFound?: boolean
  allowBroken?: boolean
}

export type Fetcher = (input: string, init?: RequestInit) => Promise<FetchResponse>

export interface WranglerCommandResult {
  exitCode: number
  stdout: string
}

export type WranglerCommandRunner = (
  arguments_: string[],
) => Promise<WranglerCommandResult>

export type WranglerAccountIdResolver = (
  configuredAccountId?: string,
) => Promise<string>

export type WranglerTomlReader = () => Promise<string>

const apiBase = 'https://api.cloudflare.com/client/v4'

/**
 * Wrangler 的 JSON 命令通常會寫入一個 JSON 物件，但本機憑證儲存診斷可能出現
 * 在它之前。只保留有效物件，不呈現原始輸出：其中可能包含操作人員帳戶詳細資料。
 */
function parseWranglerJson(stdout: string, unreadableMessage: string): Record<string, unknown> {
  const output = stdout
    .replace(/^\uFEFF/, '')
    .replace(/\u001B\[[0-?]*[ -/]*[@-~]/g, '')
    .trim()
  const end = output.lastIndexOf('}')
  for (let start = output.indexOf('{'); start >= 0 && start < end; start = output.indexOf('{', start + 1)) {
    try {
      const parsed = JSON.parse(output.slice(start, end + 1))
      if (typeof parsed === 'object' && parsed !== null && !Array.isArray(parsed)) {
        return parsed as Record<string, unknown>
      }
    } catch {
      // 如果 Wrangler 先輸出診斷訊息，嘗試尋找後面的物件起點。
    }
  }
  throw new Error(unreadableMessage)
}

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

export function localWranglerExecutable(): string {
  return decodeURIComponent(
    new URL('../node_modules/.bin/wrangler', import.meta.url).pathname,
  )
}

async function runWrangler(arguments_: string[]): Promise<WranglerCommandResult> {
  const executable = localWranglerExecutable()
  const child = Bun.spawn([executable, ...arguments_], {
    stdout: 'pipe',
    stderr: 'pipe',
  })
  const [exitCode, stdout, stderr] = await Promise.all([
    child.exited,
    new Response(child.stdout).text(),
    // 擷取兩個串流：Wrangler 憑證儲存診斷可能使用任一串流，消費 stderr 也能
    // 避免失敗的子程序阻塞。
    new Response(child.stderr).text(),
  ])
  return { exitCode, stdout: `${stdout}\n${stderr}` }
}

/**
 * 重用已驗證的 Wrangler 設定檔，而不是要求操作人員將 API 權杖複製到 shell。
 * `wrangler auth token` 會在需要時更新 OAuth 憑證，而此工具只在記憶體中保留結果。
 */
export async function authorizationTokenFromWrangler(
  run: WranglerCommandRunner = runWrangler,
): Promise<string> {
  const result = await run(['auth', 'token', '--json'])
  if (result.exitCode !== 0) {
    throw new Error('Wrangler authentication is unavailable; run `bunx wrangler login` first')
  }
  const credential = parseWranglerJson(
    result.stdout,
    'Wrangler returned an unreadable authentication response',
  )
  if (
    (credential.type === 'oauth' || credential.type === 'api_token')
    && typeof credential.token === 'string'
    && credential.token.trim()
  ) {
    return credential.token.trim()
  }
  throw new Error('Wrangler did not provide an OAuth or API token; run `bunx wrangler login` first')
}

/**
 * 從已登入的 Wrangler 設定檔解析帳戶。只有在設定檔屬於單一帳戶時，自動選擇
 * 帳戶才安全；多帳戶設定檔必須明確指定目標帳戶。
 */
export async function accountIdFromWrangler(
  run: WranglerCommandRunner = runWrangler,
  configuredAccountId?: string,
): Promise<string> {
  const result = await run(['whoami', '--json'])
  if (result.exitCode !== 0) {
    throw new Error('Wrangler authentication is unavailable; run `bunx wrangler login` first')
  }
  const identity = parseWranglerJson(
    result.stdout,
    'Wrangler returned an unreadable account response',
  )
  if (identity.loggedIn !== true || !Array.isArray(identity.accounts)) {
    throw new Error('Wrangler did not provide authenticated account information')
  }
  const accountIds = [...new Set(identity.accounts.flatMap((account): string[] => {
    if (typeof account !== 'object' || account === null) return []
    const id = (account as { id?: unknown }).id
    return typeof id === 'string' && id.trim() ? [id.trim()] : []
  }))]
  if (configuredAccountId) {
    if (accountIds.includes(configuredAccountId)) return configuredAccountId
    throw new Error('CLOUDFLARE_ACCOUNT_ID is not available to the authenticated Wrangler profile')
  }
  if (accountIds.length === 1) return accountIds[0]!
  if (accountIds.length === 0) {
    throw new Error('Wrangler did not provide an account; run `bunx wrangler login` first')
  }
  throw new Error('Wrangler profile has multiple accounts; set CLOUDFLARE_ACCOUNT_ID to the intended account')
}

function record(value: unknown): Record<string, unknown> | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : undefined
}

function requiredTomlString(value: unknown, description: string): string {
  if (typeof value !== 'string' || !value.trim()) {
    throw new Error(`wrangler.toml is missing ${description}`)
  }
  return value.trim()
}

/** 只讀取具名的部署環境值；絕不繼承開發環境設定。 */
export function purgeTargetFromWranglerToml(
  toml: string,
  environment: PurgeEnvironment,
): Pick<PurgeConfig, 'workerName' | 'workerUrl' | 'd1DatabaseId' | 'gameRoomClassName' | 'replayClassName'> {
  const root = record(Bun.TOML.parse(toml))
  const environments = record(root?.env)
  const target = record(environments?.[environment])
  if (!target) throw new Error(`wrangler.toml is missing env.${environment}`)

  const workerName = requiredTomlString(target.name, `env.${environment}.name`)
  const vars = record(target.vars)
  const workerUrl = requiredTomlString(
    vars?.BETTER_AUTH_URL,
    `env.${environment}.vars.BETTER_AUTH_URL`,
  ).replace(/\/$/, '')
  try {
    const url = new URL(workerUrl)
    if (url.protocol !== 'https:' && url.protocol !== 'http:') throw new Error()
  } catch {
    throw new Error(`wrangler.toml has an invalid env.${environment}.vars.BETTER_AUTH_URL`)
  }

  const d1Databases = target.d1_databases
  const database = Array.isArray(d1Databases)
    ? d1Databases.map(record).find(item => item?.binding === 'DB')
    : undefined
  const d1DatabaseId = requiredTomlString(
    database?.database_id,
    `env.${environment}.d1_databases DB database_id`,
  )
  if (d1DatabaseId.startsWith('REPLACE_WITH_')) {
    throw new Error(`wrangler.toml has a placeholder env.${environment} D1 database ID`)
  }

  const bindings = record(target.durable_objects)?.bindings
  const binding = (name: string): Record<string, unknown> | undefined =>
    Array.isArray(bindings) ? bindings.map(record).find(item => item?.name === name) : undefined
  return {
    workerName,
    workerUrl,
    d1DatabaseId,
    gameRoomClassName: requiredTomlString(
      binding('GAME_ROOM')?.class_name,
      `env.${environment}.durable_objects GAME_ROOM class_name`,
    ),
    replayClassName: requiredTomlString(
      binding('REPLAY')?.class_name,
      `env.${environment}.durable_objects REPLAY class_name`,
    ),
  }
}

export function localWranglerTomlPath(): string {
  return decodeURIComponent(new URL('../wrangler.toml', import.meta.url).pathname)
}

async function readWranglerToml(): Promise<string> {
  return await Bun.file(localWranglerTomlPath()).text()
}

export async function configFromEnvironment(
  args: PurgeArguments,
  source: Record<string, string | undefined> = Bun.env,
  resolveAuthorizationToken: () => Promise<string> = authorizationTokenFromWrangler,
  resolveAccountId: WranglerAccountIdResolver = configuredAccountId =>
    accountIdFromWrangler(runWrangler, configuredAccountId),
  readToml: WranglerTomlReader = readWranglerToml,
): Promise<PurgeConfig> {
  const target = purgeTargetFromWranglerToml(await readToml(), args.environment)

  return {
    ...args,
    accountId: await resolveAccountId(source.CLOUDFLARE_ACCOUNT_ID?.trim() || undefined),
    authorizationToken: await resolveAuthorizationToken(),
    ...target,
  }
}

export async function listDurableObjects(
  config: Pick<PurgeConfig, 'accountId' | 'authorizationToken'>,
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
      config.authorizationToken,
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

export async function listDurableObjectNamespaces(
  config: Pick<PurgeConfig, 'accountId' | 'authorizationToken'>,
  fetcher: Fetcher = fetch,
): Promise<DurableObjectNamespace[]> {
  const namespaces: DurableObjectNamespace[] = []
  let page = 1
  let totalPages = 1
  do {
    const search = new URLSearchParams({ page: String(page), per_page: '100' })
    const response = await cloudflareFetch(
      fetcher,
      `${apiBase}/accounts/${encodeURIComponent(config.accountId)}/workers/durable_objects/namespaces?${search}`,
      config.authorizationToken,
    ) as {
      result?: Array<{ id?: unknown, class?: unknown, script?: unknown }>
      result_info?: { total_pages?: unknown }
    }
    for (const namespace of response.result ?? []) {
      if (
        typeof namespace.id === 'string'
        && namespace.id
        && typeof namespace.class === 'string'
        && namespace.class
        && typeof namespace.script === 'string'
        && namespace.script
      ) {
        namespaces.push({
          id: namespace.id,
          className: namespace.class,
          scriptName: namespace.script,
        })
      }
    }
    totalPages = Number.isInteger(response.result_info?.total_pages)
      ? Number(response.result_info?.total_pages)
      : page
    page += 1
  } while (page <= totalPages)
  return namespaces
}

async function purgeNamespaceIds(
  config: Pick<PurgeConfig, 'accountId' | 'authorizationToken' | 'workerName' | 'gameRoomClassName' | 'replayClassName'>,
  fetcher: Fetcher,
): Promise<{ gameRoomNamespaceId: string, replayNamespaceId: string }> {
  const namespaces = await listDurableObjectNamespaces(config, fetcher)
  const idFor = (className: string): string => {
    const matches = namespaces.filter(namespace =>
      namespace.scriptName === config.workerName && namespace.className === className,
    )
    if (matches.length !== 1) {
      throw new Error(`expected one Durable Object namespace for ${config.workerName}/${className}`)
    }
    return matches[0]!.id
  }
  return {
    gameRoomNamespaceId: idFor(config.gameRoomClassName),
    replayNamespaceId: idFor(config.replayClassName),
  }
}

export async function purgeInventory(
  config: Pick<PurgeConfig, 'accountId' | 'authorizationToken' | 'workerName' | 'd1DatabaseId' | 'gameRoomClassName' | 'replayClassName'>,
  fetcher: Fetcher = fetch,
): Promise<PurgeInventory> {
  const namespaces = await purgeNamespaceIds(config, fetcher)
  const [roomRows, replayRows, gameRoomObjects, replayObjects] = await Promise.all([
    d1Query<{ game_id?: unknown }>(config, 'SELECT game_id FROM public_game_room ORDER BY game_id', fetcher),
    d1Query<{ replay_id?: unknown }>(config, 'SELECT replay_id FROM player_saved_replay ORDER BY replay_id', fetcher),
    listDurableObjects(config, namespaces.gameRoomNamespaceId, fetcher),
    listDurableObjects(config, namespaces.replayNamespaceId, fetcher),
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
): Promise<{
  inventory: PurgeInventory
  mutated: boolean
  mutationCount: number
  roomProbes?: LegacyRoomProbe[]
}> {
  const inventory = await purgeInventory(config, fetcher)
  if (!config.confirm) {
    // dry-run 也使用 Wrangler 的暫時權杖做唯讀 probe，不繞過 maintenance/auth gate。
    const roomProbes = await probeLegacyRooms(config, inventory, fetcher)
    return { inventory, mutated: false, mutationCount: 0, roomProbes }
  }

  let mutationCount = 0
  const removedRoomIds: string[] = []
  for (const object of inventory.gameRoomObjects) {
    const result = await managementPost(
      config,
      'room',
      { objectId: object.id },
      fetcher,
      { allowNotFound: true, allowBroken: true },
    ) as { status?: unknown }
    if (result.status === 'absent') {
      mutationCount += 1
    } else if (result.status === 'broken') {
      mutationCount += 1
    } else if (result.status !== 'preserved') {
      throw new Error(`management request room returned an unreadable response for ${object.id}`)
    }
  }
  for (const roomId of inventory.roomIds) {
    const result = await managementPost(
      config,
      'room-index',
      { roomId },
      fetcher,
      { allowBroken: true },
    ) as {
      status?: unknown
      roomIndexDeleted?: unknown
    }
    if (result.status === 'absent' || result.status === 'broken') {
      removedRoomIds.push(roomId)
      mutationCount += Number(result.roomIndexDeleted ?? 0)
    } else if (result.status !== 'preserved') {
      throw new Error(`management request room-index returned an unreadable response for ${roomId}`)
    }
  }
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
  await verifyPurge(config, inventory, fetcher, removedRoomIds)
  return { inventory, mutated: true, mutationCount }
}

/** CLI 唯一的輸出形狀：刻意只包含盤點資料，絕不包含憑證。 */
export function purgeRunSummary(
  result: Awaited<ReturnType<typeof runLegacyPurge>>,
): PurgeRunSummary {
  const summary: PurgeRunSummary = {
    mode: result.mutated ? 'mutated' : 'dry-run',
    mutationCount: result.mutationCount,
    rooms: result.inventory.roomIds.length,
    gameRoomObjects: result.inventory.gameRoomObjects.length,
    replayObjects: result.inventory.replayObjects.length,
  }
  if (result.roomProbes) {
    summary.candidateRooms = result.roomProbes.filter(probe => probe.status !== 'preserved').length
  }
  return summary
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
  removedRoomIds: string[] = [],
): Promise<void> {
  const [roomRows, replayCounts] = await Promise.all([
    d1Query<{ game_id?: unknown }>(config, 'SELECT game_id FROM public_game_room ORDER BY game_id', fetcher),
    d1Query<{ saved_count?: unknown, lifecycle_count?: unknown }>(
      config,
      'SELECT (SELECT COUNT(*) FROM player_saved_replay) AS saved_count, (SELECT COUNT(*) FROM replay_archive_lifecycle) AS lifecycle_count',
      fetcher,
    ),
  ])
  const roomIds = roomRows
    .map(row => row.game_id)
    .filter((value): value is string => typeof value === 'string')
  const expectedRoomIds = inventory.roomIds.filter(roomId => !removedRoomIds.includes(roomId))
  if (JSON.stringify(roomIds) !== JSON.stringify(expectedRoomIds)) {
    throw new Error('verification failed: preserved room identities do not match the dry-run inventory')
  }
  const counts = replayCounts[0] ?? {}
  if (Number(counts.saved_count ?? -1) !== 0 || Number(counts.lifecycle_count ?? -1) !== 0) {
    throw new Error('verification failed: legacy replay tables are not empty')
  }
  for (const object of inventory.gameRoomObjects) {
    const verification = await managementPost(
      config,
      'room-probe',
      { objectId: object.id },
      fetcher,
      { allowNotFound: true },
    ) as {
      status?: unknown
    }
    if (verification.status !== 'Absent' && verification.status !== 'absent'
      && verification.status !== 'preserved') {
      throw new Error(`verification failed: room response was not preserved or absent for ${object.id}`)
    }
  }
  for (const roomId of expectedRoomIds) {
    const verification = await managementPost(
      config,
      'room-probe',
      { roomId },
      fetcher,
    ) as { status?: unknown }
    if (verification.status !== 'preserved') {
      throw new Error(`verification failed: preserved room is not readable for ${roomId}`)
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
  options: ManagementPostOptions = {},
): Promise<unknown> {
  const response = await fetcher(`${config.workerUrl}/internal/legacy-purge/${path}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      'x-fewfc-cloudflare-token': config.authorizationToken,
    },
    body: JSON.stringify({ epoch: config.epoch, ...body }),
  })
  const allowsExpectedFailure = (options.allowNotFound && response.status === 404)
    || (options.allowBroken && response.status === 500)
  if (!response.ok && !allowsExpectedFailure) {
    throw new Error(`management request ${path} failed with HTTP ${response.status}`)
  }
  const result = await response.json()
  if (response.status === 404 && options.allowNotFound) {
    if (!result || typeof result !== 'object' || Array.isArray(result)) {
      throw new Error(`management request ${path} returned an unreadable not-found response`)
    }
    const status = (result as { status?: unknown }).status
    if ((result as { error?: unknown }).error === 'not found') {
      throw new Error(
        `management request ${path} was rejected by the management gate; `
        + 'verify maintenance mode, Wrangler token, and Cloudflare account ID',
      )
    }
    if (status !== 'Absent' && status !== 'absent') {
      throw new Error(`management request ${path} returned an unexpected not-found response`)
    }
  }
  if (response.status === 500 && options.allowBroken) {
    if (!result || typeof result !== 'object' || Array.isArray(result)) {
      throw new Error(`management request ${path} returned an unreadable broken response`)
    }
    if ((result as { status?: unknown }).status !== 'broken') {
      throw new Error(`management request ${path} returned an unexpected broken response`)
    }
  }
  return result
}

async function probeLegacyRooms(
  config: PurgeConfig,
  inventory: PurgeInventory,
  fetcher: Fetcher,
): Promise<LegacyRoomProbe[]> {
  const probes: LegacyRoomProbe[] = []
  for (const object of inventory.gameRoomObjects) {
    const result = await managementPost(
      config,
      'room-probe',
      { objectId: object.id },
      fetcher,
      { allowNotFound: true, allowBroken: true },
    ) as { status?: unknown }
    if (result.status !== 'preserved' && result.status !== 'absent' && result.status !== 'broken') {
      throw new Error(`management request room-probe returned an unreadable response for ${object.id}`)
    }
    probes.push({ kind: 'game-room-object', id: object.id, status: result.status })
  }
  for (const roomId of inventory.roomIds) {
    const result = await managementPost(
      config,
      'room-probe',
      { roomId },
      fetcher,
      { allowNotFound: true, allowBroken: true },
    ) as { status?: unknown }
    if (result.status !== 'preserved' && result.status !== 'absent' && result.status !== 'broken') {
      throw new Error(`management request room-probe returned an unreadable response for ${roomId}`)
    }
    probes.push({ kind: 'room-index', id: roomId, status: result.status })
  }
  return probes
}

async function d1Query<T>(
  config: Pick<PurgeConfig, 'accountId' | 'authorizationToken' | 'd1DatabaseId'>,
  sql: string,
  fetcher: Fetcher,
): Promise<T[]> {
  const response = await cloudflareFetch(
    fetcher,
    `${apiBase}/accounts/${encodeURIComponent(config.accountId)}/d1/database/${encodeURIComponent(config.d1DatabaseId)}/query`,
    config.authorizationToken,
    { method: 'POST', body: JSON.stringify({ sql }) },
  ) as Array<{ results?: T[] }> | { result?: Array<{ results?: T[] }> }
  if (Array.isArray(response)) return response[0]?.results ?? []
  return response.result?.[0]?.results ?? []
}

async function cloudflareFetch(
  fetcher: Fetcher,
  url: string,
  authorizationToken: string,
  init: RequestInit = {},
): Promise<unknown> {
  const response = await fetcher(url, {
    ...init,
    headers: {
      authorization: `Bearer ${authorizationToken}`,
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
  const sensitiveValues: Array<string | undefined> = []
  try {
    const args = parsePurgeArguments(Bun.argv.slice(2))
    const config = await configFromEnvironment(args)
    sensitiveValues.push(config.authorizationToken)
    const result = await runLegacyPurge(config)
    console.log(JSON.stringify(purgeRunSummary(result)))
  } catch (error) {
    // 絕不要在這裡插入設定或錯誤物件：SDK/fetch 錯誤可能保留請求標頭，
    // 包括管理憑證。
    const message = error instanceof Error ? error.message : 'legacy purge failed'
    console.error(`Error: ${redactSecrets(message, sensitiveValues)}`)
    console.error(usage())
    process.exitCode = 1
  }
}
