<template>
  <section
    ref="root"
    class="battlefield"
    :class="{
      'four-player': seats.length === 4,
      'is-finished': isFinished,
      'personal-deck': usesPersonalDeck,
      'detail-open': hasOpenDetail(),
      'decision-pending': decisionPlayer !== null,
    }"
    :style="battlefieldStyle"
    :aria-label="mode === 'replay' ? '重播戰場（唯讀）' : '五行戰鬥牌對戰桌'"
  >
    <header class="battle-stage-strip" aria-label="對局狀態">
      <div
        class="turn-capsule battle-glass"
        :class="{
          'has-active-turn': !isFinished && state.currentPlayer,
          'is-your-turn': !isFinished && mode === 'live' && Boolean(state.currentPlayer) && anchorPlayer === state.currentPlayer,
        }"
      >
        <Transition name="turn-capsule-update" mode="out-in" appear>
          <div :key="turnCapsuleKey" class="turn-capsule-state">
            <div class="turn-capsule-heading">
              <span class="turn-number">第 {{ state.turnNumber }} 回合</span>
              <strong>{{ turnHeadline }}</strong>
              <span v-if="!isFinished" class="turn-phase" :title="phaseHeadline">{{ phaseHeadline }}</span>
            </div>
          </div>
        </Transition>
      </div>
      <span class="environment-chip" aria-live="polite">
        <i aria-hidden="true">{{ environmentGlyph }}</i>{{ environmentLabel(state.environment) }}
      </span>
    </header>

    <div class="team-hp-strip" role="group" aria-label="隊伍生命值">
      <div
        v-for="(entry, index) in teamHpEntries"
        :key="entry.team"
        class="team-hp-card battle-glass"
        :class="`team-hp-${index % 2}`"
      >
        <span class="team-hp-orb" aria-hidden="true" />
        <strong class="team-hp-value">
          <span class="sr-only">{{ teamAriaLabel(entry.team, index) }} </span>
          {{ entry.hp }}<small> / {{ entry.initialHp }}</small>
        </strong>
        <div
          class="team-hp-meter"
          role="meter"
          :aria-label="teamAriaLabel(entry.team, index)"
          :aria-valuemin="0"
          :aria-valuemax="entry.initialHp"
          :aria-valuenow="entry.hp"
        >
          <span :style="{ width: `${entry.ratio}%` }" />
        </div>
      </div>
    </div>

    <div
      v-for="seat in seats"
      :key="seat.player"
      class="player-seat battle-glass"
      :class="[`seat-${seat.position}`, {
        acting: isActing(seat.player),
        'needs-response': decisionPlayer === seat.player,
        'is-expanded': isSeatExpanded(seat.player, seat.position),
      }]"
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
            <small class="team-label">{{ teamLabel(seat.player) }}</small>
          </span>
        </div>

        <span
          v-if="professionFor(seat.player)"
          class="profession-badge"
          tabindex="0"
          :aria-label="`職業 ${professionFor(seat.player)!.name}`"
          :aria-describedby="professionTooltipPlayer === seat.player ? `profession-summary-${seat.player}` : undefined"
          @mouseenter="openProfessionSummary(seat.player, $event.currentTarget as HTMLElement)"
          @mouseleave="closeProfessionSummary"
          @focus="openProfessionSummary(seat.player, $event.currentTarget as HTMLElement)"
          @blur="closeProfessionSummary"
          @keydown.esc.prevent="closeProfessionSummary"
        >職業・{{ professionFor(seat.player)!.name }}</span>
        <span v-else class="profession-badge profession-unknown">尚未轉職</span>

        <div class="player-statuses">
          <span v-if="isActing(seat.player)" class="turn-status">回合中</span>
          <span v-if="decisionPlayer === seat.player" class="response-status">需要回應</span>
          <span v-if="reconnectingPlayer === seat.player" class="reconnecting-label">重新連線中</span>
          <span v-for="counter in counterEffectsFor(seat.player)" :key="`${seat.player}-${counter.effectId}`" class="counter-badge">反制 · {{ counter.effectName }}</span>
          <span v-if="shieldFor(seat.player) > 0" class="shield-badge">防護罩 · {{ shieldFor(seat.player) }}</span>
          <button
            v-if="persistentEffectsFor(seat.player).length"
            class="effect-summary"
            type="button"
            aria-haspopup="dialog"
            :aria-controls="`effect-detail-${seat.player}`"
            :aria-expanded="effectDetailPlayer === seat.player"
            @click.stop="openEffectDetail(seat.player, $event.currentTarget as HTMLButtonElement)"
          >
            效果 {{ persistentEffectsFor(seat.player).length }}
          </button>
        </div>
        <div v-if="isSeatExpanded(seat.player, seat.position)" class="player-expanded-summary">
          <span v-for="fact in playerSummary(seat.player)" :key="fact" :title="fact">{{ fact }}</span>
        </div>
      </div>

      <div
        v-if="seat.position === 'top' && isSeatExpanded(seat.player, seat.position)"
        class="seat-hand-backs"
        role="img"
        :aria-label="`${playerLabel(seat.player)}的手牌 ${handCount(seat.player)} 張`"
      >
        <img v-for="index in handCount(seat.player)" :key="index" :src="CARD_BACK_IMAGE_PATH" alt="" aria-hidden="true" draggable="false">
        <span v-if="!handCount(seat.player)" class="empty-hand">無手牌</span>
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
        v-if="showKnownHandButton(seat.player, seat.position)"
        class="compact-hand-count compact-hand-button"
        type="button"
        aria-haspopup="dialog"
        :aria-controls="`hand-detail-${seat.player}`"
        :aria-expanded="handDetailPlayer === seat.player"
        :aria-label="`查看${playerLabel(seat.player)}的公開手牌，手牌 ${handCount(seat.player)} 張，${visibleHandCards(seat.player).length} 張已知`"
        @click.stop="openHandDetail(seat.player, $event.currentTarget as HTMLButtonElement)"
      >
        <template v-if="isSeatExpanded(seat.player, seat.position)">公開手牌</template>
        <template v-else>手牌 {{ handCount(seat.player) }}・公開 {{ visibleHandCards(seat.player).length }}</template>
      </button>
      <span v-else-if="!isSeatExpanded(seat.player, seat.position)" class="compact-hand-count">手牌 {{ handCount(seat.player) }}</span>
      <button
        class="player-detail-toggle"
        type="button"
        aria-haspopup="dialog"
        :aria-controls="`player-detail-${seat.player}`"
        :aria-expanded="playerDetailPlayer === seat.player"
        :aria-label="`查看${playerLabel(seat.player)}的詳細資訊`"
        @click.stop="openPlayerDetail(seat.player, $event.currentTarget as HTMLButtonElement)"
      >詳情</button>

      <div v-if="seat.position === 'bottom'" class="hand fan seat-hand" :aria-label="`${playerLabel(seat.player)}的手牌`">
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
        <div class="formation-field" :class="{ 'is-terminal': state.status === 'Finished' }">
          <template v-if="state.status === 'Finished'">
            <div class="terminal-resolution">
              <slot name="terminal-resolution" />
            </div>
          </template>
          <template v-else>
            <div class="formation-field-heading">
              <span class="formation-field-label">戰場・陣法區</span>
            </div>
            <div class="previous-formation" :class="{ 'is-empty': !state.previousTurnFormation }">
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
            <button
              v-if="activeCardFlightCount"
              class="card-flight-skip"
              type="button"
              @click="finishCardFlights"
            >略過動畫</button>
          </template>
          </div>
        </div>
      <div v-if="!isFinished" class="action-dock-slot" :class="{ 'controls-hidden': !showTurnControls }">
        <Transition name="action-dock" appear>
          <div v-if="showTurnControls" class="action-dock"><slot name="turn-controls" /></div>
        </Transition>
      </div>
    </div>

    <slot name="overlay" />

    <AnchoredSurface
      v-if="professionTooltipPlayer"
      :open="professionTooltipPlayer !== null"
      :anchor="professionTooltipAnchor"
      :id="`profession-summary-${professionTooltipPlayer}`"
      role="tooltip"
      placement="bottom-start"
      :return-focus="false"
      surface-class="profession-summary"
      @close="closeProfessionSummary"
    >
      <b>{{ professionFor(professionTooltipPlayer)!.name }}</b>
      <small v-for="ability in professionFor(professionTooltipPlayer)!.abilities" :key="ability">{{ ability }}</small>
    </AnchoredSurface>

    <BattlefieldDetailLayer
      :open="discardDetail !== null"
      id="discard-composition"
      :title="discardDetailTitle"
      kind="discard"
      :anchor="discardAnchor"
      :anchor-element="discardTrigger"
      @close="closeDiscardDetail"
      :response-required="pendingResponseLabel"
      @respond="emit('respond-to-choice')"
    >
      <CardChoiceMatrix
        v-if="activeDiscardCards.length"
        readonly
        :cards="activeDiscardCards"
        label="棄牌組成"
        caption="依五行與等級統計棄牌張數"
      />
      <p v-else class="empty-detail">目前沒有棄牌。</p>
    </BattlefieldDetailLayer>

    <BattlefieldDetailLayer
      :open="effectDetailPlayer !== null"
      :id="`effect-detail-${effectDetailPlayer ?? 'player'}`"
      :title="effectDetailTitle"
      kind="effects"
      :anchor="effectAnchor"
      :anchor-element="effectTrigger"
      @close="closeEffectDetail"
      :response-required="pendingResponseLabel"
      @respond="emit('respond-to-choice')"
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
      :anchor-element="handTrigger"
      @close="closeHandDetail"
      :response-required="pendingResponseLabel"
      @respond="emit('respond-to-choice')"
    >
      <div class="hand-detail-cards">
        <GameCard v-for="card in activeVisibleHandCards" :key="card.id" :card="card" :interpretations="state.cardInterpretations" />
      </div>
    </BattlefieldDetailLayer>

    <BattlefieldDetailLayer
      :open="playerDetailPlayer !== null"
      :id="`player-detail-${playerDetailPlayer ?? 'player'}`"
      :title="playerDetailTitle"
      kind="player"
      :anchor="playerDetailAnchor"
      :anchor-element="playerDetailTrigger"
      @close="closePlayerDetail"
      :response-required="pendingResponseLabel"
      @respond="emit('respond-to-choice')"
    >
      <template v-if="playerDetailPlayer">
        <dl class="player-detail-facts">
          <div><dt>隊伍</dt><dd>{{ teamLabel(playerDetailPlayer) }}</dd></div>
          <div><dt>職業</dt><dd>{{ professionFor(playerDetailPlayer)?.name ?? '尚未轉職' }}</dd></div>
          <div v-if="teamStar(teamForPlayer(playerDetailPlayer))"><dt>星辰</dt><dd>{{ starLabel(teamStar(teamForPlayer(playerDetailPlayer))!) }}</dd></div>
          <div v-if="starHistoryLabel(playerDetailPlayer)"><dt>召星</dt><dd>{{ starHistoryLabel(playerDetailPlayer) }}</dd></div>
          <div v-if="spiritFor(playerDetailPlayer)"><dt>精靈</dt><dd>{{ spiritLabel(spiritFor(playerDetailPlayer)!.spirit) }}・靈力 {{ spiritFor(playerDetailPlayer)!.power }} / 6</dd></div>
          <div v-if="totemFor(playerDetailPlayer)"><dt>圖騰</dt><dd><button class="totem-rules-button" type="button" :aria-label="presentTotem(totemFor(playerDetailPlayer)!.totem)" :title="presentTotem(totemFor(playerDetailPlayer)!.totem)">{{ totemFor(playerDetailPlayer)!.name }}・查看規則</button></dd></div>
          <div><dt>錦囊</dt><dd>{{ pouchFor(playerDetailPlayer)?.card?.label ?? (pouchFor(playerDetailPlayer) ? '覆蓋牌' : '無') }}</dd></div>
          <div><dt>手牌</dt><dd>{{ handCount(playerDetailPlayer) }} 張</dd></div>
          <div v-if="coveredPassiveSummary(playerDetailPlayer)"><dt>蓋牌</dt><dd>{{ coveredPassiveSummary(playerDetailPlayer) }}</dd></div>
          <div v-for="card in exposedDeckCards(playerDetailPlayer)" :key="`detail-exposed-${card.id}`"><dt>公開牌</dt><dd>{{ card.label }}</dd></div>
        </dl>
        <section v-if="visibleHandCards(playerDetailPlayer).length" class="player-detail-known-hand" aria-label="已公開手牌">
          <h3 class="player-detail-section-title">已公開手牌</h3>
          <div class="player-detail-known-hand-cards">
            <GameCard v-for="card in visibleHandCards(playerDetailPlayer)" :key="`public-hand-${card.id}`" :card="card" :interpretations="state.cardInterpretations" />
          </div>
        </section>
        <h3 class="player-detail-section-title">目前狀態</h3>
        <ul class="player-detail-statuses">
          <li v-for="counter in counterEffectsFor(playerDetailPlayer)" :key="`detail-${counter.effectId}`">反制・{{ counter.effectName }}</li>
          <li v-if="shieldFor(playerDetailPlayer) > 0">防護罩・{{ shieldFor(playerDetailPlayer) }}</li>
          <li v-for="effect in persistentEffectsFor(playerDetailPlayer)" :key="`player-effect-${effect.key}`">{{ effect.label }}</li>
          <li v-if="!counterEffectsFor(playerDetailPlayer).length && shieldFor(playerDetailPlayer) === 0 && !persistentEffectsFor(playerDetailPlayer).length">目前沒有持續狀態</li>
        </ul>
        <p v-if="professionFor(playerDetailPlayer)?.abilities.length" class="player-profession-summary">
          <strong>職業能力</strong>{{ professionFor(playerDetailPlayer)!.abilities.join('、') }}
        </p>
      </template>
    </BattlefieldDetailLayer>
  </section>
