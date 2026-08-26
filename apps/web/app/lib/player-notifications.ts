import type { PlayerNotification } from '../../shared/game-room'

export function mergePlayerNotification(
  notifications: PlayerNotification[],
  incoming: PlayerNotification,
): PlayerNotification[] {
  const current = notifications.find(notification => notification.gameId === incoming.gameId)
  // 補位通知必須讓當事人看見，不能被同房的一般更新立刻覆蓋。
  if (current?.kind === 'seatPromoted' && incoming.kind === 'roomChanged') {
    return notifications
  }
  return [
    incoming,
    ...notifications.filter(notification => notification.gameId !== incoming.gameId),
  ].slice(0, 8)
}
