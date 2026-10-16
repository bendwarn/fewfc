import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CompetitionReadyControls from '~/components/CompetitionReadyControls.vue'

const meta = {
  title: '競賽/準備控制',
  component: CompetitionReadyControls,
  tags: ['autodocs'],
  args: {
    gameId: 'game-demo',
    mode: { kind: 'ordinary' },
    sessionUserId: null,
    roomStatus: 'Waiting',
    memberUserId: null,
    memberReady: false,
    memberConnected: false,
    socketConnected: false,
    loading: false,
  },
} satisfies Meta<typeof CompetitionReadyControls>

export default meta
type Story = StoryObj<typeof meta>

export const OrdinaryGame: Story = {}
export const InvalidCompetitionContext: Story = { args: { mode: { kind: 'invalid' } } }
