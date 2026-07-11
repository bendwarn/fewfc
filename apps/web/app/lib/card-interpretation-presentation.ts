import type { CardInterpretationPresentation, Element } from '../types/fewfc'

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

export function presentCardInterpretation(
  interpretation: CardInterpretationPresentation,
): string {
  const card = interpretation.card?.label ?? '一張手牌'
  if (interpretation.type === 'professionAbility') {
    return `已準備 · ${abilityLabels[interpretation.ability]} · ${card} 視為${elementLabels[interpretation.element]} ${interpretation.level} 級 · 本回合`
  }

  return `已生效 · ${skillLabels[interpretation.skill]} · ${card}視為 ${interpretation.level} 級 · 本回合`
}
