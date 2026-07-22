import { expect, test } from 'bun:test'
import type {
  PlayerFacingActionDetail,
  PlayableAction,
  ProfessionAbilityEffect,
  RuleConsequence,
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
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'useCards', cards: [1, 2] } },
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'discardCards', cards: [3] } },
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'spendSpiritPower', amount: 2 } },
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'loseHp', amount: 6 } },
  { type: 'cost', certainty: 'conditional', cost: { type: 'optionalDiscardByPrintedElement', allowedPrintedElements: ['Metal', 'Earth'] } },
  { type: 'cost', certainty: 'guaranteed', cost: { type: 'consumePouch', sourceCard: 4 } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'previousPlayer', category: 'elemental', points: { type: 'fixed', value: 10 } } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'nextPlayer', category: 'physical', points: { type: 'formula', formula: { type: 'levelPlus', amount: 4 } } } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'selfTeam', category: 'special', points: { type: 'formula', formula: { type: 'levelSumTimes', multiplier: 3 } } } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'allPlayers', category: 'special', points: { type: 'formula', formula: { type: 'targetHandCountTimes', multiplier: 15 } } } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'attack', target: 'eachTeam', category: 'special', points: { type: 'formula', formula: { type: 'elementProductTimes', element: 'Fire', multiplier: 5 } } } },
  ...(['coverCounter', 'copyPreviousTurnFormation', 'recoverHp', 'reduceShield', 'inspectHand', 'createShield', 'returnTeamHp', 'drawCards', 'swapTeamHp', 'summonSpirit', 'clearEnvironment', 'applyStatus', 'changeEnvironment', 'breakProfession', 'limitedUseRecovery', 'resolveMelodyMainEffect', 'beginChainChoice', 'shatterSpirits', 'breakStars', 'damageEachTeamBy15', 'applyGaleRain', 'reduceEveryShieldBy20', 'attackIncreasesTo80IfShieldReduced', 'chooseEnvironmentAndRequireMatchingCardOrRevealHand', 'revealTopEightDiscardLevelThreeOrHigherThenShuffle', 'transferEnvironmentToUsedElement'] as const)
    .map(effect => ({ type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'resolveFormationEffect', effect } }) as const),
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'changeProfession', professionId: 'hero:warrior' } },
  ...([
    { type: 'damagePreviousTeamByCardLevelTimes', multiplier: 2 },
    { type: 'increaseTurnDraw', amount: 2 },
    { type: 'drawThreeThenChooseOne' },
    { type: 'createVirtualFormationCard', scope: 'baseFormation' },
    { type: 'applyYangAura' }, { type: 'prepareFormationDrawBonus' },
    { type: 'prepareCardWithLevelBonus', amount: 2, maximum: 5 }, { type: 'prepareMeteorEffect' },
    { type: 'drawTwoThenReturnOne' }, { type: 'retrievePreviousPlayerDiscardForProfessionUse' },
    { type: 'retrievePreviousPlayerDiscard' }, { type: 'revealDeckTopAndChooseDiscard' },
    { type: 'applyShuffleRecovery' }, { type: 'prepareCardAtDeclaredLevel' },
  ] satisfies ProfessionAbilityEffect[]).map(effect => ({ type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'activateProfessionAbility', abilityId: 'example', effect } }) as const),
  ...([
    { type: 'damagePreviousTeam', amount: 10 }, { type: 'recoverOwnTeam', amount: 10 },
    { type: 'discardSelectedCardAndIncreaseTurnDraw', amount: 1 }, { type: 'increaseTurnDraw', amount: 1 },
    { type: 'interpretSelectedCardLevel' }, { type: 'protectNextPlayerFromAttack' },
    { type: 'setOwnShield', amount: 40 }, { type: 'inspectRandomNextPlayerHandCards', count: 2 },
    { type: 'discardNextPlayerDeckAndDamageByHighestLevel', count: 4, multiplier: 4 },
  ] satisfies SpiritSkillEffect[]).map(effect => ({ type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'useSpiritSkill', effect } }) as const),
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'triggerSecretStrategy', effect: 'protectTriggeringPlayer' } },
  { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'movePreviousTurnDiscardToDeckTop', card: 5, previousPlayer: 'bob' } },
  ...(['selectPlayer', 'selectFormation', 'selectMelody', 'selectDeckCard', 'selectEnvironment', 'selectPouchOwnerAndOptionalStrategy'] as const)
    .map(type => ({ type: 'followUpChoice', certainty: 'followUp', choice: { type } }) as const),
  { type: 'followUpChoice', certainty: 'followUp', choice: { type: 'selectSecretStrategyInput', input: 'deckDiscardSwap' } },
  { type: 'followUpChoice', certainty: 'followUp', choice: { type: 'selectCards', minimum: 1, maximum: 2 } },
  { type: 'trustedRandomness', certainty: 'random', operation: 'shuffleDeck' },
  { type: 'trustedRandomness', certainty: 'random', operation: 'shuffleDiscardIntoDeck' },
  { type: 'trustedRandomness', certainty: 'random', operation: { type: 'selectHiddenHandCards', count: 2 } },
  { type: 'delayedEffect', certainty: 'scheduled', timing: 'nextTurnStart', effect: 'repeatMelodyMainEffect' },
  { type: 'delayedEffect', certainty: 'scheduled', timing: 'nextPlayerTurn', effect: 'selectAndPerformMelodyMainEffect' },
  ...(['doesNotEndAction', 'doesNotCreateFormationUse', 'doesNotScheduleAnotherEcho', 'ignoresOtherFormationEffects', 'effectMayBeIneffective', 'usesPrintedElement'] as const)
    .map(type => ({ type: 'ruleException', certainty: 'guaranteed', exception: { type } }) as const),
  { type: 'ruleException', certainty: 'guaranteed', exception: { type: 'limitedUse', key: 'tailwind', remaining: 0, maximum: 1 } },
  { type: 'substitution', certainty: 'guaranteed', card: 6, printedElement: 'Metal', interpretedElement: 'Earth' },
  { type: 'declaredInput', certainty: 'guaranteed', input: { type: 'card', card: 7 } },
  { type: 'declaredInput', certainty: 'guaranteed', input: { type: 'element', element: 'Water' } },
  { type: 'declaredInput', certainty: 'guaranteed', input: { type: 'level', level: 3 } },
  { type: 'declaredInput', certainty: 'guaranteed', input: { type: 'targetCard', card: 8 } },
]

