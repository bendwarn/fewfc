/**
 * CI 使用的 Worker 版本標籤與部署防護。
 * 標籤刻意以提交雜湊定址，維護切換只能選取同一提交建立的成對版本。
 */

export type WorkerVersionTags = {
  normal: string
  maintenance: string
}

export type WorkerReleaseState = {
  state: 'missing' | 'complete'
  tags: WorkerVersionTags
}

const SHA = /^[0-9a-f]{40}$/

export function versionTags(commitSha: string): WorkerVersionTags {
  const sha = commitSha.trim().toLowerCase()
  if (!SHA.test(sha)) throw new Error('commit SHA must be a 40-character hexadecimal value')
  return {
    normal: `fewfc-${sha}-normal`,
    maintenance: `fewfc-${sha}-maintenance`,
  }
}

export function parseVersionTag(tag: unknown): { sha: string; mode: 'normal' | 'maintenance' } | undefined {
  if (typeof tag !== 'string') return undefined
  const match = /^fewfc-([0-9a-f]{40})-(normal|maintenance)$/.exec(tag)
  return match ? { sha: match[1]!, mode: match[2] as 'normal' | 'maintenance' } : undefined
}

type JsonRecord = Record<string, unknown>

function record(value: unknown): JsonRecord | undefined {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as JsonRecord
    : undefined
}

function findArray(value: unknown, names: string[]): unknown[] | undefined {
  if (Array.isArray(value)) return value
  const object = record(value)
  for (const name of names) {
    if (Array.isArray(object?.[name])) return object[name] as unknown[]
  }
  const result = record(object?.result)
  if (Array.isArray(object?.result)) return object.result as unknown[]
  for (const name of names) {
    if (Array.isArray(result?.[name])) return result[name] as unknown[]
  }
  return undefined
}

function versionId(value: unknown): string | undefined {
  const object = record(value)
  for (const key of ['version_id', 'versionId', 'id']) {
    if (typeof object?.[key] === 'string' && object[key]) return object[key] as string
  }
  return undefined
}

function versionTag(value: unknown): unknown {
  const object = record(value)
  return object?.tag
    ?? record(object?.metadata)?.tag
    ?? record(object?.annotations)?.['workers/tag']
}

function percentage(value: unknown): number | undefined {
  const object = record(value)
  for (const key of ['percentage', 'percent']) {
    if (typeof object?.[key] === 'number') return object[key] as number
    if (typeof object?.[key] === 'string' && object[key].trim()) return Number(object[key])
  }
  return undefined
}

function deploymentRows(value: unknown): unknown[] {
  if (Array.isArray(value)) return value
  const direct = record(value)
  if (Array.isArray(direct?.deployments)) return direct.deployments as unknown[]
  if (Array.isArray(direct?.result)) return direct.result as unknown[]
  const result = record(direct?.result)
  if (Array.isArray(result?.deployments)) return result.deployments as unknown[]
  if (Array.isArray(result?.versions)) return [result]
  if (Array.isArray(direct?.versions)) return [direct]
  return []
}

export function deployedVersionId(deployments: unknown): string {
  const current = deploymentRows(deployments)[0]
  const versions = findArray(current, ['versions']) ?? []
  if (versions.length !== 1 || percentage(versions[0]) !== 100) {
    throw new Error('could not resolve the single 100% deployed Worker version')
  }
  const id = versionId(versions[0])
  if (!id) throw new Error('could not resolve the single 100% deployed Worker version')
  return id
}

export function resolveCurrentPair(
  deployments: unknown,
  versions: unknown,
  requestedMode?: 'enable' | 'disable',
): WorkerVersionTags {
  const activeId = deployedVersionId(deployments)
  const rows = findArray(versions, ['versions', 'result']) ?? []
  const active = rows.find((row) => versionId(row) === activeId)
  const tag = parseVersionTag(versionTag(active))
  if (!tag) {
    throw new Error('the 100% deployed Worker version is not tagged')
  }
  if (requestedMode === 'enable' && tag.mode !== 'normal') {
    throw new Error('enable requires a tagged normal version to be active')
  }
  if (requestedMode === 'disable' && tag.mode !== 'maintenance') {
    throw new Error('disable requires a tagged maintenance version to be active')
  }
  const expected = versionTags(tag.sha)
  const normal = rows.filter((row) => versionTag(row) === expected.normal)
  const maintenance = rows.filter((row) => versionTag(row) === expected.maintenance)
  if (normal.length !== 1 || maintenance.length !== 1) {
    throw new Error('the deployed version has a partial or ambiguous Worker version pair')
  }
  return expected
}

/**
 * 重新執行同一提交的部署時，不可再上傳同名版本標籤。只有完整的一對可以重用；
 * 半對或重複標籤一律拒絕，避免版本選擇變得不確定。
 */
