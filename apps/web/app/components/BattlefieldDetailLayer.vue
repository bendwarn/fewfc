<template>
  <Teleport to="body">
    <div v-if="open" class="battlefield-detail-portal" :class="`detail-${kind}`">
      <button class="battlefield-detail-backdrop" type="button" tabindex="-1" aria-label="關閉詳情" @click="emit('close')" />
      <section
        :id="id"
        ref="panel"
        class="battlefield-detail-panel"
        :style="panelPosition"
        role="dialog"
        :aria-modal="compact ? 'true' : undefined"
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
}>()

const emit = defineEmits<{ close: [] }>()
const panel = ref<HTMLElement | null>(null)
const closeButton = ref<HTMLButtonElement | null>(null)
const compact = ref(false)
const viewport = reactive({ width: 0, height: 0 })

const panelPosition = computed(() => {
  if (compact.value || !props.anchor) return undefined
  const width = 340
  const estimatedHeight = props.kind === 'effects' ? 280 : 360
  const left = Math.min(
    Math.max(16, props.anchor.left),
    Math.max(16, viewport.width - width - 16),
  )
  const top = Math.min(
    Math.max(16, props.anchor.bottom + 8),
    Math.max(16, viewport.height - estimatedHeight - 16),
  )
  return { left: `${left}px`, top: `${top}px` }
})

function updateViewport() {
  compact.value = window.matchMedia('(max-width: 1199px)').matches
  viewport.width = window.innerWidth
  viewport.height = window.innerHeight
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
.battlefield-detail-panel { @apply fixed z-1 grid max-h-[min(520px,calc(100dvh-32px))] w-[340px] grid-rows-[auto_minmax(0,1fr)] overflow-hidden border border-[var(--app-accent)] bg-[var(--app-surface-raised)] text-[var(--app-text)] shadow-[0_18px_48px_rgba(0,0,0,.52)]; }
.battlefield-detail-panel > header { @apply flex items-center justify-between gap-3 border-b border-[var(--app-border)] px-4 py-3; }
.battlefield-detail-panel h2 { @apply min-w-0 truncate font-serif text-sm text-gold-light; }
.battlefield-detail-panel header button { @apply grid size-8 shrink-0 place-items-center border border-[var(--app-border-strong)] bg-transparent text-xl leading-none text-muted hover:border-[var(--app-accent)] hover:text-gold-light; }
.battlefield-detail-content { @apply min-h-0 overflow-y-auto p-4; overscroll-behavior: contain; }

@media (max-width: 1199px) {
  .battlefield-detail-backdrop { @apply bg-[var(--app-overlay)] backdrop-blur-[3px]; }
  .battlefield-detail-panel { @apply right-0 bottom-0 left-0 max-h-[min(72dvh,620px)] w-full rounded-t-2xl border-x-0 border-b-0; }
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
