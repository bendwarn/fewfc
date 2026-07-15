import type {
  DeckCompositionCardDefinition,
  Element,
  RulesCatalog,
} from '../types/fewfc'

export interface PlayerDeckList {
  name: string
  cards: string[]
}

export interface DeckValidation {
  valid: boolean
  cardCount: number
  levelTotal: number
  errors: string[]
}

export const PLAYER_DECK_ELEMENT_ORDER: readonly Element[] = [
  'Metal',
  'Wood',
  'Water',
  'Fire',
  'Earth',
]
export const PLAYER_DECK_LEVEL_ORDER = [1, 2, 3, 4, 5] as const
export const PLAYER_DECK_MATRIX_SIZE = PLAYER_DECK_ELEMENT_ORDER.length * PLAYER_DECK_LEVEL_ORDER.length

export type PlayerDeckImportResult =
  | { counts: number[]; error: null }
  | { counts: null; error: string }

function playerDeckDefinition(
  definitions: readonly DeckCompositionCardDefinition[],
  element: Element,
  level: number,
) {
  return definitions.find(candidate => candidate.element === element && candidate.level === level)
}

/**
 * Parses the compact deck-editor format without applying any deck-rule
 * validation. A syntactically valid import may intentionally be an invalid
 * deck draft, so the existing deck validation can explain the rule errors.
 */
export function parsePlayerDeckCounts(input: string): PlayerDeckImportResult {
  const trimmed = input.trim()
  if (!trimmed) {
    return { counts: null, error: '請輸入 25 個 0 至 9 的單位數字。' }
  }

  if (/^\d+$/.test(trimmed) && !/^\d{25}$/.test(trimmed)) {
    return {
      counts: null,
      error: '未分隔的內容必須剛好是 25 位數字。',
    }
  }

  if (/^\d{25}$/.test(trimmed)) {
    return { counts: [...trimmed].map(Number), error: null }
  }

  const values = trimmed.split(/[ \t\r\n]+/)
  if (!values.every(value => /^[0-9]$/.test(value))) {
    return {
      counts: null,
      error: '每個值只能是一個 0 至 9 的十進位數字，並以空白、Tab 或換行分隔。',
    }
  }
  if (values.length !== PLAYER_DECK_MATRIX_SIZE) {
    return {
      counts: null,
      error: `必須輸入剛好 ${PLAYER_DECK_MATRIX_SIZE} 個數字，目前為 ${values.length} 個。`,
    }
  }

  return { counts: values.map(Number), error: null }
}

export function expandPlayerDeckCounts(
  counts: readonly number[],
  definitions: readonly DeckCompositionCardDefinition[],
): string[] {
  if (counts.length !== PLAYER_DECK_MATRIX_SIZE) {
    throw new Error(`Expected ${PLAYER_DECK_MATRIX_SIZE} deck counts`)
  }

  return counts.flatMap((count, index) => {
    const element = PLAYER_DECK_ELEMENT_ORDER[Math.floor(index / PLAYER_DECK_LEVEL_ORDER.length)]
    const level = PLAYER_DECK_LEVEL_ORDER[index % PLAYER_DECK_LEVEL_ORDER.length]
    if (!element || !level) {
      throw new Error(`Invalid deck count index ${index}`)
    }
    const definition = playerDeckDefinition(definitions, element, level)
    if (!definition) {
      throw new Error(`Missing card definition for ${element} level ${level}`)
    }
    return Array.from({ length: count }, () => definition.id)
  })
}

export function playerDeckCounts(
  deck: Pick<PlayerDeckList, 'cards'>,
  definitions: readonly DeckCompositionCardDefinition[],
): number[] {
  return PLAYER_DECK_ELEMENT_ORDER.flatMap(element => (
    PLAYER_DECK_LEVEL_ORDER.map((level) => {
      const definition = playerDeckDefinition(definitions, element, level)
      return definition ? deck.cards.filter(card => card === definition.id).length : 0
    })
  ))
}

export function serializePlayerDeckCounts(counts: readonly number[]): string {
  if (counts.length !== PLAYER_DECK_MATRIX_SIZE) {
    throw new Error(`Expected ${PLAYER_DECK_MATRIX_SIZE} deck counts`)
  }

  return Array.from(
    { length: PLAYER_DECK_ELEMENT_ORDER.length },
    (_, row) => counts
      .slice(row * PLAYER_DECK_LEVEL_ORDER.length, (row + 1) * PLAYER_DECK_LEVEL_ORDER.length)
      .join('\t'),
  ).join('\n')
}

export function createDeckCompositionPolicy(
  composition: RulesCatalog['deckComposition'],
) {
  const definitions = composition.cardDefinitions
  const byId = new Map(definitions.map(definition => [definition.id, definition]))
  const elements = PLAYER_DECK_ELEMENT_ORDER
  const levels = PLAYER_DECK_LEVEL_ORDER

  const validate = (deck: PlayerDeckList): DeckValidation => {
    const errors: string[] = []
    const counts = new Map<string, number>()
    let levelTotal = 0

    for (const card of deck.cards) {
      const definition = byId.get(card)
      if (!definition) {
        errors.push(`未知卡牌：${card}`)
        continue
      }
      levelTotal += definition.level
      counts.set(card, (counts.get(card) ?? 0) + 1)
    }
    if (deck.cards.length !== composition.personalDeck.exactCardCount) {
      errors.push(
        `牌組必須正好 ${composition.personalDeck.exactCardCount} 張，目前為 ${deck.cards.length} 張`,
      )
    }
    if (levelTotal > composition.personalDeck.maximumLevelTotal) {
      errors.push(
        `等級總和不得超過 ${composition.personalDeck.maximumLevelTotal}，目前為 ${levelTotal}`,
      )
    }
    for (const [card, count] of counts) {
      const maximum = byId.get(card)?.personalDeckCopyLimit ?? 0
      if (count > maximum) errors.push(`${card} 最多 ${maximum} 張，目前為 ${count} 張`)
    }

    return {
      valid: errors.length === 0,
      cardCount: deck.cards.length,
      levelTotal,
      errors,
    }
  }

  return {
    definitions,
    elements,
    levels,
    exactCardCount: composition.personalDeck.exactCardCount,
    maximumLevelTotal: composition.personalDeck.maximumLevelTotal,
    definition(element: Element, level: number) {
      return playerDeckDefinition(definitions, element, level)
    },
    counts(deck: Pick<PlayerDeckList, 'cards'>) {
      return playerDeckCounts(deck, definitions)
    },
    deckFromCounts(name: string, counts: readonly number[]): PlayerDeckList {
      return {
        name,
        cards: expandPlayerDeckCounts(counts, definitions),
      }
    },
    preconstructedDeck(): PlayerDeckList {
      return {
        name: composition.personalDeck.preconstructed.name,
        cards: [...composition.personalDeck.preconstructed.cards],
      }
    },
    validate,
    effectiveDeck(deck: PlayerDeckList | undefined) {
      return deck && validate(deck).valid
        ? { deck, source: 'custom' as const }
        : { deck: this.preconstructedDeck(), source: 'preconstructed' as const }
    },
  }
}
