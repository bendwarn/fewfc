import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CompetitionParticipantGameEntry from '~/components/CompetitionParticipantGameEntry.vue'

const meta = {
  title: '競賽/參賽者對局入口',
  component: CompetitionParticipantGameEntry,
  tags: ['autodocs'],
  args: {
    competitionId: 'cup-demo',
    matchId: '',
    sessionUserId: null,
    participantTeamId: null,
    enabled: false,
    editable: false,
  },
} satisfies Meta<typeof CompetitionParticipantGameEntry>

export default meta
type Story = StoryObj<typeof meta>

export const SignInRequired: Story = {}
export const OrganizerEntry: Story = { args: { editable: true } }
