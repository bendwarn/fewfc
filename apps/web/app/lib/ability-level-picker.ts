import type { PlayableAction } from '../types/fewfc'

export type PlayableAbility = Extract<
  PlayableAction,
  { type: 'activateProfessionAbility' | 'useSpiritSkill' }
>

export interface AbilityLevelPicker {
  id: string
  name: string
  options: PlayableAbility[]
}

export function abilityLevelPicker(
  abilities: readonly PlayableAbility[],
  type: PlayableAbility['type'],
  id: string,
): AbilityLevelPicker | null {
  const options = abilities
    .filter(ability => (
      ability.type === type
      && ability.id === id
      && ability.declaredLevel !== null
    ))
    .sort((left, right) => (left.declaredLevel ?? 0) - (right.declaredLevel ?? 0))

  return options.length
    ? {
        id,
        name: options[0]!.name,
        options,
      }
    : null
}
