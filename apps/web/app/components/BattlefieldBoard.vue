<template>
  <section
    ref="root"
    class="battlefield"
    :class="{
      'four-player': seats.length === 4,
      'personal-deck': usesPersonalDeck,
      'detail-open': hasOpenDetail(),
    }"
    :style="{ '--opponent-count': Math.max(1, seats.length - 1) }"
    :aria-label="mode === 'replay' ? '重播戰場（唯讀）' : '五行戰鬥牌對戰桌'"
  >
    <slot name="before" />

    <div
      v-for="seat in seats"
      :key="seat.player"
      class="player-seat"
      :class="[`seat-${seat.position}`, { acting: isActing(seat.player) }]"
      :style="{ '--compact-column': seat.compactColumn }"
      :aria-label="seatAriaLabel(seat.player)"
      :aria-current="isActing(seat.player) ? 'true' : undefined"
    >
      <div class="player-identity">
        <div class="player-heading">
          <span
            v-if="mode === 'live'"
            class="connection-dot"
            :class="{ connected: playerConnected(seat.player) }"
            :title="playerConnected(seat.player) ? '已連線' : '已斷線'"
          />
          <span class="avatar" aria-hidden="true">{{ playerInitial(seat.player) }}</span>
          <span class="player-name-block">
            <strong :title="playerLabel(seat.player)">{{ playerLabel(seat.player) }}</strong>
            <small>{{ teamHp(teamForPlayer(seat.player)) }} HP</small>
          </span>
          <span v-if="isActing(seat.player)" class="acting-marker" aria-hidden="true">▶</span>
        </div>

        <div class="player-facts" :title="playerFacts(seat.player)">
          <span v-if="teamStar(teamForPlayer(seat.player))">星辰 · {{ starLabel(teamStar(teamForPlayer(seat.player))!) }}</span>
          <span v-if="state.enabledRuleModules.includes('star') && starHistoryLabel(seat.player)">召星 · {{ starHistoryLabel(seat.player) }}</span>
          <span v-if="spiritFor(seat.player)" class="spirit-status">精靈 · {{ spiritLabel(spiritFor(seat.player)!.spirit) }} · 靈力 {{ spiritFor(seat.player)!.power }} / 6</span>
          <span v-if="pouchFor(seat.player)">錦囊 · {{ pouchFor(seat.player)?.card?.label ?? '覆蓋牌' }}</span>
          <span v-if="coveredPassiveSummary(seat.player)">蓋牌 · {{ coveredPassiveSummary(seat.player) }}</span>
          <span v-for="card in exposedDeckCards(seat.player)" :key="`exposed-deck-${seat.player}-${card.id}`">公開牌 · {{ card.label }}</span>
        </div>

        <div class="player-statuses">
          <span v-if="reconnectingPlayer === seat.player" class="reconnecting-label">重新連線中</span>
          <span v-for="counter in counterEffectsFor(seat.player)" :key="`${seat.player}-${counter.effectId}`" class="counter-badge">反制 · {{ counter.effectName }}</span>
          <span v-if="shieldFor(seat.player) > 0" class="shield-badge">防護罩 · {{ shieldFor(seat.player) }}</span>
          <span
            v-if="professionFor(seat.player)"
            class="profession-badge"
            tabindex="0"
            :aria-label="professionSummaryLabel(seat.player)"
          >
            職業 · {{ professionFor(seat.player)!.name }}
            <span class="profession-summary" role="note">
              <b>{{ professionFor(seat.player)!.name }}</b>
              <small v-for="ability in professionFor(seat.player)!.abilities" :key="ability">{{ ability }}</small>
            </span>
          </span>
          <button
            v-if="persistentEffectsFor(seat.player).length"
            class="effect-summary"
            type="button"
            aria-haspopup="dialog"
            :aria-controls="`effect-detail-${seat.player}`"
            :aria-expanded="effectDetailPlayer === seat.player"
            @click.stop="openEffectDetail(seat.player, $event.currentTarget as HTMLButtonElement)"
          >
            效果 {{ persistentEffectsFor(seat.player).length }}<span class="effect-preview">｜{{ persistentEffectsFor(seat.player).map(effect => effect.label).join('、') }}</span>
          </button>
        </div>
      </div>

      <DiscardPileControl
        v-if="usesPersonalDeck"
        class="seat-discard-control"
        :owner="seat.player"
        :owner-label="playerLabel(seat.player)"
        :count="personalCounts(seat.player).discard"
        :deck-count="personalCounts(seat.player).deck"
        :featured-card="personalFeaturedDiscard(seat.player)"
        :unavailable="discardUnavailable(seat.player)"
        :open="discardDetail?.owner === seat.player"
        :position="seat.position"
        @toggle="openDiscardDetail(seat.player, $event)"
      />

      <button
        v-if="showReplayHandButton(seat.player, seat.position)"
        class="compact-hand-count compact-hand-button"
        type="button"
        aria-haspopup="dialog"
        :aria-controls="`hand-detail-${seat.player}`"
        :aria-expanded="handDetailPlayer === seat.player"
        @click.stop="openHandDetail(seat.player, $event.currentTarget as HTMLButtonElement)"
      >
        手牌 {{ handCount(seat.player) }}
      </button>
      <span v-else-if="seat.position !== 'bottom'" class="compact-hand-count">手牌 {{ handCount(seat.player) }}</span>

      <div class="hand fan seat-hand" :aria-label="`${playerLabel(seat.player)}的手牌`">
        <GameCard
          v-for="(card, index) in cardsFor(seat.player)"
          :key="card.id"
          :card="card.card"
          :hidden="card.hidden"
          :selectable="card.selectable"
          :selected="card.selected"
          :disabled="cardsDisabled"
          :interpretations="state.cardInterpretations"
          :data-card-id="card.cardId"
          :shortcut="card.selectable ? shortcutLabel(index) : undefined"
          @select="emit('select-card', seat.player, card.cardId)"
        />
      </div>
    </div>

    <div v-if="!usesPersonalDeck" class="shared-discard-dock" aria-label="共用牌堆">
      <DiscardPileControl
        :owner="null"
        :count="sharedCounts.discard"
        :deck-count="sharedCounts.deck"
        :featured-card="sharedFeaturedDiscard"
        :unavailable="discardUnavailable(null)"
        :open="discardDetail?.owner === null"
        @toggle="openDiscardDetail(null, $event)"
      />
    </div>

    <div class="center-stack">
      <div class="board-center">
        <div class="formation-field">
          <div class="formation-field-heading">
            <span class="formation-field-label">陣法區</span>
            <span v-if="state.environment" class="environment-badge" aria-live="polite">環境 · {{ environmentLabel(state.environment) }}</span>
          </div>
          <div class="previous-formation">
            <template v-if="state.previousTurnFormation">
              <small>上一回合 · {{ playerLabel(state.previousTurnFormation.player) }}</small>
              <strong>{{ state.previousTurnFormation.formationName ?? (mode === 'live' ? '蓋牌' : '陣法') }}</strong>
              <div class="formation-cards">
                <GameCard
                  v-for="card in previousFormationCards"
                  :key="card.id"
                  class="formation-card"
                  :card="card.card"
                  :hidden="card.hidden"
                  :interpretations="state.cardInterpretations"
                />
              </div>
            </template>
            <p v-else>上一回合未發動陣法</p>
          </div>
        </div>
        <slot name="board-overlay" />
      </div>
      <div class="action-dock"><slot name="turn-controls" /></div>
    </div>

    <slot name="overlay" />

    <BattlefieldDetailLayer
      :open="discardDetail !== null"
      id="discard-composition"
      :title="discardDetailTitle"
      kind="discard"
      :anchor="discardAnchor"
      @close="closeDiscardDetail"
    >
      <div v-if="discardComposition.length" class="card-composition">
        <table>
          <caption class="sr-only">依五行與等級統計棄牌張數。列為五行，欄為等級。</caption>
          <thead><tr><th scope="col"><span class="sr-only">五行</span></th><th v-for="level in CARD_LEVELS" :key="level" scope="col">{{ level }} 級</th></tr></thead>
          <tbody>
            <tr v-for="row in discardComposition" :key="row.element">
              <th scope="row">{{ row.element }}</th>
              <td v-for="cell in row.cells" :key="`${cell.element}-${cell.level}`" :class="{ empty: cell.count === 0 }"><strong>{{ cell.count }}</strong></td>
            </tr>
          </tbody>
        </table>
      </div>
      <p v-else class="empty-detail">目前沒有棄牌。</p>
    </BattlefieldDetailLayer>

    <BattlefieldDetailLayer
      :open="effectDetailPlayer !== null"
      :id="`effect-detail-${effectDetailPlayer ?? 'player'}`"
      :title="effectDetailTitle"
      kind="effects"
      :anchor="effectAnchor"
      @close="closeEffectDetail"
    >
      <ul class="effect-detail-list">
        <li v-for="effect in activeEffects" :key="effect.key">{{ effect.label }}</li>
      </ul>
    </BattlefieldDetailLayer>

    <BattlefieldDetailLayer
      :open="handDetailPlayer !== null"
      :id="`hand-detail-${handDetailPlayer ?? 'player'}`"
      :title="handDetailTitle"
      kind="hand"
      :anchor="handAnchor"
      @close="closeHandDetail"
    >
      <div class="hand-detail-cards">
        <GameCard v-for="card in activeVisibleHandCards" :key="card.id" :card="card" :interpretations="state.cardInterpretations" />
      </div>
    </BattlefieldDetailLayer>
  </section>
