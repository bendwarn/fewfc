import { requireSession } from '../../utils/auth'
import { savedReplaySummaries } from '../../utils/replay'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  return { replays: await savedReplaySummaries(event, session.user.id) }
})
