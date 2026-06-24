<template>
  <div class="app-shell" :class="`screen-${screen}`">
    <NuxtRouteAnnouncer />

    <header class="site-header">
      <button class="brand" type="button" aria-label="回到首頁" @click="goHome">
        <span class="brand-mark" aria-hidden="true">五</span>
        <span>
          <strong>五行戰牌</strong>
          <small>CFECARDS</small>
        </span>
      </button>

      <nav v-if="screen !== 'login'" class="header-actions" aria-label="帳號選單">
        <span class="connection"><i /> 本機連線</span>
        <button class="profile-button" type="button" @click="profileOpen = !profileOpen">
          <span class="avatar">{{ playerInitial }}</span>
          <span>{{ displayName }}</span>
          <span aria-hidden="true">⌄</span>
        </button>
        <div v-if="profileOpen" class="profile-menu">
          <button type="button" @click="logout">登出</button>
        </div>
      </nav>
    </header>

    <main v-if="screen === 'login'" class="login-layout">
      <section class="login-hero">
        <div class="hero-copy">
          <p class="kicker"><span /> 五行交鋒，陣法成局</p>
          <h1>以牌為陣，<br><em>決勝五行。</em></h1>
          <p class="hero-description">
            運用金、木、水、火、土的生剋關係，組合陣法、洞察對手，
            在每一次出牌中掌握戰局。
          </p>
          <div class="element-orbit" aria-hidden="true">
            <span class="element metal">金</span>
            <span class="element wood">木</span>
            <span class="element water">水</span>
            <span class="element fire">火</span>
            <span class="element earth">土</span>
            <div class="orbit-core">五行</div>
          </div>
        </div>
        <footer class="hero-footer">
          <span>© 2026 CFECards</span>
          <span>遊戲規則 · 隱私權</span>
        </footer>
      </section>

      <section class="login-panel">
        <div class="auth-card">
          <div class="mobile-brand"><span class="brand-mark">五</span> 五行戰牌</div>
          <p class="section-kicker">WELCOME BACK</p>
          <h2>{{ authMode === 'sign-in' ? '登入對戰' : '建立帳號' }}</h2>
          <p class="muted">
            {{ authMode === 'sign-in' ? '使用 Email 登入，繼續你的對戰紀錄。' : '建立可在不同裝置使用的玩家身份。' }}
          </p>

          <form @submit.prevent="login">
            <template v-if="authMode === 'sign-up'">
              <label for="player-name">玩家名稱</label>
              <div class="input-wrap">
                <span aria-hidden="true">人</span>
                <input
                  id="player-name"
                  v-model.trim="nameInput"
                  type="text"
                  maxlength="16"
                  autocomplete="nickname"
                  placeholder="顯示名稱"
                >
              </div>
            </template>

            <label for="player-email">Email</label>
            <div class="input-wrap">
              <span aria-hidden="true">@</span>
              <input
                id="player-email"
                v-model.trim="emailInput"
                type="email"
                autocomplete="email"
                placeholder="you@example.com"
                autofocus
              >
            </div>

            <label for="player-password">密碼</label>
            <div class="input-wrap">
              <span aria-hidden="true">密</span>
              <input
                id="player-password"
                v-model="passwordInput"
                type="password"
                :autocomplete="authMode === 'sign-in' ? 'current-password' : 'new-password'"
                placeholder="至少 10 個字元"
              >
            </div>
            <p v-if="loginError" class="form-error">{{ loginError }}</p>
            <button class="primary-button login-button" type="submit" :disabled="authBusy">
              {{ authBusy ? '處理中…' : authMode === 'sign-in' ? '登入' : '註冊並登入' }}
              <span aria-hidden="true">→</span>
            </button>
          </form>

          <button class="auth-mode-button" type="button" @click="toggleAuthMode">
            {{ authMode === 'sign-in' ? '還沒有帳號？建立帳號' : '已經有帳號？返回登入' }}
          </button>

          <div class="divider"><span>或使用訪客身份</span></div>
          <button class="ghost-button" type="button" :disabled="authBusy" @click="guestLogin">
            快速開始
          </button>
          <p class="terms">繼續即表示你同意遊戲規範與使用條款。</p>
        </div>
      </section>
    </main>

    <main v-else-if="screen === 'lobby'" class="lobby-page">
      <div class="lobby-heading">
        <div>
          <p class="section-kicker">GAME LOBBY</p>
          <h1>準備開局</h1>
          <p class="muted">建立新的對戰房間，或使用房間代碼加入。</p>
        </div>
        <div class="lobby-tabs" role="tablist">
          <button
            type="button"
            :class="{ active: lobbyTab === 'create' }"
            @click="lobbyTab = 'create'"
          >
            建立房間
          </button>
          <button
            type="button"
            :class="{ active: lobbyTab === 'join' }"
            @click="lobbyTab = 'join'"
          >
            加入房間
          </button>
        </div>
      </div>

      <div v-if="lobbyTab === 'create'" class="lobby-grid">
        <section class="setup-card">
          <div class="card-heading">
            <span class="step-number">01</span>
            <div>
              <h2>房間設定</h2>
              <p>設定這場對戰的基本資訊。</p>
            </div>
          </div>

          <label for="room-name">房間名稱</label>
          <input id="room-name" v-model="roomName" class="text-input" maxlength="24">

          <fieldset>
            <legend>對戰模式</legend>
            <div class="option-grid">
              <button
                v-for="mode in modes"
                :key="mode.id"
                type="button"
                class="mode-option"
                :class="{ selected: roomMode === mode.id }"
                :disabled="mode.disabled"
                @click="roomMode = mode.id"
              >
                <span class="mode-icon">{{ mode.icon }}</span>
                <strong>{{ mode.label }}</strong>
                <small>{{ mode.description }}</small>
                <i v-if="mode.disabled">即將推出</i>
              </button>
            </div>
          </fieldset>

          <fieldset>
            <legend>房間權限</legend>
            <div class="segmented">
              <button
                type="button"
                :class="{ active: roomAccess === 'private' }"
                @click="roomAccess = 'private'"
              >
                私人房間
              </button>
              <button
                type="button"
                :class="{ active: roomAccess === 'public' }"
                @click="roomAccess = 'public'"
              >
                公開房間
              </button>
            </div>
          </fieldset>
        </section>

        <aside class="room-preview">
          <div class="preview-topline">
            <span>房間預覽</span>
            <span class="status-pill">等待中</span>
          </div>
          <div class="room-emblem"><span>五</span></div>
          <p class="room-code-label">ROOM CODE</p>
          <div class="room-code">
            <strong>{{ roomCode }}</strong>
            <button type="button" aria-label="複製房間代碼" @click="copyRoomCode">
              {{ copied ? '已複製' : '複製' }}
            </button>
          </div>
          <h2>{{ roomName || '未命名房間' }}</h2>
          <p>{{ roomModeLabel }} · {{ roomAccess === 'private' ? '僅限代碼加入' : '公開配對' }}</p>

          <div class="seats">
            <div class="seat ready">
              <span class="avatar">{{ playerInitial }}</span>
              <div><strong>{{ displayName }}</strong><small>房主</small></div>
              <i>已就緒</i>
            </div>
            <div class="seat">
              <span class="avatar empty">?</span>
              <div><strong>玩家 Bob</strong><small>本機對手</small></div>
              <i>已就緒</i>
            </div>
          </div>

          <button class="primary-button start-button" type="button" @click="createRoom">
            建立並開始對戰 <span>→</span>
          </button>
          <p class="preview-note">目前使用本機規則引擎建立雙人示範對局。</p>
        </aside>
      </div>

      <section v-else class="join-card">
        <span class="step-number">+</span>
        <h2>輸入房間代碼</h2>
        <p class="muted">線上多人連線將在 Durable Object 房間服務完成後開放。</p>
        <input class="code-input" type="text" value="" placeholder="例如 WUX-8K2" disabled>
        <button class="primary-button" type="button" disabled>加入房間</button>
      </section>
    </main>

    <main v-else class="game-page">
      <section class="game-statusbar">
        <div>
          <button class="back-button" type="button" @click="leaveGame">←</button>
          <div>
            <p>{{ activeRoomName }}</p>
            <span>房號 {{ roomCode }} · 第 {{ state.turnNumber }} 回合</span>
          </div>
        </div>
        <div class="turn-indicator">
          <span>目前行動</span>
          <strong>{{ playerLabel(state.currentPlayer) }}</strong>
          <i>{{ phaseLabel(state.phase) }}</i>
        </div>
        <div class="viewer-switch" aria-label="切換觀看者">
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
      </section>

      <div class="battle-layout">
        <section class="battlefield" aria-label="五行戰牌對戰桌">
          <div class="opponent-zone">
            <div class="player-identity opponent">
              <span class="avatar">B</span>
              <div><strong>玩家 Bob</strong><small>{{ teamHp('team:bob') }} HP</small></div>
              <span v-if="state.currentPlayer === 'bob'" class="turn-badge">行動中</span>
            </div>
            <div class="hand fan opponent-hand">
              <button
                v-for="card in cardsFor('bob')"
                :key="card.id"
                type="button"
                class="playing-card"
                :class="{ hidden: card.hidden, selected: card.selected }"
                :disabled="!card.selectable"
                :aria-label="card.hidden ? undefined : card.label"
                :aria-hidden="card.hidden"
                @click="game.toggleCardSelection('bob', card.cardId)"
              >
                <span v-if="!card.hidden">{{ card.label }}</span>
                <span v-else class="card-back-mark">五</span>
              </button>
            </div>
          </div>

          <div class="board-center">
            <div class="deck-pile">
              <span>牌庫</span>
              <strong>五</strong>
            </div>
            <div class="formation-field">
              <span>陣法區</span>
              <div v-if="state.coveredPassives.length" class="covered-card">伏</div>
              <p v-else>等待陣法發動</p>
            </div>
            <div class="discard-pile">
              <span>棄牌</span>
              <strong>{{ state.discard.length }}</strong>
            </div>
          </div>

          <div class="player-zone">
            <div class="hand fan player-hand">
              <button
                v-for="card in cardsFor('alice')"
                :key="card.id"
                type="button"
                class="playing-card"
                :class="[
                  { hidden: card.hidden, selected: card.selected },
                  elementClass(card.label),
                ]"
                :disabled="!card.selectable"
                :aria-label="card.hidden ? undefined : card.label"
                :aria-hidden="card.hidden"
                @click="game.toggleCardSelection('alice', card.cardId)"
              >
                <span v-if="!card.hidden" class="card-level">{{ cardLevel(card.label) }}</span>
                <span v-if="!card.hidden" class="card-element">{{ cardElement(card.label) }}</span>
                <span v-if="!card.hidden" class="card-name">{{ cardName(card.label) }}</span>
                <span v-if="card.hidden" class="card-back-mark">五</span>
              </button>
            </div>
            <div class="player-identity">
              <span class="avatar">{{ playerInitial }}</span>
              <div><strong>{{ displayName }}</strong><small>{{ teamHp('team:alice') }} HP</small></div>
              <span v-if="state.currentPlayer === 'alice'" class="turn-badge">行動中</span>
            </div>
          </div>

          <div v-if="state.pendingChoice" class="choice-overlay">
            <div>
              <p class="section-kicker">ACTION REQUIRED</p>
              <h2>{{ choiceLabel(state.pendingChoice.kind) }}</h2>
              <div class="choice-cards">
                <button
                  v-for="card in state.pendingChoice.cards"
                  :key="card.id"
                  type="button"
                  :disabled="game.isLoading.value || viewer !== state.pendingChoice.player"
                  @click="game.choosePendingCard(card.id)"
                >
                  {{ card.label }}
                </button>
              </div>
            </div>
          </div>
        </section>

        <aside class="game-sidebar">
          <section class="action-panel">
            <div class="panel-heading">
              <div><span>你的行動</span><strong>{{ selectedHint }}</strong></div>
              <span class="selection-count">{{ game.selectedCards.value.length }} / 5</span>
            </div>

            <p v-if="game.errorMessage.value" class="form-error">{{ game.errorMessage.value }}</p>

            <div v-if="game.playableFormations.value.length" class="available-formations">
              <button
                v-for="formation in game.playableFormations.value"
                :key="formation.id"
                type="button"
                :disabled="game.isLoading.value"
                @click="game.performFormation(formation)"
              >
                <span :class="formation.category === 'Attack' ? 'attack' : 'spell'">
                  {{ formation.category === 'Attack' ? '攻' : '術' }}
                </span>
                <div><strong>{{ formation.name }}</strong><small>{{ formation.summary }}</small></div>
                <i>發動 →</i>
              </button>
            </div>
            <div v-else class="empty-action">
              <span>◇</span>
              <p>{{ viewer === state.currentPlayer ? '選擇手牌以尋找可用陣法' : '等待目前玩家完成行動' }}</p>
            </div>

            <div class="utility-actions">
              <button type="button" :disabled="game.isLoading.value" @click="game.advanceAutomatic()">
                推進階段
              </button>
              <button type="button" :disabled="game.isLoading.value" @click="game.passAction()">
                跳過行動
              </button>
            </div>
          </section>

          <section class="event-panel">
            <div class="panel-title">
              <h2>戰局紀錄</h2>
              <span>LIVE</span>
            </div>
            <ol class="event-feed">
              <li v-for="event in visibleEvents" :key="event.id">
                <i />
                <div>
                  <span>{{ eventTypeLabel(event.eventType) }}</span>
                  <p>{{ event.summary }}</p>
                </div>
              </li>
            </ol>
          </section>

          <section class="zone-summary">
            <div><span>護盾</span><strong>{{ state.shields.length }}</strong></div>
            <div><span>狀態</span><strong>{{ state.statuses.length }}</strong></div>
            <div><span>伏牌</span><strong>{{ state.coveredPassives.length }}</strong></div>
          </section>
        </aside>
      </div>
    </main>
  </div>
