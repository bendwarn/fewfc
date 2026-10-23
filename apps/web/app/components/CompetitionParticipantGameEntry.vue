<template>
  <section class="participant-game-entry" aria-labelledby="participant-game-entry-title">
    <div class="entry-heading">
      <div>
        <p class="eyebrow">固定參賽者對局入口</p>
        <h2 id="participant-game-entry-title">進入目前競賽對局</h2>
      </div>
    </div>
    <p class="explanation">此入口依你的固定隊伍與目前賽程查詢可加入的當局；敗方隊長可在下方選擇普通下一局先手。</p>

    <form v-if="editable" class="lookup-form" @submit.prevent="lookup">
      <label :for="matchIdInputId">
        交手 ID
        <input
          :id="matchIdInputId"
          class="text-input"
          type="text"
          autocomplete="off"
          :value="matchId"
          :disabled="!enabled || loading"
          :aria-describedby="`${matchIdInputId}-hint`"
          @input="updateMatchId"
        >
      </label>
      <p :id="`${matchIdInputId}-hint`" class="hint">交手 ID 由主辦方提供；此處不會列舉或猜測賽程。</p>
      <button class="primary-button" type="submit" :disabled="!canLookup">
        {{ loading ? '查詢中…' : '查詢目前對局' }}
      </button>
    </form>

    <p v-else-if="matchId" class="match-reference">交手 ID：<span class="identifier">{{ normalizedMatchId }}</span></p>

    <section v-if="samePendingScope && pendingCommand" class="pending-command" role="status" aria-live="polite">
      <p>{{ pendingUnknown
        ? '先手選擇的結果尚未確認；重試會沿用原命令與局號。'
        : '正在以同一命令確認先手選擇與下一局房間。' }}</p>
      <button class="secondary-button" type="button" :disabled="sending || !enabled" @click="sendPending">
        {{ sending ? '正在確認…' : '以同一命令重試' }}
      </button>
    </section>
    <p v-if="mutationError" class="load-error" role="alert" aria-live="assertive">{{ mutationError }}</p>
    <p v-if="notice" class="state-note" role="status" aria-live="polite">{{ notice }}</p>

    <p v-if="!enabled || !sessionUserId" class="state-note" role="status">
      登入且屬於此賽事有效參賽隊伍後，才能查詢目前對局。
    </p>
    <p v-else-if="loading" class="state-note" role="status" aria-live="polite">
      正在安全查詢目前對局…
    </p>
    <div v-else-if="loadError" class="load-error" role="alert" aria-live="assertive">
      <p>{{ loadError }}</p>
      <button class="ghost-button" type="button" :disabled="!canLookup" @click="lookup">重新查詢</button>
    </div>
    <form v-else-if="firstPlayerChoice && !pendingCommand" class="first-player-form" @submit.prevent="submitFirstPlayerChoice">
      <fieldset class="first-player-options" :disabled="sending">
        <legend>第 {{ firstPlayerChoice.gameNumber }} 局：選擇先手隊伍</legend>
        <label v-for="(teamId, index) in firstPlayerChoice.teams" :key="teamId">
          <input v-model="selectedFirstPlayerTeam" type="radio" name="next-game-first-player" :value="teamId">
          <span>{{ index === 0 ? '甲隊' : '乙隊' }}・<span class="identifier">{{ teamId }}</span></span>
        </label>
      </fieldset>
      <p class="hint">確認後會建立普通下一局，並依公告重置主時間。</p>
      <button class="primary-button" type="submit" :disabled="!canAssignFirstPlayer">
        {{ sending ? '正在建立下一局…' : '確認先手並建立下一局' }}
      </button>
    </form>
    <p v-else-if="lookupCompleted && waitingForCaptain && !firstPlayerChoice" class="state-note" role="status" aria-live="polite">
      已結清上一局；等待敗方隊長選擇下一局先手。
      <button class="ghost-button" type="button" :disabled="!canLookup" @click="lookup">重新查詢</button>
    </p>
    <p v-else-if="lookupCompleted && !game" class="state-note" role="status" aria-live="polite">
      房間尚未就緒，賽事頁會自動重新查詢；也可以立即重試。
      <button class="ghost-button" type="button" :disabled="!canLookup" @click="lookup">立即重試</button>
    </p>
    <div v-else-if="game && roomPath" class="current-game" role="status" aria-live="polite">
      <p>查詢到目前可加入的第 {{ game.gameNumber }} 局。</p>
      <NuxtLink class="primary-button room-link" :to="roomPath">進入競賽對局</NuxtLink>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch, shallowRef, onMounted, onBeforeUnmount, useId } from 'vue'
