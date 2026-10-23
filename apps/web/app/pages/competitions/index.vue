<template>
  <main class="competition-page workspace-page page-scene page-scene--quiet" data-scene="hall">
    <PageIntro title="競賽對局・論劍" eyebrow="群英入席" description="建立賽事公告，或輸入賽事 ID 查看隊伍與報名狀態。" back-to="/rooms" back-label="房間大廳" />

    <section class="panel lookup-panel" aria-labelledby="competition-lookup-title">
      <div>
        <h2 id="competition-lookup-title">開啟既有賽事</h2>
        <p class="muted">目前沒有賽事清單，請使用主辦方提供的賽事 ID。</p>
      </div>
      <form class="lookup-form" @submit.prevent="openCompetition">
        <label for="competition-id">賽事 ID</label>
        <div class="inline-form">
          <input
            id="competition-id"
            v-model.trim="competitionIdInput"
            class="text-input"
            autocomplete="off"
            required
          >
          <button class="secondary-button" type="submit" :disabled="!competitionIdInput">
            開啟賽事
          </button>
        </div>
      </form>
    </section>

    <section class="panel announcement-panel" aria-labelledby="announcement-title">
      <div class="section-heading">
        <div>
          <h2 id="announcement-title">建立賽事公告</h2>
          <p class="muted">公告會固定規則、參賽型態、生命值與報名期限。</p>
        </div>
      </div>

      <p v-if="!sessionLoaded" class="muted" role="status" aria-live="polite">
        正在確認登入狀態…
      </p>
      <div v-else-if="sessionError" class="message error-message" role="alert">
        <p>{{ sessionError }}</p>
        <button class="ghost-button" type="button" @click="loadSession">重試登入狀態</button>
      </div>
      <div v-else-if="!session.userId.value" class="message">
        <p>登入後即可建立賽事公告。</p>
        <NuxtLink class="secondary-button link-button" to="/login">登入</NuxtLink>
      </div>
      <div v-else-if="rulesCatalog.loading.value && !rulesCatalog.catalog.value" class="message" role="status">
        正在載入規則選項…
      </div>
      <div v-else-if="catalogError || !rulesCatalog.catalog.value" class="message error-message" role="alert">
        <p>{{ catalogError || '目前無法取得規則版本與模組選項。' }}</p>
        <button class="ghost-button" type="button" :disabled="rulesCatalog.loading.value" @click="loadCatalog">
          {{ rulesCatalog.loading.value ? '載入中…' : '重試載入規則選項' }}
        </button>
      </div>

      <form v-else class="announcement-form" @submit.prevent="createCompetition">
        <div class="field-grid">
          <fieldset>
            <legend>規則版本</legend>
            <div class="choice-row">
              <label v-for="version in ruleVersions" :key="version" class="choice-card">
                <input
                  type="radio"
                  name="competition-rule-version"
                  :value="version"
                  :checked="ruleVersion === version"
                  :disabled="creating || createOutcomeUnknown"
                  @change="setRuleVersion(version)"
                >
                <span>版本 {{ version }}</span>
              </label>
            </div>
          </fieldset>

          <div class="fixed-ruleset">
            <span class="field-label">規則基底</span>
            <strong>基本規則</strong>
            <small>使用現有規則引擎支援的規則基底。</small>
          </div>
        </div>

        <fieldset class="module-fieldset">
          <legend>規則模組</legend>
          <p class="muted">模組與依賴選項取自目前規則目錄；改變規則版本會移除該版本不支援的模組。</p>
          <div v-for="group in ruleGroups" :key="group.id" class="module-group">
            <h3>{{ group.label }}</h3>
            <div class="module-grid">
              <label v-for="module in group.modules" :key="module.id" class="module-option">
                <input
                  :id="`competition-module-${module.id}`"
                  type="checkbox"
                  :checked="enabledRuleModules.includes(module.id)"
                  :disabled="creating || createOutcomeUnknown"
                  @change="toggleRuleModule(module.id)"
                >
                <span>{{ module.label }}</span>
              </label>
            </div>
          </div>
          <p class="policy-note">
            個人牌組政策：{{ personalDecksRequired ? '需要每位隊員提交個人牌組' : '不使用個人牌組' }}
            （依「個人牌組」規則模組設定）。
          </p>
        </fieldset>

        <div class="field-grid">
          <label for="team-size">
            參賽型態
            <select id="team-size" v-model.number="teamSize" class="text-input" :disabled="creating || createOutcomeUnknown">
              <option :value="1">個人</option>
              <option :value="2">團體（每隊 2 人）</option>
            </select>
          </label>
          <div class="fixed-ruleset">
            <span class="field-label">初始生命值</span>
            <strong>沿用規則設定</strong>
            <small>建立後以賽事公告為準。</small>
          </div>
        </div>

        <fieldset class="stage-plan-fieldset">
          <legend>各階段賽制、交手格式、時計與資格來源</legend>
          <p class="muted">完整方案會和規則、參賽型態及期限一起保存。報名截止後，建立賽程直接使用這份公告，不再要求主辦補填階段設定。</p>
          <article v-for="(stage, index) in stageDrafts" :key="stage.stageId" class="stage-plan-card">
            <h3>第 {{ index + 1 }} 階段</h3>
            <div class="field-grid">
              <label :for="`stage-format-${index}`">賽制
                <select :id="`stage-format-${index}`" v-model="stage.format" class="text-input" required :disabled="creating || createOutcomeUnknown">
                  <option value="" disabled>請選擇賽制</option>
                  <option value="singleElimination">單敗淘汰</option>
                  <option value="roundRobin">循環賽</option>
                  <option value="doubleElimination">雙敗淘汰</option>
                  <option value="swiss">瑞士制</option>
                </select>
              </label>
              <label :for="`stage-match-count-${index}`">{{ stage.format === 'singleElimination' || stage.format === 'doubleElimination' ? '勝局門檻' : '每場局數' }}
                <input :id="`stage-match-count-${index}`" v-model="stage.matchGames" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
              </label>
              <label :for="`stage-main-time-${index}`">主時間（整數分鐘；0 表示不限時）
                <input :id="`stage-main-time-${index}`" v-model="stage.mainMinutes" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
              </label>
              <label :for="`stage-byoyomi-${index}`">讀秒（行動與回合外選擇共用）
                <select :id="`stage-byoyomi-${index}`" v-model="stage.byoyomiSeconds" class="text-input" required :disabled="creating || createOutcomeUnknown">
                  <option value="30">30 秒</option><option value="60">1 分鐘</option><option value="180">3 分鐘</option>
                </select>
              </label>
            </div>

            <div v-if="stage.format === 'roundRobin'" class="policy-note">
              循環賽積分固定為勝 3、和 1、負 0；輪空 0 分。
            </div>
            <div v-if="stage.format === 'swiss'" class="swiss-points">
              <h4>瑞士輪數與積分</h4>
              <label :for="`stage-swiss-rounds-${index}`">輪數
                <input :id="`stage-swiss-rounds-${index}`" v-model="stage.swissRounds" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
              </label>
              <div class="point-grid">
                <label v-for="field in swissPointFields" :key="field.key" :for="`stage-swiss-${field.key}-${index}`">{{ field.label }}
                  <input :id="`stage-swiss-${field.key}-${index}`" v-model="stage.swissMatchPoints[field.key]" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
                </label>
              </div>
              <h4>瑞士排名政策</h4>
              <div class="point-grid">
                <label :for="`stage-swiss-credit-numerator-${index}`">對手勝率和局折算分子
                  <input :id="`stage-swiss-credit-numerator-${index}`" v-model="stage.swissPolicy.drawCredit.numerator" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
                </label>
                <label :for="`stage-swiss-credit-denominator-${index}`">對手勝率和局折算分母
                  <input :id="`stage-swiss-credit-denominator-${index}`" v-model="stage.swissPolicy.drawCredit.denominator" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
                </label>
                <label :for="`stage-swiss-floor-mode-${index}`">對手勝率下限
                  <select :id="`stage-swiss-floor-mode-${index}`" v-model="stage.swissPolicy.opponentWinRateFloorPercent.mode" class="text-input" required :disabled="creating || createOutcomeUnknown">
                    <option value="none">不設定</option><option value="value">指定百分比</option>
                  </select>
                </label>
                <label v-if="stage.swissPolicy.opponentWinRateFloorPercent.mode === 'value'" :for="`stage-swiss-floor-value-${index}`">下限百分比（0–100）
                  <input :id="`stage-swiss-floor-value-${index}`" v-model="stage.swissPolicy.opponentWinRateFloorPercent.value" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
                </label>
                <label :for="`stage-swiss-cap-mode-${index}`">對手勝率上限
                  <select :id="`stage-swiss-cap-mode-${index}`" v-model="stage.swissPolicy.opponentWinRateCapPercent.mode" class="text-input" required :disabled="creating || createOutcomeUnknown">
                    <option value="none">不設定</option><option value="value">指定百分比</option>
                  </select>
                </label>
                <label v-if="stage.swissPolicy.opponentWinRateCapPercent.mode === 'value'" :for="`stage-swiss-cap-value-${index}`">上限百分比（0–100）
                  <input :id="`stage-swiss-cap-value-${index}`" v-model="stage.swissPolicy.opponentWinRateCapPercent.value" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
                </label>
                <label :for="`stage-swiss-exclude-byes-${index}`">計算對手勝率時排除輪空
                  <select :id="`stage-swiss-exclude-byes-${index}`" v-model="stage.swissPolicy.excludeByesFromOpponentWinRate" class="text-input" required :disabled="creating || createOutcomeUnknown">
                    <option value="yes">排除</option><option value="no">不排除</option>
                  </select>
                </label>
                <label :for="`stage-swiss-tie-mode-${index}`">瑞士同分比較
                  <select :id="`stage-swiss-tie-mode-${index}`" v-model="stage.swissPolicy.tieMode" class="text-input" required :disabled="creating || createOutcomeUnknown" @change="selectSwissTieMode(stage)">
                    <option value="none">不設定</option><option value="configured">自訂比較順序</option>
                  </select>
                </label>
              </div>
              <template v-if="stage.swissPolicy.tieMode === 'configured'">
                <div v-for="(_, tieIndex) in stage.swissPolicy.tieBreakers" :key="tieIndex" class="source-slot">
                  <label :for="`stage-swiss-tie-${index}-${tieIndex}`">第 {{ tieIndex + 1 }} 項同分比較
                    <select :id="`stage-swiss-tie-${index}-${tieIndex}`" v-model="stage.swissPolicy.tieBreakers[tieIndex]" class="text-input" required :disabled="creating || createOutcomeUnknown">
                      <option value="" disabled>請選擇</option>
                      <option v-for="tieBreaker in swissTieBreakers" :key="tieBreaker.value" :value="tieBreaker.value" :disabled="stage.swissPolicy.tieBreakers.includes(tieBreaker.value) && stage.swissPolicy.tieBreakers[tieIndex] !== tieBreaker.value">{{ tieBreaker.label }}</option>
                    </select>
                  </label>
                  <button class="ghost-button" type="button" :disabled="creating || createOutcomeUnknown" @click="removeSwissTieBreaker(stage, tieIndex)">移除</button>
                </div>
                <button class="secondary-button" type="button" :disabled="creating || createOutcomeUnknown || stage.swissPolicy.tieBreakers.length >= swissTieBreakers.length" @click="addSwissTieBreaker(stage)">新增同分比較</button>
              </template>
              <small>種子抽籤完成後，完整瑞士政策依此公告執行並保存在賽程中。</small>
            </div>

            <div v-if="index === 0" class="policy-note">
              資格來源：截止時所有符合公告條件的隊伍。種子政策：報名截止後由系統抽籤，結果保存後重試不重抽。
            </div>
            <fieldset v-else class="source-slot-fieldset">
              <legend>晉級資格來源</legend>
              <div v-for="(slot, slotIndex) in stage.sourceSlots" :key="slotIndex" class="source-slot">
                <label :for="`stage-${index}-slot-${slotIndex}-source-stage`">來源階段
                  <select :id="`stage-${index}-slot-${slotIndex}-source-stage`" v-model="slot.sourceStageId" class="text-input" required :disabled="creating || createOutcomeUnknown">
                    <option value="" disabled>請選擇前階段</option>
                    <option v-for="(earlier, earlierIndex) in stageDrafts.slice(0, index)" :key="earlier.stageId" :value="earlier.stageId">第 {{ earlierIndex + 1 }} 階段</option>
                  </select>
                </label>
                <label :for="`stage-${index}-slot-${slotIndex}-source-type`">資格名額
                  <select :id="`stage-${index}-slot-${slotIndex}-source-type`" v-model="slot.sourceType" class="text-input" required :disabled="creating || createOutcomeUnknown">
                    <option value="" disabled>請選擇來源</option>
                    <option v-if="sourceStageFormat(slot.sourceStageId) === 'singleElimination' || sourceStageFormat(slot.sourceStageId) === 'doubleElimination'" value="finishedStageWinner">階段勝者</option>
                    <option v-if="sourceStageFormat(slot.sourceStageId) === 'roundRobin' || sourceStageFormat(slot.sourceStageId) === 'swiss'" value="uniqueStandingRank">唯一名次</option>
                  </select>
                </label>
                <label v-if="slot.sourceType === 'uniqueStandingRank'" :for="`stage-${index}-slot-${slotIndex}-rank`">名次
                  <input :id="`stage-${index}-slot-${slotIndex}-rank`" v-model="slot.rank" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
                </label>
                <label :for="`stage-${index}-slot-${slotIndex}-seed`">目的種子
                  <input :id="`stage-${index}-slot-${slotIndex}-seed`" v-model="slot.destinationSeed" class="text-input" inputmode="numeric" pattern="[0-9]+" required :disabled="creating || createOutcomeUnknown">
                </label>
                <button v-if="stage.sourceSlots.length > 1" class="ghost-button" type="button" :disabled="creating || createOutcomeUnknown" @click="removeSourceSlot(index, slotIndex)">移除名額</button>
              </div>
              <button class="secondary-button" type="button" :disabled="creating || createOutcomeUnknown" @click="addSourceSlot(index)">新增晉級名額</button>
              <small>階段勝者來源用於淘汰賽；唯一名次來源用於循環或瑞士賽。只有全項同分時才能由主辦或無利益衝突裁判決定名額。</small>
            </fieldset>
            <button v-if="stageDrafts.length > 1" class="ghost-button" type="button" :disabled="creating || createOutcomeUnknown" @click="removeStage(index)">移除此階段</button>
          </article>
          <button class="secondary-button" type="button" :disabled="creating || createOutcomeUnknown" @click="addStage">新增下一階段</button>

          <div class="policy-note ranking-policy" aria-label="公告排序與瑞士配對政策">
            <strong>所有階段的完整名次與資格政策</strong>
            <span>自身交手積分較高優先；同分依序比較勝分較多、負分較少。</span>
            <span>所有比較項全同時，僅由主辦或無利益衝突的指定裁判決定順序及資格，必填並公開理由。</span>
            <span>不比較生命、主時間或耗時；不建立排名同分加賽。瑞士合法配對候選的抽籤使用已保存種子，不用於最終名次。</span>
          </div>
        </fieldset>

        <fieldset>
          <legend>報名方式</legend>
          <div class="choice-row">
            <label class="choice-card">
              <input v-model="registrationMode" type="radio" name="competition-registration-mode" value="public" :disabled="creating || createOutcomeUnknown">
              <span>公開報名</span>
            </label>
            <label class="choice-card">
              <input v-model="registrationMode" type="radio" name="competition-registration-mode" value="inviteOnly" :disabled="creating || createOutcomeUnknown">
              <span>邀請制</span>
            </label>
          </div>
          <label v-if="registrationMode === 'inviteOnly'" for="invited-captains" class="stacked-field invitees-field">
            受邀隊長使用者 ID（每行一位）
            <textarea
              id="invited-captains"
              v-model="invitedCaptainsText"
              class="text-input text-area"
              rows="4"
              required
              :disabled="creating || createOutcomeUnknown"
              aria-describedby="invitees-help"
            />
            <small id="invitees-help">請輸入帳號使用者 ID，不需輸入 JSON。邀請名單不會顯示在公開公告。</small>
          </label>
        </fieldset>

        <fieldset>
          <legend>報名與比賽開始時間</legend>
          <p class="muted">請依 {{ localTimeZone }} 輸入。送出的時間會轉為 Unix 毫秒；實際期限由伺服器判定。</p>
          <div class="field-grid">
            <label for="registration-deadline">
              報名截止
              <input
                id="registration-deadline"
                v-model="registrationDeadlineLocal"
                class="text-input"
                type="datetime-local"
                step="60"
                required
                :disabled="creating || createOutcomeUnknown"
                aria-describedby="registration-deadline-help"
              >
              <small id="registration-deadline-help">報名截止以分鐘為單位，不含秒或毫秒。</small>
            </label>
            <label for="check-in-deadline">
              比賽開始時間
              <input
                id="check-in-deadline"
                v-model="checkInDeadlineLocal"
                class="text-input"
                type="datetime-local"
                step="60"
                required
                :disabled="creating || createOutcomeUnknown"
                aria-describedby="check-in-deadline-help"
              >
              <small id="check-in-deadline-help">比賽開始時間以分鐘為單位，不含秒或毫秒。</small>
            </label>
          </div>
        </fieldset>

        <p v-if="createError" class="message error-message" role="alert">{{ createError }}</p>
        <p v-if="createOutcomeUnknown" class="message warning-message" role="status">
          建立結果無法確認。賽事可能已建立；建立 API 不支援安全重試，因此不會再次送出。若你已取得賽事 ID，可使用上方欄位開啟。
        </p>
        <div class="form-actions">
          <button class="primary-button" type="submit" :disabled="creating || createOutcomeUnknown || !session.userId.value">
            {{ creating ? '正在建立…' : createOutcomeUnknown ? '建立結果待確認' : '建立賽事公告' }}
          </button>
        </div>
      </form>
    </section>
  </main>
