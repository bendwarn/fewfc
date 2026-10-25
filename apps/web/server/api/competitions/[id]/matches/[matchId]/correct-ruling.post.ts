import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../../../utils/auth'
import {
  buildRulingCorrectionCommand,
  buildRulingCorrectionRequest,
  executeRulingCorrection,
  serverRulingCorrectionTime,
} from '../../../../../utils/competition-ruling-correction'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id', { decode: true })?.trim()
  const matchId = getRouterParam(event, 'matchId', { decode: true })?.trim()
  if (!competitionId || !matchId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'InvalidCompetitionRulingCorrectionPath',
      data: { code: 'invalidRequest', message: '賽事或交手識別碼不可空白。' },
    })
  }
  let body: unknown
  try {
    body = await readBody<unknown>(event)
  } catch (error) {
    if (!isBadRequestBodyError(error)) throw error
    throw createError({
      statusCode: 400,
      statusMessage: 'InvalidCompetitionRulingCorrectionRequest',
      data: { code: 'invalidRequest', message: '更正要求格式不正確。' },
    })
  }
  const command = buildRulingCorrectionCommand(body)
  const request = buildRulingCorrectionRequest(
    competitionId,
    matchId,
    session.user.id,
    command,
    serverRulingCorrectionTime(),
  )
  return executeRulingCorrection(event, request)
})

function isBadRequestBodyError(error: unknown): boolean {
  return typeof error === 'object' && error !== null
    && 'statusCode' in error && error.statusCode === 400
}
