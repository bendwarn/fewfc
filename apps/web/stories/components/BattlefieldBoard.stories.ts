import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { expect, userEvent, waitFor, within } from 'storybook/test'
import BattlefieldBoard from '~/components/BattlefieldBoard.vue'
import GameConclusionPanel from '~/components/GameConclusionPanel.vue'
import { createExampleGameState, createFourPlayerGameState, exampleCards } from '../fixtures/components'

const displayNames = { alice: '小明', bob: '小華', carol: '小美', dave: '小安' }

const twoPlayerState = createExampleGameState()
twoPlayerState.currentPlayer = 'bob'
twoPlayerState.phase = 'Action'
twoPlayerState.environment = 'Water'

const fourPlayerState = createFourPlayerGameState()
const exposedForeignHandState = createFourPlayerGameState()
exposedForeignHandState.hands.find(hand => hand.player === 'bob')!.cards = {
  kind: 'partiallyKnown',
  cards: [exampleCards[0]!, null, exampleCards[1]!],
}
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
    await expect(canvasElement.querySelectorAll('.team-hp-meter')).toHaveLength(2)
    const actingSeat = canvasElement.querySelector('.player-seat[aria-current="true"]')
    await expect(actingSeat?.getAttribute('aria-label')).toContain('小華，對方')
    await expect(actingSeat).toHaveAttribute('aria-current', 'true')
    await expect(canvasElement.querySelector('.environment-chip')).toHaveTextContent('水行')
  },
}

export const FourPlayerPlayerView: Story = {
  args: { state: fourPlayerState, anchorPlayer: 'alice' },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvasElement.querySelectorAll('.team-hp-meter')).toHaveLength(2)
    await expect(canvasElement.querySelectorAll('.player-seat')).toHaveLength(4)
    const actingSeat = canvasElement.querySelector('.player-seat[aria-current="true"]')
    await expect(actingSeat?.getAttribute('aria-label')).toContain('小華，對方')
    await expect(actingSeat).toHaveAttribute('aria-current', 'true')
    const actingStyle = window.getComputedStyle(actingSeat)
    await expect(actingStyle.boxShadow).not.toBe('none')
    await expect(canvasElement.querySelector('.seat-bottom')?.getAttribute('aria-label')).toContain('小明，我方')
  },
}

export const FourPlayerObserverView: Story = {
  args: { state: fourPlayerState, anchorPlayer: null },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvasElement.querySelectorAll('.player-seat')).toHaveLength(4)
    await expect(canvasElement.querySelector('.seat-bottom')?.getAttribute('aria-label')).toContain('小明，隊伍 1')
    await expect(canvasElement.querySelector('.player-seat[aria-current="true"]')?.getAttribute('aria-label')).toContain('小華，隊伍 2')
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
    await expect(canvasElement.querySelector('.seat-bottom')?.getAttribute('aria-label')).toContain('小美，我方')
    await expect(canvasElement.querySelector('.player-seat[aria-current="true"]')?.getAttribute('aria-label')).toContain('小華，對方')
    await userEvent.click(canvas.getByRole('button', { name: '查看小明的詳細資訊' }))
    const page = within(canvasElement.ownerDocument.body)
    const detail = await page.findByRole('dialog', { name: '小明的對局資訊' })
    await waitFor(() => expect(detail).toBeVisible())
    await expect(within(detail).getByRole('img', { name: '火 3' })).toBeVisible()
  },
}

export const LivePublicForeignHand: Story = {
  args: { state: exposedForeignHandState, anchorPlayer: 'alice', mode: 'live' },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const page = within(canvasElement.ownerDocument.body)
    const handButton = canvas.getByRole('button', { name: /查看小華的公開手牌/ })
    await expect(handButton).toHaveTextContent('手牌 3・公開 2')
    await expect(handButton).toHaveAccessibleName(/手牌 3 張，2 張已知/)
    await userEvent.click(handButton)
    const dialog = await page.findByRole('dialog', { name: '小華的公開手牌 2 / 3' })
    await waitFor(() => expect(dialog).toBeVisible())
    await expect(within(dialog).getByRole('img', { name: '火 3' })).toBeVisible()
    await expect(within(dialog).getByRole('img', { name: '水 2' })).toBeVisible()
    await expect(dialog.querySelectorAll('.hand-detail-cards .playing-card')).toHaveLength(2)
  },
}

