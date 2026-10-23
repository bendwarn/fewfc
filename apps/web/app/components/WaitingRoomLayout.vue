<template>
  <div class="waiting-overlay">
    <div class="waiting-panel">
      <section class="waiting-main">
        <slot name="main" />
      </section>
      <aside class="waiting-side" aria-label="房間成員與操作">
        <slot name="side" />
      </aside>
    </div>
  </div>
</template>

<style scoped>
@reference "../assets/css/main.css";

.waiting-overlay { @apply absolute inset-0 flex items-start justify-center overflow-y-auto py-6; z-index: 13; background: linear-gradient(rgba(9,20,15,.28),rgba(9,20,15,.5)), url('/backgrounds/private-room.webp') center / cover; }
.waiting-panel { width: min(860px, calc(100% - 64px)); @apply my-auto grid grid-cols-[minmax(0,1.45fr)_minmax(240px,.75fr)] gap-6 border p-7 text-left; border-color: var(--ui-glass-stroke); border-radius: 28px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); backdrop-filter: blur(22px); -webkit-backdrop-filter: blur(22px); }
.waiting-main { @apply min-w-0; }
.waiting-side { @apply grid min-w-0 content-start gap-4 border-l border-[var(--app-border)] pl-6; }
.waiting-main :deep(h2) { @apply mt-0 font-serif text-3xl text-gold-light; }
.waiting-main :deep(p:not(.section-kicker)) { @apply text-sm text-muted; }
.waiting-side :slotted(.result-actions) { @apply mt-0; }

@media (max-width: 600px) {
  .waiting-panel { min-width: 0; width: calc(100vw - 32px); grid-template-columns: 1fr; gap: 18px; padding: 22px 16px; }
  .waiting-side { @apply border-t border-l-0 pt-4 pl-0; }
}
</style>

<style>
/* 席位與操作由等待室擁有，正式頁面與整頁故事共用同一份樣式。 */
.waiting-overlay .waiting-members { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: 12px; }
.waiting-overlay .waiting-members > span { display: grid; gap: 8px; padding: 14px 12px; border: 1px solid var(--ui-glass-stroke); border-radius: 14px; background: color-mix(in srgb, var(--app-surface-muted) 72%, transparent); font-size: 13px; color: var(--app-text-muted); overflow-wrap: anywhere; }
.waiting-overlay .waiting-members > .joined { border-color: var(--app-accent); color: var(--app-text); }
.waiting-overlay .waiting-members small, .waiting-overlay .waiting-observers small { font-size: 11px; color: var(--app-text-muted); }
.waiting-overlay :is(.waiting-members, .waiting-observers) button { min-height: 28px; border: 0; background: transparent; font-size: 11px; color: var(--app-danger); }
.waiting-overlay .waiting-observers { display: grid; gap: 10px; }
.waiting-overlay .waiting-observers h3 { margin: 0; font-size: 13px; }
.waiting-overlay .waiting-observers > span { display: flex; align-items: center; flex-wrap: wrap; gap: 8px; padding: 10px; border: 1px solid var(--ui-glass-stroke); border-radius: 12px; background: var(--app-surface-muted); font-size: 12px; }
.waiting-overlay .waiting-observers button { margin-left: auto; }
.waiting-overlay .result-actions { display: flex; flex-direction: column-reverse; gap: 12px; margin: 0; }
</style>
