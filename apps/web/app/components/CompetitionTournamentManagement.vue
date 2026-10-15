<script setup lang="ts">
import type { SwissTieBreaker } from '../../shared/competition'
import type { CompetitionRegistrationState } from '../lib/competition-registration'
import {
  buildTournamentCreateIntent, createTournamentManagementContext, createTournamentManagementDraft,
  generateTournamentScheduleRandomSeed, keepTournamentCreateIntent, parseTournamentCreateAck,
  parseTournamentStageDraft, tournamentManagementValidationMessage,
  type TournamentCreateIntent, type TournamentManagementDraft, type TournamentStageSettings,
} from '../lib/competition-tournament-management'
import {
  parseCompetitionTournamentViewResponse, presentCompetitionTournamentViewError,
  swissTieBreakerLabel, tournamentFormatLabel, tournamentTimeLimitLabel,
  type CompetitionTournamentViewData,
} from '../lib/competition-tournament-view'

const props = withDefaults(defineProps<{
  competitionId: string
  sessionUserId: string | null
  registration: CompetitionRegistrationState | null
  contextReady: boolean
  busy?: boolean
}>(), { busy: false })
const emit = defineEmits<{
  created: [value: { competitionId: string, revision: number }]
  'pending-change': [value: boolean]
  unresolved: []
}>()
const context = computed(() => createTournamentManagementContext(props.competitionId, props.sessionUserId, props.registration))
const scope = computed(() => JSON.stringify([props.competitionId.trim(), props.sessionUserId?.trim() ?? null]))
const enabled = computed(() => !!context.value && props.contextReady && !props.busy)
const draft = ref<TournamentManagementDraft | null>(null)
const draftScope = ref('')
const reviewed = shallowRef<TournamentStageSettings | null>(null)
const acknowledged = ref(false)
const view = shallowRef<CompetitionTournamentViewData | null>(null)
const readKind = ref<'idle' | 'notReady' | 'exists' | 'error'>('idle')
const reading = ref(false)
const sending = ref(false)
const readError = ref('')
const formError = ref('')
const mutationMessage = ref('')
const notice = ref('')
// pending 與顯示投影分開；任何 GET／身份切換均不是 mutation 收據。
const pending = shallowRef<TournamentCreateIntent | null>(null)
const wasUnknown = ref(false)
let mounted = false
let generation = 0
let readSequence = 0
let postSequence = 0
let readController: AbortController | null = null
let postController: AbortController | null = null
const pendingInScope = computed(() => !!pending.value && pending.value.scope.competitionId === props.competitionId.trim()
  && pending.value.scope.sessionUserId === props.sessionUserId?.trim())
const locked = computed(() => !enabled.value || reading.value || sending.value || !!pending.value)
const summary = computed(() => {
  // 有未知 intent 時僅呈現原 scope 的固定摘要，不能回退到未清的 reviewed。
  if (pending.value) return pendingInScope.value ? pending.value.body.stage : null
  return draftScope.value === scope.value ? reviewed.value : null
})
const canRetry = computed(() => enabled.value && pendingInScope.value && !reading.value && !sending.value)
const ties: SwissTieBreaker[] = ['matchPoints', 'headToHeadMatchPoints', 'opponentWinRate', 'netGameWins', 'seedOrder']
const pointFields = [
  { key: 'win', label: '勝場積分' }, { key: 'draw', label: '和局積分' },
  { key: 'loss', label: '敗場積分' }, { key: 'bye', label: '輪空積分' },
] satisfies { key: keyof TournamentManagementDraft['swiss']['points'], label: string }[]
const percentFields = [{ key: 'floor', label: '對手勝率下限' }, { key: 'cap', label: '對手勝率上限' }] satisfies { key: 'floor' | 'cap', label: string }[]
const uid = useId()
function fieldId(name: string) { return `${uid}-${name}` }
function mainTimeLabel(milliseconds: number | null): string {
  if (milliseconds === null || milliseconds === 0) return '不限時'
  return milliseconds % 60_000 === 0 ? `${milliseconds / 60_000} 分鐘` : tournamentTimeLimitLabel(milliseconds)
}
function teamLabel(teamId: string): string {
  const index = context.value?.teamIds.indexOf(teamId) ?? -1
  return index < 0 ? '參賽隊伍' : `第 ${index + 1} 隊`
}
const eligibilityMessage = computed(() => {
  if (!props.sessionUserId?.trim()) return '請先登入主辦帳號。'
  if (!props.contextReady || props.busy) return '正在確認賽事狀態，請稍候。'
  if (props.registration?.organizer !== props.sessionUserId.trim()) return '目前帳號不是公告所列主辦方。'
  if (props.registration?.lifecycle !== 'locked') return '名單鎖定後才能設定賽程。'
  return '目前沒有足夠且可確認的鎖定參賽隊伍。'
})

