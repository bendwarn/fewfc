import { expect, test } from 'bun:test'
import { lstat, mkdir, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises'
import { createServer } from 'node:net'
import { stopServer, withManagedServer } from './managed-server'
import { join } from 'node:path'

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

test('replaces a stale control socket left by a terminated server', async () => {
  const temp = await mkdtemp('/tmp/fewfc-stale-socket-')
  const directory = join(temp, 'state')
  const socketPath = join(directory, 'dev.sock')
  const readyPath = join(temp, 'ready')
  const orphanScript = join(temp, 'orphan.ts')
  await mkdir(directory)
  await writeFile(orphanScript, `import { createServer } from 'node:net'; const server = createServer(); server.listen(${JSON.stringify(socketPath)}, () => { void Bun.write(${JSON.stringify(readyPath)}, 'ready') }); setInterval(() => {}, 1000)`)
  const orphan = Bun.spawn([process.execPath, orphanScript], { stdout: 'ignore', stderr: 'pipe' })

  try {
    await waitForFile(readyPath)
    orphan.kill('SIGKILL')
    await orphan.exited
    expect((await lstat(socketPath)).isSocket()).toBe(true)

    await withManagedServer('dev', run => run([process.execPath, '-e', 'process.exit(0)']), directory)

    await expect(lstat(socketPath)).rejects.toMatchObject({ code: 'ENOENT' })
  } finally {
    if (orphan.exitCode === null) orphan.kill('SIGKILL')
    await stopServer('dev', directory).catch(() => {})
    await rm(temp, { recursive: true, force: true })
  }
}, 30000)

test('preserves an active control socket owned by another server', async () => {
  const temp = await mkdtemp('/tmp/fewfc-active-socket-')
  const socketPath = join(temp, 'dev.sock')
  const owner = createServer()
  await new Promise<void>((resolve, reject) => {
    owner.once('error', reject)
    owner.listen(socketPath, resolve)
  })

  try {
    let taskStarted = false
    await expect(withManagedServer('dev', async () => { taskStarted = true }, temp))
      .rejects.toMatchObject({ code: 'EADDRINUSE' })
    expect(taskStarted).toBe(false)
    expect((await lstat(socketPath)).isSocket()).toBe(true)
  } finally {
    await new Promise<void>(resolve => owner.close(() => resolve()))
    await rm(temp, { recursive: true, force: true })
  }
}, 30000)
