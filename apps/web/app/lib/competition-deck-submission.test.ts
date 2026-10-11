import { describe, expect, test } from 'bun:test'
import type { RulesCatalog } from '../types/fewfc'
import {
  PLAYER_DECK_ELEMENT_ORDER,
  PLAYER_DECK_LEVEL_ORDER,
} from './player-deck'
import {
  DeckSubmissionEligibilityError,
  DeckSubmissionProtocolError,
  buildDeckSubmissionAction,
  formatDeckSubmissionDeadline,
  parseSavedAccountDeckResponse,
  savedDeckCompositionSlots,
  validateSavedAccountDeck,
} from './competition-deck-submission'
import type { BuildDeckSubmissionActionInput } from './competition-deck-submission'
import { formatCompetitionDeadline } from './competition-registration'

const composition: RulesCatalog['deckComposition'] = {
  cardDefinitions: PLAYER_DECK_ELEMENT_ORDER.flatMap(element => PLAYER_DECK_LEVEL_ORDER.map(level => ({
    id: `${element.toLowerCase()}-${level}`,
    name: `${element} ${level}`,
    element,
    level,
    sharedDeckCopies: 4,
    personalDeckCopyLimit: 2,
    preconstructedCopies: 1,
  }))),
  sharedDeck: { exactCardCount: 10 },
  personalDeck: {
    exactCardCount: 2,
    maximumLevelTotal: 4,
    preconstructed: { name: '系統預組', cards: ['metal-1', 'wood-1'] },
  },
}

const response = {
  deck: { name: '測試自訂牌組', cards: ['metal-1', 'wood-1'] },
  source: 'custom',
  validation: { valid: true, cardCount: 2, levelTotal: 2, errors: [] },
}

function createSubmissionInput(
  overrides: Partial<BuildDeckSubmissionActionInput> = {},
): BuildDeckSubmissionActionInput {
  return {
    currentUserId: 'member-a',
    activeTeam: {
      teamId: 'team-a',
      rosterVersion: 4,
      members: ['captain-a', 'member-a'],
      status: 'registered' as const,
    },
    lifecycle: 'open',
    personalDecks: 'required',
    savedDeck: parseSavedAccountDeckResponse(response, 2),
    composition,
    acknowledgeCheckedInReset: false,
    ...overrides,
  }
}

