import { expect, test } from 'bun:test'
import type { PendingChoicePresentation } from '../types/fewfc'
import { presentPendingChoice } from './pending-choice-presentation'

test('presents every Pending Choice by semantic continuation instead of internal ID', () => {
  const cases: PendingChoicePresentation[] = [
    { type: 'turnDrawDiscard' },
    { type: 'holyWind' },
    { type: 'chaos' },
    { type: 'revelation' },
    { type: 'azureCloudStep' },
    { type: 'clearWind' },
    { type: 'clearWindTenThousandMiles' },
    { type: 'mirrorResonance' },
    { type: 'myriadResonance' },
    { type: 'thousandResonance' },
    { type: 'echoRingingMetalDeckCard' },
    ...(['ringingMetal', 'fallingWood', 'flowingWater', 'warFire', 'splitEarth'] as const)
      .map(melody => ({ type: 'echoCost' as const, melody })),
    { type: 'echoSplitEarthFormation' },
    { type: 'echoPureFirePlayer' },
    { type: 'echoPlantEarthMelody' },
    { type: 'earthRendingEnvironment' },
    { type: 'earthRendingCard' },
    { type: 'metamorphosis' },
    { type: 'sealCard' },
    { type: 'unclassified' },
  ]

  expect(cases.length).toBe(24)
  for (const presentation of cases) {
    const label = presentPendingChoice(presentation)
    expect(label.length > 0).toBeTruthy()
    expect(label).not.toMatch(/echo:|jianghu:|confluence:|tribulation:/)
  }

  expect(presentPendingChoice({ type: 'clearWind' })).toBe('晴風：選取此牌捨棄；不選則放回牌組頂')
  expect(presentPendingChoice({ type: 'clearWindTenThousandMiles' })).toBe('晴風萬里：選擇要保留的牌')
})
