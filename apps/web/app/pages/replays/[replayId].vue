<template>
  <main class="workspace-page replay-detail-page">
      <section class="replay-content" aria-label="重播播放器">
        <PageIntro title="重播" eyebrow="回看戰局" back-to="/replays" back-label="重播清單" />
        <p v-if="replayLoading && !replayFrame" class="page-notice" role="status" aria-live="polite">正在載入重播…</p>
        <p v-else-if="replayLoading" class="page-notice" role="status" aria-live="polite">正在切換重播畫面…</p>
        <template v-if="replayFrame">

          <p v-if="replayError" class="page-notice page-notice--error" role="alert">{{ replayError }}</p>
          <nav class="replay-toolbar ui-glass" aria-label="重播控制"><strong class="replay-progress" aria-live="polite">第 {{ replayFrame.currentStep }} / {{ replayFrame.totalSteps }} 步</strong>
            <button class="ghost-button" :disabled="replayFrame.currentStep === 0" @click="loadReplayFrame(0)">第一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === 0" @click="loadReplayFrame(replayFrame.currentStep - 1)">上一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === replayFrame.totalSteps" @click="loadReplayFrame(replayFrame.currentStep + 1)">下一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === replayFrame.totalSteps" @click="loadReplayFrame(replayFrame.totalSteps)">最後一步</button>
            <button class="ghost-button" @click="copyReplayLink(replayRouteId)">分享連結</button>
          </nav>
          <div class="battle-layout replay-layout battle-scene" :style="replayLayoutStyle">
            <BattlefieldBoard
              class="replay-battlefield"
              :state="replayFrame.state"
              :display-names="replayDisplayNames"
              :anchor-player="replayPerspective"
              mode="replay"
            >
              <template #overlay>
                <p v-if="replayFrame.state.pendingChoice?.visibility === 'visible' && replayFrame.state.pendingChoice.choice.type === 'card'" class="action-detail">
                  {{ replayPlayerLabel(replayFrame.state.pendingChoice.player) }} 的選擇：{{ replayFrame.state.pendingChoice.choice.cards.map(card => card.label).join('、') }}
                </p>
              </template>
              <template #terminal-resolution>
                <GameConclusionPanel
                  v-if="replayFrame.state.status === 'Finished'"
                  :state="replayFrame.state"
                  :team-label="replayTeamLabel"
                  :player-label="replayPlayerLabel"
                />
              </template>
            </BattlefieldBoard>
            <button
              ref="eventSheetTrigger"
              class="replay-mobile-record-summary"
              type="button"
              aria-haspopup="dialog"
              :aria-expanded="eventSheetOpen"
              aria-controls="replay-mobile-event-sheet"
              @click="openEventSheet"
            >
              <strong>戰局紀錄</strong>
              <span>{{ latestReplayEvent?.title ?? '尚無紀錄' }}<template v-if="latestReplayEvent?.summary"> · {{ latestReplayEvent.summary }}</template></span>
              <b aria-hidden="true">⌃</b>
            </button>
            <aside class="replay-sidebar">
              <section ref="battleRecordFeed" class="event-panel battle-glass">
                <div class="panel-title"><h2>戰局紀錄</h2><label>視角 <select v-model="replayPerspective" aria-label="重播視角" @change="loadReplayFrame(replayFrame.currentStep)"><option v-for="player in replayFrame.players" :key="player.player" :value="player.player">{{ player.displayName }}</option></select></label></div>
                <BattleRecordTimeline :groups="battleRecordGroups" :current-turn-number="currentBattleTurnNumber" />
              </section>
            </aside>
          </div>
          <ReplayEventSheet
            :open="eventSheetOpen"
            :perspective="replayPerspective"
            :players="replayFrame.players"
            :groups="battleRecordGroups"
            :current-turn-number="currentBattleTurnNumber"
            @update:perspective="replayPerspective = $event; loadReplayFrame(replayFrame.currentStep)"
            @close="closeEventSheet"
          />
        </template>
        <div v-else-if="!replayLoading" class="replay-route-error page-surface" role="alert">
          <p class="muted">{{ replayError }}</p>
          <button v-if="replayRouteId" class="ghost-button" type="button" @click="loadReplayFrame(0)">重試</button>
        </div>
      </section>
  </main>
