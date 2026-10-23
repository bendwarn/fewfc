<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount, nextTick, shallowRef } from 'vue'
import { onBeforeRouteLeave } from 'vue-router'
import type { CompetitionRegistrationState } from '../lib/competition-registration'
import {
  competitionRoundPublicationDeadlineLabel,
  competitionRoundPublicationErrorMessage,
  competitionRoundPublicationPath,
  competitionRoundPublicationStatusLabel,
  competitionTournamentPath,
  buildCompetitionRoundPublicationIntent,
  createCompetitionRoundPublicationContext,
  findCurrentCompetitionRoundPublicationTarget,
  keepCompetitionRoundPublicationIntent,
  parseCompetitionRoundAnnouncementAck,
  parseCompetitionRoundPublicationReadSnapshot,
  CompetitionRoundPublicationProtocolError,
  CompetitionRoundPublicationValidationError,
  type CompetitionRoundPublicationIntent,
  type CompetitionRoundPublicationReadSnapshot,
  type CompetitionRoundPublicationScope,
  type CompetitionRoundPublicationTargetResult,
} from '../lib/competition-round-publication'
import { formatCompetitionDeadline, localMinuteDateTimeToEpochMs } from '../lib/competition-registration'
import {
  competitionRoundReadyCutoffMs,
} from '#shared/competition-game-room'
import {
  parseCompetitionTournamentViewResponse,
  presentCompetitionTournamentViewError,
  type CompetitionTournamentViewData,
} from '../lib/competition-tournament-view'

const props = withDefaults(defineProps<{
  competitionId: string
  sessionUserId: string | null
  registration: CompetitionRegistrationState | null
  contextReady: boolean
  busy?: boolean
  automaticInitialStageId?: string | null
}>(), { busy: false })

const emit = defineEmits<{
  'pending-change': [intent: CompetitionRoundPublicationIntent | null]
  unresolved: [intent: CompetitionRoundPublicationIntent]
  announced: [receipt: { competitionId: string, stageId: string, roundNumber: number, commandId: string, revision: number, roundStartMs: number }]
}>()

interface PendingAnnouncement {
  intent: CompetitionRoundPublicationIntent
  identityKey: string
  unknown: boolean
}

const context = computed(() => createCompetitionRoundPublicationContext(
  props.competitionId, props.sessionUserId, props.registration,
))
const identityKey = computed(() => JSON.stringify([
  props.competitionId, props.sessionUserId, props.registration?.competitionId,
  props.registration?.organizer, props.registration?.lifecycle,
]))
const contextEnabled = computed(() => props.contextReady && !props.busy && context.value !== null)
const eligibilityMessage = computed(() => {
  if (!props.sessionUserId?.trim()) return '請先登入原公告主辦帳號。'
  if (!props.contextReady || props.busy) return '正在確認賽事狀態，請稍候。'
  if (props.registration?.competitionId !== props.competitionId) return '賽事身分無法確認；本元件不會跨賽事查詢或公告。'
  if (props.registration?.organizer !== props.sessionUserId) return '僅原公告主辦者可查詢與公告此輪；裁判或玩家身分不會自動取得主辦權限。'
  if (props.registration?.lifecycle !== 'locked') return '報名名單鎖定後才能公告輪次。'
  return ''
})

const tournament = shallowRef<CompetitionTournamentViewData | null>(null)
const targetResult = shallowRef<CompetitionRoundPublicationTargetResult | null>(null)
const publicationSnapshot = shallowRef<CompetitionRoundPublicationReadSnapshot | null>(null)
const publicationScopeKey = ref('')
const pending = shallowRef<PendingAnnouncement | null>(null)
const roundStartLocal = ref('')
const reading = ref(false)
const sending = ref(false)
const readError = ref('')
const mutationMessage = ref('')
const validationMessage = ref('')
const notice = ref('')
const requiresRefresh = ref(false)
const mounted = ref(false)
let generation = 0
let readSequence = 0
let postSequence = 0
let readController: AbortController | null = null
let postController: AbortController | null = null

