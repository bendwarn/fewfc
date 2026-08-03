<template>
  <component
    :is="selectable ? 'button' : 'span'"
    v-bind="attrs"
    :type="selectable ? 'button' : undefined"
    class="playing-card game-card"
    :class="[
      cardElementClass(card?.element),
      {
        hidden,
        selected,
        'is-disabled': disabled,
        'has-interpretation': interpretation,
      },
    ]"
    :disabled="selectable && disabled ? true : undefined"
    :role="selectable ? undefined : 'img'"
    :aria-label="accessibleLabel"
    :aria-pressed="selectable ? selected : undefined"
    :aria-keyshortcuts="selectable ? shortcut : undefined"
    @click="activate"
    @pointerdown="startLongPress"
    @pointerup="finishLongPress"
    @pointercancel="cancelLongPress"
    @pointerleave="cancelLongPress"
    @contextmenu.prevent
    @dragstart.prevent
  >
    <img
      v-if="imagePath && !imageFailed"
      class="game-card-image"
      :src="imagePath"
      alt=""
      draggable="false"
      @error="imageFailed = true"
    >
    <span v-else class="game-card-fallback" aria-hidden="true">
      <span v-if="!hidden" class="card-level">{{ card?.level ?? '' }}</span>
      <span v-if="!hidden" class="card-element">{{ cardElementGlyph(card?.element) }}</span>
    </span>
    <span v-if="selected" class="game-card-selected" aria-hidden="true">✓ <b>已選</b></span>
    <span
      v-if="interpretation"
      class="card-interpretation-badge"
      :title="interpretation.description"
    >{{ interpretation.label }}</span>
  </component>

  <Teleport to="body">
    <div v-if="previewVisible" class="game-card-preview-layer" aria-hidden="true">
      <div class="game-card-preview">
        <img v-if="imagePath && !imageFailed" :src="imagePath" alt="" draggable="false">
        <div v-else class="game-card-preview-fallback" :class="cardElementClass(card?.element)">
          <span v-if="!hidden">{{ cardElementGlyph(card?.element) }} {{ card?.level }}</span>
          <span v-else>牌背</span>
        </div>
        <span v-if="interpretation" class="card-interpretation-badge">{{ interpretation.label }}</span>
      </div>
    </div>
  </Teleport>
</template>

<script setup lang="ts">
import type { CardInterpretationPresentation, PublicCard } from '~/types/fewfc'
import {
  CARD_BACK_IMAGE_PATH,
  cardElementClass,
  cardElementGlyph,
  cardFaceImagePath,
} from '~/lib/card-face-presentation'
import { cardInterpretationBadge } from '~/lib/card-interpretation-presentation'

defineOptions({ inheritAttrs: false })

const props = withDefaults(defineProps<{
  card?: PublicCard | null
  hidden?: boolean
  selectable?: boolean
  selected?: boolean
  disabled?: boolean
  shortcut?: string
  interpretations?: CardInterpretationPresentation[]
}>(), {
  card: null,
  hidden: false,
  selectable: false,
  selected: false,
  disabled: false,
  shortcut: undefined,
  interpretations: () => [],
})

const emit = defineEmits<{ select: [] }>()
const attrs = useAttrs()
const imageFailed = ref(false)
const previewVisible = ref(false)
const suppressNextClick = ref(false)
const interpretation = computed(() => props.card
  ? cardInterpretationBadge(props.card, props.interpretations)
  : null)
const imagePath = computed(() => props.hidden
  ? CARD_BACK_IMAGE_PATH
  : cardFaceImagePath(props.card?.element, props.card?.level))
const accessibleLabel = computed(() => {
  if (props.hidden) return '牌背'
  const parts = [props.card?.label ?? '一張牌']
  if (interpretation.value) parts.push(interpretation.value.description)
  if (props.selected) parts.push('已選')
  return parts.join('。')
})

let longPressTimer: ReturnType<typeof setTimeout> | undefined
let clickSuppressionTimer: ReturnType<typeof setTimeout> | undefined
let longPressActivated = false

function clearLongPressTimer() {
  clearTimeout(longPressTimer)
  longPressTimer = undefined
}

function suppressUpcomingClick() {
  clearTimeout(clickSuppressionTimer)
  suppressNextClick.value = true
  clickSuppressionTimer = setTimeout(() => {
    suppressNextClick.value = false
  })
}

function startLongPress(event: PointerEvent) {
  if (event.button !== 0 || !imagePath.value) return
  clearLongPressTimer()
  longPressActivated = false
  longPressTimer = setTimeout(() => {
    longPressActivated = true
    previewVisible.value = true
  }, 480)
}

function finishLongPress(event: PointerEvent) {
  clearLongPressTimer()
  if (!longPressActivated) return
  event.preventDefault()
  suppressUpcomingClick()
  previewVisible.value = false
  longPressActivated = false
}

function cancelLongPress() {
  clearLongPressTimer()
  if (longPressActivated) suppressUpcomingClick()
  previewVisible.value = false
  longPressActivated = false
}

function activate(event: MouseEvent) {
  if (suppressNextClick.value) {
    event.preventDefault()
    event.stopPropagation()
    suppressNextClick.value = false
    return
  }
  if (props.selectable && !props.disabled) emit('select')
}

