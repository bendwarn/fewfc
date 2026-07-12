import assert from 'node:assert/strict'
import { test } from 'bun:test'
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

  assert.equal(cases.length, 24)
  for (const presentation of cases) {
    const label = presentPendingChoice(presentation)
    assert.ok(label.length > 0)
    assert.doesNotMatch(label, /echo:|jianghu:|confluence:|tribulation:/)
  }

  assert.equal(
    presentPendingChoice({ type: 'clearWind' }),
    '晴風：選取此牌捨棄；不選則放回牌組頂',
  )
})
