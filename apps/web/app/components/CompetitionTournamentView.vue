<template>
  <section class="tournament-view" aria-labelledby="tournament-view-title">
    <header class="view-heading">
      <div class="heading-copy">
        <p class="eyebrow">伺服器目前賽程</p>
        <h2 id="tournament-view-title">目前賽程、輪次與排名</h2>
        <p class="muted">只讀顯示賽事服務提供的階段與結果；不會建立或變更賽程。</p>
      </div>
      <button
        class="refresh-button"
        type="button"
        :disabled="!canRead || loading"
        @click="loadTournament"
      >
        {{ loading ? '查詢中…' : '重新查詢賽程' }}
      </button>
    </header>

    <p v-if="!canRead" class="state-message" role="status">
      賽事或登入狀態尚未確認，暫不查詢賽程。
    </p>
    <p v-else-if="!mounted || loading" class="state-message" role="status" aria-live="polite">
      {{ loading ? '正在載入目前賽程…' : '準備載入目前賽程…' }}
    </p>
    <p
      v-else-if="loadError"
      class="state-message"
      :class="{ 'not-ready': loadError.kind === 'notReady', 'error': loadError.kind !== 'notReady' }"
      :role="loadError.kind === 'notReady' ? 'status' : 'alert'"
      aria-live="polite"
    >
      {{ loadError.message }}
    </p>

    <div v-else-if="tournament" class="tournament-content">
      <p class="revision-note" role="status">賽程版本 {{ tournament.revision }}</p>

      <article
        v-for="stage in tournament.stages"
        :key="stage.config.stageId"
        class="stage-card"
        :aria-labelledby="stageHeadingId(stage.config.stageId)"
      >
        <header class="stage-heading">
          <div class="stage-title">
            <p class="eyebrow">賽事階段</p>
            <h3 :id="stageHeadingId(stage.config.stageId)">
              {{ tournamentFormatLabel(stage.config.format) }}
            </h3>
            <p class="identifier">{{ stage.config.stageId }}</p>
          </div>
          <span class="status-badge">{{ tournamentStageStatusLabel(stage.status) }}</span>
        </header>

        <section class="config-section" :aria-labelledby="configHeadingId(stage.config.stageId)">
          <h4 :id="configHeadingId(stage.config.stageId)">階段設定</h4>
          <dl class="config-list">
            <div>
              <dt>配對隨機種子</dt>
              <dd>{{ stage.config.randomSeed }}</dd>
            </div>
            <div>
              <dt>交手賽制</dt>
              <dd>{{ matchFormatLabel(stage.config.matchFormat) }}</dd>
            </div>
            <div>
              <dt>隊伍主時間</dt>
              <dd>{{ stage.config.clockConfig.mainTimeMs === 0 ? '不限時' : tournamentTimeLimitLabel(stage.config.clockConfig.mainTimeMs) }}</dd>
            </div>
            <div>
              <dt>行動讀秒</dt>
              <dd>{{ tournamentTimeLimitLabel(stage.config.clockConfig.actionTimeMs) }}</dd>
            </div>
            <div>
              <dt>回合外選擇時間</dt>
              <dd>{{ tournamentTimeLimitLabel(stage.config.clockConfig.outOfTurnChoiceTimeMs) }}</dd>
            </div>
            <div>
              <dt>瑞士輪數</dt>
              <dd>{{ stage.config.swissRounds ?? '未設定' }}</dd>
            </div>
            <div>
              <dt>賽程世代</dt>
              <dd>{{ stage.scheduleGeneration }}</dd>
            </div>
          </dl>

          <div class="seed-list-block">
            <h5>種子與隊伍</h5>
            <ul class="seed-list">
              <li v-for="team in stage.config.teams" :key="team.teamId">
                <span>種子 {{ team.seed }}</span>
                <span class="identifier">{{ team.teamId }}</span>
              </li>
            </ul>
          </div>

          <div class="swiss-policy-block">
            <h5>瑞士排名政策</h5>
            <template v-if="stage.config.swissPolicy">
              <dl class="config-list">
                <div>
                  <dt>勝／和／負／輪空積分</dt>
                  <dd>
                    {{ stage.config.swissPolicy.matchPoints.win }}／{{ stage.config.swissPolicy.matchPoints.draw }}／{{ stage.config.swissPolicy.matchPoints.loss }}／{{ stage.config.swissPolicy.matchPoints.bye }}
                  </dd>
                </div>
                <div>
                  <dt>對手勝率的和局折算</dt>
                  <dd>{{ stage.config.swissPolicy.drawCredit.numerator }}／{{ stage.config.swissPolicy.drawCredit.denominator }}</dd>
                </div>
                <div>
                  <dt>對手勝率下限／上限</dt>
                  <dd>
                    {{ percentLabel(stage.config.swissPolicy.opponentWinRateFloorPercent) }}／{{ percentLabel(stage.config.swissPolicy.opponentWinRateCapPercent) }}
                  </dd>
                </div>
                <div>
                  <dt>計算對手勝率時排除輪空</dt>
                  <dd>{{ stage.config.swissPolicy.excludeByesFromOpponentWinRate ? '是' : '否' }}</dd>
                </div>
                <div class="wide-fact">
                  <dt>同分比較順序</dt>
                  <dd>
                    {{ stage.config.swissPolicy.tieBreakers.length
                      ? stage.config.swissPolicy.tieBreakers.map(swissTieBreakerLabel).join(' → ')
                      : '未設定' }}
                  </dd>
                </div>
              </dl>
            </template>
            <p v-else class="muted">此階段未提供瑞士排名政策。</p>
          </div>
        </section>

        <p v-if="stage.hasScheduleWarning" class="review-note" role="status">
          賽程狀態待主辦方或裁判確認；此頁不推定缺少配對的原因或後續名次。
        </p>
        <p v-if="singleEliminationDoubleLossFinal(stage)" class="review-note" role="status">
          決賽雙負，冠軍空缺
        </p>
        <p v-if="stage.correctionCount > 0" class="muted correction-note">
          有 {{ stage.correctionCount }} 筆賽果更正紀錄；更正理由與歷史內容不在此頁展開。
        </p>

        <section class="rounds-section" :aria-labelledby="roundsHeadingId(stage.config.stageId)">
          <h4 :id="roundsHeadingId(stage.config.stageId)">目前輪次</h4>
          <p v-if="!stage.rounds.length" class="muted">目前沒有可顯示輪次；不推定尚未開賽或已結束。</p>
          <div v-else class="round-list">
            <article v-for="(round, roundIndex) in stage.rounds" :key="`current-${round.kind}-${round.roundNumber}-${roundIndex}`" class="round-card">
              <h5>{{ tournamentRoundKindLabel(round.kind) }}・第 {{ round.roundNumber }} 輪</h5>
              <p v-if="!round.matches.length" class="muted">此輪目前沒有可顯示交手。</p>
              <div v-else class="match-list">
                <article v-for="match in round.matches" :key="match.matchId" class="match-card">
                  <header class="match-heading">
                    <h6>交手 {{ match.sequence }} <span class="identifier">{{ match.matchId }}</span></h6>
                    <span class="status-badge">{{ tournamentMatchStatusLabel(match.status) }}</span>
                  </header>
                  <p class="match-fact">
                    隊伍：<span v-for="(teamId, index) in match.participants" :key="teamId">
                      <span v-if="index > 0">、</span><span class="identifier">{{ teamId }}</span>
                    </span>
                  </p>
                  <p class="match-fact">
                    賽果：{{ tournamentMatchResultLabel(match.result) }}
                    <span
                      v-if="match.result?.type === 'doubleLoss' && isEliminationFormat(stage.config.format)
                        && singleEliminationDoubleLossFinal(stage)?.matchId !== match.matchId"
                    >
                      （淘汰賽待裁判處理）
                    </span>
                  </p>
                  <p class="match-fact">開局狀態：{{ match.started ? '已開始' : '尚未開始' }}</p>
                  <p v-if="match.cancellationReason !== null" class="muted">
                    取消原因已記錄；詳細內容不在此頁顯示。
                  </p>
                </article>
              </div>
            </article>
          </div>
        </section>

        <section class="standings-section" :aria-labelledby="standingsHeadingId(stage.config.stageId)">
          <h4 :id="standingsHeadingId(stage.config.stageId)">伺服器排名與積分</h4>
          <p class="muted">依賽事服務提供的順序顯示；名次只採用服務提供的值，不由隊伍位置推算。</p>
          <p v-if="!stage.standings.length" class="muted">目前沒有可顯示排名。</p>
          <ol v-else class="standing-list">
            <li v-for="standing in stage.standings" :key="standing.teamId" class="standing-card">
              <header class="standing-heading">
                <h5 class="identifier">{{ standing.teamId }}</h5>
                <span class="rank-label">{{ standing.rank === null ? '排名未確定' : `第 ${standing.rank} 名` }}</span>
              </header>
              <dl class="standing-facts">
                <div><dt>賽事積分</dt><dd>{{ standing.matchPoints }}</dd></div>
                <div><dt>勝／和／負</dt><dd>{{ standing.wins }}／{{ standing.draws }}／{{ standing.losses }}</dd></div>
                <div><dt>勝／負局</dt><dd>{{ standing.gameWins }}／{{ standing.gameLosses }}</dd></div>
                <div><dt>淨勝局</dt><dd>{{ standing.netGameWins }}</dd></div>
                <div><dt>對手勝率基點</dt><dd>{{ standing.opponentWinRateBasisPoints ?? '未提供' }}</dd></div>
                <div><dt>已退賽</dt><dd>{{ standing.dropped ? '是' : '否' }}</dd></div>
              </dl>
            </li>
          </ol>
        </section>

        <section v-if="stage.supersededRounds.length" class="superseded-section" :aria-labelledby="supersededHeadingId(stage.config.stageId)">
          <h4 :id="supersededHeadingId(stage.config.stageId)">已被替代的舊輪次</h4>
          <p class="muted">以下只保留伺服器記錄供辨識，不代表目前有效配對。</p>
          <div class="round-list">
            <article v-for="(round, roundIndex) in stage.supersededRounds" :key="`old-${round.kind}-${round.roundNumber}-${roundIndex}`" class="round-card old-round">
              <h5>{{ tournamentRoundKindLabel(round.kind) }}・第 {{ round.roundNumber }} 輪</h5>
              <div class="match-list">
                <article v-for="match in round.matches" :key="match.matchId" class="match-card">
                  <header class="match-heading">
                    <h6>交手 {{ match.sequence }} <span class="identifier">{{ match.matchId }}</span></h6>
                    <span class="status-badge">{{ tournamentMatchStatusLabel(match.status) }}・舊輪次</span>
                  </header>
                  <p class="match-fact">
                    隊伍：<span v-for="(teamId, index) in match.participants" :key="teamId">
                      <span v-if="index > 0">、</span><span class="identifier">{{ teamId }}</span>
                    </span>
                  </p>
                  <p class="match-fact">賽果：{{ tournamentMatchResultLabel(match.result) }}</p>
                </article>
              </div>
            </article>
          </div>
        </section>
      </article>
    </div>
  </section>
