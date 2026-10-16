import type { Meta, StoryObj } from '@storybook/vue3-vite'
import SplitEarthFormationChoice from '~/components/SplitEarthFormationChoice.vue'

const meta = {
  title: '對局/分土成陣選擇',
  component: SplitEarthFormationChoice,
  tags: ['autodocs'],
  args: {
    groups: [
      { ruleModuleId: null, formations: [{ id: 'fire-attack', name: '烈焰陣' }, { id: 'water-mirror', name: '水鏡陣' }] },
      { ruleModuleId: 'spirit', formations: [{ id: 'spirit-gate', name: '靈門陣' }] },
    ],
    disabled: false,
  },
} satisfies Meta<typeof SplitEarthFormationChoice>

export default meta
type Story = StoryObj<typeof meta>

export const SelectFormation: Story = {}
export const Disabled: Story = { args: { disabled: true } }
