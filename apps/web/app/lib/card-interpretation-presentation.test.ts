import { expect, test } from 'bun:test'
import type { CardInterpretationPresentation } from '../types/fewfc'
import { cardInterpretationBadge, presentCardInterpretation } from './card-interpretation-presentation'

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

  expect(cases.length).toBe(9)
  for (const interpretation of cases) expect(presentCardInterpretation(interpretation).length > 0).toBeTruthy()
  expect(presentCardInterpretation(cases[0]!)).toBe('已準備 · 幻術 · 火 3 視為火行 3 級 · 本回合')
  expect(presentCardInterpretation(cases[5]!)).toBe('已生效 · 螢光 · 一張手牌視為 3 級 · 本回合')
})

test('attaches composed effective facts to the visible physical card', () => {
  const card = { id: 7, label: '火 2', element: 'Fire' as const, level: 2, secretStrategies: [] }
  const badge = cardInterpretationBadge(card, [
    {
      type: 'professionAbility',
      player: 'alice',
      ability: 'illusion',
      card,
      element: 'Metal',
      level: 4,
    },
    {
      type: 'spiritSkill',
      player: 'alice',
      skill: 'Glimmer',
      card,
      level: 3,
    },
  ])

  expect(badge).toMatchObject({
    label: '視為 金行 3 級',
    effectiveElement: 'Metal',
    effectiveLevel: 3,
  })
  expect(badge?.description).toContain('幻術、螢光')
  expect(cardInterpretationBadge(card, [{
    type: 'spiritSkill',
    player: 'bob',
    skill: 'Glimmer',
    card: null,
    level: 3,
  }])).toBeNull()
})
