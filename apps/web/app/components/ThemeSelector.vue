<template>
  <div ref="container" class="theme-selector">
    <button
      class="theme-selector-trigger"
      type="button"
      aria-haspopup="menu"
      :aria-expanded="open"
      :aria-label="`外觀：${activeLabel}`"
      title="切換外觀"
      @click.stop="open = !open"
      @keydown.esc.stop="close"
    >
      <span class="theme-selector-icon" aria-hidden="true">{{ activeIcon }}</span>
    </button>
    <div v-if="open" class="theme-selector-menu" role="menu" aria-label="外觀">
      <button
        v-for="option in options"
        :key="option.value"
        type="button"
        role="menuitemradio"
        :aria-checked="preference === option.value"
        @click="choose(option.value)"
      >
        <span aria-hidden="true">{{ option.icon }}</span>
        <span>{{ option.label }}</span>
        <i v-if="preference === option.value" aria-hidden="true">✓</i>
      </button>
      <small v-if="preference === 'system'">目前為{{ resolvedTheme === 'dark' ? '深色' : '亮色' }}</small>
    </div>
  </div>
</template>

<script setup lang="ts">
import type { ColorThemePreference } from '~/lib/color-theme'

const theme = useColorTheme()
const preference = theme.preference
const resolvedTheme = theme.resolvedTheme
const open = ref(false)
const container = ref<HTMLElement | null>(null)
const options: Array<{ value: ColorThemePreference, label: string, icon: string }> = [
  { value: 'system', label: '系統', icon: '◐' },
  { value: 'light', label: '亮色', icon: '☀' },
  { value: 'dark', label: '深色', icon: '☾' },
]
const activeOption = computed(() => options.find(option => option.value === preference.value) ?? options[0]!)
const activeLabel = computed(() => preference.value === 'system'
  ? `系統（目前${resolvedTheme.value === 'dark' ? '深色' : '亮色'}）`
  : activeOption.value.label)
const activeIcon = computed(() => activeOption.value.icon)

function choose(value: ColorThemePreference) {
  theme.setPreference(value)
  open.value = false
}

function close() {
  open.value = false
}

function closeOnOutsideClick(event: MouseEvent) {
  if (open.value && event.target instanceof Node && !container.value?.contains(event.target)) close()
}

onMounted(() => window.addEventListener('click', closeOnOutsideClick))
onBeforeUnmount(() => window.removeEventListener('click', closeOnOutsideClick))
</script>

<style scoped>
.theme-selector { position: relative; z-index: 50; }
.theme-selector-trigger {
  display: grid;
  width: 36px;
  height: 36px;
  padding: 0;
  place-items: center;
  border: 1px solid var(--app-border-strong);
  border-radius: 999px;
  background: var(--app-control);
  color: var(--app-text);
  box-shadow: var(--app-shadow-sm);
}
.theme-selector-trigger:hover,
.theme-selector-trigger:focus-visible { border-color: var(--app-accent); outline: none; }
.theme-selector-icon { font-size: 17px; line-height: 1; }
.theme-selector-menu {
  position: absolute;
  top: calc(100% + 10px);
  right: 0;
  display: grid;
  min-width: 160px;
  overflow: hidden;
  border: 1px solid var(--app-border);
  border-radius: 14px;
  background: var(--app-surface-raised);
  color: var(--app-text);
  box-shadow: var(--app-shadow-lg);
}
.theme-selector-menu button {
  display: grid;
  min-height: 42px;
  grid-template-columns: 24px 1fr 18px;
  align-items: center;
  gap: 8px;
  border: 0;
  background: transparent;
  padding: 9px 12px;
  text-align: left;
}
.theme-selector-menu button:hover,
.theme-selector-menu button:focus-visible { background: var(--app-accent-soft); outline: none; }
.theme-selector-menu button[aria-checked="true"] { color: var(--app-accent-strong); font-weight: 700; }
.theme-selector-menu i { color: var(--app-accent); font-style: normal; }
.theme-selector-menu small { padding: 6px 12px 10px 44px; color: var(--app-text-muted); font-size: 10px; }
</style>
