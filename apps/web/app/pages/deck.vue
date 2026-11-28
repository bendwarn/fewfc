<template>
  <main v-if="deckInitialLoading" class="route-state deck-route-state" aria-live="polite">
    <div class="route-state-content"><span class="route-spinner" aria-hidden="true" /><h1>正在載入</h1><p>正在取得個人牌組與規則。</p></div>
  </main>
  <main v-else-if="deckInitialError" class="route-state deck-route-state">
    <div class="route-state-content">
      <h1>{{ deckInitialError }}</h1>
      <button class="deck-button" data-tone="import" type="button" @click="loadDeckPage">
        <DeckActionIcon name="retry" />重試
      </button>
    </div>
  </main>
  <main v-else class="deck-page workspace-page page-scene" data-scene="study">
    <section class="setup-card deck-editor" aria-label="調整牌組">
      <header class="deck-editor-heading">
        <h1 class="deck-page-title">個人牌組</h1>
        <div class="deck-name-field">
          <label for="deck-name">牌組名稱</label>
          <input id="deck-name" v-model.trim="deckDraft.name" maxlength="24" :disabled="deckBusy" @input="deckFeedback = null">
        </div>
      </header>

      <table class="deck-grid" aria-label="個人牌組卡牌張數">
        <colgroup>
          <col class="deck-element-column">
          <col :span="deckLevelCounts.length">
          <col class="deck-card-total-column">
          <col class="deck-level-total-column">
        </colgroup>
        <thead>
          <tr>
            <th scope="col">屬性</th>
            <th v-for="{ level } in deckLevelCounts" :key="level" scope="col">{{ level }} 級</th>
            <th scope="col" class="deck-subtotal-heading" aria-label="屬性張數">張數</th>
            <th scope="col" class="deck-subtotal-heading" aria-label="屬性等級總和">等級總和</th>
          </tr>
        </thead>
        <tbody>
          <tr v-for="row in deckRows" :key="row.element">
            <th scope="row" class="deck-element" :data-element="row.element"><span>{{ row.name }}</span></th>
            <td v-for="cell in row.cells" :key="cell.level">
              <div class="deck-count-control">
                <button
                  class="deck-count-step"
                  data-tone="remove"
                  type="button"
                  :aria-label="`減少${row.name}${cell.level}級張數`"
                  :disabled="deckBusy || !cell.id || cell.count === 0"
                  @click="setDeckCardCount(cell, cell.count - 1)"
                ><DeckActionIcon name="remove" /></button>
                <button
                  class="deck-count-button"
                  :class="{ 'deck-count-over-limit': cell.count > cell.limit }"
                  type="button"
                  :aria-label="`${row.name}${cell.level}級張數：${cell.count} 張`"
                  aria-haspopup="menu"
                  :aria-expanded="!!countPicker && countPicker.id === cell.id"
                  :aria-controls="countPicker?.id === cell.id ? 'deck-count-menu' : undefined"
                  :disabled="deckBusy || !cell.id"
                  @click="toggleCountPicker($event, row.name, cell)"
                  @keydown.esc="countPicker?.id === cell.id && closeCountPicker(true)"
                >{{ cell.count }}</button>
                <button
                  class="deck-count-step"
                  data-tone="add"
                  type="button"
                  :aria-label="`增加${row.name}${cell.level}級張數`"
                  :disabled="deckBusy || !cell.id || cell.count >= cell.limit"
                  @click="setDeckCardCount(cell, cell.count + 1)"
                ><DeckActionIcon name="add" /></button>
              </div>
            </td>
            <td class="deck-subtotal"><span>{{ row.cardCount }}</span></td>
            <td class="deck-subtotal"><span>{{ row.levelTotal }}</span></td>
          </tr>
        </tbody>
        <tfoot aria-live="polite">
          <tr>
            <th scope="row"><span class="deck-level-count-label"><span>等級</span><span>張數</span></span></th>
            <td v-for="{ level, count } in deckLevelCounts" :key="level"><span class="deck-column-total">{{ count }}</span></td>
            <td>
              <span class="deck-grand-total" :class="{ invalid: deckValidation.cardCount !== deckRules?.exactCardCount }">
                {{ deckValidation.cardCount }} <small>/ {{ deckRules?.exactCardCount }}</small><span class="sr-only"> 張</span>
              </span>
            </td>
            <td>
              <span class="deck-grand-total" :class="{ invalid: deckValidation.levelTotal > (deckRules?.maximumLevelTotal ?? 0) }">
                {{ deckValidation.levelTotal }} <small>/ {{ deckRules?.maximumLevelTotal }}</small><span class="sr-only"> 級</span>
              </span>
            </td>
          </tr>
        </tfoot>
      </table>

      <div class="deck-overview">
        <div class="deck-summary">
          <span>{{ deckSource === 'custom' ? '目前使用自訂牌組' : '目前使用內建預組' }}</span>
          <button
            v-if="deckValidation.errors.length"
            class="deck-issues-button"
            data-tone="remove"
            type="button"
            aria-label="查看牌組問題"
            :disabled="deckBusy"
            @click="openDeckIssues"
          ><DeckActionIcon name="alert" />{{ deckValidation.errors.length }} 項問題</button>
        </div>

        <div class="deck-actions">
          <div class="deck-management-actions">
            <button class="deck-button" data-tone="import" type="button" :disabled="deckBusy" @click="openDeckImport">
              <DeckActionIcon name="import" />匯入牌組
            </button>
            <button class="deck-button" data-tone="export" type="button" :disabled="deckBusy" @click="exportDeck">
              <DeckActionIcon name="export" />匯出牌組
            </button>
            <button class="deck-button" data-tone="remove" type="button" :disabled="deckBusy" @click="resetDeck">
              <DeckActionIcon name="reset" />{{ deckRequest === 'reset' ? '重設中…' : '重設為預組' }}
            </button>
          </div>
          <div class="deck-primary-actions">
            <button class="deck-button" type="button" :disabled="deckBusy" @click="returnToLobby">
              <DeckActionIcon name="back" />返回房間
            </button>
            <button
              class="deck-button"
              data-tone="primary"
              type="button"
              :disabled="deckBusy || !deckValidation.valid"
              @click="saveDeck"
            ><DeckActionIcon name="save" />{{ deckRequest === 'save' ? '儲存中…' : '儲存牌組' }}</button>
          </div>
        </div>
        <p
          v-if="deckFeedback"
          class="deck-feedback"
          :class="{ 'deck-feedback-error': deckFeedback.error }"
          :role="deckFeedback.error ? 'alert' : 'status'"
        ><DeckActionIcon :name="deckFeedback.error ? 'alert' : 'check'" />{{ deckFeedback.message }}</p>
      </div>
    </section>

    <AnchoredSurface
      v-if="countPicker"
      :open="true"
      :anchor="countPicker.anchor"
      :offset="6"
      :return-focus="false"
      surface-class="deck-count-picker"
      @positioned="focusCurrentCount"
      @keydown="handleCountPickerKeydown"
      @close="closeCountPicker()"
    >
      <p id="deck-count-menu-label">{{ countPicker.name }}{{ countPicker.level }} 級張數</p>
      <div id="deck-count-menu" ref="countMenu" class="deck-count-options" role="menu" aria-labelledby="deck-count-menu-label" aria-orientation="horizontal">
        <button
          v-for="count in countOptions"
          :key="count"
          class="deck-count-option"
          type="button"
          role="menuitemradio"
          :aria-checked="countPicker.count === count"
          tabindex="-1"
          @click="chooseDeckCardCount(count)"
        >{{ count }}<span class="sr-only"> 張</span></button>
      </div>
    </AnchoredSurface>

    <ModalShell
      :open="deckImportOpen"
      root-class="deck-modal"
      :root-style="deckImportViewport?.style"
      panel-class="deck-dialog-panel"
      labelledby="deck-import-title"
      describedby="deck-import-help"
      initial-focus-selector="#deck-import-text"
      @close="deckImportOpen = false"
    >
      <form
        class="setup-card deck-dialog deck-import-dialog"
        :class="{ 'deck-import-compact': deckImportViewport && deckImportViewport.height < 320 }"
        @submit.prevent="applyDeckImport"
      >
        <h2 id="deck-import-title">匯入牌組</h2>
        <p id="deck-import-help">依金、木、水、火、土與 1 至 5 級的順序，貼上 25 個數字。</p>
        <div class="deck-import-examples">
          <code aria-label="Tab 與換行格式範例">{{ deckImportTableExample }}</code>
          <code aria-label="連續數字格式範例">{{ deckImportExample }}</code>
        </div>

        <div class="deck-import-field">
          <label for="deck-import-text">牌組張數</label>
          <textarea id="deck-import-text" v-model="deckImportText" rows="3" spellcheck="false" />
        </div>
        <p v-if="deckImportError" class="deck-feedback deck-feedback-error" role="alert">{{ deckImportError }}</p>

        <div class="deck-primary-actions">
          <button class="deck-button" type="button" @click="deckImportOpen = false"><DeckActionIcon name="close" />取消</button>
          <button class="deck-button" data-tone="primary" type="submit"><DeckActionIcon name="check" />套用</button>
        </div>
      </form>
    </ModalShell>

    <ModalShell
      :open="deckIssuesOpen"
      root-class="deck-modal"
      panel-class="deck-dialog-panel"
      labelledby="deck-issues-title"
      initial-focus-selector="#deck-issues-close"
      @close="deckIssuesOpen = false"
    >
      <section class="setup-card deck-dialog">
        <h2 id="deck-issues-title">牌組問題</h2>
        <div class="deck-issue" aria-live="polite" aria-atomic="true">
          <span>{{ deckIssueIndex + 1 }} / {{ deckValidation.errors.length }}</span>
          <p>{{ deckValidation.errors[deckIssueIndex] }}</p>
        </div>
        <div class="deck-issue-actions">
          <button class="deck-button" type="button" :disabled="deckIssueIndex === 0" @click="deckIssueIndex--">
            <DeckActionIcon name="previous" />上一項
          </button>
          <button class="deck-button" type="button" :disabled="deckIssueIndex >= deckValidation.errors.length - 1" @click="deckIssueIndex++">
            <DeckActionIcon name="next" />下一項
          </button>
          <button id="deck-issues-close" class="deck-button" data-tone="primary" type="button" @click="deckIssuesOpen = false">
            <DeckActionIcon name="close" />關閉
          </button>
        </div>
      </section>
    </ModalShell>
  </main>
