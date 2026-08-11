import { describe, expect, test } from 'bun:test'
import {
  accountIdFromWrangler,
  authorizationTokenFromWrangler,
  configFromEnvironment,
  listDurableObjects,
  listDurableObjectNamespaces,
  localWranglerExecutable,
  parsePurgeArguments,
  purgeTargetFromWranglerToml,
  purgeRunSummary,
  redactSecrets,
  runLegacyPurge,
  type Fetcher,
} from './purge-legacy-games'

const environment = {
  CLOUDFLARE_ACCOUNT_ID: 'account',
  LEGACY_PURGE_SECRET: 'management-secret-that-must-never-be-printed',
}

const wranglerToken = 'wrangler-oauth-token-that-must-never-be-printed'

const wranglerToml = `
[env.staging]
name = "fewfc-web-staging"

[env.staging.vars]
BETTER_AUTH_URL = "https://staging.example.test/"

[[env.staging.d1_databases]]
binding = "DB"
database_id = "d1-staging"

[[env.staging.durable_objects.bindings]]
name = "GAME_ROOM"
class_name = "GameRoom"

[[env.staging.durable_objects.bindings]]
name = "REPLAY"
class_name = "ReplayArchive"
`

async function configuration(args: Parameters<typeof configFromEnvironment>[0], source = environment) {
  return await configFromEnvironment(
    args,
    source,
    async () => wranglerToken,
    async accountId => accountId ?? 'account',
    async () => wranglerToml,
  )
}

function response(body: unknown, status = 200) {
  return {
    ok: status >= 200 && status < 300,
    status,
    json: async () => body,
    text: async () => JSON.stringify(body),
  }
}

function namespaceResponse() {
  return response({
    success: true,
    result: [
      { id: 'game-room-namespace', class: 'GameRoom', script: 'fewfc-web-staging' },
      { id: 'replay-namespace', class: 'ReplayArchive', script: 'fewfc-web-staging' },
    ],
    result_info: { total_pages: 1 },
  })
}

