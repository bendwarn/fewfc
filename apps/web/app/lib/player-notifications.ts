import type { PlayerNotification } from '../../shared/game-room'

export function mergePlayerNotification(
  notifications: PlayerNotification[],
  incoming: PlayerNotification,
): PlayerNotification[] {
  return [
    incoming,
    ...notifications.filter(notification => notification.gameId !== incoming.gameId),
  ].slice(0, 8)
}
