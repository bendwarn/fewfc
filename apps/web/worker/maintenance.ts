/** 由 Cloudflare Secret 設定的維護政策。它特意不使用儲存空間。 */
export function maintenanceEnabled(value: string | undefined): boolean {
  return value === 'true'
}

export function maintenanceBlocksPlayerMutation(method: string, pathname: string): boolean {
  return method === 'POST'
    && (
      /^\/api\/games\/[^/]+\/commands$/.test(pathname)
      || pathname === '/api/replays'
    )
}
