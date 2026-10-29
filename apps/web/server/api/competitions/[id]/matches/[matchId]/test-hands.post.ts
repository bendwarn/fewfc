import { createError, defineEventHandler, getRouterParam, readBody } from 'h3'
import { requireDevelopment } from '../../../../../utils/app-environment'
import { requireSession } from '../../../../../utils/auth'
import { callCompetitionRoom } from '../../../../../utils/competition-room'
import { workerEnv } from '../../../../../utils/worker-env'

export default defineEventHandler(async (event) => {
  requireDevelopment(event)
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id', { decode: true })?.trim()
  const matchId = getRouterParam(event, 'matchId', { decode: true })?.trim()
  if (!competitionId?.startsWith('test-') || !matchId) {
    throw createError({ statusCode: 404, statusMessage: 'Not Found' })
  }
  const body: unknown = await readBody(event)
  if (!body || typeof body !== 'object' || Array.isArray(body)
    || Object.keys(body).length !== 1 || !('hands' in body) || !Array.isArray(body.hands)) {
    throw createError({ statusCode: 400, statusMessage: 'Bad Request', data: { message: '手牌格式不正確。' } })
  }
  const game = await callCompetitionRoom(event, competitionId, room => room.readProvisionedGame({
    competitionId, matchId, callerUserId: session.user.id,
  }))
  if (game.competitionId !== competitionId || game.matchId !== matchId || game.status !== 'provisioned') {
    throw createError({ statusCode: 409, statusMessage: 'Conflict', data: { message: '對局房間尚未就緒。' } })
  }
  const namespace = workerEnv(event).GAME_ROOM
  const room = namespace.get(namespace.idFromName(game.gameId)) as unknown as {
    setCompetitionTestHands?: (request: unknown) => Promise<unknown>
  }
  if (!room || typeof room.setCompetitionTestHands !== 'function') {
    throw createError({ statusCode: 503, statusMessage: 'Service Unavailable' })
  }
  const result = await room.setCompetitionTestHands({ competitionId, gameId: game.gameId,
    actorUserId: session.user.id, hands: body.hands })
  try {
    if (!result || typeof result !== 'object' || !('ok' in result)) throw new Error('invalid response')
    if (result.ok === true) return { ok: true }
    const code = 'code' in result ? result.code : null
    if (code === 'invalidHands') throw createError({ statusCode: 400, statusMessage: 'Bad Request', data: { message: '起手牌不符合共用牌庫或發牌數量。' } })
    if (code === 'alreadyStarted') throw createError({ statusCode: 409, statusMessage: 'Conflict', data: { message: '對局已開始，無法設定起手牌。' } })
    if (code === 'forbidden') throw createError({ statusCode: 403, statusMessage: 'Forbidden' })
    if (code === 'notFound') throw createError({ statusCode: 404, statusMessage: 'Not Found' })
    throw createError({ statusCode: 502, statusMessage: 'Bad Gateway' })
  } finally {
    const dispose = result && typeof result === 'object' ? Reflect.get(result, Symbol.dispose) : undefined
    if (typeof dispose === 'function') dispose.call(result)
  }
})
