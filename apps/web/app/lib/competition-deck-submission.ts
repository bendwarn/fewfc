import type { RegistrationAction } from '../../shared/competition'
import type { DeckCompositionCardDefinition, RulesCatalog } from '../types/fewfc'
import { formatCompetitionDeadline } from './competition-registration'
import {
  PLAYER_DECK_ELEMENT_ORDER,
  PLAYER_DECK_LEVEL_ORDER,
  createDeckCompositionPolicy,
  type DeckValidation,
  type PlayerDeckList,
} from './player-deck'

export type DeckSubmissionLifecycle = 'draft' | 'open' | 'locked'
export type DeckSubmissionPolicy = 'disabled' | 'required'
export type DeckSubmissionTeamStatus = 'registered' | 'checkedIn' | 'withdrawn' | 'notCheckedIn'
export type SubmitDeckAction = Extract<RegistrationAction, { type: 'submitDeck' }>

export interface DeckSubmissionTeam {
  teamId: string
  rosterVersion: number
  members: readonly string[]
  status: DeckSubmissionTeamStatus
}

export interface SavedAccountDeck {
  deck: PlayerDeckList
  source: 'custom' | 'preconstructed'
  validation: {
    valid: true
    cardCount: number
    levelTotal: number
    errors: string[]
  }
}

export class DeckSubmissionProtocolError extends Error {
  constructor() {
    super('帳號牌組或牌組規則資料格式無法辨識，已停止提交。')
    this.name = 'DeckSubmissionProtocolError'
  }
}

export class DeckSubmissionEligibilityError extends Error {
  constructor(message: string) {
    super(message)
    this.name = 'DeckSubmissionEligibilityError'
  }
}

export function parseSavedAccountDeckResponse(
  input: unknown,
  expectedCardCount: number,
): SavedAccountDeck {
  if (!Number.isSafeInteger(expectedCardCount) || expectedCardCount < 1) {
    throw new DeckSubmissionProtocolError()
  }
  const response = asRecord(input)
  if (!response || !hasExactKeys(response, ['deck', 'source', 'validation'])) {
    throw new DeckSubmissionProtocolError()
  }
  const source = response.source
  if (source !== 'custom' && source !== 'preconstructed') throw new DeckSubmissionProtocolError()

  const deckRecord = asRecord(response.deck)
  if (!deckRecord || !hasExactKeys(deckRecord, ['name', 'cards'])) {
    throw new DeckSubmissionProtocolError()
  }
  const name = deckRecord.name
  const cards = deckRecord.cards
  if (typeof name !== 'string' || !name.trim()
    || !isNonEmptyStringArray(cards) || cards.length !== expectedCardCount) {
    throw new DeckSubmissionProtocolError()
  }

  const validationRecord = asRecord(response.validation)
  if (!validationRecord || !hasExactKeys(validationRecord, ['valid', 'cardCount', 'levelTotal', 'errors'])) {
    throw new DeckSubmissionProtocolError()
  }
  const valid = validationRecord.valid
  const cardCount = validationRecord.cardCount
  const levelTotal = validationRecord.levelTotal
  const errors = validationRecord.errors
  if (valid !== true || !isNonNegativeInteger(cardCount) || cardCount !== cards.length
    || !isNonNegativeInteger(levelTotal) || !isStringArray(errors) || errors.length !== 0) {
    throw new DeckSubmissionProtocolError()
  }

  return {
    deck: { name, cards: [...cards] },
    source,
    validation: {
      valid: true,
      cardCount,
      levelTotal,
      errors: [],
    },
  }
}

export function validateSavedAccountDeck(
  savedDeck: SavedAccountDeck,
  composition: RulesCatalog['deckComposition'],
): DeckValidation {
  assertSafeDeckComposition(composition)
  const policy = createDeckCompositionPolicy(composition)
  const hasCompleteDisplayGrid = PLAYER_DECK_ELEMENT_ORDER.length * PLAYER_DECK_LEVEL_ORDER.length === 25
    && PLAYER_DECK_ELEMENT_ORDER.every(element => PLAYER_DECK_LEVEL_ORDER.every((level) => {
      return Boolean(policy.definition(element, level))
    }))
  if (!hasCompleteDisplayGrid) throw new DeckSubmissionProtocolError()

  const validation = policy.validate(savedDeck.deck)
  if (!validation.valid
    || validation.cardCount !== savedDeck.validation.cardCount
    || validation.levelTotal !== savedDeck.validation.levelTotal) {
    throw new DeckSubmissionProtocolError()
  }
  return validation
}

export interface BuildDeckSubmissionActionInput {
  currentUserId: string | null
  activeTeam: DeckSubmissionTeam | null
  lifecycle: DeckSubmissionLifecycle
  personalDecks: DeckSubmissionPolicy
  savedDeck: SavedAccountDeck
  composition: RulesCatalog['deckComposition']
  acknowledgeCheckedInReset: boolean
}

