<template>
  <ModalShell
    :open="true"
    panel-class="room-settings-dialog-shell"
    labelledby="room-settings-title"
    initial-focus-selector="#room-name"
    :close-disabled="busy"
    @close="requestClose"
  >
    <form
      class="setup-card room-settings-dialog lobby-theme battle-glass"
      @submit.prevent="$emit('submit')"
    >
      <div class="card-heading">
        <h2 id="room-settings-title">建立房間</h2>
      </div>

      <label for="room-name">房間名稱</label>
      <input
        id="room-name"
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
            :aria-pressed="mode === option.id"
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
            :aria-pressed="access === 'private'"
            @click="$emit('update:access', 'private')"
          >
            私人房間
          </button>
          <button
            type="button"
            :class="{ active: access === 'public' }"
            :aria-pressed="access === 'public'"
            @click="$emit('update:access', 'public')"
          >
            公開房間
          </button>
        </div>
      </fieldset>

      <fieldset :disabled="busy">
        <legend>對局計時</legend>
        <div class="segmented">
          <button type="button" :class="{ active: !operationTimeLimitSeconds }" :aria-pressed="!operationTimeLimitSeconds" @click="$emit('update:operationTimeLimitSeconds', undefined)">
            不限時
          </button>
          <button type="button" :class="{ active: operationTimeLimitSeconds }" :aria-pressed="Boolean(operationTimeLimitSeconds)" @click="enableTimeLimit">
            限時模式
          </button>
        </div>
        <template v-if="operationTimeLimitSeconds">
          <label class="time-limit-label" for="room-time-limit">操作時限</label>
          <select id="room-time-limit" class="text-input" :value="operationTimeLimitSeconds" aria-describedby="room-time-limit-help" @change="updateTimeLimit">
            <option v-for="seconds in roomTimeLimits" :key="seconds" :value="seconds">{{ seconds }} 秒</option>
          </select>
          <p id="room-time-limit-help" class="time-limit-help">準備選牌、回合行動與待選擇事項各自計時。斷線仍會計時，逾時隊伍判負，兩隊同時逾時則為平局。</p>
        </template>
      </fieldset>

      <p>規則版本 {{ ruleVersion ?? '5.17' }}</p>
      <details>
        <summary>選擇過往規則版本</summary>
        <label v-for="version in (['5.17', '5.16'] as const)" :key="version">
          <input type="radio" name="create-rule-version" :checked="(ruleVersion ?? '5.17') === version" :disabled="busy" @change="$emit('update:ruleVersion', version)">
          {{ version }}
        </label>
      </details>
      <div class="setup-summary">
        <div>
          <span>目前設定</span>
          <strong>{{ roomModeLabel }} · {{ access === 'private' ? '私人房間' : '公開房間' }} · {{ operationTimeLimitSeconds ? `限時 ${operationTimeLimitSeconds} 秒` : '不限時' }}</strong>
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
  </ModalShell>
</template>

<script setup lang="ts">
import { computed } from 'vue'
import ModalShell from './ModalShell.vue'
import { isRoomTimeLimit, roomTimeLimits, type RoomTimeLimit } from '#shared/game-timing'
type RoomMode = 'duel' | 'team'

const props = defineProps<{
  ruleVersion?: '5.16' | '5.17'
  name: string
  mode: RoomMode
  access: 'private' | 'public'
  operationTimeLimitSeconds?: RoomTimeLimit
  busy: boolean
  error?: string
}>()

const emit = defineEmits<{
  close: []
  submit: []
  'update:ruleVersion': [ruleVersion: '5.16' | '5.17']
  'update:name': [name: string]
  'update:mode': [mode: RoomMode]
  'update:access': [access: 'private' | 'public']
  'update:operationTimeLimitSeconds': [seconds: RoomTimeLimit | undefined]
}>()

const modes: ReadonlyArray<{ id: RoomMode, icon: string, label: string, description: string }> = [
  { id: 'duel', icon: '雙', label: '雙人對戰', description: '1 對 1 經典規則' },
  { id: 'team', icon: '隊', label: '團隊對戰', description: '2 對 2 交錯行動' },
]
const roomModeLabel = computed(() => modes.find(option => option.id === props.mode)?.label ?? '')

function updateName(event: Event) {
  emit('update:name', (event.target as HTMLInputElement).value)
}

function requestClose() {
  if (!props.busy) emit('close')
}

function enableTimeLimit() {
  if (!props.operationTimeLimitSeconds) emit('update:operationTimeLimitSeconds', 60)
}

