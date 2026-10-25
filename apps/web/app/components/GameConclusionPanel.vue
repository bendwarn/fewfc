<template>
  <section class="result-panel ui-glass" :class="{ 'result-enter': animate }" :data-outcome="state.gameConclusion?.outcome.type" aria-label="對局結算">
    <header class="result-heading" aria-live="polite">
      <h2>{{ resultText }}</h2>
      <p v-if="endReasonText" class="result-reason">{{ endReasonText }}</p>
    </header>
    <div v-if="state.terminalResolution" class="result-summary" aria-label="最後行動">
      <div class="result-summary-text">
        <small>最後行動<template v-if="state.terminalResolution.type !== 'automatic'"> · {{ playerLabel(state.terminalResolution.player) }}</template></small>
        <strong v-if="state.terminalResolution.type === 'formation'">{{ state.terminalResolution.formationName ?? '陣法' }}</strong>
        <strong v-else-if="state.terminalResolution.type === 'discardRetrieval'">取回 {{ playerLabel(state.terminalResolution.previousPlayer) }} 的棄牌</strong>
        <strong v-else>{{ state.terminalResolution.label }}</strong>
        <p v-if="terminalHiddenCount">{{ terminalHiddenCount }} 張蓋牌</p>
      </div>
      <div v-if="terminalCards.length" class="formation-cards">
        <GameCard
          v-for="card in terminalCards"
          :key="card.id"
          class="formation-card"
          :card="card"
          :interpretations="state.cardInterpretations"
        />
      </div>
    </div>
    <div v-if="$slots.actions" class="result-actions">
      <slot name="actions" />
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import GameCard from './GameCard.vue'
import type { PublicCard, PublicCardRefs, PublicGameState, TeamId } from '~/types/fewfc'

const props = defineProps<{
  state: PublicGameState
  teamLabel: (team: TeamId) => string
  playerLabel?: (player: string) => string
  animate?: boolean
}>()

const resultText = computed(() => {
  const conclusion = props.state.gameConclusion
  if (conclusion?.outcome.type === 'winner') {
    return `${props.teamLabel(conclusion.outcome.team)} 勝利`
  }

  if (conclusion?.outcome.type === 'draw') return '平局'

  if (props.state.fiveStarAlignment) {
    return `五星連珠 · ${props.teamLabel(props.state.fiveStarAlignment.team)} 勝利`
  }

  if (props.state.winnerTeam) return `${props.teamLabel(props.state.winnerTeam)} 勝利`

  const aliveTeams = props.state.hp.filter((entry) => entry.hp > 0)
  if (aliveTeams.length === 1) return `${props.teamLabel(aliveTeams[0]!.team)} 勝利`

  return '戰局結束'
})

const endReasonText = computed(() => {
  const conclusion = props.state.gameConclusion
  if (!conclusion) return '終局結論尚未載入。'

  return conclusion.causes.map((cause) => {
    if (cause.type === 'teamHpDepleted') {
      const teams = cause.teams.map(props.teamLabel)
      return teams.length ? `${teams.join('、')} 的生命值歸零` : '隊伍生命值歸零'
    }

    const directVictoryLabels: Record<string, string> = {
      'five-star-alignment': '達成五星連珠',
      'king-yama-decree': '施展閻王令',
    }
    return directVictoryLabels[cause.rule]
      ?? `${props.teamLabel(cause.team)} 達成直接勝利條件`
  }).join('；')
})

const playerLabel = (player: string) => props.playerLabel?.(player) ?? player

const terminalCards = computed<PublicCard[]>(() => {
  const resolution = props.state.terminalResolution
  if (resolution?.type === 'discardRetrieval') return [resolution.card]
  if (resolution?.type !== 'formation') return []
  const cards: PublicCardRefs = resolution.cards
  if (cards.kind === 'known') return cards.cards
  if (cards.kind === 'partiallyKnown') return cards.cards.filter((card): card is PublicCard => card !== null)
  return []
})

