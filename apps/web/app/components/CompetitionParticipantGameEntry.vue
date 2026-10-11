<template>
  <section class="participant-game-entry" aria-labelledby="participant-game-entry-title">
    <div class="entry-heading">
      <div>
        <p class="eyebrow">固定參賽者唯讀查詢</p>
        <h2 id="participant-game-entry-title">進入目前競賽對局</h2>
      </div>
    </div>
    <p class="explanation">輸入主辦方提供的交手 ID，查詢你目前可加入的當局。查詢不會加入房間，也不會變更賽事或對局狀態。</p>

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
    <p v-else-if="lookupCompleted && !game" class="state-note" role="status" aria-live="polite">
      目前沒有可加入當局，請稍後重新查詢或向主辦方確認。
    </p>
    <div v-else-if="game && roomPath" class="current-game" role="status" aria-live="polite">
      <p>查詢到目前可加入的第 {{ game.gameNumber }} 局。</p>
      <NuxtLink class="primary-button room-link" :to="roomPath">進入競賽對局</NuxtLink>
    </div>
  </section>
</template>

<script setup lang="ts">
import {
  competitionParticipantGameErrorMessage,
  competitionParticipantRoomPath,
  normalizeCompetitionParticipantGameId,
  parseCompetitionParticipantGameResponse,
  type CompetitionParticipantGame,
} from '~/lib/competition-participant-game'

const props = withDefaults(defineProps<{
  competitionId: string
  matchId: string
  sessionUserId: string | null
  enabled: boolean
  editable?: boolean
}>(), {
  editable: false,
})

const emit = defineEmits<{
  'update:matchId': [value: string]
}>()

const matchIdInputId = useId()
const game = ref<CompetitionParticipantGame | null>(null)
const loading = ref(false)
const lookupCompleted = ref(false)
const loadError = ref('')
const mounted = ref(false)
let requestSequence = 0
let activeRequest: AbortController | null = null

const normalizedCompetitionId = computed(() => normalizeCompetitionParticipantGameId(props.competitionId))
const normalizedMatchId = computed(() => normalizeCompetitionParticipantGameId(props.matchId))
const roomPath = computed(() => competitionParticipantRoomPath(game.value))
const canLookup = computed(() => props.enabled
  && Boolean(props.sessionUserId?.trim())
  && Boolean(normalizedCompetitionId.value)
  && Boolean(normalizedMatchId.value)
  && !loading.value)

function updateMatchId(event: Event) {
  const input = event.currentTarget
  if (input instanceof HTMLInputElement) emit('update:matchId', input.value)
}

function clearCurrentLookup() {
  requestSequence += 1
  activeRequest?.abort()
  activeRequest = null
  game.value = null
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
  loadError.value = ''

  try {
    const response: unknown = await $fetch(
      `/api/competitions/${encodeURIComponent(competitionId)}/matches/${encodeURIComponent(matchId)}/participant-game`,
      { method: 'GET', signal: controller.signal },
    )
    if (!isCurrentRequest(sequence, identity)) return
    const projection = parseCompetitionParticipantGameResponse(response, competitionId, matchId)
    game.value = projection.game
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
    }
  }
}

function requestIdentity(): string {
  return JSON.stringify([
    normalizedCompetitionId.value,
    normalizedMatchId.value,
    props.sessionUserId,
    props.enabled,
  ])
}

function isCurrentRequest(sequence: number, identity: string): boolean {
  return sequence === requestSequence && identity === requestIdentity()
}

watch(
  () => [props.competitionId, props.matchId, props.sessionUserId, props.enabled] as const,
  () => {
    clearCurrentLookup()
    if (mounted.value && !props.editable) void lookup()
  },
  { flush: 'sync' },
)

watch(() => props.editable, (editable) => {
  if (!editable && mounted.value) void lookup()
})

onMounted(() => {
  mounted.value = true
  if (!props.editable) void lookup()
})

onBeforeUnmount(() => {
  clearCurrentLookup()
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
  border-radius: 9px;
  padding: 0 .875rem;
  background: var(--app-input);
  color: var(--app-text);
}
.hint { font-size: .75rem; }
.identifier { overflow-wrap: anywhere; font-family: ui-monospace, SFMono-Regular, monospace; }
.load-error { display: grid; justify-items: start; gap: .75rem; border-left: 3px solid var(--app-danger); padding-left: .875rem; }
.current-game { display: flex; flex-wrap: wrap; align-items: center; gap: 1rem; }
.room-link { text-decoration: none; }

@media (max-width: 600px) {
  .participant-game-entry { padding: 1rem; }
  .lookup-form > button, .room-link { width: 100%; }
  .current-game { align-items: stretch; }
}
</style>
