import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { defineComponent, onBeforeUnmount, type Component } from 'vue'
import { expect, userEvent, waitFor, within } from 'storybook/test'
import BattlefieldStoryScreen from './BattlefieldStoryScreen.vue'
import { createExampleGameState, createFourPlayerGameState, exampleCards } from '../fixtures/components'
import type { PublicCard } from '~/types/fewfc'
import { createConclusionState } from '../fixtures/conclusion'

const displayNames = { alice: '小明', bob: '小華', carol: '小美', dave: '小安' }

const visualHand: PublicCard[] = [
  { id: 11, label: '土 2', element: 'Earth', level: 2, secretStrategies: [] },
  { id: 12, label: '水 3', element: 'Water', level: 3, secretStrategies: [] },
  { id: 13, label: '木 2', element: 'Wood', level: 2, secretStrategies: [] },
  { id: 14, label: '木 4', element: 'Wood', level: 4, secretStrategies: [] },
  { id: 15, label: '金 1', element: 'Metal', level: 1, secretStrategies: [] },
]

// 固定呈現五張手牌與三張陣法牌，供整頁比例及窄版捲動驗收。
function woodVisualState(fourPlayers = false, handCount = 5) {
  const state = fourPlayers ? fourPlayerLiveState() : createExampleGameState()
  state.status = 'InProgress'
  state.environment = 'Wood'
  state.currentPlayer = 'alice'
  state.turnNumber = 4
  state.phase = 'Action'
  state.hp = state.hp.slice(0, 2).map((entry, index) => ({ ...entry, hp: index === 0 ? 167 : 200 }))
  state.initialHp = state.hp.map(entry => ({ ...entry, hp: 200 }))
  state.deckCount = 32
  state.hands = state.players.map(player => ({
    player: player.id,
    cards: player.id === 'alice'
      ? { kind: 'known', cards: Array.from({ length: handCount }, (_, index) => ({ ...visualHand[index % visualHand.length]!, id: 11 + index })) }
      : { kind: 'hidden', count: 4 },
  }))
  state.previousTurnFormation = {
    player: 'bob',
    formationId: 'wood-bloom',
    formationName: '青木陣',
    cards: { kind: 'known', cards: [1, 2, 4].map((level, index) => ({
      id: 31 + index, label: `木 ${level}`, element: 'Wood', level, secretStrategies: [],
    })) },
  }
  return state
}

function fourPlayerLiveState() {
  const state = createFourPlayerGameState()
  state.environment = 'Wood'
  state.currentPlayer = 'alice'
  state.turnNumber = 4
  state.phase = 'Action'
  state.hp = [{ team: 'sun', hp: 150 }, { team: 'moon', hp: 132 }]
  state.initialHp = [{ team: 'sun', hp: 180 }, { team: 'moon', hp: 180 }]
  return state
}

function fourPlayerSharedDiscardState() {
  const state = fourPlayerLiveState()
  state.turnNumber = 4
  state.deckCount = 32
  state.discard = exampleCards
  state.lastCompletedTurnDiscards = [{ player: 'carol', card: exampleCards[2]!, turnNumber: 3 }]
  return state
}

function fourPlayerPersonalDiscardState() {
  const state = fourPlayerLiveState()
  const discardCards = [
    ...exampleCards,
    { id: 4, label: '金 1', element: 'Metal' as const, level: 1, secretStrategies: [] },
    { id: 5, label: '土 2', element: 'Earth' as const, level: 2, secretStrategies: [] },
    { id: 6, label: '火 1', element: 'Fire' as const, level: 1, secretStrategies: [] },
    { id: 7, label: '水 1', element: 'Water' as const, level: 1, secretStrategies: [] },
    { id: 8, label: '木 2', element: 'Wood' as const, level: 2, secretStrategies: [] },
    { id: 9, label: '金 2', element: 'Metal' as const, level: 2, secretStrategies: [] },
    { id: 10, label: '土 1', element: 'Earth' as const, level: 1, secretStrategies: [] },
  ]
  const counts = [1, 2, 3, 4]
  let cursor = 0
  state.turnNumber = 4
  state.enabledRuleModules = ['personal-deck']
  state.playerDecks = state.turnOrder.map((player, index) => ({
    player,
    cards: { kind: 'hidden' as const, count: 24 - index * 3 },
  }))
  state.playerDiscards = state.turnOrder.map((player, index) => {
    const cards = discardCards.slice(cursor, cursor + counts[index]!)
    cursor += counts[index]!
    return { player, cards }
  })
  state.lastCompletedTurnDiscards = state.playerDiscards.map(pile => ({
    player: pile.player,
    card: pile.cards.at(-1)!,
    turnNumber: 3,
  }))
  return state
}

