<template>
  <section class="tournament-view" aria-labelledby="tournament-view-title">
    <header class="view-heading">
      <div class="heading-copy">
        <p class="eyebrow">伺服器目前賽程</p>
        <h2 id="tournament-view-title">目前賽程、輪次與結果</h2>
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
      {{ registrationLocked && loadError.kind === 'notReady'
        ? '報名名單已鎖定，系統正在依公告自動抽籤並建立首輪；完成後此頁會自動更新。'
        : loadError.message }}
    </p>

    <div v-else-if="tournament" class="tournament-content">
      <p class="revision-note" role="status">賽程版本 {{ tournament.revision }}</p>

      <section v-if="tournament.publication" class="publication-card" aria-labelledby="publication-title">
        <p class="eyebrow">賽事結果</p>
        <h3 id="publication-title">本場賽事已完成</h3>
        <p class="muted">完成時間：{{ new Date(tournament.publication.completedAtTrustedTimeMs).toLocaleString('zh-TW') }}</p>

        <section class="published-replays" aria-labelledby="published-replays-title">
          <h4 id="published-replays-title">公開重播</h4>
          <p v-if="!tournament.publication.replays.length" class="muted">本場賽事沒有可公開的對局重播。</p>
          <ul v-else class="published-replay-list">
            <li v-for="(replay, replayIndex) in tournament.publication.replays" :key="replay.replayId">
              <NuxtLink :to="`/replays/${encodeURIComponent(replay.replayId)}`">
                重播 {{ replayIndex + 1 }}・第 {{ replay.gameNumber }} 局
              </NuxtLink>
            </li>
          </ul>
        </section>

        <section class="published-decks" aria-labelledby="published-decks-title">
          <h4 id="published-decks-title">凍結牌組</h4>
          <p v-if="!tournament.publication.lockedDecks.length" class="muted">沒有可公開的凍結牌組。</p>
          <p v-else-if="publishedCardCatalogState === 'idle' || publishedCardCatalogState === 'loading'" class="muted" role="status">
            正在載入牌組卡牌名稱…
          </p>
          <p v-else-if="publishedCardCatalogState === 'unavailable'" class="muted" role="status">
            卡牌名稱暫時無法載入；未能對應的項目會顯示為「未知卡牌」。
          </p>
          <button
            v-if="publishedCardCatalogState === 'unavailable'"
            class="catalog-retry-button"
            type="button"
            @click="loadPublishedCardCatalog"
          >
            重新載入卡牌名稱
          </button>
          <div v-if="publishedCardCatalogState === 'ready' || publishedCardCatalogState === 'unavailable'" class="published-deck-list">
            <article v-for="(team, teamIndex) in tournament.publication.lockedDecks" :key="team.teamId" class="published-deck">
              <h5>隊伍 {{ teamIndex + 1 }}</h5>
              <ul>
                <li v-for="(deck, deckIndex) in team.decks" :key="`${deck.player}-${deck.name}`">
                  <strong>{{ deck.name }}</strong>・隊員 {{ deckIndex + 1 }}
                  <p v-if="!deck.cards.length" class="published-card-list muted">空牌組</p>
                  <ul v-else class="published-card-list">
                    <li v-for="card in publishedDeckCardLabels(deck.cards)" :key="card.id">
                      {{ card.name }}<span v-if="card.count > 1"> × {{ card.count }}</span>
                    </li>
                  </ul>
                </li>
              </ul>
            </article>
          </div>
        </section>
      </section>

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
          <span class="status-badge">{{ tournamentStageStatusLabel(stage.status, stage.resolution) }}</span>
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
        <p v-if="stage.resolution" class="review-note" role="status">
          {{ tournamentStageResolutionLabel(stage.resolution) }}
        </p>
        <p v-if="singleEliminationDoubleLossFinal(stage)" class="review-note" role="status">
          決賽雙負，冠軍空缺
        </p>
        <p v-if="singleEliminationVacancySummary(stage)" class="review-note" role="status">
          {{ singleEliminationVacancySummary(stage) }}
        </p>
        <p v-if="doubleEliminationVacancySummary(stage)" class="review-note" role="status">
          {{ doubleEliminationVacancySummary(stage) }}
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
                    <h6>
                      交手 {{ match.sequence }}
                      <span v-if="match.purpose === 'bronze'" class="purpose-label">季軍賽</span>
                      <span v-else-if="round.matches.some(candidate => candidate.purpose === 'bronze')" class="purpose-label">冠軍賽</span>
                      <span class="identifier">{{ match.matchId }}</span>
                    </h6>
                    <span class="status-badge">{{ tournamentMatchStatusLabel(match.status) }}</span>
                  </header>
                  <p class="match-fact">
                    隊伍：<span v-for="(teamId, index) in match.participants" :key="teamId">
                      <span v-if="index > 0">、</span><span class="identifier">{{ teamId }}</span>
                    </span>
                  </p>
                  <p class="match-fact">
                    賽果：{{ tournamentResultLabel(stage.config, match.result) }}
                  </p>
                  <p class="match-fact">開局狀態：{{ match.started ? '已開始' : '尚未開始' }}</p>
                  <CompetitionParticipantGameEntry
                    v-if="isCurrentParticipantMatch(match)"
                    :competition-id="competitionId"
                    :match-id="match.matchId"
                    :session-user-id="sessionIdentity"
                    :participant-team-id="participantTeamId ?? null"
                    :participant-is-captain="participantIsCaptain"
                    enabled
                  />
                  <p v-if="match.cancellationReason !== null" class="muted">
                    取消原因已記錄；詳細內容不在此頁顯示。
                  </p>
                </article>
              </div>
            </article>
          </div>
        </section>

        <section
          v-if="stage.config.format === 'swiss' && stage.status === 'active'"
          class="standings-section"
          :aria-labelledby="standingsHeadingId(stage.config.stageId)"
        >
          <h4 :id="standingsHeadingId(stage.config.stageId)">瑞士輪進度</h4>
          <ul class="swiss-progress-list" aria-label="各隊目前勝場">
            <li
              v-for="team in stage.config.teams"
              :key="team.teamId"
              class="swiss-progress-item"
              :data-team-id="team.teamId"
            >
              <span class="identifier">{{ team.teamId }}</span>
              <span>自身交手勝場：{{ swissTeamWins(stage, team.teamId) ?? '未提供' }}</span>
            </li>
          </ul>
          <p class="muted">正式排名將於階段結束後公布。</p>
        </section>

        <section v-else class="standings-section" :aria-labelledby="standingsHeadingId(stage.config.stageId)">
          <h4 :id="standingsHeadingId(stage.config.stageId)">伺服器排名與積分</h4>
          <p class="muted">正式名次先比自身賽事積分，再比不同對手勝分較多、負分較少。血量、耗時、種子與配對抽籤不作排名鍵；全項相同時等待公開裁決。</p>
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
                <div><dt>不同對手勝分</dt><dd>{{ standing.opponentWinScore }}</dd></div>
                <div><dt>不同對手負分</dt><dd>{{ standing.opponentLossScore }}</dd></div>
                <div><dt>已退賽</dt><dd>{{ standing.dropped ? '是' : '否' }}</dd></div>
              </dl>
            </li>
          </ol>

          <section v-if="stage.rankingTieDecisions.length" class="tie-decisions" aria-label="公開同分裁決紀錄">
            <h5>公開同分裁決紀錄</h5>
            <ul>
              <li v-for="(decision, index) in stage.rankingTieDecisions" :key="`${decision.tiedTeamIds.join(',')}-${index}`">
                <p>同分隊伍：<span class="identifier">{{ decision.tiedTeamIds.join('、') }}</span></p>
                <p>裁決順序：<span class="identifier">{{ decision.orderedTeamIds.join(' → ') }}</span></p>
                <p>裁決者：{{ decision.decidedBy.type === 'organizer' ? '主辦' : '指定裁判' }}・<span class="identifier">{{ decision.decidedBy.userId }}</span></p>
                <p>理由：{{ decision.reason }}</p>
              </li>
            </ul>
          </section>

          <section
            v-for="tie in unresolvedRankingTies(stage)"
            :key="tie.key"
            class="ranking-tie-panel"
            :aria-labelledby="`${tie.key}-title`"
          >
            <h5 :id="`${tie.key}-title`">同分順序待裁決</h5>
            <p class="muted">本群組的三項比較值完全相同，相關資格與前三名尚未確定。請主辦或無利益衝突的指定裁判選定比較順序，並填寫公開理由。</p>
            <p class="tie-score">積分 {{ tie.matchPoints }}・勝分 {{ tie.opponentWinScore }}・負分 {{ tie.opponentLossScore }}</p>
            <ol class="tie-order-list" :aria-label="`裁決${tie.tiedTeamIds.join('、')}的完整順序`">
              <li v-for="(teamId, position) in tie.tiedTeamIds" :key="teamId">
                <label :for="`${tie.key}-position-${position}`">第 {{ position + 1 }} 順位
                  <select
                    :id="`${tie.key}-position-${position}`"
                    v-model="rankingTieDraft(tie).orderedTeamIds[position]"
                    :disabled="rankingTieBusy === tie.key"
                  >
                    <option value="">選擇隊伍</option>
                    <option
                      v-for="candidate in tie.tiedTeamIds"
                      :key="candidate"
                      :value="candidate"
                      :disabled="rankingTieDraft(tie).orderedTeamIds.some((selected, selectedPosition) => selected === candidate && selectedPosition !== position)"
                    >{{ candidate }}</option>
                  </select>
                </label>
              </li>
            </ol>
            <label :for="`${tie.key}-reason`">公開裁決理由
              <textarea
                :id="`${tie.key}-reason`"
                v-model="rankingTieDraft(tie).reason"
                rows="3"
                required
                :disabled="rankingTieBusy === tie.key"
              />
            </label>
            <p class="muted">送出時會由賽事服務核對目前版本、主辦／裁判權限，以及裁決者是否屬於同分隊伍。</p>
            <p v-if="rankingTieMessages[tie.key]" role="status" aria-live="polite">{{ rankingTieMessages[tie.key] }}</p>
            <button
              class="refresh-button tie-submit"
              type="button"
              :disabled="!canSubmitRankingTie(tie)"
              @click="submitRankingTie(stage, tie)"
            >
              {{ rankingTieBusy === tie.key ? '正在保存裁決…' : '公開保存同分順序' }}
            </button>
          </section>
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
                  <h6>
                    交手 {{ match.sequence }}
                    <span v-if="match.purpose === 'bronze'" class="purpose-label">季軍賽</span>
                    <span v-else-if="round.matches.some(candidate => candidate.purpose === 'bronze')" class="purpose-label">冠軍賽</span>
                    <span class="identifier">{{ match.matchId }}</span>
                  </h6>
                    <span class="status-badge">{{ tournamentMatchStatusLabel(match.status) }}・舊輪次</span>
                  </header>
                  <p class="match-fact">
                    隊伍：<span v-for="(teamId, index) in match.participants" :key="teamId">
                      <span v-if="index > 0">、</span><span class="identifier">{{ teamId }}</span>
                    </span>
                  </p>
                  <p class="match-fact">賽果：{{ tournamentResultLabel(stage.config, match.result) }}</p>
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
import { computed, ref, watch, onMounted, onBeforeUnmount, nextTick, reactive } from 'vue'
import CompetitionParticipantGameEntry from './CompetitionParticipantGameEntry.vue'
import {
  parseCompetitionTournamentViewResponse,
  doubleEliminationVacancySummary,
  presentCompetitionTournamentViewError,
  singleEliminationDoubleLossFinal,
  singleEliminationVacancySummary,
  swissTieBreakerLabel,
  tournamentFormatLabel,
  tournamentMatchResultLabel,
  tournamentMatchStatusLabel,
  tournamentRoundKindLabel,
  tournamentStageResolutionLabel,
  tournamentStageStatusLabel,
  tournamentTimeLimitLabel,
  type CompetitionTournamentViewData,
  type CompetitionTournamentViewLoadError,
  type TournamentViewMatch,
  type TournamentViewMatchResult,
  type TournamentViewStage,
  type TournamentViewStageConfig,
} from '~/lib/competition-tournament-view'
import { useRulesCatalog } from '~/lib/rules-catalog'

