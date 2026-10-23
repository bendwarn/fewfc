<template>
  <section class="correction-panel" :aria-labelledby="`${inputId}-title`">
    <h2 :id="`${inputId}-title`">裁判更正競賽判定</h2>
    <p class="hint">本片只提供同局已保存判定，或採納一致的原始遊戲結論；不提供完整改判、棄權或重賽。原始終局證據與遊戲結論不會由本頁改寫。</p>
    <form class="lookup-form" @submit.prevent="readMatch">
      <label :for="inputId">主辦方提供的交手 ID
        <input :id="inputId" v-model="matchDraft" data-testid="correction-match" type="text" autocomplete="off"
          :disabled="Boolean(pending) || reading || sending || !contextReady || !sessionUserId" required>
      </label>
      <button class="ghost-button" data-testid="correction-refresh" type="submit" :disabled="!canRead">
        {{ reading ? '查詢中…' : '讀取／重新整理交手紀錄' }}
      </button>
    </form>
    <p v-if="!contextReady || !sessionUserId" class="hint" role="status">請登入並等待目前賽事狀態確認後，再查詢交手紀錄。</p>
    <p v-if="reading" role="status" aria-live="polite">正在查詢權威交手紀錄…</p>
    <p v-if="readError" class="error" role="alert">{{ readError }}</p>
    <p v-if="mutationError" class="error" role="alert">{{ mutationError }}</p>
    <p v-if="notice" role="status" aria-live="polite">{{ notice }}</p>

    <section v-if="pending" class="pending-note" role="status" aria-live="polite">
      <p>更正命令的結果尚未確認。重新整理、新版本或後續拒絕都不能證明先前未提交；只有原命令的收據確認能解除未知結果。</p>
      <p>離開、切換賽事／交手／帳號或中止連線，不代表伺服器取消更正。請先取得收據確認；本頁不會跨身份自動重送。</p>
      <template v-if="samePendingScope">
        <p>原命令：第 {{ pending.gameNumber }} 局 · {{ competitionRulingLabel(pending.body.replacementRuling, pending.teams) }}</p>
        <button class="secondary-button" data-testid="same-command-retry" type="button" :disabled="!canRetry" @click="sendPending">
          {{ sending ? '正在等待原命令收據…' : '以同一命令重試' }}
        </button>
      </template>
      <p v-else>請切回原賽事、交手及同一帳號再確認原命令；目前不能送出新的更正。</p>
    </section>

    <template v-if="visibleView">
      <p role="status">{{ competitionCorrectionStatusLabel(visibleView) }}</p>
      <template v-if="visibleView.match">
        <div class="team-summary" aria-label="此交手隊伍與比分">
          <p v-for="(team, index) in visibleView.match.config.teams" :key="team">
            {{ index === 0 ? '甲隊' : '乙隊' }}：{{ visibleView.match.score[index] }} 勝局
          </p>
        </div>
        <p v-if="visibleView.match.outcome?.type === 'doubleLoss'" class="hint">雙方判負；後續晉級與名次待裁判處理，本頁不指定勝者。</p>
        <p v-if="!assigned" class="hint" role="status">此交手未指派你為無利益衝突的裁判；主辦身分不自動取得改判權限。</p>
        <p v-else class="hint">此交手已指派你為裁判且未標利益衝突；是否可提交仍以伺服器目前有效授權為準。</p>
        <p v-if="!visibleView.match.games.length" class="hint" role="status">目前沒有已保存終局紀錄，無可安全選擇的更正項目。</p>
        <article v-for="game in visibleView.match.games" :key="game.gameRecordId" class="game-record">
          <h3>第 {{ game.gameNumber }} 局</h3>
          <p>{{ competitionTerminalEvidenceLabel(game.terminalEvidence, visibleView.match.config.teams) }}</p>
          <p>目前競賽判定：{{ currentRulingLabel(game.rulingRecordId) }}</p>
          <ul v-if="gameCorrections(game.gameRecordId).length" aria-label="已保存更正理由">
            <li v-for="(correction, index) in gameCorrections(game.gameRecordId)" :key="correction.correctionId">
              第 {{ index + 1 }} 次更正：{{ competitionRulingLabel(correction.replacementRuling, visibleView.match.config.teams) }} · {{ correction.reason }}
            </li>
          </ul>
        </article>
        <form v-if="assigned && visibleView.match.games.length" data-testid="correction-form" class="correction-form" @submit.prevent="submitCorrection">
          <label :for="`${inputId}-game`">要更正的已保存對局
            <select :id="`${inputId}-game`" v-model="selectedGame" data-testid="correction-game" :disabled="draftLocked" required>
              <option value="" disabled>請選擇對局</option>
              <option v-for="game in visibleView.match.games" :key="game.gameRecordId" :value="game.gameRecordId">第 {{ game.gameNumber }} 局</option>
            </select>
          </label>
          <label :for="`${inputId}-candidate`">替代競賽判定
            <select :id="`${inputId}-candidate`" v-model="selectedCandidate" data-testid="correction-candidate" :disabled="draftLocked || !candidates.length" required>
              <option value="" disabled>請選擇替代判定</option>
              <option v-for="candidate in candidates" :key="candidate.key" :value="candidate.key">{{ candidate.label }}</option>
            </select>
          </label>
          <p v-if="selectedGame && !candidates.length" class="hint" role="status">沒有可安全使用的替代候選；本頁不推定讀秒先後、續局時間或雙敗晉級。</p>
          <label :for="`${inputId}-reason`">更正理由
            <textarea :id="`${inputId}-reason`" v-model="reason" data-testid="correction-reason" rows="3" :disabled="draftLocked" required />
          </label>
          <p class="hint">請核對原始終局證據及替代判定。更正可能因後續局或賽程狀態遭拒；本頁不會自動改其他判定或暫停下游。</p>
          <button class="primary-button" data-testid="correction-submit" type="submit" :disabled="!canSubmit">提交更正</button>
        </form>
      </template>
    </template>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount, useId, shallowRef } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import {
  buildCompetitionRulingCorrection, competitionCorrectionCandidates, competitionCorrectionErrorMessage,
  competitionCorrectionErrorStatus, competitionCorrectionPath, competitionCorrectionStatusLabel,
  competitionRulingLabel, competitionTerminalEvidenceLabel, createCompetitionCorrectionScope,
  isAssignedCompetitionReferee, keepCompetitionCorrectionIntent, parseCompetitionCorrectionAck,
  parseCompetitionCorrectionMatch, type CompetitionCorrectionCommand, type CompetitionCorrectionMatch,
  type CompetitionCorrectionScope,
} from '~/lib/competition-ruling-correction'

