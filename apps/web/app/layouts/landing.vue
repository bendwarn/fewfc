<template>
  <div class="landing-layout">
    <slot />
  </div>
</template>

<style scoped>
.landing-layout {
  min-height: 100vh;
  overflow: hidden;
  background: var(--app-canvas);
  color: var(--app-text);
}
</style>
