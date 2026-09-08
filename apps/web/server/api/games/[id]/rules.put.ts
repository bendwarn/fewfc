export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')
  const body = await readBody<{ enabledRuleModules?: string[]; ruleVersion?: '5.16' | '5.17' }>(event)

  if (!gameId) {
    throw createError({
      statusCode: 400,
      statusMessage: 'Missing game id.',
    })
  }
  const response = await callGameRoom(event, gameId, {
    type: 'updateRuleModules',
    actorUserId: session.user.id,
    enabledRuleModules: body.enabledRuleModules,
    ruleVersion: body.ruleVersion,
  })

  await updatePublicRoom(event, response)
  return response
})