</template>

<script setup lang="ts">
import type { RuleModuleSpec } from '~/types/fewfc'
import type { CompetitionAnnouncementStageDraft } from '#shared/competition-announcement-plan'
import type { SwissTieBreaker } from '#shared/competition'
import type { CompetitionCreateAcknowledgement } from '#shared/competition-room'
import { createRuleModulePolicy, presentationForRuleModule } from '#shared/utils/rule-modules'
import { modulesForVersion } from '#shared/utils/rule-versions'
import { presentApiError } from '~/lib/api-error-presentation'
import {
  apiErrorStatus,
  buildCompetitionAnnouncement,
  isDefinitiveCompetitionCreateRejection,
  parseCreatedCompetitionId,
  type CompetitionRuleVersion,
} from '~/lib/competition-registration'
import { useRulesCatalog } from '~/lib/rules-catalog'

const router = useRouter()
const session = usePlayerSession()
const rulesCatalog = useRulesCatalog()
const competitionIdInput = ref('')
const sessionLoaded = ref(false)
const sessionError = ref('')
const catalogError = ref('')
const ruleVersion = ref<CompetitionRuleVersion>('5.17')
const enabledRuleModules = ref<string[]>([])
const teamSize = ref<1 | 2>(1)
const registrationMode = ref<'public' | 'inviteOnly'>('public')
const invitedCaptainsText = ref('')
const registrationDeadlineLocal = ref('')
const checkInDeadlineLocal = ref('')
let nextStageNumber = 1
const stageDrafts = ref([newStageDraft()])
const creating = ref(false)
const createOutcomeUnknown = ref(false)
const createError = ref('')
const defaultsApplied = ref(false)

