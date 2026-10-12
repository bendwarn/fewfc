<template>
  <main class="competition-detail-page">
    <header class="detail-heading">
      <div>
        <NuxtLink class="back-link" to="/competitions">← 競賽對局</NuxtLink>
        <p class="eyebrow">賽事報名</p>
        <h1>競賽對局・論劍</h1>
        <p class="muted">賽事 ID：<span class="identifier">{{ competitionId || '—' }}</span></p>
      </div>
      <button
        v-if="session.userId.value && registration"
        class="ghost-button refresh-button"
        type="button"
        :disabled="loading || mutationBusy"
        @click="loadRegistration"
      >
        {{ loading ? '更新中…' : '重新整理狀態' }}
      </button>
    </header>

    <section v-if="!sessionLoaded || loading" class="panel" role="status" aria-live="polite">
      {{ !sessionLoaded ? '正在確認登入狀態…' : '正在載入賽事報名狀態…' }}
    </section>

    <section v-else-if="sessionError" class="panel status-panel error-message" role="alert">
      <p>{{ sessionError }}</p>
      <button class="ghost-button" type="button" @click="bootstrap">重試</button>
    </section>

    <section v-else-if="!session.userId.value" class="panel status-panel">
      <p>登入後即可查看賽事公告與報名狀態。</p>
      <NuxtLink class="primary-button link-button" :to="{ path: '/login', query: { redirect: route.fullPath } }">登入</NuxtLink>
    </section>

    <section v-else-if="!competitionId" class="panel status-panel" role="status">
      <p>缺少賽事 ID。請返回競賽入口輸入主辦方提供的 ID。</p>
      <NuxtLink class="secondary-button link-button" to="/competitions">返回競賽入口</NuxtLink>
    </section>

    <section v-else-if="loadError" class="panel status-panel" :class="{ 'error-message': !notFound }" :role="notFound ? 'status' : 'alert'">
      <p>{{ loadError }}</p>
      <button class="ghost-button" type="button" @click="loadRegistration">重試載入</button>
    </section>

    <template v-else-if="registration">
      <p v-if="notice" class="panel notice-panel" role="status" aria-live="polite">{{ notice }}</p>
      <section class="panel announcement-card" aria-labelledby="announcement-title">
        <div class="announcement-title-row">
          <div>
            <p class="eyebrow">賽事公告</p>
            <h2 id="announcement-title">{{ lifecycleLabel(registration.lifecycle) }}</h2>
          </div>
          <span class="lifecycle-badge" :class="`state-${registration.lifecycle}`">
            {{ lifecycleLabel(registration.lifecycle) }}
          </span>
        </div>

        <dl class="announcement-facts">
          <div><dt>主辦人</dt><dd class="identifier">{{ registration.organizer }}</dd></div>
          <div><dt>規則版本</dt><dd>{{ registration.ruleVersion }}</dd></div>
          <div><dt>規則基底</dt><dd>{{ rulesetLabel(registration.ruleset) }}</dd></div>
          <div><dt>參賽型態</dt><dd>{{ registration.teamSize === 1 ? '個人' : `團體（每隊 ${registration.teamSize} 人）` }}</dd></div>
          <div><dt>每隊初始生命值</dt><dd>{{ registration.initialHp }}</dd></div>
          <div><dt>報名方式</dt><dd>{{ registration.registrationMode === 'public' ? '公開報名' : '邀請制' }}</dd></div>
          <div><dt>報名截止</dt><dd>{{ formatCompetitionRegistrationDeadline(registration.registrationDeadline) }}</dd></div>
          <div><dt>比賽開始時間</dt><dd>{{ formatCompetitionDeadline(registration.checkInDeadline) }}</dd></div>
          <div class="module-fact"><dt>規則模組</dt><dd>{{ ruleModuleLabels(registration.enabledRuleModules) }}</dd></div>
          <div><dt>個人牌組政策</dt><dd>{{ registration.personalDecks === 'required' ? '需要提交' : '不使用' }}</dd></div>
        </dl>

        <p class="muted">報名截止時，名單、成員確認、必要牌組與隊長資料完成確認一併凍結。資料完成確認不是房間報到或準備；進房後仍須另外連線並明示準備。</p>

        <div v-if="isOrganizer" class="organizer-actions" aria-label="主辦方操作">
          <p v-if="registration.lifecycle === 'draft'" class="muted">公告建立完成；開放報名後，符合條件的隊長即可登記隊伍。</p>
          <p v-else-if="registration.lifecycle === 'open'" class="muted">系統會在報名截止時自動鎖定名單，不必等到比賽開始時間；未完成資料的隊伍會列出排除原因。</p>
          <button
            v-if="registration.lifecycle === 'draft'"
            class="primary-button"
            type="button"
            :disabled="actionsDisabled"
            @click="mutate({ type: 'open' })"
          >
            開放報名
          </button>
        </div>
      </section>

      <section class="panel announcement-plan-card" aria-labelledby="announcement-plan-title">
        <div>
          <h2 id="announcement-plan-title">完整競賽公告方案</h2>
          <p class="muted">以下規則、階段、時計、資格來源與排序政策由主辦在報名開始前一併保存，報名隊伍查閱的是同一份公告。</p>
        </div>
        <p class="policy-line"><strong>首輪種子：</strong>報名截止後由系統抽籤，結果保存後重試不重抽。</p>
        <p class="policy-line"><strong>正式排序：</strong>自身交手積分較高優先；同分依序比勝分較多、負分較少。</p>
        <div class="policy-line ranking-score-policy" aria-label="勝分與負分計算定義">
          <strong>勝分／負分計算：</strong>
          <ul>
            <li><strong>勝分：</strong>本階段每位不同對手在本階段的最終賽事交手勝場總和。</li>
            <li><strong>負分：</strong>同一組對手在本階段的最終賽事交手敗場總和。</li>
            <li>每位對手只計一次，無論本隊對該隊的結果是勝、和或負；對手戰績包含其與本隊的那次交手，不跨階段累計。</li>
            <li>未真正開打的缺席配對不納入對手集合；合法缺席判定保留為該隊自身的賽事交手結果，但不會因此納入本項的對手集合。和局不增加勝敗場，雙方判負各增加一敗；輪空與取消不增加勝敗場。</li>
            <li>不以單局勝敗、勝率、生命、主時間或耗時代替交手勝敗，也不把勝分與負分相減成淨分。</li>
          </ul>
        </div>
        <p class="policy-line"><strong>所有比較項全同：</strong>僅主辦或無利益衝突的指定裁判決定順序及資格，必填並公開理由；不比較生命、主時間或耗時，也不建立排名同分加賽。</p>
        <p class="policy-line"><strong>瑞士配對：</strong>依公告規則建立合法配對；多個合法候選方案以保存的抽籤種子決定，不影響最終排名。</p>
        <ol class="announced-stage-list">
          <li v-for="(stage, index) in registration.plan.stages" :key="stage.stageId" class="announced-stage">
            <h3>第 {{ index + 1 }} 階段：{{ announcedStageFormat(stage.format) }}</h3>
            <dl class="stage-facts">
              <div><dt>交手格式</dt><dd>{{ announcedMatchFormat(stage) }}</dd></div>
              <div><dt>主時間</dt><dd>{{ announcedMainTime(stage.clockConfig.mainTimeMs) }}</dd></div>
              <div><dt>讀秒</dt><dd>{{ announcedByoyomi(stage.clockConfig.actionTimeMs) }}（行動與回合外選擇共用）</dd></div>
              <div v-if="stage.format === 'roundRobin'"><dt>賽事積分</dt><dd>勝 3、和 1、負 0；輪空 0</dd></div>
              <template v-if="stage.format === 'swiss' && stage.swissPolicy">
                <div><dt>瑞士輪數</dt><dd>{{ stage.swissRounds }}</dd></div>
                <div><dt>瑞士積分（勝／和／負／輪空）</dt><dd>{{ stage.swissPolicy.matchPoints.win }}／{{ stage.swissPolicy.matchPoints.draw }}／{{ stage.swissPolicy.matchPoints.loss }}／{{ stage.swissPolicy.matchPoints.bye }}</dd></div>
                <div><dt>對手勝率和局折算</dt><dd>{{ stage.swissPolicy.drawCredit.numerator }}／{{ stage.swissPolicy.drawCredit.denominator }}</dd></div>
                <div><dt>對手勝率上下限</dt><dd>{{ announcedPercent(stage.swissPolicy.opponentWinRateFloorPercent) }}／{{ announcedPercent(stage.swissPolicy.opponentWinRateCapPercent) }}</dd></div>
                <div><dt>計算對手勝率時排除輪空</dt><dd>{{ stage.swissPolicy.excludeByesFromOpponentWinRate ? '是' : '否' }}</dd></div>
                <div><dt>瑞士同分比較</dt><dd>{{ announcedSwissTieBreakers(stage.swissPolicy.tieBreakers) }}</dd></div>
              </template>
              <div v-if="stage.qualification.type === 'allEligibleTeams'"><dt>參賽資格</dt><dd>截止時所有符合公告條件的隊伍</dd></div>
              <div v-else class="qualification-fact"><dt>晉級來源與目的種子</dt><dd>
                <ul>
                  <li v-for="slot in stage.qualification.slots" :key="`${slot.sourceStageId}-${slot.destinationSeed}`">
                    {{ announcedSourceStage(slot.sourceStageId) }}：{{ announcedQualificationSource(slot.source) }} → 種子 {{ slot.destinationSeed }}
                  </li>
                </ul>
              </dd></div>
            </dl>
          </li>
        </ol>
      </section>

      <CompetitionPersonalDeckSubmission
        :current-user-id="session.userId.value"
        :active-team="currentTeam"
        :lifecycle="registration.lifecycle"
        :personal-decks="registration.personalDecks"
        :registration-deadline="registration.registrationDeadline"
        :busy="mutationBusy"
        :pending="pendingCommand !== null || roundPublicationPending !== null"
        :own-deck-submitted="ownDeckSubmitted"
        @submit-deck="mutate"
      />

      <CompetitionTournamentView
        :competition-id="competitionId"
        :session-identity="session.userId.value"
        :context-revision="registration.revision"
        :context-ready="sessionLoaded && Boolean(registration)"
        :registration-locked="automaticInitialScheduleExpected"
        :participant-team-id="currentTeam?.teamId ?? null"
        :participant-entry-enabled="canLookupParticipantGame"
      />

      <section class="panel teams-card" aria-labelledby="teams-title">
        <div class="teams-heading">
          <div>
            <h2 id="teams-title">報名隊伍</h2>
            <p class="muted">{{ activeTeamCount }} 隊有效報名</p>
          </div>
          <span class="revision-label">狀態版本 {{ registration.revision }}</span>
        </div>

        <div v-if="registration.lifecycle === 'open' && !currentTeam" class="register-panel">
          <div>
            <h3>以隊長身分登記隊伍</h3>
            <p class="muted">
              登記名單必須包含你本人，共 {{ registration.teamSize }} 位不同隊員。{{ registration.registrationMode === 'inviteOnly' ? '邀請制資格由伺服器核實。' : '' }}
            </p>
          </div>
          <form class="register-form" @submit.prevent="registerTeam">
            <label v-if="registration.teamSize > 1" for="team-member-ids">
              其他隊員使用者 ID（每行一位）
              <textarea
                id="team-member-ids"
                v-model="otherMemberIdsText"
                class="text-input text-area"
                rows="3"
                required
                :disabled="actionsDisabled"
              />
            </label>
            <button class="primary-button" type="submit" :disabled="actionsDisabled">
              登記隊伍
            </button>
          </form>
        </div>

        <p v-if="!visibleTeams.length" class="empty-state">目前沒有報名隊伍。</p>
        <div v-else class="team-list">
          <article v-for="team in visibleTeams" :key="team.teamId" class="team-card">
            <header class="team-title">
              <div>
                <h3>隊伍 <span class="identifier">{{ team.teamId }}</span></h3>
                <p class="muted">隊長：<span class="identifier">{{ team.captain }}</span></p>
              </div>
              <span class="team-status" :class="`team-${team.status}`">{{ teamStatusLabel(team.status) }}</span>
            </header>

            <div v-if="team.status === 'notCheckedIn'" class="muted" role="status">
              <p>未納入賽程原因：</p>
              <ul aria-label="未納入賽程原因">
                <li v-for="reason in team.exclusionReasons" :key="reason">{{ registrationExclusionReasonLabel(reason) }}</li>
              </ul>
            </div>

            <ul class="member-list" aria-label="隊員確認狀態">
              <li v-for="member in team.members" :key="member">
                <span class="identifier">{{ member }}</span>
                <span class="confirmation" :class="{ confirmed: team.confirmedMembers.includes(member) }">
                  {{ team.confirmedMembers.includes(member) ? '已確認' : '待確認' }}
                </span>
              </li>
            </ul>

            <p class="muted team-progress">
              已確認 {{ team.confirmedMembers.length }} / {{ registration.teamSize }} 人 · 名單版本 {{ team.rosterVersion }}
            </p>

            <div v-if="team.teamId === currentTeam?.teamId && registration.lifecycle === 'open'" class="team-actions">
              <button
                v-if="currentTeam && currentTeam.members.includes(session.userId.value) && !currentTeam.confirmedMembers.includes(session.userId.value) && currentTeam.status !== 'withdrawn'"
                class="secondary-button"
                type="button"
                :disabled="actionsDisabled"
                @click="mutate({ type: 'confirmMembership', teamId: currentTeam.teamId, rosterVersion: currentTeam.rosterVersion })"
              >
                確認本人隊員身分
              </button>
              <button
                v-if="isCurrentCaptain && currentTeam?.status === 'registered' && allMembersConfirmed(currentTeam)"
                class="primary-button"
                type="button"
                :disabled="actionsDisabled"
                @click="mutate({ type: 'checkInTeam', teamId: currentTeam.teamId, rosterVersion: currentTeam.rosterVersion })"
              >
                隊長資料完成確認
              </button>
              <button
                v-if="isCurrentCaptain && currentTeam?.status === 'registered'"
                class="danger-button"
                type="button"
                :disabled="actionsDisabled"
                @click="mutate({ type: 'withdrawTeam', teamId: currentTeam.teamId })"
              >
                退出賽事
              </button>
            </div>
          </article>
        </div>
      </section>

      <section
        v-if="pendingCommand || mutationError"
        class="panel error-message"
        :role="mutationError ? 'alert' : 'status'"
        :aria-live="mutationError ? 'assertive' : 'polite'"
      >
        <p>{{ mutationError || '先前命令的結果仍未確認。重新整理只更新賽事狀態，不會清除這個命令；可用相同命令安全重試。' }}</p>
        <button v-if="pendingCommand" class="ghost-button" type="button" :disabled="mutationBusy || roundPublicationPending !== null" @click="retryPendingCommand">
          {{ mutationBusy ? '正在以相同命令重試…' : '以相同命令重試' }}
        </button>
      </section>
    </template>

    <p v-if="roundPublicationNavigationMessage" class="panel notice-panel" role="status" aria-live="polite">
      {{ roundPublicationNavigationMessage }}
    </p>

    <!-- 公告元件穩定掛載；busy 只隔離既有寫操作，不回灌自身 pending。 -->
    <CompetitionRoundPublication
      :competition-id="competitionId"
      :session-user-id="session.userId.value"
      :registration="registrationInCurrentScope ? registration : null"
      :context-ready="tournamentContextReady"
      :automatic-initial-stage-id="automaticInitialStageId"
      :busy="mutationBusy || pendingCommand !== null"
      @pending-change="onRoundPublicationPendingChange"
      @unresolved="onRoundPublicationUnresolved"
    />

    <!-- 同身份重新整理不卸載更正 module，未知命令不能因報名 loading 遺失。 -->
    <CompetitionRulingCorrection
      :competition-id="competitionId"
      :session-user-id="session.userId.value"
      :context-ready="sessionLoaded && !loading && !loadError && !sessionError && registration?.competitionId === competitionId && roundPublicationPending === null"
    />
  </main>
