<template>
  <section class="panel test-clock" aria-labelledby="test-clock-title">
    <h2 id="test-clock-title">測試賽事控制</h2>
    <p>請使用賽程顯示的交手 ID；主辦可設定起手牌，或暫停、恢復已開局交手的時計。</p>
    <label for="test-clock-match">交手 ID
      <input id="test-clock-match" v-model.trim="matchId" class="text-input" autocomplete="off" :disabled="busy || Boolean(pending)">
    </label>
    <div class="test-hands">
      <h3>設定起手牌</h3>
      <p>只適用共用牌庫，必須在所有玩家準備開局前設定。依回合順序每行輸入一位玩家的牌名與等級，以逗號分隔；先手 4 張，其餘玩家各 5 張。</p>
      <button type="button" class="secondary-button" :disabled="!matchId || busy || Boolean(pending)" @click="readSeats">查詢發牌順序</button>
      <ol v-if="seats.length" aria-label="發牌順序">
        <li v-for="(seat, index) in seats" :key="seat.userId">{{ index === 0 ? '先手' : `第 ${index + 1} 位` }}：{{ seat.userId }}（隊伍 {{ seat.teamId }}）</li>
      </ol>
      <label for="test-starting-hands">各玩家起手牌
        <textarea id="test-starting-hands" v-model="handsText" class="text-input" rows="4" :disabled="busy || Boolean(pending)" />
      </label>
      <p>範例為先手兩張火牌與後手金 3；設定雙方初始生命為 4，可透過反震和金攻擊形成和局。</p>
      <button type="button" class="secondary-button" :disabled="!matchId || !handsText.trim() || busy || Boolean(pending)" @click="saveHands">設定起手牌</button>
    </div>
    <div class="clock-actions">
      <button type="button" class="secondary-button" :disabled="!matchId || busy || Boolean(pending)" @click="begin('pause')">暫停時計</button>
      <button type="button" class="secondary-button" :disabled="!matchId || busy || Boolean(pending)" @click="begin('resume')">恢復時計</button>
    </div>
    <p v-if="notice" role="status">{{ notice }}</p>
    <p v-if="error" role="alert">{{ error }}</p>
    <div v-if="pending" role="status">
      <p>命令結果尚未確認。請保留原賽事與帳號，以相同命令重試；重新整理頁面不會撤銷先前的操作。</p>
      <button type="button" class="secondary-button" :disabled="busy || !sameScope" @click="sendPending">重試原時計命令</button>
    </div>
  </section>
</template>

<script setup lang="ts">
import { computed, ref, shallowRef, watch } from 'vue'

const props = defineProps<{ competitionId: string; sessionUserId: string }>()
type Action = 'pause' | 'resume'
interface Pending {
  competitionId: string
  matchId: string
  sessionUserId: string
  body: { action: Action; reason: string; expectedMatchRevision: number; commandId: string }
}
const matchId = ref('')
const seats = ref<Array<{ userId: string; teamId: string }>>([])
const handsText = ref('火 4, 火 4, 金 1, 木 1\n金 3, 金 1, 木 1, 水 1, 土 1')
const busy = ref(false)
const notice = ref('')
const error = ref('')
const pending = shallowRef<Pending | null>(null)
const sameScope = computed(() => pending.value?.competitionId === props.competitionId
  && pending.value.sessionUserId === props.sessionUserId)
watch(() => [props.competitionId, props.sessionUserId, matchId.value], () => { seats.value = [] })

function matchPath(competitionId: string, selectedMatchId: string) {
  return `/api/competitions/${encodeURIComponent(competitionId)}/matches/${encodeURIComponent(selectedMatchId)}`
}

async function readSeats() {
  if (busy.value || pending.value || !matchId.value || !props.sessionUserId) return
  const competitionId = props.competitionId
  const sessionUserId = props.sessionUserId
  const selectedMatchId = matchId.value
  busy.value = true
  error.value = ''
  notice.value = ''
  try {
    const response = await $fetch<{ gameNumber: number; seats: Array<{ userId: string; teamId: string }> }>(
      `${matchPath(competitionId, selectedMatchId)}/test-hands`,
    )
    if (!Number.isSafeInteger(response.gameNumber) || response.gameNumber < 1
      || !Array.isArray(response.seats) || response.seats.length < 2 || response.seats.length > 4
      || response.seats.some(seat => typeof seat.userId !== 'string' || !seat.userId
        || typeof seat.teamId !== 'string' || !seat.teamId)) throw new Error('invalid seats')
    if (competitionId === props.competitionId && sessionUserId === props.sessionUserId && selectedMatchId === matchId.value) {
      seats.value = response.seats
      notice.value = `已查詢第 ${response.gameNumber} 局的發牌順序。`
    }
  } catch {
    error.value = '無法查詢發牌順序，請確認對局房間已建立及主辦身分。'
  } finally {
    busy.value = false
  }
}

