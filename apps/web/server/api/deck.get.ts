import { resolveDeck } from '../utils/player-deck'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const custom = await customDeckForUser(event, session.user.id)
  return await resolveDeck(session.user.id, custom)
})
