<template>
  <main class="workspace-page page-scene" data-scene="study">
      <PageIntro title="重播紀錄" eyebrow="藏局回觀" description="回看每一次出牌，找出戰局轉折的一手。" back-to="/rooms" back-label="房間大廳" />
      <section class="replay-collection" aria-label="重播紀錄">
        <p v-if="replayLoading" class="page-notice" role="status" aria-live="polite">正在載入重播紀錄…</p>
        <p v-if="replayError" class="page-notice page-notice--error" role="alert">{{ replayError }} <button class="ghost-button" type="button" @click="loadReplays">重試</button></p>
        <p v-if="replayReplaceSource" class="page-notice">已達 10 局，選擇要替換的紀錄。</p>
        <article v-for="replay in replays" :key="replay.replayId" class="replay-card page-surface">
          <div><strong>{{ replay.roomName }}</strong><p>{{ replay.players.map(player => player.displayName).join('、') }} · {{ replay.finishedAt }} · {{ replayResultText(replay.result) }}</p></div>
          <div class="result-actions">
            <button class="ghost-button" type="button" @click="openReplay(replay.replayId)">觀看</button>
            <button class="ghost-button" type="button" @click="copyReplayLink(replay.replayId)">複製連結</button>
            <button v-if="replayReplaceSource" class="primary-button" type="button" @click="replaceReplay(replay.replayId)">以本局替換</button>
            <button v-else class="ghost-button" type="button" @click="requestDelete(replay.replayId)">刪除</button>
          </div>
        </article>
        <div v-if="!replays.length && !replayLoading && !replayError" class="page-empty page-surface"><h2>尚未儲存任何重播</h2><p>對局結束後儲存紀錄，就能在這裡再次回顧。</p><button class="secondary-button" @click="returnToLobby">前往房間</button></div>
      </section>
      <ConfirmDialog :open="Boolean(deleteTarget)" title="刪除此重播？" description="刪除後將無法從你的收藏中觀看這份紀錄。" confirm-label="刪除重播" :busy="deleteBusy" :error="deleteError" @close="deleteTarget = null" @confirm="deleteReplay" />
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
const deleteTarget = ref<string | null>(null)
const deleteBusy = ref(false)
const deleteError = ref('')

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

function requestDelete(replayId: string) { deleteError.value = ''; deleteTarget.value = replayId }

async function deleteReplay() {
  const replayId = deleteTarget.value
  if (!replayId || deleteBusy.value) return
  deleteBusy.value = true
  deleteError.value = ''
  try {
    await $fetch(`/api/replays/${encodeURIComponent(replayId)}`, { method: 'DELETE' })
    deleteTarget.value = null
    await loadReplays()
  } catch (error) {
    deleteError.value = presentApiError(error, '無法刪除重播')
  } finally {
    deleteBusy.value = false
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
.replay-collection { @apply grid gap-5; }
.result-actions { @apply flex flex-wrap gap-3; }
.result-actions .ghost-button { border-color: var(--app-border-strong); color: var(--app-text); }
.replay-card { @apply flex items-center justify-between gap-6 max-[800px]:grid; }
.replay-card > div { min-width: 0; }
.replay-card strong { @apply font-serif text-lg; color: var(--app-text); overflow-wrap: anywhere; }
.replay-card p { @apply mt-2 text-xs leading-6 text-muted; overflow-wrap: anywhere; }
.replay-card .result-actions { flex-shrink: 0; }
</style>
