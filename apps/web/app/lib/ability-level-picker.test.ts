import { describe, expect, test } from 'bun:test'
import type { PlayableAction } from '../types/fewfc'
import { abilityLevelPicker, type PlayableAbility } from './ability-level-picker'

function darkSpirit(level: number): Extract<
  PlayableAction,
  { type: 'activateProfessionAbility' }
> {
  return {
    type: 'activateProfessionAbility',
    commandRole: 'activeEffect',
    id: 'dark:dark-spirit',
    name: '暗靈',
    detail: null,
    cards: [7],
    targetCard: 7,
    declaredElement: 'Earth',
    declaredLevel: level,
    inputRequirement: null,
  }
}

describe('Ability level picker', () => {
  test('keeps one Dark Spirit trigger even when only one level is legal', () => {
    const picker = abilityLevelPicker(
      [darkSpirit(1)],
      'activateProfessionAbility',
      'dark:dark-spirit',
    )

    expect(picker).toMatchObject({
      id: 'dark:dark-spirit',
      name: '暗靈',
      options: [{ declaredLevel: 1 }],
    })
  })

  test('orders engine-provided levels without merging unrelated abilities', () => {
    const abilities: PlayableAbility[] = [
      darkSpirit(2),
      {
        type: 'useSpiritSkill',
        commandRole: 'activeEffect',
        id: 'Splendor',
        name: '絢爛',
        detail: null,
        cards: [8],
        selectedCard: 8,
        declaredLevel: 5,
      },
      darkSpirit(1),
    ]

    expect(
      abilityLevelPicker(abilities, 'activateProfessionAbility', 'dark:dark-spirit')
        ?.options.map(option => option.declaredLevel),
    ).toEqual([1, 2])
  })
})
