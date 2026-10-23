<template>
  <PlayerShell
    :display-name="session.displayName.value"
    :connection-state="notifications.connectionState.value"
    :notifications="visibleNotifications"
    @home="open('/rooms')"
    @navigate="open"
    @logout="logout"
    @open-notification="openNotification"
    @dismiss="notifications.dismiss($event)"
  ><slot /></PlayerShell>
</template>

<script setup lang="ts">
import { playerNotificationsKey } from '~/lib/player-notifications-context'
import { authClient } from '~/lib/auth-client'

const router = useRouter()
const route = useRoute()
const session = usePlayerSession()
const notifications = usePlayerNotifications()
const visibleNotifications = computed(() => notifications.visibleNotifications.value.filter(notification => (
  typeof route.params.gameId !== 'string'
    || notification.gameId !== route.params.gameId
    || notification.kind === 'seatPromoted'
)))

provide(playerNotificationsKey, notifications)

function open(path: string) {
  void router.push(path)
}

async function logout() {
  await authClient.signOut()
  notifications.disconnect()
  notifications.setUserId('')
  session.clear()
  await router.replace('/login')
}

async function openNotification(gameId: string) {
  await router.push(`/rooms/${encodeURIComponent(gameId)}`)
}

onMounted(() => {
  notifications.setUserId(session.userId.value)
  notifications.connect()
})

watch(() => session.userId.value, (userId) => {
  notifications.setUserId(userId)
  if (userId) notifications.connect()
})

onBeforeUnmount(() => {
  notifications.disconnect()
})
</script>
