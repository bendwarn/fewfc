import type {
  DiscardRetrievalActionDetail,
  Element,
  PlayableAction,
  SecretStrategyAction,
} from '../types/fewfc'

type FormationPolicy = Extract<PlayableAction, { type: 'performFormation' }>['policy']

const formationPolicyDetails: Record<FormationPolicy, string> = {
  standard: '',
  pouchChain: '第一張成為友方玩家的錦囊；若選擇第二張，公開並立即觸發一個符合條件的秘計。',
  echoRingingMetal: '主效果完整結算後，可捨棄一張印刷行屬為金或土的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，也不會再次排定迴響。',
  echoFallingWood: '主效果完整結算後，可捨棄一張印刷行屬為木或水的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，也不會再次排定迴響。',
  echoFlowingWater: '主效果完整結算後，可捨棄一張印刷行屬為水或金的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，也不會再次排定迴響。',
  echoWarFire: '主效果完整結算後，可捨棄一張印刷行屬為火或木的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，也不會再次排定迴響。',
  echoSplitEarth: '主效果完整結算後，可捨棄一張印刷行屬為土或火的手牌作為迴響代價；若支付，於自己下次回合開始只再次執行此曲調主效果，不視為新的陣法，也不會再次排定迴響。',
  echoPureFire: '主效果完整結算後，不需支付迴響代價並自動排定迴響；於自己下次回合開始重新選擇玩家，只再次執行此主效果，不視為新的陣法，也不會再次排定迴響。',
  echoPlantEarth: '於自己下次回合開始，選擇鳴金、落木、流水、戰火或裂土之一並執行其主效果。',
}

const elementLabels: Record<Element, string> = {
  Metal: '金行牌',
  Wood: '木行牌',
  Water: '水行牌',
  Fire: '火行牌',
  Earth: '土行牌',
}

function punctuate(text: string): string {
  return text.endsWith('。') ? text : `${text}。`
}

export function presentPlayableAction(
  action: PlayableAction,
  cardLabel: (card: number) => string = card => `牌 ${card}`,
): string {
  let detail = punctuate(action.summary)
  if (action.type !== 'performFormation') return detail

  detail += formationPolicyDetails[action.policy]

  if (action.starSubstitution) {
    const substitution = action.starSubstitution
    detail += `星辰效果：將${cardLabel(substitution.card)}（${elementLabels[substitution.printedElement]}）視為${elementLabels[substitution.interpretedElement]}。`
  }
  return detail
}

const secretStrategyDetails: Record<SecretStrategyAction['strategy'], string> = {
  GoldenCicada: '本回合保護自己不受無法行動、無法抽牌、反制效果與其他玩家的秘計影響。',
  StealTheBeam: '觸發時手牌快照中的每張牌本回合等級＋1。',
  MuddyWaters: '本回合抽牌＋1。',
  WatchTheFire: '下家的下個回合內，由下家陣法造成的所有隊伍生命變化無效（包含攻擊傷害）。',
  LureTheTigerAway: '指定玩家一回合內無法使用職業能力與精靈技能，且精靈無法增加靈力。',
  ReturnSoul: '召喚錦囊印刷行屬的精靈；靈力為1加上被替換精靈的靈力，最高6。',
  SheepStealing: '從牌組與棄牌堆各選兩張交換，之後洗牌。',
  DarkCrossing: '直接轉職為錦囊印刷行屬對應的一階英雄學派職業。',
  DeceiveHeaven: '破除一個現有星辰，或取得一個本回合有效的指定星辰效果。',
  Retreat: '破除目前環境，或捨棄一張手牌並將環境轉移為該牌的印刷行屬。',
}

export function presentSecretStrategyAction(action: SecretStrategyAction): string {
  return secretStrategyDetails[action.strategy]
}

export function presentDiscardRetrievalAction(
  detail: DiscardRetrievalActionDetail,
  playerLabel: (player: string) => string,
): string {
  return `支付 ${detail.hpCost} 點生命，將 ${playerLabel(detail.previousPlayer)} 上回合捨棄的${detail.card.label} 放到自己的牌組頂；不結束行動。`
}
