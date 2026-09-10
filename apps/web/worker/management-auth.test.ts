import { expect, test } from 'bun:test'
import { managementAuthorized } from './management-auth'

const accountId = 'a'.repeat(32)

function request(headers: Record<string, string> = {}) {
  return new Request('https://fewfc.example.test/internal/legacy-purge/room-probe', { headers })
}

test('接受由同一 Cloudflare 帳戶 verify endpoint 確認為 active 的 Wrangler token', async () => {
  const calls: Array<{ url: string, init?: RequestInit }> = []
  const authorized = await managementAuthorized(
    request({ 'x-fewfc-cloudflare-token': 'short-lived-token' }),
    { MAINTENANCE_MODE: 'true', CLOUDFLARE_ACCOUNT_ID: accountId },
    async (url, init) => {
      calls.push({ url, init })
      return new Response(JSON.stringify({ success: true, result: { status: 'active' } }), { status: 200 })
    },
  )

  expect(authorized).toBe(true)
  expect(calls).toHaveLength(1)
  expect(calls[0]?.url).toBe(`https://api.cloudflare.com/client/v4/accounts/${accountId}/tokens/verify`)
  expect(calls[0]?.init?.headers).toEqual({ authorization: 'Bearer short-lived-token' })
})

test('帳戶 ID 單獨不能授權，且 token verify 失敗時 fail closed', async () => {
  let calls = 0
  await expect(managementAuthorized(
    request(),
    { MAINTENANCE_MODE: 'true', CLOUDFLARE_ACCOUNT_ID: accountId },
    async () => {
      calls += 1
      return new Response('{}', { status: 200 })
    },
  )).resolves.toBe(false)
  expect(calls).toBe(0)

  for (const response of [
    new Response(JSON.stringify({ success: true, result: { status: 'expired' } }), { status: 200 }),
    new Response(JSON.stringify({ success: false }), { status: 200 }),
    new Response('{}', { status: 403 }),
  ]) {
    await expect(managementAuthorized(
      request({ 'x-fewfc-cloudflare-token': 'invalid-token' }),
      { MAINTENANCE_MODE: 'true', CLOUDFLARE_ACCOUNT_ID: accountId },
      async () => response,
    )).resolves.toBe(false)
  }

  await expect(managementAuthorized(
    request({ 'x-fewfc-cloudflare-token': 'invalid-token' }),
    { MAINTENANCE_MODE: 'true', CLOUDFLARE_ACCOUNT_ID: accountId },
    async () => { throw new Error('network unavailable') },
  )).resolves.toBe(false)
})

test('maintenance gate remains required for a valid token', async () => {
  await expect(managementAuthorized(
    request({ 'x-fewfc-cloudflare-token': 'valid-token' }),
    { MAINTENANCE_MODE: 'false', CLOUDFLARE_ACCOUNT_ID: accountId },
    async () => { throw new Error('token verify must not be called') },
  )).resolves.toBe(false)
})
