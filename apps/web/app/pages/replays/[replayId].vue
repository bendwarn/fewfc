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
            <section
              class="battlefield replay-battlefield"
              :class="{ 'four-player': replaySeats.length === 4 }"
              aria-label="重播戰場（唯讀）"
            >
              <div
                v-for="seat in replaySeats"
                :key="seat.player"
                class="player-seat"
                :class="[`seat-${seat.position}`, { acting: replayFrame.state.currentPlayer === seat.player }]"
              >
                <div class="player-identity">
                  <span class="avatar">{{ replayPlayerLabel(seat.player).slice(0, 1) }}</span>
                  <div>
                    <strong>{{ replayPlayerLabel(seat.player) }}</strong>
                    <small>{{ replayTeamHp(seat.player) }} HP</small>
                    <small v-if="replayPouch(seat.player)">錦囊 · {{ replayPouch(seat.player)?.label }}</small>
                    <small v-if="replayCoveredCards(seat.player).length">蓋牌 · {{ replayCoveredCards(seat.player).map(card => card.label).join('、') }}</small>
                    <small v-if="replayFrame.state.playerDecks.length">牌庫 {{ replayDeckCount(seat.player) }}</small>
                    <small v-else>共用牌庫 {{ replayFrame.state.deckCount ?? 0 }}</small>
                    <small v-if="replayPlayerDiscardCards(seat.player).length">棄牌 · {{ replayPlayerDiscardCards(seat.player).map(card => card.label).join('、') }}</small>
                  </div>
                  <span v-if="replayFrame.state.currentPlayer === seat.player" class="turn-badge">行動中</span>
                  <span class="side-hand-count">{{ replayCardsFor(seat.player).length }} 張</span>
                </div>
                <div class="hand fan seat-hand">
                  <GameCard
                    v-for="card in replayCardsFor(seat.player)"
                    :key="card.id"
                    :card="card.card"
                    :hidden="card.hidden"
                    :interpretations="replayFrame.state.cardInterpretations"
                  />
                </div>
              </div>
              <div class="board-center">
                <div class="discard-piles" aria-label="棄牌堆">
                  <div class="discard-pile"><span>棄牌</span><strong>{{ replayFrame.state.discard.length }}</strong></div>
                  <div v-for="pile in replayFrame.state.playerDiscards" :key="pile.player" class="discard-pile"><span>{{ replayPlayerLabel(pile.player) }} 棄牌</span><strong>{{ pile.cards.length }}</strong></div>
                </div>
                <div class="formation-field">
                  <div class="formation-field-heading"><span class="formation-field-label">陣法區</span><span v-if="replayFrame.state.environment" class="environment-badge">環境 · {{ environmentLabel(replayFrame.state.environment) }}</span></div>
                  <div class="previous-formation">
                    <template v-if="replayFrame.state.previousTurnFormation">
                      <small>上一回合 · {{ replayPlayerLabel(replayFrame.state.previousTurnFormation.player) }}</small>
                      <strong>{{ replayFrame.state.previousTurnFormation.formationName ?? '陣法' }}</strong>
                      <div class="formation-cards">
                        <GameCard
                          v-for="card in replayPreviousFormationCards"
                          :key="card.id"
                          class="formation-card"
                          :card="card.card"
                          :hidden="card.hidden"
                          :interpretations="replayFrame.state.cardInterpretations"
                        />
                      </div>
                    </template>
                    <p v-else>上一回合未發動陣法</p>
                  </div>
                </div>
              </div>
              <p
                v-if="replayFrame.state.pendingChoice?.visibility === 'visible'
                  && replayFrame.state.pendingChoice.choice.type === 'card'"
                class="action-detail"
              >
                {{ replayPlayerLabel(replayFrame.state.pendingChoice.player) }} 的選擇：{{ replayFrame.state.pendingChoice.choice.cards.map(card => card.label).join('、') }}
              </p>
            </section>
          </div>
          <section class="event-panel expanded"><div class="panel-title"><h2>戰局紀錄</h2></div><ol class="event-feed"><li v-for="event in replayFrame.events" :key="event.id"><div><span>{{ event.title }}</span><p>{{ event.summary }}</p></div></li></ol></section>
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
  Element,
  PlayerId,
  PublicCard,
  PublicCardRefs,
  PublicGameEvent,
  PublicGameState,
} from '~/types/fewfc'

interface ReplayFrame {
  currentStep: number
  totalSteps: number
  state: PublicGameState
  events: PublicGameEvent[]
  players: Array<{ player: PlayerId, displayName: string }>
  firstPlayer: PlayerId
}

