import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../utils/auth'
import {
  buildRefereeCommand,
  buildRefereeRequest,
  callCompetitionRoom,
} from '../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id')
  if (!competitionId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事識別碼。', data: { code: 'invalidRequest' } })
  }

  const command = buildRefereeCommand(await readBody<unknown>(event))
  return await callCompetitionRoom(event, competitionId, room => room.grantReferee(
    buildRefereeRequest(competitionId, session.user.id, command),
  ))
})
