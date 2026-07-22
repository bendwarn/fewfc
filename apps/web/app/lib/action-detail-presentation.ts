import type {
  ActionCost,
  DeclaredInput,
  EffectAmount,
  EffectFormula,
  FollowUpChoice,
  ImmediateEffect,
  PlayerFacingActionDetail,
  PlayableAction,
  ProfessionAbilityEffect,
  RuleConsequence,
  RuleException,
  SecretStrategyOption,
  SpiritSkillEffect,
  TrustedRandomness,
} from '../types/fewfc'

type CardLabel = (card: number) => string
type PlayerLabel = (player: string) => string

const elementLabels = {
  Metal: '金行牌',
  Wood: '木行牌',
  Water: '水行牌',
  Fire: '火行牌',
  Earth: '土行牌',
} as const

const defaultCardLabel: CardLabel = card => `牌 ${card}`
const defaultPlayerLabel: PlayerLabel = player => player

function assertNever(value: never): never {
  throw new Error(`Unhandled action-detail variant: ${JSON.stringify(value)}`)
}

function cards(cards: number[], cardLabel: CardLabel): string {
  return cards.map(cardLabel).join('、')
}

function presentCost(cost: ActionCost, cardLabel: CardLabel): string {
  switch (cost.type) {
    case 'useCards': return `使用 ${cards(cost.cards, cardLabel)}`
    case 'discardCards': return `捨棄 ${cards(cost.cards, cardLabel)}`
    case 'spendSpiritPower': return `消耗 ${cost.amount} 點靈力`
    case 'loseHp': return `支付 ${cost.amount} 點生命`
    case 'optionalDiscardByPrintedElement': return `可額外捨棄一張印刷行屬為 ${cost.allowedPrintedElements.map(element => elementLabels[element]).join('或')}的手牌`
    case 'consumePouch': return `公開並消耗錦囊 ${cardLabel(cost.sourceCard)}`
    default: return assertNever(cost)
  }
}

function presentFormula(formula: EffectFormula): string {
  switch (formula.type) {
    case 'levelPlus': return `牌等級＋${formula.amount}`
    case 'levelSumTimes': return `牌等級總和×${formula.multiplier}`
    case 'targetHandCountTimes': return `目標手牌數×${formula.multiplier}`
    case 'elementProductTimes': return `${elementLabels[formula.element]}數量平方×${formula.multiplier}`
    default: return assertNever(formula)
  }
}

function presentAmount(amount: EffectAmount): string {
  switch (amount.type) {
    case 'fixed': return `${amount.value} 點`
    case 'formula': return presentFormula(amount.formula)
    default: return assertNever(amount)
  }
}

function presentProfessionAbilityEffect(effect: ProfessionAbilityEffect): string {
  switch (effect.type) {
    case 'damagePreviousTeamByCardLevelTimes': return `對上家隊伍造成所用牌等級×${effect.multiplier}的傷害`
    case 'increaseTurnDraw': return `本回合抽牌＋${effect.amount}`
    case 'drawThreeThenChooseOne': return '抽出三張牌後選擇一張保留'
    case 'createVirtualFormationCard': {
      const scope = {
        elementalStrike: '指定行屬的五行擊術', baseFormation: '基礎規則陣法', anyFormation: '任一陣法',
      } satisfies Record<typeof effect.scope, string>
      return `建立可用於${scope[effect.scope]}的虛擬牌`
    }
    case 'applyYangAura': return '獲得一輪內木、火行攻擊傷害減半的狀態'
    case 'prepareFormationDrawBonus': return '本回合合格攻擊時取得抽牌＋1'
    case 'prepareCardWithLevelBonus': return `使指定牌本回合等級＋${effect.amount}（最高${effect.maximum}）`
    case 'prepareMeteorEffect': return '使本回合陣法觸發流星效果'
    case 'drawTwoThenReturnOne': return '抽兩張牌後選擇一張放回牌組頂'
    case 'retrievePreviousPlayerDiscardForProfessionUse': return '取得上家可回收棄牌，且本回合限用於轉職或可適用的職業陣法'
    case 'retrievePreviousPlayerDiscard': return '取得上家可回收棄牌'
    case 'revealDeckTopAndChooseDiscard': return '展示牌組頂牌後選擇是否捨棄'
    case 'applyShuffleRecovery': return '本回合抽牌增加，並在自身洗牌時回復生命'
    case 'prepareCardAtDeclaredLevel': return '使指定牌本回合以宣告等級施展陣法'
    default: return assertNever(effect)
  }
}

