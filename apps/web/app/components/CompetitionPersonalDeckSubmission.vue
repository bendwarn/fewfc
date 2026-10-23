<template>
  <section class="deck-submission panel" aria-labelledby="deck-submission-title">
    <header class="panel-heading">
      <div>
        <p class="eyebrow">賽事專用快照</p>
        <h2 id="deck-submission-title">提交本人個人牌組</h2>
      </div>
      <span v-if="ownDeckSubmitted" class="snapshot-badge">已提交快照</span>
    </header>

    <p class="explanation">
      帳號牌組是可編輯的個人資料；賽事提交會複製當下牌組作為獨立快照，之後編輯帳號牌組不會改動已提交內容。
      本元件只讀取帳號牌組，不會自動提交或代為儲存。
    </p>

    <p v-if="personalDecks === 'disabled'" class="state-note" role="status">
      這場賽事不使用個人牌組，無法提交。
    </p>
    <p v-else-if="!currentUserId" class="state-note" role="status">
      登入後才能讀取並提交目前登入者本人的帳號牌組。
    </p>
    <p v-else-if="lifecycle === 'draft'" class="state-note" role="status">
      賽事尚未開放，暫時不能提交牌組。
    </p>
    <p v-else-if="lifecycle === 'locked'" class="state-note" role="status">
      賽事名單已鎖定，不能再提交牌組。
    </p>
    <p v-else-if="!activeTeam || activeTeam.status === 'withdrawn'" class="state-note" role="status">
      目前沒有有效隊伍；加入或重新登記有效隊伍後才能提交。
    </p>
    <p v-else-if="!isActiveTeamMember" class="state-note" role="alert">
      目前隊伍資料未包含登入者，已停用提交；請重新載入賽事狀態。
    </p>

    <p v-if="ownDeckSubmitted" class="submitted-note" role="status" aria-live="polite">
      目前賽事已保存本人牌組快照。下方帳號牌組可能已更新；兩者是不同資料。
    </p>
    <p v-else class="state-note" role="status">
      目前賽事尚未收到本人牌組快照。
    </p>

    <div v-if="personalDecks === 'required' && currentUserId" class="saved-deck-section">
      <div class="saved-deck-toolbar">
        <div>
          <h3>目前帳號牌組</h3>
          <p class="muted">賽事不會追蹤此牌組的後續編輯。</p>
        </div>
        <div class="saved-deck-actions">
          <NuxtLink v-if="!controlsLocked" class="secondary-button link-button" to="/deck">
            前往編輯帳號牌組
          </NuxtLink>
          <span v-else class="muted">提交處理中，暫停編輯導覽</span>
          <button
            class="ghost-button"
            type="button"
            :disabled="controlsLocked"
            @click="loadSavedDeck"
          >
            {{ deckLoading ? '載入中…' : '重新載入帳號牌組' }}
          </button>
        </div>
      </div>

      <p v-if="deckLoading" class="state-note" role="status" aria-live="polite">
        正在載入本人帳號牌組與規則目錄…
      </p>
      <div v-else-if="deckError" class="load-error" role="alert" aria-live="assertive">
        <p>{{ deckError }}</p>
        <button class="ghost-button" type="button" :disabled="controlsLocked" @click="loadSavedDeck">
          重試載入
        </button>
      </div>
      <template v-else-if="savedDeck && deckValidation">
        <div class="deck-summary">
          <p><span>牌組名稱</span><strong>{{ savedDeck.deck.name }}</strong></p>
          <p><span>來源</span><strong>{{ savedDeck.source === 'custom' ? '自訂帳號牌組' : '系統預組牌組' }}</strong></p>
          <p><span>張數</span><strong>{{ deckValidation.cardCount }} 張</strong></p>
          <p><span>等級總和</span><strong>{{ deckValidation.levelTotal }}</strong></p>
        </div>

        <ul class="deck-grid" aria-label="帳號牌組 25 格卡牌張數">
          <li v-for="slot in deckSlots" :key="slot.key" class="deck-slot">
            <span>{{ slot.name }}・{{ slot.level }} 級</span>
            <strong>{{ slot.count }} 張</strong>
          </li>
        </ul>
      </template>
    </div>

    <template v-if="canSubmit">
      <p class="deadline-note">
        牌組提交截止（報名截止）：{{ formatDeckSubmissionDeadline(registrationDeadline) }}。報名截止時參賽資料一併凍結；截止時間與提交資格由伺服器核實，本頁不使用本機時間代替。
      </p>
      <p class="submission-note">
        任何一位目前隊員都可為本人提交，不必先完成隊員身分確認。按下按鈕時會固定目前顯示的名稱與卡牌清單。
      </p>

      <div v-if="activeTeam?.status === 'checkedIn'" class="recheckin-warning" role="alert">
        <strong>重新提交會撤銷隊長資料完成確認</strong>
        <p>報名截止前可重新提交；提交後隊伍會回到「已報名」，並必須由隊長再次完成資料確認。這不是房間報到或準備。</p>
        <label class="acknowledgement">
          <input v-model="acknowledgeCheckedInReset" type="checkbox" :disabled="controlsLocked">
          <span>我了解重新提交會要求隊長再次完成資料確認。</span>
        </label>
      </div>

      <button class="primary-button submit-button" type="button" :disabled="!canEmitSubmission" @click="submitSavedDeck">
        {{ pending || emittedPending ? '牌組提交命令待確認…' : '提交目前帳號牌組作為賽事快照' }}
      </button>
      <p v-if="submissionError" class="load-error" role="alert">{{ submissionError }}</p>
    </template>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount } from 'vue'
