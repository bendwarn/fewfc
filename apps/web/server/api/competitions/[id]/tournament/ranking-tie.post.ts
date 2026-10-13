import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../../utils/auth'
import {
  buildExecuteRequest,
  buildRankingTieDecisionCommand,
  callCompetitionRoom,
} from '../../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id')
  if (!competitionId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事識別碼。', data: { code: 'invalidRequest' } })
  }
  const command = buildRankingTieDecisionCommand(await readBody<unknown>(event))
  const request = buildExecuteRequest(competitionId.trim(), session.user.id, command)
  return await callCompetitionRoom(event, competitionId.trim(), room => room.execute(request))
})
