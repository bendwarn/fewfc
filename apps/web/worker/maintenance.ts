/** Deployment-configured maintenance policy. It intentionally has no storage. */
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
