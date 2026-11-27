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
    <h1 class="deck-page-title">個人牌組</h1>

    <section class="setup-card deck-editor" aria-label="調整牌組">
      <div class="deck-edit-controls">
        <div class="deck-name-field">
          <label for="deck-name">牌組名稱</label>
          <input id="deck-name" v-model.trim="deckDraft.name" maxlength="24" :disabled="deckBusy" @input="deckFeedback = null">
        </div>

        <fieldset class="deck-element-picker">
          <legend class="sr-only">選擇卡牌屬性</legend>
          <label v-for="element in deckElements" :key="element" class="deck-element-option" :data-element="element">
            <input v-model="activeElement" class="sr-only" type="radio" name="deck-element" :value="element">
            <span>{{ deckElementLabel(element) }}</span>
          </label>
        </fieldset>

        <table class="deck-grid" aria-label="個人牌組卡牌張數">
          <thead>
            <tr>
              <th scope="col"><span class="sr-only">屬性</span></th>
              <th v-for="level in deckLevels" :key="level" scope="col">{{ level }} 級</th>
            </tr>
          </thead>
          <tbody>
            <tr v-for="element in deckElements" :key="element" :class="{ 'deck-active-element': activeElement === element }">
              <th scope="row" class="deck-element" :data-element="element"><span>{{ deckElementLabel(element) }}</span></th>
              <td v-for="level in deckLevels" :key="level">
                <div class="deck-count-control" :class="{ 'deck-count-over-limit': deckCardCount(element, level) > deckCardLimit(element, level) }">
                  <span class="deck-mobile-level">{{ level }} 級</span>
                  <button
                    class="deck-count-button"
                    data-tone="remove"
                    type="button"
                    :aria-label="`減少${deckElementLabel(element)}${level}級`"
                    :disabled="deckBusy || deckCardCount(element, level) === 0"
                    @click="adjustDeckCard(element, level, -1)"
                  ><DeckActionIcon name="remove" /></button>
                  <span class="deck-count-value" :aria-label="`${deckElementLabel(element)}${level}級張數`">{{ deckCardCount(element, level) }}</span>
                  <button
                    class="deck-count-button"
                    data-tone="add"
                    type="button"
                    :aria-label="`增加${deckElementLabel(element)}${level}級`"
                    :disabled="deckBusy || deckCardCount(element, level) >= deckCardLimit(element, level)"
                    @click="adjustDeckCard(element, level, 1)"
                  ><DeckActionIcon name="add" /></button>
                </div>
              </td>
            </tr>
          </tbody>
        </table>
      </div>

      <div class="deck-overview">
        <div class="deck-validation" :class="{ invalid: !deckValidation.valid }">
          <div class="deck-totals" aria-live="polite">
            <strong>{{ deckValidation.cardCount }} / {{ deckRules?.exactCardCount ?? '—' }} 張</strong>
            <strong>{{ deckValidation.levelTotal }} / {{ deckRules?.maximumLevelTotal ?? '—' }} 級</strong>
          </div>
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
import DeckActionIcon from '~/components/DeckActionIcon.vue'
import type { Element } from '~/types/fewfc'
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
const activeElement = ref<Element>('Metal')
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
const deckElements = computed(() => deckRules.value?.elements ?? [])
const deckLevels = computed(() => deckRules.value?.levels ?? [])
const deckValidation = computed(() => deckRules.value?.validate(deckDraft.value) ?? ({
  valid: false,
  cardCount: deckDraft.value.cards.length,
  levelTotal: 0,
  errors: [],
}))

function deckElementLabel(element: Element) {
  return deckRules.value?.definitions.find(definition => definition.element === element)?.name ?? element
}

function deckCardCount(element: Element, level: number) {
  const id = deckRules.value?.definition(element, level)?.id
  return id ? deckDraft.value.cards.filter(card => card === id).length : 0
}

function deckCardLimit(element: Element, level: number) {
  return deckRules.value?.definition(element, level)?.personalDeckCopyLimit ?? 0
}

