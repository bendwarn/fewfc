import { createError, type H3Event } from 'h3'

export type AppEnvironment = 'development' | 'staging' | 'production'

const APP_ENVIRONMENTS = new Set<AppEnvironment>([
  'development',
  'staging',
  'production',
])

export function parseAppEnvironment(value: unknown): AppEnvironment {
  if (typeof value === 'string' && APP_ENVIRONMENTS.has(value as AppEnvironment)) {
    return value as AppEnvironment
  }

  throw createError({
    statusCode: 500,
    statusMessage: 'APP_ENV must be development, staging, or production.',
  })
}

export function appEnvironment(event: H3Event): AppEnvironment {
  const platformEnvironment = (
    event.context as {
      _platform?: {
        cloudflare?: {
          env?: {
            APP_ENV?: string
          }
        }
      }
    }
  )._platform?.cloudflare?.env?.APP_ENV
  const workerEnvironment = (
    globalThis as typeof globalThis & {
      __env__?: {
        APP_ENV?: string
      }
    }
  ).__env__?.APP_ENV
  const config = useRuntimeConfig(event)
  return parseAppEnvironment(platformEnvironment ?? workerEnvironment ?? config.public.appEnv)
}

export function requireDevelopment(event: H3Event) {
  assertDevelopmentEnvironment(appEnvironment(event))
}

export function assertDevelopmentEnvironment(environment: AppEnvironment) {
  if (environment !== 'development') {
    throw createError({
      statusCode: 404,
      statusMessage: 'Not found.',
    })
  }
}
