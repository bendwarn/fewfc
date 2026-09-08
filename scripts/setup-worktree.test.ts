import { test, expect } from 'bun:test'
import { mkdtemp, mkdir, copyFile, readFile, rm, writeFile } from 'node:fs/promises'
import { tmpdir } from 'node:os'
import { join } from 'node:path'

test('concurrent worktrees receive distinct ports and preserve configuration on rerun', async () => {
  const temp = await mkdtemp(join(tmpdir(), 'fewfc-setup-'))
  const main = join(temp, 'main')
  const other = join(temp, 'other')
  function git(args: string[]) {
    const result = Bun.spawnSync(['git', ...args], { cwd: main })
    if (result.exitCode !== 0) throw new Error(result.stderr.toString())
  }
  async function setup(root: string, cloud = false) {
    const child = Bun.spawn(['bun', 'scripts/setup-worktree.ts', '--configure-only', ...(cloud ? ['--cloud'] : [])], { cwd: root, stdout: 'pipe', stderr: 'pipe' })
    const error = await new Response(child.stderr).text()
    expect(await child.exited, error).toBe(0)
    return readFile(join(root, 'apps/web/.env'), 'utf8')
  }
  try {
    await mkdir(main)
    git(['init'])
    git(['-c', 'user.name=Test', '-c', 'user.email=test@example.invalid', 'commit', '--allow-empty', '-m', 'fixture'])
    git(['worktree', 'add', '--detach', other])
    for (const root of [main, other]) {
      await mkdir(join(root, 'scripts'))
      await mkdir(join(root, 'apps/web'), { recursive: true })
      await copyFile(join(import.meta.dir, 'setup-worktree.ts'), join(root, 'scripts/setup-worktree.ts'))
      await writeFile(join(root, 'apps/web/.env.example'), 'BETTER_AUTH_SECRET=replace-with-at-least-32-random-characters\nCUSTOM=preserve\n')
    }
    const [first, second] = await Promise.all([setup(main), setup(other)])
    const ports = (env: string) => [...env.matchAll(/^FEWFC_\w+_PORT=(\d+)$/gm)].map(match => Number(match[1]))
    expect(new Set([...ports(first), ...ports(second)]).size).toBe(10)
    expect(first).toContain('CUSTOM=preserve')
    expect(first).not.toContain('replace-with-at-least')
    expect(await setup(main)).toBe(first)
    const registry = await readFile(join(main, '.git/fewfc-ports.json'), 'utf8')
    const cloud = await setup(other, true)
    expect(ports(cloud)).toEqual([8787, 9229, 8727, 9230, 8730])
    expect(await readFile(join(main, '.git/fewfc-ports.json'), 'utf8')).toBe(registry)
  } finally {
    await rm(temp, { recursive: true, force: true })
  }
}, 30000)

test('occupied E2E port fails before touching test storage', async () => {
  const { createServer } = await import('node:net')
  const server = createServer()
  await new Promise<void>(resolve => server.listen(0, resolve))
  const address = server.address()
  if (!address || typeof address === 'string') throw new Error('No TCP address')
  const temp = await mkdtemp(join(tmpdir(), 'fewfc-server-'))
  try {
    await mkdir(join(temp, '.wrangler/e2e'), { recursive: true })
    const marker = join(temp, '.wrangler/e2e/keep')
    await writeFile(marker, 'keep')
    const child = Bun.spawn(['bun', join(import.meta.dir, '../apps/web/scripts/local-server.ts'), '--e2e', '--built'], {
      cwd: temp, stdout: 'pipe', stderr: 'pipe',
      env: { ...process.env, FEWFC_E2E_PORT: String(address.port) },
    })
    const error = await new Response(child.stderr).text()
    expect(await child.exited).not.toBe(0)
    expect(error).toContain('EADDRINUSE')
    expect(await readFile(marker, 'utf8')).toBe('keep')
  } finally {
    await new Promise<void>(resolve => server.close(() => resolve()))
    await rm(temp, { recursive: true, force: true })
  }
})
