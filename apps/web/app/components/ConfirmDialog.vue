<template>
  <ModalShell :open="open" :aria-label="title" :close-disabled="busy" panel-class="confirm-dialog-shell" initial-focus-selector=".confirm-cancel" @close="$emit('close')">
    <div class="confirm-dialog page-surface">
      <span class="page-kicker">確認操作</span>
      <h2>{{ title }}</h2>
      <p>{{ description }}</p>
      <p v-if="error" class="page-notice page-notice--error" role="alert">{{ error }}</p>
      <div class="confirm-actions">
        <button class="secondary-button confirm-cancel" type="button" :disabled="busy" @click="$emit('close')">取消</button>
        <button class="primary-button" type="button" :disabled="busy" @click="$emit('confirm')">{{ busy ? '處理中…' : confirmLabel }}</button>
      </div>
    </div>
  </ModalShell>
</template>
<script setup lang="ts">
import ModalShell from './ModalShell.vue'
withDefaults(defineProps<{ open: boolean; title: string; description: string; confirmLabel?: string; busy?: boolean; error?: string }>(), { confirmLabel: '確認', busy: false, error: '' })
defineEmits<{ close: []; confirm: [] }>()
</script>
<style>
.confirm-dialog-shell { width: min(460px,100%); }
.confirm-dialog { display: grid; gap: 18px; }
.confirm-dialog h2 { font: 700 24px / 1.5 var(--font-serif); }
.confirm-dialog > p:not(.page-notice) { font-size: 14px; line-height: 1.8; color: var(--app-text-muted); }
.confirm-actions { display: flex; justify-content: flex-end; gap: 12px; margin-top: 8px; }
</style>
