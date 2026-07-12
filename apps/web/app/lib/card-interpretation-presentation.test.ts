import assert from 'node:assert/strict'
import { test } from 'bun:test'
import type { CardInterpretationPresentation } from '../types/fewfc'
import { presentCardInterpretation } from './card-interpretation-presentation'

test('presents committed Profession and Spirit Card Interpretations in Traditional Chinese', () => {
  const card = { id: 7, label: '火 3', element: 'Fire' as const, level: 3, secretStrategies: [] }
  const cases: CardInterpretationPresentation[] = [
    ...(['illusion', 'phantasm', 'blazingYangArt', 'darkSpirit', 'unclassified'] as const).map(ability => ({
      type: 'professionAbility' as const,
      player: 'p1',
      ability,
      card,
      element: 'Fire' as const,
      level: 3,
    })),
    ...(['Glimmer', 'Splendor', 'LegacyFireLevel', 'Unclassified'] as const).map(skill => ({
      type: 'spiritSkill' as const,
      player: 'p1',
      skill,
      card: null,
      level: 3,
    })),
  ]

  assert.equal(cases.length, 9)
  for (const interpretation of cases) assert.ok(presentCardInterpretation(interpretation).length > 0)
  assert.equal(presentCardInterpretation(cases[0]!), '已準備 · 幻術 · 火 3 視為火行 3 級 · 本回合')
  assert.equal(presentCardInterpretation(cases[5]!), '已生效 · 螢光 · 一張手牌視為 3 級 · 本回合')
})
