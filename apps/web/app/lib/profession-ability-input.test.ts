import { describe, expect, test } from 'bun:test'
import {
  completeVirtualFormationCardOffer,
  isVirtualFormationCardOffer,
  professionAbilityOfferKey,
  type ProfessionAbilityOffer,
} from './profession-ability-input'

function offer(overrides: Partial<ProfessionAbilityOffer> = {}): ProfessionAbilityOffer {
  return {
    type: 'activateProfessionAbility',
    id: 'illusion',
    name: '幻術',
    detail: null,
    cards: [7, 9],
    targetCard: null,
    declaredElement: null,
    declaredLevel: null,
    inputRequirement: {
      type: 'virtualFormationCard',
      elements: ['Metal', 'Wood', 'Water', 'Fire', 'Earth'],
      levels: [1, 2, 3, 4, 5],
    },
    ...overrides,
  }
}

describe('Profession Ability input', () => {
  test('recognizes a Virtual Formation Card offer without treating it as activated', () => {
    const candidate = offer()

    expect(isVirtualFormationCardOffer(candidate)).toBe(true)
    expect(candidate.declaredElement).toBeNull()
    expect(candidate.declaredLevel).toBeNull()
  })

  test('completes an allowed element and level without changing the offered cost Cards', () => {
    const candidate = offer()
    if (!isVirtualFormationCardOffer(candidate)) throw new Error('expected Virtual Formation Card')

    expect(completeVirtualFormationCardOffer(candidate, 'Fire', 3)).toMatchObject({
      id: 'illusion',
      cards: [7, 9],
      declaredElement: 'Fire',
      declaredLevel: 3,
    })
  })

  test('rejects values outside the engine-provided requirement', () => {
    const candidate = offer({
      inputRequirement: {
        type: 'virtualFormationCard',
        elements: ['Water'],
        levels: [2],
      },
    })
    if (!isVirtualFormationCardOffer(candidate)) throw new Error('expected Virtual Formation Card')

    expect(completeVirtualFormationCardOffer(candidate, 'Water', 2)).not.toBeNull()
    expect(completeVirtualFormationCardOffer(candidate, 'Fire', 2)).toBeNull()
    expect(completeVirtualFormationCardOffer(candidate, 'Water', 3)).toBeNull()
  })

  test('keys a browser-local draft to the offered Ability and selected Cards', () => {
    expect(professionAbilityOfferKey(offer()))
      .toBe('illusion:7-9::virtualFormationCard:Metal,Wood,Water,Fire,Earth:1,2,3,4,5')
    expect(professionAbilityOfferKey(offer({ cards: [7, 10] })))
      .toBe('illusion:7-10::virtualFormationCard:Metal,Wood,Water,Fire,Earth:1,2,3,4,5')
  })
})
