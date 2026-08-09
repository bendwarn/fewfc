<template>
  <main class="waiting-room-fixture">
    <WaitingRoomLayout>
      <template #main>
        <h2>桌面等待層</h2>
        <p>等待其他玩家加入並準備。</p>
        <div class="fixture-rules" aria-label="規則設定">完整規則設定</div>
      </template>
      <template #side>
        <div class="waiting-members">
          <span class="joined">房主<small>房主</small></span>
          <span>玩家 2<small>等待中</small></span>
        </div>
        <div class="result-actions">
          <button type="button">解散</button>
          <button class="primary-button" type="button">開始遊戲 <span>→</span></button>
        </div>
      </template>
    </WaitingRoomLayout>
  </main>
</template>

<script setup lang="ts">
if (!import.meta.dev) {
  throw createError({ statusCode: 404, statusMessage: 'Not found.' })
}

definePageMeta({ layout: false })
</script>

<style scoped>
@reference "../../assets/css/main.css";

.waiting-room-fixture { position: relative; height: 100vh; overflow: hidden; background: var(--app-battlefield-mid); }
.fixture-rules { min-height: 240px; margin-top: 24px; border: 1px solid var(--app-border); padding: 16px; color: var(--app-text-muted); }
.waiting-members { @apply grid grid-cols-2 gap-3; }
.waiting-members span { @apply grid gap-1 border border-[var(--app-border)] bg-[var(--app-surface-muted)] p-3 text-sm text-muted; }
.waiting-members span.joined { @apply border-[var(--app-accent)] text-[var(--app-text)]; }
.waiting-members small { @apply text-[10px] text-muted; }
.result-actions { @apply mt-2 grid grid-cols-2 gap-3; }
.result-actions .primary-button { @apply justify-between; }
</style>