</template>

<script setup lang="ts">
import type { PlayerId, TeamId, ViewerId } from '~/types/fewfc'
import { authClient } from '~/lib/auth-client'

type Screen = 'login' | 'lobby' | 'game'

const viewers: ViewerId[] = ['alice', 'bob', 'observer']
const screen = ref<Screen>('login')
const displayName = ref('玩家 Alice')
const nameInput = ref('')
const emailInput = ref('')
const passwordInput = ref('')
const authMode = ref<'sign-in' | 'sign-up'>('sign-in')
const authBusy = ref(false)
const loginError = ref('')
const profileOpen = ref(false)
const lobbyTab = ref<'create' | 'join'>('create')
const roomName = ref('五行練習場')
const roomMode = ref('duel')
const roomAccess = ref<'private' | 'public'>('private')
const roomCode = ref('WUX-8K2')
const copied = ref(false)
const activeRoomName = ref('')
const viewer = ref<ViewerId>('alice')
const game = useLocalGame(viewer)
const state = game.state

const modes = [
  { id: 'duel', icon: '⚔', label: '雙人對戰', description: '1 對 1 經典規則', disabled: false },
  { id: 'team', icon: '隊', label: '團隊對戰', description: '2 對 2 輪流行動', disabled: true },
]

const visibleEvents = computed(() => game.publicEvents.value.slice(0, 7))
const playerInitial = computed(() => displayName.value.trim().charAt(0).toUpperCase() || 'A')
const roomModeLabel = computed(() => modes.find((mode) => mode.id === roomMode.value)?.label ?? '')
const selectedHint = computed(() => {
  if (game.selectedCards.value.length) {
    return `已選擇 ${game.selectedCards.value.length} 張牌`
  }
  return viewer.value === state.value.currentPlayer ? '請選擇手牌' : '等待對手'
})