import {
  competitionParticipantGameErrorMessage,
  competitionParticipantRoomPath,
  normalizeCompetitionParticipantGameId,
  parseCompetitionParticipantGameResponse,
  type CompetitionParticipantGame,
} from '~/lib/competition-participant-game'
import {
  captainNextGameChoice,
  isCaptainNextGameCommand,
  parseCaptainNextGameAcknowledgement,
  type CaptainNextGameChoice,
  type CaptainNextGameCommand,
} from '~/lib/competition-next-game'
import {
  createCompetitionCorrectionScope,
  parseCompetitionCorrectionMatch,
} from '~/lib/competition-ruling-correction'

const props = withDefaults(defineProps<{
  competitionId: string
  matchId: string
  sessionUserId: string | null
  participantTeamId: string | null
  participantIsCaptain?: boolean
  enabled: boolean
  editable?: boolean
}>(), {
  participantIsCaptain: false,
  editable: false,
})

const emit = defineEmits<{
  'update:matchId': [value: string]
}>()

const matchIdInputId = useId()
const game = ref<CompetitionParticipantGame | null>(null)
const firstPlayerChoice = shallowRef<CaptainNextGameChoice | null>(null)
const waitingForCaptain = ref(false)
const selectedFirstPlayerTeam = ref('')
const pendingCommand = shallowRef<CaptainNextGameCommand | null>(null)
const pendingUnknown = ref(false)
const mutationError = ref('')
const notice = ref('')
const sending = ref(false)
const loading = ref(false)
const lookupCompleted = ref(false)
const loadError = ref('')
const mounted = ref(false)
let requestSequence = 0
let sendSequence = 0
let activeRequest: AbortController | null = null
let sendController: AbortController | null = null
let retryTimer: ReturnType<typeof setTimeout> | null = null

const normalizedCompetitionId = computed(() => normalizeCompetitionParticipantGameId(props.competitionId))
const normalizedMatchId = computed(() => normalizeCompetitionParticipantGameId(props.matchId))
const roomPath = computed(() => competitionParticipantRoomPath(game.value))
const scopeKey = computed(() => JSON.stringify([
  normalizedCompetitionId.value,
  normalizedMatchId.value,
  props.sessionUserId,
]))
const pendingScopeKey = shallowRef('')
const samePendingScope = computed(() => Boolean(pendingCommand.value && pendingScopeKey.value === scopeKey.value))
const canLookup = computed(() => props.enabled
  && Boolean(props.sessionUserId?.trim())
  && Boolean(normalizedCompetitionId.value)
  && Boolean(normalizedMatchId.value)
  && !loading.value && !sending.value)
const canAssignFirstPlayer = computed(() => Boolean(firstPlayerChoice.value
  && selectedFirstPlayerTeam.value
  && firstPlayerChoice.value.teams.includes(selectedFirstPlayerTeam.value)
  && firstPlayerChoice.value.matchRevision < Number.MAX_SAFE_INTEGER
  && !sending.value && !pendingCommand.value && props.enabled && props.participantIsCaptain))

function commandStorageKey(): string {
  return 'fewfc:competition-next-game:' + scopeKey.value
}

