<template>
  <main class="lobby-page">
      <div class="lobby-heading">
        <div>
          <h1>房間</h1>
          <p class="muted">選擇公開房間，或使用房間代碼加入；等待房的觀戰者會在有空位時依順序自動補為玩家。</p>
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

      <RoomSettingsDialog
        v-if="roomSettingsOpen"
        v-model:name="roomName"
        v-model:mode="roomMode"
        v-model:access="roomAccess"
        :busy="lobbyBusy"
        :error="lobbyError"
        @close="closeRoomSettings"
        @submit="createRoom"
      />

      <LobbyRoomList
        heading="公開房間"
        badge="可加入"
        empty-message="目前沒有可加入的公開房間。"
        :loading="lobbyLoading"
        :busy="lobbyBusy"
        :items="joinablePublicRoomItems"
        @open="openJoinedRoom"
      />

      <LobbyRoomList
        class="my-rooms-card"
        heading="我的房間"
        :badge="myRooms.length"
        empty-message="尚未加入任何房間。"
        :loading="lobbyLoading"
        :busy="lobbyBusy"
        :items="myRoomItems"
        @open="openJoinedRoom"
      />
  </main>
</template>

<script setup lang="ts">
import type { PlayerId } from '~/types/fewfc'
import type { GameRoomMember, GameRoomResponse } from '#shared/game-room'
import { presentApiError } from '~/lib/api-error-presentation'
import { selectJoinablePublicRooms } from '~/lib/lobby-room-selection'
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
  observers: Array<{ userId: string }>
  capacity: number
  enabledRuleModules: string[]
  createdAt: string
  updatedAt: string
}

interface LobbyRoomListItem {
  gameId: string
  code: string
  name: string
  detail: string
  ruleSummary: string
  actionLabel: string
  hasNotification: boolean
}

const router = useRouter()
const session = usePlayerSession()
const notifications = useLayoutNotifications()
const rulesCatalog = useRulesCatalog()
const roomSettingsOpen = ref(false)
const createRoomTrigger = ref<HTMLButtonElement | null>(null)
const roomName = ref('')
const roomMode = ref<'duel' | 'team'>('duel')
const roomAccess = ref<'private' | 'public'>('public')
const joinRoomCode = ref('')
const lobbyBusy = ref(false)
const lobbyLoading = ref(false)
const lobbyError = ref('')
const publicRooms = ref<PublicRoomSummary[]>([])
const myRooms = ref<PublicRoomSummary[]>([])
const roomCapacity = computed<2 | 4>(() => roomMode.value === 'team' ? 4 : 2)
const joinablePublicRooms = computed(() => selectJoinablePublicRooms(publicRooms.value, session.userId.value))
const joinablePublicRoomItems = computed<LobbyRoomListItem[]>(() => joinablePublicRooms.value.map(room => ({
  gameId: room.gameId,
  code: room.roomCode,
  name: room.name,
  detail: `${room.members.length} / ${room.capacity} 玩家 · ${room.observers.length} 位觀戰者 · ${room.status === 'Active' ? '對局中' : '等待開始'}`,
  ruleSummary: roomRuleSummary(room),
  actionLabel: room.status === 'Active' || room.members.length >= room.capacity ? '觀戰' : '加入',
  hasNotification: hasNotification(room.gameId),
})))
const myRoomItems = computed<LobbyRoomListItem[]>(() => myRooms.value.map(room => ({
  gameId: room.gameId,
  code: room.gameId.slice(0, 8),
  name: room.name,
  detail: roomStatusLabel(room),
  ruleSummary: roomRuleSummary(room),
  actionLabel: '進入',
  hasNotification: hasNotification(room.gameId),
})))

async function openRoomSettings() {
  lobbyError.value = ''
  roomSettingsOpen.value = true
}

function closeRoomSettings() {
  if (lobbyBusy.value) return
  lobbyError.value = ''
  roomSettingsOpen.value = false
  void nextTick(() => createRoomTrigger.value?.focus())
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

function roomStatusLabel(room: PublicRoomSummary): string {
  const status = { Waiting: '等待中', Active: '對局中', Finished: '已結束', Dissolved: '已解散' }[room.status] ?? room.status
  const observer = room.observers.find(member => member.userId === session.userId.value)
  return `${room.members.length} / ${room.capacity} 玩家 · ${room.observers.length} 位觀戰者 · ${status}${observer ? ' · 觀戰' : ''}`
}

function roomRuleSummary(room: PublicRoomSummary): string {
  return presentRoomRuleDifferences(rulesCatalog.catalog.value?.ruleModules ?? [], room.enabledRuleModules)
}

function hasNotification(gameId: string): boolean {
  return notifications.notifications.value.some(notification => notification.gameId === gameId)
}

onMounted(async () => {
  roomName.value = `${session.displayName.value}的房間`
  await Promise.allSettled([rulesCatalog.load(), refreshRoomLists()])
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
.code-input { max-width: 320px; height: 52px; border: 1px solid var(--app-border); color: var(--app-text); padding: 0 20px; text-align: center; letter-spacing: .2em; }
.my-rooms-card { @apply mt-6; }
@media (max-width: 600px) { .lobby-heading { align-items: start; gap: 25px; flex-direction: column; }.lobby-page { padding: 36px 16px; } }
</style>
