import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { expect, userEvent, within } from 'storybook/test'
import BattlefieldBoard from '~/components/BattlefieldBoard.vue'
import GameConclusionPanel from '~/components/GameConclusionPanel.vue'
import { createExampleGameState, createFourPlayerGameState, exampleCards } from '../fixtures/components'

const displayNames = { alice: '小明', bob: '小華', carol: '小美', dave: '小安' }

const twoPlayerState = createExampleGameState()
twoPlayerState.currentPlayer = 'bob'
twoPlayerState.phase = 'Action'
twoPlayerState.environment = 'Water'

const fourPlayerState = createFourPlayerGameState()
const fourPlayerReplayState = createFourPlayerGameState()
fourPlayerReplayState.hands = fourPlayerReplayState.turnOrder.map((player, index) => ({
  player,
  cards: { kind: 'known', cards: index === 0 ? exampleCards.slice(0, 2) : [exampleCards[index % exampleCards.length]!] },
}))

const replayHandState = createExampleGameState()
replayHandState.currentPlayer = 'bob'
replayHandState.hands[1]!.cards = { kind: 'known', cards: [exampleCards[0]!] }
replayHandState.statuses.push({
  id: 'cannot-act',
  owner: { kind: 'player', id: 'bob' },
  kind: 'CannotAct',
  presentation: 'cannotAct',
  duration: { type: 'permanent' },
})

const totemState = createExampleGameState()
totemState.totems = [{ player: 'alice', totem: 'AzureHorn', name: '青角圖騰' }]

const personalDeckState = createExampleGameState()
personalDeckState.enabledRuleModules = ['personal-deck']
personalDeckState.turnNumber = 3
personalDeckState.playerDecks = [
  { player: 'alice', cards: { kind: 'hidden', count: 12 } },
  { player: 'bob', cards: { kind: 'hidden', count: 10 } },
]
personalDeckState.playerDiscards = [
  { player: 'alice', cards: [exampleCards[0]!] },
  { player: 'bob', cards: [] },
]
personalDeckState.lastCompletedTurnDiscards = [
  { player: 'alice', card: exampleCards[0]!, turnNumber: 1 },
]

const terminalState = createExampleGameState()
const winningTeam = terminalState.players[0]!.team
const losingTeam = terminalState.players[1]!.team
terminalState.status = 'Finished'
terminalState.gameConclusion = {
  outcome: { type: 'winner', team: winningTeam },
  causes: [{ type: 'teamHpDepleted', teams: [losingTeam] }],
}
terminalState.terminalResolution = {
  type: 'formation',
  player: 'alice',
  formationId: 'fire-attack',
  formationName: '烈焰陣',
  cards: { kind: 'known', cards: [exampleCards[0]!] },
}

const storyFrame = '<div style="width: 100%; height: 100vh; min-height: 560px;"><BattlefieldBoard v-bind="args" /></div>'

const meta = {
  title: '對局/戰場棋盤',
  component: BattlefieldBoard,
  tags: ['autodocs', 'battlefield'],
  parameters: { layout: 'fullscreen' },
  args: {
    state: twoPlayerState,
    displayNames,
    anchorPlayer: 'alice',
    mode: 'live',
  },
  render: args => ({
    components: { BattlefieldBoard },
    setup: () => ({ args }),
    template: storyFrame,
  }),
} satisfies Meta<typeof BattlefieldBoard>

export default meta
type Story = StoryObj<typeof meta>

export const TwoPlayerLive: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByRole('region', { name: '五行戰鬥牌對戰桌' })).toBeVisible()
    await expect(canvas.getAllByLabelText(/HP/)).toHaveLength(2)
    await expect(canvas.getByLabelText('小華，100 HP，目前行動玩家，行動階段')).toHaveAttribute('aria-current', 'true')
    await expect(canvas.getByText('環境 · 水行')).toBeVisible()
  },
}

export const FourPlayerPlayerView: Story = {
  args: { state: fourPlayerState, anchorPlayer: 'alice' },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getAllByLabelText(/HP/)).toHaveLength(4)
    await expect(canvas.getByLabelText('小明，150 HP')).toBeVisible()
    const actingSeat = canvas.getByLabelText('小華，132 HP，目前行動玩家，行動階段')
    await expect(actingSeat).toHaveAttribute('aria-current', 'true')
    const actingStyle = window.getComputedStyle(actingSeat)
    await expect(actingStyle.outlineStyle).not.toBe('none')
    await expect(actingStyle.boxShadow).not.toBe('none')
    await expect(canvasElement.querySelector('.seat-bottom')).toHaveAttribute('aria-label', '小明，150 HP')
  },
}

export const FourPlayerObserverView: Story = {
  args: { state: fourPlayerState, anchorPlayer: null },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvasElement.querySelectorAll('.player-seat')).toHaveLength(4)
    await expect(canvasElement.querySelector('.seat-bottom')).toHaveAttribute('aria-label', '小明，150 HP')
    await expect(canvasElement.querySelector('.player-seat[aria-current="true"]')?.getAttribute('aria-label')).toContain('小華，132 HP，目前行動玩家')
  },
}