</template>

<script setup lang="ts">
import type { CardInstanceId, Element, PlayerId, PublicCard, PublicCardRefs, PublicGameState, SpiritKind, TeamId } from '~/types/fewfc'
import { buildCardComposition, CARD_LEVELS } from '~/lib/card-composition'
import { cardElementGlyph } from '~/lib/card-face-presentation'
import {
  lastCompletedTurnDiscardForPlayer,
  latestSharedTurnDiscard,
  personalPileCounts,
  publicCardRefCount,
  sharedPileCounts,
} from '~/lib/battlefield-presentation'
import { presentPersistentEffects } from '~/lib/persistent-effect-presentation'

type SeatPosition = 'top' | 'right' | 'bottom' | 'left'
interface CardToken { id: string, cardId: number, card: PublicCard | null, hidden: boolean, selectable: boolean, selected: boolean }
interface AnchorRect { top: number, right: number, bottom: number, left: number }

const props = withDefaults(defineProps<{
  state: PublicGameState
  displayNames: Record<string, string>
  anchorPlayer: PlayerId | null
  mode: 'live' | 'replay'
  selectedCardIds?: readonly CardInstanceId[]
  selectablePlayer?: PlayerId | null
  cardsDisabled?: boolean
  connectedPlayers?: readonly PlayerId[]
  reconnectingPlayer?: PlayerId | null
}>(), {
  selectedCardIds: () => [],
  selectablePlayer: null,
  cardsDisabled: false,
  connectedPlayers: () => [],
  reconnectingPlayer: null,
})

