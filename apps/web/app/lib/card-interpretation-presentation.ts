import type { CardInterpretationPresentation, Element, PublicCard } from '../types/fewfc'

const abilityLabels: Record<Extract<CardInterpretationPresentation, { type: 'professionAbility' }>['ability'], string> = {
  illusion: '幻術',
  phantasm: '幻朧',
  blazingYangArt: '烈陽訣',
  darkSpirit: '暗靈',
  unclassified: '職業能力',
}

const skillLabels: Record<Extract<CardInterpretationPresentation, { type: 'spiritSkill' }>['skill'], string> = {
  Glimmer: '螢光',
  Splendor: '絢爛',
  LegacyFireLevel: '火精靈技能',
  Unclassified: '精靈技能',
}

const elementLabels: Record<Element, string> = {
  Metal: '金行',
  Wood: '木行',
  Water: '水行',
  Fire: '火行',
  Earth: '土行',
}

export interface CardInterpretationBadge {
  label: string
  description: string
  effectiveElement: Element | null
  effectiveLevel: number | null
}

export function presentCardInterpretation(
  interpretation: CardInterpretationPresentation,
): string {
  const card = interpretation.card?.label ?? '一張手牌'
  if (interpretation.type === 'professionAbility') {
    return `已準備 · ${abilityLabels[interpretation.ability]} · ${card} 視為${elementLabels[interpretation.element]} ${interpretation.level} 級 · 本回合`
  }

  return `已生效 · ${skillLabels[interpretation.skill]} · ${card}視為 ${interpretation.level} 級 · 本回合`
}

export function cardInterpretationBadge(
  card: Pick<PublicCard, 'id' | 'element' | 'level'>,
  interpretations: readonly CardInterpretationPresentation[],
): CardInterpretationBadge | null {
  const layers = interpretations.filter(interpretation => interpretation.card?.id === card.id)
  if (!layers.length) return null

  let effectiveElement = card.element
  let effectiveLevel = card.level
  const sources: string[] = []

  for (const layer of layers) {
    if (layer.type === 'professionAbility') {
      effectiveElement = layer.element
      effectiveLevel = layer.level
      sources.push(abilityLabels[layer.ability])
    } else {
      effectiveLevel = layer.level
      sources.push(skillLabels[layer.skill])
    }
  }

  const effectiveFacts: string[] = []
  if (effectiveElement !== card.element && effectiveElement) effectiveFacts.push(elementLabels[effectiveElement])
  if (effectiveLevel !== card.level && effectiveLevel !== null) effectiveFacts.push(`${effectiveLevel} 級`)
  if (!effectiveFacts.length) {
    if (effectiveElement) effectiveFacts.push(elementLabels[effectiveElement])
    if (effectiveLevel !== null) effectiveFacts.push(`${effectiveLevel} 級`)
  }

  return {
    label: `視為 ${effectiveFacts.join(' ')}`,
    description: `${[...new Set(sources)].join('、')}：${layers.map(presentCardInterpretation).join('；')}`,
    effectiveElement,
    effectiveLevel,
  }
}