const props = withDefaults(defineProps<{
  competitionId: string
  /** 僅用於使登入者切換時失效；不會送入 GET。 */
  sessionIdentity: string | null
  /** 只作本地查詢 context 版本；不會送入 GET。 */
  contextRevision: number | null
  /** 父頁已確認 session 與公告 projection 載入完成，不代表 API 授權。 */
  contextReady: boolean
  /** 凍結後持續查詢自動建立中的首輪與進行中賽程入口。 */
  registrationLocked?: boolean
  /** 只供標記本人交手；授權仍由參賽者唯讀 API 核實。 */
  participantTeamId?: string | null
  /** 只控制網站是否顯示先手選擇；CompetitionRoom 仍會重驗隊長授權。 */
  participantIsCaptain?: boolean
  participantEntryEnabled?: boolean
}>(), { registrationLocked: false, participantIsCaptain: false })

const mounted = ref(false)
const loading = ref(false)
const tournament = ref<CompetitionTournamentViewData | null>(null)
const loadError = ref<CompetitionTournamentViewLoadError | null>(null)
const rankingTieBusy = ref<string | null>(null)
const rulesCatalog = useRulesCatalog()
const rankingTieMessages = reactive<Record<string, string>>({})
const rankingTieDrafts = reactive<Record<string, { orderedTeamIds: string[], reason: string }>>({})
let requestSequence = 0
let activeRequest: AbortController | null = null
let retryTimer: ReturnType<typeof setTimeout> | null = null