function presentSpiritSkillEffect(effect: SpiritSkillEffect): string {
  switch (effect.type) {
    case 'damagePreviousTeam': return `對上家隊伍造成 ${effect.amount} 點傷害`
    case 'recoverOwnTeam': return `自己隊伍回復 ${effect.amount} 點生命`
    case 'discardSelectedCardAndIncreaseTurnDraw': return `捨棄指定牌，本回合抽牌＋${effect.amount}`
    case 'increaseTurnDraw': return `本回合抽牌＋${effect.amount}`
    case 'interpretSelectedCardLevel': return '使指定牌本回合以宣告等級施展陣法'
    case 'protectNextPlayerFromAttack': return '保護下家免受攻擊'
    case 'setOwnShield': return `將自己的防護罩設為 ${effect.amount}`
    case 'inspectRandomNextPlayerHandCards': return `隨機檢視下家 ${effect.count} 張手牌`
    case 'discardNextPlayerDeckAndDamageByHighestLevel': return `捨棄下家牌組頂 ${effect.count} 張牌，並依最高等級×${effect.multiplier}造成傷害`
    default: return assertNever(effect)
  }
}

function presentImmediateEffect(effect: ImmediateEffect, cardLabel: CardLabel, playerLabel: PlayerLabel): string {
  switch (effect.type) {
    case 'attack': {
      const category = { elemental: '五行', physical: '物理', special: '特殊' }[effect.category]
      const target = {
        selfPlayer: '自己', selfTeam: '自己隊伍', previousPlayer: '上家', previousTeam: '上家隊伍',
        nextPlayer: '下家', nextTeam: '下家隊伍', selectedPlayer: '所選玩家', allPlayers: '所有玩家',
        otherPlayers: '所有其他玩家', eachTeam: '每支隊伍',
      }[effect.target]
      return `對${target}進行${category}攻擊（點數：${presentAmount(effect.points)}）`
    }
    case 'resolveFormationEffect': {
      const labels = {
        coverCounter: '覆蓋反制術式，於下一位玩家行動時結算',
        copyPreviousTurnFormation: '複製上家上回合基礎陣法的類別與效果', recoverHp: '回復生命',
        reduceShield: '扣除防護罩', inspectHand: '檢視指定手牌', createShield: '建構防護罩',
        returnTeamHp: '改變隊伍生命', drawCards: '增加本回合抽牌', swapTeamHp: '交換隊伍生命',
        summonSpirit: '召喚或強化精靈', clearEnvironment: '破除環境', applyStatus: '取得狀態',
        changeEnvironment: '轉移環境', breakProfession: '破除職業', limitedUseRecovery: '回復有限使用次數',
        resolveMelodyMainEffect: '結算此曲調的主效果', beginChainChoice: '開始連環的錦囊與秘計選擇',
        shatterSpirits: '削減所有精靈靈力並處理受影響隊伍生命', breakStars: '破除所有星辰並處理受影響隊伍生命',
      } satisfies Record<typeof effect.effect, string>
      return labels[effect.effect]
    }
    case 'changeProfession': return '轉職為此職業'
    case 'activateProfessionAbility': return presentProfessionAbilityEffect(effect.effect)
    case 'useSpiritSkill': return presentSpiritSkillEffect(effect.effect)
    case 'triggerSecretStrategy': {
      const labels = {
        protectTriggeringPlayer: '本回合保護自己不受無法行動、無法抽牌、反制效果與其他玩家的秘計影響',
        increaseHandLevels: '觸發時手牌快照中的每張牌本回合等級＋1',
        increaseTurnDraw: '本回合抽牌＋1',
        negateNextPlayerFormationHpChanges: '下家下個回合由陣法造成的隊伍生命變化無效',
        suppressPlayerAbilitiesAndSpiritPower: '指定玩家一回合內無法使用職業能力與精靈技能，且精靈無法增加靈力',
        summonSpiritFromPouch: '召喚錦囊印刷行屬的精靈，保留可適用的既有靈力',
        swapDeckAndDiscard: '從牌組與棄牌堆各選兩張交換，之後洗牌',
        directProfessionChange: '直接轉職為錦囊印刷行屬對應的一階英雄學派職業',
        breakOrGainStar: '破除現有星辰，或取得本回合有效的指定星辰效果',
        clearOrChangeEnvironment: '破除目前環境，或捨棄手牌將環境轉移為該牌印刷行屬',
      } satisfies Record<typeof effect.effect, string>
      return labels[effect.effect]
    }
    case 'movePreviousTurnDiscardToDeckTop': return `將${playerLabel(effect.previousPlayer)}上回合捨棄的${cardLabel(effect.card)}放到自己的牌組頂`
    default: return assertNever(effect)
  }
}

function presentFollowUp(choice: FollowUpChoice): string {
  switch (choice.type) {
    case 'selectPlayer': return '結算時選擇一名玩家'
    case 'selectFormation': return '結算時選擇一個陣法'
    case 'selectMelody': return '於下次回合開始選擇一種曲調主效果'
    case 'selectDeckCard': return '結算時從牌組選擇一張牌'
    case 'selectPouchOwnerAndOptionalStrategy': return '結算時選擇錦囊持有者，並可選擇第二張牌觸發秘計'
    case 'selectSecretStrategyInput': {
      const labels = {
        none: '不需要額外輸入', targetPlayer: '選擇目標玩家', deckDiscardSwap: '各選兩張牌交換牌組與棄牌堆',
        star: '選擇星辰效果', retreat: '選擇破除環境或捨棄手牌轉移環境',
      } satisfies Record<typeof choice.input, string>
      return `結算時${labels[choice.input]}`
    }
    case 'selectCards': return `結算時選擇 ${choice.minimum} 至 ${choice.maximum} 張牌`
    default: return assertNever(choice)
  }
}

