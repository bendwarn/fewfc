import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { defineComponent, onBeforeUnmount, type Component } from 'vue'
import { expect, fn, userEvent, waitFor, within } from 'storybook/test'
import LobbyStoryScreen from './LobbyStoryScreen.vue'

function darkTheme(story: Component): Component {
  return defineComponent({
    components: { story },
    setup() {
      const previous = document.documentElement.dataset.theme
      document.documentElement.dataset.theme = 'dark'
      onBeforeUnmount(() => { document.documentElement.dataset.theme = previous ?? 'light' })
    },
    template: '<story />',
  })
}

const meta = {
  title: '大廳/完整大廳畫面',
  component: LobbyStoryScreen,
  parameters: { layout: 'fullscreen' },
  args: {
    publicRooms: [
      { gameId: 'wood-duel', code: 'WD28AX', name: '山間切磋・歡迎新手', detail: '1 / 2 玩家 · 0 位觀戰者 · 等待開始', ruleSummary: '規則 5.17', actionLabel: '加入', hasNotification: false },
      { gameId: 'team-game', code: 'FA6K2M', name: '四人同行，一起組隊', detail: '3 / 4 玩家 · 1 位觀戰者 · 等待開始', ruleSummary: '規則 5.17', actionLabel: '加入', hasNotification: false },
      { gameId: 'watch-game', code: 'CF9P3R', name: '高手過招・觀戰席開放', detail: '2 / 2 玩家 · 3 位觀戰者 · 對局中', ruleSummary: '規則 5.16', actionLabel: '觀戰', hasNotification: false },
    ],
    myRooms: [
      { gameId: 'my-turn', code: 'ML57BQ', name: '週末好友局', detail: '2 / 2 玩家 · 0 位觀戰者 · 對局中', ruleSummary: '規則 5.17', actionLabel: '進入', hasNotification: true },
      { gameId: 'my-room', code: 'HE82TN', name: '小明的房間', detail: '1 / 2 玩家 · 0 位觀戰者 · 等待中', ruleSummary: '規則 5.17', actionLabel: '進入', hasNotification: false },
    ],
    busy: false,
    loading: false,
    onJoin: fn(), onOpen: fn(), onRefresh: fn(), onSubmit: fn(),
  },
} satisfies Meta<typeof LobbyStoryScreen>

export default meta
type Story = StoryObj<typeof meta>

export const DarkLobby: Story = {
  decorators: [darkTheme],
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByRole('button', { name: '加入', exact: true })).toBeDisabled()
    await userEvent.type(canvas.getByLabelText('房間代碼'), ' WD28AX ')
    await userEvent.click(canvas.getByRole('button', { name: '加入', exact: true }))
    await expect(args.onJoin).toHaveBeenCalledWith('WD28AX')
    await userEvent.click(canvas.getByRole('button', { name: /山間切磋/ }))
    await expect(args.onOpen).toHaveBeenCalledWith('wood-duel')
    await userEvent.click(canvas.getByRole('button', { name: /週末好友局/ }))
    await expect(args.onOpen).toHaveBeenCalledWith('my-turn')
  },
}
export const LightLobby: Story = {}
export const EmptyLobby: Story = {
  decorators: [darkTheme], args: { publicRooms: [], myRooms: [] },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByText('目前沒有可加入的公開房間。')).toBeVisible()
    await expect(canvas.getByText('尚未加入任何房間。')).toBeVisible()
    await expect(canvas.getByRole('button', { name: '建立房間' })).toBeEnabled()
  },
}
export const LoadingLobby: Story = { decorators: [darkTheme], args: { loading: true, publicRooms: [], myRooms: [] } }
export const ErrorLobby: Story = {
  decorators: [darkTheme], args: { error: '無法取得公開房間，請稍後再試。', publicRooms: [], myRooms: [] },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByRole('alert')).toHaveTextContent('無法取得公開房間')
    await userEvent.click(canvas.getByRole('button', { name: '重試' }))
    await expect(args.onRefresh).toHaveBeenCalled()
  },
}
export const CreateRoom: Story = {
  decorators: [darkTheme],
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    const page = within(canvasElement.ownerDocument.body)
    const trigger = canvas.getByRole('button', { name: '建立房間', exact: true })
    await userEvent.click(trigger)
    const dialog = await page.findByRole('dialog', { name: '建立房間' })
    await waitFor(() => expect(within(dialog).getByLabelText('房間名稱')).toHaveFocus())
    await userEvent.clear(within(dialog).getByLabelText('房間名稱'))
    await userEvent.type(within(dialog).getByLabelText('房間名稱'), '好友切磋')
    await userEvent.click(within(dialog).getByRole('button', { name: /團隊對戰/ }))
    await userEvent.click(within(dialog).getByRole('button', { name: '私人房間' }))
    await expect(within(dialog).getByRole('button', { name: /團隊對戰/ })).toHaveAttribute('aria-pressed', 'true')
    await userEvent.click(within(dialog).getByRole('button', { name: '建立房間 →' }))
    await expect(args.onSubmit).toHaveBeenCalledWith({ name: '好友切磋', mode: 'team', access: 'private', ruleVersion: '5.17' })
    await waitFor(() => expect(trigger).toHaveFocus())
  },
}
