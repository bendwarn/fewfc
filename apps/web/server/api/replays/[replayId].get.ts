import { requireSession } from '../../utils/auth'
import { replayFrame } from '../../utils/replay'

export default defineEventHandler(async (event) => {
  await requireSession(event)
  const replayId = getRouterParam(event, 'replayId')
  const step = Number(getQuery(event).step ?? 0)
  if (!replayId || !Number.isInteger(step) || step < 0) {
    throw createError({ statusCode: 400, statusMessage: 'Invalid replay request.' })
  }
  const frame = await replayFrame(event, replayId, step)
  if (!frame) throw createError({ statusCode: 404, statusMessage: '找不到這個重播' })
  return frame
})