const selectedTarget = computed(() => targetResult.value?.kind === 'ready' ? targetResult.value.target : null)
const isAutomaticInitialRound = computed(() => Boolean(selectedTarget.value
  && props.automaticInitialStageId === selectedTarget.value.stageId
  && selectedTarget.value.roundNumber === 1))
const targetScope = computed<CompetitionRoundPublicationScope | null>(() => {
  const current = context.value
  const target = selectedTarget.value
  return current && target ? { ...current, stageId: target.stageId, roundNumber: target.roundNumber } : null
})
function scopeKey(scope: CompetitionRoundPublicationScope | null): string {
  return scope ? JSON.stringify([scope.competitionId, scope.sessionUserId, scope.stageId, scope.roundNumber]) : ''
}
const samePendingIdentity = computed(() => {
  const current = context.value
  const currentPending = pending.value
  return Boolean(current && currentPending && currentPending.identityKey === identityKey.value
    && currentPending.intent.scope.competitionId === current.competitionId
    && currentPending.intent.scope.sessionUserId === current.sessionUserId)
})
const readEnabled = computed(() => contextEnabled.value)
const canRead = computed(() => readEnabled.value && !reading.value && !sending.value)
const canRetry = computed(() => Boolean(pending.value && samePendingIdentity.value && readEnabled.value
  && !reading.value && !sending.value))
const publication = computed(() => publicationScopeKey.value === scopeKey(targetScope.value)
  ? publicationSnapshot.value?.publication ?? null : null)
const publicationCutoff = computed(() => publication.value
  ? competitionRoundReadyCutoffMs(publication.value.roundStartMs) : null)
const hasPublicationRead = computed(() => publicationScopeKey.value !== ''
  && publicationScopeKey.value === scopeKey(targetScope.value))
const currentPublicationMatchesTarget = computed(() => {
  const value = publication.value
  const target = selectedTarget.value
  if (!value || !target) return true
  if (value.rooms.length !== target.matchIds.length) return false
  const actual = new Set(value.rooms.map(room => room.matchId))
  return target.matchIds.every(matchId => actual.has(matchId))
})
const canAnnounce = computed(() => Boolean(contextEnabled.value && !pending.value && !reading.value && !sending.value
  && !requiresRefresh.value && !readError.value && selectedTarget.value && hasPublicationRead.value
  && publication.value === null && !isAutomaticInitialRound.value))
const showAnnouncementForm = computed(() => Boolean(selectedTarget.value && hasPublicationRead.value
  && contextEnabled.value && publication.value === null && !readError.value && !requiresRefresh.value && !pending.value
  && !isAutomaticInitialRound.value))
const timePreview = computed(() => {
  if (!roundStartLocal.value) return null
  try {
    const timestamp = localMinuteDateTimeToEpochMs(roundStartLocal.value, '比賽開始時間')
    const cutoff = competitionRoundReadyCutoffMs(timestamp)
    return { timestamp, cutoff }
  } catch { return null }
})

function targetBlockMessage(result: CompetitionRoundPublicationTargetResult | null): string {
  if (!result || result.kind !== 'blocked') return ''
  return {
    singleActiveStageRequired: '目前沒有唯一明確的進行中階段；歧義賽程不能由本片公告。',
    scheduleNeedsReview: '賽程有未解決警示，請先由主辦或裁判處理。',
    noCurrentRound: '賽程尚未提供可公告的明確輪次；本元件不建立配對或新輪。',
    previousRoundIncomplete: '前輪尚未全部完成，不能公告下一輪。',
    currentRoundStarted: '目前輪次已有交手開始，不能在本元件重新公告。',
    currentRoundUnresolved: '目前輪次包含尚未解決或非待公告交手，不能安全公告。',
    noPendingRooms: '目前輪次沒有可開房的待公告交手；不建立空公告。',
  }[result.reason]
}

