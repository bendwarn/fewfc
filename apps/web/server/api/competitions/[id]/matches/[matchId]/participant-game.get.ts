import { defineEventHandler, getQuery, getRequestHeader, getRouterParam, type H3Event } from 'h3'
import { requireSession } from '../../../../../utils/auth'
import {
  assertCompetitionParticipantGameQuery,
  normalizeCompetitionParticipantGamePathId,
  readCompetitionParticipantGame,
} from '../../../../../utils/competition-participant-game'

function hasRequestBody(event: H3Event): boolean {
  const requestBody = event._requestBody
  if (requestBody !== undefined && requestBody !== null && requestBody !== '') return true
  const webBody = event.web?.request?.body
  if (webBody !== undefined && webBody !== null) return true

  const contentLength = getRequestHeader(event, 'content-length')
  if (contentLength !== undefined && contentLength.trim() !== '' && Number(contentLength) !== 0) {
    return true
  }
  return Boolean(getRequestHeader(event, 'transfer-encoding')?.trim())
}

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = normalizeCompetitionParticipantGamePathId(getRouterParam(event, 'id', { decode: true }))
  const matchId = normalizeCompetitionParticipantGamePathId(getRouterParam(event, 'matchId', { decode: true }))

  assertCompetitionParticipantGameQuery({ ...getQuery<Record<string, unknown>>(event) }, hasRequestBody(event))

  return await readCompetitionParticipantGame(
    event,
    competitionId,
    matchId,
    session.user.id,
  )
})
