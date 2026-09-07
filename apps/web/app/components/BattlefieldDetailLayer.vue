<template>
  <Teleport to="body">
    <button
      v-if="open && !compact"
      class="battlefield-detail-backdrop battlefield-detail-desktop-backdrop"
      type="button"
      tabindex="-1"
      aria-label="關閉詳情"
      @click="emit('close')"
    />
  </Teleport>

  <AnchoredSurface
    v-if="open && !compact"
    :open="open && !compact"
    :anchor="anchorElement ?? anchor"
    placement="bottom-start"
    surface-class="battlefield-detail-anchored"
    :return-focus="false"
    @close="emit('close')"
  >
    <section
      :id="id"
      ref="panel"
      class="battlefield-detail-panel"
      role="dialog"
      :aria-labelledby="`${id}-title`"
      tabindex="-1"
      @keydown="handleKeydown"
    >
      <header>
        <h2 :id="`${id}-title`">{{ title }}</h2>
        <button ref="closeButton" type="button" aria-label="關閉詳情" @click="emit('close')">×</button>
      </header>
      <div class="battlefield-detail-content">
        <slot />
      </div>
    </section>
  </AnchoredSurface>

  <Teleport to="body">
    <div v-if="open && compact" class="battlefield-detail-portal" :class="`detail-${kind}`">
      <button class="battlefield-detail-backdrop" type="button" tabindex="-1" aria-label="關閉詳情" @click="emit('close')" />
      <section
        :id="id"
        ref="panel"
        class="battlefield-detail-panel"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="`${id}-title`"
        tabindex="-1"
        @keydown="handleKeydown"
      >
        <header>
          <h2 :id="`${id}-title`">{{ title }}</h2>
          <button ref="closeButton" type="button" aria-label="關閉詳情" @click="emit('close')">×</button>
        </header>
        <div class="battlefield-detail-content">
          <slot />
        </div>
      </section>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
interface AnchorRect {
  top: number
  right: number
  bottom: number
  left: number
}

const props = defineProps<{
  open: boolean
  id: string
  title: string
  kind: 'discard' | 'effects' | 'hand'
  anchor: AnchorRect | null
  anchorElement?: HTMLElement | null
}>()

const emit = defineEmits<{ close: [] }>()
const panel = ref<HTMLElement | null>(null)
const closeButton = ref<HTMLButtonElement | null>(null)
const compact = ref(import.meta.client && window.matchMedia('(max-width: 1199px)').matches)

function updateViewport() {
  compact.value = window.matchMedia('(max-width: 1199px)').matches
}

function focusableElements() {
  return Array.from(panel.value?.querySelectorAll<HTMLElement>(
    'button:not([disabled]), [href], [tabindex]:not([tabindex="-1"])',
  ) ?? [])
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
  if (event.key !== 'Tab' || !compact.value) return

  const focusable = focusableElements()
  if (!focusable.length) return
  const first = focusable[0]!
  const last = focusable.at(-1)!
  if (event.shiftKey && document.activeElement === first) {
    event.preventDefault()
    last.focus()
  } else if (!event.shiftKey && document.activeElement === last) {
    event.preventDefault()
    first.focus()
  }
}

watch(() => props.open, (open) => {
  if (open) void nextTick(() => closeButton.value?.focus())
})

onMounted(() => {
  updateViewport()
  window.addEventListener('resize', updateViewport)
})

onBeforeUnmount(() => window.removeEventListener('resize', updateViewport))
</script>

<style scoped>
@reference "../assets/css/main.css";

.battlefield-detail-portal { @apply fixed inset-0 z-50; }
.battlefield-detail-backdrop { @apply absolute inset-0 size-full cursor-default border-0 bg-transparent p-0; }
.battlefield-detail-anchored { z-index: 60; }
.battlefield-detail-desktop-backdrop { position: fixed; z-index: 59; inset: 0; background: transparent; }
.battlefield-detail-panel { @apply relative grid max-h-[min(520px,calc(100dvh-32px))] w-[340px] grid-rows-[auto_minmax(0,1fr)] overflow-hidden border border-[var(--app-accent)] bg-[var(--app-surface-raised)] text-[var(--app-text)] shadow-[0_18px_48px_rgba(0,0,0,.52)]; }
.battlefield-detail-panel > header { @apply flex items-center justify-between gap-3 border-b border-[var(--app-border)] px-4 py-3; }
.battlefield-detail-panel h2 { @apply min-w-0 truncate font-serif text-sm text-gold-light; }
.battlefield-detail-panel header button { @apply grid size-8 shrink-0 place-items-center border border-[var(--app-border-strong)] bg-transparent text-xl leading-none text-muted hover:border-[var(--app-accent)] hover:text-gold-light; }
.battlefield-detail-content { @apply min-h-0 overflow-y-auto p-4; overscroll-behavior: contain; }

@media (max-width: 1199px) {
  .battlefield-detail-backdrop { @apply bg-[var(--app-overlay)] backdrop-blur-[3px]; }
  .battlefield-detail-panel { @apply fixed right-0 bottom-0 left-0 max-h-[min(72dvh,620px)] w-full rounded-t-2xl border-x-0 border-b-0; }
  .battlefield-detail-panel::before { content: ''; @apply absolute top-1.5 left-1/2 h-1 w-10 -translate-x-1/2 rounded-full bg-[var(--app-border-strong)]; }
  .battlefield-detail-panel > header { @apply pt-4; }
}

@media (prefers-reduced-motion: no-preference) and (max-width: 1199px) {
  .battlefield-detail-panel { animation: battlefield-sheet-in 160ms ease-out; }
}

@keyframes battlefield-sheet-in {
  from { transform: translateY(18px); opacity: .7; }
}
</style>
