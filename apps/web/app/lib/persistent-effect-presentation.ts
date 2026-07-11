import type {
  LimitedUsePresentation,
  PlayerId,
  PublicGameState,
  StatusDurationPresentation,
  StatusPresentation,
  TeamId,
} from '../types/fewfc'
import { echoMelodyLabels } from './echo-presentation'

type PersistentEffectState = Pick<PublicGameState,
  | 'statuses'
  | 'jianghuStates'
  | 'limitedUses'
  | 'confluenceCardObligations'
  | 'scheduledEchoes'
  | 'flowStates'
  | 'formationSuppressions'
  | 'scheduledPlantEarth'>

export interface PresentedPersistentEffect {
  key: string
  label: string
}

const statusLabels: Record<StatusPresentation, string> = {
  cannotAct: '無法行動',
  cannotDraw: '無法抽牌',
  divineCalculation: '神算',
  galeRain: '烈風暴雨',
  goldenCicada: '金蟬',
  watchFire: '觀火',
  lurePlayer: '離山（玩家）',
  lureSpirit: '離山（精靈）',
  spiritStoneShield: '石盾',
  jianghuFanBeyondHeaven: '天外飛扇',
  jianghuYangAura: '天陽氣',
  jianghuDancingYang: '舞陽訣',
  jianghuMeteor: '流星步',
  unclassified: '效果持續中',
}

const limitedUseLabels: Record<LimitedUsePresentation, string> = {
  heavenlyResonance: '天響',
  imprisoningArray: '禁錮法陣',
  tailwind: '順風',
  voidRealm: '虛空境界',
  unclassified: '限次效果',
}

function presentDuration(duration: StatusDurationPresentation): string {
  switch (duration.type) {
    case 'untilTurnStart': return '至指定玩家回合開始'
    case 'untilTurnEnd': return '至指定玩家回合結束'
    case 'untilTurnEndNumber': return `至指定玩家第 ${duration.turnNumber} 回合結束`
    case 'permanent': return '持續生效'
  }
  const exhaustive: never = duration
  return exhaustive
}

export function presentPersistentEffects(
  state: PersistentEffectState,
  player: PlayerId,
  team: TeamId,
): PresentedPersistentEffect[] {
  const effects: PresentedPersistentEffect[] = []
  for (const status of state.statuses) {
    const applies = status.owner.kind === 'player'
      ? status.owner.id === player
      : status.owner.id === team
    if (applies) {
      effects.push({
        key: `status:${status.id}`,
        label: `${statusLabels[status.presentation]} · ${presentDuration(status.duration)}`,
      })
    }
  }
  for (const active of state.jianghuStates.filter(active => active.owner === player)) {
    const label = {
      ThousandBlades: '千鋒',
      SnowTreading: '踏雪',
      Poison: `中毒（${active.remainingTurns} 回合）`,
    }[active.kind]
    effects.push({ key: `jianghu:${active.kind}`, label: `江湖狀態 · ${label}` })
  }
  for (const useCount of state.limitedUses.filter(useCount => useCount.owner === player)) {
    effects.push({
      key: `limited:${useCount.key}`,
      label: `${limitedUseLabels[useCount.presentation]} · ${useCount.remaining}/${useCount.maximum}`,
    })
  }
  for (const obligation of state.confluenceCardObligations.filter(active => active.owner === player)) {
    effects.push({
      key: `tuning:${obligation.card ?? 'hidden'}`,
      label: `調律牌 · ${obligation.card === null ? '一張手牌' : `牌 ${obligation.card}`}（${obligation.allowProfessionFormation ? '可用於職業陣法' : '僅可轉職'}）`,
    })
  }
  for (const schedule of state.scheduledEchoes.filter(active => active.player === player)) {
    effects.push({
      key: `echo:${schedule.melody}:${schedule.dueTurnNumber}`,
      label: `迴響 · ${echoMelodyLabels[schedule.melody]} · 第 ${schedule.dueTurnNumber} 回合`,
    })
  }
  const flow = state.flowStates.find(active => active.player === player)
  if (flow?.layers) effects.push({ key: 'flow', label: `流水 · ${flow.layers} 層` })
  for (const suppression of state.formationSuppressions.filter(active => active.target === player)) {
    effects.push({
      key: `suppression:${suppression.formationName}`,
      label: `裂土 · 壓制 ${suppression.formationName}`,
    })
  }
  for (const schedule of state.scheduledPlantEarth.filter(active => active.player === player)) {
    effects.push({
      key: `plant-earth:${schedule.dueTurnNumber}`,
      label: `植土 · 第 ${schedule.dueTurnNumber} 回合`,
    })
  }
  return effects
}
