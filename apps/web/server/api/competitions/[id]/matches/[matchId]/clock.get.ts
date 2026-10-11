import { defineEventHandler, getQuery, getRequestHeader, getRouterParam, type H3Event } from 'h3'
import { requireSession } from '../../../../../utils/auth'
import {
  assertCompetitionMatchClockQuery,
  buildCompetitionMatchClockReadRequest,
  normalizeCompetitionMatchClockPathId,
  readCompetitionMatchClock,
  serverCompetitionMatchClockTime,
} from '../../../../../utils/competition-match-clock'

function hasRequestBody(event: H3Event): boolean {
  if (event._requestBody !== undefined && event._requestBody !== null && event._requestBody !== '') return true
  if (event.web?.request?.body != null) return true
  const length = getRequestHeader(event, 'content-length')
  return (length !== undefined && length.trim() !== '' && Number(length) !== 0)
    || Boolean(getRequestHeader(event, 'transfer-encoding')?.trim())
}

export default defineEventHandler(async event => {
  const session = await requireSession(event)
  const competitionId = normalizeCompetitionMatchClockPathId(getRouterParam(event, 'id'))
  const matchId = normalizeCompetitionMatchClockPathId(getRouterParam(event, 'matchId'))
  assertCompetitionMatchClockQuery(getQuery(event), hasRequestBody(event))
  const request = buildCompetitionMatchClockReadRequest(competitionId, matchId, session.user.id, serverCompetitionMatchClockTime())
  return await readCompetitionMatchClock(event, request)
})
