import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { expect, userEvent, waitFor, within } from 'storybook/test'
import { ref } from 'vue'
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

const multiModulePlayerDetailState = createExampleGameState()
multiModulePlayerDetailState.ruleVersion = '5.17'
multiModulePlayerDetailState.enabledRuleModules = [
  'star',
  'hero-schools',
  'discard-retrieval',
  'personal-deck',
  'five-directions-legend',
  'spirit',
  'jianghu',
  'confluence-generation',
  'dark-glimmer',
  'echo',
  'tribulation',
  'pouch',
  'totem-formation',
]
multiModulePlayerDetailState.turnNumber = 3
multiModulePlayerDetailState.currentPlayer = 'alice'
multiModulePlayerDetailState.phase = 'Action'
multiModulePlayerDetailState.hands = [
  { player: 'alice', cards: { kind: 'known', cards: exampleCards.slice(0, 2) } },
  { player: 'bob', cards: { kind: 'hidden', count: 5 } },
]
multiModulePlayerDetailState.teamStars = [{ team: 'team:alice', star: 'Metal' }]
multiModulePlayerDetailState.starHistories = [{ player: 'alice', stars: ['Fire', 'Wood'] }]
multiModulePlayerDetailState.spirits = [{ player: 'alice', spirit: 'Fire', power: 2 }]
multiModulePlayerDetailState.totems = [{ player: 'alice', totem: 'AzureHorn', name: '青角圖騰' }]
multiModulePlayerDetailState.professions = [{
  player: 'alice',
  id: 'ink-seeker',
  name: '尋墨客',
  abilities: ['觀星', '借勢'],
}]
multiModulePlayerDetailState.pouches = [
  { owner: 'alice', card: exampleCards[2]! },
  { owner: 'bob', card: null },
]
multiModulePlayerDetailState.coveredPassives = [{
  owner: 'alice',
  formationId: null,
  cards: { kind: 'hidden', count: 2 },
  starSubstitution: null,
}]
multiModulePlayerDetailState.playerDecks = [
  { player: 'alice', cards: { kind: 'partiallyKnown', cards: [exampleCards[1]!, null] } },
  { player: 'bob', cards: { kind: 'hidden', count: 10 } },
]
multiModulePlayerDetailState.jianghuStates = [{
  owner: 'alice',
  kind: 'ThousandBlades',
  remainingTurns: 1,
  expiresOnTurn: 3,
}]
multiModulePlayerDetailState.statuses = [{
  id: 'dark-glimmer-gale-rain',
  owner: { kind: 'player', id: 'alice' },
  kind: 'GaleRain',
  presentation: 'galeRain',
  duration: { type: 'untilTurnEnd', player: 'alice' },
}]
multiModulePlayerDetailState.limitedUses = [{
  owner: 'alice',
  key: 'confluence-tailwind',
  presentation: 'tailwind',
  remaining: 0,
  maximum: 1,
}]
multiModulePlayerDetailState.confluenceCardObligations = [{
  owner: 'alice',
  card: exampleCards[1]!.id,
  allowProfessionFormation: true,
}]
multiModulePlayerDetailState.scheduledEchoes = [{
  player: 'alice',
  melody: 'warFire',
  dueTurnNumber: 5,
}]
multiModulePlayerDetailState.flowStates = [{ player: 'alice', layers: 2 }]
multiModulePlayerDetailState.formationSuppressions = [{
  source: 'bob',
  target: 'alice',
  formationName: '烈焰陣',
  expiresOnTurnNumber: 4,
}]
multiModulePlayerDetailState.scheduledPlantEarth = [{ player: 'alice', dueTurnNumber: 5 }]

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

// 僅情境控制：用新狀態替換圖騰，驗證詳情不殘留舊資訊。
const renderTotemRules: NonNullable<Story['render']> = args => ({
  components: { BattlefieldBoard },
  setup() {
    const state = ref(args.state)
    const clearTotem = () => { state.value = { ...state.value, totems: [] } }
    return { args, state, clearTotem }
  },
  template: `<div style="width: 100%; height: 100vh; min-height: 560px;">
    <button type="button" style="position: fixed; top: 8px; right: 8px; z-index: 100;" @click="clearTotem">清除圖騰（情境控制）</button>
    <BattlefieldBoard v-bind="args" :state="state" />
  </div>`,
})

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
    await expect(handButton).toHaveTextContent('公開手牌')
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
  render: renderTotemRules,
  tags: ['totem-rules'],
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
    await userEvent.click(canvas.getByRole('button', { name: '清除圖騰（情境控制）' }))
    await userEvent.click(trigger)
    const updated = await page.findByRole('dialog', { name: '小明的對局資訊' })
    await expect(within(updated).queryByRole('button', { name: /^青角圖騰：/ })).toBeNull()
    await userEvent.click(within(updated).getByRole('button', { name: '關閉詳情' }))
  },
}