const ruleVersions: CompetitionRuleVersion[] = ['5.17', '5.16']
const categoryLabels = {
  optional: '選用規則',
  advanced: '進階規則',
  theme: '主題規則',
} as const
const catalogModules = computed(() => rulesCatalog.catalog.value?.ruleModules ?? [])
const availableModules = computed(() => modulesForVersion(catalogModules.value, ruleVersion.value))
const modulePolicy = computed(() => createRuleModulePolicy(availableModules.value))
const personalDecksRequired = computed(() => enabledRuleModules.value.includes('personal-deck'))
const localTimeZone = computed(() => new Intl.DateTimeFormat(undefined, { timeZoneName: 'short' })
  .formatToParts(new Date()).find(part => part.type === 'timeZoneName')?.value ?? '本地時間')
const ruleGroups = computed(() => (['optional', 'advanced', 'theme'] as const).map(id => ({
  id,
  label: categoryLabels[id],
  modules: availableModules.value
    .filter(module => module.category === id)
    .map((module: RuleModuleSpec) => ({ id: module.id, label: presentationForRuleModule(module.id).label })),
})))
const swissPointFields = [
  { key: 'win', label: '勝場積分' }, { key: 'draw', label: '和局積分' },
  { key: 'loss', label: '敗場積分' }, { key: 'bye', label: '輪空積分' },
] as const
const swissTieBreakers: { value: SwissTieBreaker, label: string }[] = [
  { value: 'matchPoints', label: '交手積分' },
  { value: 'headToHeadMatchPoints', label: '彼此交手積分' },
  { value: 'opponentWinRate', label: '對手勝率' },
  { value: 'netGameWins', label: '淨勝局' },
  { value: 'seedOrder', label: '種子順序' },
]

