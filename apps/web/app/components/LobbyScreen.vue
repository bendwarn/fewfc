<template>
  <main class="lobby-page lobby-theme page-scene" data-scene="inn">
    <div class="lobby-content">
      <div class="lobby-heading">
        <div class="lobby-intro">
          <span class="lobby-eyebrow"><i aria-hidden="true">✦</i> 五行之間 · 以牌會友</span>
          <h1>房間</h1>
          <p>尋一位對手，或邀好友展開切磋。</p>
          <p class="lobby-intro-note">選擇公開房間，或輸入房間代碼加入對局。</p>
        </div>
        <section class="lobby-entry battle-glass" aria-label="建立或加入房間">
          <button ref="createRoomTrigger" class="lobby-create-button" type="button" :disabled="busy" @click="$emit('create')">
            <span aria-hidden="true">＋</span> 建立房間 <span aria-hidden="true">↗</span>
          </button>
          <div class="lobby-entry-divider"><span>已有房間代碼</span></div>
          <form class="room-code-form" @submit.prevent="$emit('join')">
            <label class="sr-only" for="join-room-code">房間代碼</label>
            <input id="join-room-code" v-model.trim="roomCode" class="code-input" type="text" placeholder="輸入房間代碼" autocomplete="off" spellcheck="false">
            <button type="submit" :disabled="busy || !roomCode">加入 <span aria-hidden="true">→</span></button>
          </form>
        </section>
      </div>

      <p v-if="loading" class="lobby-loading" role="status" aria-live="polite"><span aria-hidden="true" />正在載入房間…</p>
      <div v-if="error" class="lobby-error" role="alert">
        <p>{{ error }}</p>
        <button type="button" @click="$emit('refresh')">重試</button>
      </div>

      <div class="lobby-room-columns">
        <LobbyRoomList heading="公開房間" badge="可加入" empty-message="目前沒有可加入的公開房間。" :loading="loading" :busy="busy" :items="publicRooms" @open="$emit('open', $event)" />
        <LobbyRoomList class="my-rooms-card" heading="我的房間" :badge="myRooms.length" empty-message="尚未加入任何房間。" :loading="loading" :busy="busy" :items="myRooms" @open="$emit('open', $event)" />
      </div>
      <p class="lobby-footnote"><span aria-hidden="true">◇</span> 等待房的觀戰者會在有空位時，依加入順序自動補為玩家。</p>
    </div>
    <slot />
  </main>
</template>

<script setup lang="ts">
import { ref } from 'vue'
import LobbyRoomList from './LobbyRoomList.vue'
import type { LobbyRoomListItem } from '~/lib/lobby-presentation'

defineProps<{
  publicRooms: readonly LobbyRoomListItem[]
  myRooms: readonly LobbyRoomListItem[]
  loading: boolean
  busy: boolean
  error?: string
}>()
const roomCode = defineModel<string>('roomCode', { default: '' })
defineEmits<{ create: []; join: []; refresh: []; open: [gameId: string] }>()
const createRoomTrigger = ref<HTMLButtonElement | null>(null)
defineExpose({ focusCreateRoom: () => createRoomTrigger.value?.focus() })
</script>
