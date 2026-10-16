import type { Meta, StoryObj } from '@storybook/vue3-vite'
import GameCard from '~/components/GameCard.vue'
import { exampleCards } from '../fixtures/components'

const meta = {
  title: '對局/卡牌',
  component: GameCard,
  tags: ['autodocs'],
  args: { card: exampleCards[0], selectable: true, shortcut: '1' },
} satisfies Meta<typeof GameCard>

export default meta
type Story = StoryObj<typeof meta>

export const Selectable: Story = {}
export const Selected: Story = { args: { selected: true } }
export const Hidden: Story = { args: { card: null, hidden: true, selectable: false, shortcut: undefined } }
