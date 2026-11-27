import type { GameRoomResponse } from '#shared/game-room'
import type { PlayerId } from '~/types/fewfc'

export interface RoomClockSample {
  players: PlayerId[]
  localDeadlineMs: number
}

export function sampleRoomClock(clock: GameRoomResponse['operationClock']): RoomClockSample | undefined {
  if (!clock) return
  return {
    players: clock.players,
    localDeadlineMs: performance.now() + clock.deadlineMs - clock.serverTimeMs,
  }
}
