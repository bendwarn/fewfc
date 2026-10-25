import { defineEventHandler, getQuery, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../../../../../utils/auth'
import {
  announceCompetitionRound,
  assertCompetitionRoundPublicationNoQuery,
  buildCompetitionRoundAnnouncementRequest,
  competitionRoundPublicationHasQueryMarker,
  malformedCompetitionRoundPublicationJsonError,
  normalizeCompetitionRoundPublicationPathId,
  parseCompetitionRoundPublicationRoundNumber,
} from '../../../../../../../utils/competition-round-publication'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = normalizeCompetitionRoundPublicationPathId(getRouterParam(event, 'id', { decode: true }))
  const stageId = normalizeCompetitionRoundPublicationPathId(getRouterParam(event, 'stageId', { decode: true }))
  const roundNumber = parseCompetitionRoundPublicationRoundNumber(getRouterParam(event, 'roundNumber', { decode: true }))

  assertCompetitionRoundPublicationNoQuery(
    { ...getQuery<Record<string, unknown>>(event) },
    competitionRoundPublicationHasQueryMarker(event),
  )

  let body: unknown
  try {
    body = await readBody<unknown>(event)
  } catch {
    throw malformedCompetitionRoundPublicationJsonError()
  }

  const request = buildCompetitionRoundAnnouncementRequest(
    competitionId,
    stageId,
    String(roundNumber),
    session.user.id,
    body,
  )
  return await announceCompetitionRound(event, request)
})
