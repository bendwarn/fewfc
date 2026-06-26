async function repoRoot(): Promise<string> {
  const { existsSync } = await import('node:fs')
  const { resolve } = await import('node:path')
  const configured = process.env.FEWFC_ROOT

  if (configured) {
    return configured
  }

  const fromApp = resolve(process.cwd(), '../..')

  if (existsSync(resolve(fromApp, 'Cargo.toml'))) {
    return fromApp
  }

  return process.cwd()
}

async function runBridge(body: unknown): Promise<unknown> {
  const { spawn } = await import('node:child_process')
  const cwd = await repoRoot()

  return new Promise((resolveBridge, rejectBridge) => {
    const child = spawn('cargo', ['run', '--quiet', '--bin', 'fewfc_local_game_api'], {
      cwd,
      stdio: ['pipe', 'pipe', 'pipe'],
    })
    const timeout = setTimeout(() => {
      child.kill()
      rejectBridge(createError({ statusCode: 504, statusMessage: 'Rules engine timed out' }))
    }, 30000)

    let stdout = ''
    let stderr = ''

    child.stdout.setEncoding('utf8')
    child.stderr.setEncoding('utf8')
    child.stdout.on('data', (chunk) => {
      stdout += chunk
    })
    child.stderr.on('data', (chunk) => {
      stderr += chunk
    })
    child.on('error', (error) => {
      clearTimeout(timeout)
      rejectBridge(createError({ statusCode: 500, statusMessage: error.message }))
    })
    child.on('close', (code) => {
      clearTimeout(timeout)

      if (code !== 0) {
        rejectBridge(
          createError({
            statusCode: 422,
            statusMessage: stderr || `Rules engine exited with code ${code}`,
          }),
        )
        return
      }

      try {
        resolveBridge(JSON.parse(stdout))
      } catch (error) {
        rejectBridge(
          createError({
            statusCode: 500,
            statusMessage: error instanceof Error ? error.message : 'Invalid rules engine output',
          }),
        )
      }
    })

    child.stdin.end(JSON.stringify(body))
  })
}

export default defineEventHandler(async (event) => {
  if (
    (
      event.context as {
        _platform?: {
          cloudflare?: unknown
        }
      }
    )._platform?.cloudflare
  ) {
    throw createError({
      statusCode: 501,
      statusMessage: 'The local Cargo bridge is not available on Cloudflare. Use the browser WASM client instead.',
    })
  }

  const body = await readBody(event)

  return await runBridge(body)
})
