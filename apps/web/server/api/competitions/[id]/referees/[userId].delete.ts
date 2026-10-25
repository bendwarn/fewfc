import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../../utils/auth'
import {
  buildRefereeCommand,
  buildRefereeRequest,
  callCompetitionRoom,
} from '../../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id', { decode: true })
  const targetUserId = getRouterParam(event, 'userId', { decode: true })
  if (!competitionId?.trim() || !targetUserId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事或使用者識別碼。', data: { code: 'invalidRequest' } })
  }

  const command = buildRefereeCommand(await readBody<unknown>(event), targetUserId)
  return await callCompetitionRoom(event, competitionId, room => room.revokeReferee(
    buildRefereeRequest(competitionId, session.user.id, command),
  ))
})
