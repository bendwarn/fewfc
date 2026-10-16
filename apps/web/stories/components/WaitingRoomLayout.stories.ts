import type { Meta, StoryObj } from '@storybook/vue3-vite'
import WaitingRoomLayout from '~/components/WaitingRoomLayout.vue'

const meta = {
  title: '房間/等待室版面',
  component: WaitingRoomLayout,
  tags: ['autodocs'],
  render: () => ({
    components: { WaitingRoomLayout },
    template: '<WaitingRoomLayout><template #main><p class="section-kicker">等待對手</p><h2>房間準備中</h2><p>所有玩家加入後即可開始。</p></template><template #side><h3>房間成員</h3><p>小明・房主</p><p>等待其他玩家加入</p></template></WaitingRoomLayout>',
  }),
} satisfies Meta<typeof WaitingRoomLayout>

export default meta
type Story = StoryObj<typeof meta>

export const Default: Story = {}
