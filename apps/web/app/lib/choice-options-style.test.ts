import { describe, expect, test } from 'bun:test'

const appSource = await Bun.file(new URL('../app.vue', import.meta.url)).text()
const styleSource = appSource.slice(appSource.indexOf('<style'))

describe('choice option button styles', () => {
  test('provide a visible button surface and selected state', () => {
    expect(styleSource).toMatch(/\.choice-options\s+button\s*\{[^}]*\b(border|background|@apply)\b/s)
    expect(styleSource).toMatch(/\.choice-options\s+button(?:\.selected|\[aria-pressed)/)
  })

  test('provide explicit keyboard focus and primary action treatments', () => {
    expect(styleSource).toContain('.choice-options button:focus-visible')
    expect(styleSource).toContain('.choice-actions .choice-confirm')
  })
})
