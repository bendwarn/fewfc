<template>
  <main v-if="deckInitialLoading" class="route-state" aria-live="polite">
    <div class="route-state-content"><span class="route-spinner" aria-hidden="true" /><h1>正在載入</h1><p>正在取得個人牌組與規則。</p></div>
  </main>
  <main v-else-if="deckInitialError" class="route-state">
    <div class="route-state-content"><h1>{{ deckInitialError }}</h1><button class="primary-button" type="button" @click="loadDeckPage">重試</button></div>
  </main>
  <main v-else class="deck-page">
      <section class="setup-card deck-editor">
        <div class="card-heading">
          <span class="step-number">牌</span>
          <div>
            <h1>個人牌組</h1>
            <p>每種牌依屬性與等級調整張數；總數 {{ deckRules?.exactCardCount ?? '—' }}、等級總和最多 {{ deckRules?.maximumLevelTotal ?? '—' }}。</p>
          </div>
        </div>

        <label for="deck-name">牌組名稱</label>
        <input id="deck-name" v-model.trim="deckDraft.name" class="text-input" maxlength="24">

        <div class="deck-grid" role="table" aria-label="個人牌組卡牌張數">
          <div />
          <strong v-for="level in DECK_LEVELS" :key="`level-${level}`">{{ level }} 級</strong>
          <template v-for="element in DECK_ELEMENTS" :key="element">
            <strong>{{ deckElementLabel(element) }}</strong>
            <div v-for="level in DECK_LEVELS" :key="`${element}-${level}`" class="deck-count-control">
              <button
                type="button"
                :aria-label="`減少${deckElementLabel(element)}${level}級`"
                :disabled="deckCardCount(element, level) === 0"
                @click="adjustDeckCard(element, level, -1)"
              >−</button>
              <span :aria-label="`${deckElementLabel(element)}${level}級張數`">{{ deckCardCount(element, level) }}</span>
              <button
                type="button"
                :aria-label="`增加${deckElementLabel(element)}${level}級`"
                :disabled="deckCardCount(element, level) >= deckCardLimit(element, level)"
                @click="adjustDeckCard(element, level, 1)"
              >＋</button>
            </div>
          </template>
        </div>

        <div class="deck-validation" :class="{ invalid: !deckValidation.valid }">
          <strong>{{ deckValidation.cardCount }} / {{ deckRules?.exactCardCount ?? '—' }} 張</strong>
          <strong>{{ deckValidation.levelTotal }} / {{ deckRules?.maximumLevelTotal ?? '—' }} 級</strong>
          <span>{{ deckSource === 'custom' ? '目前使用自訂牌組' : '目前使用內建預組' }}</span>
        </div>
        <ul v-if="deckValidation.errors.length" class="deck-validation-errors" aria-live="polite">
          <li v-for="error in deckValidation.errors" :key="error">{{ error }}</li>
        </ul>
        <p v-if="deckError" class="form-error">{{ deckError }}</p>
        <p
          v-if="deckExportStatus"
          class="deck-transfer-status"
          :class="{ 'form-error': deckExportFailed }"
          :role="deckExportFailed ? 'alert' : 'status'"
        >{{ deckExportStatus }}</p>

        <div class="deck-management-actions">
          <button
            ref="deckImportTrigger"
            class="secondary-button"
            type="button"
            :disabled="deckBusy"
            @click="openDeckImport"
          >
            匯入牌組
          </button>
          <button class="secondary-button" type="button" :disabled="deckBusy" @click="exportDeck">
            匯出牌組
          </button>
          <button class="secondary-button" type="button" :disabled="deckBusy" @click="resetDeck">
            重設為預組
          </button>
        </div>
        <div class="setup-actions">
          <button class="secondary-button" type="button" :disabled="deckBusy" @click="returnToLobby">
            返回房間
          </button>
          <button
            class="primary-button"
            type="button"
            :disabled="deckBusy || !deckValidation.valid"
            @click="saveDeck"
          >
            {{ deckBusy ? '儲存中…' : '儲存牌組' }}
          </button>
        </div>

        <div
          v-if="deckImportOpen"
          class="room-settings-layer"
          @click.self="closeDeckImport"
        >
          <form
            class="setup-card deck-import-dialog"
            role="dialog"
            aria-modal="true"
            aria-labelledby="deck-import-title"
            aria-describedby="deck-import-help"
            @keydown.esc.prevent="closeDeckImport"
            @keydown.tab="trapDeckImportFocus"
            @submit.prevent="applyDeckImport"
          >
            <div class="card-heading">
              <span class="step-number">入</span>
              <div>
                <h2 id="deck-import-title">匯入牌組</h2>
                <div id="deck-import-help" class="deck-import-examples">
                  <p>可貼上以下任一格式：</p>
                  <code aria-label="Tab 與換行格式範例">4	1	1	3	1
