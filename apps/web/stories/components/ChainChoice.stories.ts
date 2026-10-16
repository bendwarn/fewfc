import type { Meta, StoryObj } from '@storybook/vue3-vite'
import ChainChoice from '~/components/ChainChoice.vue'
import { exampleCards } from '../fixtures/components'

const meta = {
  title: '對局/連鎖選擇',
  component: ChainChoice,
  tags: ['autodocs'],
  args: {
    pouchOwners: ['alice', 'bob'],
    pouchOwner: 'alice',
    pouchCards: exampleCards,
    pouchCard: exampleCards[0]!,
    triggerCards: exampleCards.slice(1),
    triggerCard: null,
    strategyOptions: [],
    selectedStrategy: null,
    selectedStrategyAction: null,
    selectedTarget: null,
    selectedStar: null,
    breakStar: false,
    selectedRetreatCard: null,
    handCards: exampleCards,
    playerLabel: player => player === 'alice' ? '小明' : '小華',
    strategyLabel: strategy => strategy,
    starLabel: star => star,
  },
} satisfies Meta<typeof ChainChoice>

export default meta
type Story = StoryObj<typeof meta>

export const ChoosePouchAndTrigger: Story = {}
