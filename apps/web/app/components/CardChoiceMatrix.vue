<template>
  <div
    :class="[
      'card-composition',
      'choice-card-matrix',
      { 'pouch-composition': !readonly },
    ]"
    role="group"
    :aria-label="label"
  >
    <table>
      <caption class="sr-only">{{ caption }}。列為五行，欄為等級。</caption>
      <thead>
        <tr>
          <th scope="col"><span class="sr-only">五行</span></th>
          <th v-for="level in CARD_LEVELS" :key="level" scope="col">
            {{ level }} 級
          </th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="row in composition" :key="row.element">
          <th scope="row">{{ row.element }}</th>
          <td
            v-for="cell in row.cells"
            :key="`${cell.element}-${cell.level}`"
            :class="{ empty: cell.count === 0 }"
          >
            <button
              v-if="!readonly && cell.count > 0"
              type="button"
              :disabled="cellDisabled(cell.cardIds)"
              :aria-pressed="selectedCount(cell.cardIds) > 0"
              :aria-label="cellLabel(cell.element, cell.level, cell.cardIds)"
              @click="selectCell(cell.cardIds)"
            >
              <strong aria-hidden="true">{{ cell.count }}</strong>
              <small v-if="selectedCount(cell.cardIds) > 0" aria-hidden="true">
                已選 {{ selectedCount(cell.cardIds) }}
              </small>
            </button>
            <strong v-else-if="readonly">{{ cell.count }}</strong>
            <strong v-else aria-hidden="true">0</strong>
          </td>
        </tr>
      </tbody>
    </table>
  </div>
</template>

<script setup lang="ts">
import type { CardInstanceId, PublicCard } from '~/types/fewfc'
import {
  buildCardComposition,
  cardForCompositionSelection,
  CARD_LEVELS,
  type CardCompositionSelectionMode,
} from '~/lib/card-composition'

const props = withDefaults(defineProps<{
  cards: PublicCard[]
  selectedCards?: CardInstanceId[]
  disabledCardIds?: CardInstanceId[]
  maximum?: number
  mode?: CardCompositionSelectionMode
  disabled?: boolean
  readonly?: boolean
  label: string
  caption: string
  actionLabel?: string
}>(), {
  selectedCards: () => [],
  disabledCardIds: () => [],
  maximum: 1,
  mode: 'replace',
  disabled: false,
  readonly: false,
  actionLabel: '選擇牌',
})

const emit = defineEmits<{
  choose: [card: CardInstanceId]
}>()

const composition = computed(() => buildCardComposition(props.cards))

function selectedCount(cardIds: CardInstanceId[]): number {
  return cardIds.filter(card => props.selectedCards.includes(card)).length
}

function cellDisabled(cardIds: CardInstanceId[]): boolean {
  return props.disabled
    || cardIds.every(card => props.disabledCardIds.includes(card))
    || (
    props.mode === 'toggle'
    && props.selectedCards.length >= props.maximum
    && selectedCount(cardIds) === 0
  )
}

function cellLabel(element: string, level: number, cardIds: CardInstanceId[]): string {
  const selected = selectedCount(cardIds)
  return `${props.actionLabel}：${element} ${level} 級，共 ${cardIds.length} 張，已選 ${selected} 張`
}

function selectCell(cardIds: CardInstanceId[]) {
  if (props.readonly) return
  const card = cardForCompositionSelection(
    cardIds,
    props.selectedCards,
    props.maximum,
    props.mode,
  )
  if (card !== undefined) emit('choose', card)
}
</script>

<style scoped>
@reference "../assets/css/main.css";

.card-composition table { @apply w-full table-fixed border-collapse; }
.card-composition th, .card-composition td { @apply h-8 border border-[var(--app-border)] text-center; }
.card-composition thead th { @apply text-[10px] font-bold text-[var(--app-text)]; }
.card-composition tbody th { @apply w-7 text-[10px] font-normal text-muted; }
.card-composition td strong { @apply font-serif text-sm text-[#e4c47d]; }
.card-composition td.empty strong { color: var(--app-text-soft); }
.pouch-composition { @apply mx-auto mt-5 w-[min(390px,calc(100vw-64px))] border border-[var(--app-accent)] bg-[var(--app-surface-raised)] p-3.5 text-[var(--app-text)] shadow-[0_18px_48px_rgba(0,0,0,.52)]; }
.pouch-composition td { @apply p-0; }
.pouch-composition td button { @apply grid size-full min-h-8 place-items-center border-0 bg-transparent text-[#e4c47d] hover:bg-[rgba(185,149,80,.16)] disabled:cursor-not-allowed disabled:opacity-45; }
.pouch-composition td button[aria-pressed="true"] { @apply bg-[rgba(185,149,80,.3)] shadow-[inset_0_0_0_2px_#d1ad62]; }
.choice-card-matrix td button small { @apply text-[8px] font-normal text-[#f0d99e]; }
</style>
