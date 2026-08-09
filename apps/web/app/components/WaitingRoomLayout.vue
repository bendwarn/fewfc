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

.waiting-overlay { @apply absolute inset-0 flex items-start justify-center overflow-y-auto bg-[var(--app-overlay)] py-4 backdrop-blur-[4px]; z-index: 13; }
.waiting-panel { width: min(860px, calc(100% - 64px)); @apply my-auto grid grid-cols-[minmax(0,1.45fr)_minmax(240px,.75fr)] gap-6 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-7 text-left; border-radius: 16px; box-shadow: var(--app-shadow-lg); }
.waiting-main { @apply min-w-0; }
.waiting-side { @apply grid min-w-0 content-start gap-4 border-l border-[var(--app-border)] pl-6; }
.waiting-overlay h2 { @apply mt-0 font-serif text-3xl text-gold-light; }
.waiting-overlay p:not(.section-kicker) { @apply text-sm text-muted; }
.waiting-side :slotted(.result-actions) { @apply mt-0; }

@media (max-width: 600px) {
  .waiting-panel { min-width: 0; width: calc(100vw - 32px); grid-template-columns: 1fr; gap: 18px; padding: 22px 16px; }
  .waiting-side { @apply border-t border-l-0 pt-4 pl-0; }
}
</style>
