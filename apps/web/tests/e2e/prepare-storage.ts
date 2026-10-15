import { $ } from 'bun'
import { fileURLToPath } from 'node:url'
import { isAbsolute, relative, resolve, sep } from 'node:path'

const appRoot = resolve(fileURLToPath(new URL('../..', import.meta.url)))
const storageDirectory = resolve(
  appRoot,
  process.env.FEWFC_E2E_STORAGE_DIR ?? '.wrangler/e2e',
)
const storageRelativePath = relative(appRoot, storageDirectory)
const allowedStoragePath = storageRelativePath === '.wrangler/e2e'
  || /^\.wrangler\/e2e-[a-z0-9-]+$/u.test(storageRelativePath)

if (isAbsolute(process.env.FEWFC_E2E_STORAGE_DIR ?? '') || !allowedStoragePath
  || storageRelativePath.split(sep).includes('..')) {
  throw new Error(`Refusing to remove unexpected E2E storage path: ${storageDirectory}`)
}

await $`rm -rf ${storageDirectory}`
