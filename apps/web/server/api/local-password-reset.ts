import { hashEmailPassword, EMAIL_PASSWORD_MAX_LENGTH, EMAIL_PASSWORD_MIN_LENGTH } from '../utils/auth'
import { appEnvironment } from '../utils/app-environment'
import { schema } from '../database/schema'
import { workerEnv } from '../utils/worker-env'
import { isLocalPasswordResetEnabled, type LocalPasswordResetResult } from '#shared/local-password-reset'
import { and, eq } from 'drizzle-orm'
import { drizzle } from 'drizzle-orm/d1'
import { createError, type H3Event } from 'h3'

function requireLocalPasswordReset(event: H3Event) {
  const env = workerEnv(event)
  if (!isLocalPasswordResetEnabled(appEnvironment(event), env.LOCAL_PASSWORD_RESET_ENABLED)) {
    throw createError({ statusCode: 404, statusMessage: 'Not found.' })
  }

  return env
}

function resetResult(status: LocalPasswordResetResult) {
  return { status }
}

export default defineEventHandler(async (event) => {
  const env = requireLocalPasswordReset(event)

  if (event.method === 'GET') {
    return { enabled: true }
  }

  if (event.method !== 'POST') {
    throw createError({ statusCode: 405, statusMessage: 'Method not allowed.' })
  }

  const body = await readBody<{
    email?: unknown
    newPassword?: unknown
    confirmPassword?: unknown
  }>(event)
  const email = typeof body.email === 'string' ? body.email.trim() : ''
  const newPassword = typeof body.newPassword === 'string' ? body.newPassword : ''
  const confirmPassword = typeof body.confirmPassword === 'string' ? body.confirmPassword : ''

  if (!email || !newPassword || !confirmPassword) {
    throw createError({ statusCode: 400, statusMessage: '請輸入 Email、新密碼與確認密碼。' })
  }

  if (newPassword !== confirmPassword) {
    throw createError({ statusCode: 400, statusMessage: '兩次輸入的新密碼不一致。' })
  }

  if (newPassword.length < EMAIL_PASSWORD_MIN_LENGTH) {
    throw createError({ statusCode: 400, statusMessage: `密碼至少需要 ${EMAIL_PASSWORD_MIN_LENGTH} 個字元。` })
  }

  if (newPassword.length > EMAIL_PASSWORD_MAX_LENGTH) {
    throw createError({ statusCode: 400, statusMessage: `密碼不得超過 ${EMAIL_PASSWORD_MAX_LENGTH} 個字元。` })
  }

  const db = drizzle(env.DB as Parameters<typeof drizzle>[0], { schema })
  const user = await db
    .select({ id: schema.user.id })
    .from(schema.user)
    .where(eq(schema.user.email, email))
    .get()

  if (!user) {
    return resetResult('user-not-found')
  }

  const credential = await db
    .select({ id: schema.account.id })
    .from(schema.account)
    .where(and(
      eq(schema.account.userId, user.id),
      eq(schema.account.providerId, 'credential'),
    ))
    .get()

  if (!credential) {
    return resetResult('no-credential')
  }

  await db
    .update(schema.account)
    .set({
      password: await hashEmailPassword(newPassword),
      updatedAt: new Date(),
    })
    .where(eq(schema.account.id, credential.id))

  return resetResult('reset')
})
