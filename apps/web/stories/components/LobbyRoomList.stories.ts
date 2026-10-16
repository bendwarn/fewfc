import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { expect, fn, userEvent, within } from 'storybook/test'
import LobbyRoomList from '~/components/LobbyRoomList.vue'

const meta = {
  title: '大廳/公開房間清單',
  component: LobbyRoomList,
  args: {
    heading: '可加入房間', badge: 1, emptyMessage: '目前沒有公開房間', loading: false, busy: false,
    items: [{ gameId: 'game-public-1', code: 'AB12CD', name: '週末切磋', detail: '1/2 位玩家', ruleSummary: '規則版本 5.17', actionLabel: '加入 →', hasNotification: true }],
    onOpen: fn(),
  },
} satisfies Meta<typeof LobbyRoomList>

export default meta
type Story = StoryObj<typeof meta>

export const RoomWithNotification: Story = {
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByRole('status', { name: '有房間通知' })).toBeVisible()
    await userEvent.click(canvas.getByRole('button', { name: /週末切磋/ }))
    await expect(args.onOpen).toHaveBeenCalledWith('game-public-1')
  },
}

export const BusyWhileOpening: Story = {
  args: { busy: true },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByRole('button', { name: /週末切磋/ })).toBeDisabled()
  },
}

export const Empty: Story = {
  args: { badge: 0, items: [], emptyMessage: '目前沒有公開房間' },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByText('目前沒有公開房間')).toBeVisible()
    await expect(canvas.queryByRole('button')).not.toBeInTheDocument()
  },
}

export const RuleSummaryOnlyWhenDifferent: Story = {
  args: {
    heading: '我的房間', badge: 2, emptyMessage: '尚未加入任何房間。', loading: false, busy: false,
    items: [
      { gameId: 'all-enabled', code: 'all-enab', name: '完整規則房', detail: '1 / 2 玩家 · 等待中', ruleSummary: '', actionLabel: '進入', hasNotification: true },
      { gameId: 'without-pouch', code: 'without-', name: '停用錦囊房', detail: '1 / 2 玩家 · 等待中', ruleSummary: '停用：錦囊', actionLabel: '進入', hasNotification: false },
    ],
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByRole('button', { name: /完整規則房/ })).not.toHaveTextContent('停用：')
    await expect(canvas.getByRole('button', { name: /停用錦囊房/ })).toHaveTextContent('停用：錦囊')
    await expect(canvas.getAllByText('1 / 2 玩家 · 等待中')).toHaveLength(2)
    await expect(canvas.getAllByRole('button')).toHaveLength(2)
    await expect(canvas.getByRole('status', { name: '有房間通知' })).toBeVisible()
  },
}