</template>

<script setup lang="ts">
import { onBeforeRouteLeave, onBeforeRouteUpdate } from 'vue-router'
import CompetitionRoundPublication from '~/components/CompetitionRoundPublication.vue'
import type { CompetitionRoundPublicationIntent } from '~/lib/competition-round-publication'
import { presentApiError } from '~/lib/api-error-presentation'
import { presentationForRuleModule } from '#shared/utils/rule-modules'
import { canShowCompetitionParticipantGameEntry } from '~/lib/competition-participant-game'
import {
  apiErrorStatus,
  createRegistrationCommandBody,
  createRegistrationViewBody,
  formatCompetitionDeadline,
  formatCompetitionRegistrationDeadline,
  registrationExclusionReasonLabel,
  buildRegisterTeamAction,
  isRevisionConflict,
  parseCompetitionRegistrationResponse,
  selectActiveTeamForCaller,
  type CompetitionPublicTeam,
  type CompetitionRegistrationState,
  type CompetitionTeamStatus,
  type RegistrationCommandBody,
  type RegistrationMutationAction,
} from '~/lib/competition-registration'

// 只沿用同 detail record；切換參數由 child scope 防護，不以 page key 丟棄未知命令。
definePageMeta({ key: false })

const route = useRoute()
const router = useRouter()
const session = usePlayerSession()
const competitionId = computed(() => typeof route.params.id === 'string' ? route.params.id.trim() : '')
const sessionLoaded = ref(false)
const sessionError = ref('')
const registration = ref<CompetitionRegistrationState | null>(null)
const loading = ref(false)
const loadError = ref('')
const notFound = ref(false)
const mutationBusy = ref(false)
const mutationError = ref('')
const notice = ref('')
const pendingCommand = ref<RegistrationCommandBody | null>(null)
const otherMemberIdsText = ref('')
const roundPublicationPending = shallowRef<CompetitionRoundPublicationIntent | null>(null)
const roundPublicationNavigationMessage = ref('')
const registrationIdentity = shallowRef<{ competitionId: string, userId: string } | null>(null)
let registrationReadSequence = 0
let registrationReadController: AbortController | null = null
let disposed = false
const registrationInCurrentScope = computed(() => {
  const identity = registrationIdentity.value
  return Boolean(registration.value && identity
    && registration.value.competitionId === competitionId.value
    && identity.competitionId === competitionId.value
    && identity.userId === session.userId.value)
})
const tournamentContextReady = computed(() => sessionLoaded.value && !loading.value
  && !loadError.value && !sessionError.value && !mutationError.value && !mutationBusy.value && !pendingCommand.value
  && registrationInCurrentScope.value && registration.value?.lifecycle === 'locked')