test('exhaustively presents typed RuleConsequence fixtures without formation or strategy prose maps', () => {
  const detail: PlayerFacingActionDetail = { consequences: everyConsequence }
  const presentation = presentActionDetail(detail, card => `卡${card}`, player => ({ bob: '上家' })[player] ?? player)

  expect(presentation).toContain('卡6（金行牌）視為土行牌')
  expect(presentation).toContain('於自己下次回合開始再次執行此曲調主效果')
  expect(presentation).toContain('由受信任的隨機程序選出 2 張隱藏手牌')
  expect(presentation).toContain('本局剩餘 0/1 次使用')
  expect(presentation).toContain('已排定：於下位玩家下次回合選擇並執行一個曲調主效果')
})

test('presents Echo, Chain, direct Pouch, and Discard Retrieval through their typed details', () => {
  const echoDetail: PlayerFacingActionDetail = {
    consequences: [
      { type: 'cost', certainty: 'guaranteed', cost: { type: 'useCards', cards: [1, 2] } },
      { type: 'cost', certainty: 'conditional', cost: { type: 'optionalDiscardByPrintedElement', allowedPrintedElements: ['Metal', 'Earth'] } },
      { type: 'delayedEffect', certainty: 'conditional', timing: 'nextTurnStart', effect: 'repeatMelodyMainEffect' },
      { type: 'ruleException', certainty: 'conditional', exception: { type: 'doesNotCreateFormationUse' } },
      { type: 'ruleException', certainty: 'conditional', exception: { type: 'doesNotScheduleAnotherEcho' } },
    ],
  }
  const action: PlayableAction = {
    type: 'performFormation', id: 'echo:ringing-metal', name: '商調‧鳴金', category: 'Spell', detail: echoDetail,
    cards: [1, 2], starSubstitution: null, matchOption: null,
  }
  expect(presentPlayableAction(action, card => `牌${card}`)).toContain('可額外捨棄一張印刷行屬為 金行牌或土行牌')
  expect(presentPlayableAction(action)).toContain('不視為新的陣法施展')

  const pouch: SecretStrategyOption = {
    sourceCard: 8, strategy: 'SheepStealing', input: 'deckDiscardSwap', targetPlayers: [], stars: [], breakStars: [],
    deckCards: [], discardCards: [], handCards: [], requiredCardCount: 2,
    detail: { consequences: [
      { type: 'cost', certainty: 'guaranteed', cost: { type: 'consumePouch', sourceCard: 8 } },
      { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'triggerSecretStrategy', effect: 'swapDeckAndDiscard' } },
      { type: 'followUpChoice', certainty: 'followUp', choice: { type: 'selectSecretStrategyInput', input: 'deckDiscardSwap' } },
      { type: 'trustedRandomness', certainty: 'random', operation: 'shuffleDeck' },
    ] },
  }
  expect(presentSecretStrategyOption(pouch)).toContain('各選兩張牌交換牌組與棄牌堆')
  expect(presentSecretStrategyOption(pouch)).toContain('隨機決定：由受信任的隨機程序洗牌')

  expect(presentDiscardRetrievalAction({ detail: { consequences: [
    { type: 'cost', certainty: 'guaranteed', cost: { type: 'loseHp', amount: 6 } },
    { type: 'immediateEffect', certainty: 'guaranteed', effect: { type: 'movePreviousTurnDiscardToDeckTop', card: 3, previousPlayer: 'bob' } },
    { type: 'ruleException', certainty: 'guaranteed', exception: { type: 'doesNotEndAction' } },
  ] } }, player => ({ bob: '對手' })[player] ?? player, card => `牌${card}`)).toBe('支付 6 點生命。將對手上回合捨棄的牌3放到自己的牌組頂。不結束行動階段。')
})
