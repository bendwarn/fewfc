import { mkdirSync, rmSync } from 'node:fs'
import { relative, resolve } from 'node:path'
import { randomUUID } from 'node:crypto'

const appRoot = resolve(import.meta.dir, '..')
const storageDirectory = resolve(appRoot, '.wrangler', `e2e-ticket22-${randomUUID()}`)
const serverStateDirectory = resolve(appRoot, '.wrangler', `t22sv-${randomUUID().slice(0, 8)}`)
const preserveStorageOnFailure = process.env.FEWC_E2E_PRESERVE_FAILURE === '1'
mkdirSync(resolve(appRoot, '.wrangler'), { recursive: true })

let exitCode: number | undefined
try {
  const processHandle = Bun.spawn([
    process.execPath,
    'playwright',
    'test',
    'tests/multisession/tournament-first-event.spec.ts',
  ], {
    cwd: appRoot,
    env: {
      ...process.env,
      FEWFC_TICKET22_MULTISESSION: '1',
      FEWFC_TICKET22_ISOLATED_INITIAL_HP: '20',
      FEWFC_E2E_STORAGE_DIR: relative(appRoot, storageDirectory),
      FEWFC_E2E_SERVER_STATE_DIR: relative(appRoot, serverStateDirectory),
    },
    stdout: 'inherit',
    stderr: 'inherit',
  })
  exitCode = await processHandle.exited
  process.exitCode = exitCode
} finally {
  rmSync(serverStateDirectory, { recursive: true, force: true })
  if (preserveStorageOnFailure && exitCode !== 0) {
    process.stderr.write(`Ticket 22 E2E storage retained for diagnosis: ${storageDirectory}\n`)
  } else {
    rmSync(storageDirectory, { recursive: true, force: true })
  }
}
