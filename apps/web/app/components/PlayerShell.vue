<template>
  <div class="app-shell">
    <BattleHeader>
      <button class="brand" type="button" aria-label="回到首頁" @click="emit('home')">
        <img class="brand-banner" :src="'/header-banner.svg'" alt="五行戰鬥牌">
      </button>

      <nav class="header-actions" aria-label="帳號選單">
        <ThemeSelector />
        <span class="connection" :class="{ offline: connectionState !== 'connected' }">
          <i /> {{ connectionState === 'connected' ? '已連線' : '連線中' }}
        </span>
        <div ref="profileMenuContainer" class="profile-menu-container" @keydown.esc.stop.prevent="closeProfile">
          <button ref="profileTrigger" class="profile-button" type="button" :aria-label="`${displayName}的玩家選單`" :aria-expanded="profileOpen" aria-controls="player-profile-menu" @click="profileOpen = !profileOpen">
            <span class="profile-avatar">{{ playerInitial }}</span>
            <span>{{ displayName }}</span>
            <span aria-hidden="true">⌄</span>
          </button>
          <div v-if="profileOpen" id="player-profile-menu" class="profile-menu" aria-label="玩家功能">
            <button type="button" @click="open('/account')">帳號設定</button>
            <button type="button" @click="open('/deck')">個人牌組</button>
            <button type="button" @click="open('/replays')">重播紀錄</button>
            <button type="button" @click="signOut">登出</button>
          </div>
        </div>
      </nav>
    </BattleHeader>

    <slot />

    <aside v-if="notifications.length" class="notification-stack" aria-label="玩家通知">
      <article v-for="notification in notifications" :key="notification.id" class="notification-item">
        <button v-if="canOpenNotification(notification)" class="notification-main" type="button" @click="emit('openNotification', notification.gameId)">
          <strong>{{ notification.message }}</strong>
          <span>進入房間</span>
        </button>
        <div v-else class="notification-main" role="status">
          <strong>{{ notification.message }}</strong>
        </div>
        <button class="notification-dismiss" type="button" aria-label="關閉通知" @click="emit('dismiss', notification.id)">×</button>
      </article>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { computed, nextTick, onBeforeUnmount, onMounted, ref } from 'vue'
import type { PlayerNotification } from '#shared/game-room'
import { isTransientPlayerNotification } from '~/lib/player-notifications'
import BattleHeader from './BattleHeader.vue'
import ThemeSelector from './ThemeSelector.vue'

const props = defineProps<{ displayName: string; connectionState: string; notifications: readonly PlayerNotification[] }>()
const emit = defineEmits<{ home: []; navigate: [path: string]; logout: []; openNotification: [gameId: string]; dismiss: [id: string] }>()
const profileOpen = ref(false)
const profileMenuContainer = ref<HTMLElement | null>(null)
const profileTrigger = ref<HTMLButtonElement | null>(null)
const playerInitial = computed(() => props.displayName.trim().charAt(0).toUpperCase() || 'A')
function open(path: string) { profileOpen.value = false; emit('navigate', path) }
function signOut() { profileOpen.value = false; emit('logout') }
function closeProfile() {
  profileOpen.value = false
  void nextTick(() => profileTrigger.value?.focus())
}
function canOpenNotification(notification: PlayerNotification) { return !isTransientPlayerNotification(notification) }
function closeOnOutsideClick(event: MouseEvent) {
  if (event.target instanceof Node && !profileMenuContainer.value?.contains(event.target)) profileOpen.value = false
}
onMounted(() => window.addEventListener('click', closeOnOutsideClick))
onBeforeUnmount(() => window.removeEventListener('click', closeOnOutsideClick))
</script>
