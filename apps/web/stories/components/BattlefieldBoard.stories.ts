import type { Meta, StoryObj } from '@storybook/vue3-vite'
import BattlefieldBoard from '~/components/BattlefieldBoard.vue'
import { createExampleGameState } from '../fixtures/components'

const meta = {
  title: '對局/戰場棋盤',
  component: BattlefieldBoard,
  tags: ['autodocs'],
  args: {
    state: createExampleGameState(),
    displayNames: { alice: '小明', bob: '小華' },
    anchorPlayer: 'alice',
    mode: 'live',
  },
} satisfies Meta<typeof BattlefieldBoard>

export default meta
type Story = StoryObj<typeof meta>

export const LiveGame: Story = {}
export const Replay: Story = { args: { mode: 'replay' } }
