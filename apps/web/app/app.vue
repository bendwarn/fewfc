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
                :aria-label="card.hidden ? '隱藏牌' : card.label"
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
                :aria-label="card.hidden ? '隱藏牌' : card.label"
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
@import url('https://fonts.googleapis.com/css2?family=Noto+Sans+TC:wght@400;500;600;700;800&family=Noto+Serif+TC:wght@700;900&display=swap');

:root {
  color: #e8e4d8;
  background: #101512;
  font-family: "Noto Sans TC", ui-sans-serif, system-ui, sans-serif;
  font-synthesis: none;
}

* { box-sizing: border-box; }
body { margin: 0; min-width: 320px; background: #101512; }
button, input { font: inherit; }
button { color: inherit; cursor: pointer; }
button:disabled { cursor: not-allowed; opacity: .45; }
h1, h2, h3, p { margin: 0; }

.app-shell { min-height: 100vh; background: #101512; }
.site-header {
  height: 72px; padding: 0 clamp(20px, 4vw, 64px); display: flex; align-items: center;
  justify-content: space-between; border-bottom: 1px solid #29322d; background: rgba(14, 19, 16, .96);
  position: relative; z-index: 20;
}
.screen-login .site-header { position: absolute; width: 100%; background: transparent; border: 0; }
.brand { display: flex; align-items: center; gap: 12px; border: 0; background: transparent; padding: 0; }
.brand-mark {
  width: 38px; height: 38px; border: 1px solid #d6af5d; color: #e4c47d; display: grid;
  place-items: center; transform: rotate(45deg); font-family: "Noto Serif TC", serif; font-weight: 900;
}
.brand-mark::first-letter { transform: rotate(-45deg); }
.brand > span:last-child { text-align: left; display: grid; }
.brand strong { font-family: "Noto Serif TC", serif; letter-spacing: .12em; }
.brand small { color: #8b948e; font-size: 8px; letter-spacing: .38em; }
.header-actions { display: flex; align-items: center; gap: 18px; position: relative; }
.connection { color: #98a39c; font-size: 12px; }
.connection i { display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: #62b585; margin-right: 6px; box-shadow: 0 0 8px #62b585; }
.profile-button { background: transparent; border: 0; display: flex; align-items: center; gap: 8px; }
.avatar { width: 34px; height: 34px; border-radius: 50%; display: grid; place-items: center; background: #b48a47; color: #141813; font-weight: 800; }
.profile-menu { position: absolute; right: 0; top: 48px; background: #202822; border: 1px solid #39443d; padding: 6px; min-width: 120px; }
.profile-menu button { width: 100%; background: transparent; border: 0; text-align: left; padding: 8px 12px; }

.login-layout { min-height: 100vh; display: grid; grid-template-columns: 1.15fr .85fr; }
.login-hero {
  min-height: 100vh; padding: 140px clamp(40px, 8vw, 130px) 34px; display: flex; flex-direction: column;
  justify-content: space-between; overflow: hidden; position: relative;
  background: radial-gradient(circle at 68% 58%, rgba(157, 118, 49, .18), transparent 25%), linear-gradient(145deg, #16201a 0%, #0d120f 68%);
}
.login-hero::before {
  content: ""; position: absolute; inset: 0; opacity: .14;
  background-image: linear-gradient(30deg, #768278 1px, transparent 1px), linear-gradient(150deg, #768278 1px, transparent 1px);
  background-size: 72px 126px;
}
.hero-copy, .hero-footer { position: relative; z-index: 1; }
.kicker, .section-kicker { color: #c6a35e; font-size: 11px; letter-spacing: .28em; font-weight: 700; }
.kicker span { display: inline-block; width: 32px; height: 1px; background: #c6a35e; vertical-align: middle; margin-right: 12px; }
.hero-copy h1 { font-family: "Noto Serif TC", serif; font-size: clamp(48px, 5.8vw, 88px); line-height: 1.22; margin: 22px 0; letter-spacing: .04em; }
.hero-copy h1 em { color: #d1ad62; font-style: normal; }
.hero-description { color: #a7b0a9; line-height: 1.95; max-width: 510px; font-size: 14px; }
.hero-footer { display: flex; justify-content: space-between; color: #687169; font-size: 11px; }
.element-orbit { position: absolute; width: 460px; height: 460px; border: 1px solid rgba(195, 157, 87, .18); border-radius: 50%; left: 55%; top: 72%; transform: translate(-50%, -50%); }
.element-orbit::after { content: ""; position: absolute; inset: 50px; border: 1px dashed rgba(195, 157, 87, .14); border-radius: 50%; }
.orbit-core { position: absolute; inset: 50%; width: 86px; height: 86px; margin: -43px; border: 1px solid #826b3f; border-radius: 50%; display: grid; place-items: center; color: #d4b46e; font-family: "Noto Serif TC", serif; font-size: 22px; background: #131a16; }
.element { position: absolute; width: 54px; height: 54px; border-radius: 50%; display: grid; place-items: center; font-family: "Noto Serif TC", serif; border: 1px solid currentColor; background: #151c18; }
.element.metal { color: #ded5ba; left: 203px; top: -27px; }
.element.wood { color: #6ba77b; right: 7px; top: 120px; }
.element.water { color: #6197af; right: 58px; bottom: 35px; }
.element.fire { color: #c76655; left: 58px; bottom: 35px; }
.element.earth { color: #bd9656; left: 7px; top: 120px; }

.login-panel { display: grid; place-items: center; background: #f1eee5; color: #18201c; padding: 80px 30px 30px; }
.auth-card { width: min(400px, 100%); }
.mobile-brand { display: none; }
.auth-card h2, .lobby-heading h1 { font-family: "Noto Serif TC", serif; font-size: 36px; margin: 10px 0 8px; }
.muted { color: #707872; font-size: 13px; line-height: 1.7; }
.auth-card form { margin-top: 30px; }
.auth-card label, .setup-card > label, fieldset legend { display: block; font-size: 12px; font-weight: 700; margin-bottom: 9px; }
.auth-card form label:not(:first-child) { margin-top: 14px; }
.input-wrap { border: 1px solid #c7c8c0; background: #faf9f5; height: 52px; display: flex; align-items: center; padding: 0 16px; gap: 12px; }
.input-wrap span { color: #9a8151; font-family: "Noto Serif TC", serif; }
.input-wrap input, .text-input, .code-input { border: 0; outline: 0; background: transparent; width: 100%; color: #18201c; }
.input-wrap:focus-within, .text-input:focus { border-color: #a57d35; box-shadow: 0 0 0 2px rgba(165, 125, 53, .12); }
.primary-button {
  min-height: 50px; border: 1px solid #b99550; background: linear-gradient(135deg, #b58c43, #8f6a2d);
  color: #fff; font-weight: 700; padding: 0 22px; display: flex; align-items: center; justify-content: space-between;
}
.login-button { width: 100%; margin-top: 16px; }
.primary-button:hover { filter: brightness(1.08); }
.auth-mode-button { width: 100%; border: 0; background: transparent; color: #82672f; font-size: 11px; margin-top: 15px; }
.divider { height: 1px; background: #d2d1ca; margin: 32px 0; text-align: center; }
.divider span { position: relative; top: -10px; background: #f1eee5; padding: 0 16px; color: #969b96; font-size: 11px; }
.ghost-button { width: 100%; min-height: 48px; background: transparent; border: 1px solid #bcbdb7; color: #323a35; }
.terms { color: #9a9e9a; font-size: 10px; text-align: center; margin-top: 24px; }
.form-error { color: #c84d45; font-size: 12px; margin-top: 8px; }

.lobby-page { max-width: 1180px; margin: 0 auto; padding: 60px 30px 90px; }
.lobby-heading { display: flex; align-items: flex-end; justify-content: space-between; margin-bottom: 38px; }
.lobby-tabs { display: flex; border-bottom: 1px solid #38423c; }
.lobby-tabs button { border: 0; background: transparent; color: #78827b; padding: 12px 24px; border-bottom: 2px solid transparent; }
.lobby-tabs button.active { color: #ddc17f; border-color: #c6a35e; }
.lobby-grid { display: grid; grid-template-columns: 1.25fr .75fr; gap: 22px; }
.setup-card, .room-preview, .join-card { border: 1px solid #303a34; background: #171e1a; padding: 32px; }
.card-heading { display: flex; gap: 18px; margin-bottom: 32px; }
.step-number { width: 42px; height: 42px; display: grid; place-items: center; border: 1px solid #7e693e; color: #d3ae62; font-family: serif; }
.card-heading h2, .room-preview h2, .join-card h2 { font-family: "Noto Serif TC", serif; font-size: 21px; margin-bottom: 4px; }
.card-heading p, .room-preview > p { color: #7f8982; font-size: 12px; }
.text-input { height: 48px; border: 1px solid #39443d; padding: 0 14px; color: #ece8dd; background: #111713; margin-bottom: 26px; }
fieldset { border: 0; padding: 0; margin: 0 0 26px; }
.option-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 12px; }
.mode-option { border: 1px solid #354039; background: #121814; min-height: 108px; color: #d5d8d4; padding: 16px; display: grid; grid-template-columns: 42px 1fr; text-align: left; position: relative; }
.mode-option.selected { border-color: #b99550; background: #1d2118; box-shadow: inset 0 0 0 1px #b99550; }
.mode-icon { grid-row: 1 / span 2; width: 32px; height: 32px; border-radius: 50%; display: grid; place-items: center; background: #2b3027; color: #d8b569; }
.mode-option small { color: #758078; }
.mode-option i { position: absolute; right: 8px; top: 8px; font-style: normal; font-size: 9px; color: #8a918c; }
.segmented { display: grid; grid-template-columns: 1fr 1fr; background: #111713; padding: 4px; }
.segmented button { background: transparent; color: #7f8982; border: 0; min-height: 40px; }
.segmented button.active { background: #293128; color: #e1c47f; }
.room-preview { background: linear-gradient(160deg, #1b241e, #121814); text-align: center; }
.preview-topline { display: flex; justify-content: space-between; text-align: left; color: #818b84; font-size: 11px; }
.status-pill { color: #80be92; border: 1px solid #3a6547; padding: 3px 8px; border-radius: 20px; }
.room-emblem { width: 84px; height: 84px; border: 1px solid #8f733d; border-radius: 50%; margin: 30px auto 16px; display: grid; place-items: center; background: radial-gradient(circle, #36331f, #151b17 67%); }
.room-emblem span { font-family: "Noto Serif TC", serif; font-size: 30px; color: #d7b66d; }
.room-code-label { letter-spacing: .25em; font-size: 9px !important; }
.room-code { display: flex; align-items: center; justify-content: center; gap: 12px; margin: 4px 0 20px; }
.room-code strong { font-size: 24px; letter-spacing: .18em; color: #e6d59d; }
.room-code button { background: transparent; border: 0; color: #a48b59; font-size: 10px; }
.seats { display: grid; gap: 8px; margin: 28px 0 20px; text-align: left; }
.seat { display: flex; align-items: center; gap: 10px; border: 1px solid #303a34; padding: 10px; background: #141a16; }
.seat div { display: grid; flex: 1; }
.seat small { color: #717b74; font-size: 10px; }
.seat i { font-style: normal; color: #6da27c; font-size: 10px; }
.avatar.empty { background: #252d28; color: #707b73; }
.start-button { width: 100%; }
.preview-note { margin-top: 12px !important; font-size: 10px !important; }
.join-card { max-width: 560px; margin: 40px auto; text-align: center; display: grid; gap: 20px; justify-items: center; }
.code-input { max-width: 320px; height: 52px; border: 1px solid #3a443e; color: white; padding: 0 20px; text-align: center; letter-spacing: .2em; }

.game-page { height: calc(100vh - 72px); overflow: hidden; display: flex; flex-direction: column; }
.game-statusbar { min-height: 64px; display: grid; grid-template-columns: 1fr auto 1fr; align-items: center; border-bottom: 1px solid #303a34; padding: 0 26px; background: #171e1a; }
.game-statusbar > div:first-child { display: flex; align-items: center; gap: 12px; }
.game-statusbar p { font-size: 13px; font-weight: 700; }
.game-statusbar span { color: #7d8780; font-size: 10px; }
.back-button { width: 32px; height: 32px; border: 1px solid #39433d; background: transparent; }
.turn-indicator { text-align: center; display: grid; padding: 4px 38px; border-left: 1px solid #303a34; border-right: 1px solid #303a34; }
.turn-indicator strong { color: #ddc17f; font-size: 13px; }
.turn-indicator i { color: #7e8981; font-size: 9px; font-style: normal; }
.viewer-switch { display: flex; justify-content: flex-end; }
.viewer-switch button { border: 0; background: transparent; color: #69736c; padding: 6px 9px; font-size: 10px; }
.viewer-switch button.active { color: #e0c27a; background: #282d24; }
.battle-layout { min-height: 0; flex: 1; display: grid; grid-template-columns: minmax(0, 1fr) 330px; }
.battlefield { position: relative; min-width: 0; display: grid; grid-template-rows: 1fr 1fr 1.15fr; padding: 20px 48px; overflow: hidden; background: radial-gradient(ellipse at center, #273029 0%, #141b17 58%, #0f1512 100%); }
.battlefield::before { content: ""; position: absolute; inset: 22px; border: 1px solid rgba(175, 143, 79, .18); pointer-events: none; }
.battlefield::after { content: "五 行"; position: absolute; left: 50%; top: 50%; transform: translate(-50%, -50%); width: 270px; height: 270px; border: 1px solid rgba(183, 148, 77, .1); border-radius: 50%; display: grid; place-items: center; color: rgba(204, 171, 100, .06); font-family: serif; font-size: 70px; pointer-events: none; }
.opponent-zone, .player-zone { z-index: 1; display: grid; align-items: center; }
.opponent-zone { grid-template-columns: 210px 1fr; }
.player-zone { grid-template-columns: 1fr 210px; }
.player-identity { display: flex; align-items: center; gap: 10px; }
.player-identity div { display: grid; }
.player-identity small { color: #d0a450; font-size: 11px; }
.turn-badge { color: #77bd8d !important; font-size: 9px !important; border: 1px solid #396147; border-radius: 20px; padding: 3px 7px; }
.hand { display: flex; justify-content: center; align-items: center; min-width: 0; }
.playing-card {
  width: clamp(62px, 7vw, 92px); aspect-ratio: 5 / 7; border: 1px solid #79715e; border-radius: 5px;
  background: linear-gradient(145deg, #e9e1ce, #bcb39e); color: #18201c; margin-left: -10px;
  position: relative; display: flex; flex-direction: column; align-items: center; justify-content: center; transition: .18s ease;
  box-shadow: 0 5px 15px rgba(0,0,0,.35); padding: 8px;
}
.playing-card:enabled:hover, .playing-card.selected { transform: translateY(-14px); border-color: #e2bd67; box-shadow: 0 0 0 2px #c9a451, 0 12px 18px rgba(0,0,0,.45); z-index: 5; }
.playing-card.hidden { background: repeating-linear-gradient(45deg, #232e28, #232e28 5px, #344039 5px, #344039 10px); border: 2px solid #85714a; color: #c7a65e; }
.card-back-mark { border: 1px solid #9b814d; width: 42px; height: 42px; transform: rotate(45deg); display: grid; place-items: center; font-family: serif; font-size: 18px; }
.card-level { position: absolute; top: 5px; left: 7px; font-family: serif; font-weight: 800; font-size: 16px; }
.card-element { width: 35px; height: 35px; display: grid; place-items: center; border: 1px solid currentColor; border-radius: 50%; font-family: serif; font-size: 18px; }
.card-name { font-size: 9px; font-weight: 700; margin-top: 8px; max-width: 100%; overflow: hidden; }
.element-火 .card-element { color: #a43d32; }.element-水 .card-element { color: #357a99; }
.element-木 .card-element { color: #467d51; }.element-金 .card-element { color: #887b55; }.element-土 .card-element { color: #9b6e35; }
.opponent-hand .playing-card { width: clamp(48px, 5vw, 68px); }
.board-center { z-index: 1; display: grid; grid-template-columns: 120px 1fr 120px; align-items: center; justify-items: center; }
.deck-pile, .discard-pile { display: grid; justify-items: center; gap: 6px; color: #707b73; font-size: 9px; }
.deck-pile strong, .discard-pile strong, .covered-card { width: 58px; aspect-ratio: 5/7; border: 1px solid #665b44; display: grid; place-items: center; color: #a68d56; background: #18201b; font-family: serif; font-size: 20px; }
.formation-field { min-width: 220px; min-height: 100px; display: grid; place-items: center; color: #69736c; font-size: 10px; border-left: 1px solid rgba(166, 141, 86, .14); border-right: 1px solid rgba(166, 141, 86, .14); }
.formation-field > span { color: #9a8251; letter-spacing: .2em; }
.choice-overlay { position: absolute; inset: 0; z-index: 12; background: rgba(7, 10, 8, .76); backdrop-filter: blur(4px); display: grid; place-items: center; text-align: center; }
.choice-overlay > div { border: 1px solid #8e733d; background: #18201b; padding: 30px; min-width: 360px; }
.choice-overlay h2 { font-family: serif; margin: 10px 0 20px; }
.choice-cards { display: flex; gap: 8px; justify-content: center; }
.choice-cards button { border: 1px solid #ae8b47; background: #ede6d4; color: #18201c; padding: 10px; }

.game-sidebar { min-height: 0; border-left: 1px solid #303a34; background: #171e1a; display: grid; grid-template-rows: auto 1fr auto; overflow: hidden; }
.action-panel, .event-panel { padding: 20px; border-bottom: 1px solid #303a34; }
.panel-heading, .panel-title { display: flex; justify-content: space-between; align-items: start; }
.panel-heading div { display: grid; }
.panel-heading span, .panel-title span { color: #8a948d; font-size: 9px; letter-spacing: .14em; }
.panel-heading strong { font-size: 13px; margin-top: 3px; }
.selection-count { border: 1px solid #3b463f; padding: 4px 7px; }
.available-formations { display: grid; gap: 7px; margin-top: 14px; }
.available-formations button { min-height: 58px; border: 1px solid #3b463f; background: #111713; display: flex; align-items: center; gap: 9px; text-align: left; padding: 8px; }
.available-formations button > span { width: 30px; height: 30px; display: grid; place-items: center; border-radius: 50%; }
.available-formations .attack { color: #d77765; border: 1px solid #7c4037; }
.available-formations .spell { color: #77a7c2; border: 1px solid #3f6275; }
.available-formations button div { display: grid; flex: 1; }
.available-formations small { color: #78827b; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; max-width: 170px; }
.available-formations i { color: #c8a75f; font-style: normal; font-size: 10px; }
.empty-action { min-height: 92px; display: grid; place-items: center; align-content: center; gap: 6px; color: #667069; text-align: center; font-size: 10px; }
.empty-action span { font-size: 22px; color: #806c43; }
.utility-actions { display: grid; grid-template-columns: 1fr 1fr; gap: 7px; }
.utility-actions button { border: 1px solid #3c463f; background: #222a25; min-height: 36px; font-size: 10px; }
.event-panel { min-height: 0; overflow: auto; }
.panel-title h2 { font-family: serif; font-size: 15px; }
.panel-title span { color: #70b585; }
.event-feed { list-style: none; padding: 0; margin: 16px 0 0; display: grid; gap: 13px; }
.event-feed li { display: grid; grid-template-columns: 10px 1fr; gap: 7px; }
.event-feed li > i { width: 5px; height: 5px; border-radius: 50%; background: #b79550; margin-top: 6px; box-shadow: 0 0 0 4px rgba(183, 149, 80, .08); }
.event-feed span { color: #d4d8d4; font-size: 10px; font-weight: 700; }
.event-feed p { color: #6f7972; font-size: 9px; line-height: 1.45; margin-top: 2px; }
.zone-summary { display: grid; grid-template-columns: repeat(3, 1fr); padding: 12px; }
.zone-summary div { text-align: center; display: grid; border-right: 1px solid #303a34; }
.zone-summary div:last-child { border: 0; }
.zone-summary span { color: #707a73; font-size: 9px; }
.zone-summary strong { color: #cbaa64; font-size: 13px; }

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