function choiceRequiredState(decisionPlayer: 'alice' | 'dave' = 'alice') {
  const state = fourPlayerLiveState()
  state.currentPlayer = 'bob'
  state.pendingChoice = {
    visibility: 'visible',
    choiceId: 42,
    player: decisionPlayer,
    reason: { type: 'revelation' },
    choice: { type: 'card', cards: exampleCards, minimum: 1, maximum: 1, canDecline: false },
  }
  return state
}

function currentPlayerExpandedState() {
  const state = fourPlayerLiveState()
  state.currentPlayer = 'bob'
  return state
}

const twoPlayerState = createExampleGameState()
twoPlayerState.phase = 'Action'
twoPlayerState.environment = null

const fireSpiritState = fourPlayerLiveState()
fireSpiritState.environment = 'Fire'

const motionAcceptanceState = structuredClone(fireSpiritState)
motionAcceptanceState.previousTurnFormation = {
  player: 'carol',
  formationId: 'wood-bloom',
  formationName: '青木陣',
  cards: { kind: 'known', cards: exampleCards },
}

function themeDecorator(theme: 'light' | 'dark') {
  return (story: Component): Component => defineComponent({
    components: { story },
    setup() {
      const previousTheme = document.documentElement.dataset.theme
      document.documentElement.dataset.theme = theme
      onBeforeUnmount(() => {
        if (previousTheme) document.documentElement.dataset.theme = previousTheme
        else delete document.documentElement.dataset.theme
      })
      return {}
    },
    template: '<story />',
  })
}
const darkThemeDecorator = themeDecorator('dark')
const lightThemeDecorator = themeDecorator('light')

const meta = {
  title: '對局/完整戰場畫面',
  component: BattlefieldStoryScreen,
  tags: ['battlefield'],
  parameters: { layout: 'fullscreen' },
  args: {
    state: fourPlayerLiveState(),
    displayNames,
    anchorPlayer: 'alice',
    mode: 'live',
  },
} satisfies Meta<typeof BattlefieldStoryScreen>

export default meta
type Story = StoryObj<typeof meta>

export const FourPlayerLive: Story = {}

export const CurrentPlayerExpanded: Story = {
  args: { state: currentPlayerExpandedState() },
  play: async ({ canvasElement }) => {
    const bob = canvasElement.querySelector<HTMLElement>('.seat-top[aria-label*="小華"]')
    const carol = canvasElement.querySelector<HTMLElement>('.seat-top[aria-label*="小美"]')
    if (!bob || !carol) throw new Error('缺少固定順序的上方玩家欄位')
    await expect(bob).toHaveAttribute('aria-current', 'true')
    await expect(bob).toHaveClass(/is-expanded/)
    await new Promise(resolve => window.setTimeout(resolve, 260))
    await expect(bob.getBoundingClientRect().width).toBeGreaterThan(carol.getBoundingClientRect().width)
  },
}

export const OffTurnDecision: Story = {
  args: {
    state: choiceRequiredState('dave'),
  },
  play: async ({ canvasElement }) => {
    const bob = canvasElement.querySelector<HTMLElement>('.seat-top[aria-label*="小華"]')
    const dave = canvasElement.querySelector<HTMLElement>('.seat-top[aria-label*="小安"]')
    if (!bob || !dave) throw new Error('缺少目前回合玩家或決策玩家')
    await expect(bob.getAttribute('aria-label')).toContain('回合玩家')
    await expect(dave.getAttribute('aria-label')).toContain('目前決策者')
    await expect(bob).toHaveClass(/is-expanded/)
    await expect(dave).toHaveClass(/needs-response/)
    await expect(dave).not.toHaveClass(/is-expanded/)
    await new Promise(resolve => window.setTimeout(resolve, 260))
    await expect(bob.getBoundingClientRect().width).toBeGreaterThan(dave.getBoundingClientRect().width)
    const order = [...canvasElement.querySelectorAll<HTMLElement>('.seat-top')]
      .map(seat => seat.getAttribute('aria-label')?.split('，')[0])
    await expect(order).toEqual(['小華', '小美', '小安'])
  },
}