interface CardToken {
  id: string
  cardId: number
  label: string
  hidden: boolean
  selectable: boolean
  selected: boolean
}

async function login() {
  if (!emailInput.value || !passwordInput.value) {
    loginError.value = '請輸入 Email 與密碼'
    return
  }

  if (authMode.value === 'sign-up' && !nameInput.value) {
    loginError.value = '請輸入玩家名稱'
    return
  }

  authBusy.value = true
  loginError.value = ''

  try {
    const result = authMode.value === 'sign-in'
      ? await authClient.signIn.email({
          email: emailInput.value,
          password: passwordInput.value,
          rememberMe: true,
        })
      : await authClient.signUp.email({
          name: nameInput.value,
          email: emailInput.value,
          password: passwordInput.value,
        })

    if (result.error) {
      loginError.value = result.error.message || '無法完成登入'
      return
    }

    const session = await authClient.getSession()
    displayName.value = session.data?.user.name || nameInput.value || '玩家'
    screen.value = 'lobby'
  } catch {
    loginError.value = '帳號服務目前無法使用'
  } finally {
    authBusy.value = false
  }
}

async function guestLogin() {
  authBusy.value = true
  loginError.value = ''

  try {
    const result = await authClient.signIn.anonymous()

    if (result.error) {
      loginError.value = result.error.message || '無法建立訪客身份'
      return
    }

    const session = await authClient.getSession()
    displayName.value = session.data?.user.name || '旅人'
    screen.value = 'lobby'
  } catch {
    loginError.value = '帳號服務目前無法使用'
  } finally {
    authBusy.value = false
  }
}

