import type { Meta, StoryObj } from '@storybook/vue3-vite'
import AnchoredSurface from '~/components/AnchoredSurface.vue'

const meta = {
  title: '元件/AnchoredSurface',
  component: AnchoredSurface,
  tags: ['autodocs'],
  args: {
    open: true,
    anchor: { top: 120, right: 220, bottom: 150, left: 140 },
    id: 'anchored-example',
    role: 'dialog',
    labelledby: 'anchored-example-title',
    placement: 'bottom-start',
  },
  render: args => ({
    components: { AnchoredSurface },
    setup: () => ({ args }),
    template: '<AnchoredSurface v-bind="args"><section class="panel"><h2 id="anchored-example-title">錨定詳情</h2><p>此面板會依觸發位置與視窗邊界調整。</p><button type="button">檢視內容</button></section></AnchoredSurface>',
  }),
} satisfies Meta<typeof AnchoredSurface>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}
