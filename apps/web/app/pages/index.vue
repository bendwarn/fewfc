<template>
  <main class="route-state" aria-live="polite">
    <div class="route-state-content">
      <span class="route-spinner" aria-hidden="true" />
      <h1>正在前往房間</h1>
    </div>
  </main>
</template>

<script setup lang="ts">
onMounted(() => {
  void navigateTo('/rooms', { replace: true })
})
</script>
