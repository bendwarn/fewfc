import { expect, test } from 'bun:test'
import type {
  PlayerFacingActionDetail,
  PlayableAction,
  ProfessionAbilityEffect,
  RuleConsequence,
  SecretStrategyEffect,
  SecretStrategyOption,
  SpiritSkillEffect,
} from '../types/fewfc'
import {
  presentActionDetail,
  presentDiscardRetrievalAction,
  presentPlayableAction,
  presentSecretStrategyOption,
} from './action-detail-presentation'

const everyConsequence: RuleConsequence[] = [
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'discardSelectedCards' } },
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'spendSpiritPower', amount: 2 } },
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'loseHp', amount: 6 } },
  { type: 'cost', certainty: 'conditional', cost: { type: 'optionalDiscardByPrintedElement', allowedPrintedElements: ['Metal', 'Earth'] } },
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'consumePouch' } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'previousPlayer', category: 'elemental', points: { type: 'fixed', value: 10 } } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'nextPlayer', category: 'physical', points: { type: 'formula', formula: { type: 'levelPlus', amount: 4 } } } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'selfTeam', category: 'special', points: { type: 'formula', formula: { type: 'levelSumTimes', multiplier: 3 } } } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'allPlayers', category: 'special', points: { type: 'formula', formula: { type: 'targetHandCountTimes', multiplier: 15 } } } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'eachTeam', category: 'special', points: { type: 'formula', formula: { type: 'elementProductTimes', element: 'Fire', multiplier: 5 } } } },
  ...(['coverCounter', 'copyPreviousTurnFormation', 'recoverHp', 'reduceShield', 'inspectHand', 'createShield', 'returnTeamHp', 'drawCards', 'swapTeamHp', 'summonSpirit', 'clearEnvironment', 'applyStatus', 'changeEnvironment', 'breakProfession', 'limitedUseRecovery', 'resolveMelodyMainEffect', 'beginChainChoice', 'shatterSpirits', 'breakStars', 'damageEachTeamBy15', 'applyGaleRain', 'reduceEveryShieldBy20', 'attackIncreasesTo80IfShieldReduced', 'chooseEnvironmentAndRequireMatchingCardOrRevealHand', 'revealTopEightDiscardLevelThreeOrHigherThenShuffle', 'transferEnvironmentToUsedElement'] as const)
    .map(effect => ({ type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'resolveFormationEffect', effect } }) as const),
  ...([
    { type: 'damagePreviousTeamByCardLevelTimes', multiplier: 2 },
    { type: 'increaseTurnDraw', amount: 2 },
    { type: 'drawThreeThenChooseOne' },
    { type: 'createVirtualFormationCard', scope: 'baseFormation' },
    { type: 'applyYangAura' },
    { type: 'prepareFormationDrawBonus' },
    { type: 'prepareCardWithLevelBonus', amount: 2, maximum: 5 },
    { type: 'prepareMeteorEffect' },
    { type: 'drawTwoThenReturnOne' },
    { type: 'retrievePreviousPlayerDiscardForProfessionUse' },
    { type: 'retrievePreviousPlayerDiscard' },
    { type: 'revealDeckTopAndChooseDiscard' },
    { type: 'applyShuffleRecovery' },
    { type: 'prepareCardAtDeclaredLevel' },
  ] satisfies ProfessionAbilityEffect[]).map(effect => (
    { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'activateProfessionAbility', effect } } as const
  )),
  ...([
    { type: 'damagePreviousTeam', amount: 10 },
    { type: 'recoverOwnTeam', amount: 10 },
    { type: 'discardSelectedCardAndIncreaseTurnDraw', amount: 1 },
    { type: 'increaseTurnDraw', amount: 1 },
    { type: 'interpretSelectedCardLevel' },
    { type: 'protectNextPlayerFromAttack' },
    { type: 'setOwnShield', amount: 40 },
    { type: 'inspectRandomNextPlayerHandCards', count: 2 },
    { type: 'discardNextPlayerDeckAndDamageByHighestLevel', count: 4, multiplier: 4 },
  ] satisfies SpiritSkillEffect[]).map(effect => (
    { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'useSpiritSkill', effect } } as const
  )),
  ...([
    'protectTriggeringPlayer',
    'increaseHandLevels',
    'increaseTurnDraw',
    'negateNextPlayerFormationHpChanges',
    'suppressPlayerAbilitiesAndSpiritPower',
    'summonSpiritFromPouch',
    'swapDeckAndDiscard',
    'directProfessionChange',
    'breakOrGainStar',
    'clearOrChangeEnvironment',
  ] satisfies SecretStrategyEffect[]).map(effect => (
    { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'triggerSecretStrategy', effect } } as const
  )),
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'movePreviousTurnDiscardToDeckTop' } },
  ...(['selectPlayer', 'selectFormation', 'selectDeckCard', 'selectEnvironment', 'selectPouchOwnerAndOptionalStrategy'] as const)
    .map(type => ({ type: 'followUpChoice', certainty: 'followUp', choice: { type } }) as const),
  { type: 'followUpChoice', certainty: 'followUp', choice: { type: 'selectCards', minimum: 1, maximum: 2 } },
  { type: 'trustedRandomness', certainty: 'random', operation: { type: 'shuffleDeck' } },
  { type: 'trustedRandomness', certainty: 'random', operation: { type: 'shuffleDiscardIntoDeck' } },
  { type: 'trustedRandomness', certainty: 'random', operation: { type: 'selectHiddenHandCards', count: 2 } },
  { type: 'delayedEffect', certainty: 'scheduled', timing: 'nextTurnStart', effect: 'repeatMelodyMainEffect' },
  { type: 'delayedEffect', certainty: 'scheduled', timing: 'nextPlayerTurn', effect: 'selectAndPerformMelodyMainEffect' },
  { type: 'ruleException', certainty: 'guaranteed', exception: { type: 'ignoresOtherFormationEffects' } },
  { type: 'ruleException', certainty: 'guaranteed', exception: { type: 'limitedUse', key: 'tailwind', remaining: 0, maximum: 1 } },
]

