import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CompetitionRulingCorrection from '~/components/CompetitionRulingCorrection.vue'

const meta = {
  title: '競賽/裁判更正',
  component: CompetitionRulingCorrection,
  tags: ['autodocs'],
  args: {
    competitionId: 'cup-demo',
    sessionUserId: null,
    contextReady: false,
    matchId: '',
  },
} satisfies Meta<typeof CompetitionRulingCorrection>

export default meta
type Story = StoryObj<typeof meta>

export const ContextLoading: Story = {}
