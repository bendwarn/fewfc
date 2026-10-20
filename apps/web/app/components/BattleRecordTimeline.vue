<template>
  <ol class="battle-record-timeline" aria-label="依回合分組的戰局紀錄">
    <li
      v-for="group in groups"
      :key="group.id"
      class="battle-record-group"
      :class="{ 'is-current': group.turnNumber != null && group.turnNumber === currentTurnNumber }"
      :aria-current="group.turnNumber != null && group.turnNumber === currentTurnNumber ? 'step' : undefined"
    >
      <header class="battle-record-group-heading">
        <span class="battle-record-group-icon" aria-hidden="true">◉</span>
        <strong>{{ group.title }}</strong>
        <span v-if="group.turnNumber != null && group.turnNumber === currentTurnNumber" class="battle-record-current-label">目前回合</span>
      </header>
      <ol class="battle-record-events">
        <li v-for="entry in group.entries" :key="entry.id">
          <i aria-hidden="true" />
          <div>
            <strong>{{ entry.title }}</strong>
            <p v-if="entry.summary">{{ entry.summary }}</p>
          </div>
        </li>
        <li v-if="!group.entries.length && group.turnNumber === currentTurnNumber" class="battle-record-empty-current">
          <i aria-hidden="true" />
          <div><span>等待本回合的新紀錄</span></div>
        </li>
      </ol>
    </li>
    <li v-if="!groups.length" class="battle-record-empty">尚無戰局紀錄。</li>
  </ol>
</template>

<script setup lang="ts">
export interface BattleRecordTimelineEntry {
  id: string
  title: string
  summary?: string | null
}

export interface BattleRecordTimelineGroup {
  id: string
  title: string
  turnNumber?: number | null
  entries: readonly BattleRecordTimelineEntry[]
}

defineProps<{
  groups: readonly BattleRecordTimelineGroup[]
  currentTurnNumber?: number | null
}>()
</script>

<style scoped>
@reference "../assets/css/main.css";

.battle-record-timeline { @apply grid list-none content-start gap-4 p-0; }
.battle-record-group { @apply grid min-w-0 gap-2; }
.battle-record-group-heading { @apply flex min-h-8 min-w-0 items-center gap-2 rounded-full border px-2.5 py-1; border-color: color-mix(in srgb, var(--app-border-strong) 42%, transparent); background: linear-gradient(135deg, color-mix(in srgb, var(--app-text) 7%, transparent), transparent 58%), color-mix(in srgb, var(--app-surface-raised) 48%, transparent); }
.battle-record-group-icon { @apply grid size-[18px] shrink-0 place-items-center rounded-full border text-[8px]; border-color: color-mix(in srgb, var(--app-border-strong) 58%, transparent); color: var(--app-accent-strong); }
.battle-record-group-heading strong { @apply min-w-0 flex-1 truncate font-serif text-sm; color: var(--app-text); }
.battle-record-current-label { @apply shrink-0 rounded-full px-2 py-1 text-[8px] font-bold; background: var(--app-accent-soft); color: var(--app-accent-strong); }
.battle-record-group.is-current .battle-record-group-heading { border-color: color-mix(in srgb, var(--app-accent) 82%, transparent); background: linear-gradient(135deg, color-mix(in srgb, var(--app-accent) 22%, transparent), color-mix(in srgb, var(--app-surface-raised) 70%, transparent)); box-shadow: inset 0 1px color-mix(in srgb, var(--app-text) 20%, transparent), 0 0 16px color-mix(in srgb, var(--app-accent) 14%, transparent); }
.battle-record-group.is-current .battle-record-group-icon { border-color: color-mix(in srgb, var(--app-accent-strong) 80%, transparent); background: color-mix(in srgb, var(--app-accent) 22%, transparent); }
.battle-record-events { @apply relative grid list-none gap-2.5 p-0 pl-1; }
.battle-record-events::before { @apply absolute top-2 bottom-2 left-[7px] w-px; background: linear-gradient(color-mix(in srgb, var(--app-border-strong) 68%, transparent), color-mix(in srgb, var(--app-border) 30%, transparent)); content: ''; }
.battle-record-events li { @apply relative grid min-w-0 grid-cols-[15px_minmax(0,1fr)] gap-2; }
.battle-record-events li > i { @apply relative z-1 mt-1 size-[9px] rounded-full border-2; border-color: color-mix(in srgb, var(--app-accent-strong) 70%, var(--app-border-strong)); background: var(--app-surface); box-shadow: 0 0 0 3px color-mix(in srgb, var(--app-accent) 10%, transparent); }
.battle-record-events li strong { @apply text-[13px] font-bold; color: var(--app-text); }
.battle-record-events li p { @apply mt-0.5 text-xs leading-[1.7]; color: var(--battle-text-secondary, var(--app-text-muted)); }
.battle-record-empty-current span, .battle-record-empty { @apply text-xs; color: var(--battle-text-secondary, var(--app-text-muted)); }
.battle-record-group.is-current .battle-record-events li > i { border-color: var(--app-accent-strong); background: color-mix(in srgb, var(--app-accent) 72%, var(--app-surface)); box-shadow: 0 0 0 3px color-mix(in srgb, var(--app-accent) 16%, transparent); }

@media (min-width: 901px) {
  .battle-record-group-heading strong { @apply text-base; }
  .battle-record-events li strong { @apply text-sm; }
  .battle-record-events li p { @apply text-[13px]; }
}
</style>
