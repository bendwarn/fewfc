import type { Meta, StoryObj } from '@storybook/vue3-vite'
import RoomPage from '~/pages/rooms/[gameId].vue'
import { expect, userEvent, waitFor, within } from 'storybook/test'
import { configurePage } from '../../.storybook/mocks/page-runtime'
import { configureRoom, releaseRoomQuery, roomQueryMode } from '../../.storybook/mocks/room-runtime'
import { createRoomPageFixture, type RoomPageScenario } from '../fixtures/room-page'

const meta = {
  title: '頁面/正式房間', component: RoomPage, tags: ['ui-migration'], parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof RoomPage>
export default meta
type Story = StoryObj<typeof meta>

function room(scenario: RoomPageScenario): Story {
  return {
    render: () => ({
      components: { RoomPage },
      setup() {
        configurePage({ path: '/rooms/story-room', params: { gameId: 'story-room' }, fetch: url => { throw new Error(`房間故事不應送出 HTTP：${url}`) } })
        const { response, offeredActions } = createRoomPageFixture(scenario)
        configureRoom(response, offeredActions)
      },
      template: '<div style="display:flex;height:100dvh"><RoomPage /></div>',
    }),
  }
}

export const Actions = room('actions')
export const Discard = room('discard')
export const InitialPouch = room('initial-pouch')
export const Chain = room('chain')
export const CardChoice = room('card-choice')
export const PouchAbility = room('pouch-ability')

export const ActionQueryStates: Story = {
  ...room('actions'),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const card = await canvas.findByRole('button', { name: '火 3' })
    const reveal = canvasElement.querySelector<HTMLElement>('.setup-reveal')
    if (reveal) await userEvent.click(reveal)
    roomQueryMode.value = 'pending'
    try {
      await userEvent.click(card)
      await waitFor(() => expect(canvas.getByText('處理中', { exact: true })).toBeVisible())
      releaseRoomQuery()
      const formation = await canvas.findByRole('button', { name: /^禁錮法陣/ })
      formation.focus()
      await expect(formation).toHaveFocus()
      await expect(canvasElement.querySelector('.action-detail')).toHaveTextContent('所有其他玩家 1 回合內無法行動及抽牌')
      roomQueryMode.value = 'normal'
      await userEvent.click(card)
      await waitFor(() => expect(canvas.getByText('選擇手牌以尋找可用行動')).toBeVisible())
      roomQueryMode.value = 'error'
      await userEvent.click(card)
      await waitFor(() => expect(canvasElement.querySelector('.action-error')).toHaveTextContent('無法取得可用行動，請稍後再試。'))
      await expect(canvasElement.querySelector('.action-error')).not.toHaveTextContent(/500|Internal Server Error|query failed/)
    } finally {
      releaseRoomQuery()
      roomQueryMode.value = 'normal'
    }
  },
}