</template>

<script setup lang="ts">
import { useEventListener } from '@vueuse/core'
import AnchoredSurface from '~/components/AnchoredSurface.vue'
import DeckActionIcon from '~/components/DeckActionIcon.vue'
import { cardElementGlyph } from '~/lib/card-face-presentation'
import {
  createDeckCompositionPolicy,
  parsePlayerDeckCounts,
  serializePlayerDeckCounts,
  type PlayerDeckList,
} from '~/lib/player-deck'
import { presentApiError } from '~/lib/api-error-presentation'
import { useRulesCatalog } from '~/lib/rules-catalog'

const router = useRouter()
const rulesCatalog = useRulesCatalog()
const deckDraft = ref<PlayerDeckList>({ name: '', cards: [] })
const deckSource = ref<'custom' | 'preconstructed'>('preconstructed')
type DeckCell = { id?: string, level: number, count: number, limit: number }
const countPicker = shallowRef<(DeckCell & { id: string, name: string, anchor: HTMLButtonElement }) | null>(null)
const countMenu = ref<HTMLDivElement | null>(null)
const deckRequest = ref<'save' | 'reset' | null>(null)
const deckBusy = computed(() => deckRequest.value !== null)
const deckInitialLoading = ref(true)
const deckInitialError = ref('')
const deckImportExample = '4113141122444334113341113'
const deckImportTableExample = serializePlayerDeckCounts([...deckImportExample].map(Number))
const deckImportOpen = ref(false)
const deckImportText = ref('')
const deckImportError = ref('')
let deckImportOpeningHeight = 0
const deckImportViewport = ref<{ height: number, style: { top: string, bottom: string, height: string } }>()
const deckIssuesOpen = ref(false)
const deckIssueIndex = ref(0)
const deckFeedback = ref<{ message: string, error: boolean } | null>(null)

