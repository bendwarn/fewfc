import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CardChoiceMatrix from '~/components/CardChoiceMatrix.vue'
import { exampleCards } from '../fixtures/components'

const meta = {
  title: '對局/卡牌組合選擇',
  component: CardChoiceMatrix,
  tags: ['autodocs'],
  args: {
    cards: exampleCards,
    label: '選擇卡牌',
    caption: '從手牌中選擇一張牌。',
    actionLabel: '選擇',
    maximum: 1,
  },
} satisfies Meta<typeof CardChoiceMatrix>

export default meta
type Story = StoryObj<typeof meta>

export const SingleCard: Story = {}
export const MultiSelect: Story = { args: { mode: 'toggle', maximum: 2, selectedCards: [1] } }
export const ReadOnly: Story = { args: { readonly: true, selectedCards: [1] } }