const props = withDefaults(defineProps<{
  competitionId: string
  sessionUserId: string | null
  contextReady: boolean
  matchId?: string
}>(), { matchId: '' })
const emit = defineEmits<{ unresolved: [] }>()
const inputId = useId()
const matchDraft = ref(props.matchId)
const selectedGame = ref('')
const selectedCandidate = ref('')
const reason = ref('')
const view = shallowRef<CompetitionCorrectionMatch | null>(null)
const viewScope = ref('')
const reading = ref(false)
const sending = ref(false)
const readError = ref('')
const mutationError = ref('')
const notice = ref('')
const mounted = ref(false)
let hasRead = false
let readSequence = 0
let sendSequence = 0
let readController: AbortController | null = null
let sendController: AbortController | null = null
interface PendingCorrection {
  scope: CompetitionCorrectionScope
  body: Readonly<CompetitionCorrectionCommand>
  unknown: boolean
  teams: [string, string]
  gameNumber: number
}
// shallowRef 不把 immutable snapshot 轉成可修改的 reactive draft。
const pending = shallowRef<PendingCorrection | null>(null)
const scope = computed(() => createCompetitionCorrectionScope(props.competitionId, matchDraft.value, props.sessionUserId))
function key(value: CompetitionCorrectionScope | null): string { return value ? JSON.stringify([value.competitionId, value.matchId, value.sessionUserId]) : '' }
const scopeKey = computed(() => key(scope.value))
const enabled = computed(() => props.contextReady && Boolean(scope.value))
const visibleView = computed(() => enabled.value && viewScope.value === scopeKey.value ? view.value : null)
const assigned = computed(() => isAssignedCompetitionReferee(visibleView.value, scope.value?.sessionUserId ?? null))
const samePendingScope = computed(() => Boolean(pending.value && key(pending.value.scope) === scopeKey.value))
const canRead = computed(() => enabled.value && !reading.value && !sending.value)
const canRetry = computed(() => enabled.value && samePendingScope.value && !reading.value && !sending.value)
const draftLocked = computed(() => !enabled.value || reading.value || sending.value || Boolean(pending.value))
const candidates = computed(() => visibleView.value ? competitionCorrectionCandidates(visibleView.value, selectedGame.value) : [])
const canSubmit = computed(() => !draftLocked.value && assigned.value && reason.value.trim().length > 0
  && candidates.value.some(candidate => candidate.key === selectedCandidate.value)
  && Boolean(visibleView.value && visibleView.value.matchRevision < Number.MAX_SAFE_INTEGER))