function acceptedRead(sequence: number, token: number, identity: string, controller: AbortController): boolean {
  return mounted.value && token === generation && sequence === readSequence && identity === identityKey.value
    && !controller.signal.aborted
}
function acceptedPost(sequence: number, token: number, identity: string, intent: CompetitionRoundPublicationIntent,
  controller: AbortController): boolean {
  const currentPending = pending.value
  return mounted.value && token === generation && sequence === postSequence && identity === identityKey.value
    && currentPending !== null && currentPending.intent === intent && currentPending.identityKey === identity
    && samePendingIdentity.value && !controller.signal.aborted
}

async function loadPublication(): Promise<void> {
  const current = context.value
  if (!mounted.value || !canRead.value || !current) return
  readController?.abort()
  const controller = new AbortController()
  readController = controller
  const sequence = ++readSequence
  const token = generation
  const identity = identityKey.value
  reading.value = true
  readError.value = ''
  notice.value = ''
  requiresRefresh.value = false
  tournament.value = null
  targetResult.value = null
  publicationSnapshot.value = null
  publicationScopeKey.value = ''
  let phase: 'tournament' | 'publication' = 'tournament'
  try {
    const rawTournament: unknown = await $fetch(competitionTournamentPath(current), {
      method: 'GET', retry: 0, signal: controller.signal,
    })
    if (!acceptedRead(sequence, token, identity, controller)) return
    const parsed = parseCompetitionTournamentViewResponse(rawTournament, current.competitionId)
    tournament.value = parsed
    const result = findCurrentCompetitionRoundPublicationTarget(parsed)
    targetResult.value = result
    if (result.kind !== 'ready') return

    const scope: CompetitionRoundPublicationScope = {
      ...current, stageId: result.target.stageId, roundNumber: result.target.roundNumber,
    }
    phase = 'publication'
    const rawPublication: unknown = await $fetch(competitionRoundPublicationPath(scope), {
      method: 'GET', retry: 0, signal: controller.signal,
    })
    if (!acceptedRead(sequence, token, identity, controller)) return
    const snapshot = parseCompetitionRoundPublicationReadSnapshot(rawPublication, scope)
    if (snapshot.revision !== parsed.revision) {
      throw new CompetitionRoundPublicationProtocolError()
    }
    publicationSnapshot.value = snapshot
    publicationScopeKey.value = scopeKey(scope)
  } catch (error) {
    if (!acceptedRead(sequence, token, identity, controller)) return
    if (phase === 'tournament') {
      readError.value = presentCompetitionTournamentViewError(error).message
    } else {
      readError.value = competitionRoundPublicationErrorMessage(error, 'read')
    }
  } finally {
    if (acceptedRead(sequence, token, identity, controller)) {
      reading.value = false
      readController = null
    }
  }
}

async function announceRound(): Promise<void> {
  const current = context.value
  const parsed = tournament.value
  if (!canAnnounce.value || !current || !parsed) return
  try {
    const intent = buildCompetitionRoundPublicationIntent(current, parsed, roundStartLocal.value, Date.now())
    pending.value = { intent, identityKey: identityKey.value, unknown: false }
    mutationMessage.value = '正在送出固定的原公告命令；不會自動順延或改寫 T。'
    validationMessage.value = ''
    emit('pending-change', intent)
    await sendPending()
  } catch (error) {
    validationMessage.value = error instanceof CompetitionRoundPublicationValidationError
      ? error.message : '公告命令無法安全建立，請重新查詢賽程。'
  }
}

