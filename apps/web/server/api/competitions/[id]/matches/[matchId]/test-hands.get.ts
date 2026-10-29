import { createError, defineEventHandler, getRouterParam, setHeader } from 'h3'
import { validateCompetitionGameBinding } from '../../../../../../shared/competition-game-room'
import { requireDevelopment } from '../../../../../utils/app-environment'
import { requireSession } from '../../../../../utils/auth'
import { callCompetitionRoom } from '../../../../../utils/competition-room'

export default defineEventHandler(async (event) => {
  requireDevelopment(event)
  const session = await requireSession(event)
  const competitionId = getRouterParam(event, 'id', { decode: true })?.trim()
  const matchId = getRouterParam(event, 'matchId', { decode: true })?.trim()
  if (!competitionId?.startsWith('test-') || !matchId) throw createError({ statusCode: 404, statusMessage: 'Not Found' })
  const game = await callCompetitionRoom(event, competitionId, room => room.readProvisionedGame({
    competitionId, matchId, callerUserId: session.user.id,
  }))
  try {
    validateCompetitionGameBinding(game.binding)
    if (game.competitionId !== competitionId || game.matchId !== matchId
      || game.binding.competitionId !== competitionId || game.binding.matchId !== matchId
      || game.binding.organizerUserId !== session.user.id || game.status !== 'provisioned') throw new Error('invalid binding')
    const seats = game.binding.turnOrder.map(player => {
      const member = game.binding.players.find(candidate => candidate.player === player)
      if (!member) throw new Error('invalid seat')
      return { userId: member.userId, teamId: member.team }
    })
    setHeader(event, 'Cache-Control', 'no-store')
    return { gameNumber: game.gameNumber, seats }
  } catch {
    throw createError({ statusCode: 409, statusMessage: 'Conflict', data: { message: '對局座次尚未就緒。' } })
  }
})