function invalidate() {
  readSequence += 1
  sendSequence += 1
  readController?.abort()
  sendController?.abort()
  readController = null
  sendController = null
  if (sending.value && pending.value) pending.value = { ...pending.value, unknown: true }
  reading.value = false
  sending.value = false
  view.value = null
  viewScope.value = ''
  readError.value = ''
  notice.value = ''
}
function currentRead(sequence: number, identity: string): boolean {
  return mounted.value && enabled.value && sequence === readSequence && identity === scopeKey.value
}
function currentSend(sequence: number, intent: PendingCorrection): boolean {
  return mounted.value && enabled.value && sequence === sendSequence && key(intent.scope) === scopeKey.value
    && pending.value?.body === intent.body
}

async function readMatch() {
  const identity = scope.value
  if (!canRead.value || !identity) return
  hasRead = true
  const controller = new AbortController()
  readController?.abort()
  readController = controller
  const sequence = ++readSequence
  const identityKey = key(identity)
  reading.value = true
  view.value = null
  viewScope.value = ''
  readError.value = ''
  try {
    const raw: unknown = await $fetch(competitionCorrectionPath(identity), { method: 'GET', retry: 0, signal: controller.signal })
    if (!currentRead(sequence, identityKey)) return
    view.value = parseCompetitionCorrectionMatch(raw, identity)
    viewScope.value = identityKey
  } catch (error) {
    if (currentRead(sequence, identityKey)) readError.value = competitionCorrectionErrorMessage(error, 'read')
  } finally {
    if (currentRead(sequence, identityKey)) { reading.value = false; readController = null }
  }
}

async function submitCorrection() {
  const identity = scope.value
  const state = visibleView.value
  if (!canSubmit.value || !identity || !state?.match) return
  try {
    const game = state.match.games.find(item => item.gameRecordId === selectedGame.value)
    if (!game) return
    const body = buildCompetitionRulingCorrection(state, identity, selectedGame.value, selectedCandidate.value, reason.value)
    pending.value = { scope: { ...identity }, body, unknown: false, teams: [...state.match.config.teams], gameNumber: game.gameNumber }
    await sendPending()
  } catch {
    mutationError.value = '更正草稿無法安全建立；請重新核對交手、候選與理由。'
  }
}
async function sendPending() {
  const intent = pending.value
  if (!canRetry.value || !intent) return
  const controller = new AbortController()
  sendController = controller
  const sequence = ++sendSequence
  sending.value = true
  mutationError.value = ''
  notice.value = ''
  let refresh = false
  try {
    const raw: unknown = await $fetch(`${competitionCorrectionPath(intent.scope)}/correct-ruling`, {
      method: 'POST', body: intent.body, retry: 0, signal: controller.signal,
    })
    if (!currentSend(sequence, intent)) return
    parseCompetitionCorrectionAck(raw, intent.scope, intent.body)
    pending.value = null
    reason.value = ''
    selectedCandidate.value = ''
    notice.value = '更正已獲伺服器收據確認；正在重新讀取交手紀錄。'
    refresh = true
  } catch (error) {
    if (!currentSend(sequence, intent)) return
    mutationError.value = competitionCorrectionErrorMessage(error)
    if (keepCompetitionCorrectionIntent(intent.unknown, error)) pending.value = { ...intent, unknown: true }
    else pending.value = null
    refresh = competitionCorrectionErrorStatus(error) === 409
  } finally {
    // ACK 可能已清 pending；generation 仍必須相同才能解除這個 request 的 busy。
    if (mounted.value && sequence === sendSequence && key(intent.scope) === scopeKey.value && enabled.value) {
      sending.value = false
      sendController = null
    }
  }
  if (refresh && mounted.value && sequence === sendSequence && key(intent.scope) === scopeKey.value && enabled.value) {
    await readMatch()
    if (!pending.value && notice.value && readError.value) notice.value = '更正已獲伺服器收據確認，但交手顯示尚未更新；請重新讀取。'
  }
}
function currentRulingLabel(rulingRecordId: string): string {
  const match = visibleView.value?.match
  const record = match?.rulings.find(item => item.rulingRecordId === rulingRecordId)
  return match && record ? competitionRulingLabel(record.ruling, match.config.teams) : '目前判定無法確認'
}
function gameCorrections(gameRecordId: string) { return visibleView.value?.match?.corrections.filter(item => item.gameRecordId === gameRecordId) ?? [] }
function warnUnload(event: BeforeUnloadEvent) {
  if (!pending.value) return
  event.preventDefault()
  event.returnValue = ''
}
watch(selectedGame, () => { selectedCandidate.value = '' })
watch(() => props.matchId, value => { matchDraft.value = value })
watch(scopeKey, () => {
  invalidate()
  hasRead = false
  selectedGame.value = ''
  selectedCandidate.value = ''
  reason.value = ''
  mutationError.value = ''
}, { flush: 'sync' })
watch(() => props.contextReady, ready => {
  invalidate()
  if (ready && mounted.value && hasRead) void readMatch()
}, { flush: 'sync' })
onBeforeRouteLeave(() => !pending.value || window.confirm('更正結果尚未取得收據確認；離開不代表伺服器取消更正。仍要離開嗎？'))
onMounted(() => {
  mounted.value = true
  window.addEventListener('beforeunload', warnUnload)
  if (props.matchId) void readMatch()
})
onBeforeUnmount(() => {
  if (pending.value) emit('unresolved')
  mounted.value = false
  invalidate()
  window.removeEventListener('beforeunload', warnUnload)
})
</script>

