import type { Meta, StoryObj } from '@storybook/vue3-vite'
import GameConclusionPanel from '~/components/GameConclusionPanel.vue'
import { createExampleGameState } from '../fixtures/components'

const state = createExampleGameState()
state.status = 'Finished'
state.gameConclusion = {
  outcome: { type: 'winner', team: state.players[0]!.team },
  causes: [{ type: 'teamHpDepleted', teams: [state.players[1]!.team] }],
}

const meta = {
  title: '對局/結算面板',
  component: GameConclusionPanel,
  tags: ['autodocs'],
  args: {
    state,
    teamLabel: team => team === state.players[0]!.team ? '甲隊' : '乙隊',
    playerLabel: player => player === 'alice' ? '小明' : '小華',
  },
} satisfies Meta<typeof GameConclusionPanel>

export default meta
type Story = StoryObj<typeof meta>

export const TeamVictory: Story = {}
export const Draw: Story = { args: { state: { ...state, gameConclusion: { outcome: { type: 'draw' }, causes: [] } } } }