describe('competition deck submission contracts', () => {
  test('shares minute-precision competition deadline formatting with timezone', () => {
    const timestamp = new Date(2026, 8, 30, 13, 14, 15, 6).getTime()
    const formatted = formatDeckSubmissionDeadline(timestamp)
    expect(formatted).toBe(formatCompetitionDeadline(timestamp))
    expect(formatted).not.toMatch(/\d{1,2}:\d{2}:\d{2}/u)
    expect(formatted).not.toContain('006')
    expect(formatDeckSubmissionDeadline(Number.NaN)).toBe('時間無法顯示')
  })

  test('parses only the known own-account deck response and copies card IDs', () => {
    const parsed = parseSavedAccountDeckResponse(response, 2)
    expect(parsed).toEqual({
      deck: { name: '測試自訂牌組', cards: ['metal-1', 'wood-1'] },
      source: 'custom',
      validation: { valid: true, cardCount: 2, levelTotal: 2, errors: [] },
    })
    expect(parsed.deck.cards).not.toBe(response.deck.cards)
    expect(parseSavedAccountDeckResponse({ ...response, source: 'preconstructed' }, 2).source)
      .toBe('preconstructed')
  })

  test('rejects unknown, damaged, invalid, or wrong-sized API projections', () => {
    expect(() => parseSavedAccountDeckResponse({ ...response, serverState: {} }, 2))
      .toThrow(DeckSubmissionProtocolError)
    expect(() => parseSavedAccountDeckResponse({ ...response, source: 'mystery' }, 2))
      .toThrow(DeckSubmissionProtocolError)
    expect(() => parseSavedAccountDeckResponse({
      ...response,
      deck: { name: '損壞牌組', cards: ['metal-1', 12] },
    }, 2)).toThrow(DeckSubmissionProtocolError)
    expect(() => parseSavedAccountDeckResponse({
      ...response,
      validation: { valid: false, cardCount: 2, levelTotal: 2, errors: ['invalid'] },
    }, 2)).toThrow(DeckSubmissionProtocolError)
    expect(() => parseSavedAccountDeckResponse(response, 3)).toThrow(DeckSubmissionProtocolError)
  })

  test('reuses the catalog composition policy for validation and 25-slot summaries', () => {
    const savedDeck = parseSavedAccountDeckResponse(response, 2)
    expect(validateSavedAccountDeck(savedDeck, composition)).toMatchObject({
      valid: true,
      cardCount: 2,
      levelTotal: 2,
    })
    const slots = savedDeckCompositionSlots(savedDeck, composition)
    expect(slots).toHaveLength(25)
    expect(slots.find(slot => slot.key === 'metal-1')).toMatchObject({ name: 'Metal 1', level: 1, count: 1 })

    const corrupt = parseSavedAccountDeckResponse({
      ...response,
      deck: { name: '未知卡牌', cards: ['not-in-catalog', 'wood-1'] },
    }, 2)
    expect(() => validateSavedAccountDeck(corrupt, composition)).toThrow(DeckSubmissionProtocolError)
  })

  test('rejects malformed runtime catalog values before invoking the existing policy', () => {
    const savedDeck = parseSavedAccountDeckResponse(response, 2)
    const expectRejectedComposition = (rawComposition: unknown) => {
      expect(() => buildDeckSubmissionAction(createSubmissionInput({
        savedDeck,
        composition: rawComposition as RulesCatalog['deckComposition'],
      }))).toThrow(DeckSubmissionProtocolError)
    }
    const withPersonalDeckField = (field: string, value: unknown): unknown => ({
      ...composition,
      personalDeck: { ...composition.personalDeck, [field]: value },
    })
    const withDefinitions = (
      update: (definitions: RulesCatalog['deckComposition']['cardDefinitions']) => unknown,
    ): unknown => ({
      ...composition,
      cardDefinitions: update(composition.cardDefinitions.map(definition => ({ ...definition }))),
    })

    for (const invalidValue of ['oops', Number.NaN, Number.POSITIVE_INFINITY, null, -1]) {
      expectRejectedComposition(withPersonalDeckField('maximumLevelTotal', invalidValue))
    }
    for (const invalidValue of ['2', Number.NaN, Number.POSITIVE_INFINITY, null, -1, 0]) {
      expectRejectedComposition(withPersonalDeckField('exactCardCount', invalidValue))
    }
    expectRejectedComposition({ ...composition, cardDefinitions: null })
    expectRejectedComposition(withDefinitions(definitions => definitions.slice(1)))
    expectRejectedComposition(withDefinitions((definitions) => {
      definitions.find(definition => definition.id === 'water-5')!.id = 'earth-5'
      return definitions
    }))
    expectRejectedComposition(withDefinitions((definitions) => {
      definitions[1]!.element = definitions[0]!.element
      definitions[1]!.level = definitions[0]!.level
      return definitions
    }))
    expectRejectedComposition(withDefinitions((definitions) => {
      definitions[0]!.element = 'Air' as RulesCatalog['deckComposition']['cardDefinitions'][number]['element']
      return definitions
    }))
    expectRejectedComposition(withDefinitions((definitions) => {
      definitions[0]!.level = 6
      return definitions
    }))
    for (const invalidValue of ['2', Number.NaN, Number.POSITIVE_INFINITY, null, -1]) {
      expectRejectedComposition(withDefinitions((definitions) => {
        definitions.find(definition => definition.id === 'earth-5')!.personalDeckCopyLimit = invalidValue as number
        return definitions
      }))
    }

    const identityUpdatedCatalog: RulesCatalog['deckComposition'] = {
      ...composition,
      cardDefinitions: composition.cardDefinitions.map(definition => definition.id === 'earth-5'
        ? { ...definition, id: 'earth-five', name: 'Earth Five' }
        : definition),
    }
    expect(buildDeckSubmissionAction(createSubmissionInput({ composition: identityUpdatedCatalog }))).toMatchObject({
      type: 'submitDeck',
      teamId: 'team-a',
      name: response.deck.name,
      cards: response.deck.cards,
    })

    expect(() => validateSavedAccountDeck(savedDeck, undefined as unknown as RulesCatalog['deckComposition']))
      .toThrow('資料格式無法辨識')
  })

  test('builds exact submitDeck action from a frozen copy without caller or time fields', () => {
    const input = createSubmissionInput()
    const action = buildDeckSubmissionAction(input)

    expect(action).toEqual({
      type: 'submitDeck',
      teamId: 'team-a',
      rosterVersion: 4,
      name: '測試自訂牌組',
      cards: ['metal-1', 'wood-1'],
    })
    expect(Object.keys(action).sort()).toEqual(['cards', 'name', 'rosterVersion', 'teamId', 'type'])
    expect(Object.isFrozen(action)).toBe(true)
    expect(Object.isFrozen(action.cards)).toBe(true)
    expect(() => action.cards.push('fire-2')).toThrow()
    expect(JSON.stringify(action)).not.toMatch(/member-a|callerUserId|player|trustedTimeMs/)
  })

  test('allows any team member without prior membership confirmation', () => {
    const action = buildDeckSubmissionAction(createSubmissionInput())
    expect(action.teamId).toBe('team-a')
  })

  test('requires own active-team membership, open lifecycle, and enabled personal decks', () => {
    expect(() => buildDeckSubmissionAction(createSubmissionInput({ currentUserId: 'stranger' })))
      .toThrow(DeckSubmissionEligibilityError)
    expect(() => buildDeckSubmissionAction(createSubmissionInput({ lifecycle: 'locked' })))
      .toThrow(DeckSubmissionEligibilityError)
    expect(() => buildDeckSubmissionAction(createSubmissionInput({ personalDecks: 'disabled' })))
      .toThrow(DeckSubmissionEligibilityError)
    expect(() => buildDeckSubmissionAction(createSubmissionInput({
      activeTeam: { teamId: 'old-team', rosterVersion: 1, members: ['member-a'], status: 'withdrawn' },
    }))).toThrow(DeckSubmissionEligibilityError)
  })

  test('requires explicit acknowledgement before resubmitting for an already checked-in team', () => {
    const activeTeam = {
      teamId: 'team-a',
      rosterVersion: 4,
      members: ['captain-a', 'member-a'],
      status: 'checkedIn' as const,
    }
    expect(() => buildDeckSubmissionAction(createSubmissionInput({ activeTeam })))
      .toThrow('必須由隊長再次完成資料確認')
    expect(buildDeckSubmissionAction(createSubmissionInput({
      activeTeam,
      acknowledgeCheckedInReset: true,
    })).teamId).toBe('team-a')
  })

  test('does not change the emitted snapshot when the saved account deck later changes', () => {
    const savedDeck = parseSavedAccountDeckResponse(response, 2)
    const action = buildDeckSubmissionAction(createSubmissionInput({ savedDeck }))
    savedDeck.deck.cards[0] = 'fire-2'
    savedDeck.deck.name = '之後編輯的牌組'

    expect(action.name).toBe('測試自訂牌組')
    expect(action.cards).toEqual(['metal-1', 'wood-1'])
  })
})

describe('opening-freeze deck submission', () => {
  test('requires reconfirming captain data completion after resubmission without any new room-ready or time payload', () => {
    const activeTeam: BuildDeckSubmissionActionInput['activeTeam'] = {
      teamId: 'team-a', rosterVersion: 4, members: ['captain-a', 'member-a'], status: 'checkedIn',
    }
    expect(() => buildDeckSubmissionAction(createSubmissionInput({ activeTeam })))
      .toThrow('重新提交會取消隊長資料完成確認，請先確認你了解必須由隊長再次完成資料確認。')
    const action = buildDeckSubmissionAction(createSubmissionInput({ activeTeam, acknowledgeCheckedInReset: true }))
    expect(action).toEqual({ type: 'submitDeck', teamId: 'team-a', rosterVersion: 4,
      name: '測試自訂牌組', cards: ['metal-1', 'wood-1'] })
    expect(Object.keys(action).sort()).toEqual(['cards', 'name', 'rosterVersion', 'teamId', 'type'])
    expect(Object.isFrozen(action)).toBe(true)
    expect(Object.isFrozen(action.cards)).toBe(true)
  })
})