const automaticInitialStageId = computed(() => {
  const current = registration.value
  const firstStage = current?.plan.stages[0]
  if (!registrationInCurrentScope.value || current?.lifecycle !== 'locked'
    || current.teams.filter(team => team.status === 'checkedIn').length < 2
    || firstStage?.qualification.type !== 'allEligibleTeams') return null
  return firstStage.stageId
})
const automaticInitialScheduleExpected = computed(() => automaticInitialStageId.value !== null)

const isOrganizer = computed(() => Boolean(registration.value
  && session.userId.value
  && registration.value.organizer === session.userId.value))

function announcedStageFormat(format: string): string {
  return ({ singleElimination: '單敗淘汰', roundRobin: '循環賽', doubleElimination: '雙敗淘汰', swiss: '瑞士制' } as Record<string, string>)[format] ?? '未知賽制'
}
function announcedMatchFormat(stage: CompetitionRegistrationState['plan']['stages'][number]): string {
  return stage.matchFormat.type === 'firstToWins' ? `先取得 ${stage.matchFormat.value} 勝局` : `固定 ${stage.matchFormat.value} 局`
}
function announcedMainTime(milliseconds: number | null): string {
  if (milliseconds === null || milliseconds === 0) return '不限時'
  return `${milliseconds / 60_000} 分鐘`
}
function announcedByoyomi(milliseconds: number | null): string {
  if (milliseconds === 30_000) return '30 秒'
  if (milliseconds === 60_000) return '1 分鐘'
  if (milliseconds === 180_000) return '3 分鐘'
  return '公告資料無效'
}
function announcedPercent(value: number | null): string {
  return value === null ? '不設定' : `${value}%`
}
function announcedSwissTieBreakers(values: string[]): string {
  const labels: Record<string, string> = {
    matchPoints: '交手積分', headToHeadMatchPoints: '彼此交手積分', opponentWinRate: '對手勝率',
    netGameWins: '淨勝局', seedOrder: '種子順序',
  }
  return values.length ? values.map(value => labels[value] ?? '公告資料無效').join(' → ') : '不設定'
}
function announcedSourceStage(stageId: string): string {
  const index = registration.value?.plan.stages.findIndex(stage => stage.stageId === stageId) ?? -1
  return index < 0 ? '未知階段' : `第 ${index + 1} 階段`
}
function announcedQualificationSource(source: { type: string, rank?: number }): string {
  if (source.type === 'finishedStageWinner') return '該階段勝者'
  if (source.type === 'uniqueStandingRank' && source.rank) return `唯一第 ${source.rank} 名`
  return '公告資料無效'
}
const visibleTeams = computed(() => registration.value?.teams ?? [])
const activeTeamCount = computed(() => visibleTeams.value.filter(team => team.status !== 'withdrawn').length)
const currentTeam = computed(() => registration.value && session.userId.value
  ? selectActiveTeamForCaller(registration.value.teams, registration.value.ownTeam, session.userId.value)
  : null)