const emit = defineEmits<{ 'select-card': [player: PlayerId, card: number] }>()
const root = ref<HTMLElement | null>(null)
const discardDetail = ref<{ owner: PlayerId | null } | null>(null)
const discardAnchor = ref<AnchorRect | null>(null)
const discardTrigger = ref<HTMLButtonElement | null>(null)
const effectDetailPlayer = ref<PlayerId | null>(null)
const effectAnchor = ref<AnchorRect | null>(null)
const effectTrigger = ref<HTMLButtonElement | null>(null)
const handDetailPlayer = ref<PlayerId | null>(null)
const handAnchor = ref<AnchorRect | null>(null)
const handTrigger = ref<HTMLButtonElement | null>(null)
const HAND_SHORTCUT_KEYS = ['1', '2', '3', '4', '5'] as const

const seats = computed(() => {
  const order = props.state.turnOrder
  const index = props.anchorPlayer ? order.indexOf(props.anchorPlayer) : 0
  const relative = index > 0 ? [...order.slice(index), ...order.slice(0, index)] : order
  const positions: SeatPosition[] = relative.length === 4 ? ['bottom', 'left', 'top', 'right'] : ['bottom', 'top']
  let compactColumn = 0
  return relative.map((player, position) => {
    const seatPosition = positions[position] ?? 'top'
    if (seatPosition !== 'bottom') compactColumn += 1
    return { player, position: seatPosition, compactColumn: seatPosition === 'bottom' ? 1 : compactColumn }
  })
})
const usesPersonalDeck = computed(() => props.state.enabledRuleModules.includes('personal-deck') || props.state.playerDiscards.length > 0)
const sharedCounts = computed(() => sharedPileCounts(props.state))
const sharedFeaturedDiscard = computed(() => latestSharedTurnDiscard(props.state)?.card ?? null)
const previousFormationCards = computed(() => cardTokensForRefs(props.state.previousTurnFormation?.cards, 'previous-formation'))
const activeDiscardCards = computed(() => {
  const detail = discardDetail.value
  if (!detail) return []
  if (detail.owner === null) return props.state.discard
  return props.state.playerDiscards.find(pile => pile.player === detail.owner)?.cards ?? []
})
const activeDiscardCounts = computed(() => {
  const owner = discardDetail.value?.owner
  return owner ? personalPileCounts(props.state, owner) : sharedPileCounts(props.state)
})
const discardDetailTitle = computed(() => {
  const owner = discardDetail.value?.owner
  const prefix = owner ? `${playerLabel(owner)}` : ''
  return `${prefix}目前棄牌 ${activeDiscardCounts.value.discard} · 牌庫 ${activeDiscardCounts.value.deck}`
})
const discardComposition = computed(() => activeDiscardCards.value.length ? buildCardComposition(activeDiscardCards.value) : [])
const activeEffects = computed(() => effectDetailPlayer.value ? persistentEffectsFor(effectDetailPlayer.value) : [])
const effectDetailTitle = computed(() => effectDetailPlayer.value ? `${playerLabel(effectDetailPlayer.value)}的效果 ${activeEffects.value.length}` : '效果')
const activeVisibleHandCards = computed(() => handDetailPlayer.value ? visibleHandCards(handDetailPlayer.value) : [])
const handDetailTitle = computed(() => handDetailPlayer.value ? `${playerLabel(handDetailPlayer.value)}的手牌 ${handCount(handDetailPlayer.value)}` : '手牌')

