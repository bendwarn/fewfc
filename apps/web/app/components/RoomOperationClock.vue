<template>
  <div class="room-operation-clock" aria-label="限時模式">
    <span>等待 {{ clock.players.map(playerLabel).join('、') }} 操作</span>
    <span class="clock-value" role="timer" aria-label="剩餘操作時間" :data-urgent="seconds <= 10">{{ timeLabel }}</span>
    <span v-if="seconds === 0" class="clock-confirmation" role="status">等待伺服器確認逾時</span>
  </div>
</template>

<script setup lang="ts">
import { computed, onMounted, onUnmounted, ref } from 'vue'
import type { RoomClockSample } from '~/lib/room-clock'
import type { PlayerId } from '~/types/fewfc'

const props = defineProps<{
  clock: RoomClockSample
  playerLabel: (player: PlayerId) => string
}>()
const now = ref(performance.now())
const seconds = computed(() => Math.max(0, Math.ceil((props.clock.localDeadlineMs - now.value) / 1000)))
const timeLabel = computed(() => `${Math.floor(seconds.value / 60)}:${String(seconds.value % 60).padStart(2, '0')}`)
let timer: ReturnType<typeof setInterval> | undefined

onMounted(() => {
  timer = setInterval(() => { now.value = performance.now() }, 200)
})
onUnmounted(() => clearInterval(timer))
</script>

<style scoped>
.room-operation-clock {
  display: flex;
  flex: none;
  flex-wrap: wrap;
  justify-content: center;
  align-items: center;
  gap: 4px 12px;
  padding: 6px 14px;
  border-bottom: 1px solid var(--app-border);
  background: var(--app-surface-raised);
  color: var(--app-text);
  font-size: 13px;
  overflow-wrap: anywhere;
}
.clock-value { font-weight: 700; font-variant-numeric: tabular-nums; }
.clock-value[data-urgent="true"] { color: var(--app-accent-strong); }
.clock-confirmation { color: var(--app-text-muted); }
</style>
