<template>
  <header class="page-intro">
    <div class="page-intro-copy">
      <NuxtLink v-if="backTo" class="page-back" :to="backTo">← {{ backLabel }}</NuxtLink>
      <span v-if="eyebrow" class="page-kicker">{{ eyebrow }}</span>
      <h1>{{ title }}</h1>
      <p v-if="description">{{ description }}</p>
      <slot name="description" />
    </div>
    <slot />
  </header>
</template>
<script setup lang="ts">
defineProps<{ title: string; eyebrow?: string; description?: string; backTo?: string; backLabel?: string }>()
</script>
