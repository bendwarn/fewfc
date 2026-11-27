<template>
  <svg class="deck-icon" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
    <path :d="paths[name]" />
  </svg>
</template>

<script setup lang="ts">
defineProps<{ name: keyof typeof paths }>()

const paths = {
  add: 'M12 5v14M5 12h14',
  remove: 'M5 12h14',
  import: 'M12 3v11m-4-4 4 4 4-4M4 15v5h16v-5',
  export: 'M12 15V4m-4 4 4-4 4 4M4 15v5h16v-5',
  reset: 'M3 11a9 9 0 1 1 2.5 7M3 5v6h6',
  retry: 'M21 11a9 9 0 1 0-2.5 7M21 5v6h-6',
  back: 'm10 6-6 6 6 6M4 12h16',
  save: 'M5 3h12l4 4v14H3V3h2Zm2 0v6h10V3M7 21v-8h10v8',
  check: 'm5 12 4 4L19 6',
  close: 'm6 6 12 12M18 6 6 18',
  alert: 'M12 8v5m0 3h.01M12 3a9 9 0 1 0 0 18 9 9 0 0 0 0-18Z',
  previous: 'm14 6-6 6 6 6',
  next: 'm10 6 6 6-6 6',
}
</script>

<style scoped>
.deck-icon { width: 1.15em; height: 1.15em; flex: none; }
</style>
