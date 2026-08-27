<template>
  <div class="app-shell">
    <header class="site-header">
      <button class="brand" type="button" aria-label="回到首頁" @click="router.push('/rooms')">
        <img class="brand-banner" src="/header-banner.svg" alt="五行戰鬥牌">
      </button>

      <nav class="header-actions" aria-label="帳號選單">
        <ThemeSelector />
        <span class="connection" :class="{ offline: notifications.connectionState.value !== 'connected' }">
          <i /> {{ notifications.connectionState.value === 'connected' ? '已連線' : '連線中' }}
        </span>
        <div ref="profileMenuContainer" class="profile-menu-container">
          <button class="profile-button" type="button" @click="profileOpen = !profileOpen">
            <span class="profile-avatar">{{ playerInitial }}</span>
            <span>{{ session.displayName.value }}</span>
            <span aria-hidden="true">⌄</span>
          </button>
          <div v-if="profileOpen" class="profile-menu">
            <button type="button" @click="open('/deck')">個人牌組</button>
            <button type="button" @click="open('/replays')">重播紀錄</button>
            <button type="button" @click="logout">登出</button>
          </div>
        </div>
      </nav>
    </header>

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

<style>
@reference "../assets/css/main.css";

.app-shell { @apply flex min-h-screen flex-col bg-ink; color: var(--app-text); }
.app-shell:has(> .game-page) { @apply h-dvh min-h-0 overflow-hidden; }
.site-header { @apply relative z-20 flex min-h-[84px] items-center justify-between border-b border-line; padding: 10px clamp(14px, 4vw, 64px); background: var(--app-header); box-shadow: var(--app-shadow-sm); }
.brand { @apply flex min-w-0 items-center border-0 bg-transparent p-0; }
.brand-banner { @apply block h-auto w-[min(52vw,456px)] max-w-full rounded-md shadow-[0_10px_28px_rgba(0,0,0,.32)]; }
.header-actions { @apply relative flex items-center gap-[18px]; }
.connection { @apply text-xs text-[#98a39c]; }
.connection i { display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: #62b585; margin-right: 6px; box-shadow: 0 0 8px #62b585; }
.connection.offline i { background: #c7a35d; box-shadow: none; }
.profile-menu-container { @apply relative; }
.profile-button { @apply flex items-center gap-2 border-0 bg-transparent; }
.profile-avatar { @apply grid size-[34px] place-items-center rounded-full font-extrabold; background: var(--app-accent); color: var(--app-on-accent); }
.profile-menu { @apply absolute right-0 top-[calc(100%+10px)] z-30 grid min-w-32 overflow-hidden border border-line; border-radius: 12px; background: var(--app-surface-raised); box-shadow: var(--app-shadow-lg); }
.profile-menu button { @apply border-0 bg-transparent px-4 py-2.5 text-left text-xs hover:text-gold-light; color: var(--app-text); }
.profile-menu button:hover { background: var(--app-accent-soft); }
.notification-stack { @apply fixed top-24 right-5 z-30 grid w-[min(360px,calc(100vw-32px))] gap-2; }
.notification-item { @apply grid grid-cols-[1fr_34px] border; border-color: var(--app-accent); border-radius: 12px; background: var(--app-surface-raised); box-shadow: var(--app-shadow-lg); }
.notification-main { @apply grid gap-1 border-0 bg-transparent p-3 text-left; }
.notification-main strong { @apply text-xs; color: var(--app-text); }
.notification-main span { @apply text-[10px] text-gold-light; }
.notification-dismiss { @apply border-0 border-l border-line bg-transparent text-muted; }
.primary-button { @apply inline-flex min-h-[50px] items-center justify-center px-5 font-bold disabled:cursor-not-allowed disabled:opacity-45; border: 1px solid var(--app-accent); border-radius: 10px; background: var(--app-accent); color: var(--app-on-accent); }
.primary-button:hover { background: var(--app-accent-strong); }
.secondary-button { @apply min-h-[50px] bg-transparent px-5 text-muted disabled:cursor-not-allowed disabled:opacity-45; border: 1px solid var(--app-border-strong); border-radius: 10px; }
.secondary-button:hover, .ghost-button:hover { border-color: var(--app-accent); color: var(--app-accent-strong); }
.ghost-button { @apply min-h-10 bg-transparent px-4 text-xs; border: 1px solid var(--app-border-strong); border-radius: 9px; color: var(--app-text); }
.setup-card { @apply border border-line bg-panel p-8 max-[600px]:px-[18px] max-[600px]:py-[22px]; border-radius: 16px; box-shadow: var(--app-shadow-md); }
.card-heading { @apply mb-8 flex gap-[18px]; }
.step-number { @apply grid size-[42px] place-items-center border border-[#7e693e] font-serif text-[#d3ae62]; }
.card-heading h2 { @apply mb-1 font-serif text-[21px]; }
.card-heading p { @apply text-xs text-muted; }
.text-input { @apply mb-[26px] h-12 px-3.5; border: 1px solid var(--app-border); border-radius: 9px; background: var(--app-input); color: var(--app-text); }
.room-settings-layer { @apply fixed inset-0 z-40 grid place-items-center overflow-y-auto p-5 backdrop-blur-[3px]; background: var(--app-overlay); }
.sr-only { @apply absolute size-px overflow-hidden whitespace-nowrap; clip: rect(0, 0, 0, 0); }

@media (max-width: 600px) {
  .site-header { min-height: 68px; padding: 8px 12px; }
  .brand-banner { width: min(70vw, 300px); }
  .connection, .profile-button > span:nth-child(2) { display: none; }
  .notification-stack { top: 76px; right: 16px; }
}
</style>