watch(draft, () => { reviewed.value = null; acknowledged.value = false; formError.value = '' }, { deep: true, flush: 'sync' })
watch(() => [scope.value, enabled.value, context.value?.revision, context.value?.teamIds.join('\0')], () => {
  invalidate()
  view.value = null
  readKind.value = 'idle'
  readError.value = ''
  notice.value = ''
  if (pending.value) mutationMessage.value = '先前建立結果尚未確認；離開或切換帳號不會取消伺服器操作。'
  if (!pending.value && context.value && draftScope.value !== scope.value) {
    draft.value = createTournamentManagementDraft(context.value)
    draftScope.value = scope.value
  } else if (!pending.value && context.value) {
    // 名單／revision 改變時重建尚未送出的草稿，不把舊設定默套到新名單。
    draft.value = createTournamentManagementDraft(context.value)
  }
  if (mounted && enabled.value) scheduleRead()
}, { flush: 'sync' })

function invalidate() {
  generation += 1
  readSequence += 1
  postSequence += 1
  readController?.abort()
  postController?.abort()
  readController = null
  postController = null
  if (sending.value && pending.value) wasUnknown.value = true
  reading.value = false
  sending.value = false
}
function scheduleRead() {
  // 先停用操作，再等 Vue 同步；舊 finally 不能解鎖新 read。
  reading.value = true
  const token = generation
  void nextTick().then(() => {
    if (mounted && token === generation && enabled.value) return loadTournament()
  })
}
async function loadTournament() {
  const current = context.value
  if (!mounted || !enabled.value || !current || sending.value) return
  readController?.abort()
  const controller = new AbortController()
  readController = controller
  const sequence = ++readSequence
  const token = generation
  const identity = scope.value
  reading.value = true
  readError.value = ''
  view.value = null
  const accepted = () => mounted && token === generation && sequence === readSequence && identity === scope.value && !controller.signal.aborted
  try {
    const result: unknown = await $fetch(`/api/competitions/${encodeURIComponent(current.competitionId)}/tournament`,
      { method: 'GET', retry: 0, signal: controller.signal })
    if (!accepted()) return
    view.value = parseCompetitionTournamentViewResponse(result, current.competitionId)
    readKind.value = 'exists'
  } catch (error) {
    if (!accepted()) return
    const presented = presentCompetitionTournamentViewError(error)
    readKind.value = presented.kind === 'notReady' ? 'notReady' : 'error'
    readError.value = presented.message
  } finally {
    if (accepted()) { reading.value = false; readController = null }
  }
}
function assignSeeds() {
  if (locked.value || !draft.value) return
  draft.value.teams.forEach((team, index) => { team.seed = String(index + 1) })
}
function generateSeed() {
  if (locked.value || !draft.value) return
  try { draft.value.randomSeed = String(generateTournamentScheduleRandomSeed()) }
  catch { formError.value = '無法產生排程亂數種子，請自行輸入。' }
}
function reviewDraft() {
  if (locked.value || readKind.value !== 'notReady' || !context.value || !draft.value) return
  try {
    reviewed.value = parseTournamentStageDraft(context.value, draft.value)
    acknowledged.value = false
    formError.value = ''
  } catch (error) { formError.value = tournamentManagementValidationMessage(error) }
}
function addTie() {
  if (!locked.value && draft.value && draft.value.swiss.tieBreakers.length < ties.length) draft.value.swiss.tieBreakers.push('')
}
function removeTie(index: number) {
  if (!locked.value) draft.value?.swiss.tieBreakers.splice(index, 1)
}
function selectTieMode() { if (draft.value) draft.value.swiss.tieBreakers = [] }
function confirmCreate() {
  if (locked.value || readKind.value !== 'notReady' || !context.value || !draft.value || !reviewed.value || !acknowledged.value) return
  try {
    pending.value = buildTournamentCreateIntent(context.value, draft.value)
    wasUnknown.value = false
    emit('pending-change', true)
    void sendPending()
  } catch (error) { formError.value = tournamentManagementValidationMessage(error) }
}
async function sendPending() {
  const intent = pending.value
  if (!intent || !canRetry.value) return
  const controller = new AbortController()
  postController = controller
  const sequence = ++postSequence
  const token = generation
  sending.value = true
  mutationMessage.value = ''
  const accepted = () => mounted && token === generation && sequence === postSequence && pending.value === intent
    && pendingInScope.value && enabled.value && !controller.signal.aborted
  let confirmed = false
  try {
    const result: unknown = await $fetch(intent.path, { method: intent.method, body: intent.body, retry: 0, signal: controller.signal })
    if (!accepted()) return
    const receipt = parseTournamentCreateAck(result, intent)
    pending.value = null
    wasUnknown.value = false
    mutationMessage.value = ''
    notice.value = '建立賽程已獲伺服器收據確認。'
    emit('pending-change', false)
    emit('created', { competitionId: intent.scope.competitionId, revision: receipt.revision })
    confirmed = true
  } catch (error) {
    if (!accepted()) return
    if (keepTournamentCreateIntent(wasUnknown.value, error)) {
      wasUnknown.value = true
      mutationMessage.value = '建立結果尚未確認，可能已建立。請以相同命令重試；重新查詢不會確認此收據。'
    } else {
      pending.value = null
      emit('pending-change', false)
      mutationMessage.value = '伺服器未接受本次建立設定，請重新查詢狀態並檢查設定。'
      readKind.value = 'idle'
    }
  } finally {
    // 成功時 pending 已清，仍以 request generation 決定 finally 的所有權。
    if (mounted && token === generation && sequence === postSequence && !controller.signal.aborted) {
      sending.value = false
      postController = null
      if (confirmed) void loadTournament()
    }
  }
}
function beforeUnload(event: BeforeUnloadEvent) {
  if (pending.value) { event.preventDefault(); event.returnValue = '' }
}
onMounted(() => {
  mounted = true
  if (context.value) { draft.value = createTournamentManagementDraft(context.value); draftScope.value = scope.value }
  window.addEventListener('beforeunload', beforeUnload)
  if (enabled.value) scheduleRead()
})
onBeforeUnmount(() => {
  if (pending.value) emit('unresolved')
  mounted = false
  invalidate()
  window.removeEventListener('beforeunload', beforeUnload)
})
</script>

