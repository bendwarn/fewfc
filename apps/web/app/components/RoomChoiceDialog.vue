<template>
  <ModalShell :open="true" root-class="choice-overlay" panel-class="choice-overlay-panel">
    <RoomOperationClock v-if="clock" :clock="clock" :player-label="playerLabel" />
    <slot />
  </ModalShell>
</template>

<script setup lang="ts">
import ModalShell from './ModalShell.vue'
import RoomOperationClock from './RoomOperationClock.vue'
import type { RoomClockSample } from '~/lib/room-clock'
import type { PlayerId } from '~/types/fewfc'

defineProps<{
  clock?: RoomClockSample
  playerLabel: (player: PlayerId) => string
}>()
</script>
