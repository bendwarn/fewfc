import { requireSession } from '../../utils/auth'
import { completedReplayDraft, saveReplayReference } from '../../utils/replay'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const body = await readBody<{ sourceGameId?: string; replaceReplayId?: string }>(event)
  if (!body.sourceGameId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: 'Missing sourceGameId.' })
  }
  // Fetch the source before changing a reference: an expired source must never
  // evict an existing favourite in replacement mode.
  const draft = await completedReplayDraft(event, body.sourceGameId, session.user.id)
  return await saveReplayReference(event, session.user.id, draft, body.replaceReplayId)
})
