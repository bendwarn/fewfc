import { computed, ref } from 'vue'
import type { ColorThemePreference } from '../../app/lib/color-theme'

const preference = ref<ColorThemePreference>('system')
const resolvedTheme = computed(() => preference.value === 'dark' ? 'dark' : 'light')

export function useColorTheme() {
  return {
    preference,
    resolvedTheme,
    setPreference(value: ColorThemePreference) {
      preference.value = value
    },
  }
}
