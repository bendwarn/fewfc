import { spawn } from 'node:child_process'
import { createConnection, createServer } from 'node:net'
import { mkdir, unlink } from 'node:fs/promises'
import { resolve } from 'node:path'

export const serverNames = ['dev', 'e2e', 'component'] as const
export type ServerName = typeof serverNames[number]
export const stateDirectory = resolve(import.meta.dir, '../.wrangler/servers')
const delay = (ms: number) => new Promise(resolve => setTimeout(resolve, ms))

export async function stopServer(name: ServerName, directory = stateDirectory): Promise<boolean> {
  return new Promise((resolve, reject) => {
    const socket = createConnection(`${directory}/${name}.sock`)
    socket.setTimeout(10_000, () => socket.destroy(new Error(`Timed out stopping ${name}`)))
    let response = ''
    socket.on('connect', () => socket.write('stop\n'))
    socket.on('data', data => { response += data })
    socket.on('end', () => response === 'stopped\n' ? resolve(true) : reject(new Error(`Unexpected response from ${name}`)))
    socket.on('error', (error: NodeJS.ErrnoException) => {
      if (error.code === 'ENOENT' || error.code === 'ECONNREFUSED') resolve(false)
      else reject(error)
    })
  })
}

export async function withManagedServer(
  name: ServerName,
  task: (run: (args: string[]) => Promise<void>) => Promise<void>,
  directory = stateDirectory,
): Promise<void> {
  await mkdir(directory, { recursive: true, mode: 0o700 })
  const socketPath = `${directory}/${name}.sock`
  let stopping = false
  let group: number | undefined
  let stopPromise: Promise<void> | undefined
  async function stopGroup() {
    if (!group) return
    const pid = group
    const signal = (value: NodeJS.Signals | 0) => {
      try { process.kill(-pid, value); return true } catch (error) {
        if ((error as NodeJS.ErrnoException).code === 'ESRCH') return false
        throw error
      }
    }
    signal('SIGTERM')
    for (let attempt = 0; attempt < 50 && signal(0); attempt++) await delay(100)
    if (signal(0)) {
      signal('SIGKILL')
      for (let attempt = 0; attempt < 50 && signal(0); attempt++) await delay(20)
      if (signal(0)) throw new Error('Server process group did not exit')
    }
    group = undefined
  }
  function stop() {
    stopping = true
    return stopPromise ??= stopGroup()
  }
  const controller = createServer(socket => {
    socket.setTimeout(1000, () => socket.destroy())
    let request = ''
    socket.on('error', () => {})
    socket.on('data', data => {
      request += data
      if (request === 'stop\n') {
        socket.setTimeout(0)
        void stop().then(() => socket.end('stopped\n'), () => socket.destroy())
      } else if (request.length > 5) socket.destroy()
    })
  })
  // socket 的獨占綁定避免同一 worktree 重複啟動；殘留 socket 明確報錯，避免搶走現有服務。
  await new Promise<void>((resolve, reject) => {
    controller.once('error', reject)
    controller.listen(socketPath, resolve)
  })
  const onSignal = () => { void stop() }
  process.on('SIGINT', onSignal)
  process.on('SIGTERM', onSignal)
  try {
    await task(async args => {
      if (stopping) throw new Error('Server stopped')
      // 每次啟動獨立程序群組，關閉時包含 Wrangler、workerd 與建置子程序。
      const child = spawn(args[0]!, args.slice(1), { stdio: 'inherit', detached: true })
      group = child.pid
      const code = await new Promise<number | null>((resolve, reject) => {
        child.once('error', reject)
        child.once('exit', resolve)
      })
      if (stopPromise) await stopPromise
      else await stopGroup()
      if (stopping) throw new Error('Server stopped')
      if (code !== 0) throw new Error(`Command exited with code ${code}: ${args[0]}`)
    })
  } catch (error) {
    if (!stopping) throw error
  } finally {
    await stop()
    await new Promise<void>((resolve, reject) => controller.close(error => error ? reject(error) : resolve()))
    await unlink(socketPath).catch(error => { if (error.code !== 'ENOENT') throw error })
    process.off('SIGINT', onSignal)
    process.off('SIGTERM', onSignal)
  }
}
