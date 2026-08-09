<template>
  <div class="room-settings-layer" @click.self="requestClose">
    <form
      class="setup-card room-settings-dialog"
      role="dialog"
      aria-modal="true"
      aria-labelledby="room-settings-title"
      @submit.prevent="$emit('submit')"
    >
      <div class="card-heading">
        <span class="step-number">+</span>
        <div>
          <h2 id="room-settings-title">建立房間</h2>
          <p>設定這場對戰的基本資訊。</p>
        </div>
      </div>

      <label for="room-name">房間名稱</label>
      <input
        id="room-name"
        ref="roomNameInput"
        :value="name"
        class="text-input"
        maxlength="24"
        @input="updateName"
      >

      <fieldset>
        <legend>對戰模式</legend>
        <div class="option-grid">
          <button
            v-for="option in modes"
            :key="option.id"
            type="button"
            class="mode-option"
            :class="{ selected: mode === option.id }"
            @click="$emit('update:mode', option.id)"
          >
            <span class="mode-icon">{{ option.icon }}</span>
            <strong>{{ option.label }}</strong>
            <small>{{ option.description }}</small>
          </button>
        </div>
      </fieldset>

      <fieldset>
        <legend>房間權限</legend>
        <div class="segmented">
          <button
            type="button"
            :class="{ active: access === 'private' }"
            @click="$emit('update:access', 'private')"
          >
            私人房間
          </button>
          <button
            type="button"
            :class="{ active: access === 'public' }"
            @click="$emit('update:access', 'public')"
          >
            公開房間
          </button>
        </div>
      </fieldset>

      <div class="setup-summary">
        <div>
          <span>目前設定</span>
          <strong>{{ roomModeLabel }} · {{ access === 'private' ? '私人房間' : '公開房間' }}</strong>
        </div>
        <p>{{ access === 'public' ? '公開房間會顯示於可加入清單。' : '私人房間僅能透過邀請連結或房碼加入。' }}</p>
      </div>

      <p v-if="error" class="form-error">{{ error }}</p>

      <div class="setup-actions">
        <button class="secondary-button" type="button" :disabled="busy" @click="requestClose">
          取消
        </button>
        <button class="primary-button start-button" type="submit" :disabled="busy">
          {{ busy ? '處理中…' : '建立房間' }} <span>→</span>
        </button>
      </div>
    </form>
  </div>
</template>

<script setup lang="ts">
type RoomMode = 'duel' | 'team'

const props = defineProps<{
  name: string
  mode: RoomMode
  access: 'private' | 'public'
  busy: boolean
  error?: string
}>()

const emit = defineEmits<{
  close: []
  submit: []
  'update:name': [name: string]
  'update:mode': [mode: RoomMode]
  'update:access': [access: 'private' | 'public']
}>()

const roomNameInput = ref<HTMLInputElement | null>(null)
const modes: ReadonlyArray<{ id: RoomMode, icon: string, label: string, description: string }> = [
  { id: 'duel', icon: '雙', label: '雙人對戰', description: '1 對 1 經典規則' },
  { id: 'team', icon: '隊', label: '團隊對戰', description: '2 對 2 交錯行動' },
]
const roomModeLabel = computed(() => modes.find(option => option.id === props.mode)?.label ?? '')

onMounted(async () => {
  window.addEventListener('keydown', handleKeydown)
  await nextTick()
  roomNameInput.value?.focus()
})

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleKeydown)
})

function updateName(event: Event) {
  emit('update:name', (event.target as HTMLInputElement).value)
}

function requestClose() {
  if (!props.busy) emit('close')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape') return
  event.preventDefault()
  requestClose()
}
</script>

<style scoped>
@reference "../assets/css/main.css";

.room-settings-dialog { @apply my-auto w-full max-w-[720px] shadow-[0_24px_70px_rgba(0,0,0,.5)]; }
fieldset { @apply mb-[26px] border-0 p-0; }
.option-grid { @apply grid grid-cols-2 gap-3 max-[600px]:grid-cols-1; }
.mode-option { @apply relative grid min-h-27 grid-cols-[42px_1fr] p-4 text-left; border: 1px solid var(--app-border); border-radius: 12px; background: var(--app-surface-muted); color: var(--app-text); }
.mode-option.selected { border-color: var(--app-accent); background: var(--app-accent-soft); box-shadow: inset 0 0 0 1px var(--app-accent); }
.mode-icon { @apply row-span-2 grid size-8 place-items-center rounded-full; background: var(--app-control); color: var(--app-accent-strong); }
.mode-option small { color: var(--app-text-muted); }
.segmented { @apply grid grid-cols-2 p-1; border-radius: 10px; background: var(--app-surface-muted); }
.segmented button { @apply min-h-10 border-0 bg-transparent text-muted; }
.segmented button.active { background: var(--app-surface-raised); color: var(--app-accent-strong); box-shadow: var(--app-shadow-sm); }
.setup-summary { @apply mb-5 grid gap-2 p-4; border: 1px solid var(--app-border); border-radius: 10px; background: var(--app-surface-muted); }
.setup-summary div { @apply flex items-center justify-between gap-4 max-[600px]:grid; }
.setup-summary span { @apply text-[10px] tracking-[.18em] text-muted; }
.setup-summary strong { @apply text-sm text-gold-light; }
.setup-summary p { @apply text-xs text-muted; }
.start-button { @apply w-full; }
.setup-actions { @apply mt-5 grid grid-cols-[auto_1fr] gap-3; }

@media (max-width: 600px) {
  .room-settings-dialog { padding: 22px 18px; }
  .option-grid { grid-template-columns: 1fr; }
}
</style>
