import { ruleModuleLabel } from './ruleset-presentation'

interface SplitEarthChoiceContext {
  player: string
  purpose: string
  presentation: { type: string }
  formationGroups: Array<{
    ruleModuleId: string | null
    formations: Array<{ id: string }>
  }>
}

export function splitEarthRuleLabel(ruleModuleId: string | null): string {
  return ruleModuleId === null ? '基礎規則' : ruleModuleLabel(ruleModuleId)
}

export function usesSplitEarthFormationGroups(choice: SplitEarthChoiceContext): boolean {
  return choice.presentation.type === 'echoSplitEarthFormation'
    && choice.formationGroups.length > 0
}

export function splitEarthChoiceKey(
  choice: SplitEarthChoiceContext,
  turnNumber: number,
  roomConnected: boolean,
): string {
  return JSON.stringify({
    turnNumber,
    player: choice.player,
    purpose: choice.purpose,
    presentation: choice.presentation.type,
    roomConnected,
    groups: choice.formationGroups.map(group => ({
      ruleModuleId: group.ruleModuleId,
      formations: group.formations.map(formation => formation.id),
    })),
  })
}
