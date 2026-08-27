<template>
  <section class="public-rooms-card">
    <div class="panel-title">
      <h2>{{ heading }}</h2>
      <span>{{ badge }}</span>
    </div>
    <p v-if="!loading && !items.length" class="muted">{{ emptyMessage }}</p>
    <div v-else class="public-room-list">
      <button
        v-for="room in items"
        :key="room.gameId"
        type="button"
        :disabled="busy"
        @click="$emit('open', room.gameId)"
      >
        <span class="room-code">{{ room.code }}</span>
        <div>
          <strong>{{ room.name }} <span v-if="room.hasNotification" class="notification-dot" role="status" aria-label="有房間通知" /></strong>
          <small>{{ room.detail }}</small>
          <small v-if="room.ruleSummary">{{ room.ruleSummary }}</small>
        </div>
        <i>{{ room.actionLabel }}</i>
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
interface LobbyRoomListItem {
  gameId: string
  code: string
  name: string
  detail: string
  ruleSummary: string
  actionLabel: string
  hasNotification: boolean
}

defineProps<{
  heading: string
  badge: string | number
  emptyMessage: string
  loading: boolean
  busy: boolean
  items: readonly LobbyRoomListItem[]
}>()

defineEmits<{
  open: [gameId: string]
}>()
</script>

<style scoped>
@reference "../assets/css/main.css";

.public-rooms-card { @apply mt-0 border border-line bg-panel p-6; border-radius: 16px; box-shadow: var(--app-shadow-md); }
.public-rooms-card :deep(.panel-title) { @apply mb-4; }
.public-room-list { @apply grid gap-3; }
.public-room-list button { @apply grid grid-cols-[92px_1fr_auto] items-center gap-3 p-4 text-left; border: 1px solid var(--app-border); border-radius: 12px; background: var(--app-surface-muted); }
.public-room-list button:hover { border-color: var(--app-accent); background: var(--app-accent-soft); }
.public-room-list button:disabled { @apply cursor-not-allowed opacity-55; }
.public-room-list strong { @apply block text-sm; color: var(--app-text); }
.public-room-list small { @apply text-xs text-muted; }
.public-room-list i { @apply text-[10px] not-italic text-gold-light; }
.room-code { @apply font-mono text-[10px]; color: var(--app-text-muted); overflow-wrap: anywhere; }
.notification-dot { display: inline-block; width: 8px; height: 8px; margin-left: 6px; border-radius: 50%; background: var(--app-accent); vertical-align: middle; }
</style>
