import { useColorTheme } from '../../.storybook/mocks/useColorTheme'
import { onBeforeUnmount } from 'vue'
import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { expect, fn, userEvent, within } from 'storybook/test'
import PlayerShell from '~/components/PlayerShell.vue'
import LobbyScreen from '~/components/LobbyScreen.vue'

const meta = {
  title: '共用/玩家導覽與通知',
  component: PlayerShell,
  parameters: { layout: 'fullscreen' },
  args: {
    displayName: '小明', connectionState: 'connected',
    notifications: [
      { id: 'turn-1', gameId: 'wood-duel', kind: 'yourTurn', message: '好友切磋：輪到你的回合。', createdAt: '2026-10-23T00:00:00Z' },
      { id: 'removed-1', gameId: 'old-room', kind: 'removed', message: '你已離開原來的房間。', createdAt: '2026-10-23T00:00:00Z' },
    ],
    onHome: fn(), onNavigate: fn(), onLogout: fn(), onOpenNotification: fn(), onDismiss: fn(),
  },
  render: args => ({
    components: { PlayerShell, LobbyScreen },
    setup: () => ({ args }),
    template: '<PlayerShell v-bind="args"><LobbyScreen :public-rooms="[]" :my-rooms="[]" :loading="false" :busy="false" /></PlayerShell>',
  }),
} satisfies Meta<typeof PlayerShell>
export default meta
type Story = StoryObj<typeof meta>

export const Light: Story = {}
export const Dark: Story = {
  decorators: [story => ({
    components: { story },
    setup() {
      useColorTheme().setPreference('dark')
      onBeforeUnmount(() => { useColorTheme().setPreference('light') })
    },
    template: '<story />',
  })],
}
export const Interactions: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    const trigger = canvas.getByRole('button', { name: '小明的玩家選單' })
    await userEvent.click(trigger)
    await userEvent.click(canvas.getByRole('button', { name: '個人牌組' }))
    await expect(args.onNavigate).toHaveBeenCalledWith('/deck')
    await userEvent.click(trigger)
    await userEvent.keyboard('{Escape}')
    await expect(trigger).toHaveAttribute('aria-expanded', 'false')
    await expect(trigger).toHaveFocus()
    await userEvent.click(canvas.getByRole('button', { name: /好友切磋：輪到你的回合/ }))
    await expect(args.onOpenNotification).toHaveBeenCalledWith('wood-duel')
    await expect(canvas.getByRole('status')).toHaveTextContent('你已離開原來的房間。')
    await userEvent.click(canvas.getAllByRole('button', { name: '關閉通知' })[0]!)
    await expect(args.onDismiss).toHaveBeenCalledWith('turn-1')
  },
}
