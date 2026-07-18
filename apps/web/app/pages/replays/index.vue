<template>
  <main class="lobby-page">
      <section class="lobby-content" aria-label="重播紀錄">
        <div class="lobby-heading"><h1>重播紀錄</h1><button class="ghost-button" type="button" @click="returnToLobby">返回房間</button></div>
        <p v-if="replayLoading" class="muted" role="status" aria-live="polite">正在載入重播紀錄…</p>
        <p v-if="replayError" class="form-error" role="alert">{{ replayError }} <button class="ghost-button" type="button" @click="loadReplays">重試</button></p>
        <p v-if="replayReplaceSource" class="muted">已達 10 局，選擇要替換的紀錄。</p>
        <article v-for="replay in replays" :key="replay.replayId" class="room-card">
          <div><strong>{{ replay.roomName }}</strong><p>{{ replay.players.map(player => player.displayName).join('、') }} · {{ replay.finishedAt }} · {{ replayResultText(replay.result) }}</p></div>
          <div class="result-actions">
            <button class="ghost-button" type="button" @click="openReplay(replay.replayId)">觀看</button>
            <button class="ghost-button" type="button" @click="copyReplayLink(replay.replayId)">複製連結</button>
            <button v-if="replayReplaceSource" class="primary-button" type="button" @click="replaceReplay(replay.replayId)">以本局替換</button>
            <button v-else class="ghost-button" type="button" @click="deleteReplay(replay.replayId)">刪除</button>
          </div>
        </article>
        <p v-if="!replays.length && !replayLoading && !replayError" class="muted">尚未儲存任何重播。</p>
      </section>
  </main>
</template>

<script setup lang="ts">
import { presentApiError } from '~/lib/api-error-presentation'

interface ReplaySummary {
  replayId: string
  sourceGameId: string
  roomName: string
  players: Array<{ player: string, displayName: string }>
  result: unknown
  finishedAt: string
  savedAt: string
}

const route = useRoute()
const router = useRouter()
const replays = ref<ReplaySummary[]>([])
const replayLoading = ref(false)
const replayError = ref('')
const replayReplaceSource = ref<string | null>(null)

function replayResultText(result: unknown): string {
  if (!result || typeof result !== 'object') return '結果未知'
  return (result as { status?: unknown }).status === 'Finished' ? '已完成' : '結果已記錄'
}

function openReplay(replayId: string) {
  void router.push(`/replays/${encodeURIComponent(replayId)}`)
}

async function copyReplayLink(replayId: string) {
  if (!replayId) return
  await navigator.clipboard.writeText(new URL(`/replays/${encodeURIComponent(replayId)}`, window.location.origin).toString())
}

async function loadReplays() {
  replayLoading.value = true
  replayError.value = ''
  try {
    replays.value = (await $fetch<{ replays: ReplaySummary[] }>('/api/replays')).replays
    replayReplaceSource.value = typeof route.query.save === 'string' ? route.query.save : null
  } catch (error) {
    replayError.value = presentApiError(error, '無法取得重播紀錄')
  } finally {
    replayLoading.value = false
  }
}

async function replaceReplay(replaceReplayId: string) {
  const sourceGameId = replayReplaceSource.value
  if (!sourceGameId) return
  try {
    await $fetch('/api/replays', { method: 'POST', body: { sourceGameId, replaceReplayId } })
    replayReplaceSource.value = null
    await loadReplays()
  } catch (error) {
    replayError.value = presentApiError(error, '無法替換重播')
  }
}

async function deleteReplay(replayId: string) {
  if (!window.confirm('確定要刪除此收藏嗎？')) return
  try {
    await $fetch(`/api/replays/${encodeURIComponent(replayId)}`, { method: 'DELETE' })
    await loadReplays()
  } catch (error) {
    replayError.value = presentApiError(error, '無法刪除重播')
  }
}

function returnToLobby() {
  void router.push('/rooms')
}

onMounted(() => {
  void loadReplays()
})

watch(() => route.query.save, () => {
  void loadReplays()
})
</script>

<style scoped>
@reference "../../assets/css/main.css";
.lobby-page { @apply mx-auto max-w-[1180px] px-[30px] pt-15 pb-[90px] max-[600px]:px-4 max-[600px]:py-9; }
.lobby-content { @apply grid gap-5; }
.lobby-heading { @apply flex items-center justify-between gap-4; }
.lobby-heading h1 { @apply font-serif text-3xl text-gold-light; }
.result-actions { @apply mt-2 grid grid-cols-2 gap-3; }
.result-actions .ghost-button { @apply border-[#59635c] text-[#e8e4d8]; }
.room-card { @apply flex items-center justify-between gap-5 border border-line bg-panel p-5 max-[600px]:grid; }
.room-card strong { @apply text-lg text-[#ece8dd]; }
.room-card p { @apply mt-2 text-xs text-muted; }
@media (max-width: 600px) { .lobby-page { padding: 36px 16px; } }
</style>