const deckRules = computed(() => rulesCatalog.catalog.value
  ? createDeckCompositionPolicy(rulesCatalog.catalog.value.deckComposition)
  : null)
const deckRows = computed(() => {
  const rules = deckRules.value
  if (!rules) return []
  const counts = rules.counts(deckDraft.value)
  return rules.elements.map((element, rowIndex) => {
    const cells = rules.levels.map((level, columnIndex): DeckCell => {
      const definition = rules.definition(element, level)
      return {
        id: definition?.id,
        level,
        count: counts[rowIndex * rules.levels.length + columnIndex] ?? 0,
        limit: definition?.personalDeckCopyLimit ?? 0,
      }
    })
    return {
      element,
      name: rules.definitions.find(definition => definition.element === element)?.name ?? cardElementGlyph(element),
      cells,
      cardCount: cells.reduce((total, cell) => total + cell.count, 0),
      levelTotal: cells.reduce((total, cell) => total + cell.count * cell.level, 0),
    }
  })
})
const deckLevelCounts = computed(() => deckRules.value?.levels.map((level, columnIndex) => ({
  level,
  count: deckRows.value.reduce((total, row) => total + (row.cells[columnIndex]?.count ?? 0), 0),
})) ?? [])
const countOptions = computed(() => countPicker.value
  ? Array.from({ length: countPicker.value.limit + 1 }, (_, count) => count)
  : [])