4	1	1	2	2
4	4	4	3	3
4	1	1	3	3
4	1	1	1	3</code>
                  <code aria-label="連續數字格式範例">4113141122444334113341113</code>
                </div>
              </div>
            </div>

            <label for="deck-import-text">牌組張數</label>
            <textarea
              id="deck-import-text"
              ref="deckImportInput"
              v-model="deckImportText"
              class="deck-import-text"
              rows="5"
              spellcheck="false"
            />
            <p v-if="deckImportError" class="form-error" role="alert">{{ deckImportError }}</p>

            <div class="setup-actions">
              <button class="secondary-button" type="button" @click="closeDeckImport">取消</button>
              <button class="primary-button" type="submit">套用</button>
            </div>
          </form>
        </div>
      </section>
  </main>
</template>

<script setup lang="ts">
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
const deckBusy = ref(false)
const deckError = ref('')
const deckInitialLoading = ref(true)
const deckInitialError = ref('')
const deckImportOpen = ref(false)
const deckImportText = ref('')
const deckImportError = ref('')
const deckImportTrigger = ref<HTMLButtonElement | null>(null)
const deckImportInput = ref<HTMLTextAreaElement | null>(null)
const deckExportStatus = ref('')
const deckExportFailed = ref(false)

const deckRules = computed(() => rulesCatalog.catalog.value
  ? createDeckCompositionPolicy(rulesCatalog.catalog.value.deckComposition)
  : null)
const DECK_ELEMENTS = computed(() => deckRules.value?.elements ?? [])
const DECK_LEVELS = computed(() => deckRules.value?.levels ?? [])
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
  if (delta > 0) {
    deckDraft.value.cards.push(id)
    return
  }
  const index = deckDraft.value.cards.indexOf(id)
  if (index >= 0) deckDraft.value.cards.splice(index, 1)
}

async function openDeckImport() {
  deckImportText.value = ''
  deckImportError.value = ''
  deckImportOpen.value = true
  await nextTick()
  deckImportInput.value?.focus()
}

function closeDeckImport() {
  deckImportOpen.value = false
  deckImportError.value = ''
  void nextTick(() => deckImportTrigger.value?.focus())
}

function trapDeckImportFocus(event: KeyboardEvent) {
  const dialog = event.currentTarget as HTMLElement
  const focusable = [...dialog.querySelectorAll<HTMLElement>(
    'button:not(:disabled), textarea:not(:disabled)',
  )]
  const first = focusable[0]
  const last = focusable.at(-1)
  if (!first || !last) return

  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

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
  deckExportStatus.value = ''
  deckExportFailed.value = false
  closeDeckImport()
}

async function exportDeck() {
  if (!deckRules.value) {
    deckExportFailed.value = true
    deckExportStatus.value = '牌組規則尚未載入，無法匯出。'
    return
  }

  try {
    await navigator.clipboard.writeText(serializePlayerDeckCounts(deckRules.value.counts(deckDraft.value)))
    deckExportFailed.value = false
    deckExportStatus.value = '牌組已複製到剪貼簿。'
  } catch {
    deckExportFailed.value = true
    deckExportStatus.value = '無法寫入剪貼簿，請確認瀏覽器權限後再試。'
  }
}

