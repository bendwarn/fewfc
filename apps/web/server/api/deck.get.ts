import { effectiveDeck, validateDeck } from '../utils/player-deck'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const custom = await customDeckForUser(event, session.user.id)
  const resolved = effectiveDeck(custom)

  return {
    ...resolved,
    validation: validateDeck(resolved.deck),
  }
})