const canLookupParticipantGame = computed(() => {
  return canShowCompetitionParticipantGameEntry(
    registration.value?.lifecycle,
    session.userId.value,
    currentTeam.value,
  )
})
const isCurrentCaptain = computed(() => Boolean(currentTeam.value
  && currentTeam.value.captain === session.userId.value))
const ownDeckSubmitted = computed(() => {
  const ownTeam = registration.value?.ownTeam
  return Boolean(ownTeam
    && currentTeam.value
    && ownTeam.teamId === currentTeam.value.teamId
    && ownTeam.ownDeckSubmitted)
})
const actionsDisabled = computed(() => loading.value || mutationBusy.value || Boolean(loadError.value)
  || Boolean(pendingCommand.value) || Boolean(roundPublicationPending.value))

async function bootstrap() {
  sessionError.value = ''
  sessionLoaded.value = false
  try {
    await session.refresh()
    if (disposed) return
    sessionLoaded.value = true
    if (session.userId.value) await loadRegistration()
  } catch (error) {
    if (disposed) return
    sessionError.value = presentApiError(error, '無法確認登入狀態，請重試。')
    sessionLoaded.value = true
  }
}

async function loadRegistration() {
  const capturedCompetitionId = competitionId.value
  const capturedUserId = session.userId.value
  if (disposed || !capturedCompetitionId || !capturedUserId) return
  registrationReadController?.abort()
  const controller = new AbortController()
  registrationReadController = controller
  const sequence = ++registrationReadSequence
  const accepted = () => !disposed && sequence === registrationReadSequence && !controller.signal.aborted
    && competitionId.value === capturedCompetitionId && session.userId.value === capturedUserId
  loading.value = true
  loadError.value = ''
  notFound.value = false
  mutationError.value = ''
  notice.value = ''
  try {
    const body = createRegistrationViewBody(registrationInCurrentScope.value ? registration.value?.revision ?? 1 : 1)
    const response: unknown = await $fetch(
      `/api/competitions/${encodeURIComponent(capturedCompetitionId)}/commands`,
      { method: 'POST', body, retry: 0, signal: controller.signal },
    )
    if (!accepted()) return
    registration.value = parseCompetitionRegistrationResponse(
      response,
      capturedCompetitionId,
      body.commandId,
      capturedUserId,
    )
    registrationIdentity.value = { competitionId: capturedCompetitionId, userId: capturedUserId }
  } catch (error) {
    if (!accepted()) return
    notFound.value = apiErrorStatus(error) === 404
    loadError.value = notFound.value
      ? '找不到這個賽事，或賽事 ID 無效。'
      : presentApiError(error, '無法載入賽事狀態，請重試。')
  } finally {
    if (accepted()) {
      loading.value = false
      registrationReadController = null
    }
  }
}