import { presentApiError } from '~/lib/api-error-presentation'
import { useRulesCatalog } from '~/lib/rules-catalog'
import {
  DeckSubmissionEligibilityError,
  DeckSubmissionProtocolError,
  buildDeckSubmissionAction,
  formatDeckSubmissionDeadline,
  parseSavedAccountDeckResponse,
  savedDeckCompositionSlots,
  validateSavedAccountDeck,
  type DeckSubmissionLifecycle,
  type DeckSubmissionPolicy,
  type DeckSubmissionTeam,
  type SavedAccountDeck,
  type SubmitDeckAction,
} from '~/lib/competition-deck-submission'
import type { DeckValidation } from '~/lib/player-deck'

const props = defineProps<{
  currentUserId: string | null
  activeTeam: DeckSubmissionTeam | null
  lifecycle: DeckSubmissionLifecycle
  personalDecks: DeckSubmissionPolicy
  registrationDeadline: number
  busy: boolean
  pending: boolean
  ownDeckSubmitted: boolean
}>()

const emit = defineEmits<{
  submitDeck: [action: SubmitDeckAction]
}>()

const rulesCatalog = useRulesCatalog()
const savedDeck = ref<SavedAccountDeck | null>(null)
const deckValidation = ref<DeckValidation | null>(null)
const deckSlots = ref<ReturnType<typeof savedDeckCompositionSlots>>([])
const deckLoading = ref(false)
const deckError = ref('')
const submissionError = ref('')
const acknowledgeCheckedInReset = ref(false)
const emittedPending = ref(false)
const mounted = ref(false)
let loadSequence = 0

const activeTeam = computed(() => props.activeTeam)
const isActiveTeamMember = computed(() => Boolean(props.currentUserId
  && activeTeam.value
  && activeTeam.value.status !== 'withdrawn'
  && activeTeam.value.members.includes(props.currentUserId)))
const controlsLocked = computed(() => props.busy || props.pending || emittedPending.value || deckLoading.value)
const canSubmit = computed(() => props.currentUserId !== null
  && props.personalDecks === 'required'
  && props.lifecycle === 'open'
  && isActiveTeamMember.value
  && savedDeck.value !== null
  && deckValidation.value?.valid === true)
const canEmitSubmission = computed(() => canSubmit.value
  && !controlsLocked.value
  && (activeTeam.value?.status !== 'checkedIn' || acknowledgeCheckedInReset.value))

async function loadSavedDeck() {
  if (!props.currentUserId || props.personalDecks !== 'required'
    || props.busy || props.pending || emittedPending.value || deckLoading.value) return

  const requestSequence = ++loadSequence
  deckLoading.value = true
  deckError.value = ''
  submissionError.value = ''
  savedDeck.value = null
  deckValidation.value = null
  deckSlots.value = []
  try {
    const [catalog, response] = await Promise.all([
      rulesCatalog.load(),
      $fetch<unknown>('/api/deck'),
    ])
    const parsed = parseSavedAccountDeckResponse(
      response,
      catalog.deckComposition.personalDeck.exactCardCount,
    )
    const validation = validateSavedAccountDeck(parsed, catalog.deckComposition)
    const slots = savedDeckCompositionSlots(parsed, catalog.deckComposition)
    if (requestSequence !== loadSequence) return
    savedDeck.value = parsed
    deckValidation.value = validation
    deckSlots.value = slots
  } catch (error) {
    if (requestSequence !== loadSequence) return
    deckError.value = error instanceof DeckSubmissionProtocolError
      ? error.message
      : presentApiError(error, '無法載入本人帳號牌組或規則目錄。')
  } finally {
    if (requestSequence === loadSequence) deckLoading.value = false
  }
}