watch(
  [
    () => props.state.pendingChoice,
    () => props.state.pendingRandomness,
    () => activeDiscardCards.value.length,
  ],
  () => {
    if (discardDetail.value && discardUnavailable(discardDetail.value.owner)) closeDiscardDetail()
  },
)

function hasOpenDetail() { return discardDetail.value !== null || effectDetailPlayer.value !== null || handDetailPlayer.value !== null }
function closeDetail() {
  if (discardDetail.value) closeDiscardDetail()
  else if (effectDetailPlayer.value) closeEffectDetail()
  else if (handDetailPlayer.value) closeHandDetail()
}
defineExpose({ root, hasOpenDetail, closeDetail })

function triggerRect(trigger: HTMLButtonElement): AnchorRect {
  const rect = trigger.getBoundingClientRect()
  return { top: rect.top, right: rect.right, bottom: rect.bottom, left: rect.left }
}
function closeOtherDetails(except: 'discard' | 'effects' | 'hand') {
  if (except !== 'discard') discardDetail.value = null
  if (except !== 'effects') effectDetailPlayer.value = null
  if (except !== 'hand') handDetailPlayer.value = null
}
function openDiscardDetail(owner: PlayerId | null, trigger: HTMLButtonElement) {
  if (discardDetail.value?.owner === owner) return closeDiscardDetail()
  if (discardUnavailable(owner)) return
  closeOtherDetails('discard')
  discardTrigger.value = trigger
  discardAnchor.value = triggerRect(trigger)
  discardDetail.value = { owner }
}
function closeDiscardDetail() {
  if (!discardDetail.value) return
  discardDetail.value = null
  void nextTick(() => discardTrigger.value?.focus())
}
function openEffectDetail(player: PlayerId, trigger: HTMLButtonElement) {
  if (effectDetailPlayer.value === player) return closeEffectDetail()
  closeOtherDetails('effects')
  effectTrigger.value = trigger
  effectAnchor.value = triggerRect(trigger)
  effectDetailPlayer.value = player
}
function closeEffectDetail() {
  if (!effectDetailPlayer.value) return
  effectDetailPlayer.value = null
  void nextTick(() => effectTrigger.value?.focus())
}
function openHandDetail(player: PlayerId, trigger: HTMLButtonElement) {
  if (handDetailPlayer.value === player) return closeHandDetail()
  closeOtherDetails('hand')
  handTrigger.value = trigger
  handAnchor.value = triggerRect(trigger)
  handDetailPlayer.value = player
}
function closeHandDetail() {
  if (!handDetailPlayer.value) return
  handDetailPlayer.value = null
  void nextTick(() => handTrigger.value?.focus())
}

