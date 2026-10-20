<template>
  <main class="battlefield-story-screen">
    <div class="battlefield-story-layout battle-scene" :style="battleLayoutStyle">
      <BattlefieldBoard
        class="battlefield-story-board"
        :state="displayState"
        :display-names="displayNames"
        :anchor-player="anchorPlayer"
        :mode="mode"
        :selected-card-ids="selectedHandCardIds"
        :selectable-player="displayState.currentPlayer === anchorPlayer ? anchorPlayer : null"
        :connected-players="displayState.players.map(player => player.id)"
        :show-turn-controls="mode === 'live' && Boolean(anchorPlayer && displayState.currentPlayer === anchorPlayer)"
        ref="battlefield"
        @select-card="toggleStoryHandCard"
        @respond-to-choice="openPendingChoice"
      >
        <template #turn-controls>
          <div class="story-turn-controls battle-turn-controls">
            <section class="story-action-group story-abilities battle-action-group battle-glass">
              <header><h2>能力</h2><span>使用後仍可行動</span></header>
              <div class="battle-action-options">
                <button type="button">棄牌回收</button>
                <div class="story-spirit-picker">
                  <button
                    ref="splendorMenuTrigger"
                    class="story-fire-skill"
                    type="button"
                    aria-haspopup="menu"
                    :aria-expanded="splendorMenuOpen"
                    @click="splendorMenuOpen = !splendorMenuOpen"
                  >
                    絢爛
                  </button>
                  <AnchoredSurface
                    id="story-splendor-menu"
                    :open="splendorMenuOpen"
                    :anchor="splendorMenuTrigger"
                    role="menu"
                    placement="top-start"
                    :return-focus="false"
                    surface-class="story-splendor-menu"
                    @close="splendorMenuOpen = false"
                  >
                    <button v-for="level in [1, 2, 3, 4, 5]" :key="level" type="button" role="menuitem" @click="splendorMenuOpen = false">
                      {{ level }} 級
                    </button>
                  </AnchoredSurface>
                </div>
              </div>
            </section>
            <section class="story-action-group story-actions battle-action-group battle-action-group--primary battle-glass">
              <header><h2>行動</h2><span>本回合擇一</span></header>
              <div class="battle-action-options">
                <button type="button">施展火陣</button>
                <button type="button">轉職</button>
              </div>
            </section>
          </div>
        </template>
      </BattlefieldBoard>

      <aside class="battlefield-story-record battle-glass" :class="{ 'has-motion-controls': showMotionDemo }" aria-label="戰局紀錄">
        <header class="story-record-heading">
          <span class="story-record-emblem" aria-hidden="true">✧</span>
          <h2>戰局紀錄</h2>
          <span class="story-record-live"><i />即時更新</span>
        </header>
        <div v-if="showMotionDemo" class="story-motion-controls" aria-label="動效預覽控制">
          <p>點擊自己的手牌可切換選取狀態。</p>
          <button type="button" @click="toggleTurnPreview">切換回合提示</button>
          <button type="button" @click="previewAcceptedCardPlay">{{ demoCardConsumed ? '重設出牌預覽' : '預覽成功出牌' }}</button>
          <button type="button" @click="triggerPendingChoicePreview">模擬必要選擇</button>
        </div>
        <BattleRecordTimeline class="story-record-timeline" :groups="storyRecordGroups" :current-turn-number="storyCurrentTurnNumber" />
      </aside>
      <button class="battlefield-story-mobile-record" type="button" :aria-expanded="recordOpen" @click="recordOpen = true">
        <strong>戰局紀錄</strong><span>小明的回合開始 · 目前可選擇能力與本回合行動。</span><b aria-hidden="true">⌃</b>
      </button>
    </div>
    <Teleport to="body">
      <div v-if="recordOpen" class="battlefield-story-record-sheet">
        <button class="story-record-backdrop" type="button" aria-label="關閉戰局紀錄" @click="recordOpen = false" />
        <section role="dialog" aria-modal="true" aria-label="戰局紀錄">
          <header><h2>戰局紀錄</h2><button type="button" aria-label="關閉戰局紀錄" @click="recordOpen = false">×</button></header>
          <BattleRecordTimeline class="story-record-timeline" :groups="storyRecordGroups" :current-turn-number="storyCurrentTurnNumber" />
        </section>
      </div>
    </Teleport>
    <ModalShell
      :open="responseOpen"
      root-class="choice-overlay"
      panel-class="choice-overlay-panel"
      labelledby="story-pending-choice-title"
      initial-focus-selector=".story-choice-option"
      @close="responseOpen = false"
    >
      <div class="story-pending-choice">
        <h2 id="story-pending-choice-title">啟示：選擇一張牌</h2>
        <p v-if="state.pendingChoice?.visibility === 'visible'">{{ presentPendingChoice(state.pendingChoice.reason) }}</p>
        <div v-if="state.pendingChoice?.visibility === 'visible' && state.pendingChoice.choice.type === 'card'" class="story-choice-options">
          <button v-for="card in state.pendingChoice.choice.cards" :key="card.id" class="story-choice-option" type="button" @click="responseOpen = false">{{ card.label }}</button>
        </div>
      </div>
    </ModalShell>
  </main>
