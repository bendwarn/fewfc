import type { InjectionKey } from 'vue'
import { usePlayerNotifications } from '~/composables/usePlayerNotifications'

export type PlayerNotifications = ReturnType<typeof usePlayerNotifications>

export const playerNotificationsKey: InjectionKey<PlayerNotifications> = Symbol('player-notifications')

export function useLayoutNotifications(): PlayerNotifications {
  const notifications = inject(playerNotificationsKey)

  if (!notifications) {
    throw new Error('Player Notifications require the authenticated layout.')
  }

  return notifications
}