async function sendPending(): Promise<void> {
  const current = pending.value
  if (!current || !canRetry.value) return
  const intent = current.intent
  const controller = new AbortController()
  postController = controller
  const sequence = ++postSequence
  const token = generation
  const identity = identityKey.value
  sending.value = true
  mutationMessage.value = ''
  try {
    const raw: unknown = await $fetch(intent.path, {
      method: intent.method, body: intent.body, retry: 0, signal: controller.signal,
    })
    if (!acceptedPost(sequence, token, identity, intent, controller)) return
    const receipt = parseCompetitionRoundAnnouncementAck(raw, intent)
    pending.value = null
    emit('pending-change', null)
    const snapshot: CompetitionRoundPublicationReadSnapshot = Object.freeze({
      competitionId: receipt.competitionId, revision: receipt.revision, publication: receipt.publication,
    })
    const receiptScopeKey = scopeKey(intent.scope)
    // 同輪非空 GET 是目前權威投影；原 v1 收據不能重置版本、房間 ACK 或處置狀態。
    // 沒有目前公告投影時仍沿用收據顯示；命令結清與通知不依賴此顯示判斷。
    if (receiptScopeKey === scopeKey(targetScope.value) && publication.value === null) {
      publicationSnapshot.value = snapshot
      publicationScopeKey.value = receiptScopeKey
    }
    mutationMessage.value = ''
    notice.value = '公告收據已確認；本公告只記錄後端輪次狀態，不表示遊戲已開局。'
    emit('announced', {
      competitionId: receipt.competitionId, stageId: receipt.stageId, roundNumber: receipt.roundNumber,
      commandId: receipt.commandId, revision: receipt.revision, roundStartMs: receipt.publication.roundStartMs,
    })
  } catch (error) {
    if (!acceptedPost(sequence, token, identity, intent, controller)) return
    const keep = keepCompetitionRoundPublicationIntent(current.unknown, error)
    mutationMessage.value = competitionRoundPublicationErrorMessage(error, 'announce')
    if (keep) {
      pending.value = { intent, identityKey: current.identityKey, unknown: true }
      emit('unresolved', intent)
    } else {
      pending.value = null
      requiresRefresh.value = true
      emit('pending-change', null)
    }
  } finally {
    if (mounted.value && token === generation && sequence === postSequence && !controller.signal.aborted) {
      sending.value = false
      postController = null
    }
  }
}

function invalidateForIdentityChange(): void {
  generation += 1
  readSequence += 1
  postSequence += 1
  readController?.abort()
  postController?.abort()
  readController = null
  postController = null
  if (pending.value) {
    pending.value = { ...pending.value, unknown: true }
    emit('pending-change', pending.value.intent)
    emit('unresolved', pending.value.intent)
    mutationMessage.value = '原公告命令仍未確認；離開、重新查詢或切換帳號都不代表伺服器已取消。'
  }
  reading.value = false
  sending.value = false
  tournament.value = null
  targetResult.value = null
  publicationSnapshot.value = null
  publicationScopeKey.value = ''
  readError.value = ''
  notice.value = ''
}

function warnBeforeUnload(event: BeforeUnloadEvent): void {
  if (!pending.value) return
  event.preventDefault()
  event.returnValue = ''
}

function formatRoomAck(timestamp: number | null): string {
  return timestamp === null ? '尚未收到' : formatCompetitionDeadline(timestamp)
}

function publicationRoomWarning(): string {
  if (!currentPublicationMatchesTarget.value) return '既有公告房間集合與目前賽程不一致；本片不會撤銷、重配或重公告。'
  return ''
}

watch(identityKey, () => {
  invalidateForIdentityChange()
  roundStartLocal.value = ''
  validationMessage.value = ''
  if (mounted.value && contextEnabled.value) {
    void nextTick().then(() => { if (mounted.value && contextEnabled.value) void loadPublication() })
  }
}, { flush: 'sync' })

watch(contextEnabled, enabled => {
  if (!enabled || !mounted.value) return
  void nextTick().then(() => {
    if (mounted.value && contextEnabled.value && !reading.value && !sending.value) {
      void loadPublication()
    }
  })
})

onMounted(() => {
  mounted.value = true
  window.addEventListener('beforeunload', warnBeforeUnload)
  if (contextEnabled.value) void loadPublication()
})

