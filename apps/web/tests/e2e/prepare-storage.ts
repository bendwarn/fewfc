import { $ } from 'bun'

const storageUrl = new URL('../../.wrangler/e2e/', import.meta.url)
const expectedSuffix = '/apps/web/.wrangler/e2e/'

if (storageUrl.protocol !== 'file:' || !storageUrl.pathname.endsWith(expectedSuffix)) {
  throw new Error(`Refusing to remove unexpected E2E storage path: ${storageUrl.href}`)
}

await $`rm -rf ${decodeURIComponent(storageUrl.pathname)}`
