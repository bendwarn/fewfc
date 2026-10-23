<template>
  <main class="route-state">
    <section class="route-state-content" :aria-live="loading ? 'polite' : undefined">
      <span v-if="loading" class="route-spinner" aria-hidden="true" />
      <span v-else class="route-status-symbol" aria-hidden="true">{{ code === 404 ? '迷' : '歇' }}</span>
      <span v-if="code" class="page-kicker">{{ code }}</span>
      <h1>{{ title }}</h1>
      <p>{{ description }}</p>
      <button v-if="!loading" class="primary-button" type="button" @click="$emit('action')">{{ actionLabel }}</button>
    </section>
  </main>
</template>
<script setup lang="ts">
withDefaults(defineProps<{ title: string; description: string; code?: number; loading?: boolean; actionLabel?: string }>(), { loading: false, actionLabel: '返回首頁' })
defineEmits<{ action: [] }>()
</script>
<style scoped>
.route-status-symbol { display: grid; place-items: center; width: 60px; height: 60px; border: 1px solid var(--ui-glass-stroke); border-radius: 50%; color: var(--app-accent-strong); font: 26px var(--font-serif); background: var(--app-accent-soft); }
</style>