const deckValidation = computed(() => deckRules.value?.validate(deckDraft.value) ?? ({
  valid: false,
  cardCount: deckDraft.value.cards.length,
  levelTotal: 0,
  errors: [],
}))

function toggleCountPicker(event: MouseEvent, name: string, cell: DeckCell) {
  if (!cell.id || deckBusy.value) return
  if (countPicker.value?.id === cell.id) {
    closeCountPicker(true)
    return
  }
  countPicker.value = { ...cell, id: cell.id, name, anchor: event.currentTarget as HTMLButtonElement }
}

function closeCountPicker(restoreFocus = false) {
  const anchor = countPicker.value?.anchor
  countPicker.value = null
  if (restoreFocus) anchor?.focus({ preventScroll: true })
}

function focusCurrentCount() {
  const current = countMenu.value?.querySelector<HTMLButtonElement>('[aria-checked="true"]')
    ?? countMenu.value?.querySelector<HTMLButtonElement>('button')
  current?.focus({ preventScroll: true })
}

function handleCountPickerKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' || event.key === 'Tab') {
    if (event.key === 'Escape') event.preventDefault()
    closeCountPicker(true)
    return
  }

  const options = Array.from(countMenu.value?.querySelectorAll<HTMLButtonElement>('button') ?? [])
  const index = options.findIndex(option => option === document.activeElement)
  let nextIndex: number
  switch (event.key) {
    case 'Home': nextIndex = 0; break
    case 'End': nextIndex = options.length - 1; break
    case 'ArrowRight':
    case 'ArrowDown': nextIndex = (index + 1) % options.length; break
    case 'ArrowLeft':
    case 'ArrowUp': nextIndex = (index + options.length - 1) % options.length; break
    default: return
  }
  event.preventDefault()
  options[nextIndex]?.focus({ preventScroll: true })
}

function chooseDeckCardCount(count: number) {
  if (countPicker.value) setDeckCardCount(countPicker.value, count)
  closeCountPicker(true)
}

function setDeckCardCount(cell: DeckCell, count: number) {
  const { id } = cell
  if (!id || deckBusy.value || count === cell.count) return
  deckFeedback.value = null
  const delta = count - cell.count
  if (delta > 0) {
    deckDraft.value.cards.push(...Array<string>(delta).fill(id))
  } else {
    let remaining = -delta
    deckDraft.value.cards = deckDraft.value.cards.filter(card => {
      if (card !== id || remaining === 0) return true
      remaining--
      return false
    })
  }
}

function openDeckImport(event: MouseEvent) {
  (event.currentTarget as HTMLButtonElement).focus({ preventScroll: true })
  deckImportText.value = ''
  deckImportError.value = ''
  deckImportOpeningHeight = window.innerHeight
  deckImportViewport.value = undefined
  deckImportOpen.value = true
}

function openDeckIssues(event: MouseEvent) {
  (event.currentTarget as HTMLButtonElement).focus({ preventScroll: true })
  deckIssueIndex.value = 0
  deckIssuesOpen.value = true
}

useEventListener(
  () => deckImportOpen.value && typeof window !== 'undefined' ? window.visualViewport : null,
  ['resize', 'scroll'],
  () => {
    const viewport = window.visualViewport
    // 手機鍵盤可能只縮小可視區域，將匯入視窗留在鍵盤上方。
    const availableHeight = Math.max(window.innerHeight, deckImportOpeningHeight)
    deckImportViewport.value = viewport && viewport.scale === 1 && viewport.height < availableHeight - 80
      ? { height: viewport.height, style: { top: `${viewport.offsetTop}px`, bottom: 'auto', height: `${viewport.height}px` } }
      : undefined
  },
)