</template>

<script setup lang="ts">
import {
  parseCompetitionTournamentViewResponse,
  presentCompetitionTournamentViewError,
  singleEliminationDoubleLossFinal,
  swissTieBreakerLabel,
  tournamentFormatLabel,
  tournamentMatchResultLabel,
  tournamentMatchStatusLabel,
  tournamentRoundKindLabel,
  tournamentStageStatusLabel,
  tournamentTimeLimitLabel,
  type CompetitionTournamentViewData,
  type CompetitionTournamentViewLoadError,
  type TournamentViewStageConfig,
} from '~/lib/competition-tournament-view'

const props = defineProps<{
  competitionId: string
  /** 僅用於使登入者切換時失效；不會送入 GET。 */
  sessionIdentity: string | null
  /** 只作本地查詢 context 版本；不會送入 GET。 */
  contextRevision: number | null
  /** 父頁已確認 session 與公告 projection 載入完成，不代表 API 授權。 */
  contextReady: boolean
}>()

const mounted = ref(false)
const loading = ref(false)
const tournament = ref<CompetitionTournamentViewData | null>(null)
const loadError = ref<CompetitionTournamentViewLoadError | null>(null)
let requestSequence = 0
let activeRequest: AbortController | null = null

const normalizedCompetitionId = computed(() => normalizeCompetitionId(props.competitionId))
const canRead = computed(() => props.contextReady
  && Boolean(normalizedCompetitionId.value)
  && Boolean(props.sessionIdentity?.trim())
  && typeof props.contextRevision === 'number'
  && Number.isSafeInteger(props.contextRevision)
  && props.contextRevision > 0)
