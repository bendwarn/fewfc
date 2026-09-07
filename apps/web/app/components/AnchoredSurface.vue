<template>
  <Teleport to="body">
    <div
      v-if="open"
      :id="id"
      ref="surface"
      class="anchored-surface"
      :class="surfaceClass"
      :style="surfaceStyle"
      :role="role"
      :aria-labelledby="labelledby"
      :aria-describedby="describedby"
      @keydown="handleKeydown"
    >
      <slot />
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

type Placement = 'top-start' | 'top-end' | 'bottom-start' | 'bottom-end' | 'left-start' | 'left-end' | 'right-start' | 'right-end'
type Anchor = HTMLElement | AnchorRect | null

const props = withDefaults(defineProps<{
  open: boolean
  anchor: Anchor
  id?: string
  role?: string
  labelledby?: string
  describedby?: string
  placement?: Placement
  offset?: number
  boundaryPadding?: number
  surfaceClass?: string
  returnFocus?: boolean
}>(), {
  role: undefined,
  labelledby: undefined,
  describedby: undefined,
  placement: 'bottom-start',
  offset: 8,
  boundaryPadding: 12,
  surfaceClass: undefined,
  returnFocus: true,
})

const emit = defineEmits<{
  close: []
  keydown: [event: KeyboardEvent]
}>()

const surface = ref<HTMLElement | null>(null)
const position = reactive({ left: 0, top: 0, ready: false })
let resizeObserver: ResizeObserver | undefined
let frame: number | undefined
let returnFocusTarget: HTMLElement | null = null

const surfaceStyle = computed(() => ({
  left: `${position.left}px`,
  top: `${position.top}px`,
  visibility: position.ready ? ('visible' as const) : ('hidden' as const),
}))

function rectForAnchor(): AnchorRect | null {
  if (!props.anchor) return null
  if (props.anchor instanceof HTMLElement) {
    const rect = props.anchor.getBoundingClientRect()
    return { top: rect.top, right: rect.right, bottom: rect.bottom, left: rect.left }
  }
  return props.anchor
}

function schedulePosition() {
  if (frame !== undefined) cancelAnimationFrame(frame)
  frame = requestAnimationFrame(() => {
    frame = undefined
    updatePosition()
  })
}

function updatePosition() {
  if (!props.open || !surface.value || !props.anchor) return
  const anchor = rectForAnchor()
  if (!anchor) return

  const width = surface.value.offsetWidth
  const height = surface.value.offsetHeight
  const viewportWidth = window.innerWidth
  const viewportHeight = window.innerHeight
  const pad = props.boundaryPadding
  const gap = props.offset
  let placement = props.placement

  const availableBelow = viewportHeight - anchor.bottom - gap - pad
  const availableAbove = anchor.top - gap - pad
  if (placement.startsWith('bottom') && anchor.bottom + gap + height > viewportHeight - pad && availableAbove > availableBelow) {
    placement = placement.replace('bottom', 'top') as Placement
  } else if (placement.startsWith('top') && anchor.top - gap - height < pad && availableBelow > availableAbove) {
    placement = placement.replace('top', 'bottom') as Placement
  }

  let left = anchor.left
  let top = anchor.bottom + gap
  if (placement.startsWith('top')) top = anchor.top - height - gap
  if (placement.startsWith('left')) {
    left = anchor.left - width - gap
    top = placement.endsWith('start') ? anchor.top : anchor.bottom - height
  } else if (placement.startsWith('right')) {
    left = anchor.right + gap
    top = placement.endsWith('start') ? anchor.top : anchor.bottom - height
  } else if (placement.endsWith('end')) {
    left = anchor.right - width
  }

  if (placement.startsWith('left') && left < pad && anchor.right + gap + width <= viewportWidth - pad) {
    left = anchor.right + gap
  } else if (placement.startsWith('right') && left + width > viewportWidth - pad && anchor.left - gap - width >= pad) {
    left = anchor.left - width - gap
  }

  position.left = Math.min(Math.max(pad, left), Math.max(pad, viewportWidth - width - pad))
  position.top = Math.min(Math.max(pad, top), Math.max(pad, viewportHeight - height - pad))
  position.ready = true
}

function focusableElements() {
  return Array.from(surface.value?.querySelectorAll<HTMLElement>(
    'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
  ) ?? [])
}

function handleKeydown(event: KeyboardEvent) {
  emit('keydown', event)
  if (event.defaultPrevented) return
  if (event.key === 'Escape') {
    event.preventDefault()
    emit('close')
    return
  }
}

function handleDocumentPointerdown(event: PointerEvent) {
  const target = event.target
  if (!(target instanceof Node)) return
  if (surface.value?.contains(target)) return
  if (props.anchor instanceof HTMLElement && props.anchor.contains(target)) return
  emit('close')
}

function handleDocumentKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !props.open) return
  const target = event.target
  if (target instanceof Node && (surface.value?.contains(target) || (props.anchor instanceof HTMLElement && props.anchor.contains(target)))) return
  event.preventDefault()
  emit('close')
}

watch(() => props.open, async (open) => {
  if (open) {
    returnFocusTarget = document.activeElement instanceof HTMLElement ? document.activeElement : null
    position.ready = false
    await nextTick()
    resizeObserver?.disconnect()
    resizeObserver = typeof ResizeObserver === 'undefined' ? undefined : new ResizeObserver(schedulePosition)
    if (surface.value && resizeObserver) resizeObserver.observe(surface.value)
    if (props.anchor instanceof HTMLElement && resizeObserver) resizeObserver.observe(props.anchor)
    schedulePosition()
    document.addEventListener('pointerdown', handleDocumentPointerdown, true)
    document.addEventListener('keydown', handleDocumentKeydown, true)
    window.addEventListener('resize', schedulePosition)
    window.addEventListener('scroll', schedulePosition, true)
  } else {
    document.removeEventListener('pointerdown', handleDocumentPointerdown, true)
    document.removeEventListener('keydown', handleDocumentKeydown, true)
    window.removeEventListener('resize', schedulePosition)
    window.removeEventListener('scroll', schedulePosition, true)
    resizeObserver?.disconnect()
    resizeObserver = undefined
    position.ready = false
    if (props.returnFocus) void nextTick(() => returnFocusTarget?.focus())
  }
}, { immediate: true })

watch(() => props.anchor, async () => {
  if (!props.open) return
  await nextTick()
  if (resizeObserver) {
    resizeObserver.disconnect()
    if (surface.value) resizeObserver.observe(surface.value)
    if (props.anchor instanceof HTMLElement) resizeObserver.observe(props.anchor)
  }
  schedulePosition()
})

onBeforeUnmount(() => {
  if (frame !== undefined) cancelAnimationFrame(frame)
  document.removeEventListener('pointerdown', handleDocumentPointerdown, true)
  document.removeEventListener('keydown', handleDocumentKeydown, true)
  window.removeEventListener('resize', schedulePosition)
  window.removeEventListener('scroll', schedulePosition, true)
  resizeObserver?.disconnect()
})

defineExpose({ surface, updatePosition, focusableElements })
</script>

<style scoped>
.anchored-surface {
  position: fixed;
  z-index: 60;
  max-width: calc(100vw - 24px);
  max-height: calc(100dvh - 24px);
}
</style>
