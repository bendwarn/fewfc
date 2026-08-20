import { requireSession } from '../../utils/auth'
import { completedReplayDraft, saveReplayReference } from '../../utils/replay'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const body = await readBody<{ sourceGameId?: string; replaceReplayId?: string }>(event)
  if (!body.sourceGameId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: 'Missing sourceGameId.' })
  }
  // 變更參照前先取得來源：過期來源絕不能在替代模式中驅逐既有的最愛項目。
  const draft = await completedReplayDraft(event, body.sourceGameId, session.user.id)
  return await saveReplayReference(event, session.user.id, draft, body.replaceReplayId)
})
