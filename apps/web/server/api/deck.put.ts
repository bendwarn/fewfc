import type { PlayerDeckList } from '../../shared/game-room'
import { validateDeck } from '../utils/player-deck'

export default defineEventHandler(async (event) => {
  const session = await requireSession(event)
  const body = await readBody<PlayerDeckList>(event)

  await saveCustomDeck(event, session.user.id, {
    name: typeof body.name === 'string' ? body.name : '我的牌組',
    cards: Array.isArray(body.cards) ? body.cards : [],
  })

  const custom = await customDeckForUser(event, session.user.id)
  return {
    deck: custom,
    source: 'custom' as const,
    validation: validateDeck(custom ?? { name: '', cards: [] }),
  }
})
