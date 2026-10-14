import { createError, defineEventHandler, getQuery, getRouterParam } from 'h3'
import { requireSession } from '../../utils/auth'
import { competitionReplayDraft, createReplayArchive, replayFrame } from '../../utils/replay'
import { parseCompetitionReplayId } from '../../../shared/competition-room'
import { readCompetitionTournament } from '../../utils/competition-tournament'

export default defineEventHandler(async (event) => {
  const replayId = getRouterParam(event, 'replayId')
  const query = getQuery(event)
  const step = Number(query.step ?? 0)
  if (!replayId || !Number.isInteger(step) || step < 0) {
    throw createError({ statusCode: 400, statusMessage: 'Invalid replay request.' })
  }

  if (replayId.startsWith('competition:')) {
    const ref = parseCompetitionReplayId(replayId)
    if (!ref) throw createError({ statusCode: 404, statusMessage: '找不到這個重播' })
    const tournament = await readCompetitionTournament(event, ref.competitionId, null)
    const published = tournament.publication?.replays.some(replay => (
      replay.replayId === replayId && replay.gameId === ref.gameId
    )) ?? false
    if (!published) throw createError({ statusCode: 404, statusMessage: '找不到這個重播' })

    const draft = await competitionReplayDraft(event, ref.competitionId, ref.gameId)
    await createReplayArchive(event, draft, { referenceCount: 1, version: 1 })
    const perspective = typeof query.perspective === 'string' ? query.perspective : undefined
    const frame = await replayFrame(event, replayId, step, perspective, true)
    if (!frame) throw createError({ statusCode: 404, statusMessage: '找不到這個重播' })
    return frame
  }

  await requireSession(event)
  const perspective = typeof query.perspective === 'string' ? query.perspective : undefined
  const frame = await replayFrame(event, replayId, step, perspective)
  if (!frame) throw createError({ statusCode: 404, statusMessage: '找不到這個重播' })
  return frame
})
