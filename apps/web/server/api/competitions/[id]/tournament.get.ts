import { defineEventHandler, getQuery, getRequestHeader, getRouterParam, type H3Event } from 'h3'
import { requireSession } from '../../../utils/auth'
import {
  assertCompetitionTournamentQuery,
  normalizeCompetitionTournamentPathId,
  readCompetitionTournament,
} from '../../../utils/competition-tournament'

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
  const competitionId = normalizeCompetitionTournamentPathId(getRouterParam(event, 'id'))

  assertCompetitionTournamentQuery(getQuery(event), hasRequestBody(event))

  return await readCompetitionTournament(event, competitionId, session.user.id)
})
