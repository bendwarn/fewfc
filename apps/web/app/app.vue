<template>
  <div class="app-shell">
    <NuxtRouteAnnouncer />
    <header class="topbar">
      <div>
        <p class="eyebrow">CFECards 規則引擎</p>
        <h1>對戰桌</h1>
      </div>
      <div class="viewer-switch" aria-label="觀看者">
        <button
          v-for="option in viewers"
          :key="option"
          type="button"
          :class="{ active: viewer === option }"
          @click="viewer = option"
        >
          {{ viewerLabel(option) }}
        </button>
      </div>
    </header>

    <main class="table-layout">
      <section class="state-panel" aria-labelledby="state-heading">
        <div class="section-heading">
          <h2 id="state-heading">公開狀態</h2>
          <span>{{ statusLabel(state.status) }}</span>
        </div>

        <div class="metric-grid">
          <div>
            <span>階段</span>
            <strong>{{ phaseLabel(state.phase) }}</strong>
          </div>
          <div>
            <span>目前玩家</span>
            <strong>{{ playerLabel(state.currentPlayer) }}</strong>
          </div>
          <div>
            <span>回合</span>
            <strong>{{ state.turnNumber }}</strong>
          </div>
        </div>

        <div class="arena" aria-label="隊伍與手牌可見性">
          <article
            v-for="player in state.players"
            :key="player.id"
            class="player-lane"
          >
            <div>
              <h3>{{ playerLabel(player.id) }}</h3>
              <p>{{ teamLabel(player.team) }}</p>
            </div>
            <div class="hand-row" :aria-label="`${playerLabel(player.id)} 手牌`">
              <button
                v-for="card in cardsFor(player.id)"
                :key="card.id"
                type="button"
                class="card-token"
                :class="{ hidden: card.hidden, selected: card.selected }"
                :disabled="!card.selectable"
                :aria-label="card.hidden ? '隱藏牌' : card.label"
                @click="game.toggleCardSelection(player.id, card.cardId)"
              >
                {{ card.label }}
              </button>
            </div>
          </article>
        </div>
      </section>

      <aside class="side-panel" aria-labelledby="controls-heading">
        <section class="control-band">
          <h2 id="controls-heading">操作</h2>
          <p v-if="game.errorMessage.value" class="error-message">
            {{ game.errorMessage.value }}
          </p>
          <div class="button-row">
            <button type="button" :disabled="game.isLoading.value" @click="game.startSampleGame()">重新開始</button>
            <button type="button" :disabled="game.isLoading.value" @click="game.passAction()">跳過行動</button>
            <button type="button" :disabled="game.isLoading.value" @click="game.advanceAutomatic()">自動推進</button>
          </div>
        </section>

        <section class="control-band">
          <h2>隊伍</h2>
          <dl class="team-list">
            <div v-for="team in state.hp" :key="team.team">
              <dt>{{ teamLabel(team.team) }}</dt>
              <dd>{{ team.hp }} HP</dd>
            </div>
          </dl>
        </section>

        <section class="control-band">
          <h2>可用陣法</h2>
          <p v-if="game.selectedCards.value.length === 0">請先點選自己的手牌。</p>
          <p v-else-if="game.playableFormations.value.length === 0">目前選牌沒有可用陣法。</p>
          <div v-else class="formation-list">
            <button
              v-for="formation in game.playableFormations.value"
              :key="formation.id"
              type="button"
              class="formation-option"
              :disabled="game.isLoading.value"
              @click="game.performFormation(formation)"
            >
              <span>{{ formation.name }}</span>
              <small>{{ formationSummary(formation.category) }}</small>
              <p>{{ formation.summary }}</p>
            </button>
          </div>
        </section>

        <section class="control-band">
          <h2>待選擇</h2>
          <p v-if="state.pendingChoice">
            {{ playerLabel(state.pendingChoice.player) }}:
            {{ choiceLabel(state.pendingChoice.kind) }}
          </p>
          <div
            v-if="state.pendingChoice && state.pendingChoice.cards.length > 0"
            class="choice-card-list"
          >
            <button
              v-for="card in state.pendingChoice.cards"
              :key="card.id"
              type="button"
              class="choice-card"
              :disabled="game.isLoading.value || viewer !== state.pendingChoice.player"
              @click="game.choosePendingCard(card.id)"
            >
              {{ card.label }}
            </button>
          </div>
          <p v-if="!state.pendingChoice">目前沒有待選擇項目</p>
        </section>

        <section class="control-band">
          <h2>公開區域</h2>
          <dl class="compact-list">
            <div>
              <dt>棄牌區</dt>
              <dd>{{ state.discard.length }} 張</dd>
            </div>
            <div>
              <dt>覆蓋被動</dt>
              <dd>{{ state.coveredPassives.length }}</dd>
            </div>
            <div>
              <dt>護盾</dt>
              <dd>{{ state.shields.length }}</dd>
            </div>
            <div>
              <dt>狀態</dt>
              <dd>{{ state.statuses.length }}</dd>
            </div>
          </dl>
        </section>

        <section class="control-band">
          <h2>近期事件</h2>
          <ol class="event-feed">
            <li v-for="event in visibleEvents" :key="event.id">
              <span>{{ eventTypeLabel(event.eventType) }}</span>
              <p>{{ event.summary }}</p>
            </li>
          </ol>
        </section>
      </aside>
    </main>
  </div>