function newStageDraft(): CompetitionAnnouncementStageDraft {
  return {
    stageId: `stage-${nextStageNumber++}`,
    format: '',
    matchGames: '',
    mainMinutes: '5',
    byoyomiSeconds: '30',
    swissRounds: '',
    swissMatchPoints: { win: '3', draw: '1', loss: '0', bye: '3' },
    swissPolicy: {
      drawCredit: { numerator: '1', denominator: '2' },
      opponentWinRateFloorPercent: { mode: 'none', value: '' },
      opponentWinRateCapPercent: { mode: 'none', value: '' },
      excludeByesFromOpponentWinRate: 'no',
      tieMode: 'none',
      tieBreakers: [],
    },
    sourceSlots: [] as { sourceStageId: string, sourceType: '' | 'finishedStageWinner' | 'uniqueStandingRank', rank: string, destinationSeed: string }[],
  }
}

function selectSwissTieMode(stage: CompetitionAnnouncementStageDraft) {
  if (stage.swissPolicy.tieMode === 'none') stage.swissPolicy.tieBreakers = []
}

function addSwissTieBreaker(stage: CompetitionAnnouncementStageDraft) {
  if (stage.swissPolicy.tieBreakers.length < swissTieBreakers.length) stage.swissPolicy.tieBreakers.push('')
}

