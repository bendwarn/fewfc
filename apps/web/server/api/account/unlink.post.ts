import type { D1Database } from '@cloudflare/workers-types'
import { createError } from 'h3'
import { requireSession } from '../../utils/auth'
import { unlinkAccountAtomically } from '../../utils/social-auth'
import { workerEnv } from '../../utils/worker-env'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const body = await readBody<{ accountId?: unknown }>(event)
  const accountId = typeof body.accountId === 'string' ? body.accountId : ''
  if (!accountId) throw createError({ statusCode: 400, statusMessage: '缺少登入方式。' })

  const env = workerEnv(event)
  const outcome = await unlinkAccountAtomically(env.DB as D1Database, session.user.id, accountId)
  if (outcome === 'last-authentication-method') {
    throw createError({ statusCode: 409, statusMessage: '至少要保留一種登入方式。' })
  }
  if (outcome === 'account-not-found') {
    throw createError({ statusCode: 404, statusMessage: '找不到登入方式。' })
  }
  return { unlinked: true }
})
