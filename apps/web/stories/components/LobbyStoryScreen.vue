<template>
  <div class="app-shell">
    <BattleHeader>
      <button class="brand" type="button" aria-label="回到首頁"><img class="brand-banner" :src="'/header-banner.svg'" alt="五行戰鬥牌"></button>
      <nav class="header-actions" aria-label="帳號選單">
        <ThemeSelector />
        <span class="connection"><i />已連線</span>
        <button class="profile-button" type="button"><span class="profile-avatar">明</span><span>小明</span><span aria-hidden="true">⌄</span></button>
      </nav>
    </BattleHeader>
    <LobbyScreen ref="screen" v-model:room-code="roomCode" v-bind="props" @create="settingsOpen = true" @join="emit('join', roomCode)" @open="emit('open', $event)" @refresh="emit('refresh')">
      <RoomSettingsDialog v-if="settingsOpen" v-model:name="name" v-model:mode="mode" v-model:access="access" v-model:rule-version="ruleVersion" :busy="busy" @close="closeSettings" @submit="submitSettings" />
    </LobbyScreen>
  </div>
</template>

<script setup lang="ts">
import { nextTick, ref } from 'vue'
import BattleHeader from '~/components/BattleHeader.vue'
import LobbyScreen from '~/components/LobbyScreen.vue'
import RoomSettingsDialog from '~/components/RoomSettingsDialog.vue'
import ThemeSelector from '~/components/ThemeSelector.vue'
import type { LobbyRoomListItem } from '~/lib/lobby-presentation'

const props = defineProps<{ publicRooms: LobbyRoomListItem[]; myRooms: LobbyRoomListItem[]; loading: boolean; busy: boolean; error?: string }>()
const emit = defineEmits<{ join: [code: string]; open: [gameId: string]; refresh: []; submit: [settings: { name: string; mode: string; access: string; ruleVersion: string }] }>()
const screen = ref<InstanceType<typeof LobbyScreen> | null>(null)
const roomCode = ref('')
const settingsOpen = ref(false)
const name = ref('小明的房間')
const mode = ref<'duel' | 'team'>('duel')
const access = ref<'public' | 'private'>('public')
const ruleVersion = ref<'5.16' | '5.17'>('5.17')
function closeSettings() {
  settingsOpen.value = false
  void nextTick(() => screen.value?.focusCreateRoom())
}
function submitSettings() {
  emit('submit', { name: name.value, mode: mode.value, access: access.value, ruleVersion: ruleVersion.value })
  closeSettings()
}
</script>
