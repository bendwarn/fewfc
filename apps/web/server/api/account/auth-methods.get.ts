import { eq } from 'drizzle-orm'
import type { D1Database } from '@cloudflare/workers-types'
import { drizzle } from 'drizzle-orm/d1'
import { schema } from '../../database/schema'
import { requireSession } from '../../utils/auth'
import { canAnonymousUpgrade, hasActiveRoom, socialProviderCapabilities } from '../../utils/social-auth'
import { workerEnv } from '../../utils/worker-env'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const env = workerEnv(event)
  const db = drizzle(env.DB as Parameters<typeof drizzle>[0], { schema })
  const accounts = await db
    .select({ id: schema.account.id, providerId: schema.account.providerId, createdAt: schema.account.createdAt })
    .from(schema.account)
    .where(eq(schema.account.userId, session.user.id))
  const isAnonymous = session.user.isAnonymous === true
  const activeRoom = isAnonymous && await hasActiveRoom(env.DB as D1Database, session.user.id)

  return {
    providers: socialProviderCapabilities(env),
    accounts,
    isAnonymous,
    canUpgrade: canAnonymousUpgrade(activeRoom),
    upgradeMessage: activeRoom ? '請先離開等待中或進行中的房間，再升級帳號。' : undefined,
  }
})