function restorePendingCommand() {
  pendingCommand.value = null
  pendingUnknown.value = false
  pendingScopeKey.value = ''
  if (!import.meta.client || !normalizedCompetitionId.value || !normalizedMatchId.value || !props.sessionUserId) return
  try {
    const stored = sessionStorage.getItem(commandStorageKey())
    if (stored === null) return
    const parsed: unknown = JSON.parse(stored)
    if (!isCaptainNextGameCommand(parsed)) {
      sessionStorage.removeItem(commandStorageKey())
      return
    }
    pendingCommand.value = parsed
    pendingScopeKey.value = scopeKey.value
    pendingUnknown.value = true
  } catch {
    try { sessionStorage.removeItem(commandStorageKey()) } catch {}
  }
}

function savePendingCommand(command: CaptainNextGameCommand) {
  pendingCommand.value = command
  pendingScopeKey.value = scopeKey.value
  pendingUnknown.value = false
  if (import.meta.client) {
    try { sessionStorage.setItem(commandStorageKey(), JSON.stringify(command)) } catch {}
  }
}

function clearPendingCommand(command: CaptainNextGameCommand) {
  if (pendingCommand.value !== command) return
  if (import.meta.client) {
    try { sessionStorage.removeItem(commandStorageKey()) } catch {}
  }
  pendingCommand.value = null
  pendingScopeKey.value = ''
  pendingUnknown.value = false
}

function updateMatchId(event: Event) {
  const input = event.currentTarget
  if (input instanceof HTMLInputElement) emit('update:matchId', input.value)
}

function clearCurrentLookup() {
  requestSequence += 1
  activeRequest?.abort()
  activeRequest = null
  if (retryTimer !== null) clearTimeout(retryTimer)
  retryTimer = null
  game.value = null
  firstPlayerChoice.value = null
  waitingForCaptain.value = false
  loading.value = false
  lookupCompleted.value = false
  loadError.value = ''
}

async function lookup() {
  if (!canLookup.value) return
  const competitionId = normalizedCompetitionId.value
  const matchId = normalizedMatchId.value
  if (!competitionId || !matchId) return

  activeRequest?.abort()
  const controller = new AbortController()
  activeRequest = controller
  const sequence = ++requestSequence
  const identity = requestIdentity()
  loading.value = true
  lookupCompleted.value = false
  game.value = null
  firstPlayerChoice.value = null
  waitingForCaptain.value = false
  selectedFirstPlayerTeam.value = ''
  loadError.value = ''
  mutationError.value = ''
  notice.value = ''

  try {
    const response: unknown = await $fetch(
      `/api/competitions/${encodeURIComponent(competitionId)}/matches/${encodeURIComponent(matchId)}/participant-game`,
      { method: 'GET', signal: controller.signal },
    )
    if (!isCurrentRequest(sequence, identity)) return
    const projection = parseCompetitionParticipantGameResponse(response, competitionId, matchId)
    game.value = projection.game
    if (projection.game && samePendingScope.value && pendingCommand.value) {
      clearPendingCommand(pendingCommand.value)
    }
    if (!projection.game && props.participantTeamId && props.sessionUserId) {
      const scope = createCompetitionCorrectionScope(competitionId, matchId, props.sessionUserId)
      if (scope) {
        try {
          const matchResponse: unknown = await $fetch(
            '/api/competitions/' + encodeURIComponent(competitionId) + '/matches/' + encodeURIComponent(matchId),
            { method: 'GET', signal: controller.signal, retry: 0 },
          )
          if (!isCurrentRequest(sequence, identity)) return
          const matchView = parseCompetitionCorrectionMatch(matchResponse, scope)
          waitingForCaptain.value = matchView.match?.status === 'awaitingFirstPlayer'
          firstPlayerChoice.value = captainNextGameChoice(matchView, props.participantTeamId, props.participantIsCaptain)
        } catch {
          // 賽程／交手讀取失敗時只隱藏選擇；實際權限仍由提交 API 驗證。
          firstPlayerChoice.value = null
        }
      }
    }
    lookupCompleted.value = true
  } catch (error) {
    if (!isCurrentRequest(sequence, identity)) return
    loadError.value = error instanceof Error && error.name === 'CompetitionParticipantGameProtocolError'
      ? '競賽房間回應格式不正確，請稍後重試。'
      : competitionParticipantGameErrorMessage(error)
  } finally {
    if (isCurrentRequest(sequence, identity)) {
      loading.value = false
      activeRequest = null
      if (!props.editable && (loadError.value || (lookupCompleted.value && !game.value && !firstPlayerChoice.value))) scheduleLookupRetry()
    }
  }
}

