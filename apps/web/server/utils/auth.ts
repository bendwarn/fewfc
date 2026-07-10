import { drizzleAdapter } from '@better-auth/drizzle-adapter'
import { betterAuth } from 'better-auth'
import { hashPassword, verifyPassword } from 'better-auth/crypto'
import { anonymous } from 'better-auth/plugins'
import { drizzle } from 'drizzle-orm/d1'
import { eq } from 'drizzle-orm'
import { createError, type H3Event } from 'h3'
import { schema } from '../database/schema'
import { workerEnv } from './worker-env'

export const EMAIL_PASSWORD_MIN_LENGTH = 10
export const EMAIL_PASSWORD_MAX_LENGTH = 128

export const hashEmailPassword = hashPassword
export const verifyEmailPassword = verifyPassword

export function authForEvent(event: H3Event) {
  const env = workerEnv(event)
  const secret = env.BETTER_AUTH_SECRET

  if (!env.DB) {
    throw createError({
      statusCode: 501,
      statusMessage: 'The D1 DB binding is unavailable.',
    })
  }

  if (!secret || secret.length < 32) {
    throw createError({
      statusCode: 500,
      statusMessage: 'BETTER_AUTH_SECRET must contain at least 32 characters.',
    })
  }

  const db = drizzle(env.DB as Parameters<typeof drizzle>[0], { schema })

  return betterAuth({
    baseURL: env.BETTER_AUTH_URL,
    secret,
    database: drizzleAdapter(db, {
      provider: 'sqlite',
      schema,
    }),
    emailAndPassword: {
      enabled: true,
      minPasswordLength: EMAIL_PASSWORD_MIN_LENGTH,
      maxPasswordLength: EMAIL_PASSWORD_MAX_LENGTH,
      password: {
        hash: hashEmailPassword,
        verify: verifyEmailPassword,
      },
    },
    session: {
      expiresIn: 60 * 60 * 24 * 7,
      updateAge: 60 * 60 * 24,
      cookieCache: {
        enabled: true,
        maxAge: 60,
      },
    },
    plugins: [
      anonymous({
        generateName: () => `旅人-${crypto.randomUUID().slice(0, 6)}`,
      }),
    ],
    databaseHooks: {
      user: {
        create: {
          after: async (createdUser) => {
            await db
              .insert(schema.playerProfile)
              .values({
                userId: createdUser.id,
                displayName: createdUser.name,
                createdAt: new Date(),
                updatedAt: new Date(),
              })
              .onConflictDoNothing()
          },
        },
        update: {
          after: async (updatedUser) => {
            await db
              .update(schema.playerProfile)
              .set({
                displayName: updatedUser.name,
                avatarUrl: updatedUser.image,
                updatedAt: new Date(),
              })
              .where(eq(schema.playerProfile.userId, updatedUser.id))
          },
        },
      },
    },
  })
}

export async function requireSession(event: H3Event) {
  const session = await authForEvent(event).api.getSession({
    headers: event.headers,
  })

  if (!session?.user) {
    throw createError({
      statusCode: 401,
      statusMessage: 'Authentication required.',
    })
  }

  return session
}