</template>

<script setup lang="ts">
import type { PlayerId, ViewerId } from '~/types/fewfc'

const viewers: ViewerId[] = ['observer', 'alice', 'bob']
const viewer = ref<ViewerId>('observer')
const game = useLocalGame(viewer)
const state = game.state

const visibleEvents = computed(() => game.publicEvents.value.slice(0, 6))

interface CardToken {
  id: string
  cardId: number
  label: string
  hidden: boolean
  selectable: boolean
  selected: boolean
}

function cardsFor(player: PlayerId): CardToken[] {
  const hand = state.value.hands.find((entry) => entry.player === player)

  if (!hand) {
    return []
  }

  if (hand.cards.kind === 'known') {
    return hand.cards.cards.map((card, index) => ({
      id: `${player}-known-${index}-${card.id}`,
      cardId: card.id,
      label: card.label,
      hidden: false,
      selectable: game.canSelectCard(player),
      selected: game.selectedCards.value.includes(card.id),
    }))
  }

  return Array.from({ length: hand.cards.count }, (_, index) => ({
    id: `${player}-hidden-${index}`,
    cardId: -index - 1,
    label: '',
    hidden: true,
    selectable: false,
    selected: false,
  }))
}

function viewerLabel(value: ViewerId): string {
  return value === 'observer' ? '觀戰者' : playerLabel(value)
}

function playerLabel(value: PlayerId | null): string {
  if (!value) {
    return '尚未開始'
  }

  return value === 'alice' ? '玩家 Alice' : '玩家 Bob'
}

function teamLabel(value: string): string {
  if (value === 'team:alice') {
    return 'Alice 隊'
  }

  if (value === 'team:bob') {
    return 'Bob 隊'
  }

  return value
}

function statusLabel(value: string): string {
  return value === 'InProgress' ? '進行中' : '已結束'
}

function phaseLabel(value: string): string {
  const labels: Record<string, string> = {
    Main: '主要階段',
    MainPhase: '主要階段',
    TurnStart: '回合開始',
    TurnDraw: '回合抽牌',
    TurnDrawDiscardChoice: '回合抽牌棄牌選擇',
    TurnEnd: '回合結束',
  }

  return labels[value] ?? value
}

function choiceLabel(value: string): string {
  const labels: Record<string, string> = {
    'Choose one drawn card to discard': '選擇一張本回合抽到的牌棄置',
    TurnDrawDiscard: '選擇一張本回合抽到的牌棄置',
    EffectGenerated: '選擇效果指定的牌',
    Hidden: '隱藏選擇',
  }

  return labels[value] ?? value
}

function eventTypeLabel(value: string): string {
  const labels: Record<string, string> = {
    ActionPassed: '跳過行動',
    AutomaticAdvance: '自動推進',
    CardsDealt: '發牌',
    FormationPerformed: '陣法發動',
    TurnStarted: '回合開始',
  }

  return labels[value] ?? value
}

function formationSummary(value: string): string {
  return value === 'Attack' ? '攻擊' : '法術'
}
</script>

<style>
:root {
  color: #18201c;
  background: #f4f1e8;
  font-family:
    Inter, ui-sans-serif, system-ui, -apple-system, BlinkMacSystemFont, "Segoe UI",
    sans-serif;
}

* {
  box-sizing: border-box;
}

body {
  margin: 0;
}

button {
  border: 1px solid #18201c;
  background: #f8f6ef;
  color: #18201c;
  cursor: pointer;
  font: inherit;
  min-height: 2.5rem;
  padding: 0 0.875rem;
}

button:hover,
button.active {
  background: #18201c;
  color: #f8f6ef;
}

.app-shell {
  min-height: 100vh;
}

.topbar {
  align-items: end;
  border-bottom: 1px solid #d5cfbf;
  display: flex;
  gap: 1rem;
  justify-content: space-between;
  padding: 1.25rem clamp(1rem, 4vw, 3rem);
}

.eyebrow {
  color: #667067;
  font-size: 0.75rem;
  font-weight: 700;
  letter-spacing: 0;
  margin: 0 0 0.25rem;
  text-transform: uppercase;
}

h1,
h2,
h3,
p {
  margin: 0;
}

h1 {
  font-size: clamp(2rem, 6vw, 4.25rem);
  line-height: 0.95;
}

h2 {
  font-size: 1rem;
}

