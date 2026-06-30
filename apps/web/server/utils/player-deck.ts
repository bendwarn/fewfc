import { drizzle } from 'drizzle-orm/d1'
import { eq } from 'drizzle-orm'
import { createError, type H3Event } from 'h3'
import type { PlayerDeckList } from '../../shared/game-room'
import { schema } from '../database/schema'
import { workerEnv } from './worker-env'

function database(event: H3Event) {
  return drizzle(workerEnv(event).DB as Parameters<typeof drizzle>[0], { schema })
}

const elements = ['metal', 'wood', 'water', 'fire', 'earth']
const levels = [1, 2, 3, 4, 5]

export function preconstructedDeck(): PlayerDeckList {
  const copies = [3, 2, 3, 2, 2]
  return {
    name: '五行均衡預組',
    cards: elements.flatMap(element => levels.flatMap(
      (level, index) => Array.from({ length: copies[index] ?? 0 }, () => `${element}-${level}`),
    )),
  }
}

export function validateDeck(deck: PlayerDeckList) {
  const errors: string[] = []
  const counts = new Map<string, number>()
  let levelTotal = 0
  for (const card of deck.cards) {
    const match = /^(metal|wood|water|fire|earth)-([1-5])$/.exec(card)
    if (!match) {
      errors.push(`未知卡牌：${card}`)
      continue
    }
    const level = Number(match[2])
    levelTotal += level
    counts.set(card, (counts.get(card) ?? 0) + 1)
  }
  if (deck.cards.length !== 60) errors.push(`牌組必須正好 60 張，目前為 ${deck.cards.length} 張`)
  if (levelTotal > 170) errors.push(`等級總和不得超過 170，目前為 ${levelTotal}`)
  for (const [card, count] of counts) {
    const maximum = Number(card.at(-1)) <= 3 ? 4 : 3
    if (count > maximum) errors.push(`${card} 最多 ${maximum} 張，目前為 ${count} 張`)
  }
  return {
    valid: errors.length === 0,
    cardCount: deck.cards.length,
    levelTotal,
    errors,
  }
}

export function effectiveDeck(deck: PlayerDeckList | undefined) {
  return deck && validateDeck(deck).valid
    ? { deck, source: 'custom' as const }
    : { deck: preconstructedDeck(), source: 'preconstructed' as const }
}

export async function customDeckForUser(
  event: H3Event,
  userId: string,
): Promise<PlayerDeckList | undefined> {
  const row = await database(event)
    .select()
    .from(schema.playerDeck)
    .where(eq(schema.playerDeck.userId, userId))
    .get()

  if (!row) return undefined

  try {
    const cards = JSON.parse(row.cardsJson)
    return Array.isArray(cards) && cards.every(card => typeof card === 'string')
      ? { name: row.name, cards }
      : undefined
  } catch {
    return undefined
  }
}

export async function effectiveDeckForUser(event: H3Event, userId: string) {
  return effectiveDeck(await customDeckForUser(event, userId))
}

export async function saveCustomDeck(
  event: H3Event,
  userId: string,
  deck: PlayerDeckList,
) {
  const validation = validateDeck(deck)
  if (!validation.valid) {
    throw createError({
      statusCode: 400,
      statusMessage: validation.errors.join('；'),
    })
  }

  const now = new Date()
  await database(event)
    .insert(schema.playerDeck)
    .values({
      userId,
      name: deck.name.trim() || '我的牌組',
      cardsJson: JSON.stringify(deck.cards),
      createdAt: now,
      updatedAt: now,
    })
    .onConflictDoUpdate({
      target: schema.playerDeck.userId,
      set: {
        name: deck.name.trim() || '我的牌組',
        cardsJson: JSON.stringify(deck.cards),
        updatedAt: now,
      },
    })
}

export async function deleteCustomDeck(event: H3Event, userId: string) {
  await database(event)
    .delete(schema.playerDeck)
    .where(eq(schema.playerDeck.userId, userId))
}