function removeSwissTieBreaker(stage: CompetitionAnnouncementStageDraft, index: number) {
  stage.swissPolicy.tieBreakers.splice(index, 1)
}

function addStage() {
  const stage = newStageDraft()
  if (stageDrafts.value.length > 0) stage.sourceSlots.push({ sourceStageId: '', sourceType: '', rank: '', destinationSeed: '' })
  stageDrafts.value.push(stage)
}

function removeStage(index: number) { stageDrafts.value.splice(index, 1) }
function addSourceSlot(index: number) {
  const stage = stageDrafts.value[index]
  if (!stage) return
  stage.sourceSlots.push({ sourceStageId: '', sourceType: '', rank: '', destinationSeed: '' })
}
function removeSourceSlot(stageIndex: number, slotIndex: number) {
  stageDrafts.value[stageIndex]?.sourceSlots.splice(slotIndex, 1)
}
function sourceStageFormat(stageId: string) {
  return stageDrafts.value.find(stage => stage.stageId === stageId)?.format ?? ''
}

function openCompetition() {
  const id = competitionIdInput.value.trim()
  if (id) void router.push(`/competitions/${encodeURIComponent(id)}`)
}

function setRuleVersion(version: CompetitionRuleVersion) {
  ruleVersion.value = version
  enabledRuleModules.value = modulePolicy.value.normalize(enabledRuleModules.value)
}

