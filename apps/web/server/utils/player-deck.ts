import { drizzle } from 'drizzle-orm/d1'
import { eq } from 'drizzle-orm'
import { createError, type H3Event } from 'h3'
import type { PlayerDeckList } from '../../shared/game-room'
import type { PersonalDeckResolution } from '../../app/types/fewfc'
import { schema } from '../database/schema'
import { workerEnv } from './worker-env'
import { rulesEngine } from './rules-engine'

interface PlayerDeckStatement {
  run(): Promise<unknown>
}

interface PlayerDeckDatabase {
  prepare(query: string): PlayerDeckStatement
}

function rawDatabase(event: H3Event): PlayerDeckDatabase {
  return workerEnv(event).DB as PlayerDeckDatabase
}

function database(event: H3Event) {
  return drizzle(workerEnv(event).DB as Parameters<typeof drizzle>[0], { schema })
}

async function ensurePlayerDeckTable(event: H3Event) {
  await rawDatabase(event)
    .prepare(`
      CREATE TABLE IF NOT EXISTS player_deck (
        user_id text PRIMARY KEY NOT NULL,
        name text NOT NULL,
        cards_json text NOT NULL,
        created_at integer NOT NULL,
        updated_at integer NOT NULL,
        FOREIGN KEY (user_id) REFERENCES user(id) ON UPDATE no action ON DELETE cascade
      )
    `)
    .run()
}

function validationPresentation(validation: PersonalDeckResolution['effectiveValidation']) {
  return {
    valid: validation.valid,
    cardCount: validation.cardCount,
    levelTotal: validation.levelTotal,
    errors: validation.issues.map(issue => issue.code),
  }
}

export async function resolveDeck(
  userId: string,
  candidate?: PlayerDeckList,
) {
  const resolved = await rulesEngine().resolvePersonalDeck(userId, candidate)
  return {
    deck: {
      name: resolved.effective.name,
      cards: resolved.effective.cards,
    },
    source: resolved.source,
    validation: validationPresentation(resolved.effectiveValidation),
  }
}

export async function customDeckForUser(
  event: H3Event,
  userId: string,
): Promise<PlayerDeckList | undefined> {
  await ensurePlayerDeckTable(event)

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
  return await resolveDeck(userId, await customDeckForUser(event, userId))
}

export async function saveCustomDeck(
  event: H3Event,
  userId: string,
  deck: PlayerDeckList,
) {
  const resolved = await rulesEngine().resolvePersonalDeck(userId, deck)
  if (!resolved.candidateValidation?.valid) {
    throw createError({
      statusCode: 400,
      statusMessage: resolved.candidateValidation?.issues.map(issue => issue.code).join('；')
        || 'Invalid deck list.',
    })
  }

  await ensurePlayerDeckTable(event)

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
  await ensurePlayerDeckTable(event)

  await database(event)
    .delete(schema.playerDeck)
    .where(eq(schema.playerDeck.userId, userId))
}