onBeforeUnmount(() => {
  clearLongPressTimer()
  clearTimeout(clickSuppressionTimer)
})
</script>

<style scoped>
.game-card {
  --card-face: #ded8c8;
  --card-face-light: #f0eadc;
  --card-border: #79715e;
  --card-ink: #18201c;
  position: relative;
  display: flex;
  width: clamp(62px, 7vw, 92px);
  aspect-ratio: 627 / 949;
  flex: 0 0 auto;
  align-items: center;
  justify-content: center;
  overflow: visible;
  border: 1px solid var(--card-border);
  border-radius: 7px;
  background: linear-gradient(145deg, var(--card-face-light), var(--card-face));
  color: var(--card-ink);
  padding: 0;
  box-shadow: 0 5px 15px rgba(0, 0, 0, .25);
  touch-action: manipulation;
  user-select: none;
  -webkit-touch-callout: none;
  transition: transform .18s ease, box-shadow .18s ease, opacity .18s ease;
}
.game-card-image,
.game-card-fallback { width: 100%; height: 100%; border-radius: inherit; pointer-events: none; }
.game-card-image { display: block; object-fit: cover; }
.game-card-fallback { position: relative; display: grid; place-items: center; overflow: hidden; }
.game-card.selected {
  z-index: 5;
  transform: translateY(-12px);
  border-color: var(--app-accent);
  box-shadow: 0 0 0 3px var(--app-accent), 0 14px 24px rgba(0, 0, 0, .32);
}
.game-card.is-disabled { opacity: .52; }
.game-card:focus-visible { outline: 3px solid var(--app-accent); outline-offset: 3px; }
.game-card-selected {
  position: absolute;
  top: -9px;
  left: 50%;
  display: inline-flex;
  min-height: 20px;
  transform: translateX(-50%);
  align-items: center;
  gap: 3px;
  border: 2px solid var(--app-surface-raised);
  border-radius: 999px;
  background: var(--app-accent);
  color: var(--app-on-accent);
  padding: 1px 6px;
  font-size: 9px;
  line-height: 1;
  white-space: nowrap;
  box-shadow: var(--app-shadow-sm);
}
.game-card-selected b { font-weight: 800; }
.card-interpretation-badge {
  position: absolute;
  top: 4px;
  right: 4px;
  max-width: calc(100% - 8px);
  overflow: hidden;
  border: 1px solid #b9d5e5;
  border-radius: 5px;
  background: rgba(20, 34, 45, .92);
  color: #dceefa;
  padding: 3px 5px;
  font-size: clamp(7px, 1vw, 10px);
  font-weight: 800;
  line-height: 1.2;
  text-overflow: ellipsis;
  white-space: nowrap;
  box-shadow: 0 2px 8px rgba(0, 0, 0, .32);
}
.card-level { position: absolute; top: 5px; left: 50%; transform: translateX(-50%); font-weight: 800; }
.card-element { display: grid; width: 38%; aspect-ratio: 1; place-items: center; border: 1px solid currentColor; border-radius: 50%; font-family: serif; font-size: 1.15rem; }
.game-card.element-Metal { --card-face: #ddd5b5; --card-face-light: #f3eed8; --card-border: #89783e; --card-ink: #67571e; }
.game-card.element-Wood { --card-face: #cfe0ce; --card-face-light: #e8f1e5; --card-border: #52765a; --card-ink: #315f3d; }
.game-card.element-Water { --card-face: #cbdfe8; --card-face-light: #e7f1f5; --card-border: #4d7890; --card-ink: #245d78; }
.game-card.element-Fire { --card-face: #ead0c9; --card-face-light: #f6e7e2; --card-border: #985448; --card-ink: #8d3026; }
.game-card.element-Earth { --card-face: #e4d5ba; --card-face-light: #f3ead8; --card-border: #936d3b; --card-ink: #785027; }
.game-card.hidden .game-card-fallback,
.game-card-preview-fallback {
  background: repeating-linear-gradient(45deg, #232e28, #232e28 6px, #344039 6px, #344039 12px);
  color: #ddc17f;
}
.game-card-preview-layer {
  position: fixed;
  inset: 0;
  z-index: 100;
  display: grid;
  place-items: center;
  background: rgba(8, 10, 9, .36);
  pointer-events: none;
  backdrop-filter: blur(2px);
}
.game-card-preview {
  position: relative;
  width: min(320px, 72vw);
  aspect-ratio: 627 / 949;
  border-radius: 15px;
  box-shadow: 0 28px 80px rgba(0, 0, 0, .62);
}
.game-card-preview img,
.game-card-preview-fallback { display: grid; width: 100%; height: 100%; place-items: center; border-radius: inherit; object-fit: cover; }
.game-card-preview .card-interpretation-badge { top: 10px; right: 10px; padding: 7px 9px; font-size: 13px; }

@media (max-width: 600px) {
  .game-card { width: 54px; }
  .card-interpretation-badge { max-width: calc(100% - 4px); top: 2px; right: 2px; padding: 2px 3px; }
}
</style>