</template>

<script setup lang="ts">
import { computed, watch, nextTick, onBeforeUnmount, ref } from 'vue'
import AnchoredSurface from './AnchoredSurface.vue'
import BattlefieldDetailLayer from './BattlefieldDetailLayer.vue'
import CardChoiceMatrix from './CardChoiceMatrix.vue'
import DiscardPileControl from './DiscardPileControl.vue'
import GameCard from './GameCard.vue'
import { presentTotem } from '~/lib/totem-presentation'
import { presentPendingChoice } from '~/lib/pending-choice-presentation'
import type { CardInstanceId, Element, PlayerId, PublicCard, PublicCardRefs, PublicGameState, SpiritKind, TeamId } from '~/types/fewfc'
import { CARD_BACK_IMAGE_PATH, cardElementGlyph } from '~/lib/card-face-presentation'
import {
  lastCompletedTurnDiscardForPlayer,
  latestSharedTurnDiscard,
  personalPileCounts,
  publicCardRefCount,
  sharedPileCounts,
} from '~/lib/battlefield-presentation'
import { presentPersistentEffects } from '~/lib/persistent-effect-presentation'

type SeatPosition = 'top' | 'bottom'
interface CardToken { id: string, cardId: number, card: PublicCard | null, hidden: boolean, selectable: boolean, selected: boolean }
interface AnchorRect { top: number, right: number, bottom: number, left: number }
interface CardFlightSnapshot { cardId: number, src: string, left: number, top: number, width: number, height: number }

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
  showTurnControls?: boolean
}>(), {
  selectedCardIds: () => [],
  selectablePlayer: null,
  cardsDisabled: false,
  connectedPlayers: () => [],
  reconnectingPlayer: null,
  showTurnControls: false,
})

const emit = defineEmits<{
  'select-card': [player: PlayerId, card: number]
  'respond-to-choice': []
  'detail-state-change': [open: boolean]
}>()
const root = ref<HTMLElement | null>(null)
const activeCardFlightCount = ref(0)
let cardFlightAnimations: Array<{ element: HTMLImageElement, animation: Animation }> = []
let cardFlightCleanupTimer: ReturnType<typeof setTimeout> | undefined
const discardDetail = ref<{ owner: PlayerId | null } | null>(null)
const discardAnchor = ref<AnchorRect | null>(null)
const discardTrigger = ref<HTMLButtonElement | null>(null)
const effectDetailPlayer = ref<PlayerId | null>(null)
const effectAnchor = ref<AnchorRect | null>(null)
const effectTrigger = ref<HTMLButtonElement | null>(null)
const handDetailPlayer = ref<PlayerId | null>(null)
const handAnchor = ref<AnchorRect | null>(null)
const handTrigger = ref<HTMLButtonElement | null>(null)
const playerDetailPlayer = ref<PlayerId | null>(null)
const playerDetailAnchor = ref<AnchorRect | null>(null)
const playerDetailTrigger = ref<HTMLButtonElement | null>(null)
const professionTooltipPlayer = ref<PlayerId | null>(null)
const professionTooltipAnchor = ref<HTMLElement | null>(null)
const HAND_SHORTCUT_KEYS = ['1', '2', '3', '4', '5'] as const

