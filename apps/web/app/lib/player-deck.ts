import type { Element, RulesCatalog } from '../types/fewfc'

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

export function createDeckCompositionPolicy(
  composition: RulesCatalog['deckComposition'],
) {
  const definitions = composition.cardDefinitions
  const byId = new Map(definitions.map(definition => [definition.id, definition]))
  const elements = [...new Set(definitions.map(definition => definition.element))]
  const levels = [...new Set(definitions.map(definition => definition.level))]

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
      return definitions.find(candidate => candidate.element === element && candidate.level === level)
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
