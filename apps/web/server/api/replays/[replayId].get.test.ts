import { createApp, createRouter, eventHandler, toWebHandler } from 'h3'
import { afterEach, expect, mock, test } from 'bun:test'
import { competitionReplayId } from '../../../shared/competition-room'

let sessionCalls = 0
let tournamentReads: Array<[string, string | null]> = []
let drafts: Array<[string, string]> = []
let archiveCreates: Array<[string, { referenceCount: number, version: number }]> = []
let frameReads: Array<[string, number, string | undefined, boolean | undefined]> = []
let publication: { replays: Array<{ matchId: string, gameId: string, gameNumber: number, replayId: string }> } = { replays: [] }

mock.module('../../utils/auth', () => ({
  requireSession: async () => {
    sessionCalls += 1
    return { user: { id: 'user-a' } }
  },
}))

mock.module('../../utils/competition-tournament', () => ({
  readCompetitionTournament: async (_event: unknown, competitionId: string, callerUserId: string | null) => {
    tournamentReads.push([competitionId, callerUserId])
    return { publication }
  },
}))

mock.module('../../utils/replay', () => ({
  competitionReplayDraft: async (_event: unknown, competitionId: string, gameId: string) => {
    drafts.push([competitionId, gameId])
    return { replayId: competitionReplayId(competitionId, gameId), sourceGameId: gameId }
  },
  createReplayArchive: async (_event: unknown, draft: { replayId: string }, lifecycle: { referenceCount: number, version: number }) => {
    archiveCreates.push([draft.replayId, lifecycle])
  },
  replayFrame: async (
    _event: unknown, replayId: string, step: number,
    perspective: string | undefined, publicOnly: boolean | undefined,
  ) => {
    frameReads.push([replayId, step, perspective, publicOnly])
    return { currentStep: step, replayId }
  },
}))

const { default: replayGet } = await import('./[replayId].get')

function reset(nextPublication = { replays: [] as typeof publication.replays }) {
  sessionCalls = 0
  tournamentReads = []
  drafts = []
  archiveCreates = []
  frameReads = []
  publication = nextPublication
}

function handler() {
  const app = createApp()
  const router = createRouter()
  router.get('/api/replays/:replayId', replayGet)
  app.use(router.handler)
  return toWebHandler(app)
}

async function get(path: string) {
  return handler()(new Request(`http://fewfc.test${path}`))
}

afterEach(() => {
  mock.restore()
})

test('published competition replay is rebuilt idempotently and projected for anonymous observers', async () => {
  const replayId = competitionReplayId('competition-1', 'game-1')
  publication = {
    replays: [{ matchId: 'match-1', gameId: 'game-1', gameNumber: 1, replayId }],
  }
  reset(publication)
  const response = await get(`/api/replays/${encodeURIComponent(replayId)}?step=4&perspective=player-a`)
  expect(response.status).toBe(200)
  const body: unknown = await response.json()
  expect(body).toEqual({ currentStep: 4, replayId })
  expect(sessionCalls).toBe(0)
  expect(tournamentReads).toEqual([['competition-1', null]])
  expect(drafts).toEqual([['competition-1', 'game-1']])
  expect(archiveCreates).toEqual([[replayId, { referenceCount: 1, version: 1 }]])
  expect(frameReads).toEqual([[replayId, 4, 'player-a', true]])
})

test('competition replay ids outside the completion publication cannot create an archive', async () => {
  const replayId = competitionReplayId('competition-1', 'game-1')
  reset()
  const response = await get(`/api/replays/${encodeURIComponent(replayId)}`)
  expect(response.status).toBe(404)
  expect(sessionCalls).toBe(0)
  expect(tournamentReads).toEqual([['competition-1', null]])
  expect(drafts).toHaveLength(0)
  expect(archiveCreates).toHaveLength(0)
  expect(frameReads).toHaveLength(0)
})

test('ordinary saved replay requests keep their authenticated private projection', async () => {
  reset()
  const response = await get('/api/replays/saved-replay-1?step=3')
  expect(response.status).toBe(200)
  expect(sessionCalls).toBe(1)
  expect(tournamentReads).toHaveLength(0)
  expect(frameReads).toEqual([['saved-replay-1', 3, undefined, undefined]])
})