onBeforeRouteLeave(() => !pending.value || window.confirm('原公告命令尚未取得收據；離開不代表伺服器取消。仍要離開嗎？'))

onBeforeUnmount(() => {
  if (pending.value) {
    if (sending.value) pending.value = { ...pending.value, unknown: true }
    emit('pending-change', pending.value.intent)
    emit('unresolved', pending.value.intent)
  }
  mounted.value = false
  generation += 1
  readSequence += 1
  postSequence += 1
  readController?.abort()
  postController?.abort()
  window.removeEventListener('beforeunload', warnBeforeUnload)
})
</script>

<template>
  <section class="round-publication" aria-labelledby="round-publication-title" :aria-busy="reading || sending">
    <h2 id="round-publication-title">輪次公告</h2>
    <p>本元件只公告賽程已提供的目前輪次與比賽開始時間（T）；不建立配對、房間或新賽事。</p>
    <p v-if="eligibilityMessage" role="status">{{ eligibilityMessage }}</p>
    <button type="button" data-testid="round-publication-refresh" :disabled="!canRead" @click="loadPublication">
      {{ reading ? '查詢中…' : '重新查詢輪次公告' }}
    </button>
    <p v-if="reading" role="status" aria-live="polite">正在讀取最新賽程與輪次公告…</p>
    <p v-if="readError" role="alert">{{ readError }}</p>
    <p v-if="targetResult?.kind === 'blocked' && !reading" role="status">{{ targetBlockMessage(targetResult) }}</p>
    <p v-if="isAutomaticInitialRound && !publication && !reading" role="status">
      首輪沿用賽事公告的比賽開始時間，由系統自動建立固定房間。
    </p>
    <p v-if="notice" role="status" aria-live="polite">{{ notice }}</p>

    <p v-if="selectedTarget && !reading" role="status">
      目前輪次：第 {{ selectedTarget.roundNumber }} 輪 · {{ selectedTarget.matchIds.length }} 間待公告房間。
    </p>
    <details v-if="selectedTarget && !publication" class="technical-identifiers">
      <summary>技術識別資料</summary>
      <p>階段識別碼：{{ selectedTarget.stageId }}</p>
      <ul aria-label="待公告交手識別碼">
        <li v-for="matchId in selectedTarget.matchIds" :key="matchId">交手識別碼：{{ matchId }}</li>
      </ul>
    </details>

    <form v-if="showAnnouncementForm" data-testid="round-announcement-form" @submit.prevent="announceRound">
      <label for="round-start-time">比賽開始時間（T）
        <input id="round-start-time" v-model="roundStartLocal" data-testid="round-start-time" type="datetime-local"
          step="60" :disabled="!canAnnounce" aria-describedby="round-start-help" @input="validationMessage = ''">
      </label>
      <small id="round-start-help">只接受本地時間 HH:mm，不含秒或毫秒；所有房間必須在 T 前至少五分鐘取得伺服器確認（ACK）。</small>
      <p v-if="timePreview" role="status">{{ competitionRoundPublicationDeadlineLabel(timePreview.timestamp) }}</p>
      <p v-if="timePreview?.cutoff !== null && timePreview?.cutoff !== undefined" role="status">
        所有房間的伺服器確認（ACK）不得晚於 T−5 分鐘：{{ formatCompetitionDeadline(timePreview.cutoff) }}。實際資格與競態由伺服器判定，本提示不會自動順延 T。
      </p>
      <p>本元件不含裁判指派編輯，送出合法空指派；裁判身分不會自動取得主辦權限。</p>
      <p v-if="validationMessage" role="alert">{{ validationMessage }}</p>
      <button type="submit" data-testid="round-announcement-submit" :disabled="!canAnnounce || !roundStartLocal">
        公告此輪
      </button>
    </form>

    <section v-if="publication" class="publication-state" aria-label="目前輪次公告狀態">
      <h3>{{ competitionRoundPublicationStatusLabel(publication.status) }}</h3>
      <p>{{ competitionRoundPublicationDeadlineLabel(publication.roundStartMs) }}</p>
      <p v-if="publicationCutoff !== null">所有固定房間須在 T−5 分鐘（{{ formatCompetitionDeadline(publicationCutoff) }}）或更早取得伺服器確認（ACK），才能依後端條件釋出。</p>
      <p>公告不代表正式開局、全員已準備，或計時已開始。</p>
      <p v-if="publication.requiresReferee">需要裁判處置；此旗標不表示裁判已指派。</p>
      <p v-if="publicationRoomWarning()" role="status">{{ publicationRoomWarning() }}</p>
      <ul aria-label="每間房間的伺服器確認狀態">
        <li v-for="(room, index) in publication.rooms" :key="room.gameId">
          房間 {{ index + 1 }} · 伺服器確認（ACK）：{{ formatRoomAck(room.ackConfirmedAtMs) }}
        </li>
      </ul>
      <details class="technical-identifiers">
        <summary>技術識別資料</summary>
        <p>階段識別碼：{{ publication.roundRef.stageId }}</p>
        <ul aria-label="公告房間識別碼">
          <li v-for="(room, index) in publication.rooms" :key="room.gameId">
            第 {{ index + 1 }} 間房間 · 交手識別碼：{{ room.matchId }} · 房間識別碼：{{ room.gameId }}
          </li>
        </ul>
      </details>
      <p>既有公告目前沒有安全撤銷或重新公告流程；請勿以新命令取代舊公告。</p>
    </section>

    <section v-if="pending" class="pending-note" role="status" aria-live="polite">
      <p>原公告命令仍未確認。重新查詢顯示相似或相同結果都不是送出收據；不會自動更換命令識別碼、更新版本或改寫 T。</p>
      <p>切換賽事／帳號或離開不代表伺服器取消。只要仍是原賽事與原主辦者身分，便可在此手動重試原命令；不會依目前輪次改寫命令。</p>
      <button v-if="samePendingIdentity" type="button" data-testid="round-same-command-retry" :disabled="!canRetry" @click="sendPending">
        {{ sending ? '等待原命令收據…' : '以相同命令手動重試' }}
      </button>
    </section>
    <p v-if="mutationMessage" role="status" aria-live="polite">{{ mutationMessage }}</p>
  </section>
