import { resolveDeck } from '../utils/player-deck'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  await deleteCustomDeck(event, session.user.id)
  return await resolveDeck(session.user.id)
})