function submitSavedDeck() {
  if (!canEmitSubmission.value || !savedDeck.value || !rulesCatalog.catalog.value) return
  submissionError.value = ''
  try {
    const action = buildDeckSubmissionAction({
      currentUserId: props.currentUserId,
      activeTeam: props.activeTeam,
      lifecycle: props.lifecycle,
      personalDecks: props.personalDecks,
      savedDeck: savedDeck.value,
      composition: rulesCatalog.catalog.value.deckComposition,
      acknowledgeCheckedInReset: acknowledgeCheckedInReset.value,
    })
    emittedPending.value = true
    emit('submitDeck', action)
  } catch (error) {
    submissionError.value = error instanceof DeckSubmissionEligibilityError
      || error instanceof DeckSubmissionProtocolError
      ? error.message
      : '無法建立牌組提交快照，請重新載入帳號牌組。'
  }
}

watch(() => props.pending, (pending, previous) => {
  if (previous && !pending) {
    emittedPending.value = false
    acknowledgeCheckedInReset.value = false
  }
})

watch(() => [props.activeTeam?.teamId, props.activeTeam?.rosterVersion, props.activeTeam?.status], () => {
  acknowledgeCheckedInReset.value = false
})

watch(() => [props.currentUserId, props.personalDecks] as const, () => {
  if (!mounted.value) return
  loadSequence += 1
  savedDeck.value = null
  deckValidation.value = null
  deckSlots.value = []
  deckError.value = ''
  deckLoading.value = false
  if (props.currentUserId && props.personalDecks === 'required') void loadSavedDeck()
})

onMounted(() => {
  mounted.value = true
  if (props.currentUserId && props.personalDecks === 'required') void loadSavedDeck()
})

onBeforeUnmount(() => {
  loadSequence += 1
})
</script>

<style scoped>
@reference "../assets/css/main.css";

.deck-submission { @apply grid gap-4 border border-line bg-panel p-5 max-[600px]:p-4; border-radius: 16px; box-shadow: var(--app-shadow-md); }
.panel-heading { @apply flex flex-wrap items-start justify-between gap-3; }
.panel-heading h2, .saved-deck-toolbar h3 { @apply m-0 font-serif text-xl; }
.eyebrow { @apply m-0 text-xs text-gold-light; }
.explanation, .muted, .deadline-note, .submission-note { @apply m-0 text-sm leading-6; color: var(--app-text-muted); }
.state-note { @apply m-0 text-sm leading-6; }
.submitted-note { @apply m-0 rounded-lg border border-line p-3 text-sm leading-6; color: var(--app-accent-strong); background: var(--app-accent-soft); }
.snapshot-badge { @apply rounded-full border border-line px-3 py-1 text-xs; color: var(--app-accent-strong); }
.saved-deck-section { @apply grid gap-4 border-t border-line pt-4; }
.saved-deck-toolbar { @apply flex flex-wrap items-start justify-between gap-4; }
.saved-deck-toolbar > div:first-child { @apply grid gap-1; }
.saved-deck-toolbar h3 { @apply text-base; }
.saved-deck-actions { @apply flex flex-wrap items-center gap-2; }
.deck-summary { @apply grid grid-cols-2 gap-2 max-[480px]:grid-cols-1; }
.deck-summary p { @apply grid min-w-0 gap-1 rounded-lg border border-line p-3 text-sm; background: var(--app-surface-muted); }
.deck-summary span { @apply text-xs; color: var(--app-text-muted); }
.deck-summary strong { @apply break-words; }
.deck-grid { @apply m-0 grid list-none grid-cols-5 gap-2 p-0 max-[600px]:grid-cols-3 max-[380px]:grid-cols-2; }
.deck-slot { @apply grid min-w-0 gap-1 rounded-md border border-line p-2 text-center text-xs; background: var(--app-surface-muted); }
.deck-slot span { @apply break-words; }
.deck-slot strong { @apply text-gold-light; }
.recheckin-warning, .load-error { @apply grid gap-2 rounded-lg border border-line p-4; border-color: var(--app-danger); background: var(--app-danger-surface); }
.recheckin-warning strong { @apply text-sm; color: var(--app-danger); }
.recheckin-warning p, .load-error p { @apply m-0 text-sm leading-6; }
.acknowledgement { @apply flex items-start gap-2 text-sm leading-6; }
.acknowledgement input { @apply mt-1 shrink-0; }
.submit-button { @apply w-fit max-[600px]:w-full; }
.link-button { @apply inline-flex items-center justify-center no-underline; }

@media (max-width: 600px) {
  .saved-deck-actions { @apply grid w-full; }
  .saved-deck-actions > * { @apply w-full; }
}
.deck-submission { border: 1px solid var(--ui-glass-stroke); border-radius: 22px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); -webkit-backdrop-filter: blur(18px); backdrop-filter: blur(18px); }
.deck-submission :is(input, select, textarea, button):focus-visible { outline: 2px solid var(--app-accent); outline-offset: 3px; }
</style>
