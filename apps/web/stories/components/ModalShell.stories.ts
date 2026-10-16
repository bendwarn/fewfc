import type { Meta, StoryObj } from '@storybook/vue3-vite'
import ModalShell from '~/components/ModalShell.vue'

const meta = {
  title: '元件/ModalShell',
  component: ModalShell,
  tags: ['autodocs'],
  args: {
    open: true,
    labelledby: 'modal-story-title',
    describedby: 'modal-story-description',
  },
  render: args => ({
    components: { ModalShell },
    setup: () => ({ args }),
    template: '<ModalShell v-bind="args"><header><h2 id="modal-story-title">確認房間設定</h2></header><p id="modal-story-description">檢視設定後再繼續。</p><button type="button">返回</button><button type="button">確認</button></ModalShell>',
  }),
} satisfies Meta<typeof ModalShell>

export default meta
type Story = StoryObj<typeof meta>

export const Open: Story = {}
export const CloseDisabled: Story = { args: { closeDisabled: true } }