function applyDeckImport() {
  const imported = parsePlayerDeckCounts(deckImportText.value)
  if (!imported.counts) {
    deckImportError.value = imported.error
    return
  }
  if (!deckRules.value) {
    deckImportError.value = '牌組規則尚未載入，請稍後再試。'
    return
  }

  deckDraft.value = deckRules.value.deckFromCounts(deckDraft.value.name, imported.counts)
  deckFeedback.value = null
  deckImportOpen.value = false
}

async function exportDeck() {
  if (!deckRules.value) {
    deckFeedback.value = { error: true, message: '牌組規則尚未載入，無法匯出。' }
    return
  }

  try {
    await navigator.clipboard.writeText(serializePlayerDeckCounts(deckRules.value.counts(deckDraft.value)))
    deckFeedback.value = { error: false, message: '牌組已複製到剪貼簿。' }
  } catch {
    deckFeedback.value = { error: true, message: '無法寫入剪貼簿，請確認瀏覽器權限後再試。' }
  }
}

async function loadDeck() {
  const response = await $fetch<{ deck: PlayerDeckList, source: 'custom' | 'preconstructed' }>('/api/deck')
  deckDraft.value = { name: response.deck.name, cards: [...response.deck.cards] }
  deckSource.value = response.source
}

async function saveDeck() {
  if (deckBusy.value || !deckValidation.value.valid) return
  deckRequest.value = 'save'
  deckFeedback.value = null
  try {
    const body = { name: deckDraft.value.name, cards: [...deckDraft.value.cards] }
    const response = await $fetch<{ deck: PlayerDeckList }>('/api/deck', { method: 'PUT', body })
    deckDraft.value = { name: response.deck.name, cards: [...response.deck.cards] }
    deckSource.value = 'custom'
    deckFeedback.value = { error: false, message: '牌組已儲存。' }
  } catch (error) {
    deckFeedback.value = { error: true, message: presentApiError(error, '無法儲存牌組') }
  } finally {
    deckRequest.value = null
  }
}

async function resetDeck() {
  if (deckBusy.value) return
  deckRequest.value = 'reset'
  deckFeedback.value = null
  try {
    const response = await $fetch<{ deck: PlayerDeckList }>('/api/deck', { method: 'DELETE' })
    deckDraft.value = { name: response.deck.name, cards: [...response.deck.cards] }
    deckSource.value = 'preconstructed'
    deckFeedback.value = { error: false, message: '已重設為內建預組。' }
  } catch (error) {
    deckFeedback.value = { error: true, message: presentApiError(error, '無法重設牌組') }
  } finally {
    deckRequest.value = null
  }
}

function returnToLobby() {
  void router.push('/rooms')
}

async function loadDeckPage() {
  deckInitialLoading.value = true
  deckInitialError.value = ''
  const [catalogResult, deckResult] = await Promise.allSettled([rulesCatalog.load(), loadDeck()])
  if (catalogResult.status === 'rejected') {
    deckInitialError.value = '無法載入牌組規則'
  } else if (deckResult.status === 'rejected') {
    deckInitialError.value = presentApiError(deckResult.reason, '無法載入牌組')
  }
  deckInitialLoading.value = false
}

onMounted(() => {
  void loadDeckPage()
})
</script>

<style>
.deck-page {
  --deck-gap: 14px;
  --deck-button-height: 44px;
  --deck-count-height: 42px;
  --deck-cell-padding: 3px;
  width: min(1056px, 100%);
  padding: 22px 24px 24px;
}
.app-shell:has(> .deck-page, > .deck-route-state) { min-height: 100dvh; }
.deck-route-state { flex: 1; min-height: 0; }
.deck-editor-heading {
  display: grid;
  grid-template-columns: minmax(0, 1fr) minmax(0, 360px);
  align-items: center;
  gap: 8px 16px;
}
.deck-page-title {
  font: 900 clamp(24px, 3.2vw, 36px) / 1.25 var(--font-serif);
}
.deck-editor {
  display: grid;
  gap: var(--deck-gap);
  padding: clamp(14px, 2.3vw, 28px);
}
.deck-overview, .deck-actions { display: grid; min-width: 0; gap: var(--deck-gap); }
.deck-actions { grid-template-columns: minmax(0, 3fr) minmax(0, 2fr); }
.deck-name-field, .deck-import-field { display: grid; align-items: center; gap: 6px 10px; }
.deck-name-field { grid-template-columns: auto minmax(0, 1fr); }
.deck-name-field label, .deck-import-field label { font-size: 13px; font-weight: 700; }
.deck-name-field input { min-height: var(--deck-button-height); padding-inline: 10px; }

