import { authClient } from '~/lib/auth-client'

export function usePlayerSession() {
  const userId = useState<string>('player-session-user-id', () => '')
  const displayName = useState<string>('player-session-display-name', () => '玩家')

  async function refresh(): Promise<boolean> {
    const session = await authClient.getSession()
    const user = session.data?.user

    userId.value = user?.id ?? ''
    displayName.value = user?.name || '玩家'
    return Boolean(user)
  }

  function setUser(user: { id: string, name?: string | null }) {
    userId.value = user.id
    displayName.value = user.name || '玩家'
  }

  function clear() {
    userId.value = ''
    displayName.value = '玩家'
  }

  return { userId, displayName, refresh, setUser, clear }
}