</template>

<script setup lang="ts">
import { computed, nextTick, ref, toRaw, watch } from 'vue'
import AnchoredSurface from '~/components/AnchoredSurface.vue'
import BattleRecordTimeline from '~/components/BattleRecordTimeline.vue'
import BattlefieldBoard from '~/components/BattlefieldBoard.vue'
import ModalShell from '~/components/ModalShell.vue'
import { presentPendingChoice } from '~/lib/pending-choice-presentation'
import type { CardInstanceId, PlayerId, PublicGameState } from '~/types/fewfc'

const props = defineProps<{
  state: PublicGameState
  displayNames: Record<string, string>
  anchorPlayer: PlayerId | null
  mode: 'live' | 'replay'
  showSplendorMenu?: boolean
  showMotionDemo?: boolean
}>()

const storyRounds = [
  {
    title: '第 1 回合 · 小明',
    events: [
      { title: '小明發動烈焰陣', description: '以火 3、水 2、木 1 組成陣法。' },
      { title: '小華選擇承受傷害', description: '我方隊伍生命值減少 18。' },
    ],
  },
  {
    title: '第 2 回合 · 小美',
    events: [
      { title: '小美使用棄牌回收', description: '從棄牌堆取回水 2。' },
      { title: '小安抽取一張牌', description: '回合抽牌完成。' },
    ],
  },
  {
    title: '第 3 回合 · 小明',
    events: [
      { title: '施展青木陣', description: '使用木 5、木 2、木 4 攻擊對手。' },
      { title: '小明完成出牌', description: '抽取 3 張牌，等待下一個回合。' },
    ],
  },
]
const currentStoryRound = {
  title: '第 4 回合 · 你的回合',
  description: '選擇行動…',
}
const displayState = ref(props.state)
const storyCurrentTurnNumber = computed(() => displayState.value.turnNumber)
const storyRecordGroups = computed(() => [
  ...storyRounds.map((round, index) => ({
    id: `turn-${index + 1}`,
    turnNumber: index + 1,
    title: round.title,
    entries: round.events.map((event, eventIndex) => ({
      id: `turn-${index + 1}-event-${eventIndex + 1}`,
      title: event.title,
      summary: event.description,
    })),
  })),
  {
    id: `turn-${displayState.value.turnNumber}`,
    turnNumber: displayState.value.turnNumber,
    title: `第 ${displayState.value.turnNumber} 回合 · ${props.displayNames[displayState.value.currentPlayer ?? ''] ?? '等待開始'}的回合`,
    entries: [{
      id: `turn-${displayState.value.turnNumber}-current`,
      title: displayState.value.currentPlayer === props.anchorPlayer ? currentStoryRound.description : '等待玩家行動。',
    }],
  },
])

watch(() => props.state, value => { displayState.value = value }, { deep: true })
function cloneState(state: PublicGameState) { return structuredClone(toRaw(state)) }
const battleLayoutStyle = computed(() => ({
  '--battle-page-background': `url('/backgrounds/${displayState.value.environment?.toLowerCase() ?? 'void'}.webp')`,
}))