watch(
  [discardDetail, effectDetailPlayer, handDetailPlayer, playerDetailPlayer],
  () => emit('detail-state-change', hasOpenDetail()),
  { flush: 'post' },
)

const seats = computed(() => {
  const order = props.state.turnOrder
  const requestedIndex = props.anchorPlayer ? order.indexOf(props.anchorPlayer) : 0
  const index = requestedIndex >= 0 ? requestedIndex : 0
  const relative = index > 0 ? [...order.slice(index), ...order.slice(0, index)] : order
  const positions: SeatPosition[] = relative.map((_, position) => position === 0 ? 'bottom' : 'top')
  let compactColumn = 0
  return relative.map((player, position) => {
    const seatPosition = positions[position] ?? 'top'
    if (seatPosition !== 'bottom') compactColumn += 1
    return { player, position: seatPosition, compactColumn: seatPosition === 'bottom' ? 1 : compactColumn }
  })
})
const isFinished = computed(() => props.state.status === 'Finished')
const decisionPlayer = computed(() => isFinished.value ? null : props.state.pendingChoice?.player ?? null)
function isSeatExpanded(player: PlayerId, position: SeatPosition) {
  return position === 'bottom' || seats.value.length === 2 || player === props.state.currentPlayer
}
const expandedSeatColumns = computed(() => {
  const topSeats = seats.value.filter(seat => seat.position === 'top')
  if (topSeats.length !== 3) return ''
  return topSeats.map(seat => seat.player === props.state.currentPlayer ? '1.65fr' : '1fr').join(' ')
})
const teamHpEntries = computed(() => props.state.hp.map((entry, index) => {
  const initialHp = props.state.initialHp.find(candidate => candidate.team === entry.team)?.hp ?? 0
  return {
    team: entry.team,
    hp: entry.hp,
    initialHp,
    ratio: initialHp > 0 ? Math.max(0, Math.min(100, (entry.hp / initialHp) * 100)) : 0,
    index,
  }
}))
const battlefieldStyle = computed(() => {
  const environment = props.state.environment
  const background = environment ? environment.toLowerCase() : 'void'
  const accent: Record<string, string> = {
    Metal: '#d7b96c', Wood: '#63bb82', Water: '#6abfe0',
    Fire: '#e88c5d', Earth: '#c5a06d',
  }
  return {
    '--opponent-count': Math.max(1, seats.value.length - 1),
    '--seat-grid-columns': expandedSeatColumns.value || 'repeat(3, minmax(0, 1fr))',
    '--battle-background': `url('/backgrounds/${background}.webp')`,
    '--battle-accent': environment ? accent[environment] : '#b6b9c6',
  }
})
const turnHeadline = computed(() => {
  if (isFinished.value) return '戰局結束'
  if (props.mode === 'replay') return `回放・${playerLabel(props.state.currentPlayer)}行動`
  if (props.state.pendingRandomness) return '伺服器正在洗牌'
  if (decisionPlayer.value) {
    return props.anchorPlayer === decisionPlayer.value
      ? '需要你選擇'
      : `等待 ${playerLabel(decisionPlayer.value)} 選擇`
  }
  if (props.state.status === 'Preparing' && props.state.initialPouchSelection?.remainingPlayers.includes(props.anchorPlayer ?? '')) {
    return '選擇初始錦囊'
  }
  if (!props.state.currentPlayer) return '等待對局開始'
  return props.anchorPlayer === props.state.currentPlayer
    ? '你的回合'
    : `${playerLabel(props.state.currentPlayer)}的回合`
})
const phaseHeadline = computed(() => {
  if (props.state.pendingChoice) return pendingChoiceHeadline.value
  if (props.state.pendingRandomness) return `第 ${props.state.pendingRandomness.cardCount} 張牌正在洗牌`
  if (props.state.status === 'Preparing') return '對局準備'
  return phaseLabel(props.state.phase)
})
const turnCapsuleKey = computed(() => [
  props.state.status,
  props.state.turnNumber,
  props.state.currentPlayer ?? '',
  props.state.phase,
  decisionPlayer.value ?? '',
  props.state.pendingRandomness ? 'randomness' : '',
].join(':'))
const environmentGlyph = computed(() => props.state.environment
  ? { Metal: '金', Wood: '木', Water: '水', Fire: '火', Earth: '土' }[props.state.environment]
  : '無')
const pendingChoiceHeadline = computed(() => {
  const choice = props.state.pendingChoice
  if (!choice) return '需要完成對局選擇'
  return choice.visibility === 'visible'
    ? presentPendingChoice(choice.reason)
    : '正在等待必要選擇'
})
const pendingResponseLabel = computed(() => (
  props.mode === 'live'
    && !isFinished.value
    && props.state.pendingChoice?.visibility === 'visible'
    && props.anchorPlayer === props.state.pendingChoice.player
    ? pendingChoiceHeadline.value
    : null
))
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
const activeEffects = computed(() => effectDetailPlayer.value ? persistentEffectsFor(effectDetailPlayer.value) : [])
const effectDetailTitle = computed(() => effectDetailPlayer.value ? `${playerLabel(effectDetailPlayer.value)}的效果 ${activeEffects.value.length}` : '效果')
const activeVisibleHandCards = computed(() => handDetailPlayer.value ? visibleHandCards(handDetailPlayer.value) : [])
const handDetailTitle = computed(() => {
  const player = handDetailPlayer.value
  if (!player) return '手牌'
  const visible = visibleHandCards(player).length
  const total = handCount(player)
  return visible < total
    ? `${playerLabel(player)}的公開手牌 ${visible} / ${total}`
    : `${playerLabel(player)}的手牌 ${total}`
})
const playerDetailTitle = computed(() => playerDetailPlayer.value ? `${playerLabel(playerDetailPlayer.value)}的對局資訊` : '玩家資訊')

watch(
  [
    () => props.state.pendingChoice,
    () => props.state.pendingRandomness,
    () => activeDiscardCards.value.length,
  ],
  () => {
    if (discardDetail.value && activeDiscardCards.value.length === 0) closeDiscardDetail()
  },
)

function hasOpenDetail() { return discardDetail.value !== null || effectDetailPlayer.value !== null || handDetailPlayer.value !== null || playerDetailPlayer.value !== null }
function closeDetail() {
  if (discardDetail.value) closeDiscardDetail()
  else if (effectDetailPlayer.value) closeEffectDetail()
  else if (handDetailPlayer.value) closeHandDetail()
  else if (playerDetailPlayer.value) closePlayerDetail()
}

