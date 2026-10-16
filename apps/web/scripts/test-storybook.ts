const port = 6006
const origin = `http://127.0.0.1:${port}`
const server = Bun.spawn(
  ['pnpm', 'exec', 'storybook', 'dev', '--ci', '--no-open', '--exact-port', '--port', String(port)],
  {
    cwd: import.meta.dir + '/..', stdout: 'inherit', stderr: 'inherit',
    env: { ...process.env, STORYBOOK: 'true' },
  },
)

let ready = false
try {
  for (let attempt = 0; attempt < 180; attempt += 1) {
    if (server.exitCode !== null) throw new Error(`Storybook server exited with code ${server.exitCode}`)
    try {
      const response = await fetch(`${origin}/index.json`)
      if (response.ok) {
        ready = true
        break
      }
    }
    catch {
      // Storybook 尚未監聽時等待下一輪。
    }
    await Bun.sleep(500)
  }

  if (!ready) throw new Error(`Storybook did not become ready at ${origin}`)

  const test = Bun.spawn(
    ['pnpm', 'exec', 'test-storybook', '--url', origin, '--maxWorkers=1', '--ci'],
    { cwd: import.meta.dir + '/..', stdout: 'inherit', stderr: 'inherit' },
  )
  const exitCode = await test.exited
  if (exitCode !== 0) process.exitCode = exitCode
}
finally {
  server.kill()
  await server.exited
}
