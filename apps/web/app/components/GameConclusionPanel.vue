<template>
  <section class="result-panel">
    <h2>{{ resultText }}</h2>
    <p class="result-reason"><strong>終局原因</strong>{{ endReasonText }}</p>
    <p v-if="summary">{{ summary }}</p>
    <div v-if="$slots.actions" class="result-actions">
      <slot name="actions" />
    </div>
  </section>
</template>

<script setup lang="ts">
import type { PublicGameState, TeamId } from '~/types/fewfc'

const props = defineProps<{
  state: PublicGameState
  teamLabel: (team: TeamId) => string
  summary?: string
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
</script>

<style scoped>
@reference "../assets/css/main.css";

.result-panel { @apply border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-5; }
.result-panel h2 { @apply font-serif text-2xl text-gold-light; }
.result-panel p { @apply mt-1 text-xs text-muted; }
.result-panel .result-reason { @apply mt-3 border-l-2 border-[var(--app-accent)] pl-2 text-[var(--app-text)]; }
.result-reason strong { @apply mr-2 text-gold-light; }
.result-panel .result-actions { @apply mt-2 grid grid-cols-1 gap-3; }
:slotted(.primary-button) { @apply justify-between; }
.result-panel.battlefield-conclusion {
  @apply absolute inset-0 z-15 grid min-h-0 min-w-0 justify-items-center overflow-y-auto border bg-[rgba(23,28,25,.96)] px-4 py-3 text-center backdrop-blur-[5px];
  align-content: safe center;
  overscroll-behavior: contain;
}
.battlefield-conclusion h2 { @apply text-xl; }
.battlefield-conclusion p { @apply max-w-[34rem]; }
.battlefield-conclusion .result-reason { @apply text-left; }
.battlefield-conclusion .result-actions { @apply w-full max-w-56; }

@media (max-width: 600px) {
  .result-panel.battlefield-conclusion { @apply px-3 py-2; }
  .battlefield-conclusion h2 { @apply text-lg; }
  .battlefield-conclusion p { @apply text-[11px]; }
  .battlefield-conclusion .result-reason { @apply mt-2; }
  .battlefield-conclusion .result-actions { @apply mt-2 max-w-48; }
}
</style>