export function releaseState(versions: unknown, commitSha: string): WorkerReleaseState {
  const tags = versionTags(commitSha)
  const rows = findArray(versions, ['versions', 'result']) ?? []
  const normal = rows.filter((row) => versionTag(row) === tags.normal)
  const maintenance = rows.filter((row) => versionTag(row) === tags.maintenance)
  if (normal.length === 0 && maintenance.length === 0) return { state: 'missing', tags }
  if (normal.length === 1 && maintenance.length === 1) return { state: 'complete', tags }
  throw new Error('the requested commit has a partial or ambiguous Worker version pair')
}

export function resolveCurrentNormalPair(deployments: unknown, versions: unknown): WorkerVersionTags {
  const tags = resolveCurrentPair(deployments, versions)
  const activeId = deployedVersionId(deployments)
  const rows = findArray(versions, ['versions', 'result']) ?? []
  const active = rows.find((row) => versionId(row) === activeId)
  if (parseVersionTag(versionTag(active))?.mode !== 'normal') {
    throw new Error('the 100% deployed Worker version is not a tagged normal version')
  }
  return tags
}

export function assertNotMaintenanceDeployment(deployments: unknown, versions: unknown): void {
  try {
    resolveCurrentNormalPair(deployments, versions)
  } catch {
    throw new Error('refusing ordinary deployment: active Worker version is not a complete tagged normal pair')
  }
}

export async function fetchVersionPromotionMetadata(
  accountId: string,
  workerName: string,
  authorizationToken: string,
  fetcher: typeof fetch = fetch,
): Promise<{ deployments: unknown; versions: unknown[] }> {
  const base = `https://api.cloudflare.com/client/v4/accounts/${encodeURIComponent(accountId)}/workers/scripts/${encodeURIComponent(workerName)}`
  const headers = { Authorization: `Bearer ${authorizationToken}` }
  const deploymentsResponse = await fetcher(`${base}/deployments`, { headers })
  if (!deploymentsResponse.ok) throw new Error(`Cloudflare deployments lookup failed with HTTP ${deploymentsResponse.status}`)
  const deployments = await deploymentsResponse.json() as unknown
  if (record(deployments)?.success !== true) throw new Error('Cloudflare deployments lookup was unsuccessful')

  const versions: unknown[] = []
  let page = 1
  let totalPages = 1
  do {
    const response = await fetcher(`${base}/versions?per_page=100&deployable=true&page=${page}`, { headers })
    if (!response.ok) throw new Error(`Cloudflare versions lookup failed with HTTP ${response.status}`)
    const payload = await response.json() as unknown
    if (record(payload)?.success !== true) throw new Error('Cloudflare versions lookup was unsuccessful')
    const rows = findArray(payload, ['result']) ?? []
    versions.push(...rows)
    const info = record(record(payload)?.result_info)
    const reportedPages = Number(info?.total_pages)
    totalPages = Number.isInteger(reportedPages) && reportedPages > 0 ? reportedPages : page
    page += 1
  } while (page <= totalPages)
  return { deployments, versions }
}

if (import.meta.main) {
  const [command, ...arguments_] = Bun.argv.slice(2)
  if (command === 'metadata') {
    const [accountId, workerName, outputDirectory] = arguments_
    const token = process.env.CLOUDFLARE_API_TOKEN
    if (!accountId || !workerName || !outputDirectory || !token) {
      throw new Error('usage: worker-version-promotion.ts metadata <account-id> <worker-name> <output-directory>')
    }
    const metadata = await fetchVersionPromotionMetadata(accountId, workerName, token)
    await Bun.write(`${outputDirectory}/deployments.json`, JSON.stringify(metadata.deployments))
    await Bun.write(`${outputDirectory}/versions.json`, JSON.stringify(metadata.versions))
    process.exit(0)
  }
  if (command === 'release') {
    const [versionsPath, commitSha] = arguments_
    if (!versionsPath || !commitSha) {
      throw new Error('usage: worker-version-promotion.ts release <versions.json> <commit-sha>')
    }
    const versions = JSON.parse(await Bun.file(versionsPath).text())
    console.log(JSON.stringify(releaseState(versions, commitSha)))
    process.exit(0)
  }
  const [deploymentsPath, versionsPath, requestedMode] = arguments_
  if ((command !== 'guard' && command !== 'pair') || !deploymentsPath || !versionsPath) {
    throw new Error('usage: worker-version-promotion.ts <guard|pair> <deployments.json> <versions.json>')
  }
  const deployments = JSON.parse(await Bun.file(deploymentsPath).text())
  const versions = JSON.parse(await Bun.file(versionsPath).text())
  if (command === 'guard') assertNotMaintenanceDeployment(deployments, versions)
  else {
    if (requestedMode !== undefined && requestedMode !== 'enable' && requestedMode !== 'disable') {
      throw new Error('pair mode must be enable or disable')
    }
    console.log(JSON.stringify(resolveCurrentPair(deployments, versions, requestedMode)))
  }
}
