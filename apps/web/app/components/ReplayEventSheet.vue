<template>
  <Teleport to="body">
    <div v-if="open" class="replay-event-sheet-portal">
      <button class="replay-event-sheet-backdrop" type="button" tabindex="-1" aria-label="關閉戰局紀錄" @click="emit('close')" />
      <section
        ref="dialog"
        id="replay-mobile-event-sheet"
        class="replay-event-sheet"
        role="dialog"
        aria-modal="true"
        aria-labelledby="replay-mobile-event-sheet-title"
        @keydown.stop="handleKeydown"
      >
        <header>
          <h2 id="replay-mobile-event-sheet-title">戰局紀錄</h2>
          <button ref="closeButton" type="button" aria-label="關閉戰局紀錄" @click="emit('close')">×</button>
        </header>
        <label class="replay-mobile-perspective">重播視角
          <select :value="perspective" aria-label="重播視角" @change="changePerspective">
            <option v-for="player in players" :key="player.player" :value="player.player">{{ player.displayName }}</option>
          </select>
        </label>
        <div ref="feed" class="replay-event-sheet-feed" @scroll.passive="trackLatestPosition">
          <BattleRecordTimeline :groups="groups" :current-turn-number="currentTurnNumber" />
        </div>
      </section>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import { nextTick, ref, watch } from 'vue'
import BattleRecordTimeline, { type BattleRecordTimelineGroup } from './BattleRecordTimeline.vue'
import { battleRecordContentRevision, scrollBattleRecordToLatest } from '~/lib/battle-record-scroll'
import type { PlayerId } from '~/types/fewfc'

const props = defineProps<{
  open: boolean
  perspective: PlayerId
  players: readonly { player: PlayerId, displayName: string }[]
  groups: readonly BattleRecordTimelineGroup[]
  currentTurnNumber: number | null
}>()

const emit = defineEmits<{
  close: []
  'update:perspective': [player: PlayerId]
}>()

const dialog = ref<HTMLElement | null>(null)
const closeButton = ref<HTMLButtonElement | null>(null)
const feed = ref<HTMLElement | null>(null)
const followsLatest = ref(true)

watch(() => props.open, (open) => {
  if (open) {
    followsLatest.value = true
    void nextTick(() => {
      scrollBattleRecordToLatest(feed.value)
      closeButton.value?.focus()
    })
  }
})

watch(() => battleRecordContentRevision(props.groups), () => {
  if (props.open && followsLatest.value) void nextTick(() => scrollBattleRecordToLatest(feed.value))
})

function trackLatestPosition() {
  const element = feed.value
  if (!element) return
  followsLatest.value = element.scrollHeight - element.scrollTop - element.clientHeight <= 24
}

function changePerspective(event: Event) {
  emit('update:perspective', (event.currentTarget as HTMLSelectElement).value as PlayerId)
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
  if (event.key !== 'Tab') return
  const focusable = Array.from(dialog.value?.querySelectorAll<HTMLElement>(
    'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
  ) ?? [])
  if (!focusable.length) return
  const first = focusable[0]!
  const last = focusable.at(-1)!
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}
</script>

<style scoped>
@reference "../assets/css/main.css";

.replay-event-sheet-portal { @apply fixed inset-0 z-50; }
.replay-event-sheet-backdrop { @apply absolute inset-0 size-full cursor-default border-0 bg-[var(--app-overlay)] p-0 backdrop-blur-[3px]; }
.replay-event-sheet { @apply fixed right-0 bottom-0 left-0 z-1 grid max-h-[72dvh] grid-rows-[auto_auto_minmax(0,1fr)] overflow-hidden rounded-t-2xl border-x-0 border-b-0 border-t border-[var(--app-accent)] bg-[var(--app-surface-raised)] text-[var(--app-text)] shadow-[0_-18px_48px_rgba(0,0,0,.45)]; }
.replay-event-sheet > header { @apply flex items-center justify-between border-b border-line px-4 py-3; }
.replay-event-sheet h2 { @apply font-serif text-base text-gold-light; }
.replay-event-sheet header button { @apply grid size-8 place-items-center border border-[var(--app-border-strong)] bg-transparent text-xl text-muted; }
.replay-mobile-perspective { @apply flex items-center border-b border-line px-4 py-2 text-xs text-[var(--app-text-muted)]; }
.replay-mobile-perspective select { @apply ml-2 max-w-44 border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-2 py-1 text-xs text-[var(--app-text)]; }
.replay-event-sheet-feed { @apply min-h-0 overflow-y-auto p-4; overscroll-behavior: contain; }
</style>
