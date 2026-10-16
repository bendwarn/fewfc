import type { Meta, StoryObj } from '@storybook/vue3-vite'
import BattlefieldDetailLayer from '~/components/BattlefieldDetailLayer.vue'

const meta = {
  title: '對局/戰場詳情面板',
  component: BattlefieldDetailLayer,
  tags: ['autodocs'],
  args: {
    open: true,
    id: 'discard-detail',
    title: '目前棄牌 3 · 牌庫 24',
    kind: 'discard',
    anchor: { top: 150, right: 360, bottom: 190, left: 280 },
  },
  render: args => ({
    components: { BattlefieldDetailLayer },
    setup: () => ({ args }),
    template: '<BattlefieldDetailLayer v-bind="args"><ul><li>火 3</li><li>水 2</li><li>木 1</li></ul></BattlefieldDetailLayer>',
  }),
} satisfies Meta<typeof BattlefieldDetailLayer>

export default meta
type Story = StoryObj<typeof meta>

export const DiscardDetails: Story = {}
export const Closed: Story = { args: { open: false } }
