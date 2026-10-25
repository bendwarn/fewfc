import { createError, defineEventHandler, getQuery, getRouterParam } from 'h3'
import { requireSession } from '../../../utils/auth'
import {
  boundedAuditLimit,
  buildAuditRequest,
  callCompetitionRoom,
} from '../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id', { decode: true })
  if (!competitionId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事識別碼。', data: { code: 'invalidRequest' } })
  }
  const query = getQuery(event)
  const limit = boundedAuditLimit(query.limit)

  return await callCompetitionRoom(event, competitionId, room => room.audit(
    buildAuditRequest(competitionId, session.user.id, limit),
  ))
})
