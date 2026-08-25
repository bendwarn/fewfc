/** 重播未指定視角時固定採先手；指定者必須是封存中的玩家。 */
export function resolveReplayPerspective(
  requested: string | undefined,
  firstPlayer: string,
  players: readonly string[],
): string | undefined {
  const perspective = requested ?? firstPlayer
  return players.includes(perspective) ? perspective : undefined
}