async function saveHands() {
  if (busy.value || pending.value || !matchId.value || !props.sessionUserId) return
  const competitionId = props.competitionId
  const sessionUserId = props.sessionUserId
  const selectedMatchId = matchId.value
  const elements: Record<string, string> = { 金: 'metal', 木: 'wood', 水: 'water', 火: 'fire', 土: 'earth' }
  const parsed = handsText.value.trim().split(/\r?\n/).map(line => line.split(/[,，]/).map(card => {
    const match = card.trim().match(/^([金木水火土])\s*([1-5])$/)
    return match ? `${elements[match[1]!]}-${match[2]}` : null
  }))
  if (parsed.some(hand => hand.some(card => card === null))) {
    error.value = '請使用「火 4」這類牌名與等級，並以逗號分隔。'
    return
  }
  const hands = parsed as string[][]
  busy.value = true
  notice.value = ''
  error.value = ''
  try {
    const result = await $fetch<{ ok: boolean }>(`${matchPath(competitionId, selectedMatchId)}/test-hands`, {
      method: 'POST', body: { hands },
    })
    if (result.ok !== true) throw new Error('invalid receipt')
    if (competitionId === props.competitionId && sessionUserId === props.sessionUserId && selectedMatchId === matchId.value) {
      notice.value = '起手牌已設定，將在此對局開始時依回合順序發牌。'
    }
  } catch {
    error.value = '無法設定起手牌。請確認對局尚未開始、使用共用牌庫，且牌數及牌名正確。'
  } finally {
    busy.value = false
  }
}

async function begin(action: Action) {
  if (busy.value || pending.value || !matchId.value || !props.sessionUserId) return
  busy.value = true
  notice.value = ''
  error.value = ''
  const competitionId = props.competitionId
  const sessionUserId = props.sessionUserId
  const selectedMatchId = matchId.value
  try {
    const match = await $fetch<{ competitionId: string; matchId: string; matchRevision: number }>(matchPath(competitionId, selectedMatchId))
    if (competitionId !== props.competitionId || sessionUserId !== props.sessionUserId
      || selectedMatchId !== matchId.value) return
    if (match.competitionId !== competitionId || match.matchId !== selectedMatchId
      || !Number.isSafeInteger(match.matchRevision) || match.matchRevision < 1) throw new Error('invalid match')
    pending.value = { competitionId, matchId: selectedMatchId, sessionUserId, body: {
      action, reason: '測試賽事時計控制', expectedMatchRevision: match.matchRevision, commandId: crypto.randomUUID(),
    } }
  } catch {
    error.value = '無法讀取交手狀態，請確認交手 ID 與主辦身分。'
  } finally {
    busy.value = false
  }
  if (pending.value) await sendPending()
}

async function sendPending() {
  const command = pending.value
  if (!command || busy.value || !sameScope.value) return
  busy.value = true
  error.value = ''
  try {
    const result = await $fetch<{ competitionId: string; matchId: string; commandId: string; action: Action }>(
      `${matchPath(command.competitionId, command.matchId)}/clock-control`, { method: 'POST', body: command.body },
    )
    if (result.competitionId !== command.competitionId || result.matchId !== command.matchId
      || result.commandId !== command.body.commandId || result.action !== command.body.action) throw new Error('invalid receipt')
    if (pending.value !== command) return
    pending.value = null
    notice.value = command.body.action === 'pause' ? '時計已暫停。' : '時計已恢復。'
  } catch (cause) {
    const status = cause && typeof cause === 'object' && 'statusCode' in cause ? cause.statusCode : null
    if (typeof status === 'number' && [400, 403, 404, 409, 422].includes(status)) {
      pending.value = null
      error.value = '時計控制遭伺服器拒絕，請確認對局狀態後再試。'
    } else {
      error.value = '時計控制結果尚未確認，請以原命令重試。'
    }
  } finally {
    busy.value = false
  }
}
</script>

<style scoped>
@reference "../assets/css/main.css";
.test-clock { @apply grid gap-4 border border-line bg-panel p-7 max-[600px]:p-4; border-radius: 24px; }
.test-clock p { margin: 0; }
.test-clock label { display: grid; gap: .5rem; }
.clock-actions { display: flex; flex-wrap: wrap; gap: .75rem; }
.test-hands { display: grid; gap: .75rem; }
.test-hands h3 { margin: 0; }
</style>