const normalizedCompetitionId = computed(() => normalizeCompetitionId(props.competitionId))
const canRead = computed(() => props.contextReady
  && Boolean(normalizedCompetitionId.value)
  && (props.contextRevision === null
    || (Number.isSafeInteger(props.contextRevision) && props.contextRevision > 0)))
const shouldAutoRefresh = computed(() => props.registrationLocked && (
  loadError.value?.kind === 'notReady'
  || (props.participantEntryEnabled && Boolean(tournament.value?.stages.some(stage => stage.status === 'active')))
))
const requestContext = computed(() => JSON.stringify([
  normalizedCompetitionId.value,
  props.sessionIdentity,
  props.contextRevision,
  props.contextReady,
]))
const publishedCardNames = computed(() => new Map(
  (rulesCatalog.catalog.value?.deckComposition.cardDefinitions ?? [])
    .map(definition => [definition.id, definition.name] as const),
))

function publishedDeckCardLabels(cards: string[]) {
  const counts = new Map<string, number>()
  for (const card of cards) counts.set(card, (counts.get(card) ?? 0) + 1)
  return [...counts].map(([id, count]) => ({
    id,
    name: publishedCardNames.value.get(id)?.trim() || '未知卡牌',
    count,
  }))
}

type PublishedCardCatalogState = 'idle' | 'loading' | 'ready' | 'unavailable'
const publishedCardCatalogState = ref<PublishedCardCatalogState>('idle')
let publishedCardCatalogRequest: Promise<void> | null = null

