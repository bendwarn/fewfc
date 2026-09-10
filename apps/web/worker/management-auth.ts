interface ManagementAuthEnv {
  MAINTENANCE_MODE?: string
  CLOUDFLARE_ACCOUNT_ID?: string
}

export type ManagementAuthFetcher = (
  input: string,
  init?: RequestInit,
) => Promise<Response>

/**
 * 管理端點只接受固定的維護閘門，加上 Wrangler token 的有效認證。
 * token 必須向 Cloudflare 帳戶的官方 verify endpoint 確認為 active；帳戶 ID
 * 只用來選定驗證端點，單獨提供帳戶 ID 絕不會授權請求。
 */
export async function managementAuthorized(
  request: Request,
  env: ManagementAuthEnv,
  fetcher: ManagementAuthFetcher = fetch,
): Promise<boolean> {
  if (env.MAINTENANCE_MODE?.trim().toLowerCase() !== 'true') return false

  const token = request.headers.get('x-fewfc-cloudflare-token')?.trim()
  const accountId = env.CLOUDFLARE_ACCOUNT_ID?.trim()
  if (!token || !accountId || !/^[0-9a-f]{32}$/i.test(accountId)) return false

  try {
    const response = await fetcher(
      `https://api.cloudflare.com/client/v4/accounts/${encodeURIComponent(accountId)}/tokens/verify`,
      { headers: { authorization: `Bearer ${token}` } },
    )
    if (!response.ok) return false
    const body = await response.json() as {
      success?: unknown
      result?: { status?: unknown }
    }
    return body.success === true && body.result?.status === 'active'
  } catch {
    // 驗證服務不可用時 fail closed；不把 token 或上游錯誤寫入 log。
    return false
  }
}
