import { existsSync } from 'node:fs'
import { mkdir, readFile, writeFile, rm, realpath, rename } from 'node:fs/promises'
import { resolve } from 'node:path'
import { createServer } from 'node:net'

const root = resolve(import.meta.dir, '..')
const web = resolve(root, 'apps/web')
const cloud = process.argv.includes('--cloud')
const keys = ['FEWFC_DEV_PORT', 'FEWFC_DEV_INSPECTOR_PORT', 'FEWFC_E2E_PORT', 'FEWFC_E2E_INSPECTOR_PORT', 'FEWFC_COMPONENT_PORT']
const defaults = [8787, 9229, 8727, 9230, 8730]
async function available(port: number) {
  return new Promise<boolean>(resolve => {
    const server = createServer()
    server.once('error', () => resolve(false))
    server.listen(port, () => server.close(() => resolve(true)))
  })
}
async function configure() {
  const envPath = resolve(web, '.env')
  let env: string
  let selected = defaults
  let lock: string | undefined
  let ownsLock = false
  try {
    if (!cloud) {
      const git = Bun.spawnSync(['git', 'rev-parse', '--git-common-dir'], { cwd: root })
      if (git.exitCode !== 0) throw new Error('Cannot locate Git common directory')
      const common = resolve(root, git.stdout.toString().trim())
      lock = resolve(common, 'fewfc-ports.lock')
      // 原子建立鎖目錄；遇到異常留下的鎖時明確失敗，不搶走其他初始化程序的鎖。
      let acquired = false
      for (let attempt = 0; attempt < 100; attempt++) {
        try { await mkdir(lock); acquired = true; ownsLock = true; break } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== 'EEXIST') throw error
          await Bun.sleep(100)
        }
      }
      if (!acquired) { lock = undefined; throw new Error('Port allocation lock busy; retry or inspect fewfc-ports.lock in Git common directory') }
      const registryPath = resolve(common, 'fewfc-ports.json')
      const registry: Record<string, number[]> = existsSync(registryPath) ? JSON.parse(await readFile(registryPath, 'utf8')) : {}
      const identity = await realpath(root)
      for (const path of Object.keys(registry)) if (!existsSync(path)) delete registry[path]
      if (registry[identity]) selected = registry[identity]!
      else {
        const used = new Set(Object.values(registry).flat())
        let found = false
        for (let base = 18000; base < 60000; base += 5) {
          const candidate = keys.map((_, index) => base + index)
          if (candidate.some(port => used.has(port))) continue
          if (!(await Promise.all(candidate.map(available))).every(Boolean)) continue
          selected = candidate; found = true; break
        }
        if (!found) throw new Error('No free port block found')
        registry[identity] = selected
        await writeFile(registryPath + '.tmp', JSON.stringify(registry, null, 2) + '\n')
        await rename(registryPath + '.tmp', registryPath)
      }
    }
    env = existsSync(envPath) ? await readFile(envPath, 'utf8') : await readFile(resolve(web, '.env.example'), 'utf8')
    env = env.replace(/\n?# BEGIN FEWFC PORTS[\s\S]*?# END FEWFC PORTS\n?/g, '')
    for (const key of [...keys, 'BETTER_AUTH_URL']) {
      env = env.replace(new RegExp(`^${key}=.*\\r?\\n?`, 'gm'), '')
    }
    const brave = '/Applications/Brave Browser.app/Contents/MacOS/Brave Browser'
    if (!cloud && existsSync(brave) && !/^PLAYWRIGHT_BROWSER_PATH=/m.test(env)) env += `\nPLAYWRIGHT_BROWSER_PATH="${brave}"\n`
    env = env.trimEnd() + '\n\n# BEGIN FEWFC PORTS\n' + keys.map((key, index) => `${key}=${selected[index]}`).join('\n') + `\nBETTER_AUTH_URL=http://localhost:${selected[0]}\n# END FEWFC PORTS\n`
    env = env.replace('replace-with-at-least-32-random-characters', crypto.randomUUID() + crypto.randomUUID())
    await writeFile(envPath, env, { mode: 0o600 })
    console.log(`Configured ${cloud ? 'cloud' : 'local'} ports: ${selected.join(', ')}`)
  } finally { if (ownsLock && lock) await rm(lock, { recursive: true }) }
}
await configure()
if (!process.argv.includes('--configure-only')) {
  for (const command of [
    ['pnpm', 'install', '--frozen-lockfile'],
    ['rustup', 'target', 'add', 'wasm32-unknown-unknown'],
    ['bun', 'run', 'build'],
    ['bun', 'run', 'db:migrate:local'],
  ]) {
    const child = Bun.spawn(['sh', '-c', 'exec "$@"', 'fewfc-setup', ...command], { cwd: web, stdin: 'inherit', stdout: 'inherit', stderr: 'inherit', env: { ...process.env, APP_ENV: 'development' } })
    if (await child.exited !== 0) throw new Error(`Setup failed: ${command.join(' ')}`)
  }
}