function toggleRuleModule(moduleId: string) {
  enabledRuleModules.value = enabledRuleModules.value.includes(moduleId)
    ? modulePolicy.value.disable(enabledRuleModules.value, moduleId)
    : modulePolicy.value.enable(enabledRuleModules.value, moduleId)
}

async function loadSession() {
  sessionError.value = ''
  sessionLoaded.value = false
  try {
    await session.refresh()
  } catch (error) {
    sessionError.value = presentApiError(error, '無法確認登入狀態，請重試。')
  } finally {
    sessionLoaded.value = true
  }
}

async function loadCatalog() {
  catalogError.value = ''
  try {
    const catalog = await rulesCatalog.load()
    if (!defaultsApplied.value) {
      enabledRuleModules.value = createRuleModulePolicy(modulesForVersion(catalog.ruleModules, ruleVersion.value)).defaults
      defaultsApplied.value = true
    }
  } catch (error) {
    catalogError.value = presentApiError(error, '無法取得規則選項，請重試。')
  }
}

async function createCompetition() {
  if (creating.value || createOutcomeUnknown.value || !session.userId.value || !rulesCatalog.catalog.value) return
  createError.value = ''
  let body: ReturnType<typeof buildCompetitionAnnouncement>
  try {
    body = buildCompetitionAnnouncement({
      ruleVersion: ruleVersion.value,
      enabledRuleModules: enabledRuleModules.value,
      teamSize: Number(teamSize.value),
      registrationMode: registrationMode.value,
      invitedCaptainsText: invitedCaptainsText.value,
      registrationDeadlineLocal: registrationDeadlineLocal.value,
      checkInDeadlineLocal: checkInDeadlineLocal.value,
      stageDrafts: stageDrafts.value,
    }, rulesCatalog.catalog.value.ruleModules)
  } catch (error) {
    createError.value = error instanceof Error ? error.message : '請檢查賽事設定。'
    return
  }

  creating.value = true
  let requestSent = false
  try {
    requestSent = true
    const response = await $fetch<CompetitionCreateAcknowledgement>('/api/competitions', { method: 'POST', body })
    const id = parseCreatedCompetitionId(response)
    await router.push(`/competitions/${encodeURIComponent(id)}`)
  } catch (error) {
    if (requestSent && !isDefinitiveCompetitionCreateRejection(error)) {
      createOutcomeUnknown.value = true
      createError.value = '建立結果無法確認。伺服器可能已建立賽事；請勿重新送出，以免建立重複公告。若已有賽事 ID，請從上方開啟。'
    } else {
      createError.value = presentApiError(error, '無法建立賽事公告。')
    }
  } finally {
    creating.value = false
  }
}

