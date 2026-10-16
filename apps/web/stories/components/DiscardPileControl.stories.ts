import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { expect, fn, userEvent, within } from 'storybook/test'
import DiscardPileControl from '~/components/DiscardPileControl.vue'

const meta = {
  title: '對局/棄牌區控制',
  component: DiscardPileControl,
  args: { owner: null, count: 4, deckCount: 37, unavailable: false, open: true, onToggle: fn() },
} satisfies Meta<typeof DiscardPileControl>

export default meta
type Story = StoryObj<typeof meta>

export const SharedPileWithLastDiscard: Story = {
  args: { featuredCard: { id: 18, label: '火 3 級', element: 'Fire', level: 3, secretStrategies: [] } },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    const trigger = canvas.getByRole('button', { name: '查看共用棄牌詳情。目前棄牌 4 張，牌庫 37 張。上回合棄牌：火 3 級' })
    await expect(canvas.getByRole('img', { name: /火 3 級/ })).toBeVisible()
    await expect(trigger).toHaveAttribute('aria-expanded', 'true')
    await userEvent.click(trigger)
    await expect(args.onToggle).toHaveBeenCalledTimes(1)
  },
}

export const UnavailablePile: Story = {
  args: { count: 0, deckCount: 45, unavailable: true, open: false, featuredCard: undefined },
  play: async ({ canvasElement, args }) => {
    const trigger = within(canvasElement).getByRole('button', { name: '查看共用棄牌詳情。目前棄牌 0 張，牌庫 45 張' })
    await expect(trigger).toBeDisabled()
    await expect(trigger).toHaveAttribute('aria-disabled', 'true')
    await expect(trigger).toHaveAttribute('aria-expanded', 'false')
    await userEvent.click(trigger)
    await expect(args.onToggle).not.toHaveBeenCalled()
  },
}

export const PlayerPileWithoutPreview: Story = {
  args: { owner: 'p1', ownerLabel: '甲', count: 2, deckCount: 18, unavailable: false, open: false, featuredCard: undefined },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    const trigger = canvas.getByRole('button', { name: '查看甲的棄牌詳情。目前棄牌 2 張，牌庫 18 張' })
    await expect(canvas.getByText('甲的棄牌')).toBeVisible()
    await expect(canvas.getByRole('button', { name: /目前棄牌 2 張，牌庫 18 張/ })).toBeVisible()
    await expect(canvas.queryByRole('img')).not.toBeInTheDocument()
    await userEvent.click(trigger)
    await expect(args.onToggle).toHaveBeenCalledTimes(1)
  },
}

export const HistoricalPreviewOnUnavailablePile: Story = {
  args: {
    count: 0, deckCount: 45, unavailable: true, open: false,
    featuredCard: { id: 18, label: '火 3 級', element: 'Fire', level: 3, secretStrategies: [] },
  },
  play: async ({ canvasElement, args }) => {
    const canvas = within(canvasElement)
    const trigger = canvas.getByRole('button', { name: /查看共用棄牌詳情/ })
    await expect(trigger).toHaveAttribute('aria-disabled', 'true')
    await expect(trigger).not.toBeDisabled()
    await userEvent.click(trigger)
    await expect(args.onToggle).not.toHaveBeenCalled()
  },
}