const recordOpen = ref(false)
const responseOpen = ref(false)
const splendorMenuOpen = ref(Boolean(props.showSplendorMenu))
const splendorMenuTrigger = ref<HTMLButtonElement | null>(null)
interface CardFlightSnapshot { cardId: number, src: string, left: number, top: number, width: number, height: number }
const battlefield = ref<{
  closeDetail: () => void
  captureCardFlights: (cardIds: readonly number[]) => CardFlightSnapshot[]
  animateCardFlights: (flights: readonly CardFlightSnapshot[]) => void
} | null>(null)
const demoCardConsumed = ref(false)
const selectedHandCardIds = ref<CardInstanceId[]>([])

function toggleStoryHandCard(player: PlayerId, cardId: CardInstanceId) {
  if (player !== props.anchorPlayer || displayState.value.currentPlayer !== player) return
  selectedHandCardIds.value = selectedHandCardIds.value.includes(cardId)
    ? selectedHandCardIds.value.filter(selected => selected !== cardId)
    : [...selectedHandCardIds.value, cardId]
}

function toggleTurnPreview() {
  const next = cloneState(displayState.value)
  const otherPlayer = next.turnOrder.find(player => player !== props.anchorPlayer)
  next.currentPlayer = next.currentPlayer === props.anchorPlayer ? otherPlayer ?? next.currentPlayer : props.anchorPlayer
  next.turnNumber += 1
  next.phase = next.phase === 'Action' ? 'ActiveEffects' : 'Action'
  displayState.value = next
}

function triggerPendingChoicePreview() {
  if (!props.anchorPlayer) return
  const hand = displayState.value.hands.find(candidate => candidate.player === props.anchorPlayer)?.cards
  const cards = hand?.kind === 'known'
    ? hand.cards
    : hand?.kind === 'partiallyKnown'
      ? hand.cards.flatMap(card => card ? [card] : [])
      : []
  if (!cards.length) return
  const next = cloneState(displayState.value)
  next.pendingChoice = {
    visibility: 'visible',
    choiceId: 4242,
    player: props.anchorPlayer,
    reason: { type: 'revelation' },
    choice: { type: 'card', cards, minimum: 1, maximum: 1, canDecline: false },
  }
  displayState.value = next
}

async function previewAcceptedCardPlay() {
  if (demoCardConsumed.value) {
    displayState.value = cloneState(props.state)
    demoCardConsumed.value = false
    selectedHandCardIds.value = []
    return
  }

  if (!props.anchorPlayer) return
  const hand = displayState.value.hands.find(candidate => candidate.player === props.anchorPlayer)?.cards
  const card = hand?.kind === 'known'
    ? hand.cards[0]
    : hand?.kind === 'partiallyKnown'
      ? hand.cards.find(candidate => candidate !== null)
      : undefined
  if (!card) return

  const flights = battlefield.value?.captureCardFlights([card.id]) ?? []
  const next = cloneState(displayState.value)
  const nextHand = next.hands.find(candidate => candidate.player === props.anchorPlayer)?.cards
  if (nextHand?.kind === 'known') nextHand.cards = nextHand.cards.filter(candidate => candidate.id !== card.id)
  if (nextHand?.kind === 'partiallyKnown') nextHand.cards = nextHand.cards.filter(candidate => candidate?.id !== card.id)
  selectedHandCardIds.value = selectedHandCardIds.value.filter(selected => selected !== card.id)
  displayState.value = next
  demoCardConsumed.value = true
  await nextTick()
  battlefield.value?.animateCardFlights(flights)
}

function openPendingChoice() {
  battlefield.value?.closeDetail()
  responseOpen.value = true
}
</script>