const terminalHiddenCount = computed(() => {
  const resolution = props.state.terminalResolution
  if (resolution?.type !== 'formation') return 0
  if (resolution.cards.kind === 'hidden') return resolution.cards.count
  if (resolution.cards.kind === 'partiallyKnown') return resolution.cards.cards.filter(card => card === null).length
  return 0
})
</script>

<style scoped>
@reference "../assets/css/main.css";

.result-panel { display: grid; width: min(100%, 560px); min-width: 0; gap: 18px; padding: 22px 28px; border-radius: 24px; color: var(--app-text); text-align: center; }
.result-heading { display: grid; justify-items: center; gap: 8px; }
.result-panel h2 { color: var(--app-accent-strong); font: 900 clamp(28px, 2.4vw, 32px) / 1.3 var(--font-serif); }
.result-panel[data-outcome="draw"] h2 { color: var(--app-text); }
.result-reason { font-size: 13px; line-height: 1.6; }
.result-summary { display: flex; min-width: 0; align-items: center; justify-content: center; flex-wrap: wrap; gap: 12px 20px; padding-top: 14px; border-top: 1px solid var(--ui-glass-stroke); }
.result-summary-text { display: grid; min-width: 0; gap: 5px; text-align: left; }
.result-summary small, .result-summary p { color: var(--app-text-muted); font-size: 12px; line-height: 1.5; }
.result-summary strong { font: 700 16px / 1.5 var(--font-serif); }
.formation-cards { display: flex; min-width: 0; flex-wrap: wrap; justify-content: center; gap: 6px; }
.formation-cards :deep(.playing-card) { width: 48px; }
.result-actions { display: flex; justify-content: center; flex-wrap: wrap; gap: 10px; padding-top: 2px; }
:slotted(.primary-button) { display: inline-flex; min-width: 160px; min-height: 44px; align-items: center; justify-content: center; gap: 18px; padding: 10px 18px; font-size: 13px; }
.result-enter { animation: result-arrive .28s ease-out both; }
.result-enter h2 { animation: result-title-arrive .3s ease-out both; }
@keyframes result-arrive { from { opacity: 0; transform: translateY(6px); } to { opacity: 1; transform: translateY(0); } }
@keyframes result-title-arrive { from { opacity: .2; transform: translateY(3px); } to { opacity: 1; transform: translateY(0); } }

@media (max-width: 600px) {
  .result-panel { gap: 12px; padding: 16px; border-radius: 20px; }
  .result-panel h2 { font-size: 24px; }
  .result-reason { font-size: 12px; }
  .result-summary { gap: 8px 12px; padding-top: 10px; }
  .result-summary small, .result-summary p { font-size: 11px; }
  .result-summary strong { font-size: 14px; }
  .formation-cards :deep(.playing-card) { width: 38px; }
  .result-actions { gap: 8px; }
  :slotted(.primary-button) { flex: 1 1 auto; gap: 10px; padding-inline: 12px; font-size: 12px; }
}
/* 依中央區實際高度縮短留白，頁首與戰局紀錄佔用空間時仍完整顯示結算。 */
@container conclusion (max-height: 260px) {
  .result-panel { gap: 8px; padding: 10px 16px; border-radius: 20px; }
  .result-heading { gap: 4px; }
  .result-panel h2 { font-size: 24px; line-height: 1.2; }
  .result-reason { font-size: 12px; line-height: 1.5; }
  .result-summary { gap: 6px 12px; padding-top: 8px; }
  .result-summary-text { gap: 3px; }
  .result-summary small, .result-summary p { font-size: 11px; }
  .result-summary strong { font-size: 14px; }
  .formation-cards :deep(.playing-card) { width: 32px; }
}
@media (prefers-reduced-motion: reduce) { .result-enter, .result-enter h2 { animation: none; } }
</style>