export const FourPlayerSharedDiscard: Story = {
  args: { state: fourPlayerSharedDiscardState() },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const page = within(canvasElement.ownerDocument.body)
    await userEvent.click(canvas.getByRole('button', { name: /查看共用棄牌詳情/ }))
    const detail = await page.findByRole('dialog', { name: '目前棄牌 3 · 牌庫 32' })
    await waitFor(() => expect(detail).toBeVisible())
    const woodRow = within(detail).getByRole('row', { name: /^木 / })
    await expect(within(woodRow).getAllByRole('cell')[0]).toHaveTextContent('1')
  },
}

export const FourPlayerPersonalDiscardPiles: Story = {
  args: { state: fourPlayerPersonalDiscardState() },
  tags: ['ui-migration'],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const page = within(canvasElement.ownerDocument.body)
    const piles = [
      { player: '小明', count: 1, deck: 24, element: '火', level: 3 },
      { player: '小華', count: 2, deck: 21, element: '木', level: 1 },
      { player: '小美', count: 3, deck: 18, element: '火', level: 1 },
      { player: '小安', count: 4, deck: 15, element: '土', level: 1 },
    ]
    await expect(canvasElement.querySelectorAll('.player-seat')).toHaveLength(4)
    await expect(canvasElement.querySelectorAll('.seat-discard-control')).toHaveLength(4)
    await expect(canvasElement.querySelectorAll('.discard-position-top')).toHaveLength(3)
    await expect(canvasElement.querySelectorAll('.discard-position-bottom')).toHaveLength(1)
    await expect(canvasElement.querySelectorAll('.discard-position-left, .discard-position-right')).toHaveLength(0)
    await expect(canvasElement.querySelectorAll('.shared-discard-dock')).toHaveLength(0)
    for (const pile of piles) {
      await userEvent.click(canvas.getByRole('button', { name: new RegExp(`查看${pile.player}的棄牌詳情`) }))
      const detail = await page.findByRole('dialog', {
        name: `${pile.player}目前棄牌 ${pile.count} · 牌庫 ${pile.deck}`,
      })
      const row = within(detail).getByRole('row', { name: new RegExp(`^${pile.element} `) })
      await expect(within(row).getAllByRole('cell')[pile.level - 1]).toHaveTextContent('1')
      await userEvent.click(within(detail).getByRole('button', { name: '關閉詳情' }))
    }
    const details = canvas.getAllByRole('button', { name: /查看.*的詳細資訊/ })
    await expect(details).toHaveLength(4)
    for (const trigger of details) {
      await userEvent.click(trigger)
      const detail = await page.findByRole('dialog')
      await expect(detail.querySelector('.player-detail-facts')).toHaveTextContent('隊伍')
      await waitFor(() => expect(within(detail).getByRole('button', { name: '關閉詳情' })).toHaveFocus())
      await userEvent.keyboard('{Escape}')
      await waitFor(() => expect(page.queryByRole('dialog')).not.toBeInTheDocument())
      await expect(trigger).toHaveFocus()
    }
  },
}

export const FourPlayerObserver: Story = {
  args: { state: fourPlayerLiveState(), anchorPlayer: null },
}

export const FourPlayerReplay: Story = {
  args: { state: fourPlayerLiveState(), anchorPlayer: 'carol', mode: 'replay' },
}

export const TwoPlayerNoEnvironment: Story = {
  args: { state: twoPlayerState, anchorPlayer: 'alice' },
}

export const DarkFourPlayer: Story = {
  args: { state: fourPlayerLiveState() },
  decorators: [darkThemeDecorator],
}

export const WoodTwoPlayerVisual: Story = {
  args: { state: woodVisualState() },
  decorators: [darkThemeDecorator],
}

export const WoodFourPlayerVisual: Story = {
  args: { state: woodVisualState(true) },
  decorators: [darkThemeDecorator],
}

export const LongHandVisual: Story = {
  args: {
    state: woodVisualState(true, 10),
    displayNames: { ...displayNames, alice: '名字很長的五行旅人', bob: '另一位名字很長的旅人' },
  },
  decorators: [darkThemeDecorator],
}

export const DesktopRevealHeader: Story = {
  args: { state: woodVisualState(), showSiteHeader: true },
  decorators: [darkThemeDecorator],
}

export const TwoPlayerAlwaysExpanded: Story = {
  args: { state: woodVisualState(), showMotionDemo: true },
  decorators: [darkThemeDecorator],
}

const manyOpponentCards = woodVisualState(true)
manyOpponentCards.currentPlayer = 'bob'
manyOpponentCards.hands.find(hand => hand.player === 'bob')!.cards = { kind: 'hidden', count: 14 }
export const ExpandedOpponentHand: Story = {
  args: { state: manyOpponentCards, showMotionDemo: true },
  decorators: [darkThemeDecorator],
}