function onRoundPublicationPendingChange(value: CompetitionRoundPublicationIntent | null) {
  // 原 intent 只由公告元件的命令結果更新；GET 與目前輪次進度不會清除。
  roundPublicationPending.value = value
  if (!value) roundPublicationNavigationMessage.value = ''
}

function onRoundPublicationUnresolved(value: CompetitionRoundPublicationIntent) {
  roundPublicationPending.value = value
  roundPublicationNavigationMessage.value = '輪次公告的結果仍未確認，其他寫入操作暫停；重新整理只更新狀態，不會取消原命令。請以原主辦帳號手動重試相同公告命令。'
}

async function mutate(action: RegistrationMutationAction) {
  if (actionsDisabled.value || !registration.value) return
  try {
    pendingCommand.value = createRegistrationCommandBody(registration.value.revision, action)
  } catch {
    mutationError.value = '目前賽事版本無效，請重新載入狀態。'
    return
  }
  await sendPendingCommand()
}

async function retryPendingCommand() {
  if (!pendingCommand.value || mutationBusy.value || roundPublicationPending.value) return
  await sendPendingCommand()
}

async function sendPendingCommand() {
  const command = pendingCommand.value
  if (!command || !competitionId.value || roundPublicationPending.value) return
  mutationBusy.value = true
  mutationError.value = ''
  notice.value = ''
  try {
    const response: unknown = await $fetch(
      `/api/competitions/${encodeURIComponent(competitionId.value)}/commands`,
      { method: 'POST', body: command },
    )
    const nextState = parseCompetitionRegistrationResponse(
      response,
      competitionId.value,
      command.commandId,
      session.userId.value,
    )
    registration.value = nextState
    pendingCommand.value = null
    otherMemberIdsText.value = ''
  } catch (error) {
    if (isRevisionConflict(error)) {
      pendingCommand.value = null
      try {
        await loadRegistration()
        if (!loadError.value) notice.value = '賽事狀態已更新，已載入最新版本；請確認目前資料後再操作。'
      } catch {
        loadError.value = '賽事狀態已更新，但無法取得最新資料。請重新載入後再操作。'
      }
      mutationError.value = '這次操作遇到版本衝突，沒有自動重送；請依最新狀態重新操作。'
    } else {
      const status = apiErrorStatus(error)
      if (status !== undefined && status >= 400 && status < 500) pendingCommand.value = null
      mutationError.value = presentApiError(error, pendingCommand.value
        ? '操作結果尚未確認。可用同一命令安全重試。'
        : '伺服器拒絕了這項操作；請依錯誤訊息調整後再試。')
    }
  } finally {
    mutationBusy.value = false
  }
}

