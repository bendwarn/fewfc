import { createError, defineEventHandler, getRouterParam } from 'h3'
import { requireSession } from '../../utils/auth'
import { buildReadRequest, callCompetitionRoom } from '../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id')
  if (!competitionId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事識別碼。', data: { code: 'invalidRequest' } })
  }

  return await callCompetitionRoom(event, competitionId, room => room.read(
    buildReadRequest(competitionId, session.user.id),
  ))
})