<style scoped>
.correction-panel { min-width: 0; display: grid; gap: 1rem; border: 1px solid var(--app-border); border-radius: 16px; padding: 1.25rem; background: var(--app-surface); }
h2, h3, p { margin: 0; }
h2, h3 { font-family: var(--font-serif); }
h2 { font-size: 1.25rem; }
h3 { font-size: 1rem; }
p, li { font-size: .875rem; line-height: 1.65; overflow-wrap: anywhere; }
.hint { color: var(--app-text-muted); }
.lookup-form, .correction-form { display: grid; gap: .875rem; min-width: 0; }
label { display: grid; min-width: 0; gap: .5rem; font-size: .875rem; }
input, select, textarea { box-sizing: border-box; width: 100%; min-width: 0; border: 1px solid var(--app-border); border-radius: 12px; padding: .75rem; color: var(--app-text); background: var(--app-input); }
textarea { resize: vertical; }
button { min-height: 44px; min-width: 0; border: 1px solid var(--app-border); border-radius: 12px; padding: .75rem 1rem; color: var(--app-text); background: var(--app-control); white-space: normal; overflow-wrap: anywhere; cursor: pointer; }
.primary-button { border-color: var(--app-accent); color: var(--ui-on-gold); background: var(--ui-gold-fill); }
.ghost-button { background: var(--app-surface); }
button:disabled { opacity: .6; cursor: not-allowed; }
button:focus-visible, input:focus-visible, select:focus-visible, textarea:focus-visible { outline: 2px solid var(--app-accent-strong); outline-offset: 3px; box-shadow: 0 0 0 4px var(--app-focus-ring); }
.team-summary { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .75rem; }
.game-record, .pending-note { display: grid; min-width: 0; gap: .625rem; border-left: 3px solid var(--app-border); padding-left: .875rem; }
.pending-note { border-color: var(--app-accent-strong); }
.error { color: var(--app-danger); }
@media (max-width: 600px) { .correction-panel { padding: 1rem; } }
.correction-panel { border: 1px solid var(--ui-glass-stroke); border-radius: 22px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); -webkit-backdrop-filter: blur(18px); backdrop-filter: blur(18px); }
.correction-panel :is(input, select, textarea, button):focus-visible { outline: 2px solid var(--app-accent); outline-offset: 3px; }
</style>