async function loadDeck() {
  deckBusy.value = true
  deckError.value = ''
  try {
    const response = await $fetch<{ deck: PlayerDeckList, source: 'custom' | 'preconstructed' }>('/api/deck')
    deckDraft.value = { name: response.deck.name, cards: [...response.deck.cards] }
    deckSource.value = response.source
  } catch (error) {
    deckError.value = presentApiError(error, '無法載入牌組')
    throw error
  } finally {
    deckBusy.value = false
  }
}

async function saveDeck() {
  if (!deckValidation.value.valid) return
  deckBusy.value = true
  deckError.value = ''
  try {
    const response = await $fetch<{ deck: PlayerDeckList }>('/api/deck', { method: 'PUT', body: deckDraft.value })
    deckDraft.value = { name: response.deck.name, cards: [...response.deck.cards] }
    deckSource.value = 'custom'
  } catch (error) {
    deckError.value = presentApiError(error, '無法儲存牌組')
  } finally {
    deckBusy.value = false
  }
}

async function resetDeck() {
  deckBusy.value = true
  deckError.value = ''
  try {
    const response = await $fetch<{ deck: PlayerDeckList }>('/api/deck', { method: 'DELETE' })
    deckDraft.value = { name: response.deck.name, cards: [...response.deck.cards] }
    deckSource.value = 'preconstructed'
  } catch (error) {
    deckError.value = presentApiError(error, '無法重設牌組')
  } finally {
    deckBusy.value = false
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
    deckInitialError.value = deckError.value || '無法載入牌組'
  }
  deckInitialLoading.value = false
}

onMounted(() => {
  void loadDeckPage()
})
</script>

<style>
@reference "../assets/css/main.css";

.deck-page { @apply mx-auto min-h-screen max-w-5xl px-6 pt-28 pb-12; }
.deck-editor { @apply grid gap-5; }
.deck-grid { @apply grid grid-cols-6 gap-2 overflow-x-auto; }
.deck-grid > strong { @apply flex min-h-11 items-center justify-center text-sm; }
.deck-count-control { @apply flex min-w-28 items-center justify-between rounded-lg p-1; border: 1px solid var(--app-border); background: var(--app-surface-raised); }
.deck-count-control button { @apply grid size-9 place-items-center rounded-md font-bold disabled:opacity-35; background: var(--app-control); color: var(--app-text); }
.deck-count-control span { @apply min-w-6 text-center font-bold; color: var(--app-text); }
.deck-validation { @apply flex flex-wrap gap-5 rounded-lg border border-emerald-700/30 bg-emerald-50 p-4 text-emerald-900; }
.deck-validation.invalid { @apply border-red-700/30 bg-red-50 text-red-900; }
.deck-validation-errors { @apply grid gap-1 text-sm text-red-700; }
.deck-transfer-status { @apply text-sm text-emerald-700; }
.deck-import-dialog { @apply my-auto w-full max-w-[640px] shadow-[0_24px_70px_rgba(0,0,0,.5)]; }
.deck-management-actions { @apply grid grid-cols-3 gap-3 max-[600px]:grid-cols-1; }
.deck-import-examples { @apply grid gap-2 text-xs text-muted; }
.deck-import-examples code { @apply block whitespace-pre-wrap p-2 font-mono; border: 1px solid var(--app-border); border-radius: 8px; background: var(--app-surface-muted); color: var(--app-text); }
.deck-import-text { @apply min-h-36 w-full p-3 font-mono outline-0; border: 1px solid var(--app-border); border-radius: 10px; background: var(--app-input); color: var(--app-text); }
.deck-import-text:focus { border-color: var(--app-accent); box-shadow: 0 0 0 3px var(--app-focus-ring); }
</style>
