import {
  COLOR_THEME_STORAGE_KEY,
  explicitThemeAttribute,
  parseColorThemePreference,
  resolveColorTheme,
  type ColorThemePreference,
} from '~/lib/color-theme'

export function useColorTheme() {
  const preference = useState<ColorThemePreference>('color-theme-preference', () => 'system')
  const systemDark = useState('color-theme-system-dark', () => false)
  const resolvedTheme = computed(() => resolveColorTheme(preference.value, systemDark.value))

  function applyPreference(value: ColorThemePreference, persist = true) {
    if (!import.meta.client) return
    const explicitTheme = explicitThemeAttribute(value)
    if (explicitTheme) {
      document.documentElement.dataset.theme = explicitTheme
    } else {
      delete document.documentElement.dataset.theme
    }
    document.documentElement.style.colorScheme = resolvedTheme.value
    if (persist) localStorage.setItem(COLOR_THEME_STORAGE_KEY, value)
  }

  function setPreference(value: ColorThemePreference) {
    preference.value = value
  }

  watch(preference, value => applyPreference(value), { flush: 'sync' })
  watch(systemDark, () => {
    if (preference.value === 'system') applyPreference('system', false)
  })

  onMounted(() => {
    const media = window.matchMedia('(prefers-color-scheme: dark)')
    const updateSystemTheme = (event: MediaQueryListEvent | MediaQueryList) => {
      systemDark.value = event.matches
    }
    const syncStoredPreference = (event: StorageEvent) => {
      if (event.key !== COLOR_THEME_STORAGE_KEY) return
      preference.value = parseColorThemePreference(event.newValue)
    }

    systemDark.value = media.matches
    preference.value = parseColorThemePreference(localStorage.getItem(COLOR_THEME_STORAGE_KEY))
    applyPreference(preference.value, false)
    media.addEventListener('change', updateSystemTheme)
    window.addEventListener('storage', syncStoredPreference)

    onScopeDispose(() => {
      media.removeEventListener('change', updateSystemTheme)
      window.removeEventListener('storage', syncStoredPreference)
    })
  })

  return {
    preference: readonly(preference),
    resolvedTheme: readonly(resolvedTheme),
    setPreference,
  }
}
