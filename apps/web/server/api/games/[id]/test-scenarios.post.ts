import { isDevelopmentScenario } from '../../../../shared/development-scenarios'

export default defineEventHandler(async (event) => {
  requireDevelopment(event)
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')
  const scenario = await readBody(event)

  if (!gameId) {
    throw createError({ statusCode: 400, statusMessage: 'Missing game id.' })
  }
  if (!isDevelopmentScenario(scenario)) {
    throw createError({ statusCode: 400, statusMessage: 'Unknown development scenario.' })
  }

  return await callGameRoom(event, gameId, {
    type: 'seedDevelopmentScenario',
    actorUserId: session.user.id,
    scenario,
  })
})