<style scoped>
.battlefield-story-screen { width: 100%; height: 100dvh; min-height: 600px; overflow: hidden; background: var(--app-canvas); color: var(--app-text); }
.battlefield-story-layout { display: grid; width: 100%; height: 100%; min-height: 0; grid-template-columns: minmax(0, 1fr) clamp(270px, 26vw, 360px); gap: 10px; padding: 10px; }
.battlefield-story-board { min-height: 0; border: 0; border-radius: 0; box-shadow: none; }
.battlefield-story-record { display: grid; min-height: 0; grid-template-rows: auto minmax(0, 1fr); gap: 18px; overflow: hidden; padding: 20px; border-radius: 24px; }
.battlefield-story-record.has-motion-controls { grid-template-rows: auto auto minmax(0, 1fr); }
.battlefield-story-record > header, .battlefield-story-record-sheet section > header { display: flex; align-items: center; justify-content: space-between; gap: 8px; }
.battlefield-story-record h2, .battlefield-story-record-sheet h2 { margin: 0; color: var(--app-text); font: 600 16px var(--font-serif); }
.story-record-heading { padding-bottom: 12px; border-bottom: 1px solid var(--app-border); }
.story-record-emblem, .story-record-current-icon { display: grid; flex: none; width: 27px; height: 27px; place-items: center; border: 1px solid var(--glass-stroke); border-radius: 50%; background: linear-gradient(135deg, rgba(255,255,255,.18), transparent 60%), color-mix(in srgb, var(--glass-surface) 76%, transparent); color: #dcebed; font-size: 15px; box-shadow: inset 0 1px rgba(255,255,255,.3); }
.story-record-heading h2 { flex: 1; margin-left: 2px; }
.story-record-live { display: inline-flex; align-items: center; gap: 5px; color: var(--battle-text-secondary); font-size: 11px; white-space: nowrap; }
.story-record-live i { width: 6px; height: 6px; border-radius: 50%; background: #a7d9cb; box-shadow: 0 0 8px rgba(167,217,203,.7); }
.story-motion-controls { display: flex; flex-wrap: wrap; align-items: center; gap: 6px; }
.story-motion-controls p { flex-basis: 100%; margin: 0 0 2px; color: var(--app-text-muted); font-size: 10px; }
.story-motion-controls button { min-height: 30px; padding: 5px 9px; border: 1px solid var(--glass-stroke); border-radius: 9px; background: linear-gradient(135deg, rgba(255,255,255,.14), transparent 46%), color-mix(in srgb, var(--glass-surface) 82%, transparent); color: var(--app-text); font-size: 9px; transition: background .16s ease, border-color .16s ease, transform .16s ease; backdrop-filter: blur(14px) saturate(1.2); }
.story-motion-controls button:hover, .story-motion-controls button:focus-visible { transform: translateY(-1px); border-color: var(--app-accent-strong); background: rgba(201,163,79,.15); outline: 2px solid rgba(226,196,111,.65); outline-offset: 2px; }
.story-record-timeline { min-height: 0; margin: 0; padding: 0 3px 2px 0; overflow: auto; scrollbar-color: color-mix(in srgb, var(--app-border-strong) 56%, transparent) transparent; scrollbar-width: thin; }
.story-record-round { display: grid; gap: 8px; }
.story-record-round-heading { display: flex; align-items: center; gap: 8px; min-height: 31px; padding: 5px 9px; border: 1px solid rgba(228,244,246,.24); border-radius: 999px; background: linear-gradient(135deg, rgba(255,255,255,.12), transparent 58%), rgba(218,239,240,.055); }
.story-record-round-heading span { display: grid; flex: none; width: 18px; height: 18px; place-items: center; border: 1px solid rgba(228,244,246,.34); border-radius: 50%; color: #d8e7e8; font-size: 8px; }
.story-record-round-heading strong { color: var(--app-text); font: 600 11px var(--font-serif); }
.story-record-events { position: relative; display: grid; gap: 10px; margin: 0; padding: 2px 0 2px 3px; list-style: none; }
.story-record-events::before { position: absolute; top: 8px; bottom: 8px; left: 6px; width: 1px; background: linear-gradient(rgba(208,231,232,.4), rgba(208,231,232,.12)); content: ''; }
.story-record-events li { position: relative; display: grid; grid-template-columns: 13px minmax(0,1fr); gap: 8px; }
.story-record-events li > i { z-index: 1; width: 8px; height: 8px; margin-top: 4px; border: 2px solid #cfe4e4; border-radius: 50%; background: #607b7c; box-shadow: 0 0 0 3px rgba(207,228,228,.1); }
.story-record-events li strong { color: var(--app-text); font-size: 11px; }
.story-record-events li p { margin: 3px 0 0; color: var(--app-text-muted); font-size: 10px; line-height: 1.5; }
.story-record-current { display: grid; grid-template-columns: auto minmax(0,1fr); align-items: center; gap: 9px; padding: 8px 10px; border: 1px solid rgba(255,220,133,.84); border-radius: 16px; background: radial-gradient(ellipse at 95% 100%, rgba(255,206,92,.2), transparent 58%), linear-gradient(135deg, rgba(255,255,255,.18), transparent 48%), rgba(115,91,43,.32); box-shadow: inset 0 1px rgba(255,255,255,.28), 0 0 18px rgba(255,190,63,.16); }
.story-record-current-icon { border-color: rgba(255,232,174,.8); background: linear-gradient(135deg, rgba(255,255,255,.5), rgba(255,206,92,.25)); color: #fff0bd; text-shadow: 0 0 9px rgba(255,209,104,.72); }
.story-record-current strong { display: block; color: #fff0bd; font: 700 11px var(--font-serif); text-shadow: 0 0 8px rgba(255,205,91,.3); }
.story-record-current p { margin: 2px 0 0; color: rgba(255,242,205,.76); font-size: 9px; }
.story-spirit-picker { position: relative; }
.story-action-group .story-fire-skill { border-color: rgba(232,140,93,.7); border-radius: 10px; background: radial-gradient(ellipse at 12% 0%, rgba(232,140,93,.26), transparent 72%), linear-gradient(140deg, rgba(255,255,255,.14), transparent 42%), rgba(29,48,52,.68); color: #ffc091; box-shadow: inset 0 1px rgba(255,255,255,.28), 0 0 14px rgba(232,140,93,.12); }
.story-action-group .story-fire-skill:hover, .story-action-group .story-fire-skill:focus-visible { border-color: #f3a16f; color: #ffd0aa; box-shadow: inset 0 1px rgba(255,255,255,.28), 0 0 18px rgba(232,140,93,.22); outline: 2px solid rgba(243,161,111,.8); outline-offset: 2px; }
:global(.anchored-surface.story-splendor-menu) { --glass-light: color-mix(in srgb, #dcebed 14%, transparent); display: grid; min-width: 72px; gap: 4px; padding: 5px; border: 1px solid rgba(232,140,93,.62); border-radius: 12px; background: radial-gradient(ellipse at 12% 0%, rgba(232,140,93,.2), transparent 70%), linear-gradient(135deg, rgba(255,255,255,.16), transparent 42%), linear-gradient(165deg, var(--glass-light), rgba(26,44,48,.82)); box-shadow: inset 0 1px rgba(255,255,255,.3), 0 0 18px rgba(232,140,93,.13), 0 10px 24px rgba(0,0,0,.38); -webkit-backdrop-filter: blur(18px) saturate(1.25); backdrop-filter: blur(18px) saturate(1.25); }
:global(.anchored-surface.story-splendor-menu button) { min-height: 28px; border: 1px solid color-mix(in srgb, var(--app-border-strong) 54%, transparent); border-radius: 8px; background: color-mix(in srgb, var(--app-text) 7%, transparent); color: var(--app-text); font-size: 10px; transition: transform .16s ease, background .16s ease, border-color .16s ease, color .16s ease; }
:global(.anchored-surface.story-splendor-menu button:hover), :global(.anchored-surface.story-splendor-menu button:focus-visible) { border-color: #f3a16f; background: rgba(232,140,93,.18); color: #ffd0aa; outline-color: #f3a16f; }
:global(.anchored-surface.story-splendor-menu button:hover) { transform: translateY(-1px); }
.battlefield-story-mobile-record { display: none; }
.battlefield-story-record-sheet { position: fixed; z-index: 80; inset: 0; }
.story-record-backdrop { position: absolute; inset: 0; width: 100%; height: 100%; border: 0; background: var(--app-overlay); }
:global(.battlefield-story-record-sheet) { --glass-stroke: color-mix(in srgb, var(--app-border-strong) 62%, #dcebed 38%); --glass-light: color-mix(in srgb, #dcebed 14%, transparent); }
:global(.battlefield-story-record-sheet) section { position: absolute; right: 0; bottom: 0; left: 0; display: grid; max-height: 72dvh; grid-template-rows: auto minmax(0, 1fr) auto; gap: 12px; padding: 18px 18px 24px; overflow: auto; border: 1px solid var(--glass-stroke); border-bottom: 0; border-radius: 20px 20px 0 0; background: radial-gradient(ellipse at 90% 100%, rgba(167,214,218,.15), transparent 58%), linear-gradient(135deg, rgba(255,255,255,.2), transparent 36%), linear-gradient(165deg, var(--glass-light), color-mix(in srgb, var(--app-surface-raised) 78%, transparent)); box-shadow: inset 0 1px rgba(255,255,255,.42), 0 -18px 48px rgba(0,0,0,.4); -webkit-backdrop-filter: blur(22px) saturate(1.3); backdrop-filter: blur(22px) saturate(1.3); }
.battlefield-story-record-sheet section > header { padding-bottom: 12px; border-bottom: 1px solid var(--app-border); }
.battlefield-story-record-sheet header button { width: 32px; height: 32px; border: 1px solid var(--app-border-strong); background: transparent; color: var(--app-text); font-size: 20px; }
:global(.modal-shell.choice-overlay) { --glass-stroke: color-mix(in srgb, var(--app-border-strong) 62%, #dcebed 38%); --glass-light: color-mix(in srgb, #dcebed 14%, transparent); position: fixed; z-index: 70; inset: 0; display: grid; place-items: center; overflow-y: auto; padding: 16px; background: transparent; color: var(--app-text); text-align: center; }
:global(.modal-shell.choice-overlay > .modal-shell-backdrop) { background: var(--app-choice-overlay); backdrop-filter: blur(5px); }
:global(.modal-shell.choice-overlay > .modal-shell-panel.choice-overlay-panel) { position: relative; z-index: 1; width: min(560px, calc(100vw - 32px)); max-width: calc(100vw - 32px); max-height: calc(100dvh - 32px); min-width: 0; overflow-y: auto; padding: 28px; border: 1px solid var(--glass-stroke); border-radius: 20px; background: radial-gradient(ellipse at 90% 100%, rgba(167,214,218,.15), transparent 58%), linear-gradient(140deg, rgba(255,255,255,.2), rgba(158,190,192,.05) 45%, transparent 76%), linear-gradient(165deg, var(--glass-light), color-mix(in srgb, var(--app-surface-raised) 78%, transparent)); box-shadow: inset 0 1px rgba(255,255,255,.42), var(--app-shadow-lg); -webkit-backdrop-filter: blur(24px) saturate(1.32); backdrop-filter: blur(24px) saturate(1.32); }
.story-pending-choice { color: var(--app-text); }
.story-pending-choice h2 { margin: 0 0 12px; color: var(--app-text); font: 700 20px var(--font-serif); }
.story-pending-choice p { margin: 0; color: var(--app-text-muted); font-size: 12px; }
.story-choice-options { display: flex; flex-wrap: wrap; justify-content: center; gap: 10px; margin-top: 22px; }
.story-choice-option { min-height: 42px; padding: 8px 18px; border: 1px solid var(--app-border-strong); border-radius: 10px; background: var(--app-surface-raised); color: var(--app-text); font-size: 12px; }
.story-choice-option:hover, .story-choice-option:focus-visible { border-color: var(--app-accent); outline: 2px solid var(--app-focus-ring); outline-offset: 2px; }
@media (max-width: 900px) {
  .battlefield-story-layout { grid-template-columns: minmax(0, 1fr); grid-template-rows: minmax(0, 1fr) auto; gap: 0; padding: 0; }
  .battlefield-story-record { display: none; }
  .battlefield-story-mobile-record { display: flex; min-width: 0; align-items: center; gap: 10px; padding: 10px 14px; border: 0; border-top: 1px solid var(--glass-stroke); background: linear-gradient(135deg, rgba(255,255,255,.18), transparent 60%), color-mix(in srgb, var(--app-surface-raised) 70%, transparent); color: var(--app-text); text-align: left; backdrop-filter: blur(20px) saturate(1.24); }
  .battlefield-story-mobile-record strong { flex: none; color: var(--app-accent-strong); font: 600 11px var(--font-serif); }
  .battlefield-story-mobile-record span { min-width: 0; flex: 1; overflow: hidden; color: var(--app-text-muted); font-size: 9px; text-overflow: ellipsis; white-space: nowrap; }
  .battlefield-story-mobile-record b { color: var(--app-accent-strong); }
  .battlefield-story-board { border-radius: 0; }
}
</style>
