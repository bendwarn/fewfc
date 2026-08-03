<template>
  <main class="lobby-page">
      <div class="lobby-heading">
        <div>
          <h1>房間</h1>
          <p class="muted">選擇等待中的房間，或使用房間代碼加入。</p>
        </div>
        <div class="lobby-actions">
          <form class="room-code-form" @submit.prevent="joinRoomByCode">
            <label class="sr-only" for="join-room-code">房間代碼</label>
            <input
              id="join-room-code"
              v-model.trim="joinRoomCode"
              class="code-input"
              type="text"
              placeholder="房間代碼"
            >
            <button type="submit" :disabled="lobbyBusy || !joinRoomCode">
              加入
            </button>
          </form>
          <button
            ref="createRoomTrigger"
            class="primary-button create-room-button"
            type="button"
            @click="openRoomSettings"
          >
            建立房間
          </button>
        </div>
      </div>

      <p v-if="lobbyLoading" class="muted" role="status" aria-live="polite">正在載入房間…</p>
      <p v-if="lobbyError && !roomSettingsOpen" class="form-error lobby-error" role="alert">
        {{ lobbyError }}
        <button class="ghost-button" type="button" @click="refreshRoomLists">重試</button>
      </p>

      <div
        v-if="roomSettingsOpen"
        class="room-settings-layer"
        @click.self="closeRoomSettings"
      >
        <form
          class="setup-card room-settings-dialog"
          role="dialog"
          aria-modal="true"
          aria-labelledby="room-settings-title"
          @submit.prevent="createRoom"
        >
          <div class="card-heading">
            <span class="step-number">+</span>
            <div>
              <h2 id="room-settings-title">建立房間</h2>
              <p>設定這場對戰的基本資訊。</p>
            </div>
          </div>

          <label for="room-name">房間名稱</label>
          <input
            id="room-name"
            ref="roomNameInput"
            v-model="roomName"
            class="text-input"
            maxlength="24"
          >

          <fieldset>
            <legend>對戰模式</legend>
            <div class="option-grid">
              <button
                v-for="mode in modes"
                :key="mode.id"
                type="button"
                class="mode-option"
                :class="{ selected: roomMode === mode.id }"
                @click="roomMode = mode.id"
              >
                <span class="mode-icon">{{ mode.icon }}</span>
                <strong>{{ mode.label }}</strong>
                <small>{{ mode.description }}</small>
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

          <div class="setup-summary">
            <div>
              <span>目前設定</span>
              <strong>{{ roomModeLabel }} · {{ roomAccess === 'private' ? '私人房間' : '公開房間' }}</strong>
            </div>
            <p>{{ roomAccess === 'public' ? '公開房間會顯示於可加入清單。' : '私人房間僅能透過邀請連結或房碼加入。' }}</p>
          </div>

          <p v-if="lobbyError" class="form-error">{{ lobbyError }}</p>

          <div class="setup-actions">
            <button class="secondary-button" type="button" :disabled="lobbyBusy" @click="closeRoomSettings">
              取消
            </button>
            <button class="primary-button start-button" type="submit" :disabled="lobbyBusy">
              {{ lobbyBusy ? '處理中…' : '建立房間' }} <span>→</span>
            </button>
          </div>
        </form>
      </div>

      <section class="public-rooms-card">
        <div class="panel-title">
          <h2>公開房間</h2>
          <span>可加入</span>
        </div>
        <p v-if="!lobbyLoading && !joinablePublicRooms.length" class="muted">目前沒有可加入的公開房間。</p>
        <div v-else class="public-room-list">
          <button
            v-for="room in joinablePublicRooms"
            :key="room.gameId"
            type="button"
            :disabled="lobbyBusy"
            @click="enterListedRoom(room)"
          >
            <span class="room-code">{{ room.roomCode }}</span>
            <div>
              <strong>{{ room.name }}</strong>
              <small>{{ room.members.length }} / {{ room.capacity }} 玩家 · 等待開始</small>
              <small v-if="roomRuleSummary(room)">{{ roomRuleSummary(room) }}</small>
            </div>
            <i>加入</i>
          </button>
        </div>
      </section>

      <section class="public-rooms-card my-rooms-card">
        <div class="panel-title">
          <h2>我的房間</h2>
          <span>{{ myRooms.length }}</span>
        </div>
        <p v-if="!lobbyLoading && !myRooms.length" class="muted">尚未加入任何房間。</p>
        <div v-else class="public-room-list">
          <button
            v-for="room in myRooms"
            :key="`mine-${room.gameId}`"
            type="button"
            :disabled="lobbyBusy"
            @click="openJoinedRoom(room.gameId)"
          >
            <span class="room-code">{{ room.gameId.slice(0, 8) }}</span>
            <div>
              <strong>{{ room.name }}</strong>
              <small>{{ roomStatusLabel(room) }}</small>
              <small v-if="roomRuleSummary(room)">{{ roomRuleSummary(room) }}</small>
            </div>
            <i>{{ roomNeedsAttention(room) ? '輪到你' : '進入' }}</i>
          </button>
        </div>
      </section>
  </main>