</template>

<style scoped>
.round-publication { display: grid; gap: .75rem; min-width: 0; border: 1px solid var(--app-border); border-radius: 16px; padding: 1rem; background: var(--app-surface); }
h2, h3, p { margin: 0; }
h2, h3 { font-family: var(--font-serif); }
p, small, li { font-size: .875rem; line-height: 1.65; overflow-wrap: anywhere; }
label { display: block; margin-block: .5rem; }
input { display: block; width: 100%; min-height: 2.75rem; margin-top: .35rem; padding: .5rem; color: var(--app-text); background: var(--app-input); border: 1px solid var(--app-border); border-radius: 12px; }
button { max-width: 100%; min-height: 2.75rem; padding: .5rem .75rem; color: var(--app-text); background: var(--app-control); border: 1px solid var(--app-border); border-radius: 12px; white-space: normal; }
button:disabled { opacity: .65; }
.publication-state, .pending-note { display: grid; gap: .625rem; border: 1px solid var(--app-border); border-radius: .5rem; padding: .875rem; }
.pending-note { border-color: var(--app-accent-strong); }
ul { margin: 0; padding-inline-start: 1.25rem; }
.round-publication { border: 1px solid var(--ui-glass-stroke); border-radius: 22px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); -webkit-backdrop-filter: blur(18px); backdrop-filter: blur(18px); }
.round-publication :is(input, select, textarea, button):focus-visible { outline: 2px solid var(--app-accent); outline-offset: 3px; }
</style>