describe('purge-legacy-games', () => {
  test('uses the project-local Wrangler command shim', () => {
    expect(localWranglerExecutable()).toEndWith('/node_modules/.bin/wrangler')
  })

  test('reads all fixed target settings from the named Wrangler TOML environment', () => {
    expect(purgeTargetFromWranglerToml(wranglerToml, 'staging')).toEqual({
      workerName: 'fewfc-web-staging',
      workerUrl: 'https://staging.example.test',
      d1DatabaseId: 'd1-staging',
      gameRoomClassName: 'GameRoom',
      replayClassName: 'ReplayArchive',
    })
    expect(() => purgeTargetFromWranglerToml(wranglerToml, 'production'))
      .toThrow('missing env.production')
  })

  test('requires one explicit target environment and a stable epoch', () => {
    expect(() => parsePurgeArguments(['--epoch', 'cutover-75'])).toThrow('--env')
    expect(() => parsePurgeArguments(['--env', 'local', '--epoch', 'cutover-75'])).toThrow('staging or production')
    expect(() => parsePurgeArguments(['--env', 'staging', '--env', 'production', '--epoch', 'cutover-75'])).toThrow('exactly once')
    expect(() => parsePurgeArguments(['--env', 'staging', '--epoch', 'x'])).toThrow('stable identifier')
    expect(parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75'])).toEqual({
      environment: 'staging', epoch: 'cutover-75', confirm: false,
    })
  })

  test('uses the authenticated Wrangler OAuth profile and sole account without environment credentials', async () => {
    const calls: string[][] = []
    const run = async (arguments_: string[]) => {
      calls.push(arguments_)
      if (arguments_[0] === 'auth') {
        return {
          exitCode: 0,
          stdout: JSON.stringify({ type: 'oauth', token: wranglerToken }),
        }
      }
      return {
        exitCode: 0,
        stdout: JSON.stringify({
          loggedIn: true,
          accounts: [{ id: 'account', name: 'Fewfc' }],
        }),
      }
    }
    const config = await configFromEnvironment(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75']),
      { ...environment, CLOUDFLARE_ACCOUNT_ID: undefined },
      () => authorizationTokenFromWrangler(run),
      accountId => accountIdFromWrangler(run, accountId),
      async () => wranglerToml,
    )

    expect(calls).toEqual([['whoami', '--json'], ['auth', 'token', '--json']])
    expect(config.authorizationToken).toBe(wranglerToken)
    expect(config.accountId).toBe('account')
    expect(config).toMatchObject({
      workerName: 'fewfc-web-staging',
      workerUrl: 'https://staging.example.test',
      d1DatabaseId: 'd1-staging',
      gameRoomClassName: 'GameRoom',
      replayClassName: 'ReplayArchive',
    })
    expect('CLOUDFLARE_API_TOKEN' in environment).toBe(false)
    expect(Object.keys(environment).some(key => key.startsWith('FEWFC_STAGING_'))).toBe(false)
  })

  test('fails closed when Wrangler does not return an OAuth or API token', async () => {
    await expect(authorizationTokenFromWrangler(async () => ({
      exitCode: 1,
      stdout: 'not JSON',
    }))).rejects.toThrow('wrangler login')
    await expect(authorizationTokenFromWrangler(async () => ({
      exitCode: 0,
      stdout: JSON.stringify({ type: 'api_key', token: 'not-an-acceptable-credential' }),
    }))).rejects.toThrow('did not provide an OAuth or API token')
  })

  test('requires an explicit account ID for a multi-account Wrangler profile', async () => {
    const run = async () => ({
      exitCode: 0,
      stdout: JSON.stringify({
        loggedIn: true,
        accounts: [{ id: 'first' }, { id: 'second' }],
      }),
    })

    await expect(accountIdFromWrangler(run)).rejects.toThrow('multiple accounts')
    await expect(accountIdFromWrangler(run, 'second')).resolves.toBe('second')
    await expect(accountIdFromWrangler(run, 'other')).rejects.toThrow('not available')
  })

  test('accepts the valid account JSON after non-JSON Wrangler diagnostics', async () => {
    await expect(accountIdFromWrangler(async () => ({
      exitCode: 0,
      stdout: `\u001B[31mWrangler credential diagnostic\u001B[0m\n${JSON.stringify({
        loggedIn: true,
        accounts: [{ id: 'account' }],
      })}`,
    }))).resolves.toBe('account')
  })

  test('defaults to dry-run and does not require or transmit a management secret', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75']),
      { ...environment, LEGACY_PURGE_SECRET: undefined },
    )
    const calls: string[] = []
    const fetcher: Fetcher = async (url) => {
      calls.push(url)
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) return response({ success: true, result: [{ results: [{ game_id: 'room-1' }] }] })
      return response({ success: true, result: [] })
    }

    const result = await runLegacyPurge(config, fetcher)

    expect(result).toMatchObject({ mutated: false, mutationCount: 0 })
    expect(calls.some(url => url.includes('internal/legacy-purge'))).toBe(false)
  })

  test('prints an inventory-only summary and redacts API credentials from failures', () => {
    const summary = purgeRunSummary({
      inventory: {
        roomIds: ['room-1'], replayIds: ['replay-1'],
        gameRoomObjects: [{ id: 'room-object', hasStoredData: true }],
        replayObjects: [{ id: 'replay-object', hasStoredData: true }],
      },
      mutated: false,
      mutationCount: 0,
    })

    expect(JSON.stringify(summary)).not.toContain(wranglerToken)
    expect(JSON.stringify(summary)).not.toContain(environment.LEGACY_PURGE_SECRET)
    expect(redactSecrets(
      `request failed for ${wranglerToken}/${environment.LEGACY_PURGE_SECRET}`,
      [wranglerToken, environment.LEGACY_PURGE_SECRET],
    )).toBe('request failed for [redacted]/[redacted]')
  })

  test('follows Durable Object API cursors and retains only object ids and storage facts', async () => {
    const calls: string[] = []
    const fetcher: Fetcher = async (url) => {
      calls.push(url)
      if (url.includes('cursor=next-page')) {
        return response({ success: true, result: [{ id: 'second', hasStoredData: false }], result_info: {} })
      }
      return response({
        success: true,
        result: [{ id: 'first', hasStoredData: true }],
        result_info: { cursor: 'next-page' },
      })
    }

    await expect(listDurableObjects({ accountId: 'account', authorizationToken: 'secret' }, 'namespace', fetcher))
      .resolves.toEqual([
        { id: 'first', hasStoredData: true },
        { id: 'second', hasStoredData: false },
      ])
    expect(calls).toHaveLength(2)
  })

  test('lists and paginates Durable Object namespaces by worker and class metadata', async () => {
    const calls: string[] = []
    const fetcher: Fetcher = async (url) => {
      calls.push(url)
      if (url.includes('page=2')) {
        return response({
          success: true,
          result: [{ id: 'replay-namespace', class: 'ReplayArchive', script: 'fewfc-web-staging' }],
          result_info: { total_pages: 2 },
        })
      }
      return response({
        success: true,
        result: [{ id: 'game-room-namespace', class: 'GameRoom', script: 'fewfc-web-staging' }],
        result_info: { total_pages: 2 },
      })
    }

    await expect(listDurableObjectNamespaces({ accountId: 'account', authorizationToken: 'secret' }, fetcher))
      .resolves.toEqual([
        { id: 'game-room-namespace', className: 'GameRoom', scriptName: 'fewfc-web-staging' },
        { id: 'replay-namespace', className: 'ReplayArchive', scriptName: 'fewfc-web-staging' },
      ])
    expect(calls).toHaveLength(2)
  })

  test('fails closed before inventory when a configured Durable Object namespace is ambiguous', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75']),
      { ...environment, LEGACY_PURGE_SECRET: undefined },
    )
    const calls: string[] = []
    const fetcher: Fetcher = async (url) => {
      calls.push(url)
      return response({
        success: true,
        result: [
          { id: 'first', class: 'GameRoom', script: 'fewfc-web-staging' },
          { id: 'second', class: 'GameRoom', script: 'fewfc-web-staging' },
          { id: 'replay', class: 'ReplayArchive', script: 'fewfc-web-staging' },
        ],
        result_info: { total_pages: 1 },
      })
    }

    await expect(runLegacyPurge(config, fetcher))
      .rejects.toThrow('expected one Durable Object namespace for fewfc-web-staging/GameRoom')
    expect(calls).toHaveLength(1)
    expect(calls[0]).toContain('/durable_objects/namespaces?')
  })

  test('reports zero new mutation when a same-epoch rerun sees only already-purged rooms', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75', '--confirm']),
      environment,
    )
    const fetcher: Fetcher = async (url, init) => {
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) {
        const sql = JSON.parse(String(init?.body ?? '{}')).sql as string
        if (sql.includes('SELECT game_id')) {
          return response({ success: true, result: [{ results: [{ game_id: 'room-1' }] }] })
        }
        if (sql.includes('saved_count')) {
          return response({ success: true, result: [{ results: [{ saved_count: 0, lifecycle_count: 0 }] }] })
        }
        return response({ success: true, result: [{ results: [{ count: 0 }] }] })
      }
      if (url.includes('/objects?')) {
        if (url.includes('game-room-namespace')) {
          return response({ success: true, result: [{ id: 'room-object', hasStoredData: true }], result_info: {} })
        }
        return response({ success: true, result: [], result_info: {} })
      }
      if (url.endsWith('/room')) return response({ status: 'alreadyPurged' })
      if (url.endsWith('/room-index')) return response({ roomIndexUpdated: 0 })
      if (url.endsWith('/replay-index')) return response({ playerSavedReplayDeleted: 0, replayArchiveLifecycleDeleted: 0 })
      if (url.endsWith('/room-verify')) return response({ status: 'Waiting', hasGameRecord: false })
      throw new Error(`unexpected test request ${url}`)
    }

    await expect(runLegacyPurge(config, fetcher)).resolves.toMatchObject({
      mutated: true,
      mutationCount: 0,
    })
  })

  test('targets only legacy replay tables, preserves room identities, and verifies the cutover', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75', '--confirm']),
      environment,
    )
    const sql: string[] = []
    const managementCalls: Array<{ path: string, body: Record<string, unknown> }> = []
    const fetcher: Fetcher = async (url, init) => {
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) {
        const statement = JSON.parse(String(init?.body ?? '{}')).sql as string
        sql.push(statement)
        if (statement.includes('SELECT game_id')) {
          return response({ success: true, result: [{ results: [{ game_id: 'room-1' }] }] })
        }
        if (statement.includes('SELECT replay_id')) {
          return response({ success: true, result: [{ results: [{ replay_id: 'replay-1' }] }] })
        }
        if (statement.includes('saved_count')) {
          return response({ success: true, result: [{ results: [{ saved_count: 0, lifecycle_count: 0 }] }] })
        }
        return response({ success: true, result: [{ results: [{ count: 0 }] }] })
      }
      if (url.includes('/objects?')) {
        if (url.includes('game-room-namespace')) {
          return response({ success: true, result: [{ id: 'room-object', hasStoredData: true }], result_info: {} })
        }
        return response({ success: true, result: [{ id: 'replay-object', hasStoredData: true }], result_info: {} })
      }
      const path = new URL(url).pathname.split('/').at(-1) as string
      const body = JSON.parse(String(init?.body ?? '{}')) as Record<string, unknown>
      managementCalls.push({ path, body })
      if (path === 'room') return response({ status: 'purged' })
      if (path === 'room-index') return response({ roomIndexUpdated: 1 })
      if (path === 'replay') return response({ purged: true })
      if (path === 'replay-index') return response({ playerSavedReplayDeleted: 1, replayArchiveLifecycleDeleted: 1 })
      if (path === 'room-verify') return response({ status: 'Waiting', hasGameRecord: false })
      if (path === 'replay-verify') return response({ keyCount: 0 })
      if (path === 'replay-sample') return response({ status: 404 })
      throw new Error(`unexpected management endpoint ${path}`)
    }

    await expect(runLegacyPurge(config, fetcher)).resolves.toMatchObject({
      mutationCount: 5,
      inventory: { roomIds: ['room-1'], replayIds: ['replay-1'] },
    })
    expect(sql.filter(statement => statement.startsWith('DELETE'))).toEqual([])
    expect(managementCalls.find(call => call.path === 'room-index')?.body).toEqual({ epoch: 'cutover-75' })
    expect(managementCalls.find(call => call.path === 'replay-index')?.body).toEqual({ epoch: 'cutover-75' })
    expect(managementCalls.find(call => call.path === 'room')?.body).toEqual({
      epoch: 'cutover-75', objectId: 'room-object',
    })
    expect(managementCalls.find(call => call.path === 'replay')?.body).toEqual({
      epoch: 'cutover-75', objectId: 'replay-object',
    })
  })

  test('does not attempt to reopen traffic after a partial cleanup failure', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75', '--confirm']),
      environment,
    )
    const managementPaths: string[] = []
    const fetcher: Fetcher = async (url, init) => {
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) {
        const sql = JSON.parse(String(init?.body ?? '{}')).sql as string
        if (sql.includes('SELECT game_id')) return response({ success: true, result: [{ results: [] }] })
        if (sql.includes('SELECT replay_id')) return response({ success: true, result: [{ results: [] }] })
        return response({ success: true, result: [{ results: [{ count: 0 }] }] })
      }
      if (url.includes('/objects?')) {
        return response({
          success: true,
          result: url.includes('game-room-namespace')
            ? [{ id: 'room-object', hasStoredData: true }]
            : [],
          result_info: {},
        })
      }
      const path = new URL(url).pathname.split('/').at(-1) as string
      managementPaths.push(path)
      if (path === 'room') return response({ status: 'purged' })
      if (path === 'room-index') return response({ error: 'storage failure' }, 500)
      throw new Error(`unexpected management endpoint ${path}`)
    }

    await expect(runLegacyPurge(config, fetcher)).rejects.toThrow('room-index')
    expect(managementPaths).toEqual(['room', 'room-index'])
    expect(managementPaths).not.toContain('reopen-traffic')
  })
})
