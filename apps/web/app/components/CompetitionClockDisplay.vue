<template>
  <section class="competition-clock" :aria-labelledby="headingId">
    <header class="clock-header">
      <div>
        <h2 :id="headingId">競賽時計</h2>
        <p class="clock-note">數字來自伺服器權威取樣；本頁不自行扣時或判定終局。</p>
      </div>
      <button class="clock-refresh" type="button" :disabled="!activeKey || loading" @keydown.stop @click="readClock">
        {{ loading ? '查詢中…' : errorMessage ? '重試時計查詢' : '重新查詢時計' }}
      </button>
    </header>

    <p v-if="!scope" class="clock-note" role="status">{{ eligibilityMessage }}</p>
    <p v-else-if="!visible" class="clock-note">頁面隱藏時暫停時計查詢；重新顯示後立即取樣。</p>
    <p v-else-if="errorMessage" class="clock-error" role="alert">{{ errorMessage }}</p>
    <p v-else-if="loading && !projection" class="clock-note" role="status">正在取得權威時計…</p>

    <template v-if="projection">
      <p v-if="projection.game" class="clock-note clock-sampling-note">
        {{ loading ? '正在更新；數字停留在上次權威取樣，不代表即時剩餘時間。' : '目前顯示最近一次權威取樣；查詢完成後才更新數字。' }}
      </p>
      <p v-if="projection.matchStatus === 'completed'" class="clock-result" role="status">
        交手已完成；此查詢不改變房間的正式終局狀態。
      </p>
      <p v-else-if="!projection.game" class="clock-result" role="status">
        目前沒有可顯示的當局時計，請稍後重新查詢或向主辦方確認。
      </p>
      <template v-else>
        <p v-if="projection.clock.awaitingAuthority" class="clock-authority" role="status">
          時計正在等待伺服器權威確認；顯示數字不代表判負或正式終局。
        </p>
        <p v-if="projection.clock.paused" class="clock-phase">計時已暫停：{{ windowLabel(projection.clock.paused) }}</p>
        <p v-else-if="projection.clock.running" class="clock-phase">{{ windowLabel(projection.clock.running) }}</p>
        <p v-else class="clock-phase">目前沒有正在扣時的視窗。</p>
        <p v-if="choiceWindow" class="clock-choice">
          回合外選擇剩餘：{{ formatCompetitionClockTime(choiceWindow.remainingMs) }}
        </p>

        <div class="clock-teams" aria-live="off">
          <article v-for="team in projection.clock.teams" :key="team.team" class="clock-team">
            <h3>{{ teamLabel(team.team) }}</h3>
            <dl>
              <div><dt>主時間</dt><dd>{{ formatCompetitionClockTime(team.mainTimeRemainingMs) }}</dd></div>
              <div>
                <dt>{{ team.byoyomiActive ? '讀秒剩餘' : '讀秒額度' }}</dt>
                <dd>{{ formatCompetitionClockTime(team.byoyomiTimeRemainingMs) }}</dd>
              </div>
            </dl>
            <p class="clock-note">{{ team.byoyomiActive ? '讀秒中' : '尚未進入讀秒' }}</p>
          </article>
        </div>
        <p v-if="hasZero" class="clock-note">時間顯示為 0 不代表判負，請等待伺服器權威判定。</p>
      </template>
      <p v-if="projection.matchStatus === 'awaitingReferee' && !projection.game" class="clock-note">等待裁判確認。</p>
    </template>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, watch, onMounted, onBeforeUnmount, nextTick, useId, shallowRef } from 'vue'
import type { ClockCheckpointWindow } from '../../shared/competition'
import type { CompetitionRoomMatchClockReadValue } from '../../shared/competition-room'
import type { CompetitionRoomMode } from '../lib/competition-ready'
import {
  competitionClockErrorMessage, competitionClockPath, createCompetitionClockScope,
  formatCompetitionClockTime, parseCompetitionClockResponse,
} from '../lib/competition-clock'

const props = defineProps<{
  mode: CompetitionRoomMode
  gameId: string
  roomGameId: string | null
  sessionUserId: string | null
  memberUserId: string | null
}>()

const headingId = useId()
const mounted = ref(false)
const visible = ref(false)
const loading = ref(false)
const projection = shallowRef<CompetitionRoomMatchClockReadValue | null>(null)
const errorMessage = ref('')
let sequence = 0
let controller: AbortController | null = null
let timer: ReturnType<typeof setTimeout> | null = null

const scope = computed(() => createCompetitionClockScope(props))
const scopeKey = computed(() => scope.value ? JSON.stringify([
  scope.value.competitionId, scope.value.matchId, scope.value.gameNumber,
  scope.value.gameId, scope.value.sessionUserId,
]) : null)
const activeKey = computed(() => mounted.value && visible.value ? scopeKey.value : null)
const eligibilityMessage = computed(() => !props.sessionUserId ? '請登入後查詢競賽時計。'
  : props.mode.kind === 'invalid' ? '競賽房間資訊無法確認，未查詢時計。'
    : props.mode.kind === 'ordinary' ? '自由對局・切磋不使用競賽時計查詢。'
      : '目前無法確認本人固定參賽席位或房間，未查詢時計。')
const choiceWindow = computed(() => {
  const window = projection.value?.clock.paused ?? projection.value?.clock.running
  return window?.type === 'outOfTurnChoice' ? window : null
})
const hasZero = computed(() => projection.value?.clock.teams.some(team =>
  team.mainTimeRemainingMs === 0 || team.byoyomiTimeRemainingMs === 0,
) || choiceWindow.value?.remainingMs === 0)

