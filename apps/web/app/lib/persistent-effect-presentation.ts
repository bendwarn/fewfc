import type {
  CardInstanceId,
  LimitedUsePresentation,
  PlayerId,
  PublicGameState,
  StatusDurationPresentation,
  StatusPresentation,
  TeamId,
} from '../types/fewfc'
import { echoMelodyLabels } from './echo-presentation'

type PersistentEffectState = Pick<PublicGameState,
  | 'turnNumber'
  | 'currentPlayer'
  | 'turnOrder'
  | 'statuses'
  | 'jianghuStates'
  | 'limitedUses'
  | 'confluenceCardObligations'
  | 'scheduledEchoes'
  | 'flowStates'
  | 'formationSuppressions'
  | 'scheduledPlantEarth'>
  & Partial<Pick<PublicGameState, 'hands'>>

export interface PresentedPersistentEffect {
  key: string
  label: string
}

interface PresentedExpiry {
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
  jianghuYangAura: '天陽罡',
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

function remainingTurnsThrough(
  state: PersistentEffectState,
  player: PlayerId,
  turnNumber: number,
): number | null {
  if (!state.currentPlayer || state.turnOrder.length === 0) return null
  const currentIndex = state.turnOrder.indexOf(state.currentPlayer)
  const targetIndex = state.turnOrder.indexOf(player)
  if (currentIndex < 0 || targetIndex < 0) return null

  const firstDistance = (targetIndex - currentIndex + state.turnOrder.length) % state.turnOrder.length
  const firstTurn = state.turnNumber + firstDistance
  if (firstTurn > turnNumber) return 0
  return Math.floor((turnNumber - firstTurn) / state.turnOrder.length) + 1
}

function turnExpiry(player: PlayerId, turns: number): PresentedExpiry {
  return {
    key: `turn:${player}:${turns}`,
    label: `剩餘 ${turns} 回合`,
  }
}

function numberedTurnEndExpiry(
  state: PersistentEffectState,
  player: PlayerId,
  turnNumber: number,
): PresentedExpiry {
  const turns = remainingTurnsThrough(state, player, turnNumber)
  return turns === null
    ? { key: `turn-end:${player}:${turnNumber}`, label: '至自身回合結束' }
    : turnExpiry(player, turns)
}

function presentDuration(
  state: PersistentEffectState,
  duration: StatusDurationPresentation,
): PresentedExpiry {
  switch (duration.type) {
    case 'untilTurnStart': {
      return {
        key: `round:${duration.player}:1`,
        label: '剩餘 1 輪',
      }
    }
    case 'untilTurnEnd': {
      return turnExpiry(duration.player, 1)
    }
    case 'untilTurnEndNumber': {
      return numberedTurnEndExpiry(state, duration.player, duration.turnNumber)
    }
    case 'permanent': return { key: 'permanent', label: '持續生效' }
  }
  const exhaustive: never = duration
  return exhaustive
}

function visibleHandCardLabel(
  state: PersistentEffectState,
  player: PlayerId,
  cardId: CardInstanceId,
): string | null {
  const cards = state.hands?.find(hand => hand.player === player)?.cards
  if (!cards || cards.kind === 'hidden') return null
  if (cards.kind === 'known') {
    return cards.cards.find(card => card.id === cardId)?.label ?? null
  }
  return cards.cards.find(card => card?.id === cardId)?.label ?? null
}

export function presentPersistentEffects(
  state: PersistentEffectState,
  player: PlayerId,
  team: TeamId,
): PresentedPersistentEffect[] {
  const effects: PresentedPersistentEffect[] = []
  const expiryGroups = new Map<string, {
    expiry: PresentedExpiry
    labels: string[]
    itemKeys: string[]
  }>()
  const addExpiringEffect = (expiry: PresentedExpiry, label: string, itemKey: string) => {
    const group = expiryGroups.get(expiry.key) ?? { expiry, labels: [], itemKeys: [] }
    group.labels.push(label)
    group.itemKeys.push(itemKey)
    expiryGroups.set(expiry.key, group)
  }

  for (const status of state.statuses) {
    const applies = status.owner.kind === 'player'
      ? status.owner.id === player
      : status.owner.id === team
    if (applies) {
      const expiry = presentDuration(state, status.duration)
      addExpiringEffect(expiry, statusLabels[status.presentation], `status:${status.id}`)
    }
  }
  for (const active of state.jianghuStates.filter(active => active.owner === player)) {
    const label = {
      ThousandBlades: '千鋒',
      SnowTreading: '踏雪',
      Poison: '中毒',
    }[active.kind]
    if (active.kind === 'Poison') {
      addExpiringEffect(
        turnExpiry(active.owner, active.remainingTurns),
        `江湖狀態：${label}`,
        `jianghu:${active.kind}`,
      )
    } else if (active.expiresOnTurn === null) {
      effects.push({ key: `jianghu:${active.kind}`, label: `江湖狀態：${label}` })
    } else {
      addExpiringEffect(
        numberedTurnEndExpiry(state, active.owner, active.expiresOnTurn),
        `江湖狀態：${label}`,
        `jianghu:${active.kind}`,
      )
    }
  }
  for (const suppression of state.formationSuppressions.filter(active => active.target === player)) {
    addExpiringEffect(
      numberedTurnEndExpiry(state, suppression.target, suppression.expiresOnTurnNumber),
      `裂土：壓制 ${suppression.formationName}`,
      `suppression:${suppression.formationName}`,
    )
  }
  for (const group of expiryGroups.values()) {
    effects.push({
      key: `expiry:${group.itemKeys.join(':')}`,
      label: `${group.expiry.label} · ${group.labels.join('、')}`,
    })
  }
  for (const useCount of state.limitedUses.filter(useCount => useCount.owner === player)) {
    effects.push({
      key: `limited:${useCount.key}`,
      label: `${limitedUseLabels[useCount.presentation]} · ${useCount.remaining}/${useCount.maximum}`,
    })
  }
  for (const obligation of state.confluenceCardObligations.filter(active => active.owner === player)) {
    const cardLabel = obligation.card === null
      ? null
      : visibleHandCardLabel(state, obligation.owner, obligation.card)
    effects.push({
      key: `tuning:${obligation.card ?? 'hidden'}`,
      label: `調律牌 · ${cardLabel ?? '一張手牌'}（${obligation.allowProfessionFormation ? '可用於職業陣法' : '僅可轉職'}）`,
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
  for (const schedule of state.scheduledPlantEarth.filter(active => active.player === player)) {
    effects.push({
      key: `plant-earth:${schedule.dueTurnNumber}`,
      label: `植土 · 第 ${schedule.dueTurnNumber} 回合`,
    })
  }
  return effects
}
