import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { expect, fn, userEvent, within } from 'storybook/test'
import RuleModuleSettings from '~/components/RuleModuleSettings.vue'
import { allRuleModulesEnabled, ruleModuleCatalog } from '../fixtures/rule-modules'

const meta = {
  title: '遊戲設定/規則模組',
  component: RuleModuleSettings,
  args: {
    catalog: ruleModuleCatalog,
    enabledRuleModules: allRuleModulesEnabled,
    isOwner: true,
    'onUpdate:enabledRuleModules': fn(),
  },
} satisfies Meta<typeof RuleModuleSettings>

export default meta
type Story = StoryObj<typeof meta>

export const OwnerChangesDependencies: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    const spirit = canvas.getByRole('checkbox', { name: '精靈' })
    await expect(spirit).toBeChecked()
    await userEvent.click(spirit)
    await expect(args['onUpdate:enabledRuleModules']).toHaveBeenCalledWith([
      'discard-retrieval', 'personal-deck', 'five-directions-legend', 'star', 'hero-schools',
    ])
  },
}

export const ReadOnlyForNonOwner: Story = {
  args: { enabledRuleModules: ['star', 'five-directions-legend', 'hero-schools', 'spirit'], isOwner: false },
  play: async ({ canvasElement }) => {
    const spirit = within(canvasElement).getByRole('checkbox', { name: '精靈' })
    await expect(spirit).toBeChecked()
    await expect(spirit).toBeDisabled()
  },
}
