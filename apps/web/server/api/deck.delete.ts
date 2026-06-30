import { effectiveDeck, validateDeck } from '../utils/player-deck'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  await deleteCustomDeck(event, session.user.id)
  const resolved = effectiveDeck(undefined)

  return {
    ...resolved,
    validation: validateDeck(resolved.deck),
  }
})
