<template>
  <div
    class="discard-pile-control"
    :class="[
      { disabled: unavailable },
      position ? `discard-position-${position}` : '',
    ]"
  >
    <span class="discard-pile-label">{{ scopeLabel }}</span>
    <button
      class="discard-pile-trigger"
      type="button"
      aria-haspopup="dialog"
      aria-controls="discard-composition"
      :aria-expanded="open"
      :aria-disabled="unavailable"
      :disabled="unavailable && !featuredCard"
      :aria-label="detailLabel"
      @click.stop="requestToggle"
    >
      <GameCard
        v-if="featuredCard"
        class="featured-discard-card"
        :card="featuredCard"
        :selectable="false"
      />
      <span v-else class="discard-counts" aria-hidden="true">
        <span>{{ count }}</span>
        <span class="discard-count-divider">—</span>
        <span>{{ deckCount }}</span>
      </span>
    </button>
  </div>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import GameCard from '~/components/GameCard.vue'
import type { PublicCard } from '~/types/fewfc'

const props = withDefaults(defineProps<{
  owner: string | null
  ownerLabel?: string
  count: number
  deckCount?: number
  featuredCard?: PublicCard | null
  unavailable: boolean
  open: boolean
  // 暫時保留給既有棋盤呼叫端相容；元件本身不再假設絕對定位。
  position?: 'top' | 'left' | 'right' | 'bottom' | null
}>(), {
  ownerLabel: '',
  deckCount: 0,
  featuredCard: null,
  position: null,
})

const emit = defineEmits<{
  toggle: [trigger: HTMLButtonElement]
}>()

const scopeLabel = computed(() => {
  if (props.ownerLabel) return `${props.ownerLabel}的棄牌`
  if (props.owner) return `${props.owner}的棄牌`
  return '共用棄牌'
})

const detailLabel = computed(() => {
  const parts = [`查看${scopeLabel.value}詳情。目前棄牌 ${props.count} 張，牌庫 ${props.deckCount} 張`]
  if (props.featuredCard) parts.push(`上回合棄牌：${props.featuredCard.label}`)
  return parts.join('。')
})

function requestToggle(event: MouseEvent) {
  if (props.unavailable) return
  emit('toggle', event.currentTarget as HTMLButtonElement)
}
</script>

<style scoped>
@reference "../assets/css/main.css";

.discard-pile-control { @apply grid justify-items-center gap-1.5 text-[9px] text-[var(--app-text-muted)]; }
.discard-pile-label { @apply text-center; }
.discard-pile-trigger { @apply grid w-[52px] place-items-center overflow-hidden font-serif text-xl text-[#d4b76d] p-0; aspect-ratio: 5 / 7; border: 1px solid color-mix(in srgb, var(--app-text) 29%, transparent); border-radius: 10px; background: linear-gradient(140deg, rgba(239,255,246,.17), rgba(122,174,143,.035) 54%, transparent 78%), color-mix(in srgb, var(--app-surface-raised) 72%, transparent); box-shadow: inset 0 1px rgba(255,255,255,.24), 0 8px 18px rgba(0,0,0,.18); -webkit-backdrop-filter: saturate(1.22) blur(14px); backdrop-filter: saturate(1.22) blur(14px); transition: border-color .16s ease, transform .16s ease; }
.discard-pile-trigger:hover:not(:disabled) { border-color: color-mix(in srgb, var(--app-accent) 75%, white); transform: translateY(-1px); }
.discard-pile-trigger:focus-visible { outline: 2px solid #d1ad62; outline-offset: 3px; }
.discard-pile-control.disabled .discard-pile-trigger { @apply cursor-not-allowed opacity-45; }
.discard-pile-trigger > .featured-discard-card { width: auto; max-width: calc(100% - 4px); height: calc(100% - 4px); box-shadow: none; }
.discard-counts { @apply grid w-full place-items-center py-1 leading-none; aspect-ratio: 5 / 7; }
.discard-count-divider { @apply text-sm text-[var(--app-text-muted)]; }

@media (min-width: 901px) {
  .discard-pile-control { font-size: 11px; }
}

@media (max-width: 600px) {
  .discard-pile-trigger { width: 38px; }
}
</style>
