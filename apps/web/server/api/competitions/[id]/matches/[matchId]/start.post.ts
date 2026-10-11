import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../../../utils/auth'
import type {
  CompetitionRoomProvisionInitialScheduledGameValue,
  CompetitionRoomStartScheduledMatchValue,
} from '../../../../../../shared/competition-room'
import {
  buildStartScheduledMatchCommand,
  buildStartScheduledMatchCompositeRequests,
  callCompetitionRoomExact,
  projectProvisionedInitialGameValue,
  serverTrustedTime,
  validateStartedScheduledMatchValue,
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

  const command = buildStartScheduledMatchCommand(await readBody<unknown>(event))
  const trustedTimeMs = serverTrustedTime()
  const requests = buildStartScheduledMatchCompositeRequests(
    normalizedCompetitionId,
    normalizedMatchId,
    session.user.id,
    command,
    trustedTimeMs,
  )

  const started = await callCompetitionRoomExact<CompetitionRoomStartScheduledMatchValue>(event, normalizedCompetitionId, room =>
    room.startScheduledMatch(requests.start),
  )
  validateStartedScheduledMatchValue(started, requests.start)

  const provisioned = await callCompetitionRoomExact<CompetitionRoomProvisionInitialScheduledGameValue>(event, normalizedCompetitionId, room =>
    room.provisionInitialScheduledGame(requests.initialGame),
  )
  return projectProvisionedInitialGameValue(
    provisioned,
    requests.initialGame,
    started.revision,
    requests.rootCommandId,
  )
})
