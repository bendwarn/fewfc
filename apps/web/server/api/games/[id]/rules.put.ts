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
  let enabledRuleModules: string[]
  try {
    enabledRuleModules = await resolveServerRuleModules(body.enabledRuleModules)
  } catch {
    throw createError({ statusCode: 400, statusMessage: 'Invalid Rule Module configuration.' })
  }
  const response = await callGameRoom(event, gameId, {
    type: 'updateRuleModules',
    actorUserId: session.user.id,
    enabledRuleModules,
  })

  await updatePublicRoom(event, response)
  return response
})
