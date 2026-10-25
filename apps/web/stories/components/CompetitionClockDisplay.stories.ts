import type { Meta, StoryObj } from '@storybook/vue3-vite'
import CompetitionClockDisplay from '~/components/CompetitionClockDisplay.vue'

const meta = {
  title: '競賽/對局時計',
  component: CompetitionClockDisplay,
  tags: ['autodocs'],
  args: {
    readClock: async () => { throw new Error('此情境不應查詢時計') },
    mode: { kind: 'ordinary' },
    gameId: 'game-demo',
    roomGameId: null,
    sessionUserId: null,
    memberUserId: null,
  },
} satisfies Meta<typeof CompetitionClockDisplay>

export default meta
type Story = StoryObj<typeof meta>

export const OrdinaryGame: Story = {}
export const SignInRequired: Story = { args: { mode: { kind: 'competition', context: { competitionId: 'cup-demo', matchId: 'match-demo', gameNumber: 1 } }, roomGameId: 'game-demo' } }
