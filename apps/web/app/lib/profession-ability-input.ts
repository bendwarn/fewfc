import type { Element, PlayableAction } from '../types/fewfc'

export type ProfessionAbilityOffer = Extract<
  PlayableAction,
  { type: 'activateProfessionAbility' }
>

export type VirtualFormationCardOffer = ProfessionAbilityOffer & {
  inputRequirement: Extract<
    NonNullable<ProfessionAbilityOffer['inputRequirement']>,
    { type: 'virtualFormationCard' }
  >
}

export function isVirtualFormationCardOffer(
  offer: ProfessionAbilityOffer,
): offer is VirtualFormationCardOffer {
  return offer.inputRequirement?.type === 'virtualFormationCard'
}

export function completeVirtualFormationCardOffer(
  offer: VirtualFormationCardOffer,
  element: Element,
  level: number,
): ProfessionAbilityOffer | null {
  if (
    !offer.inputRequirement.elements.includes(element)
    || !offer.inputRequirement.levels.includes(level)
  ) {
    return null
  }

  return {
    ...offer,
    declaredElement: element,
    declaredLevel: level,
  }
}

export function professionAbilityOfferKey(offer: ProfessionAbilityOffer): string {
  const inputKey = offer.inputRequirement?.type === 'virtualFormationCard'
    ? `${offer.inputRequirement.elements.join(',')}:${offer.inputRequirement.levels.join(',')}`
    : ''
  return [
    offer.id,
    offer.cards.join('-'),
    offer.targetCard ?? '',
    offer.inputRequirement?.type ?? '',
    inputKey,
  ].join(':')
}
