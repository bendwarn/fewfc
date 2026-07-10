export default defineEventHandler(async (event) => {
  requireDevelopment(event)
  const session = await requireSession(event)
  const gameId = getRouterParam(event, 'id')

  if (!gameId) {
    throw createError({ statusCode: 400, statusMessage: 'Missing game id.' })
  }
  const body = (await readBody<{ mode?: 'actionDetail' }>(event).catch(() => undefined)) ?? {}

  return await callGameRoom(event, gameId, {
    type: 'seedEchoFixture',
    actorUserId: session.user.id,
    mode: body.mode,
  })
})
