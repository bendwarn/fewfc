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

  test('allows confirmed operation with Wrangler token authentication only', async () => {
    const config = await configFromEnvironment(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75', '--confirm']),
      environment,
      async () => wranglerToken,
      async accountId => accountId ?? 'account',
      async () => wranglerToml,
    )

    expect(config.authorizationToken).toBe(wranglerToken)
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

  test('defaults to dry-run and uses the Wrangler token without requiring a management secret', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75']),
      environment,
    )
    const calls: Array<{ url: string, init?: RequestInit }> = []
    const fetcher: Fetcher = async (url, init) => {
      calls.push({ url, init })
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) return response({ success: true, result: [{ results: [{ game_id: 'room-1' }] }] })
      if (url.endsWith('/room-probe')) return response({ status: 'preserved', gameId: 'room-1' })
      return response({ success: true, result: [] })
    }

    const result = await runLegacyPurge(config, fetcher)

    expect(result).toMatchObject({ mutated: false, mutationCount: 0 })
    const probe = calls.find(call => call.url.endsWith('/room-probe'))
    expect(probe?.init?.headers).toMatchObject({ 'x-fewfc-cloudflare-token': wranglerToken })
  })

  test('distinguishes a management-gate 404 from an absent room', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75']),
      environment,
    )
    const fetcher: Fetcher = async (url) => {
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) return response({ success: true, result: [{ results: [{ game_id: 'room-1' }] }] })
      if (url.endsWith('/room-probe')) return response({ error: 'not found' }, 404)
      return response({ success: true, result: [] })
    }

    await expect(runLegacyPurge(config, fetcher)).rejects.toThrow(
      'was rejected by the management gate; verify maintenance mode, Wrangler token, and Cloudflare account ID',
    )
  })

  test('dry-run probes indexed rooms and GameRoom objects when management auth is available', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75']),
      environment,
    )
    const probeCalls: Array<Record<string, unknown>> = []
    const fetcher: Fetcher = async (url, init) => {
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) {
        const sql = JSON.parse(String(init?.body ?? '{}')).sql as string
        if (sql.includes('SELECT game_id')) {
          return response({ success: true, result: [{ results: [{ game_id: 'missing-index' }] }] })
        }
        if (sql.includes('SELECT replay_id')) return response({ success: true, result: [{ results: [] }] })
        throw new Error(`unexpected dry-run SQL ${sql}`)
      }
      if (url.includes('/objects?')) {
        return response({
          success: true,
          result: url.includes('game-room-namespace')
            ? [{ id: 'broken-object', hasStoredData: true }]
            : [],
          result_info: {},
        })
      }
      if (url.endsWith('/room-probe')) {
        const body = JSON.parse(String(init?.body ?? '{}')) as Record<string, unknown>
        probeCalls.push(body)
        return body.objectId === 'broken-object'
          ? response({ status: 'broken', gameId: 'broken-room' }, 500)
          : response({ status: 'absent' }, 404)
      }
      throw new Error(`unexpected dry-run request ${url}`)
    }

    const result = await runLegacyPurge(config, fetcher)

    expect(result).toMatchObject({ mutated: false, mutationCount: 0 })
    expect(result.roomProbes).toEqual([
      { kind: 'game-room-object', id: 'broken-object', status: 'broken' },
      { kind: 'room-index', id: 'missing-index', status: 'absent' },
    ])
    expect(probeCalls).toEqual([
      { epoch: 'cutover-75', objectId: 'broken-object' },
      { epoch: 'cutover-75', roomId: 'missing-index' },
    ])
    expect(purgeRunSummary(result)).toMatchObject({ mode: 'dry-run', candidateRooms: 2 })
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
    expect(redactSecrets(
      `request failed for ${wranglerToken}`,
      [wranglerToken],
    )).toBe('request failed for [redacted]')
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
      environment,
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

  test('reports zero new mutation when a same-epoch rerun sees no stored room objects', async () => {
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
      if (url.endsWith('/room')) return response({ status: 'preserved', gameId: 'room-1' })
      if (url.endsWith('/room-index')) return response({ status: 'preserved', roomIndexDeleted: 0 })
      if (url.endsWith('/room-probe')) return response({ status: 'preserved', gameId: 'room-1' })
      if (url.endsWith('/replay-index')) return response({ playerSavedReplayDeleted: 0, replayArchiveLifecycleDeleted: 0 })
      if (url.endsWith('/room-verify')) return response({ status: 'Waiting', hasGameRecord: false })
      throw new Error(`unexpected test request ${url}`)
    }

    await expect(runLegacyPurge(config, fetcher)).resolves.toMatchObject({
      mutated: true,
      mutationCount: 0,
    })
  })

  test('deletes only rooms whose management response is absent or broken and preserves all others', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75', '--confirm']),
      environment,
    )
    const sql: string[] = []
    const managementCalls: Array<{ path: string, body: Record<string, unknown> }> = []
    let staleRoomPresent = true
    const fetcher: Fetcher = async (url, init) => {
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) {
        const statement = JSON.parse(String(init?.body ?? '{}')).sql as string
        sql.push(statement)
        if (statement.includes('SELECT game_id')) {
          return response({ success: true, result: [{ results: [
            { game_id: 'room-1' },
            ...(staleRoomPresent ? [{ game_id: 'stale-room' }] : []),
          ] }] })
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
          return response({ success: true, result: [
            { id: 'orphan-room-object', hasStoredData: true },
            { id: 'broken-room-object', hasStoredData: true },
            { id: 'live-room-object', hasStoredData: true },
          ], result_info: {} })
        }
        return response({ success: true, result: [{ id: 'replay-object', hasStoredData: true }], result_info: {} })
      }
      const path = new URL(url).pathname.split('/').at(-1) as string
      const body = JSON.parse(String(init?.body ?? '{}')) as Record<string, unknown>
      managementCalls.push({ path, body })
      if (path === 'room') {
        if (body.objectId === 'orphan-room-object') return response({ status: 'absent' }, 404)
        if (body.objectId === 'broken-room-object') return response({ status: 'broken', gameId: 'broken-room' }, 500)
        return response({ status: 'preserved', gameId: 'room-1' })
      }
      if (path === 'room-index') {
        if (body.roomId === 'stale-room') {
          staleRoomPresent = false
          return response({ status: 'absent', roomIndexDeleted: 1 })
        }
        return response({ status: 'preserved', roomIndexDeleted: 0 })
      }
      if (path === 'replay') return response({ purged: true })
      if (path === 'replay-index') return response({ playerSavedReplayDeleted: 1, replayArchiveLifecycleDeleted: 1 })
      if (path === 'room-probe') {
        if (body.objectId === 'orphan-room-object' || body.objectId === 'broken-room-object') {
          return response({ status: 'Absent' }, 404)
        }
        return response({ status: 'preserved', gameId: 'room-1' })
      }
      if (path === 'replay-verify') return response({ keyCount: 0 })
      if (path === 'replay-sample') return response({ status: 404 })
      throw new Error(`unexpected management endpoint ${path}`)
    }

    await expect(runLegacyPurge(config, fetcher)).resolves.toMatchObject({
      mutationCount: 6,
      inventory: { roomIds: ['room-1', 'stale-room'], replayIds: ['replay-1'] },
    })
    expect(sql.filter(statement => statement.startsWith('DELETE'))).toEqual([])
    expect(managementCalls.find(call => call.path === 'replay-index')?.body).toEqual({ epoch: 'cutover-75' })
    expect(managementCalls.filter(call => call.path === 'room-index').map(call => call.body)).toEqual([
      { epoch: 'cutover-75', roomId: 'room-1' },
      { epoch: 'cutover-75', roomId: 'stale-room' },
    ])
    expect(managementCalls.filter(call => call.path === 'room').map(call => call.body)).toEqual([
      { epoch: 'cutover-75', objectId: 'orphan-room-object' },
      { epoch: 'cutover-75', objectId: 'broken-room-object' },
      { epoch: 'cutover-75', objectId: 'live-room-object' },
    ])
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
      if (path === 'room') return response({ status: 'preserved' })
      if (path === 'replay-index') return response({ error: 'storage failure' }, 500)
      throw new Error(`unexpected management endpoint ${path}`)
    }

    await expect(runLegacyPurge(config, fetcher)).rejects.toThrow('replay-index')
    expect(managementPaths).toEqual(['room', 'replay-index'])
    expect(managementPaths).not.toContain('reopen-traffic')
  })

  test('fails closed when a preserved room becomes broken during verification', async () => {
    const config = await configuration(
      parsePurgeArguments(['--env', 'staging', '--epoch', 'cutover-75', '--confirm']),
      environment,
    )
    const managementPaths: string[] = []
    const fetcher: Fetcher = async (url, init) => {
      if (url.includes('/durable_objects/namespaces?')) return namespaceResponse()
      if (url.includes('/d1/database/')) {
        const sql = JSON.parse(String(init?.body ?? '{}')).sql as string
        if (sql.includes('SELECT game_id') || sql.includes('SELECT replay_id')) {
          return response({ success: true, result: [{ results: [] }] })
        }
        return response({ success: true, result: [{ results: [{ saved_count: 0, lifecycle_count: 0 }] }] })
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
      if (path === 'room') return response({ status: 'preserved', gameId: 'room-1' })
      if (path === 'room-probe') return response({ status: 'broken', gameId: 'room-1' }, 500)
      if (path === 'replay-index') return response({ playerSavedReplayDeleted: 0, replayArchiveLifecycleDeleted: 0 })
      throw new Error(`unexpected management endpoint ${path}`)
    }

    await expect(runLegacyPurge(config, fetcher)).rejects.toThrow('room-probe failed with HTTP 500')
    expect(managementPaths).toEqual(['room', 'replay-index', 'room-probe'])
  })
})
