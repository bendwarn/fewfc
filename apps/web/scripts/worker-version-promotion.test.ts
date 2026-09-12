import { describe, expect, test } from 'bun:test'
import { readFile } from 'node:fs/promises'
import {
  assertNotMaintenanceDeployment,
  fetchVersionPromotionMetadata,
  releaseState,
  resolveCurrentPair,
  parseVersionTag,
  resolveCurrentNormalPair,
  versionTags,
} from './worker-version-promotion'

const sha = '0123456789abcdef0123456789abcdef01234567'
const tags = versionTags(sha)

describe('Worker version pairing', () => {
  test('creates immutable paired tags from the commit SHA', () => {
    expect(tags).toEqual({
      normal: `fewfc-${sha}-normal`,
      maintenance: `fewfc-${sha}-maintenance`,
    })
    expect(parseVersionTag(tags.maintenance)).toEqual({ sha, mode: 'maintenance' })
  })

  test('resolves the pair from the currently 100% deployed normal version', () => {
    const deployments = [{ versions: [{ version_id: 'normal-id', percentage: 100 }] }]
    const versions = [
      { id: 'normal-id', annotations: { 'workers/tag': tags.normal } },
      { id: 'maintenance-id', annotations: { 'workers/tag': tags.maintenance } },
    ]
    expect(resolveCurrentNormalPair(deployments, versions)).toEqual(tags)
    expect(() => assertNotMaintenanceDeployment(deployments, versions)).not.toThrow()
    expect(() => assertNotMaintenanceDeployment({ result: { versions: [{ version_id: 'normal-id', percentage: 100 }] } }, versions)).not.toThrow()
    expect(resolveCurrentNormalPair([
      { versions: [{ version_id: 'normal-id', percentage: 100 }] },
      { versions: [{ version_id: 'older-id', percentage: 100 }] },
    ], versions)).toEqual(tags)
  })

  test('fails closed for maintenance, untagged, split, or unpaired deployments', () => {
    const maintenance = { versions: [{ version_id: 'maintenance-id', percentage: 100 }] }
    const versions = [
      { id: 'normal-id', annotations: { 'workers/tag': tags.normal } },
      { id: 'maintenance-id', annotations: { 'workers/tag': tags.maintenance } },
    ]
    expect(() => assertNotMaintenanceDeployment(maintenance, versions)).toThrow()
    expect(resolveCurrentPair(maintenance, versions, 'disable')).toEqual(tags)
    expect(() => resolveCurrentPair(maintenance, versions, 'enable')).toThrow()
    expect(() => resolveCurrentNormalPair([{ versions: [{ version_id: 'normal-id', percentage: 50 }, { version_id: 'maintenance-id', percentage: 50 }] }], versions)).toThrow()
    expect(() => resolveCurrentNormalPair([{ versions: [{ version_id: 'normal-id', percentage: 100 }] }], [{ id: 'normal-id', annotations: { 'workers/tag': tags.normal } }])).toThrow()
    expect(() => assertNotMaintenanceDeployment([{ versions: [{ version_id: 'normal-id', percentage: 100 }] }], [{ id: 'normal-id', annotations: { 'workers/tag': tags.normal } }])).toThrow()
  })

  test('reuses only a complete, unambiguous pair for a retried commit', () => {
    const complete = [
      { id: 'normal-id', annotations: { 'workers/tag': tags.normal } },
      { id: 'maintenance-id', annotations: { 'workers/tag': tags.maintenance } },
    ]
    expect(releaseState([], sha)).toEqual({ state: 'missing', tags })
    expect(releaseState(complete, sha)).toEqual({ state: 'complete', tags })
    expect(() => releaseState(complete.slice(0, 1), sha)).toThrow(/partial or ambiguous/)
    expect(() => releaseState([...complete, complete[0]], sha)).toThrow(/partial or ambiguous/)
  })

  test('paginates deployable versions for aged releases', async () => {
    const requests: string[] = []
    const fetcher = async (input: string): Promise<Response> => {
      requests.push(input)
      if (input.endsWith('/deployments')) {
        return Response.json({ success: true, result: [] })
      }
      const page = new URL(input).searchParams.get('page')
      return Response.json({
        success: true,
        result: [{ id: `v-${page}` }],
        result_info: { total_pages: 2 },
      })
    }
    const metadata = await fetchVersionPromotionMetadata('account', 'worker', 'token', fetcher)
    expect(metadata.versions).toHaveLength(2)
    expect(requests.some((request) => request.includes('page=2'))).toBe(true)
  })

  test('keeps direct deployment disabled and checks the active pair before migrations', async () => {
    const packageJson = JSON.parse(await readFile(new URL('../package.json', import.meta.url), 'utf8')) as {
      scripts: Record<string, string>
    }
    expect(packageJson.scripts['cf:deploy:staging']).not.toContain('wrangler deploy')
    expect(packageJson.scripts['cf:deploy:production']).not.toContain('wrangler deploy')

    const workflow = await readFile(new URL('../../../.github/workflows/ci-cd.yml', import.meta.url), 'utf8')
    expect(workflow.indexOf('worker-version-promotion.ts guard')).toBeLessThan(
      workflow.indexOf('wrangler d1 migrations apply fewfc-auth-staging'),
    )
  })
})