function presentException(exception: RuleException): string {
  switch (exception.type) {
    case 'doesNotEndAction': return '不結束行動階段'
    case 'doesNotCreateFormationUse': return '此後續效果不視為新的陣法施展'
    case 'doesNotScheduleAnotherEcho': return '此後續效果不會再次排定迴響'
    case 'limitedUse': return `本局剩餘 ${exception.remaining}/${exception.maximum} 次使用`
    case 'effectMayBeIneffective': return '仍受適用的規則限制，效果可能無效'
    case 'usesPrintedElement': return '此處判定使用卡牌印刷行屬'
    default: return assertNever(exception)
  }
}

function presentDeclaredInput(input: DeclaredInput, cardLabel: CardLabel): string {
  switch (input.type) {
    case 'card': return `指定 ${cardLabel(input.card)}`
    case 'element': return `宣告 ${elementLabels[input.element]}`
    case 'level': return `宣告 ${input.level} 級`
    case 'targetCard': return `指定牌 ${cardLabel(input.card)}`
    default: return assertNever(input)
  }
}

function presentTrustedRandomness(operation: TrustedRandomness): string {
  if (typeof operation !== 'string') {
    switch (operation.type) {
      case 'selectHiddenHandCards': return `由受信任的隨機程序選出 ${operation.count} 張隱藏手牌`
    }
  }
  switch (operation) {
    case 'shuffleDeck': return '由受信任的隨機程序洗牌'
    case 'shuffleDiscardIntoDeck': return '由受信任的隨機程序洗棄牌堆後重組牌組'
    default: return assertNever(operation)
  }
}

function presentDelayedEffect(timing: 'nextTurnStart' | 'nextPlayerTurn', effect: 'repeatMelodyMainEffect' | 'selectAndPerformMelodyMainEffect'): string {
  const timingText = (() => {
    switch (timing) {
      case 'nextTurnStart': return '自己下次回合開始'
      case 'nextPlayerTurn': return '下位玩家下次回合'
      default: return assertNever(timing)
    }
  })()
  switch (effect) {
    case 'repeatMelodyMainEffect': return `於${timingText}再次執行此曲調主效果`
    case 'selectAndPerformMelodyMainEffect': return `於${timingText}選擇並執行一個曲調主效果`
    default: return assertNever(effect)
  }
}

function qualify(certainty: RuleConsequence['certainty'], text: string): string {
  switch (certainty) {
    case 'guaranteed': return text
    case 'conditional': return `若符合後續條件，${text}`
    case 'random': return `隨機決定：${text}`
    case 'followUp': return `後續選擇：${text}`
    case 'scheduled': return `已排定：${text}`
    default: return assertNever(certainty)
  }
}

function presentConsequence(consequence: RuleConsequence, cardLabel: CardLabel, playerLabel: PlayerLabel): string {
  const text = (() => {
    switch (consequence.type) {
      case 'cost': return presentCost(consequence.cost, cardLabel)
      case 'immediateEffect': return presentImmediateEffect(consequence.effect, cardLabel, playerLabel)
      case 'followUpChoice': return presentFollowUp(consequence.choice)
      case 'trustedRandomness': return presentTrustedRandomness(consequence.operation)
      case 'delayedEffect': return presentDelayedEffect(consequence.timing, consequence.effect)
      case 'ruleException': return presentException(consequence.exception)
      case 'substitution': return `星辰效果將${cardLabel(consequence.card)}（${elementLabels[consequence.printedElement]}）視為${elementLabels[consequence.interpretedElement]}`
      case 'declaredInput': return presentDeclaredInput(consequence.input, cardLabel)
      default: return assertNever(consequence)
    }
  })()
  return qualify(consequence.certainty, text)
}

export function presentActionDetail(
  detail: PlayerFacingActionDetail,
  cardLabel: CardLabel = defaultCardLabel,
  playerLabel: PlayerLabel = defaultPlayerLabel,
): string {
  return detail.consequences
    .map(consequence => presentConsequence(consequence, cardLabel, playerLabel))
    .map(text => `${text}。`)
    .join('')
}

export function presentPlayableAction(action: PlayableAction, cardLabel: CardLabel = defaultCardLabel): string {
  return presentActionDetail(action.detail, cardLabel)
}

export function presentSecretStrategyOption(action: SecretStrategyOption, cardLabel: CardLabel = defaultCardLabel): string {
  return presentActionDetail(action.detail, cardLabel)
}

export function presentDiscardRetrievalAction(
  detail: { detail: PlayerFacingActionDetail },
  playerLabel: PlayerLabel,
  cardLabel: CardLabel = defaultCardLabel,
): string {
  return presentActionDetail(detail.detail, cardLabel, playerLabel)
}