export const FourPlayerReplayPerspective: Story = {
  args: {
    state: fourPlayerReplayState,
    anchorPlayer: 'carol',
    mode: 'replay',
  },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvas.getByRole('region', { name: '重播戰場（唯讀）' })).toBeVisible()
    await expect(canvasElement.querySelector('.seat-bottom')).toHaveAttribute('aria-label', '小美，150 HP')
    await expect(canvasElement.querySelector('.player-seat[aria-current="true"]')?.getAttribute('aria-label')).toContain('小華，132 HP，目前行動玩家')
    await expect(canvas.getAllByRole('img', { name: '火 3' })[0]).toBeVisible()
  },
}

export const LiveTotemRules: Story = {
  args: { state: totemState },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const totem = canvas.getByLabelText(/^青角圖騰：/)
    await expect(totem).toHaveTextContent('青角圖騰')
    await expect(totem.getAttribute('title')).toContain('防護罩不適用')
    await expect(totem).toHaveAttribute('tabindex', '0')
    await userEvent.tab()
    await expect(totem).toHaveFocus()
  },
}

export const ReplayTotemRules: Story = {
  args: { state: totemState, mode: 'replay' },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const totem = canvas.getByLabelText(/^青角圖騰：/)
    await expect(totem).toHaveTextContent('青角圖騰')
    await expect(totem.getAttribute('title')).toContain('防護罩不適用')
    await expect(totem).toHaveAttribute('tabindex', '0')
    await userEvent.tab()
    await expect(totem).toHaveFocus()
  },
}

export const ReplayHandInspection: Story = {
  args: { state: replayHandState, mode: 'replay' },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvasElement.querySelectorAll('.player-seat')).toHaveLength(2)
    const actingSeat = canvasElement.querySelector('.player-seat[aria-current="true"]')
    await expect(actingSeat?.getAttribute('aria-label')).toContain('小華')
    await expect(canvas.getByText('效果 1')).toBeVisible()
    const handButton = canvasElement.querySelector<HTMLButtonElement>('.player-seat[aria-current="true"] .compact-hand-button')
    if (!handButton) throw new Error('缺少重播手牌檢視按鈕')
    await expect(handButton).toHaveTextContent('手牌 1')
    handButton.click()
    const page = within(canvasElement.ownerDocument.body)
    const dialog = await page.findByRole('dialog', { name: '小華的手牌 1' })
    await expect(dialog).toBeVisible()
    await expect(within(dialog).getByRole('img', { name: '火 3' })).toBeVisible()
    await expect(canvasElement.ownerDocument.body.querySelectorAll('.hand-detail-cards .playing-card')).toHaveLength(1)
  },
}

export const PersonalDeckDiscardSummary: Story = {
  args: { state: personalDeckState },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvasElement.querySelectorAll('.seat-discard-control')).toHaveLength(2)
    await expect(canvasElement.querySelector('.seat-top .discard-counts')).toHaveTextContent('0—10')
    await expect(canvas.getByRole('img', { name: '火 3' })).toBeVisible()
    await expect(canvasElement.querySelectorAll('.featured-discard-card')).toHaveLength(1)
    await expect(canvasElement.querySelectorAll('.shared-discard-dock')).toHaveLength(0)
  },
}

export const TerminalFormation: Story = {
  args: { state: terminalState, mode: 'replay' },
  render: args => ({
    components: { BattlefieldBoard, GameConclusionPanel },
    setup: () => ({
      args,
      teamLabel: (team: string) => team === winningTeam ? '我方' : '對方',
      playerLabel: (player: string) => player === 'alice' ? '小明' : player,
    }),
    template: `<div style="width: 100%; height: 100vh; min-height: 560px;">
      <BattlefieldBoard v-bind="args">
        <template #terminal-resolution>
          <GameConclusionPanel :state="args.state" :team-label="teamLabel" :player-label="playerLabel">
            <template #actions><button class="primary-button" type="button">返回房間</button></template>
          </GameConclusionPanel>
        </template>
      </BattlefieldBoard>
    </div>`,
  }),
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const resultPanel = canvasElement.querySelector('.formation-field .result-panel')
    if (!resultPanel) throw new Error('終局結算應顯示在陣法區')
    await expect(resultPanel).toBeVisible()
    await expect(canvasElement.querySelectorAll('.board-center > .result-panel')).toHaveLength(0)
    await expect(canvasElement.querySelectorAll('.formation-field-heading')).toHaveLength(0)
    await expect(canvas.getByRole('heading', { name: '我方 勝利' })).toBeVisible()
    await expect(canvas.getByText('對方 的生命值歸零')).toBeVisible()
    const result = within(resultPanel)
    await expect(result.getByText('小明', { exact: true })).toBeVisible()
    await expect(result.getByText('烈焰陣', { exact: true })).toBeVisible()
    await expect(canvasElement.querySelectorAll('.formation-field .formation-card')).toHaveLength(1)
    await expect(canvasElement.textContent).not.toContain('先手')
    await expect(canvas.getByRole('button', { name: '返回房間' })).toBeVisible()
    await expect(canvas.getByRole('button', { name: '返回房間' })).toHaveClass(/primary-button/)
  },
}
