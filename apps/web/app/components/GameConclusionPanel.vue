<template>
  <section class="result-panel">
    <template v-if="state.terminalResolution?.type === 'formation'">
      <small>{{ playerLabel(state.terminalResolution.player) }}</small>
      <strong>{{ state.terminalResolution.formationName ?? '陣法' }}</strong>
      <div class="formation-cards">
        <GameCard
          v-for="card in terminalFormationCards"
          :key="card.id"
          class="formation-card"
          :card="card"
          :interpretations="state.cardInterpretations"
        />
      </div>
      <p v-if="terminalHiddenCount">{{ terminalHiddenCount }} 張蓋牌</p>
    </template>
    <template v-else-if="state.terminalResolution?.type === 'discardRetrieval'">
      <small>{{ playerLabel(state.terminalResolution.player) }}</small>
      <strong>取回 {{ playerLabel(state.terminalResolution.previousPlayer) }} 的棄牌</strong>
      <div class="formation-cards"><GameCard class="formation-card" :card="state.terminalResolution.card" :interpretations="state.cardInterpretations" /></div>
    </template>
    <template v-else-if="state.terminalResolution?.type === 'automatic'">
      <strong>{{ state.terminalResolution.label }}</strong>
    </template>
    <h2>{{ resultText }}</h2>
    <p class="result-reason">{{ endReasonText }}</p>
    <div v-if="$slots.actions" class="result-actions">
      <slot name="actions" />
    </div>
  </section>
</template>

<script setup lang="ts">
import type { PublicCard, PublicCardRefs, PublicGameState, TeamId } from '~/types/fewfc'

const props = defineProps<{
  state: PublicGameState
  teamLabel: (team: TeamId) => string
  playerLabel?: (player: string) => string
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
      ?? `${props.teamLabel(cause.team)} 達成「${cause.rule}」的直接勝利條件`
  }).join('；')
})

const playerLabel = (player: string) => props.playerLabel?.(player) ?? player

const terminalFormationCards = computed<PublicCard[]>(() => {
  const resolution = props.state.terminalResolution
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

.result-panel { @apply grid min-h-20 content-center justify-items-center gap-2 px-4 py-3 text-center; }
.result-panel small { @apply text-[9px] text-muted; }
.result-panel > strong { @apply font-serif text-sm text-gold-light; }
.result-panel h2 { @apply mt-1 font-serif text-lg text-gold-light; }
.result-panel p { @apply text-xs text-muted; }
.result-panel .result-reason { @apply max-w-[34rem] text-[var(--app-text)]; }
.formation-cards { @apply flex min-h-10 items-center justify-center; }
.formation-cards :deep(.playing-card) { width: 34px; margin-left: -4px; }
.result-panel .result-actions { @apply mt-2 grid w-full max-w-56 grid-cols-1 gap-3; }
:slotted(.primary-button) { @apply justify-between; }

@media (max-width: 600px) {
  .result-panel { @apply px-3 py-2; }
  .result-panel h2 { @apply text-base; }
  .result-panel p { @apply text-[11px]; }
  .result-panel .result-actions { @apply max-w-48; }
  .formation-cards :deep(.playing-card) { width: 30px; }
}
</style>
