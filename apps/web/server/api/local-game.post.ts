import { spawn } from 'node:child_process'
import { existsSync } from 'node:fs'
import { resolve } from 'node:path'

function repoRoot(): string {
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

function runBridge(body: unknown): Promise<unknown> {
  return new Promise((resolveBridge, rejectBridge) => {
    const child = spawn('cargo', ['run', '--quiet', '--bin', 'fewfc_local_game_api'], {
      cwd: repoRoot(),
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
  const body = await readBody(event)

  return runBridge(body)
})