<template>
  <section class="tournament-management" aria-label="設定與建立賽程" :aria-busy="reading || sending">
    <h2>設定與建立賽程</h2>
    <p>本片提供單階段設定與建立；不預測配對、晉級或名次。實際建立權限與設定由伺服器確認。</p>
    <p v-if="!enabled" role="status">{{ eligibilityMessage }}</p>
    <button type="button" data-testid="tournament-refresh" :disabled="!enabled || reading || sending" @click="loadTournament">重新查詢賽程</button>
    <p v-if="reading" role="status">正在查詢賽程…</p>
    <p v-if="readError" role="status">{{ readError }}</p>
    <p v-if="view && !reading" role="status">已存在 {{ view.stages.length }} 個階段的賽程；本片不新增其他階段。請至賽程檢視查看。</p>
    <p v-if="notice" role="status">{{ notice }}</p>
    <div v-if="pending || mutationMessage" class="pending-notice" role="status">
      <p>{{ mutationMessage || '正在送出固定設定，請勿重複建立。' }}</p>
      <template v-if="pending">
        <p>尚未確認的原命令會保留原種子、設定與版本；離開不代表取消建立。</p>
        <p v-if="!pendingInScope">請回到原賽事並使用原帳號確認；不可換身份重送。</p>
        <button type="button" data-testid="tournament-same-command-retry" :disabled="!canRetry" @click="sendPending">以相同命令重試</button>
      </template>
    </div>
    <form v-if="draft && draftScope === scope && context && (readKind === 'notReady' || pendingInScope)"
      data-testid="tournament-form" @submit.prevent="reviewDraft">
      <fieldset :disabled="locked">
        <legend>賽制、種子與時計政策</legend>
        <div class="fields">
          <label :for="fieldId('format')">賽制
            <select :id="fieldId('format')" v-model="draft.format" data-testid="tournament-format" required>
              <option value="" disabled>請選擇賽制</option>
              <option value="singleElimination">單敗淘汰</option><option value="roundRobin">循環賽</option>
              <option value="doubleElimination">雙敗淘汰</option><option value="swiss">瑞士制</option>
            </select>
          </label>
          <label :for="fieldId('games')">{{ draft.format === 'singleElimination' || draft.format === 'doubleElimination' ? '勝局門檻' : '每場局數' }}
            <input :id="fieldId('games')" v-model="draft.matchGames" data-testid="tournament-games" inputmode="numeric" required>
          </label>
        </div>
        <h3>已鎖定參賽隊伍的種子</h3>
        <p>第幾隊僅依名單順序標示，不是既有排名；種子必須由主辦指定。</p>
        <div class="fields">
          <label v-for="(team, index) in draft.teams" :key="team.teamId" :for="fieldId(`seed-${index}`)">{{ teamLabel(team.teamId) }}種子
            <input :id="fieldId(`seed-${index}`)" v-model="team.seed" :data-testid="`tournament-team-seed-${index}`" inputmode="numeric" required>
          </label>
        </div>
        <button type="button" data-testid="tournament-assign-seeds" @click="assignSeeds">依名單順序指定種子 1 至 {{ draft.teams.length }}</button>
        <label :for="fieldId('random')">排程亂數種子（非負整數，公開設定）
          <input :id="fieldId('random')" v-model="draft.randomSeed" data-testid="tournament-random-seed" inputmode="numeric" required>
        </label>
        <button type="button" data-testid="tournament-generate-seed" @click="generateSeed">產生排程亂數種子</button>
        <p>此數值僅用於排程，不是牌組或房間的秘密亂數種子；送出與重試不會重新產生。</p>
        <h3>時計設定</h3>
        <div class="fields">
          <label :for="fieldId('clock-main')">主時間（整數分鐘；0 表示不限時）
            <input :id="fieldId('clock-main')" v-model="draft.clock.mainMinutes" data-testid="tournament-clock-main"
              inputmode="numeric" pattern="[0-9]+" required>
          </label>
          <label :for="fieldId('byoyomi')">讀秒（行動與回合外選擇共用）
            <select :id="fieldId('byoyomi')" v-model="draft.clock.byoyomiSeconds" data-testid="tournament-byoyomi" required>
              <option value="30">30 秒</option><option value="60">1 分鐘</option><option value="180">3 分鐘</option>
            </select>
          </label>
        </div>
        <template v-if="draft.format === 'swiss'">
          <h3>瑞士制完整政策</h3>
          <label :for="fieldId('rounds')">輪數<input :id="fieldId('rounds')" v-model="draft.swiss.rounds" data-testid="tournament-swiss-rounds" inputmode="numeric" required></label>
          <div class="fields">
            <label v-for="field in pointFields" :key="field.key" :for="fieldId(`points-${field.key}`)">{{ field.label }}
              <input :id="fieldId(`points-${field.key}`)" v-model="draft.swiss.points[field.key]" :data-testid="`tournament-points-${field.key}`" inputmode="numeric" required>
            </label>
            <label :for="fieldId('numerator')">對手勝率的和局折算：分子<input :id="fieldId('numerator')" v-model="draft.swiss.credit.numerator" data-testid="tournament-credit-numerator" inputmode="numeric" required></label>
            <label :for="fieldId('denominator')">對手勝率的和局折算：分母<input :id="fieldId('denominator')" v-model="draft.swiss.credit.denominator" data-testid="tournament-credit-denominator" inputmode="numeric" required></label>
            <div v-for="field in percentFields" :key="field.key">
              <label :for="fieldId(field.key)">{{ field.label }}
                <select :id="fieldId(field.key)" v-model="draft.swiss[field.key].mode" :data-testid="`tournament-${field.key}-mode`" required>
                  <option value="" disabled>請選擇</option><option value="none">不設定</option><option value="value">指定百分比</option>
                </select>
              </label>
              <label v-if="draft.swiss[field.key].mode === 'value'" :for="fieldId(`${field.key}-value`)">{{ field.label }}百分比（零至一百）
                <input :id="fieldId(`${field.key}-value`)" v-model="draft.swiss[field.key].value" :data-testid="`tournament-${field.key}-value`" inputmode="numeric" required>
              </label>
            </div>
            <label :for="fieldId('byes')">計算對手勝率時是否排除輪空
              <select :id="fieldId('byes')" v-model="draft.swiss.excludeByes" data-testid="tournament-exclude-byes" required>
                <option value="" disabled>請選擇</option><option value="yes">排除輪空</option><option value="no">不排除輪空</option>
              </select>
            </label>
            <label :for="fieldId('ties')">同分比較政策
              <select :id="fieldId('ties')" v-model="draft.swiss.tieMode" data-testid="tournament-tie-mode" required @change="selectTieMode">
                  <option value="" disabled>請選擇</option><option value="none">不設定同分比較</option><option value="configured">自訂比較順序</option>
              </select>
            </label>
          </div>
          <template v-if="draft.swiss.tieMode === 'configured'">
            <div v-for="(_, index) in draft.swiss.tieBreakers" :key="index" class="tie-row">
              <label :for="fieldId(`tie-${index}`)">第 {{ index + 1 }} 項同分比較
                <select :id="fieldId(`tie-${index}`)" v-model="draft.swiss.tieBreakers[index]" :data-testid="`tournament-tie-${index}`" required>
                  <option value="" disabled>請選擇</option><option v-for="tie in ties" :key="tie" :value="tie">{{ swissTieBreakerLabel(tie) }}</option>
                </select>
              </label>
              <button type="button" :aria-label="`移除第 ${index + 1} 項同分比較`" @click="removeTie(index)">移除</button>
            </div>
            <button type="button" data-testid="tournament-add-tie" :disabled="draft.swiss.tieBreakers.length >= ties.length" @click="addTie">新增下一項同分比較</button>
          </template>
        </template>
        <button type="submit" data-testid="tournament-review">檢視設定摘要</button>
      </fieldset>
    </form>
    <p v-if="formError" role="alert">{{ formError }}</p>
    <div v-if="summary" class="summary" data-testid="tournament-summary">
      <h3>{{ pending ? '原命令的固定設定摘要' : '待確認設定摘要' }}</h3>
      <dl>
        <dt>賽制</dt><dd>{{ tournamentFormatLabel(summary.format) }}</dd>
        <dt>{{ summary.matchFormat.type === 'firstToWins' ? '勝局門檻' : '每場局數' }}</dt><dd>{{ summary.matchFormat.value }}</dd>
        <dt>排程亂數種子</dt><dd>{{ summary.randomSeed }}</dd>
        <dt>主時間</dt><dd>{{ mainTimeLabel(summary.clockConfig.mainTimeMs) }}</dd>
        <dt>讀秒（行動與回合外選擇）</dt><dd>{{ tournamentTimeLimitLabel(summary.clockConfig.actionTimeMs) }}</dd>
      </dl>
      <ul><li v-for="team in summary.teams" :key="team.teamId">{{ teamLabel(team.teamId) }}：種子 {{ team.seed }}</li></ul>
      <template v-if="summary.swissPolicy">
        <p>瑞士制輪數：{{ summary.swissRounds }}</p>
        <p>勝場／和局／敗場／輪空積分：{{ summary.swissPolicy.matchPoints.win }}／{{ summary.swissPolicy.matchPoints.draw }}／{{ summary.swissPolicy.matchPoints.loss }}／{{ summary.swissPolicy.matchPoints.bye }}</p>
        <p>對手勝率的和局折算：{{ summary.swissPolicy.drawCredit.numerator }}／{{ summary.swissPolicy.drawCredit.denominator }}</p>
        <p>對手勝率下限：{{ summary.swissPolicy.opponentWinRateFloorPercent === null ? '不設定' : `${summary.swissPolicy.opponentWinRateFloorPercent}%` }}；上限：{{ summary.swissPolicy.opponentWinRateCapPercent === null ? '不設定' : `${summary.swissPolicy.opponentWinRateCapPercent}%` }}</p>
        <p>計算對手勝率{{ summary.swissPolicy.excludeByesFromOpponentWinRate ? '排除輪空' : '不排除輪空' }}</p>
        <p>同分比較：{{ summary.swissPolicy.tieBreakers.length ? summary.swissPolicy.tieBreakers.map(swissTieBreakerLabel).join(' → ') : '不設定' }}</p>
      </template>
      <template v-if="!pending">
        <label class="check"><input v-model="acknowledged" type="checkbox" data-testid="tournament-acknowledge" :disabled="locked">我已核對全部設定與種子，確認建立單階段賽程。</label>
        <button type="button" data-testid="tournament-confirm-create" :disabled="locked || !acknowledged || readKind !== 'notReady'" @click="confirmCreate">確認建立賽程</button>
      </template>
    </div>
  </section>