function adjustDeckCard(element: Element, level: number, delta: number) {
  const id = deckRules.value?.definition(element, level)?.id
  if (!id) return
  deckFeedback.value = null
  if (delta > 0) {
    deckDraft.value.cards.push(id)
    return
  }
  const index = deckDraft.value.cards.indexOf(id)
  if (index >= 0) deckDraft.value.cards.splice(index, 1)
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
  --deck-count-height: 36px;
  width: min(1120px, 100%);
  padding: 22px 24px 24px;
}
.app-shell:has(> .deck-page, > .deck-route-state) { min-height: 100dvh; }
.deck-route-state { flex: 1; min-height: 0; }
.deck-page-title {
  margin-bottom: 12px;
  color: #f4f0e2;
  font: 900 clamp(24px, 3.2vw, 40px) / 1.2 var(--font-serif);
  text-shadow: 0 2px 16px rgba(4, 17, 13, .6);
}
.deck-editor {
  display: grid;
  grid-template-columns: minmax(0, 1fr) clamp(310px, 33%, 342px);
  align-items: start;
  gap: clamp(16px, 2.5vw, 32px);
  padding: clamp(14px, 2.3vw, 28px);
}
.deck-edit-controls, .deck-overview, .deck-actions { display: grid; min-width: 0; gap: var(--deck-gap); }
.deck-overview { align-self: stretch; align-content: start; padding-inline-start: clamp(16px, 2.5vw, 28px); border-inline-start: 1px solid var(--ui-glass-stroke); }
.deck-name-field, .deck-import-field { display: grid; align-items: center; gap: 6px 10px; }
.deck-name-field { grid-template-columns: auto minmax(0, 1fr); }
.deck-name-field label, .deck-import-field label { font-size: 13px; font-weight: 700; }
.deck-name-field input { min-height: var(--deck-button-height); padding-inline: 10px; }

