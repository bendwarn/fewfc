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
  const competitionId = normalizeCompetitionRoundPublicationPathId(getRouterParam(event, 'id'))
  const stageId = normalizeCompetitionRoundPublicationPathId(getRouterParam(event, 'stageId'))
  const roundNumber = parseCompetitionRoundPublicationRoundNumber(getRouterParam(event, 'roundNumber'))

  assertCompetitionRoundPublicationNoQuery(
    getQuery(event),
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