</template>

<style scoped>
.tournament-management { width: 100%; min-width: 0; overflow-wrap: anywhere; }
.tournament-management * { box-sizing: border-box; min-width: 0; }
h2, h3, p { margin-block: .75rem; }
fieldset, .summary, .pending-notice { border: 1px solid var(--app-border, #aaa); border-radius: .5rem; padding: 1rem; margin-block: 1rem; }
legend { padding-inline: .25rem; }
.fields { display: grid; grid-template-columns: repeat(auto-fit, minmax(min(100%, 15rem), 1fr)); gap: 1rem; }
label { display: block; margin-block: .5rem; }
input:not([type="checkbox"]), select { display: block; width: 100%; margin-top: .35rem; min-height: 2.75rem; padding: .5rem; color: var(--app-text); background: var(--app-input); border: 1px solid var(--app-border); border-radius: .25rem; }
button { max-width: 100%; min-height: 2.75rem; white-space: normal; padding: .5rem .75rem; margin-block: .5rem; color: var(--app-text); background: var(--app-control); border: 1px solid var(--app-border); border-radius: .25rem; }
button:disabled, fieldset:disabled { opacity: .65; }
.check { display: flex; align-items: flex-start; gap: .5rem; }
.check input { flex-shrink: 0; margin-top: .25rem; }
.tie-row { display: flex; align-items: end; gap: .5rem; }
.tie-row label { flex: 1; }
dl { display: grid; grid-template-columns: minmax(0, 1fr) minmax(0, 1fr); gap: .5rem; }
dd { margin: 0; }
:is(button, input, select):focus-visible { outline: 2px solid var(--app-accent); outline-offset: 3px; }
@media (max-width: 480px) { fieldset, .summary, .pending-notice { padding: .75rem; } .tie-row { flex-wrap: wrap; } }
</style>
