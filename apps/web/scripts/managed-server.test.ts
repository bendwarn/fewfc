import { expect, test } from 'bun:test'
import { mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { join } from 'node:path'
import { stopServer } from './managed-server'

async function waitForFile(path: string) {
  for (let attempt = 0; attempt < 100; attempt++) {
    try { return await readFile(path, 'utf8') } catch { await Bun.sleep(50) }
  }
  throw new Error(`Timed out waiting for ${path}`)
}
function alive(pid: number) {
  try { process.kill(pid, 0); return true } catch { return false }
}

test('stop shuts down owned descendants and leaves another worktree running', async () => {
  const temp = await mkdtemp('/tmp/fewfc-stop-')
  const processes: ReturnType<typeof Bun.spawn>[] = []
  try {
    for (const name of ['first', 'second']) {
      const grandchild = join(temp, `${name}-grandchild.ts`)
      const child = join(temp, `${name}-child.ts`)
      const runner = join(temp, `${name}-runner.ts`)
      await writeFile(grandchild, `await Bun.write(${JSON.stringify(join(temp, `${name}.pid`))}, String(process.pid)); ${name === 'first' ? "process.on('SIGTERM', () => {});" : ''} setInterval(() => {}, 1000)`)
      await writeFile(child, `Bun.spawn([process.execPath, ${JSON.stringify(grandchild)}]); setInterval(() => {}, 1000)`)
      await writeFile(runner, `import { withManagedServer } from ${JSON.stringify(join(import.meta.dir, 'managed-server.ts'))}; await withManagedServer('dev', run => run([process.execPath, ${JSON.stringify(child)}]), ${JSON.stringify(join(temp, name))})`)
      processes.push(Bun.spawn([process.execPath, runner], { stdout: 'pipe', stderr: 'pipe' }))
    }
    const firstPid = Number(await waitForFile(join(temp, 'first.pid')))
    const secondPid = Number(await waitForFile(join(temp, 'second.pid')))
    expect(await stopServer('dev', join(temp, 'first'))).toBe(true)
    expect(await processes[0]!.exited).toBe(0)
    expect(alive(firstPid)).toBe(false)
    expect(alive(secondPid)).toBe(true)
    expect(await stopServer('dev', join(temp, 'first'))).toBe(false)
    expect(await stopServer('dev', join(temp, 'second'))).toBe(true)
    expect(await processes[1]!.exited).toBe(0)
    expect(alive(secondPid)).toBe(false)
  } finally {
    for (const name of ['first', 'second']) await stopServer('dev', join(temp, name)).catch(() => {})
    for (const child of processes) if (child.exitCode === null) child.kill()
    await rm(temp, { recursive: true, force: true })
  }
}, 30000)
