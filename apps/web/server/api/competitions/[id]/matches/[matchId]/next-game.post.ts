import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../../../utils/auth'
import type { CompetitionRoomAssignNextFirstPlayerByCaptainValue } from '../../../../../../shared/competition-room'
import {
  buildCaptainNextGameCommand,
  buildCaptainNextGameCompositeRequests,
  buildCaptainNextGameProvisionRequest,
  callCompetitionRoomExact,
  projectCaptainAssignedGameValue,
  serverTrustedTime,
  validateCaptainFirstPlayerAssignmentValue,
} from '../../../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id', { decode: true })
  const matchId = getRouterParam(event, 'matchId', { decode: true })
  if (!competitionId?.trim() || !matchId?.trim()) {
    throw createError({ statusCode: 400, statusMessage: '缺少賽事或交手識別碼。', data: { code: 'invalidRequest' } })
  }
  const normalizedCompetitionId = competitionId.trim()
  const normalizedMatchId = matchId.trim()
  const command = buildCaptainNextGameCommand(await readBody<unknown>(event))
  const trustedTimeMs = serverTrustedTime()
  const composite = buildCaptainNextGameCompositeRequests(
    normalizedCompetitionId,
    normalizedMatchId,
    session.user.id,
    command,
    trustedTimeMs,
  )

  const assignment = await callCompetitionRoomExact<CompetitionRoomAssignNextFirstPlayerByCaptainValue>(
    event,
    normalizedCompetitionId,
    room => room.assignNextFirstPlayerByCaptain(composite.assignment),
  )
  const validatedAssignment = validateCaptainFirstPlayerAssignmentValue(
    assignment, composite.assignment,
  )
  const provisionRequest = buildCaptainNextGameProvisionRequest(
    composite, validatedAssignment.matchRevision,
  )
  const provisioned = await callCompetitionRoomExact<unknown>(
    event,
    normalizedCompetitionId,
    room => room.provisionCaptainAssignedScheduledGame(provisionRequest),
  )
  try {
    return projectCaptainAssignedGameValue(
      provisioned, provisionRequest, validatedAssignment, composite.rootCommandId,
    )
  } catch {
    throw createError({
      statusCode: 502,
      statusMessage: '賽事服務暫時無法處理要求。',
      data: { code: 'engineProtocolError' },
    })
  }
})
