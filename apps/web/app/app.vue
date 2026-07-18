<template>
  <NuxtRouteAnnouncer />
  <NuxtLayout :name="layoutName">
    <NuxtPage />
  </NuxtLayout>
</template>

<script setup lang="ts">
const route = useRoute()
const layoutName = computed(() => typeof route.meta.layout === 'string' ? route.meta.layout : 'default')
</script>
