import { createServer } from 'node:net'
import { resolve } from 'node:path'
import { ports } from './local-ports'
import { withManagedServer } from './managed-server'

const component = process.argv.includes('--component')
const e2e = process.argv.includes('--e2e')
const httpPort = component ? ports.component : e2e ? ports.e2e : ports.dev
const inspectorPort = e2e ? ports.e2eInspector : ports.devInspector
const e2eStorageDirectory = process.env.FEWFC_E2E_STORAGE_DIR ?? '.wrangler/e2e'
const managedServerStateDirectory = e2e && process.env.FEWFC_E2E_SERVER_STATE_DIR
  ? resolve(import.meta.dir, '..', process.env.FEWFC_E2E_SERVER_STATE_DIR)
  : undefined
await withManagedServer(component ? 'component' : e2e ? 'e2e' : 'dev', async run => {
  // 在清除測試資料前確認服務與除錯 port，避免影響已啟動的服務。
  for (const port of component ? [httpPort] : [httpPort, inspectorPort]) {
    await new Promise<void>((resolve, reject) => {
      const server = createServer()
      server.once('error', reject)
      server.listen(port, () => server.close(error => error ? reject(error) : resolve()))
    })
  }
  if (component) {
    process.env.APP_ENV = 'development'
    await run(['bun', 'x', '--no-install', 'nuxi', 'dev', '--host', '127.0.0.1', '--port', String(httpPort)])
    return
  }
  if (e2e) {
    await run(['bun', 'tests/e2e/prepare-storage.ts'])
    if (!process.argv.includes('--built')) await run(['bun', 'run', 'build'])
    await run(['bun', 'run', 'db:migrate:local', '--persist-to', e2eStorageDirectory])
  }
  await run(['bun', 'x', '--no-install', 'wrangler', 'dev',
    '--port', String(httpPort), '--inspector-port', String(inspectorPort),
    '--var', `BETTER_AUTH_URL:http://localhost:${httpPort}`,
    ...(e2e && process.env.FEWFC_TICKET22_ISOLATED_INITIAL_HP === '20'
      ? ['--var', 'TICKET22_ISOLATED_INITIAL_HP:20']
      : []),
    ...(e2e ? ['--persist-to', e2eStorageDirectory] : []), ...process.argv.slice(2).filter(arg => !['--e2e', '--built'].includes(arg)),
  ])

}, managedServerStateDirectory)