export const LightMetalVisual: Story = {
  args: { state: { ...woodVisualState(), environment: 'Metal' } },
}

export const FireSpiritSplendor: Story = {
  args: { state: fireSpiritState, showSplendorMenu: true },
  decorators: [darkThemeDecorator],
}

export const MotionAcceptance: Story = {
  args: { state: motionAcceptanceState, showMotionDemo: true },
  decorators: [darkThemeDecorator],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const readBattlefieldGeometry = () => [
      '.battle-stage-strip',
      '.team-hp-strip',
      '.seat-top',
      '.center-stack',
      '.board-center',
      '.formation-field',
      '.formation-cards',
      '.formation-card',
      '.seat-bottom',
      '.seat-bottom .seat-hand',
    ].flatMap((selector) => [...canvasElement.querySelectorAll<HTMLElement>(selector)]
      .map((element, index) => {
        const { x, y, width, height } = element.getBoundingClientRect()
        if (selector === '.seat-top') return { selector, index, y: Math.round(y * 10) / 10, height: Math.round(height * 10) / 10 }
        return {
          selector,
          index,
          x: Math.round(x * 10) / 10,
          y: Math.round(y * 10) / 10,
          width: Math.round(width * 10) / 10,
          height: Math.round(height * 10) / 10,
        }
      }))

    const assertGeometryStable = (
      before: ReturnType<typeof readBattlefieldGeometry>,
      after: ReturnType<typeof readBattlefieldGeometry>,
      phase: string,
    ) => {
      if (before.length !== after.length) {
        throw new Error(`${phase} 改變戰場元素數量：${before.length} → ${after.length}`)
      }
      const differences = after.flatMap((entry, index) => {
        const expected = before[index]
        if (!expected) return [{ expected, actual: entry }]
        const matches = Object.entries(entry).every(([key, actual]) => {
          const prior = (expected as Record<string, unknown>)[key]
          return typeof actual === 'number' && typeof prior === 'number'
            ? Math.abs(actual - prior) < 0.5
            : actual === prior
        })
        return matches ? [] : [{ expected, actual: entry }]
      })
      if (differences.length) throw new Error(`${phase} 改變戰場幾何：${JSON.stringify(differences)}`)
    }

    const handCard = canvasElement.querySelector<HTMLButtonElement>(
      '.seat-bottom .seat-hand .game-card[data-card-id="1"]',
    )
    if (!handCard) throw new Error('缺少可選取的火 3 手牌')
    const handCardBefore = handCard.getBoundingClientRect()
    await userEvent.click(handCard)
    await expect(handCard).toHaveAttribute('aria-pressed', 'true')
    await waitFor(() => {
      const selectedCard = handCard.getBoundingClientRect()
      expect(selectedCard.top).toBeLessThan(handCardBefore.top - 10)
      expect(selectedCard.height).toBeGreaterThan(handCardBefore.height * 1.03)
    })
    const selectionBadge = handCard.querySelector<HTMLElement>('.game-card-selected')
    if (!selectionBadge) throw new Error('缺少手牌選取狀態節點')
    await expect(window.getComputedStyle(selectionBadge).display).toBe('none')
    await userEvent.click(handCard)

    const before = readBattlefieldGeometry()
    const currentRecordGroup = canvasElement.querySelector('.battle-record-group[aria-current="step"]')
    if (!currentRecordGroup) throw new Error('戰局紀錄缺少目前回合高亮')
    await expect(currentRecordGroup.textContent).toContain('目前回合')
    await userEvent.click(canvas.getByRole('button', { name: '切換回合提示' }))
    await new Promise(resolve => window.setTimeout(resolve, 240))
    const afterTurnSwitch = readBattlefieldGeometry()
    assertGeometryStable(before, afterTurnSwitch, '切換回合提示')
    await userEvent.click(canvas.getByRole('button', { name: '切換回合提示' }))
    await new Promise(resolve => window.setTimeout(resolve, 240))
    assertGeometryStable(before, readBattlefieldGeometry(), '還原回合提示')

    const beforePlay = readBattlefieldGeometry()
    await userEvent.click(canvas.getByRole('button', { name: '預覽成功出牌' }))
    await expect(canvas.getByRole('button', { name: '重設出牌預覽' })).toBeVisible()
    await expect(canvasElement.querySelectorAll('.card-flight-ghost')).toHaveLength(1)
    const afterPlay = readBattlefieldGeometry()
    assertGeometryStable(beforePlay, afterPlay, '出牌動畫')
    await new Promise(resolve => window.setTimeout(resolve, 550))
    await userEvent.click(canvas.getByRole('button', { name: '重設出牌預覽' }))
  },
}

