<template>
  <div class="error-page">
    <PublicHeader />
    <RouteStatus :code="error?.statusCode" :title="error?.statusCode === 404 ? '此處沒有找到頁面' : '暫時無法開啟頁面'" :description="error?.statusCode === 404 ? '連結可能已變更，回到首頁重新入局。' : '請稍後再試，或先回到首頁。'" @action="clearError({ redirect: '/' })" />
  </div>
</template>
<script setup lang="ts">
import type { NuxtError } from '#app'
defineProps<{ error: NuxtError }>()
</script>
<style scoped>
.error-page { min-height: 100dvh; display: flow-root; background: radial-gradient(ellipse at top,var(--app-accent-soft),transparent 65%),var(--app-canvas); color: var(--app-text); }
</style>