</template>

<script setup lang="ts">
import type { PlayerId } from '~/types/fewfc'
import type { GameRoomMember, GameRoomResponse } from '#shared/game-room'
import { presentApiError } from '~/lib/api-error-presentation'
import { presentRoomRuleDifferences } from '#shared/utils/ruleset-presentation'
import { useLayoutNotifications } from '~/lib/player-notifications-context'
import { useRulesCatalog } from '~/lib/rules-catalog'

interface PublicRoomSummary {
  gameId: string
  roomCode: string
  name: string
  access: 'private' | 'public'
  status: string
  ownerUserId: string
  players: PlayerId[]
  members: GameRoomMember[]
  capacity: number
  enabledRuleModules: string[]
  createdAt: string
  updatedAt: string
}

const router = useRouter()
const session = usePlayerSession()
const notifications = useLayoutNotifications()
const rulesCatalog = useRulesCatalog()
const roomSettingsOpen = ref(false)
const createRoomTrigger = ref<HTMLButtonElement | null>(null)
const roomNameInput = ref<HTMLInputElement | null>(null)
const roomName = ref('')
const roomMode = ref('duel')
const roomAccess = ref<'private' | 'public'>('public')
const joinRoomCode = ref('')
const lobbyBusy = ref(false)
const lobbyLoading = ref(false)
const lobbyError = ref('')
const publicRooms = ref<PublicRoomSummary[]>([])
const myRooms = ref<PublicRoomSummary[]>([])
const modes = [
  { id: 'duel', icon: '雙', label: '雙人對戰', description: '1 對 1 經典規則' },
  { id: 'team', icon: '隊', label: '團隊對戰', description: '2 對 2 交錯行動' },
]
const roomCapacity = computed<2 | 4>(() => roomMode.value === 'team' ? 4 : 2)
const roomModeLabel = computed(() => modes.find(mode => mode.id === roomMode.value)?.label ?? '')
const joinablePublicRooms = computed(() => publicRooms.value.filter(
  room => !room.members.some(member => member.userId === session.userId.value),
))

async function openRoomSettings() {
  lobbyError.value = ''
  roomSettingsOpen.value = true
  await nextTick()
  roomNameInput.value?.focus()
}

function closeRoomSettings() {
  if (lobbyBusy.value) return
  lobbyError.value = ''
  roomSettingsOpen.value = false
  void nextTick(() => createRoomTrigger.value?.focus())
}

function handlePageKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape' && roomSettingsOpen.value) {
    event.preventDefault()
    closeRoomSettings()
  }
}

async function createRoom() {
  lobbyBusy.value = true
  lobbyError.value = ''
  try {
    const response = await $fetch<GameRoomResponse>('/api/games', {
      method: 'POST',
      body: {
        name: roomName.value || `${session.displayName.value}的房間`,
        access: roomAccess.value,
        capacity: roomCapacity.value,
      },
    })
    await router.push(`/rooms/${encodeURIComponent(response.gameId)}`)
  } catch (error) {
    lobbyError.value = presentApiError(error, '無法建立房間')
  } finally {
    lobbyBusy.value = false
  }
}

async function refreshRoomLists() {
  lobbyLoading.value = true
  lobbyError.value = ''
  try {
    const response = await $fetch<{ rooms: PublicRoomSummary[], myRooms: PublicRoomSummary[] }>('/api/games')
    publicRooms.value = response.rooms
    myRooms.value = response.myRooms
  } catch (error) {
    lobbyError.value = presentApiError(error, '無法取得公開房間')
  } finally {
    lobbyLoading.value = false
  }
}

async function joinRoomByCode() {
  if (!joinRoomCode.value) return
  lobbyBusy.value = true
  lobbyError.value = ''
  try {
    const response = await $fetch<GameRoomResponse>('/api/games/join', {
      method: 'POST',
      body: { code: joinRoomCode.value.toUpperCase() },
    })
    await router.push(`/rooms/${encodeURIComponent(response.gameId)}`)
  } catch (error) {
    lobbyError.value = presentApiError(error, '無法加入房間')
  } finally {
    lobbyBusy.value = false
  }
}

async function openJoinedRoom(gameId: string) {
  await router.push(`/rooms/${encodeURIComponent(gameId)}`)
}

async function enterListedRoom(room: PublicRoomSummary) {
  await openJoinedRoom(room.gameId)
}

function roomStatusLabel(room: PublicRoomSummary): string {
  const status = { Waiting: '等待中', Active: '對局中', Finished: '已結束', Dissolved: '已解散' }[room.status] ?? room.status
  return `${room.members.length} / ${room.capacity} 玩家 · ${status}`
}

function roomRuleSummary(room: PublicRoomSummary): string {
  return presentRoomRuleDifferences(rulesCatalog.catalog.value?.ruleModules ?? [], room.enabledRuleModules)
}

function roomNeedsAttention(room: PublicRoomSummary): boolean {
  return notifications.notifications.value.some(notification => (
    notification.gameId === room.gameId && (notification.kind === 'gameStarted' || notification.kind === 'yourTurn')
  ))
}

