<template>
  <main class="lobby-page replay-detail-page">
      <section class="lobby-content" aria-label="重播播放器">
        <div class="lobby-heading"><h1>重播</h1><button class="ghost-button" type="button" @click="openReplays">返回重播清單</button></div>
        <p v-if="replayLoading" class="muted" role="status" aria-live="polite">正在載入重播…</p>
        <template v-else-if="replayFrame">
          <p>第 {{ replayFrame.currentStep }} / {{ replayFrame.totalSteps }} 步</p>
          <div class="result-actions">
            <button class="ghost-button" :disabled="replayFrame.currentStep === 0" @click="loadReplayFrame(0)">第一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === 0" @click="loadReplayFrame(replayFrame.currentStep - 1)">上一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === replayFrame.totalSteps" @click="loadReplayFrame(replayFrame.currentStep + 1)">下一步</button>
            <button class="ghost-button" :disabled="replayFrame.currentStep === replayFrame.totalSteps" @click="loadReplayFrame(replayFrame.totalSteps)">最後一步</button>
            <button class="ghost-button" @click="copyReplayLink(replayRouteId)">分享連結</button>
          </div>
          <div class="battle-layout replay-layout">
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
            </BattlefieldBoard>
          </div>
          <section ref="battleRecordFeed" class="event-panel expanded">
            <div class="panel-title"><h2>戰局紀錄</h2><label>視角 <select v-model="replayPerspective" @change="loadReplayFrame(replayFrame.currentStep)"><option v-for="player in replayFrame.players" :key="player.player" :value="player.player">{{ player.displayName }}</option></select></label></div>
            <ol class="event-feed"><template v-for="group in battleRecordGroups" :key="group.id"><li class="event-group-title"><strong>{{ group.title }}</strong></li><li v-for="entry in group.entries" :key="entry.id"><i /><div><span>{{ entry.title }}</span><p v-if="entry.summary">{{ entry.summary }}</p></div></li></template></ol>
          </section>
        </template>
        <div v-else class="replay-route-error" role="alert">
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
} from '~/types/fewfc'
import { scrollBattleRecordToLatest } from '~/lib/battle-record-scroll'

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
const replayLoading = ref(false)
const replayError = ref('')
const replayPerspective = ref<PlayerId>('')
const battleRecordFeed = ref<HTMLElement | null>(null)
const replayRouteId = computed(() => typeof route.params.replayId === 'string' ? route.params.replayId : '')
const replayDisplayNames = computed<Record<string, string>>(() => Object.fromEntries(
  (replayFrame.value?.players ?? []).map(player => [player.player, player.displayName]),
))
const battleRecordGroups = computed(() => {
  const record = replayFrame.value?.battleRecord
  if (!record) return []
  return [
    { id: 'preparation', title: '對局準備', entries: record.preparation.entries },
    ...record.turns.map(group => ({ id: `turn-${group.turnNumber}`, title: group.title, entries: group.entries })),
  ].filter(group => group.entries.length > 0)
})
let replayLoadRevision = 0

async function loadReplayFrame(step = 0) {
  const replayId = replayRouteId.value
  if (!replayId) return
  const revision = ++replayLoadRevision
  replayLoading.value = true
  replayFrame.value = null
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

async function copyReplayLink(replayId: string) {
  if (!replayId) return
  await navigator.clipboard.writeText(new URL(`/replays/${encodeURIComponent(replayId)}`, window.location.origin).toString())
}

function replayPlayerLabel(player: PlayerId): string {
  return replayFrame.value?.players.find(entry => entry.player === player)?.displayName ?? player
}

onMounted(() => {
  void loadReplayFrame(0)
})

watch(replayRouteId, () => {
  replayPerspective.value = ''
  void loadReplayFrame(0)
})

watch(() => battleRecordGroups.value.flatMap(group => group.entries).length, () => {
  void nextTick(() => {
    scrollBattleRecordToLatest(battleRecordFeed.value)
  })
})
</script>

<style scoped>
@reference "../../assets/css/main.css";

.lobby-page { @apply mx-auto max-w-[1180px] px-[30px] pt-15 pb-[90px] max-[600px]:px-4 max-[600px]:py-9; }
.lobby-content { @apply grid gap-5; }.lobby-heading { @apply flex items-center justify-between gap-4; }.lobby-heading h1 { @apply font-serif text-3xl text-gold-light; }.result-actions { @apply mt-2 grid grid-cols-2 gap-3; }.result-actions .ghost-button { @apply border-[var(--app-border-strong)] text-[var(--app-text)]; }
.replay-layout { @apply block; }.replay-battlefield { @apply min-h-[620px] rounded-[18px] shadow-[var(--app-shadow-md)]; }.action-detail { @apply absolute right-0 bottom-[calc(100%+8px)] left-0 z-8 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3 text-left text-xs leading-5 text-muted shadow-[0_12px_28px_rgba(0,0,0,.4)]; }.event-panel { @apply min-h-0 overflow-auto border-b border-line bg-panel p-5; }.panel-title { @apply flex items-start justify-between; }.panel-title h2 { @apply font-serif text-[15px]; }.event-feed { @apply mt-4 grid list-none gap-[13px] p-0; }.event-feed li { @apply grid grid-cols-[10px_1fr] gap-[7px]; }.event-feed li > i { @apply mt-1.5 size-[5px] rounded-full bg-[#b79550] shadow-[0_0_0_4px_rgba(183,149,80,.08)]; }.event-feed span { @apply text-[10px] font-bold text-[var(--app-text)]; }.event-feed p { @apply mt-0.5 text-[9px] leading-5 text-[var(--app-text-muted)]; }
.event-feed li.event-group-title { grid-template-columns: minmax(0, 1fr); }
</style>