function updateTimeLimit(event: Event) {
  const seconds = Number((event.target as HTMLSelectElement).value)
  if (isRoomTimeLimit(seconds)) emit('update:operationTimeLimitSeconds', seconds)
}

</script>

<style scoped>
@reference "../assets/css/main.css";

:global(.room-settings-dialog-shell) { width: min(720px, 100%); }
.room-settings-dialog { margin: auto; width: 100%; max-width: 720px; padding: 24px 28px; border: 1px solid var(--glass-stroke); border-radius: 28px; background: var(--battle-glass-fill); box-shadow: var(--battle-glass-shadow); }
.card-heading { margin-bottom: 16px; padding-bottom: 12px; border-bottom: 1px solid color-mix(in srgb, var(--glass-stroke) 40%, transparent); }
.room-settings-dialog > label, legend { display: block; margin-bottom: 9px; font-size: 13px; font-weight: 600; }
.text-input { width: 100%; height: 44px; margin-bottom: 16px; border-color: var(--glass-stroke); background: color-mix(in srgb, var(--app-input) 55%, transparent); border-radius: 12px; }
select.text-input { margin-bottom: 0; }
.room-settings-dialog :is(input, button, summary, select):focus-visible { outline: 2px solid var(--app-accent-strong); outline-offset: 3px; }
fieldset { @apply mb-4 border-0 p-0; }
.option-grid { @apply grid grid-cols-2 gap-3 max-[600px]:grid-cols-1; }
.mode-option { @apply relative grid min-h-20 grid-cols-[42px_1fr] p-3 text-left; border: 1px solid var(--glass-stroke); border-radius: 16px; background: color-mix(in srgb, var(--glass-surface-strong) 65%, transparent); color: var(--app-text); }
.mode-option.selected { border-color: var(--app-accent); background: color-mix(in srgb, var(--app-accent) 16%, var(--glass-surface-strong)); box-shadow: inset 0 0 0 1px var(--app-accent), 0 0 20px color-mix(in srgb, var(--app-accent) 15%, transparent); }
.mode-icon { @apply row-span-2 grid size-8 place-items-center rounded-full; background: var(--app-control); color: var(--app-accent-strong); }
.mode-option small { color: var(--battle-text-secondary); }
.segmented { @apply grid grid-cols-2 p-1; border: 1px solid var(--glass-stroke); border-radius: 14px; background: color-mix(in srgb, var(--glass-surface-strong) 65%, transparent); }
.segmented button { @apply min-h-10 border-0 bg-transparent text-muted; }
.segmented button.active { border-radius: 10px; background: color-mix(in srgb, var(--app-accent) 14%, transparent); color: var(--app-accent-strong); box-shadow: inset 0 1px color-mix(in srgb, var(--glass-stroke) 50%, transparent); }
.setup-summary { @apply grid gap-2 p-4; border: 1px solid var(--glass-stroke); border-radius: 16px; background: color-mix(in srgb, var(--glass-surface-strong) 65%, transparent); }
.setup-summary div { @apply flex items-center justify-between gap-4 max-[600px]:grid; }
.setup-summary span { @apply text-[10px] tracking-[.18em] text-muted; }
.setup-summary strong { @apply text-sm text-gold-light; }
.setup-summary p { font-size: 12px; color: var(--battle-text-secondary); }
.room-settings-dialog > p { font-size: 13px; }
.time-limit-label { display: block; margin: 12px 0 6px; font-size: 13px; }
.time-limit-help { margin: 8px 0 0; color: var(--battle-text-secondary); font-size: 12px; }
details { margin: 8px 0 14px; color: var(--battle-text-secondary); font-size: 12px; }
summary { width: fit-content; cursor: pointer; padding: 4px 0; }
details label { display: inline-flex; align-items: center; gap: 7px; margin: 8px 18px 0 0; }
.primary-button { border-radius: 14px; border-color: #ead69e; color: #28372c; background: linear-gradient(140deg, #f1e3b6, #c5a463); box-shadow: inset 0 1px rgba(255,255,239,.65); }
.primary-button:hover:not(:disabled) { background: linear-gradient(140deg, #fff1c5, #dac080); }
.secondary-button { border-radius: 14px; border-color: var(--glass-stroke); color: var(--app-text); }
.start-button { @apply w-full; }
.setup-actions { @apply mt-4 grid grid-cols-[auto_1fr] gap-3; }

@media (max-width: 600px) {
  .room-settings-dialog { padding: 18px; }
  .option-grid { grid-template-columns: 1fr; }
}
</style>