async function loadPublishedCardCatalog(): Promise<void> {
  if (rulesCatalog.catalog.value) {
    publishedCardCatalogState.value = 'ready'
    return
  }
  if (publishedCardCatalogRequest) return await publishedCardCatalogRequest

  publishedCardCatalogState.value = 'loading'
  publishedCardCatalogRequest = rulesCatalog.load().then(
    () => { publishedCardCatalogState.value = 'ready' },
    () => { publishedCardCatalogState.value = 'unavailable' },
  ).finally(() => { publishedCardCatalogRequest = null })
  await publishedCardCatalogRequest
}

function tournamentResultLabel(
  stage: TournamentViewStageConfig,
  result: TournamentViewMatchResult | null,
): string {
  return tournamentMatchResultLabel(result, teamId => {
    const team = stage.teams.find(entry => entry.teamId === teamId)
    return team ? `種子 ${team.seed}` : '該隊'
  })
}

function isCurrentParticipantMatch(match: TournamentViewMatch): boolean {
  const teamId = props.participantTeamId
  if (!props.participantEntryEnabled || !teamId
    || !match.participants.includes(teamId)
    || !['pending', 'inProgress'].includes(match.status)) return false
  const currentMatch = tournament.value?.stages
    .filter(stage => stage.status === 'active')
    .flatMap(stage => stage.rounds.flatMap(round => round.matches))
    .find(candidate => candidate.participants.includes(teamId)
      && ['pending', 'inProgress'].includes(candidate.status))
  return currentMatch?.matchId === match.matchId
}

function swissTeamWins(stage: TournamentViewStage, teamId: string): number | null {
  return stage.standings.find(standing => standing.teamId === teamId)?.wins ?? null
}

interface UnresolvedRankingTie {
  key: string
  matchPoints: string
  opponentWinScore: string
  opponentLossScore: string
  tiedTeamIds: string[]
}

function unresolvedRankingTies(stage: TournamentViewStage): UnresolvedRankingTie[] {
  if (stage.status !== 'finished' || !['roundRobin', 'swiss'].includes(stage.config.format)) return []
  const groups = new Map<string, UnresolvedRankingTie>()
  for (const standing of stage.standings) {
    if (standing.rank !== null) continue
    const key = `ranking-tie-${encodeURIComponent(stage.config.stageId)}-${standing.matchPoints}-${standing.opponentWinScore}-${standing.opponentLossScore}`
    const group = groups.get(key) ?? {
      key,
      matchPoints: standing.matchPoints,
      opponentWinScore: standing.opponentWinScore,
      opponentLossScore: standing.opponentLossScore,
      tiedTeamIds: [],
    }
    group.tiedTeamIds.push(standing.teamId)
    groups.set(key, group)
  }
  return [...groups.values()].filter(group => group.tiedTeamIds.length > 1)
}