.deck-grid {
  width: 100%;
  border: 1px solid var(--ui-glass-stroke);
  border-radius: 12px;
  border-collapse: separate;
  border-spacing: 0;
  table-layout: fixed;
  text-align: center;
  font-variant-numeric: tabular-nums;
}
.deck-grid th, .deck-grid td { padding: var(--deck-cell-padding); }
.deck-grid th { font-size: 13px; font-weight: 700; }
.deck-grid thead th { padding-block: 6px; border-bottom: 1px solid var(--ui-glass-stroke); line-height: 1.2; }
.deck-grid tbody tr:nth-child(even) > :is(th, td) { background: color-mix(in srgb, var(--app-surface) 26%, transparent); }
.deck-element-column { width: 10%; }
.deck-card-total-column { width: 11%; }
.deck-level-total-column { width: 14%; }
.deck-grid .deck-subtotal-heading { color: var(--app-accent-strong); }
.deck-element span {
  display: grid;
  min-height: var(--deck-count-height);
  place-items: center;
  border-radius: 8px;
  color: var(--deck-element-ink);
  background: color-mix(in srgb, var(--deck-element-ink) 13%, var(--app-surface));
  font-weight: 800;
}
.deck-page [data-element="Metal"] { --deck-element-ink: light-dark(#795718, #e4c584); }
.deck-page [data-element="Wood"] { --deck-element-ink: light-dark(#31694c, #88c59d); }
.deck-page [data-element="Water"] { --deck-element-ink: light-dark(#2a6185, #9cc9eb); }
.deck-page [data-element="Fire"] { --deck-element-ink: light-dark(#993f33, #e7a096); }
.deck-page [data-element="Earth"] { --deck-element-ink: light-dark(#775342, #d7b19b); }
.deck-count-button, .deck-count-step {
  min-height: var(--deck-count-height);
  border: 1px solid var(--app-border);
  border-radius: 8px;
  font-weight: 800;
}
.deck-count-button { display: block; width: 100%; }
.deck-count-step { display: none; min-width: 0; padding: 0; }
.deck-count-step .deck-icon { width: .9em; height: .9em; }
.deck-count-button[aria-expanded="true"] { border-color: var(--app-accent); }
.deck-count-button.deck-count-over-limit { --deck-action-color: var(--app-danger); border-color: var(--app-danger); }
.deck-subtotal span, .deck-column-total, .deck-level-count-label, .deck-grand-total {
  display: flex;
  min-height: 28px;
  align-items: center;
  justify-content: center;
  border-radius: 7px;
  font-weight: 750;
}
.deck-subtotal span { background: color-mix(in srgb, var(--app-accent) 12%, var(--app-surface)); }
.deck-grid tfoot :is(th, td) { border-top: 1px solid var(--ui-glass-stroke); }
.deck-column-total, .deck-level-count-label { background: var(--app-surface-muted); }
.deck-grand-total {
  gap: 3px;
  white-space: nowrap;
  color: var(--app-success);
  background: var(--app-success-surface);
}
.deck-grand-total.invalid { color: var(--app-danger); background: var(--app-danger-surface); }
.deck-grand-total small { font-size: .8em; font-weight: 600; }
.deck-count-picker {
  padding: 8px;
  border: 1px solid var(--app-border);
  border-radius: 12px;
  background: var(--app-surface-raised);
  box-shadow: var(--ui-glass-shadow);
  color: var(--app-text);
}
.deck-count-picker > p { margin: 0 0 6px 2px; font-size: 13px; font-weight: 700; }
.deck-count-options { display: flex; flex-wrap: wrap; gap: 5px; }
.deck-count-option {
  display: grid;
  width: 44px;
  min-height: 44px;
  place-items: center;
  border: 1px solid var(--app-border);
  border-radius: 8px;
  font-weight: 800;
}
.deck-count-option[aria-checked="true"] { --deck-action-color: var(--app-accent-strong); border-color: var(--app-accent); background: var(--app-accent-soft); }
.deck-summary { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; min-height: 24px; gap: 2px 8px; font-size: 12px; }
.deck-issues-button { display: inline-flex; min-height: 28px; align-items: center; gap: 4px; padding-inline: 6px; border-radius: 6px; font-weight: 700; }
.deck-feedback { display: flex; align-items: baseline; gap: 5px; color: var(--app-success); font-size: 13px; overflow-wrap: anywhere; }
.deck-feedback .deck-icon { align-self: center; }
.deck-feedback-error { color: var(--app-danger); }

.deck-management-actions, .deck-primary-actions, .deck-issue-actions { display: grid; gap: 6px; }
.deck-management-actions, .deck-issue-actions { grid-template-columns: repeat(3, minmax(0, 1fr)); }
.deck-management-actions .deck-button { gap: 4px; padding-inline: 3px; font-size: 12px; white-space: nowrap; }
.deck-primary-actions { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.deck-button {
  display: inline-flex;
  min-height: var(--deck-button-height, 44px);
  align-items: center;
  justify-content: center;
  gap: 5px;
  padding-inline: 4px;
  border: 1px solid color-mix(in srgb, var(--deck-action-color, var(--app-text)) 28%, var(--app-border));
  border-radius: 9px;
  font-size: 13px;
  font-weight: 700;
  line-height: 1.25;
}
:is(.deck-button, .deck-count-button, .deck-count-step, .deck-count-option, .deck-issues-button) {
  color: var(--deck-action-color, var(--app-text));
  background: color-mix(in srgb, var(--deck-action-color, var(--app-text)) 10%, var(--app-surface));
}
:is(.deck-button, .deck-count-button, .deck-count-step, .deck-count-option, .deck-issues-button):hover:not(:disabled) {
  background: color-mix(in srgb, var(--deck-action-color, var(--app-text)) 19%, var(--app-surface));
}
:is(.deck-button, .deck-count-button, .deck-count-step, .deck-count-option, .deck-issues-button):focus-visible {
  position: relative;
  z-index: 1;
  outline: 2px solid var(--deck-action-color, var(--app-accent-strong));
  outline-offset: 2px;
}
.deck-button[data-tone="primary"] { border-color: #cfb477; color: var(--ui-on-gold); background: var(--ui-gold-fill); }
.deck-button[data-tone="primary"]:hover:not(:disabled) { background: var(--ui-gold-hover); }
.deck-button[data-tone="import"] { --deck-action-color: light-dark(#285e87, #a6d1f0); }
.deck-button[data-tone="export"] { --deck-action-color: light-dark(#225f60, #a1d9ce); }
:is(.deck-button, .deck-issues-button, .deck-count-step)[data-tone="remove"] { --deck-action-color: var(--app-danger); }
.deck-count-step[data-tone="add"] { --deck-action-color: var(--app-success); }

@media (min-width: 900px) and (pointer: fine) {
  .deck-count-control { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr) minmax(0, 1fr); gap: 2px; }
  .deck-count-step { display: grid; place-items: center; }
}

.modal-shell.deck-modal { padding: clamp(6px, 2vw, 20px); }
.modal-shell.deck-modal:has(.deck-import-compact) { padding: 4px; }
.modal-shell > .modal-shell-panel.deck-dialog-panel { width: min(440px, 100%); max-height: 100%; }
.deck-dialog { display: grid; gap: 8px; padding: clamp(10px, 3vw, 22px); border-radius: 14px; }
.deck-dialog h2 { font: 700 20px / 1.25 var(--font-serif); }
.deck-dialog > p, .deck-issue { font-size: 13px; }
.deck-import-examples { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1.2fr); align-items: start; gap: 6px; }
.deck-import-examples code { padding: 5px 6px; border: 1px solid var(--app-border); border-radius: 7px; background: var(--app-surface-muted); font-size: 12px; line-height: 1.25; white-space: pre-wrap; overflow-wrap: anywhere; tab-size: 2; }
.deck-import-field { align-self: start; }
.deck-import-field textarea {
  min-height: 46px;
  max-height: 100px;
  width: 100%;
  padding: 7px 9px;
  border: 1px solid var(--app-border-strong);
  border-radius: 8px;
  background: var(--app-input);
  font: 16px / 1.25 ui-monospace, monospace;
  resize: none;
}
.deck-import-field textarea:focus-visible { outline: 2px solid var(--app-accent-strong); outline-offset: 1px; }
.deck-import-compact { gap: 4px; padding: 5px 10px; }
.deck-import-compact h2 { font-size: 17px; }
.deck-import-compact > p { font-size: 12px; line-height: 1.25; }
.deck-import-compact .deck-import-examples { display: none; }
.deck-import-compact .deck-import-field label { position: absolute; width: 1px; height: 1px; clip-path: inset(50%); overflow: hidden; }
.deck-import-compact .deck-import-field textarea { height: 32px; min-height: 32px; padding-block: 4px; }
.deck-import-compact .deck-button { --deck-button-height: 34px; }
.deck-issue { padding: 8px; border-radius: 8px; background: var(--app-danger-surface); color: var(--app-danger); }
.deck-issue span { font-size: 12px; font-variant-numeric: tabular-nums; }
.deck-issue p { overflow-wrap: anywhere; }

@media (max-width: 749px) {
  .deck-element-column { width: 40px; }
  .deck-card-total-column { width: 38px; }
  .deck-level-total-column { width: 54px; }
  .deck-grid th { font-size: 12px; }
  .deck-level-count-label { flex-direction: column; line-height: 1.05; }
}
@media (max-width: 649px) {
  .deck-page { --deck-gap: 8px; --deck-cell-padding: 1px; padding: 10px 10px 12px; }
  .deck-editor { padding: 10px; border-radius: 16px; }
  .deck-editor-heading, .deck-actions { grid-template-columns: minmax(0, 1fr); }
  .deck-grid { font-size: 13px; }
  .deck-count-button, .deck-element span { border-radius: 6px; }
}
@media (max-width: 900px) {
  .deck-grand-total { flex-direction: column; gap: 0; font-size: 13px; line-height: 1; }
}
@media (max-height: 720px) {
  .app-shell:has(> .deck-page) .site-header { min-height: 50px; padding-block: 4px; }
  .app-shell:has(> .deck-page) .brand-banner { max-height: 40px; width: auto; object-fit: contain; }
  .deck-page { --deck-gap: 6px; --deck-button-height: 36px; --deck-count-height: 32px; padding-block: 6px; }
  .deck-page-title { font-size: 25px; }
  .deck-editor { padding-block: 8px; }
}
@media (max-height: 600px) {
  .app-shell:has(> .deck-page) .profile-avatar { width: 28px; height: 28px; }
  .deck-page { --deck-gap: 5px; --deck-button-height: 34px; --deck-count-height: 30px; }
  .deck-editor { padding-block: 6px; }
}
@media (min-width: 540px) and (max-height: 500px) {
  .deck-editor { grid-template-columns: minmax(0, 1fr) clamp(174px, 32%, 286px); column-gap: 12px; align-items: start; }
  .deck-editor-heading { grid-column: 1 / -1; grid-template-columns: minmax(0, 1fr) minmax(0, 300px); }
  .deck-overview { padding-inline-start: 8px; border-inline-start: 1px solid var(--ui-glass-stroke); }
  .deck-actions { grid-template-columns: minmax(0, 1fr); }
}
@media (min-width: 540px) and (max-width: 749px) and (max-height: 500px) {
  .deck-management-actions { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .deck-management-actions .deck-button:last-child { grid-column: 1 / -1; }
  .deck-summary { justify-content: start; }
}
@media (max-height: 540px) {
  .app-shell:has(> .deck-page) .site-header { min-height: 44px; padding-block: 2px; }
  .app-shell:has(> .deck-page) .brand-banner { max-height: 34px; }
  .deck-page { --deck-gap: 4px; --deck-button-height: 30px; --deck-count-height: 25px; --deck-cell-padding: 1px; padding-block: 4px; }
  .deck-page-title { font-size: 20px; }
  .deck-editor { padding-block: 5px; }
  .deck-editor-heading { grid-template-columns: auto minmax(0, 260px); justify-content: space-between; gap: 6px; }
  .deck-name-field { gap: 4px 6px; }
  .deck-name-field label { font-size: 11px; }
  .deck-grid thead th { padding-block: 4px; }
  .deck-subtotal span, .deck-column-total, .deck-level-count-label, .deck-grand-total { min-height: 24px; }
  .deck-issues-button { min-height: 24px; }
  .deck-dialog { gap: 4px; }
  .deck-dialog .deck-button { --deck-button-height: 34px; }
}
@media (min-width: 540px) and (max-height: 360px) {
  .deck-page { --deck-count-height: 24px; padding-block: 2px; }
  .deck-editor { padding-block: 4px; }
  .deck-name-field input { min-height: 26px; }
  .deck-grid thead th { padding-block: 2px; font-size: 11px; }
  .modal-shell > .modal-shell-panel.deck-dialog-panel:has(.deck-import-dialog) { width: min(520px, 100%); }
  .deck-import-dialog:not(.deck-import-compact) { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
  .deck-import-dialog > :is(h2, p, .deck-primary-actions) { grid-column: 1 / -1; }
}
</style>
