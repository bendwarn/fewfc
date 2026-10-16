import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CompetitionTournamentView from '~/components/CompetitionTournamentView.vue'

const meta = {
  title: '競賽/目前賽程',
  component: CompetitionTournamentView,
  tags: ['autodocs'],
  args: {
    competitionId: 'cup-demo',
    sessionIdentity: null,
    contextRevision: null,
    contextReady: false,
    registrationLocked: false,
    participantTeamId: null,
    participantIsCaptain: false,
    participantEntryEnabled: false,
  },
} satisfies Meta<typeof CompetitionTournamentView>

export default meta
type Story = StoryObj<typeof meta>

export const ContextLoading: Story = {}