const requestContext = computed(() => JSON.stringify([
  normalizedCompetitionId.value,
  props.sessionIdentity,
  props.contextRevision,
  props.contextReady,
]))

async function loadTournament() {
  if (!canRead.value || loading.value) return
  const competitionId = normalizedCompetitionId.value
  if (!competitionId) return

  const identity = requestContext.value
  const sequence = ++requestSequence
  const controller = new AbortController()
  activeRequest = controller
  loading.value = true
  tournament.value = null
  loadError.value = null

  try {
    const response: unknown = await $fetch(
      `/api/competitions/${encodeURIComponent(competitionId)}/tournament`,
      { method: 'GET', signal: controller.signal, retry: 0 },
    )
    if (!isCurrentRequest(sequence, identity)) return
    tournament.value = parseCompetitionTournamentViewResponse(response, competitionId)
  } catch (error) {
    if (!isCurrentRequest(sequence, identity)) return
    loadError.value = presentCompetitionTournamentViewError(error)
  } finally {
    if (isCurrentRequest(sequence, identity)) {
      loading.value = false
      activeRequest = null
    }
  }
}

function invalidateRequest() {
  requestSequence += 1
  activeRequest?.abort()
  activeRequest = null
  loading.value = false
  tournament.value = null
  loadError.value = null
}

