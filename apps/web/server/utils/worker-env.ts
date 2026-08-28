import { createError, type H3Event } from 'h3'
import { parseAppEnvironment } from './app-environment'

export interface DurableObjectNamespaceBinding {
  idFromName(name: string): unknown
  get(id: unknown): {
    fetch(input: RequestInfo | URL, init?: RequestInit): Promise<Response>
  }
}

export interface WorkerEnv {
  DB: unknown
  GAME_ROOM: DurableObjectNamespaceBinding
  PLAYER_NOTIFICATIONS: DurableObjectNamespaceBinding
  REPLAY: DurableObjectNamespaceBinding
  APP_ENV?: string
  BETTER_AUTH_SECRET?: string
  BETTER_AUTH_URL?: string
  GOOGLE_CLIENT_ID?: string
  GOOGLE_CLIENT_SECRET?: string
  GITHUB_CLIENT_ID?: string
  GITHUB_CLIENT_SECRET?: string
}

declare global {
  // 在模組 Worker 預設設定中，Nitro 會在此公開 Cloudflare bindings。
  // eslint-disable-next-line no-var
  var __env__: WorkerEnv | undefined
}

export function workerEnv(event: H3Event): WorkerEnv {
  const platformEnv = (
    event.context as {
      _platform?: {
        cloudflare?: {
          env?: WorkerEnv
        }
      }
    }
  )._platform?.cloudflare?.env
  const env = platformEnv ?? globalThis.__env__

  if (!env) {
    throw createError({
      statusCode: 501,
      statusMessage: 'Cloudflare bindings are unavailable. Run the app through Wrangler.',
    })
  }

  parseAppEnvironment(env.APP_ENV)

  return env
}