async function submitFirstPlayerChoice() {
  const choice = firstPlayerChoice.value
  if (!canAssignFirstPlayer.value || !choice || !selectedFirstPlayerTeam.value) return
  const command: CaptainNextGameCommand = {
    selectedFirstPlayerTeam: selectedFirstPlayerTeam.value,
    expectedMatchRevision: choice.matchRevision,
    commandId: crypto.randomUUID(),
  }
  savePendingCommand(command)
  await sendPending()
}

async function sendPending() {
  const command = pendingCommand.value
  const competitionId = normalizedCompetitionId.value
  const matchId = normalizedMatchId.value
  if (!command || !samePendingScope.value || !competitionId || !matchId
    || !props.enabled || sending.value) return
  const sequence = ++sendSequence
  const identity = requestIdentity()
  const controller = new AbortController()
  sendController?.abort()
  sendController = controller
  sending.value = true
  mutationError.value = ''
  notice.value = ''
  let refresh = false
  try {
    const response: unknown = await $fetch(
      '/api/competitions/' + encodeURIComponent(competitionId) + '/matches/' + encodeURIComponent(matchId) + '/next-game',
      { method: 'POST', body: command, retry: 0, signal: controller.signal },
    )
    if (!isCurrentSend(sequence, identity, command)) return
    const acknowledgement = parseCaptainNextGameAcknowledgement(response, competitionId, matchId, command)
    if (acknowledgement.status === 'blocked') {
      pendingUnknown.value = true
      mutationError.value = '先手已保存，但下一局房間尚未建立；可用同一命令重試。'
      refresh = false
    } else {
      clearPendingCommand(command)
      notice.value = acknowledgement.status === 'provisioned'
        ? '下一局房間已就緒，正在更新入口。'
        : '先手選擇已保存，系統正在建立下一局房間。'
      refresh = true
    }
  } catch (error) {
    if (!isCurrentSend(sequence, identity, command)) return
    const message = competitionParticipantGameErrorMessage(error)
    if (keepNextGameCommand(error)) {
      mutationError.value = message
      pendingUnknown.value = true
    } else {
      clearPendingCommand(command)
      notice.value = message
      refresh = true
    }
  } finally {
    if (sequence === sendSequence && identity === requestIdentity()) {
      sending.value = false
      sendController = null
    }
  }
  if (refresh && identity === requestIdentity() && props.enabled) await lookup()
}

function scheduleLookupRetry() {
  if (retryTimer !== null || !props.enabled) return
  retryTimer = setTimeout(() => {
    retryTimer = null
    void lookup()
  }, loadError.value ? 15_000 : 5_000)
}

function requestIdentity(): string {
  return JSON.stringify([
    normalizedCompetitionId.value,
    normalizedMatchId.value,
    props.sessionUserId,
    props.participantTeamId,
    props.participantIsCaptain,
    props.enabled,
  ])
}

function isCurrentSend(sequence: number, identity: string, command: CaptainNextGameCommand): boolean {
  return sequence === sendSequence && identity === requestIdentity()
    && pendingCommand.value === command && samePendingScope.value
}

function keepNextGameCommand(error: unknown): boolean {
  if (pendingUnknown.value) return true
  const value = typeof error === 'object' && error !== null ? error as Record<string, unknown> : null
  const status = value?.statusCode ?? value?.status
  const data = asRecord(value?.data)
  const nestedData = asRecord(data?.data)
  const response = asRecord(value?.response)
  const responseData = asRecord(response?._data)
  const responseBodyData = asRecord(responseData?.data)
  const code = data?.code ?? nestedData?.code ?? responseBodyData?.code ?? responseData?.code
  return typeof status !== 'number' || status >= 500
    || (status === 409 && (code === 'gameRoomNotProvisioned' || code === 'trustedTimeRegression'))
}