export const MultiModulePlayerDetail: Story = {
  args: { state: multiModulePlayerDetailState },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const page = within(canvasElement.ownerDocument.body)
    const aliceTrigger = canvas.getByRole('button', { name: '查看小明的詳細資訊' })
    await userEvent.click(aliceTrigger)
    const aliceDetail = await page.findByRole('dialog', { name: '小明的對局資訊' })
    await waitFor(() => expect(aliceDetail).toBeVisible())
    const aliceFacts = aliceDetail.querySelector('.player-detail-facts')
    if (!aliceFacts) throw new Error('玩家詳情應顯示資料清單')
    await expect(aliceFacts).toHaveTextContent('金星‧太白')
    await expect(aliceFacts).toHaveTextContent('火、木')
    await expect(aliceFacts).toHaveTextContent('火精靈・靈力 2 / 6')
    await expect(aliceFacts).toHaveTextContent('青角圖騰')
    await expect(aliceFacts).toHaveTextContent('木 1')
    await expect(aliceFacts).toHaveTextContent('2 張')
    await expect(aliceFacts).toHaveTextContent('尋墨客')
    await expect([...aliceFacts.querySelectorAll('dt')].some(label => label.textContent === '公開牌')).toBe(true)
    const aliceStatuses = aliceDetail.querySelector('.player-detail-statuses')
    if (!aliceStatuses) throw new Error('玩家詳情應顯示狀態清單')
    await expect(aliceStatuses).toHaveTextContent('烈風暴雨')
    await expect(aliceStatuses).toHaveTextContent('江湖狀態：千鋒')
    await expect(aliceStatuses).toHaveTextContent('順風 · 0/1')
    await expect(aliceStatuses).toHaveTextContent('調律牌 · 水 2')
    await expect(aliceStatuses).toHaveTextContent('迴響 · 徵調‧戰火 · 第 5 回合')
    await expect(aliceStatuses).toHaveTextContent('流水 · 2 層')
    await expect(aliceStatuses).toHaveTextContent('烈焰陣')
    await expect(aliceStatuses).toHaveTextContent('植土')
    await expect(aliceStatuses.textContent?.match(/江湖狀態：千鋒/g) ?? []).toHaveLength(1)

    await userEvent.click(within(aliceDetail).getByRole('button', { name: '關閉詳情' }))
    const bobTrigger = canvas.getByRole('button', { name: '查看小華的詳細資訊' })
    await userEvent.click(bobTrigger)
    const bobDetail = await page.findByRole('dialog', { name: '小華的對局資訊' })
    await waitFor(() => expect(bobDetail).toBeVisible())
    const bobFacts = bobDetail.querySelector('.player-detail-facts')
    if (!bobFacts) throw new Error('玩家詳情應顯示資料清單')
    await expect(bobFacts).toHaveTextContent('覆蓋牌')
    const bobFactLabels = [...bobFacts.querySelectorAll('dt')].map(label => label.textContent)
    await expect(bobFactLabels).not.toContain('星辰')
    await expect(bobFactLabels).not.toContain('精靈')
    await expect(bobFactLabels).not.toContain('圖騰')
    await expect(bobFactLabels).not.toContain('公開牌')
    await expect(bobFacts.textContent).not.toContain('木 2')
  },
}

export const ReplayTotemRules: Story = {
  render: renderTotemRules,
  tags: ['totem-rules'],
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
    await userEvent.click(canvas.getByRole('button', { name: '清除圖騰（情境控制）' }))
    await userEvent.click(trigger)
    const updated = await page.findByRole('dialog', { name: '小明的對局資訊' })
    await expect(within(updated).queryByRole('button', { name: /^青角圖騰：/ })).toBeNull()
    await userEvent.click(within(updated).getByRole('button', { name: '關閉詳情' }))
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
    await expect(handButton).toHaveTextContent('公開手牌')
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
