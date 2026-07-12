import type { PublicCard } from '../types/fewfc'

export function isLegalChainTrigger(
  pouch: PublicCard | null | undefined,
  trigger: PublicCard,
): boolean {
  return Boolean(
    pouch
    && trigger.id !== pouch.id
    && trigger.element !== null
    && pouch.element !== null
    && trigger.element !== pouch.element
    && trigger.level !== null
    && pouch.level !== null
    && trigger.level !== pouch.level,
  )
}
