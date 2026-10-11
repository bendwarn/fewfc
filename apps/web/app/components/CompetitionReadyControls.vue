<template>
  <section class="competition-ready" aria-labelledby="competition-ready-title">
    <h2 id="competition-ready-title">競賽對局準備</h2>

    <template v-if="mode.kind === 'invalid'">
      <p class="state-note" role="alert">競賽房間資訊無法確認，請重新查詢房間。</p>
      <button class="ghost-button" type="button" :disabled="loading" @click="emit('refresh')">
        重新查詢房間
      </button>
    </template>

    <p v-else-if="mode.kind === 'ordinary'" class="state-note" role="status">
      此準備控制僅適用於伺服器確認的競賽房間。
    </p>

    <template v-else>
      <p class="explanation">
        所有固定參賽者都要各自明示準備，隊長與其他隊員使用相同流程。準備狀態以房間伺服器更新為準。
      </p>
      <p class="current-status" role="status" aria-live="polite">
        目前狀態：{{ memberReady ? '已準備' : '尚未準備' }}
      </p>

      <p v-if="!sessionUserId" class="state-note" role="status">請登入後重新查詢房間。</p>
      <p v-else-if="roomStatus !== 'Waiting'" class="state-note" role="status">
        目前不在競賽等待階段，準備操作已關閉。
      </p>
      <p v-else-if="!memberConnected || !socketConnected" class="state-note" role="status">
        房間連線中斷，重新連線後才能變更準備狀態。
      </p>

      <p v-if="mutationError" class="form-error" role="alert" aria-live="assertive">
        {{ mutationError }}
      </p>
      <p v-else-if="pendingDesired !== null && acknowledged" class="state-note" role="status" aria-live="polite">
        已收到伺服器回覆，正在同步房間狀態；在確認前不會改送相反狀態。
      </p>

      <div v-if="roomStatus === 'Waiting'" class="ready-actions">
        <button
          class="primary-button ready-action"
          type="button"
          :disabled="!canOperate || pendingDesired !== null || sending"
          @click="submitDesired(!memberReady)"
        >
          {{ memberReady ? '明示取消準備' : '明示已準備' }}
        </button>
        <button
          v-if="pendingDesired !== null && retryable"
          class="ghost-button same-state-retry"
          type="button"
          :disabled="!canOperate || sending"
          @click="submitDesired(pendingDesired)"
        >
          以相同狀態重試
        </button>
        <button
          v-else-if="pendingDesired !== null && acknowledged"
          class="ghost-button refresh-status"
          type="button"
          :disabled="!canOperate || loading || sending"
          @click="emit('refresh')"
        >
          重新查詢目前狀態
        </button>
        <p v-if="sending" class="state-note" role="status" aria-live="polite">正在送出明示準備狀態…</p>
      </div>
    </template>
  </section>
</template>

<script setup lang="ts">
import { canSetCompetitionReady, competitionReadyErrorMessage, isExactCompetitionReadyAck, type CompetitionRoomMode } from '~/lib/competition-ready'

const props = defineProps<{
  gameId: string
  mode: CompetitionRoomMode
  sessionUserId: string | null
  roomStatus: string | null
  memberUserId: string | null
  memberReady: boolean
  memberConnected: boolean
  socketConnected: boolean
  loading: boolean
}>()

const emit = defineEmits<{
  refresh: []
}>()

const pendingDesired = ref<boolean | null>(null)
const sending = ref(false)
const retryable = ref(false)
const acknowledged = ref(false)
const mutationError = ref('')
let requestSequence = 0
let activeRequest: AbortController | null = null

const canOperate = computed(() => canSetCompetitionReady({
  mode: props.mode,
  roomStatus: props.roomStatus,
  sessionUserId: props.sessionUserId,
  memberUserId: props.memberUserId,
  memberConnected: props.memberConnected,
  socketConnected: props.socketConnected,
  loading: props.loading,
}) && Boolean(props.gameId.trim()))

function scopeIdentity(): string {
  const context = props.mode.kind === 'competition' ? props.mode.context : null
  return JSON.stringify([
    props.gameId,
    props.mode.kind,
    context?.competitionId ?? null,
    context?.matchId ?? null,
    context?.gameNumber ?? null,
    props.sessionUserId,
    props.memberUserId,
    props.roomStatus === 'Active',
  ])
}

function requestIdentity(): string {
  return JSON.stringify([
    scopeIdentity(),
    props.roomStatus,
    props.memberConnected,
    props.socketConnected,
    props.loading,
  ])
}

function isCurrentRequest(sequence: number, identity: string): boolean {
  return sequence === requestSequence && identity === requestIdentity()
}

function invalidateActiveRequest() {
  requestSequence += 1
  activeRequest?.abort()
  activeRequest = null
  sending.value = false
  if (pendingDesired.value !== null && !acknowledged.value) retryable.value = true
}

function clearPendingAction() {
  invalidateActiveRequest()
  pendingDesired.value = null
  retryable.value = false
  acknowledged.value = false
  mutationError.value = ''
}

watch(scopeIdentity, clearPendingAction, { flush: 'sync' })
watch(requestIdentity, invalidateActiveRequest, { flush: 'sync' })
watch(() => props.memberReady, (ready) => {
  if (pendingDesired.value !== null && ready === pendingDesired.value) clearPendingAction()
}, { flush: 'sync' })
onBeforeUnmount(clearPendingAction)

async function submitDesired(desired: boolean) {
  if (!canOperate.value || sending.value) return
  if (pendingDesired.value !== null) {
    if (!retryable.value || pendingDesired.value !== desired) return
  } else {
    if (desired === props.memberReady) return
    pendingDesired.value = desired
  }

  const identity = requestIdentity()
  const sequence = ++requestSequence
  const controller = new AbortController()
  activeRequest?.abort()
  activeRequest = controller
  sending.value = true
  retryable.value = false
  acknowledged.value = false
  mutationError.value = ''
  let shouldRefresh = false

  try {
    const response: unknown = await $fetch(
      `/api/games/${encodeURIComponent(props.gameId.trim())}/competition-ready`,
      {
        method: 'POST',
        body: { ready: desired },
        signal: controller.signal,
      },
    )
    if (!isCurrentRequest(sequence, identity)) return
    if (!isExactCompetitionReadyAck(response, desired)) {
      const error = new Error('Invalid competition-ready response')
      error.name = 'CompetitionReadyProtocolError'
      throw error
    }
    acknowledged.value = true
    shouldRefresh = true
  } catch (error) {
    if (!isCurrentRequest(sequence, identity)) return
    mutationError.value = competitionReadyErrorMessage(error)
    retryable.value = true
    shouldRefresh = true
  } finally {
    if (isCurrentRequest(sequence, identity)) {
      sending.value = false
      activeRequest = null
    }
  }

  if (shouldRefresh && isCurrentRequest(sequence, identity)) emit('refresh')
}
</script>

<style scoped>
.competition-ready {
  display: grid;
  gap: 0.75rem;
  padding: 1rem;
  border: 1px solid var(--app-border-strong);
  border-radius: 1rem;
  background: var(--app-surface);
}

.competition-ready h2,
.competition-ready p {
  margin: 0;
}

.explanation,
.state-note {
  color: var(--app-text-muted);
  line-height: 1.5;
}

.ready-actions {
  display: flex;
  flex-wrap: wrap;
  align-items: center;
  gap: 0.75rem;
}

@media (max-width: 36rem) {
  .ready-actions {
    align-items: stretch;
    flex-direction: column;
  }
}
</style>