function rankingTieDraft(tie: UnresolvedRankingTie) {
  rankingTieDrafts[tie.key] ??= { orderedTeamIds: Array.from({ length: tie.tiedTeamIds.length }, () => ''), reason: '' }
  return rankingTieDrafts[tie.key]!
}

function canSubmitRankingTie(tie: UnresolvedRankingTie): boolean {
  if (rankingTieBusy.value !== null || !canRead.value || !tournament.value) return false
  const draft = rankingTieDraft(tie)
  return draft.orderedTeamIds.length === tie.tiedTeamIds.length
    && draft.orderedTeamIds.every(teamId => tie.tiedTeamIds.includes(teamId))
    && new Set(draft.orderedTeamIds).size === tie.tiedTeamIds.length
    && Boolean(draft.reason.trim())
}

async function submitRankingTie(stage: TournamentViewStage, tie: UnresolvedRankingTie) {
  const competitionId = normalizedCompetitionId.value
  if (!canSubmitRankingTie(tie) || !tournament.value || !competitionId) return
  const draft = rankingTieDraft(tie)
  rankingTieBusy.value = tie.key
  rankingTieMessages[tie.key] = ''
  try {
    await $fetch(`/api/competitions/${encodeURIComponent(competitionId)}/tournament/ranking-tie`, {
      method: 'POST',
      body: {
        commandId: `ranking-tie-${crypto.randomUUID()}`,
        expectedRevision: tournament.value.revision,
        stageId: stage.config.stageId,
        orderedTeamIds: [...draft.orderedTeamIds],
        reason: draft.reason.trim(),
      },
      retry: 0,
    })
    rankingTieMessages[tie.key] = '同分順序已提交，正在確認公開賽程。'
  } catch {
    rankingTieMessages[tie.key] = '裁決未確認；正在重新查詢最新賽程，請依目前公開紀錄確認結果。'
  } finally {
    rankingTieBusy.value = null
    await loadTournament()
  }
}

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
    if (tournament.value.publication) {
      void loadPublishedCardCatalog()
    }
  } catch (error) {
    if (!isCurrentRequest(sequence, identity)) return
    loadError.value = presentCompetitionTournamentViewError(error)
  } finally {
    if (isCurrentRequest(sequence, identity)) {
      loading.value = false
      activeRequest = null
      if (shouldAutoRefresh.value) {
        retryTimer = setTimeout(() => {
          retryTimer = null
          void loadTournament()
        }, 5_000)
      }
    }
  }
}

