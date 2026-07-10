import { rmSync } from 'node:fs'
import { resolve, sep } from 'node:path'

const storage = resolve('.wrangler/e2e')
const expectedSuffix = `${sep}.wrangler${sep}e2e`

if (!storage.endsWith(expectedSuffix)) {
  throw new Error(`Refusing to remove unexpected E2E storage path: ${storage}`)
}

rmSync(storage, { recursive: true, force: true })