export const LiveTotemRules: Story = {
  args: { state: totemState },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const trigger = canvas.getByRole('button', { name: '查看小明的詳細資訊' })
    await userEvent.click(trigger)
    const page = within(canvasElement.ownerDocument.body)
    const detail = await page.findByRole('dialog', { name: '小明的對局資訊' })
    await waitFor(() => expect(detail).toBeVisible())
    const totem = within(detail).getByRole('button', { name: /^青角圖騰：/ })
    await expect(totem).toHaveTextContent('青角圖騰')
    await expect(totem.getAttribute('title')).toContain('防護罩不適用')
    await waitFor(() => expect(within(detail).getByRole('button', { name: '關閉詳情' })).toHaveFocus())
    await userEvent.tab()
    await expect(totem).toHaveFocus()
    await userEvent.click(within(detail).getByRole('button', { name: '關閉詳情' }))
    await expect(page.queryByRole('dialog', { name: '小明的對局資訊' })).toBeNull()
    await waitFor(() => expect(trigger).toHaveFocus())
  },
}

export const ReplayTotemRules: Story = {
  args: { state: totemState, mode: 'replay' },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const trigger = canvas.getByRole('button', { name: '查看小明的詳細資訊' })
    await userEvent.click(trigger)
    const page = within(canvasElement.ownerDocument.body)
    const detail = await page.findByRole('dialog', { name: '小明的對局資訊' })
    await waitFor(() => expect(detail).toBeVisible())
    const totem = within(detail).getByRole('button', { name: /^青角圖騰：/ })
    await expect(totem).toHaveTextContent('青角圖騰')
    await expect(totem.getAttribute('title')).toContain('防護罩不適用')
    await waitFor(() => expect(within(detail).getByRole('button', { name: '關閉詳情' })).toHaveFocus())
    await userEvent.tab()
    await expect(totem).toHaveFocus()
    await userEvent.click(within(detail).getByRole('button', { name: '關閉詳情' }))
    await expect(page.queryByRole('dialog', { name: '小明的對局資訊' })).toBeNull()
    await waitFor(() => expect(trigger).toHaveFocus())
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
    await userEvent.click(handButton)
    const page = within(canvasElement.ownerDocument.body)
    const dialog = await page.findByRole('dialog', { name: '小華的手牌 1' })
    await waitFor(() => expect(dialog).toBeVisible())
    await expect(within(dialog).getByRole('img', { name: '火 3' })).toBeVisible()
    await expect(canvasElement.ownerDocument.body.querySelectorAll('.hand-detail-cards .playing-card')).toHaveLength(1)
  },
}

export const PersonalDeckDiscardSummary: Story = {
  args: { state: personalDeckState },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    await expect(canvasElement.querySelectorAll('.seat-discard-control')).toHaveLength(2)
    await expect(canvasElement.querySelectorAll('.seat-top .seat-discard-control')).toHaveLength(1)
    await expect(canvasElement.querySelectorAll('.seat-bottom .seat-discard-control')).toHaveLength(1)
    await expect(canvasElement.querySelector('.seat-top .discard-counts')).toHaveTextContent('0—10')
    await expect(canvas.getByRole('img', { name: '火 3' })).toBeVisible()
    await expect(canvasElement.querySelectorAll('.featured-discard-card')).toHaveLength(1)
    await expect(canvasElement.querySelectorAll('.shared-discard-dock')).toHaveLength(0)

    const formation = canvasElement.querySelector('.formation-field')!.getBoundingClientRect()
    const overlapsFormation = [...canvasElement.querySelectorAll('.seat-discard-control')].some((pile) => {
      const box = pile.getBoundingClientRect()
      return box.left < formation.right
        && box.right > formation.left
        && box.top < formation.bottom
        && box.bottom > formation.top
    })
    await expect(overlapsFormation).toBe(false)
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