</template>

<script setup lang="ts">
import type {
  PlayerId,
  BattleRecord,
  PublicGameState,
  TeamId,
} from '~/types/fewfc'
import { battleRecordContentRevision, scrollBattleRecordToLatest } from '~/lib/battle-record-scroll'
import { battleRecordCurrentTurnNumber } from '~/lib/battle-record-timeline'

interface ReplayFrame {
  currentStep: number
  totalSteps: number
  state: PublicGameState
  battleRecord: BattleRecord
  players: Array<{ player: PlayerId, displayName: string }>
  firstPlayer: PlayerId
}

const route = useRoute()
const router = useRouter()
const replayFrame = ref<ReplayFrame | null>(null)
const replayLayoutStyle = computed(() => ({
  '--battle-page-background': `url('/backgrounds/${replayFrame.value?.state.environment?.toLowerCase() ?? 'void'}.webp')`,
}))
const replayLoading = ref(false)
const replayError = ref('')
const replayPerspective = ref<PlayerId>('')
const battleRecordFeed = ref<HTMLElement | null>(null)
const eventSheetOpen = ref(false)
const eventSheetTrigger = ref<HTMLButtonElement | null>(null)
const replayRouteId = computed(() => typeof route.params.replayId === 'string' ? route.params.replayId : '')
const replayDisplayNames = computed<Record<string, string>>(() => Object.fromEntries(
  (replayFrame.value?.players ?? []).map(player => [player.player, player.displayName]),
))
const battleRecordGroups = computed(() => {
  const frame = replayFrame.value
  const record = frame?.battleRecord
  if (!record) return []
  const groups = [
    { id: 'preparation', title: '對局準備', turnNumber: null, entries: record.preparation.entries },
    ...record.turns.map(group => ({ id: `turn-${group.turnNumber}`, title: group.title, turnNumber: group.turnNumber, entries: group.entries })),
  ].filter(group => group.id !== 'preparation' || group.entries.length > 0)
  const turnNumber = currentBattleTurnNumber.value
  if (turnNumber !== null && !groups.some(group => group.turnNumber === turnNumber)) {
    groups.push({
      id: `turn-${turnNumber}`,
      title: `第 ${turnNumber} 回合 · ${replayPlayerLabel(frame.state.currentPlayer ?? frame.firstPlayer)}的回合`,
      turnNumber,
      entries: [],
    })
  }
  return groups
})
const currentBattleTurnNumber = computed(() => {
  const state = replayFrame.value?.state
  return battleRecordCurrentTurnNumber(state?.status, state?.turnNumber)
})
const latestReplayEvent = computed(() => battleRecordGroups.value.flatMap(group => group.entries).at(-1) ?? null)
let replayLoadRevision = 0

async function loadReplayFrame(step = 0) {
  const replayId = replayRouteId.value
  if (!replayId) return
  const revision = ++replayLoadRevision
  replayLoading.value = true
  replayError.value = ''
  try {
    const frame = await $fetch<ReplayFrame>(
      `/api/replays/${encodeURIComponent(replayId)}`,
      { query: { step, perspective: replayPerspective.value || undefined } },
    )
    if (revision === replayLoadRevision) {
      replayFrame.value = frame
      if (!replayPerspective.value) replayPerspective.value = frame.firstPlayer
    }
  } catch (error) {
    if (revision === replayLoadRevision) {
      const status = (error as { statusCode?: number, status?: number, response?: { status?: number } }).statusCode
        ?? (error as { status?: number }).status
        ?? (error as { response?: { status?: number } }).response?.status
      replayError.value = status === 404 ? '找不到這個重播' : '無法載入重播，請稍後再試。'
    }
  } finally {
    if (revision === replayLoadRevision) replayLoading.value = false
  }
}

function openReplays() {
  void router.push('/replays')
}

function openEventSheet() {
  eventSheetOpen.value = true
}

function closeEventSheet() {
  eventSheetOpen.value = false
  void nextTick(() => eventSheetTrigger.value?.focus())
}

async function copyReplayLink(replayId: string) {
  if (!replayId) return
  await navigator.clipboard.writeText(new URL(`/replays/${encodeURIComponent(replayId)}`, window.location.origin).toString())
}

