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

it('selects a past version through the collapsed control and allows a subset after the authoritative reset', async () => {
  const versionCatalog: RuleModuleSpec[] = [...catalog,
    { id: 'echo', category: 'theme', defaultEnabled: true, dependencies: ['star', 'five-directions-legend', 'hero-schools'] },
    { id: 'tribulation', category: 'theme', defaultEnabled: true, dependencies: ['star', 'five-directions-legend', 'hero-schools'] },
    { id: 'totem-formation', category: 'theme', defaultEnabled: true, dependencies: ['star', 'five-directions-legend', 'hero-schools'] },
  ]
  const wrapper = await mountSuspended(RuleModuleSettings, {
    props: { catalog: versionCatalog, ruleVersion: '5.17', enabledRuleModules: ['star'], isOwner: true },
  })
  expect(wrapper.get('details').element.open).toBe(false)
  expect(wrapper.find('input[aria-label="迴響"]').exists()).toBe(false)
  await wrapper.get('summary').trigger('click')
  await wrapper.get('input[value="5.16"]').setValue(true)
  expect(wrapper.emitted('update:ruleVersion')).toEqual([['5.16']])

  // 房間回覆先套用版本完整預設，再接受玩家的部分模組選擇。
  const resetModules = versionCatalog.filter(module => module.id !== 'totem-formation').map(module => module.id)
  await wrapper.setProps({ ruleVersion: '5.16', enabledRuleModules: resetModules })
  expect(wrapper.find('input[aria-label="圖騰法陣"]').exists()).toBe(false)
  expect(wrapper.get('input[aria-label="迴響"]').element.checked).toBe(true)
  await wrapper.get('input[aria-label="迴響"]').setValue(false)
  const subset = wrapper.emitted('update:enabledRuleModules')!.at(-1)![0] as string[]
  expect(subset).not.toContain('echo')
  expect(subset).toContain('tribulation')
  await wrapper.setProps({ enabledRuleModules: subset })
  expect(wrapper.get('input[aria-label="迴響"]').element.checked).toBe(false)
  expect(wrapper.get('input[value="5.16"]').element.checked).toBe(true)
  await wrapper.setProps({ isOwner: false })
  expect(wrapper.find('details').exists()).toBe(false)
})