.deck-grid { width: calc(100% + 12px); margin: -6px; border-collapse: separate; border-spacing: 6px; table-layout: fixed; }
.deck-grid th { font-size: 13px; text-align: center; }
.deck-grid thead th:first-child { width: 32px; }
.deck-grid th, .deck-grid td { padding: 0; }
.deck-element span, .deck-element-option span {
  border-radius: 8px;
  color: var(--deck-element-ink);
  background: color-mix(in srgb, var(--deck-element-ink) 13%, var(--app-surface));
  font-weight: 800;
}
.deck-element span { display: grid; min-height: var(--deck-count-height); place-items: center; }
.deck-page [data-element="Metal"] { --deck-element-ink: light-dark(#795718, #e4c584); }
.deck-page [data-element="Wood"] { --deck-element-ink: light-dark(#31694c, #88c59d); }
.deck-page [data-element="Water"] { --deck-element-ink: light-dark(#2a6185, #9cc9eb); }
.deck-page [data-element="Fire"] { --deck-element-ink: light-dark(#993f33, #e7a096); }
.deck-page [data-element="Earth"] { --deck-element-ink: light-dark(#775342, #d7b19b); }
.deck-element-picker, .deck-mobile-level { display: none; }
.deck-count-control {
  display: grid;
  grid-template-columns: minmax(24px, 1fr) auto minmax(24px, 1fr);
  align-items: center;
  gap: 2px;
  min-width: 0;
  padding: 3px;
  border: 1px solid var(--ui-glass-stroke);
  border-radius: 9px;
  background: var(--app-surface);
}
.deck-count-value { min-width: 1.15em; text-align: center; font-weight: 800; font-variant-numeric: tabular-nums; }
.deck-count-over-limit { border-color: var(--app-danger); }
.deck-count-over-limit .deck-count-value { color: var(--app-danger); }
.deck-count-button { display: grid; min-height: var(--deck-count-height); place-items: center; border-radius: 6px; }

.deck-validation {
  min-width: 0;
  padding: 10px;
  border: 1px solid color-mix(in srgb, var(--app-success) 50%, var(--app-border));
  border-radius: 12px;
  color: var(--app-success);
  background: var(--app-success-surface);
}
.deck-validation.invalid {
  border-color: color-mix(in srgb, var(--app-danger) 55%, var(--app-border));
  color: var(--app-danger);
  background: var(--app-danger-surface);
}
.deck-totals, .deck-summary { display: flex; flex-wrap: wrap; align-items: center; justify-content: space-between; gap: 2px 8px; }
.deck-totals { font-size: 16px; font-variant-numeric: tabular-nums; }
.deck-summary { margin-top: 6px; font-size: 12px; color: var(--app-text); }
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
:is(.deck-button, .deck-count-button, .deck-issues-button) {
  color: var(--deck-action-color, var(--app-text));
  background: color-mix(in srgb, var(--deck-action-color, var(--app-text)) 10%, var(--app-surface));
}
:is(.deck-button, .deck-count-button, .deck-issues-button):hover:not(:disabled) {
  background: color-mix(in srgb, var(--deck-action-color, var(--app-text)) 19%, var(--app-surface));
}
:is(.deck-button, .deck-count-button, .deck-issues-button):focus-visible {
  position: relative;
  z-index: 1;
  outline: 2px solid var(--deck-action-color, var(--app-accent-strong));
  outline-offset: 2px;
}
.deck-button[data-tone="primary"] { border-color: #cfb477; color: var(--ui-on-gold); background: var(--ui-gold-fill); }
.deck-button[data-tone="primary"]:hover:not(:disabled) { background: var(--ui-gold-hover); }
.deck-button[data-tone="import"] { --deck-action-color: light-dark(#285e87, #a6d1f0); }
.deck-button[data-tone="export"] { --deck-action-color: light-dark(#225f60, #a1d9ce); }
.deck-count-button[data-tone="add"] { --deck-action-color: var(--app-success); }
:is(.deck-button, .deck-count-button, .deck-issues-button)[data-tone="remove"] { --deck-action-color: var(--app-danger); }

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

@media (max-width: 899px) {
  .deck-element-picker { display: grid; grid-template-columns: repeat(5, minmax(0, 1fr)); gap: 5px; }
  .deck-element-option { position: relative; cursor: pointer; }
  .deck-element-option span {
    display: grid;
    min-height: var(--deck-button-height);
    place-items: center;
    border: 1px solid var(--ui-glass-stroke);
  }
  .deck-element-option input:checked + span { border-color: var(--deck-element-ink); box-shadow: inset 0 0 0 1px var(--deck-element-ink); }
  .deck-element-option input:focus-visible + span { outline: 2px solid var(--app-accent-strong); outline-offset: 2px; }
  .deck-grid, .deck-grid tbody { display: block; width: 100%; margin: 0; }
  .deck-grid thead, .deck-grid tbody th, .deck-grid tbody tr:not(.deck-active-element) { display: none; }
  .deck-grid tbody tr { display: grid; gap: 4px; }
  .deck-grid td { display: block; }
  .deck-count-control { grid-template-columns: 1fr repeat(3, minmax(34px, 15%)); padding: 1px 3px; }
  .deck-mobile-level { display: block; padding-inline-start: 7px; font-size: 13px; font-weight: 700; }
}
@media (max-width: 599px) {
  .deck-page { --deck-gap: 8px; --deck-count-height: 42px; padding: 10px 10px 12px; }
  .deck-page-title { margin-bottom: 8px; }
  .deck-editor { grid-template-columns: minmax(0, 1fr); gap: 10px; padding: 10px; border-radius: 16px; }
  .deck-overview { padding-inline-start: 0; border-inline-start: 0; }
  .deck-totals { justify-content: start; gap: 14px; }
  .deck-summary { margin-top: 0; min-height: 24px; }
  .deck-validation { padding: 4px 8px; }
}
@media (max-height: 720px) {
  .app-shell:has(> .deck-page) .site-header { min-height: 50px; padding-block: 4px; }
  .app-shell:has(> .deck-page) .brand-banner { max-height: 40px; width: auto; object-fit: contain; }
  .deck-page { --deck-gap: 6px; --deck-button-height: 36px; --deck-count-height: 32px; padding-block: 6px; }
  .deck-page-title { font-size: 25px; margin-bottom: 6px; }
  .deck-editor { gap: 8px 14px; padding-block: 8px; }
}
@media (max-height: 600px) {
  .app-shell:has(> .deck-page) .profile-avatar { width: 28px; height: 28px; }
  .deck-page { --deck-gap: 5px; --deck-button-height: 34px; --deck-count-height: 30px; }
  .deck-editor { padding-block: 6px; }
}
@media (min-width: 540px) and (max-height: 570px) {
  .deck-editor { grid-template-columns: minmax(0, 1fr) minmax(245px, 42%); }
  .deck-overview { padding-inline-start: 10px; border-inline-start: 1px solid var(--ui-glass-stroke); }
  .deck-validation { padding: 4px 8px; }
  .deck-summary { margin-top: 0; }
}
@media (min-width: 540px) and (max-width: 719px) and (max-height: 570px) {
  .deck-management-actions { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .deck-management-actions .deck-button:last-child { grid-column: 1 / -1; }
}
@media (max-width: 539px) and (max-height: 555px), (max-width: 899px) and (max-height: 335px) {
  .deck-grid tbody tr { grid-template-columns: repeat(2, minmax(0, 1fr)); }
  .deck-count-control { grid-template-columns: minmax(0, 1fr) repeat(3, minmax(25px, 19%)); }
  .deck-mobile-level { padding-inline-start: 2px; font-size: 12px; }
}
@media (max-height: 475px) {
  .app-shell:has(> .deck-page) .site-header { min-height: 44px; padding-block: 2px; }
  .app-shell:has(> .deck-page) .brand-banner { max-height: 34px; }
  .deck-page { --deck-gap: 4px; --deck-button-height: 30px; --deck-count-height: 26px; padding-block: 4px; }
  .deck-page-title { font-size: 20px; margin-bottom: 4px; }
  .deck-editor { padding-block: 5px; }
  .deck-grid tbody tr { gap: 2px; }
  .deck-issues-button { min-height: 24px; }
  .deck-dialog { gap: 4px; }
  .deck-dialog .deck-button { --deck-button-height: 34px; }
}
@media (min-width: 540px) and (max-height: 360px) {
  .modal-shell > .modal-shell-panel.deck-dialog-panel:has(.deck-import-dialog) { width: min(520px, 100%); }
  .deck-import-dialog:not(.deck-import-compact) { grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); }
  .deck-import-dialog > :is(h2, p, .deck-primary-actions) { grid-column: 1 / -1; }
}
</style>