onMounted(() => {
  void loadSession()
  void loadCatalog()
})
</script>

<style scoped>
@reference "../../assets/css/main.css";

.competition-page { @apply mx-auto grid w-full max-w-[1050px] gap-6 px-6 py-12 max-[600px]:gap-4 max-[600px]:px-4 max-[600px]:py-7; }
.page-heading { @apply grid gap-2; }
.page-heading h1 { @apply m-0 font-serif text-3xl max-[600px]:text-2xl; }
.page-heading > p:last-child { @apply m-0; }
.eyebrow { @apply m-0 text-xs tracking-[.16em] text-gold-light; }
.panel { @apply grid gap-6 border border-line bg-panel p-7 max-[600px]:gap-5 max-[600px]:p-4; border-color: var(--ui-glass-stroke); border-radius: 24px; background: var(--ui-glass-fill); box-shadow: var(--ui-glass-shadow); backdrop-filter: blur(18px); -webkit-backdrop-filter: blur(18px); }
.panel h2 { @apply m-0 font-serif text-xl; }
.panel p { @apply m-0; }
.lookup-panel { @apply grid-cols-[minmax(0,1fr)_minmax(280px,1fr)] items-end max-[760px]:grid-cols-1; }
.lookup-form, .announcement-form, .stacked-field { @apply grid gap-2; }
.lookup-form > label, .announcement-form > label, .stacked-field { @apply text-sm; }
.inline-form { @apply grid grid-cols-[minmax(0,1fr)_auto] items-center gap-3 max-[480px]:grid-cols-1; }
.text-input { @apply m-0 min-h-12 w-full px-3.5; border: 1px solid var(--ui-glass-stroke); border-radius: 12px; background: color-mix(in srgb,var(--app-input) 90%,transparent); color: var(--app-text); }
.text-input:focus { outline: 2px solid var(--app-focus-ring); outline-offset: 1px; border-color: var(--app-accent); }
.inline-form .text-input { @apply min-w-0; }
.section-heading { @apply flex items-start justify-between gap-4; }
.announcement-form { @apply gap-7; }
.announcement-form fieldset { @apply grid gap-3 border-0 p-0; }
.stage-plan-fieldset, .source-slot-fieldset { @apply grid gap-4; }
.stage-plan-card { @apply grid gap-4 border border-line bg-panel p-5; border-color: var(--ui-glass-stroke); border-radius: 16px; }
.stage-plan-card h3 { @apply m-0 text-base font-semibold; }
.source-slot { @apply grid grid-cols-[repeat(auto-fit,minmax(150px,1fr))] items-end gap-3 border-t border-line pt-3; }
.source-slot label, .swiss-points label { @apply grid gap-2 text-sm; }
.swiss-points { @apply grid gap-3; }
.swiss-points h4 { @apply m-0 font-semibold; }
.point-grid { @apply grid grid-cols-4 gap-3 max-[700px]:grid-cols-2; }
.ranking-policy { @apply grid gap-2; }
.ranking-policy strong { @apply text-sm; }
.announcement-form legend, .field-label { @apply mb-1 text-sm font-semibold; }
.field-grid { @apply grid grid-cols-2 gap-5 max-[640px]:grid-cols-1; }
.field-grid > label { @apply grid content-start gap-2 text-sm; }
.choice-row { @apply flex flex-wrap gap-3; }
.choice-card { @apply flex min-h-12 items-center gap-2 border border-line px-4 py-3 text-sm; border-radius: 10px; background: color-mix(in srgb,var(--app-surface-muted) 80%,transparent); }
.choice-card:has(input:checked) { border-color: var(--app-accent); background: var(--app-accent-soft); }
.fixed-ruleset { @apply grid content-start gap-2 rounded-xl border border-line p-4; background: color-mix(in srgb,var(--app-surface-muted) 80%,transparent); }
.fixed-ruleset strong { @apply text-sm text-gold-light; }
.fixed-ruleset small, .muted, .policy-note, .stacked-field small { color: var(--app-text-muted); }
.module-fieldset { @apply rounded-xl border border-line p-4; background: color-mix(in srgb,var(--app-surface-muted) 80%,transparent); }
.module-fieldset > p { @apply mb-4 text-xs leading-5; }
.module-group { @apply mb-5 grid gap-3; }
.module-group h3 { @apply m-0 text-sm; }
.module-grid { @apply grid grid-cols-2 gap-2 max-[540px]:grid-cols-1; }
.module-option { @apply flex min-h-11 items-center gap-2 rounded-lg border border-line bg-panel px-3 py-2 text-sm; }
.policy-note { @apply border-t border-line pt-4 text-xs leading-5; }
.invitees-field { @apply mt-2; }
.text-area { @apply min-h-24 resize-y py-3; }
.message { @apply grid gap-3 rounded-xl border border-line p-4 text-sm; background: color-mix(in srgb,var(--app-surface-muted) 80%,transparent); }
.error-message { border-color: var(--app-danger); background: var(--app-danger-surface); }
.warning-message { border-color: var(--app-accent); }
.form-actions { @apply flex justify-end max-[480px]:grid; }
.link-button { @apply inline-flex w-fit items-center justify-center no-underline; }
.muted { @apply text-sm leading-6; }

@media (max-width: 600px) {
  .inline-form .secondary-button, .form-actions .primary-button { @apply w-full; }
  .page-heading h1 { @apply text-[26px]; }
}
.workspace-page > :deep(.page-intro) { margin-bottom: 0; }
.workspace-page { max-width: 1180px; }
@media(max-width:600px) { .panel { border-radius: 20px; } }
</style>