type SeatPosition = 'top' | 'right' | 'bottom' | 'left'
interface PlayerSeat { player: PlayerId, position: SeatPosition }
interface CardToken {
  id: string
  card: PublicCard | null
  label: string
  element?: Element | null
  level?: number | null
  hidden: boolean
}

const route = useRoute()
const router = useRouter()
const replayFrame = ref<ReplayFrame | null>(null)
const replayLoading = ref(false)
const replayError = ref('')
const replayRouteId = computed(() => typeof route.params.replayId === 'string' ? route.params.replayId : '')
let replayLoadRevision = 0

const replaySeats = computed<PlayerSeat[]>(() => {
  const order = replayFrame.value?.state.turnOrder ?? []
  const firstPlayer = replayFrame.value?.firstPlayer
  const firstIndex = firstPlayer ? order.indexOf(firstPlayer) : 0
  const relativeOrder = firstIndex > 0 ? [...order.slice(firstIndex), ...order.slice(0, firstIndex)] : order
  const positions: SeatPosition[] = relativeOrder.length === 4 ? ['bottom', 'left', 'top', 'right'] : ['bottom', 'top']
  return relativeOrder.map((player, index) => ({ player, position: positions[index] ?? 'top' }))
})

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
      { query: { step } },
    )
    if (revision === replayLoadRevision) replayFrame.value = frame
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

function replayTeamHp(player: PlayerId): number {
  const state = replayFrame.value?.state
  const team = state?.players.find(entry => entry.id === player)?.team
  return state?.hp.find(entry => entry.team === team)?.hp ?? 0
}

function cardTokensForRefs(cards: PublicCardRefs | undefined, prefix: string): CardToken[] {
  if (!cards) return []
  if (cards.kind === 'known') {
    return cards.cards.map((card, index) => ({ id: `${prefix}-known-${index}-${card.id}`, card, label: card.label, element: card.element, level: card.level, hidden: false }))
  }
  if (cards.kind === 'partiallyKnown') {
    return cards.cards.map((card, index) => card
      ? { id: `${prefix}-known-${index}-${card.id}`, card, label: card.label, element: card.element, level: card.level, hidden: false }
      : { id: `${prefix}-hidden-${index}`, card: null, label: '', element: null, level: null, hidden: true })
  }
  return Array.from({ length: cards.count }, (_, index) => ({ id: `${prefix}-hidden-${index}`, card: null, label: '', element: null, level: null, hidden: true }))
}

function replayCardsFor(player: PlayerId): CardToken[] {
  return cardTokensForRefs(replayFrame.value?.state.hands.find(entry => entry.player === player)?.cards, `replay-hand-${player}`)
}

const replayPreviousFormationCards = computed(() => cardTokensForRefs(
  replayFrame.value?.state.previousTurnFormation?.cards,
  'replay-previous-formation',
))

function replayDeckCount(player: PlayerId): number {
  const cards = replayFrame.value?.state.playerDecks.find(entry => entry.player === player)?.cards
  return !cards ? 0 : cards.kind === 'hidden' ? cards.count : cards.cards.length
}

function replayPouch(player: PlayerId): PublicCard | undefined {
  return replayFrame.value?.state.pouches.find(entry => entry.owner === player)?.card ?? undefined
}

function replayCoveredCards(player: PlayerId): PublicCard[] {
  return replayFrame.value?.state.coveredPassives
    .filter(passive => passive.owner === player)
    .flatMap(passive => passive.cards.kind === 'known' ? passive.cards.cards : []) ?? []
}

function replayPlayerDiscardCards(player: PlayerId): PublicCard[] {
  return replayFrame.value?.state.playerDiscards.find(entry => entry.player === player)?.cards ?? []
}

function environmentLabel(value: PublicGameState['environment']): string {
  if (!value) return '無環境'
  return { Metal: '金行', Wood: '木行', Water: '水行', Fire: '火行', Earth: '土行' }[value]
}

onMounted(() => {
  void loadReplayFrame(0)
})

watch(replayRouteId, () => {
  void loadReplayFrame(0)
})
</script>

<style scoped>
@reference "../../assets/css/main.css";

