import type { Meta, StoryObj } from '@storybook/vue3-vite'
import { computed, defineComponent, nextTick, ref } from 'vue'
import { expect, userEvent, waitFor, within } from 'storybook/test'
import ReplayEventSheet from '~/components/ReplayEventSheet.vue'
import type { BattleRecordTimelineGroup } from '~/components/BattleRecordTimeline.vue'

const players = [
  { player: 'alice', displayName: '小明' },
  { player: 'bob', displayName: '小華' },
] as const

const recordEntries = [
  { id: 'turn-3-1', title: '小明發動烈焰陣', summary: '以火 3、水 2、木 1 組成陣法。' },
  { id: 'turn-3-2', title: '小華選擇承受傷害', summary: '我方隊伍生命值減少 18。' },
  ...Array.from({ length: 22 }, (_, index) => ({
    id: `turn-3-${index + 3}`,
    title: `第 ${index + 3} 筆對局紀錄`,
    summary: `記錄視角：第 ${index + 1} 次行動的結算內容。`,
  })),
]

const ReplayEventSheetStory = defineComponent({
  components: { ReplayEventSheet },
  setup() {
    const open = ref(false)
    const perspective = ref('alice')
    const trigger = ref<HTMLButtonElement | null>(null)
    const groups = computed<BattleRecordTimelineGroup[]>(() => [
      {
        id: 'turn-3',
        title: '第 3 回合 · 小明',
        turnNumber: 3,
        entries: recordEntries.map(entry => ({
          ...entry,
          summary: `${entry.summary}（${perspective.value === 'alice' ? '小明' : '小華'}視角）`,
        })),
      },
      {
        id: 'turn-4',
        title: '第 4 回合 · 小華的回合',
        turnNumber: 4,
        entries: [],
      },
    ])
    function close() {
      open.value = false
      void nextTick(() => trigger.value?.focus())
    }
    return { close, groups, open, perspective, players, trigger }
  },
  template: `
    <main class="replay-event-sheet-story">
      <button ref="trigger" type="button" aria-haspopup="dialog" @click="open = true">開啟戰局紀錄</button>
      <ReplayEventSheet
        :open="open"
        :perspective="perspective"
        :players="players"
        :groups="groups"
        :current-turn-number="4"
        @update:perspective="perspective = $event"
        @close="close"
      />
    </main>
  `,
})

const meta = {
  title: '對局/重播手機紀錄面板',
  component: ReplayEventSheetStory,
  tags: ['battlefield'],
  parameters: { layout: 'fullscreen' },
} satisfies Meta<typeof ReplayEventSheetStory>

export default meta
type Story = StoryObj<typeof meta>

export const PerspectiveAndFocus: Story = {
  play: async ({ canvasElement }) => {
    const canvas = within(canvasElement)
    const page = within(canvasElement.ownerDocument.body)
    const trigger = canvas.getByRole('button', { name: '開啟戰局紀錄' })
    await userEvent.click(trigger)
    const dialog = await page.findByRole('dialog', { name: '戰局紀錄' })
    const close = within(dialog).getByRole('button', { name: '關閉戰局紀錄' })
    const perspective = within(dialog).getByRole('combobox', { name: '重播視角' })
    const feed = dialog.querySelector<HTMLElement>('.replay-event-sheet-feed')
    if (!feed) throw new Error('缺少重播紀錄捲動區')
    await expect(perspective).toBeVisible()
    await expect(close).toHaveFocus()
    await waitFor(() => expect(feed.scrollHeight).toBeGreaterThan(feed.clientHeight))
    await waitFor(() => expect(feed.scrollHeight - feed.scrollTop - feed.clientHeight).toBeLessThanOrEqual(24))
    await userEvent.tab()
    await expect(perspective).toHaveFocus()
    await userEvent.selectOptions(perspective, 'bob')
    await expect(perspective).toHaveValue('bob')
    feed.scrollTop = 0
    feed.dispatchEvent(new Event('scroll'))
    await waitFor(() => expect(feed.scrollTop).toBe(0))
    await userEvent.selectOptions(perspective, 'alice')
    await expect(perspective).toHaveValue('alice')
    await waitFor(() => expect(feed.scrollTop).toBe(0))
    feed.scrollTop = feed.scrollHeight
    feed.dispatchEvent(new Event('scroll'))
    await waitFor(() => expect(feed.scrollHeight - feed.scrollTop - feed.clientHeight).toBeLessThanOrEqual(24))
    await userEvent.selectOptions(perspective, 'bob')
    await expect(perspective).toHaveValue('bob')
    await waitFor(() => expect(feed.scrollHeight - feed.scrollTop - feed.clientHeight).toBeLessThanOrEqual(24))
    await userEvent.tab()
    await expect(close).toHaveFocus()
    await userEvent.click(close)
    await expect(page.queryByRole('dialog', { name: '戰局紀錄' })).toBeNull()
    await expect(trigger).toHaveFocus()
    await userEvent.click(trigger)
    await expect(await page.findByRole('dialog', { name: '戰局紀錄' })).toBeVisible()
  },
}

