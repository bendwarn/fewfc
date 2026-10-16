import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CompetitionTournamentManagement from '~/components/CompetitionTournamentManagement.vue'

const meta = {
  title: '競賽/賽程設定',
  component: CompetitionTournamentManagement,
  tags: ['autodocs'],
  args: {
    competitionId: 'cup-demo',
    sessionUserId: null,
    registration: null,
    contextReady: false,
    busy: false,
  },
} satisfies Meta<typeof CompetitionTournamentManagement>

export default meta
type Story = StoryObj<typeof meta>

export const ContextLoading: Story = {}