function invalidateRequest() {
  requestSequence += 1
  if (retryTimer !== null) clearTimeout(retryTimer)
  retryTimer = null
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
.standings-section, .superseded-section, .round-list, .match-list, .standing-list, .swiss-progress-list,
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
.purpose-label { justify-self: start; border: 1px solid var(--app-border-strong); border-radius: 999px; padding: .1rem .5rem; color: var(--app-text); font-size: .72rem; }

.refresh-button {
  flex: 0 0 auto;
  min-height: 2.75rem;
  border: 1px solid var(--app-border-strong);
  border-radius: 12px;
  padding: .55rem .85rem;
  background: var(--app-control);
  color: var(--app-text);
  cursor: pointer;
}
.refresh-button:disabled { cursor: not-allowed; opacity: .55; }
.refresh-button:focus-visible { outline: 3px solid var(--app-focus-ring); outline-offset: 2px; }

.state-message { border-radius: 12px; padding: .8rem 1rem; background: var(--app-surface-muted); }
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
.seed-list, .standing-list, .swiss-progress-list { margin: 0; padding: 0; list-style: none; }
.seed-list li { display: grid; grid-template-columns: auto minmax(0, 1fr); gap: .75rem; border: 1px solid var(--app-border); border-radius: 8px; padding: .55rem .7rem; background: var(--app-surface-muted); font-size: .82rem; }
.swiss-progress-item { display: grid; grid-template-columns: minmax(0, 1fr) auto; gap: .75rem; border: 1px solid var(--app-border); border-radius: 8px; padding: .55rem .7rem; background: var(--app-surface-muted); font-size: .82rem; }
.review-note { margin: 0; border-left: 3px solid var(--app-accent); padding: .65rem .8rem; background: var(--app-accent-soft); font-size: .875rem; line-height: 1.55; }
.correction-note { font-size: .8rem; }
.round-card, .match-card, .standing-card { display: grid; min-width: 0; gap: .65rem; border: 1px solid var(--app-border); border-radius: 12px; padding: .75rem; background: var(--app-surface-muted); }
.round-card h5 { font-size: .9rem; }
.match-heading { align-items: center; }
.match-heading h6 { display: grid; min-width: 0; gap: .2rem; font-size: .82rem; }
.match-fact { overflow-wrap: anywhere; }
.standing-heading { align-items: center; }
.standing-heading h5 { min-width: 0; overflow-wrap: anywhere; font-size: .88rem; }
.standing-facts { grid-template-columns: repeat(2, minmax(0, 1fr)); }
.tie-decisions, .ranking-tie-panel { display: grid; gap: .65rem; border-top: 1px solid var(--app-border); padding-top: .9rem; }
.tie-decisions h5, .ranking-tie-panel h5 { margin: 0; font-size: .9rem; }
.tie-decisions ul, .tie-order-list { display: grid; gap: .55rem; margin: 0; padding-left: 1.25rem; }
.tie-decisions li { display: grid; gap: .2rem; border: 1px solid var(--app-border); border-radius: 8px; padding: .65rem; background: var(--app-surface-muted); }
.tie-decisions p, .tie-score { margin: 0; font-size: .85rem; line-height: 1.5; }
.tie-order-list label, .ranking-tie-panel label { display: grid; gap: .3rem; font-size: .82rem; }
.tie-order-list select, .ranking-tie-panel textarea { width: 100%; min-height: 2.6rem; border: 1px solid var(--app-border-strong); border-radius: 8px; padding: .55rem .7rem; background: var(--app-control); color: var(--app-text); font: inherit; }
.ranking-tie-panel textarea { resize: vertical; }
.tie-submit { justify-self: start; }
.old-round { border-style: dashed; opacity: .82; }
.publication-card { display: grid; gap: .8rem; border: 1px solid var(--app-accent); border-radius: 12px; padding: 1rem; background: var(--app-accent-soft); }
.publication-card h3, .published-replays h4, .published-decks h4, .published-deck h5 { margin: 0; }
.published-replays, .published-decks, .published-deck { display: grid; min-width: 0; gap: .6rem; }
.published-replay-list, .published-deck-list, .published-deck ul, .published-card-list { display: grid; gap: .4rem; margin: 0; padding: 0; list-style: none; }
.published-replay-list li, .published-deck { min-width: 0; border: 1px solid var(--app-border); border-radius: 8px; padding: .65rem .75rem; background: var(--app-surface-muted); }
.published-replay-list a { color: var(--app-link); }
.published-deck h5 { overflow-wrap: anywhere; font-size: .85rem; }
.published-deck ul { margin-top: .5rem; }
.published-card-list { grid-template-columns: repeat(auto-fit, minmax(min(100%, 12rem), 1fr)); font-size: .82rem; }
.published-deck > ul { padding-left: 0; }
.catalog-retry-button { justify-self: start; min-height: 2.5rem; border: 1px solid var(--app-border-strong); border-radius: 8px; padding: .45rem .75rem; background: var(--app-control); color: var(--app-text); cursor: pointer; }
.catalog-retry-button:focus-visible { outline: 3px solid var(--app-focus-ring); outline-offset: 2px; }

@media (max-width: 600px) {
  .tournament-view { padding: 1rem; }
  .view-heading { display: grid; }
  .refresh-button { width: 100%; }
  .stage-heading { flex-wrap: wrap; }
  .config-list { grid-template-columns: minmax(0, 1fr); }
  .wide-fact { grid-column: auto; }
  .status-badge { white-space: normal; }
  .standing-facts { grid-template-columns: minmax(0, 1fr); }
  .swiss-progress-item { grid-template-columns: minmax(0, 1fr); }
  .match-heading { align-items: flex-start; flex-direction: column; }
}
.tournament-view { border: 1px solid var(--ui-glass-stroke); border-radius: 22px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); -webkit-backdrop-filter: blur(18px); backdrop-filter: blur(18px); }
.tournament-view :is(input, select, textarea, button):focus-visible { outline: 2px solid var(--app-accent); outline-offset: 3px; }
</style>
