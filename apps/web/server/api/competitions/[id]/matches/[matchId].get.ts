import { createError, defineEventHandler, getRouterParam } from 'h3'
import { requireSession } from '../../../../utils/auth'
import {
  buildMatchReadRequest,
  callCompetitionRoom,
} from '../../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id', { decode: true })
  const matchId = getRouterParam(event, 'matchId', { decode: true })
  if (!competitionId?.trim() || !matchId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事或交手識別碼。', data: { code: 'invalidRequest' } })
  }

  return await callCompetitionRoom(event, competitionId, room => room.readMatch(
    buildMatchReadRequest(competitionId, matchId, session.user.id),
  ))
})
