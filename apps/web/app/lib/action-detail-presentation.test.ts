import { expect, test } from 'bun:test'
import type { PlayableAction, SecretStrategyAction } from '../types/fewfc'
import {
  presentDiscardRetrievalAction,
  presentPlayableAction,
  presentSecretStrategyAction,
} from './action-detail-presentation'

test('composes Formation consequences in Web without changing catalog rule text', () => {
  const chain: PlayableAction = {
    type: 'performFormation',
    id: 'pouch:chain',
    name: '連環',
    category: 'Spell',
    policy: 'pouchChain',
    summary: '三張不同行、不同級牌；從自身牌組選擇一或兩張牌',
    cards: [1, 2, 3],
    starSubstitution: null,
    matchOption: null,
  }

  expect(presentPlayableAction(chain)).toBe('三張不同行、不同級牌；從自身牌組選擇一或兩張牌。第一張成為友方玩家的錦囊；若選擇第二張，公開並立即觸發一個符合條件的秘計。')

  const policies = [
    'standard',
    'pouchChain',
    'echoRingingMetal',
    'echoFallingWood',
    'echoFlowingWater',
    'echoWarFire',
    'echoSplitEarth',
    'echoPureFire',
    'echoPlantEarth',
  ] as const
  for (const policy of policies) {
    expect(presentPlayableAction({ ...chain, policy }).length > 0).toBeTruthy()
  }

  expect(presentPlayableAction({
      ...chain,
      id: 'echo:plant-earth',
      name: '變宮‧植土',
      policy: 'echoPlantEarth',
      summary: '一張土行牌和一張木行牌，等級合計至少 7',
    })).toBe('一張土行牌和一張木行牌，等級合計至少 7。於自己下次回合開始，選擇鳴金、落木、流水、戰火或裂土之一並執行其主效果。')
})

test('presents Pouch and Discard Retrieval active effects before commit', () => {
  const baseStrategy: Omit<SecretStrategyAction, 'strategy'> = {
    sourceCard: 8,
    input: 'none',
    targetPlayers: [],
    stars: [],
    breakStars: [],
    deckCards: [],
    discardCards: [],
    handCards: [],
    requiredCardCount: 0,
  }

  const strategies: SecretStrategyAction['strategy'][] = [
    'GoldenCicada',
    'StealTheBeam',
    'MuddyWaters',
    'WatchTheFire',
    'LureTheTigerAway',
    'ReturnSoul',
    'SheepStealing',
    'DarkCrossing',
    'DeceiveHeaven',
    'Retreat',
  ]

  expect(strategies.length).toBe(10)
  for (const strategy of strategies) {
    expect(presentSecretStrategyAction({ ...baseStrategy, strategy }).length > 0).toBeTruthy()
  }
  expect(presentSecretStrategyAction({ ...baseStrategy, strategy: 'GoldenCicada' })).toMatch(/本回合/)
  expect(presentSecretStrategyAction({ ...baseStrategy, strategy: 'WatchTheFire' })).toBe('下家的下個回合內，由下家陣法造成的所有隊伍生命變化無效（包含攻擊傷害）。')
  expect(presentDiscardRetrievalAction(
      {
        card: { id: 3, label: '火 3', element: 'Fire', level: 3, secretStrategies: [] },
        previousPlayer: 'p2',
        hpCost: 6,
      },
      player => ({ p2: '對手' })[player] ?? player,
    )).toBe('支付 6 點生命，將 對手 上回合捨棄的火 3 放到自己的牌組頂；不結束行動。')
})
