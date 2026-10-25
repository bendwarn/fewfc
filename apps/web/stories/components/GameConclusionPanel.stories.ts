import type { Meta, StoryObj } from '@storybook/vue3-vite'
import GameConclusionPanel from '~/components/GameConclusionPanel.vue'
import { createExampleGameState } from '../fixtures/components'
import { createConclusionState } from '../fixtures/conclusion'
import { expect, within } from 'storybook/test'

const state = createExampleGameState()
state.status = 'Finished'
state.gameConclusion = {
  outcome: { type: 'winner', team: state.players[0]!.team },
  causes: [{ type: 'teamHpDepleted', teams: [state.players[1]!.team] }],
}

const meta = {
  title: '對局/結算面板',
  component: GameConclusionPanel,
  tags: ['autodocs', 'conclusion'],
  args: {
    state,
    teamLabel: team => team === state.players[0]!.team ? '甲隊' : '乙隊',
    playerLabel: player => player === 'alice' ? '小明' : '小華',
  },
} satisfies Meta<typeof GameConclusionPanel>

export default meta
type Story = StoryObj<typeof meta>

export const TeamVictory: Story = {}
export const Draw: Story = { args: { state: createConclusionState('draw') } }
export const HiddenCards: Story = {
  args: { state: createConclusionState('hidden') },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByText('1 張蓋牌')).toBeVisible()
    await expect(canvas.getAllByRole('img')).toHaveLength(1)
    await expect(canvas.getByRole('img', { name: '火 1' })).toBeVisible()
  },
}
const unknownRuleState = createConclusionState('direct')
unknownRuleState.gameConclusion!.causes = [{ type: 'directVictory', team: state.players[0]!.team, rule: 'future-rule' }]
export const UnknownDirectVictory: Story = {
  args: { state: unknownRuleState },
  play: async ({ canvasElement }) => {
    await expect(within(canvasElement).getByText('甲隊 達成直接勝利條件')).toBeVisible()
  },
}
