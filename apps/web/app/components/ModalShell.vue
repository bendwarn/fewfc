<template>
  <Teleport to="body">
    <div
      v-if="open"
      ref="shell"
      class="modal-shell"
      :class="rootClass"
      @click.self="requestClose"
    >
      <button
        v-if="!closeDisabled"
        class="modal-shell-backdrop"
        type="button"
        tabindex="-1"
        aria-label="關閉視窗"
        @click="requestClose"
      />
      <div v-else class="modal-shell-backdrop" aria-hidden="true" />
      <section
        ref="panel"
        class="modal-shell-panel"
        :class="panelClass"
        role="dialog"
        aria-modal="true"
        :aria-labelledby="labelledby"
        :aria-describedby="describedby"
        :aria-label="ariaLabel"
        tabindex="-1"
        @keydown="handleKeydown"
      >
        <slot />
      </section>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
const props = withDefaults(defineProps<{
  open: boolean
  labelledby?: string
  describedby?: string
  ariaLabel?: string
  rootClass?: string
  panelClass?: string
  initialFocusSelector?: string
  closeDisabled?: boolean
}>(), {
  labelledby: undefined,
  describedby: undefined,
  ariaLabel: undefined,
  rootClass: undefined,
  panelClass: undefined,
  initialFocusSelector: undefined,
  closeDisabled: false,
})

const emit = defineEmits<{ close: [] }>()
const shell = ref<HTMLElement | null>(null)
const panel = ref<HTMLElement | null>(null)
let returnFocusTarget: HTMLElement | null = null
const inertSiblings = new Map<HTMLElement, { inert: boolean, ariaHidden: string | null }>()

function focusableElements() {
  return Array.from(panel.value?.querySelectorAll<HTMLElement>(
    'button:not([disabled]), [href], input:not([disabled]), select:not([disabled]), textarea:not([disabled]), [tabindex]:not([tabindex="-1"])',
  ) ?? [])
}

function focusInitialControl() {
  const target = props.initialFocusSelector
    ? panel.value?.querySelector<HTMLElement>(props.initialFocusSelector)
    : undefined
  ;(target ?? focusableElements()[0] ?? panel.value)?.focus()
}

function setBackgroundInert(inert: boolean) {
  if (inert) {
    inertSiblings.clear()
    let current = shell.value
    while (current?.parentElement) {
      const container = current.parentElement
      for (const child of Array.from(container.children)) {
        if (!(child instanceof HTMLElement) || child === current || inertSiblings.has(child)) continue
        inertSiblings.set(child, { inert: child.inert, ariaHidden: child.getAttribute('aria-hidden') })
        child.inert = true
        child.setAttribute('aria-hidden', 'true')
      }
      if (container === document.body) break
      current = container
    }
    return
  }
  for (const [child, previous] of inertSiblings) {
    child.inert = previous.inert
    if (previous.ariaHidden === null) child.removeAttribute('aria-hidden')
    else child.setAttribute('aria-hidden', previous.ariaHidden)
  }
  inertSiblings.clear()
}

function requestClose() {
  if (!props.closeDisabled) emit('close')
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    event.preventDefault()
    event.stopPropagation()
    requestClose()
    return
  }
  if (event.key !== 'Tab') return
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

function handleWindowKeydown(event: KeyboardEvent) {
  if (event.key !== 'Escape' || !props.open) return
  event.preventDefault()
  requestClose()
}

watch(() => props.open, async (open) => {
  if (open) {
    returnFocusTarget = document.activeElement instanceof HTMLElement ? document.activeElement : null
    await nextTick()
    // 在父元件以 v-if 建立本元件時，第一次 nextTick 可能仍早於 root ref。
    if (!shell.value) await nextTick()
    setBackgroundInert(true)
    focusInitialControl()
    window.addEventListener('keydown', handleWindowKeydown)
  } else {
    window.removeEventListener('keydown', handleWindowKeydown)
    setBackgroundInert(false)
    void nextTick(() => returnFocusTarget?.focus())
  }
}, { immediate: true })

onBeforeUnmount(() => {
  window.removeEventListener('keydown', handleWindowKeydown)
  setBackgroundInert(false)
  if (props.open) void nextTick(() => returnFocusTarget?.focus())
})
</script>

<style scoped>
.modal-shell {
  position: fixed;
  z-index: 70;
  inset: 0;
  display: grid;
  place-items: center;
  overflow-y: auto;
  padding: 20px;
}
.modal-shell-backdrop {
  position: absolute;
  inset: 0;
  width: 100%;
  height: 100%;
  border: 0;
  padding: 0;
  cursor: default;
  background: var(--app-overlay);
  backdrop-filter: blur(3px);
}
.modal-shell-panel {
  position: relative;
  z-index: 1;
  max-height: calc(100dvh - 40px);
  max-width: 100%;
  overflow-y: auto;
}
</style>
