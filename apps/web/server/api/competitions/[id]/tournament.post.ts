import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../utils/auth'
import {
  buildCreateTournamentCommand,
  buildCreateTournamentRequest,
  callCompetitionRoom,
  projectCreateTournamentPublicValue,
} from '../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id')
  if (!competitionId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事識別碼。', data: { code: 'invalidRequest' } })
  }
  const normalizedCompetitionId = competitionId.trim()

  const command = buildCreateTournamentCommand(await readBody<unknown>(event))
  const request = buildCreateTournamentRequest(normalizedCompetitionId, session.user.id, command)
  const value = await callCompetitionRoom(event, normalizedCompetitionId, room =>
    room.createTournamentFromLockedRoster(request),
  )
  return projectCreateTournamentPublicValue(value)
})