function registerTeam() {
  if (!registration.value || !session.userId.value) return
  try {
    const action = buildRegisterTeamAction(
      crypto.randomUUID(),
      session.userId.value,
      otherMemberIdsText.value,
      registration.value.teamSize,
    )
    void mutate(action)
  } catch (error) {
    mutationError.value = error instanceof Error ? error.message : '請檢查隊員 ID。'
  }
}

function allMembersConfirmed(team: CompetitionPublicTeam): boolean {
  return team.members.every(member => team.confirmedMembers.includes(member))
}

function lifecycleLabel(value: CompetitionRegistrationState['lifecycle']): string {
  return { draft: '尚未開放報名', open: '報名與資料確認中', locked: '名單已鎖定' }[value]
}

function teamStatusLabel(value: CompetitionTeamStatus): string {
  return {
    registered: '已報名',
    checkedIn: '資料完成確認已完成',
    withdrawn: '已退出',
    notCheckedIn: '未納入賽程',
  }[value]
}

function rulesetLabel(value: string): string {
  return value === 'base' ? '基本規則' : '未識別的規則基底'
}

function ruleModuleLabels(moduleIds: string[]): string {
  if (!moduleIds.length) return '無額外模組'
  return moduleIds.map(id => presentationForRuleModule(id).label).join('、')
}

