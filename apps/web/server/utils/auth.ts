import { drizzleAdapter } from '@better-auth/drizzle-adapter'
import type { D1Database } from '@cloudflare/workers-types'
import { betterAuth } from 'better-auth'
import { APIError, createAuthMiddleware, getSessionFromCtx } from 'better-auth/api'
import { setSessionCookie } from 'better-auth/cookies'
import { hashPassword, verifyPassword } from 'better-auth/crypto'
import { anonymous } from 'better-auth/plugins'
import { drizzle } from 'drizzle-orm/d1'
import { and, eq, isNull, or } from 'drizzle-orm'
import { createError, type H3Event } from 'h3'
import { schema } from '../database/schema'
import {
  activeAnonymousUpgradeMessage,
  hasActiveRoom,
  migrateAnonymousData,
  providerUserUpdateWithCanonicalName,
  shouldFillAvatar,
  socialAuthConfiguration,
} from './social-auth'
import { workerEnv } from './worker-env'

export const EMAIL_PASSWORD_MIN_LENGTH = 10
export const EMAIL_PASSWORD_MAX_LENGTH = 128

export const hashEmailPassword = hashPassword
export const verifyEmailPassword = verifyPassword

function blockAnonymousUpgradeInActiveRoom(database: D1Database) {
  return {
    id: 'block-anonymous-upgrade-in-active-room',
    hooks: {
      before: [{
        matcher(context: { path?: string }) {
          return context.path === '/sign-in/social'
        },
        handler: createAuthMiddleware(async (context) => {
          const session = await getSessionFromCtx(context, { disableRefresh: true })
          if (session?.user.isAnonymous && await hasActiveRoom(database, session.user.id)) {
            throw APIError.from('BAD_REQUEST', {
              message: activeAnonymousUpgradeMessage,
              code: 'ANONYMOUS_UPGRADE_ACTIVE_ROOM',
            })
          }
        }),
      }],
    },
  }
}

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
  const socialAuth = socialAuthConfiguration(env)

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
    ...socialAuth,
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
        onLinkAccount: async ({ anonymousUser, newUser, ctx }) => {
          const migration = await migrateAnonymousData(
            env.DB as D1Database,
            anonymousUser.user.id,
            newUser.user.id,
          )
          if (migration.targetName) {
            await setSessionCookie(ctx, {
              session: newUser.session,
              user: { ...newUser.user, name: migration.targetName },
            })
          }
        },
      }),
      blockAnonymousUpgradeInActiveRoom(env.DB as D1Database),
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
                avatarUrl: createdUser.image,
                createdAt: new Date(),
                updatedAt: new Date(),
              })
              .onConflictDoNothing()
          },
        },
        update: {
          before: async (candidate) => ({
            data: providerUserUpdateWithCanonicalName(candidate),
          }),
          after: async (updatedUser) => {
            if (!updatedUser.image) return
            const profile = await db
              .select({ avatarUrl: schema.playerProfile.avatarUrl })
              .from(schema.playerProfile)
              .where(eq(schema.playerProfile.userId, updatedUser.id))
              .get()
            if (!shouldFillAvatar(profile?.avatarUrl, updatedUser.image)) return
            await db
              .update(schema.playerProfile)
              .set({
                avatarUrl: updatedUser.image,
                updatedAt: new Date(),
              })
              .where(and(
                eq(schema.playerProfile.userId, updatedUser.id),
                or(isNull(schema.playerProfile.avatarUrl), eq(schema.playerProfile.avatarUrl, '')),
              ))
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