function replayPlayerLabel(player: PlayerId): string {
  return replayFrame.value?.players.find(entry => entry.player === player)?.displayName ?? player
}

function replayTeamLabel(team: TeamId): string {
  const perspective = replayPerspective.value || replayFrame.value?.firstPlayer
  const perspectiveTeam = replayFrame.value?.state.players.find(
    (player) => player.id === perspective,
  )?.team
  if (!perspectiveTeam) return '隊伍'
  return team === perspectiveTeam ? '我方' : '對方'
}

onMounted(() => {
  void loadReplayFrame(0)
})

watch(replayRouteId, () => {
  replayPerspective.value = ''
  replayFrame.value = null
  void loadReplayFrame(0)
})

watch(() => battleRecordContentRevision(battleRecordGroups.value), () => {
  void nextTick(() => {
    scrollBattleRecordToLatest(battleRecordFeed.value)
  })
})
</script>

<style scoped>
@reference "../../assets/css/main.css";

.replay-detail-page { max-width: 1680px; }
.replay-content { display: grid; gap: 20px; }
.replay-content .page-intro { margin-bottom: 0; }
.replay-detail-page:has(.battle-scene) :deep(.page-intro) { color: #f4f0e2; text-shadow: 0 2px 16px #10251b; }
.replay-detail-page:has(.battle-scene) :deep(.page-intro :is(.page-back,.page-kicker)) { color: #eed699; }
.replay-toolbar { display: flex; flex-wrap: wrap; align-items: center; gap: 10px; padding: 14px 18px; border-radius: 20px; }
.replay-progress { margin-right: auto; font-size: 13px; }
.replay-toolbar .ghost-button { min-height: 40px; }
.replay-route-error { display: grid; justify-items: start; gap: 18px; }
@media(max-width:600px) { .replay-toolbar { display: grid; grid-template-columns: repeat(3,minmax(0,1fr)); padding: 12px; gap: 8px; } .replay-progress { grid-column: 1 / -1; padding-bottom: 4px; } .replay-toolbar .ghost-button { padding-inline: 8px; font-size: 11px; } }
.replay-layout { @apply grid min-h-[min(700px,calc(100dvh-240px))]; grid-template-columns: minmax(0, 1fr) clamp(270px, 26vw, 360px); gap: 10px; }
.replay-battlefield { @apply min-h-[620px] rounded-[18px] shadow-[var(--app-shadow-md)]; }
.replay-sidebar { @apply grid min-h-0 grid-rows-[minmax(0,1fr)] overflow-hidden; }
.action-detail { @apply absolute right-0 bottom-[calc(100%+8px)] left-0 z-8 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3 text-left text-xs leading-5 text-muted shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.event-panel { @apply min-h-0 overflow-auto rounded-[24px] p-5; }
.panel-title { @apply flex items-start justify-between gap-3; }.panel-title h2 { @apply font-serif text-[15px]; }
.event-panel > .battle-record-timeline { @apply mt-4; }
.panel-title select { @apply max-w-36 border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-2 py-1 text-xs text-[var(--app-text)]; }
.replay-mobile-record-summary { display: none; }
@media (max-width: 900px) {
  .replay-layout { @apply grid-cols-1 grid-rows-[minmax(620px,1fr)_auto]; min-height: 0; gap: 0; padding: 0; }
  .replay-sidebar { display: none; }
  .replay-battlefield { @apply rounded-none; }
  .replay-mobile-record-summary { @apply flex min-w-0 items-center gap-2 border-0 px-3 py-2 text-left; background: linear-gradient(135deg, rgba(239,255,246,.14), transparent 60%), color-mix(in srgb, var(--app-surface-raised) 70%, transparent); box-shadow: 0 -8px 22px rgba(0,0,0,.18); -webkit-backdrop-filter: blur(20px) saturate(1.24); backdrop-filter: blur(20px) saturate(1.24); }
  .replay-mobile-record-summary strong { @apply shrink-0 font-serif text-[11px] text-gold-light; }
  .replay-mobile-record-summary span { @apply min-w-0 flex-1 truncate text-[9px] text-muted; }
  .replay-mobile-record-summary b { @apply text-[var(--app-accent-strong)]; }
}
</style>