function isCurrentRequest(sequence: number, identity: string): boolean {
  return sequence === requestSequence && identity === requestContext.value
}

function normalizeCompetitionId(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const normalized = value.trim()
  return normalized ? normalized : null
}

function stageHeadingId(stageId: string): string {
  return `tournament-stage-${encodeURIComponent(stageId)}`
}

function configHeadingId(stageId: string): string {
  return `tournament-config-${encodeURIComponent(stageId)}`
}

function roundsHeadingId(stageId: string): string {
  return `tournament-rounds-${encodeURIComponent(stageId)}`
}

function standingsHeadingId(stageId: string): string {
  return `tournament-standings-${encodeURIComponent(stageId)}`
}

function supersededHeadingId(stageId: string): string {
  return `tournament-superseded-${encodeURIComponent(stageId)}`
}

function matchFormatLabel(value: TournamentViewStageConfig['matchFormat']): string {
  return value.type === 'fixedGames' ? `固定 ${value.value} 局` : `先取得 ${value.value} 勝`
}

function isEliminationFormat(format: CompetitionTournamentViewData['stages'][number]['config']['format']): boolean {
  return format === 'singleElimination' || format === 'doubleElimination'
}

function percentLabel(value: number | null): string {
  return value === null ? '未設定' : `${value}%`
}

watch(requestContext, () => {
  invalidateRequest()
  if (!mounted.value || !canRead.value) return
  const sequence = requestSequence
  // loading 包含排程等待，避免清掉舊查詢後、新 GET 啟動前出現可操作空窗。
  loading.value = true
  // 同次父頁更新的多個 props 合併後才發出一次新 GET。
  void nextTick(() => {
    if (sequence !== requestSequence) return
    loading.value = false
    if (mounted.value && canRead.value) void loadTournament()
  })
}, { flush: 'sync' })

onMounted(() => {
  mounted.value = true
  if (canRead.value) void loadTournament()
})

onBeforeUnmount(() => {
  mounted.value = false
  invalidateRequest()
})
</script>

<style scoped>
.tournament-view {
  display: grid;
  gap: 1rem;
  min-width: 0;
  width: 100%;
  border: 1px solid var(--app-border);
  border-radius: 16px;
  padding: 1.25rem;
  background: var(--app-surface);
  color: var(--app-text);
  box-shadow: var(--app-shadow-md);
}

.view-heading, .stage-heading, .match-heading, .standing-heading {
  display: flex;
  min-width: 0;
  align-items: flex-start;
  justify-content: space-between;
  gap: .75rem;
}

.heading-copy, .tournament-content, .stage-card, .config-section, .rounds-section,
.standings-section, .superseded-section, .round-list, .match-list, .standing-list,
.config-list, .standing-facts, .seed-list, .swiss-policy-block {
  display: grid;
  min-width: 0;
  gap: .75rem;
}

.heading-copy h2, .stage-title h3, .config-section h4, .rounds-section h4,
.standings-section h4, .superseded-section h4, .seed-list-block h5,
.swiss-policy-block h5, .round-card h5, .match-heading h6, .standing-heading h5 {
  margin: 0;
}