function cardTokensForRefs(cards: PublicCardRefs | undefined, prefix: string): CardToken[] {
  if (!cards) return []
  if (cards.kind === 'known') return cards.cards.map((card, index) => ({ id: `${prefix}-known-${index}-${card.id}`, cardId: card.id, card, hidden: false, selectable: false, selected: false }))
  if (cards.kind === 'partiallyKnown') return cards.cards.map((card, index) => card
    ? { id: `${prefix}-known-${index}-${card.id}`, cardId: card.id, card, hidden: false, selectable: false, selected: false }
    : { id: `${prefix}-hidden-${index}`, cardId: -index - 1, card: null, hidden: true, selectable: false, selected: false })
  return Array.from({ length: cards.count }, (_, index) => ({ id: `${prefix}-hidden-${index}`, cardId: -index - 1, card: null, hidden: true, selectable: false, selected: false }))
}
function cardsFor(player: PlayerId): CardToken[] {
  const cards = props.state.hands.find(hand => hand.player === player)?.cards
  return cardTokensForRefs(cards, `hand-${player}`).map(token => ({
    ...token,
    selectable: !token.hidden && props.mode === 'live' && props.selectablePlayer === player,
    selected: props.selectedCardIds.includes(token.cardId),
  }))
}
function visibleHandCards(player: PlayerId) { return cardsFor(player).flatMap(token => token.card ? [token.card] : []) }
function showReplayHandButton(player: PlayerId, position: SeatPosition) { return props.mode === 'replay' && position !== 'bottom' && visibleHandCards(player).length > 0 }
function personalCounts(player: PlayerId) { return personalPileCounts(props.state, player) }
function personalFeaturedDiscard(player: PlayerId) { return lastCompletedTurnDiscardForPlayer(props.state, player)?.card ?? null }
function discardUnavailable(owner: PlayerId | null) {
  const count = owner === null
    ? props.state.discard.length
    : props.state.playerDiscards.find(pile => pile.player === owner)?.cards.length ?? 0
  return count === 0 || Boolean(props.state.pendingChoice) || Boolean(props.state.pendingRandomness)
}
function playerLabel(player: PlayerId | null): string { return player ? props.displayNames[player] ?? player : '準備開始' }
function playerInitial(player: PlayerId) { return playerLabel(player).trim().charAt(0).toUpperCase() || '玩' }
function teamForPlayer(player: PlayerId): TeamId { return props.state.players.find(candidate => candidate.id === player)?.team ?? '' }
function teamHp(team: TeamId) { return props.state.hp.find(entry => entry.team === team)?.hp ?? 0 }
function handCount(player: PlayerId) { const cards = props.state.hands.find(entry => entry.player === player)?.cards; return !cards ? 0 : cards.kind === 'hidden' ? cards.count : cards.cards.length }
function teamStar(team: TeamId) { return props.state.teamStars.find(entry => entry.team === team)?.star }
function starHistoryLabel(player: PlayerId) { return (props.state.starHistories.find(entry => entry.player === player)?.stars ?? []).map(cardElementGlyph).join('、') }
function spiritFor(player: PlayerId) { return props.state.spirits.find(entry => entry.player === player) }
function spiritLabel(spirit: SpiritKind) { return { Metal: '金精靈', Wood: '木精靈', Water: '水精靈', Fire: '火精靈', Earth: '土精靈', Evil: '惡精靈', Death: '死精靈' }[spirit] }
function pouchFor(player: PlayerId) { return props.state.pouches.find(pouch => pouch.owner === player) }
function coveredPassiveSummary(player: PlayerId) {
  const passives = props.state.coveredPassives.filter(passive => passive.owner === player)
  const visible = passives.flatMap((passive) => {
    if (passive.cards.kind === 'known') return passive.cards.cards.map(card => card.label)
    if (passive.cards.kind === 'partiallyKnown') return passive.cards.cards.flatMap(card => card ? [card.label] : [])
    return []
  })
  if (visible.length) return visible.join('、')
  const count = passives.reduce((total, passive) => total + publicCardRefCount(passive.cards), 0)
  return count ? `${count} 張` : ''
}
function professionFor(player: PlayerId) { return props.state.professions.find(entry => entry.player === player) }
function professionSummaryLabel(player: PlayerId) { const profession = professionFor(player); return profession ? `職業 ${profession.name}。能力：${profession.abilities.join('；')}` : '' }
function persistentEffectsFor(player: PlayerId) { return presentPersistentEffects(props.state, player, teamForPlayer(player)) }
function counterEffectsFor(player: PlayerId) { return props.state.counterEffects.filter(entry => entry.owner === player) }
function shieldFor(player: PlayerId) { return props.state.shields.find(entry => entry.player === player)?.value ?? 0 }
function exposedDeckCards(player: PlayerId) { const cards = props.state.playerDecks.find(entry => entry.player === player)?.cards; return cards?.kind === 'partiallyKnown' ? cards.cards.filter((card): card is PublicCard => card !== null) : [] }
function playerConnected(player: PlayerId) { return props.connectedPlayers.includes(player) }
function shortcutLabel(index: number) { return HAND_SHORTCUT_KEYS[index]?.toUpperCase() }
function starLabel(star: NonNullable<ReturnType<typeof teamStar>>) { return { Metal: '金星‧太白', Wood: '木星‧歲星', Water: '水星‧辰星', Fire: '火星‧熒惑', Earth: '土星‧鎮星' }[star] }
function environmentLabel(value: Element | null) { return value ? { Metal: '金行', Wood: '木行', Water: '水行', Fire: '火行', Earth: '土行' }[value] : '無環境' }
function isActing(player: PlayerId) { return props.state.status !== 'Finished' && props.state.currentPlayer === player }
function playerFacts(player: PlayerId) {
  const facts = [
    teamStar(teamForPlayer(player)) ? `星辰 ${starLabel(teamStar(teamForPlayer(player))!)}` : '',
    spiritFor(player) ? `${spiritLabel(spiritFor(player)!.spirit)}，靈力 ${spiritFor(player)!.power} / 6` : '',
    pouchFor(player) ? `錦囊 ${pouchFor(player)?.card?.label ?? '覆蓋牌'}` : '',
  ]
  return facts.filter(Boolean).join('。')
}
function seatAriaLabel(player: PlayerId) {
  const current = isActing(player) ? `，目前行動玩家，${props.mode === 'live' ? phaseLabel(props.state.phase) : ''}` : ''
  return `${playerLabel(player)}，${teamHp(teamForPlayer(player))} HP${current}`
}
function phaseLabel(value: string) { return { TurnStart: '回合開始', ActiveEffects: '能力階段', Action: '行動階段', TurnDraw: '抽牌階段', TurnEnd: '回合結束' }[value] ?? value }
</script>

