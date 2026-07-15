import { requireSession } from '../../utils/auth'
import { deleteReplayReference } from '../../utils/replay'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const replayId = getRouterParam(event, 'replayId')
  if (!replayId) throw createError({ statusCode: 400, statusMessage: 'Missing replayId.' })
  await deleteReplayReference(event, session.user.id, replayId)
  return { deleted: true }
})
