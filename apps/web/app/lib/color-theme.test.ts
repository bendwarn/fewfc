import { expect, test } from 'bun:test'
import {
  explicitThemeAttribute,
  parseColorThemePreference,
  resolveColorTheme,
} from './color-theme'

test('color theme preferences accept only the three public choices', () => {
  expect(parseColorThemePreference('system')).toBe('system')
  expect(parseColorThemePreference('light')).toBe('light')
  expect(parseColorThemePreference('dark')).toBe('dark')
  expect(parseColorThemePreference('sepia')).toBe('system')
  expect(parseColorThemePreference(null)).toBe('system')
})

test('system theme stays live while explicit themes override the system', () => {
  expect(resolveColorTheme('system', false)).toBe('light')
  expect(resolveColorTheme('system', true)).toBe('dark')
  expect(resolveColorTheme('light', true)).toBe('light')
  expect(resolveColorTheme('dark', false)).toBe('dark')
  expect(explicitThemeAttribute('system')).toBeNull()
  expect(explicitThemeAttribute('light')).toBe('light')
  expect(explicitThemeAttribute('dark')).toBe('dark')
})