function asRecord(value: unknown): Record<string, unknown> | null {
  return typeof value === 'object' && value !== null && !Array.isArray(value)
    ? value as Record<string, unknown>
    : null
}

function isCurrentRequest(sequence: number, identity: string): boolean {
  return sequence === requestSequence && identity === requestIdentity()
}

watch(
  () => [props.competitionId, props.matchId, props.sessionUserId, props.participantTeamId,
    props.participantIsCaptain, props.enabled] as const,
  () => {
    clearCurrentLookup()
    sendSequence += 1
    sendController?.abort()
    sendController = null
    sending.value = false
    restorePendingCommand()
    if (mounted.value && !props.editable) void lookup()
  },
  { flush: 'sync' },
)

watch(() => props.editable, (editable) => {
  if (!editable && mounted.value) void lookup()
})

onMounted(() => {
  mounted.value = true
  restorePendingCommand()
  if (!props.editable) void lookup()
})

onBeforeUnmount(() => {
  clearCurrentLookup()
  sendSequence += 1
  sendController?.abort()
  sendController = null
})
</script>

<style scoped>
.participant-game-entry {
  display: grid;
  gap: 1rem;
  border: 1px solid var(--app-border);
  border-radius: 16px;
  padding: 1.25rem;
  background: var(--app-surface);
  box-shadow: var(--app-shadow-md);
}

.entry-heading h2 { margin: 0; font-family: var(--font-serif); font-size: 1.25rem; }
.eyebrow { margin: 0 0 .4rem; color: var(--app-accent-strong); font-size: .75rem; letter-spacing: .16em; }
.explanation, .hint, .state-note, .match-reference, .current-game p, .load-error p {
  margin: 0;
  color: var(--app-text-muted);
  font-size: .875rem;
  line-height: 1.6;
}
.lookup-form { display: grid; justify-items: start; gap: .75rem; }
.lookup-form label { display: grid; width: 100%; max-width: 34rem; gap: .5rem; font-size: .875rem; }
.text-input {
  min-height: 3rem;
  width: 100%;
  border: 1px solid var(--app-border);
  border-radius: 12px;
  padding: 0 .875rem;
  background: var(--app-input);
  color: var(--app-text);
}
.hint { font-size: .75rem; }
.identifier { overflow-wrap: anywhere; font-family: ui-monospace, SFMono-Regular, monospace; }
.load-error { display: grid; justify-items: start; gap: .75rem; border-left: 3px solid var(--app-danger); padding-left: .875rem; }
.current-game { display: flex; flex-wrap: wrap; align-items: center; gap: 1rem; }
.pending-command { display: flex; flex-wrap: wrap; align-items: center; gap: .75rem; border-left: 3px solid var(--app-accent-strong); padding-left: .875rem; }
.pending-command p { margin: 0; color: var(--app-text-muted); font-size: .875rem; line-height: 1.6; }
.first-player-form { display: grid; justify-items: start; gap: .75rem; }
.first-player-options { display: grid; gap: .65rem; margin: 0; border: 1px solid var(--app-border); border-radius: 10px; padding: .9rem 1rem; }
.first-player-options legend { padding: 0 .3rem; font-weight: 650; }
.first-player-options label { display: flex; align-items: center; gap: .6rem; }
.first-player-options input { accent-color: var(--app-accent-strong); }
.room-link { text-decoration: none; }

@media (max-width: 600px) {
  .participant-game-entry { padding: 1rem; }
  .lookup-form > button, .room-link { width: 100%; }
  .current-game { align-items: stretch; }
}
.participant-game-entry { border: 1px solid var(--ui-glass-stroke); border-radius: 22px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); -webkit-backdrop-filter: blur(18px); backdrop-filter: blur(18px); }
.participant-game-entry :is(input, select, textarea, button):focus-visible { outline: 2px solid var(--app-accent); outline-offset: 3px; }
</style>
