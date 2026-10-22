<template>
  <section class="public-rooms-card lobby-theme battle-glass" :aria-label="heading" :aria-busy="loading">
    <div class="panel-title">
      <h2><span class="room-list-emblem" aria-hidden="true">{{ heading === '我的房間' ? '◇' : '✧' }}</span>{{ heading }}</h2>
      <span>{{ badge }}</span>
    </div>
    <div v-if="loading && !items.length" class="room-list-loading" aria-hidden="true">
      <div v-for="index in 3" :key="index"><i /><span /><span /></div>
    </div>
    <div v-else-if="!items.length" class="room-list-empty"><span aria-hidden="true">✧</span><p>{{ emptyMessage }}</p></div>
    <div v-else class="public-room-list">
      <button
        v-for="room in items"
        :key="room.gameId"
        type="button"
        :disabled="busy"
        @click="$emit('open', room.gameId)"
      >
        <span class="room-code"><small>房碼</small>{{ room.code }}</span>
        <span class="room-description">
          <strong>{{ room.name }} <span v-if="room.hasNotification" class="notification-dot" role="status" aria-label="有房間通知" /></strong>
          <small>{{ room.detail }}</small>
          <small v-if="room.ruleSummary">{{ room.ruleSummary }}</small>
        </span>
        <i>{{ room.actionLabel }} <span aria-hidden="true">↗</span></i>
      </button>
    </div>
  </section>
</template>

<script setup lang="ts">
import type { LobbyRoomListItem } from '~/lib/lobby-presentation'

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

.public-rooms-card { min-width: 0; padding: 24px; border-radius: 26px; }
.panel-title { display: flex; align-items: center; justify-content: space-between; gap: 12px; margin-bottom: 18px; padding-bottom: 18px; border-bottom: 1px solid color-mix(in srgb, var(--glass-stroke) 45%, transparent); }
.panel-title h2 { display: flex; align-items: center; gap: 10px; font: 700 21px var(--font-serif); }
.room-list-emblem { display: grid; place-items: center; width: 30px; height: 30px; border: 1px solid var(--glass-stroke); border-radius: 50%; color: var(--app-accent-strong); font-size: 22px; }
.panel-title > span { padding: 5px 10px; border-radius: 999px; background: color-mix(in srgb, var(--app-accent) 12%, transparent); color: var(--app-accent-strong); font-size: 11px; }
.public-room-list { display: grid; gap: 10px; }
.public-room-list button { display: grid; grid-template-columns: 72px minmax(0, 1fr) auto; align-items: center; gap: 14px; min-height: 104px; padding: 16px; text-align: left; border: 1px solid color-mix(in srgb, var(--glass-stroke) 55%, transparent); border-radius: 18px; background: linear-gradient(135deg, rgba(230, 248, 234, .12), transparent 60%), color-mix(in srgb, var(--glass-surface-strong) 65%, transparent); box-shadow: inset 0 1px rgba(246, 255, 245, .1); transition: border-color .18s, background .18s; }
.public-room-list button:hover:not(:disabled) { border-color: var(--app-accent); background: color-mix(in srgb, var(--app-accent) 13%, var(--glass-surface-strong)); }
.public-room-list button:focus-visible { outline: 2px solid var(--app-accent-strong); outline-offset: 3px; }
.public-room-list button:disabled { @apply cursor-not-allowed opacity-55; }
.room-description { display: grid; min-width: 0; gap: 4px; }
.public-room-list strong { color: var(--app-text); font-size: 15px; line-height: 1.6; overflow-wrap: anywhere; }
.room-description small { color: var(--battle-text-secondary); font-size: 11px; line-height: 1.7; }
.public-room-list i { display: flex; align-items: center; gap: 8px; color: var(--app-accent-strong); font-size: 12px; font-style: normal; white-space: nowrap; }
.public-room-list i > span { font-size: 16px; }
.room-code { display: grid; gap: 5px; align-content: center; align-self: stretch; border-right: 1px solid color-mix(in srgb, var(--glass-stroke) 30%, transparent); padding-right: 12px; font-family: ui-monospace, monospace; font-size: 11px; color: var(--app-text); overflow-wrap: anywhere; }
.room-code small { font-family: var(--font-sans); font-size: 10px; color: var(--battle-text-secondary); }
.notification-dot { display: inline-block; width: 8px; height: 8px; margin-left: 6px; border-radius: 50%; background: var(--app-accent); vertical-align: middle; }
.room-list-empty { display: grid; justify-items: center; align-content: center; gap: 12px; min-height: 190px; padding: 24px 12px; color: var(--battle-text-secondary); text-align: center; font-size: 13px; }
.room-list-empty > span { font-size: 38px; color: var(--app-accent-strong); opacity: .65; }
.room-list-loading { display: grid; gap: 10px; }
.room-list-loading > div { display: grid; grid-template-columns: 50px 1fr; gap: 10px 16px; padding: 20px 16px; border: 1px solid color-mix(in srgb, var(--glass-stroke) 30%, transparent); border-radius: 18px; }
.room-list-loading i { grid-row: span 2; border-radius: 8px; background: color-mix(in srgb, var(--app-text) 8%, transparent); }
.room-list-loading span { height: 13px; border-radius: 4px; background: color-mix(in srgb, var(--app-text) 10%, transparent); }
.room-list-loading span:last-child { width: 60%; }
@media (max-width: 600px) {
  .public-rooms-card { padding: 18px 14px; border-radius: 22px; }
  .panel-title { margin-bottom: 14px; padding: 0 4px 14px; }
  .panel-title h2 { font-size: 18px; }
  .public-room-list button { grid-template-columns: 56px minmax(0, 1fr) auto; gap: 10px; padding: 14px 10px; }
  .room-code { font-size: 10px; padding-right: 8px; }
  .public-room-list strong { font-size: 13px; }
  .public-room-list i { gap: 3px; font-size: 11px; }
}
</style>
