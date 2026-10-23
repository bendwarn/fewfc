<template>
  <header class="public-header ui-glass">
    <NuxtLink class="public-brand" to="/" aria-label="返回五行戰鬥牌首頁"><img :src="'/header-banner.svg'" alt="五行戰鬥牌" width="760" height="120"></NuxtLink>
    <nav aria-label="頁面導覽"><NuxtLink to="/">返回首頁</NuxtLink><ThemeSelector /></nav>
  </header>
</template>
<script setup lang="ts">
import ThemeSelector from './ThemeSelector.vue'
</script>
<style scoped>
.public-header { display: flex; width: min(calc(100% - 40px),1180px); margin: 18px auto 0; min-height: 76px; padding: 10px 24px; border-radius: 22px; align-items: center; justify-content: space-between; gap: 20px; }
.public-brand { min-width: 0; }
.public-brand img { display: block; width: min(34vw,290px); height: auto; border-radius: 6px; }
nav { display: flex; align-items: center; gap: 20px; }
nav > a { white-space: nowrap; color: var(--app-text); font-size: 12px; text-decoration: none; }
nav > a:hover { text-decoration: underline; }
@media(max-width:600px) { .public-header { width: calc(100% - 28px); min-height: 64px; padding: 10px 12px; border-radius: 18px; } .public-brand img { width: min(42vw,210px); } nav { gap: 12px; } }
</style>