watch(() => [competitionId.value, session.userId.value], () => {
  registrationReadSequence += 1
  registrationReadController?.abort()
  registrationReadController = null
  registration.value = null
  registrationIdentity.value = null
  loading.value = false
  loadError.value = ''
  notFound.value = false
  if (sessionLoaded.value && session.userId.value) void loadRegistration()
}, { flush: 'sync' })

onBeforeRouteUpdate(to => {
  const nextCompetitionId = typeof to.params.id === 'string' ? to.params.id.trim() : ''
  if (pendingCommand.value && nextCompetitionId !== competitionId.value) {
    notice.value = '報名命令的結果仍未確認，請先在目前賽事確認結果；尚不能切換賽事。'
    return false
  }
})

onBeforeUnmount(() => {
  disposed = true
  registrationReadSequence += 1
  registrationReadController?.abort()
})

onMounted(() => {
  void bootstrap()
})
</script>

<style scoped>
@reference "../../assets/css/main.css";

.competition-detail-page { @apply mx-auto grid w-full max-w-[1050px] gap-6 px-6 py-12 max-[600px]:gap-4 max-[600px]:px-4 max-[600px]:py-7; }
.detail-heading { @apply flex items-end justify-between gap-5 max-[600px]:items-start; }
.detail-heading > div { @apply grid gap-2; }
.detail-heading h1 { @apply m-0 font-serif text-3xl max-[600px]:text-2xl; }
.eyebrow { @apply m-0 text-xs tracking-[.16em] text-gold-light; }
.back-link { @apply w-fit text-sm text-gold-light no-underline hover:underline; }
.muted { @apply m-0 text-sm leading-6; color: var(--app-text-muted); }
.identifier { @apply break-all font-mono text-[.92em]; }
.panel { @apply grid gap-5 border border-line bg-panel p-6 max-[600px]:gap-4 max-[600px]:p-4; border-radius: 16px; box-shadow: var(--app-shadow-md); }
.announcement-title-row, .teams-heading { @apply flex items-start justify-between gap-4; }
.announcement-plan-card { @apply grid gap-4; }
.announcement-plan-card h2, .announced-stage h3 { @apply m-0 font-serif text-xl; }
.policy-line { @apply m-0 text-sm leading-6; }
.ranking-score-policy ul { @apply mt-2 grid gap-1 pl-5; list-style: disc; }
.announced-stage-list { @apply grid gap-4 p-0; list-style: none; }
.announced-stage { @apply grid gap-3 border border-line bg-panel p-5; border-radius: 12px; }
.announced-stage h3 { @apply text-base; }
.stage-facts { @apply m-0 grid grid-cols-2 gap-3 max-[620px]:grid-cols-1; }
.stage-facts > div { @apply grid gap-1; }
.stage-facts dt { @apply text-xs; color: var(--app-text-muted); }
.stage-facts dd { @apply m-0 break-words text-sm; }
.qualification-fact ul { @apply m-0 list-disc pl-5; }
.announcement-title-row h2, .teams-heading h2 { @apply m-0 font-serif text-xl; }
.lifecycle-badge, .team-status { @apply rounded-full border px-3 py-1 text-xs; border-color: var(--app-border-strong); color: var(--app-text); }
.state-open, .team-checkedIn { border-color: var(--app-accent); color: var(--app-accent-strong); }
.state-locked, .team-withdrawn, .team-notCheckedIn { color: var(--app-text-muted); }
.announcement-facts { @apply m-0 grid grid-cols-2 gap-x-7 gap-y-4 max-[620px]:grid-cols-1; }
.announcement-facts > div { @apply grid min-w-0 gap-1; }
.announcement-facts dt { @apply text-xs; color: var(--app-text-muted); }
.announcement-facts dd { @apply m-0 break-words text-sm; }
.module-fact { @apply col-span-2 max-[620px]:col-span-1; }
.deck-notice, .register-panel { @apply grid gap-2 rounded-xl border border-line p-4; background: var(--app-surface-muted); }
.deck-notice strong, .register-panel h3 { @apply m-0 text-sm text-gold-light; }
.deck-notice p, .register-panel p { @apply m-0 text-xs leading-5; color: var(--app-text-muted); }
.organizer-actions { @apply flex flex-wrap items-center justify-between gap-3 border-t border-line pt-4; }
.revision-label { @apply text-xs; color: var(--app-text-muted); }
.empty-state { @apply rounded-xl border border-dashed border-line p-6 text-center text-sm; color: var(--app-text-muted); }
.team-list { @apply grid gap-4; }
.team-card { @apply grid gap-4 rounded-xl border border-line p-4; background: var(--app-surface-muted); }
.team-title { @apply flex items-start justify-between gap-4; }
.team-title h3 { @apply m-0 break-all text-sm font-semibold; }
.team-title p { @apply mt-1 text-xs; }
.member-list { @apply m-0 grid gap-2 p-0; list-style: none; }
.member-list li { @apply flex min-w-0 items-center justify-between gap-3 rounded-lg border border-line bg-panel px-3 py-2 text-sm; }
.confirmation { @apply shrink-0 text-xs; color: var(--app-text-muted); }
.confirmation.confirmed { color: var(--app-accent-strong); }
.team-progress { @apply text-xs; }
.team-actions { @apply flex flex-wrap gap-2 border-t border-line pt-3; }
.register-form { @apply grid justify-items-start gap-3; }
.register-form label { @apply grid w-full max-w-xl gap-2 text-sm; }
.text-input { @apply min-h-12 w-full px-3.5; border: 1px solid var(--app-border); border-radius: 9px; background: var(--app-input); color: var(--app-text); }
.text-area { @apply min-h-24 resize-y py-3; }
.status-panel { @apply justify-items-start; }
.status-panel p { @apply m-0 text-sm; }
.error-message { border-color: var(--app-danger); background: var(--app-danger-surface); }
.notice-panel { @apply text-sm; border-color: var(--app-accent); }
.danger-button { @apply min-h-[50px] border px-5 text-sm; border-color: var(--app-danger); border-radius: 10px; background: transparent; color: var(--app-danger); }
.danger-button:hover { background: var(--app-danger-surface); }
.link-button { @apply inline-flex items-center justify-center no-underline; }
.refresh-button { @apply min-h-10; }

@media (max-width: 600px) {
  .detail-heading { @apply grid; }
  .refresh-button { @apply w-full; }
  .team-title { @apply flex-col; }
  .team-actions > button { @apply w-full; }
  .organizer-actions { @apply items-stretch; }
  .organizer-actions > button { @apply w-full; }
  .module-fact { grid-column: auto; }
}
</style>