test('exhaustively presents the remaining typed action-detail vocabulary', () => {
  const presentation = presentActionDetail({ consequences: everyConsequence })

  expect(presentation).toContain('捨棄所選牌。')
  expect(presentation).toContain('結算主效果後，可捨棄一張金或土屬性的手牌。')
  expect(presentation).toContain('隨機檢視下家 2 張手牌。')
  expect(presentation).toContain('下位玩家下次回合選擇並執行一個曲調主效果。')
  expect(presentation).toContain('本局剩餘 0/1 次使用。')
})

test('allows actions with no contextual supplement', () => {
  expect(presentActionDetail(null)).toBe('')
  expect(presentActionDetail({ consequences: [] })).toBe('')
})

test('composes related Echo facts into reader-facing clauses', () => {
  const detail: PlayerFacingActionDetail = {
    consequences: [
      { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'resolveFormationEffect', effect: 'resolveMelodyMainEffect' } },
      { type: 'cost', certainty: 'conditional', cost: { type: 'optionalDiscardByPrintedElement', allowedPrintedElements: ['Metal', 'Earth'] } },
      { type: 'delayedEffect', certainty: 'conditional', timing: 'nextTurnStart', effect: 'repeatMelodyMainEffect' },
      { type: 'followUpChoice', certainty: 'followUp', choice: { type: 'selectDeckCard' } },
      { type: 'trustedRandomness', certainty: 'random', operation: { type: 'shuffleDeck' } },
    ],
  }
  const action: PlayableAction = {
    type: 'performFormation',
    id: 'echo:ringing-metal',
    name: '商調‧鳴金',
    category: 'Spell',
    detail,
    cards: [1, 2],
    starSubstitution: null,
    matchOption: null,
  }

  expect(presentPlayableAction(action)).toBe(
    '結算此曲調的主效果。結算主效果後，可捨棄一張金或土屬性的手牌；若捨棄，下次回合開始再執行一次此曲調主效果。結算時從牌組選擇一張牌後洗牌。',
  )
})

test('presents Chain, direct Pouch, and Discard Retrieval without repeating their selected inputs', () => {
  expect(presentActionDetail({ consequences: [
    { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'resolveFormationEffect', effect: 'beginChainChoice' } },
    { type: 'followUpChoice', certainty: 'followUp', choice: { type: 'selectPouchOwnerAndOptionalStrategy' } },
  ] })).toBe('結算時選擇錦囊持有者，並可選擇第二張牌觸發秘計。')

  const pouch: SecretStrategyOption = {
    sourceCard: 8,
    strategy: 'SheepStealing',
    input: 'deckDiscardSwap',
    targetPlayers: [],
    stars: [],
    breakStars: [],
    deckCards: [],
    discardCards: [],
    handCards: [],
    requiredCardCount: 2,
    detail: { consequences: [
      { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'triggerSecretStrategy', effect: 'swapDeckAndDiscard' } },
      { type: 'cost', certainty: 'guaranteed', cost: { type: 'consumePouch' } },
    ] },
  }
  expect(presentSecretStrategyOption(pouch)).toBe('從牌組與棄牌堆各選兩張交換，之後洗牌。公開並消耗該錦囊。')

  expect(presentDiscardRetrievalAction({ detail: { consequences: [
    { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'movePreviousTurnDiscardToDeckTop' } },
    { type: 'cost', certainty: 'guaranteed', cost: { type: 'loseHp', amount: 6 } },
  ] } })).toBe('將上回合棄牌放到自己的牌組頂。支付 6 點生命。')
})
