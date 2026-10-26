import { expect, mock, test } from 'bun:test'
import type { H3Event } from 'h3'
import { competitionReplayId } from '../../shared/competition-room'

const competitionId = 'competition-1'
const gameId = 'game-1'
const replayDraft = {
  schemaVersion: 1 as const,
  replayId: competitionReplayId(competitionId, gameId),
  sourceGameId: gameId,
  finishedAt: '2026-10-26T00:00:00.000Z',
  setup: {},
  record: [],
  players: [{ player: 'player-1', displayName: 'Player One' }],
  originalUserIds: ['user-1'],
  roomName: 'Test competition game',
  result: { status: 'Finished' },
  firstPlayer: 'player-1',
}
const rpcDispose = Reflect.get(Symbol, 'dispose') as symbol
Object.defineProperty(replayDraft, rpcDispose, { value: () => {} })
Object.defineProperty(replayDraft.players[0]!, rpcDispose, { value: () => {} })

mock.module('./worker-env', () => ({
  workerEnv: () => ({
    GAME_ROOM: {
      idFromName: (name: string) => name,
      get: () => ({ getCompetitionReplayDraft: async () => replayDraft }),
    },
  }),
}))

const { competitionReplayDraft } = await import('./replay')

test('competition replay validation ignores platform Symbol.dispose RPC metadata', async () => {
  const result = await competitionReplayDraft({} as H3Event, competitionId, gameId)

  expect(result).toMatchObject({
    schemaVersion: 1,
    replayId: competitionReplayId(competitionId, gameId),
    sourceGameId: gameId,
  })
})
