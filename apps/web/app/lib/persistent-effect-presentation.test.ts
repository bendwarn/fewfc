import { expect, test } from 'bun:test'
import type { PublicGameState } from '../types/fewfc'
import { presentPersistentEffects } from './persistent-effect-presentation'

test('presents typed persistent effects without leaking internal IDs', () => {
  const state = {
    statuses: [{
      id: 'pouch-golden',
      owner: { kind: 'player', id: 'p1' },
      kind: 'PouchGoldenCicada',
      presentation: 'goldenCicada',
      duration: { type: 'untilTurnEnd', player: 'p1' },
    }, {
      id: 'pouch-watch-fire',
      owner: { kind: 'player', id: 'p1' },
      kind: 'PouchWatchFire',
      presentation: 'watchFire',
      duration: { type: 'untilTurnEnd', player: 'p1' },
    }],
    turnNumber: 6,
    currentPlayer: 'p1',
    turnOrder: ['p1', 'p2'],
    jianghuStates: [{
      owner: 'p1',
      kind: 'Poison',
      remainingTurns: 2,
      expiresOnTurn: 7,
    }],
    limitedUses: [{
      owner: 'p1',
      key: 'confluence:heavenly-resonance',
      presentation: 'heavenlyResonance',
      remaining: 0,
      maximum: 1,
    }],
    confluenceCardObligations: [],
    scheduledEchoes: [{ player: 'p1', melody: 'fallingWood', dueTurnNumber: 8 }],
    flowStates: [{ player: 'p1', layers: 2 }],
    formationSuppressions: [{
      source: 'p1',
      target: 'p1',
      formationName: '兵器',
      expiresOnTurnNumber: 8,
    }],
    scheduledPlantEarth: [{ player: 'p1', dueTurnNumber: 8 }],
  } satisfies Pick<PublicGameState,
    | 'statuses'
    | 'turnNumber'
    | 'currentPlayer'
    | 'turnOrder'
    | 'jianghuStates'
    | 'limitedUses'
    | 'confluenceCardObligations'
    | 'scheduledEchoes'
    | 'flowStates'
    | 'formationSuppressions'
    | 'scheduledPlantEarth'>

  expect(presentPersistentEffects(state, 'p1', 'team-a').map(effect => effect.label)).toStrictEqual([
      '本回合結束 · 金蟬、觀火',
      '再 2 回合結束 · 江湖狀態：中毒、裂土：壓制 兵器',
      '天響 · 0/1',
      '迴響 · 角調‧落木 · 第 8 回合',
      '流水 · 2 層',
      '植土 · 第 8 回合',
    ])
})

test('exhaustively presents every closed status, duration, limited use, and Echo melody', () => {
  const statusPresentations = [
    'cannotAct',
    'cannotDraw',
    'divineCalculation',
    'galeRain',
    'goldenCicada',
    'watchFire',
    'lurePlayer',
    'lureSpirit',
    'spiritStoneShield',
    'jianghuFanBeyondHeaven',
    'jianghuYangAura',
    'jianghuDancingYang',
    'jianghuMeteor',
    'unclassified',
  ] as const
  const durations = [
    { type: 'untilTurnStart' as const, player: 'p1' },
    { type: 'untilTurnEnd' as const, player: 'p1' },
    { type: 'untilTurnEndNumber' as const, player: 'p1', turnNumber: 9 },
    { type: 'permanent' as const },
  ]
  const limitedUses = [
    'heavenlyResonance',
    'imprisoningArray',
    'tailwind',
    'voidRealm',
    'unclassified',
  ] as const
  const melodies = [
    'ringingMetal',
    'fallingWood',
    'flowingWater',
    'warFire',
    'splitEarth',
    'pureFire',
    'unclassified',
  ] as const
  const state = {
    turnNumber: 6,
    currentPlayer: 'p1',
    turnOrder: ['p1', 'p2'],
    statuses: statusPresentations.map((presentation, index) => ({
      id: `status-${index}`,
      owner: { kind: 'player' as const, id: 'p1' },
      kind: `internal-status-${index}`,
      presentation,
      duration: durations[index % durations.length]!,
    })),
    jianghuStates: [
      { owner: 'p1', kind: 'ThousandBlades' as const, remainingTurns: 1, expiresOnTurn: 6 },
      { owner: 'p1', kind: 'SnowTreading' as const, remainingTurns: 1, expiresOnTurn: 7 },
      { owner: 'p1', kind: 'Poison' as const, remainingTurns: 1, expiresOnTurn: null },
    ],
    limitedUses: limitedUses.map((presentation, index) => ({
      owner: 'p1',
      key: `internal-limited-${index}`,
      presentation,
      remaining: 1,
      maximum: 1,
    })),
    confluenceCardObligations: [
      { owner: 'p1', card: null, allowProfessionFormation: false },
      { owner: 'p1', card: 7, allowProfessionFormation: true },
    ],
    scheduledEchoes: melodies.map((melody, index) => ({
      player: 'p1', melody, dueTurnNumber: index + 1,
    })),
    flowStates: [{ player: 'p1', layers: 1 }],
    formationSuppressions: [{
      source: 'p2', target: 'p1', formationName: '防禦', expiresOnTurnNumber: 9,
    }],
    scheduledPlantEarth: [{ player: 'p1', dueTurnNumber: 9 }],
  } satisfies Pick<PublicGameState,
    | 'statuses'
    | 'turnNumber'
    | 'currentPlayer'
    | 'turnOrder'
    | 'jianghuStates'
    | 'limitedUses'
    | 'confluenceCardObligations'
    | 'scheduledEchoes'
    | 'flowStates'
    | 'formationSuppressions'
    | 'scheduledPlantEarth'>

  const labels = presentPersistentEffects(state, 'p1', 'team-a').map(effect => effect.label)
  expect(labels.length).toBe(5 + 5 + 2 + 7 + 1 + 1)
  expect(labels.every(label => !label.includes('internal-'))).toBeTruthy()
  expect(labels.some(label => (
    label.startsWith('本回合結束 · ')
    && label.includes('江湖狀態：千鋒')
    && label.includes('江湖狀態：中毒')
  ))).toBeTruthy()
  expect(labels.some(label => label === '再 1 回合結束 · 江湖狀態：踏雪')).toBeTruthy()
  expect(labels.some(label => label.startsWith('再 2 回合開始時結束 · '))).toBeTruthy()
  expect(labels.some(label => (
    label.startsWith('再 3 回合結束 · ')
    && label.includes('裂土：壓制 防禦')
  ))).toBeTruthy()
  expect(labels.some(label => label.startsWith('持續生效 · '))).toBeTruthy()
})
