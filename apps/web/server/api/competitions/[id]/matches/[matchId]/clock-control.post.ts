import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../../../utils/auth'
import type { CompetitionRoomMatchExecuteValue } from '../../../../../../shared/competition-room'
import {
  buildClockControlCommand,
  buildClockControlRequest,
  callCompetitionRoomExact,
  projectClockControlPublicValue,
  serverTrustedTime,
} from '../../../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id')
  const matchId = getRouterParam(event, 'matchId')
  if (!competitionId?.trim() || !matchId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事或交手識別碼。', data: { code: 'invalidRequest' } })
  }
  const normalizedCompetitionId = competitionId.trim()
  const normalizedMatchId = matchId.trim()
  const command = buildClockControlCommand(await readBody<unknown>(event))
  const trustedTimeMs = serverTrustedTime()
  const request = buildClockControlRequest(
    normalizedCompetitionId,
    normalizedMatchId,
    session.user.id,
    command,
    trustedTimeMs,
  )
  const value = await callCompetitionRoomExact<CompetitionRoomMatchExecuteValue>(
    event,
    normalizedCompetitionId,
    room => room.executeMatch(request),
  )
  return projectClockControlPublicValue(value, request, command.action)
})
