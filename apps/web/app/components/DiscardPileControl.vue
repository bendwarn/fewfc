<template>
  <div
    class="discard-pile"
    :class="[
      { disabled: unavailable },
      position ? `discard-position-${position}` : '',
    ]"
  >
    <span>{{ ownerLabel ? `${ownerLabel} 棄牌` : '棄牌' }}</span>
    <button
      class="discard-pile-trigger"
      type="button"
      aria-haspopup="dialog"
      aria-controls="discard-composition"
      :aria-expanded="open"
      :aria-disabled="unavailable"
      :aria-label="`查看${ownerLabel ? `${ownerLabel}的` : ''}棄牌內容，共 ${count} 張`"
      @click.stop="requestToggle"
    >
      {{ count }}
    </button>
  </div>
</template>

<script setup lang="ts">
const props = withDefaults(defineProps<{
  owner: string | null
  ownerLabel?: string
  count: number
  unavailable: boolean
  open: boolean
  position?: 'top' | 'left' | 'right' | 'bottom' | null
}>(), {
  ownerLabel: '',
  position: null,
})

const emit = defineEmits<{
  toggle: [trigger: HTMLButtonElement]
}>()

function requestToggle(event: MouseEvent) {
  if (props.unavailable) return
  emit('toggle', event.currentTarget as HTMLButtonElement)
}
</script>

<style scoped>
@reference "../assets/css/main.css";

.discard-pile { @apply grid justify-items-center gap-1.5 text-[9px] text-[var(--app-text-muted)]; }
:global(.discard-piles.personal) .discard-pile { @apply pointer-events-auto absolute w-[90px]; }
.discard-position-top { top: 0; left: 0; }
.discard-position-left { bottom: 0; left: 0; }
.discard-position-right { top: 0; right: 0; }
.discard-position-bottom { right: 0; bottom: 0; }
.discard-pile-trigger { @apply grid w-[52px] place-items-center border border-[var(--app-accent)] bg-[var(--app-surface-raised)] font-serif text-xl text-[#a68d56] p-0 hover:border-[var(--app-accent)] hover:text-gold-light; aspect-ratio: 5/7; }
.discard-pile-trigger:focus-visible { outline: 2px solid #d1ad62; outline-offset: 3px; }
.discard-pile.disabled .discard-pile-trigger { @apply cursor-not-allowed opacity-45; }

@media (min-width: 901px) {
  .discard-pile { font-size: 11px; }
}

@media (max-width: 600px) {
  :global(.discard-piles.personal) .discard-pile { width: 48px; }
  .discard-pile-trigger { width: 38px; }
}
</style>