/** 已驗取樣的兩隊順序只作中性顯示對照，不推測本隊／對手或取得私人隊名。 */
function teamLabel(teamId: string): string {
  const index = projection.value?.clock.teams.findIndex(team => team.team === teamId)
  return index === 0 ? '甲隊' : index === 1 ? '乙隊' : '隊伍資訊無法確認'
}

function windowLabel(window: ClockCheckpointWindow): string {
  switch (window.type) {
    case 'preparation': return `準備計時：${window.teams.map(teamLabel).join('、')}`
    case 'action': return `行動計時：${teamLabel(window.team)}，回合編號 ${window.turn}`
    case 'outOfTurnChoice': return `回合外選擇計時：${teamLabel(window.team)}`
  }
}

function clearTimer() {
  if (timer !== null) clearTimeout(timer)
  timer = null
}

function invalidate() {
  sequence += 1
  controller?.abort()
  controller = null
  clearTimer()
  loading.value = false
  projection.value = null
  errorMessage.value = ''
}

async function readClock() {
  const current = scope.value
  const key = activeKey.value
  if (!current || !key || loading.value) return
  clearTimer()
  const requestSequence = ++sequence
  const requestController = new AbortController()
  controller = requestController
  loading.value = true
  errorMessage.value = ''
  let succeeded = false
  const stillCurrent = () => requestSequence === sequence && activeKey.value === key && !requestController.signal.aborted
  try {
    const response: unknown = await $fetch(competitionClockPath(current), {
      method: 'GET', signal: requestController.signal, retry: 0,
    })
    if (!stillCurrent()) return
    projection.value = parseCompetitionClockResponse(response, current)
    succeeded = true
  } catch (error) {
    if (!stillCurrent()) return
    projection.value = null
    errorMessage.value = competitionClockErrorMessage(error)
  } finally {
    // 舊 request 的 finally 不能解鎖新查詢或重新啟動舊 scope 的 timer。
    if (stillCurrent()) {
      loading.value = false
      controller = null
      if (succeeded && projection.value?.matchStatus !== 'completed') {
        timer = setTimeout(() => { timer = null; void readClock() }, 1000)
      }
    }
  }
}

watch(activeKey, (key) => {
  invalidate()
  if (!key) return
  const scheduledSequence = sequence
  loading.value = true
  // 合併同批 props 更新才查詢；排程期間仍停用按鈕，不讓半套 context 發出多筆 GET。
  void nextTick(() => {
    if (scheduledSequence !== sequence || activeKey.value !== key) return
    loading.value = false
    void readClock()
  })
}, { flush: 'sync' })

function updateVisibility() { visible.value = document.visibilityState === 'visible' }

onMounted(() => {
  updateVisibility()
  document.addEventListener('visibilitychange', updateVisibility)
  mounted.value = true
})

onBeforeUnmount(() => {
  mounted.value = false
  invalidate()
  document.removeEventListener('visibilitychange', updateVisibility)
})
</script>

<style scoped>
.competition-clock { min-width: 0; max-width: 100%; padding: .75rem; border: 1px solid var(--app-border); border-radius: .75rem; background: var(--app-surface); }
.clock-header { display: flex; align-items: flex-start; justify-content: space-between; gap: .5rem; flex-wrap: wrap; }
.clock-header > div { min-width: 0; flex: 1 1 10rem; }
.clock-header h2, .clock-team h3 { margin: 0; overflow-wrap: anywhere; }
.clock-header h2 { font-size: 1rem; }
.clock-note { margin: .25rem 0; color: var(--app-text-muted); font-size: .75rem; line-height: 1.4; overflow-wrap: anywhere; }
.clock-sampling-note { min-height: 2.8em; }
.clock-refresh { padding: .5rem .75rem; border: 1px solid var(--app-border); border-radius: .5rem; color: inherit; background: var(--app-surface); font: inherit; cursor: pointer; }
.clock-refresh:disabled { opacity: .55; cursor: not-allowed; }
.clock-refresh:focus-visible { outline: 2px solid var(--app-accent-strong); outline-offset: 3px; }
.clock-error { color: var(--app-danger); overflow-wrap: anywhere; }
.clock-phase, .clock-choice, .clock-result, .clock-authority { margin: .5rem 0; overflow-wrap: anywhere; font-size: .875rem; }
.clock-teams { display: grid; grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .5rem; }
.clock-team { min-width: 0; padding: .5rem; border: 1px solid var(--app-border); border-radius: .5rem; }
.clock-team h3 { font-size: .875rem; }
.clock-team dl { margin: .5rem 0; font-size: .875rem; }
.clock-team dl > div { display: flex; flex-wrap: wrap; justify-content: space-between; gap: .25rem .75rem; }
.clock-team dt { color: var(--app-text-muted); }
.clock-team dd { margin: 0; font-variant-numeric: tabular-nums; overflow-wrap: anywhere; max-width: 100%; }
@media (max-width: 600px) { .competition-clock { padding: .5rem; } .clock-refresh { padding: .375rem .5rem; font-size: .75rem; } }
@media (max-width: 340px) { .clock-teams { grid-template-columns: minmax(0, 1fr); } }
.competition-clock { border: 1px solid var(--ui-glass-stroke); border-radius: 22px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); -webkit-backdrop-filter: blur(18px); backdrop-filter: blur(18px); }
.competition-clock :is(input, select, textarea, button):focus-visible { outline: 2px solid var(--app-accent); outline-offset: 3px; }
</style>
