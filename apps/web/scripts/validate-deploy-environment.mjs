import { readFile } from 'node:fs/promises'

const environment = process.argv[2]

if (environment !== 'staging' && environment !== 'production') {
  throw new Error('Expected staging or production deployment environment.')
}

const wrangler = await readFile(new URL('../wrangler.toml', import.meta.url), 'utf8')
const placeholders = environment === 'staging'
  ? ['https://staging.example.com', 'REPLACE_WITH_STAGING_D1_DATABASE_ID']
  : ['https://example.com', 'REPLACE_WITH_PRODUCTION_D1_DATABASE_ID']

const unresolved = placeholders.filter((placeholder) => wrangler.includes(placeholder))

if (unresolved.length > 0) {
  throw new Error(
    `${environment} deployment has unresolved configuration: ${unresolved.join(', ')}`,
  )
}