.heading-copy h2 { font-family: var(--font-serif); font-size: 1.3rem; }
.stage-title h3 { font-family: var(--font-serif); font-size: 1.15rem; }
.eyebrow { margin: 0; color: var(--app-accent-strong); font-size: .75rem; letter-spacing: .12em; }
.muted, .state-message, .revision-note, .match-fact, .rank-label { margin: 0; font-size: .875rem; line-height: 1.55; }
.muted { color: var(--app-text-muted); }
.identifier { min-width: 0; overflow-wrap: anywhere; font-family: ui-monospace, SFMono-Regular, monospace; }
.stage-title .identifier { color: var(--app-text-muted); font-size: .82rem; }

.refresh-button {
  flex: 0 0 auto;
  min-height: 2.75rem;
  border: 1px solid var(--app-border-strong);
  border-radius: 9px;
  padding: .55rem .85rem;
  background: var(--app-control);
  color: var(--app-text);
  cursor: pointer;
}
.refresh-button:disabled { cursor: not-allowed; opacity: .55; }
.refresh-button:focus-visible { outline: 3px solid var(--app-focus-ring); outline-offset: 2px; }

.state-message { border-radius: 9px; padding: .8rem 1rem; background: var(--app-surface-muted); }
.state-message.error { border: 1px solid var(--app-danger); background: var(--app-danger-surface); }
.state-message.not-ready { border: 1px solid var(--app-border); }
.revision-note { color: var(--app-text-muted); }
.stage-card { gap: 1rem; min-width: 0; border: 1px solid var(--app-border); border-radius: 12px; padding: 1rem; background: var(--app-surface-raised); }
.status-badge, .rank-label { flex: 0 0 auto; border: 1px solid var(--app-border-strong); border-radius: 999px; padding: .25rem .65rem; color: var(--app-text); font-size: .75rem; }
.config-section, .rounds-section, .standings-section, .superseded-section { border-top: 1px solid var(--app-border); padding-top: .9rem; }
.config-section h4, .rounds-section h4, .standings-section h4, .superseded-section h4 { font-size: 1rem; }
.config-list { grid-template-columns: repeat(2, minmax(0, 1fr)); gap: .65rem 1rem; margin: 0; }
.config-list > div, .standing-facts > div { display: grid; min-width: 0; gap: .2rem; }
.config-list dt, .standing-facts dt { color: var(--app-text-muted); font-size: .75rem; }
.config-list dd, .standing-facts dd { margin: 0; overflow-wrap: anywhere; font-size: .875rem; }
.wide-fact { grid-column: 1 / -1; }
.seed-list-block, .swiss-policy-block { min-width: 0; }
.seed-list, .standing-list { margin: 0; padding: 0; list-style: none; }
.seed-list li { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: .75rem; border: 1px solid var(--app-border); border-radius: 8px; padding: .55rem .7rem; background: var(--app-surface-muted); font-size: .82rem; }
.review-note { margin: 0; border-left: 3px solid var(--app-accent); padding: .65rem .8rem; background: var(--app-accent-soft); font-size: .875rem; line-height: 1.55; }
.correction-note { font-size: .8rem; }
.round-card, .match-card, .standing-card { display: grid; min-width: 0; gap: .65rem; border: 1px solid var(--app-border); border-radius: 9px; padding: .75rem; background: var(--app-surface-muted); }
.round-card h5 { font-size: .9rem; }
.match-heading { align-items: center; }
.match-heading h6 { display: grid; min-width: 0; gap: .2rem; font-size: .82rem; }
.match-fact { overflow-wrap: anywhere; }
.standing-heading { align-items: center; }
.standing-heading h5 { min-width: 0; overflow-wrap: anywhere; font-size: .88rem; }
.standing-facts { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.old-round { border-style: dashed; opacity: .82; }

@media (max-width: 600px) {
  .tournament-view { padding: 1rem; }
  .view-heading { display: grid; }
  .refresh-button { width: 100%; }
  .stage-heading { flex-wrap: wrap; }
  .config-list { grid-template-columns: minmax(0, 1fr); }
  .wide-fact { grid-column: auto; }
  .status-badge { white-space: normal; }
  .standing-facts { grid-template-columns: minmax(0, 1fr); }
  .match-heading { align-items: flex-start; flex-direction: column; }
}
</style>
