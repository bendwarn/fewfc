import {
  hasValidServerRuleModuleDependencies,
  normalizeServerRuleModules,
} from '../../../utils/rule-modules'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')
  const body = await readBody<{ enabledRuleModules?: unknown }>(event)

  if (!gameId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing game id.',
    })
  }
  if (!hasValidServerRuleModuleDependencies(body.enabledRuleModules)) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Spirit requires every Advanced Rule Module.',
    })
  }

  const response = await callGameRoom(event, gameId, {
    type: 'updateRuleModules',
    actorUserId: session.user.id,
    enabledRuleModules: normalizeServerRuleModules(body.enabledRuleModules),
  })

  await updatePublicRoom(event, response)
  return response
})
