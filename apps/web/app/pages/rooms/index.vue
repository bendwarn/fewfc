<template>
  <LobbyScreen
    ref="lobbyScreen"
    v-model:room-code="joinRoomCode"
    :public-rooms="joinablePublicRoomItems"
    :my-rooms="myRoomItems"
    :loading="lobbyLoading"
    :busy="lobbyBusy"
    :error="roomSettingsOpen ? '' : lobbyError"
    @create="openRoomSettings"
    @join="joinRoomByCode"
    @refresh="refreshRoomLists"
    @open="openJoinedRoom"
  >
    <RoomSettingsDialog
      v-if="roomSettingsOpen"
      v-model:name="roomName"
      v-model:rule-version="roomRuleVersion"
      v-model:mode="roomMode"
      v-model:access="roomAccess"
      :busy="lobbyBusy"
      :error="lobbyError"
      @close="closeRoomSettings"
      @submit="createRoom"
    />
  </LobbyScreen>
</template>

<script setup lang="ts">
import LobbyScreen from '~/components/LobbyScreen.vue'
import type { LobbyRoomListItem } from '~/lib/lobby-presentation'
import { modulesForVersion } from '#shared/utils/rule-versions'
import type { GameRoomLobbyItem } from '#shared/game-room'
import { presentApiError } from '~/lib/api-error-presentation'
import { selectJoinablePublicRooms } from '~/lib/lobby-room-selection'
import { presentRoomRuleDifferences } from '#shared/utils/ruleset-presentation'
import { useLayoutNotifications } from '~/lib/player-notifications-context'
import { useRulesCatalog } from '~/lib/rules-catalog'

const router = useRouter()
const session = usePlayerSession()
const notifications = useLayoutNotifications()
const rulesCatalog = useRulesCatalog()
const roomSettingsOpen = ref(false)
const roomRuleVersion = ref<'5.16' | '5.17'>('5.17')
const lobbyScreen = ref<InstanceType<typeof LobbyScreen> | null>(null)
const roomName = ref('')
const roomMode = ref<'duel' | 'team'>('duel')
const roomAccess = ref<'private' | 'public'>('public')
const joinRoomCode = ref('')
const lobbyBusy = ref(false)
const lobbyLoading = ref(false)
const lobbyError = ref('')
const publicRooms = ref<GameRoomLobbyItem[]>([])
const myRooms = ref<GameRoomLobbyItem[]>([])
const roomCapacity = computed<2 | 4>(() => roomMode.value === 'team' ? 4 : 2)
const joinablePublicRooms = computed(() => selectJoinablePublicRooms(publicRooms.value))
const joinablePublicRoomItems = computed<LobbyRoomListItem[]>(() => joinablePublicRooms.value.map(room => ({
  gameId: room.gameId,
  code: room.roomCode,
  name: room.name,
  detail: `${room.memberCount} / ${room.capacity} 玩家 · ${room.observerCount} 位觀戰者 · ${room.status === 'Active' ? '對局中' : '等待開始'}`,
  ruleSummary: roomRuleSummary(room),
  actionLabel: room.status === 'Active' || room.memberCount >= room.capacity ? '觀戰' : '加入',
  hasNotification: hasNotification(room.gameId),
})))
const myRoomItems = computed<LobbyRoomListItem[]>(() => myRooms.value.map(room => ({
  gameId: room.gameId,
  code: room.roomCode,
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
  void nextTick(() => lobbyScreen.value?.focusCreateRoom())
}

async function createRoom() {
  lobbyBusy.value = true
  lobbyError.value = ''
  try {
    const response = await notifications.request('create', {
      name: roomName.value || `${session.displayName.value}的房間`,
      access: roomAccess.value,
      capacity: roomCapacity.value,
      ruleVersion: roomRuleVersion.value,
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
    const response = await notifications.request('list', undefined)
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
    const response = await notifications.request('joinCode', { code: joinRoomCode.value.toUpperCase() })
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

function roomStatusLabel(room: GameRoomLobbyItem): string {
  const status = { Waiting: '等待中', Active: '對局中', Finished: '已結束', Dissolved: '已解散' }[room.status] ?? room.status
  return `${room.memberCount} / ${room.capacity} 玩家 · ${room.observerCount} 位觀戰者 · ${status}${room.isObserver ? ' · 觀戰' : ''}`
}

function roomRuleSummary(room: GameRoomLobbyItem): string {
  const version = room.ruleVersion ?? '5.16'
  const differences = presentRoomRuleDifferences(modulesForVersion(rulesCatalog.catalog.value?.ruleModules ?? [], version), room.enabledRuleModules)
  return `規則 ${version}${differences ? ` · ${differences}` : ''}`
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
