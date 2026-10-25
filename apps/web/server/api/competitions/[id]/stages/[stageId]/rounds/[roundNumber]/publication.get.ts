import { defineEventHandler, getQuery, getRouterParam } from 'h3'
import { requireSession } from '../../../../../../../utils/auth'
import {
  assertCompetitionRoundPublicationNoQuery,
  buildCompetitionRoundPublicationReadRequest,
  competitionRoundPublicationHasQueryMarker,
  competitionRoundPublicationHasRequestBody,
  normalizeCompetitionRoundPublicationPathId,
  parseCompetitionRoundPublicationRoundNumber,
  readCompetitionRoundPublication,
} from '../../../../../../../utils/competition-round-publication'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = normalizeCompetitionRoundPublicationPathId(getRouterParam(event, 'id', { decode: true }))
  const stageId = normalizeCompetitionRoundPublicationPathId(getRouterParam(event, 'stageId', { decode: true }))
  const roundNumber = parseCompetitionRoundPublicationRoundNumber(getRouterParam(event, 'roundNumber', { decode: true }))

  assertCompetitionRoundPublicationNoQuery(
    { ...getQuery<Record<string, unknown>>(event) },
    competitionRoundPublicationHasQueryMarker(event),
    competitionRoundPublicationHasRequestBody(event),
  )

  const request = buildCompetitionRoundPublicationReadRequest(
    competitionId,
    stageId,
    String(roundNumber),
    session.user.id,
  )
  return await readCompetitionRoundPublication(event, request)
})
