import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CompetitionRoundPublication from '~/components/CompetitionRoundPublication.vue'

const meta = {
  title: '競賽/輪次公告',
  component: CompetitionRoundPublication,
  tags: ['autodocs'],
  args: {
    competitionId: 'cup-demo',
    sessionUserId: null,
    registration: null,
    contextReady: false,
  },
} satisfies Meta<typeof CompetitionRoundPublication>

export default meta
type Story = StoryObj<typeof meta>

export const ContextLoading: Story = {}