<style scoped>
@reference "../assets/css/main.css";

.battlefield {
  --card-back-base: #232e28;
  --card-back-stripe: #344039;
  --card-back-border: #85714a;
  @apply relative grid size-full min-h-0 min-w-0 overflow-hidden;
  grid-template-areas: 'top top top' 'left center right' 'bottom bottom bottom';
  grid-template-columns: minmax(108px, .6fr) minmax(360px, 1.9fr) minmax(108px, .6fr);
  grid-template-rows: minmax(128px, .75fr) minmax(230px, 1.25fr) minmax(160px, .9fr);
  gap: 8px 14px;
  padding: 22px 36px;
  background: radial-gradient(ellipse at center, var(--app-battlefield-center) 0%, var(--app-battlefield-mid) 58%, var(--app-battlefield-edge) 100%);
}
.battlefield::before { content: ''; @apply pointer-events-none absolute inset-[22px] border border-[rgba(175,143,79,.18)]; }
.battlefield::after { content: '五 行'; @apply pointer-events-none absolute top-1/2 left-1/2 grid size-[270px] -translate-1/2 place-items-center rounded-full border border-[rgba(183,148,77,.1)] font-serif text-[70px] text-[rgba(204,171,100,.06)]; }
.battlefield.detail-open { z-index: 25; }
.player-seat { @apply relative z-1 flex min-h-0 min-w-0 items-center justify-center gap-3 rounded-xl p-2; }
.seat-top { grid-area: top; }
.seat-bottom { grid-area: bottom; flex-direction: row-reverse; }
.seat-left { grid-area: left; flex-direction: column; }
.seat-right { grid-area: right; flex-direction: column; }
.player-seat.acting { outline: 2px solid #d9b661; outline-offset: -2px; box-shadow: 0 0 0 4px rgba(113, 174, 132, .58), 0 0 24px rgba(211, 177, 93, .18); }
.player-identity { @apply grid min-w-0 max-w-full gap-1.5; }
.player-heading { @apply flex min-w-0 items-center gap-2; }
.player-name-block { @apply grid min-w-0; }
.player-name-block strong { @apply max-w-40 truncate text-xs; }
.player-name-block small { @apply text-[11px] text-[#d0a450]; }
.avatar { @apply grid size-[34px] shrink-0 place-items-center rounded-full bg-[#b48a47] font-extrabold text-[#141813]; }
.acting-marker { @apply ml-auto shrink-0 text-xs text-[#f1d485]; }
.connection-dot { @apply size-2 shrink-0 rounded-full border border-[#76524b] bg-[#6f3c34]; }
.connection-dot.connected { @apply border-[#477557] bg-[#63a979]; }
.player-facts { @apply flex min-w-0 max-w-full gap-2 overflow-hidden text-[10px] whitespace-nowrap text-[#d0a450]; }
.player-facts span { @apply shrink-0; }
.player-statuses { @apply flex min-w-0 max-w-full flex-wrap gap-1; }
.reconnecting-label, .counter-badge, .shield-badge, .profession-badge, .effect-summary { @apply max-w-full truncate border px-[7px] py-[3px] text-[9px] whitespace-nowrap; }
.reconnecting-label { @apply border-[#8a733b] bg-[#292415] text-[#d5b868]; }
.counter-badge { @apply border-[#8a733b] bg-[#292415] text-[#d5b868]; }
.shield-badge { @apply border-[#557684] bg-[#17262c] text-[#8fc1d5]; }
.effect-summary { @apply border-[#765557] bg-[#28191b] text-[#d49a9a] hover:border-[#b7787d] hover:text-[#efb9b9]; }
.profession-badge { @apply relative cursor-help border-[var(--app-accent)] bg-[var(--app-surface-raised)] text-gold-light outline-none; }
.profession-summary { @apply invisible absolute top-[calc(100%+6px)] left-0 z-20 grid w-64 gap-1 border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-2.5 text-left opacity-0 shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.profession-summary b { @apply font-serif text-xs text-gold-light; }
.profession-summary small { @apply whitespace-normal text-[10px] leading-4 text-muted; }
.profession-badge:hover .profession-summary, .profession-badge:focus .profession-summary { @apply visible opacity-100; }
.hand { @apply flex min-w-0 items-center justify-center gap-2; }
.seat-top :deep(.playing-card) { width: clamp(48px, 5vw, 68px); }
.seat-left .seat-hand, .seat-right .seat-hand { @apply flex-col gap-1; }
.seat-left :deep(.playing-card), .seat-right :deep(.playing-card) { width: 30px; }
.compact-hand-count { @apply hidden text-[10px] whitespace-nowrap text-muted; }
.compact-hand-button { @apply border border-[var(--app-border-strong)] bg-[var(--app-surface-raised)] px-2 py-1 hover:border-[var(--app-accent)] hover:text-gold-light; }
.seat-discard-control { @apply shrink-0; }
.shared-discard-dock { @apply absolute top-4 right-4 z-8; }
.center-stack { grid-area: center; @apply relative z-2 grid min-h-0 min-w-0 grid-rows-[minmax(112px,1fr)_auto] gap-2; }
.board-center { @apply relative grid min-h-0 min-w-0 place-items-center; }
.formation-field { @apply relative grid size-full min-h-28 min-w-0 grid-rows-[auto_1fr] items-center border-x border-[rgba(166,141,86,.14)] px-3 py-2 text-center text-[10px] text-[var(--app-text-muted)]; }
.formation-field-heading { @apply flex flex-wrap items-center justify-center gap-2; }
.formation-field-label { @apply text-[#9a8251]; letter-spacing: .2em; }
.environment-badge { @apply border border-[var(--app-accent)] bg-[var(--app-surface-raised)] px-2 py-1 text-[9px] text-gold-light; }
.previous-formation { @apply grid min-h-20 content-center justify-items-center gap-1; }
.previous-formation small { @apply text-[9px] text-muted; }
.previous-formation strong { @apply font-serif text-sm text-gold-light; }
.previous-formation p { @apply text-[10px] text-[var(--app-text-muted)]; }
.formation-cards { @apply flex min-h-10 items-center justify-center; }
.formation-card { width: 34px; margin-left: -4px; }
.action-dock { @apply relative z-5 min-h-0 min-w-0 overflow-y-auto; overscroll-behavior: contain; }
.card-composition table { @apply w-full table-fixed border-collapse; }
.card-composition th, .card-composition td { @apply h-8 border border-[var(--app-border)] text-center; }
.card-composition thead th { @apply text-[10px] font-bold text-[var(--app-text)]; }
.card-composition tbody th { @apply w-7 text-[10px] font-normal text-muted; }
.card-composition td strong { @apply font-serif text-sm text-[#e4c47d]; }
.card-composition td.empty strong { color: var(--app-text-soft); }
.effect-detail-list { @apply grid list-none gap-2 p-0; }
.effect-detail-list li { @apply border-l-2 border-[#8f5f63] bg-[rgba(118,85,87,.12)] px-3 py-2 text-xs leading-5; }
.hand-detail-cards { @apply flex flex-wrap justify-center gap-2; }
.hand-detail-cards :deep(.playing-card) { width: 78px; }
.empty-detail { @apply text-sm text-muted; }

@media (min-width: 1200px) {
  .battlefield:not(.four-player) {
    grid-template-areas: 'top' 'center' 'bottom';
    grid-template-columns: minmax(0, 1fr);
    grid-template-rows: minmax(128px, .75fr) minmax(230px, 1.25fr) minmax(160px, .9fr);
  }
  .battlefield:not(.four-player) .player-seat { @apply w-full; }
  .battlefield:not(.four-player) .player-identity { @apply max-w-[42%]; }
  .player-name-block strong { font-size: 14px; }
  .player-name-block small, .player-facts { font-size: 12px; }
  .reconnecting-label, .counter-badge, .shield-badge, .profession-badge, .effect-summary { font-size: 11px; }
  .formation-field { font-size: 12px; }
  .previous-formation small { font-size: 11px; }
  .previous-formation p { font-size: 12px; }
}

@media (max-width: 1199px) {
  .battlefield {
    grid-template-areas: none;
    grid-template-columns: repeat(var(--opponent-count), minmax(0, 1fr));
    grid-template-rows: 104px minmax(0, 1fr) minmax(128px, auto);
    gap: 6px;
    padding: 42px 8px 8px;
  }
  .battlefield::before { inset: 8px; }
  .battlefield::after { width: 190px; height: 190px; font-size: 48px; }
  .player-seat { @apply rounded-lg p-1.5; }
  .player-seat:not(.seat-bottom) { grid-row: 1; grid-column: var(--compact-column); @apply h-[104px] flex-col gap-1 overflow-hidden; }
  .seat-bottom { grid-row: 3; grid-column: 1 / -1; @apply min-h-0 flex-row-reverse; }
  .center-stack { grid-row: 2; grid-column: 1 / -1; @apply grid min-h-0 grid-rows-[minmax(88px,1fr)_auto] gap-1; }
  .board-center { @apply min-h-0; }
  .formation-field { @apply min-h-0 py-1; }
  .previous-formation { @apply min-h-12; }
  .formation-cards { @apply min-h-8; }
  .formation-card { width: 28px; }
  .action-dock { max-height: 150px; }
  .player-seat:not(.seat-bottom) .avatar { @apply hidden; }
  .player-seat:not(.seat-bottom) .player-heading { @apply w-full gap-1; }
  .player-seat:not(.seat-bottom) .player-name-block { @apply min-w-0; }
  .player-seat:not(.seat-bottom) .player-name-block strong { @apply max-w-full text-[10px]; }
  .player-seat:not(.seat-bottom) .player-name-block small { @apply text-[9px]; }
  .player-seat:not(.seat-bottom) .player-facts { @apply w-full text-[8px]; }
  .player-seat:not(.seat-bottom) .player-statuses { @apply w-full flex-nowrap overflow-hidden; }
  .player-seat:not(.seat-bottom) .profession-badge, .player-seat:not(.seat-bottom) .counter-badge, .player-seat:not(.seat-bottom) .shield-badge, .player-seat:not(.seat-bottom) .effect-summary { @apply shrink-0 px-1 py-0.5 text-[8px]; }
  .effect-preview { @apply hidden; }
  .player-seat:not(.seat-bottom) .seat-hand { @apply hidden; }
  .player-seat:not(.seat-bottom) .compact-hand-count { @apply block; }
  .player-seat:not(.seat-bottom) .seat-discard-control { @apply absolute right-1 bottom-1; }
  .player-seat:not(.seat-bottom) .seat-discard-control :deep(.discard-pile-label) { @apply hidden; }
  .player-seat:not(.seat-bottom) .seat-discard-control :deep(.discard-pile-trigger) { width: 30px; }
  .seat-bottom .player-identity { @apply max-w-[42%]; }
  .seat-bottom :deep(.playing-card) { width: clamp(48px, 13vw, 64px); }
  .shared-discard-dock { @apply top-1.5 right-2; }
  .shared-discard-dock :deep(.discard-pile-label) { @apply hidden; }
  .shared-discard-dock :deep(.discard-pile-trigger) { width: 34px; }
}

@media (max-width: 600px) {
  .battlefield { grid-template-rows: 96px minmax(0, 1fr) minmax(120px, auto); padding-top: 39px; }
  .player-seat:not(.seat-bottom) { height: 96px; }
  .seat-bottom { @apply gap-1; }
  .seat-bottom .avatar { @apply size-7 text-xs; }
  .seat-bottom .player-identity { @apply max-w-[38%]; }
  .seat-bottom .player-facts { @apply text-[8px]; }
  .seat-bottom .player-statuses { @apply max-h-10 overflow-hidden; }
  .seat-bottom :deep(.playing-card) { width: clamp(44px, 12vw, 56px); }
  .action-dock { max-height: 132px; }
}

@media (prefers-reduced-motion: reduce) {
  .player-seat.acting { transition: none; }
}
</style>