async function logout() {
  await authClient.signOut()
  profileOpen.value = false
  screen.value = 'login'
  nameInput.value = ''
  emailInput.value = ''
  passwordInput.value = ''
}

function toggleAuthMode() {
  authMode.value = authMode.value === 'sign-in' ? 'sign-up' : 'sign-in'
  loginError.value = ''
}

function goHome() {
  if (screen.value === 'game') {
    screen.value = 'lobby'
  }
}

async function createRoom() {
  activeRoomName.value = roomName.value || '未命名房間'
  viewer.value = 'alice'
  await game.startSampleGame()
  screen.value = 'game'
}

function leaveGame() {
  screen.value = 'lobby'
}

async function copyRoomCode() {
  await navigator.clipboard?.writeText(roomCode.value)
  copied.value = true
  window.setTimeout(() => {
    copied.value = false
  }, 1400)
}

onMounted(async () => {
  const session = await authClient.getSession()

  if (session.data?.user) {
    displayName.value = session.data.user.name
    screen.value = 'lobby'
  }
})

function cardsFor(player: PlayerId): CardToken[] {
  const hand = state.value.hands.find((entry) => entry.player === player)
  if (!hand) return []

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

function teamHp(team: TeamId): number {
  return state.value.hp.find((entry) => entry.team === team)?.hp ?? 0
}

function viewerLabel(value: ViewerId): string {
  if (value === 'observer') return '觀戰'
  if (value === 'alice') return displayName.value
  return '玩家 Bob'
}

function playerLabel(value: PlayerId | null): string {
  if (!value) return '準備開始'
  return value === 'alice' ? displayName.value : '玩家 Bob'
}

function phaseLabel(value: string): string {
  const labels: Record<string, string> = {
    Main: '主要階段',
    MainPhase: '主要階段',
    TurnStart: '回合開始',
    TurnDraw: '回合抽牌',
    TurnDrawDiscardChoice: '棄牌選擇',
    TurnEnd: '回合結束',
  }
  return labels[value] ?? value
}

function choiceLabel(value: string): string {
  const labels: Record<string, string> = {
    'Choose one drawn card to discard': '選擇一張本回合抽到的牌棄置',
    TurnDrawDiscard: '選擇一張本回合抽到的牌棄置',
    EffectGenerated: '選擇效果指定的牌',
    Hidden: '等待隱藏選擇',
  }
  return labels[value] ?? value
}

function eventTypeLabel(value: string): string {
  const labels: Record<string, string> = {
    ActionPassed: '跳過行動',
    AutomaticAdvance: '階段推進',
    CardsDealt: '初始發牌',
    FormationPerformed: '陣法發動',
    TurnStarted: '回合開始',
  }
  return labels[value] ?? value
}

function elementClass(label: string): string {
  const value = cardElement(label)
  return value ? `element-${value}` : ''
}

function cardElement(label: string): string {
  return ['金', '木', '水', '火', '土'].find((element) => label.includes(element)) ?? ''
}

function cardLevel(label: string): string {
  return label.match(/\d+/)?.[0] ?? '◆'
}

function cardName(label: string): string {
  return label.replace(/\d+/g, '').replace(/[金木水火土]/g, '').trim() || label
}
</script>

<style>
@reference "./assets/css/main.css";

.app-shell { @apply min-h-screen bg-ink; }
.site-header {
  @apply relative z-20 flex h-18 items-center justify-between border-b border-[#29322d] bg-[rgba(14,19,16,.96)];
  padding-inline: clamp(20px, 4vw, 64px);
}
.screen-login .site-header { @apply absolute w-full border-0 bg-transparent; }
.brand { @apply flex items-center gap-3 border-0 bg-transparent p-0; }
.brand-mark {
  @apply grid size-[38px] rotate-45 place-items-center border border-[#d6af5d] font-serif font-black text-[#e4c47d];
}
.brand-mark::first-letter { transform: rotate(-45deg); }
.brand > span:last-child { @apply grid text-left; }
.brand strong { @apply font-serif tracking-[.12em]; }
.brand small { @apply text-[8px] tracking-[.38em] text-[#8b948e]; }
.header-actions { @apply relative flex items-center gap-[18px]; }
.connection { @apply text-xs text-[#98a39c]; }
.connection i { display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: #62b585; margin-right: 6px; box-shadow: 0 0 8px #62b585; }
.profile-button { @apply flex items-center gap-2 border-0 bg-transparent; }
.avatar { @apply grid size-[34px] place-items-center rounded-full bg-[#b48a47] font-extrabold text-[#141813]; }
.profile-menu { @apply absolute right-0 top-12 min-w-30 border border-[#39443d] bg-[#202822] p-1.5; }
.profile-menu button { @apply w-full border-0 bg-transparent px-3 py-2 text-left; }

.login-layout { @apply grid min-h-screen grid-cols-[1.15fr_.85fr] max-[900px]:grid-cols-1; }
.login-hero {
  @apply relative flex min-h-screen flex-col justify-between overflow-hidden max-[900px]:hidden;
  padding: 140px clamp(40px, 8vw, 130px) 34px;
  background: radial-gradient(circle at 68% 58%, rgba(157, 118, 49, .18), transparent 25%), linear-gradient(145deg, #16201a 0%, #0d120f 68%);
}
.login-hero::before {
  content: ""; position: absolute; inset: 0; opacity: .14;
  background-image: linear-gradient(30deg, #768278 1px, transparent 1px), linear-gradient(150deg, #768278 1px, transparent 1px);
  background-size: 72px 126px;
}
.hero-copy, .hero-footer { @apply relative z-1; }
.kicker, .section-kicker { @apply text-[11px] font-bold tracking-[.28em] text-gold; }
.kicker span { display: inline-block; width: 32px; height: 1px; background: #c6a35e; vertical-align: middle; margin-right: 12px; }
.hero-copy h1 { @apply my-[22px] font-serif leading-[1.22] tracking-[.04em]; font-size: clamp(48px, 5.8vw, 88px); }
.hero-copy h1 em { @apply not-italic text-[#d1ad62]; }
.hero-description { @apply max-w-[510px] text-sm leading-[1.95] text-[#a7b0a9]; }
.hero-footer { @apply flex justify-between text-[11px] text-[#687169]; }
.element-orbit { position: absolute; width: 460px; height: 460px; border: 1px solid rgba(195, 157, 87, .18); border-radius: 50%; left: 55%; top: 72%; transform: translate(-50%, -50%); }
.element-orbit::after { content: ""; position: absolute; inset: 50px; border: 1px dashed rgba(195, 157, 87, .14); border-radius: 50%; }
.orbit-core { position: absolute; inset: 50%; width: 86px; height: 86px; margin: -43px; border: 1px solid #826b3f; border-radius: 50%; display: grid; place-items: center; color: #d4b46e; font-family: "Noto Serif TC", serif; font-size: 22px; background: #131a16; }
.element { position: absolute; width: 54px; height: 54px; border-radius: 50%; display: grid; place-items: center; font-family: "Noto Serif TC", serif; border: 1px solid currentColor; background: #151c18; }
.element.metal { color: #ded5ba; left: 203px; top: -27px; }
.element.wood { color: #6ba77b; right: 7px; top: 120px; }
.element.water { color: #6197af; right: 58px; bottom: 35px; }
.element.fire { color: #c76655; left: 58px; bottom: 35px; }
.element.earth { color: #bd9656; left: 7px; top: 120px; }

.login-panel { @apply grid place-items-center bg-[#f1eee5] px-[30px] pt-20 pb-[30px] text-[#18201c] max-[900px]:min-h-screen; }
.auth-card { @apply w-full max-w-100; }
.mobile-brand { @apply hidden max-[900px]:mb-15 max-[900px]:flex max-[900px]:items-center max-[900px]:gap-[15px] max-[900px]:font-serif max-[900px]:font-extrabold; }
.auth-card h2, .lobby-heading h1 { @apply my-2.5 mb-2 font-serif text-4xl; }
.muted { @apply text-[13px] leading-[1.7] text-[#707872]; }
.auth-card form { @apply mt-[30px]; }
.auth-card label, .setup-card > label, fieldset legend { @apply mb-[9px] block text-xs font-bold; }
.auth-card form label:not(:first-child) { @apply mt-3.5; }
.input-wrap { @apply flex h-13 items-center gap-3 border border-[#c7c8c0] bg-[#faf9f5] px-4; }
.input-wrap span { @apply font-serif text-[#9a8151]; }
.input-wrap input, .text-input, .code-input { @apply w-full border-0 bg-transparent text-[#18201c] outline-0; }
.input-wrap:focus-within, .text-input:focus { border-color: #a57d35; box-shadow: 0 0 0 2px rgba(165, 125, 53, .12); }
.primary-button {
  @apply flex min-h-[50px] items-center justify-between border border-[#b99550] px-[22px] font-bold text-white hover:brightness-[1.08];
  background: linear-gradient(135deg, #b58c43, #8f6a2d);
}
.login-button { @apply mt-4 w-full; }
.auth-mode-button { @apply mt-[15px] w-full border-0 bg-transparent text-[11px] text-[#82672f]; }
.divider { @apply my-8 h-px bg-[#d2d1ca] text-center; }
.divider span { position: relative; top: -10px; background: #f1eee5; padding: 0 16px; color: #969b96; font-size: 11px; }
.ghost-button { @apply min-h-12 w-full border border-[#bcbdb7] bg-transparent text-[#323a35]; }
.terms { @apply mt-6 text-center text-[10px] text-[#9a9e9a]; }
.form-error { @apply mt-2 text-xs text-[#c84d45]; }

.lobby-page { @apply mx-auto max-w-[1180px] px-[30px] pt-15 pb-[90px] max-[600px]:px-4 max-[600px]:py-9; }
.lobby-heading { @apply mb-[38px] flex items-end justify-between max-[900px]:flex-col max-[900px]:items-start max-[900px]:gap-6; }
.lobby-tabs { @apply flex border-b border-[#38423c]; }
.lobby-tabs button { @apply border-0 border-b-2 border-transparent bg-transparent px-6 py-3 text-[#78827b]; }
.lobby-tabs button.active { @apply border-gold text-gold-light; }
.lobby-grid { @apply grid grid-cols-[1.25fr_.75fr] gap-[22px] max-[900px]:grid-cols-1; }
.setup-card, .room-preview, .join-card { @apply border border-line bg-panel p-8 max-[600px]:px-[18px] max-[600px]:py-[22px]; }
.card-heading { @apply mb-8 flex gap-[18px]; }
.step-number { @apply grid size-[42px] place-items-center border border-[#7e693e] font-serif text-[#d3ae62]; }
.card-heading h2, .room-preview h2, .join-card h2 { @apply mb-1 font-serif text-[21px]; }
.card-heading p, .room-preview > p { @apply text-xs text-muted; }
.text-input { @apply mb-[26px] h-12 border border-[#39443d] bg-[#111713] px-3.5 text-[#ece8dd]; }
fieldset { @apply mb-[26px] border-0 p-0; }
.option-grid { @apply grid grid-cols-2 gap-3 max-[600px]:grid-cols-1; }
.mode-option { @apply relative grid min-h-27 grid-cols-[42px_1fr] border border-[#354039] bg-[#121814] p-4 text-left text-[#d5d8d4]; }
.mode-option.selected { border-color: #b99550; background: #1d2118; box-shadow: inset 0 0 0 1px #b99550; }
.mode-icon { @apply row-span-2 grid size-8 place-items-center rounded-full bg-[#2b3027] text-[#d8b569]; }
.mode-option small { @apply text-[#758078]; }
.mode-option i { @apply absolute top-2 right-2 text-[9px] not-italic text-[#8a918c]; }
.segmented { @apply grid grid-cols-2 bg-[#111713] p-1; }
.segmented button { @apply min-h-10 border-0 bg-transparent text-muted; }
.segmented button.active { @apply bg-[#293128] text-[#e1c47f]; }
.room-preview { @apply text-center; background: linear-gradient(160deg, #1b241e, #121814); }
.preview-topline { @apply flex justify-between text-left text-[11px] text-[#818b84]; }
.status-pill { @apply rounded-[20px] border border-[#3a6547] px-2 py-[3px] text-[#80be92]; }
.room-emblem { width: 84px; height: 84px; border: 1px solid #8f733d; border-radius: 50%; margin: 30px auto 16px; display: grid; place-items: center; background: radial-gradient(circle, #36331f, #151b17 67%); }
.room-emblem span { font-family: "Noto Serif TC", serif; font-size: 30px; color: #d7b66d; }
.room-code-label { letter-spacing: .25em; font-size: 9px !important; }
.room-code { @apply mt-1 mb-5 flex items-center justify-center gap-3; }
.room-code strong { @apply text-2xl tracking-[.18em] text-[#e6d59d]; }
.room-code button { @apply border-0 bg-transparent text-[10px] text-[#a48b59]; }
.seats { @apply mt-7 mb-5 grid gap-2 text-left; }
.seat { @apply flex items-center gap-2.5 border border-line bg-[#141a16] p-2.5; }
.seat div { @apply grid flex-1; }
.seat small { @apply text-[10px] text-[#717b74]; }
.seat i { @apply text-[10px] not-italic text-[#6da27c]; }
.avatar.empty { @apply bg-[#252d28] text-[#707b73]; }
.start-button { @apply w-full; }
.preview-note { margin-top: 12px !important; font-size: 10px !important; }
.join-card { max-width: 560px; margin: 40px auto; text-align: center; display: grid; gap: 20px; justify-items: center; }
.code-input { max-width: 320px; height: 52px; border: 1px solid #3a443e; color: white; padding: 0 20px; text-align: center; letter-spacing: .2em; }

.game-page { @apply flex h-[calc(100vh-72px)] flex-col overflow-hidden max-[900px]:h-auto max-[900px]:overflow-visible; }
.game-statusbar { @apply grid min-h-16 grid-cols-[1fr_auto_1fr] items-center border-b border-line bg-panel px-[26px] max-[900px]:grid-cols-[1fr_auto] max-[600px]:px-2.5; }
.game-statusbar > div:first-child { @apply flex items-center gap-3; }
.game-statusbar p { @apply text-[13px] font-bold; }
.game-statusbar span { @apply text-[10px] text-[#7d8780]; }
.back-button { @apply size-8 border border-[#39433d] bg-transparent; }
.turn-indicator { @apply grid border-x border-line px-[38px] py-1 text-center max-[600px]:px-3; }
.turn-indicator strong { @apply text-[13px] text-gold-light; }
.turn-indicator i { @apply text-[9px] not-italic text-[#7e8981]; }
.viewer-switch { @apply flex justify-end max-[900px]:hidden; }
.viewer-switch button { @apply border-0 bg-transparent px-[9px] py-1.5 text-[10px] text-[#69736c]; }
.viewer-switch button.active { @apply bg-[#282d24] text-[#e0c27a]; }
.battle-layout { @apply grid min-h-0 flex-1 grid-cols-[minmax(0,1fr)_330px] max-[900px]:grid-cols-1 max-[900px]:overflow-auto; }
.battlefield { position: relative; min-width: 0; display: grid; grid-template-rows: 1fr 1fr 1.15fr; padding: 20px 48px; overflow: hidden; background: radial-gradient(ellipse at center, #273029 0%, #141b17 58%, #0f1512 100%); }
.battlefield::before { content: ""; position: absolute; inset: 22px; border: 1px solid rgba(175, 143, 79, .18); pointer-events: none; }
.battlefield::after { content: "五 行"; position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); width: 270px; height: 270px; border: 1px solid rgba(183, 148, 77, .1); border-radius: 50%; display: grid; place-items: center; color: rgba(204, 171, 100, .06); font-family: serif; font-size: 70px; pointer-events: none; }
.opponent-zone, .player-zone { @apply z-1 grid items-center max-[600px]:grid-cols-1 max-[600px]:justify-items-center; }
.opponent-zone { grid-template-columns: 210px 1fr; }
.player-zone { grid-template-columns: 1fr 210px; }
.player-identity { @apply flex items-center gap-2.5; }
.player-identity div { @apply grid; }
.player-identity small { @apply text-[11px] text-[#d0a450]; }
.turn-badge { @apply rounded-[20px] border border-[#396147] px-[7px] py-[3px] text-[9px]! text-[#77bd8d]!; }
.hand { @apply flex min-w-0 items-center justify-center; }
.playing-card {
  width: clamp(62px, 7vw, 92px); aspect-ratio: 5 / 7; border: 1px solid #79715e; border-radius: 5px;
  background: linear-gradient(145deg, #e9e1ce, #bcb39e); color: #18201c; margin-left: -10px;
  @apply relative flex flex-col items-center justify-center p-2 transition-[.18s] max-[600px]:w-[58px];
  box-shadow: 0 5px 15px rgba(0,0,0,.35);
}
.playing-card:enabled:hover, .playing-card.selected { transform: translateY(-14px); border-color: #e2bd67; box-shadow: 0 0 0 2px #c9a451, 0 12px 18px rgba(0,0,0,.45); z-index: 5; }
.playing-card.hidden { background: repeating-linear-gradient(45deg, #232e28, #232e28 5px, #344039 5px, #344039 10px); border: 2px solid #85714a; color: #c7a65e; }
.card-back-mark { @apply grid size-[42px] rotate-45 place-items-center border border-[#9b814d] font-serif text-lg; }
.card-level { @apply absolute top-[5px] left-[7px] font-serif text-base font-extrabold; }
.card-element { @apply grid size-[35px] place-items-center rounded-full border border-current font-serif text-lg; }
.card-name { @apply mt-2 max-w-full overflow-hidden text-[9px] font-bold; }
.element-火 .card-element { color: #a43d32; }.element-水 .card-element { color: #357a99; }
.element-木 .card-element { color: #467d51; }.element-金 .card-element { color: #887b55; }.element-土 .card-element { color: #9b6e35; }
.opponent-hand .playing-card { width: clamp(48px, 5vw, 68px); }
.board-center { @apply z-1 grid grid-cols-[120px_1fr_120px] items-center justify-items-center max-[600px]:w-full max-[600px]:grid-cols-[70px_1fr_70px]; }
.deck-pile, .discard-pile { @apply grid justify-items-center gap-1.5 text-[9px] text-[#707b73]; }
.deck-pile strong, .discard-pile strong, .covered-card { @apply grid w-[58px] place-items-center border border-[#665b44] bg-[#18201b] font-serif text-xl text-[#a68d56]; aspect-ratio: 5/7; }
.formation-field { min-width: 220px; min-height: 100px; display: grid; place-items: center; color: #69736c; font-size: 10px; border-left: 1px solid rgba(166, 141, 86, .14); border-right: 1px solid rgba(166, 141, 86, .14); }
.formation-field > span { color: #9a8251; letter-spacing: .2em; }
.choice-overlay { @apply absolute inset-0 z-12 grid place-items-center bg-[rgba(7,10,8,.76)] text-center backdrop-blur-[4px]; }
.choice-overlay > div { @apply min-w-90 border border-[#8e733d] bg-[#18201b] p-[30px]; }
.choice-overlay h2 { @apply mt-2.5 mb-5 font-serif; }
.choice-cards { @apply flex justify-center gap-2; }
.choice-cards button { @apply border border-[#ae8b47] bg-[#ede6d4] p-2.5 text-[#18201c]; }

.game-sidebar { @apply grid min-h-0 grid-rows-[auto_1fr_auto] overflow-hidden border-l border-line bg-panel max-[900px]:border-l-0; }
.action-panel, .event-panel { @apply border-b border-line p-5; }
.panel-heading, .panel-title { @apply flex items-start justify-between; }
.panel-heading div { @apply grid; }
.panel-heading span, .panel-title span { @apply text-[9px] tracking-[.14em] text-[#8a948d]; }
.panel-heading strong { @apply mt-[3px] text-[13px]; }
.selection-count { @apply border border-[#3b463f] px-[7px] py-1; }
.available-formations { @apply mt-3.5 grid gap-[7px]; }
.available-formations button { min-height: 58px; border: 1px solid #3b463f; background: #111713; display: flex; align-items: center; gap: 9px; text-align: left; padding: 8px; }
.available-formations button > span { width: 30px; height: 30px; display: grid; place-items: center; border-radius: 50%; }
.available-formations .attack { color: #d77765; border: 1px solid #7c4037; }
.available-formations .spell { color: #77a7c2; border: 1px solid #3f6275; }
.available-formations button div { display: grid; flex: 1; }
.available-formations small { color: #78827b; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 170px; }
.available-formations i { color: #c8a75f; font-style: normal; font-size: 10px; }
.empty-action { @apply grid min-h-[92px] content-center place-items-center gap-1.5 text-center text-[10px] text-[#667069]; }
.empty-action span { @apply text-[22px] text-[#806c43]; }
.utility-actions { @apply grid grid-cols-2 gap-[7px]; }
.utility-actions button { border: 1px solid #3c463f; background: #222a25; min-height: 36px; font-size: 10px; }
.event-panel { @apply min-h-0 overflow-auto; }
.panel-title h2 { @apply font-serif text-[15px]; }
.panel-title span { @apply text-[#70b585]; }
.event-feed { @apply mt-4 grid list-none gap-[13px] p-0; }
.event-feed li { @apply grid grid-cols-[10px_1fr] gap-[7px]; }
.event-feed li > i { width: 5px; height: 5px; border-radius: 50%; background: #b79550; margin-top: 6px; box-shadow: 0 0 0 4px rgba(183, 149, 80, .08); }
.event-feed span { color: #d4d8d4; font-size: 10px; font-weight: 700; }
.event-feed p { color: #6f7972; font-size: 9px; line-height: 1.45; margin-top: 2px; }
.zone-summary { @apply grid grid-cols-3 p-3; }
.zone-summary div { @apply grid border-r border-line text-center; }
.zone-summary div:last-child { @apply border-0; }
.zone-summary span { @apply text-[9px] text-[#707a73]; }
.zone-summary strong { @apply text-[13px] text-[#cbaa64]; }

@media (max-width: 900px) {
  .login-layout { grid-template-columns: 1fr; }
  .login-hero { display: none; }
  .screen-login .site-header { display: none; }
  .mobile-brand { display: flex; align-items: center; gap: 15px; margin-bottom: 60px; font-family: serif; font-weight: 800; }
  .login-panel { min-height: 100vh; }
  .lobby-grid { grid-template-columns: 1fr; }
  .lobby-heading { align-items: start; gap: 25px; flex-direction: column; }
  .battle-layout { grid-template-columns: 1fr; overflow: auto; }
  .game-page { height: auto; overflow: visible; }
  .battlefield { min-height: 680px; padding: 20px; }
  .game-sidebar { border-left: 0; }
  .game-statusbar { grid-template-columns: 1fr auto; }
  .viewer-switch { display: none; }
}

@media (max-width: 600px) {
  .site-header { padding: 0 16px; }
  .connection, .profile-button > span:nth-child(2) { display: none; }
  .lobby-page { padding: 36px 16px; }
  .setup-card, .room-preview { padding: 22px 18px; }
  .option-grid { grid-template-columns: 1fr; }
  .game-statusbar { padding: 0 10px; }
  .turn-indicator { padding: 4px 12px; }
  .battlefield { grid-template-rows: 1fr .8fr 1fr; }
  .opponent-zone, .player-zone { grid-template-columns: 1fr; justify-items: center; }
  .opponent-zone .player-identity { order: 2; }
  .player-zone .player-identity { order: 2; }
  .board-center { grid-template-columns: 70px 1fr 70px; width: 100%; }
  .formation-field { min-width: 0; width: 100%; }
  .playing-card { width: 58px; }
}
</style>
