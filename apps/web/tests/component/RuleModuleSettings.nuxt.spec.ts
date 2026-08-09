import { mountSuspended } from '@nuxt/test-utils/runtime'
import { describe, expect, it } from 'vitest'
import RuleModuleSettings from '~/components/RuleModuleSettings.vue'
import type { RuleModuleSpec } from '~/types/fewfc'

const catalog: RuleModuleSpec[] = [
  { id: 'discard-retrieval', category: 'optional', defaultEnabled: true, dependencies: [] },
  { id: 'personal-deck', category: 'optional', defaultEnabled: true, dependencies: [] },
  { id: 'five-directions-legend', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'star', category: 'advanced', defaultEnabled: true, dependencies: [] },
  { id: 'hero-schools', category: 'advanced', defaultEnabled: true, dependencies: [] },
  {
    id: 'spirit',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['star', 'five-directions-legend', 'hero-schools'],
  },
  {
    id: 'dark-glimmer',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['spirit'],
  },
  {
    id: 'pouch',
    category: 'theme',
    defaultEnabled: true,
    dependencies: ['personal-deck', 'spirit'],
  },
]

describe('RuleModuleSettings', () => {
  it('uses the catalog policy when an owner disables a dependency', async () => {
    const wrapper = await mountSuspended(RuleModuleSettings, {
      props: {
        catalog,
        enabledRuleModules: catalog.map(module => module.id),
        isOwner: true,
      },
    })

    expect(wrapper.get('fieldset').text()).toContain('規則模組')
    expect(wrapper.get('input[aria-label="精靈"]').element.checked).toBe(true)

    await wrapper.get('input[aria-label="精靈"]').setValue(false)

    expect(wrapper.emitted('update:enabledRuleModules')).toStrictEqual([[
      [
        'discard-retrieval',
        'personal-deck',
        'five-directions-legend',
        'star',
        'hero-schools',
      ],
    ]])
  })

  it('exposes the same state read-only for a non-owner', async () => {
    const wrapper = await mountSuspended(RuleModuleSettings, {
      props: {
        catalog,
        enabledRuleModules: ['star', 'five-directions-legend', 'hero-schools', 'spirit'],
        isOwner: false,
      },
    })

    expect(wrapper.get('fieldset').text()).toContain('啟用規則')
    expect(wrapper.get('input[aria-label="精靈"]').attributes('disabled')).toBeDefined()
  })
})