onMounted(async () => {
  roomName.value = `${session.displayName.value}的房間`
  window.addEventListener('keydown', handlePageKeydown)
  await Promise.allSettled([rulesCatalog.load(), refreshRoomLists()])
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handlePageKeydown)
})

watch(() => notifications.roomListRevision.value, () => {
  void refreshRoomLists()
})
</script>

<style scoped>
@reference "../../assets/css/main.css";

.lobby-page { @apply mx-auto max-w-[1180px] px-[30px] pt-15 pb-[90px] max-[600px]:px-4 max-[600px]:py-9; }
.lobby-heading { @apply mb-[38px] flex items-end justify-between gap-6 max-[900px]:flex-col max-[900px]:items-start; }
.lobby-actions { @apply flex items-stretch gap-3 max-[600px]:w-full max-[600px]:flex-col-reverse; }
.room-code-form { @apply flex min-h-[50px]; border: 1px solid var(--app-border); border-radius: 10px; background: var(--app-input); }
.room-code-form:focus-within { border-color: var(--app-accent); box-shadow: 0 0 0 3px var(--app-focus-ring); }
.room-code-form .code-input { @apply h-auto min-w-48 border-0 px-4 text-left tracking-[.08em] max-[600px]:min-w-0; }
.room-code-form button { @apply border-0 border-l px-4 text-xs text-gold-light disabled:cursor-not-allowed disabled:opacity-45; border-color: var(--app-border); border-radius: 0 9px 9px 0; background: var(--app-control); }
.create-room-button { @apply min-w-35 justify-center; }
.lobby-error { @apply mb-2 p-3; border: 1px solid var(--app-danger); border-radius: 10px; background: var(--app-danger-surface); }
.room-settings-dialog { @apply my-auto w-full max-w-[720px] shadow-[0_24px_70px_rgba(0,0,0,.5)]; }
fieldset { @apply mb-[26px] border-0 p-0; }
.option-grid { @apply grid grid-cols-2 gap-3 max-[600px]:grid-cols-1; }
.mode-option { @apply relative grid min-h-27 grid-cols-[42px_1fr] p-4 text-left; border: 1px solid var(--app-border); border-radius: 12px; background: var(--app-surface-muted); color: var(--app-text); }
.mode-option.selected { border-color: var(--app-accent); background: var(--app-accent-soft); box-shadow: inset 0 0 0 1px var(--app-accent); }
.mode-icon { @apply row-span-2 grid size-8 place-items-center rounded-full; background: var(--app-control); color: var(--app-accent-strong); }
.mode-option small { color: var(--app-text-muted); }
.segmented { @apply grid grid-cols-2 p-1; border-radius: 10px; background: var(--app-surface-muted); }
.segmented button { @apply min-h-10 border-0 bg-transparent text-muted; }
.segmented button.active { background: var(--app-surface-raised); color: var(--app-accent-strong); box-shadow: var(--app-shadow-sm); }
.setup-summary { @apply mb-5 grid gap-2 p-4; border: 1px solid var(--app-border); border-radius: 10px; background: var(--app-surface-muted); }
.setup-summary div { @apply flex items-center justify-between gap-4 max-[600px]:grid; }
.setup-summary span { @apply text-[10px] tracking-[.18em] text-muted; }
.setup-summary strong { @apply text-sm text-gold-light; }
.setup-summary p { @apply text-xs text-muted; }
.start-button { @apply w-full; }
.setup-actions { @apply mt-5 grid grid-cols-[auto_1fr] gap-3; }
.code-input { max-width: 320px; height: 52px; border: 1px solid var(--app-border); color: var(--app-text); padding: 0 20px; text-align: center; letter-spacing: .2em; }
.public-rooms-card { @apply mt-0 border border-line bg-panel p-6; border-radius: 16px; box-shadow: var(--app-shadow-md); }
.my-rooms-card { @apply mt-6; }
.public-rooms-card .panel-title { @apply mb-4; }
.public-room-list { @apply grid gap-3; }
.public-room-list button { @apply grid grid-cols-[92px_1fr_auto] items-center gap-3 p-4 text-left; border: 1px solid var(--app-border); border-radius: 12px; background: var(--app-surface-muted); }
.public-room-list button:hover { border-color: var(--app-accent); background: var(--app-accent-soft); }
.public-room-list button:disabled { @apply cursor-not-allowed opacity-55; }
.public-room-list strong { @apply block text-sm; color: var(--app-text); }
.public-room-list small { @apply text-xs text-muted; }
.public-room-list i { @apply text-[10px] not-italic text-gold-light; }
.room-code { @apply font-mono text-[10px]; color: var(--app-text-muted); overflow-wrap: anywhere; }
@media (max-width: 600px) { .lobby-heading { align-items: start; gap: 25px; flex-direction: column; }.lobby-page { padding: 36px 16px; }.option-grid { grid-template-columns: 1fr; }.setup-card { padding: 22px 18px; } }
</style>