function captureCardFlights(cardIds: readonly number[]): CardFlightSnapshot[] {
  if (!root.value || typeof window === 'undefined' || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return []
  const rootRect = root.value.getBoundingClientRect()
  return [...new Set(cardIds)].flatMap((cardId) => {
    const card = root.value?.querySelector<HTMLElement>(`.seat-bottom .seat-hand [data-card-id="${cardId}"]`)
    const image = card?.querySelector<HTMLImageElement>('.game-card-image')
    if (!card || !image) return []
    const rect = card.getBoundingClientRect()
    return [{
      cardId,
      src: image.currentSrc || image.src,
      left: rect.left - rootRect.left,
      top: rect.top - rootRect.top,
      width: rect.width,
      height: rect.height,
    }]
  })
}

function finishCardFlights() {
  clearTimeout(cardFlightCleanupTimer)
  cardFlightCleanupTimer = undefined
  const animations = cardFlightAnimations
  cardFlightAnimations = []
  for (const flight of animations) {
    flight.animation.onfinish = null
    flight.animation.cancel()
    flight.element.remove()
  }
  activeCardFlightCount.value = 0
}

function animateCardFlights(snapshots: readonly CardFlightSnapshot[]) {
  finishCardFlights()
  const board = root.value
  const field = board?.querySelector<HTMLElement>('.formation-field')
  if (!board || !field || !snapshots.length || typeof window === 'undefined'
    || window.matchMedia('(prefers-reduced-motion: reduce)').matches) return

  const boardRect = board.getBoundingClientRect()
  const fieldRect = field.getBoundingClientRect()
  const targetWidth = Math.min(38, fieldRect.width * 0.16)
  const targetHeight = targetWidth * (949 / 627)
  const targetLeft = fieldRect.left - boardRect.left + fieldRect.width / 2 - targetWidth / 2
  const targetTop = fieldRect.top - boardRect.top + fieldRect.height / 2 - targetHeight / 2

  snapshots.forEach((snapshot, index) => {
    const image = document.createElement('img')
    image.className = 'card-flight-ghost'
    image.src = snapshot.src
    image.alt = ''
    image.setAttribute('aria-hidden', 'true')
    image.draggable = false
    image.style.left = `${snapshot.left}px`
    image.style.top = `${snapshot.top}px`
    image.style.width = `${snapshot.width}px`
    image.style.height = `${snapshot.height}px`
    board.appendChild(image)

    const dx = targetLeft + Math.min(index, 3) * 5 - snapshot.left
    const dy = targetTop - Math.min(index, 3) * 4 - snapshot.top
    const scale = Math.min(targetWidth / snapshot.width, targetHeight / snapshot.height)
    const animation = image.animate([
      { transform: 'translate3d(0, 0, 0) scale(1)', opacity: 1 },
      { transform: `translate3d(${dx * 0.68}px, ${dy * 0.68}px, 0) scale(${1 - (1 - scale) * 0.68})`, opacity: 0.94, offset: 0.72 },
      { transform: `translate3d(${dx}px, ${dy}px, 0) scale(${scale})`, opacity: 0.08 },
    ], {
      duration: 380,
      delay: Math.min(index * 28, 56),
      easing: 'cubic-bezier(.22,.78,.2,1)',
      fill: 'forwards',
    })
    cardFlightAnimations.push({ element: image, animation })
    animation.onfinish = () => {
      image.remove()
      cardFlightAnimations = cardFlightAnimations.filter(flight => flight.element !== image)
      activeCardFlightCount.value = cardFlightAnimations.length
      if (!cardFlightAnimations.length) {
        clearTimeout(cardFlightCleanupTimer)
        cardFlightCleanupTimer = undefined
      }
    }
  })

  activeCardFlightCount.value = cardFlightAnimations.length
  cardFlightCleanupTimer = setTimeout(finishCardFlights, 520)
}

defineExpose({ root, hasOpenDetail, closeDetail, captureCardFlights, animateCardFlights, finishCardFlights })

onBeforeUnmount(finishCardFlights)

function triggerRect(trigger: HTMLButtonElement): AnchorRect {
  const rect = trigger.getBoundingClientRect()
  return { top: rect.top, right: rect.right, bottom: rect.bottom, left: rect.left }
}
function closeOtherDetails(except: 'discard' | 'effects' | 'hand' | 'player') {
  if (except !== 'discard') discardDetail.value = null
  if (except !== 'effects') effectDetailPlayer.value = null
  if (except !== 'hand') handDetailPlayer.value = null
  if (except !== 'player') playerDetailPlayer.value = null
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
  const trigger = discardTrigger.value
  discardDetail.value = null
  restoreDetailFocus(trigger)
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
  const trigger = effectTrigger.value
  effectDetailPlayer.value = null
  restoreDetailFocus(trigger)
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
  const trigger = handTrigger.value
  handDetailPlayer.value = null
  restoreDetailFocus(trigger)
}
function openPlayerDetail(player: PlayerId, trigger: HTMLButtonElement) {
  if (playerDetailPlayer.value === player) return closePlayerDetail()
  closeOtherDetails('player')
  playerDetailTrigger.value = trigger
  playerDetailAnchor.value = triggerRect(trigger)
  playerDetailPlayer.value = player
}
function closePlayerDetail() {
  if (!playerDetailPlayer.value) return
  const trigger = playerDetailTrigger.value
  playerDetailPlayer.value = null
  restoreDetailFocus(trigger)
}
function restoreDetailFocus(trigger: HTMLButtonElement | null) {
  if (!trigger) return
  const restore = () => {
    const active = document.activeElement
    const shouldRestore = !active
      || active === document.body
      || active === trigger
      || active.classList.contains('battlefield-detail-backdrop')
      || Boolean(active.closest('.battlefield-detail-panel'))
    if (shouldRestore) trigger.focus()
  }
  void nextTick(() => {
    restore()
    requestAnimationFrame(restore)
  })
}
function openProfessionSummary(player: PlayerId, trigger: HTMLElement) {
  professionTooltipPlayer.value = player
  professionTooltipAnchor.value = trigger
}
function closeProfessionSummary() {
  professionTooltipPlayer.value = null
  professionTooltipAnchor.value = null
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
function showKnownHandButton(player: PlayerId, position: SeatPosition) { return position !== 'bottom' && visibleHandCards(player).length > 0 }
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
function teamLabel(player: PlayerId) {
  const team = teamForPlayer(player)
  const anchorTeam = props.anchorPlayer ? teamForPlayer(props.anchorPlayer) : ''
  if (anchorTeam) return team === anchorTeam ? '我方' : '對方'
  const index = props.state.hp.findIndex(entry => entry.team === team)
  return index >= 0 ? `隊伍 ${index + 1}` : '隊伍'
}
function teamAriaLabel(team: TeamId, index: number) {
  const anchorTeam = props.anchorPlayer ? teamForPlayer(props.anchorPlayer) : ''
  const relation = anchorTeam
    ? team === anchorTeam ? '我方' : '對方'
    : `第 ${index + 1} 隊`
  return `${relation}隊伍 HP`
}
function handCount(player: PlayerId) { const cards = props.state.hands.find(entry => entry.player === player)?.cards; return !cards ? 0 : cards.kind === 'hidden' ? cards.count : cards.cards.length }
function teamStar(team: TeamId) { return props.state.teamStars.find(entry => entry.team === team)?.star }
function starHistoryLabel(player: PlayerId) { return (props.state.starHistories.find(entry => entry.player === player)?.stars ?? []).map(cardElementGlyph).join('、') }
function totemFor(player: PlayerId) { return props.state.totems?.find(entry => entry.player === player) }
function spiritFor(player: PlayerId) { return props.state.spirits.find(entry => entry.player === player) }
function spiritLabel(spirit: SpiritKind) { return { Metal: '金精靈', Wood: '木精靈', Water: '水精靈', Fire: '火精靈', Earth: '土精靈', Evil: '惡精靈', Death: '死精靈' }[spirit] }
function pouchFor(player: PlayerId) { return props.state.pouches.find(pouch => pouch.owner === player) }
function playerSummary(player: PlayerId) {
  const star = teamStar(teamForPlayer(player))
  const spirit = spiritFor(player)
  const history = starHistoryLabel(player)
  const pouch = pouchFor(player)
  return [
    star ? `星辰・${starLabel(star)}` : null,
    spirit ? `${spiritLabel(spirit.spirit)}・靈力 ${spirit.power} / 6` : null,
    history ? `召星・${history}` : null,
    `錦囊・${pouch?.card?.label ?? (pouch ? '覆蓋牌' : '無')}`,
  ].filter((fact): fact is string => Boolean(fact))
}
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
function persistentEffectsFor(player: PlayerId) { return presentPersistentEffects(props.state, player, teamForPlayer(player)) }
function counterEffectsFor(player: PlayerId) { return props.state.counterEffects.filter(entry => entry.owner === player) }
function shieldFor(player: PlayerId) { return props.state.shields.find(entry => entry.player === player)?.value ?? 0 }
function exposedDeckCards(player: PlayerId) { const cards = props.state.playerDecks.find(entry => entry.player === player)?.cards; return cards?.kind === 'partiallyKnown' ? cards.cards.filter((card): card is PublicCard => card !== null) : [] }
function playerConnected(player: PlayerId) { return props.connectedPlayers.includes(player) }
function shortcutLabel(index: number) { return HAND_SHORTCUT_KEYS[index]?.toUpperCase() }
function starLabel(star: NonNullable<ReturnType<typeof teamStar>>) { return { Metal: '金星‧太白', Wood: '木星‧歲星', Water: '水星‧辰星', Fire: '火星‧熒惑', Earth: '土星‧鎮星' }[star] }
function environmentLabel(value: Element | null) { return value ? { Metal: '金行', Wood: '木行', Water: '水行', Fire: '火行', Earth: '土行' }[value] : '無環境' }
function isActing(player: PlayerId) { return props.state.status !== 'Finished' && props.state.currentPlayer === player }
function seatAriaLabel(player: PlayerId) {
  const current = isActing(player) ? `，回合玩家，${props.mode === 'live' ? phaseLabel(props.state.phase) : ''}` : ''
  const decision = decisionPlayer.value === player ? '，目前決策者' : ''
  return `${playerLabel(player)}，${teamLabel(player)}，職業 ${professionFor(player)?.name ?? '尚未轉職'}，手牌 ${handCount(player)} 張${current}${decision}`
}
function phaseLabel(value: string) { return { TurnStart: '回合開始', ActiveEffects: '能力階段', Action: '行動階段', TurnDraw: '抽牌階段', TurnEnd: '回合結束' }[value] ?? value }
</script>

<style scoped>
@reference "../assets/css/main.css";

.battlefield {
  --card-back-base: #232e28;
  --card-back-stripe: #344039;
  --card-back-border: #85714a;
  --stage-text-soft: var(--battle-text-secondary);
  @apply relative grid size-full min-h-0 min-w-0 overflow-hidden;
  grid-template-columns: var(--seat-grid-columns, repeat(3, minmax(0, 1fr)));
  grid-template-rows: 52px 36px clamp(92px, 12dvh, 116px) minmax(200px, 1fr) clamp(180px, 26dvh, 232px);
  gap: 12px;
  padding: 14px 18px 18px;
  color: var(--app-text);
  transition: grid-template-columns 220ms ease;
  isolation: isolate;
}
.battlefield::before { content: ''; @apply pointer-events-none absolute inset-0 -z-1; background: radial-gradient(ellipse at 50% 44%, rgba(111, 171, 139, .08), transparent 62%), radial-gradient(ellipse at center, transparent 24%, rgba(3, 8, 7, .12) 100%); }
.battlefield.detail-open { z-index: 25; }
.battle-stage-strip { @apply relative z-1 col-span-3 flex min-w-0 items-center justify-center; grid-row: 1; height: 52px; min-height: 52px; }
.turn-capsule {
  @apply relative isolate grid min-w-0 justify-items-center text-center;
  max-width: calc(100% - 100px);
  padding: 10px 22px;
  border-radius: 999px;
}
.turn-capsule.is-your-turn {
  border-color: rgba(255, 229, 157, .98);
  box-shadow: inset 0 1px rgba(255,255,255,.34), inset 0 -1px rgba(255,238,190,.16), 0 0 0 1px rgba(255, 208, 91, .42), 0 0 22px rgba(255, 190, 63, .48), 0 0 44px rgba(255, 190, 63, .2), 0 16px 34px rgba(0,0,0,.2);
}
.turn-capsule.is-your-turn strong { color: var(--app-accent-strong); font-size: 24px; font-weight: 800; letter-spacing: .04em; text-shadow: 0 0 12px rgba(255, 205, 91, .56); }
.turn-capsule-state { position: relative; z-index: 1; min-width: 0; }
.turn-capsule-update-enter-active, .turn-capsule-update-leave-active { transition: opacity .12s ease, transform .12s ease; }
.turn-capsule-update-enter-from, .turn-capsule-update-leave-to { opacity: 0; transform: translateY(3px) scale(.97); }
.turn-capsule-update-enter-active { transition-duration: .16s; }
.turn-capsule-update-leave-active { transition-duration: .1s; }
.turn-capsule-heading { display: grid; min-width: 0; grid-template-columns: auto minmax(0, auto) minmax(0, auto); align-items: center; justify-content: center; gap: 12px; white-space: nowrap; }
.turn-number { color: var(--app-text); font-family: var(--font-serif); font-size: 22px; font-weight: 700; white-space: nowrap; }
.turn-capsule strong { min-width: 0; overflow: hidden; color: var(--app-text); font-family: var(--font-serif); font-size: 24px; line-height: 1.25; text-overflow: ellipsis; }
.turn-phase { min-width: 0; overflow: hidden; text-overflow: ellipsis; color: var(--stage-text-soft); font-size: 12px; white-space: nowrap; }
.environment-chip { @apply absolute right-0 inline-flex items-center gap-2 rounded-full px-3 py-1.5 text-[10px] font-semibold; border: 1px solid color-mix(in srgb, var(--battle-accent) 46%, var(--glass-stroke)); background: var(--glass-surface); color: var(--app-text); -webkit-backdrop-filter: blur(14px); backdrop-filter: blur(14px); }
.environment-chip i { @apply grid size-5 place-items-center rounded-full not-italic; background: color-mix(in srgb, var(--battle-accent) 22%, transparent); color: var(--battle-accent); }
.team-hp-strip { @apply relative z-1 col-span-3 flex min-w-0 items-center justify-center gap-3; grid-row: 2; }
.team-hp-card { @apply grid min-w-0 flex-1 items-center gap-2 rounded-xl px-3 py-1.5; grid-template-columns: 9px auto minmax(0, 1fr); max-width: 420px; }
.team-hp-orb { width: 8px; height: 8px; border-radius: 50%; background: #d68174; box-shadow: 0 0 10px rgba(214,129,116,.6); }
.team-hp-1 .team-hp-orb { background: #78bdad; box-shadow: 0 0 10px rgba(120,189,173,.6); }
.team-hp-value { @apply whitespace-nowrap; color: var(--app-text); font-size: 15px; font-variant-numeric: tabular-nums; }
.team-hp-value small { color: var(--stage-text-soft); font-size: 12px; }
.team-hp-meter { height: 8px; overflow: hidden; border-radius: 999px; background: color-mix(in srgb, var(--app-text) 13%, transparent); }
.team-hp-meter span { display: block; height: 100%; border-radius: inherit; background: linear-gradient(90deg, #b65f56, #e69b86); transition: width .24s ease; }
.team-hp-1 .team-hp-meter span { background: linear-gradient(90deg, #478a83, #8dcbbc); }
.player-seat { @apply relative z-1 grid min-h-0 min-w-0 items-center gap-x-2; --player-identity-width: clamp(100px, 10vw, 148px); border-radius: 24px; padding: 12px 16px; transition: border-color .2s ease, box-shadow .2s ease; }
.seat-top { grid-row: 3; grid-column: var(--compact-column); grid-template-columns: minmax(0, 1fr) auto; grid-template-rows: minmax(0, 1fr) auto; }
.seat-bottom { grid-row: 5; grid-column: 1 / -1; grid-template-columns: var(--player-identity-width) minmax(0, 1fr) auto; grid-template-rows: minmax(0, 1fr); gap: 6px; }
.battlefield:not(.four-player) .seat-top { grid-column: 1 / -1; }
.player-seat.acting { border-color: color-mix(in srgb, var(--battle-accent) 82%, white); box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--battle-accent) 56%, transparent), 0 0 20px color-mix(in srgb, var(--battle-accent) 20%, transparent), 0 10px 24px rgba(0,0,0,.14); }
.player-seat.needs-response { border-color: #69c3b0; box-shadow: inset 0 0 0 1px rgba(105,195,176,.42), 0 0 20px rgba(105,195,176,.21); }
.player-seat.is-expanded { z-index: 4; }
.player-identity { @apply flex min-w-0 max-w-full flex-wrap items-center gap-x-2 gap-y-1; grid-column: 1 / -1; grid-row: 1; }
.player-heading { @apply flex min-w-0 items-center gap-2; }
.player-name-block { @apply grid min-w-0; }
.player-name-block strong { @apply max-w-48 truncate; font-size: 16px; color: var(--app-text); }
.team-label { color: var(--stage-text-soft); font-size: 11px; }
.avatar { @apply grid size-[38px] shrink-0 place-items-center rounded-full font-extrabold; background: color-mix(in srgb, var(--battle-accent) 74%, #f1e8cf); color: #171d18; box-shadow: inset 0 1px rgba(255,255,255,.65); }
.connection-dot { @apply size-2 shrink-0 rounded-full border border-[#9d554b] bg-[#a85349]; }
.connection-dot.connected { @apply border-[#477557] bg-[#63a979]; }
.player-statuses { @apply flex min-w-0 max-w-full flex-wrap gap-1; }
.reconnecting-label, .counter-badge, .shield-badge, .profession-badge, .effect-summary, .turn-status, .response-status { @apply max-w-full truncate rounded-md border px-[8px] py-[4px] text-[11px] whitespace-nowrap; }
.reconnecting-label { border-color: rgba(203,166,86,.4); background: rgba(86,67,26,.28); color: #f0ce7e; }
.counter-badge { border-color: rgba(203,166,86,.4); background: rgba(86,67,26,.22); color: #e8cb86; }
.shield-badge { border-color: rgba(117,185,202,.4); background: rgba(35,87,102,.24); color: #9bd9e8; }
.effect-summary { border-color: rgba(191,124,126,.42); background: rgba(98,51,57,.22); color: #efb2b6; }
.profession-badge { @apply relative outline-none; border-color: color-mix(in srgb, var(--battle-accent) 34%, transparent); background: color-mix(in srgb, var(--battle-accent) 12%, transparent); color: color-mix(in srgb, var(--app-text) 92%, var(--battle-accent)); }
.profession-unknown { border-color: var(--glass-stroke); background: transparent; color: var(--stage-text-soft); }
.turn-status { border-color: rgba(201,163,79,.48); background: rgba(119,89,30,.3); color: #f0d18a; }
.response-status { border-color: rgba(94,183,161,.52); background: rgba(28,103,87,.3); color: #9ce0cf; }
.player-detail-toggle, .compact-hand-button { @apply inline-flex min-h-8 items-center justify-center rounded-lg px-3 text-xs whitespace-nowrap; border: 1px solid var(--glass-stroke); background: color-mix(in srgb, var(--glass-surface-strong) 78%, transparent); color: var(--app-text); }
.player-detail-toggle:hover, .compact-hand-button:hover { border-color: var(--battle-accent); color: var(--battle-accent); }
.player-detail-toggle:focus-visible, .compact-hand-button:focus-visible { outline: 2px solid var(--battle-accent); outline-offset: 2px; }
.player-expanded-summary { display: flex; flex-basis: 100%; flex-wrap: wrap; gap: 2px 10px; max-height: 32px; overflow: hidden; color: var(--stage-text-soft); font-size: 11px; line-height: 15px; }
.player-expanded-summary span { min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
.seat-top.is-expanded { grid-template-columns: var(--player-identity-width) minmax(0, 1fr) auto; column-gap: 6px; }
.seat-top.is-expanded .player-identity { grid-column: 1; grid-row: 1 / 3; max-height: 100%; overflow: hidden; }
.seat-top.is-expanded .compact-hand-button { grid-column: 3; grid-row: 2; }
.seat-top.is-expanded .player-detail-toggle { grid-column: 3; grid-row: 1; align-self: start; }
.seat-hand-backs { grid-column: 2; grid-row: 1 / 3; display: flex; align-items: center; width: max-content; max-width: 100%; height: 44px; min-width: 0; overflow: auto hidden; padding: 2px 6px; scrollbar-width: thin; scrollbar-color: var(--glass-stroke) transparent; }
.seat-hand-backs img { flex: 0 0 26px; width: 26px; height: 39px; object-fit: cover; border: 1px solid var(--glass-stroke); border-radius: 3px; }
.seat-hand-backs img + img { margin-left: -10px; }
.empty-hand { color: var(--stage-text-soft); font-size: 11px; }
.seat-top .compact-hand-count { @apply block self-center; grid-row: 2; grid-column: 1; color: var(--stage-text-soft); font-size: 12px; }
.seat-top .compact-hand-button { justify-self: start; }
.seat-top .player-detail-toggle { grid-row: 2; grid-column: 2; }
.seat-bottom .player-identity { @apply content-center; grid-column: 1; grid-row: 1; }
.player-seat.is-expanded .player-heading { max-width: 100%; }
.player-seat.is-expanded .player-name-block strong { max-width: 100%; }
.seat-bottom .player-detail-toggle { grid-column: 3; grid-row: 1; align-self: start; }
.seat-bottom .compact-hand-count { @apply hidden; }
.seat-bottom .player-statuses { @apply mt-1; }
.seat-discard-control { @apply shrink-0; }
.personal-deck .seat-top { grid-template-columns: minmax(0, 1fr) auto auto; }
.personal-deck .seat-top .player-identity { grid-column: 1 / 3; }
.personal-deck .seat-top.is-expanded { grid-template-columns: var(--player-identity-width) minmax(0, 1fr) auto auto; }
.personal-deck .seat-top.is-expanded .player-identity { grid-column: 1; }
.personal-deck .seat-top.is-expanded .seat-discard-control { grid-column: 3; }
.personal-deck .seat-top.is-expanded :is(.player-detail-toggle, .compact-hand-button) { grid-column: 4; }
.seat-top .seat-discard-control { grid-column: 3; grid-row: 1 / 3; }
.seat-top .seat-discard-control :deep(.discard-pile-trigger) { width: 32px; }
.personal-deck .seat-bottom { grid-template-columns: var(--player-identity-width) minmax(0, 1fr) auto auto; }
.personal-deck .seat-bottom .player-detail-toggle { grid-column: 4; }
.seat-bottom .seat-discard-control { grid-column: 3; grid-row: 1; }
.hand { @apply flex min-w-0 items-center; justify-content: flex-start; }
.seat-hand { @apply relative z-2 min-h-0; grid-column: 2; grid-row: 1; gap: 8px; overflow-x: auto; overflow-y: hidden; padding: 22px 6px 12px; scrollbar-width: thin; scrollbar-color: var(--glass-stroke) transparent; }
/* 手牌靠左並依可用寬度縮小，保留選牌上浮與外框所需的內距。 */
.seat-bottom .seat-hand :deep(.playing-card) { width: clamp(80px, min(8vw, 16dvh), 124px); min-width: 0; flex-shrink: 1; }
.seat-top .seat-hand { @apply hidden; }
.seat-hand :deep(.playing-card:hover) { transform: translateY(-8px); }
.seat-hand :deep(.playing-card.selected) { transform: translateY(-14px) scale(1.04); border-color: var(--battle-accent); box-shadow: 0 0 0 2px #f0d894, 0 0 18px rgba(255, 219, 133, .5), 0 15px 26px rgba(0,0,0,.32); }
.seat-hand :deep(.game-card-selected) { display: none; }
:global(.anchored-surface.profession-summary) {
  @apply grid w-64 gap-1 p-2.5 text-left;
  border: 1px solid var(--glass-stroke);
  border-radius: 14px;
  background: linear-gradient(135deg, rgba(255,255,255,.2), transparent 36%), linear-gradient(165deg, var(--glass-light), var(--glass-surface-strong));
  box-shadow: inset 0 1px rgba(255,255,255,.38), 0 12px 28px rgba(0,0,0,.4);
  -webkit-backdrop-filter: saturate(1.28) blur(20px);
  backdrop-filter: saturate(1.28) blur(20px);
}
:global(.anchored-surface.profession-summary b) { @apply font-serif text-xs text-gold-light; }
:global(.anchored-surface.profession-summary small) { @apply whitespace-normal text-[10px] leading-4 text-muted; }
.shared-discard-dock { @apply relative z-8 m-2; grid-row: 4; grid-column: 1; align-self: start; justify-self: start; }
.center-stack { @apply relative z-2 grid min-h-0 min-w-0 gap-2; grid-row: 4; grid-column: 1 / -1; grid-template-columns: minmax(0, 1fr) minmax(300px, .96fr); grid-template-rows: minmax(0, 1fr); }
.board-center { @apply relative grid size-full min-h-0 min-w-0 place-items-center; }
.action-dock-slot { @apply relative min-h-0 min-w-0 overflow-hidden; }
.action-dock-slot.controls-hidden { pointer-events: none; }
.formation-field { @apply relative isolate grid size-full min-h-0 min-w-0 text-center; grid-template-rows: auto auto; align-content: end; padding: 10px 12px 16px; color: #f5f3e7; background: radial-gradient(ellipse at 50% 96%, rgba(9,16,13,.32), transparent 62%); }
.formation-field-heading { @apply relative z-1 flex flex-wrap items-center justify-center gap-2 px-2; }
.formation-field-label { color: #f5e6b8; font-size: 14px; font-weight: 700; letter-spacing: .12em; text-shadow: 0 2px 8px color-mix(in srgb, var(--app-canvas) 62%, transparent); }
.previous-formation { @apply relative z-1 grid min-h-0 content-end justify-items-center gap-2; grid-template-rows: auto auto auto; text-shadow: 0 2px 8px rgba(0,0,0,.72); }
.previous-formation.is-empty { @apply content-center; grid-template-rows: auto; }
.previous-formation small { color: #eeeee3; font-size: 13px; }
.previous-formation strong { @apply font-serif text-lg; color: #f5e6b8; }
.previous-formation p { color: #eeeee3; font-size: 13px; }
.formation-cards { @apply flex min-h-16 items-end justify-center gap-2 pb-2; }
.formation-cards :deep(.formation-card) { width: clamp(64px, min(8vw, 16dvh), 124px); }
.terminal-resolution { @apply grid size-full min-h-20 content-center; }
.action-dock { @apply relative z-5 size-full min-h-0 min-w-0 overflow-y-auto; overscroll-behavior: contain; transform-origin: top center; }
.action-dock-enter-active, .action-dock-leave-active { transition: opacity .2s ease, transform .24s cubic-bezier(.2,.75,.25,1); }
.action-dock-enter-from, .action-dock-leave-to { opacity: 0; transform: translateY(10px) scale(.985); }
:global(.card-flight-ghost) { position: absolute; z-index: 30; display: block; max-width: none; border: 1px solid rgba(244, 211, 130, .82); border-radius: 7px; box-shadow: 0 0 0 2px rgba(233, 185, 87, .24), 0 12px 24px rgba(0,0,0,.38); object-fit: cover; pointer-events: none; will-change: transform, opacity; }
.card-flight-skip { position: absolute; z-index: 4; right: 8px; bottom: 8px; min-height: 28px; padding: 4px 9px; border: 1px solid rgba(244, 211, 130, .6); border-radius: 999px; background: rgba(16, 35, 29, .88); color: #f6dfaa; font-size: 9px; box-shadow: 0 4px 14px rgba(0,0,0,.28); }
.card-flight-skip:hover, .card-flight-skip:focus-visible { border-color: #ffe3a1; background: rgba(119, 89, 30, .62); outline: 2px solid rgba(244, 211, 130, .78); outline-offset: 2px; }
.effect-detail-list { @apply grid list-none gap-2 p-0; }
.effect-detail-list li { @apply border-l-2 border-[#8f5f63] bg-[rgba(118,85,87,.12)] px-3 py-2 text-xs leading-5; }
.hand-detail-cards { @apply flex flex-wrap justify-center gap-2; }
.hand-detail-cards :deep(.playing-card) { width: 78px; }
.empty-detail { @apply text-sm text-muted; }
.player-detail-facts { @apply grid grid-cols-2 gap-x-5 gap-y-3; margin: 0; }
.player-detail-facts > div { @apply grid min-w-0 gap-1; }
.player-detail-facts dt { color: var(--app-text-muted); font-size: 10px; }
.player-detail-facts dd { @apply m-0 break-words; color: var(--app-text); font-size: 12px; line-height: 1.55; }
.totem-rules-button { @apply border-0 bg-transparent p-0 text-left text-[var(--app-text)] underline decoration-dotted underline-offset-2 hover:text-gold-light focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-[var(--app-accent)]; }
.player-detail-known-hand { @apply mt-4; }
.player-detail-known-hand-cards { @apply flex flex-wrap gap-2; }
.player-detail-known-hand-cards :deep(.playing-card) { width: 56px; }
.player-detail-section-title { @apply mt-5 mb-2 font-serif text-xs; color: var(--app-text); }
.player-detail-statuses { @apply grid list-none gap-2 p-0; }
.player-detail-statuses li { @apply rounded-lg px-3 py-2 text-xs leading-5; border: 1px solid var(--glass-stroke); background: var(--glass-surface); }
.player-profession-summary { @apply mt-4 grid gap-1 text-xs leading-5; color: var(--app-text-muted); }
.player-profession-summary strong { color: var(--app-accent-strong); }
.detail-response-callout { @apply sticky top-0 z-2 mb-4 grid gap-3 rounded-xl p-3; border: 1px solid rgba(101,190,169,.55); background: linear-gradient(135deg, rgba(32,109,93,.22), rgba(15,46,41,.87)); box-shadow: 0 8px 20px rgba(0,0,0,.2); }
.detail-response-callout > div { @apply grid gap-1; }
.detail-response-callout strong { color: #9ce0cf; font-size: 12px; }
.detail-response-callout span { color: var(--app-text); font-size: 10px; line-height: 1.5; }
.detail-response-callout button { @apply inline-flex min-h-9 items-center justify-center gap-2 rounded-lg px-3 text-xs font-bold; border: 1px solid #73c7b2; background: rgba(62,150,129,.28); color: #c0f1e4; }
.detail-response-callout button:focus-visible { outline: 2px solid #9ce0cf; outline-offset: 2px; }

@media (prefers-reduced-motion: reduce) {
  .battlefield, .player-seat, .team-hp-meter span { transition: none; }
  .seat-hand :deep(.playing-card:hover), .seat-hand :deep(.playing-card.selected) { transform: none; }
}

@media (prefers-reduced-motion: no-preference) {
  .turn-capsule.is-your-turn { animation: your-turn-border-flow 2.2s linear infinite; }
}

@keyframes your-turn-border-flow {
  0%, 100% {
    border-color: rgba(255, 248, 218, 1) rgba(255, 207, 91, .72) rgba(255, 238, 187, .48) rgba(255, 190, 63, .56);
    box-shadow: inset 0 1px rgba(255,255,255,.34), inset 0 -1px rgba(255,238,190,.16), 0 0 0 1px rgba(255, 208, 91, .5), 0 -3px 18px rgba(255, 198, 74, .56), 0 0 34px rgba(255, 190, 63, .28), 0 16px 34px rgba(0,0,0,.2);
  }
  25% {
    border-color: rgba(255, 207, 91, .72) rgba(255, 248, 218, 1) rgba(255, 207, 91, .72) rgba(255, 238, 187, .48);
    box-shadow: inset 0 1px rgba(255,255,255,.34), inset 0 -1px rgba(255,238,190,.16), 0 0 0 1px rgba(255, 208, 91, .5), 3px 0 18px rgba(255, 198, 74, .56), 0 0 34px rgba(255, 190, 63, .28), 0 16px 34px rgba(0,0,0,.2);
  }
  50% {
    border-color: rgba(255, 238, 187, .48) rgba(255, 207, 91, .72) rgba(255, 248, 218, 1) rgba(255, 207, 91, .72);
    box-shadow: inset 0 1px rgba(255,255,255,.34), inset 0 -1px rgba(255,238,190,.16), 0 0 0 1px rgba(255, 208, 91, .5), 0 3px 18px rgba(255, 198, 74, .56), 0 0 34px rgba(255, 190, 63, .28), 0 16px 34px rgba(0,0,0,.2);
  }
  75% {
    border-color: rgba(255, 207, 91, .72) rgba(255, 238, 187, .48) rgba(255, 207, 91, .72) rgba(255, 248, 218, 1);
    box-shadow: inset 0 1px rgba(255,255,255,.34), inset 0 -1px rgba(255,238,190,.16), 0 0 0 1px rgba(255, 208, 91, .5), -3px 0 18px rgba(255, 198, 74, .56), 0 0 34px rgba(255, 190, 63, .28), 0 16px 34px rgba(0,0,0,.2);
  }
}

/* 桌機雙人對局共用左右欄線：左側放對手與陣法，右側行動區跨越兩列。 */
@media (min-width: 901px) {
  .battlefield:not(.four-player) { grid-template-columns: minmax(0, 1fr) minmax(300px, .96fr); column-gap: 8px; }
  .battlefield:not(.four-player) :is(.battle-stage-strip, .team-hp-strip) { grid-column: 1 / -1; }
  .battlefield:not(.four-player) .seat-top { grid-column: 1; }
  .battlefield:not(.four-player) .center-stack { grid-row: 3 / 5; grid-template-columns: subgrid; grid-template-rows: subgrid; gap: inherit; }
  .battlefield:not(.four-player) .board-center { grid-column: 1; grid-row: 2; }
  .battlefield:not(.four-player) .action-dock-slot { grid-column: 2; grid-row: 1 / 3; }
}

@media (max-width: 900px) {
  .battlefield { padding: 12px 12px 14px; gap: 7px; }
  .center-stack { grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(112px, .72fr) minmax(156px, 1fr); }
  .board-center { grid-column: 1; grid-row: 1; }
  .action-dock-slot { grid-column: 1; grid-row: 2; }
  .action-dock { max-height: 100%; }
}

@media (min-width: 601px) {
  .seat-top.is-expanded .player-identity { gap: 2px 6px; }
  .seat-top.is-expanded .avatar { width: 28px; height: 28px; }
  .seat-top.is-expanded .player-name-block { line-height: 1.2; }
  .seat-top.is-expanded .player-name-block strong { font-size: 14px; }
  .seat-top.is-expanded :is(.profession-badge, .turn-status, .response-status) { padding: 2px 6px; font-size: 10px; }
  .seat-top.is-expanded .player-expanded-summary { font-size: 10px; line-height: 12px; max-height: 24px; }
}

@media (max-width: 600px) {
  .battlefield { grid-template-rows: 44px 32px minmax(80px, .26fr) minmax(196px, 1fr) clamp(128px, 19dvh, 160px); gap: 6px; padding: 8px 8px 10px; }
  .battle-stage-strip { height: 44px; min-height: 44px; justify-content: center; }
  .turn-capsule { max-width: calc(100% - 32px); min-width: 0; gap: 0; padding: 7px 12px; }
  .turn-capsule-heading { gap: 0 6px; }
  .turn-capsule strong { font-size: 12px; }
  .turn-capsule.is-your-turn strong { font-size: 13px; }
  .turn-number { font-size: 10px; }
  .turn-phase { font-size: 9px; }
  .environment-chip { top: 0; right: 0; gap: 0; width: 26px; height: 26px; justify-content: center; padding: 0; overflow: hidden; font-size: 0; }
  .environment-chip i { width: 20px; height: 20px; font-size: 9px; }
  .team-hp-strip { gap: 6px; }
  .team-hp-card { grid-template-columns: 7px auto minmax(0, 1fr); gap: 5px; padding: 5px 7px; border-radius: 10px; }
  .team-hp-orb { width: 6px; height: 6px; }
  .team-hp-value { font-size: 10px; }
  .team-hp-value small { font-size: 8px; }
  .team-hp-meter { height: 5px; }
  .player-seat { --player-identity-width: 64px; border-radius: 14px; padding: 6px; }
  .seat-top { grid-template-rows: minmax(0, 1fr) auto; gap: 2px; }
  .seat-top.is-expanded { column-gap: 5px; }
  .seat-top .player-identity { gap: 2px 5px; }
  .seat-top .avatar { display: none; }
  .seat-top .player-heading { width: 100%; gap: 4px; }
  .seat-top .player-name-block { display: flex; align-items: baseline; gap: 4px; }
  .seat-top .player-name-block strong { max-width: 100%; font-size: 10px; }
  .seat-top .team-label { font-size: 7px; }
  .seat-top .profession-badge, .seat-top .effect-summary, .seat-top .counter-badge, .seat-top .shield-badge, .seat-top .turn-status, .seat-top .response-status { padding: 2px 4px; font-size: 7px; }
  .seat-top .player-statuses { max-height: 20px; flex-wrap: nowrap; overflow: hidden; }
  .seat-top .compact-hand-count { font-size: 8px; }
  .seat-hand-backs { height: 30px; padding-inline: 5px; }
  .seat-hand-backs img { flex-basis: 18px; width: 18px; height: 27px; }
  .seat-hand-backs img + img { margin-left: -10px; }
  .player-expanded-summary { max-height: 24px; gap: 0 5px; font-size: 8px; line-height: 12px; }
  .player-detail-toggle, .compact-hand-button { min-height: 22px; padding: 0 6px; font-size: 8px; }
  .seat-bottom { gap: 5px; }
  .seat-top .seat-discard-control :deep(.discard-pile-trigger) { width: 20px; }
  .seat-discard-control :deep(.discard-pile-label) { max-width: 38px; font-size: 7px; }
  .seat-bottom .player-heading { align-items: flex-start; gap: 4px; }
  .seat-bottom .player-heading .avatar { width: 24px; height: 24px; font-size: 10px; }
  .seat-bottom .player-name-block strong { max-width: 70px; font-size: 12px; }
  .seat-bottom .team-label { font-size: 7px; }
  .seat-bottom .player-identity { align-content: center; gap: 3px; }
  .seat-bottom .profession-badge, .seat-bottom .effect-summary, .seat-bottom .counter-badge, .seat-bottom .shield-badge, .seat-bottom .turn-status, .seat-bottom .response-status { padding: 2px 4px; font-size: 7px; }
  .seat-bottom .player-statuses { max-height: 20px; flex-wrap: nowrap; overflow: hidden; }
  .seat-bottom .seat-hand { gap: 3px; padding: 16px 5px 8px; }
  .seat-bottom .seat-hand :deep(.playing-card) { width: clamp(36px, 12vw, 52px); min-width: 32px; }
  .shared-discard-dock { top: 4px; right: 4px; }
  .shared-discard-dock :deep(.discard-pile-label) { display: none; }
  .shared-discard-dock :deep(.discard-pile-trigger) { width: 30px; }
  .formation-field { border-radius: 16px; min-height: 0; padding: 4px 6px; }
  .formation-field.is-terminal { padding-block: 0; }
  .previous-formation { min-height: 46px; }
  .formation-cards { min-height: 30px; }
  .formation-cards :deep(.formation-card) { width: clamp(30px, 9vw, 42px); }
  .formation-field-label, .previous-formation small, .previous-formation p { font-size: 10px; }
  .previous-formation strong { font-size: 12px; }
  .center-stack { grid-template-rows: minmax(76px, .55fr) minmax(0, .95fr); gap: 5px; }
  .player-detail-facts { grid-template-columns: 1fr 1fr; gap: 10px 14px; }
}
/* 終局使用整個中央區，保留兩隊資訊與牌桌，讓結算成為閱讀焦點。 */
.battlefield.is-finished { grid-template-rows: 44px 32px clamp(72px, 10dvh, 88px) minmax(0, 1fr) clamp(128px, 20dvh, 160px); gap: 8px; padding: 12px; }
.battlefield.is-finished .battle-stage-strip { height: 44px; min-height: 44px; }
.battlefield.is-finished .turn-capsule { padding-block: 7px; }
.battlefield.is-finished .center-stack { grid-row: 4; grid-column: 1 / -1; grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(0, 1fr); }
.battlefield.is-finished .board-center { grid-column: 1; grid-row: 1; }
.battlefield.is-finished .seat-top { grid-column: var(--compact-column); }
.battlefield.is-finished:not(.four-player) .seat-top { grid-column: 1 / -1; }
.battlefield.is-finished .formation-field { grid-template-rows: minmax(0, 1fr); align-content: center; padding: 8px 64px; background: none; }
.battlefield.is-finished .terminal-resolution { container: conclusion / size; min-height: 0; justify-items: center; align-content: safe center; overflow-y: auto; padding: 4px; }
.battlefield.is-finished .turn-capsule-heading { grid-template-columns: auto minmax(0, auto); }
@media (min-width: 601px) {
  .battlefield.is-finished .seat-bottom .seat-hand { padding-block: 6px; }
  .battlefield.is-finished .seat-bottom .seat-hand :deep(.playing-card) { width: clamp(52px, 9dvh, 80px); }
}
@media (min-width: 601px) and (max-height: 700px) {
  .battlefield.is-finished { grid-template-rows: 40px 28px 64px minmax(0, 1fr) 112px; gap: 6px; padding: 8px; }
  .battlefield.is-finished .battle-stage-strip { height: 40px; min-height: 40px; }
  .battlefield.is-finished .player-seat { padding-block: 8px; }
}
@media (max-width: 600px) {
  .battlefield.is-finished { grid-template-rows: 40px 28px 64px minmax(0, 1fr) 112px; gap: 6px; padding: 8px; }
  .battlefield.is-finished .battle-stage-strip { height: 40px; min-height: 40px; }
  .battlefield.is-finished .formation-field { padding: 8px 0; }
  .battlefield.is-finished:not(.personal-deck) .formation-field { padding-top: 48px; }
  .battlefield.is-finished .shared-discard-dock { margin: 0; top: 4px; right: auto; }
  .battlefield.is-finished .shared-discard-dock :deep(.discard-pile-trigger) { width: 30px; }
}
@media (max-width: 600px) and (max-height: 700px) {
  .battlefield.is-finished { grid-template-rows: 40px 28px 56px minmax(0, 1fr) 104px; gap: 4px; }
}
</style>