export function buildDeckSubmissionAction(
  input: BuildDeckSubmissionActionInput,
): SubmitDeckAction {
  if (!input.currentUserId?.trim()) {
    throw new DeckSubmissionEligibilityError('請先登入後再提交本人牌組。')
  }
  if (input.personalDecks !== 'required') {
    throw new DeckSubmissionEligibilityError('這場賽事不接受個人牌組。')
  }
  if (input.lifecycle !== 'open') {
    throw new DeckSubmissionEligibilityError('目前賽事尚未開放牌組提交。')
  }

  const team = input.activeTeam
  if (!team || !isAvailableTeamStatus(team.status)
    || typeof team.teamId !== 'string' || !team.teamId.trim()
    || !Number.isSafeInteger(team.rosterVersion) || team.rosterVersion < 1
    || !Array.isArray(team.members) || !team.members.every(isNonEmptyString)) {
    throw new DeckSubmissionEligibilityError('目前沒有可用的有效隊伍。')
  }
  if (!team.members.includes(input.currentUserId)) {
    throw new DeckSubmissionEligibilityError('只能為目前登入者所屬的有效隊伍提交牌組。')
  }
  if (team.status === 'checkedIn' && !input.acknowledgeCheckedInReset) {
    throw new DeckSubmissionEligibilityError('重新提交會取消隊長資料完成確認，請先確認你了解必須由隊長再次完成資料確認。')
  }

  validateSavedAccountDeck(input.savedDeck, input.composition)
  const cards = [...input.savedDeck.deck.cards]
  const action: SubmitDeckAction = {
    type: 'submitDeck',
    teamId: team.teamId,
    rosterVersion: team.rosterVersion,
    name: input.savedDeck.deck.name.trim(),
    cards,
  }
  if (!action.name) throw new DeckSubmissionProtocolError()

  Object.freeze(action.cards)
  Object.freeze(action)
  return action
}

export interface DeckCompositionSlot {
  key: string
  name: string
  element: DeckCompositionCardDefinition['element']
  level: number
  count: number
}

export function savedDeckCompositionSlots(
  savedDeck: SavedAccountDeck,
  composition: RulesCatalog['deckComposition'],
): DeckCompositionSlot[] {
  validateSavedAccountDeck(savedDeck, composition)
  const policy = createDeckCompositionPolicy(composition)
  const counts = policy.counts(savedDeck.deck)
  let index = 0
  return PLAYER_DECK_ELEMENT_ORDER.flatMap(element => PLAYER_DECK_LEVEL_ORDER.map((level) => {
    const definition = policy.definition(element, level)
    const count = counts[index]
    index += 1
    if (!definition || count === undefined) throw new DeckSubmissionProtocolError()
    return {
      key: definition.id,
      name: definition.name,
      element,
      level,
      count,
    }
  }))
}

export function formatDeckSubmissionDeadline(timestamp: number): string {
  return formatCompetitionDeadline(timestamp)
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null
}

function hasExactKeys(value: Record<string, unknown>, keys: readonly string[]): boolean {
  const actual = Object.keys(value)
  return actual.length === keys.length && keys.every(key => Object.hasOwn(value, key))
}

function isNonEmptyString(value: unknown): value is string {
  return typeof value === 'string' && value.trim().length > 0
}

function isNonEmptyStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(isNonEmptyString)
}

function isStringArray(value: unknown): value is string[] {
  return Array.isArray(value) && value.every(item => typeof item === 'string')
}

function isAvailableTeamStatus(value: unknown): value is Exclude<DeckSubmissionTeamStatus, 'withdrawn'> {
  return value === 'registered' || value === 'checkedIn' || value === 'notCheckedIn'
}

function isNonNegativeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value >= 0
}

function assertSafeDeckComposition(value: unknown): asserts value is RulesCatalog['deckComposition'] {
  const composition = asRecord(value)
  const personalDeck = asRecord(composition?.personalDeck)
  const definitions = composition?.cardDefinitions
  if (!personalDeck || !Array.isArray(definitions)
    || !isPositiveSafeInteger(personalDeck.exactCardCount)
    || !isNonNegativeInteger(personalDeck.maximumLevelTotal)
    || definitions.length !== PLAYER_DECK_ELEMENT_ORDER.length * PLAYER_DECK_LEVEL_ORDER.length) {
    throw new DeckSubmissionProtocolError()
  }

  const ids = new Set<string>()
  const cells = new Set<string>()
  for (const value of definitions) {
    const definition = asRecord(value)
    if (!definition
      || !isNonEmptyString(definition.id)
      || !isNonEmptyString(definition.name)
      || !isDeckElement(definition.element)
      || !isDeckLevel(definition.level)
      || !isNonNegativeInteger(definition.personalDeckCopyLimit)) {
      throw new DeckSubmissionProtocolError()
    }

    const cell = `${definition.element}:${definition.level}`
    if (ids.has(definition.id) || cells.has(cell)) throw new DeckSubmissionProtocolError()
    ids.add(definition.id)
    cells.add(cell)
  }

  if (cells.size !== PLAYER_DECK_ELEMENT_ORDER.length * PLAYER_DECK_LEVEL_ORDER.length) {
    throw new DeckSubmissionProtocolError()
  }
}

function isPositiveSafeInteger(value: unknown): value is number {
  return typeof value === 'number' && Number.isSafeInteger(value) && value > 0
}

function isDeckElement(value: unknown): value is DeckCompositionCardDefinition['element'] {
  return PLAYER_DECK_ELEMENT_ORDER.some(element => element === value)
}

function isDeckLevel(value: unknown): value is typeof PLAYER_DECK_LEVEL_ORDER[number] {
  return PLAYER_DECK_LEVEL_ORDER.some(level => level === value)
}