h3 {
  font-size: 1rem;
}

.viewer-switch,
.button-row {
  display: flex;
  flex-wrap: wrap;
  gap: 0.5rem;
}

.table-layout {
  display: grid;
  gap: 1.5rem;
  grid-template-columns: minmax(0, 1fr) minmax(20rem, 24rem);
  padding: 1.5rem clamp(1rem, 4vw, 3rem) 3rem;
}

.state-panel,
.side-panel {
  min-width: 0;
}

.section-heading {
  align-items: center;
  display: flex;
  justify-content: space-between;
  margin-bottom: 1rem;
}

.section-heading span {
  border: 1px solid #d5cfbf;
  padding: 0.375rem 0.625rem;
}

.metric-grid {
  display: grid;
  gap: 0.75rem;
  grid-template-columns: repeat(3, minmax(0, 1fr));
  margin-bottom: 1.25rem;
}

.metric-grid div,
.control-band {
  border: 1px solid #d5cfbf;
  background: #fffdf8;
  padding: 1rem;
}

.metric-grid span {
  color: #667067;
  display: block;
  font-size: 0.75rem;
  margin-bottom: 0.25rem;
}

.arena {
  background:
    linear-gradient(90deg, rgba(24, 32, 28, 0.07) 1px, transparent 1px),
    linear-gradient(rgba(24, 32, 28, 0.07) 1px, transparent 1px),
    #ebe2cf;
  background-size: 2rem 2rem;
  border: 1px solid #c9bea8;
  display: grid;
  gap: 1rem;
  min-height: 30rem;
  padding: 1rem;
}

.player-lane {
  align-items: center;
  background: rgba(255, 253, 248, 0.9);
  border: 1px solid #c9bea8;
  display: grid;
  gap: 1rem;
  grid-template-columns: minmax(7rem, 10rem) 1fr;
  padding: 1rem;
}

.player-lane p {
  color: #667067;
  font-size: 0.875rem;
  margin-top: 0.25rem;
}

.hand-row {
  display: grid;
  gap: 0.625rem;
  grid-template-columns: repeat(auto-fit, minmax(4.25rem, 1fr));
}

.card-token {
  align-items: center;
  aspect-ratio: 5 / 7;
  background: #f7d05c;
  border: 1px solid #18201c;
  display: flex;
  font-size: 0.8125rem;
  font-weight: 700;
  justify-content: center;
  max-height: 7rem;
  min-width: 0;
  padding: 0.375rem;
  text-align: center;
}

.card-token:disabled {
  cursor: default;
}

.card-token:not(:disabled):hover,
.card-token.selected {
  box-shadow: 0 0 0 3px #2d7d5f;
  transform: translateY(-0.125rem);
}

.card-token.hidden {
  background: repeating-linear-gradient(
    135deg,
    #27352e 0,
    #27352e 0.5rem,
    #3f5248 0.5rem,
    #3f5248 1rem
  );
  color: #f8f6ef;
}

.formation-list {
  display: grid;
  gap: 0.625rem;
  margin-top: 0.75rem;
}

.choice-card-list {
  display: grid;
  gap: 0.5rem;
  grid-template-columns: repeat(auto-fit, minmax(4.5rem, 1fr));
  margin-top: 0.75rem;
}

.choice-card {
  min-height: 2.75rem;
}

.formation-option {
  align-items: start;
  display: grid;
  gap: 0.25rem;
  justify-items: start;
  min-height: auto;
  padding: 0.75rem;
  text-align: left;
}

.formation-option span {
  font-weight: 800;
}

.formation-option small {
  color: #667067;
}

.formation-option:hover small {
  color: #dfe9df;
}

.error-message {
  color: #a83232;
  margin-bottom: 0.75rem;
}

.side-panel {
  display: grid;
  gap: 1rem;
}

.team-list {
  display: grid;
  gap: 0.75rem;
  margin: 0.75rem 0 0;
}

.team-list div,
.compact-list div {
  align-items: center;
  display: flex;
  justify-content: space-between;
}

.team-list dd,
.compact-list dd {
  font-weight: 800;
  margin: 0;
}

.compact-list {
  display: grid;
  gap: 0.5rem;
  margin: 0.75rem 0 0;
}

.event-feed {
  display: grid;
  gap: 0.75rem;
  list-style: none;
  margin: 0.75rem 0 0;
  padding: 0;
}

.event-feed li {
  border-top: 1px solid #d5cfbf;
  padding-top: 0.75rem;
}

.event-feed span {
  display: block;
  font-size: 0.75rem;
  font-weight: 800;
  margin-bottom: 0.25rem;
}

@media (max-width: 860px) {
  .topbar {
    align-items: start;
    flex-direction: column;
  }

  .table-layout,
  .metric-grid,
  .player-lane {
    grid-template-columns: 1fr;
  }
}
</style>