.lobby-page { @apply mx-auto max-w-[1180px] px-[30px] pt-15 pb-[90px] max-[600px]:px-4 max-[600px]:py-9; }
.lobby-content { @apply grid gap-5; }.lobby-heading { @apply flex items-center justify-between gap-4; }.lobby-heading h1 { @apply font-serif text-3xl text-gold-light; }.result-actions { @apply mt-2 grid grid-cols-2 gap-3; }.result-actions .ghost-button { @apply border-[var(--app-border-strong)] text-[var(--app-text)]; }
.replay-layout { @apply block; }.replay-battlefield { @apply min-h-[620px]; }
.battlefield { position: relative; display: grid; grid-template-areas: 'top top top' 'left center right' 'bottom bottom bottom'; grid-template-columns: minmax(108px,.6fr) minmax(300px,1.8fr) minmax(108px,.6fr); grid-template-rows: minmax(150px,.8fr) minmax(230px,1.15fr) minmax(175px,1fr); gap: 8px 14px; padding: 22px 36px; overflow: hidden; border-radius: 18px; background: radial-gradient(ellipse at center,var(--app-battlefield-center) 0%,var(--app-battlefield-mid) 58%,var(--app-battlefield-edge) 100%); box-shadow: var(--app-shadow-md); }
.battlefield::before { content: ''; position: absolute; inset: 22px; border: 1px solid rgba(175,143,79,.18); pointer-events: none; }
.player-seat { @apply relative z-1 flex min-h-0 min-w-0 items-center justify-center gap-4; }.seat-top { grid-area: top; }.seat-bottom { grid-area: bottom; flex-direction: row-reverse; }.seat-left { grid-area: left; flex-direction: column; }.seat-right { grid-area: right; flex-direction: column; }
.player-identity { @apply flex min-w-0 flex-wrap items-center gap-2.5; }.player-identity div { @apply grid; }.player-identity strong { @apply max-w-36 truncate text-xs; }.player-identity small { @apply text-[11px] text-[#d0a450]; }.avatar { @apply grid size-[34px] place-items-center rounded-full bg-[#b48a47] font-extrabold text-[#141813]; }.turn-badge { @apply border border-[#477557] bg-[#16251b] px-[7px] py-[3px] text-[9px] whitespace-nowrap text-[#77bd8d]; }.side-hand-count { @apply hidden text-[9px] text-muted; }
.hand { @apply flex min-w-0 items-center justify-center gap-2; }
.board-center { grid-area:center; @apply relative z-1 grid min-w-0 grid-cols-[90px_minmax(220px,1fr)_90px] items-center justify-items-center; }.discard-piles { @apply z-2 flex w-full justify-end; grid-column:1/-1;grid-row:1; }.discard-pile { @apply grid justify-items-center gap-1.5 text-[9px] text-[var(--app-text-muted)]; }.formation-field { @apply relative z-3 grid min-h-48 w-full min-w-0 grid-rows-[auto_1fr_auto] items-center border-x border-[rgba(166,141,86,.14)] px-3 py-2 text-center text-[10px] text-[var(--app-text-muted)]; grid-column:2;grid-row:1; }.formation-field-heading { @apply flex flex-wrap items-center justify-center gap-2; }.formation-field-label { @apply text-[#9a8251];letter-spacing:.2em; }.environment-badge { @apply border border-[var(--app-accent)] bg-[var(--app-surface-raised)] px-2 py-1 text-[9px] text-gold-light; }.previous-formation { @apply grid min-h-24 content-center justify-items-center gap-1.5; }.previous-formation small { @apply text-[9px] text-muted; }.previous-formation strong { @apply font-serif text-sm text-gold-light; }.previous-formation p { @apply text-[10px] text-[var(--app-text-muted)]; }.formation-cards { @apply flex min-h-12 items-center justify-center; }.formation-card { width: 34px; margin-left: -4px; }.action-detail { @apply absolute right-0 bottom-[calc(100%+8px)] left-0 z-8 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3 text-left text-xs leading-5 text-muted shadow-[0_12px_28px_rgba(0,0,0,.4)]; }.event-panel { @apply min-h-0 overflow-auto border-b border-line bg-panel p-5; }.panel-title { @apply flex items-start justify-between; }.panel-title h2 { @apply font-serif text-[15px]; }.event-feed { @apply mt-4 grid list-none gap-[13px] p-0; }.event-feed li { @apply grid grid-cols-[10px_1fr] gap-[7px]; }.event-feed span { @apply text-[10px] font-bold text-[var(--app-text)]; }.event-feed p { @apply mt-0.5 text-[9px] leading-5 text-[var(--app-text-muted)]; }
@media (max-width: 900px) { .battlefield { min-height:720px; padding:22px; }.side-hand-count { @apply block; } } @media (max-width:600px) { .lobby-page { padding:36px 16px; }.battlefield { min-height:690px;grid-template-columns:76px minmax(0,1fr) 76px;grid-template-rows:165px minmax(250px,1fr) 185px;gap:4px;padding:46px 8px 12px; }.seat-left .seat-hand,.seat-right .seat-hand { display:none; }.playing-card { width:54px; }.seat-top .playing-card { width:43px; }.board-center { grid-template-columns:48px minmax(0,1fr) 48px;width:100%; } }
</style>
