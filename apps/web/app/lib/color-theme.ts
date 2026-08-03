export const COLOR_THEME_STORAGE_KEY = 'fewfc-color-theme'

export const COLOR_THEME_PREFERENCES = ['system', 'light', 'dark'] as const

export type ColorThemePreference = typeof COLOR_THEME_PREFERENCES[number]
export type ResolvedColorTheme = Exclude<ColorThemePreference, 'system'>

export function parseColorThemePreference(value: unknown): ColorThemePreference {
  return COLOR_THEME_PREFERENCES.includes(value as ColorThemePreference)
    ? value as ColorThemePreference
    : 'system'
}

export function resolveColorTheme(
  preference: ColorThemePreference,
  systemDark: boolean,
): ResolvedColorTheme {
  if (preference !== 'system') return preference
  return systemDark ? 'dark' : 'light'
}

export function explicitThemeAttribute(
  preference: ColorThemePreference,
): ResolvedColorTheme | null {
  return preference === 'system' ? null : preference
}
