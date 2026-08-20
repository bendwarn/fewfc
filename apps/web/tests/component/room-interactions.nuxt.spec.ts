import { mountSuspended } from '@nuxt/test-utils/runtime'
import { afterEach, describe, expect, it, vi } from 'vitest'
import CardChoiceMatrix from '~/components/CardChoiceMatrix.vue'
import ChainChoice from '~/components/ChainChoice.vue'
import GameCard from '~/components/GameCard.vue'
import SplitEarthFormationChoice from '~/components/SplitEarthFormationChoice.vue'
import ThemeSelector from '~/components/ThemeSelector.vue'

const cards = [
  { id: 11, label: '金 2 級', element: 'Metal' as const, level: 2, secretStrategies: [] },
  { id: 12, label: '火 4 級', element: 'Fire' as const, level: 4, secretStrategies: [] },
]

afterEach(() => {
  document.querySelectorAll('.game-card-preview-layer').forEach(element => element.remove())
  localStorage.clear()
  delete document.documentElement.dataset.theme
  document.documentElement.style.colorScheme = ''
  vi.useRealTimers()
})

describe('ThemeSelector', () => {
  it('persists explicit preference and follows system changes in system mode', async () => {
    let systemListener: ((event: MediaQueryListEvent) => void) | undefined
    const media = {
      matches: true,
      addEventListener: (_event: string, listener: (event: MediaQueryListEvent) => void) => {
        systemListener = listener
      },
      removeEventListener: vi.fn(),
    }
    vi.stubGlobal('matchMedia', vi.fn(() => media))

    const wrapper = await mountSuspended(ThemeSelector)
    await wrapper.get('[aria-label^="外觀："]').trigger('click')
    await wrapper.findAll('[role="menuitemradio"]').find(option => option.text().includes('亮色'))!.trigger('click')
    expect(document.documentElement.dataset.theme).toBe('light')
    expect(localStorage.getItem('fewfc-color-theme')).toBe('light')

    await wrapper.get('[aria-label^="外觀："]').trigger('click')
    await wrapper.findAll('[role="menuitemradio"]').find(option => option.text().includes('系統'))!.trigger('click')
    expect(document.documentElement.dataset.theme).toBeUndefined()
    expect(document.documentElement.style.colorScheme).toBe('dark')

    systemListener?.({ matches: false } as MediaQueryListEvent)
    await wrapper.vm.$nextTick()
    expect(document.documentElement.style.colorScheme).toBe('light')
  })
})

describe('GameCard', () => {
  it('previews after a long press without selecting, then still selects on a later click', async () => {
    vi.useFakeTimers()
    const wrapper = await mountSuspended(GameCard, {
      attachTo: document.body,
      props: { card: cards[0], selectable: true },
    })

    await wrapper.get('button').trigger('pointerdown', { button: 0 })
    await vi.advanceTimersByTimeAsync(480)
    expect(document.querySelector('.game-card-preview-layer')).not.toBeNull()

    await wrapper.get('button').trigger('pointerup', { button: 0 })
    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('select')).toBeUndefined()

    await wrapper.get('button').trigger('click')
    expect(wrapper.emitted('select')).toStrictEqual([[]])
  })
})

describe('CardChoiceMatrix', () => {
  it('announces the cell and emits the precise selected card id', async () => {
    const wrapper = await mountSuspended(CardChoiceMatrix, {
      props: {
        cards,
        label: '商調‧鳴金牌組矩陣',
        caption: '從牌組選擇一張牌',
        actionLabel: '選擇牌',
      },
    })

    const metalTwo = wrapper.get('[aria-label="選擇牌：金 2 級，共 1 張，已選 0 張"]')
    expect(wrapper.get('[role="group"]').attributes('aria-label')).toBe('商調‧鳴金牌組矩陣')
    await metalTwo.trigger('click')
    expect(wrapper.emitted('choose')).toStrictEqual([[11]])
  })
})

describe('ChainChoice', () => {
  const chainProps = {
    pouchOwners: ['p1'],
    pouchOwner: 'p1',
    pouchCards: cards,
    pouchCard: cards[0],
    triggerCards: [cards[1]],
    triggerCard: cards[1],
    strategyOptions: [],
    selectedStrategy: null,
    selectedStrategyAction: null,
    selectedTarget: null,
    selectedStar: null,
    breakStar: false,
    selectedRetreatCard: null,
    handCards: [],
    playerLabel: (player: string) => ({ p1: '甲', p2: '乙' })[player] ?? player,
    strategyLabel: (strategy: string) => strategy,
    starLabel: (star: string) => star,
  }

  it('summarizes the sole pouch recipient before the cards when no local recipient draft exists', async () => {
    const wrapper = await mountSuspended(ChainChoice, {
      props: {
        ...chainProps,
        pouchOwner: null,
      },
    })

    expect(wrapper.find('.choice-selection-summary').text()).toContain('錦囊給予：甲')
    expect(wrapper.find('[aria-label="選擇錦囊持有者"]').exists()).toBe(false)
    expect(wrapper.findAll('h3').map(heading => heading.text())).toEqual([
      '錦囊給予對象',
      '選擇錦囊牌',
      '選擇觸發牌（可選）',
      '觸發秘計',
    ])
  })

  it('keeps the existing selected owner and selector when multiple recipients are available', async () => {
    const wrapper = await mountSuspended(ChainChoice, {
      props: {
        ...chainProps,
        pouchOwners: ['p1', 'p2'],
      },
    })

    const owners = wrapper.get('[aria-label="選擇錦囊持有者"]')
    expect(owners.get('button').attributes('aria-pressed')).toBe('true')
    await owners.findAll('button')[1].trigger('click')
    expect(wrapper.emitted('select-pouch-owner')).toStrictEqual([['p2']])
  })
})

describe('SplitEarthFormationChoice', () => {
  it('keeps formation selection local until it emits the chosen formation', async () => {
    const wrapper = await mountSuspended(SplitEarthFormationChoice, {
      props: {
        groups: [
          { ruleModuleId: null, formations: [{ id: 'weapon', name: '武器' }] },
          { ruleModuleId: 'five-directions-legend', formations: [{ id: 'azure-dragon', name: '東‧青龍' }] },
        ],
      },
    })

    await wrapper.get('[aria-label="選擇規則 五方傳說"]').trigger('click')
    expect(wrapper.find('[aria-label="選擇提供陣法的規則"]').exists()).toBe(false)
    await wrapper.get('[aria-label="選擇陣法 東‧青龍"]').trigger('click')
    expect(wrapper.emitted('select')).toStrictEqual([['azure-dragon']])
  })
})