export const ResponseNeededWhileReading: Story = {
  args: { state: fourPlayerSharedDiscardState(), showMotionDemo: true },
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const page = within(canvasElement.ownerDocument.body)
    await userEvent.click(canvas.getByRole('button', { name: /查看共用棄牌詳情/ }))
    const detail = await page.findByRole('dialog', { name: '目前棄牌 3 · 牌庫 32' })
    // 模擬對局更新抵達，避免滑鼠點擊外部預覽控制先關閉詳情。
    canvas.getByRole('button', { name: '模擬必要選擇' }).click()
    await waitFor(() => {
      expect(detail).toBeVisible()
      expect(within(detail).getByRole('alert')).toHaveTextContent('需要你回應')
      expect(within(detail).getByRole('button', { name: /前往選擇/ })).toBeVisible()
    })
  },
}

export const ThemeAcceptance: Story = {
  args: { state: fourPlayerLiveState() },
  play: async ({ canvasElement }) => {
    const root = canvasElement.ownerDocument.documentElement
    const previousTheme = root.dataset.theme
    const readTheme = () => {
      const board = canvasElement.querySelector<HTMLElement>('.battlefield')
      const seat = canvasElement.querySelector<HTMLElement>('.player-seat')
      if (!board || !seat) throw new Error('缺少戰場主題驗收元件')
      return {
        canvas: window.getComputedStyle(board).getPropertyValue('--app-canvas').trim(),
        text: window.getComputedStyle(board).color,
        background: window.getComputedStyle(board.closest('.battle-scene') ?? board).backgroundColor,
        glass: window.getComputedStyle(seat).backgroundImage,
      }
    }
    try {
      root.dataset.theme = 'light'
      const light = readTheme()
      root.dataset.theme = 'dark'
      const dark = readTheme()
      await expect(dark.canvas).not.toBe(light.canvas)
      await expect(dark.text).not.toBe(light.text)
      await expect(dark.background).not.toBe(light.background)
      await expect(dark.glass).not.toBe(light.glass)
    } finally {
      if (previousTheme) root.dataset.theme = previousTheme
      else delete root.dataset.theme
    }
  },
}

export const TerminalVictory: Story = {
  args: { state: createConclusionState(), showSiteHeader: true },
  decorators: [lightThemeDecorator],
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const result = within(canvas.getByRole('region', { name: '對局結算' }))
    await waitFor(() => expect(result.getByRole('heading', { name: '我方 勝利' })).toBeVisible())
    await waitFor(() => expect(canvas.getByText('戰局結束', { selector: '.turn-capsule strong' })).toBeVisible())
    await expect(canvas.getByRole('meter', { name: '對方隊伍 HP' })).toHaveAttribute('aria-valuenow', '0')
    await expect(canvasElement.querySelectorAll('.is-your-turn, .needs-response, .player-seat.acting')).toHaveLength(0)
    await expect(canvasElement.querySelector('.action-dock-slot')).toBeNull()
    await expect(result.getByRole('button', { name: '返回房間' })).toBeVisible()
  },
}
export const TerminalVictoryDark: Story = { args: { state: createConclusionState(), showSiteHeader: true }, decorators: [darkThemeDecorator] }
export const TerminalDraw: Story = { args: { state: createConclusionState('draw', true) }, decorators: [lightThemeDecorator] }
export const TerminalDirectVictory: Story = { args: { state: createConclusionState('direct'), competition: true }, decorators: [darkThemeDecorator] }
export const TerminalAutomatic: Story = { args: { state: createConclusionState('automatic') }, decorators: [lightThemeDecorator] }
export const TerminalDiscardRetrieval: Story = { args: { state: createConclusionState('retrieval') }, decorators: [darkThemeDecorator] }
export const TerminalWithoutResolution: Story = { args: { state: createConclusionState('no-resolution') }, decorators: [lightThemeDecorator] }
export const TerminalObserver: Story = { args: { state: createConclusionState('hidden', true), anchorPlayer: null }, decorators: [darkThemeDecorator] }
export const TerminalReplay: Story = { args: { state: createConclusionState('victory', true), mode: 'replay', anchorPlayer: 'bob' }, decorators: [darkThemeDecorator] }
