<template>
  <div class="app-shell">
    <header class="site-header">
      <button class="brand" type="button" aria-label="回到首頁" @click="router.push('/rooms')">
        <img class="brand-banner" src="/header-banner.svg" alt="五行戰鬥牌">
      </button>

      <nav class="header-actions" aria-label="帳號選單">
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
        <button class="notification-main" type="button" @click="openNotification(notification.gameId, notification.id)">
          <strong>{{ notification.message }}</strong>
          <span>進入房間</span>
        </button>
        <button class="notification-dismiss" type="button" aria-label="關閉通知" @click="notifications.dismiss(notification.id)">×</button>
      </article>
    </aside>
  </div>
</template>

<script setup lang="ts">
import { playerNotificationsKey } from '~/lib/player-notifications-context'
import { authClient } from '~/lib/auth-client'

const router = useRouter()
const route = useRoute()
const session = usePlayerSession()
const notifications = usePlayerNotifications()
const profileOpen = ref(false)
const profileMenuContainer = ref<HTMLElement | null>(null)
const playerInitial = computed(() => session.displayName.value.trim().charAt(0).toUpperCase() || 'A')
const visibleNotifications = computed(() => notifications.notifications.value.filter(notification => (
  typeof route.params.gameId !== 'string' || notification.gameId !== route.params.gameId
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
  session.clear()
  await router.replace('/login')
}

async function openNotification(gameId: string, notificationId: string) {
  notifications.dismiss(notificationId)
  await router.push(`/rooms/${encodeURIComponent(gameId)}`)
}

function closeProfileOnOutsideClick(event: MouseEvent) {
  if (profileOpen.value && event.target instanceof Node && !profileMenuContainer.value?.contains(event.target)) {
    profileOpen.value = false
  }
}

onMounted(() => {
  notifications.connect()
  window.addEventListener('click', closeProfileOnOutsideClick)
})

onBeforeUnmount(() => {
  window.removeEventListener('click', closeProfileOnOutsideClick)
  notifications.disconnect()
})
</script>

<style>
@reference "../assets/css/main.css";

.app-shell { @apply min-h-screen bg-ink; }
.site-header { @apply relative z-20 flex min-h-[84px] items-center justify-between border-b border-[#29322d] bg-[rgba(14,19,16,.96)]; padding: 10px clamp(14px, 4vw, 64px); }
.brand { @apply flex min-w-0 items-center border-0 bg-transparent p-0; }
.brand-banner { @apply block h-auto w-[min(52vw,456px)] max-w-full rounded-md shadow-[0_10px_28px_rgba(0,0,0,.32)]; }
.header-actions { @apply relative flex items-center gap-[18px]; }
.connection { @apply text-xs text-[#98a39c]; }
.connection i { display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: #62b585; margin-right: 6px; box-shadow: 0 0 8px #62b585; }
.connection.offline i { background: #c7a35d; box-shadow: none; }
.profile-menu-container { @apply relative; }
.profile-button { @apply flex items-center gap-2 border-0 bg-transparent; }
.profile-avatar { @apply grid size-[34px] place-items-center rounded-full bg-[#b48a47] font-extrabold text-[#141813]; }
.profile-menu { @apply absolute right-0 top-[calc(100%+10px)] z-30 grid min-w-32 overflow-hidden border border-[#64583f] bg-[#18201b] shadow-[0_12px_28px_rgba(0,0,0,.4)]; }
.profile-menu button { @apply border-0 bg-transparent px-4 py-2.5 text-left text-xs text-[#e8e4d8] hover:bg-[#28332c] hover:text-gold-light; }
.notification-stack { @apply fixed top-24 right-5 z-30 grid w-[min(360px,calc(100vw-32px))] gap-2; }
.notification-item { @apply grid grid-cols-[1fr_34px] border border-[#8e733d] bg-[#18201b] shadow-[0_12px_36px_rgba(0,0,0,.4)]; }
.notification-main { @apply grid gap-1 border-0 bg-transparent p-3 text-left; }
.notification-main strong { @apply text-xs text-[#ece8dd]; }
.notification-main span { @apply text-[10px] text-gold-light; }
.notification-dismiss { @apply border-0 border-l border-line bg-transparent text-muted; }
.primary-button { @apply inline-flex min-h-[50px] items-center justify-center border border-[#b99550] bg-[#b99550] px-5 font-bold text-[#141813] hover:bg-[#c9a451] disabled:cursor-not-allowed disabled:opacity-45; }
.secondary-button { @apply min-h-[50px] border border-[#4a554e] bg-transparent px-5 text-muted hover:border-[#b99550] hover:text-gold-light disabled:cursor-not-allowed disabled:opacity-45; }
.ghost-button { @apply min-h-10 border border-[#59635c] bg-transparent px-4 text-xs text-[#e8e4d8] hover:border-[#b99550] hover:text-gold-light; }
.setup-card { @apply border border-line bg-panel p-8 max-[600px]:px-[18px] max-[600px]:py-[22px]; }
.card-heading { @apply mb-8 flex gap-[18px]; }
.step-number { @apply grid size-[42px] place-items-center border border-[#7e693e] font-serif text-[#d3ae62]; }
.card-heading h2 { @apply mb-1 font-serif text-[21px]; }
.card-heading p { @apply text-xs text-muted; }
.text-input { @apply mb-[26px] h-12 border border-[#39443d] bg-[#111713] px-3.5 text-[#ece8dd]; }
.room-settings-layer { @apply fixed inset-0 z-40 grid place-items-center overflow-y-auto bg-[rgba(7,10,8,.76)] p-5 backdrop-blur-[3px]; }
.sr-only { @apply absolute size-px overflow-hidden whitespace-nowrap; clip: rect(0, 0, 0, 0); }

@media (max-width: 600px) {
  .site-header { min-height: 68px; padding: 8px 12px; }
  .brand-banner { width: min(70vw, 300px); }
  .connection, .profile-button > span:nth-child(2) { display: none; }
  .notification-stack { top: 76px; right: 16px; }
}
</style>
