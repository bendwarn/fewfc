<template>
  <div class="app-shell">
    <BattleHeader>
      <button class="brand" type="button" aria-label="回到首頁" @click="router.push('/rooms')">
        <img class="brand-banner" src="/header-banner.svg" alt="五行戰鬥牌">
      </button>

      <nav class="header-actions" aria-label="帳號選單">
        <ThemeSelector />
        <span class="connection" :class="{ offline: notifications.connectionState.value !== 'connected' }">
          <i /> {{ notifications.connectionState.value === 'connected' ? '已連線' : '連線中' }}
        </span>
        <div ref="profileMenuContainer" class="profile-menu-container">
          <button class="profile-button" type="button" aria-haspopup="menu" :aria-expanded="profileOpen" @click="profileOpen = !profileOpen">
            <span class="profile-avatar">{{ playerInitial }}</span>
            <span>{{ session.displayName.value }}</span>
            <span aria-hidden="true">⌄</span>
          </button>
          <div v-if="profileOpen" class="profile-menu">
            <button type="button" @click="open('/account')">帳號設定</button>
            <button type="button" @click="open('/deck')">個人牌組</button>
            <button type="button" @click="open('/replays')">重播紀錄</button>
            <button type="button" @click="logout">登出</button>
          </div>
        </div>
      </nav>
    </BattleHeader>

    <slot />

    <aside v-if="visibleNotifications.length" class="notification-stack" aria-label="玩家通知">
      <article v-for="notification in visibleNotifications" :key="notification.id" class="notification-item">
        <button v-if="canOpenNotification(notification)" class="notification-main" type="button" @click="openNotification(notification.gameId)">
          <strong>{{ notification.message }}</strong>
          <span>進入房間</span>
        </button>
        <div v-else class="notification-main" role="status">
          <strong>{{ notification.message }}</strong>
        </div>
        <button class="notification-dismiss" type="button" aria-label="關閉通知" @click="notifications.dismiss(notification.id)">×</button>
      </article>
    </aside>
  </div>
</template>

<script setup lang="ts">
import type { PlayerNotification } from '#shared/game-room'
import { playerNotificationsKey } from '~/lib/player-notifications-context'
import { isTransientPlayerNotification } from '~/lib/player-notifications'
import { authClient } from '~/lib/auth-client'

const router = useRouter()
const route = useRoute()
const session = usePlayerSession()
const notifications = usePlayerNotifications()
const profileOpen = ref(false)
const profileMenuContainer = ref<HTMLElement | null>(null)
const playerInitial = computed(() => session.displayName.value.trim().charAt(0).toUpperCase() || 'A')
const visibleNotifications = computed(() => notifications.visibleNotifications.value.filter(notification => (
  typeof route.params.gameId !== 'string'
    || notification.gameId !== route.params.gameId
    || notification.kind === 'seatPromoted'
)))

provide(playerNotificationsKey, notifications)

function open(path: string) {
  profileOpen.value = false
  void router.push(path)
}

async function logout() {
  profileOpen.value = false
  await authClient.signOut()
  notifications.disconnect()
  notifications.setUserId('')
  session.clear()
  await router.replace('/login')
}

async function openNotification(gameId: string) {
  await router.push(`/rooms/${encodeURIComponent(gameId)}`)
}

function canOpenNotification(notification: PlayerNotification): boolean {
  return !isTransientPlayerNotification(notification)
}

function closeProfileOnOutsideClick(event: MouseEvent) {
  if (profileOpen.value && event.target instanceof Node && !profileMenuContainer.value?.contains(event.target)) {
    profileOpen.value = false
  }
}

onMounted(() => {
  notifications.setUserId(session.userId.value)
  notifications.connect()
  window.addEventListener('click', closeProfileOnOutsideClick)
})

watch(() => session.userId.value, (userId) => {
  notifications.setUserId(userId)
  if (userId) notifications.connect()
})

onBeforeUnmount(() => {
  window.removeEventListener('click', closeProfileOnOutsideClick)
  notifications.disconnect()
})
</script>
