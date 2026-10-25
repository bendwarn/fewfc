import { defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireSession } from '../../../utils/auth'
import {
  buildCompetitionGameReadyRequest,
  callCompetitionGameReady,
  competitionGameReadyError,
  normalizeCompetitionGameId,
} from '../../../utils/competition-game-room'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const gameId = normalizeCompetitionGameId(getRouterParam(event, 'id', { decode: true }))

  let body: unknown
  try {
    body = await readBody<unknown>(event)
  } catch {
    // 不回傳 JSON 解析器訊息或任何未通過驗證的輸入片段。
    throw competitionGameReadyError('invalidRequest')
  }
  const request = buildCompetitionGameReadyRequest(body, session.user.id)
  return await callCompetitionGameReady(event, gameId, request)
})
