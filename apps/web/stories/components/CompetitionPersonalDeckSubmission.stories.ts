import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CompetitionPersonalDeckSubmission from '~/components/CompetitionPersonalDeckSubmission.vue'

const meta = {
  title: '競賽/個人牌組提交',
  component: CompetitionPersonalDeckSubmission,
  tags: ['autodocs'],
  args: {
    currentUserId: null,
    activeTeam: null,
    lifecycle: 'draft',
    personalDecks: 'disabled',
    registrationDeadline: Date.UTC(2026, 10, 1),
    busy: false,
    pending: false,
    ownDeckSubmitted: false,
  },
} satisfies Meta<typeof CompetitionPersonalDeckSubmission>

export default meta
type Story = StoryObj<typeof meta>

export const NoActiveTeam: Story = {}
